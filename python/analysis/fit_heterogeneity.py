"""fit_heterogeneity - labeled-run re-fit of the risk-heterogeneity
instrument's two constants (world-gets-big spec section 8; post haven-lurk-fix).

FRAME (PDR-0006): the fit output is pasted into diagnostics.rs doc + const
(provenance commit); the instrument separates TRUE-clumped from TRUE-equalized
LABELED runs. It is a measurement of the lab's own ruler, never a gate on the
game.

METHOD (the 2026-06-11 labeled-run method, diagnostics.rs:30-56):
  * per labeled run, compute mean per-window active-pirate-NORMALIZED HHI
    (milli) over occupied routes, and the hot-change excess
    (hot-route argmax changes - traffic argmax changes) using integer math
    mirroring diagnostics.rs:270-316 exactly,
  * threshold = floor midpoint of (min over clumped, max over equalized);
    slack = floor midpoint of (max clumped excess, min equalized excess),
  * held-out runs are never in the fit: they are printed with their side of
    the fitted boundary, recorded,
  * a closed margin (labels overlap) is reported as a finding; this script
    never invents a boundary.

Usage:
    python3 python/analysis/fit_heterogeneity.py \
        --clumped DIR/baseline_s*.jsonl --equalized DIR/control_s*.jsonl \
        --heldout-clumped HDIR/baseline_s*.jsonl \
        --heldout-equalized HDIR/control_s*.jsonl
"""
import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip() and '"tick"' in line]


def argmax_lowest(xs):
    """Strictly greatest positive value, ties to the lowest index."""
    best = None
    for i, x in enumerate(xs):
        if x > 0 and (best is None or x > xs[best]):
            best = i
    return best


def mean_norm_hhi_milli(windows):
    """Mean per-window active-pirate-normalized HHI over occupied routes."""
    norm_sum, robbing = 0, 0
    for w in windows:
        robs = [
            r if t > 0 else 0
            for r, t in zip(w["per_route_robs"], w["per_route_traffic"])
        ]
        total = sum(robs)
        if total == 0:
            continue
        robbing += 1
        hhi = sum(r * r for r in robs) * 1000 // (total * total)
        norm_sum += hhi * max(w["active_pirates"], 1)
    return None if robbing == 0 else norm_sum // robbing


def hot_change_excess(windows):
    """Hot-route argmax changes minus traffic argmax changes."""
    hot_changes = traffic_changes = 0
    prev_hot = prev_traffic = None
    for w in windows:
        robs = [
            r if t > 0 else 0
            for r, t in zip(w["per_route_robs"], w["per_route_traffic"])
        ]
        if sum(robs) == 0:
            continue
        hot = argmax_lowest(robs)
        if hot is not None and prev_hot is not None and hot != prev_hot:
            hot_changes += 1
        if hot is not None:
            prev_hot = hot
        tmax = argmax_lowest(w["per_route_traffic"])
        if tmax is not None and prev_traffic is not None and tmax != prev_traffic:
            traffic_changes += 1
        if tmax is not None:
            prev_traffic = tmax
    return hot_changes - traffic_changes


def fit_threshold(clumped, equalized):
    lo, hi = max(equalized), min(clumped)
    return {
        "threshold": (hi + lo) // 2,
        "clumped_min": hi,
        "equalized_max": lo,
        "margin_open": hi > lo,
    }


def fit_slack(clumped_excess, equalized_excess):
    hi, lo = max(clumped_excess), min(equalized_excess)
    return {
        "slack": (hi + lo) // 2,
        "clumped_max": hi,
        "equalized_min": lo,
        "margin_open": lo > hi,
    }


def measure(paths):
    rows = []
    for p in paths:
        ws = load(p)
        rows.append((p, mean_norm_hhi_milli(ws), hot_change_excess(ws)))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--clumped", nargs="+", required=True)
    ap.add_argument("--equalized", nargs="+", required=True)
    ap.add_argument("--heldout-clumped", nargs="*", default=[])
    ap.add_argument("--heldout-equalized", nargs="*", default=[])
    args = ap.parse_args()

    print("fit_heterogeneity (labeled-run method; PDR-0006: a ruler check, not a gate)")
    blocks = {}
    for label, paths in (
        ("clumped", args.clumped),
        ("equalized", args.equalized),
        ("heldout-clumped", args.heldout_clumped),
        ("heldout-equalized", args.heldout_equalized),
    ):
        rows = measure(paths)
        blocks[label] = rows
        for p, hhi, excess in rows:
            print(f"  {label:<18} {p}: mean_norm_hhi_milli={hhi} hot_change_excess={excess}")

    c = [h for _, h, _ in blocks["clumped"] if h is not None]
    e = [h for _, h, _ in blocks["equalized"] if h is not None]
    ce = [x for _, _, x in blocks["clumped"]]
    ee = [x for _, _, x in blocks["equalized"]]
    if not c or not e:
        raise SystemExit("a label produced no robbing windows: fit impossible; record it")
    t, s = fit_threshold(c, e), fit_slack(ce, ee)
    print(f"\nFIT threshold: {t}")
    print(f"FIT slack:     {s}")
    if not (t["margin_open"] and s["margin_open"]):
        print(
            "MARGIN CLOSED on this set; do NOT move the constants from this fit. "
            "Record the overlap, keep the current literals, and register "
            "scenario-conditional thresholds as the named deferred trigger."
        )
    for label in ("heldout-clumped", "heldout-equalized"):
        for p, hhi, excess in blocks[label]:
            side = None if hhi is None else ("clumped" if hhi >= t["threshold"] else "equalized")
            print(
                f"HELD-OUT {label} {p}: hhi={hhi} -> boundary side={side} "
                f"excess={excess} (RECORDED, never gated)"
            )


if __name__ == "__main__":
    main()
