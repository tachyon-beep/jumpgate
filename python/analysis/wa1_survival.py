"""wa1_survival — WA1 survival-by-market reader (spec §5 WA1).

FRAME (PDR-0006): windows, not gates. Either answer is a finding:
- localized starvation at rim stations = expected with clumped topology
- universal starvation = the market broke
Anti-mirroring (L4-F4): the factory transport table is read from the no-tick
JSONL tail row emitted by the runner, never mirrored as a Python constant.

Usage:
    python3 python/analysis/wa1_survival.py <windows.jsonl> \\
        [--gossip-log <gossip.jsonl>] [--n-stations N] [--n-goods G]
"""

import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def _flat_stock(window, n_goods):
    """Return a flat station-major good-minor stock list from a window.

    The runner emits per_station_stock as either a flat list (synthetic /
    legacy) or a nested n_stations × n_goods matrix (the live JSONL shape).
    Both collapse to the same flat indexing here (st * n_goods + g).
    """
    stock = window.get("per_station_stock", [])
    if stock and isinstance(stock[0], (list, tuple)):
        flat = []
        for row in stock:
            flat.extend(row[:n_goods])
        return flat
    return stock


def stock_runs(windows, n_stations, n_goods):
    """Per-(station, good) max consecutive-zero-stock window run-length.

    Returns list of dicts: {station, good, max_zero_run, zero_windows, total_windows}.
    Zero windows = the consumer-starve raw count; max_zero_run = the worst
    localized drought.
    """
    results = []
    flats = [_flat_stock(w, n_goods) for w in windows]
    for st in range(n_stations):
        for g in range(n_goods):
            idx = st * n_goods + g
            cur_run = 0
            max_run = 0
            zero_count = 0
            for flat in flats:
                if idx < len(flat) and flat[idx] == 0:
                    cur_run += 1
                    zero_count += 1
                    max_run = max(max_run, cur_run)
                else:
                    cur_run = 0
            results.append({
                "station": st,
                "good": g,
                "max_zero_run": max_run,
                "zero_windows": zero_count,
                "total_windows": len(windows),
            })
    return results


def stalled_consumers(windows, deliver_rows, hauler_slots):
    """Per-craft deliver counts over the run.

    A craft with zero delivers is a stalled consumer: either broke (wage
    mode, waiting for work) or stranded. Either reading is a finding.
    Returns dict with craft_{slot}_deliver_count keys.
    """
    counts = {s: 0 for s in hauler_slots}
    for row in deliver_rows:
        if row.get("e") == "deliver":
            h = row.get("hauler")
            if h in counts:
                counts[h] += 1
    return {f"craft_{s}_deliver_count": v for s, v in counts.items()}


def read_transport_table(rows):
    """Read the factory transport table from the no-tick JSONL tail row.

    Returns the tail row dict if found, else None (anti-mirroring: L4-F4).
    The runner emits one row with e='transport_table' at run end; older
    runs without the row return None — version gate, no abort.
    """
    for row in rows:
        if row.get("e") == "transport_table":
            return row
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("windows", help="per-window JSONL file from trophic_run")
    ap.add_argument("--gossip-log", help="gossip-log JSONL with deliver rows")
    ap.add_argument("--n-stations", type=int, required=True)
    ap.add_argument("--n-goods", type=int, required=True)
    args = ap.parse_args()

    all_rows = load(args.windows)
    windows = [r for r in all_rows if "tick" in r]
    gossip = load(args.gossip_log) if args.gossip_log else []

    transport = read_transport_table(all_rows + gossip)
    if transport is None:
        print("WA1 transport table: absent (pre-A0 run — anti-mirroring: L4-F4 not yet wired)")
    else:
        print(f"WA1 transport table: {transport}")

    # Hauler slots: derive from per_craft_role if present (role 1 = hauler),
    # else assume all crafts in per_craft_credits are haulers.
    hauler_slots = []
    if windows and "per_craft_role" in windows[0]:
        for slot, role in enumerate(windows[0]["per_craft_role"]):
            if role == 1:
                hauler_slots.append(slot)
    elif windows and "per_craft_credits" in windows[0]:
        # Fallback: no role info. Use META haulers count if available.
        hauler_slots = list(range(len(windows[0]["per_craft_credits"])))

    deliver_rows = [r for r in gossip if r.get("e") == "deliver"]
    stall = stalled_consumers(windows, deliver_rows, hauler_slots)
    zero_delivers = sum(1 for v in stall.values() if v == 0)
    total_haulers = len(hauler_slots)
    print(
        f"WA1 stalled consumers (zero delivers over run): "
        f"{zero_delivers}/{total_haulers} haulers "
        "(RECORDED, never gated — PDR-0006; zero = no deliver events or pre-A0)"
    )

    runs = stock_runs(windows, args.n_stations, args.n_goods)
    print(
        f"\nWA1 survival-by-market ({len(windows)} windows, "
        f"{args.n_stations} stations × {args.n_goods} goods) "
        "(RECORDED, never gated — PDR-0006):"
    )
    print(f"  {'station':>7}  {'good':>4}  {'max_zero_run':>12}  "
          f"{'zero_windows':>12}  {'total':>5}")
    for r in runs:
        flag = " <-- STARVATION" if r["max_zero_run"] > 0 else ""
        print(
            f"  {r['station']:>7}  {r['good']:>4}  {r['max_zero_run']:>12}  "
            f"{r['zero_windows']:>12}  {r['total_windows']:>5}{flag}"
        )

    # Summary: either answer is a finding
    starving = [r for r in runs if r["max_zero_run"] > 0]
    if not starving:
        print("\nWA1 reading: NoStarvation — all goods at all stations held stock "
              "above zero in every window (RECORDED; finding: market feeds the world)")
    else:
        stations_hit = {r["station"] for r in starving}
        all_stations = set(range(args.n_stations))
        rim = max(all_stations)
        rim_only = stations_hit <= {rim, rim - 1}
        if rim_only:
            print(f"\nWA1 reading: RimLocalized — starvation confined to "
                  f"rim stations {sorted(stations_hit)} (RECORDED; "
                  "finding: market feeds core; rim wants supply or a lane)")
        else:
            print(f"\nWA1 reading: Universal — starvation at stations "
                  f"{sorted(stations_hit)} (RECORDED; finding: market broke or "
                  "topology mismatch — owner's call)")


if __name__ == "__main__":
    main()
