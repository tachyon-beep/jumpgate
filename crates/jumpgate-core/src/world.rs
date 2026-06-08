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
use crate::stores::{BodyStore, NavState, ShipStore, effective_params};
use crate::time::{Dt, Tick};
use crate::types::{EntityRef, Lod, NavDest};

/// The authoritative simulation aggregate. Single writer; all facades read via `StateView`.
pub struct World {
    // `ships`/`bodies` are pub(crate) so the per-tick state hash (hash.rs, a later
    // task) can fold their canonical SoA arrays in directly. Everything else stays
    // private behind StateView / narrow mutators.
    pub(crate) ships: ShipStore,
    pub(crate) bodies: BodyStore,
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

impl World {
    /// Build a World from a RunConfig: precompute ephemeris, seed rng from the
    /// master seed, spawn bodies then craft, and return the config hash.
    /// `seed` and `dt` come from `cfg`; nothing is read from the environment.
    pub fn reset(cfg: RunConfig) -> (World, ConfigHash) {
        let hash = cfg.config_hash();
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

        let mut ships = ShipStore {
            ids: SlotMap::new(),
            pos: Vec::new(),
            vel: Vec::new(),
            fuel_mass: Vec::new(),
            spec: Vec::new(),
            nav: Vec::new(),
            lod: Vec::new(),
            prev_fuel: Vec::new(),
            prev_inside_dest: Vec::new(),
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
        }

        let rng = RngStreams::from_master(cfg.master_seed);
        let dt = cfg.dt;
        let world = World {
            ships,
            bodies,
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
        (world, hash)
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
    fn body_index(&self, id: BodyId) -> Option<usize> {
        self.bodies.ids.dense_index(id.slot, id.generation)
    }
    fn craft_id_at(&self, dense_index: usize) -> CraftId {
        // SlotMap::id_at returns Option; delegate to the ShipStore wrapper
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

            let eff = effective_params(&self.ships.spec[ci]);
            let pos = self.ships.pos[ci];
            let vel = self.ships.vel[ci];
            let fuel = self.ships.fuel_mass[ci];

            let dest_pos = match self.ships.nav[ci] {
                NavState::Seeking { dest, .. } => self.resolve_dest_pos(dest, cur),
                NavState::Idle => pos, // unused (throttle will be 0)
            };
            let (thrust_dir, throttle) =
                autopilot_command(self.ships.nav[ci], pos, vel, dest_pos, &eff);

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

        // (4) copy-forward the boundary-edge arrays so next tick's detection has a
        //     prior. These arrays are folded into state_hash at the position fixed by
        //     HASH_FIELD_ORDER (a later hash task pins their contribution).
        for ci in 0..n_craft {
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

    /// Observer-parameterized projection. The presence mask is sourced from the
    /// single `visible(observer, entity)` predicate (all-true for `FullObserver`).
    /// This is the ONE location a future fog-of-war / per-faction filter edits.
    pub fn project<O: Observer>(&self, observer: &O) -> View {
        let mut craft = Vec::new();
        for id in self.craft_ids() {
            if observer.visible(EntityRef::Craft(id)) {
                let i = self.ship_index(id).expect("live id");
                // effective fuel_capacity rides the accessor seam (effective==base in v1).
                let cap = effective_params(&self.ships.spec[i]).fuel_capacity;
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
            .map(|i| effective_params(&self.ships.spec[i]).fuel_capacity)
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
    use crate::config::{BaseSpec, BodyInit, CraftInit, OrbitalElements, SubstepCfg};
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
                    // Recalibrated from the plan's 1e-9: with dry+fuel == 2e-12,
                    // thrust accel == max_thrust / 2e-12, so 1e-9 yields ~500 AU/day²
                    // (Δv≈500/tick) which overshoots the 0.05 burn_budget on the very
                    // first tick and flings the craft away. 1e-13 gives accel≈0.05,
                    // exactly one budgeted burn toward the dest, then a clean coast in.
                    base_max_thrust: 1e-13,
                    base_exhaust_velocity: 1e-3,
                    base_fuel_capacity: 1e-12,
                },
                // 1 AU out, on a roughly circular prograde orbit (v ~ sqrt(GM/r)).
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 0.0172, 0.0),
                fuel_mass: 1e-12,
            }],
        }
    }

    #[test]
    fn reset_starts_at_tick_zero_and_hashes_config() {
        let cfg = one_body_one_craft();
        let expected = cfg.config_hash();
        let (world, returned) = World::reset(cfg);
        assert_eq!(returned, expected, "reset must return RunConfig::config_hash()");
        assert_eq!(world.tick(), Tick(0));
        assert_eq!(world.dt().get(), 1.0);
        assert_eq!(world.craft_ids().len(), 1);
        assert_eq!(world.body_ids().len(), 1);
    }

    #[test]
    fn step_advances_tick_and_coasts_under_gravity() {
        let cfg = one_body_one_craft();
        let (mut world, _) = World::reset(cfg);

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
    fn commanded_craft_moves_toward_dest_and_history_is_visible() {
        use crate::types::{EntityRef, NavDest, Target};
        let cfg = one_body_one_craft();
        let (mut world, _) = World::reset(cfg);
        let id = world.craft_ids()[0];

        let dest = NavDest::Position(Vec3::new(3.0, 0.0, 0.0));
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id)),
            kind: CommandKind::Destination { dest, burn_budget: Some(0.05) },
        }];

        let r0 = world.craft_pos(id).unwrap().length();
        let d0 = world.craft_pos(id).unwrap().sub(Vec3::new(3.0, 0.0, 0.0)).length();

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

        // Keep stepping; the craft should net-approach the destination.
        for _ in 0..20 {
            let mut none: Vec<Command> = Vec::new();
            world.step(&mut none);
        }
        let d1 = world.craft_pos(id).unwrap().sub(Vec3::new(3.0, 0.0, 0.0)).length();
        let r1 = world.craft_pos(id).unwrap().length();
        assert!(d1 < d0, "craft moved toward dest: {d0} -> {d1}");
        assert!(r1 > r0, "thrusting outward increased orbital radius: {r0} -> {r1}");
    }

    #[test]
    fn dormant_craft_skips_physics() {
        let cfg = one_body_one_craft();
        let (mut world, _) = World::reset(cfg);
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
        let (world, _) = World::reset(cfg);
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
}
