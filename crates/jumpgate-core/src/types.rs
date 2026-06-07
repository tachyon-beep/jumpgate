//! Primitive seam types shared across the contract (spec §4.4). Split into their
//! own module so `stores.rs` (Task 4: `NavState{dest:NavDest}`, `lod:Vec<Lod>`)
//! resolves them BEFORE `contract.rs` (Task 6) builds `Command`/`Event`/traits on
//! top — this breaks the stores<->contract cycle. These are pure data: no methods.

use crate::ids::{BodyId, CraftId};
use crate::math::Vec3;

/// Level-of-detail seam (spec §3 must-shape). v1 implements `Player` behaviour;
/// the dispatch + wake-event hook lives in `world.rs`. The other variants exist
/// so the seam is shaped, not so they are built in v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lod {
    Player,
    NpcInteraction,
    Nothing,
}

/// Entity address: a craft OR a body. Generational ids keep stale refs distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityRef {
    Craft(CraftId),
    Body(BodyId),
}

/// Command address sum (spec §4.4): widened from day one so spawn / world-sim
/// interventions / time-scoped commands are not foreclosed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Entity(EntityRef),
    World,
    Sim,
}

/// Navigator destination: an absolute position OR an entity to chase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavDest {
    Position(Vec3),
    Entity(EntityRef),
}

/// v1's ONLY command kind. `burn_budget`: optional scalar Δv cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandKind {
    Destination {
        dest: NavDest,
        burn_budget: Option<f64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lod_has_the_three_contract_variants() {
        // Compiles iff exactly these variants exist; v1 default behaviour = Player.
        let all = [Lod::Player, Lod::NpcInteraction, Lod::Nothing];
        assert_eq!(all[0], Lod::Player);
        assert_ne!(Lod::Player, Lod::Nothing);
    }

    #[test]
    fn entity_ref_distinguishes_craft_from_body() {
        let c = EntityRef::Craft(CraftId { slot: 0, gen: 0 });
        let b = EntityRef::Body(BodyId { slot: 0, gen: 0 });
        assert_ne!(c, b);
    }

    #[test]
    fn target_carries_all_scopes() {
        let e = Target::Entity(EntityRef::Body(BodyId { slot: 2, gen: 1 }));
        assert_ne!(e, Target::World);
        assert_ne!(Target::World, Target::Sim);
    }

    #[test]
    fn navdest_supports_position_and_entity() {
        let p = NavDest::Position(Vec3::new(1.0, 2.0, 3.0));
        let en = NavDest::Entity(EntityRef::Craft(CraftId { slot: 1, gen: 0 }));
        assert_ne!(p, en);
        assert_eq!(p, NavDest::Position(Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn command_kind_destination_holds_dest_and_optional_budget() {
        let k = CommandKind::Destination {
            dest: NavDest::Position(Vec3::ZERO),
            burn_budget: Some(0.5),
        };
        match k {
            CommandKind::Destination { dest, burn_budget } => {
                assert_eq!(dest, NavDest::Position(Vec3::ZERO));
                assert_eq!(burn_budget, Some(0.5));
            }
        }
    }
}
