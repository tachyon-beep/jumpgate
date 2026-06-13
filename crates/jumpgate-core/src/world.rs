//! World aggregate: owns all stores + ephemeris + rng + logs, drives the tick.
use crate::autopilot::autopilot_command;
use crate::config::{ConfigHash, RunConfig};
use crate::contract::{Command, Event, EventKind, Integrator, StateView};
use crate::ephemeris::Ephemeris;
use crate::events::EventStream;
use crate::ids::{BodyId, ContractId, CraftId, SlotMap, StationId};
use crate::ingest::ActionLog;
use crate::integrator::{VelocityVerlet, gravity_accel, substep_count};
use crate::math::Vec3;
use crate::rng::RngStreams;
use crate::ship::thrust_accel_and_burn;
use crate::stores::{BodyStore, CraftStore, NavState, effective_params};
use crate::time::{Dt, Tick};
use crate::types::{EntityRef, Lod, NavDest};

/// Per-directed-route robbery evidence rings (the media seam's v0 storage,
/// spec §7): for each directed station route (dense row-major `n_stations²`,
/// route index = `from_row * n_stations + to_row`), the last 8 rob ticks and a
/// ring cursor. World-level HASHED state (HASH_FIELD_ORDER word 29, format v4):
/// written at robbery settlement (`pirate::resolve_encounters`), read
/// dock-gated through `info_tick` (`World::route_evidence`). Sized ONCE at
/// reset from the station count (no mid-run station spawn in v1), so the
/// length is transitively pinned by config_hash.
pub struct RouteEvidence {
    /// Ring of the last 8 rob ticks per directed route. Zero-init `Tick(0)` ==
    /// "no recorded rob" (older than any evidence window once past warm-up).
    pub robs: Vec<[Tick; 8]>,
    /// Next write slot per ring (0..8), length-parallel with `robs`.
    pub cursor: Vec<u8>,
}

impl RouteEvidence {
    /// Count of ring entries on `route` inside the half-open read window
    /// `(info_tick - window, info_tick]` — EVIDENCE ONLY, no decay arithmetic:
    /// staleness is a property of the READ, not the store (spec §7). The
    /// zero-init `Tick(0)` sentinel can never match (robs settle at tick >= 1
    /// and the lower bound is exclusive with a saturating subtraction).
    /// Out-of-range routes read 0 (spec §8 totality).
    pub fn count_recent(&self, route: usize, info_tick: Tick, window: u64) -> u32 {
        let Some(ring) = self.robs.get(route) else {
            return 0;
        };
        let lo = info_tick.0.saturating_sub(window);
        ring.iter().filter(|t| t.0 > lo && t.0 <= info_tick.0).count() as u32
    }
}

/// The authoritative simulation aggregate. Single writer; all facades read via `StateView`.
pub struct World {
    // `ships`/`bodies` are pub(crate) so the per-tick state hash (hash.rs, a later
    // task) can fold their canonical SoA arrays in directly. Everything else stays
    // private behind StateView / narrow mutators.
    pub(crate) ships: CraftStore,
    pub(crate) bodies: BodyStore,
    // Economy stores (pub(crate) so the per-tick state hash folds their SoA arrays
    // directly, like ships/bodies). Minted EMPTY here; `World::reset` populates them
    // from `RunConfig` in a later task. `econ` is the audited flow counters.
    pub(crate) stations: crate::economy::StationStore,
    pub(crate) producers: crate::economy::ProducerStore,
    pub(crate) corporations: crate::economy::CorporationStore,
    pub(crate) contracts: crate::economy::ContractStore,
    pub(crate) econ: crate::economy::EconCounters,
    /// World-level robbery-evidence rings (pub(crate) so the per-tick state
    /// hash folds them directly, like the stores above).
    pub(crate) route_evidence: RouteEvidence,
    /// Per-station gossip reservoirs (media rung cut 1, spec §2): length
    /// `n_stations` when media-live, empty otherwise. Sized ONCE at reset (the
    /// RouteEvidence sizing law). HASHED (HASH_FIELD_ORDER word 31).
    pub(crate) station_gossip: Vec<crate::media::GossipBuffer>,
    /// World mint counter for `GossipAlert.alert_seq` — identity/dedup/
    /// eviction tie-break/lab join key. HASHED (HASH_FIELD_ORDER word 32).
    pub(crate) next_alert_seq: u32,
    /// UNHASHED media diagnostics (the `engagement_diag` pattern — never a
    /// behavior input): eviction count + dock-edge contact records. Written by
    /// the stage-3b2 media mechanics and read ONLY by instruments.
    pub(crate) media_diag: crate::media::MediaDiag,
    /// UNHASHED ASSIGN instrumentation (the WHY-panel windows: count
    /// histogram + gossip-vs-ring argmax-flip share). Never a behavior input.
    pub(crate) assign_diag: crate::economy::AssignDiag,
    /// Per-engagement kinematic snapshots, pushed by the stage-3b2 emission
    /// sites and read ONLY by the diagnostics sampler (`sample_window`).
    /// UNHASHED diagnostics-only state — never an input to any behavior stage
    /// (see `pirate::EngagementSnapshot`).
    pub(crate) engagement_diag: Vec<crate::pirate::EngagementSnapshot>,
    /// UNHASHED fuel diagnostics (world-gets-big phase 0b): per-craft duty,
    /// burn, low-water, and contract-leg burn brackets. Read only by
    /// `sample_window`; no behavior stage reads this field.
    pub(crate) fuel_diag: crate::diagnostics::FuelDiag,
    eph: Ephemeris,
    rng: RngStreams,
    log: ActionLog,
    events: EventStream,
    tick: Tick,
    dt: Dt,
    config: RunConfig,
}

/// Read filter applied at the single projection seam (`project`). v1: all-visible.
pub trait Observer {
    fn visible(&self, target: EntityRef) -> bool;
}
/// v1 default observer: everything is visible.
pub struct FullObserver;
impl Observer for FullObserver {
    fn visible(&self, _target: EntityRef) -> bool {
        true
    }
}

/// Projected, presence-masked snapshot the obs layer reads.
/// Each craft row: (id, pos, vel, fuel_mass, fuel_capacity). Accessor methods below
/// are the contract the obs layer (Task 16 `write_obs_frame_relative`) reads through.
pub struct View {
    pub tick: Tick,
    pub craft: Vec<(CraftId, Vec3, Vec3, f64, f64)>,
    /// (id, pos) for each visible body at `tick`, in sorted-id order.
    pub bodies: Vec<(BodyId, Vec3)>,
}

impl View {
    fn craft_row(&self, id: CraftId) -> Option<&(CraftId, Vec3, Vec3, f64, f64)> {
        self.craft.iter().find(|r| r.0 == id)
    }
    pub fn craft_pos(&self, id: CraftId) -> Option<Vec3> {
        self.craft_row(id).map(|r| r.1)
    }
    pub fn craft_vel(&self, id: CraftId) -> Option<Vec3> {
        self.craft_row(id).map(|r| r.2)
    }
    pub fn craft_fuel(&self, id: CraftId) -> Option<f64> {
        self.craft_row(id).map(|r| r.3)
    }
    pub fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
        self.craft_row(id).map(|r| r.4)
    }
    pub fn body_pos(&self, id: BodyId) -> Option<Vec3> {
        self.bodies.iter().find(|r| r.0 == id).map(|r| r.1)
    }
}

/// A `RunConfig` rejected by `World::reset`'s resolvability guard (§6). Part of
/// the recorded contract surface (replay calls `reset` and asserts its hash), so
/// it is re-exported from `lib.rs` for the gym/FFI layer to match on.
#[derive(Clone, Debug, PartialEq)]
pub enum ResetError {
    /// Craft `craft_index`'s worst-case (empty-tank) braking cannot resolve the
    /// arrival sphere at this `dt`: `a_max_empty * dt^2 >= limit` (limit = R/(2·k_brake)).
    Unbrakable { craft_index: usize, a_max_empty: f64, dt: f64, limit: f64 },
    /// An economy init vec referenced an out-of-range index (`what` names the
    /// reference kind, e.g. `"station.body_index"`; `index` is the bad value).
    /// Validated before tick 0 so a malformed economy config never mints a
    /// half-populated, SoA-misaligned world.
    BadEconomyRef { what: &'static str, index: usize },
    /// A half-on `MediaCfg`: exactly one of the two gossip slot caps is > 0.
    /// The media-live predicate requires BOTH caps live (dual gate, spec §11);
    /// a half-on config is a misconfiguration, rejected before tick 0.
    BadMediaCfg { reason: &'static str },
    /// A half-on `RefuelCfg`: `lot_mass > 0` while the Fuel price surface is
    /// structurally dead (`price_cfg.base_micros[Fuel] == 0`) or any station's
    /// seeded `initial_price_micros[Fuel] == 0`, or while the Port corp index
    /// cannot resolve. Refuel settlement is a four-legged transfer, so reject
    /// before tick 0 rather than minting a zero-price or one-legged world.
    BadRefuelCfg { reason: &'static str },
    /// The `RunConfig.goods` table is invalid: either zero goods, or a station's
    /// `initial_stock` / `initial_price_micros` Vec length does not equal
    /// `n_goods`.  Rejected before tick 0.
    BadGoodsCfg { reason: &'static str },
}

impl std::fmt::Display for ResetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResetError::Unbrakable { craft_index, a_max_empty, dt, limit } => write!(
                f,
                "craft {craft_index} is unbrakable: a_max_empty*dt^2 = {} >= R/(2*k_brake) = {limit} \
                 (a_max_empty={a_max_empty}, dt={dt}); remedy: lower max_thrust, raise dry_mass, or shrink dt",
                a_max_empty * dt * dt
            ),
            ResetError::BadEconomyRef { what, index } => write!(
                f,
                "economy config {what} references out-of-range index {index}"
            ),
            ResetError::BadMediaCfg { reason } => {
                write!(f, "bad media config: {reason}")
            }
            ResetError::BadRefuelCfg { reason } => {
                write!(f, "bad refuel config: {reason}")
            }
            ResetError::BadGoodsCfg { reason } => {
                write!(f, "bad goods config: {reason}")
            }
        }
    }
}

impl std::error::Error for ResetError {}

impl World {
    /// Build a World from a RunConfig: precompute ephemeris, seed rng from the
    /// master seed, spawn bodies then craft, and return the config hash.
    /// `seed` and `dt` come from `cfg`; nothing is read from the environment.
    pub fn reset(cfg: RunConfig) -> Result<(World, ConfigHash), ResetError> {
        let hash = cfg.config_hash();
        // Validate goods table (A1b): reject zero-goods configs and length mismatches
        // before minting any stores.
        let n_goods = cfg.goods.goods.len();
        if n_goods == 0 {
            return Err(ResetError::BadGoodsCfg { reason: "GoodsCfg has zero goods" });
        }
        for (si, s) in cfg.stations.iter().enumerate() {
            if s.initial_stock.len() != n_goods {
                return Err(ResetError::BadGoodsCfg {
                    reason: "station initial_stock length != n_goods",
                });
            }
            if s.initial_price_micros.len() != n_goods {
                return Err(ResetError::BadGoodsCfg {
                    reason: "station initial_price_micros length != n_goods",
                });
            }
            let _ = si; // si used in error messages if the & str is expanded later
        }
        if cfg.price_cfg.base_micros.len() != n_goods || cfg.price_cfg.cap.len() != n_goods {
            return Err(ResetError::BadGoodsCfg {
                reason: "PriceCfg base_micros or cap length != n_goods",
            });
        }
        // §6 resolvability guard: reject any craft whose worst-case (empty-tank)
        // braking would tunnel the arrival sphere at this dt. Reads only inputs
        // already in config_hash (dt, base_max_thrust, base_dry_mass, guidance.k_brake),
        // runs before tick 0, persists no state -> determinism-neutral.
        let dt = cfg.dt.get();
        let limit = crate::autopilot::ARRIVAL_RADIUS / (2.0 * cfg.guidance.k_brake);
        // TODO(forward-debt, Person+Ship / spec §6.5): this reads the BASE max_thrust,
        // but the Task-3 autopilot backstop reads the EFFECTIVE a_max. When EffectiveMods
        // lands and multiplies max_thrust, a crew-boosted craft could pass here on base
        // values yet violate the runtime backstop. The Person line resolves `mods` at reset
        // BEFORE this guard; honour that ordering and read
        // effective_params(&c.spec, &reset_mods).max_thrust here. Identity in v1 (no mods yet).
        for (i, c) in cfg.craft.iter().enumerate() {
            let dry = c.spec.base_dry_mass;
            let a_max_empty = c.spec.base_max_thrust / dry;
            if !(dry > 0.0 && a_max_empty.is_finite() && a_max_empty * dt * dt < limit) {
                return Err(ResetError::Unbrakable { craft_index: i, a_max_empty, dt, limit });
            }
        }
        // Media half-on validation (BEFORE minting, like BadEconomyRef): the
        // media-live predicate requires BOTH gossip slot caps live together
        // (dual gate, spec §11) — exactly one cap > 0 is a misconfiguration.
        if (cfg.media.station_gossip_slots > 0) != (cfg.media.craft_gossip_slots > 0) {
            return Err(ResetError::BadMediaCfg {
                reason: "half-on media: both gossip slot caps must be > 0 together",
            });
        }
        // Refuel half-on validation (BEFORE minting, like BadMediaCfg): once
        // `lot_mass` opens the verb, settlement must have a config-wide live
        // Fuel price surface and a resolvable Port corporation row.
        if cfg.refuel.lot_mass > 0.0 {
            let fuel = crate::economy::Good::FUEL.index();
            if cfg.price_cfg.base_micros.get(fuel).copied().unwrap_or(0) == 0 {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but price_cfg.base_micros[Fuel] == 0",
                });
            }
            if cfg.stations.iter().any(|s| s.initial_price_micros.get(fuel).copied().unwrap_or(0) == 0) {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but a station's seeded initial_price_micros[Fuel] == 0",
                });
            }
            if (cfg.refuel.corp_index as usize) >= cfg.corporations.len() {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 requires refuel.corp_index to name an existing corporation",
                });
            }
        }
        // The media-live dual gate (config caps AND the trophic inert lever):
        // decides whether reset mints gossip buffers at all.
        let media_live = cfg.media.caps_live() && cfg.trophic.engage_radius_au > 0.0;
        // Ephemeris::precompute (Task 9) must yield a FINITE position for an a==0.0
        // conic: a central star sits at the origin for all ticks (no NaN from a 0/0
        // mean-anomaly solve). The Task 7 gravity_accel softening (r^2 + eps^2)^1.5
        // then keeps accel finite even when a craft coincides with the star.
        let eph = Ephemeris::precompute(&cfg.bodies, cfg.dt, cfg.ephemeris_window);

        let mut bodies = BodyStore {
            ids: SlotMap::new(),
            mass: Vec::new(),
            eph_index: Vec::new(),
        };
        for (i, b) in cfg.bodies.iter().enumerate() {
            bodies.ids.insert(());
            bodies.mass.push(b.mass);
            bodies.eph_index.push(i);
        }

        let mut ships = CraftStore {
            ids: SlotMap::new(),
            pos: Vec::new(),
            vel: Vec::new(),
            fuel_mass: Vec::new(),
            spec: Vec::new(),
            nav: Vec::new(),
            lod: Vec::new(),
            prev_fuel: Vec::new(),
            prev_inside_dest: Vec::new(),
            prev_pos: Vec::new(),
            mods: Vec::new(),
            role: Vec::new(),
            cargo: Vec::new(),
            credits_micros: Vec::new(),
            contract: Vec::new(),
            risk_appetite: Vec::new(),
            pirate: Vec::new(),
            upgrades: Vec::new(),
            info_tick: Vec::new(),
            pending_upgrade: Vec::new(),
            pending_refuel: Vec::new(),
            gossip: Vec::new(),
            hold: Vec::new(),
        };
        for c in cfg.craft.iter() {
            ships.ids.insert(());
            ships.pos.push(c.pos);
            ships.vel.push(c.vel);
            ships.fuel_mass.push(c.fuel_mass);
            ships.spec.push(c.spec.clone());
            ships.nav.push(NavState::Idle);
            ships.lod.push(Lod::Player);
            // Boundary-edge previous state: at tick 0 prev == current, so no spurious
            // FuelEmpty/Arrival fires on the first step (edge detection needs a prior).
            ships.prev_fuel.push(c.fuel_mass);
            ships.prev_inside_dest.push(false);
            // Swept-arrival chord start: prev_pos == pos at tick 0 (zero-length
            // chord), so no spurious Arrival clip on the first step.
            ships.prev_pos.push(c.pos);
            ships.mods.push(crate::stores::EffectiveMods::IDENTITY);
            // Hauler economy columns: every craft starts Idle, empty-handed, broke,
            // uncontracted (the loop builds the SoA manually; keep all columns
            // length-parallel or the state hash's dense-row unwrap panics).
            ships.role.push(c.role);
            ships.cargo.push(None);
            ships.credits_micros.push(0);
            ships.contract.push(None);
            // Trophic columns: default 0 risk-appetite; Pirate-role rows mint a
            // PirateState (grubstake food, zero notoriety, immediately active).
            ships.risk_appetite.push(0);
            ships.pirate.push(match c.role {
                crate::stores::CraftRole::Pirate => Some(crate::stores::PirateState {
                    food_micros: cfg.trophic.grubstake_micros,
                    notoriety: 0,
                    lie_low_until: Tick(0),
                    engage_cooldown_until: Tick(0),
                }),
                _ => None,
            });
            // Pirates-rung (v4) columns: empty fleet ledger, tick-0 info
            // freshness, no pending purchase intent (transient, never hashed).
            ships.upgrades.push(crate::stores::UpgradeLevels::default());
            ships.info_tick.push(Tick(0));
            ships.pending_upgrade.push(None);
            ships.pending_refuel.push(None);
            // Media column (v5): a comms-log for non-pirate craft on a
            // media-live world; pirates are information-blind by construction
            // (spec §16 OD-6) — `None`, like every row when media is off.
            ships.gossip.push(if media_live && c.role != crate::stores::CraftRole::Pirate {
                Some(crate::media::GossipBuffer::empty(cfg.media.craft_gossip_slots))
            } else {
                None
            });
            // Goods-rung hold (v6): empty for all craft including pirates.
            ships.hold.push(Vec::new());
        }

        // Mint economy stores from the RunConfig init vecs. Dependency order:
        // stations (resolve body_index) -> producers/corporations (resolve
        // station_index) -> contracts (resolve corp_index + from/to station). Every
        // index is validated against the already-minted slot-maps; an out-of-range
        // ref aborts BEFORE tick 0 with BadEconomyRef (no half-populated SoA). The
        // minted ids are dense (slot == row, no despawn), so id_at(index) maps a
        // config index straight to the live id.
        let mut stations = crate::economy::StationStore::empty();
        let mut station_ids: Vec<crate::ids::StationId> = Vec::with_capacity(cfg.stations.len());
        for s in cfg.stations.iter() {
            let (slot, generation) = bodies.ids.id_at(s.body_index).ok_or(
                ResetError::BadEconomyRef { what: "station.body_index", index: s.body_index },
            )?;
            let body = BodyId { slot, generation };
            station_ids.push(stations.push(body, s.initial_stock.clone(), s.initial_price_micros.clone()));
        }

        let mut producers = crate::economy::ProducerStore::empty();
        for p in cfg.producers.iter() {
            let station = *station_ids.get(p.station_index).ok_or(
                ResetError::BadEconomyRef { what: "producer.station_index", index: p.station_index },
            )?;
            producers.push(station, p.recipe);
        }

        let mut corporations = crate::economy::CorporationStore::empty();
        let mut corp_ids: Vec<crate::ids::CorporationId> =
            Vec::with_capacity(cfg.corporations.len());
        for c in cfg.corporations.iter() {
            let home_station = *station_ids.get(c.home_station_index).ok_or(
                ResetError::BadEconomyRef {
                    what: "corporation.home_station_index",
                    index: c.home_station_index,
                },
            )?;
            corp_ids.push(corporations.push(c.treasury_micros, home_station));
        }

        let mut contracts = crate::economy::ContractStore::empty();
        for k in cfg.contracts.iter() {
            let corp = *corp_ids.get(k.corp_index).ok_or(ResetError::BadEconomyRef {
                what: "contract.corp_index",
                index: k.corp_index,
            })?;
            let from_station = *station_ids.get(k.from_station_index).ok_or(
                ResetError::BadEconomyRef {
                    what: "contract.from_station_index",
                    index: k.from_station_index,
                },
            )?;
            let to_station = *station_ids.get(k.to_station_index).ok_or(
                ResetError::BadEconomyRef {
                    what: "contract.to_station_index",
                    index: k.to_station_index,
                },
            )?;
            // status Offered (ContractStore::push seeds it), escrow 0, no hauler.
            contracts.push(corp, k.resource, k.qty, from_station, to_station, k.reward_micros);
        }

        // Route-evidence rings: dense n_stations² directed routes, sized once
        // here (no mid-run station spawn in v1). Saturating per the spec §8
        // totality discipline — an absurd station count degrades, never panics.
        let n_routes = stations.ids.len().saturating_mul(stations.ids.len());
        let route_evidence = RouteEvidence {
            robs: vec![[Tick(0); 8]; n_routes],
            cursor: vec![0u8; n_routes],
        };

        // Station gossip reservoirs: one per station when media-live, sized
        // ONCE here (the RouteEvidence sizing law — no mid-run station spawn
        // in v1); empty Vec when media is off.
        let station_gossip = if media_live {
            vec![
                crate::media::GossipBuffer::empty(cfg.media.station_gossip_slots);
                stations.ids.len()
            ]
        } else {
            Vec::new()
        };

        let mut rng = RngStreams::from_master(cfg.master_seed);
        // Initial lurk assignment (spec §5): drawn from the Piracy stream AT
        // RESET — never config-fixed (a fixed pirate→station map would let a
        // gym policy memorize geography instead of reading contacts). Dense
        // pirate-row order, uniform over ALL stations (the initial scatter;
        // later relocation is reach-bounded). Scripted stage: skips
        // `!scripted` craft. Gated by the spec-§8 inert lever, so existing
        // pirate-free / inert worlds consume no draws and stay bit-identical.
        // The advanced stream cursor is Class-3 transitively-pinned state
        // (replay rebuilds from reset + log; see hash.rs).
        if cfg.trophic.engage_radius_au > 0.0 && !station_ids.is_empty() {
            use rand_core::Rng;
            for row in 0..ships.ids.len() {
                if ships.role[row] != crate::stores::CraftRole::Pirate {
                    continue;
                }
                if cfg.craft.get(row).is_some_and(|c| !c.scripted) {
                    continue;
                }
                let u = rng.stream(crate::rng::RngStream::Piracy).next_u64();
                // The haven station (hideout body) is excluded from the
                // scatter — a pirate does not lurk where it fences (the
                // seed-23 ghetto lesson; see pirate.rs::relocate_lurk_target).
                let haven: Option<usize> = bodies
                    .ids
                    .id_at(cfg.trophic.hideout_body_index as usize)
                    .map(|(slot, generation)| crate::ids::BodyId { slot, generation })
                    .and_then(|hb| (0..station_ids.len()).find(|&s| stations.body[s] == hb));
                let candidates: Vec<usize> =
                    (0..station_ids.len()).filter(|&s| Some(s) != haven).collect();
                let Some(&srow) = candidates.get((u % candidates.len().max(1) as u64) as usize)
                else {
                    continue; // haven-only world: nowhere huntable to scatter
                };
                let body = stations.body[srow];
                let eff = effective_params(&ships.spec[row], &ships.mods[row]);
                let dv = crate::math::tsiolkovsky_dv(
                    eff.exhaust_velocity,
                    eff.dry_mass,
                    ships.fuel_mass[row],
                );
                ships.nav[row] = NavState::Seeking {
                    dest: NavDest::Entity(EntityRef::Body(body)),
                    dv_remaining: dv,
                };
            }
        }
        let fuel_diag = crate::diagnostics::FuelDiag {
            thrust_ticks: vec![0; ships.fuel_mass.len()],
            burned_mass: vec![0.0; ships.fuel_mass.len()],
            min_fuel_mass: ships.fuel_mass.clone(),
            leg_start_fuel: vec![None; ships.fuel_mass.len()],
            leg_burns: Vec::new(),
        };
        let dt = cfg.dt;
        let world = World {
            ships,
            bodies,
            stations,
            producers,
            corporations,
            contracts,
            econ: crate::economy::EconCounters::zero(n_goods),
            route_evidence,
            station_gossip,
            next_alert_seq: 0,
            media_diag: Default::default(),
            assign_diag: Default::default(),
            engagement_diag: Vec::new(),
            fuel_diag,
            eph,
            rng,
            log: ActionLog {
                entries: Vec::new(),
                commands_flat: Vec::new(),
                config_hash: hash,
            },
            events: EventStream { events: Vec::new() },
            tick: Tick(0),
            dt,
            config: cfg,
        };
        Ok((world, hash))
    }

    // --- narrow mutators the single ingestion path writes through (Step 2) ---
    pub(crate) fn log_mut(&mut self) -> &mut ActionLog {
        &mut self.log
    }
    pub(crate) fn events_mut(&mut self) -> &mut EventStream {
        &mut self.events
    }
    pub(crate) fn set_nav(&mut self, id: CraftId, nav: NavState) {
        if let Some(i) = self.ship_index(id) {
            self.ships.nav[i] = nav;
        }
    }

    fn ship_index(&self, id: CraftId) -> Option<usize> {
        self.ships.ids.dense_index(id.slot, id.generation)
    }

    /// Fuel-derived Δv budget for a live craft (D9): `tsiolkovsky_dv` over effective
    /// params + current fuel. `0.0` for a stale id. The single source the live ingest
    /// path uses when no explicit `burn_budget` is given.
    pub(crate) fn dv_from_fuel_for(&self, id: CraftId) -> f64 {
        match self.ship_index(id) {
            Some(i) => {
                let eff = effective_params(&self.ships.spec[i], &self.ships.mods[i]);
                crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, self.ships.fuel_mass[i])
            }
            None => 0.0,
        }
    }
    fn body_index(&self, id: BodyId) -> Option<usize> {
        self.bodies.ids.dense_index(id.slot, id.generation)
    }
    fn craft_id_at(&self, dense_index: usize) -> CraftId {
        // SlotMap::id_at returns Option; delegate to the CraftStore wrapper
        // `ids_at`, which does the `expect` internally and returns CraftId.
        self.ships.ids_at(dense_index)
    }

    // --- read-only trader accessors (the strategic layer's board/wallet reads) ---
    // Plain reads over already-hashed economy state: no store layout, fold-order, or
    // stepping change — hash- and determinism-neutral by construction.

    /// `Offered` + unclaimed contracts (the strategic board), dense row order.
    /// A row is OFF the board if its status is not `Offered`, OR a hauler is
    /// bound, OR any craft holds accept-INTENT for it (the ingest ACCEPT path
    /// records intent on `ships.contract` one stage before `resolve_contracts`
    /// binds the contract side).
    pub fn offered_contracts(&self) -> Vec<(ContractId, i64, StationId, StationId)> {
        use crate::economy::ContractStatus;
        (0..self.contracts.ids.len())
            .filter_map(|k| {
                if self.contracts.status[k] != ContractStatus::Offered
                    || self.contracts.hauler[k].is_some()
                {
                    return None;
                }
                let (slot, generation) = self.contracts.ids.id_at(k)?;
                let cid = ContractId { slot, generation };
                let intent_claimed =
                    (0..self.ships.ids.len()).any(|r| self.ships.contract[r] == Some(cid));
                if intent_claimed {
                    return None;
                }
                Some((
                    cid,
                    self.contracts.reward_micros[k],
                    self.contracts.from_station[k],
                    self.contracts.to_station[k],
                ))
            })
            .collect()
    }

    /// The station's body position at the CURRENT tick (orbits move), or `None`
    /// for a stale/unknown station id.
    pub fn station_pos(&self, id: StationId) -> Option<Vec3> {
        let row = self.stations.ids.dense_index(id.slot, id.generation)?;
        let body = self.stations.body[row];
        let brow = self.bodies.ids.dense_index(body.slot, body.generation)?;
        Some(self.eph.body_pos(self.bodies.eph_index[brow], self.tick))
    }

    /// Earned credits (microcredits) of a live craft, or `None` for a stale id.
    pub fn craft_credits(&self, id: CraftId) -> Option<i64> {
        self.ship_index(id).map(|i| self.ships.credits_micros[i])
    }

    /// Role of a live craft (the chronicle epilogue's read — world-gets-big
    /// spec §7), or `None` for a stale id. Plain read over already-hashed
    /// state (the trader-accessor pattern): no layout, fold-order, or
    /// stepping change.
    pub fn craft_role(&self, id: CraftId) -> Option<crate::stores::CraftRole> {
        self.ship_index(id).map(|i| self.ships.role[i])
    }

    /// Route-evidence read (media rung spec §7) — the propagation model now
    /// lives BEHIND this unchanged signature (the spec-§7 promise kept: the
    /// accessor took the READER, not a timestamp, precisely so this swap
    /// would touch no call site).
    ///
    /// MEDIA-LIVE (`media_live()`): the RAW count of the reader's OWN
    /// comms-log alerts on the directed `route` (dense row-major
    /// `from_row * n_stations + to_row`) still inside its window — staleness
    /// anchors on each copy's `first_heard` at THIS reader (the per-reader
    /// forgetting clock; the return to a cooled route staggers by lived
    /// docking rhythms). Unweighted: valence stays in the consumer
    /// (PDR-0006). Pirate rows hold no buffer and read 0 (information-blind).
    ///
    /// MEDIA-OFF: the legacy ring read, byte-identical — recorded robs inside
    /// `(info_tick - evidence_window, info_tick]` where `info_tick` is the
    /// reader's own last-dock tick. The ring keeps being written either way
    /// (the media-off fallback; retirement = cut 2, OD-2), and `info_tick`
    /// keeps refreshing (it is the dock detector); its evidence-read role
    /// ends when media is live. Stale readers and out-of-range routes read 0
    /// (spec §8).
    pub fn route_evidence(&self, reader: CraftId, route: usize) -> u32 {
        let Some(crow) = self.ship_index(reader) else {
            return 0;
        };
        if self.media_live() {
            self.ships.gossip[crow].as_ref().map_or(0, |buf| {
                buf.count_route_recent(
                    route,
                    self.tick,
                    self.config.trophic.evidence_window,
                    self.config.media.staleness_from_rob_tick,
                )
            })
        } else {
            self.route_evidence.count_recent(
                route,
                self.ships.info_tick[crow],
                self.config.trophic.evidence_window,
            )
        }
    }

    /// Shipyard config (the Yard's corp index) for the diagnostics sampler's
    /// treasury read (`diagnostics::sample_window`, the §9 Yard-circulation
    /// panel). Plain read over already-hashed config — never a behavior input.
    pub(crate) fn shipyard_cfg(&self) -> &crate::config::ShipyardCfg {
        &self.config.shipyard
    }

    /// Trophic config for the diagnostics sampler (the `shipyard_cfg`
    /// precedent — `config` is private): the hideout index and engage radius
    /// feed the pirate-location partition read. Plain read over already-hashed
    /// config — never a behavior input.
    pub(crate) fn trophic_cfg(&self) -> &crate::config::TrophicCfg {
        &self.config.trophic
    }

    /// The media-live dual gate (spec §11): BOTH gossip slot caps > 0 AND the
    /// trophic inert lever open (`engage_radius_au > 0`). Pure read over
    /// already-hashed config — pub so tests (and instruments) can read it.
    pub fn media_live(&self) -> bool {
        self.config.media.caps_live() && self.config.trophic.engage_radius_au > 0.0
    }

    /// Idle == available for a strategic decision: role `Idle` AND no bound (or
    /// intended) contract. `None` for a stale id.
    pub fn craft_is_idle(&self, id: CraftId) -> Option<bool> {
        self.ship_index(id).map(|i| {
            self.ships.role[i] == crate::stores::CraftRole::Idle
                && self.ships.contract[i].is_none()
        })
    }

    /// Pirate contacts for the gym obs (pirates rung spec §11): every live
    /// Pirate-role craft except the observer, as RAW evidence
    /// `(rel_pos, rel_vel, strength, active)` — positions and capability
    /// magnitude relative to the observer, plus lying-low visibility
    /// (`active` = off lie-low at the current tick). NEVER `route_evidence`
    /// counts or any derived score (the agent derives route danger from
    /// contacts + geometry). Sorted by distance ascending, ties to the lower
    /// dense row (stable sort over dense-row order — deterministic). Plain
    /// read over already-hashed state (the trader-accessor pattern): no
    /// layout, fold-order, or stepping change. Empty for a stale observer.
    pub fn pirate_contacts(&self, observer: CraftId) -> Vec<(Vec3, Vec3, u32, bool)> {
        let Some(orow) = self.ship_index(observer) else {
            return Vec::new();
        };
        let opos = self.ships.pos[orow];
        let ovel = self.ships.vel[orow];
        let mut rows: Vec<(f64, usize)> = Vec::new();
        for row in 0..self.ships.ids.len() {
            if row == orow || self.ships.role[row] != crate::stores::CraftRole::Pirate {
                continue;
            }
            rows.push((self.ships.pos[row].sub(opos).length(), row));
        }
        // f64 distances here are finite by construction (physics state);
        // total_cmp keeps the sort deterministic without an unwrap.
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        rows.into_iter()
            .map(|(_, row)| {
                let active = self.ships.pirate[row]
                    .is_some_and(|ps| self.tick >= ps.lie_low_until);
                let s = crate::pirate::strength(
                    self.ships.role[row],
                    self.ships.upgrades[row],
                    &self.config.trophic,
                );
                (
                    self.ships.pos[row].sub(opos),
                    self.ships.vel[row].sub(ovel),
                    s as u32,
                    active,
                )
            })
            .collect()
    }

    /// Advance one tick. `dt` is owned by the World, never an argument.
    /// (1) ingest commands canonically, (2) Lod-dispatch: skip physics for dormant
    /// (`Lod::Nothing`) craft and emit `Wake` on dormant->active, integrate the rest,
    /// (3) detect boundary events against the new quantized state, (4) copy-forward
    /// the boundary-edge arrays, (5) tick++.
    pub fn step(&mut self, cmds: &mut Vec<Command>) {
        let cur = self.tick;
        let dt = self.dt.get();
        let next = Tick(cur.0 + 1);

        // (1) single ingestion path (Step 2): sorts canonically, resolves NavDest
        //     into NavState, logs, emits ActionIngested.
        crate::ingest::ingest_commands(self, cur, cmds);

        // (1b) economy production stage: fire eligible producers BEFORE physics.
        //      `next` is the firing tick (consistent with event tick stamping).
        crate::economy::run_producers(
            &mut self.stations,
            &mut self.producers,
            &mut self.econ,
            next,
            &mut self.events,
        );

        // (1b2) scripted dispatch + repost (stage 7, Task 17): repost demanded routes
        //       and assign Idle haulers so the Stage-1 loop SELF-RUNS with no external
        //       commands. Placed AFTER run_producers (so demand reflects this tick's
        //       production) and BEFORE resolve_contracts (so a same-tick accept is
        //       escrowed/loaded). Identity-neutral: touches no counter/stock/cargo.
        // Media-live flips the ASSIGN evidence read from the legacy ring to
        // each hauler's own comms-log (media spec §7, Task 7). Config is
        // reset-pinned, so this is constant for a run (hoisted: a pure config
        // read, before the disjoint &mut field borrows below).
        let media_live = self.media_live();
        crate::economy::run_scripted_dispatch(
            &mut self.contracts,
            &self.stations,
            &mut self.ships,
            &self.config.craft,
            &self.route_evidence,
            media_live,
            self.config.media.staleness_from_rob_tick,
            &mut self.assign_diag,
            &self.config.dispatch_cfg,
            &self.config.shipyard,
            &self.config.trophic,
            next,
            &mut self.events,
        );

        // (1c) economy contract stage: drive the accept/escrow/load/dispatch
        //      lifecycle for haulers bound by this tick's ingest. Disjoint &mut
        //      field borrows + read-only bodies/eph (mirrors run_producers); `next`
        //      is the resolution/event tick. Runs PRE-physics: ships.pos is still the
        //      tick-`cur` state, so try_load's co-location gate samples body_pos at
        //      `next - 1 == cur` — the same frame (see try_load).
        crate::economy::resolve_contracts(
            &mut self.contracts,
            &mut self.corporations,
            &mut self.stations,
            &mut self.ships,
            &self.bodies,
            &self.eph,
            &self.config.guidance,
            &self.config.shipyard,
            next,
            &mut self.events,
        );

        // (1c2) pirate-brain stage (pirates rung spec §5): bounded DUMB lurkers
        //       — lie-low routing to the hideout, staggered reach-bounded
        //       relocation (uniform-in-reach, NEVER traffic-weighted), loiter
        //       re-seek strictly inside the engagement envelope. PRE-physics:
        //       ships.pos is the tick-`cur` state, body_pos sampled at
        //       `next - 1 == cur` (the try_load frame precedent). Scripted
        //       stage: skips !scripted craft. Shares the spec-§8 inert lever.
        if self.config.trophic.engage_radius_au > 0.0 {
            crate::pirate::run_pirate_brains(
                &mut self.ships,
                &self.config.craft,
                &self.stations,
                &self.bodies,
                &self.eph,
                &self.config.trophic,
                &mut self.rng,
                next,
                &mut self.events,
            );
        }

        // (1c3) scripted purchase policies (pirates rung §6): write the
        //       `pending_upgrade` INTENT (haulers: the BuyPolicy ladder with
        //       working-capital headroom at a vendor dock; pirates: Escort
        //       while lying low at the hideout), consumed by stage 1d below
        //       the SAME tick — the column stays None at every hash point.
        //       Scripted stage: skips !scripted craft. Inert by default
        //       (BuyPolicy::Off + engage_radius 0.0).
        crate::economy::run_purchase_policies(
            &mut self.ships,
            &self.config.craft,
            &self.stations,
            &self.config.stations,
            &self.bodies,
            &self.eph,
            &self.config.trophic,
            &self.config.shipyard,
            next,
        );

        // (1c3b) scripted refuel policies (world-gets-big §5): write
        //       `pending_refuel` for docked, scripted, non-pirate craft with
        //       >= 1 lot of headroom and a wallet covering one unit at the
        //       dock's live price. Consumed by stage 1d2 below the same tick.
        crate::economy::run_refuel_policies(
            &mut self.ships,
            &self.config.craft,
            &self.stations,
            &self.bodies,
            &self.eph,
            &self.config.refuel,
            next,
        );

        // (1d) upgrade-purchase settle stage (pirates rung §6): consume every
        //      BuyUpgrade intent written by this tick's ingest and the stage-1c3
        //      scripted purchase policies. AFTER resolve_contracts, PRE-
        //      physics: ships.pos is still the tick-`cur` state, so the vendor
        //      dock predicate samples body_pos at `next - 1 == cur` — the same
        //      frame (the try_load precedent). Settle is a pure wallet -> Yard
        //      transfer (zero new identity legs); every intent is consumed here,
        //      keeping `pending_upgrade` None at all hash points.
        crate::economy::resolve_purchases(
            &mut self.ships,
            &self.stations,
            &self.config.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &self.config.shipyard,
            next,
            &mut self.events,
        );

        // (1d2) refuel settle stage (world-gets-big §5): consume every Refuel
        //       intent written by this tick's ingest or scripted refuel policy.
        //       AFTER purchases, PRE-physics: same-tick burn draws from the
        //       refilled tank. `prev_fuel` is untouched here; the stage-4
        //       copy-forward keeps FuelEmpty edge detection pinned.
        crate::economy::resolve_refuels(
            &mut self.ships,
            &mut self.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &mut self.econ,
            &self.config.refuel,
            next,
            &mut self.events,
        );

        // Snapshot body eph_index + mass to avoid borrowing self inside the closure.
        let body_indices: Vec<(usize, f64)> = (0..self.bodies.mass.len())
            .map(|i| (self.bodies.eph_index[i], self.bodies.mass[i]))
            .collect();
        let softening = self.config.softening;
        let substep_cfg = self.config.substep_cfg;
        let integrator = VelocityVerlet;
        let n_craft = self.ships.pos.len();

        for ci in 0..n_craft {
            // (2) Lod-dispatch must-shape seam. v1 implements `Player` (full physics).
            // `Nothing` = dormant / not ticked (spec §3.2): skip physics entirely.
            // A future tier that wakes a craft (Nothing -> Player) emits `Wake`.
            match self.ships.lod[ci] {
                Lod::Nothing => {
                    // Dormant: state is propagated closed-form elsewhere; do nothing here.
                    // (Seam exercised; analytic propagation deferred per spec.)
                    continue;
                }
                Lod::NpcInteraction => {
                    // Deferred tier; v1 falls through to Player-grade physics so the
                    // dispatch branch exists and is type-checked.
                }
                Lod::Player => {}
            }

            let eff = effective_params(&self.ships.spec[ci], &self.ships.mods[ci]);
            let pos = self.ships.pos[ci];
            let vel = self.ships.vel[ci];
            let fuel = self.ships.fuel_mass[ci];

            let (dest_pos, dest_vel) = match self.ships.nav[ci] {
                NavState::Seeking { dest, .. } => {
                    (self.resolve_dest_pos(dest, cur), self.resolve_dest_vel(dest, cur))
                }
                NavState::Idle => (pos, Vec3::ZERO), // unused (throttle will be 0)
                NavState::DirectThrust { .. } => (pos, Vec3::ZERO), // unused (autopilot ignores dest)
            };
            let (thrust_dir, throttle) = autopilot_command(
                self.ships.nav[ci], pos, vel, dest_pos, dest_vel, fuel, &eff,
                &self.config.guidance, dt,
            );

            let (thrust_accel, fuel_consumed) =
                thrust_accel_and_burn(&eff, fuel, thrust_dir, throttle, dt);

            // accel_at(p, sub_t_days): softened gravity at the sub-tick instant the
            // body has moved to, plus the (tick-constant) thrust acceleration.
            let eph = &self.eph;
            let accel_at = |p: Vec3, sub_t: f64| -> Vec3 {
                let frac = sub_t / dt; // days into the tick -> fractional tick
                let body_positions: Vec<(Vec3, f64)> = body_indices
                    .iter()
                    .map(|&(eidx, m)| (eph.body_pos_subtick(eidx, cur, frac), m))
                    .collect();
                gravity_accel(p, &body_positions, softening).add(thrust_accel)
            };

            // N = pure fn of QUANTIZED total local acceleration magnitude.
            let total_accel_mag = accel_at(pos, 0.0).length();
            let n = substep_count(total_accel_mag, substep_cfg);

            let (new_pos, new_vel) = integrator.step_craft(pos, vel, &accel_at, dt, n);

            self.ships.pos[ci] = new_pos;
            self.ships.vel[ci] = new_vel;
            self.ships.fuel_mass[ci] = (fuel - fuel_consumed).max(0.0);

            // UNHASHED fuel diagnostics: duty, cumulative burn, and low-water.
            // Diagnostics-only; no behavior stage reads `fuel_diag`.
            if fuel_consumed > 0.0 {
                self.fuel_diag.thrust_ticks[ci] =
                    self.fuel_diag.thrust_ticks[ci].saturating_add(1);
                self.fuel_diag.burned_mass[ci] += fuel_consumed;
            }
            if self.ships.fuel_mass[ci] < self.fuel_diag.min_fuel_mass[ci] {
                self.fuel_diag.min_fuel_mass[ci] = self.ships.fuel_mass[ci];
            }

            if throttle > 0.0 {
                let dv = thrust_accel.length() * dt;
                if let NavState::Seeking { dest, dv_remaining } = self.ships.nav[ci] {
                    self.ships.nav[ci] = NavState::Seeking {
                        dest,
                        dv_remaining: dv_remaining - dv,
                    };
                }
                let id = self.craft_id_at(ci);
                self.events.emit(Event {
                    tick: next,
                    kind: EventKind::ThrustApplied { craft: id, dv },
                });
            }
        }

        // (3) detect Arrival / FuelEmpty at the new boundary. MANDATORY borrow split:
        //     detect_boundary_events borrows `&self.ships`/`&self.bodies`/`&self.eph`
        //     (reads stores) AND writes the event sink; passing `&mut self.events`
        //     alongside `&self.*` field borrows is E0502. Take the EventStream out,
        //     run detection against the shared field borrows, put it back.
        let mut ev = std::mem::take(&mut self.events);
        crate::events::detect_boundary_events(&self.ships, &self.bodies, &self.eph, next, &mut ev);
        self.events = ev;

        // (3b) economy delivery stage: settle any InTransit contract whose hauler
        //      just arrived at its destination body. Lift this tick's Arrival
        //      (craft, dest) pairs out of the event stream FIRST (drops the immutable
        //      borrow), then settle. Unload + payout are TRANSFERS (no flow/credit
        //      created); resolution is sorted-ContractId order for determinism.
        let arrivals: Vec<(CraftId, NavDest)> = self
            .events
            .since(next)
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Arrival { craft, dest } => Some((craft, dest)),
                _ => None,
            })
            .collect();
        crate::economy::resolve_deliveries(
            &mut self.contracts,
            &mut self.stations,
            &mut self.ships,
            &arrivals,
            next,
            &mut self.events,
        );

        // (3b2) encounter stage (pirates rung spec §2): choke-point engagements
        //       over the POST-physics current-tick state. Stage order is
        //       load-bearing: AFTER 3b — a same-tick Arrival settles the
        //       delivery first, so the DESTINATION dock is sanctuary by
        //       ordering — and BEFORE 3c (extending the proven 3b-before-3c
        //       precedent). First runtime RngStream::Piracy consumer: draws at
        //       exactly this stage, dense pirate-row order, one per engagement.
        // (3b3) pirate population stage (spec §4): upkeep/starvation/heat/decay
        //       per pirate, dense order — AFTER 3b2 so this tick's robberies
        //       feed food/notoriety before the lifecycle reads them.
        //       Both stages share the spec-§8 inert lever (engage_radius 0.0 =
        //       default = the whole trophic machinery off); the outer guard
        //       also skips the station-position resolve on inert worlds.
        if self.config.trophic.engage_radius_au > 0.0 {
            // Station body positions at `next` (post-physics frame), dense
            // station-row order — the trip-phase diagnostic's input only; the
            // engagement predicate itself is craft-craft and needs no body frame.
            let station_pos: Vec<Vec3> = (0..self.stations.ids.len())
                .map(|srow| {
                    let body = self.stations.body[srow];
                    self.bodies
                        .ids
                        .dense_index(body.slot, body.generation)
                        .map(|brow| self.eph.body_pos(self.bodies.eph_index[brow], next))
                        .unwrap_or(Vec3::ZERO)
                })
                .collect();
            // (ii) One arrival-radius scan: the LOWEST station row within
            // radius, per craft — the dock partner (media rung spec §5) AND
            // the dock detector for the info refresh below.
            let dock_station: Vec<Option<usize>> = (0..self.ships.ids.len())
                .map(|crow| {
                    (0..station_pos.len()).find(|&s| {
                        self.ships.pos[crow].sub(station_pos[s]).length()
                            <= crate::autopilot::ARRIVAL_RADIUS
                    })
                })
                .collect();
            // Edge-triggered gossip exchange (media rung spec §5): reads the
            // PRE-refresh `info_tick` (the edge predicate), so it MUST run
            // before the refresh loop. Behind the media dual gate — media-off
            // worlds consume zero Media draws and are bit-identical.
            if self.media_live() {
                crate::media::run_gossip_exchange(
                    &mut self.ships,
                    &mut self.station_gossip,
                    &self.stations,
                    &dock_station,
                    &self.config.media,
                    self.config.trophic.evidence_window,
                    &mut self.rng,
                    next,
                    &mut self.events,
                    &mut self.media_diag,
                );
            }
            // Dock-gated info refresh (spec §7): a craft within ARRIVAL_RADIUS
            // of ANY station body refreshes `info_tick` to the current tick —
            // information is a POSITIONED resource; the population's beliefs
            // desynchronize by lived docking rhythms, not a global lag. Hashed
            // state, so it rides the same inert lever as the rest of the rung.
            for (crow, dock) in dock_station.iter().enumerate() {
                if dock.is_some() {
                    self.ships.info_tick[crow] = next;
                }
            }
            crate::pirate::resolve_encounters(
                &mut self.ships,
                &mut self.contracts,
                &mut self.corporations,
                &mut self.econ,
                &self.stations,
                &station_pos,
                &mut self.route_evidence,
                &mut self.station_gossip,
                &mut self.next_alert_seq,
                &self.config.trophic,
                &self.config.media,
                &mut self.rng,
                next,
                &mut self.events,
                &mut self.engagement_diag,
                &mut self.media_diag,
            );
            crate::pirate::update_pirate_population(
                &mut self.ships,
                &self.config.trophic,
                next,
                &mut self.events,
            );
        }

        // (3c) economy failure stage: fail any InTransit contract whose hauler ran out
        //      of propellant this tick. Lift this tick's FuelEmpty craft-ids out of the
        //      event stream FIRST (drops the immutable borrow), then mutate — same
        //      event-lift borrow pattern as 3b. Runs AFTER 3b so a same-tick
        //      Arrival+FuelEmpty resolves as delivered (3b clears the contract; 3c skips
        //      the now-non-InTransit row). Refund is a credit TRANSFER; cargo loss is an
        //      accounted SINK leg (consumed += qty). Resolution is sorted-ContractId.
        let failed_craft: Vec<CraftId> = self
            .events
            .since(next)
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::FuelEmpty { craft } => Some(craft),
                _ => None,
            })
            .collect();
        crate::economy::resolve_failures(
            &mut self.contracts,
            &mut self.corporations,
            &mut self.ships,
            &mut self.econ,
            &failed_craft,
            next,
            &mut self.events,
        );

        // (3c2) UNHASHED fuel-leg diagnostics: open on ContractAccepted, close
        // on ContractFulfilled. Robbed/failed legs do not close; the next accept
        // overwrites the bracket. Burn is recorded through the permille_floor
        // seam as permille of effective capacity.
        let leg_edges: Vec<(CraftId, bool)> = self
            .events
            .since(next)
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::ContractAccepted { hauler, .. } => Some((hauler, true)),
                EventKind::ContractFulfilled { hauler, .. } => Some((hauler, false)),
                _ => None,
            })
            .collect();
        for (craft, opened) in leg_edges {
            let Some(row) = self.ship_index(craft) else { continue };
            if opened {
                self.fuel_diag.leg_start_fuel[row] = Some(self.ships.fuel_mass[row]);
            } else if let Some(start) = self.fuel_diag.leg_start_fuel[row].take() {
                let cap =
                    effective_params(&self.ships.spec[row], &self.ships.mods[row]).fuel_capacity;
                let burned = (start - self.ships.fuel_mass[row]).max(0.0);
                self.fuel_diag
                    .leg_burns
                    .push((next, crate::diagnostics::permille_floor(burned, cap)));
            }
        }

        // (3d) economy reprice stage (Stage-2 — Task 20): on a tick-gated clock,
        //      recompute station micro-prices against this tick's FULLY-SETTLED stock
        //      (production + deliveries + sink-consume all applied). NOT lazy-on-read —
        //      repricing happens here in the step path so the cadence is part of the
        //      recorded schedule. The `reprice_interval > 0` guard avoids a
        //      modulo-by-zero if a fixture sets the interval to 0 (clock disabled).
        //      Disjoint &mut field borrows (`stations`, `events`) + read-only price_cfg.
        if self.config.price_cfg.reprice_interval > 0
            && next.0.is_multiple_of(self.config.price_cfg.reprice_interval as u64)
        {
            crate::economy::update_prices(
                &mut self.stations,
                &self.config.price_cfg,
                next,
                &mut self.events,
            );
        }

        // (4) copy-forward the boundary-edge arrays so next tick's detection has a
        //     prior. These arrays are folded into state_hash at the position fixed by
        //     HASH_FIELD_ORDER (a later hash task pins their contribution).
        for ci in 0..n_craft {
            // TODO(spec §13, deferred): prev_inside_dest below is an ENDPOINT
            // point-in-sphere test, not the swept verdict. A pure chord-clip arrival
            // (closest approach inside R, neither endpoint inside) could re-fire the
            // once-only latch. Out of scope for v1 (the rel_speed gate suppresses the
            // flyby case; the rendezvous case has an endpoint inside R). Deriving
            // inside-prev from the chord is explicitly deferred.
            self.ships.prev_pos[ci] = self.ships.pos[ci];
            self.ships.prev_fuel[ci] = self.ships.fuel_mass[ci];
            self.ships.prev_inside_dest[ci] = match self.ships.nav[ci] {
                NavState::Seeking { dest, .. } => {
                    let dp = self.resolve_dest_pos(dest, next);
                    self.ships.pos[ci].sub(dp).length() <= crate::autopilot::ARRIVAL_RADIUS
                }
                NavState::Idle => false,
                // No destination to be "inside of": direct thrust never arms the
                // arrival edge-detector.
                NavState::DirectThrust { .. } => false,
            };
        }

        // (5) advance.
        self.tick = next;
    }

    /// Resolve a NavDest to a concrete position at `tick`
    /// (Entity bodies are tick-derived from the ephemeris).
    fn resolve_dest_pos(&self, dest: NavDest, tick: Tick) -> Vec3 {
        match dest {
            NavDest::Position(p) => p,
            NavDest::Entity(EntityRef::Body(b)) => self.body_pos(b, tick).unwrap_or(Vec3::ZERO),
            NavDest::Entity(EntityRef::Craft(c)) => self.craft_pos(c).unwrap_or(Vec3::ZERO),
        }
    }

    /// Resolve a NavDest to the target's VELOCITY at `tick` (the frame the
    /// autopilot brakes into). Symmetric to `resolve_dest_pos`:
    /// - `Position` => `Vec3::ZERO` (a fixed point does not move),
    /// - `Entity(Body)` => the tick-derived ephemeris velocity,
    /// - `Entity(Craft)` => that craft's stored velocity (best-effort stub;
    ///   craft→craft nav is not a fully-supported v1 target — `ZERO` if unresolved).
    fn resolve_dest_vel(&self, dest: NavDest, tick: Tick) -> Vec3 {
        match dest {
            NavDest::Position(_) => Vec3::ZERO,
            NavDest::Entity(EntityRef::Body(b)) => self
                .body_index(b)
                .map(|i| self.eph.body_vel(self.bodies.eph_index[i], tick))
                .unwrap_or(Vec3::ZERO),
            NavDest::Entity(EntityRef::Craft(c)) => self.craft_vel(c).unwrap_or(Vec3::ZERO),
        }
    }

    /// Observer-parameterized projection. The presence mask is sourced from the
    /// single `visible(observer, entity)` predicate (all-true for `FullObserver`).
    /// This is the ONE location a future fog-of-war / per-faction filter edits.
    pub fn project<O: Observer>(&self, observer: &O) -> View {
        let mut craft = Vec::new();
        for id in self.craft_ids() {
            if observer.visible(EntityRef::Craft(id)) {
                let i = self.ship_index(id).expect("live id");
                // effective fuel_capacity rides the accessor seam (effective==base in v1).
                let cap = effective_params(&self.ships.spec[i], &self.ships.mods[i]).fuel_capacity;
                craft.push((
                    id,
                    self.ships.pos[i],
                    self.ships.vel[i],
                    self.ships.fuel_mass[i],
                    cap,
                ));
            }
        }
        let mut bodies = Vec::new();
        let t = self.tick;
        for id in self.body_ids() {
            if observer.visible(EntityRef::Body(id)) {
                bodies.push((id, self.body_pos(id, t).expect("live id")));
            }
        }
        View {
            tick: t,
            craft,
            bodies,
        }
    }

    #[cfg(test)]
    fn set_lod_for_test(&mut self, id: CraftId, lod: Lod) {
        if let Some(i) = self.ship_index(id) {
            self.ships.lod[i] = lod;
        }
    }
}

impl StateView for World {
    fn tick(&self) -> Tick {
        self.tick
    }
    fn dt(&self) -> Dt {
        self.dt
    }
    fn craft_ids(&self) -> Vec<CraftId> {
        let mut v: Vec<CraftId> = self
            .ships
            .ids
            .iter_ids()
            .map(|(slot, generation)| CraftId { slot, generation })
            .collect();
        v.sort();
        v
    }
    fn craft_pos(&self, id: CraftId) -> Option<Vec3> {
        self.ship_index(id).map(|i| self.ships.pos[i])
    }
    fn craft_vel(&self, id: CraftId) -> Option<Vec3> {
        self.ship_index(id).map(|i| self.ships.vel[i])
    }
    fn craft_fuel(&self, id: CraftId) -> Option<f64> {
        self.ship_index(id).map(|i| self.ships.fuel_mass[i])
    }
    fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
        // effective_params (use crate::stores::effective_params, in scope above)
        // applies the spec's modifiers; fuel_capacity is the effective field.
        self.ship_index(id)
            .map(|i| effective_params(&self.ships.spec[i], &self.ships.mods[i]).fuel_capacity)
    }
    fn body_ids(&self) -> Vec<BodyId> {
        let mut v: Vec<BodyId> = self
            .bodies
            .ids
            .iter_ids()
            .map(|(slot, generation)| BodyId { slot, generation })
            .collect();
        v.sort();
        v
    }
    fn body_pos(&self, id: BodyId, tick: Tick) -> Option<Vec3> {
        self.body_index(id)
            .map(|i| self.eph.body_pos(self.bodies.eph_index[i], tick))
    }
    fn recent_commands(&self, since: Tick) -> &[Command] {
        self.log.since_commands(since)
    }
    fn recent_events(&self, since: Tick) -> &[Event] {
        self.events.since(since)
    }
    fn lod(&self, id: CraftId) -> Option<Lod> {
        self.ship_index(id).map(|i| self.ships.lod[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BaseSpec, BodyInit, CraftInit, GuidanceParams, OrbitalElements, SubstepCfg};
    use crate::contract::StateView;
    use crate::types::CommandKind;

    fn one_body_one_craft() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(1.0),
            softening: 1e-4,
            substep_cfg: SubstepCfg {
                accel_ref: 1.0,
                max_substeps: 64,
            },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1.0, // 1 M_sun central star at the origin (a == 0.0 conic)
                elements: OrbitalElements {
                    a: 0.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-12,
                    // Thrust set below the resolvability ceiling (coast fixture; value is
                    // not behavioural). The craft never `Seeking`s in any of this fixture's
                    // tests, so thrust magnitude is behaviourally irrelevant; coast
                    // trajectories under gravity do not read max_thrust. At dry+fuel == 2e-12,
                    // a_max_empty = 1e-17/1e-12 = 1e-5, so a_max_empty*dt^2 = 1e-5*1^2 = 1e-5
                    // < R/(2*k_brake) = 1e-4 -> passes the §6 reset guard.
                    base_max_thrust: 1e-17,
                    base_exhaust_velocity: 1e-3,
                    base_fuel_capacity: 1e-12,
                    base_cargo_capacity: 5,
                },
                // 1 AU out, on a roughly circular prograde orbit (v ~ sqrt(GM/r)).
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 0.0172, 0.0),
                fuel_mass: 1e-12,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
            trophic: crate::config::TrophicCfg::default(),
            shipyard: crate::config::ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: crate::config::RefuelCfg::default(),
            goods: crate::config::GoodsCfg::default(),
        }
    }

    #[test]
    fn craft_role_reads_role_and_none_for_stale() {
        // World-gets-big spec §7: the chronicle epilogue's role read — a
        // plain pub accessor over already-hashed state (trader-accessor
        // pattern; no layout, fold-order, or stepping change).
        let (world, _) = World::reset(one_body_one_craft()).expect("resolvable cfg");
        let id = world.ships.ids_at(0);
        assert_eq!(world.craft_role(id), Some(crate::stores::CraftRole::Idle), "live read");
        let stale = CraftId { slot: id.slot, generation: id.generation + 1 };
        assert_eq!(world.craft_role(stale), None, "stale id reads None");
    }

    /// A config for the velocity-targeting braking law: one craft at REST in a
    /// far-out, negligible-gravity region (5 AU; central accel ~ G*M/5^2 ~ 1.2e-5),
    /// fueled so its Δv budget (`v_e*ln(2) ~ 6.9e-3`) covers the round-trip
    /// accelerate+brake burn the law commands, and sized so the empty-tank
    /// `a_max*dt` stays well under the cruise cap (`cruise_burn_fraction` x
    /// full-tank Δv, ~2e-3 AU/day here; no coarse-step aliasing). This is the
    /// regime the new law is designed for; the orbital `one_body_one_craft` fixture
    /// is left untouched for the coast/dormant/projection tests that never thrust.
    fn one_body_one_thrusting_craft() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg {
                accel_ref: 1e-3,
                max_substeps: 64,
            },
            ephemeris_window: 4096,
            bodies: vec![BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 0.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-9,
                    // a_max(full) = 1e-12/2e-9 = 5e-4 >> local gravity (~1.2e-5);
                    // a_max(empty) = 1e-12/1e-9 = 1e-3, so a_max*dt = 2.5e-4 << cruise cap.
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2, // Δv_max = 1e-2*ln(2) ~ 6.9e-3 > 2x cruise cap
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::new(5.0, 0.0, 0.0),
                vel: Vec3::ZERO, // start at REST: no orbital-velocity Δv tax
                fuel_mass: 1e-9,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
            trophic: crate::config::TrophicCfg::default(),
            shipyard: crate::config::ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: crate::config::RefuelCfg::default(),
            goods: crate::config::GoodsCfg::default(),
        }
    }

    /// A parameterized variant of `one_body_one_thrusting_craft` that lets a test
    /// set `base_max_thrust` to exercise the §6 resolvability guard.
    fn one_body_one_thrusting_craft_with_thrust(max_thrust: f64) -> RunConfig {
        let mut cfg = one_body_one_thrusting_craft();
        cfg.craft[0].spec.base_max_thrust = max_thrust;
        cfg
    }

    #[test]
    fn reset_rejects_unbrakable_high_thrust_craft() {
        // dry=1e-9, max_thrust=1e-11 (10x the passing fixture) at dt=0.25:
        // a_max_empty = 1e-2, a_max*dt^2 = 6.25e-4 >= R=1e-4 -> REJECT.
        let cfg = one_body_one_thrusting_craft_with_thrust(1.0e-11);
        // `World` is not `Debug` (it owns large stores), so map the Ok arm to its
        // error-relevant shape before formatting rather than printing the World.
        match World::reset(cfg).map(|_| ()) {
            Err(ResetError::Unbrakable { craft_index, .. }) => assert_eq!(craft_index, 0),
            other => panic!("expected Unbrakable, got {other:?}"),
        }
    }

    #[test]
    fn reset_accepts_resolvable_thrusting_craft() {
        // The real fixture: dry=1e-9, max_thrust=1e-12 -> a_max*dt^2 = 6.25e-5 < R -> Ok.
        assert!(World::reset(one_body_one_thrusting_craft()).is_ok());
    }

    #[test]
    fn reset_rejects_zero_dry_mass_craft() {
        // dry = 0 -> a_max_empty = max_thrust/0 = INFINITY; the `dry > 0.0` /
        // `is_finite()` guard branch must reject (else a divide-by-zero ship slips through).
        let mut cfg = one_body_one_thrusting_craft();
        cfg.craft[0].spec.base_dry_mass = 0.0;
        assert!(matches!(World::reset(cfg), Err(ResetError::Unbrakable { craft_index: 0, .. })));
    }

    #[test]
    fn reset_starts_at_tick_zero_and_hashes_config() {
        let cfg = one_body_one_craft();
        let expected = cfg.config_hash();
        let (world, returned) = World::reset(cfg).expect("resolvable config");
        assert_eq!(returned, expected, "reset must return RunConfig::config_hash()");
        assert_eq!(world.tick(), Tick(0));
        assert_eq!(world.dt().get(), 1.0);
        assert_eq!(world.craft_ids().len(), 1);
        assert_eq!(world.body_ids().len(), 1);
    }

    #[test]
    fn step_advances_tick_and_coasts_under_gravity() {
        let cfg = one_body_one_craft();
        let (mut world, _) = World::reset(cfg).expect("resolvable config");

        let start_r = world.craft_pos(world.craft_ids()[0]).unwrap().length();
        let body = world.body_ids()[0];
        let body_at_0 = world.body_pos(body, Tick(0)).unwrap();
        // a==0.0 star fix (Task 7/9): the sample must be FINITE, else the assert below
        // is NaN != NaN and the determinism claim is vacuous.
        assert!(body_at_0.x.is_finite() && body_at_0.y.is_finite() && body_at_0.z.is_finite());

        // No commands: the craft coasts (nav stays Idle, autopilot throttles 0).
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..10 {
            world.step(&mut empty);
        }

        assert_eq!(world.tick(), Tick(10), "10 steps -> tick 10");

        // Body position is derived from tick via ephemeris, never mutated in a store:
        // body_pos(t) must equal the ephemeris sample for that t regardless of stepping.
        let body_at_0_again = world.body_pos(body, Tick(0)).unwrap();
        assert_eq!(body_at_0, body_at_0_again, "body_pos(0) is tick-derived, not stateful");

        // The craft moved but did not blow up: radius stays within a sane band.
        let r = world.craft_pos(world.craft_ids()[0]).unwrap().length();
        assert!(r > 0.5 * start_r && r < 2.0 * start_r, "coast stayed bounded: r={r}");
    }

    #[test]
    fn live_ingest_no_budget_uses_fuel_derived_dv_not_infinity() {
        use crate::types::{EntityRef, NavDest, Target};
        let (mut world, _h) = World::reset(one_body_one_thrusting_craft()).expect("resolvable");
        let id = world.ships.ids_at(0); // typed CraftId for dense row 0 (no-despawn v1)
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Destination {
                dest: NavDest::Position(Vec3::new(1.0, 0.0, 0.0)),
                burn_budget: None, // no explicit budget -> must derive from fuel, not INFINITY
            },
        }];
        crate::ingest::ingest_commands(&mut world, Tick(0), &mut cmds);
        match world.ships.nav[0] {
            NavState::Seeking { dv_remaining, .. } => {
                assert!(dv_remaining.is_finite(), "dv must be finite, got {dv_remaining}");
                assert!(dv_remaining > 0.0, "fuelled craft has positive dv budget");
            }
            other => panic!("expected Seeking, got {other:?}"),
        }
    }

    #[test]
    fn thrust_command_accelerates_craft_and_burns_fuel() {
        use crate::types::{EntityRef, Target};
        // Economy-free resolvable fixture (same regime reset_accepts_resolvable_
        // thrusting_craft proves): dry=1e-9, thrust=1e-12, fuel=1e-9 at dt=0.25 ->
        // a_max(full) = 5e-4 AU/day^2, local gravity at 5 AU ~ 1.2e-5 (negligible).
        let (mut world, _h) = World::reset(one_body_one_thrusting_craft()).expect("resolvable");
        let id = world.ships.ids_at(0);
        let dt = 0.25_f64;
        let a_max_full = 1e-12 / (1e-9 + 1e-9); // max_thrust / (dry + full tank)
        let fuel0 = world.ships.fuel_mass[0];
        let vel0_x = world.ships.vel[0].x;
        assert_eq!(vel0_x, 0.0, "fixture starts at rest");

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Thrust {
                throttle_vec: Vec3::new(1.0, 0.0, 0.0),
            },
        }];
        world.step(&mut cmds);
        let vel1_x = world.ships.vel[0].x;
        let fuel1 = world.ships.fuel_mass[0];
        let mut empty: Vec<Command> = Vec::new();
        world.step(&mut empty);
        let vel2_x = world.ships.vel[0].x;
        let fuel2 = world.ships.fuel_mass[0];

        // vel.x increased by ~ a_max*dt per tick (gravity pulls -x at ~1.2e-5,
        // ~2.4% of thrust accel; allow a generous integrator/gravity band).
        let per_tick = a_max_full * dt;
        assert!(
            vel1_x > 0.8 * per_tick && vel1_x < 1.2 * per_tick,
            "tick 1 dv ~ a_max*dt: vel1_x={vel1_x}, expected ~{per_tick}"
        );
        let dv2 = vel2_x - vel1_x;
        assert!(
            dv2 > 0.8 * per_tick && dv2 < 1.2 * per_tick,
            "tick 2 dv ~ a_max*dt (held stick): dv2={dv2}, expected ~{per_tick}"
        );

        // Fuel strictly decreased on both ticks.
        assert!(fuel1 < fuel0, "tick 1 burned fuel: {fuel1} < {fuel0}");
        assert!(fuel2 < fuel1, "tick 2 burned fuel: {fuel2} < {fuel1}");

        // A ThrustApplied event was emitted for the craft.
        assert!(
            world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::ThrustApplied { craft, dv } if craft == id && dv > 0.0)),
            "ThrustApplied must be emitted for a thrusting craft"
        );
    }

    #[test]
    fn thrust_command_persists_until_replaced() {
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(one_body_one_thrusting_craft()).expect("resolvable");
        let id = world.ships.ids_at(0);

        // Ingest ONE Thrust command, then step 3 ticks with an empty cmd vec:
        // the stick is held (NavState persists), so fuel burns on ALL three ticks.
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Thrust {
                throttle_vec: Vec3::new(0.5, 0.5, 0.0),
            },
        }];
        let mut fuel_prev = world.ships.fuel_mass[0];
        world.step(&mut cmds);
        for tick in 0..3 {
            let fuel_now = world.ships.fuel_mass[0];
            assert!(
                fuel_now < fuel_prev,
                "held stick must burn fuel on tick {tick}: {fuel_now} !< {fuel_prev}"
            );
            fuel_prev = fuel_now;
            let mut empty: Vec<Command> = Vec::new();
            world.step(&mut empty);
        }

        // Replace with Thrust{ZERO}: the next tick burns no fuel.
        let mut zero = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Thrust {
                throttle_vec: Vec3::ZERO,
            },
        }];
        world.step(&mut zero);
        let fuel_after_zero = world.ships.fuel_mass[0];
        let mut empty: Vec<Command> = Vec::new();
        world.step(&mut empty);
        assert_eq!(
            world.ships.fuel_mass[0], fuel_after_zero,
            "Thrust{{ZERO}} coasts: fuel constant once the stick is zeroed"
        );
    }

    #[test]
    fn commanded_craft_moves_toward_dest_and_history_is_visible() {
        use crate::types::{EntityRef, NavDest, Target};
        // Use the thrusting-craft regime (at-rest, weak gravity, short hop): the
        // velocity-targeting braking law cannot drive the old orbital fixture,
        // whose Δv budget (~6.9e-4) is dwarfed by its 0.0172 AU/day orbital
        // velocity that the law must null first.
        let target = Vec3::new(5.3, 0.0, 0.0); // 0.3 AU hop from the 5 AU start
        let cfg = one_body_one_thrusting_craft();
        let (mut world, _) = World::reset(cfg).expect("resolvable config");
        let id = world.craft_ids()[0];

        let dest = NavDest::Position(target);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Destination { dest, burn_budget: Some(1.0) },
        }];

        let d0 = world.craft_pos(id).unwrap().sub(target).length();

        world.step(&mut cmds); // tick 0 -> 1: ingest + integrate

        // History: the command was logged at tick 0 and is visible via StateView.
        let recent = world.recent_commands(Tick(0));
        assert_eq!(recent.len(), 1, "the issued command is recorded and exposed");
        assert!(
            matches!(recent[0].kind, CommandKind::Destination { .. }),
            "recorded command kind preserved"
        );

        // The single ingestion path emitted an ActionIngested event.
        let evs = world.recent_events(Tick(0));
        assert!(
            evs.iter().any(|e| matches!(
                e.kind,
                EventKind::ActionIngested { target: Target::Entity(EntityRef::Craft(_)) }
            )),
            "ingestion emits ActionIngested"
        );

        // Keep stepping; under the velocity-targeting braking law the at-rest
        // craft accelerates toward the dest (capped at the cruise cap,
        // cruise_burn_fraction x full-tank Δv) and net-approaches.
        for _ in 0..400 {
            let mut none: Vec<Command> = Vec::new();
            world.step(&mut none);
        }
        let d1 = world.craft_pos(id).unwrap().sub(target).length();
        let p1 = world.craft_pos(id).unwrap();
        assert!(d1 < d0, "craft moved toward dest: {d0} -> {d1}");
        assert!(
            p1.x.is_finite() && p1.y.is_finite() && p1.z.is_finite(),
            "craft position stayed finite: {p1:?}"
        );
    }

    #[test]
    fn dormant_craft_skips_physics() {
        let cfg = one_body_one_craft();
        let (mut world, _) = World::reset(cfg).expect("resolvable config");
        let id = world.craft_ids()[0];
        let p0 = world.craft_pos(id).unwrap();

        // Force the craft dormant via the Lod seam (test-only mutator below).
        world.set_lod_for_test(id, Lod::Nothing);

        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..10 {
            world.step(&mut empty);
        }
        // Dormant craft are not ticked: position is unchanged.
        assert_eq!(world.craft_pos(id).unwrap(), p0, "Lod::Nothing skips integration");
        assert_eq!(world.tick(), Tick(10));
    }

    struct DenyAll;
    impl Observer for DenyAll {
        fn visible(&self, _t: crate::types::EntityRef) -> bool {
            false
        }
    }

    #[test]
    fn project_respects_observer_visibility_and_accessors() {
        let cfg = one_body_one_craft();
        let (world, _) = World::reset(cfg).expect("resolvable config");
        let cid = world.craft_ids()[0];

        let full = world.project(&FullObserver);
        assert_eq!(full.tick, Tick(0));
        assert_eq!(full.craft.len(), 1, "FullObserver sees the one craft");
        assert_eq!(full.bodies.len(), 1, "FullObserver sees the one body");

        // View accessor methods (the contract Task 16's write_obs_frame_relative reads):
        assert_eq!(full.craft_pos(cid), world.craft_pos(cid));
        assert_eq!(full.craft_vel(cid), world.craft_vel(cid));
        assert_eq!(full.craft_fuel(cid), world.craft_fuel(cid));
        assert_eq!(full.craft_fuel_capacity(cid), Some(1e-12), "fuel_capacity surfaced");
        // Body position in the View is the tick-derived ephemeris sample.
        assert_eq!(full.bodies[0].1, world.body_pos(world.body_ids()[0], Tick(0)).unwrap());

        let none = world.project(&DenyAll);
        assert!(none.craft.is_empty() && none.bodies.is_empty(), "deny-all hides all entities");
    }

    #[test]
    fn identity_mods_preserve_trajectory() {
        // With the default (all-IDENTITY) mods, a thrusting transfer must produce the
        // SAME state as the pre-bundle build: thrust_factor == 1.0 and x*1.0 == x, so
        // max_thrust — and therefore pos/vel/fuel — are bit-identical.
        let cfg = one_body_one_thrusting_craft();
        let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
        let id = world.craft_ids()[0];

        assert_eq!(world.ships.mods[0], crate::stores::EffectiveMods::IDENTITY);

        use crate::types::{EntityRef, NavDest, Target};
        let target = Vec3::new(5.3, 0.0, 0.0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Destination { dest: NavDest::Position(target), burn_budget: Some(1.0) },
        }];
        world.step(&mut cmds);
        for _ in 0..50 {
            let mut none: Vec<Command> = Vec::new();
            world.step(&mut none);
        }

        // mods is never written in plan A, so it stays IDENTITY throughout.
        assert_eq!(world.ships.mods[0], crate::stores::EffectiveMods::IDENTITY);
        let p = world.craft_pos(id).unwrap();
        assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
    }

    /// A `one_body_one_craft` config with an economy: 2 stations on body 0 plus one
    /// ∅->Ore miner attached to station 0. Exercises the RESET-FROM-CONFIG minting
    /// path (distinct from `populated_economy_parity`, which hand-mutates).
    fn one_body_two_stations_one_miner() -> RunConfig {
        use crate::config::{ProducerInit, StationInit};
        use crate::economy::{Recipe, Good};
        let mut cfg = one_body_one_craft();
        cfg.stations = vec![
            StationInit {
                body_index: 0,
                initial_stock: vec![7i64, 3i64],
                initial_price_micros: vec![100i64, 200i64],
                sells_upgrades: false,
            },
            StationInit {
                body_index: 0,
                initial_stock: vec![0i64, 0i64],
                initial_price_micros: vec![150i64, 250i64],
                sells_upgrades: false,
            },
        ];
        cfg.producers = vec![ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 1 },
        }];
        cfg
    }

    #[test]
    fn reset_mints_economy_from_config_deterministically() {
        // Two worlds reset from the SAME economy config mint bit-identical state.
        let (wa, _) = World::reset(one_body_two_stations_one_miner()).expect("resolvable config");
        let (wb, _) = World::reset(one_body_two_stations_one_miner()).expect("resolvable config");
        assert_eq!(
            crate::hash::state_hash(&wa),
            crate::hash::state_hash(&wb),
            "two worlds minted from the same economy config must have equal state_hash"
        );

        // Minted store contents match the RunConfig init vecs.
        assert_eq!(wa.stations.ids.len(), 2, "2 stations minted");
        assert_eq!(wa.producers.ids.len(), 1, "1 producer minted");
        // Station 0's opening market came straight from StationInit[0].
        assert_eq!(wa.stations.stock[0], vec![7i64, 3i64]);
        assert_eq!(wa.stations.price_micros[0], vec![100i64, 200i64]);
        // The minted producer points at the minted StationId for station_index 0.
        let st0 = wa.stations.ids.id_at(0).map(|(slot, generation)| crate::ids::StationId {
            slot,
            generation,
        });
        assert_eq!(Some(wa.producers.station[0]), st0, "producer bound to minted station 0");
        // Flow counters start zero (no firing at reset).
        assert_eq!(wa.econ, crate::economy::EconCounters::zero(crate::economy::N_GOODS_V1));
    }

    #[test]
    fn half_on_media_config_is_rejected() {
        // Exactly one gossip slot cap > 0 is a misconfiguration (the dual
        // gate's config half must be all-or-nothing): reset rejects BEFORE
        // tick 0 with BadMediaCfg, like BadEconomyRef.
        let mut cfg = one_body_two_stations_one_miner();
        cfg.media.station_gossip_slots = 16;
        cfg.media.craft_gossip_slots = 0;
        assert!(matches!(
            World::reset(cfg).map(|_| ()),
            Err(ResetError::BadMediaCfg { .. })
        ));
    }

    #[test]
    fn refuel_half_on_price_surface_is_a_reset_error() {
        // lot_mass > 0 with a dead Fuel price surface would make every refuel a
        // silent `unit_price < 1` no-op. Reject it before tick 0, like BadMediaCfg.
        let mut cfg = one_body_two_stations_one_miner();
        cfg.corporations = vec![crate::config::CorporationInit {
            treasury_micros: 0,
            home_station_index: 0,
        }];
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 5e-11, corp_index: 0 };

        // Arm 1: price_cfg.base_micros[Fuel] == 0 (the PriceCfg default).
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with base_micros[Fuel] == 0 must reject"
        );

        let fuel = crate::economy::Good::FUEL.index();
        cfg.price_cfg.base_micros[fuel] = 5_000;
        cfg.stations[0].initial_price_micros[fuel] = 0;
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with a zero seeded station Fuel price must reject"
        );

        cfg.refuel.corp_index = cfg.corporations.len() as u32;
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with a stale Port corp index must reject"
        );

        cfg.refuel.corp_index = 0;
        for station in cfg.stations.iter_mut() {
            station.initial_price_micros[fuel] = 5_000;
        }
        assert!(World::reset(cfg).is_ok(), "fully-on refuel config resolves");
    }

    #[test]
    fn media_live_reset_mints_buffers() {
        use crate::stores::CraftRole;
        // 2-station fixture + a hauler and a pirate craft. Caps 16/8 with the
        // trophic lever open (engage > 0) => media-live: station buffers len 2
        // cap 16; non-pirate craft rows Some cap 8; pirate rows None
        // (information-blind by construction, spec §16 OD-6).
        let mut cfg = one_body_two_stations_one_miner();
        let mut hauler = cfg.craft[0].clone();
        hauler.role = CraftRole::Hauler;
        let mut pirate = cfg.craft[0].clone();
        pirate.role = CraftRole::Pirate;
        cfg.craft.push(hauler);
        cfg.craft.push(pirate);
        cfg.media.station_gossip_slots = 16;
        cfg.media.craft_gossip_slots = 8;
        cfg.trophic.engage_radius_au = 0.05;
        let (w, _) = World::reset(cfg.clone()).expect("resolvable media-live config");
        assert!(w.media_live(), "caps 16/8 + engage>0 must be media-live");
        assert_eq!(w.station_gossip.len(), 2, "one reservoir per station");
        for buf in &w.station_gossip {
            assert_eq!(buf.slots.len(), 16, "station reservoir cap from config");
            assert_eq!(buf.occupied(), 0, "reservoirs mint empty");
        }
        // Row 0 is the fixture's Idle craft — non-pirate, so it gets a
        // comms-log too (the mint rule is `role != Pirate`, not Hauler-only).
        for row in [0usize, 1] {
            let buf = w.ships.gossip[row].as_ref().expect("non-pirate row mints Some");
            assert_eq!(buf.slots.len(), 8, "craft comms-log cap from config");
            assert_eq!(buf.occupied(), 0, "comms-logs mint empty");
        }
        assert!(w.ships.gossip[2].is_none(), "pirate rows are information-blind: None");
        assert_eq!(w.next_alert_seq, 0, "alert mint counter starts at 0");
        assert_eq!(w.media_diag.evictions, 0, "diagnostics mint zeroed");
        assert!(w.media_diag.contacts.is_empty(), "diagnostics mint empty");

        // The dual gate's single-lever case: same caps, engage 0.0 => NOT
        // media-live; everything None/empty (no buffers anywhere).
        cfg.trophic.engage_radius_au = 0.0;
        let (w_off, _) = World::reset(cfg).expect("resolvable inert config");
        assert!(!w_off.media_live(), "engage 0.0 must close the dual gate");
        assert!(w_off.station_gossip.is_empty(), "no reservoirs when media is off");
        assert!(
            w_off.ships.gossip.iter().all(Option::is_none),
            "no comms-logs when media is off"
        );
    }

    /// Media-live 2-station fixture with two scripted Hauler craft on rows 1
    /// and 2 (row 0 is the fixture's Idle craft) — the Task-7 read-swap test
    /// bed: reset media-live, hand-sit alerts, read the accessor.
    fn media_live_two_haulers() -> RunConfig {
        use crate::stores::CraftRole;
        let mut cfg = one_body_two_stations_one_miner();
        let mut hauler = cfg.craft[0].clone();
        hauler.role = CraftRole::Hauler;
        cfg.craft.push(hauler.clone());
        cfg.craft.push(hauler);
        cfg.media.station_gossip_slots = 16;
        cfg.media.craft_gossip_slots = 8;
        cfg.trophic.engage_radius_au = 0.05;
        cfg
    }

    /// A hand-sat `GossipAlert` for the read-swap tests: only `route` and
    /// `first_heard` are load-bearing at the read (raw count — the cover
    /// payload is deliberately ignored by `route_evidence`, PDR-0006).
    fn test_alert(seq: u32, route: u32, first_heard: Tick) -> crate::media::GossipAlert {
        crate::media::GossipAlert {
            alert_seq: seq,
            route,
            pirate_slot: 0,
            rob_tick: Tick(1),
            claimed_value_micros: 2_000_000,
            first_heard,
            hops: 1,
        }
    }

    /// Live `CraftId` of a dense row (test helper; rows here are config-minted).
    fn craft_id_at(w: &World, row: usize) -> crate::ids::CraftId {
        let (slot, generation) = w.ships.ids.id_at(row).expect("live craft row");
        crate::ids::CraftId { slot, generation }
    }

    #[test]
    fn route_evidence_media_path_counts_own_recent_route_matches() {
        // Spec §7 (Task 7): media-live, the accessor counts the READER's OWN
        // buffer items on the route still inside its window — per-reader
        // CONTENT, not just per-reader staleness.
        let (mut w, _) =
            World::reset(media_live_two_haulers()).expect("resolvable media-live config");
        assert!(w.media_live(), "fixture must be media-live");
        let window = w.config.trophic.evidence_window;
        let a = craft_id_at(&w, 1);
        let b = craft_id_at(&w, 2);
        // Advance the clock so staleness can bite, then hand-sit hauler A's
        // comms-log: route 3 fresh, route 3 stale by 1 past the window,
        // route 5 fresh. Hauler B's buffer stays empty.
        w.tick = Tick(window + 1000);
        let now = w.tick;
        {
            let buf = w.ships.gossip[1].as_mut().expect("hauler row mints a comms-log");
            buf.slots[0] = Some(test_alert(0, 3, now));
            buf.slots[1] = Some(test_alert(1, 3, Tick(now.0 - window - 1)));
            buf.slots[2] = Some(test_alert(2, 5, now));
        }
        assert_eq!(w.route_evidence(a, 3), 1, "route 3: one fresh, one aged out");
        assert_eq!(w.route_evidence(a, 5), 1, "route 5: the fresh alert counts");
        assert_eq!(w.route_evidence(a, 9), 0, "unmentioned route reads 0");
        for route in [3usize, 5, 9] {
            assert_eq!(
                w.route_evidence(b, route),
                0,
                "B's empty buffer reads 0 on every route: per-reader CONTENT"
            );
        }
    }

    #[test]
    fn route_evidence_media_off_is_byte_identical_legacy() {
        // The legacy parity pin (Task 7): media OFF, the accessor returns
        // exactly `count_recent(route, info_tick[reader], evidence_window)`
        // on a hand-seeded ring — the media-off fallback is byte-identical.
        let (mut w, _) =
            World::reset(one_body_two_stations_one_miner()).expect("resolvable config");
        assert!(!w.media_live(), "default media caps must be off");
        let window = w.config.trophic.evidence_window;
        let reader = craft_id_at(&w, 0);
        // 2 stations -> 4 directed routes; seed route 1's ring by hand.
        w.route_evidence.robs[1][0] = Tick(10);
        w.route_evidence.robs[1][1] = Tick(50);
        w.route_evidence.robs[1][2] = Tick(70);
        w.ships.info_tick[0] = Tick(60);
        assert_eq!(
            w.route_evidence(reader, 1),
            2,
            "(info_tick - window, info_tick] holds ticks 10 and 50; 70 is after the dock"
        );
        assert_eq!(
            w.route_evidence(reader, 1),
            w.route_evidence.count_recent(1, w.ships.info_tick[0], window),
            "media-off accessor == the legacy ring read"
        );
        assert_eq!(w.route_evidence(reader, 9), 0, "out-of-range route reads 0");
    }

    #[test]
    fn deaf_control_behavioral_trace_identity() {
        // The deaf control (media spec §9, instrument-kill discipline): media
        // LIVE but UNREAD (`hauler_belief_scoring = false`) must leave
        // behavior untouched — the pre-registered behavioral trace (per-window
        // robs / laden_trips / per_route_accepts / per_craft_credits) is
        // element-identical to the media-OFF belief-off arm. Deliberately NOT
        // state_hash: gossip state legitimately differs between the arms.
        use crate::scenario::{apply_knob, scenario_trophic};
        let mut live_cfg = scenario_trophic(7);
        apply_knob(&mut live_cfg, "station_gossip_slots", "16").expect("media knob");
        apply_knob(&mut live_cfg, "craft_gossip_slots", "8").expect("media knob");
        apply_knob(&mut live_cfg, "hauler_belief_scoring", "false").expect("belief knob");
        let mut off_cfg = scenario_trophic(7);
        apply_knob(&mut off_cfg, "hauler_belief_scoring", "false").expect("belief knob");
        let (mut w_live, _) = World::reset(live_cfg).expect("media-live arm resolves");
        let (mut w_off, _) = World::reset(off_cfg).expect("media-off arm resolves");
        assert!(w_live.media_live(), "arm 1 must be media-live");
        assert!(!w_off.media_live(), "arm 2 must be media-off");
        let mut cmds: Vec<Command> = Vec::new();
        let mut ws_live = Tick(0);
        let mut ws_off = Tick(0);
        for t in 1..=6_000u64 {
            w_live.step(&mut cmds);
            w_off.step(&mut cmds);
            if t % 2_000 == 0 {
                let sl = crate::diagnostics::sample_window(&w_live, ws_live);
                let so = crate::diagnostics::sample_window(&w_off, ws_off);
                assert_eq!(sl.robs, so.robs, "robs diverge at tick {t}: media leaked");
                assert_eq!(
                    sl.laden_trips, so.laden_trips,
                    "laden_trips diverge at tick {t}: media leaked"
                );
                assert_eq!(
                    sl.per_route_accepts, so.per_route_accepts,
                    "per_route_accepts diverge at tick {t}: media leaked"
                );
                assert_eq!(
                    sl.per_craft_credits, so.per_craft_credits,
                    "per_craft_credits diverge at tick {t}: media leaked"
                );
                ws_live = w_live.tick();
                ws_off = w_off.tick();
            }
        }
    }

    #[test]
    fn media_default_is_inert() {
        // The instrument-kill control (media spec §9, plan Task 8.5): the
        // UNMODIFIED scenario (media caps default 0) and the single-lever
        // case (caps open, `engage_radius_au = 0.0`) must both be media-dead
        // — zero media events, zero gossip state, and the Media stream
        // cursor untouched (the draw-one-and-compare-to-fresh trick: one
        // draw from the stepped world equals the first draw of a fresh
        // RngStreams, so the run consumed ZERO Media draws).
        use crate::rng::{RngStream, RngStreams};
        use crate::scenario::{apply_knob, scenario_trophic};
        use rand_core::Rng;
        let arm = |cfg: crate::config::RunConfig| {
            let master = cfg.master_seed;
            let (mut w, _) = World::reset(cfg).expect("scenario resolves");
            assert!(!w.media_live(), "the dual gate must read closed");
            let mut cmds: Vec<Command> = Vec::new();
            for _ in 0..3_000 {
                w.step(&mut cmds);
            }
            assert!(
                !w.recent_events(Tick(0)).iter().any(|e| matches!(
                    e.kind,
                    EventKind::AlertBorn { .. } | EventKind::GossipHeard { .. }
                )),
                "zero media events over 3k ticks"
            );
            assert!(
                w.ships.gossip.iter().all(Option::is_none),
                "every gossip column None"
            );
            assert!(w.station_gossip.is_empty(), "no station reservoirs");
            assert_eq!(w.next_alert_seq, 0, "mint counter untouched");
            assert_eq!(
                w.rng.stream(RngStream::Media).next_u64(),
                RngStreams::from_master(master).stream(RngStream::Media).next_u64(),
                "Media stream cursor untouched (zero draws consumed)"
            );
        };
        // Arm 1: scenario_trophic(7) UNMODIFIED — caps default 0.
        arm(scenario_trophic(7));
        // Arm 2 (the dual gate's single lever): caps 16/8 but engage 0.0 —
        // reset mints nothing, so the same assertions hold verbatim.
        let mut cfg = scenario_trophic(7);
        apply_knob(&mut cfg, "station_gossip_slots", "16").expect("media knob");
        apply_knob(&mut cfg, "craft_gossip_slots", "8").expect("media knob");
        apply_knob(&mut cfg, "engage_radius_au", "0.0").expect("trophic knob");
        arm(cfg);
    }

    #[test]
    fn per_reader_forgetting_clock() {
        // Spec §7 (the staggered-return mechanism): the SAME alert copied into
        // two readers at different first-heard ticks ages out on each reader's
        // own acquisition clock, never on one synchronized world clock.
        let (mut w, _) =
            World::reset(media_live_two_haulers()).expect("resolvable media-live config");
        let window = w.config.trophic.evidence_window;
        let a = craft_id_at(&w, 1);
        let b = craft_id_at(&w, 2);
        let t0 = Tick(1000);
        w.ships.gossip[1].as_mut().expect("comms-log").slots[0] =
            Some(test_alert(9, 2, t0));
        w.ships.gossip[2].as_mut().expect("comms-log").slots[0] =
            Some(test_alert(9, 2, Tick(t0.0 + 3000)));
        // Past A's horizon (t0 + window) but not past B's (t0 + 3000 + window).
        w.tick = Tick(t0.0 + window + 1500);
        assert_eq!(w.route_evidence(a, 2), 0, "A forgets on its own clock");
        assert_eq!(w.route_evidence(b, 2), 1, "B still holds it: the return staggers");
    }

    /// One station (Ore=0) with a ∅->Ore(5) miner at interval 1, attached to it.
    /// Used to prove `run_producers` is wired into `World::step`.
    fn one_body_one_station_one_miner_ore_zero() -> RunConfig {
        use crate::config::{ProducerInit, StationInit};
        use crate::economy::{Recipe, Good};
        let mut cfg = one_body_one_craft();
        cfg.stations = vec![StationInit {
            body_index: 0,
            initial_stock: vec![0i64, 0i64],
            initial_price_micros: vec![0i64, 0i64],
            sells_upgrades: false,
        }];
        cfg.producers = vec![ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 1 },
        }];
        cfg
    }

    #[test]
    fn step_runs_producers_each_tick() {
        use crate::economy::Good;
        // Miner ∅->Ore(5), interval 1, station Ore starts at 0. Stepping 3 ticks
        // fires the producer at next = 1,2,3 -> stock and mined[Ore] both reach 15.
        let (mut world, _) =
            World::reset(one_body_one_station_one_miner_ore_zero()).expect("resolvable config");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..3 {
            world.step(&mut empty);
        }
        assert_eq!(
            world.stations.stock[0][Good::ORE.index()],
            15,
            "3 firings of ∅->Ore(5) raise station Ore stock to 15"
        );
        assert_eq!(
            world.econ.mined[Good::ORE.index()],
            15,
            "mined[Ore] counter tracks the 3 firings"
        );
    }

    #[test]
    fn reset_rejects_out_of_range_economy_ref() {
        use crate::config::StationInit;
        let mut cfg = one_body_one_craft();
        // body_index 5 is out of range (only 1 body) -> BadEconomyRef before tick 0.
        cfg.stations = vec![StationInit {
            body_index: 5,
            initial_stock: vec![0i64, 0i64],
            initial_price_micros: vec![0i64, 0i64],
            sells_upgrades: false,
        }];
        assert!(matches!(
            World::reset(cfg),
            Err(ResetError::BadEconomyRef { what: "station.body_index", index: 5 })
        ));
    }

    /// Two-body economy fixture for the contract accept/load path (Task 14).
    /// Body 0 is the stationary central star at the origin (a == 0.0), host of
    /// station A (the contract ORIGIN); the craft starts co-located at the origin
    /// so the co-location-at-accept check is exact and tick-independent. Body 1 is
    /// a distinct body (small orbit) hosting station B (the DESTINATION), so "nav
    /// now Seeking station-B-body" is a meaningful, non-origin target. One
    /// corporation (treasury 5_000_000µ, home station A) funds one Offered contract:
    /// move 5 Fuel A->B for reward 1_000_000µ. Station A opens with 10 Fuel in stock
    /// so a 5-unit load is covered. The single craft is Idle.
    fn two_body_contract_fixture() -> RunConfig {
        use crate::config::{ContractInit, CorporationInit, StationInit};
        use crate::economy::Good;
        let mut cfg = one_body_one_thrusting_craft();
        // Drop the central body's mass so (a) the craft co-located at the origin is
        // not gravity-trapped and (b) station B's body is near-stationary — a from-rest
        // craft with a ~6.9e-3 Δv budget can only rendezvous with a slow frame. With
        // mu = G_CANONICAL·(m_central + m_body) the destination's orbital speed
        // collapses to ~1e-6 AU/day, a fixed point for the autopilot. (Fixture params
        // are NOT hashed state; no golden moves.)
        cfg.bodies[0].mass = 1e-9;
        // Add a second body at a reachable 0.3 AU orbit (station B's host) — inside the
        // proven free-space-transfer envelope (cf. physics_sanity transfer tests).
        cfg.bodies.push(BodyInit {
            mass: 1e-12, // negligible-mass marker body; only its position matters
            elements: OrbitalElements {
                a: 0.3,
                e: 0.0,
                i: 0.0,
                raan: 0.0,
                argp: 0.0,
                m0: 0.0,
            },
        });
        // The craft starts co-located with body 0 (the origin star) so it is at
        // station A's body when the contract is accepted.
        cfg.craft[0].pos = Vec3::ZERO;
        cfg.craft[0].vel = Vec3::ZERO;
        cfg.stations = vec![
            // Station A (origin): 10 Fuel in stock to cover the 5-unit load.
            StationInit {
                body_index: 0,
                initial_stock: vec![0i64, 10i64],
                initial_price_micros: vec![0i64, 0i64],
                sells_upgrades: false,
            },
            // Station B (the delivery destination).
            StationInit {
                body_index: 1,
                initial_stock: vec![0i64, 0i64],
                initial_price_micros: vec![0i64, 0i64],
                sells_upgrades: false,
            },
        ];
        cfg.corporations = vec![CorporationInit {
            treasury_micros: 5_000_000,
            home_station_index: 0,
        }];
        cfg.contracts = vec![ContractInit {
            corp_index: 0,
            resource: Good::FUEL,
            qty: 5,
            from_station_index: 0,
            to_station_index: 1,
            reward_micros: 1_000_000,
        }];
        cfg
    }

    /// A FAST-ORBIT variant of `two_body_contract_fixture`: the contract ORIGIN
    /// station rides body 1 on a tight a = 0.05 AU circular orbit around the
    /// full-mass (1 M_sun) central star, so the pickup body moves
    /// v·dt ≈ 0.077 AU/day · 0.25 day ≈ 0.019 AU ≈ 190× ARRIVAL_RADIUS per
    /// tick. The craft spawns EXACTLY co-located and co-orbiting with body 1
    /// (for e=0/i=0 conics the ephemeris state at m0=0 is pos = (a, 0, 0),
    /// vel = v_circ·(0, 1, 0) with v_circ = sqrt(mu/a), mu = G·(M_star+m_body)).
    /// Regression fixture for the try_load frame fix: `resolve_contracts` runs
    /// PRE-physics, so `ships.pos` is the tick t-1 state and the co-location
    /// gate must sample `body_pos` at the SAME tick — sampling at t put the
    /// body ~190 arrival-radii ahead and the load starved forever.
    fn fast_orbit_pickup_fixture() -> RunConfig {
        use crate::config::{ContractInit, CorporationInit, StationInit};
        use crate::economy::Good;
        let mut cfg = one_body_one_thrusting_craft();
        // Body 1: the fast-orbit pickup host (negligible-mass marker body).
        let a = 0.05;
        let body_mass = 1e-12;
        cfg.bodies.push(BodyInit {
            mass: body_mass,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
        });
        // Craft co-located + co-orbiting with body 1 at tick 0.
        let v_circ = (crate::G_CANONICAL * (1.0 + body_mass) / a).sqrt();
        cfg.craft[0].pos = Vec3::new(a, 0.0, 0.0);
        cfg.craft[0].vel = Vec3::new(0.0, v_circ, 0.0);
        cfg.stations = vec![
            // Station A (origin) on the FAST body: 10 Fuel covers the 5-unit load.
            StationInit { body_index: 1, initial_stock: vec![0i64, 10i64], initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
            // Station B (destination) on the central star.
            StationInit { body_index: 0, initial_stock: vec![0i64, 0i64], initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        ];
        cfg.corporations =
            vec![CorporationInit { treasury_micros: 5_000_000, home_station_index: 0 }];
        cfg.contracts = vec![ContractInit {
            corp_index: 0,
            resource: Good::FUEL,
            qty: 5,
            from_station_index: 0,
            to_station_index: 1,
            reward_micros: 1_000_000,
        }];
        cfg
    }

    #[test]
    fn try_load_compares_craft_and_body_in_the_same_frame() {
        use crate::economy::{ContractStatus, Good};
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(fast_orbit_pickup_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let contract = world
            .contracts
            .ids
            .id_at(0)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        // The frame-correct gate loads same-tick: ships.pos (the tick-0
        // pre-physics state) vs body_pos(0) is an EXACT co-location. The
        // frame-mixed gate (body_pos(1)) saw the body ~190x ARRIVAL_RADIUS
        // ahead and left the contract Accepted, chasing a body it can never
        // catch (orbital speed 0.077 AU/day vs craft a_max·day ~ 1e-3).
        assert_eq!(
            world.contracts.status[0],
            ContractStatus::CargoLoaded,
            "co-located fast-orbit pickup loads on step 1"
        );
        let crow = world.ships.index_of(craft).unwrap();
        assert_eq!(
            world.ships.cargo[crow],
            Some((Good::FUEL, 5)),
            "cargo transferred from the fast-orbit station"
        );
    }

    /// A STARVED variant of `two_body_contract_fixture`: the craft can still accept
    /// and load at station A (loading pulls economy Fuel cargo from A's stock, which
    /// is independent of propellant `fuel_mass`), but its propellant is exhausted
    /// mid-transit before it can rendezvous with station B, so a `FuelEmpty` event
    /// fires while the contract is `InTransit`. The lever is `fuel_mass`: it starts
    /// just above `FUEL_EMPTY_EPS` (1e-11), enough to survive step 1 (still
    /// `CargoLoaded`, so the once-only FuelEmpty edge must NOT fire there) but drained
    /// across the eps threshold a couple of ticks into the burn, long before the craft
    /// can cover the 0.3 AU to station B.
    fn two_body_starved_contract_fixture() -> RunConfig {
        let mut cfg = two_body_contract_fixture();
        // REDESIGNED (not nudged) for the eps re-bake 1e-9 -> 1e-11 (spec §4
        // item 1; was 1.06e-9 = old eps + 6e-11). Same 6e-11 headroom above the
        // NEW eps = 2.4 full-throttle burn ticks (burn/tick = max_thrust/v_e*dt
        // = 1e-12/1e-2*0.25 = 2.5e-11). Tick 1 (load+dispatch): 7e-11 -> 4.5e-11
        // (survives step 1; the once-only edge must NOT fire while CargoLoaded);
        // tick 2: -> 2e-11; tick 3: clamped to 0 <= eps with prev 2e-11 > eps ->
        // FuelEmpty fires while InTransit (the stage-1c promotion ran on tick 2),
        // long before the craft covers the 0.3 AU to station B.
        cfg.craft[0].fuel_mass = 7.0e-11;
        cfg
    }

    #[test]
    fn starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss() {
        use crate::economy::{ContractStatus, Good};
        use crate::stores::CraftRole;
        use crate::types::{EntityRef, Target};
        let (mut world, _h) =
            World::reset(two_body_starved_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0usize; // sole contract, dense row 0
        let fuel = Good::FUEL.index();

        // Credit identity baseline (escrow is corp money held off-balance-sheet).
        let initial_credit = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();
        let initial_treasury = world.corporations.treasury_micros[0];
        let consumed_fuel_before = world.econ.consumed[fuel];

        // Accept the sole Offered contract (escrow + load on step 1).
        let contract = world
            .contracts
            .ids
            .id_at(cidx)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert_eq!(
            world.contracts.status[cidx],
            ContractStatus::CargoLoaded,
            "loaded at origin on step 1"
        );
        // The loaded cargo qty (debited from A at load); lost when the contract fails.
        let crow0 = world.ships.index_of(craft).unwrap();
        let (lost_res, lost_qty) = world.ships.cargo[crow0].expect("cargo loaded on step 1");
        assert_eq!((lost_res, lost_qty), (Good::FUEL, 5));

        // Step (no commands) until a FuelEmpty fires while InTransit and the failure
        // stage settles the contract. Bounded loop; break on Failed.
        let mut empty: Vec<Command> = Vec::new();
        let mut failed = false;
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.contracts.status[cidx] == ContractStatus::Failed {
                failed = true;
                break;
            }
        }
        assert!(failed, "contract reached Failed within the step bound");

        // Escrow refunded to the owning corp; corp treasury back to its pre-accept
        // value; escrow zeroed.
        assert_eq!(world.contracts.escrow_micros[cidx], 0, "escrow zeroed on fail");
        assert_eq!(
            world.corporations.treasury_micros[0], initial_treasury,
            "escrow refunded to corp treasury"
        );

        // Craft cargo/contract handle cleared; role back to Idle.
        let crow = world.ships.index_of(craft).unwrap();
        assert_eq!(world.ships.cargo[crow], None, "cargo cleared (lost) on fail");
        assert_eq!(world.ships.contract[crow], None, "contract handle cleared");
        assert_eq!(world.ships.role[crow], CraftRole::Idle, "role back to Idle");

        // Cargo-loss accounting leg: consumed[Fuel] rose by the lost cargo qty.
        assert_eq!(
            world.econ.consumed[fuel],
            consumed_fuel_before + lost_qty as i64,
            "lost cargo accounted into consumed[Fuel]"
        );

        // Global credit identity holds: refund creates/destroys no money.
        let final_credit = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();
        assert_eq!(final_credit, initial_credit, "Σtreasury+Σcredits+Σescrow invariant");
    }

    #[test]
    fn accept_contract_escrows_loads_cargo_and_dispatches_hauler() {
        use crate::economy::{ContractStatus, Good};
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0usize; // sole contract, dense row 0
        let fuel = Good::FUEL.index();

        // Issue AcceptContract for the sole Offered contract.
        let contract = world
            .contracts
            .ids
            .id_at(cidx)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];

        // One step: ingest sets craft.contract + role Hauler; resolve_contracts then
        // escrows (Offered->Accepted) and loads at the co-located origin (->CargoLoaded).
        world.step(&mut cmds);

        // Contract escrowed and loaded (final status CargoLoaded after one step).
        assert_eq!(world.contracts.status[cidx], ContractStatus::CargoLoaded);
        assert_eq!(world.contracts.escrow_micros[cidx], 1_000_000, "escrow == reward");
        assert_eq!(world.contracts.hauler[cidx], Some(craft), "hauler bound");
        // Corp treasury debited by the reward (escrow held off-balance-sheet).
        assert_eq!(world.corporations.treasury_micros[0], 4_000_000);
        // Cargo loaded onto the craft; station A Fuel stock dropped by 5.
        let crow = world.ships.index_of(craft).unwrap();
        assert_eq!(world.ships.cargo[crow], Some((Good::FUEL, 5)));
        assert_eq!(world.stations.stock[0][fuel], 5, "station A Fuel 10 -> 5");
        // The craft is now dispatched: Seeking station B's body.
        let to_body = world
            .bodies
            .ids
            .id_at(1)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        match world.ships.nav[crow] {
            NavState::Seeking { dest, .. } => assert_eq!(
                dest,
                NavDest::Entity(EntityRef::Body(to_body)),
                "nav Seeking station-B body"
            ),
            other => panic!("expected Seeking B, got {other:?}"),
        }
        // ContractAccepted was emitted for this craft/contract.
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractAccepted { contract: c, hauler } if c == contract && hauler == craft
            )),
            "ContractAccepted emitted"
        );
    }

    /// FULL Stage-1 self-running fixture (Task 17). Extends the two-body contract
    /// world with a complete production chain so the loop is closed:
    ///   * station A (body 0, origin): a ∅->Ore(5) MINER and an Ore(2)->Fuel(2)
    ///     REFINER, so A continuously restocks Fuel for the route to load.
    ///   * station B (body 1, destination): a Fuel(1)->∅ DEMAND SINK that consumes
    ///     delivered Fuel each tick (its input leg bumps consumed[Fuel] for free).
    ///   * one corp (treasury funds the escrowed reward), one Idle hauler co-located
    ///     at A's body, and ONE seeded ContractInit (the route TEMPLATE: Fuel A->B).
    ///
    /// `demand_low` is set above the per-contract qty so the sink keeps B below the
    /// trigger and a repost is warranted after the first delivery.
    fn full_stage1_self_running_fixture() -> RunConfig {
        use crate::config::{CorporationInit, ProducerInit};
        use crate::economy::{Recipe, Good};
        let mut cfg = two_body_contract_fixture();
        // Producers on station A (origin) + a demand sink on station B (destination).
        cfg.producers = vec![
            // Miner: ∅ -> Ore(5) every tick.
            ProducerInit {
                station_index: 0,
                recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 1 },
            },
            // Refiner: Ore(2) -> Fuel(2) every tick (keeps A restocked with Fuel).
            ProducerInit {
                station_index: 0,
                recipe: Recipe {
                    input: Some((Good::ORE, 2)),
                    output: Some((Good::FUEL, 2)),
                    interval: 1,
                },
            },
            // Demand sink at B: Fuel(1) -> ∅ every tick (consumes delivered Fuel).
            ProducerInit {
                station_index: 1,
                recipe: Recipe { input: Some((Good::FUEL, 1)), output: None, interval: 1 },
            },
        ];
        // Trigger repost while B's stock sits below demand_low (5-unit deliveries,
        // demand_low 10 -> demand persists right after a delivery).
        cfg.dispatch_cfg.demand_low = 10;
        cfg.dispatch_cfg.demand_high = 20;
        cfg.dispatch_cfg.contract_reward_micros = 1_000_000;
        cfg.dispatch_cfg.contract_qty = 5;
        // The seeded contract (from two_body_contract_fixture): Fuel 5, A->B, reward
        // 1_000_000 — the route TEMPLATE every repost clones. Verify it is present.
        assert_eq!(cfg.contracts.len(), 1, "exactly one seeded route template");
        // Give the corp enough treasury to fund repeated reposts' escrows.
        cfg.corporations = vec![CorporationInit {
            treasury_micros: 100_000_000,
            home_station_index: 0,
        }];
        cfg
    }

    #[test]
    fn scripted_dispatch_makes_stage1_loop_self_run() {
        use crate::economy::{ContractStatus, N_GOODS_V1, Good};
        let (mut world, _h) =
            World::reset(full_stage1_self_running_fixture()).expect("resolvable cfg");
        let fuel = Good::FUEL.index();

        // Resource accounting identity baseline: initial[r] = Σ station stock at tick 0
        // (mined == consumed == 0 at reset).
        let mut initial = [0i64; N_GOODS_V1];
        for r in 0..N_GOODS_V1 {
            initial[r] = world.stations.stock.iter().map(|s| s[r]).sum();
        }

        // Identity check helper: Σstock(r) + Σin_transit_cargo(r) == initial(r)
        // + mined(r) - consumed(r), per resource, EVERY tick.
        let check_identity = |w: &World, tag: &str| {
            for r in 0..N_GOODS_V1 {
                let stock: i64 = w.stations.stock.iter().map(|s| s[r]).sum();
                let in_transit: i64 = w
                    .ships
                    .cargo
                    .iter()
                    .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
                    .sum();
                let lhs = stock + in_transit;
                let rhs = initial[r] + w.econ.mined[r] - w.econ.consumed[r];
                assert_eq!(
                    lhs, rhs,
                    "resource identity ({tag}) for r={r}: {lhs} != {rhs}"
                );
            }
        };

        check_identity(&world, "tick 0");

        // Self-run: NO external commands. The scripted policy must assign the Idle
        // hauler to the seeded Offered contract, load + dispatch it, deliver at B, and
        // (because B stays below demand_low) repost a clone of the route.
        let mut empty: Vec<Command> = Vec::new();
        let mut completed = false;
        let mut b_fuel_seen_positive = false;
        for _ in 0..6000 {
            world.step(&mut empty);
            check_identity(&world, "per-tick");
            if world.stations.stock[1][fuel] > 0 {
                b_fuel_seen_positive = true;
            }
            if world.contracts.status.contains(&ContractStatus::Completed) {
                completed = true;
                break;
            }
        }
        assert!(completed, "at least one contract self-completed within the step bound");
        assert!(b_fuel_seen_positive, "station B Fuel stock saw deliveries (>0 at some point)");

        // Run a few ticks past the first completion so the REPOST branch fires (B's
        // stock is below demand_low after the sink consumes the delivery).
        for _ in 0..50 {
            world.step(&mut empty);
            check_identity(&world, "post-completion");
        }
        assert!(
            world.contracts.ids.len() > 1,
            "the scripted policy reposted at least one fresh contract (route clone)"
        );
    }

    #[test]
    fn deliver_on_arrival_settles_escrow_and_holds_credit_identity() {
        use crate::economy::{ContractStatus, Good};
        use crate::stores::CraftRole;
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0usize; // sole contract, dense row 0
        let fuel = Good::FUEL.index();

        // Credit identity baseline: Σtreasury + Σcredits + Σescrow is invariant
        // (escrow is corp money held off-balance-sheet, paid to the craft on
        // delivery). Capture it before any contract motion.
        let initial_credit = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();

        // Accept the sole Offered contract (escrow + load happen on step 1).
        let contract = world
            .contracts
            .ids
            .id_at(cidx)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert_eq!(
            world.contracts.status[cidx],
            ContractStatus::CargoLoaded,
            "loaded at origin on step 1"
        );

        // Step (no further commands) until the hauler rendezvous-arrives at station B
        // and the delivery stage settles the contract. Bounded loop: the destination
        // body 1 sits at a == 0.3 AU and the craft has ample Δv, so this converges
        // well within the bound; break on Completed, fail if it never lands.
        let mut empty: Vec<Command> = Vec::new();
        let mut completed = false;
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.contracts.status[cidx] == ContractStatus::Completed {
                completed = true;
                break;
            }
        }
        assert!(completed, "contract reached Completed within the step bound");

        // Cargo unloaded into station B (+5 Fuel); station B opened at 0.
        let crow = world.ships.index_of(craft).unwrap();
        assert_eq!(world.stations.stock[1][fuel], 5, "station B Fuel 0 -> 5");
        // Escrow paid out to the craft; escrow zeroed.
        assert_eq!(world.contracts.escrow_micros[cidx], 0, "escrow drained");
        assert_eq!(
            world.ships.credits_micros[crow], 1_000_000,
            "escrow settled to craft credits"
        );
        // Craft cargo/contract handle cleared; role back to Idle.
        assert_eq!(world.ships.cargo[crow], None, "cargo cleared");
        assert_eq!(world.ships.contract[crow], None, "contract handle cleared");
        assert_eq!(world.ships.role[crow], CraftRole::Idle, "role cleared");

        // ContractFulfilled emitted for this craft/contract.
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFulfilled { contract: c, hauler } if c == contract && hauler == craft
            )),
            "ContractFulfilled emitted"
        );

        // Global credit identity holds: no money created or destroyed by delivery.
        let final_credit = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();
        assert_eq!(final_credit, initial_credit, "Σtreasury+Σcredits+Σescrow invariant");
    }

    /// Phase-0b fuel instrument: one delivery run brackets exactly one
    /// contract leg and moves every per-craft diagnostic channel. UNHASHED:
    /// no golden involvement.
    #[test]
    fn fuel_diag_brackets_the_delivery_leg_and_tracks_burn() {
        use crate::economy::ContractStatus;
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0;
        let contract = world
            .contracts
            .ids
            .id_at(cidx)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert!(
            world.fuel_diag.leg_start_fuel[0].is_some(),
            "accept opens the leg bracket at the current tank"
        );

        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.contracts.status[cidx] == ContractStatus::Completed {
                break;
            }
        }
        assert_eq!(
            world.contracts.status[cidx],
            ContractStatus::Completed,
            "delivery completed within the step bound"
        );
        assert_eq!(world.fuel_diag.leg_burns.len(), 1, "exactly one leg closed");
        let (close_tick, burn_permille) = world.fuel_diag.leg_burns[0];
        assert!(close_tick.0 > 0, "leg closed at a real tick");
        assert!(burn_permille <= 1000, "leg burn is a permille of capacity");
        assert_eq!(world.fuel_diag.leg_start_fuel[0], None, "bracket consumed at close");
        assert!(world.fuel_diag.thrust_ticks[0] > 0, "duty counted thrusting ticks");
        assert!(world.fuel_diag.burned_mass[0] > 0.0, "cumulative burn accumulated");
        assert!(
            world.fuel_diag.min_fuel_mass[0] <= world.ships.fuel_mass[0],
            "low-water mark never exceeds the live tank"
        );
    }

    #[test]
    fn trader_read_accessors_expose_board_and_wallet() {
        // The four read-only trader accessors (strategic-layer board/wallet reads):
        // offered_contracts / station_pos / craft_credits / craft_is_idle. A contract
        // is ON the board iff status == Offered AND hauler is None AND no craft holds
        // accept-intent for it (`ships.contract` pointing at it pre-resolve).
        use crate::ids::{ContractId, StationId};
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let crow = world.ships.index_of(craft).unwrap();

        // (1) Board: exactly the seeded Offered row, with its route fields.
        let board = world.offered_contracts();
        assert_eq!(board.len(), 1, "one seeded Offered contract");
        let (cid, reward, from, to) = board[0];
        assert_eq!(cid, ContractId { slot: 0, generation: 0 });
        assert_eq!(reward, 1_000_000);
        assert_eq!(from, StationId { slot: 0, generation: 0 });
        assert_eq!(to, StationId { slot: 1, generation: 0 });

        // (2) station_pos == the station's body position at the current tick (the
        // same eph read the projection makes); stale id -> None.
        let view = world.project(&FullObserver);
        assert_eq!(world.station_pos(from), view.body_pos(world.stations.body[0]));
        assert_eq!(world.station_pos(to), view.body_pos(world.stations.body[1]));
        assert_eq!(world.station_pos(StationId { slot: 99, generation: 0 }), None);

        // (3) Wallet/idleness before any motion; stale craft id -> None.
        assert_eq!(world.craft_credits(craft), Some(0));
        assert_eq!(world.craft_is_idle(craft), Some(true));
        let stale = CraftId { slot: 99, generation: 0 };
        assert_eq!(world.craft_credits(stale), None);
        assert_eq!(world.craft_is_idle(stale), None);

        // (4) Accept-INTENT alone (ships.contract set pre-resolve; contract still
        // Offered + hauler None) takes the slot OFF the board.
        world.ships.contract[crow] = Some(cid);
        assert!(world.offered_contracts().is_empty(), "intent claims the slot");
        assert_eq!(world.craft_is_idle(craft), Some(false), "intent breaks idleness");
        world.ships.contract[crow] = None;
        assert_eq!(world.offered_contracts().len(), 1, "board restored");

        // (5) Real accept through the single ingest path: the board empties (status
        // leaves Offered) and the craft is no longer idle.
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        assert!(world.offered_contracts().is_empty(), "accepted contract off the board");
        assert_eq!(world.craft_is_idle(craft), Some(false));

        // (6) Drive to delivery: escrow settles into the wallet, idle again.
        let mut empty: Vec<Command> = Vec::new();
        let mut paid = false;
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.craft_credits(craft) == Some(1_000_000) {
                paid = true;
                break;
            }
        }
        assert!(paid, "escrow settled into craft_credits within the step bound");
        assert_eq!(world.craft_is_idle(craft), Some(true), "idle again after delivery");
    }

    // ---- Task 18: the PHASE-1 gate -----------------------------------------
    //
    // Two verification tests over the FULL Stage-1 fixture (`full_stage1_self_running_fixture`,
    // T17): (1) the per-resource accounting identity holds every tick, and (2) replay is
    // deterministic (two instances from the same config produce bit-identical state_hash
    // sequences). Neither moves any golden — the identity is an internal invariant and the
    // determinism test is a cross-INSTANCE equality, not a pinned constant (per the
    // digest-tests-are-determinism-not-golden principle).

    /// PER RESOURCE r: Σ over stations of stock[*][r] + Σ over ALL craft of (cargo qty
    /// where cargo == Some((r, q))) == initial[r] + econ.mined[r] − econ.consumed[r].
    /// `initial[r]` is the Σ-station-stock-per-resource snapshot captured right after
    /// `World::reset` (tick 0, when mined == consumed == 0). Mirrors the inline check the
    /// T17 self-run test runs through completion.
    fn assert_resource_identity(world: &World, initial: &[i64; crate::economy::N_GOODS_V1]) {
        for r in 0..crate::economy::N_GOODS_V1 {
            let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
            let in_transit: i64 = world
                .ships
                .cargo
                .iter()
                .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
                .sum();
            let lhs = stock + in_transit;
            let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
            assert_eq!(
                lhs, rhs,
                "resource identity for r={r}: {lhs} != {rhs} (stock+in_transit vs initial+mined-consumed)"
            );
        }
    }

    #[test]
    fn phase1_gate_resource_accounting_identity_holds_every_tick() {
        use crate::economy::{N_GOODS_V1, Good};
        let (mut world, _h) =
            World::reset(full_stage1_self_running_fixture()).expect("resolvable cfg");

        // initial[r] = Σ station stock per resource at tick 0 (mined == consumed == 0).
        let mut initial = [0i64; N_GOODS_V1];
        for (r, slot) in initial.iter_mut().enumerate() {
            *slot = world.stations.stock.iter().map(|s| s[r]).sum();
        }

        // Identity holds at reset, and EVERY tick of the self-running fixture. The
        // window must be long enough to drive ALL five legs (mine, refine, load,
        // deliver, sink-consume) under the identity, not just the producer side. The
        // destination sits at 0.3 AU, so the first delivery + sink-consume lands well
        // after tick ~696; run 1000 ticks so the consume leg is genuinely exercised
        // here (the non-vacuity guards below would fail on a too-short window).
        assert_resource_identity(&world, &initial);
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..1000 {
            world.step(&mut empty);
            assert_resource_identity(&world, &initial);
        }

        // Non-vacuity guards: a stalled world satisfies the identity trivially, so
        // assert every leg of the identity actually fired — miner (mined Ore), refiner
        // output (mined Fuel) and input (consumed Ore), and the deliver→sink-consume
        // path (consumed Fuel > 0 proves Fuel reached station B and was consumed there,
        // which is only possible after a contract delivered). (The Failed→consumed leg
        // never fires on this healthy fixture; it is covered by T16's targeted test.)
        let ore = Good::ORE.index();
        let fuel = Good::FUEL.index();
        assert!(world.econ.mined[ore] > 0, "miner is live (mined Ore > 0)");
        assert!(world.econ.mined[fuel] > 0, "refiner output leg live (mined Fuel > 0)");
        assert!(world.econ.consumed[ore] > 0, "refiner input leg live (consumed Ore > 0)");
        assert!(
            world.econ.consumed[fuel] > 0,
            "deliver + sink-consume legs fired (consumed Fuel > 0)"
        );
    }

    #[test]
    fn phase1_gate_replay_is_deterministic_state_hash_tick_by_tick() {
        // Build TWO worlds from the SAME config and the SAME (empty) command inputs.
        // step() takes the command vec mutably, so give each world its OWN vec to rule
        // out any cross-contamination.
        let (mut world_a, _ha) =
            World::reset(full_stage1_self_running_fixture()).expect("resolvable cfg");
        let (mut world_b, _hb) =
            World::reset(full_stage1_self_running_fixture()).expect("resolvable cfg");

        let h0 = crate::hash::state_hash(&world_a);
        assert_eq!(h0, crate::hash::state_hash(&world_b), "tick 0 hashes agree");

        let mut empty_a: Vec<Command> = Vec::new();
        let mut empty_b: Vec<Command> = Vec::new();
        let mut last = h0;
        for t in 1..=200 {
            world_a.step(&mut empty_a);
            world_b.step(&mut empty_b);
            let ha = crate::hash::state_hash(&world_a);
            let hb = crate::hash::state_hash(&world_b);
            assert_eq!(ha, hb, "replay determinism: state_hash diverged at tick {t}");
            last = ha;
        }

        // Non-vacuity guard: the hash sequence must actually EVOLVE over the run — two
        // constant sequences would compare equal trivially.
        assert_ne!(last, h0, "state_hash evolved over the 200-tick run (not a constant)");
    }

    /// Reprice-clock fixture (Task 20): one station (Fuel price live, Ore inert) with a
    /// ∅->Fuel(5) miner at interval 1, so the station's Fuel stock GROWS every tick
    /// while the reprice clock only recomputes the price on ticks that are multiples of
    /// `reprice_interval`. `base_micros[Fuel]=100_000`, `cap[Fuel]=100`,
    /// `slope_milli=1000`, so `p(s) = 100_000*(2000 - s*1000/100)/1000 = 100_000*(2000 -
    /// 10*s)/1000`; with `reprice_interval=4`, stock at reprice tick `t` is `5*t`, so
    /// price lands at `t=4 -> s=20 -> 180_000`, `t=8 -> s=40 -> 160_000`, `t=12 -> s=60
    /// -> 140_000` (all exact integers, all stocks < cap). `base_micros[Ore]=0` and
    /// `initial_price_micros=[0,0]` keep Ore inert (its computed price is also 0, so no
    /// spurious change there).
    fn one_station_growing_fuel_reprice_4() -> RunConfig {
        use crate::config::{PriceCfg, ProducerInit, StationInit};
        use crate::economy::{Recipe, Good};
        let mut cfg = one_body_one_craft();
        cfg.stations = vec![StationInit {
            body_index: 0,
            initial_stock: vec![0i64, 0i64],
            initial_price_micros: vec![0i64, 0i64],
            sells_upgrades: false,
        }];
        cfg.producers = vec![ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Good::FUEL, 5)), interval: 1 },
        }];
        cfg.price_cfg = PriceCfg {
            base_micros: vec![0i64, 100_000i64],
            cap: vec![0i64, 100i64],
            slope_milli: 1000,
            reprice_interval: 4,
        };
        cfg
    }

    #[test]
    fn step_reprice_clock_is_tick_gated_and_deterministic() {
        use crate::economy::Good;
        let fi = Good::FUEL.index();

        let (mut world, _) =
            World::reset(one_station_growing_fuel_reprice_4()).expect("resolvable config");
        let mut empty: Vec<Command> = Vec::new();

        // Capture the Fuel price AFTER each of 12 steps (i.e. priced[t-1] is the price
        // observed after stepping to tick t). The miner adds 5 Fuel/tick, so stock keeps
        // climbing every tick — but the price must only CHANGE on reprice ticks (t%4==0).
        let mut priced = [0i64; 12];
        for (i, slot) in priced.iter_mut().enumerate() {
            world.step(&mut empty);
            *slot = world.stations.price_micros[0][fi];
            // Stock truly moves every tick (5 per firing), proving constancy below is the
            // clock gating the price, not a frozen stock.
            assert_eq!(
                world.stations.stock[0][fi],
                5 * (i as i64 + 1),
                "Fuel stock grows 5/tick (clock-gated price is not just frozen stock)"
            );
        }

        // Price stays at its opening value (0) through ticks 1..3, then RECOMPUTES at t=4.
        assert_eq!(priced[0], 0, "t=1 (not a reprice tick): price unchanged from open (0)");
        assert_eq!(priced[1], 0, "t=2 (not a reprice tick): price unchanged");
        assert_eq!(priced[2], 0, "t=3 (not a reprice tick): price unchanged");
        assert_eq!(priced[3], 180_000, "t=4 reprice: s=20 -> 100_000*(2000-200)/1000");
        // Constant between reprice ticks even as stock moves (20 -> 35 over t=4..7).
        assert_eq!(priced[4], 180_000, "t=5: held constant despite stock 25");
        assert_eq!(priced[5], 180_000, "t=6: held constant despite stock 30");
        assert_eq!(priced[6], 180_000, "t=7: held constant despite stock 35");
        assert_eq!(priced[7], 160_000, "t=8 reprice: s=40 -> 100_000*(2000-400)/1000");
        assert_eq!(priced[8], 160_000, "t=9: held constant");
        assert_eq!(priced[9], 160_000, "t=10: held constant");
        assert_eq!(priced[10], 160_000, "t=11: held constant");
        assert_eq!(priced[11], 140_000, "t=12 reprice: s=60 -> 100_000*(2000-600)/1000");

        // The constancy-despite-moving-stock claim, asserted on both edges:
        assert_ne!(priced[3], priced[2], "price CHANGES at the reprice tick (t=4 vs t=3)");
        assert_eq!(priced[4], priced[3], "price HELD between reprice ticks (t=5 == t=4)");

        // Determinism leg: two worlds, same config, identical empty inputs -> identical
        // price sequence.
        let (mut wa, _) =
            World::reset(one_station_growing_fuel_reprice_4()).expect("resolvable config");
        let (mut wb, _) =
            World::reset(one_station_growing_fuel_reprice_4()).expect("resolvable config");
        let mut ea: Vec<Command> = Vec::new();
        let mut eb: Vec<Command> = Vec::new();
        for t in 1..=12 {
            wa.step(&mut ea);
            wb.step(&mut eb);
            assert_eq!(
                wa.stations.price_micros[0][fi],
                wb.stations.price_micros[0][fi],
                "reprice sequence diverged at tick {t}"
            );
        }
    }

    // ---- Task 21: hysteresis deadband + staggered dispatch (Stage-2 stability) ----
    //
    // A COMPARATIVE A/B harness. ONE fixture, ONE driven-loop body, run on TWO
    // dispatch configs that differ ONLY in the new knobs (`demand_high`,
    // `stagger_period`). The fixture is a closed loop: 4 Idle haulers co-located at
    // origin station A, a miner+refiner restocking A's Fuel, a Fuel demand SINK at a
    // SHORT-transit destination B (a == 0.02 AU; first delivery ~tick 58), one corp,
    // and one seeded `ContractInit` route template (Fuel A->B). Stage-2 pricing is
    // live (`base_micros[Fuel] > 0`) so price actually moves with stock.

    /// Build the Stage-2 A/B closed-loop fixture for the named dispatch knobs. Pure
    /// fixture params (haulers, transit distance, sink rate) are NOT hashed state — no
    /// golden moves. `demand_low` is fixed; `demand_high`/`stagger_period` are the
    /// A/B levers.
    fn stage2_ab_loop_fixture(demand_high: i64, stagger_period: u32) -> RunConfig {
        use crate::config::{ContractInit, CorporationInit, ProducerInit, StationInit};
        use crate::economy::{Recipe, Good};
        let fuel = Good::FUEL.index();
        let mut cfg = one_body_one_thrusting_craft();
        // Negligible central mass so B's body is near-stationary and the from-rest
        // hauler can rendezvous (same trick as two_body_contract_fixture).
        cfg.bodies[0].mass = 1e-9;
        // Destination body B at a SHORT 0.02 AU orbit -> first delivery ~tick 58, so a
        // full post->dispatch->deliver wave fits well inside the 1000-tick window.
        cfg.bodies.push(BodyInit {
            mass: 1e-12,
            elements: OrbitalElements { a: 0.02, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
        });
        // FOUR Idle haulers, all co-located at origin body 0 (so staggered dispatch is
        // meaningful: >1 hauler can claim the burst).
        let mut proto = cfg.craft[0].clone();
        // Foragers need propellant for MANY round trips, not one delivery. Raise ONLY
        // the exhaust velocity (Δv = v_e·ln(wet/dry)): it feeds the Δv budget + fuel-burn
        // rate but NOT the integrator's thrust/mass acceleration, so trips stay fast and
        // the §6 anti-tunnel guard is untouched — while the tank lasts ~dozens of legs.
        // (Adding fuel MASS instead would balloon wet mass and slow every trip ~50x.)
        proto.spec.base_exhaust_velocity = 1.0;
        cfg.craft = vec![proto.clone(), proto.clone(), proto.clone(), proto];
        for c in cfg.craft.iter_mut() {
            c.pos = Vec3::ZERO;
            c.vel = Vec3::ZERO;
        }
        cfg.stations = vec![
            // Station A (origin): deep Fuel stock + producers keep it restocked.
            StationInit { body_index: 0, initial_stock: vec![0i64, 1000i64], initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
            // Station B (destination): a Fuel demand sink drains it each tick.
            StationInit { body_index: 1, initial_stock: vec![0i64, 0i64], initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        ];
        cfg.producers = vec![
            ProducerInit { station_index: 0, recipe: Recipe { input: None, output: Some((Good::ORE, 20)), interval: 1 } },
            ProducerInit { station_index: 0, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 1 } },
            // Sink at B: consumes 5 Fuel/tick (== qty) so a staggered arrival is fully
            // drained before the next lands -> staggering visibly flattens the peak.
            ProducerInit { station_index: 1, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 1 } },
        ];
        cfg.corporations = vec![CorporationInit { treasury_micros: 100_000_000_000, home_station_index: 0 }];
        // Seeded route template (Fuel A->B, qty 5) — the order-up-to repost clones it.
        cfg.contracts = vec![ContractInit {
            corp_index: 0, resource: Good::FUEL, qty: 5,
            from_station_index: 0, to_station_index: 1, reward_micros: 1_000,
        }];
        // Stage-2 pricing LIVE so price actually moves (named fixture constraint).
        cfg.price_cfg.base_micros[fuel] = 100_000;
        cfg.price_cfg.cap[fuel] = 100;
        // demand_low fixed at 20 (== 4 haulers x qty 5): a cold-start burst posts 4
        // contracts -> one per hauler. demand_high/stagger_period are the A/B levers.
        cfg.dispatch_cfg.demand_low = 20;
        cfg.dispatch_cfg.demand_high = demand_high;
        cfg.dispatch_cfg.stagger_period = stagger_period;
        cfg.dispatch_cfg.contract_reward_micros = 1_000;
        cfg.dispatch_cfg.contract_qty = 5;
        cfg
    }

    /// One 1000-tick Stage-2 run with NO external commands, reduced to the metrics the
    /// stability A/B/C compares.
    struct Stage2Run {
        /// max station-B Fuel stock over the whole run (the clumped-vs-spread overshoot).
        peak: i64,
        /// max - min of station-B Fuel over the last 200 ticks (steady-state jitter band).
        band: i64,
        /// DISTINCT ticks on which the first 4 `ContractAccepted` events fired (how spread
        /// the opening dispatch wave is).
        accept_ticks: std::collections::BTreeSet<u64>,
        /// max simultaneous NON-terminal (Offered/Accepted/CargoLoaded/InTransit) contracts
        /// for the route — the depth of the order-up-to in-flight buffer.
        max_in_flight: usize,
        /// total `ContractFulfilled` events — proves the forage loop sustained (vs jammed).
        completions: usize,
    }

    /// Drive the A/B fixture 1000 ticks and reduce it to a [`Stage2Run`].
    fn drive_stage2_loop(cfg: RunConfig) -> Stage2Run {
        use crate::economy::ContractStatus;
        let fuel = crate::economy::Good::FUEL.index();
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let mut empty: Vec<Command> = Vec::new();
        let mut series = Vec::with_capacity(1000);
        let mut accept_ticks = std::collections::BTreeSet::new();
        let mut accepts_seen = 0usize;
        let mut max_in_flight = 0usize;
        let mut completions = 0usize;
        for t in 1..=1000u64 {
            world.step(&mut empty);
            series.push(world.stations.stock[1][fuel]);
            for e in world.events_mut().since(Tick(t)) {
                // Opening-wave spread: the ticks of the FIRST FOUR accepts.
                if accepts_seen < 4 && matches!(e.kind, EventKind::ContractAccepted { .. }) {
                    accept_ticks.insert(t);
                    accepts_seen += 1;
                }
                if matches!(e.kind, EventKind::ContractFulfilled { .. }) {
                    completions += 1;
                }
            }
            let in_flight = (0..world.contracts.ids.len())
                .filter(|&k| {
                    !matches!(
                        world.contracts.status[k],
                        ContractStatus::Completed | ContractStatus::Failed
                    )
                })
                .count();
            max_in_flight = max_in_flight.max(in_flight);
        }
        let peak = *series.iter().max().unwrap();
        let last200 = &series[800..];
        let band = last200.iter().max().unwrap() - last200.iter().min().unwrap();
        Stage2Run { peak, band, accept_ticks, max_in_flight, completions }
    }

    #[test]
    fn stage2_hysteresis_and_stagger_each_have_a_measured_stabilising_effect() {
        // With return-to-origin routing the closed loop SUSTAINS (haulers deliver, walk
        // back, reload, repeat), so both Stage-2 damping knobs now bite measurably. This
        // is an A/B/C over three configs differing ONLY in the deadband (demand_high) and
        // the stagger (stagger_period), each effect guarded by a discriminating assertion.
        //
        // UNDAMPED   (demand_high == demand_low == 20, stagger 1): deadband collapsed, no
        //            stagger -> the opening burst's 4 accepts clump onto ONE tick, the 4
        //            haulers deliver together -> peak == 4 x qty == 20; shallow in-flight
        //            buffer (4 == one contract per hauler).
        // DEADBAND   (demand_high 30 > demand_low 20, stagger 1): the order-up-to ceiling
        //            keeps a DEEPER in-flight buffer (6 > 4) so foragers are never starved
        //            of an Offered contract — but WITHOUT stagger the deeper buffer arrives
        //            in bigger clumps, so the steady-state band WIDENS (15 > 5).
        // DAMPED     (demand_high 30, stagger 4): same deep buffer, but staggering spreads
        //            the arrivals -> overshoot peak flattens (20 -> 5) AND the band the
        //            deadband alone would widen is tamed back (15 -> 5).
        let undamped = drive_stage2_loop(stage2_ab_loop_fixture(20, 1));
        let deadband = drive_stage2_loop(stage2_ab_loop_fixture(30, 1));
        let damped = drive_stage2_loop(stage2_ab_loop_fixture(30, 4));

        // (0) THE LOOP SUSTAINS in every config (return-routing): far past the 4-delivery
        // single-wave jam ceiling, with a BOUNDED steady-state band (no growing cycle).
        for (label, r) in [("undamped", &undamped), ("deadband", &deadband), ("damped", &damped)] {
            assert!(r.completions > 4, "{label}: forage loop sustains (completions={})", r.completions);
            assert!(r.band <= r.peak, "{label}: steady-state band bounded by the overshoot, not growing");
        }

        // (1) THE DEADBAND has a real, discriminating effect: the order-up-to ceiling
        // keeps a STRICTLY deeper in-flight buffer than the collapsed-deadband baseline.
        // (RED if the demand_high ceiling is neutralised to demand_low.)
        assert!(
            deadband.max_in_flight > undamped.max_in_flight,
            "deadband deepens the in-flight buffer: deadband={} undamped={}",
            deadband.max_in_flight, undamped.max_in_flight
        );
        assert_eq!(
            damped.max_in_flight, deadband.max_in_flight,
            "stagger does not change buffer depth (only WHEN accepts fire)"
        );

        // (2) STAGGER flattens the overshoot peak (clumped 4-hauler spike -> spread +
        // drained). Non-vacuity floor on the undamped peak, then a >=2x cut.
        assert!(undamped.peak >= 20, "undamped overshoot is real: {}", undamped.peak);
        assert!(
            damped.peak * 2 <= undamped.peak,
            "stagger at least halves the overshoot peak: damped={} undamped={}",
            damped.peak, undamped.peak
        );

        // (3) STAGGER also TAMES the steady-state band the deadband alone would widen:
        // deadband-without-stagger jitters MORE than the collapsed baseline, and adding
        // stagger brings it back down. (RED if stagger is disabled.)
        assert!(
            deadband.band > undamped.band,
            "deadband alone widens the steady-state band: deadband={} undamped={}",
            deadband.band, undamped.band
        );
        assert!(
            damped.band < deadband.band,
            "stagger tames the band the deadband widens: damped={} deadband={}",
            damped.band, deadband.band
        );

        // (4) STAGGER spreads the opening wave across ticks; undamped clumps it onto one.
        assert_eq!(undamped.accept_ticks.len(), 1, "undamped clumps opening accepts: {:?}", undamped.accept_ticks);
        assert!(damped.accept_ticks.len() > 1, "stagger spreads opening accepts: {:?}", damped.accept_ticks);
    }

    #[test]
    fn return_routing_sustains_the_forage_loop_past_one_wave() {
        // A hauler is a forager: it WALKS to the food (navigates to a contract's pickup
        // station), EATS (loads + delivers), then forages again. Without return-routing
        // a hauler delivers A->B once and strands at B — Idle but not co-located with the
        // A pickup, so it can never load its next A->B contract. The economy then runs a
        // single opening wave (4 haulers x qty 5 == 20 Fuel eaten by the B sink) and jams.
        // With return-routing the hauler walks back to A, reloads, and delivers again, so
        // the loop sustains for many waves.
        use crate::economy::Good;
        let fuel = Good::FUEL.index();
        let (mut world, _h) =
            World::reset(stage2_ab_loop_fixture(30, 4)).expect("resolvable cfg");
        let mut empty: Vec<Command> = Vec::new();
        let mut completions = 0usize;
        for t in 1..=1000u64 {
            world.step(&mut empty);
            for e in world.events_mut().since(Tick(t)) {
                if matches!(e.kind, EventKind::ContractFulfilled { .. }) {
                    completions += 1;
                }
            }
        }
        // The single-wave jam ceiling is exactly 4 deliveries (one per hauler) == 20 Fuel
        // consumed by the sink. A sustained forage loop blows well past both.
        assert!(
            completions > 4,
            "forage loop sustains past the opening wave (completions={completions}, one-wave jam == 4)"
        );
        assert!(
            world.econ.consumed[fuel] > 20,
            "B sink keeps eating across multiple waves (consumed Fuel={}, one-wave == 20)",
            world.econ.consumed[fuel]
        );
    }

    // ---- Task 22: the PHASE-2 gate -----------------------------------------
    //
    // Two verification tests over the FULL Stage-2 demand-deflation fixture
    // (`stage2_ab_loop_fixture`, T21): repricing LIVE (its base = `one_body_one_thrusting_craft`
    // -> `PriceCfg::default` -> `reprice_interval: 1`, and `base_micros[Fuel] = 100_000`,
    // `cap[Fuel] = 100`), supply (miner + refiner at A), a Fuel sink at B, FOUR haulers and a
    // seeded route template that the order-up-to repost clones. (1) BOTH conservation
    // identities — the per-resource accounting identity AND the global credit identity
    // (Σtreasury + Σcredits + Σescrow == initial) — hold EVERY tick under live demand-deflation
    // pricing; and (2) replay is bit-identical tick-by-tick. Neither moves a golden: the
    // identities are internal invariants and the determinism test is a cross-INSTANCE equality.
    //
    // The headline Phase-2 claim is that PRICING (PriceUpdate = a price write + event only)
    // disturbs NEITHER resource nor credit quantities. To make that non-vacuous the test
    // guards that the Fuel price actually MOVED over the run — conservation under a pricing
    // system that never fired would be a hollow gate.

    #[test]
    fn phase2_gate_full_demand_deflation_harness_conserves() {
        use crate::economy::{N_GOODS_V1, Good};
        // Conservation is config-independent; use the damped (30, 4) config so the full
        // post -> dispatch -> deliver -> sink-consume wave fires within the window
        // (first delivery ~tick 58), making the `consumed Fuel > 0` leg reachable.
        let (mut world, _h) =
            World::reset(stage2_ab_loop_fixture(30, 4)).expect("resolvable cfg");

        // initial[r] = Σ station stock per resource at tick 0 (mined == consumed == 0).
        let mut initial = [0i64; N_GOODS_V1];
        for (r, slot) in initial.iter_mut().enumerate() {
            *slot = world.stations.stock.iter().map(|s| s[r]).sum();
        }
        // initial_credit = the three off-balance-sheet buckets at reset (treasury + craft
        // credits + escrow), captured before any contract motion.
        let initial_credit = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();

        let fuel = Good::FUEL.index();
        // Snapshot station B's Fuel price at reset to prove repricing actually fires.
        let price_b_fuel_0 = world.stations.price_micros[1][fuel];
        let mut price_moved = false;

        // BOTH identities hold at reset and EVERY tick of the live demand-deflation loop.
        assert_resource_identity(&world, &initial);
        let credit_now = |w: &World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        assert_eq!(credit_now(&world), initial_credit, "credit identity holds at reset");

        let mut empty: Vec<Command> = Vec::new();
        for t in 1..=1000u64 {
            world.step(&mut empty);
            assert_resource_identity(&world, &initial);
            assert_eq!(
                credit_now(&world),
                initial_credit,
                "Σtreasury+Σcredits+Σescrow invariant at tick {t}"
            );
            if world.stations.price_micros[1][fuel] != price_b_fuel_0 {
                price_moved = true;
            }
        }

        // Non-vacuity guards: a stalled world satisfies both identities trivially, so assert
        // every leg actually fired — miner (mined Ore), refiner output (mined Fuel) and input
        // (consumed Ore), and the deliver -> sink-consume path (consumed Fuel > 0 proves Fuel
        // reached station B via a delivered contract and was consumed by the sink there).
        let ore = Good::ORE.index();
        assert!(world.econ.mined[ore] > 0, "miner is live (mined Ore > 0)");
        assert!(world.econ.mined[fuel] > 0, "refiner output leg live (mined Fuel > 0)");
        assert!(world.econ.consumed[ore] > 0, "refiner input leg live (consumed Ore > 0)");
        assert!(
            world.econ.consumed[fuel] > 0,
            "deliver + sink-consume legs fired (consumed Fuel > 0)"
        );

        // HEADLINE non-vacuity: demand-deflation pricing actually FIRED — station B's Fuel
        // price moved as its stock changed. Without this, the identities would be conserved
        // under a pricing system that never wrote anything (a hollow Phase-2 gate).
        assert!(
            price_moved,
            "demand-deflation pricing fired (station B Fuel price moved from {price_b_fuel_0})"
        );
    }

    #[test]
    fn phase2_gate_replay_is_deterministic_state_hash_tick_by_tick() {
        // Two worlds from the SAME Stage-2 demand-deflation config and the SAME (empty)
        // command inputs. step() takes the command vec mutably, so give each world its OWN
        // vec to rule out cross-contamination.
        let (mut world_a, _ha) =
            World::reset(stage2_ab_loop_fixture(30, 4)).expect("resolvable cfg");
        let (mut world_b, _hb) =
            World::reset(stage2_ab_loop_fixture(30, 4)).expect("resolvable cfg");

        let h0 = crate::hash::state_hash(&world_a);
        assert_eq!(h0, crate::hash::state_hash(&world_b), "tick 0 hashes agree");

        let mut empty_a: Vec<Command> = Vec::new();
        let mut empty_b: Vec<Command> = Vec::new();
        let mut last = h0;
        for t in 1..=1000u64 {
            world_a.step(&mut empty_a);
            world_b.step(&mut empty_b);
            let ha = crate::hash::state_hash(&world_a);
            let hb = crate::hash::state_hash(&world_b);
            assert_eq!(ha, hb, "replay determinism: state_hash diverged at tick {t}");
            last = ha;
        }

        // Non-vacuity guard: the hash sequence must actually EVOLVE over the run — two
        // constant sequences would compare equal trivially.
        assert_ne!(last, h0, "state_hash evolved over the 1000-tick run (not a constant)");
    }

    #[test]
    fn bad_goods_cfg_zero_goods_is_rejected() {
        let mut cfg = one_body_one_craft();
        cfg.goods.goods.clear();
        assert!(matches!(
            World::reset(cfg),
            Err(ResetError::BadGoodsCfg { .. })
        ));
    }

    #[test]
    fn bad_goods_cfg_stock_length_mismatch_is_rejected() {
        use crate::config::StationInit;
        let mut cfg = one_body_one_craft();
        // Stock vec length 1 but GoodsCfg has 2 goods -> mismatch
        cfg.stations = vec![StationInit {
            body_index: 0,
            initial_stock: vec![0i64],
            initial_price_micros: vec![0i64, 0i64],
            sells_upgrades: false,
        }];
        assert!(matches!(
            World::reset(cfg),
            Err(ResetError::BadGoodsCfg { .. })
        ));
    }
}
