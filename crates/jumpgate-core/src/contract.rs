//! Shared contract types — the cross-cutting DTOs and read/integrate traits
//! that the ingestion path, event stream, replay format, and all facades
//! agree on.
//!
//! This module is the "drift lock": downstream tasks implement against these
//! exact names/signatures. Bodies are stubbed where logic does not yet exist.
//!
//! Single-definition rule (spec §5.2/§4.4): the `Integrator` and `StateView`
//! traits are defined ONLY here. `integrator.rs` (Task 8) imports
//! `crate::contract::Integrator` and writes impls only — it must not
//! re-declare or `pub use` re-export the trait.
//!
//! The primitive seam enums (`Lod`, `NavDest`, `Target`, `EntityRef`,
//! `CommandKind`) live in `crate::types` (Task 3) so `stores.rs` can consume
//! them without a contract<->stores cycle; this module imports them.

use crate::ids::{BodyId, CraftId};
use crate::math::Vec3;
use crate::time::{Dt, Tick};
use crate::types::{CommandKind, EntityRef, Lod, NavDest, Target};

// ---- command DTO ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    pub target: Target,
    pub kind: CommandKind,
}

/// Total, deterministic ordering across World/Sim/Entity scopes for canonical
/// apply. Returns `(scope_rank, slot, gen)` with `Sim=0, World=1, Entity=2`.
pub fn command_sort_key(c: &Command) -> (u8, u32, u32) {
    match c.target {
        Target::Sim => (0, 0, 0),
        Target::World => (1, 0, 0),
        Target::Entity(EntityRef::Craft(id)) => (2, id.slot, id.gen),
        Target::Entity(EntityRef::Body(id)) => (2, id.slot, id.gen),
    }
}

// ---- event stream ----

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    Arrival { craft: CraftId, dest: NavDest },
    FuelEmpty { craft: CraftId },
    ThrustApplied { craft: CraftId, dv: f64 },
    ActionIngested { target: Target },
    Reward { craft: CraftId, value: f64 },
    /// Emitted by the LOD-dispatch seam in `World::step` on a
    /// Dormant -> Active transition (the §3.2 wake hook). The
    /// emitting branch is Task 12; the variant is pinned here.
    Wake { craft: CraftId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    pub tick: Tick,
    pub kind: EventKind,
}

// ---- integrator trait (DEFINED ONCE; Task 8 supplies impls only) ----

/// Verlet needs body pos at BOTH t_n and t_{n+1}; impls take an ephemeris
/// sampler. `accel_at` returns gravity(softened) + thrust at a sub-tick.
pub trait Integrator {
    fn step_craft(
        &self,
        pos: Vec3,
        vel: Vec3,
        accel_at: &dyn Fn(Vec3, f64 /*sub_t in days*/) -> Vec3,
        dt: f64,
        n_substeps: u32,
    ) -> (Vec3, Vec3);
    fn name(&self) -> &'static str;
}

// ---- state-access read trait (DEFINED ONCE; Task 12 impls for World) ----

/// Read trait ALL facades read through. Carries intent (cmd + event history),
/// not just physics. Methods reference only ids / Tick / Dt / Vec3 / Command /
/// Event / Lod, so the trait compiles standalone (no `World` yet).
pub trait StateView {
    fn tick(&self) -> Tick;
    fn dt(&self) -> Dt;
    fn craft_ids(&self) -> Vec<CraftId>;
    fn craft_pos(&self, id: CraftId) -> Option<Vec3>;
    fn craft_vel(&self, id: CraftId) -> Option<Vec3>;
    fn craft_fuel(&self, id: CraftId) -> Option<f64>;
    /// Effective fuel capacity. The real impl (Task 12) reads
    /// `effective_params(&spec).fuel_capacity` — NEVER `base_fuel_capacity`
    /// (§5.5: physics/readers go through the effective-param accessor).
    fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64>;
    fn body_ids(&self) -> Vec<BodyId>;
    fn body_pos(&self, id: BodyId, tick: Tick) -> Option<Vec3>;
    fn recent_commands(&self, since: Tick) -> &[Command];
    fn recent_events(&self, since: Tick) -> &[Event];
    fn lod(&self, id: CraftId) -> Option<Lod>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{BodyId, CraftId};
    use crate::math::Vec3;
    use crate::types::{CommandKind, EntityRef, Lod, NavDest, Target};

    fn dest_cmd(target: Target) -> Command {
        Command {
            target,
            kind: CommandKind::Destination {
                dest: NavDest::Position(Vec3::ZERO),
                burn_budget: None,
            },
        }
    }

    #[test]
    fn command_sort_key_total_order() {
        let sim = dest_cmd(Target::Sim);
        let world = dest_cmd(Target::World);
        let craft_a = dest_cmd(Target::Entity(EntityRef::Craft(CraftId {
            slot: 5,
            gen: 0,
        })));
        let craft_b = dest_cmd(Target::Entity(EntityRef::Craft(CraftId {
            slot: 2,
            gen: 1,
        })));
        let body = dest_cmd(Target::Entity(EntityRef::Body(BodyId {
            slot: 3,
            gen: 0,
        })));

        // Scope ranks: Sim=0, World=1, Entity=2.
        assert_eq!(command_sort_key(&sim), (0, 0, 0));
        assert_eq!(command_sort_key(&world), (1, 0, 0));
        assert_eq!(command_sort_key(&craft_a), (2, 5, 0));
        assert_eq!(command_sort_key(&craft_b), (2, 2, 1));
        assert_eq!(command_sort_key(&body), (2, 3, 0));

        // Sorting a shuffled mix yields a total, deterministic order:
        // Sim, World, then entities by (slot, gen).
        let mut v = [craft_a, body, sim, craft_b, world];
        v.sort_by_key(command_sort_key);
        let keys: Vec<(u8, u32, u32)> = v.iter().map(command_sort_key).collect();
        assert_eq!(
            keys,
            vec![(0, 0, 0), (1, 0, 0), (2, 2, 1), (2, 3, 0), (2, 5, 0)]
        );
    }

    #[test]
    fn command_sort_key_relies_on_stable_sort_for_collisions() {
        // A Craft and a Body with identical slot/gen deliberately map to the
        // SAME sort key (2,7,1). Canonical apply ordering therefore depends on
        // `sort_by_key` being STABLE — an unstable sort could reorder these and
        // the existing total-order test (all-distinct keys) would not catch it.
        let craft = dest_cmd(Target::Entity(EntityRef::Craft(CraftId {
            slot: 7,
            gen: 1,
        })));
        let body = dest_cmd(Target::Entity(EntityRef::Body(BodyId {
            slot: 7,
            gen: 1,
        })));

        // Both collide on the exact same key.
        assert_eq!(command_sort_key(&craft), (2, 7, 1));
        assert_eq!(command_sort_key(&body), (2, 7, 1));
        assert_eq!(command_sort_key(&craft), command_sort_key(&body));
        // ...yet the commands themselves are distinct (different entity kind).
        assert_ne!(craft, body);

        // Insertion order craft-then-body must be preserved after the stable sort.
        let mut v1 = [craft, body];
        v1.sort_by_key(command_sort_key);
        assert_eq!(v1[0], craft, "stable sort keeps the first-inserted craft first");
        assert_eq!(v1[1], body);

        // The reverse insertion order is likewise preserved (proves it is the
        // insertion order, not an accidental tie-break on content).
        let mut v2 = [body, craft];
        v2.sort_by_key(command_sort_key);
        assert_eq!(v2[0], body, "stable sort keeps the first-inserted body first");
        assert_eq!(v2[1], craft);
    }

    #[test]
    fn enums_round_trip_via_partial_eq() {
        let c = CraftId { slot: 7, gen: 2 };

        // Command equality (PartialEq, holds f64 via burn_budget).
        let cmd = Command {
            target: Target::Entity(EntityRef::Craft(c)),
            kind: CommandKind::Destination {
                dest: NavDest::Entity(EntityRef::Craft(c)),
                burn_budget: Some(1.5),
            },
        };
        assert_eq!(cmd, cmd);
        assert_ne!(cmd.target, Target::World);

        // Event equality (PartialEq).
        let e1 = Event {
            tick: Tick(10),
            kind: EventKind::Arrival {
                craft: c,
                dest: NavDest::Position(Vec3::new(1.0, 2.0, 3.0)),
            },
        };
        let e2 = Event {
            tick: Tick(10),
            kind: EventKind::FuelEmpty { craft: c },
        };
        assert_eq!(e1, e1);
        assert_ne!(e1, e2);

        // New surface (fix #10): the Wake variant is Copy + PartialEq and
        // distinct from other kinds.
        let wake = Event {
            tick: Tick(10),
            kind: EventKind::Wake { craft: c },
        };
        let wake_copy = wake; // Copy
        assert_eq!(wake, wake_copy);
        assert_ne!(wake, e2);

        // Lod is Eq.
        assert_eq!(Lod::Player, Lod::Player);
        assert_ne!(Lod::Player, Lod::Nothing);
    }

    /// Trivial integrator: forward-Euler-ish, proves the trait is object-safe and
    /// implementable against the real signature.
    struct Dummy;
    impl Integrator for Dummy {
        fn step_craft(
            &self,
            pos: Vec3,
            vel: Vec3,
            accel_at: &dyn Fn(Vec3, f64) -> Vec3,
            dt: f64,
            _n_substeps: u32,
        ) -> (Vec3, Vec3) {
            let a = accel_at(pos, 0.0);
            (pos.add(vel.scale(dt)), vel.add(a.scale(dt)))
        }
        fn name(&self) -> &'static str {
            "dummy"
        }
    }

    #[test]
    fn integrator_trait_is_implementable_and_object_safe() {
        let integ = Dummy;
        let obj: &dyn Integrator = &integ; // object-safety check
        assert_eq!(obj.name(), "dummy");

        let zero_accel = |_p: Vec3, _t: f64| Vec3::ZERO;
        let (p, v) = obj.step_craft(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            &zero_accel,
            2.0,
            1,
        );
        assert_eq!(p, Vec3::new(2.0, 0.0, 0.0)); // pos += vel*dt
        assert_eq!(v, Vec3::new(1.0, 0.0, 0.0)); // vel unchanged (zero accel)
    }

    /// Trivial StateView backed by owned Vecs — proves the read trait is usable
    /// without `World`, including the slice-returning intent methods AND the new
    /// `craft_fuel_capacity` accessor (fix #2).
    struct DummyView {
        commands: Vec<Command>,
        events: Vec<Event>,
    }
    impl StateView for DummyView {
        fn tick(&self) -> Tick {
            Tick(0)
        }
        fn dt(&self) -> Dt {
            Dt::new(1.0)
        }
        fn craft_ids(&self) -> Vec<CraftId> {
            Vec::new()
        }
        fn craft_pos(&self, _id: CraftId) -> Option<Vec3> {
            None
        }
        fn craft_vel(&self, _id: CraftId) -> Option<Vec3> {
            None
        }
        fn craft_fuel(&self, _id: CraftId) -> Option<f64> {
            None
        }
        fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
            // Trivial backing: a known id resolves to a capacity, others None.
            // The real impl (Task 12) returns effective_params(&spec).fuel_capacity.
            if id == (CraftId { slot: 0, gen: 0 }) {
                Some(100.0)
            } else {
                None
            }
        }
        fn body_ids(&self) -> Vec<BodyId> {
            Vec::new()
        }
        fn body_pos(&self, _id: BodyId, _tick: Tick) -> Option<Vec3> {
            None
        }
        fn recent_commands(&self, _since: Tick) -> &[Command] {
            &self.commands
        }
        fn recent_events(&self, _since: Tick) -> &[Event] {
            &self.events
        }
        fn lod(&self, _id: CraftId) -> Option<Lod> {
            Some(Lod::Player)
        }
    }

    #[test]
    fn state_view_trait_is_implementable_standalone() {
        let view = DummyView {
            commands: vec![dest_cmd(Target::World)],
            events: vec![Event {
                tick: Tick(1),
                kind: EventKind::ActionIngested {
                    target: Target::World,
                },
            }],
        };
        let obj: &dyn StateView = &view; // object-safety check
        assert_eq!(obj.tick(), Tick(0));
        assert_eq!(obj.dt().get(), 1.0);
        assert_eq!(obj.recent_commands(Tick(0)).len(), 1);
        assert_eq!(obj.recent_events(Tick(0)).len(), 1);
        assert_eq!(obj.lod(CraftId { slot: 0, gen: 0 }), Some(Lod::Player));

        // New surface (fix #2): craft_fuel_capacity is Option-typed and present.
        assert_eq!(
            obj.craft_fuel_capacity(CraftId { slot: 0, gen: 0 }),
            Some(100.0)
        );
        assert_eq!(obj.craft_fuel_capacity(CraftId { slot: 9, gen: 9 }), None);
    }
}
