//! `scenario_trophic` — the pirates-rung GAME scenario + sweep knob surface
//! (pirates rung 1, Commit F — spec §10, plan Task 6).
//!
//! FRAME (PDR-0006): this is the lab's standard world for the owner's
//! observe→steer→re-observe loop — a config FACTORY, not a gate. Everything
//! the console tuning phase varies rides `apply_knob` (the `--set knob=value`
//! surface of `examples/trophic_run.rs` and `python/analysis/sweep_trophic.py`).
//!
//! Scenario law (spec §10):
//! * 1 star at the proven 1e-3 calibration; 6 station bodies a ∈ 0.35–1.4 AU
//!   with SEED-DERIVED mean anomalies (anti-memorization, the trader-template
//!   precedent); hideout = the OUTERMOST body.
//! * 12 scripted haulers (ASSIGN on, stagger 16, belief scoring ON), spawned
//!   2-per-station so heterogeneity comes from POSITION, not taste scalars.
//! * 6-pirate pool sized for ~2 expected-active (expected-active ≤ stations − 2,
//!   the Saturated guard): the §4 food band gives a 1-window active runway and
//!   a 2-window starvation refuge, so an UNFED pirate duty-cycles at 1/3.
//! * 12 directed route templates across 3 tier corps (qty 5/10/15 at per-unit
//!   1.00×/1.15×/1.30× — value-concentration priced as juicier prey) + the
//!   Yard corp (receives all upgrade payments) + 2 vendor stations.
//! * `exhaust_velocity` ×10 vs the trader spec (fuel endurance, spec §6: tank
//!   ≈ 80k thrusting ticks; the reset guard reads dry-mass a_max, unaffected).

use crate::config::{
    BaseSpec, BodyInit, BuyPolicy, ContractInit, CorporationInit, CraftInit, DispatchCfg,
    GuidanceParams, MediaCfg, OrbitalElements, PriceCfg, ProducerInit, RefuelCfg, RunConfig,
    ShipyardCfg, StationInit, SubstepCfg, TrophicCfg,
};
use crate::economy::{Good, Recipe};
use crate::math::{G_CANONICAL, Vec3};
use crate::stores::CraftRole;
use crate::time::Dt;

/// Station-body semi-major axes (AU): 6 bodies evenly spread over the spec-§10
/// 0.35–1.4 AU band. Body index k+1 hosts station row k; body 6 (1.4 AU) is
/// the outermost — the hideout.
pub const STATION_ORBIT_AU: [f64; 6] = [0.35, 0.56, 0.77, 0.98, 1.19, 1.40];

/// Frontier station-body semi-major axes (AU) — the geometric band
/// `a_k = 0.35*r^k`, `r = (3.0/0.35)^(1/9)` (spec §2; endpoints exact, law
/// pinned by `frontier_orbit_band_is_the_pinned_geometric_law`). Body index
/// k+1 hosts station row k. Radial gaps run 0.094 -> 0.637 AU; the 8-9 gap
/// (0.637) exceeds `pirate_max_reach_au` 0.6 BY DESIGN — the one hop
/// haulers can fly and pirates can never walk (the never-opens seam).
pub const FRONTIER_ORBIT_AU: [f64; 10] = [
    0.35,
    0.444_365_796_521_264_1,
    0.564_174_174_622_793,
    0.716_284_875_665_669_2,
    0.909_407_140_889_456_3,
    1.154_598_367_209_910_7,
    1.465_897_208_878_237_4,
    1.861_127_373_832_788_5,
    2.362_918_136_859_245,
    3.0,
];

/// Frontier populations (spec §2): 2 haulers per station; 10 pirates is a
/// 2:1 predator:prey DESIGN CHOICE carried from the band, NOT a guard-derived
/// cap — the Saturated guard's integer floor admits up to 13 at n=10 (the
/// guard stays as ceiling documentation in `scenario_frontier_shape`).
pub const FRONTIER_NUM_HAULERS: usize = 20;
pub const FRONTIER_NUM_PIRATES: usize = 10;

/// Frontier HAULER exhaust velocity — CALIBRATED, not designed
/// (world-gets-big spec §4 step 3, OD-5b: k = 2.5 applied to the MEASURED
/// worst hauler-leg burn, never spec arithmetic). Instrument: 20-seed
/// `scenario_frontier` ensemble, `--set fuel_capacity_scale=100` (endurance
/// 400k full-throttle ticks >> the 100k-tick run: burn tail uncorrupted),
/// banked at
/// `docs/superpowers/posts/2026-06-12-world-gets-big-calibration/`.
/// Measured worst hauler-leg burn: 170 permille of the scaled tank (seed 9,
/// window close tick 86000) = 170e-10 fuel mass. Bake:
/// `v_e = 2.5 * 170e-10 / 1.0e-9 * 1.0 = 42.5`. Was the analytic prior 1.0.
/// Pirates do NOT use this const — they keep the band's 20.0 per-craft
/// (OD-6: the x10 endurance spec, no taste scalar).
pub const FRONTIER_HAULER_EXHAUST_VELOCITY: f64 = 42.5;

/// Haven station row (spec §3, OD-3): the dark port at the SEAM — hosted by
/// body 7 (1.4660 AU), a vendor (the pirate escort settle path requires a
/// vendor at the hideout dock), hosting NO producer and NO contract endpoint.
pub const FRONTIER_HAVEN_STATION: usize = 6;

/// Partitioned tier loops (spec §3, OD-2 — the self-averaging fix):
/// `(source_a, source_b, dest, fuel_sink)` station rows per tier. Dests and
/// sinks are per-tier disjoint (independent Schmitt triggers); every loop
/// touches a vendor (the vendor sits at the dest); the tier-2 return (9->8)
/// rides the never-walkable 8-9 gap.
pub const FRONTIER_TIER_WIRING: [(usize, usize, usize, usize); 3] =
    [(0, 1, 2, 3), (3, 4, 5, 4), (7, 8, 9, 8)];

/// Scripted haulers (2 per station body).
pub const NUM_HAULERS: usize = 12;

/// Pirate pool (expected-active ≈ 2 of these under the §4 food band).
pub const NUM_PIRATES: usize = 6;

/// Per-unit base reward (micros) — the P0/trader convention (qty-5 ⇒ 1.0M).
pub const PER_UNIT_BASE_MICROS: i64 = 200_000;

/// Tier table: (qty, per-unit multiplier in milli). Retail / bulk / heavy —
/// the qty-5 retail floor is load-bearing (the anti-extinction guarantee on
/// the capacity dimension, spec §6).
pub const TIERS: [(u32, i64); 3] = [(5, 1000), (10, 1150), (15, 1300)];

/// SplitMix64-style finalizer over `(seed, k)` — config-input derivation only,
/// never world RNG (the trader template's dependency-free mix).
fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Top-53-bits uniform [0, 1) construction (bit-exact, reproducible).
fn u64_to_unit_f64(x: u64) -> f64 {
    ((x >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}

/// Build the pirates-rung game scenario for one master seed (spec §10).
/// Pure config: same seed ⇒ identical RunConfig (and config_hash); body mean
/// anomalies and therefore all spawn geometry are seed-derived.
pub fn scenario_trophic(seed: u64) -> RunConfig {
    const STAR_MASS: f64 = 1.0e-3;
    const BODY_MASS: f64 = 1.0e-12;

    // --- bodies: star + 6 station bodies, seed-derived phases --------------
    let mut bodies = vec![BodyInit {
        mass: STAR_MASS,
        elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
    }];
    for (k, &a) in STATION_ORBIT_AU.iter().enumerate() {
        let m0 = u64_to_unit_f64(mix(seed, (k + 1) as u64)) * std::f64::consts::TAU;
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    // --- craft: 12 haulers (2 per station) + 6-pirate pool ------------------
    // exhaust_velocity ×10 vs the trader spec's 2.0 (fuel endurance, spec §6).
    let spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 20.0,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    // Co-orbit spawn at a body: position on the body's seeded phase, circular
    // velocity (the trader-template spawn math).
    let co_orbit = |body_index: usize| -> (Vec3, Vec3) {
        let el = &bodies[body_index].elements;
        let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
        let v_circ = (mu / el.a).sqrt();
        let pos = Vec3::new(el.a * el.m0.cos(), el.a * el.m0.sin(), 0.0);
        let vel = Vec3::new(-v_circ * el.m0.sin(), v_circ * el.m0.cos(), 0.0);
        (pos, vel)
    };
    let mut craft = Vec::with_capacity(NUM_HAULERS + NUM_PIRATES);
    for k in 0..NUM_HAULERS {
        let (pos, vel) = co_orbit(1 + (k % STATION_ORBIT_AU.len()));
        craft.push(CraftInit {
            spec: spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Idle,
            scripted: true,
        });
    }
    for _ in 0..NUM_PIRATES {
        // Pirates start co-orbiting the hideout (outermost body); the reset
        // Piracy draw scatters their initial lurks (spec §5).
        let (pos, vel) = co_orbit(STATION_ORBIT_AU.len());
        craft.push(CraftInit {
            spec: spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Pirate,
            scripted: true,
        });
    }

    // --- stations: 0-2 = Ore sources + tier fuel sinks; 3-5 = per-tier Ore
    // destinations (refiners). Vendors at 3 (the retail hub, where early
    // haulers idle after delivery) and 5 (the hideout's station — the pirate
    // haven, "a station with the right column set", where pirates shop while
    // lying low; the resolve_purchases settle requires a vendor at the dock).
    //
    // DEVIATION from the plan's per-tier demand bands (10/20, 5/15, 0/10):
    // `DispatchCfg` carries ONE global band and is config-golden-frozen, so
    // the per-tier Schmitt offsets are realized as per-tier INITIAL destination
    // stocks (18/14/10 against the global 10/20 band) + per-tier destination
    // stations — same interleaved first-burst structure, zero config-surface
    // change.
    let stock = |ore: i64, fuel: i64| -> Vec<i64> {
        let mut s = vec![0i64; crate::economy::N_GOODS_V1];
        s[Good::ORE.index()]  = ore;
        s[Good::FUEL.index()] = fuel;
        s
    };
    let stations = vec![
        StationInit { body_index: 1, initial_stock: stock(40, 18), initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        StationInit { body_index: 2, initial_stock: stock(40, 14), initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        StationInit { body_index: 3, initial_stock: stock(40, 10), initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        StationInit { body_index: 4, initial_stock: stock(18, 0), initial_price_micros: vec![0i64, 0i64], sells_upgrades: true },
        StationInit { body_index: 5, initial_stock: stock(14, 0), initial_price_micros: vec![0i64, 0i64], sells_upgrades: false },
        StationInit { body_index: 6, initial_stock: stock(10, 0), initial_price_micros: vec![0i64, 0i64], sells_upgrades: true },
    ];
    let producers = vec![
        // Ore miners at the sources.
        ProducerInit { station_index: 0, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 2, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        // Refiners at the tier destinations (Ore -> Fuel): the Ore demand sinks.
        ProducerInit { station_index: 3, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        ProducerInit { station_index: 5, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        // Fuel sinks back at the sources (per-tier return-leg demand).
        ProducerInit { station_index: 0, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 2, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
    ];

    // --- corps: 3 tier corps + the Yard (corp 3, receives upgrade payments).
    let corporations = vec![
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 3 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 4 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 5 },
        CorporationInit { treasury_micros: 0, home_station_index: 3 }, // the Yard
    ];

    // --- 12 directed route templates: per tier, 3 Ore legs out to the tier's
    // own destination + 1 Fuel return leg to the tier's own source sink.
    // reward = qty × per-unit base × tier multiplier (value-concentration
    // priced: a bigger lot is juicier prey, spec §6).
    let mut contracts = Vec::with_capacity(12);
    for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
        let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
        let dest = 3 + tier; // per-tier Ore destination (independent trigger)
        for from in 0..3usize {
            contracts.push(ContractInit {
                corp_index: tier,
                resource: Good::ORE,
                qty,
                from_station_index: from,
                to_station_index: dest,
                reward_micros: reward,
            });
        }
        contracts.push(ContractInit {
            corp_index: tier,
            resource: Good::FUEL,
            qty,
            from_station_index: dest,
            to_station_index: tier, // per-tier Fuel sink (independent trigger)
            reward_micros: reward,
        });
    }

    // --- console-session-2 band (2026-06-11, owner-judged): the original
    // §4-calibrated band (upkeep 25 / grubstake 50k / ransom 2cr) starved
    // pirates into duty-cycle loops and let the hauler escort ladder end the
    // war by t~10k (PermanentPeace). The walked band keeps every test seed's
    // predation alive through the final third: a rob buys ~2 windows of
    // life, a fresh grubstake ~1.4, and a 6cr ransom funds the pirate
    // counter-rung (escort L1 = 5cr) off ~1 good score.
    let trophic = TrophicCfg {
        engage_radius_au: 5.0e-4, // LIVE (5× ARRIVAL_RADIUS, spec §2)
        upkeep_per_tick: 12,
        food_per_unit_micros: 10_000,
        grubstake_micros: 100_000,
        ransom_cap_micros: 6_000_000,
        starve_lie_low_ticks: 4_000,
        hideout_body_index: 6, // outermost body (1.4 AU)
        pirate_max_reach_au: 0.6, // EXPLICIT (WGB §6) — was silent ..default(); unchanged
        hauler_belief_scoring: true,
        hauler_buy_policy: BuyPolicy::EscortFirst,
        ..TrophicCfg::default()
    };

    RunConfig {
        master_seed: seed,
        dt: Dt::new(0.25),
        softening: 1.0e-4,
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        ephemeris_window: 100_000,
        bodies,
        craft,
        guidance: GuidanceParams::default(),
        stations,
        producers,
        corporations,
        contracts,
        price_cfg: PriceCfg::default(),
        dispatch_cfg: DispatchCfg {
            demand_low: 10,
            demand_high: 20,
            stagger_period: 16,
            contract_reward_micros: 0,
            contract_qty: 0,
        },
        trophic,
        shipyard: ShipyardCfg { corp_index: 3, ..ShipyardCfg::default() },
        media: MediaCfg::default(),
        refuel: crate::config::RefuelCfg::default(),
        goods: crate::config::GoodsCfg::default(),
    }
}

/// Build the world-gets-big frontier scenario for one master seed (WGB spec
/// §2-§3): 10 stations on the geometric 0.35->3.0 AU band, partitioned tier
/// loops (core/mid/frontier), the dark seam haven, per-class craft specs.
/// Pure config: same seed => identical RunConfig (and config_hash); body mean
/// anomalies and all spawn geometry are seed-derived (the same `mix`).
///
/// A NEW world sharing the band's economic constants (GEO-C3): all cross-map
/// reads are rate-normalized distribution-vs-distribution, never same-seed
/// paired deltas.
pub fn scenario_frontier(seed: u64) -> RunConfig {
    const STAR_MASS: f64 = 1.0e-3;
    const BODY_MASS: f64 = 1.0e-12;

    // --- bodies: star + 10 station bodies on the pinned band, seed-derived
    // phases via the existing mix (anti-memorization unchanged) -------------
    let mut bodies = vec![BodyInit {
        mass: STAR_MASS,
        elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
    }];
    for (k, &a) in FRONTIER_ORBIT_AU.iter().enumerate() {
        let m0 = u64_to_unit_f64(mix(seed, (k + 1) as u64)) * std::f64::consts::TAU;
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    // --- craft: per-CLASS specs (spec §4/§6, OD-6) --------------------------
    // Haulers: v_e = the calibrated frontier constant above; tank 1e-9 =
    // 100x the re-baked eps — the FuelEmpty edge is LIVE.
    let hauler_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: FRONTIER_HAULER_EXHAUST_VELOCITY,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    // Pirates: the band's x10 endurance spec (~80k thrusting ticks — pirates
    // cannot strand this rung; the unification trigger is W11).
    let pirate_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 20.0,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    let co_orbit = |body_index: usize| -> (Vec3, Vec3) {
        let el = &bodies[body_index].elements;
        let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
        let v_circ = (mu / el.a).sqrt();
        let pos = Vec3::new(el.a * el.m0.cos(), el.a * el.m0.sin(), 0.0);
        let vel = Vec3::new(-v_circ * el.m0.sin(), v_circ * el.m0.cos(), 0.0);
        (pos, vel)
    };
    let mut craft = Vec::with_capacity(FRONTIER_NUM_HAULERS + FRONTIER_NUM_PIRATES);
    for k in 0..FRONTIER_NUM_HAULERS {
        let (pos, vel) = co_orbit(1 + (k % FRONTIER_ORBIT_AU.len()));
        craft.push(CraftInit {
            spec: hauler_spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Idle,
            scripted: true,
        });
    }
    for _ in 0..FRONTIER_NUM_PIRATES {
        // Pirates start co-orbiting the haven body (the seam); the reset
        // Piracy draw scatters their initial lurks.
        let (pos, vel) = co_orbit(1 + FRONTIER_HAVEN_STATION);
        craft.push(CraftInit {
            spec: pirate_spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Pirate,
            scripted: true,
        });
    }

    // --- stations: partitioned tier loops (spec §3, FRONTIER_TIER_WIRING) --
    // Vendors at the three tier dests (2/5/9: every loop touches a vendor)
    // and the haven (6). Schmitt stagger carried as per-tier INITIAL stocks
    // (18/14/10 dest Ore + 18/14/10 sink Fuel) against the ONE global 10/20
    // band — the trophic DEVIATION comment applies unchanged.
    let stock = |ore: i64, fuel: i64| -> Vec<i64> {
        let mut s = vec![0i64; crate::economy::N_GOODS_V1];
        s[Good::ORE.index()]  = ore;
        s[Good::FUEL.index()] = fuel;
        s
    };
    // Demand-deflation curve seed (spec §5): the SAME integer curve
    // update_prices walks — price = base*(2000 - min(stock,cap)*slope/cap)/1000
    // at base 5_000 / cap 40 / slope 1800 => dry 10_000, full 1_000.
    let fuel_price = |fuel_stock: i64| -> i64 {
        let s = fuel_stock.clamp(0, 40);
        (5_000 * (2000 - s * 1800 / 40) / 1000).max(0)
    };
    let station = |body_index: usize, ore: i64, fuel: i64, vendor: bool| StationInit {
        body_index,
        initial_stock: stock(ore, fuel),
        initial_price_micros: vec![0i64, fuel_price(fuel)],
        sells_upgrades: vendor,
    };
    let stations = vec![
        // Tier-0 core: sources 0-1 -> dest 2 (vendor); Fuel sink at 3.
        station(1, 40, 0, false),
        station(2, 40, 0, false),
        station(3, 18, 0, true),
        // Tier-1 mid: sources 3-4 -> dest 5 (vendor); Fuel sink at 4. Row 3
        // doubles as the tier-0 Fuel sink (18), row 4 as tier-1's own (14).
        station(4, 40, 18, false),
        station(5, 40, 14, false),
        station(6, 14, 0, true),
        // The haven (row 6, body 7): the dark port at the seam — vendor,
        // NO producer, NO contract endpoint (spec §3).
        station(7, 0, 0, true),
        // Tier-2 frontier: sources 7-8 -> dest 9 (vendor); Fuel sink at 8
        // (10). The 9->8 return rides the never-walkable 8-9 gap.
        station(8, 40, 0, false),
        station(9, 40, 10, false),
        station(10, 10, 0, true),
    ];
    let producers = vec![
        // Ore miners at the six tier sources.
        ProducerInit { station_index: 0, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 3, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 7, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        ProducerInit { station_index: 8, recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 } },
        // Refiners at the three tier dests: the Ore demand sinks and the
        // propellant supply geography.
        ProducerInit { station_index: 2, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        ProducerInit { station_index: 5, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        ProducerInit { station_index: 9, recipe: Recipe { input: Some((Good::ORE, 5)), output: Some((Good::FUEL, 5)), interval: 60 } },
        // Fuel sinks at the per-tier return-leg destinations.
        ProducerInit { station_index: 3, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 8, recipe: Recipe { input: Some((Good::FUEL, 5)), output: None, interval: 80 } },
    ];

    // --- corps: 3 tier corps + the Yard (3, upgrade payments) + the Port
    // (4, propellant revenue — armed by RefuelCfg.corp_index in task 2.4).
    let corporations = vec![
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 2 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 5 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 9 },
        CorporationInit { treasury_micros: 0, home_station_index: 2 },
        CorporationInit { treasury_micros: 0, home_station_index: 2 },
    ];

    // --- 9 directed route templates: per tier, 2 Ore legs src->dest + 1 Fuel
    // return dest->sink (rewards 1.0M / 2.3M / 3.9M via the tier table).
    let mut contracts = Vec::with_capacity(9);
    for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
        let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
        let (src_a, src_b, dest, sink) = FRONTIER_TIER_WIRING[tier];
        for from in [src_a, src_b] {
            contracts.push(ContractInit {
                corp_index: tier,
                resource: Good::ORE,
                qty,
                from_station_index: from,
                to_station_index: dest,
                reward_micros: reward,
            });
        }
        contracts.push(ContractInit {
            corp_index: tier,
            resource: Good::FUEL,
            qty,
            from_station_index: dest,
            to_station_index: sink,
            reward_micros: reward,
        });
    }

    // --- the band's trophic constants as the STARTING WALK (spec §3): food
    // 10k->15k (dock-exposure dilution; identities still pass), everything
    // else carried and re-walked at the console — never "same band".
    let trophic = TrophicCfg {
        engage_radius_au: 5.0e-4,
        upkeep_per_tick: 12,
        food_per_unit_micros: 15_000,
        grubstake_micros: 100_000,
        ransom_cap_micros: 6_000_000,
        starve_lie_low_ticks: 4_000,
        hideout_body_index: 7,
        pirate_max_reach_au: 0.6,
        hauler_belief_scoring: true,
        hauler_buy_policy: BuyPolicy::EscortFirst,
        ..TrophicCfg::default()
    };

    RunConfig {
        master_seed: seed,
        dt: Dt::new(0.25),
        softening: 1.0e-4,
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        ephemeris_window: 120_000,
        bodies,
        craft,
        guidance: GuidanceParams::default(),
        stations,
        producers,
        corporations,
        contracts,
        price_cfg: PriceCfg {
            // The first live price (OD-4): Fuel only — full (stock >= 40)
            // 1_000 -> dry 10_000 micros/unit; a full fill ~= the grubstake
            // ~= 10% of a tier-1 reward. cap[Ore] == 0 = the structural-off
            // switch (update_prices skips the row).
            base_micros: vec![0i64, 5_000i64],
            cap: vec![0i64, 40i64],
            slope_milli: 1800,
            reprice_interval: 1,
        },
        dispatch_cfg: DispatchCfg {
            demand_low: 10,
            demand_high: 20,
            stagger_period: 16,
            contract_reward_micros: 0,
            contract_qty: 0,
        },
        trophic,
        shipyard: ShipyardCfg { corp_index: 3, ..ShipyardCfg::default() },
        media: MediaCfg::default(),
        // Refuel LIVE (spec §5): 20 lots/tank (~1 lot core leg, ~3-4
        // frontier leg); revenue -> the Port corp (index 4, treasury 0) —
        // generator AND consumer land in one rung (the OD-5b two-sided law).
        refuel: RefuelCfg { lot_mass: 5.0e-11, corp_index: 4 },
        goods: crate::config::GoodsCfg::default(),
    }
}

/// Bazaar scenario — the goods-rung world (spec §3). MINIMAL A2.4 stub: the
/// full own-trade bazaar factory lands in A5.2. For now this scaffolds the
/// Food good (Good(2)) and its consumption sinks on top of the frontier
/// geometry so the rung-A behaviour can be exercised incrementally.
///
/// The stub starts from `scenario_frontier` (the 10-station tier band, rows
/// 3/4/8 already host Fuel sinks), promotes the goods table to three goods
/// (Ore/Fuel/Food), resizes every per-good station array to length 3, and adds
/// three input-only Food consumption producers (qty 5, interval 80) at the
/// tier-sink rows. Food has NO producer here yet (A5.2 supplies WA1); the
/// sinks keep the Food spread re-opening once supply exists (recommended cut
/// §1.3).
pub fn scenario_bazaar(seed: u64) -> RunConfig {
    use crate::config::{GoodSpec, GoodsCfg};
    let mut cfg = scenario_frontier(seed);

    // Promote the goods table to three goods: Ore(0), Fuel(1), Food(2).
    cfg.goods = GoodsCfg {
        goods: vec![
            GoodSpec { name: "Ore".to_string(), unit_mass_milli: 1000 },
            GoodSpec { name: "Fuel".to_string(), unit_mass_milli: 1000 },
            GoodSpec { name: "Food".to_string(), unit_mass_milli: 1000 },
        ],
    };
    // Every per-good station array must be sized to the new n_goods (3). The
    // frontier stations seed only Ore/Fuel; Food starts empty everywhere.
    let n_goods = cfg.goods.goods.len();
    for s in cfg.stations.iter_mut() {
        s.initial_stock.resize(n_goods, 0);
        s.initial_price_micros.resize(n_goods, 0);
    }
    // The price curve cfg is also per-good; extend with a structural-off Food
    // row (base 0 / cap 0 -> update_prices skips it until A5.2 prices Food).
    cfg.price_cfg.base_micros.resize(n_goods, 0);
    cfg.price_cfg.cap.resize(n_goods, 0);

    // Food consumption sinks (input-only, fuel-sink shape): qty 5, interval 80
    // at station rows 3, 4, 8 (the same tier-sink geometry as Fuel sinks in
    // scenario_frontier). Keeps the Food spread open after deliveries arrive.
    for sink_row in [3usize, 4, 8] {
        cfg.producers.push(ProducerInit {
            station_index: sink_row,
            recipe: Recipe {
                input: Some((Good::FOOD, 5)),
                output: None,
                interval: 80,
            },
        });
    }
    cfg
}

/// Apply one `--set knob=value` override to a built config — the sweep lab's
/// whole tuning surface (spec §9 matrix knobs; PDR-0006: tuning levers, not
/// gates). Unknown knobs and malformed values are ERRORS (a silent typo in a
/// sweep grid would poison a whole matrix read).
pub fn apply_knob(cfg: &mut RunConfig, name: &str, value: &str) -> Result<(), String> {
    fn p<T: std::str::FromStr>(name: &str, value: &str) -> Result<T, String>
    where
        T::Err: std::fmt::Display,
    {
        value.parse::<T>().map_err(|e| format!("--set {name}={value}: {e}"))
    }
    let t = &mut cfg.trophic;
    let y = &mut cfg.shipyard;
    let m = &mut cfg.media;
    match name {
        // TrophicCfg (spec §§2-7 knobs).
        "engage_radius_au" => t.engage_radius_au = p(name, value)?,
        "engage_speed" => t.engage_speed = p(name, value)?,
        "p_rob_milli" => t.p_rob_milli = p(name, value)?,
        "ransom_cap_micros" => t.ransom_cap_micros = p(name, value)?,
        "food_per_unit_micros" => t.food_per_unit_micros = p(name, value)?,
        "upkeep_per_tick" => t.upkeep_per_tick = p(name, value)?,
        "grubstake_micros" => t.grubstake_micros = p(name, value)?,
        "starve_lie_low_ticks" => t.starve_lie_low_ticks = p(name, value)?,
        "heat_threshold" => t.heat_threshold = p(name, value)?,
        "notoriety_per_rob" => t.notoriety_per_rob = p(name, value)?,
        "notoriety_decay_milli" => t.notoriety_decay_milli = p(name, value)?,
        "decay_interval" => t.decay_interval = p(name, value)?,
        "heat_lie_low_ticks" => t.heat_lie_low_ticks = p(name, value)?,
        "rob_cooldown" => t.rob_cooldown = p(name, value)?,
        "driveoff_cooldown" => t.driveoff_cooldown = p(name, value)?,
        "pirate_base_strength" => t.pirate_base_strength = p(name, value)?,
        "pirate_max_reach_au" => t.pirate_max_reach_au = p(name, value)?,
        "relocate_period" => t.relocate_period = p(name, value)?,
        "stay_milli" => t.stay_milli = p(name, value)?,
        "hideout_body_index" => t.hideout_body_index = p(name, value)?,
        "evidence_window" => t.evidence_window = p(name, value)?,
        "evidence_penalty_milli" => t.evidence_penalty_milli = p(name, value)?,
        "hauler_belief_scoring" => t.hauler_belief_scoring = p(name, value)?,
        "hauler_buy_policy" => {
            t.hauler_buy_policy = match value {
                "Off" => BuyPolicy::Off,
                "EscortFirst" => BuyPolicy::EscortFirst,
                "HullFirst" => BuyPolicy::HullFirst,
                other => return Err(format!("--set {name}={other}: expected Off|EscortFirst|HullFirst")),
            }
        }
        // ShipyardCfg (arms-race knobs).
        "hull_price_1" => y.hull_price_micros[0] = p(name, value)?,
        "hull_price_2" => y.hull_price_micros[1] = p(name, value)?,
        "escort_price_1" => y.escort_price_micros[0] = p(name, value)?,
        "escort_price_2" => y.escort_price_micros[1] = p(name, value)?,
        "hull_step_units" => y.hull_step_units = p(name, value)?,
        "max_hulls" => y.max_hulls = p(name, value)?,
        "max_escorts" => y.max_escorts = p(name, value)?,
        "buy_headroom_milli" => y.buy_headroom_milli = p(name, value)?,
        // DispatchCfg (prey-flux knobs).
        "demand_low" => cfg.dispatch_cfg.demand_low = p(name, value)?,
        "demand_high" => cfg.dispatch_cfg.demand_high = p(name, value)?,
        "stagger_period" => cfg.dispatch_cfg.stagger_period = p(name, value)?,
        // MediaCfg (media rung cut 1, spec §11 knobs).
        "station_gossip_slots" => m.station_gossip_slots = p(name, value)?,
        "craft_gossip_slots" => m.craft_gossip_slots = p(name, value)?,
        "sig_floor_milli" => m.sig_floor_milli = p(name, value)?,
        "sig_divisor_micros" => m.sig_divisor_micros = p(name, value)?,
        "hop_loss_milli" => m.hop_loss_milli = p(name, value)?,
        "inflation_milli" => m.inflation_milli = p(name, value)?,
        "claimed_value_cap_micros" => m.claimed_value_cap_micros = p(name, value)?,
        "value_ticks_milli" => m.value_ticks_milli = p(name, value)?,
        "staleness_from_rob_tick" => m.staleness_from_rob_tick = p(name, value)?,
        // Craft-spec knobs (world-gets-big spec §4 — calibration levers).
        // Scales EVERY craft's tank and starting fuel together (full-tank
        // starts preserved; pirates' endurance ratio preserved). Zero,
        // negative, and non-finite values would poison a whole sweep grid, so
        // they are loud errors.
        "fuel_capacity_scale" => {
            let scale: f64 = p(name, value)?;
            if !(scale.is_finite() && scale > 0.0) {
                return Err(format!("--set {name}={value}: scale must be finite and > 0"));
            }
            for c in &mut cfg.craft {
                c.spec.base_fuel_capacity *= scale;
                c.fuel_mass *= scale;
            }
        }
        other => return Err(format!("--set {other}: unknown knob")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Command, EventKind, StateView};
    use crate::diagnostics::WINDOW_TICKS;
    use crate::time::Tick;
    use crate::world::World;

    #[test]
    fn scenario_bazaar_has_food_consumption_recipe() {
        // Food (Good(2)) must appear as input to at least one producer in the
        // bazaar scenario (the fuel-sink shape: input-only, no output). This
        // verifies WA1's supply is non-trivially demanded.
        use crate::economy::Good;
        let cfg = scenario_bazaar(7);
        let food_consumers = cfg
            .producers
            .iter()
            .filter(|p| matches!(p.recipe.input, Some((g, _)) if g == Good::FOOD))
            .filter(|p| p.recipe.output.is_none())
            .count();
        assert!(
            food_consumers >= 1,
            "scenario_bazaar must have at least one Food consumption sink (input-only recipe)"
        );
    }

    #[test]
    fn good_food_has_index_2() {
        // Good::FOOD must be Good(2) — the globally pinned index.
        use crate::economy::Good;
        assert_eq!(Good::FOOD.0, 2, "Food must be index 2 in the Good ordering");
    }

    #[test]
    fn scenario_trophic_shape() {
        let cfg = scenario_trophic(7);

        // 1 star + 6 station bodies, a ascending within the 0.35-1.4 AU band.
        assert_eq!(cfg.bodies.len(), 7, "star + 6 station bodies");
        assert_eq!(cfg.bodies[0].elements.a, 0.0, "central star");
        let axes: Vec<f64> = cfg.bodies[1..].iter().map(|b| b.elements.a).collect();
        for w in axes.windows(2) {
            assert!(w[0] < w[1], "station orbits ascending: {axes:?}");
        }
        assert!(axes.iter().all(|&a| (0.35..=1.4).contains(&a)), "a in 0.35-1.4: {axes:?}");

        // 6 stations on bodies 1..=6; exactly 2 vendors; the hideout's station
        // (outermost body) is one of them (pirates shop while lying low).
        assert_eq!(cfg.stations.len(), 6);
        let body_idx: Vec<usize> = cfg.stations.iter().map(|s| s.body_index).collect();
        assert_eq!(body_idx, vec![1, 2, 3, 4, 5, 6]);
        let vendors: Vec<usize> =
            (0..6).filter(|&s| cfg.stations[s].sells_upgrades).collect();
        assert_eq!(vendors.len(), 2, "2 vendor stations (spec §6)");
        assert!(
            cfg.stations.iter().any(|s| s.sells_upgrades
                && s.body_index as u32 == cfg.trophic.hideout_body_index),
            "the hideout station is a vendor (the pirate-haven settle path)"
        );

        // 12 scripted haulers + 6-pirate pool, all scripted, all x10 endurance.
        assert_eq!(cfg.craft.len(), NUM_HAULERS + NUM_PIRATES);
        let pirates = cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count();
        let haulers = cfg.craft.iter().filter(|c| c.role == CraftRole::Idle).count();
        assert_eq!(pirates, 6, "6-pirate pool");
        assert_eq!(haulers, 12, "12 haulers");
        assert!(cfg.craft.iter().all(|c| c.scripted), "all scripted (no gym craft)");
        assert!(
            cfg.craft.iter().all(|c| c.spec.base_exhaust_velocity == 20.0),
            "exhaust_velocity x10 vs the trader spec's 2.0 (fuel endurance, spec §6)"
        );

        // The Saturated guard (spec §10): expected-active <= stations - 2.
        // Unfed active runway = grubstake/upkeep ticks; the §4 duty cycle is
        // runway / (runway + starve refuge).
        let runway = cfg.trophic.grubstake_micros / cfg.trophic.upkeep_per_tick;
        let cycle = runway as u64 + cfg.trophic.starve_lie_low_ticks;
        let expected_active = pirates as u64 * runway as u64 / cycle;
        assert!(
            expected_active <= cfg.stations.len() as u64 - 2,
            "expected-active {expected_active} <= stations - 2"
        );

        // The console-session-2 band (2026-06-11): a qty-5 rob sustains AT
        // LEAST two windows of upkeep (the original one-window band starved
        // pirates into permanent duty-cycle loops); the grubstake covers more
        // than one window so a fresh pirate survives its first bad draw; the
        // ransom cap funds the pirate counter-rung (escort L1) off one score.
        assert!(
            5 * cfg.trophic.food_per_unit_micros
                >= 2 * cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "one qty-5 rob sustains >= 2 windows"
        );
        assert!(
            cfg.trophic.grubstake_micros > cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "grubstake outlasts one window"
        );
        assert!(
            cfg.trophic.ransom_cap_micros >= cfg.shipyard.escort_price_micros[0],
            "one capped ransom funds the pirate counter-rung"
        );

        // >= 12 directed routes across exactly 3 tier corps + the Yard corp;
        // tier pricing qty 5/10/15 at per-unit 1.00x/1.15x/1.30x.
        assert_eq!(cfg.corporations.len(), 4, "3 tier corps + the Yard");
        assert!(cfg.contracts.len() >= 12, "≥ 12 directed route templates");
        assert_eq!(cfg.shipyard.corp_index, 3, "the Yard receives upgrade payments");
        assert!(
            cfg.contracts.iter().all(|k| k.corp_index < 3),
            "the Yard posts no haulage routes"
        );
        for k in &cfg.contracts {
            let (qty, mult_milli) = TIERS[k.corp_index];
            assert_eq!(k.qty, qty, "tier {} lot size", k.corp_index);
            assert_eq!(
                k.reward_micros,
                qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000,
                "tier {} per-unit ladder", k.corp_index
            );
        }
        // Each tier's triggers are independent: per-tier destination stations.
        for tier in 0..3 {
            let dests: std::collections::BTreeSet<usize> = cfg
                .contracts
                .iter()
                .filter(|k| k.corp_index == tier)
                .map(|k| k.to_station_index)
                .collect();
            for other in 0..3 {
                if other == tier {
                    continue;
                }
                assert!(
                    cfg.contracts
                        .iter()
                        .filter(|k| k.corp_index == other)
                        .all(|k| !dests.contains(&k.to_station_index)),
                    "tiers {tier}/{other} share a destination (Schmitt triggers would couple)"
                );
            }
        }

        // ASSIGN on at stagger 16, belief scoring ON, ladder ON, machinery live.
        assert_eq!(cfg.dispatch_cfg.stagger_period, 16);
        assert_eq!(cfg.dispatch_cfg.demand_low, 10);
        assert_eq!(cfg.dispatch_cfg.demand_high, 20);
        assert!(cfg.trophic.hauler_belief_scoring, "belief scoring ON");
        assert_eq!(cfg.trophic.hauler_buy_policy, BuyPolicy::EscortFirst);
        assert!(cfg.trophic.engage_radius_au > 0.0, "trophic machinery LIVE");
        // Reach is EXPLICIT in the factory (WGB §6) — the 0.6 the band was
        // judged at, no longer a silent ..TrophicCfg::default() inheritance.
        assert_eq!(cfg.trophic.pirate_max_reach_au, 0.6);
        assert_eq!(
            cfg.refuel.lot_mass, 0.0,
            "the trophic-inertness gate: the refuel verb stays OFF on the band"
        );

        // Hideout = the OUTERMOST body.
        let outermost = 1 + axes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(i, _)| i)
            .unwrap();
        assert_eq!(cfg.trophic.hideout_body_index as usize, outermost);

        // Resolvable + brakable; reset mints the 6-pirate pool.
        let (w, _h) = World::reset(cfg).expect("scenario_trophic must resolve");
        assert_eq!(w.ships.pirate.iter().filter(|p| p.is_some()).count(), 6);
    }

    #[test]
    fn scenario_is_seed_derived_and_deterministic() {
        // Same seed -> identical config hash; different seed -> different
        // body phases AND a different config hash (anti-memorization).
        assert_eq!(
            scenario_trophic(7).config_hash(),
            scenario_trophic(7).config_hash()
        );
        let a = scenario_trophic(7);
        let b = scenario_trophic(8);
        assert_ne!(a.config_hash(), b.config_hash());
        assert!(
            a.bodies[1..]
                .iter()
                .zip(&b.bodies[1..])
                .any(|(x, y)| x.elements.m0 != y.elements.m0),
            "mean anomalies are seed-derived"
        );
    }

    #[test]
    fn apply_knob_overrides_and_rejects_unknown() {
        let mut cfg = scenario_trophic(7);
        // The positive-control pair (plan 6.3).
        apply_knob(&mut cfg, "pirate_max_reach_au", "999").expect("reach knob");
        apply_knob(&mut cfg, "stay_milli", "0").expect("stay knob");
        assert_eq!(cfg.trophic.pirate_max_reach_au, 999.0);
        assert_eq!(cfg.trophic.stay_milli, 0);
        // Typed parses across the families.
        apply_knob(&mut cfg, "escort_price_1", "4000000").expect("price knob");
        assert_eq!(cfg.shipyard.escort_price_micros[0], 4_000_000);
        apply_knob(&mut cfg, "hauler_belief_scoring", "false").expect("bool knob");
        assert!(!cfg.trophic.hauler_belief_scoring);
        apply_knob(&mut cfg, "hauler_buy_policy", "HullFirst").expect("enum knob");
        assert_eq!(cfg.trophic.hauler_buy_policy, BuyPolicy::HullFirst);
        apply_knob(&mut cfg, "demand_high", "25").expect("dispatch knob");
        assert_eq!(cfg.dispatch_cfg.demand_high, 25);
        // MediaCfg knobs (media rung cut 1, spec §11).
        apply_knob(&mut cfg, "station_gossip_slots", "16").expect("media slot knob");
        assert_eq!(cfg.media.station_gossip_slots, 16);
        apply_knob(&mut cfg, "hop_loss_milli", "200").expect("media hop knob");
        assert_eq!(cfg.media.hop_loss_milli, 200);
        // Errors are loud: unknown knob, malformed value, bad enum.
        assert!(apply_knob(&mut cfg, "warp_factor", "9").is_err());
        assert!(apply_knob(&mut cfg, "p_rob_milli", "many").is_err());
        assert!(apply_knob(&mut cfg, "hauler_buy_policy", "Maximal").is_err());
    }

    #[test]
    fn fuel_capacity_scale_knob_scales_every_tank() {
        // World-gets-big spec §4 step 3: the calibration ensemble's lever —
        // scales capacity AND starting fuel (full-tank starts preserved) so
        // endurance exceeds run length and the burn tail is uncorrupted.
        let mut cfg = scenario_trophic(7);
        let base: Vec<(f64, f64)> =
            cfg.craft.iter().map(|c| (c.spec.base_fuel_capacity, c.fuel_mass)).collect();
        apply_knob(&mut cfg, "fuel_capacity_scale", "100").expect("knob applies");
        for (c, (cap0, fuel0)) in cfg.craft.iter().zip(&base) {
            assert_eq!(c.spec.base_fuel_capacity, cap0 * 100.0, "capacity scaled");
            assert_eq!(c.fuel_mass, fuel0 * 100.0, "starting fuel scaled");
        }
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "0").is_err(), "zero is loud");
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "-1").is_err(), "negative is loud");
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "nan").is_err(), "NaN is loud");
    }

    #[test]
    fn frontier_orbit_band_is_the_pinned_geometric_law() {
        // Spec §2: a_k = 0.35*r^k, r = (3.0/0.35)^(1/9) — endpoints EXACT,
        // interior pinned to the recomputed law (never to rounded prose).
        let r = (3.0f64 / 0.35).powf(1.0 / 9.0);
        assert_eq!(FRONTIER_ORBIT_AU.len(), 10);
        assert_eq!(FRONTIER_ORBIT_AU[0], 0.35, "inner endpoint exact");
        assert_eq!(FRONTIER_ORBIT_AU[9], 3.0, "outer endpoint exact");
        for (k, &a) in FRONTIER_ORBIT_AU.iter().enumerate() {
            let law = 0.35 * r.powi(k as i32);
            assert!(
                (a - law).abs() <= 1.0e-12,
                "a_{k} = {a} deviates from the geometric law {law}"
            );
        }
        for w in FRONTIER_ORBIT_AU.windows(2) {
            assert!(w[0] < w[1], "ascending band: {FRONTIER_ORBIT_AU:?}");
        }
        // The designed seam (spec §2/§6): the 8-9 radial gap (0.637) exceeds
        // pirate_max_reach_au 0.6 — the one hop haulers can fly and pirates
        // can never walk. Recorded design law, not a run gate.
        let outer_gap = FRONTIER_ORBIT_AU[9] - FRONTIER_ORBIT_AU[8];
        assert!(
            outer_gap > 0.6,
            "outer gap {outer_gap} must exceed pirate reach 0.6 (never-opens seam)"
        );
    }

    #[test]
    fn scenario_frontier_shape() {
        let cfg = scenario_frontier(7);

        // 1 star + 10 station bodies riding the pinned band in order.
        assert_eq!(cfg.bodies.len(), 11, "star + 10 station bodies");
        assert_eq!(cfg.bodies[0].elements.a, 0.0, "central star");
        let axes: Vec<f64> = cfg.bodies[1..].iter().map(|b| b.elements.a).collect();
        assert_eq!(axes, FRONTIER_ORBIT_AU.to_vec(), "bodies ride FRONTIER_ORBIT_AU");

        // 10 stations; body k+1 hosts station row k (the trophic law, n=10).
        assert_eq!(cfg.stations.len(), 10);
        let body_idx: Vec<usize> = cfg.stations.iter().map(|s| s.body_index).collect();
        assert_eq!(body_idx, (1..=10).collect::<Vec<_>>());

        // Populations (spec §2): 20 haulers (2/station), 10 pirates — a 2:1
        // predator:prey DESIGN CHOICE, all scripted (no gym craft).
        assert_eq!(cfg.craft.len(), FRONTIER_NUM_HAULERS + FRONTIER_NUM_PIRATES);
        let pirates = cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count();
        let haulers = cfg.craft.iter().filter(|c| c.role == CraftRole::Idle).count();
        assert_eq!(haulers, 20, "20 haulers");
        assert_eq!(pirates, 10, "10-pirate pool");
        assert!(cfg.craft.iter().all(|c| c.scripted), "all scripted (no gym craft)");
        assert_eq!(haulers % cfg.stations.len(), 0, "haulers == 0 mod n (2/station)");

        // Per-CLASS craft specs (spec §4/§6, OD-6): haulers ride the NAMED
        // calibration-pending const; pirates keep the band's x10 endurance.
        for c in &cfg.craft {
            match c.role {
                CraftRole::Pirate => assert_eq!(
                    c.spec.base_exhaust_velocity, 20.0,
                    "pirate v_e 20 per-craft (OD-6; cannot strand this rung)"
                ),
                _ => assert_eq!(
                    c.spec.base_exhaust_velocity, FRONTIER_HAULER_EXHAUST_VELOCITY,
                    "hauler v_e = the named analytic prior (calibration bakes it)"
                ),
            }
            assert_eq!(c.spec.base_fuel_capacity, 1.0e-9, "tank = 100x re-baked eps");
            assert_eq!(c.fuel_mass, 1.0e-9, "spawn with a full tank");
        }

        // The Saturated guard kept as CEILING DOCUMENTATION (spec §2): 10
        // pirates is a predator:prey choice, not the guard's integer floor.
        let runway = cfg.trophic.grubstake_micros / cfg.trophic.upkeep_per_tick;
        let cycle = runway as u64 + cfg.trophic.starve_lie_low_ticks;
        let expected_active = pirates as u64 * runway as u64 / cycle;
        assert!(
            expected_active <= cfg.stations.len() as u64 - 2,
            "expected-active {expected_active} <= stations - 2"
        );

        // Food band re-walk STARTS at 15k (spec §3, OD-2: dock-exposure
        // dilution); the band identities still pass at the new value.
        assert_eq!(cfg.trophic.food_per_unit_micros, 15_000);
        assert!(
            5 * cfg.trophic.food_per_unit_micros
                >= 2 * cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "one qty-5 rob sustains >= 2 windows"
        );
        assert!(
            cfg.trophic.grubstake_micros > cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "grubstake outlasts one window"
        );
        assert!(
            cfg.trophic.ransom_cap_micros >= cfg.shipyard.escort_price_micros[0],
            "one capped ransom funds the pirate counter-rung"
        );

        // Physics block VERBATIM from the band (spec §2) + the 120k window.
        assert_eq!(cfg.dt.get(), 0.25);
        assert_eq!(cfg.softening, 1.0e-4);
        assert_eq!(cfg.substep_cfg.accel_ref, 3.0e-4);
        assert_eq!(cfg.substep_cfg.max_substeps, 64);
        assert_eq!(cfg.ephemeris_window, 120_000, "frontier window (runner guard 2.5)");

        // Seam-haven law REPLACES hideout-outermost (spec §3, OD-3): haven =
        // station 6 hosted by body 7 (1.4660 AU), a vendor (the pirate escort
        // settle path), NOT the outermost body.
        assert_eq!(cfg.trophic.hideout_body_index, 7);
        assert_eq!(cfg.stations[FRONTIER_HAVEN_STATION].body_index, 7);
        assert!(
            cfg.stations[FRONTIER_HAVEN_STATION].sells_upgrades,
            "haven is a vendor (resolve_purchases settle path)"
        );
        assert!(
            (cfg.trophic.hideout_body_index as usize) < cfg.bodies.len() - 1,
            "haven sits at the SEAM, not the outermost body"
        );

        // Reach EXPLICIT in this factory too (spec §6) — the 8-9 gap is the
        // never-opens seam against exactly this value.
        assert_eq!(cfg.trophic.pirate_max_reach_au, 0.6);

        // ASSIGN/belief/buy machinery carried from the band.
        assert_eq!(cfg.dispatch_cfg.stagger_period, 16);
        assert_eq!(cfg.dispatch_cfg.demand_low, 10);
        assert_eq!(cfg.dispatch_cfg.demand_high, 20);
        assert!(cfg.trophic.hauler_belief_scoring, "belief scoring ON");
        assert_eq!(cfg.trophic.hauler_buy_policy, BuyPolicy::EscortFirst);
        assert!(cfg.trophic.engage_radius_au > 0.0, "trophic machinery LIVE");
    }

    #[test]
    fn scenario_frontier_wiring_invariants() {
        let cfg = scenario_frontier(7);
        let n = cfg.stations.len();

        // Partitioned tier loops EXACT (spec §3): per tier, 2 Ore legs
        // src->dest + 1 Fuel return dest->sink; rewards 1.0M / 2.3M / 3.9M.
        assert_eq!(cfg.contracts.len(), 9, "3 tiers x (2 Ore legs + 1 Fuel return)");
        for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
            let (src_a, src_b, dest, sink) = FRONTIER_TIER_WIRING[tier];
            let legs: Vec<&ContractInit> =
                cfg.contracts.iter().filter(|k| k.corp_index == tier).collect();
            assert_eq!(legs.len(), 3, "tier {tier} has 3 legs");
            let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
            for k in &legs {
                assert_eq!(k.qty, qty, "tier {tier} lot size");
                assert_eq!(k.reward_micros, reward, "tier {tier} reward ladder");
            }
            let ore_froms: std::collections::BTreeSet<usize> = legs
                .iter()
                .filter(|k| k.resource == Good::ORE)
                .map(|k| k.from_station_index)
                .collect();
            assert_eq!(
                ore_froms,
                [src_a, src_b].into_iter().collect::<std::collections::BTreeSet<_>>(),
                "tier {tier} sources"
            );
            assert!(
                legs.iter()
                    .filter(|k| k.resource == Good::ORE)
                    .all(|k| k.to_station_index == dest),
                "tier {tier} Ore legs land at dest {dest}"
            );
            let ret: Vec<_> = legs.iter().filter(|k| k.resource == Good::FUEL).collect();
            assert_eq!(ret.len(), 1, "tier {tier} has exactly one Fuel return");
            assert_eq!(ret[0].from_station_index, dest, "return departs the dest");
            assert_eq!(ret[0].to_station_index, sink, "return lands at the sink");
        }
        // Spec §3 headline rewards, recomputed from the tier table.
        let rewards: Vec<i64> = TIERS
            .iter()
            .map(|&(q, m)| q as i64 * PER_UNIT_BASE_MICROS * m / 1000)
            .collect();
        assert_eq!(rewards, vec![1_000_000, 2_300_000, 3_900_000]);

        // Per-tier dests and sinks pairwise DISJOINT (independent Schmitt
        // triggers — the trophic decoupling law carried to the big map).
        for (i, wi) in FRONTIER_TIER_WIRING.iter().enumerate() {
            for (j, wj) in FRONTIER_TIER_WIRING.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert_ne!(wi.2, wj.2, "tier dests disjoint");
                assert_ne!(wi.3, wj.3, "tier sinks disjoint");
            }
        }

        // Every station in sources + dests + sinks + {haven} — no orphans.
        let mut covered = std::collections::BTreeSet::new();
        for &(a, b, d, s) in &FRONTIER_TIER_WIRING {
            covered.extend([a, b, d, s]);
        }
        covered.insert(FRONTIER_HAVEN_STATION);
        assert_eq!(
            covered,
            (0..n).collect::<std::collections::BTreeSet<_>>(),
            "every station is in sources + dests + sinks + {{haven}}"
        );

        // The haven is DARK (spec §3): vendor, NO producer, NO contract
        // endpoint — a dark port at the seam.
        assert!(
            cfg.contracts.iter().all(|k| {
                k.from_station_index != FRONTIER_HAVEN_STATION
                    && k.to_station_index != FRONTIER_HAVEN_STATION
            }),
            "haven hosts no contract endpoint"
        );
        assert!(
            cfg.producers.iter().all(|p| p.station_index != FRONTIER_HAVEN_STATION),
            "haven hosts no producer"
        );

        // Every tier loop touches a vendor (heavy haulers shop where they
        // deliver — the restored mechanism): the vendor sits at each dest.
        for &(_, _, dest, _) in &FRONTIER_TIER_WIRING {
            assert!(cfg.stations[dest].sells_upgrades, "tier dest {dest} is a vendor");
        }

        // Per-tier Schmitt-stagger initial stocks carried (18/14/10 against
        // the ONE global 10/20 band): dest Ore + sink Fuel, descending.
        let dest_ore: Vec<i64> = FRONTIER_TIER_WIRING
            .iter()
            .map(|w| cfg.stations[w.2].initial_stock[Good::ORE.index()])
            .collect();
        let sink_fuel: Vec<i64> = FRONTIER_TIER_WIRING
            .iter()
            .map(|w| cfg.stations[w.3].initial_stock[Good::FUEL.index()])
            .collect();
        assert_eq!(dest_ore, vec![18, 14, 10], "dest Ore Schmitt stagger");
        assert_eq!(sink_fuel, vec![18, 14, 10], "sink Fuel Schmitt stagger");

        // Producers: miners at all 6 sources, refiners at the 3 dests, fuel
        // sinks at the 3 sink rows.
        assert_eq!(cfg.producers.len(), 12, "6 miners + 3 refiners + 3 fuel sinks");

        // Corps: 3 tier corps + the Yard + the Port (Port armed in 2.4).
        assert_eq!(cfg.corporations.len(), 5, "3 tier corps + Yard + Port");
        assert_eq!(cfg.shipyard.corp_index, 3, "the Yard receives upgrade payments");
        assert_eq!(cfg.corporations[4].treasury_micros, 0, "the Port starts empty");
        assert!(cfg.contracts.iter().all(|k| k.corp_index < 3), "Yard/Port post no routes");

        // Resolvable + brakable; reset mints the 10-pirate pool.
        let (w, _h) = World::reset(cfg).expect("scenario_frontier must resolve");
        assert_eq!(w.ships.pirate.iter().filter(|p| p.is_some()).count(), 10);
    }

    #[test]
    #[ignore = "prints the golden constant for frontier_trajectory_golden"]
    fn print_golden_frontier() {
        let (mut w, _) =
            World::reset(scenario_frontier(7)).expect("scenario_frontier must resolve");
        let mut cmds = Vec::new();
        for _ in 0..2_000 {
            w.step(&mut cmds);
        }
        println!("FRONTIER_TRAJECTORY_GOLDEN=0x{:016x}", crate::hash::state_hash(&w));
    }

    /// The NEW frontier trajectory golden (world-gets-big spec §9): seed-7
    /// `scenario_frontier` stepped 2_000 ticks (one window), state_hash
    /// pinned. Existing goldens pin tick-0 worlds only; this pins a STEPPED
    /// big-map trajectory so physics/stage/config drift on the frontier is
    /// loud. Re-derive ONLY via `print_golden_frontier` (single-cause re-pin
    /// commits; the calibration v_e bake is the one scheduled re-pin).
    // RE-PINNED: v5->v6 (+hold). Was 0x050de98bd4b6793c.
    const FRONTIER_TRAJECTORY_GOLDEN: u64 = 0x53d88bc83c712b83;

    #[test]
    fn frontier_trajectory_golden() {
        let (mut w, _) =
            World::reset(scenario_frontier(7)).expect("scenario_frontier must resolve");
        let mut cmds = Vec::new();
        for _ in 0..2_000 {
            w.step(&mut cmds);
        }
        assert_eq!(
            crate::hash::state_hash(&w),
            FRONTIER_TRAJECTORY_GOLDEN,
            "frontier trajectory drifted: re-pin only if intentional (single-cause commit, \
             re-derive via print_golden_frontier)"
        );
    }

    #[test]
    fn scenario_frontier_is_seed_derived_and_deterministic() {
        assert_eq!(
            scenario_frontier(7).config_hash(),
            scenario_frontier(7).config_hash()
        );
        let a = scenario_frontier(7);
        let b = scenario_frontier(8);
        assert_ne!(a.config_hash(), b.config_hash());
        assert!(
            a.bodies[1..]
                .iter()
                .zip(&b.bodies[1..])
                .any(|(x, y)| x.elements.m0 != y.elements.m0),
            "mean anomalies are seed-derived"
        );
        // A NEW world, not a re-skin: frontier != trophic at the same seed.
        assert_ne!(
            scenario_frontier(7).config_hash(),
            scenario_trophic(7).config_hash()
        );
    }

    #[test]
    fn scenario_frontier_fuel_pricing_and_port() {
        let cfg = scenario_frontier(7);

        // PriceCfg: Fuel-only live (spec §5, OD-4). cap[Ore]==0 is the
        // structural-off switch — Ore stays dead by construction.
        assert_eq!(cfg.price_cfg.base_micros, vec![0i64, 5_000i64], "Fuel-only base");
        assert_eq!(cfg.price_cfg.cap, vec![0i64, 40i64], "cap[Ore]==0 keeps Ore structurally dead");
        assert_eq!(cfg.price_cfg.slope_milli, 1800);
        assert_eq!(cfg.price_cfg.reprice_interval, 1);

        // Curve endpoints: dry (s=0) 10_000; full (s=cap) 1_000 micros/unit.
        let curve_price = |stock: i64| {
            let s = stock.clamp(0, 40);
            (5_000 * (2000 - s * 1800 / 40) / 1000).max(0)
        };
        assert_eq!(curve_price(0), 10_000);
        assert_eq!(curve_price(40), 1_000);

        // initial_price_micros[Fuel] is seeded FROM THE CURVE at the
        // station's initial stock (spec §5); Ore price 0 everywhere; every
        // seeded fuel price nonzero (the phase-1 half-on guard's input).
        for (row, s) in cfg.stations.iter().enumerate() {
            assert_eq!(
                s.initial_price_micros[Good::ORE.index()],
                0,
                "station {row}: Ore price dead"
            );
            let st = s.initial_stock[Good::FUEL.index()].clamp(0, 40);
            let want = curve_price(st);
            assert_eq!(
                s.initial_price_micros[Good::FUEL.index()],
                want,
                "station {row}: fuel price seeded from the curve"
            );
            assert!(
                s.initial_price_micros[Good::FUEL.index()] > 0,
                "station {row}: half-on guard input must be nonzero"
            );
        }

        // RefuelCfg LIVE (spec §5): lot 5e-11 => 20 lots per 1e-9 tank
        // (~1 lot core leg, ~3-4 frontier leg); revenue -> the Port corp.
        assert_eq!(cfg.refuel.lot_mass, 5.0e-11, "lot_mass");
        assert_eq!(cfg.refuel.corp_index, 4, "the Port corp index");
        let lots = (1.0e-9 / cfg.refuel.lot_mass).round() as u32;
        assert_eq!(lots, 20, "20 lots per tank");

        // The half-on guard accepts the armed factory: reset resolves.
        World::reset(scenario_frontier(7)).expect("frontier resolves with refuel live");
    }

    #[test]
    fn frontier_ore_price_never_updates_and_fuel_rides_the_curve() {
        // cap[Ore]==0 => update_prices skips the row forever; Fuel prices
        // stay inside the curve band [1_000, 10_000].
        let (mut world, _h) = World::reset(scenario_frontier(7)).expect("resolve");
        let mut cmds: Vec<Command> = Vec::new();
        for _ in 0..500 {
            world.step(&mut cmds);
        }
        let mut fuel_updates = 0u32;
        for e in world.recent_events(Tick(0)) {
            if let EventKind::PriceUpdate { resource, price_micros, .. } = e.kind {
                match resource {
                    Good::ORE => panic!("Ore price updated — cap[Ore]==0 must keep it dead"),
                    Good::FUEL => {
                        fuel_updates += 1;
                        assert!(
                            (1_000..=10_000).contains(&price_micros),
                            "fuel price {price_micros} outside the curve band"
                        );
                    }
                    other => panic!("unexpected good {other:?} in frontier v1"),
                }
            }
        }
        // Non-vacuity: the dest refiners land Fuel within 500 ticks
        // (interval 60) — stock moves => at least one Fuel PriceUpdate.
        assert!(fuel_updates > 0, "no Fuel PriceUpdate in 500 ticks — vacuous test");
    }
}
