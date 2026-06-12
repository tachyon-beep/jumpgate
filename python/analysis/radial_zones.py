"""radial_zones - I2 per-zone boom-bust / fuel-scarcity panel.

World-gets-big spec section 8 / W10. These are recorded windows, never gates.
The fuel_starve discriminator separates NoStockout, BoomBust, DeathSpiral,
Stockout, and ShortRun reads.
"""
import argparse
import json

ZONE_NAMES = ["core", "mid", "haven", "frontier"]

RATIONING_TEXT = (
    "row-order rationing (RECORDED, never gated - PDR-0006): resolve_refuels "
    "fills in dense craft-row order; under scarce stock low rows drink first, "
    "so a fill-share gradient by row is a rationing artifact, not strategy."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_zones(spec):
    return [[int(x) for x in part.split(",") if x] for part in spec.split("|")]


def zone_series(windows, zone):
    """Per-window zone aggregates over routes touching the zone."""
    out = []
    zset = set(zone)
    for w in windows:
        n = len(w["per_station_fuel_stock"])
        traffic = robs = 0
        for fr in range(n):
            for to in range(n):
                if fr in zset or to in zset:
                    traffic += w["per_route_traffic"][fr * n + to]
                    robs += w["per_route_robs"][fr * n + to]
        out.append({
            "tick": w["tick"],
            "traffic": traffic,
            "robs": robs,
            "stock": sum(w["per_station_fuel_stock"][s] for s in zone),
            "price_max": max((w["per_station_fuel_price"][s] for s in zone), default=0),
            "alerts": sum(w["per_station_alerts"][s] for s in zone)
            if w["per_station_alerts"] else 0,
        })
    return out


def fuel_starve(stock, traffic):
    """OD-4 death-spiral-vs-boom-bust discriminator."""
    if len(stock) < 4:
        return "ShortRun"
    if 0 not in stock:
        return "NoStockout"
    q = len(traffic) // 4
    quarters = [sum(traffic[i * q:(i + 1) * q]) for i in range(4)]
    if stock[-1] == 0 and quarters[3] * 4 < max(quarters):
        return "DeathSpiral"
    first_zero = stock.index(0)
    if stock[-1] > 0 and any(s > 0 for s in stock[first_zero:]):
        return "BoomBust"
    return "Stockout"


def fill_permille(ev):
    """Floor permille of remaining tank headroom filled by one refuel."""
    before, after = ev["before_permille"], ev["after_permille"]
    if before >= 1000:
        return None
    return (after - before) * 1000 // (1000 - before)


def fill_share_rows(refuels):
    """Per craft row: (row, n events, total units, median fill permille)."""
    per = {}
    for ev in refuels:
        f = fill_permille(ev)
        if f is None:
            continue
        per.setdefault(ev["craft"], {"n": 0, "units": 0, "fills": []})
        per[ev["craft"]]["n"] += 1
        per[ev["craft"]]["units"] += ev["units"]
        per[ev["craft"]]["fills"].append(f)
    rows = []
    for craft in sorted(per):
        fills = sorted(per[craft]["fills"])
        rows.append((craft, per[craft]["n"], per[craft]["units"], fills[(len(fills) - 1) // 2]))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("windows", help="trophic_run --jsonl per-window file")
    ap.add_argument("--gossip-log", help="same run's --gossip-log refuel events")
    ap.add_argument("--zones", default="0,1,2|3,4,5|6|7,8,9")
    args = ap.parse_args()

    windows = [r for r in load(args.windows) if "tick" in r]
    zones = parse_zones(args.zones)
    refuels = (
        [e for e in load(args.gossip_log) if e["e"] == "refuel"]
        if args.gossip_log else []
    )
    print(
        f"radial_zones (PDR-0006: windows, not gates) - {len(windows)} windows, "
        f"{len(refuels)} refuel events, zones={args.zones}"
    )
    for zi, zone in enumerate(zones):
        name = ZONE_NAMES[zi] if zi < len(ZONE_NAMES) else f"zone{zi}"
        series = zone_series(windows, zone)
        stock = [s["stock"] for s in series]
        traffic = [s["traffic"] for s in series]
        reading = fuel_starve(stock, traffic)
        print(
            f"\n-- zone {name} (stations {zone}) - fuel_starve={reading} "
            "(RECORDED; either answer is a finding - OD-4) --"
        )
        print("  window_close  traffic  robs  fuel_stock  price_max  alerts")
        for s in series:
            print(
                f"  {s['tick']:>12}  {s['traffic']:>7}  {s['robs']:>4}  "
                f"{s['stock']:>10}  {s['price_max']:>9}  {s['alerts']:>6}"
            )
    print("\n-- per-row refuel fill share --")
    rows = fill_share_rows(refuels)
    if not rows:
        print("  no refuel events (zero-refuel sentinel - the MEDIA precedent)")
    else:
        print("  row  refuels  units  median_fill_permille")
        for craft, n, units, med in rows:
            print(f"  {craft:>3}  {n:>7}  {units:>5}  {med:>20}")
    print(f"  {RATIONING_TEXT}")


if __name__ == "__main__":
    main()
