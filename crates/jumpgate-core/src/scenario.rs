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
    GuidanceParams, OrbitalElements, PriceCfg, ProducerInit, RunConfig, ShipyardCfg, StationInit,
    SubstepCfg, TrophicCfg,
};
use crate::economy::{Recipe, Resource};
use crate::math::{G_CANONICAL, Vec3};
use crate::stores::CraftRole;
use crate::time::Dt;

/// Station-body semi-major axes (AU): 6 bodies evenly spread over the spec-§10
/// 0.35–1.4 AU band. Body index k+1 hosts station row k; body 6 (1.4 AU) is
/// the outermost — the hideout.
pub const STATION_ORBIT_AU: [f64; 6] = [0.35, 0.56, 0.77, 0.98, 1.19, 1.40];

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
    let stock = |ore: i64, fuel: i64| -> [i64; crate::economy::N_RESOURCES] {
        let mut s = [0i64; crate::economy::N_RESOURCES];
        s[Resource::Ore.index()] = ore;
        s[Resource::Fuel.index()] = fuel;
        s
    };
    let stations = vec![
        StationInit { body_index: 1, initial_stock: stock(40, 18), initial_price_micros: [0, 0], sells_upgrades: false },
        StationInit { body_index: 2, initial_stock: stock(40, 14), initial_price_micros: [0, 0], sells_upgrades: false },
        StationInit { body_index: 3, initial_stock: stock(40, 10), initial_price_micros: [0, 0], sells_upgrades: false },
        StationInit { body_index: 4, initial_stock: stock(18, 0), initial_price_micros: [0, 0], sells_upgrades: true },
        StationInit { body_index: 5, initial_stock: stock(14, 0), initial_price_micros: [0, 0], sells_upgrades: false },
        StationInit { body_index: 6, initial_stock: stock(10, 0), initial_price_micros: [0, 0], sells_upgrades: true },
    ];
    let producers = vec![
        // Ore miners at the sources.
        ProducerInit { station_index: 0, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 2, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        // Refiners at the tier destinations (Ore -> Fuel): the Ore demand sinks.
        ProducerInit { station_index: 3, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        ProducerInit { station_index: 5, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        // Fuel sinks back at the sources (per-tier return-leg demand).
        ProducerInit { station_index: 0, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 2, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
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
                resource: Resource::Ore,
                qty,
                from_station_index: from,
                to_station_index: dest,
                reward_micros: reward,
            });
        }
        contracts.push(ContractInit {
            corp_index: tier,
            resource: Resource::Fuel,
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
    }
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
        other => return Err(format!("--set {other}: unknown knob")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::WINDOW_TICKS;
    use crate::world::World;

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
        // Errors are loud: unknown knob, malformed value, bad enum.
        assert!(apply_knob(&mut cfg, "warp_factor", "9").is_err());
        assert!(apply_knob(&mut cfg, "p_rob_milli", "many").is_err());
        assert!(apply_knob(&mut cfg, "hauler_buy_policy", "Maximal").is_err());
    }
}
