//! The single hashed run-config struct (spec §6). Initial conditions — body set,
//! craft count, per-ship base spec, master seed, dt, softening, substep params —
//! live HERE, recorded and folded into the CONFIG hash. This config hash is
//! DISTINCT from the per-tick STATE hash (`hash.rs`): this one hashes immutable
//! initial conditions ONCE with its own `"CONFIG_1"` tag; that one hashes the
//! evolving world each tick via the shared `FnvHasher` seeded with `HASH_MAGIC`.
//! Different magic/purpose; never conflate or share state.

use crate::math::Vec3;
use crate::time::Dt;

/// Nominal ("base") ship numbers. Physics reads EFFECTIVE values via an accessor
/// (Task 4 `stores::effective_params`); v1 effective == base.
#[derive(Clone, Debug)]
pub struct BaseSpec {
    pub base_dry_mass: f64,
    pub base_max_thrust: f64,
    pub base_exhaust_velocity: f64,
    pub base_fuel_capacity: f64,
    /// Base cargo hold (units). Effective capacity is DERIVED at the read site
    /// (`base + hulls * ShipyardCfg.hull_step_units`, pirates rung §6) — never
    /// stored. Default 5 everywhere keeps existing scenarios identical.
    pub base_cargo_capacity: u32,
}

/// Classical Kepler conic elements (radians for angles), solved once at init.
#[derive(Clone, Debug)]
pub struct OrbitalElements {
    pub a: f64,
    pub e: f64,
    pub i: f64,
    pub raan: f64,
    pub argp: f64,
    pub m0: f64,
}

#[derive(Clone, Debug)]
pub struct BodyInit {
    pub mass: f64,
    pub elements: OrbitalElements,
}

#[derive(Clone, Debug)]
pub struct CraftInit {
    pub spec: BaseSpec,
    pub pos: Vec3,
    pub vel: Vec3,
    pub fuel_mass: f64,
    /// Economic role minted at reset. `Pirate` rows get a `PirateState`
    /// (grubstake food, zero notoriety). Default `Idle` — existing scenarios
    /// identical (pirates rung Commit A; closes the no-way-to-mint-a-pirate gap).
    pub role: crate::stores::CraftRole,
    /// Scripted stages (ASSIGN, pirate brains, purchase policies) skip
    /// `!scripted` craft — the gym-exclusion flag, decided at config so the
    /// config golden moves once (spec §5). Default `true`.
    pub scripted: bool,
}

/// N substeps = pure fn of QUANTIZED total local acceleration magnitude (Task 7).
#[derive(Clone, Copy, Debug)]
pub struct SubstepCfg {
    /// Reference acceleration (AU/day²) for the substep schedule, NOT a log base:
    /// `substep_count` (Task 8) uses `n = 1 + floor(log2(max(1, mag/accel_ref)))`,
    /// clamped to `[1, max_substeps]`. At/below `accel_ref` → 1 substep; every
    /// doubling of `mag` above it adds one substep.
    pub accel_ref: f64,
    pub max_substeps: u32,
}

/// Class-2 run-level guidance POLICY (config-hashed). Dimensionless tunables a
/// caller may legitimately vary per run; folded into `config_hash` so a changed
/// value yields a different config whose recordings are correctly rejected at the
/// replay config-hash guard. (In a future fleet layer this migrates to a per-fleet
/// attribute; v1 holds it run-level — see spec §13.)
#[derive(Clone, Copy, Debug)]
pub struct GuidanceParams {
    /// Closing-speed cap as a FRACTION of full-tank Tsiolkovsky Δv
    /// (`exhaust_velocity * ln((dry + capacity)/dry)`). Replaces the absolute
    /// `V_CRUISE = 2e-3`. Default 0.25 (D5 derivation note).
    pub cruise_burn_fraction: f64,
    /// Brake-early safety margin (< 1). Exact carryover of the old `K_BRAKE`.
    pub k_brake: f64,
    /// Velocity-matched deadband (canonical AU/day). Exact carryover of `V_ERR_EPS`.
    pub v_err_eps: f64,
}

impl Default for GuidanceParams {
    fn default() -> Self {
        GuidanceParams { cruise_burn_fraction: 0.25, k_brake: 0.5, v_err_eps: 1.0e-4 }
    }
}

// --- Economy initial conditions (the first demand-driven loop, deterministic
// harness). All money is i64 microcredits. These are folded at the TAIL of
// config_hash, append-only, after `guidance`. References resolve at `World::reset`
// (an out-of-range *_index is a `ResetError`, validated before tick 0). ---

/// A station's initial market: which Body it rides, and its per-resource opening
/// integer stock + micro-price.
#[derive(Clone, Debug)]
pub struct StationInit {
    pub body_index: usize,
    pub initial_stock: [i64; crate::economy::N_RESOURCES],
    pub initial_price_micros: [i64; crate::economy::N_RESOURCES],
    /// First station capability mixin (spec §6): this station vends Hull/Escort
    /// upgrades (the Yard's storefront). Default `false`.
    pub sells_upgrades: bool,
}

/// A producer attached to a station, running `recipe` every `recipe.interval` ticks.
#[derive(Clone, Debug)]
pub struct ProducerInit {
    pub station_index: usize,
    pub recipe: crate::economy::Recipe,
}

/// A funded corporation (the contract originator). Non-spatial; `home_station_index`
/// is where it operates from.
#[derive(Clone, Debug)]
pub struct CorporationInit {
    pub treasury_micros: i64,
    pub home_station_index: usize,
}

/// A delivery contract seeded at config (status `Offered` at reset): move `qty` of
/// `resource` from `from_station_index` to `to_station_index` for `reward_micros`.
#[derive(Clone, Debug)]
pub struct ContractInit {
    pub corp_index: usize,
    pub resource: crate::economy::Resource,
    pub qty: u32,
    pub from_station_index: usize,
    pub to_station_index: usize,
    pub reward_micros: i64,
}

/// Stage-2 linear demand-deflation price curve + its reprice clock (config-hashed).
/// `price_micros(r) = base_micros[r] * (2000 - min(stock, cap[r]) * slope_milli / cap[r]) / 1000`,
/// clamped `>= 0`: at stock 0 → `base*2`; at stock `cap` → `base*(2 - slope)`. The
/// reprice cadence is part of the recorded schedule (invoked from `World::step`, NOT
/// lazily on read). Consumed by `update_prices` (Stage 2 — Tasks 19/20).
#[derive(Clone, Copy, Debug)]
pub struct PriceCfg {
    pub base_micros: [i64; crate::economy::N_RESOURCES],
    pub cap: [i64; crate::economy::N_RESOURCES],
    /// `k * 1000`, e.g. `1800 == 1.8`.
    pub slope_milli: i64,
    /// `update_prices` runs when `tick % reprice_interval == 0`. `1` == every tick.
    pub reprice_interval: u32,
}

impl Default for PriceCfg {
    fn default() -> Self {
        PriceCfg {
            base_micros: [0; crate::economy::N_RESOURCES],
            cap: [1; crate::economy::N_RESOURCES],
            slope_milli: 1800,
            reprice_interval: 1,
        }
    }
}

/// Scripted dispatch + hysteresis tunables (config-hashed). Drives the
/// repost/dispatch stage (Stage-1 — Task 17) and its Stage-2 stability refinements
/// (hysteresis deadband + staggered dispatch — Task 21). Inert defaults (no
/// auto-posting) so a fixture that does not configure dispatch is unaffected.
#[derive(Clone, Copy, Debug)]
pub struct DispatchCfg {
    /// Post a delivery contract for a route when the destination station's stock of
    /// the traded resource is at/below this (units). The demand trigger.
    pub demand_low: i64,
    /// Stop posting for that route once destination stock recovers to/above this.
    /// Hysteresis upper edge; set `> demand_low` to avoid chatter.
    pub demand_high: i64,
    /// Staggered dispatch period: an Idle hauler in dense row `s` may accept only on
    /// ticks where `tick % stagger_period == s % stagger_period`. `1` == no stagger.
    /// `0` disables scripted acceptance entirely (manual / RL `AcceptContract` only);
    /// REPOST is unaffected.
    pub stagger_period: u32,
    /// Microcredits the corp escrows per posted contract (Stage-1 fixed reward).
    pub contract_reward_micros: i64,
    /// Units of the traded resource moved per posted contract.
    pub contract_qty: u32,
}

impl Default for DispatchCfg {
    fn default() -> Self {
        DispatchCfg {
            demand_low: 0,
            demand_high: 0,
            stagger_period: 1,
            contract_reward_micros: 0,
            contract_qty: 0,
        }
    }
}

/// Scripted hauler purchase policy (pirates rung §6). Folded into config_hash
/// via `rank()` (stable discriminant, APPEND-ONLY).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyPolicy {
    /// No scripted purchases (default — trader gym and existing tests untouched).
    Off,
    /// Escort L1 -> Hull L1 -> Escort L2 -> Hull L2 (the spec §6 ladder).
    EscortFirst,
    /// Hull L1 -> Escort L1 -> Hull L2 -> Escort L2.
    HullFirst,
}
impl BuyPolicy {
    /// Stable discriminant for config-hash folding. APPEND-ONLY.
    pub fn rank(self) -> u8 {
        match self {
            BuyPolicy::Off => 0,
            BuyPolicy::EscortFirst => 1,
            BuyPolicy::HullFirst => 2,
        }
    }
}

/// Pirates-rung trophic knobs (spec §§2-7; pirates rung Commit A). Everything
/// the sweep lab varies is per-run config, folded at the TAIL of config_hash.
/// `engage_radius_au == 0.0` (the default) leaves the WHOLE trophic machinery
/// inert: no encounter envelope, no rolls, no Piracy-stream runtime draws.
/// These are DIAGNOSTIC/TUNING knobs, not gates (PDR-0006); the food-band
/// values (`food_per_unit_micros`, `upkeep_per_tick`, `grubstake_micros`)
/// are deliberately 0 by default — they are console-calibrated from the P0
/// measured `laden_trips_per_window` via the spec §4 formulas, never pinned here.
#[derive(Clone, Copy, Debug)]
pub struct TrophicCfg {
    /// Encounter envelope radius (AU). 0.0 = whole machinery inert (default).
    /// Live runs use ~5e-4 (5x ARRIVAL_RADIUS, spec §2).
    pub engage_radius_au: f64,
    /// Relative-speed gate of the envelope (AU/day). A Δv-advantaged hauler
    /// under way is out of envelope — flee-by-physics preserved.
    pub engage_speed: f64,
    /// P(Robbed | engaged) in milli (u < p_rob_milli on RngStream::Piracy).
    pub p_rob_milli: u32,
    /// Ransom = min(hauler wallet, this cap) — pure transfer, no new identity leg.
    pub ransom_cap_micros: i64,
    /// Food credited per robbed cargo unit. Console-calibrated from P0 (spec §4).
    pub food_per_unit_micros: i64,
    /// Metabolic drain while active. Console-calibrated from P0 (spec §4).
    pub upkeep_per_tick: i64,
    /// Food a pirate re-emerges with after starving (and is minted with at reset).
    pub grubstake_micros: i64,
    /// Lie-low duration after starvation.
    pub starve_lie_low_ticks: u64,
    /// Notoriety at/above which heat forces a lie-low.
    pub heat_threshold: u32,
    /// Notoriety accrued per successful rob.
    pub notoriety_per_rob: u32,
    /// Geometric notoriety decay factor (milli) applied every `decay_interval`.
    pub notoriety_decay_milli: u32,
    /// Ticks between notoriety decay applications.
    pub decay_interval: u64,
    /// Lie-low duration when heat trips.
    pub heat_lie_low_ticks: u64,
    /// Engage cooldown after a Robbed outcome ("digestion", ~one trip-time).
    pub rob_cooldown: u64,
    /// Engage cooldown after a DrivenOff outcome.
    pub driveoff_cooldown: u64,
    /// Base strength a Pirate role contributes (strength = escorts + this, §2/§6).
    pub pirate_base_strength: u8,
    /// Relocation reach (AU) — the PRIMARY locality lever (1-2 neighbors, never
    /// the whole map).
    pub pirate_max_reach_au: f64,
    /// Staggered relocation period (ticks; ~4 trips — sticky on prey timescale).
    pub relocate_period: u64,
    /// P(keep current lurk station) per relocation check, in milli.
    pub stay_milli: u32,
    /// Body index of the lie-low refuge (outermost in the game scenario).
    pub hideout_body_index: u32,
    /// Route-evidence read window (ticks before the reader's own info_tick).
    pub evidence_window: u64,
    /// ASSIGN scoring penalty per recent rob (milli), clamped at 900 (spec §7).
    pub evidence_penalty_milli: u32,
    /// Gate for evidence-scored scripted ASSIGN (default false: trader gym and
    /// all existing tests untouched).
    pub hauler_belief_scoring: bool,
    /// Scripted hauler purchase ladder.
    pub hauler_buy_policy: BuyPolicy,
}

impl Default for TrophicCfg {
    fn default() -> Self {
        TrophicCfg {
            engage_radius_au: 0.0, // inert: no encounter envelope at all
            engage_speed: 2.0e-3,
            p_rob_milli: 700,
            ransom_cap_micros: 2_000_000,
            food_per_unit_micros: 0, // console-calibrated from P0 (spec §4)
            upkeep_per_tick: 0,      // console-calibrated from P0 (spec §4)
            grubstake_micros: 0,     // console-calibrated from P0 (spec §4)
            starve_lie_low_ticks: 2000,
            heat_threshold: 250,
            notoriety_per_rob: 100,
            notoriety_decay_milli: 950,
            decay_interval: 200,
            heat_lie_low_ticks: 1500,
            rob_cooldown: 600,
            driveoff_cooldown: 200,
            pirate_base_strength: 1,
            pirate_max_reach_au: 0.6,
            relocate_period: 2500,
            stay_milli: 500,
            hideout_body_index: 0,
            evidence_window: 4000,
            evidence_penalty_milli: 150,
            hauler_belief_scoring: false,
            hauler_buy_policy: BuyPolicy::Off,
        }
    }
}

/// The Yard (pirates rung §6): one config-minted corporation receives all
/// upgrade payments (credits recycle corp -> escrow -> wallet -> upgrade -> corp).
/// Caps are STRUCTURAL: settle is a no-op at cap, keeping strength in [0, 3]
/// and the un-simulated wing small enough for a chronicle line.
#[derive(Clone, Copy, Debug)]
pub struct ShipyardCfg {
    /// Corporation (config index) credited with every upgrade payment.
    pub corp_index: u32,
    /// Price of hull L1 / L2 (micros).
    pub hull_price_micros: [i64; 2],
    /// Price of escort L1 / L2 (micros).
    pub escort_price_micros: [i64; 2],
    /// Cargo units added per hull (capacity = base + hulls * this).
    pub hull_step_units: u32,
    /// Structural cap on the hull count (fleet ledger, spec §6 owner caveat).
    pub max_hulls: u8,
    /// Structural cap on the escort count.
    pub max_escorts: u8,
    /// Scripted-hauler working-capital headroom: buy only when
    /// `credits >= price * buy_headroom_milli / 1000`.
    pub buy_headroom_milli: u32,
}

impl Default for ShipyardCfg {
    fn default() -> Self {
        ShipyardCfg {
            corp_index: 0,
            hull_price_micros: [8_000_000, 20_000_000],
            escort_price_micros: [5_000_000, 12_000_000],
            hull_step_units: 5,
            max_hulls: 2,
            max_escorts: 2,
            buy_headroom_milli: 1500,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    /// gym reset(seed) OVERWRITES this per episode.
    pub master_seed: u64,
    pub dt: Dt,
    /// epsilon in (r^2 + eps^2)^1.5 gravity softening.
    pub softening: f64,
    pub substep_cfg: SubstepCfg,
    /// ticks precomputed in the ephemeris window.
    pub ephemeris_window: u64,
    pub bodies: Vec<BodyInit>,
    pub craft: Vec<CraftInit>,
    /// Class-2 guidance policy (D4). Folded at the TAIL of config_hash.
    pub guidance: GuidanceParams,
    // Economy initial conditions (folded AFTER guidance, append-only). Empty vecs +
    // default cfgs leave the world an inert physics sim (no stations/producers/etc.).
    pub stations: Vec<StationInit>,
    pub producers: Vec<ProducerInit>,
    pub corporations: Vec<CorporationInit>,
    pub contracts: Vec<ContractInit>,
    pub price_cfg: PriceCfg,
    pub dispatch_cfg: DispatchCfg,
    // Pirates rung (Commit A, folded AFTER dispatch_cfg, append-only). Defaults
    // leave the trophic machinery inert (engage_radius_au == 0.0, BuyPolicy::Off).
    pub trophic: TrophicCfg,
    pub shipyard: ShipyardCfg,
}

/// The CONFIG hash (immutable initial conditions). NOT the per-tick state hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigHash(pub u64);

// FNV-1a 64-bit, folding one u64 at a time as 8 little-endian bytes. LOCAL to
// the CONFIG hash; the per-tick STATE hash (hash.rs) is a separate hasher with a
// different seed magic. The two hash spaces must never alias.
const CONFIG_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const CONFIG_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

struct ConfigFnv {
    state: u64,
}

impl ConfigFnv {
    fn new() -> Self {
        let mut h = ConfigFnv {
            state: CONFIG_FNV_OFFSET,
        };
        h.write_u64(0x434f_4e46_4947_5f31); // "CONFIG_1" tag, distinct space
        h
    }

    fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(CONFIG_FNV_PRIME);
        }
    }

    fn finish(self) -> u64 {
        self.state
    }
}

impl RunConfig {
    /// FNV-1a over master_seed, dt.bits(), softening.to_bits(), substep cfg, the
    /// ephemeris window, and every numeric field of every body/craft in a FIXED
    /// order (counts folded in first so two scenarios with different cardinality
    /// can never collide). DISTINCT from the per-tick state hash.
    ///
    /// CONFIG_FIELD_ORDER (config_hash fold order — append-only; re-pin the golden on change):
    ///   1. master_seed                       9.  per-body: mass + 6 elements
    ///   2. dt.bits()                         10. per-craft: 4 spec + pos[3] + vel[3] + fuel
    ///   3. softening.to_bits()               11. guidance.cruise_burn_fraction   (D4)
    ///   4. substep_cfg.accel_ref.to_bits()   12. guidance.k_brake                (D4)
    ///   5. substep_cfg.max_substeps          13. guidance.v_err_eps              (D4)
    ///   6. ephemeris_window
    ///   7. bodies.len()   8. craft.len()
    ///  14. economy counts: stations.len(), producers.len(), corporations.len(), contracts.len()
    ///  15. per-station: body_index, then per-resource (initial_stock, initial_price_micros)
    ///  16. per-producer: station_index, then recipe (input disc+payload, output disc+payload, interval)
    ///  17. per-corporation: treasury_micros, home_station_index
    ///  18. per-contract: corp_index, resource.index(), qty, from_station_index, to_station_index, reward_micros
    ///  19. price_cfg: slope_milli, reprice_interval, per-resource (base_micros, cap)
    ///  20. dispatch_cfg: demand_low, demand_high, stagger_period, contract_reward_micros, contract_qty
    ///  21. per-craft: role.rank(), scripted, spec.base_cargo_capacity   (pirates rung A)
    ///  22. per-station: sells_upgrades                                  (pirates rung A)
    ///  23. trophic: all fields in declaration order (f64 via to_bits, enums via rank)
    ///  24. shipyard: all fields in declaration order
    pub fn config_hash(&self) -> ConfigHash {
        // Exhaustive destructure: a NEW RunConfig field is a COMPILE ERROR here
        // until it is explicitly folded below (D10/M6 — closes the silent-omission
        // provenance hole). Field FOLD ORDER below is unchanged (value-preserving).
        let RunConfig {
            master_seed,
            dt,
            softening,
            substep_cfg,
            ephemeris_window,
            bodies,
            craft,
            guidance, // NEW (D4): destructure forces folding below
            stations,     // NEW (economy): destructure forces folding below
            producers,
            corporations,
            contracts,
            price_cfg,
            dispatch_cfg,
            trophic,  // NEW (pirates rung A): destructure forces folding below
            shipyard, // NEW (pirates rung A): destructure forces folding below
        } = self;
        let mut h = ConfigFnv::new();
        // Scalars in fixed order.
        h.write_u64(*master_seed);
        h.write_u64(dt.bits());
        h.write_u64(softening.to_bits());
        h.write_u64(substep_cfg.accel_ref.to_bits());
        h.write_u64(substep_cfg.max_substeps as u64);
        h.write_u64(*ephemeris_window);
        // Counts folded BEFORE field values so cardinality changes always move
        // the hash even if the new elements are all-zero.
        h.write_u64(bodies.len() as u64);
        h.write_u64(craft.len() as u64);
        // Bodies in declaration order; each field in fixed order.
        for b in bodies {
            h.write_u64(b.mass.to_bits());
            h.write_u64(b.elements.a.to_bits());
            h.write_u64(b.elements.e.to_bits());
            h.write_u64(b.elements.i.to_bits());
            h.write_u64(b.elements.raan.to_bits());
            h.write_u64(b.elements.argp.to_bits());
            h.write_u64(b.elements.m0.to_bits());
        }
        // Craft in declaration order; spec, pos, vel, fuel in fixed order.
        for c in craft {
            h.write_u64(c.spec.base_dry_mass.to_bits());
            h.write_u64(c.spec.base_max_thrust.to_bits());
            h.write_u64(c.spec.base_exhaust_velocity.to_bits());
            h.write_u64(c.spec.base_fuel_capacity.to_bits());
            let p = c.pos.to_bits();
            h.write_u64(p[0]);
            h.write_u64(p[1]);
            h.write_u64(p[2]);
            let v = c.vel.to_bits();
            h.write_u64(v[0]);
            h.write_u64(v[1]);
            h.write_u64(v[2]);
            h.write_u64(c.fuel_mass.to_bits());
        }
        // GUIDANCE (D4/D9) at the TAIL: the existing byte stream above stays
        // byte-identical; config_hash only EXTENDS. Order: cruise_burn_fraction,
        // k_brake, v_err_eps (CONFIG_FIELD_ORDER words below).
        h.write_u64(guidance.cruise_burn_fraction.to_bits());
        h.write_u64(guidance.k_brake.to_bits());
        h.write_u64(guidance.v_err_eps.to_bits());
        // ECONOMY (TAIL, append-only — CONFIG_FIELD_ORDER 14..=20). The byte stream
        // above stays byte-identical; this only EXTENDS it. Counts FIRST so a
        // cardinality change always moves the hash even when new elements are zero.
        // Integers fold directly; Resource/Recipe fold via discriminant + payload.
        h.write_u64(stations.len() as u64);
        h.write_u64(producers.len() as u64);
        h.write_u64(corporations.len() as u64);
        h.write_u64(contracts.len() as u64);
        for s in stations {
            h.write_u64(s.body_index as u64);
            for r in 0..crate::economy::N_RESOURCES {
                h.write_u64(s.initial_stock[r] as u64);
                h.write_u64(s.initial_price_micros[r] as u64);
            }
        }
        for p in producers {
            h.write_u64(p.station_index as u64);
            write_recipe(&mut h, &p.recipe);
        }
        for c in corporations {
            h.write_u64(c.treasury_micros as u64);
            h.write_u64(c.home_station_index as u64);
        }
        for k in contracts {
            h.write_u64(k.corp_index as u64);
            h.write_u64(k.resource.index() as u64);
            h.write_u64(k.qty as u64);
            h.write_u64(k.from_station_index as u64);
            h.write_u64(k.to_station_index as u64);
            h.write_u64(k.reward_micros as u64);
        }
        h.write_u64(price_cfg.slope_milli as u64);
        h.write_u64(price_cfg.reprice_interval as u64);
        for r in 0..crate::economy::N_RESOURCES {
            h.write_u64(price_cfg.base_micros[r] as u64);
            h.write_u64(price_cfg.cap[r] as u64);
        }
        h.write_u64(dispatch_cfg.demand_low as u64);
        h.write_u64(dispatch_cfg.demand_high as u64);
        h.write_u64(dispatch_cfg.stagger_period as u64);
        h.write_u64(dispatch_cfg.contract_reward_micros as u64);
        h.write_u64(dispatch_cfg.contract_qty as u64);
        // PIRATES RUNG A (TAIL, append-only — CONFIG_FIELD_ORDER 21..=24). The
        // byte stream above stays byte-identical; this only EXTENDS it. Counts
        // for craft/stations are already folded at words 7-8/14, so per-element
        // appends here cannot alias across cardinalities.
        for c in craft {
            h.write_u64(c.role.rank() as u64);
            h.write_u64(c.scripted as u64);
            h.write_u64(c.spec.base_cargo_capacity as u64);
        }
        for s in stations {
            h.write_u64(s.sells_upgrades as u64);
        }
        // Exhaustive destructures: a NEW TrophicCfg/ShipyardCfg field is a
        // COMPILE ERROR here until explicitly folded (the D10/M6 discipline).
        let TrophicCfg {
            engage_radius_au,
            engage_speed,
            p_rob_milli,
            ransom_cap_micros,
            food_per_unit_micros,
            upkeep_per_tick,
            grubstake_micros,
            starve_lie_low_ticks,
            heat_threshold,
            notoriety_per_rob,
            notoriety_decay_milli,
            decay_interval,
            heat_lie_low_ticks,
            rob_cooldown,
            driveoff_cooldown,
            pirate_base_strength,
            pirate_max_reach_au,
            relocate_period,
            stay_milli,
            hideout_body_index,
            evidence_window,
            evidence_penalty_milli,
            hauler_belief_scoring,
            hauler_buy_policy,
        } = trophic;
        h.write_u64(engage_radius_au.to_bits());
        h.write_u64(engage_speed.to_bits());
        h.write_u64(*p_rob_milli as u64);
        h.write_u64(*ransom_cap_micros as u64);
        h.write_u64(*food_per_unit_micros as u64);
        h.write_u64(*upkeep_per_tick as u64);
        h.write_u64(*grubstake_micros as u64);
        h.write_u64(*starve_lie_low_ticks);
        h.write_u64(*heat_threshold as u64);
        h.write_u64(*notoriety_per_rob as u64);
        h.write_u64(*notoriety_decay_milli as u64);
        h.write_u64(*decay_interval);
        h.write_u64(*heat_lie_low_ticks);
        h.write_u64(*rob_cooldown);
        h.write_u64(*driveoff_cooldown);
        h.write_u64(*pirate_base_strength as u64);
        h.write_u64(pirate_max_reach_au.to_bits());
        h.write_u64(*relocate_period);
        h.write_u64(*stay_milli as u64);
        h.write_u64(*hideout_body_index as u64);
        h.write_u64(*evidence_window);
        h.write_u64(*evidence_penalty_milli as u64);
        h.write_u64(*hauler_belief_scoring as u64);
        h.write_u64(hauler_buy_policy.rank() as u64);
        let ShipyardCfg {
            corp_index,
            hull_price_micros,
            escort_price_micros,
            hull_step_units,
            max_hulls,
            max_escorts,
            buy_headroom_milli,
        } = shipyard;
        h.write_u64(*corp_index as u64);
        h.write_u64(hull_price_micros[0] as u64);
        h.write_u64(hull_price_micros[1] as u64);
        h.write_u64(escort_price_micros[0] as u64);
        h.write_u64(escort_price_micros[1] as u64);
        h.write_u64(*hull_step_units as u64);
        h.write_u64(*max_hulls as u64);
        h.write_u64(*max_escorts as u64);
        h.write_u64(*buy_headroom_milli as u64);
        ConfigHash(h.finish())
    }
}

/// Fold a `Recipe` into the config hash: input (0/1 discriminant, then
/// `(resource.index(), qty)` if present), output (same), then `interval`.
/// Self-delimiting so a `None` input cannot alias a present one.
fn write_recipe(h: &mut ConfigFnv, r: &crate::economy::Recipe) {
    match r.input {
        None => h.write_u64(0),
        Some((res, qty)) => {
            h.write_u64(1);
            h.write_u64(res.index() as u64);
            h.write_u64(qty as u64);
        }
    }
    match r.output {
        None => h.write_u64(0),
        Some((res, qty)) => {
            h.write_u64(1);
            h.write_u64(res.index() as u64);
            h.write_u64(qty as u64);
        }
    }
    h.write_u64(r.interval as u64);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_CONFIG_HASH: u64 = 0x1798_b108_edae_5bb6; // RE-PINNED: +trophic/shipyard/role/scripted/sells_upgrades/base_cargo_capacity config surface (pirates rung A). Was 0xf4bc_85c3_7cb6_8a6b.

    fn sample() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.5),
            softening: 1e-4,
            substep_cfg: SubstepCfg {
                accel_ref: 2.0,
                max_substeps: 64,
            },
            ephemeris_window: 10_000,
            bodies: vec![BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 1.0,
                    e: 0.0167,
                    i: 0.0,
                    raan: 0.0,
                    argp: 1.0,
                    m0: 0.5,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1.0,
                    base_max_thrust: 0.01,
                    base_exhaust_velocity: 3.0,
                    base_fuel_capacity: 0.5,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 1.0, 0.0),
                fuel_mass: 0.5,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: PriceCfg::default(),
            dispatch_cfg: DispatchCfg::default(),
            trophic: TrophicCfg::default(),
            shipyard: ShipyardCfg::default(),
        }
    }

    #[test]
    fn config_hash_golden_anchor_is_stable() {
        // Drift-lock: the sample config's hash must not move under a refactor that
        // is meant to be value-preserving (e.g. the exhaustive-destructure change).
        // If a NEW field is added and folded, this value SHOULD change and be re-pinned
        // deliberately (mirrors the state_hash golden discipline).
        let got = sample().config_hash();
        assert_eq!(
            got,
            ConfigHash(GOLDEN_CONFIG_HASH),
            "config_hash drifted: re-pin only if intentional"
        );
    }

    #[test]
    fn same_config_same_hash() {
        assert_eq!(sample().config_hash(), sample().config_hash());
    }

    #[test]
    fn changing_seed_changes_hash() {
        let mut c = sample();
        c.master_seed = 43;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_dt_changes_hash() {
        let mut c = sample();
        c.dt = Dt::new(0.25);
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_softening_changes_hash() {
        let mut c = sample();
        c.softening = 2e-4;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_a_body_field_changes_hash() {
        let mut c = sample();
        c.bodies[0].elements.e = 0.02;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_a_craft_field_changes_hash() {
        let mut c = sample();
        c.craft[0].spec.base_max_thrust = 0.02;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_craft_position_changes_hash() {
        let mut c = sample();
        c.craft[0].pos = Vec3::new(1.5, 0.0, 0.0);
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_cruise_burn_fraction_changes_hash() {
        let mut c = sample();
        c.guidance.cruise_burn_fraction = 0.30;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_k_brake_changes_hash() {
        let mut c = sample();
        c.guidance.k_brake = 0.6;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_v_err_eps_changes_hash() {
        let mut c = sample();
        c.guidance.v_err_eps = 2.0e-4;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_substep_cfg_changes_hash() {
        let mut c = sample();
        c.substep_cfg.max_substeps = 128;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_ephemeris_window_changes_hash() {
        let mut c = sample();
        c.ephemeris_window = 20_000;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_an_economy_field_changes_config_hash() {
        // Adding a station moves the hash (cardinality folded first).
        let mut c = sample();
        c.stations.push(StationInit {
            body_index: 0,
            initial_stock: [10, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        });
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_a_producer_recipe_changes_config_hash() {
        // A producer + its recipe both participate (recipe folded discriminant-first).
        let mut c = sample();
        c.producers.push(ProducerInit {
            station_index: 0,
            recipe: crate::economy::Recipe {
                input: None,
                output: Some((crate::economy::Resource::Ore, 5)),
                interval: 1,
            },
        });
        let h_mine = c.config_hash();
        // Flip output Ore->Fuel: recipe payload must move the hash.
        c.producers[0].recipe.output = Some((crate::economy::Resource::Fuel, 5));
        assert_ne!(h_mine, c.config_hash());
        // ...and differs from the no-producer baseline.
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_price_cfg_changes_config_hash() {
        let mut c = sample();
        c.price_cfg.slope_milli = 1700;
        assert_ne!(sample().config_hash(), c.config_hash());
        let mut d = sample();
        d.price_cfg.reprice_interval = 4;
        assert_ne!(sample().config_hash(), d.config_hash());
    }

    #[test]
    fn changing_dispatch_cfg_changes_config_hash() {
        let mut c = sample();
        c.dispatch_cfg.demand_low = 3;
        assert_ne!(sample().config_hash(), c.config_hash());
        let mut d = sample();
        d.dispatch_cfg.stagger_period = 2;
        assert_ne!(sample().config_hash(), d.config_hash());
    }

    #[test]
    fn trophic_cfg_defaults_are_inert() {
        // Default TrophicCfg disables the whole predation machinery: an
        // engage radius of 0 means no encounter envelope ever contains a hauler.
        let t = TrophicCfg::default();
        assert_eq!(t.engage_radius_au, 0.0);
        // ...and a RunConfig built without pirates mints zero pirate rows.
        // (dt 0.01 passes the reset brakability guard; sample() is hash-only.)
        let mut c = sample();
        c.dt = Dt::new(0.01);
        let (w, _) = crate::world::World::reset(c).expect("resolvable config");
        assert!(
            w.ships.pirate.iter().all(Option::is_none),
            "no Pirate-role rows => no Some(pirate) rows"
        );
    }

    #[test]
    fn pirate_role_mints_pirate_state() {
        let mut c = sample();
        c.dt = Dt::new(0.01); // pass the reset brakability guard
        c.trophic.grubstake_micros = 1_500_000;
        c.craft[0].role = crate::stores::CraftRole::Pirate;
        let (w, _) = crate::world::World::reset(c).expect("resolvable config");
        let p = w.ships.pirate[0].expect("Pirate role mints PirateState at reset");
        assert_eq!(p.food_micros, 1_500_000, "minted with the configured grubstake");
        assert_eq!(p.notoriety, 0);
        assert_eq!(p.lie_low_until, crate::time::Tick(0));
        assert_eq!(w.ships.role[0], crate::stores::CraftRole::Pirate);
    }

    #[test]
    fn changing_trophic_or_shipyard_changes_config_hash() {
        let mut c = sample();
        c.trophic.engage_radius_au = 5.0e-4;
        assert_ne!(sample().config_hash(), c.config_hash());
        let mut d = sample();
        d.trophic.hauler_buy_policy = BuyPolicy::EscortFirst;
        assert_ne!(sample().config_hash(), d.config_hash());
        let mut e = sample();
        e.shipyard.hull_price_micros[1] = 21_000_000;
        assert_ne!(sample().config_hash(), e.config_hash());
    }

    #[test]
    fn changing_role_scripted_vendor_capacity_changes_config_hash() {
        let mut c = sample();
        c.craft[0].role = crate::stores::CraftRole::Pirate;
        assert_ne!(sample().config_hash(), c.config_hash());
        let mut d = sample();
        d.craft[0].scripted = false;
        assert_ne!(sample().config_hash(), d.config_hash());
        let mut e = sample();
        e.craft[0].spec.base_cargo_capacity = 10;
        assert_ne!(sample().config_hash(), e.config_hash());
        // sells_upgrades participates: same station with/without the vendor bit.
        let station = |sells: bool| StationInit {
            body_index: 0,
            initial_stock: [10, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: sells,
        };
        let mut f = sample();
        f.stations.push(station(false));
        let mut g = sample();
        g.stations.push(station(true));
        assert_ne!(f.config_hash(), g.config_hash());
    }

    #[test]
    #[ignore = "prints the golden constant for config_hash_golden_anchor_is_stable"]
    fn print_golden_config() {
        println!("GOLDEN_CONFIG_HASH=0x{:016x}", sample().config_hash().0);
    }

    #[test]
    fn changing_cardinality_changes_hash() {
        // An extra all-zero craft must still change the hash, because counts are
        // folded in BEFORE field values.
        let mut c = sample();
        c.craft.push(CraftInit {
            spec: BaseSpec {
                base_dry_mass: 0.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 0.0,
                base_fuel_capacity: 0.0,
                base_cargo_capacity: 5,
            },
            pos: Vec3::new(0.0, 0.0, 0.0),
            vel: Vec3::new(0.0, 0.0, 0.0),
            fuel_mass: 0.0,
            role: crate::stores::CraftRole::Idle,
            scripted: true,
        });
        assert_ne!(sample().config_hash(), c.config_hash());
    }
}
