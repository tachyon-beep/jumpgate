"""wa2_spread_closure — WA2 spread-closure reader (spec §5 WA2).

FRAME (PDR-0006): windows, not gates.
Decay is the signal: posted spread on a (route, good) pair should fall
after delivery as the arbitrage opportunity self-eliminates. Any trend
(rising = routes opening up, flat = persistent gap, falling = closure)
is a finding.

Join order: post → accept → deliver rows per contract_id. The "post"
gossip-log row is runner-enriched (from the ContractOffered event + current
prices at log time) with spread_micros; the "deliver" row (ContractFulfilled,
A0 instrument) carries spread_at_deliver (price spread at delivery tick).
Contracts without a deliver row (still in flight) are excluded.

Usage:
    python3 python/analysis/wa2_spread_closure.py <gossip.jsonl>
"""

import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def spread_series(rows):
    """Build per-(route, good) series of completed contracts.

    Returns dict[(route, good)] = list of {contract, spread_at_post,
    spread_at_deliver, post_tick, deliver_tick} in post-tick order.
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

    result = {}
    for c, d in delivers.items():
        p = posts.get(c)
        if p is None:
            continue
        key = (p.get("route"), p.get("good"))
        if None in key:
            continue
        entry = {
            "contract": c,
            "spread_at_post": p.get("spread_micros", 0),
            "spread_at_deliver": d.get("spread_at_deliver", 0),
            "post_tick": p.get("tick", 0),
            "deliver_tick": d.get("tick", 0),
        }
        result.setdefault(key, []).append(entry)

    for key in result:
        result[key].sort(key=lambda x: x["post_tick"])
    return result


def summarize(result):
    """Per-(route, good) decay summary rows for printing."""
    out = []
    for (route, good), series in sorted(result.items()):
        if len(series) < 2:
            decaying = None  # too few data points
        else:
            first_half = series[: len(series) // 2]
            second_half = series[len(series) // 2 :]
            first_avg = sum(s["spread_at_post"] for s in first_half) // len(first_half)
            second_avg = sum(s["spread_at_post"] for s in second_half) // len(second_half)
            decaying = second_avg < first_avg
        out.append({
            "route": route,
            "good": good,
            "n_contracts": len(series),
            "first_spread": series[0]["spread_at_post"] if series else None,
            "last_spread": series[-1]["spread_at_post"] if series else None,
            "decaying": decaying,
        })
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
    args = ap.parse_args()

    rows = load(args.gossip_log)
    result = spread_series(rows)
    summary = summarize(result)

    print(
        f"WA2 spread-closure ({len(result)} route×good pairs with completed contracts) "
        "(RECORDED, never gated — PDR-0006):"
    )
    if not summary:
        print("  no completed contracts in gossip log (pre-A2 run or no bazaar traffic)")
        return

    print(f"  {'route':>5}  {'good':>4}  {'n':>4}  {'first_spread':>12}  "
          f"{'last_spread':>11}  {'decaying':>8}")
    for r in summary:
        d = str(r["decaying"]) if r["decaying"] is not None else "?"
        print(
            f"  {r['route']:>5}  {r['good']:>4}  {r['n_contracts']:>4}  "
            f"{r['first_spread']!s:>12}  {r['last_spread']!s:>11}  {d:>8}"
        )

    decaying_count = sum(1 for r in summary if r["decaying"] is True)
    flat_count = sum(1 for r in summary if r["decaying"] is False)
    pending_count = sum(1 for r in summary if r["decaying"] is None)
    print(
        f"\nWA2 reading: decaying={decaying_count} flat={flat_count} "
        f"pending={pending_count} (decaying = arbitrage arbitrages; "
        "flat = persistent gap = priced-in transport or no competition; "
        "either is a finding — owner's call)"
    )


if __name__ == "__main__":
    main()
