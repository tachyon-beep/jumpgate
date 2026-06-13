//! Per-type Struct-of-Arrays stores keyed by generational slot-map ids.

use crate::config::BaseSpec;
use crate::ids::{CraftId, SlotMap};
use crate::math::Vec3;
use crate::time::Tick;
use crate::types::{Lod, NavDest};

/// Resolved navigation state the autopilot reads each tick. This is the RESOLVED
/// field (set by command ingestion), NOT a `Command` — the autopilot never reads
/// the command stream directly.
#[derive(Clone, Copy, Debug)]
pub enum NavState {
    Idle,
    Seeking { dest: NavDest, dv_remaining: f64 },
    /// Direct thrust (no destination, no Δv budget — fuel is the budget).
    DirectThrust { throttle_vec: Vec3 },
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

/// Economic role of a craft. `Idle`, `Hauler`, or `Pirate` (the 2nd trophic
/// level). Hashed economy state (HASH_FIELD_ORDER, folded discriminant-first via
/// `rank()`). APPEND-ONLY: `Pirate` appends `rank() = 2`; `Idle`/`Hauler` ranks
/// are unchanged so existing replay identity is preserved for those craft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftRole {
    Idle,
    Hauler,
    Pirate,
}
impl CraftRole {
    /// Stable discriminant for self-delimiting state-hash folding. APPEND-ONLY.
    pub fn rank(self) -> u8 {
        match self {
            CraftRole::Idle => 0,
            CraftRole::Hauler => 1,
            CraftRole::Pirate => 2,
        }
    }
}

/// Per-pirate trophic state (the 2nd trophic level). Carried on `CraftStore` as
/// an `Option<PirateState>` column: `None` for every non-pirate craft, `Some` for
/// a pirate. HASHED economy state (folded self-delimitingly: tag 0 for `None`,
/// tag 1 + fields for `Some`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PirateState {
    /// Accumulated food (i64 microcredits of robbed cargo value). Drives the
    /// food-driven population dynamics (spawn / lie-low / leave).
    pub food_micros: i64,
    /// Accrued heat. Past a threshold it forces a lie-low refuge.
    pub notoriety: u32,
    /// While `lie_low_until > tick` the pirate is off the predation field (the
    /// structural refuge that stops hunters exterminating prey).
    pub lie_low_until: Tick,
    /// No re-engagement until this tick (the post-rob/drive-off cooldown that
    /// stops same-pair rerolls every tick, spec §2). HASHED: appended INSIDE
    /// the word-26 self-delimiting `Some` fold (format v4).
    pub engage_cooldown_until: Tick,
}

/// What a `BuyUpgrade` intent purchases (spec §6 catalog: a SHIP, never a stat).
/// APPEND-ONLY: never reorder the variants — the `UpgradePurchased` event payload
/// and any future stable-rank fold depend on their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeKind {
    Hull,
    Escort,
}

/// **Fleet-ledger honesty (owner caveat, spec §6, decision-3 sign-off):** the
/// columns are `UpgradeLevels { hulls: u8, escorts: u8 }` — **counts of ships the
/// craft owns that the sim does not yet individually fly**, never abstract stat
/// levels. The purchase verb buys a SHIP; the chronicle narrates a wing ("H7's
/// escorts drove them off"), never a level-up. A fleet is "a collection of ships
/// with a single policy acting as a strategic head" (the commodore chair, per the
/// glossary captain→commodore→admiral taxonomy) — so the migration is a DEMOTION
/// of this ledger, not a reinterpretation: when the commodore rung lands, each
/// count mints real craft into a fleet under one GuidanceParams policy, and the
/// columns die. Named sunset debt: **the ledger must not outlive the fleet rung.**
/// Implementation rules that keep the demotion honest: nothing may fold
/// escorts/hulls into physics or EffectiveMods (ships fly, stats don't); nothing
/// may assume the wing is unlosable (attrition becomes possible the day they're
/// real); caps stay small (2) so the un-simulated wing never grows past what a
/// chronicle line can carry.
///
/// Strength and capacity are DERIVED from these counts, never stored. HASHED
/// state (HASH_FIELD_ORDER word 27, format v4).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct UpgradeLevels {
    pub hulls: u8,
    pub escorts: u8,
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
    // --- Hauler economy columns (length-parallel, HASHED economy state) ---
    /// Economic role. `Idle` until a contract is accepted (`Hauler`).
    pub role: Vec<CraftRole>,
    /// Loaded cargo: `Some((resource, qty))` while carrying a delivery, else `None`.
    /// Distinct from `fuel_mass` (propellant) — traded Fuel is cargo in v1.
    pub cargo: Vec<Option<(crate::economy::Good, u32)>>,
    /// Earned credits (i64 microcredits). Paid from contract escrow on delivery.
    pub credits_micros: Vec<i64>,
    /// The contract this craft is fulfilling, or `None`.
    pub contract: Vec<Option<crate::ids::ContractId>>,
    // --- Trophic columns (length-parallel, HASHED economy state) ---
    /// Per-hauler risk appetite (fixed-point 0..=1000; 0 default; cautious→greedy).
    /// Drives the heterogeneous belief-weighted contract choice (Component C).
    pub risk_appetite: Vec<i32>,
    /// Per-pirate trophic state: `Some(PirateState)` for a pirate, `None` otherwise.
    pub pirate: Vec<Option<PirateState>>,
    // --- Pirates-rung columns (length-parallel, state v4) ---
    /// The fleet ledger (see `UpgradeLevels` doc: counts of un-simulated ships,
    /// never stat levels). HASHED (HASH_FIELD_ORDER word 27).
    pub upgrades: Vec<UpgradeLevels>,
    /// Tick of this craft's last information refresh — dock-gated route-evidence
    /// read freshness (spec §7): refreshed only while docked, stale in flight.
    /// HASHED (HASH_FIELD_ORDER word 28).
    pub info_tick: Vec<Tick>,
    /// TRANSIENT purchase intent (the `prev_*` unhashed-by-design doc pattern,
    /// but stricter): written by ingest/scripted policy, consumed by
    /// `resolve_purchases` (stage 1d) the SAME tick, so it is always `None` at
    /// every hash point — `state_hash` debug_asserts exactly that. NOT folded
    /// into HASH_FIELD_ORDER.
    pub pending_upgrade: Vec<Option<UpgradeKind>>,
    /// TRANSIENT refuel intent (world-gets-big §5 — the `pending_upgrade`
    /// pattern): written by ingest or the scripted refuel policy, consumed by
    /// `resolve_refuels` the SAME tick, and debug-asserted all-None at hash
    /// points. NOT folded into HASH_FIELD_ORDER.
    pub pending_refuel: Vec<Option<()>>,
    /// TRANSIENT own-trade BUY intent written by run_trade_policies (stage 1c3x)
    /// and consumed unconditionally by resolve_trade_buys (stage 1dx) the same tick.
    /// Payload: (good_index, qty, source_station_id). None at every state-hash point.
    pub pending_trade_buy: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>,
    /// TRANSIENT own-trade SELL intent written by run_trade_policies (stage 1c3x)
    /// and consumed unconditionally by resolve_trade_sells (stage 1dx) the same tick.
    /// Payload: destination StationId (goods and qty read from hold). None at every hash point.
    pub pending_trade_sell: Vec<Option<crate::ids::StationId>>,
    // --- Media-rung column (length-parallel, state v5) ---
    /// Per-craft gossip comms-log: `Some(GossipBuffer)` ONLY for non-pirate
    /// rows on a media-live world — pirates are information-blind by
    /// construction (spec §16 OD-6) and get `None`, as does every row when
    /// media is off. HASHED (HASH_FIELD_ORDER word 30).
    pub gossip: Vec<Option<crate::media::GossipBuffer>>,
    // --- Goods-rung columns (HASHED v6+) ---
    /// Owned-cargo hold for own-trade craft; canonical ascending-Good no-zero-qty form.
    /// Pirates get `Vec::new()` — they never become own-traders (D6/D7).
    /// Fold: count-first after word 28 in `write_craft_economy`.
    pub hold: Vec<Vec<(crate::economy::Good, u32)>>,
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
            pending_trade_buy: Vec::new(),
            pending_trade_sell: Vec::new(),
            gossip: Vec::new(),
            hold: Vec::new(),
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
        self.role.push(CraftRole::Idle);
        self.cargo.push(None);
        self.credits_micros.push(0);
        self.contract.push(None);
        self.risk_appetite.push(0);
        self.pirate.push(None);
        self.upgrades.push(UpgradeLevels::default());
        self.info_tick.push(Tick(0));
        self.pending_upgrade.push(None);
        self.pending_refuel.push(None);
        self.pending_trade_buy.push(None);
        self.pending_trade_sell.push(None);
        // Media: `Some` only for non-pirate rows on a media-live world; the
        // generic push seeds `None` (World::reset's mint loop decides).
        self.gossip.push(None);
        // Goods-rung (v6): all craft start with an empty hold (pirates never
        // fill theirs; the empty-vec count word keeps the fold uniform).
        self.hold.push(Vec::new());
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
    fn navstate_direct_thrust_is_constructible_and_copy() {
        // Tactical Rung 1: the held-stick variant (no destination, no Δv budget —
        // fuel is the budget). Constructible + Copy like the other variants.
        let dt = NavState::DirectThrust {
            throttle_vec: Vec3::new(0.5, -0.25, 0.0),
        };
        let copy = dt;
        if let NavState::DirectThrust { throttle_vec } = copy {
            assert_eq!(throttle_vec, Vec3::new(0.5, -0.25, 0.0));
        } else {
            panic!("expected DirectThrust");
        }
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
            base_cargo_capacity: 5,
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
            base_cargo_capacity: 5,
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
    fn push_initializes_hauler_columns_idle_empty() {
        let mut ship = CraftStore::empty();
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
            base_cargo_capacity: 5,
        };
        ship.push(spec, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, 40.0);
        assert_eq!(ship.role[0], CraftRole::Idle);
        assert_eq!(ship.cargo[0], None);
        assert_eq!(ship.credits_micros[0], 0);
        assert_eq!(ship.contract[0], None);
        // Trophic columns default to 0 risk-appetite and no pirate state.
        assert_eq!(ship.risk_appetite[0], 0);
        assert_eq!(ship.pirate[0], None);
        // Pirates-rung (v4) columns default to an empty fleet ledger, tick-0
        // info freshness, and no pending purchase intent.
        assert_eq!(ship.upgrades[0], UpgradeLevels::default());
        assert_eq!(ship.upgrades[0].hulls, 0);
        assert_eq!(ship.upgrades[0].escorts, 0);
        assert_eq!(ship.info_tick[0], Tick(0));
        assert_eq!(ship.pending_upgrade[0], None);
        // length-parallel with the id authority.
        assert_eq!(ship.role.len(), ship.ids.len());
        assert_eq!(ship.cargo.len(), ship.ids.len());
        assert_eq!(ship.credits_micros.len(), ship.ids.len());
        assert_eq!(ship.contract.len(), ship.ids.len());
        assert_eq!(ship.risk_appetite.len(), ship.ids.len());
        assert_eq!(ship.pirate.len(), ship.ids.len());
        assert_eq!(ship.upgrades.len(), ship.ids.len());
        assert_eq!(ship.info_tick.len(), ship.ids.len());
        assert_eq!(ship.pending_upgrade.len(), ship.ids.len());
        assert_eq!(ship.pending_refuel.len(), ship.ids.len());
    }

    #[test]
    fn pirate_role_appends_rank_two() {
        // APPEND-ONLY: existing ranks unchanged, Pirate = 2.
        assert_eq!(CraftRole::Idle.rank(), 0);
        assert_eq!(CraftRole::Hauler.rank(), 1);
        assert_eq!(CraftRole::Pirate.rank(), 2);
    }

    #[test]
    fn trophic_columns_round_trip_on_push() {
        let mut ship = CraftStore::empty();
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
            base_cargo_capacity: 5,
        };
        ship.push(spec, Vec3::ZERO, Vec3::ZERO, 40.0);
        // Mutate the new columns and read them back (round-trip).
        ship.risk_appetite[0] = 750;
        ship.role[0] = CraftRole::Pirate;
        ship.pirate[0] = Some(PirateState {
            food_micros: 12_345,
            notoriety: 7,
            lie_low_until: Tick(99),
            engage_cooldown_until: Tick(123),
        });
        assert_eq!(ship.risk_appetite[0], 750);
        assert_eq!(ship.role[0], CraftRole::Pirate);
        assert_eq!(
            ship.pirate[0],
            Some(PirateState {
                food_micros: 12_345,
                notoriety: 7,
                lie_low_until: Tick(99),
                engage_cooldown_until: Tick(123),
            })
        );
    }

    #[test]
    fn shipstore_push_and_accessors() {
        let mut ship = CraftStore::empty();
        let spec = BaseSpec {
            base_dry_mass: 10.0,
            base_max_thrust: 250.0,
            base_exhaust_velocity: 30.0,
            base_fuel_capacity: 40.0,
            base_cargo_capacity: 5,
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
