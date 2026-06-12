"""lurk_moves - W6 breakout / landing / dwell panel.

World-gets-big spec section 8 / W6. Reads `trophic_run --gossip-log` JSONL
after the runner's `lurk_moved` event mapping. Values are console windows,
never gates.
"""
import argparse
import json
from collections import Counter, defaultdict

ZONE_NAMES = ["core", "mid", "haven", "frontier"]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_zones(spec):
    return [[int(x) for x in part.split(",") if x] for part in spec.split("|")]


def lurk_events(events):
    return [e for e in events if e.get("e") == "lurk_moved"]


def breakout_share(events):
    moves = len(events)
    breakouts = sum(1 for e in events if e["breakout"])
    return {
        "moves": moves,
        "breakouts": breakouts,
        "breakout_permille": 0 if moves == 0 else breakouts * 1000 // moves,
    }


def station_counts(events):
    return dict(sorted(Counter(e["to_station"] for e in events).items()))


def zone_counts(events, zones):
    by_station = station_counts(events)
    out = {}
    for i, zone in enumerate(zones):
        name = ZONE_NAMES[i] if i < len(ZONE_NAMES) else f"zone{i}"
        out[name] = sum(by_station.get(s, 0) for s in zone)
    return out


def dwell_ticks(events):
    by_pirate = defaultdict(list)
    for e in events:
        by_pirate[e["pirate"]].append(e["tick"])
    out = []
    for ticks in by_pirate.values():
        ticks.sort()
        out.extend(b - a for a, b in zip(ticks, ticks[1:]))
    return sorted(out)


def quartiles(xs):
    if not xs:
        return None
    s = sorted(xs)
    n = len(s)
    return (s[(n - 1) // 4], s[(n - 1) // 2], s[(3 * (n - 1)) // 4])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="trophic_run --gossip-log JSONL")
    ap.add_argument("--zones", default="0,1,2|3,4,5|6|7,8,9")
    args = ap.parse_args()

    events = lurk_events(load(args.gossip_log))
    zones = parse_zones(args.zones)
    share = breakout_share(events)
    dwell = dwell_ticks(events)
    print(
        "lurk_moves (PDR-0006: windows, not gates) - "
        f"moves={share['moves']} breakouts={share['breakouts']} "
        f"breakout_permille={share['breakout_permille']}"
    )
    print(f"landing_by_station={station_counts(events)}")
    print(f"landing_by_zone={zone_counts(events, zones)}")
    print(f"dwell_ticks_count={len(dwell)} dwell_ticks_q={quartiles(dwell)}")


if __name__ == "__main__":
    main()
