"""media_log — gossip-log panels for the media-rung lab (media cut 1, Task 8.4).

FRAME (PDR-0006): every number printed here is a designer's WINDOW for the
console observe->steer->re-observe loop — never an acceptance gate, never a
build trigger, no kill-criterion vocabulary anywhere.

Reads ONE gossip log (`trophic_run --gossip-log PATH` — JSONL with one line
per AlertBorn "born" / GossipHeard "heard" / Robbed "rob" / ContractAccepted
"accept") and prints:
  * per-alert reach (nodes heard) by claimed-value quartile — the
    bimodal-reach panel (big news travels, small news dies),
  * the saturation window: fraction of alerts reaching > 800 permille of
    craft. Pre-registered expected band at defaults: LOW, single-digit
    permille (the cut is buying LOCALITY); record the actual,
  * the avoidance-lag panel: for each hot route (>= 1 rob), the first craft
    hearing tick vs the next ContractAccepted tick on that route — the
    event-resolution avoidance read,
  * the P(escape) analytic check: observed escape vs 1 - (1-p)^k, p from the
    hops-1 transfer P of the mean claimed value, k = mean dock edges per
    alert lifetime — REPORTED. The dock-edge denominator
    (`per_station_contacts`) lives in the runner's per-window JSONL, not the
    event log, so pass that file as --windows to enable this check.

Usage:
    python3 python/analysis/media_log.py /tmp/gossip.jsonl \
        --windows /tmp/run.jsonl
"""

import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def craft_reach(heards_for_alert):
    """Distinct CRAFT nodes that heard one alert (carriers 'c<slot>')."""
    return {h["carrier"] for h in heards_for_alert if h["carrier"].startswith("c")}


def quartile_reach_panel(borns, heard_by_alert):
    """Per-alert reach (distinct nodes heard) by claimed-value-at-birth
    quartile — the bimodal-reach panel."""
    print("\n-- per-alert reach by claimed-value quartile (bimodal-reach) --")
    ranked = sorted(borns, key=lambda b: (b["claimed"], b["alert"]))
    n = len(ranked)
    if n == 0:
        print("  no alerts born")
        return
    for q in range(4):
        lo, hi = q * n // 4, (q + 1) * n // 4
        chunk = ranked[lo:hi]
        if not chunk:
            continue
        reaches = [
            len({h["carrier"] for h in heard_by_alert.get(b["alert"], [])})
            for b in chunk
        ]
        creaches = [len(craft_reach(heard_by_alert.get(b["alert"], []))) for b in chunk]
        mean = sum(reaches) / len(reaches)
        cmean = sum(creaches) / len(creaches)
        print(
            f"  Q{q + 1} claimed [{chunk[0]['claimed']}..{chunk[-1]['claimed']}] "
            f"n={len(chunk)}: mean reach={mean:.2f} (craft {cmean:.2f}) "
            f"max={max(reaches)}"
        )


def saturation_panel(borns, heard_by_alert, accepts):
    """Fraction of alerts held by > 800 permille of craft — the saturation
    window (pre-registered expected band at defaults: LOW, single-digit
    permille; record the actual)."""
    print("\n-- saturation window --")
    craft = {h["carrier"]
             for hs in heard_by_alert.values()
             for h in hs if h["carrier"].startswith("c")}
    craft |= {f"c{a['hauler']}" for a in accepts}
    craft |= {f"c{b['hauler']}" for b in borns}
    if not borns or not craft:
        print("  no alerts or no craft observed")
        return
    bar = 0.8 * len(craft)
    saturated = sum(
        1 for b in borns if len(craft_reach(heard_by_alert.get(b["alert"], []))) > bar
    )
    permille = saturated * 1000 // len(borns)
    print(
        f"  craft observed={len(craft)}; alerts reaching >800 permille of craft: "
        f"{saturated}/{len(borns)} = {permille} permille "
        "(pre-registered band at defaults: LOW, single-digit permille — "
        "RECORDED, never gated)"
    )


def avoidance_lag_panel(robs, heard_by_alert, accepts, borns):
    """Per hot route: first craft hearing tick vs the NEXT ContractAccepted
    tick on that route (event-resolution avoidance-lag)."""
    print("\n-- avoidance lag (per hot route) --")
    hot = sorted({r["route"] for r in robs if r["route"] is not None})
    if not hot:
        print("  no robbed routes")
        return
    route_of_alert = {b["alert"]: b["route"] for b in borns}
    for route in hot:
        c_hears = sorted(
            h["tick"]
            for alert, hs in heard_by_alert.items()
            if route_of_alert.get(alert) == route
            for h in hs
            if h["carrier"].startswith("c")
        )
        if not c_hears:
            print(f"  route {route}: no craft hearing")
            continue
        first = c_hears[0]
        nxt = [a["tick"] for a in accepts if a["route"] == route and a["tick"] > first]
        if nxt:
            print(
                f"  route {route}: first craft hearing t={first}, next accept "
                f"t={min(nxt)} (lag {min(nxt) - first})"
            )
        else:
            print(f"  route {route}: first craft hearing t={first}, no later accept")


def sig_milli(claimed, floor, divisor):
    """Mirror of media.rs sig_milli: linear with a floor, clamped to 1000."""
    return max(floor, min(1000, claimed // divisor + floor))


def escape_check(borns, heard_by_alert, windows, args):
    """Observed escape vs the analytic 1-(1-p)^k (REPORTED): p = hops-1
    transfer P of the mean claimed value; k = mean dock edges at the pier
    station per alert lifetime (overlap-weighted over the runner's per-window
    `per_station_contacts`)."""
    print("\n-- P(escape) analytic check (REPORTED) --")
    if not borns:
        print("  no alerts born")
        return
    escaped = sum(
        1 for b in borns if craft_reach(heard_by_alert.get(b["alert"], []))
    )
    observed = escaped / len(borns)
    mean_claimed = sum(b["claimed"] for b in borns) // len(borns)
    sig = sig_milli(mean_claimed, args.sig_floor, args.sig_divisor)
    p1 = sig * (1000 - args.hop_loss) // 1000  # hops-1 transfer P (milli)
    print(
        f"  observed escape = {escaped}/{len(borns)} = {observed:.3f}; "
        f"mean claimed at birth = {mean_claimed}; hops-1 transfer P = {p1} milli"
    )
    if not windows:
        print("  k unavailable: pass --windows (the runner's --jsonl) for the "
              "per_station_contacts denominator")
        return
    # Pier of each alert = the station of its first 's' hearing (the deposit);
    # k = overlap-weighted contacts at that station over (deposit, deposit+W].
    width = windows[1]["tick"] - windows[0]["tick"] if len(windows) > 1 else 2000
    ks = []
    for b in borns:
        piers = [
            h for h in heard_by_alert.get(b["alert"], [])
            if h["carrier"].startswith("s")
        ]
        if not piers:
            continue
        dep = min(piers, key=lambda h: h["tick"])
        srow = int(dep["carrier"][1:])
        lo, hi = dep["tick"], dep["tick"] + args.evidence_window
        k = 0.0
        for w in windows:
            w_lo, w_hi = w["tick"] - width, w["tick"]
            overlap = max(0, min(hi, w_hi) - max(lo, w_lo))
            if overlap and srow < len(w["per_station_contacts"]):
                k += overlap / width * w["per_station_contacts"][srow]
        ks.append(k)
    if not ks:
        print("  no pier deposits found: k unavailable")
        return
    k_bar = sum(ks) / len(ks)
    predicted = 1 - (1 - p1 / 1000) ** k_bar
    print(
        f"  k (mean dock edges at the pier per alert lifetime, "
        f"W={args.evidence_window}) = {k_bar:.2f}; "
        f"analytic 1-(1-p)^k = {predicted:.3f} vs observed {observed:.3f}"
    )


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="trophic_run --gossip-log output (JSONL)")
    ap.add_argument(
        "--windows",
        help="the same run's --jsonl per-window file (enables the P(escape) "
        "dock-edge denominator)",
    )
    # MediaCfg mirrors (config.rs defaults) — override when sweeping knobs.
    ap.add_argument("--evidence-window", type=int, default=4000)
    ap.add_argument("--sig-floor", type=int, default=50)
    ap.add_argument("--sig-divisor", type=int, default=10_000)
    ap.add_argument("--hop-loss", type=int, default=150)
    args = ap.parse_args()

    events = load(args.gossip_log)
    borns = [e for e in events if e["e"] == "born"]
    robs = [e for e in events if e["e"] == "rob"]
    accepts = [e for e in events if e["e"] == "accept"]
    heard_by_alert = {}
    for e in events:
        if e["e"] == "heard":
            heard_by_alert.setdefault(e["alert"], []).append(e)
    # Instrument-format gate: runner --jsonl now may include meta/tail rows.
    # Window rows are exactly the rows with "tick"; banked older files pass
    # through unchanged.
    windows = [w for w in load(args.windows) if "tick" in w] if args.windows else None

    print(
        "media_log (PDR-0006: windows, not gates) — "
        f"born={len(borns)} heard={sum(len(v) for v in heard_by_alert.values())} "
        f"robs={len(robs)} accepts={len(accepts)}"
    )
    quartile_reach_panel(borns, heard_by_alert)
    saturation_panel(borns, heard_by_alert, accepts)
    avoidance_lag_panel(robs, heard_by_alert, accepts, borns)
    escape_check(borns, heard_by_alert, windows, args)


if __name__ == "__main__":
    main()
