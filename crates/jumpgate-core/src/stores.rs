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

/// The ONLY accessor the integrator/autopilot read for craft params. v1 applies
/// `mods.thrust_factor` to `max_thrust`; all other fields pass through. With
/// `EffectiveMods::IDENTITY` the result is bit-identical to the base spec.
/// Use a plain `*` (no FMA contraction) so the multiply is a single rounding.
pub fn effective_params(spec: &BaseSpec, mods: &EffectiveMods) -> Effective {
    Effective {
        dry_mass: spec.base_dry_mass,
        max_thrust: spec.base_max_thrust * mods.thrust_factor,
        exhaust_velocity: spec.base_exhaust_velocity,
        fuel_capacity: spec.base_fuel_capacity,
    }
}

/// Per-craft EFFECTIVE-parameter modifier bundle — the single combined multiply
/// applied to `BaseSpec` by `effective_params`. This is the `× component-mods ×
/// wear` half of the founding `Effective = base × component-mods × wear` intent,
/// PRE-REDUCED into one struct so `effective_params` never changes signature
/// again as new factor sources land.
///
/// v1 carries only `thrust_factor` (the crew-contributed engine multiplier,
/// written later by `compute_crew_mods`). Future wear/component factors fold into
/// the SAME bundle (e.g. `compute_wear`), never a new `effective_params` arg.
///
/// DERIVED state: written by the crew-mod / wear tick-stages; read by
/// `effective_params`. NOT folded into the per-tick state hash — transitively
/// pinned by its hashed inputs, exactly like `prev_fuel`. v1: identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveMods {
    /// Multiplier on `max_thrust` — the one wired channel in v1. `1.0` == no effect.
    pub thrust_factor: f64,
    // Reserved (default 1.0 in IDENTITY) for wear/component factors — adding a
    // field here is additive; it is NEVER a change to `effective_params`'s signature.
}

impl EffectiveMods {
    /// The no-effect value. `effective_params(spec, &IDENTITY)` is bit-identical
    /// to the pre-bundle `effective_params(spec)` (`x * 1.0 == x` for finite f64).
    pub const IDENTITY: EffectiveMods = EffectiveMods { thrust_factor: 1.0 };
}

/// SoA store for mobile craft. `ids` is the slot/generation authority; every other Vec
/// is indexed by the same dense row (v1 invariant: `slot == row`) and must stay
/// length-parallel. `prev_fuel` / `prev_inside_dest` / `prev_pos` snapshot the
/// previous tick's values for edge-triggered event detection
/// (`detect_boundary_events` reads them; `World::step` copy-forwards into them at
/// tick end). They are NOT folded into the per-tick `state_hash` in v1 — they sit
/// at deferred `HASH_FIELD_ORDER` words 14/15 behind a future `HASH_FORMAT_VERSION`
/// bump, and are transitively pinned anyway (`prev_fuel[t] == fuel[t-1]`, which is
/// hashed at tick t-1). `prev_pos` is the analogous copy-forward of `pos` (the
/// swept-arrival chord start, §5): `prev_pos[t] == pos[t-1]`, hashed at word 9 at
/// tick t-1, so it too is transitively pinned and unhashed-by-design (Class-3).
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
    pub prev_pos: Vec<Vec3>,
    /// Per-craft EFFECTIVE-modifier cache. DERIVED (written by the crew-mod / wear
    /// tick-stages; IDENTITY until then), length-parallel, NOT hashed. Initialized
    /// to `EffectiveMods::IDENTITY` so reads before the first `step` (projections)
    /// are well-defined. INVARIANT: `mods` is a pure function of state that is
    /// either constant (v1) or folded into HASH_FIELD_ORDER (Plan B); it must NEVER
    /// depend on an unhashed runtime-mutable input.
    pub mods: Vec<EffectiveMods>,
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
            prev_pos: Vec::new(),
            mods: Vec::new(),
        }
    }

    /// Append a craft, returning its typed `CraftId`. Initializes `nav = Idle`,
    /// `lod = Player`, and the prev-* snapshots (`prev_fuel = fuel`,
    /// `prev_inside_dest = false`, `prev_pos = pos` so the tick-0 swept chord is
    /// zero-length and never spuriously clips R). Enforces the v1 `slot == row`
    /// invariant.
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
        self.prev_pos.push(pos);
        self.mods.push(EffectiveMods::IDENTITY);
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
            .map(|i| effective_params(&self.spec[i], &self.mods[i]).fuel_capacity)
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
    fn effective_mods_identity_is_unit() {
        let m = EffectiveMods::IDENTITY;
        assert_eq!(m.thrust_factor, 1.0);
        let n = m; // Copy + PartialEq are part of the contract (read every tick).
        assert_eq!(m, n);
    }

    #[test]
    fn effective_scales_with_thrust_factor() {
        use crate::config::BaseSpec;
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
        };
        // IDENTITY is bit-identical to the base numbers.
        let id = effective_params(&spec, &EffectiveMods::IDENTITY);
        assert_eq!(id.max_thrust, 250.0);
        assert_eq!(id.dry_mass, spec.base_dry_mass);
        assert_eq!(id.exhaust_velocity, spec.base_exhaust_velocity);
        assert_eq!(id.fuel_capacity, spec.base_fuel_capacity);

        // thrust_factor multiplies ONLY max_thrust (the one wired channel in v1).
        let boosted = effective_params(&spec, &EffectiveMods { thrust_factor: 1.5 });
        assert_eq!(boosted.max_thrust, 375.0);
        assert_eq!(boosted.dry_mass, spec.base_dry_mass, "dry_mass unaffected");
        assert_eq!(boosted.exhaust_velocity, spec.base_exhaust_velocity, "v_e unaffected");
        assert_eq!(boosted.fuel_capacity, spec.base_fuel_capacity, "capacity unaffected");
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
        let eff = effective_params(&spec, &EffectiveMods::IDENTITY);
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
        assert_eq!(ship.prev_pos.len(), n);
        // mods is a length-parallel DERIVED column, initialized to IDENTITY.
        assert_eq!(ship.mods.len(), n);

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
        assert_eq!(ship.prev_pos.len(), n);
        assert_eq!(ship.mods.len(), n);
        assert_eq!(ship.mods[0], EffectiveMods::IDENTITY, "push initializes mods to IDENTITY");
        assert_eq!(ship.mods[1], EffectiveMods::IDENTITY);

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
