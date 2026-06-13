"""wa4_tanker — WA4 emergent-tanker reader (spec §5 WA4).

FRAME (PDR-0006): windows, not gates.
Signal: fuel packages posted to non-refinery stations, with zero
fuel-specific dispatch code. The tanker is emergent because the poster
(the Exchange corp) computes the same spread-clearing trigger for Fuel as
for any other good — the refinery-to-rim price gradient is what makes the
economics work.

The read: gossip-log post rows where good == fuel_good_index AND
to_station ∉ refinery_stations, joined to accept + deliver rows for
completion confirmation. The fuel_good_index is read from the META goods=
tail field (A0 instrument), defaulting to 1 (Fuel slot in scenario_bazaar).

Usage:
    python3 python/analysis/wa4_tanker.py <gossip.jsonl> \\
        [--fuel-good 1] [--refinery-stations 2 5 9]
"""

import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def find_tankers(rows, fuel_good, refinery_stations):
    """Return completed fuel-package deliveries to non-refinery stations.

    A 'tanker event' = a contract where:
      - the 'post' row has good == fuel_good
      - the destination station is not a refinery
      - the contract has both an 'accept' and a 'deliver' row (completed)

    Returns list of dicts: {contract, post_tick, accept_tick, deliver_tick,
    to_station, hauler}.
    """
    posts = {}
    accepts = {}
    delivers = {}
    for row in rows:
        e = row.get("e")
        c = row.get("contract")
        if c is None:
            continue
        if e == "post":
            posts[c] = row
        elif e == "accept":
            accepts[c] = row
        elif e == "deliver":
            delivers[c] = row

    result = []
    for c, p in posts.items():
        if p.get("good") != fuel_good:
            continue
        if p.get("to_station") in refinery_stations:
            continue
        d = delivers.get(c)
        if d is None:
            continue  # in-flight, not confirmed
        a = accepts.get(c)
        result.append({
            "contract": c,
            "post_tick": p.get("tick", 0),
            "accept_tick": a["tick"] if a else None,
            "deliver_tick": d.get("tick", 0),
            "to_station": p.get("to_station"),
            "hauler": a["hauler"] if a else None,
            "route": p.get("route"),
        })

    result.sort(key=lambda x: x["post_tick"])
    return result


def first_tanker(tankers):
    """The first (earliest post_tick) confirmed tanker event."""
    if not tankers:
        return None
    return min(tankers, key=lambda x: x["post_tick"])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
    ap.add_argument("--fuel-good", type=int, default=1,
                    help="good index for Fuel (default 1; read from META goods= tail "
                         "when the A0 instrument is present)")
    ap.add_argument("--refinery-stations", type=int, nargs="+", default=[2, 5, 9],
                    help="station indices that are refineries in scenario_bazaar "
                         "(default: 2 5 9; read from META when the A0 instrument "
                         "carries station roles)")
    args = ap.parse_args()

    rows = load(args.gossip_log)
    refinery_set = set(args.refinery_stations)

    tankers = find_tankers(rows, fuel_good=args.fuel_good,
                           refinery_stations=refinery_set)

    print(
        f"WA4 emergent tankers (fuel good={args.fuel_good}, "
        f"refinery stations={sorted(refinery_set)}) "
        "(RECORDED, never gated — PDR-0006):"
    )
    if not tankers:
        print(
            "  WA4 reading: NoTanker — no completed fuel packages to non-refinery "
            "stations observed. Either the fuel price gradient is too flat to clear "
            "the arbitrage trigger, or too few ticks. Recorded as a finding: "
            "the tanker is the WA4 test of price-driven emergence — its absence "
            "is equally informative (PDR-0006)."
        )
        return

    first = first_tanker(tankers)
    print(
        f"  WA4 reading: Tanker — {len(tankers)} confirmed fuel packages to "
        f"non-refinery stations."
    )
    print(
        f"  First tanker: contract={first['contract']} "
        f"post_tick={first['post_tick']} accept_tick={first['accept_tick']} "
        f"deliver_tick={first['deliver_tick']} "
        f"to_station={first['to_station']} hauler={first['hauler']}"
    )
    print(
        "  (zero fuel-specific dispatch code — pure price-driven emergence; "
        "the console chronicle arc starts here)"
    )
    print()
    print(f"  {'contract':>10}  {'post_tick':>9}  {'deliver_tick':>12}  "
          f"{'to_station':>10}  {'hauler':>6}  {'route':>5}")
    for t in tankers:
        print(
            f"  {t['contract']:>10}  {t['post_tick']:>9}  {t['deliver_tick']:>12}  "
            f"  {t['to_station']:>9}  {t['hauler']!s:>6}  {t['route']!s:>5}"
        )


if __name__ == "__main__":
    main()
