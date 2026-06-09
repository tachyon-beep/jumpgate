//! World aggregate: owns all stores + ephemeris + rng + logs, drives the tick.
use crate::autopilot::autopilot_command;
use crate::config::{ConfigHash, RunConfig};
use crate::contract::{Command, Event, EventKind, Integrator, StateView};
use crate::ephemeris::Ephemeris;
use crate::events::EventStream;
use crate::ids::{BodyId, CraftId, SlotMap};
use crate::ingest::ActionLog;
use crate::integrator::{VelocityVerlet, gravity_accel, substep_count};
use crate::math::Vec3;
use crate::rng::RngStreams;
use crate::ship::thrust_accel_and_burn;
use crate::stores::{BodyStore, CraftStore, NavState, effective_params};
use crate::time::{Dt, Tick};
use crate::types::{EntityRef, Lod, NavDest};

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
    eph: Ephemeris,
    #[allow(dead_code)]
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
            ships.role.push(crate::stores::CraftRole::Idle);
            ships.cargo.push(None);
            ships.credits_micros.push(0);
            ships.contract.push(None);
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
            station_ids.push(stations.push(body, s.initial_stock, s.initial_price_micros));
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

        let rng = RngStreams::from_master(cfg.master_seed);
        let dt = cfg.dt;
        let world = World {
            ships,
            bodies,
            stations,
            producers,
            corporations,
            contracts,
            econ: crate::economy::EconCounters::zero(),
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

        // (1c) economy contract stage: drive the accept/escrow/load/dispatch
        //      lifecycle for haulers bound by this tick's ingest. Disjoint &mut
        //      field borrows + read-only bodies/eph (mirrors run_producers); `next`
        //      is the resolution tick (co-location is checked against body_pos(next)).
        crate::economy::resolve_contracts(
            &mut self.contracts,
            &mut self.corporations,
            &mut self.stations,
            &mut self.ships,
            &self.bodies,
            &self.eph,
            &self.config.guidance,
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
                },
                // 1 AU out, on a roughly circular prograde orbit (v ~ sqrt(GM/r)).
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 0.0172, 0.0),
                fuel_mass: 1e-12,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
        }
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
                },
                pos: Vec3::new(5.0, 0.0, 0.0),
                vel: Vec3::ZERO, // start at REST: no orbital-velocity Δv tax
                fuel_mass: 1e-9,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
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
        use crate::economy::{N_RESOURCES, Recipe, Resource};
        let mut cfg = one_body_one_craft();
        cfg.stations = vec![
            StationInit {
                body_index: 0,
                initial_stock: [7, 3],
                initial_price_micros: [100, 200],
            },
            StationInit {
                body_index: 0,
                initial_stock: [0; N_RESOURCES],
                initial_price_micros: [150, 250],
            },
        ];
        cfg.producers = vec![ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 1 },
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
        assert_eq!(wa.stations.stock[0], [7, 3]);
        assert_eq!(wa.stations.price_micros[0], [100, 200]);
        // The minted producer points at the minted StationId for station_index 0.
        let st0 = wa.stations.ids.id_at(0).map(|(slot, generation)| crate::ids::StationId {
            slot,
            generation,
        });
        assert_eq!(Some(wa.producers.station[0]), st0, "producer bound to minted station 0");
        // Flow counters start zero (no firing at reset).
        assert_eq!(wa.econ, crate::economy::EconCounters::zero());
    }

    /// One station (Ore=0) with a ∅->Ore(5) miner at interval 1, attached to it.
    /// Used to prove `run_producers` is wired into `World::step`.
    fn one_body_one_station_one_miner_ore_zero() -> RunConfig {
        use crate::config::{ProducerInit, StationInit};
        use crate::economy::{N_RESOURCES, Recipe, Resource};
        let mut cfg = one_body_one_craft();
        cfg.stations = vec![StationInit {
            body_index: 0,
            initial_stock: [0; N_RESOURCES],
            initial_price_micros: [0; N_RESOURCES],
        }];
        cfg.producers = vec![ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 1 },
        }];
        cfg
    }

    #[test]
    fn step_runs_producers_each_tick() {
        use crate::economy::Resource;
        // Miner ∅->Ore(5), interval 1, station Ore starts at 0. Stepping 3 ticks
        // fires the producer at next = 1,2,3 -> stock and mined[Ore] both reach 15.
        let (mut world, _) =
            World::reset(one_body_one_station_one_miner_ore_zero()).expect("resolvable config");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..3 {
            world.step(&mut empty);
        }
        assert_eq!(
            world.stations.stock[0][Resource::Ore.index()],
            15,
            "3 firings of ∅->Ore(5) raise station Ore stock to 15"
        );
        assert_eq!(
            world.econ.mined[Resource::Ore.index()],
            15,
            "mined[Ore] counter tracks the 3 firings"
        );
    }

    #[test]
    fn reset_rejects_out_of_range_economy_ref() {
        use crate::config::StationInit;
        use crate::economy::N_RESOURCES;
        let mut cfg = one_body_one_craft();
        // body_index 5 is out of range (only 1 body) -> BadEconomyRef before tick 0.
        cfg.stations = vec![StationInit {
            body_index: 5,
            initial_stock: [0; N_RESOURCES],
            initial_price_micros: [0; N_RESOURCES],
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
        use crate::economy::Resource;
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
                initial_stock: [0, 10],
                initial_price_micros: [0, 0],
            },
            // Station B (the delivery destination).
            StationInit {
                body_index: 1,
                initial_stock: [0, 0],
                initial_price_micros: [0, 0],
            },
        ];
        cfg.corporations = vec![CorporationInit {
            treasury_micros: 5_000_000,
            home_station_index: 0,
        }];
        cfg.contracts = vec![ContractInit {
            corp_index: 0,
            resource: Resource::Fuel,
            qty: 5,
            from_station_index: 0,
            to_station_index: 1,
            reward_micros: 1_000_000,
        }];
        cfg
    }

    #[test]
    fn accept_contract_escrows_loads_cargo_and_dispatches_hauler() {
        use crate::economy::{ContractStatus, Resource};
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0usize; // sole contract, dense row 0
        let fuel = Resource::Fuel.index();

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
        assert_eq!(world.ships.cargo[crow], Some((Resource::Fuel, 5)));
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

    #[test]
    fn deliver_on_arrival_settles_escrow_and_holds_credit_identity() {
        use crate::economy::{ContractStatus, Resource};
        use crate::stores::CraftRole;
        use crate::types::{EntityRef, Target};
        let (mut world, _h) = World::reset(two_body_contract_fixture()).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0usize; // sole contract, dense row 0
        let fuel = Resource::Fuel.index();

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
        // body 1 sits at a == 0.001 AU and the craft has ample Δv, so this converges
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
}
