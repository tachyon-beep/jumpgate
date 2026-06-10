//! trophic_run — the P0 game runner + chronicle (pirates rung 1, plan Task 0).
//!
//! FRAME (PDR-0006): a designer's window, not a gate. This runner steps a
//! seeded world, samples one integer `TrophicSample` per `WINDOW_TICKS`
//! window, emits JSONL for the sweep lab, prints the classifier's diagnosis
//! as evidence (never a verdict on the build), and can print a per-craft
//! chronicle and run a two-run replay bit-identity check.
//!
//! P0 scope: the world is the EXISTING trader-style economy (no pirates) via a
//! temporary local config fn cloned from the trader template shape — its job
//! is to measure `laden_trips_per_window`, THE spec-§4 calibration input for
//! the food band. Task 6 replaces the config with `scenario_trophic`.
//!
//! Usage:
//!   cargo run -p jumpgate-core --example trophic_run -- \
//!     --seed 7 --ticks 10000 --jsonl /tmp/p0.jsonl --chronicle --replay-check

use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::ExitCode;

use jumpgate_core::diagnostics::{self, TrophicSample};
use jumpgate_core::economy::{Recipe, Resource};
use jumpgate_core::{
    BaseSpec, BodyInit, Command, ContractInit, CorporationInit, CraftId, CraftInit, DispatchCfg,
    Dt, EventKind, G_CANONICAL, GuidanceParams, OrbitalElements, PriceCfg, ProducerInit,
    RunConfig, StateView, StationInit, SubstepCfg, Tick, Vec3, World, state_hash,
};

/// Replay-check / hash-stream sampling stride (ticks).
const HASH_SAMPLE_EVERY: u64 = 1000;

/// Scripted haulers in the P0 baseline (one per seeded route template).
const NUM_CRAFT: usize = 4;

/// SplitMix64-style finalizer over `(seed, k)` — the trader template's
/// dependency-free config-derivation mix (config inputs only, never world RNG).
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

/// TEMPORARY P0 baseline config — the trader scenario template's shape
/// (jumpgate-py `trader_config_template`: 1e-3 star, 4 station bodies at
/// a = 0.35/0.55/0.8/1.1 AU with seed-derived phases, Ore miners/sinks, four
/// rate-mispriced route templates) with ONE deviation: `stagger_period = 4`
/// turns the scripted ASSIGN ON (the trader gym sets 0 because the RL agent
/// owns acceptance; P0 needs the haul loop to self-run with no commands).
/// Replaced by `scenario_trophic` in Task 6.
fn p0_config(seed: u64, num_craft: usize) -> RunConfig {
    const ORBIT_AU: [f64; 4] = [0.35, 0.55, 0.8, 1.1];
    const BODY_MASS: f64 = 1.0e-12;
    const STAR_MASS: f64 = 1.0e-3;

    let star = BodyInit {
        mass: STAR_MASS,
        elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
    };
    let mut bodies = vec![star];
    for (k, &a) in ORBIT_AU.iter().enumerate() {
        let m0 = u64_to_unit_f64(mix(seed, (k + 1) as u64)) * std::f64::consts::TAU;
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    let spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 2.0,
        base_fuel_capacity: 1.0e-9,
    };

    // Spawn co-located with body 1 (station 0's host), co-orbiting.
    let m0_home = bodies[1].elements.m0;
    let a_home = ORBIT_AU[0];
    let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
    let v_circ = (mu / a_home).sqrt();
    let pos = Vec3::new(a_home * m0_home.cos(), a_home * m0_home.sin(), 0.0);
    let vel = Vec3::new(-v_circ * m0_home.sin(), v_circ * m0_home.cos(), 0.0);
    let craft = (0..num_craft)
        .map(|_| CraftInit { spec: spec.clone(), pos, vel, fuel_mass: 1.0e-9 })
        .collect();

    let stations = vec![
        StationInit { body_index: 1, initial_stock: [40, 0], initial_price_micros: [0, 0] },
        StationInit { body_index: 2, initial_stock: [0, 0], initial_price_micros: [0, 0] },
        StationInit { body_index: 3, initial_stock: [40, 0], initial_price_micros: [0, 0] },
        StationInit { body_index: 4, initial_stock: [0, 0], initial_price_micros: [0, 0] },
    ];
    let producers = vec![
        ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 },
        },
        ProducerInit {
            station_index: 2,
            recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 },
        },
        ProducerInit {
            station_index: 1,
            recipe: Recipe { input: Some((Resource::Ore, 5)), output: None, interval: 60 },
        },
        ProducerInit {
            station_index: 3,
            recipe: Recipe { input: Some((Resource::Ore, 5)), output: None, interval: 60 },
        },
    ];
    let corporations =
        vec![CorporationInit { treasury_micros: 1_000_000_000, home_station_index: 0 }];
    let contracts = vec![
        ContractInit { corp_index: 0, resource: Resource::Ore, qty: 5, from_station_index: 0, to_station_index: 1, reward_micros: 1_000_000 },
        ContractInit { corp_index: 0, resource: Resource::Ore, qty: 5, from_station_index: 2, to_station_index: 3, reward_micros: 1_200_000 },
        ContractInit { corp_index: 0, resource: Resource::Ore, qty: 5, from_station_index: 0, to_station_index: 3, reward_micros: 1_600_000 },
        ContractInit { corp_index: 0, resource: Resource::Ore, qty: 5, from_station_index: 2, to_station_index: 1, reward_micros: 3_000_000 },
    ];

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
            // DEVIATION from the trader template (0): scripted ASSIGN ON so
            // the haul loop self-runs; stagger 4 spreads accepts across craft.
            stagger_period: 4,
            contract_reward_micros: 0,
            contract_qty: 0,
        },
    }
}

struct Args {
    seed: u64,
    ticks: u64,
    jsonl: Option<String>,
    chronicle: bool,
    replay_check: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { seed: 7, ticks: 50_000, jsonl: None, chronicle: false, replay_check: false };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--seed" => {
                let v = it.next().ok_or("--seed needs a value")?;
                args.seed = v.parse().map_err(|e| format!("--seed: {e}"))?;
            }
            "--ticks" => {
                let v = it.next().ok_or("--ticks needs a value")?;
                args.ticks = v.parse().map_err(|e| format!("--ticks: {e}"))?;
            }
            "--jsonl" => args.jsonl = Some(it.next().ok_or("--jsonl needs a path")?),
            "--chronicle" => args.chronicle = true,
            "--replay-check" => args.replay_check = true,
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    Ok(args)
}

/// One full seeded run: returns the per-window samples, the sampled
/// `(tick, state_hash)` stream, and the final world (for the chronicle).
fn simulate(
    seed: u64,
    ticks: u64,
    mut jsonl: Option<&mut BufWriter<File>>,
) -> (Vec<TrophicSample>, Vec<(u64, u64)>, World) {
    let (mut world, _config_hash) = World::reset(p0_config(seed, NUM_CRAFT))
        .unwrap_or_else(|e| panic!("p0 config must resolve: {e:?}"));
    let mut cmds: Vec<Command> = Vec::new();
    let mut samples = Vec::new();
    let mut hashes = Vec::new();
    let mut window_start = Tick(0);
    for _ in 0..ticks {
        world.step(&mut cmds);
        let t = world.tick().0;
        if t % HASH_SAMPLE_EVERY == 0 {
            hashes.push((t, state_hash(&world)));
        }
        if t % diagnostics::WINDOW_TICKS == 0 {
            let s = diagnostics::sample_window(&world, window_start);
            if let Some(w) = jsonl.as_mut() {
                writeln!(w, "{}", sample_json(&s)).expect("jsonl write");
            }
            window_start = world.tick();
            samples.push(s);
        }
    }
    (samples, hashes, world)
}

/// Serialize one window sample as a JSONL line (field-for-field).
fn sample_json(s: &TrophicSample) -> String {
    serde_json::json!({
        "tick": s.tick,
        "active_pirates": s.active_pirates,
        "lying_low": s.lying_low,
        "laden_in_transit": s.laden_in_transit,
        "laden_trips": s.laden_trips,
        "robs": s.robs,
        "drivenoffs": s.drivenoffs,
        "purchases_hull": s.purchases_hull,
        "purchases_escort": s.purchases_escort,
        "per_route_robs": s.per_route_robs,
        "per_route_accepts": s.per_route_accepts,
        "per_route_traffic": s.per_route_traffic,
        "yard_treasury_micros": s.yard_treasury_micros,
        "per_craft_credits": s.per_craft_credits,
        "engagement_phase_milli": s.engagement_phase_milli,
    })
    .to_string()
}

/// The craft a chronicle line belongs to. Per-tick noise (ThrustApplied,
/// ActionIngested) and world-scoped economy events (offers, prices,
/// production, trades) have no chronicle subject; future variants default
/// to skipped rather than breaking the printer.
fn chronicle_subject(kind: &EventKind) -> Option<CraftId> {
    match *kind {
        EventKind::Arrival { craft, .. }
        | EventKind::FuelEmpty { craft }
        | EventKind::Wake { craft }
        | EventKind::Reward { craft, .. } => Some(craft),
        EventKind::ContractAccepted { hauler, .. }
        | EventKind::ContractFulfilled { hauler, .. } => Some(hauler),
        EventKind::Robbed { pirate, .. }
        | EventKind::DrivenOff { pirate, .. }
        | EventKind::HaulerKilled { pirate, .. }
        | EventKind::PirateLieLow { pirate, .. }
        | EventKind::PirateLeft { pirate }
        | EventKind::PirateSpawned { pirate } => Some(pirate),
        _ => None,
    }
}

/// Per-craft life-arc printer: group `recent_events` by craft id, one
/// tick-stamped line per event (spec §10's chronicle, v0 form).
fn print_chronicle(world: &World) {
    println!("--- chronicle ---");
    for id in world.craft_ids() {
        println!("craft {}/{}:", id.slot, id.generation);
        for e in world.recent_events(Tick(0)) {
            if chronicle_subject(&e.kind) == Some(id) {
                println!("  t={:>6} {:?}", e.tick.0, e.kind);
            }
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("trophic_run: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut jsonl_writer = args.jsonl.as_ref().map(|p| {
        BufWriter::new(File::create(p).unwrap_or_else(|e| panic!("--jsonl {p}: {e}")))
    });
    let (samples, hashes, world) = simulate(args.seed, args.ticks, jsonl_writer.as_mut());
    if let Some(mut w) = jsonl_writer {
        w.flush().expect("jsonl flush");
    }

    println!(
        "trophic_run: seed={} ticks={} windows={} (W={})",
        args.seed,
        args.ticks,
        samples.len(),
        diagnostics::WINDOW_TICKS
    );
    for s in &samples {
        println!(
            "  window@{:>6}: laden_trips={:>3} accepts={:?} laden_in_transit={} credits={:?}",
            s.tick,
            s.laden_trips,
            s.per_route_accepts,
            s.laden_in_transit,
            s.per_craft_credits
        );
    }
    if !samples.is_empty() {
        let total: u64 = samples.iter().map(|s| u64::from(s.laden_trips)).sum();
        // Mean laden trips per window, in milli (integer report).
        let mean_milli = total * 1000 / samples.len() as u64;
        println!(
            "laden_trips_per_window: mean={}.{:03} over {} windows (THE spec-§4 CALIBRATION INPUT)",
            mean_milli / 1000,
            mean_milli % 1000,
            samples.len()
        );
    }
    let diag = diagnostics::classify(&samples);
    println!("diagnosis (a window, not a gate — PDR-0006): {diag:?}");

    if args.chronicle {
        print_chronicle(&world);
    }

    if args.replay_check {
        let (_, hashes2, _) = simulate(args.seed, args.ticks, None);
        if hashes == hashes2 {
            println!(
                "replay-check OK: {} (tick, state_hash) samples bit-identical (every {} ticks)",
                hashes.len(),
                HASH_SAMPLE_EVERY
            );
        } else {
            let first = hashes
                .iter()
                .zip(&hashes2)
                .find(|(a, b)| a != b)
                .map(|(a, _)| a.0);
            eprintln!("replay-check FAILED: streams diverge at tick {first:?}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
