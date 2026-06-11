"""haves_havenots - I1 per-hauler knowledge-horizon panel.

World-gets-big spec section 8 / W5. Every number printed here is a
designer's window for the console observe/steer/re-observe loop, never an
acceptance gate.

Joins, per hauler:
  * workplace radius: floor mean station radius (milli-AU) over endpoint
    stations of accepted contracts,
  * hearing lag: first-hearing tick minus the alert's born tick, never the
    carried rob_tick,
  * end credits: final window per_craft_credits[row].
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE

CONFOUND_TEXT = (
    "PRE-REGISTERED CONFOUND (PLAY-C3 / W5): ASSIGN is position-blind; "
    "workplace radius is an artifact of dispatch order and the per-tier "
    "capacity ladder, not a chosen home. A radius x outcome correlation may "
    "be a capacity-ladder read, not a locality read. Compare against the "
    "6-station control. REPORTED, never gated - PDR-0006."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_meta(stdout_text):
    """META line -> dict, with radii decoded to ints."""
    for line in stdout_text.splitlines():
        m = META_RE.match(line.strip())
        if m:
            d = m.groupdict()
            d["radii"] = [int(x.strip()) for x in d["radii"].split(",") if x.strip()]
            return d
    raise SystemExit("no META line in --stdout (re-run with the phase-0b+ runner)")


def workplace_radius_milli(accepts, radii, n_stations):
    """Per-hauler floor-mean endpoint-station radius in milli-AU."""
    per = {}
    for a in accepts:
        if a["route"] is None:
            continue
        fr, to = divmod(a["route"], n_stations)
        per.setdefault(a["hauler"], []).extend((radii[fr], radii[to]))
    return {c: sum(v) // len(v) for c, v in per.items()}


def hearing_lags(borns, heards):
    """Per-hauler first-hearing lags, joined on the alert's born tick."""
    born_tick = {b["alert"]: b["tick"] for b in borns}
    first = {}
    for h in heards:
        if not h["carrier"].startswith("c"):
            continue
        key = (h["carrier"], h["alert"])
        if key not in first or h["tick"] < first[key]:
            first[key] = h["tick"]
    lags = {}
    for (carrier, alert), t in sorted(first.items()):
        if alert in born_tick:
            lags.setdefault(int(carrier[1:]), []).append(t - born_tick[alert])
    return lags


def median(xs):
    s = sorted(xs)
    return s[(len(s) - 1) // 2]


def spearman(pairs):
    """Spearman rank correlation with mean ranks for ties."""
    if len(pairs) < 3:
        return None

    def ranks(vals):
        order = sorted(range(len(vals)), key=lambda i: vals[i])
        r = [0.0] * len(vals)
        i = 0
        while i < len(order):
            j = i
            while j + 1 < len(order) and vals[order[j + 1]] == vals[order[i]]:
                j += 1
            mean_rank = (i + j) / 2 + 1
            for k in range(i, j + 1):
                r[order[k]] = mean_rank
            i = j + 1
        return r

    xs, ys = zip(*pairs)
    if len(set(xs)) < 2 or len(set(ys)) < 2:
        return None
    rx, ry = ranks(list(xs)), ranks(list(ys))
    n = len(pairs)
    mx, my = sum(rx) / n, sum(ry) / n
    cov = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    vx = sum((a - mx) ** 2 for a in rx)
    vy = sum((b - my) ** 2 for b in ry)
    if vx == 0 or vy == 0:
        return None
    return cov / (vx * vy) ** 0.5


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="trophic_run --gossip-log output (JSONL)")
    ap.add_argument("--windows", required=True, help="same run's --jsonl window file")
    ap.add_argument("--stdout", required=True, help="same run's banked stdout")
    args = ap.parse_args()

    events = load(args.gossip_log)
    borns = [e for e in events if e["e"] == "born"]
    heards = [e for e in events if e["e"] == "heard"]
    accepts = [e for e in events if e["e"] == "accept"]
    windows = [r for r in load(args.windows) if "tick" in r]
    meta = parse_meta(pathlib.Path(args.stdout).read_text())
    n_stations = int(meta["stations"])
    haulers = int(meta["haulers"])
    radii = meta["radii"]

    radius = workplace_radius_milli(accepts, radii, n_stations)
    lags = hearing_lags(borns, heards)
    final_credits = windows[-1]["per_craft_credits"] if windows else []

    print(
        f"haves_havenots (PDR-0006: windows, not gates) - scenario={meta['scenario']} "
        f"seed={meta['seed']} stations={n_stations} haulers={haulers}"
    )
    print("\n-- per-hauler knowledge horizon --")
    print("  row  accepts  workplace_milli_au  heard_n  median_lag  end_credits")
    rl_pairs, rc_pairs = [], []
    for row in range(haulers):
        r = radius.get(row)
        ls = lags.get(row, [])
        cred = final_credits[row] if row < len(final_credits) else None
        med = median(ls) if ls else None
        print(
            f"  {row:>3}  {sum(1 for a in accepts if a['hauler'] == row):>7}  "
            f"{r if r is not None else '-':>18}  {len(ls):>7}  "
            f"{med if med is not None else 'never-heard':>10}  {cred}"
        )
        if r is not None and med is not None:
            rl_pairs.append((r, med))
        if r is not None and cred is not None:
            rc_pairs.append((r, cred))

    never = [row for row in range(haulers) if not lags.get(row)]
    print(f"  never-heard haulers: {never}")
    sl = spearman(rl_pairs)
    sc = spearman(rc_pairs)
    print(
        "\nspearman(workplace radius, median hearing lag)  = "
        f"{'n/a' if sl is None else f'{sl:.3f}'} over {len(rl_pairs)} haulers"
    )
    print(
        "spearman(workplace radius, end credits)        = "
        f"{'n/a' if sc is None else f'{sc:.3f}'} over {len(rc_pairs)} haulers"
    )
    print(f"\n{CONFOUND_TEXT}")


if __name__ == "__main__":
    main()
