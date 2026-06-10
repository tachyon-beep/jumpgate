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
    and the endurance (FuelEmpty) count.

Usage:
    python3 python/analysis/sweep_trophic.py --seeds 7 11 13 --ticks 50000 \
        --knobset baseline \
        --knobset "control:pirate_max_reach_au=999,stay_milli=0"

A knob set is "name" (no overrides) or "name:k=v,k=v,..." (each k=v becomes
a `--set`). Unknown knobs make the runner exit nonzero and the sweep stops:
a silent typo would poison a whole matrix read.
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

PHASE_BINS = 10  # trip-phase histogram bins over [0, 1000] milli


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


def run_one(args, name, knobs, seed, out_dir):
    jsonl = out_dir / f"{name}_s{seed}.jsonl"
    cmd = [
        "cargo", "run", "-q", "-p", "jumpgate-core", "--release",
        "--example", "trophic_run", "--",
        "--seed", str(seed), "--ticks", str(args.ticks), "--jsonl", str(jsonl),
    ]
    for k, v in knobs:
        cmd += ["--set", f"{k}={v}"]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit(f"run failed: {name} seed={seed}")
    result = None
    for line in proc.stdout.splitlines():
        m = RESULT_RE.match(line.strip())
        if m:
            result = {k: v for k, v in m.groupdict().items()}
    if result is None:
        sys.stderr.write(proc.stdout)
        raise SystemExit(f"no RESULT line: {name} seed={seed}")
    windows = [json.loads(l) for l in jsonl.read_text().splitlines() if l.strip()]
    return result, windows


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
    verdicts = Counter(r["verdict"] for r, _ in runs)
    print("diagnosis-matrix rows (windows, not gates — PDR-0006):")
    for v, n in verdicts.most_common():
        print(f"  {v:<24} {n}")

    # Endurance window: FuelEmpty must be 0 on every run (spec section 6).
    fuel = [int(r["fuel_empty"]) for r, _ in runs]
    print(f"endurance: fuel_empty per run = {fuel} (window expects all 0)")

    # Endpoint-ambush trip-phase histogram (the owner's pre-registered
    # discriminator, spec section 2: bimodal at trip endpoints).
    phases = [p for _, ws in runs for w in ws for p in w["engagement_phase_milli"]]
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
    for _, ws in runs:
        buy_windows = [i for i, w in enumerate(ws) if w["purchases_escort"] > 0]
        if buy_windows:
            spreads.append(buy_windows[-1] - buy_windows[0])
    print(f"purchase-desync: escort-purchase window spread per run = {spreads}")

    # Yard circulation: treasury bounded? monotone? (broken-flow diagnostic).
    for (r, ws) in runs[:1]:
        ts = [w["yard_treasury_micros"] for w in ws]
        mono = all(a <= b for a, b in zip(ts, ts[1:]))
        print(
            f"yard treasury (seed {r['seed']}): first={ts[0]} max={max(ts)} "
            f"final={ts[-1]} monotone={mono}"
        )

    # Population cycle + risk-concentration evidence. The RUN-AGGREGATE HHI is
    # the sparsity-robust read (per-window HHI saturates at 1-3 robs/window —
    # the live-control finding); print both so the owner sees the instrument
    # beside the world.
    for (r, ws) in runs:
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


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--seeds", type=int, nargs="+", default=[7, 11, 13])
    ap.add_argument("--ticks", type=int, default=50_000)
    ap.add_argument(
        "--knobset",
        action="append",
        default=None,
        help="'name' or 'name:k=v,k=v' (repeatable). Default: baseline + the "
        "reach=inf positive control (must read RiskEqualized — instrument-kill).",
    )
    ap.add_argument("--out", default="/tmp/sweep_trophic")
    args = ap.parse_args()

    specs = args.knobset or [
        "baseline",
        "control:pirate_max_reach_au=999,stay_milli=0",
    ]
    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(
        "sweep_trophic (PDR-0006: windows, not gates) — "
        f"seeds={args.seeds} ticks={args.ticks} sets={specs}"
    )
    all_runs = {}
    for spec in specs:
        name, knobs = parse_knobset(spec)
        runs = []
        for seed in args.seeds:
            result, windows = run_one(args, name, knobs, seed, out_dir)
            runs.append((result, windows))
            print(
                f"  ran {name} seed={seed}: verdict={result['verdict']} "
                f"robs={result['robs']} fuel_empty={result['fuel_empty']}"
            )
        all_runs[name] = runs

    for name, runs in all_runs.items():
        panel(name, runs)

    # The live positive control, restated wherever the default grid ran it
    # (spec section 1 instrument-kill: reach=inf MUST read RiskEqualized;
    # if it does not, fix the INSTRUMENT before tuning anything).
    if "control" in all_runs:
        n = sum(1 for r, _ in all_runs["control"] if r["verdict"] == "RiskEqualized")
        total = len(all_runs["control"])
        print(
            f"\npositive control (reach=inf): {n}/{total} runs read RiskEqualized "
            "(expected ALL — anything else means the instrument is broken)"
        )


if __name__ == "__main__":
    main()
