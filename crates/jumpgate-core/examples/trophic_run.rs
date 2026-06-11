//! trophic_run — the pirates-rung game runner + chronicle (plan Tasks 0 + 6).
//!
//! FRAME (PDR-0006): a designer's window, not a gate. This runner steps a
//! seeded `scenario_trophic` world, samples one integer `TrophicSample` per
//! `WINDOW_TICKS` window, emits JSONL for the sweep lab
//! (`python/analysis/sweep_trophic.py`), prints the classifier's diagnosis as
//! evidence (never a verdict on the build), and can print a per-craft
//! chronicle and run a two-run replay bit-identity check.
//!
//! Every spec-§9 tuning knob rides `--set knob=value` (repeatable;
//! `scenario::apply_knob` is the surface). The live positive control is the
//! hungry-roamer injection `--set pirate_max_reach_au=999 --set stay_milli=0
//! --set upkeep_per_tick=200 --set grubstake_micros=2000000000` (must read
//! RiskEqualized — the instrument-kill check, spec §1/§9; the old reach+stay
//! recipe was neutralized by the hunger gate: fed pirates camp, genuinely
//! clumped, correctly Alive — jumpgate-50c6a8a3bd). `--assert-no-fuel-empty`
//! makes the 50k-tick endurance window mechanical (zero `FuelEmpty` events;
//! a determinism-cheap window, not an aliveness gate).
//!
//! Usage:
//!   cargo run -p jumpgate-core --example trophic_run -- \
//!     --seed 7 --ticks 50000 --jsonl /tmp/run.jsonl --chronicle \
//!     --replay-check --assert-no-fuel-empty --set p_rob_milli=600

use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::ExitCode;

use jumpgate_core::diagnostics::{self, TrophicSample};
use jumpgate_core::{
    Command, CraftId, EventKind, RunConfig, StateView, Tick, World, apply_knob, scenario_trophic,
    state_hash,
};

/// Replay-check / hash-stream sampling stride (ticks).
const HASH_SAMPLE_EVERY: u64 = 1000;

struct Args {
    seed: u64,
    ticks: u64,
    jsonl: Option<String>,
    chronicle: bool,
    replay_check: bool,
    assert_no_fuel_empty: bool,
    sets: Vec<(String, String)>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 7,
        ticks: 50_000,
        jsonl: None,
        chronicle: false,
        replay_check: false,
        assert_no_fuel_empty: false,
        sets: Vec::new(),
    };
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
            "--assert-no-fuel-empty" => args.assert_no_fuel_empty = true,
            "--set" => {
                let kv = it.next().ok_or("--set needs knob=value")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| format!("--set {kv}: expected knob=value"))?;
                args.sets.push((k.to_string(), v.to_string()));
            }
            other => return Err(format!("unknown arg: {other}")),
        }
    }
    Ok(args)
}

/// `simulate`'s product: per-window samples, the sampled `(tick, state_hash)`
/// stream, and the final world (chronicle + event counts).
type RunProduct = (Vec<TrophicSample>, Vec<(u64, u64)>, World);

/// One full seeded run. The config is rebuilt per run from `(seed, sets)` so
/// the replay-check's second run shares nothing but the recipe.
fn simulate(
    args: &Args,
    mut jsonl: Option<&mut BufWriter<File>>,
) -> Result<RunProduct, String> {
    let mut cfg: RunConfig = scenario_trophic(args.seed);
    for (k, v) in &args.sets {
        apply_knob(&mut cfg, k, v)?;
    }
    let (mut world, _config_hash) =
        World::reset(cfg).map_err(|e| format!("scenario_trophic must resolve: {e}"))?;
    let mut cmds: Vec<Command> = Vec::new();
    let mut samples = Vec::new();
    let mut hashes = Vec::new();
    let mut window_start = Tick(0);
    for _ in 0..args.ticks {
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
    Ok((samples, hashes, world))
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
        | EventKind::Reward { craft, .. }
        | EventKind::UpgradePurchased { craft, .. } => Some(craft),
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
/// tick-stamped line per event (spec §10's chronicle, v0 form). Consecutive
/// repeats of the same event shape for the same craft (a lying-low pirate
/// re-seeking its hideout emits an Arrival every ~10 ticks — watchability
/// noise, seed-7 lesson) collapse into one line with a repeat count.
fn print_chronicle(world: &World) {
    println!("--- chronicle ---");
    for id in world.craft_ids() {
        println!("craft {}/{}:", id.slot, id.generation);
        let mut pending: Option<(u64, u64, String)> = None; // (first_tick, count, line)
        let flush = |p: &Option<(u64, u64, String)>| {
            if let Some((t, n, line)) = p {
                if *n > 1 {
                    println!("  t={t:>6} {line}  (x{n}, consecutive)");
                } else {
                    println!("  t={t:>6} {line}");
                }
            }
        };
        for e in world.recent_events(Tick(0)) {
            if chronicle_subject(&e.kind) != Some(id) {
                continue;
            }
            let line = format!("{:?}", e.kind);
            match &mut pending {
                Some((_, n, prev)) if *prev == line => *n += 1,
                _ => {
                    flush(&pending);
                    pending = Some((e.tick.0, 1, line));
                }
            }
        }
        flush(&pending);
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
    let (samples, hashes, world) = match simulate(&args, jsonl_writer.as_mut()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("trophic_run: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Some(mut w) = jsonl_writer {
        w.flush().expect("jsonl flush");
    }

    println!(
        "trophic_run: seed={} ticks={} windows={} (W={}) sets={:?}",
        args.seed,
        args.ticks,
        samples.len(),
        diagnostics::WINDOW_TICKS,
        args.sets
    );
    for s in &samples {
        println!(
            "  window@{:>6}: active={} low={} trips={:>3} robs={:>3} drivenoffs={:>3} \
             buys(h/e)={}/{} yard={} laden={}",
            s.tick,
            s.active_pirates,
            s.lying_low,
            s.laden_trips,
            s.robs,
            s.drivenoffs,
            s.purchases_hull,
            s.purchases_escort,
            s.yard_treasury_micros,
            s.laden_in_transit,
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

    // The endurance window (spec §6): FuelEmpty count over the whole run.
    let fuel_empty = world
        .recent_events(Tick(0))
        .iter()
        .filter(|e| matches!(e.kind, EventKind::FuelEmpty { .. }))
        .count();
    let robs_total: u64 = samples.iter().map(|s| u64::from(s.robs)).sum();
    let trips_total: u64 = samples.iter().map(|s| u64::from(s.laden_trips)).sum();
    let buys_total: u64 = samples
        .iter()
        .map(|s| u64::from(s.purchases_hull) + u64::from(s.purchases_escort))
        .sum();
    // Machine-readable summary line (the sweep aggregator parses this).
    println!(
        "RESULT seed={} ticks={} verdict={:?} cycled={} risk_heterogeneous={} \
         outcomes_disperse={} fuel_empty={} robs={} laden_trips={} purchases={}",
        args.seed,
        args.ticks,
        diag.verdict,
        diag.cycled,
        diag.risk_heterogeneous,
        diag.outcomes_disperse,
        fuel_empty,
        robs_total,
        trips_total,
        buys_total,
    );

    if args.chronicle {
        print_chronicle(&world);
    }

    if args.assert_no_fuel_empty && fuel_empty > 0 {
        eprintln!("trophic_run: endurance window violated — {fuel_empty} FuelEmpty event(s)");
        return ExitCode::FAILURE;
    }

    if args.replay_check {
        let (_, hashes2, _) = match simulate(&args, None) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("trophic_run: replay arm: {e}");
                return ExitCode::FAILURE;
            }
        };
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
