//! Primitive seam types shared across the contract (spec §4.4). Split into their
//! own module so `stores.rs` (Task 4: `NavState{dest:NavDest}`, `lod:Vec<Lod>`)
//! resolves them BEFORE `contract.rs` (Task 6) builds `Command`/`Event`/traits on
//! top — this breaks the stores<->contract cycle. These are pure data: no methods.

use crate::ids::{BodyId, CraftId, StationId};
use crate::math::Vec3;

/// A directed route between two stations (from, to). The key for per-route risk
/// registers / beliefs (Components D/E). `Copy + Eq + Hash + Ord` so it indexes a
/// map and sorts deterministically. Pure data: no methods.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RouteKey(pub StationId, pub StationId);

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

/// Command kinds (spec §4.4). `Destination` carries an optional scalar Δv cap.
/// The economy kinds (`AcceptContract`/`SetRole`) are resolved on the single
/// ingestion path against the live World. `CommandKind` is NOT hashed (commands
/// resolve into already-hashed state), so adding variants is hash-neutral.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandKind {
    Destination {
        dest: NavDest,
        burn_budget: Option<f64>,
    },
    /// Intent to take a contract: ingestion sets the target craft's `contract`
    /// column + `role = Hauler` (deferred state transition lives in resolve_contracts).
    AcceptContract { contract: crate::ids::ContractId },
    /// Set the target craft's economic role.
    SetRole { role: crate::stores::CraftRole },
    /// Direct thrust intent (tactical Rung 1): world-frame throttle vector,
    /// |v| in [0,1] = throttle fraction (over-length clamps to 1 at the
    /// autopilot pass-through). Persists as NavState::DirectThrust until replaced.
    Thrust { throttle_vec: crate::math::Vec3 },
    /// Intent to buy an upgrade SHIP (pirates rung §6 — the fleet ledger buys
    /// ships, never stat levels): ingestion writes the transient
    /// `pending_upgrade` column only; the settle (vendor dock check, price
    /// debit, Yard credit, count bump) lives in `resolve_purchases` (stage 1d),
    /// which consumes the intent the same tick.
    BuyUpgrade { kind: crate::stores::UpgradeKind },
    /// Intent to top up propellant at the docked station (world-gets-big §5):
    /// ingestion writes the transient `pending_refuel` column only; the settle
    /// lives in `resolve_refuels` (stage 1d2), which consumes the intent the
    /// same tick. Top-to-full, threshold-free: the verb carries no quantity.
    Refuel,
    /// Intent to buy goods from the docked station (goods-as-goods rung A):
    /// ingestion writes the transient `pending_trade_buy` column only; the settle
    /// lives in `resolve_trade_buys` (stage 1dx), which consumes the intent the
    /// same tick. Payload: (good, qty, source station).
    TradeBuy {
        good: crate::economy::Good,
        qty: u32,
        station: crate::ids::StationId,
    },
    /// Intent to sell the held goods at the docked station (goods-as-goods rung A):
    /// ingestion writes the transient `pending_trade_sell` column only; the settle
    /// lives in `resolve_trade_sells` (stage 1dx). Payload: destination station.
    TradeSell { station: crate::ids::StationId },
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
        let c = EntityRef::Craft(CraftId {
            slot: 0,
            generation: 0,
        });
        let b = EntityRef::Body(BodyId {
            slot: 0,
            generation: 0,
        });
        assert_ne!(c, b);
    }

    #[test]
    fn target_carries_all_scopes() {
        let e = Target::Entity(EntityRef::Body(BodyId {
            slot: 2,
            generation: 1,
        }));
        assert_ne!(e, Target::World);
        assert_ne!(Target::World, Target::Sim);
    }

    #[test]
    fn navdest_supports_position_and_entity() {
        let p = NavDest::Position(Vec3::new(1.0, 2.0, 3.0));
        let en = NavDest::Entity(EntityRef::Craft(CraftId {
            slot: 1,
            generation: 0,
        }));
        assert_ne!(p, en);
        assert_eq!(p, NavDest::Position(Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn route_key_is_directed_hashable_ordered() {
        use std::collections::HashSet;
        let a = StationId {
            slot: 0,
            generation: 0,
        };
        let b = StationId {
            slot: 1,
            generation: 0,
        };
        let ab = RouteKey(a, b);
        let ba = RouteKey(b, a);
        // Directed: (a,b) != (b,a).
        assert_ne!(ab, ba);
        // Hashable (usable as a map/set key).
        let mut set = HashSet::new();
        set.insert(ab);
        assert!(set.contains(&RouteKey(a, b)));
        assert!(!set.contains(&ba));
        // Ord is total and deterministic.
        assert!(ab < ba);
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
            other => panic!("expected Destination, got {other:?}"),
        }
    }
}
