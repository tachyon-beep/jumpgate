//! THE single ingestion path (lever invariant, §4.4). Sorts commands by the
//! canonical `command_sort_key`, resolves each `NavDest` into a resolved
//! `NavState::Seeking`, logs every command tick-stamped, and emits an
//! `ActionIngested` event. No out-of-band store mutation happens anywhere else.

use crate::config::ConfigHash;
use crate::contract::{Command, Event, EventKind, command_sort_key};
use crate::economy::ContractStatus;
use crate::ephemeris::Ephemeris;
use crate::events::EventStream;
use crate::math::Vec3;
use crate::stores::{CraftRole, CraftStore, NavState};
use crate::time::Tick;
use crate::types::{CommandKind, EntityRef, NavDest, Target};

/// Tick-stamped append-only command log. Replay re-feeds these entries; the
/// policy is never re-run (§6). `config_hash` is the provenance stamp of the
/// `RunConfig` this log was recorded under — Task 14 compares it against a
/// freshly-computed hash to detect a config/replay mismatch.
pub struct ActionLog {
    pub entries: Vec<(Tick, Command)>,
    /// Parallel to `entries`, pushed in lockstep by `record` (the SINGLE writer),
    /// so `at`/`since_commands` return zero-copy `&[Command]`. The lockstep push
    /// is what makes the parallel vec safe — it can never desync from `entries`.
    pub commands_flat: Vec<Command>,
    pub config_hash: ConfigHash,
}

impl ActionLog {
    /// Construct a log stamped with the recording run's config hash.
    pub fn new(config_hash: ConfigHash) -> Self {
        ActionLog {
            entries: Vec::new(),
            commands_flat: Vec::new(),
            config_hash,
        }
    }

    /// The ONLY writer: pushes BOTH vecs in lockstep so they cannot diverge.
    pub fn record(&mut self, tick: Tick, cmd: Command) {
        self.entries.push((tick, cmd));
        self.commands_flat.push(cmd);
    }

    /// All commands logged exactly at `tick`, in insertion (canonical) order,
    /// as a zero-copy contiguous slice (entries are append-only, tick-monotone).
    pub fn at(&self, tick: Tick) -> &[Command] {
        let start = self.entries.partition_point(|(t, _)| *t < tick);
        let end = self.entries.partition_point(|(t, _)| *t <= tick);
        &self.commands_flat[start..end]
    }

    /// Every command logged at `tick >= since`, in insertion (canonical) order,
    /// as a zero-copy contiguous tail slice. Task 12's `StateView::recent_commands`
    /// (which the locked contract fixes as returning `&[Command]`) is built on this.
    pub fn since_commands(&self, since: Tick) -> &[Command] {
        let start = self.entries.partition_point(|(t, _)| *t < since);
        &self.commands_flat[start..]
    }
}

/// Construct an `ActionIngested` event. Lives HERE (not `events.rs`) so the
/// single ingestion path owns its only event constructor and the module graph
/// stays acyclic (Task 10 must not depend on a Task-11 symbol).
fn action_ingested(tick: Tick, target: Target) -> Event {
    Event {
        tick,
        kind: EventKind::ActionIngested { target },
    }
}

/// Resolve a `NavDest` to a concrete world `Vec3` at `tick`.
/// `Position` is already absolute; `Entity` is looked up via the ephemeris
/// (bodies) or the ship store (craft). Returns `None` if the referent is gone.
fn resolve_dest(dest: NavDest, tick: Tick, ship: &CraftStore, eph: &Ephemeris) -> Option<Vec3> {
    match dest {
        NavDest::Position(p) => Some(p),
        NavDest::Entity(EntityRef::Body(bid)) => {
            // v1 ephemeris is indexed positionally; the BodyId slot is the row.
            Some(eph.body_pos(bid.slot as usize, tick))
        }
        NavDest::Entity(EntityRef::Craft(cid)) => ship.craft_pos_by_id(cid),
    }
}

/// THE single ingestion path. Sorts `cmds` into canonical order in place,
/// then for each command: resolves the destination, sets the target craft's
/// `NavState::Seeking`, logs the command tick-stamped, and emits
/// `ActionIngested`. Lever invariant: this is the only craft-nav write path.
pub fn ingest_into(
    ship: &mut CraftStore,
    eph: &Ephemeris,
    log: &mut ActionLog,
    events: &mut EventStream,
    tick: Tick,
    cmds: &mut [Command],
) {
    // Canonical, total, deterministic ordering across all Target scopes.
    cmds.sort_by_key(command_sort_key);

    for cmd in cmds.iter() {
        // Log every command in canonical order (resolved values; §4.4 rule 2).
        log.record(tick, *cmd);

        match (cmd.target, cmd.kind) {
            (
                Target::Entity(EntityRef::Craft(cid)),
                CommandKind::Destination { dest, burn_budget },
            ) => {
                if let Some(idx) = ship.index_of(cid) {
                    // dv budget: explicit cap, else Tsiolkovsky fuel-derived.
                    let dv = burn_budget.unwrap_or_else(|| dv_from_fuel(ship, idx));
                    // Validate the dest resolves now; drop silently if it does
                    // not. The autopilot recomputes the live dest each tick, so
                    // we store the dest reference (moving targets are tracked).
                    if resolve_dest(dest, tick, ship, eph).is_some() {
                        ship.nav[idx] = NavState::Seeking {
                            dest,
                            dv_remaining: dv,
                        };
                        events.emit(action_ingested(tick, cmd.target));
                    }
                }
            }
            // Direct thrust (tactical Rung 1): resolves immediately — no dest lookup.
            (Target::Entity(EntityRef::Craft(cid)), CommandKind::Thrust { throttle_vec }) => {
                if let Some(idx) = ship.index_of(cid) {
                    ship.nav[idx] = NavState::DirectThrust { throttle_vec };
                    events.emit(action_ingested(tick, cmd.target));
                }
            }
            // World / Sim / Body targets, plus the economy CommandKinds (which this
            // CraftStore-only path cannot resolve — it has no ContractStore): no nav
            // effect, but the command is logged above so replay identity is preserved
            // and the ingestion event still fires for the legibility stream.
            _ => {
                events.emit(action_ingested(tick, cmd.target));
            }
        }
    }
}

/// THE single ingestion path against the real `World`. Sorts by `command_sort_key`
/// (total over World/Sim/entity scopes), resolves each `NavDest` into a concrete
/// `NavState::Seeking`, logs (resolved values, never re-rolled intentions — the
/// lever invariant), and emits `ActionIngested`. v1's only `CommandKind` is
/// `Destination`. Writes through three narrow World mutators (`log_mut`,
/// `set_nav`, `events_mut`) rather than touching another module's private fields.
pub fn ingest_commands(world: &mut crate::world::World, tick: Tick, cmds: &mut Vec<Command>) {
    cmds.sort_by_key(command_sort_key);
    for &cmd in cmds.iter() {
        world.log_mut().record(tick, cmd);
        // Only craft targets carry a v1 command effect; World/Sim/Body targets are
        // logged + ActionIngested-emitted (the seam) but otherwise inert.
        if let Target::Entity(EntityRef::Craft(id)) = cmd.target {
            match cmd.kind {
                CommandKind::Destination { dest, burn_budget } => {
                    // dv budget: explicit cap, else Tsiolkovsky fuel-derived (D9/M5) — path-
                    // independent with the slice path, and never INFINITY into dv_remaining.
                    let dv = burn_budget.unwrap_or_else(|| world.dv_from_fuel_for(id));
                    world.set_nav(
                        id,
                        NavState::Seeking {
                            dest,
                            dv_remaining: dv,
                        },
                    );
                }
                CommandKind::AcceptContract { contract } => {
                    // Record INTENT only: set the craft's contract column + role Hauler
                    // iff the contract is Offered and unassigned. The actual contract
                    // state transition (status/escrow) is DEFERRED to the resolve_contracts
                    // stage. A stale craft/contract or a non-Offered/already-taken contract
                    // is a deterministic skip (no column write); the command is still logged
                    // above and ActionIngested still fires below (the seam).
                    let craft_row = world.ships.index_of(id);
                    let contract_row = world
                        .contracts
                        .ids
                        .dense_index(contract.slot, contract.generation);
                    if let (Some(i), Some(ci)) = (craft_row, contract_row)
                        && world.contracts.status[ci] == ContractStatus::Offered
                        && world.contracts.hauler[ci].is_none()
                    {
                        world.ships.contract[i] = Some(contract);
                        world.ships.role[i] = CraftRole::Hauler;
                    }
                }
                CommandKind::SetRole { role } => {
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.role[i] = role;
                    }
                }
                CommandKind::Thrust { throttle_vec } => {
                    world.set_nav(id, NavState::DirectThrust { throttle_vec });
                }
                CommandKind::BuyUpgrade { kind } => {
                    // Record INTENT only (the AcceptContract template): write the
                    // transient `pending_upgrade` column. The settle — vendor dock
                    // check, price debit, Yard credit, count bump — is DEFERRED to
                    // `resolve_purchases` (stage 1d), which consumes the intent the
                    // SAME tick, so `pending_upgrade` is always None at hash points.
                    // A stale craft id is a deterministic skip; the command is still
                    // logged above and ActionIngested still fires below (the seam).
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_upgrade[i] = Some(kind);
                    }
                }
                CommandKind::Refuel => {
                    // Record INTENT only (the BuyUpgrade template): write the
                    // transient `pending_refuel` column. The settle is deferred
                    // to `resolve_refuels` (stage 1d2), which consumes the
                    // intent the same tick, including as a lot-0 no-op.
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_refuel[i] = Some(());
                    }
                }
                CommandKind::TradeBuy { good, qty, station } => {
                    // Record INTENT only (the Refuel template): write the transient
                    // `pending_trade_buy` column. The settle lives in
                    // `resolve_trade_buys` (stage 1dx), which consumes the intent
                    // the same tick (including the exchange-inactive no-op).
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_trade_buy[i] = Some((good, qty, station));
                    }
                }
                CommandKind::TradeSell { station } => {
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_trade_sell[i] = Some(station);
                    }
                }
            }
        }
        world.events_mut().emit(Event {
            tick,
            kind: EventKind::ActionIngested { target: cmd.target },
        });
    }
    cmds.clear();
}

/// Fuel-derived Δv fallback when no explicit budget is given: Tsiolkovsky Δv via
/// the shared `math::tsiolkovsky_dv` helper, using effective params (§5.5). Bit-for-bit
/// identical to the prior inline form (same operands, same left-to-right grouping).
fn dv_from_fuel(ship: &CraftStore, idx: usize) -> f64 {
    let eff = crate::stores::effective_params(&ship.spec[idx], &ship.mods[idx]);
    crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ship.fuel_mass[idx])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BaseSpec, BodyInit, CraftInit, GuidanceParams, OrbitalElements, RunConfig, SubstepCfg,
    };
    use crate::economy::Good;
    use crate::ids::{CorporationId, CraftId, StationId};
    use crate::stores::CraftRole;
    use crate::time::Dt;
    use crate::world::World;

    fn cfg_hash() -> ConfigHash {
        // A stand-in provenance stamp; only its round-trip identity matters here.
        ConfigHash(0xABCD_0001)
    }

    fn empty_ephemeris() -> Ephemeris {
        // Zero bodies: NavDest::Position resolution needs no body lookup.
        Ephemeris::precompute(&[] as &[BodyInit], Dt::new(1.0), 1)
    }

    fn ship_store_with(n: usize) -> CraftStore {
        let mut store = CraftStore::empty();
        for _ in 0..n {
            store.push(
                BaseSpec {
                    base_dry_mass: 1.0,
                    base_max_thrust: 1.0,
                    base_exhaust_velocity: 1.0,
                    base_fuel_capacity: 1.0,
                    base_cargo_capacity: 5,
                },
                Vec3::ZERO,
                Vec3::ZERO,
                0.5, // fuel_mass
            );
        }
        store
    }

    fn dest_for(id: CraftId, x: f64) -> Command {
        Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Destination {
                dest: NavDest::Position(Vec3::new(x, 0.0, 0.0)),
                burn_budget: Some(2.0),
            },
        }
    }

    #[test]
    fn log_records_queries_by_tick_and_since() {
        let mut log = ActionLog::new(cfg_hash());
        log.record(
            Tick(5),
            dest_for(
                CraftId {
                    slot: 0,
                    generation: 1,
                },
                1.0,
            ),
        );
        log.record(
            Tick(5),
            dest_for(
                CraftId {
                    slot: 1,
                    generation: 1,
                },
                2.0,
            ),
        );
        log.record(
            Tick(6),
            dest_for(
                CraftId {
                    slot: 0,
                    generation: 1,
                },
                3.0,
            ),
        );
        assert_eq!(log.at(Tick(5)).len(), 2);
        assert_eq!(log.at(Tick(6)).len(), 1);
        assert_eq!(log.at(Tick(7)).len(), 0);
        assert_eq!(log.entries.len(), 3);
        // since_commands returns a zero-copy &[Command] tail slice; commands_flat
        // is pushed in lockstep with entries by the single writer, so no desync.
        assert_eq!(log.since_commands(Tick(0)).len(), 3);
        assert_eq!(log.since_commands(Tick(6)).len(), 1);
        assert_eq!(log.since_commands(Tick(7)).len(), 0);
        assert_eq!(log.commands_flat.len(), log.entries.len());
        // config_hash provenance is preserved verbatim for Task 14's guard.
        assert_eq!(log.config_hash, cfg_hash());
    }

    #[test]
    fn out_of_order_yields_same_navstate_as_presorted() {
        let eph = empty_ephemeris();

        // Build two identical stores; feed one shuffled, one pre-sorted.
        let mut store_a = ship_store_with(2);
        let mut store_b = ship_store_with(2);
        let id0 = store_a.ids_at(0);
        let id1 = store_a.ids_at(1);

        let mut shuffled = vec![dest_for(id1, 9.0), dest_for(id0, 4.0)];
        let mut presorted = shuffled.clone();
        presorted.sort_by_key(command_sort_key);

        let mut log_a = ActionLog::new(cfg_hash());
        let mut log_b = ActionLog::new(cfg_hash());
        let mut ev_a = EventStream::new();
        let mut ev_b = EventStream::new();

        ingest_into(
            &mut store_a,
            &eph,
            &mut log_a,
            &mut ev_a,
            Tick(0),
            &mut shuffled,
        );
        ingest_into(
            &mut store_b,
            &eph,
            &mut log_b,
            &mut ev_b,
            Tick(0),
            &mut presorted,
        );

        // Resolved NavState must be identical regardless of input order.
        for i in 0..2 {
            match (store_a.nav[i], store_b.nav[i]) {
                (
                    NavState::Seeking {
                        dest: da,
                        dv_remaining: va,
                    },
                    NavState::Seeking {
                        dest: db,
                        dv_remaining: vb,
                    },
                ) => {
                    assert_eq!(da, db, "dest mismatch at craft {i}");
                    assert_eq!(va, vb, "dv mismatch at craft {i}");
                }
                other => panic!("expected both Seeking at {i}, got {other:?}"),
            }
        }

        // The log is sorted into canonical order on both paths -> identical.
        assert_eq!(log_a.entries, log_b.entries);

        // dv budget honoured: burn_budget Some(2.0) -> dv_remaining 2.0.
        if let NavState::Seeking { dv_remaining, .. } = store_a.nav[0] {
            assert_eq!(dv_remaining, 2.0);
        } else {
            panic!("craft 0 not Seeking");
        }
    }

    #[test]
    fn ingest_emits_action_ingested_event() {
        let eph = empty_ephemeris();
        let mut store = ship_store_with(1);
        let id0 = store.ids_at(0);
        let mut log = ActionLog::new(cfg_hash());
        let mut ev = EventStream::new();
        let mut cmds = vec![dest_for(id0, 4.0)];
        ingest_into(&mut store, &eph, &mut log, &mut ev, Tick(3), &mut cmds);

        let emitted = ev.since(Tick(0));
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].tick, Tick(3));
        match emitted[0].kind {
            EventKind::ActionIngested { target } => {
                assert_eq!(target, Target::Entity(EntityRef::Craft(id0)));
            }
            other => panic!("expected ActionIngested, got {other:?}"),
        }
    }

    /// Minimal inert-physics config with one body + one craft (no economy seeded
    /// from config — contracts are pushed onto the live store directly by the test).
    fn one_body_one_craft_cfg() -> RunConfig {
        RunConfig {
            master_seed: 7,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg {
                accel_ref: 1e-3,
                max_substeps: 64,
            },
            ephemeris_window: 256,
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
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2,
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::new(5.0, 0.0, 0.0),
                vel: Vec3::ZERO,
                fuel_mass: 1e-9,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
                trade_reserve_micros: 0,
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
            exchange: crate::config::ExchangeCfg::default(),
            arbitrage: crate::config::ArbitrageCfg::default(),
        }
    }

    #[test]
    fn accept_contract_sets_columns_logs_and_emits_action_ingested() {
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        // Seed one Offered, unassigned contract directly on the live store (test
        // precondition; config minting is a later task).
        let corp = CorporationId {
            slot: 0,
            generation: 0,
        };
        let from = StationId {
            slot: 0,
            generation: 0,
        };
        let to = StationId {
            slot: 1,
            generation: 0,
        };
        let cid = world.contracts.push(corp, Good::ORE, 5, from, to, 1_000);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        // Intent recorded on the craft columns (the contract state transition is
        // deferred to resolve_contracts; here we only set contract + role).
        assert_eq!(world.ships.contract[0], Some(cid), "contract column set");
        assert_eq!(world.ships.role[0], CraftRole::Hauler, "role set to Hauler");

        // The command was logged at its tick on the single ingestion path.
        assert_eq!(
            world.log_mut().at(Tick(2)).len(),
            1,
            "command logged at tick"
        );

        // An ActionIngested event was emitted for the craft target (the seam).
        let emitted = world.events_mut().since(Tick(0));
        assert_eq!(emitted.len(), 1);
        match emitted[0].kind {
            EventKind::ActionIngested { target } => {
                assert_eq!(target, Target::Entity(EntityRef::Craft(id0)));
            }
            other => panic!("expected ActionIngested, got {other:?}"),
        }
    }

    #[test]
    fn buy_upgrade_writes_pending_intent_logs_and_emits_action_ingested() {
        // The BuyUpgrade ingest arm is INTENT-ONLY (the AcceptContract template):
        // it writes the transient `pending_upgrade` column and nothing else — the
        // settle (dock/price/cap checks, debit, Yard credit, level bump) is
        // deferred to `resolve_purchases` (stage 1d), which consumes the intent
        // the same tick.
        use crate::stores::{UpgradeKind, UpgradeLevels};
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::BuyUpgrade {
                kind: UpgradeKind::Escort,
            },
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        // Intent written; NO settle on the ingest path (single-ingestion-path lever).
        assert_eq!(
            world.ships.pending_upgrade[0],
            Some(UpgradeKind::Escort),
            "intent column set"
        );
        assert_eq!(
            world.ships.upgrades[0],
            UpgradeLevels::default(),
            "no level change at ingest"
        );
        assert_eq!(
            world.ships.credits_micros[0], 0,
            "no credit movement at ingest"
        );

        // Logged + ActionIngested (the seam fires for every command).
        assert_eq!(
            world.log_mut().at(Tick(2)).len(),
            1,
            "command logged at tick"
        );
        let emitted = world.events_mut().since(Tick(0));
        assert_eq!(emitted.len(), 1);
        assert!(matches!(
            emitted[0].kind,
            EventKind::ActionIngested { target } if target == Target::Entity(EntityRef::Craft(id0))
        ));
    }

    #[test]
    fn refuel_writes_pending_intent_logs_and_emits_action_ingested() {
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::Refuel,
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        assert_eq!(world.ships.pending_refuel[0], Some(()), "intent column set");
        assert_eq!(
            world.ships.fuel_mass[0], world.ships.prev_fuel[0],
            "no tank movement at ingest"
        );
        assert_eq!(
            world.ships.credits_micros[0], 0,
            "no credit movement at ingest"
        );
        assert_eq!(
            world.log_mut().at(Tick(2)).len(),
            1,
            "command logged at tick"
        );
        let emitted = world.events_mut().since(Tick(0));
        assert_eq!(emitted.len(), 1);
        assert!(matches!(
            emitted[0].kind,
            EventKind::ActionIngested { target } if target == Target::Entity(EntityRef::Craft(id0))
        ));
    }

    #[test]
    fn accept_contract_deterministic_skip_leaves_columns_but_still_logs() {
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        // Seed a contract that is already assigned to ANOTHER craft -> not acceptable.
        let corp = CorporationId {
            slot: 0,
            generation: 0,
        };
        let from = StationId {
            slot: 0,
            generation: 0,
        };
        let to = StationId {
            slot: 1,
            generation: 0,
        };
        let cid = world.contracts.push(corp, Good::ORE, 5, from, to, 1_000);
        let other = CraftId {
            slot: 99,
            generation: 0,
        };
        let cidx = world
            .contracts
            .ids
            .dense_index(cid.slot, cid.generation)
            .unwrap();
        world.contracts.hauler[cidx] = Some(other);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        // Deterministic skip: craft columns untouched, but command still logged +
        // ActionIngested still emitted (the seam fires for every command).
        assert_eq!(
            world.ships.contract[0], None,
            "skipped: contract column stays None"
        );
        assert_eq!(
            world.ships.role[0],
            CraftRole::Idle,
            "skipped: role stays Idle"
        );
        assert_eq!(world.log_mut().at(Tick(2)).len(), 1, "command still logged");
        assert_eq!(
            world.events_mut().since(Tick(0)).len(),
            1,
            "ActionIngested still emitted"
        );
    }
}
