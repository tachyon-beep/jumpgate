"""wa3_wa5_joint — WA3 + WA5 joint reader (spec §5 WA3/WA5).

FRAME (PDR-0006): windows, not gates.
JOINT READ (panel directive): own-trade share (WA3) IS the pirate food
supply. High own-trade share → fewer crate haulers on the public board →
fewer targets → PermanentPeace masquerade. NEVER read WA5 without reading
WA3 first and checking for the prey-shrink confound on the same seeds.

WA5: verdict distribution-vs-frontier-bank, NEVER same-seed paired.
Use clean_seeds() (blind-born != PermanentPeace) before reading the
verdict mix — PermanentPeace is first in the verdict chain and overrides
cycled even when boom-bust is live (diagnostics.rs:288).

Usage:
    python3 python/analysis/wa3_wa5_joint.py <bazaar-out-dir> \\
        --frontier-dir <frontier-out-dir> [--seeds 7 11 13 ...]
"""

import argparse
import json
import pathlib
import sys
from collections import Counter

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE, RESULT_RE


SEEDS = [
    7, 11, 13, 23, 29, 31, 37, 41, 42, 43,
    47, 53, 57, 59, 61, 67, 71, 73, 99, 101,
]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def own_trade_share_milli(windows, n_haulers):
    """Fraction of (own-trade sells) / (own-trade sells + contract delivers),
    milli, FLOOR. Returns 0 if no activity.

    WA3 signal: starts near 0 (all craft broke, wage mode) and rises as
    capital accumulates (rich craft go own-trade). The joint WA5 read:
    if this rises AND PermanentPeace appears on the same seeds, that is the
    prey-shrink confound (panel warning).
    """
    total_trade = sum(w.get("trade_sold_count", 0) for w in windows)
    total_contract = sum(w.get("laden_trips", 0) for w in windows)
    denom = total_trade + total_contract
    if denom == 0:
        return 0
    return total_trade * 1000 // denom


def clean_seeds(cells):
    """CLEAN = blind-born (or baseline) verdict != PermanentPeace.

    Cells is dict[seed] -> {verdict, ...}.
    PermanentPeace is first in the verdict chain and overrides cycled
    (diagnostics.rs:288) — seeds where war ended before market dynamics
    play out contaminate the WA5 distribution.
    """
    return [s for s, c in sorted(cells.items()) if c["verdict"] != "PermanentPeace"]


def prey_shrink_confound_seeds(bazaar_cells, threshold_milli=500):
    """Seeds where own-trade share >= threshold AND verdict == PermanentPeace.

    These are the prey-shrink confound seeds: the bazaar's own-trade share
    shrank the pool of crate-hauler targets, causing PermanentPeace through
    prey depletion rather than predator extinction. RECORDED, not a gate;
    owner judges whether to tune the capitalization curve or accept the read.
    """
    return [
        s for s, c in sorted(bazaar_cells.items())
        if c.get("verdict") == "PermanentPeace"
        and c.get("own_trade_share_milli", 0) >= threshold_milli
    ]


def verdict_distributions(bazaar_bag, frontier_bag):
    """Compare verdict distributions as independent sample bags (NEVER same-seed paired).

    WA5 is a DISTRIBUTION comparison, not a per-seed comparison. The two
    bags are drawn from independent campaigns; the caller must not pair them
    by seed. Returns {bazaar: Counter, frontier: Counter}.
    """
    return {
        "bazaar": dict(Counter(bazaar_bag)),
        "frontier": dict(Counter(frontier_bag)),
    }


def load_cell_stdout(path):
    """Parse one stdout file for RESULT + META."""
    result = meta = None
    for line in path.read_text().splitlines():
        stripped = line.strip()
        m = RESULT_RE.match(stripped)
        if m:
            result = m.groupdict()
        m = META_RE.match(stripped)
        if m:
            meta = m.groupdict()
    return result, meta


def load_windows(path):
    return [r for r in load(path) if "tick" in r]


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("bazaar_dir", help="bazaar sweep output directory")
    ap.add_argument("--frontier-dir", help="frontier bank sweep directory for WA5")
    ap.add_argument("--arm", default="baseline",
                    help="stdout arm prefix (default: baseline)")
    ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    ap.add_argument("--prey-shrink-threshold-milli", type=int, default=500,
                    help="own-trade share threshold for prey-shrink confound flag")
    args = ap.parse_args()

    bazaar_dir = pathlib.Path(args.bazaar_dir)

    # --- WA3: own-trade share per seed (bazaar) ---
    print(
        "WA3+WA5 JOINT READ (panel directive): own-trade share IS the pirate "
        "food supply — read side-by-side always (PDR-0006: RECORDED, NEVER GATED)"
    )
    print()

    bazaar_cells = {}
    for seed in args.seeds:
        stdout_path = bazaar_dir / f"{args.arm}_s{seed}.stdout"
        jsonl_path = bazaar_dir / f"{args.arm}_s{seed}.jsonl"
        if not stdout_path.exists() or not jsonl_path.exists():
            continue
        result, meta = load_cell_stdout(stdout_path)
        if result is None or meta is None:
            continue
        windows = load_windows(jsonl_path)
        n_haulers = int(meta.get("haulers", 0))
        share = own_trade_share_milli(windows, n_haulers)
        bazaar_cells[seed] = {
            "verdict": result["verdict"],
            "own_trade_share_milli": share,
            "robs": int(result.get("robs", 0)),
            "trips": int(result.get("trips", 0)),
        }

    print("WA3 own-trade share per seed (bazaar, 50k ticks):")
    print(f"  {'seed':>6}  {'verdict':>20}  {'trade_share‰':>12}  "
          f"{'robs':>6}  {'trips':>6}")
    for seed, c in sorted(bazaar_cells.items()):
        print(
            f"  {seed:>6}  {c['verdict']:>20}  {c['own_trade_share_milli']:>12}  "
            f"  {c['robs']:>4}  {c['trips']:>5}"
        )

    # Prey-shrink confound check
    confound = prey_shrink_confound_seeds(bazaar_cells, args.prey_shrink_threshold_milli)
    if confound:
        print(
            f"\n  PREY-SHRINK CONFOUND WARNING (panel directive): seeds {confound} "
            f"show PermanentPeace WITH own-trade share >= "
            f"{args.prey_shrink_threshold_milli}‰ — "
            "this is NOT a finding that bazaar killed boom-bust; it means the "
            "own-trade share shrank the crate-hauler prey pool. Owner's call: "
            "tune capitalization curve, or accept read as 'prey-limited regime'."
        )
    else:
        print(
            f"\n  Prey-shrink confound (threshold {args.prey_shrink_threshold_milli}‰): "
            "none detected on these seeds."
        )

    # --- WA5: verdict distribution vs frontier bank ---
    print()
    clean = clean_seeds(bazaar_cells)
    dirty = [s for s in args.seeds if s in bazaar_cells and s not in clean]
    bazaar_bag = [bazaar_cells[s]["verdict"] for s in clean if s in bazaar_cells]
    print(
        f"WA5 trophic preservation — clean seeds (bazaar baseline != PermanentPeace): "
        f"{len(clean)}/{len(bazaar_cells)}; excluded={dirty}"
    )
    print(f"  bazaar verdict distribution (clean, 50k): {Counter(bazaar_bag)}")

    if args.frontier_dir:
        frontier_dir = pathlib.Path(args.frontier_dir)
        frontier_bag = []
        for seed in args.seeds:
            stdout_path = frontier_dir / f"baseline_s{seed}.stdout"
            if not stdout_path.exists():
                continue
            result, _ = load_cell_stdout(stdout_path)
            if result is not None and result["verdict"] != "PermanentPeace":
                frontier_bag.append(result["verdict"])
        dist = verdict_distributions(bazaar_bag, frontier_bag)
        print(f"  frontier verdict distribution (bank, 50k): {Counter(frontier_bag)}")
        print(
            f"\n  WA5 reading: distribution-vs-distribution (NEVER same-seed paired). "
            "Comparable Alive fractions = trophic dynamics preserved through "
            "the demand-mechanism swap. Divergence = the market changed piracy ecology. "
            "Either is a finding (PDR-0006 — owner's call)."
        )
        # Alive fraction comparison
        def alive_frac(bag):
            if not bag:
                return None
            return bag.count("Alive") * 1000 // len(bag)
        ba = alive_frac(bazaar_bag)
        fa = alive_frac(frontier_bag)
        print(f"  Alive‰: bazaar={ba} frontier={fa}")
    else:
        print("  (no --frontier-dir; WA5 distribution comparison skipped)")


if __name__ == "__main__":
    main()
