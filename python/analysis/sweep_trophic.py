"""sweep_trophic — grid runner + aggregator for the pirates-rung lab (Task 6).

FRAME (PDR-0006): every number printed here is a designer's WINDOW for the
console observe->steer->re-observe loop — never an acceptance gate, never a
build trigger. The owner reads chronicles; this prints the evidence beside
the window.

Runs `trophic_run` over a (seeds x knob-sets) grid via subprocess, parses each
run's RESULT line, aggregates the per-window JSONL, and prints:
  * diagnosis-matrix row counts per knob set (spec section 9),
  * the per-mechanic discriminator panels: endpoint-ambush trip-phase
    histogram, purchase-desync spread, Yard-circulation treasury panel,
    population alternations, occupied-route rob concentration (HHI),
    and the endurance (FuelEmpty) count,
  * the media panels (media cut 1, Task 8.3): per-knobset MediaReading
    distribution + escaped_milli, the pooled knowledge-front lag histogram,
    the news-geography hub/backwater ratio, and — when a media-on AND a
    media-off knobset run in the SAME invocation — the value-of-information
    line (median final hauler credits per arm; for the spec section-9 VoI
    panel give BOTH arms hauler_belief_scoring=true). REPORTED, never gated.

Usage:
    python3 python/analysis/sweep_trophic.py --seeds 7 11 13 --ticks 50000 \
        --knobset baseline \
        --knobset "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000,engage_radius_au=0.05"

A knob set is "name" (no overrides) or "name:k=v,k=v,..." (each k=v becomes
a `--set`). Unknown knobs make the runner exit nonzero and the sweep stops:
a silent typo would poison a whole matrix read.

POSITIVE CONTROL (the instrument-kill disease injection, recipe revised
2026-06-11, filigree jumpgate-50c6a8a3bd): the old `reach=999 + stay=0`
recipe was neutralized by the hunger gate — FED pirates stop roaming and
camp, which is genuinely clumped risk, so the instrument correctly read
Alive and the "control" no longer injected the disease it was built to
inject. The control must make pirates PERPETUALLY HUNGRY ROAMERS:
`pirate_max_reach_au=999` (no locality) + `stay_milli=0` (no stickiness)
+ `upkeep_per_tick=200` with `grubstake_micros=2000000000` (hunger that
never lets them settle) + `engage_radius_au=0.05` (frontier geometry
equalizer: suppresses radius inflation as a false clump witness). That
equalizes risk over routes by construction and MUST read RiskEqualized;
anything else means the instrument is broken.
"""

import argparse
import json
import pathlib
import re
import subprocess
import sys
from collections import Counter

RESULT_RE = re.compile(
    r"^RESULT seed=(?P<seed>\d+) ticks=(?P<ticks>\d+) verdict=(?P<verdict>\w+) "
    r"cycled=(?P<cycled>\w+) risk_heterogeneous=(?P<hetero>\w+) "
    r"outcomes_disperse=(?P<disperse>\w+) fuel_empty=(?P<fuel_empty>\d+) "
    r"robs=(?P<robs>\d+) laden_trips=(?P<trips>\d+) purchases=(?P<purchases>\d+)$"
)

# The MEDIA line (media rung Task 8.3) — lands in the SAME commit as the
# runner's println! (the lockstep rule): exactly the anchored shape.
MEDIA_RE = re.compile(
    r"^MEDIA seed=(?P<seed>\d+) born=(?P<born>\d+) "
    r"escaped_milli=(?P<escaped_milli>\d+) median_lag=(?P<median_lag>\d+) "
    r"p90_lag=(?P<p90_lag>\d+) reading=(?P<reading>\w+)$"
)

# Instrument-format versioning (world-gets-big phase 0b): anchored lines grow
# over time. Presence is the version gate; older banked outputs parse optional
# lines as None.
META_RE = re.compile(
    r"^META seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
    r"stations=(?P<stations>\d+) haulers=(?P<haulers>\d+) "
    r"pirates_initial=(?P<pirates_initial>\d+) "
    r"station_radii_milli_au=\[(?P<radii>[0-9, ]*)\]"
    r"(?: goods=(?P<goods>\d+))?$"
)

# The FUEL line (world-gets-big phase 0b) — lands in the SAME commit as the
# runner's println! (the lockstep rule). HAULER numbers only on the anchored
# line; pirates ride the per-role JSONL tail rows. Refuel fields append with
# the mechanic, and tail groups are optional so banked pre-refuel and pre-W9
# stdout still parse.
FUEL_RE = re.compile(
    r"^FUEL seed=(?P<seed>\d+) hauler_duty_milli=(?P<duty>\d+) "
    r"hauler_burn_total_milli=(?P<burn>\d+) "
    r"hauler_median_leg_burn_permille=(?P<leg>\d+) "
    r"hauler_min_tank_permille=(?P<min_tank>\d+)"
    r"(?: refuels=(?P<refuels>\d+) refuel_spend_micros=(?P<refuel_spend_micros>-?\d+)"
    r"(?: strandings=(?P<strandings>\d+) adrift_end=(?P<adrift_end>\d+))?)?$"
)

# The BAZAAR line (rung A, scenario_bazaar; config-gated — absent from
# trophic/frontier stdout). Regex lands in the SAME commit as the Rust println!
# (lockstep rule). Optional in ANCHORED: pre-bazaar banked outputs parse as None.
BAZAAR_RE = re.compile(
    r"^BAZAAR seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
    r"exchange_treasury_micros=(?P<exchange_treasury>-?\d+) "
    r"trade_buys=(?P<trade_buys>\d+) trade_sells=(?P<trade_sells>\d+) "
    r"arb_posts=(?P<arb_posts>\d+) arb_withdrawals=(?P<arb_withdrawals>\d+)$"
)

# The EXCHANGE standing-read line (rung A, OD-2 drain monitor): the Exchange
# treasury and its per-100k-tick drain. Scenario-blind — printed every run
# (zero when the Exchange is inactive). Regex lands in the SAME commit as the
# Rust println! (lockstep rule). Optional in ANCHORED: pre-A3.7 banked outputs
# parse as None.
EXCHANGE_RE = re.compile(
    r"^EXCHANGE\s+treasury_micros=(?P<exchange_treasury_micros>-?\d+)"
    r"\s+drain_per_100k=(?P<exchange_drain_per_100k>-?\d+)$"
)

# The BAZAAR drain read (rung A, A4.6, OD-2 solvency honesty): the Exchange
# battery's consumed window (initial seeded treasury - final treasury) over the
# run. Printed only when ExchangeCfg::active. Regex lands in the SAME commit as
# the Rust println! (lockstep rule). Optional in ANCHORED: pre-A4.6 banked
# outputs and Exchange-inactive runs parse as None.
BAZAAR_DRAIN_RE = re.compile(
    r"^BAZAAR drain=(?P<exchange_drain>-?\d+) ticks=(?P<ticks>\d+)$"
)

ANCHORED = {
    "result": (True, RESULT_RE),
    "media": (True, MEDIA_RE),
    "meta": (False, META_RE),
    "fuel": (False, FUEL_RE),
    "bazaar": (False, BAZAAR_RE),   # rung A, scenario_bazaar; config-gated
    "exchange": (False, EXCHANGE_RE),  # rung A, OD-2 standing drain read
    "bazaar_drain": (False, BAZAAR_DRAIN_RE),  # rung A (A4.6), OD-2 battery drain
}

PHASE_BINS = 10  # trip-phase histogram bins over [0, 1000] milli

LAG_BINS = 10  # knowledge-front histogram bins over the pooled lag range


def parse_knobset(spec: str):
    """'name' or 'name:k=v,k=v' -> (name, [(k, v), ...])."""
    if ":" not in spec:
        return spec, []
    name, rest = spec.split(":", 1)
    pairs = []
    for kv in rest.split(","):
        k, _, v = kv.partition("=")
        if not k or not v:
            raise SystemExit(f"--knobset {spec!r}: bad override {kv!r}")
        pairs.append((k, v))
    return name, pairs


def default_knobsets():
    """Default labeled-run pair: baseline vs hungry-roamer positive control."""
    return [
        "baseline",
        "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,"
        "grubstake_micros=2000000000,engage_radius_au=0.05",
    ]


def runner_cmd(scenario, seed, ticks, jsonl, knobs):
    """The trophic_run invocation for one (arm, seed) cell.

    --scenario is passed UNCONDITIONALLY: the runner owns the
    unknown-scenario error.
    """
    cmd = [
        "cargo", "run", "-q", "-p", "jumpgate-core", "--release",
        "--example", "trophic_run", "--",
        "--scenario", scenario,
        "--seed", str(seed), "--ticks", str(ticks), "--jsonl", str(jsonl),
    ]
    for k, v in knobs:
        cmd += ["--set", f"{k}={v}"]
    return cmd


def parse_stdout(text):
    """Scan one run's stdout for anchored lines.

    Required lines must be present in every format. Optional lines absent from
    older output read None, which is the version gate for banked artifacts.
    """
    found = {key: None for key in ANCHORED}
    for line in text.splitlines():
        stripped = line.strip()
        for key, (_required, rx) in ANCHORED.items():
            m = rx.match(stripped)
            if m:
                found[key] = m.groupdict()
    return found


def run_one(args, name, knobs, seed, out_dir):
    jsonl = out_dir / f"{name}_s{seed}.jsonl"
    cmd = runner_cmd(args.scenario, seed, args.ticks, jsonl, knobs)
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit(f"run failed: {name} seed={seed}")
    # Bank the full stdout beside the JSONL: standalone grid/packet panels
    # parse META/RESULT from it, and /tmp sweep dirs need same-day capture.
    (out_dir / f"{name}_s{seed}.stdout").write_text(proc.stdout)
    parsed = parse_stdout(proc.stdout)
    for key, (required, _rx) in ANCHORED.items():
        if required and parsed[key] is None:
            sys.stderr.write(proc.stdout)
            raise SystemExit(f"no {key.upper()} line: {name} seed={seed}")
    rows = [json.loads(l) for l in jsonl.read_text().splitlines() if l.strip()]
    # JSONL version gate: window rows carry "tick"; meta and future tail rows do
    # not. Banked v1 files are all window rows, so this is a no-op there.
    parsed["windows"] = [r for r in rows if "tick" in r]
    return parsed


def occupied_hhi_milli(w):
    """HHI (milli) of robs over OCCUPIED routes for one window; None if no robs."""
    robs = [r for r, t in zip(w["per_route_robs"], w["per_route_traffic"]) if t > 0]
    total = sum(robs)
    if total == 0:
        return None
    return sum(r * r for r in robs) * 1000 // (total * total)


def alternations(series):
    """Direction changes of an integer series (the boom/bust count)."""
    alts, prev = 0, 0
    for a, b in zip(series, series[1:]):
        s = (b > a) - (b < a)
        if s != 0:
            if prev != 0 and s != prev:
                alts += 1
            prev = s
    return alts


def panel(name, runs):
    """Print the per-mechanic discriminator panels for one knob set."""
    print(f"\n=== knob set: {name} ({len(runs)} runs) ===")
    m0 = runs[0]["meta"] if runs else None
    if m0 is not None:
        print(
            f"map: scenario={m0['scenario']} stations={m0['stations']} "
            f"haulers={m0['haulers']} pirates_initial={m0['pirates_initial']} "
            f"radii_milli_au=[{m0['radii']}]"
        )
    else:
        print("map: n/a (pre-FUEL instrument format)")

    verdicts = Counter(run["result"]["verdict"] for run in runs)
    print("diagnosis-matrix rows (windows, not gates — PDR-0006):")
    for v, n in verdicts.most_common():
        print(f"  {v:<24} {n}")

    # Endurance window: FuelEmpty must be 0 on every run (spec section 6).
    fuel = [int(run["result"]["fuel_empty"]) for run in runs]
    print(f"endurance: fuel_empty per run = {fuel} (window expects all 0)")

    # FUEL window (phase 0b, spec §8): hauler duty/burn/low-water per run.
    # Version-gated: pre-FUEL banked output prints n/a, never dies.
    if all(run["fuel"] is not None for run in runs):
        print(
            "fuel (hauler): duty_milli="
            f"{[int(run['fuel']['duty']) for run in runs]} burn_total_milli="
            f"{[int(run['fuel']['burn']) for run in runs]} median_leg_burn_permille="
            f"{[int(run['fuel']['leg']) for run in runs]} min_tank_permille="
            f"{[int(run['fuel']['min_tank']) for run in runs]}"
        )
    else:
        print("fuel: n/a (pre-FUEL instrument format)")

    # Endpoint-ambush trip-phase histogram (the owner's pre-registered
    # discriminator, spec section 2: bimodal at trip endpoints).
    phases = [p for run in runs for w in run["windows"] for p in w["engagement_phase_milli"]]
    print(f"endpoint-ambush: {len(phases)} engagements; trip-phase histogram (0..1000):")
    if phases:
        bins = [0] * PHASE_BINS
        for p in phases:
            bins[min(p * PHASE_BINS // 1001, PHASE_BINS - 1)] += 1
        peak = max(bins)
        for i, n in enumerate(bins):
            bar = "#" * (0 if peak == 0 else round(40 * n / peak))
            print(f"  [{i * 100:>4}-{i * 100 + 99:>4}] {n:>5} {bar}")
        endpoint = bins[0] + bins[-1]
        print(f"  endpoint share (first+last bin): {endpoint}/{len(phases)}")

    # Purchase-desync spread: windows between the first and last escort
    # purchase (near-zero spread = the synchronization death, spec section 9).
    spreads = []
    for run in runs:
        ws = run["windows"]
        buy_windows = [i for i, w in enumerate(ws) if w["purchases_escort"] > 0]
        if buy_windows:
            spreads.append(buy_windows[-1] - buy_windows[0])
    print(f"purchase-desync: escort-purchase window spread per run = {spreads}")

    # Yard circulation: treasury bounded? monotone? (broken-flow diagnostic).
    for run in runs[:1]:
        r, ws = run["result"], run["windows"]
        ts = [w["yard_treasury_micros"] for w in ws]
        mono = all(a <= b for a, b in zip(ts, ts[1:]))
        print(
            f"yard treasury (seed {r['seed']}): first={ts[0]} max={max(ts)} "
            f"final={ts[-1]} monotone={mono}"
        )

    # Population cycle + risk-concentration evidence. MEASURED 2026-06-11
    # (labeled-run recalibration, filigree jumpgate-50c6a8a3bd): the
    # RUN-AGGREGATE HHI does NOT separate the labeled runs (raw 130-153
    # clumped vs 123-143 equalized); the calibrated instrument read is the
    # mean PER-WINDOW active-pirate-NORMALIZED HHI (clumped 2918-3498 vs
    # equalized 1472-1490) plus the slacked hot-route persistence clause
    # (diagnostics.rs). Both raw reads stay printed as context beside the
    # window, not as the instrument.
    for run in runs:
        r, ws = run["result"], run["windows"]
        act = [w["active_pirates"] for w in ws]
        alts = alternations(act)
        hhis = [h for w in ws if (h := occupied_hhi_milli(w)) is not None]
        mean_hhi = sum(hhis) // len(hhis) if hhis else None
        agg = [0] * len(ws[0]["per_route_robs"]) if ws else []
        occupied = set()
        for w in ws:
            for i, (rr, t) in enumerate(zip(w["per_route_robs"], w["per_route_traffic"])):
                agg[i] += rr
                if t > 0:
                    occupied.add(i)
        tot = sum(agg[i] for i in occupied)
        agg_hhi = (
            sum(agg[i] * agg[i] for i in occupied) * 1000 // (tot * tot) if tot else None
        )
        print(
            f"seed {r['seed']}: verdict={r['verdict']} active-alternations={alts} "
            f"robs={r['robs']} trips={r['trips']} purchases={r['purchases']} "
            f"per-window-HHI(milli)={mean_hhi} RUN-AGGREGATE-HHI(milli)={agg_hhi} "
            f"routes-robbed={sum(1 for i in occupied if agg[i] > 0)}"
        )

    media_panel(runs)


def media_panel(runs):
    """Media propagation panels for one knob set (Task 8.3; spec section 9 —
    windows, never gates)."""
    # Reading distribution (the MEDIA line's classifier, per run).
    readings = Counter(run["media"]["reading"] for run in runs)
    print("media readings (windows, not gates — PDR-0006):")
    for v, n in readings.most_common():
        print(f"  {v:<24} {n}")
    print(
        "media escaped_milli per run = "
        f"{[int(run['media']['escaped_milli']) for run in runs]}"
    )

    # Knowledge front: pooled craft first-hearing lag histogram (raw ticks).
    lags = [l for run in runs for w in run["windows"] for l in w["heard_lag_ticks"]]
    print(f"knowledge front: {len(lags)} craft hearings; lag histogram (ticks):")
    if lags:
        hi = max(lags) + 1
        width = max(1, -(-hi // LAG_BINS))  # ceil
        bins = [0] * LAG_BINS
        for l in lags:
            bins[min(l // width, LAG_BINS - 1)] += 1
        peak = max(bins)
        for i, n in enumerate(bins):
            bar = "#" * (0 if peak == 0 else round(40 * n / peak))
            print(f"  [{i * width:>6}-{(i + 1) * width - 1:>6}] {n:>5} {bar}")

    # News geography (hub/backwater): run-summed held-alert windows per
    # station, ratio max/min over stations that saw any dock traffic.
    for run in runs:
        r, ws = run["result"], run["windows"]
        if not ws or not ws[0]["per_station_alerts"]:
            continue
        n_st = len(ws[0]["per_station_alerts"])
        alerts = [sum(w["per_station_alerts"][i] for w in ws) for i in range(n_st)]
        contacts = [sum(w["per_station_contacts"][i] for w in ws) for i in range(n_st)]
        trafficked = [alerts[i] for i in range(n_st) if contacts[i] > 0]
        if trafficked:
            lo = min(trafficked)
            ratio = f"{max(trafficked) / lo:.1f}" if lo > 0 else "inf"
        else:
            ratio = "n/a"
        print(
            f"news geography (seed {r['seed']}): per-station summed alerts={alerts} "
            f"contacts={contacts} hub/backwater ratio={ratio}"
        )


def knobset_is_media_on(knobs):
    """Media-on arm: the knobset opens the gossip caps (default config is 0)."""
    return any(k == "station_gossip_slots" and int(v) > 0 for k, v in knobs)


def hauler_rows(run):
    """Hauler-row count for wallet slices.

    v2 reads META haulers=, killing the module-level mirror. v1 banked output
    keeps the old scenario_trophic count behind the version gate.
    """
    if run["meta"] is not None:
        return int(run["meta"]["haulers"])
    return 12


def voi_line(all_runs, all_knobs):
    """Value-of-information line (spec section 9): media-on vs media-off arms
    in the SAME invocation — median final hauler-row credits per arm.
    REPORTED, NEVER GATED (PDR-0006): a ~0 reading is a finding that points
    the next bet at world prices — owner's call."""
    on = [n for n in all_runs if knobset_is_media_on(all_knobs[n])]
    off = [n for n in all_runs if not knobset_is_media_on(all_knobs[n])]
    if not on or not off:
        return

    def median_final_hauler_credits(names):
        pool = []
        for n in names:
            for run in all_runs[n]:
                ws = run["windows"]
                if ws:
                    pool.extend(ws[-1]["per_craft_credits"][: hauler_rows(run)])
        pool.sort()
        return pool[(len(pool) - 1) // 2] if pool else None

    on_med = median_final_hauler_credits(on)
    off_med = median_final_hauler_credits(off)
    print(
        f"\nvalue-of-information (REPORTED, NEVER GATED — PDR-0006): "
        f"median final hauler credits media-on({'+'.join(on)})={on_med} "
        f"media-off({'+'.join(off)})={off_med}"
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--seeds", type=int, nargs="+", default=[7, 11, 13])
    ap.add_argument("--ticks", type=int, default=50_000)
    ap.add_argument(
        "--scenario",
        default="trophic",
        help="runner scenario factory (phase-2 flag): trophic | frontier | bazaar",
    )
    ap.add_argument(
        "--knobset",
        action="append",
        default=None,
        help="'name' or 'name:k=v,k=v' (repeatable). Default: baseline + the "
        "hungry-roamer positive control (reach=inf, no stickiness, upkeep "
        "that never lets pirates settle, frontier geometry equalizer; must read RiskEqualized — "
        "instrument-kill). The old reach+stay-only recipe is retired: the "
        "hunger gate made fed pirates camp (genuinely clumped, correctly "
        "Alive), so it stopped injecting the disease.",
    )
    ap.add_argument("--out", default="/tmp/sweep_trophic")
    args = ap.parse_args()

    specs = args.knobset or default_knobsets()
    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(
        "sweep_trophic (PDR-0006: windows, not gates) — "
        f"seeds={args.seeds} ticks={args.ticks} sets={specs}"
    )
    all_runs = {}
    all_knobs = {}
    for spec in specs:
        name, knobs = parse_knobset(spec)
        runs = []
        for seed in args.seeds:
            run = run_one(args, name, knobs, seed, out_dir)
            runs.append(run)
            print(
                f"  ran {name} seed={seed}: verdict={run['result']['verdict']} "
                f"robs={run['result']['robs']} fuel_empty={run['result']['fuel_empty']} "
                f"media={run['media']['reading']}"
            )
        all_runs[name] = runs
        all_knobs[name] = knobs

    for name, runs in all_runs.items():
        panel(name, runs)

    voi_line(all_runs, all_knobs)

    # The live positive control, restated wherever the default grid ran it
    # (spec section 1 instrument-kill: the hungry-roamer injection MUST read
    # RiskEqualized; if it does not, fix the INSTRUMENT before tuning
    # anything).
    if "control" in all_runs:
        n = sum(
            1 for run in all_runs["control"] if run["result"]["verdict"] == "RiskEqualized"
        )
        total = len(all_runs["control"])
        print(
            f"\npositive control (hungry roamers, reach=inf): {n}/{total} runs read "
            "RiskEqualized (expected ALL — anything else means the instrument is broken)"
        )


if __name__ == "__main__":
    main()
