"""w4_grid - headline 20-seed x 6-arm frontier grid reader.

World-gets-big spec section 8 / 11 W4. Values are reported, never gated.
The W4 question is whether gossip-vs-ring carries value now that the world is
bigger than the news. Blind/ring born-vs-rob twins are A/A instrument controls:
the anchor is consumed only on the gossip read, so result divergence there is a
wiring bug to investigate before reading W4.
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE, RESULT_RE

ARMS = [
    "blind-born",
    "blind-rob",
    "ring-born",
    "ring-rob",
    "gossip-born",
    "gossip-rob",
]

ARM_KNOBSETS = [
    "blind-born:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "blind-rob:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "ring-born:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "ring-rob:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "gossip-born:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=false",
    "gossip-rob:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=true",
]

SEEDS = [
    7, 11, 13, 23, 29, 31, 37, 41, 42, 43,
    47, 53, 57, 59, 61, 67, 71, 73, 99, 101,
]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def load_windows(path):
    return [row for row in load(path) if "tick" in row]


def load_cell(out_dir, arm, seed):
    """One (arm, seed) cell from banked stdout plus per-window JSONL."""
    stdout_path = out_dir / f"{arm}_s{seed}.stdout"
    jsonl_path = out_dir / f"{arm}_s{seed}.jsonl"
    result = meta = None
    for line in stdout_path.read_text().splitlines():
        stripped = line.strip()
        m = RESULT_RE.match(stripped)
        if m:
            result = m.groupdict()
        m = META_RE.match(stripped)
        if m:
            meta = m.groupdict()
    if result is None:
        raise SystemExit(f"no RESULT in {stdout_path}")
    if meta is None:
        raise SystemExit(f"no META in {stdout_path}")
    return {
        "result": result,
        "meta": meta,
        "windows": load_windows(jsonl_path),
    }


def clean_seeds(cells, seeds):
    """CLEAN = blind-born verdict != PermanentPeace."""
    return [
        seed for seed in seeds
        if cells[("blind-born", seed)]["result"]["verdict"] != "PermanentPeace"
    ]


def run_value(cell):
    """Median final hauler credits and rate per laden trip for one run."""
    haulers = int(cell["meta"]["haulers"])
    credits = sorted(cell["windows"][-1]["per_craft_credits"][:haulers])
    median = credits[(len(credits) - 1) // 2]
    trips = int(cell["result"]["trips"])
    return {
        "median_credits": median,
        "laden_trips": trips,
        "credits_per_trip_milli": median * 1000 // max(trips, 1),
    }


def aa_twin_divergences(cells, seeds):
    """Return blind/ring born-vs-rob RESULT divergences by family and seed."""
    bad = []
    for family in ("blind", "ring"):
        for seed in seeds:
            born = cells.get((f"{family}-born", seed))
            rob = cells.get((f"{family}-rob", seed))
            if born is None or rob is None:
                continue
            left = born["result"]
            right = rob["result"]
            if left != right:
                keys = sorted(set(left) | set(right))
                bad.append((
                    family,
                    seed,
                    {key: (left.get(key), right.get(key)) for key in keys if left.get(key) != right.get(key)},
                ))
    return bad


def quartiles(xs):
    sorted_xs = sorted(xs)
    n = len(sorted_xs)
    return (
        sorted_xs[(n - 1) // 4],
        sorted_xs[(n - 1) // 2],
        sorted_xs[(3 * (n - 1)) // 4],
    )


def arm_table(cells, seeds, label):
    print(f"\n-- per-arm value table ({label}: {len(seeds)} seeds) --")
    print("  arm          med(median_credits)  med(cr/trip_milli)  flips/decisions(pooled)  readings")
    out = {}
    for arm in ARMS:
        values = [run_value(cells[(arm, seed)]) for seed in seeds]
        med_credits = quartiles([v["median_credits"] for v in values])[1] if values else None
        med_rate = quartiles([v["credits_per_trip_milli"] for v in values])[1] if values else None
        flips = decisions = 0
        readings = {}
        for seed in seeds:
            windows = cells[(arm, seed)]["windows"]
            if windows:
                flips += windows[-1]["assign_flips_cum"]
                decisions += windows[-1]["assign_decisions_cum"]
            verdict = cells[(arm, seed)]["result"]["verdict"]
            readings[verdict] = readings.get(verdict, 0) + 1
        out[arm] = {"med_credits": med_credits, "med_rate": med_rate}
        print(f"  {arm:<12} {med_credits!s:>19}  {med_rate!s:>18}  {flips}/{decisions:<22} {readings}")
    for anchor in ("born", "rob"):
        gossip = out[f"gossip-{anchor}"]
        ring = out[f"ring-{anchor}"]
        if gossip["med_credits"] is not None and ring["med_credits"] is not None:
            print(
                f"  W4 VALUE delta ({anchor}-anchor): gossip-ring = "
                f"{gossip['med_credits'] - ring['med_credits']} micros median "
                f"final hauler credits ({gossip['med_rate'] - ring['med_rate']} "
                "milli per laden trip) - REPORTED, NEVER GATED; registered "
                "alternative: mixing persists -> deferred dispatch-locality lever"
            )
    return out


def map_distributions(stdout_dir, pattern):
    """Rate-normalized per-run reads for one banked stdout directory."""
    rates = []
    rob_rates = []
    for path in sorted(pathlib.Path(stdout_dir).glob(pattern)):
        result = meta = None
        for line in path.read_text().splitlines():
            stripped = line.strip()
            m = RESULT_RE.match(stripped)
            if m:
                result = m.groupdict()
            m = META_RE.match(stripped)
            if m:
                meta = m.groupdict()
        jsonl_path = path.with_suffix(".jsonl")
        if result is None or meta is None or not jsonl_path.exists():
            continue
        windows = load_windows(jsonl_path)
        if not windows:
            continue
        haulers = int(meta["haulers"])
        credits = sorted(windows[-1]["per_craft_credits"][:haulers])
        median = credits[(len(credits) - 1) // 2]
        trips = max(int(result["trips"]), 1)
        rates.append(median * 1000 // trips)
        rob_rates.append(int(result["robs"]) * 1000 // trips)
    return rates, rob_rates


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("out_dir", help="grid sweep output directory")
    ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    ap.add_argument(
        "--compare",
        help="banked post-fix trophic baseline sweep directory for cross-map reads",
    )
    args = ap.parse_args()

    out_dir = pathlib.Path(args.out_dir)
    cells = {
        (arm, seed): load_cell(out_dir, arm, seed)
        for arm in ARMS
        for seed in args.seeds
    }

    print(
        "w4_grid (PDR-0006: REPORTED, NEVER GATED) - "
        f"{len(ARMS)} arms x {len(args.seeds)} seeds"
    )
    bad = aa_twin_divergences(cells, args.seeds)
    if bad:
        print("\nA/A TWIN DIVERGENCE (wiring bug - fix the instrument before reading W4):")
        for family, seed, fields in bad:
            print(f"  {family} seed={seed}: {fields}")
    else:
        print("A/A twins (blind, ring): born-vs-rob RESULT identical on every seed")

    clean = clean_seeds(cells, args.seeds)
    dirty = [seed for seed in args.seeds if seed not in clean]
    print(f"\nclean seeds (blind-born != PermanentPeace): {len(clean)}/{len(args.seeds)}; dirty={dirty}")
    arm_table(cells, clean, "CLEAN")
    arm_table(cells, args.seeds, "ALL (context)")

    if args.compare:
        frontier_rates, frontier_rob_rates = map_distributions(out_dir, "gossip-born_s*.stdout")
        trophic_rates, trophic_rob_rates = map_distributions(args.compare, "baseline_s*.stdout")
        print(
            "\n-- cross-map (rate-normalized, distribution-vs-distribution; "
            "never same-seed paired - GEO-C3) --"
        )
        print(
            "  frontier gossip-born: credits/trip milli quartiles="
            f"{quartiles(frontier_rates) if frontier_rates else None} "
            f"robs/1000trips quartiles={quartiles(frontier_rob_rates) if frontier_rob_rates else None}"
        )
        print(
            "  trophic  baseline:    credits/trip milli quartiles="
            f"{quartiles(trophic_rates) if trophic_rates else None} "
            f"robs/1000trips quartiles={quartiles(trophic_rob_rates) if trophic_rob_rates else None}"
        )


if __name__ == "__main__":
    main()
