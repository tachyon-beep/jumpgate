"""sweep_bazaar — rung-A WA1-5 20-seed campaign runner.

FRAME (PDR-0006): every number here is a designer's window, never a gate.
Runs the bazaar scenario over the 20 SEEDS at both run lengths (50k for WA5
bank-comparability; 100k for per-good/WA1-4 reads), aggregates the
readers (WA1 survival, WA2 spread closure, WA3+WA5 joint, WA4 tanker,
per-good route concentration), and prints the rung-A console packet.

The 50k run is bank-comparable with frontier (WA5 distribution). The 100k
run carries the per-good reads. Both use the 20-seed W4 SEEDS ladder.

Exit condition (RECORDED, NEVER GATED — PDR-0006):
  1. sha256 digest of trophic + frontier vs the A0 baseline (from A6.0).
  2. WA1-5 readings printed.
  3. First-look chronicle materials banked to runs/gag-rung-a/.
Any divergence in step 1 is a determinism break — STOP, bisect, never rationalize.

Usage:
    python3 python/analysis/sweep_bazaar.py \\
        --out runs/gag-rung-a \\
        --baseline-dir runs/gag-a6-baseline \\
        [--seeds 7 11 13] [--ticks-short 50000] [--ticks-long 100000]
"""

import argparse
import hashlib
import json
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import parse_stdout, runner_cmd
from w4_grid import SEEDS
from wa1_survival import stock_runs
from wa2_spread_closure import spread_series, summarize as wa2_summarize
from wa3_wa5_joint import (
    own_trade_share_milli,
    clean_seeds as wa3_clean_seeds,
    prey_shrink_confound_seeds,
    verdict_distributions,
    load_cell_stdout,
)
from wa4_tanker import find_tankers, first_tanker
from wa_route_concentration import per_good_hhi_milli


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def run_one_seed(scenario, seed, ticks, out_dir, arm="baseline"):
    """Run trophic_run for one (scenario, seed, ticks) cell."""
    jsonl = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.jsonl"
    gossip = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.gossip.jsonl"
    stdout_path = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.stdout"

    if stdout_path.exists() and jsonl.exists() and gossip.exists():
        # Already banked; skip re-run (idempotent sweep).
        stdout_text = stdout_path.read_text()
    else:
        cmd = runner_cmd(scenario, seed, ticks, jsonl, [])
        cmd += ["--gossip-log", str(gossip), "--chronicle"]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            sys.stderr.write(proc.stdout + proc.stderr)
            raise SystemExit(f"run failed: {scenario} seed={seed} ticks={ticks}")
        stdout_path.write_text(proc.stdout)
        stdout_text = proc.stdout

    parsed = parse_stdout(stdout_text)
    all_rows = load(jsonl)
    windows = [r for r in all_rows if "tick" in r]
    gossip_rows = load(gossip)
    return parsed, windows, gossip_rows, jsonl, gossip


def verify_digest_vs_baseline(baseline_dir, out_dir, seeds=(7, 23)):
    """Cross-branch digest: trophic + frontier vs A0 baseline.

    Compares sha256 of the behavior streams (window JSONL + gossip-log) for
    seeds 7 and 23 at 50k ticks (the A6.0 baseline cells). The stdout (.out)
    digest is intentionally NOT compared: the A4-era EXCHANGE standing-read
    line is an additive instrument that moves stdout while the behavior stream
    stays bit-identical (documented in the A0 baseline note, A5.5 clean-pass).
    Any divergence in a behavior stream is a determinism break — print and
    abort; never rationalize.
    """
    print("\n=== rung-A exit digest (A0 baseline vs HEAD; behavior streams) ===")
    ok = True
    base = pathlib.Path(baseline_dir)
    for scenario in ("trophic", "frontier"):
        for s in seeds:
            for ext in ("jsonl", "gossip.jsonl"):
                base_f = base / f"{scenario}-base-s{s}.{ext}"
                head_f = out_dir / f"digest-{scenario}_s{s}_t50000.{ext}"
                if not base_f.exists():
                    print(f"  SKIP {base_f.name} (no baseline — run A6.0 first)")
                    continue
                if not head_f.exists():
                    jsonl_h = out_dir / f"digest-{scenario}_s{s}_t50000.jsonl"
                    gossip_h = out_dir / f"digest-{scenario}_s{s}_t50000.gossip.jsonl"
                    stdout_h = out_dir / f"digest-{scenario}_s{s}_t50000.out"
                    cmd = runner_cmd(scenario, s, 50000, jsonl_h, [])
                    cmd += ["--gossip-log", str(gossip_h)]
                    proc = subprocess.run(cmd, capture_output=True, text=True)
                    if proc.returncode != 0:
                        raise SystemExit(f"digest run failed: {scenario} seed={s}")
                    stdout_h.write_text(proc.stdout)
                base_digest = sha256_file(base_f)
                head_digest = sha256_file(head_f)
                match = "OK" if base_digest == head_digest else "DIVERGE"
                print(f"  {match}  {scenario} s={s} {ext}: "
                      f"base={base_digest[:12]}... head={head_digest[:12]}...")
                if match != "OK":
                    ok = False
    if not ok:
        print("\n  DETERMINISM BREAK: one or more behavior streams diverged "
              "from the A0 baseline.")
        print("  STOP — bisect commit-by-commit. Do NOT rationalize.")
    else:
        print("  All behavior-stream digest checks pass — rung-A mechanics are "
              "behavior-equivalent on trophic and frontier (the OD-1 "
              "hash-neutrality confirmation).")
    return ok


def ensemble_quartiles_good(all_hhi, good):
    vals = sorted(v[good] for v in all_hhi.values() if good in v)
    n = len(vals)
    if n < 2:
        return None
    return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default="runs/gag-rung-a")
    ap.add_argument("--baseline-dir", default="runs/gag-a6-baseline")
    ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    ap.add_argument("--ticks-short", type=int, default=50_000,
                    help="50k for WA5 bank-comparability")
    ap.add_argument("--ticks-long", type=int, default=100_000,
                    help="100k for per-good reads (WA1-4)")
    ap.add_argument("--frontier-baseline-dir",
                    help="frontier bank sweep dir for WA5 comparison")
    args = ap.parse_args()

    out_dir = pathlib.Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)

    print(
        "sweep_bazaar rung-A exit campaign "
        f"(PDR-0006: RECORDED, NEVER GATED) — "
        f"seeds={len(args.seeds)} ticks_short={args.ticks_short} "
        f"ticks_long={args.ticks_long}"
    )

    # ---- Step 1: Behavior digest ----
    ok = verify_digest_vs_baseline(
        args.baseline_dir, out_dir, seeds=(7, 23)
    )
    if not ok:
        raise SystemExit(1)

    # ---- Step 2: 50k ensemble (WA5 bank-comparability) ----
    print("\n=== 50k ensemble (20 seeds, WA5 verdict distribution) ===")
    cells_50k = {}
    for seed in args.seeds:
        parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
            "bazaar", seed, args.ticks_short, out_dir
        )
        haulers = int(parsed["meta"]["haulers"]) if parsed.get("meta") else 0
        share = own_trade_share_milli(windows, haulers)
        cells_50k[seed] = {
            "verdict": parsed["result"]["verdict"] if parsed.get("result") else "Unknown",
            "own_trade_share_milli": share,
            "robs": int(parsed["result"].get("robs", 0)) if parsed.get("result") else 0,
            "trips": int(parsed["result"].get("trips", 0)) if parsed.get("result") else 0,
        }
        print(
            f"  seed={seed}: verdict={cells_50k[seed]['verdict']} "
            f"robs={cells_50k[seed]['robs']} trips={cells_50k[seed]['trips']} "
            f"own_trade‰={share}"
        )

    # WA3+WA5 joint
    confound = prey_shrink_confound_seeds(cells_50k, threshold_milli=500)
    if confound:
        print(f"\n  PREY-SHRINK CONFOUND WARNING: seeds {confound} — "
              "PermanentPeace with high own-trade share. See wa3_wa5_joint for detail.")
    clean = wa3_clean_seeds(cells_50k)
    bazaar_bag = [cells_50k[s]["verdict"] for s in clean]
    print(f"\nWA5 verdict distribution (clean seeds, n={len(clean)}): "
          f"{dict(sorted((v, bazaar_bag.count(v)) for v in set(bazaar_bag)))}")

    if args.frontier_baseline_dir:
        frontier_bag = []
        fdir = pathlib.Path(args.frontier_baseline_dir)
        for p in sorted(fdir.glob("baseline_s*.stdout")):
            result, _ = load_cell_stdout(p)
            if result and result["verdict"] != "PermanentPeace":
                frontier_bag.append(result["verdict"])
        verdict_distributions(bazaar_bag, frontier_bag)
        print(f"WA5 frontier distribution (bank): "
              f"{dict(sorted((v, frontier_bag.count(v)) for v in set(frontier_bag)))}")
        alive_b = bazaar_bag.count("Alive") * 1000 // max(len(bazaar_bag), 1)
        alive_f = frontier_bag.count("Alive") * 1000 // max(len(frontier_bag), 1)
        print(f"WA5 Alive‰: bazaar={alive_b} frontier={alive_f} "
              "(distribution-vs-distribution, NEVER same-seed paired)")

    # ---- Step 3: 100k ensemble (WA1-4 per-good reads) ----
    print("\n=== 100k ensemble (20 seeds, WA1-4 per-good reads) ===")
    all_hhi = {}
    tanker_total = 0
    first_tanker_global = None

    for seed in args.seeds:
        parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
            "bazaar", seed, args.ticks_long, out_dir
        )
        n_stations = int(parsed["meta"]["stations"]) if parsed.get("meta") else 10
        n_goods = int(parsed["meta"]["goods"]) if (
            parsed.get("meta") and parsed["meta"].get("goods")) else 0

        # WA1: survival
        if n_goods > 0:
            runs = stock_runs(windows, n_stations, n_goods)
            starving = [r for r in runs if r["max_zero_run"] > 0]
            print(
                f"  WA1 seed={seed}: {len(starving)}/{n_stations * n_goods} "
                f"station×good pairs had zero-stock windows"
            )

        # WA2: spread closure
        s_result = spread_series(gossip_rows)
        wa2_rows = wa2_summarize(s_result)
        decaying = sum(1 for r in wa2_rows if r["decaying"] is True)
        print(
            f"  WA2 seed={seed}: {decaying}/{len(wa2_rows)} route×good pairs "
            "show spread decay"
        )

        # WA4: tankers
        fuel_good = 1  # Fuel slot in scenario_bazaar
        refinery_set = {2, 5, 9}  # OD-3 refinery stations in scenario_bazaar
        tankers = find_tankers(gossip_rows, fuel_good=fuel_good,
                               refinery_stations=refinery_set)
        tanker_total += len(tankers)
        ft = first_tanker(tankers)
        if ft is not None:
            if first_tanker_global is None or ft["post_tick"] < first_tanker_global["post_tick"]:
                first_tanker_global = {**ft, "seed": seed}
        print(
            f"  WA4 seed={seed}: {len(tankers)} tanker events "
            f"({'first at t=' + str(ft['post_tick']) if ft else 'none'})"
        )

        # Per-good route concentration
        hhi = per_good_hhi_milli(gossip_rows, n_routes=n_stations * n_stations)
        all_hhi[seed] = hhi

    # Route concentration ensemble summary (L4-C3/L5-C3)
    if all_hhi:
        all_goods = sorted({g for h in all_hhi.values() for g in h})
        print("\nPer-good route concentration (HHI‰ quartiles, 100k, 20 seeds):")
        print(f"  {'good':>4}  {'q1':>6}  {'median':>6}  {'q3':>6}  reading")
        for g in all_goods:
            q = ensemble_quartiles_good(all_hhi, g)
            if q is None:
                continue
            q1, med, q3 = q
            reading = ("concentrated" if med >= 600
                       else "moderate" if med >= 200
                       else "LOW (self-averaging — L5-C3)")
            print(f"  {g:>4}  {q1:>6}  {med:>6}  {q3:>6}  {reading}")

    # WA4 summary
    print(f"\nWA4 emergent tankers (100k, 20 seeds): {tanker_total} total tanker events")
    if first_tanker_global:
        print(
            f"  First tanker across ensemble: seed={first_tanker_global['seed']} "
            f"contract={first_tanker_global['contract']} "
            f"to_station={first_tanker_global['to_station']} "
            f"post_tick={first_tanker_global['post_tick']} "
            "(console chronicle: 'the market fixed the fuel desert')"
        )
    else:
        print("  WA4 reading: NoTanker across ensemble — recorded as a finding.")

    print(
        "\n=== rung-A exit complete (PDR-0006: RECORDED, NEVER GATED) ===\n"
        "Judgment sessions to bank same-day:\n"
        "  1. 'the market fixed the fuel desert' (WA4 tanker arc)\n"
        "  2. 'the trader who flew too close' (WA3 mode-flip arc)\n"
        "  3. trophic preservation read (WA5 distribution vs frontier bank)"
    )


if __name__ == "__main__":
    main()
