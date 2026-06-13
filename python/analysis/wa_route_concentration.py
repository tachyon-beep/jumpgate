"""wa_route_concentration — rung-A per-good route-concentration panel.

L4-C3 / L5-C3 fix: per-good route traffic and rob HHI beside WA5 verdict,
from the accept-row 'resource' key in the gossip-log. Route vectors have no
goods dimension today (grounding §4); this panel is entirely script-side.

Run in the SAME campaign as the WA5 threshold fit (Part 3, DL5-2) so the
open/closed margin is interpretable. The clumped-topology factory constraint
(L1-C2) means goods should travel on a small subset of routes; high HHI per
good is EXPECTED and is the design proof that clumped topology is working.
A low HHI (< ~200) means self-averaging has started — the L5-C3 warning.

Usage:
    python3 python/analysis/wa_route_concentration.py <gossip.jsonl> \\
        [--n-routes N]
"""

import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def per_good_hhi_milli(rows, n_routes):
    """Per-good occupied-route HHI (milli), from gossip-log accept rows.

    Only rows with e='accept' and a 'resource' key are counted (A0
    instrument: accept row gains resource+reward). Returns dict[good_index]
    -> HHI_milli for goods with at least one accept.
    """
    counts = {}  # good -> {route -> count}
    for row in rows:
        if row.get("e") != "accept":
            continue
        g = row.get("resource")
        r = row.get("route")
        if g is None or r is None:
            continue
        counts.setdefault(g, {}).setdefault(r, 0)
        counts[g][r] += 1

    result = {}
    for good, route_counts in counts.items():
        total = sum(route_counts.values())
        if total == 0:
            continue
        hhi = sum(c * c for c in route_counts.values()) * 1000 // (total * total)
        result[good] = hhi
    return result


def ensemble_quartiles(per_seed, good):
    """Lower-index quartiles of per-good HHI across seeds.

    per_seed: dict[seed] -> dict[good -> hhi_milli]
    Returns (q1, median, q3) or None if fewer than 2 seeds have data.
    """
    vals = sorted(v[good] for v in per_seed.values() if good in v)
    n = len(vals)
    if n < 2:
        return None
    return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="gossip-log JSONL with accept rows (resource key required)")
    ap.add_argument("--n-routes", type=int, default=90,
                    help="n_stations^2 (default 90 = 10^2 for scenario_bazaar)")
    args = ap.parse_args()

    rows = load(args.gossip_log)
    result = per_good_hhi_milli(rows, n_routes=args.n_routes)

    print(
        f"rung-A per-good route concentration (HHI, milli) "
        f"— {sum(1 for r in rows if r.get('e') == 'accept')} accept events "
        "(RECORDED, never gated — PDR-0006; L4-C3/L5-C3 fix):"
    )
    if not result:
        print(
            "  no accept rows with 'resource' key (pre-A0 run — the A0 instrument "
            "must add resource+reward to the accept row)"
        )
        return

    print(f"  {'good':>4}  {'HHI‰':>6}  reading")
    for good in sorted(result):
        hhi = result[good]
        if hhi >= 600:
            reading = "concentrated (clumped topology working)"
        elif hhi >= 200:
            reading = "moderate"
        else:
            reading = "low (self-averaging warning — L5-C3)"
        print(f"  {good:>4}  {hhi:>6}  {reading}")

    print(
        "\n  Interpretation (panel L4-C3/L5-C3): high HHI per good is EXPECTED "
        "under clumped topology — it is the design proof. Low HHI means a good "
        "is spreading over all routes (self-averaging); increase goods-topology "
        "concentration or run the DL5-2 threshold fit to check margin."
    )


if __name__ == "__main__":
    main()
