//! Per-type Struct-of-Arrays stores keyed by generational slot-map ids.

use crate::config::BaseSpec;
use crate::ids::{CraftId, SlotMap};
use crate::math::Vec3;
use crate::types::{Lod, NavDest};

/// Resolved navigation state the autopilot reads each tick. This is the RESOLVED
/// field (set by command ingestion), NOT a `Command` — the autopilot never reads
/// the command stream directly.
#[derive(Clone, Copy, Debug)]
pub enum NavState {
    Idle,
    Seeking { dest: NavDest, dv_remaining: f64 },
}

/// Effective ship parameters = base × component-mods × wear. In v1 the mod and
/// wear factors are identity, so `effective == base`. The integrator and autopilot
/// read ONLY through this accessor — never `BaseSpec` directly (§5.5 seam).
#[derive(Clone, Copy, Debug)]
pub struct Effective {
    pub dry_mass: f64,
    pub max_thrust: f64,
    pub exhaust_velocity: f64,
    pub fuel_capacity: f64,
}

/// The ONLY accessor the integrator/autopilot read for ship params. v1: identity.
pub fn effective_params(spec: &BaseSpec) -> Effective {
    Effective {
        dry_mass: spec.base_dry_mass,
        max_thrust: spec.base_max_thrust,
        exhaust_velocity: spec.base_exhaust_velocity,
        fuel_capacity: spec.base_fuel_capacity,
    }
}

/// SoA store for mobile craft. `ids` is the slot/generation authority; every other Vec
/// is indexed by the same dense row (v1 invariant: `slot == row`) and must stay
/// length-parallel. `prev_fuel` / `prev_inside_dest` snapshot the previous tick's
/// values for edge-triggered event detection (`detect_boundary_events` reads them;
/// `World::step` copy-forwards into them at tick end). They are NOT folded into the
/// per-tick `state_hash` in v1 — they sit at deferred `HASH_FIELD_ORDER` words 14/15
/// behind a future `HASH_FORMAT_VERSION` bump, and are transitively pinned anyway
/// (`prev_fuel[t] == fuel[t-1]`, which is hashed at tick t-1).
pub struct CraftStore {
    pub ids: SlotMap<()>,
    pub pos: Vec<Vec3>,
    pub vel: Vec<Vec3>,
    pub fuel_mass: Vec<f64>,
    pub spec: Vec<BaseSpec>,
    pub nav: Vec<NavState>,
    pub lod: Vec<Lod>,
    pub prev_fuel: Vec<f64>,
    pub prev_inside_dest: Vec<bool>,
}

/// SoA store for massive on-rails bodies. `eph_index` maps a body slot to its
/// row in the precomputed ephemeris table (§5.4).
pub struct BodyStore {
    pub ids: SlotMap<()>,
    pub mass: Vec<f64>,
    pub eph_index: Vec<usize>,
}

impl CraftStore {
    /// A zero-craft store with every SoA array empty. All craft are minted via
    /// `push` at reset; there is no mid-run despawn in v1, so slots allocate
    /// contiguously and `slot == row` holds.
    pub fn empty() -> Self {
        CraftStore {
            ids: SlotMap::new(),
            pos: Vec::new(),
            vel: Vec::new(),
            fuel_mass: Vec::new(),
            spec: Vec::new(),
            nav: Vec::new(),
            lod: Vec::new(),
            prev_fuel: Vec::new(),
            prev_inside_dest: Vec::new(),
        }
    }

    /// Append a craft, returning its typed `CraftId`. Initializes `nav = Idle`,
    /// `lod = Player`, and the prev-* snapshots (`prev_fuel = fuel`,
    /// `prev_inside_dest = false`). Enforces the v1 `slot == row` invariant.
    pub fn push(&mut self, spec: BaseSpec, pos: Vec3, vel: Vec3, fuel: f64) -> CraftId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(
            slot as usize,
            self.pos.len(),
            "v1 invariant violated: slot must equal dense row (no mid-run despawn)"
        );
        self.pos.push(pos);
        self.vel.push(vel);
        self.fuel_mass.push(fuel);
        self.spec.push(spec);
        self.nav.push(NavState::Idle);
        self.lod.push(Lod::Player);
        self.prev_fuel.push(fuel);
        self.prev_inside_dest.push(false);
        CraftId { slot, generation }
    }

    /// The typed `CraftId` occupying dense row `idx`. Panics if `idx` is not a
    /// live row (callers iterate `0..ids.len()` over a no-despawn v1 store).
    pub fn ids_at(&self, idx: usize) -> CraftId {
        let (slot, generation) = self
            .ids
            .id_at(idx)
            .expect("ids_at called with a non-live dense row");
        CraftId { slot, generation }
    }

    /// Dense SoA row for a live `CraftId`, or `None` for a stale/unknown id.
    pub fn index_of(&self, id: CraftId) -> Option<usize> {
        self.ids.dense_index(id.slot, id.generation)
    }

    /// Position of a live craft by id, or `None` if the id is stale.
    pub fn craft_pos_by_id(&self, id: CraftId) -> Option<Vec3> {
        self.index_of(id).map(|i| self.pos[i])
    }

    /// Effective fuel capacity of a live craft by id, read through
    /// `effective_params` (capacity's single source of truth is `spec`). `None`
    /// for a stale id.
    pub fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
        self.index_of(id)
            .map(|i| effective_params(&self.spec[i]).fuel_capacity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{BodyId, SlotMap};
    use crate::math::Vec3;
    use crate::types::NavDest;

    #[test]
    fn navstate_and_effective_shapes() {
        let idle = NavState::Idle;
        let seeking = NavState::Seeking {
            dest: NavDest::Position(Vec3::new(1.0, 2.0, 3.0)),
            dv_remaining: 0.5,
        };
        // both variants are constructible and Copy (used by value in the integrator).
        let _copy = idle;
        let _copy2 = seeking;

        let eff = Effective {
            dry_mass: 1.0,
            max_thrust: 2.0,
            exhaust_velocity: 3.0,
            fuel_capacity: 4.0,
        };
        assert_eq!(eff.dry_mass, 1.0);
        assert_eq!(eff.max_thrust, 2.0);
        assert_eq!(eff.exhaust_velocity, 3.0);
        assert_eq!(eff.fuel_capacity, 4.0);
    }

    #[test]
    fn effective_equals_base_in_v1() {
        use crate::config::BaseSpec;
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
        };
        let eff = effective_params(&spec);
        assert_eq!(eff.dry_mass, spec.base_dry_mass);
        assert_eq!(eff.max_thrust, spec.base_max_thrust);
        assert_eq!(eff.exhaust_velocity, spec.base_exhaust_velocity);
        assert_eq!(eff.fuel_capacity, spec.base_fuel_capacity);
    }

    #[test]
    fn stores_construct_soa_parallel() {
        let ship = CraftStore::empty();
        assert_eq!(ship.ids.len(), 0);
        let n = ship.ids.len();
        assert_eq!(ship.pos.len(), n);
        assert_eq!(ship.vel.len(), n);
        assert_eq!(ship.fuel_mass.len(), n);
        assert_eq!(ship.spec.len(), n);
        assert_eq!(ship.nav.len(), n);
        assert_eq!(ship.lod.len(), n);
        // the two prev-* arrays for edge-triggered event detection (deferred
        // HASH_FIELD_ORDER words 14/15; not folded into state_hash in v1) start
        // empty and parallel.
        assert_eq!(ship.prev_fuel.len(), n);
        assert_eq!(ship.prev_inside_dest.len(), n);

        let mut body = BodyStore {
            ids: SlotMap::new(),
            mass: Vec::new(),
            eph_index: Vec::new(),
        };
        let (bslot, bgen) = body.ids.insert(());
        let bid = BodyId {
            slot: bslot,
            generation: bgen,
        };
        body.mass.push(1.0);
        body.eph_index.push(0);
        assert_eq!(bid.slot, bslot);
        assert_eq!(body.mass.len(), body.ids.len());
        assert_eq!(body.eph_index.len(), body.ids.len());
    }

    #[test]
    fn shipstore_push_and_accessors() {
        let mut ship = CraftStore::empty();
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
        };
        let id0 = ship.push(spec.clone(), Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, 40.0);
        let id1 = ship.push(spec.clone(), Vec3::new(2.0, 0.0, 0.0), Vec3::ZERO, 20.0);
        assert_eq!(
            id0,
            CraftId {
                slot: 0,
                generation: 0
            }
        );
        assert_eq!(
            id1,
            CraftId {
                slot: 1,
                generation: 0
            }
        );

        // every SoA array stayed length-parallel, including the prev-* pair.
        let n = ship.ids.len();
        assert_eq!(n, 2);
        assert_eq!(ship.pos.len(), n);
        assert_eq!(ship.vel.len(), n);
        assert_eq!(ship.fuel_mass.len(), n);
        assert_eq!(ship.spec.len(), n);
        assert_eq!(ship.nav.len(), n);
        assert_eq!(ship.lod.len(), n);
        assert_eq!(ship.prev_fuel.len(), n);
        assert_eq!(ship.prev_inside_dest.len(), n);

        // ids_at wraps the dense row into a typed CraftId.
        assert_eq!(ship.ids_at(0), id0);
        assert_eq!(ship.ids_at(1), id1);

        // index_of resolves a live typed id to its row; stale -> None.
        assert_eq!(ship.index_of(id0), Some(0));
        assert_eq!(ship.index_of(id1), Some(1));
        let stale = CraftId {
            slot: 0,
            generation: 99,
        };
        assert_eq!(ship.index_of(stale), None, "stale generation -> None");

        // craft_pos_by_id reads the row's position; stale -> None.
        assert_eq!(ship.craft_pos_by_id(id0), Some(Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!(ship.craft_pos_by_id(id1), Some(Vec3::new(2.0, 0.0, 0.0)));
        assert_eq!(ship.craft_pos_by_id(stale), None);

        // craft_fuel_capacity reads through effective_params (spec is the single
        // source of truth), NOT current fuel_mass.
        assert_eq!(ship.craft_fuel_capacity(id0), Some(40.0));
        assert_eq!(ship.craft_fuel_capacity(id1), Some(40.0));
        assert_eq!(ship.craft_fuel_capacity(stale), None);

        // initial nav/prev-* defaults set by push.
        assert!(matches!(ship.nav[0], NavState::Idle));
        assert_eq!(ship.prev_fuel[1], 20.0);
        assert!(!ship.prev_inside_dest[0]);
    }
}
