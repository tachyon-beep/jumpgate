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
    Command, CraftId, CraftRole, EventKind, GossipNode, RunConfig, StateView, Tick, World,
    apply_knob, scenario_frontier, scenario_trophic, state_hash,
};

/// Replay-check / hash-stream sampling stride (ticks).
const HASH_SAMPLE_EVERY: u64 = 1000;

struct Args {
    seed: u64,
    ticks: u64,
    /// Scenario factory: "trophic" (default, the banked control world) or
    /// "frontier" (WGB §2). Unknown names are loud errors.
    scenario: String,
    jsonl: Option<String>,
    chronicle: bool,
    /// Printer-side filter (media rung spec §8): skip `GossipHeard` chronicle
    /// lines whose `claimed_value_micros` is below this. 0 = print all.
    chronicle_gossip_min_micros: i64,
    replay_check: bool,
    assert_no_fuel_empty: bool,
    /// Post-run media event log (JSONL; media rung plan Task 8.4): one line
    /// per AlertBorn / GossipHeard / Robbed / ContractAccepted, written from
    /// the retained event stream for `python/analysis/media_log.py`.
    gossip_log: Option<String>,
    sets: Vec<(String, String)>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        seed: 7,
        ticks: 50_000,
        scenario: "trophic".to_string(),
        jsonl: None,
        chronicle: false,
        chronicle_gossip_min_micros: 0,
        replay_check: false,
        assert_no_fuel_empty: false,
        gossip_log: None,
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
            "--scenario" => {
                args.scenario = it.next().ok_or("--scenario needs a value")?;
            }
            "--jsonl" => args.jsonl = Some(it.next().ok_or("--jsonl needs a path")?),
            "--chronicle" => args.chronicle = true,
            "--chronicle-gossip-min-micros" => {
                let v = it
                    .next()
                    .ok_or("--chronicle-gossip-min-micros needs a value")?;
                args.chronicle_gossip_min_micros = v
                    .parse()
                    .map_err(|e| format!("--chronicle-gossip-min-micros: {e}"))?;
            }
            "--replay-check" => args.replay_check = true,
            "--assert-no-fuel-empty" => args.assert_no_fuel_empty = true,
            "--gossip-log" => {
                args.gossip_log = Some(it.next().ok_or("--gossip-log needs a path")?);
            }
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

/// Config-derived facts for the META anchored line (world-gets-big phase 0b).
/// Computed from the resolved RunConfig before reset consumes it. `scenario`
/// is hardcoded until the phase-2 `--scenario` flag exists.
struct MetaFacts {
    scenario: &'static str,
    stations: usize,
    haulers: usize,
    pirates_initial: usize,
    station_radii_milli_au: Vec<u32>,
}

/// `simulate`'s product: per-window samples, the sampled `(tick, state_hash)`
/// stream, the final world (chronicle + event counts), and the META facts.
type RunProduct = (Vec<TrophicSample>, Vec<(u64, u64)>, World, MetaFacts);

/// One full seeded run. The config is rebuilt per run from `(seed, sets)` so
/// the replay-check's second run shares nothing but the recipe.
fn simulate(args: &Args, mut jsonl: Option<&mut BufWriter<File>>) -> Result<RunProduct, String> {
    let (scenario_name, mut cfg): (&'static str, RunConfig) = match args.scenario.as_str() {
        "trophic" => ("trophic", scenario_trophic(args.seed)),
        "frontier" => ("frontier", scenario_frontier(args.seed)),
        other => return Err(format!("--scenario {other}: unknown scenario (trophic|frontier)")),
    };
    for (k, v) in &args.sets {
        apply_knob(&mut cfg, k, v)?;
    }
    // WGB §2 runner guard: past-window ephemeris lookups silently clamp to
    // the last sample. Checked after knob overrides, against the actual cfg.
    if args.ticks > cfg.ephemeris_window {
        return Err(format!(
            "--ticks {} > ephemeris_window {}: past-window orbits silently freeze; lower --ticks or raise the window",
            args.ticks, cfg.ephemeris_window
        ));
    }
    let meta = MetaFacts {
        scenario: scenario_name,
        stations: cfg.stations.len(),
        haulers: cfg
            .craft
            .iter()
            .filter(|c| c.role != CraftRole::Pirate)
            .count(),
        pirates_initial: cfg
            .craft
            .iter()
            .filter(|c| c.role == CraftRole::Pirate)
            .count(),
        station_radii_milli_au: cfg
            .stations
            .iter()
            .map(|s| diagnostics::permille_floor(cfg.bodies[s.body_index].elements.a, 1.0))
            .collect(),
    };
    let (mut world, _config_hash) =
        World::reset(cfg).map_err(|e| format!("scenario_{} must resolve: {e}", args.scenario))?;
    let mut cmds: Vec<Command> = Vec::new();
    let mut samples = Vec::new();
    let mut hashes = Vec::new();
    let mut window_start = Tick(0);
    if let Some(w) = jsonl.as_mut() {
        // META row first. It has no "tick" key, so window consumers can gate on
        // row shape and older JSONL stays parseable.
        writeln!(
            w,
            "{}",
            serde_json::json!({
                "meta_seed": args.seed,
                "meta_scenario": meta.scenario,
                "meta_stations": meta.stations,
                "meta_haulers": meta.haulers,
                "meta_pirates_initial": meta.pirates_initial,
                "meta_station_radii_milli_au": meta.station_radii_milli_au,
            })
        )
        .expect("jsonl write");
    }
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
    Ok((samples, hashes, world, meta))
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
        // Media lab keys (Task 8.1) — ADDITIVE: every pre-media key above is
        // byte-untouched.
        "gossip_born": s.gossip_born,
        "gossip_first_heard": s.gossip_first_heard,
        "gossip_born_cum": s.gossip_born_cum,
        "gossip_escaped_cum": s.gossip_escaped_cum,
        "alerts_carried": s.alerts_carried,
        "stations_with_news": s.stations_with_news,
        "per_station_alerts": s.per_station_alerts,
        "per_station_contacts": s.per_station_contacts,
        "heard_lag_ticks": s.heard_lag_ticks,
        "heard_hops": s.heard_hops,
        "alerts_evicted_cum": s.alerts_evicted_cum,
        "assign_decisions_cum": s.assign_decisions_cum,
        "assign_flips_cum": s.assign_flips_cum,
        "assign_counts_cum": s.assign_counts_cum,
        // Fuel lab keys (world-gets-big phase 0b) — ADDITIVE: every pre-fuel
        // key above is byte-untouched.
        "per_craft_role": s.per_craft_role,
        "per_craft_thrust_ticks": s.per_craft_thrust_ticks,
        "per_craft_burn_milli": s.per_craft_burn_milli,
        "per_craft_min_tank_permille": s.per_craft_min_tank_permille,
        "leg_burn_permille": s.leg_burn_permille,
    })
    .to_string()
}

/// Role-split fuel aggregates (phase 0b, spec §8 — windows, never gates).
/// The pirate side exists for the per-role JSONL rows only; the anchored FUEL
/// line carries HAULER numbers. 0 sentinels mirror the MEDIA precedent.
struct FuelAgg {
    duty_milli: u64,
    burn_total_milli: u64,
    median_leg_burn_permille: u32,
    min_tank_permille: u32,
}

fn fuel_agg(samples: &[TrophicSample], pirate_side: bool) -> FuelAgg {
    let zero = FuelAgg {
        duty_milli: 0,
        burn_total_milli: 0,
        median_leg_burn_permille: 0,
        min_tank_permille: 0,
    };
    let Some(last) = samples.last() else {
        return zero;
    };
    let rows: Vec<usize> = (0..last.per_craft_role.len())
        .filter(|&r| (last.per_craft_role[r] == 2) == pirate_side)
        .collect();
    if rows.is_empty() {
        return zero;
    }

    // Duty: pooled thrusting ticks over pooled craft-ticks, milli, FLOOR.
    let thrust: u64 = rows.iter().map(|&r| last.per_craft_thrust_ticks[r]).sum();
    let craft_ticks = (rows.len() as u64).saturating_mul(last.tick);
    let duty_milli = thrust
        .saturating_mul(1000)
        .checked_div(craft_ticks)
        .unwrap_or(0);
    let burn_total_milli: u64 =
        rows.iter().map(|&r| u64::from(last.per_craft_burn_milli[r])).sum();
    // Contract legs are hauler-only by construction; pirates read the 0
    // sentinel because they fly no contract legs.
    let median_leg_burn_permille = if pirate_side {
        0
    } else {
        let mut legs: Vec<u32> = samples
            .iter()
            .flat_map(|s| s.leg_burn_permille.iter().copied())
            .collect();
        legs.sort_unstable();
        if legs.is_empty() { 0 } else { legs[(legs.len() - 1) / 2] }
    };
    let min_tank_permille = rows
        .iter()
        .map(|&r| last.per_craft_min_tank_permille[r])
        .min()
        .unwrap_or(0);
    FuelAgg {
        duty_milli,
        burn_total_milli,
        median_leg_burn_permille,
        min_tank_permille,
    }
}

/// Post-run media event log (plan Task 8.4): one JSONL line per
/// media-relevant event from the retained stream — AlertBorn ("born"),
/// GossipHeard ("heard"), Robbed ("rob"), ContractAccepted ("accept") — for
/// `python/analysis/media_log.py`. Routes join through the now-public
/// `diagnostics::route_of` (a settled/unresolvable contract reads `null`).
/// Carrier encoding: `"s<row>"` station / `"c<slot>"` craft — station slot ==
/// dense row in v1 (stations mint once at reset and never despawn).
fn write_gossip_log(world: &World, path: &str) {
    let mut w =
        BufWriter::new(File::create(path).unwrap_or_else(|e| panic!("--gossip-log {path}: {e}")));
    for e in world.recent_events(Tick(0)) {
        let line = match e.kind {
            EventKind::AlertBorn {
                alert_seq,
                route,
                pirate,
                hauler,
                truth_value_micros,
                claimed_value_micros,
            } => serde_json::json!({
                "e": "born", "tick": e.tick.0, "alert": alert_seq, "route": route,
                "pirate": pirate.slot, "hauler": hauler.slot,
                "truth": truth_value_micros, "claimed": claimed_value_micros,
            }),
            EventKind::GossipHeard {
                carrier,
                alert_seq,
                route,
                claimed_value_micros,
                hops,
                rob_tick,
                ..
            } => {
                let carrier = match carrier {
                    GossipNode::Station(s) => format!("s{}", s.slot),
                    GossipNode::Craft(c) => format!("c{}", c.slot),
                };
                serde_json::json!({
                    "e": "heard", "tick": e.tick.0, "alert": alert_seq,
                    "carrier": carrier, "route": route, "hops": hops,
                    "claimed": claimed_value_micros, "rob_tick": rob_tick.0,
                })
            }
            EventKind::Robbed { contract, .. } => serde_json::json!({
                "e": "rob", "tick": e.tick.0,
                "route": diagnostics::route_of(world, contract),
            }),
            EventKind::ContractAccepted { contract, hauler } => serde_json::json!({
                "e": "accept", "tick": e.tick.0,
                "route": diagnostics::route_of(world, contract),
                "hauler": hauler.slot,
            }),
            _ => continue,
        };
        writeln!(w, "{line}").expect("gossip-log write");
    }
    w.flush().expect("gossip-log flush");
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
        // World-gets-big §7: the refuel and the failure thread into the
        // craft's life arc, so a stranded run reads end-to-end.
        EventKind::Refueled { craft, .. } => Some(craft),
        EventKind::ContractFailed { hauler, .. } => Some(hauler),
        EventKind::Robbed { pirate, .. }
        | EventKind::DrivenOff { pirate, .. }
        | EventKind::HaulerKilled { pirate, .. }
        | EventKind::PirateLieLow { pirate, .. }
        | EventKind::PirateLeft { pirate }
        | EventKind::PirateSpawned { pirate } => Some(pirate),
        // Craft hearings thread into the carrier's life arc; station hearings
        // feed the gossip log/panels (a station-thread chronicle is a named
        // deferral, media rung spec §8). AlertBorn shadows Robbed: no arm.
        EventKind::GossipHeard {
            carrier: GossipNode::Craft(c),
            ..
        } => Some(c),
        EventKind::GossipHeard { .. } | EventKind::AlertBorn { .. } => None,
        _ => None,
    }
}

/// Per-craft life-arc printer: group `recent_events` by craft id, one
/// tick-stamped line per event (spec §10's chronicle, v0 form). Consecutive
/// repeats of the same event shape for the same craft (a lying-low pirate
/// re-seeking its hideout emits an Arrival every ~10 ticks — watchability
/// noise, seed-7 lesson) collapse into one line with a repeat count.
/// `gossip_min_micros` skips `GossipHeard` lines below the claimed-value
/// threshold (printer-side only; 0 = print all — the owner tunes at console).
fn print_chronicle(world: &World, gossip_min_micros: i64) {
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
            if let EventKind::GossipHeard {
                claimed_value_micros,
                ..
            } = e.kind
                && claimed_value_micros < gossip_min_micros
            {
                continue; // below the watchability threshold (printer-side only)
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

    let mut jsonl_writer = args
        .jsonl
        .as_ref()
        .map(|p| BufWriter::new(File::create(p).unwrap_or_else(|e| panic!("--jsonl {p}: {e}"))));
    let (samples, hashes, world, meta) = match simulate(&args, jsonl_writer.as_mut()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("trophic_run: {e}");
            return ExitCode::FAILURE;
        }
    };
    let hauler_fuel = fuel_agg(&samples, false);
    let pirate_fuel = fuel_agg(&samples, true);
    if let Some(mut w) = jsonl_writer {
        // Per-role FUEL rows (phase 0b, spec §8): the anchored stdout line
        // carries HAULER numbers only; pirates ride these JSONL tail rows.
        // No "tick" key — window consumers gate on `"tick" in row`.
        for (role, a) in [("hauler", &hauler_fuel), ("pirate", &pirate_fuel)] {
            writeln!(
                w,
                "{}",
                serde_json::json!({
                    "fuel_role": role,
                    "duty_milli": a.duty_milli,
                    "burn_total_milli": a.burn_total_milli,
                    "median_leg_burn_permille": a.median_leg_burn_permille,
                    "min_tank_permille": a.min_tank_permille,
                })
            )
            .expect("jsonl write");
        }
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
    // The META anchored line (world-gets-big phase 0b): population and map facts
    // come from the run, not from mirrored Python constants.
    println!(
        "META seed={} scenario={} stations={} haulers={} pirates_initial={} \
         station_radii_milli_au={:?}",
        args.seed,
        meta.scenario,
        meta.stations,
        meta.haulers,
        meta.pirates_initial,
        meta.station_radii_milli_au,
    );
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

    // The MEDIA line (Task 8.3; spec §9 — a window, not a gate): anchored
    // machine-readable shape, parsed by sweep_trophic.py's MEDIA_RE (the
    // lockstep rule: line and regex land in the SAME commit). Lags pool over
    // all windows' Craft-carrier first-hearings; integer lower-median and
    // p90 with a 0 sentinel when no craft ever heard news.
    let born_total: u64 = samples.iter().map(|s| u64::from(s.gossip_born)).sum();
    let mut lags: Vec<u32> = samples
        .iter()
        .flat_map(|s| s.heard_lag_ticks.iter().copied())
        .collect();
    lags.sort_unstable();
    let median_lag = if lags.is_empty() {
        0
    } else {
        lags[(lags.len() - 1) / 2]
    };
    let p90_lag = if lags.is_empty() {
        0
    } else {
        lags[(lags.len() * 9 / 10).min(lags.len() - 1)]
    };
    println!(
        "MEDIA seed={} born={} escaped_milli={} median_lag={} p90_lag={} reading={:?}",
        args.seed,
        born_total,
        diagnostics::escaped_milli(&samples),
        median_lag,
        p90_lag,
        diagnostics::media_classify(&samples),
    );

    // The FUEL line (world-gets-big phase 0b, spec §8 — a window, not a gate;
    // the lockstep rule: this line and FUEL_RE land in the SAME commit).
    // MEASURED fields only. Refuel fields append with the phase-1 mechanic,
    // never as zeros for a verb that does not exist yet.
    println!(
        "FUEL seed={} hauler_duty_milli={} hauler_burn_total_milli={} \
         hauler_median_leg_burn_permille={} hauler_min_tank_permille={}",
        args.seed,
        hauler_fuel.duty_milli,
        hauler_fuel.burn_total_milli,
        hauler_fuel.median_leg_burn_permille,
        hauler_fuel.min_tank_permille,
    );

    // The ASSIGN line (WHY-panel windows, 2026-06-11; a window, not a gate):
    // how many belief-scored picks were made, how often the gossip read and
    // the legacy-ring read would have picked DIFFERENTLY (media-live only;
    // flip_milli = 0 means the channel's realism never reached a decision),
    // and where the evidence counts sat on the avoidance curve (buckets
    // 0..=5 then >=6 == the 900-clamp flat region).
    if let Some(last) = samples.last() {
        let flip_milli =
            last.assign_flips_cum.saturating_mul(1000) / last.assign_decisions_cum.max(1);
        println!(
            "ASSIGN seed={} decisions={} flips={} flip_milli={} counts={:?}",
            args.seed,
            last.assign_decisions_cum,
            last.assign_flips_cum,
            flip_milli,
            last.assign_counts_cum,
        );
    }

    if let Some(path) = &args.gossip_log {
        write_gossip_log(&world, path);
    }

    if args.chronicle {
        print_chronicle(&world, args.chronicle_gossip_min_micros);
    }

    if args.assert_no_fuel_empty && fuel_empty > 0 {
        eprintln!("trophic_run: endurance window violated — {fuel_empty} FuelEmpty event(s)");
        return ExitCode::FAILURE;
    }

    if args.replay_check {
        let (_, hashes2, _, _) = match simulate(&args, None) {
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
