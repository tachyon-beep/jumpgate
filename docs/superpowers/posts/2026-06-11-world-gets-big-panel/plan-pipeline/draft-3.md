# Phase 3 — science + console (world-gets-big, spec §8 / §9 phase 3 / §11)

Plan section for `/home/john/jumpgate` @ `e7e490e` (writing-plans discipline).
Spec: `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` §8, §9
(phase 3), §11. Frame: PDR-0006 — every number below is a WINDOW, recorded
and never gated; the only pass/fail surfaces in this phase are determinism
and unit tests.

**Cross-phase dependencies (must have landed before Phase 3 starts):**
phase 0a haven-lurk fix; phase 0b `META` line + role-split `FUEL` line +
version-gated `META_RE`/`FUEL_RE` in `sweep_trophic.py` + the banked
post-fix 20-seed trophic baseline; phase 1 refuel verb
(`RefuelCfg{lot_mass, corp_index}`, `pending_refuel`, `run_refuel_policies`
at 1c3b, `resolve_refuels` at 1d2, `Refueled`/`ContractFailed`); phase 2
`scenario_frontier`, the runner `--scenario` flag, `LurkMoved`,
`fuel_capacity_scale` calibration arm, and the TrophicSample additive fields
`per_station_lurking_pirates, pirates_commuting, pirates_at_haven,
per_station_fuel_stock, per_station_fuel_price, refuels, refuel_units,
refuel_spend_micros`. Where this section imports phase-0b symbols
(`META_RE`, its group names) the builder aligns spellings with what phase 0b
actually landed before running the failing test.

House rules encoded in every task: commits via `git commit -F -` heredoc
ending with the exact trailer line
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`;
`git add` explicit paths only (never `-A`, never `.`); never stage `runs/`
or anything under `/tmp`; reward surfaces untouched; measured constants and
golden literals are PASTED from instrument output, never invented by the
planner or the builder's head.

---

### Task 3.1: sweep `--scenario` passthrough + per-cell stdout banking (the grid/fit substrate)

The fit (3.2), the I1/I2 control runs (3.3/3.5), and the headline grid (3.6)
all drive `trophic_run` through `sweep_trophic.py`, which today hardcodes
the runner invocation without `--scenario` (sweep_trophic.py:91-96) and
throws the stdout away after parsing (`run_one`, :89-118). The standalone
panels (3.6, the packet) need the banked stdout for `META`/`RESULT`.

**Files**
- Create: `/home/john/jumpgate/python/tests/test_sweep_cli.py`
- Modify: `/home/john/jumpgate/python/analysis/sweep_trophic.py`
  (`run_one` :89-118, argparse in `main` :300-312)

NOTE: spec §9 phase 2 gives the *runner* the `--scenario` flag. If the
phase-2 section also added a sweep passthrough, keep its semantics and land
only the `runner_cmd` extraction + stdout banking from this task; the test
below must pass either way.

- [ ] **Step 1: failing test — `runner_cmd` carries scenario, seed, knobs**

```python
"""CLI-seam tests for sweep_trophic (world-gets-big phase 3.1)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import sweep_trophic


def test_runner_cmd_carries_scenario_seed_and_knobs():
    cmd = sweep_trophic.runner_cmd(
        "frontier", 7, 50_000, "/tmp/x.jsonl", [("pirate_max_reach_au", "999")]
    )
    assert cmd[cmd.index("--scenario") + 1] == "frontier"
    assert cmd[cmd.index("--seed") + 1] == "7"
    assert cmd[cmd.index("--ticks") + 1] == "50000"
    assert cmd[cmd.index("--set") + 1] == "pirate_max_reach_au=999"


def test_runner_cmd_trophic_is_still_explicit():
    # The flag is passed UNCONDITIONALLY: the runner owns the
    # unknown-scenario error (a silent default would hide a typo'd arm).
    cmd = sweep_trophic.runner_cmd("trophic", 11, 1_000, "/tmp/y.jsonl", [])
    assert cmd[cmd.index("--scenario") + 1] == "trophic"
    assert "--set" not in cmd
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_cli.py`
Expected failure: `AttributeError: module 'sweep_trophic' has no attribute 'runner_cmd'`.

- [ ] **Step 2: implement — extract `runner_cmd`, add `--scenario`, bank stdout**

In `sweep_trophic.py`, above `run_one` (after `parse_knobset`), add:

```python
def runner_cmd(scenario, seed, ticks, jsonl, knobs):
    """The trophic_run invocation for one (arm, seed) cell. --scenario is
    passed UNCONDITIONALLY — the runner owns the unknown-scenario error."""
    cmd = [
        "cargo", "run", "-q", "-p", "jumpgate-core", "--release",
        "--example", "trophic_run", "--",
        "--scenario", scenario,
        "--seed", str(seed), "--ticks", str(ticks), "--jsonl", str(jsonl),
    ]
    for k, v in knobs:
        cmd += ["--set", f"{k}={v}"]
    return cmd
```

Rewrite the head of `run_one` (keep everything from `proc = subprocess.run`
parsing downward byte-identical except the inserted banking line):

```python
def run_one(args, name, knobs, seed, out_dir):
    jsonl = out_dir / f"{name}_s{seed}.jsonl"
    cmd = runner_cmd(args.scenario, seed, args.ticks, jsonl, knobs)
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit(f"run failed: {name} seed={seed}")
    # Bank the full stdout beside the JSONL: the standalone grid/packet
    # panels (w4_grid.py, the console packet) parse META/RESULT from it,
    # and /tmp sweep dirs get banked same-day (the capture practice).
    (out_dir / f"{name}_s{seed}.stdout").write_text(proc.stdout)
```

In `main()`'s argparse block (beside `--ticks`):

```python
    ap.add_argument(
        "--scenario",
        default="trophic",
        help="runner scenario factory (phase-2 flag): trophic | frontier",
    )
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_cli.py`
→ `2 passed`.

- [ ] **Step 4: commit**

```bash
git add python/analysis/sweep_trophic.py python/tests/test_sweep_cli.py
git commit -F - <<'EOF'
lab(sweep): --scenario passthrough + per-cell stdout banking

Phase-3 substrate for the dual-map fit and the 20-seed x 6-arm grid:
runner_cmd extracted (testable seam), --scenario forwarded
unconditionally, and each cell's stdout banked beside its JSONL so the
standalone panels and the console packet can parse META/RESULT later.
Windows, not gates (PDR-0006).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.2: dual-map HHI/slack re-fit post-leak-fix (labeled-run method, held-out seeds)

The heterogeneity instrument's two fitted constants —
`HHI_NORM_MIN_MILLI = 2204` (diagnostics.rs:44, doc :30-43) and
`HOT_PERSISTENCE_SLACK_CHANGES = 3` (diagnostics.rs:57, doc :46-56) — were
fitted on the 2026-06-11 labeled set, which the haven-lurk leak (spec §6,
fixed phase 0a) contaminates. Spec §8 requires a re-fit on BOTH maps with
the same labeled-run method. Constants are RE-DERIVED from measured runs and
pasted from the fit instrument's output (the golden discipline applied to
calibration constants) — never nudged. Neither constant is config-hashed:
zero goldens move.

**Pre-registered seed split (named now, before any frontier run):**
FIT seeds `7 23 42 99` (the historical labeled four); HELD-OUT seeds
`11 31 57 101`. Labels: `baseline` knobset = TRUE-clumped;
hungry-roamer `control` knobset = TRUE-equalized (recipe verbatim from
sweep_trophic.py:31-40).

**Files**
- Create: `/home/john/jumpgate/python/analysis/fit_heterogeneity.py`
- Create: `/home/john/jumpgate/python/tests/test_fit_heterogeneity.py`
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs`
  (constants + docs :30-57; classifier synthetics :667-810 only if the
  re-fitted boundary moves across them)

- [ ] **Step 1: failing test — the fit math mirrors diagnostics.rs and reproduces the documented 2026-06-11 fit**

`python/tests/test_fit_heterogeneity.py`:

```python
"""Pins fit_heterogeneity's integer math against diagnostics.rs (:270-316)
and against the DOCUMENTED 2026-06-11 fit (2204 / 3) — the mirror is the
method's instrument, so it ships with synthetics that would catch it lying
(the diagnostics.rs:896-899 house rule, applied lab-side)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import fit_heterogeneity as fh


def w(robs, traffic, active):
    return {
        "per_route_robs": robs,
        "per_route_traffic": traffic,
        "active_pirates": active,
    }


def test_mean_norm_hhi_masks_unoccupied_routes_and_normalizes_by_active():
    # Window 1: route 1 has robs but NO traffic -> masked to 0; occupied
    # robs [4, 0], HHI = 16*1000 // 16 = 1000 milli; x active.max(1)=3
    # -> 3000. Window 2: zero robs -> not a robbing window.
    ws = [w([4, 5], [1, 0], 3), w([0, 0], [1, 1], 9)]
    assert fh.mean_norm_hhi_milli(ws) == 3000


def test_mean_norm_hhi_floor_division_over_robbing_windows():
    # ([3,1] occupied: HHI=(9+1)*1000//16=625, x2=1250) and
    # ([1,1]: HHI=2*1000//4=500, x1=500) -> (1250+500)//2 = 875.
    ws = [w([3, 1], [1, 2], 2), w([1, 1], [3, 1], 1)]
    assert fh.mean_norm_hhi_milli(ws) == 875


def test_mean_norm_hhi_none_when_no_robbing_window():
    assert fh.mean_norm_hhi_milli([w([0, 0], [1, 1], 5)]) is None


def test_hot_change_excess_argmax_ties_to_lowest_index():
    # hot argmax: [2,1]->0, [1,2]->1, [2,2]->0 (tie -> LOWEST) = 2 changes;
    # traffic argmax constant at 0 -> 0 changes; excess = +2.
    ws = [w([2, 1], [1, 1], 1), w([1, 2], [1, 1], 1), w([2, 2], [1, 1], 1)]
    assert fh.hot_change_excess(ws) == 2


def test_fit_reproduces_the_documented_2026_06_11_constants():
    # The diagnostics.rs:30-56 doc tables ARE the regression fixture: the
    # method must reproduce threshold 2204 and slack 3 from them.
    t = fh.fit_threshold([3070, 2962, 2918, 3498], [1490, 1472])
    assert t == {
        "threshold": 2204,
        "clumped_min": 2918,
        "equalized_max": 1490,
        "margin_open": True,
    }
    s = fh.fit_slack([1, -1, -3, -6], [6, 5])
    assert s == {
        "slack": 3,
        "clumped_max": 1,
        "equalized_min": 5,
        "margin_open": True,
    }


def test_fit_reports_a_closed_margin_instead_of_inventing_a_boundary():
    t = fh.fit_threshold([1500, 1600], [1700])
    assert t["margin_open"] is False
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_fit_heterogeneity.py`
Expected failure: `ModuleNotFoundError: No module named 'fit_heterogeneity'`.

- [ ] **Step 2: implement `fit_heterogeneity.py`**

```python
"""fit_heterogeneity — labeled-run re-fit of the risk-heterogeneity
instrument's two constants (world-gets-big spec §8; post haven-lurk-fix).

FRAME (PDR-0006): the fit OUTPUT is pasted into diagnostics.rs doc + const
(provenance commit); the instrument separates TRUE-clumped from
TRUE-equalized LABELED runs — it is a measurement of the lab's own ruler,
never a gate on the game.

METHOD (the 2026-06-11 labeled-run method, diagnostics.rs:30-56):
  * per labeled run, compute mean per-window active-pirate-NORMALIZED HHI
    (milli) over OCCUPIED routes, and the hot-change excess
    (hot-route argmax changes - traffic argmax changes) — integer math
    mirroring diagnostics.rs:270-316 exactly (pinned by
    python/tests/test_fit_heterogeneity.py),
  * threshold = FLOOR midpoint of (min over clumped, max over equalized);
    slack = FLOOR midpoint of (max clumped excess, min equalized excess),
  * HELD-OUT runs are never in the fit: they are printed with their side
    of the fitted boundary, RECORDED,
  * a CLOSED margin (labels overlap) is reported as a finding — the script
    never invents a boundary.

Usage (per map, then pooled):
    python3 python/analysis/fit_heterogeneity.py \
        --clumped DIR/baseline_s*.jsonl --equalized DIR/control_s*.jsonl \
        --heldout-clumped HDIR/baseline_s*.jsonl \
        --heldout-equalized HDIR/control_s*.jsonl
"""
import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def argmax_lowest(xs):
    """Strictly greatest POSITIVE value, ties to the LOWEST index; None when
    everything is zero/empty (mirror of diagnostics.rs argmax_lowest)."""
    best = None
    for i, x in enumerate(xs):
        if x > 0 and (best is None or x > xs[best]):
            best = i
    return best


def mean_norm_hhi_milli(windows):
    """Mean per-window active-pirate-normalized HHI (milli) over OCCUPIED
    routes; None if no window robbed (mirror of diagnostics.rs:270-316)."""
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
    """hot-route argmax changes minus traffic argmax changes, counted over
    robbing windows only (the diagnostics.rs persistence clause inputs)."""
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
        raise SystemExit("a label produced no robbing windows: fit impossible — record it")
    t, s = fit_threshold(c, e), fit_slack(ce, ee)
    print(f"\nFIT threshold: {t}")
    print(f"FIT slack:     {s}")
    if not (t["margin_open"] and s["margin_open"]):
        print(
            "MARGIN CLOSED on this set — the labels overlap; do NOT move the "
            "constants from this fit. Record the overlap (a finding about the "
            "instrument on this map), keep the current literals, and register "
            "scenario-conditional thresholds as the named deferred trigger."
        )
    for label in ("heldout-clumped", "heldout-equalized"):
        for p, hhi, excess in blocks[label]:
            side = None if hhi is None else ("clumped" if hhi >= t["threshold"] else "equalized")
            print(f"HELD-OUT {label} {p}: hhi={hhi} -> boundary side={side} "
                  f"excess={excess} (RECORDED, never gated)")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_fit_heterogeneity.py`
→ `6 passed`.

- [ ] **Step 4: commit the instrument (before any field run)**

```bash
git add python/analysis/fit_heterogeneity.py python/tests/test_fit_heterogeneity.py
git commit -F - <<'EOF'
lab(fit): labeled-run re-fit instrument for HHI/slack (dual-map, spec s8)

Mirrors diagnostics.rs:270-316 integer math (pinned by synthetics that
reproduce the documented 2026-06-11 fit: 2204 / 3), fits FLOOR midpoints
over labeled runs, validates held-out seeds as recorded readings, and
refuses to invent a boundary when the margin is closed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 5: PROCEDURE — frontier positive control FIRST (ordering step, not a ship gate)**

Run the hungry-roamer disease injection on `scenario_frontier` and read its
restatement BEFORE any other frontier reading is taken or recorded:

```bash
mkdir -p /tmp/wgb-fit
python3 python/analysis/sweep_trophic.py --scenario frontier \
  --seeds 7 23 42 99 --ticks 50000 \
  --knobset "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000" \
  --out /tmp/wgb-fit/frontier-control-first \
  | tee /tmp/wgb-fit/frontier-control-first/sweep_stdout.txt
```

Read the trailing `positive control (hungry roamers, ...)` line. Expected:
`4/4 runs read RiskEqualized`. If NOT: the instrument is broken on the new
map — stop the fit, diagnose the instrument (this is the one place the
procedure ORDER is mandatory), and record what was seen. This step gates the
*procedure order*, never the shipping of any mechanic.

- [ ] **Step 6: run the labeled ensembles on both maps (50k ticks, default knobsets = baseline + control)**

```bash
python3 python/analysis/sweep_trophic.py --scenario trophic  --seeds 7 23 42 99   --ticks 50000 --out /tmp/wgb-fit/trophic-fit   | tee /tmp/wgb-fit/trophic-fit/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario trophic  --seeds 11 31 57 101 --ticks 50000 --out /tmp/wgb-fit/trophic-held  | tee /tmp/wgb-fit/trophic-held/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario frontier --seeds 7 23 42 99   --ticks 50000 --out /tmp/wgb-fit/frontier-fit  | tee /tmp/wgb-fit/frontier-fit/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario frontier --seeds 11 31 57 101 --ticks 50000 --out /tmp/wgb-fit/frontier-held | tee /tmp/wgb-fit/frontier-held/sweep_stdout.txt
```

- [ ] **Step 7: fit per map, then pooled (the adopted constants come from the POOLED fit)**

```bash
python3 python/analysis/fit_heterogeneity.py \
  --clumped /tmp/wgb-fit/trophic-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/trophic-fit/control_s*.jsonl \
  --heldout-clumped /tmp/wgb-fit/trophic-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/trophic-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_trophic.txt

python3 python/analysis/fit_heterogeneity.py \
  --clumped /tmp/wgb-fit/frontier-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/frontier-fit/control_s*.jsonl \
  --heldout-clumped /tmp/wgb-fit/frontier-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/frontier-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_frontier.txt

python3 python/analysis/fit_heterogeneity.py \
  --clumped  /tmp/wgb-fit/trophic-fit/baseline_s*.jsonl /tmp/wgb-fit/frontier-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/trophic-fit/control_s*.jsonl /tmp/wgb-fit/frontier-fit/control_s*.jsonl \
  --heldout-clumped  /tmp/wgb-fit/trophic-held/baseline_s*.jsonl /tmp/wgb-fit/frontier-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/trophic-held/control_s*.jsonl /tmp/wgb-fit/frontier-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_pooled.txt
```

The pooled fit's midpoint of the INTERSECTED margin is the adopted pair. If
the pooled (or either per-map) margin is CLOSED, the script says so: do NOT
move the constants; record the overlap in the console packet (Task 3.7) and
register scenario-conditional thresholds as a named deferred trigger — the
existing literals stay with a doc note. The build ships either way.

- [ ] **Step 8: re-derive the constants in `diagnostics.rs` (paste-only, single-cause commit)**

Edit `crates/jumpgate-core/src/diagnostics.rs:30-57`. The two literals and
the doc tables are PASTED from `/tmp/wgb-fit/fit_pooled.txt` (with the
per-map tables from `fit_trophic.txt` / `fit_frontier.txt`) — never typed
from memory, never nudged. Shape (the `«…»` slots are paste targets, the
golden discipline applied to calibration constants):

```rust
/// Minimum mean active-pirate-normalized HHI (milli) of robberies over
/// OCCUPIED routes for "risk heterogeneous". 1000 milli ≈ "each active pirate
/// owns one route"; even spread over m ≫ k routes reads ≈ 1000·k/m.
///
/// RE-FITTED «date» (dual-map labeled-run method, world-gets-big spec §8;
/// CAUSE: the phase-0a haven-lurk-leak fix changed the band the 2026-06-11
/// fit was measured on, and scenario_frontier joins the instrument's
/// domain; previous literal 2204). Fit seeds 7/23/42/99, held-out
/// 11/31/57/101, 50k ticks, baseline = TRUE-clumped vs hungry-roamer
/// control = TRUE-equalized; constants = FLOOR midpoint of the POOLED
/// intersected margin (python/analysis/fit_heterogeneity.py):
///   trophic  clumped «paste» vs equalized «paste»
///   frontier clumped «paste» vs equalized «paste»
///   pooled threshold = «paste»; held-out sides: «paste summary»
pub const HHI_NORM_MIN_MILLI: u64 = «paste pooled threshold»;
```

and the same treatment for `HOT_PERSISTENCE_SLACK_CHANGES` (previous
literal 3, excess tables pasted, slack = pasted pooled midpoint).

- [ ] **Step 9: re-check the classifier synthetics against the moved boundary**

```bash
cargo test -p jumpgate-core --lib diagnostics
```

Expected: all pass. If `cycling_heterogeneous_reads_alive` (:716),
`cycling_equalized_reads_risk_equalized` (:735), or
`sparse_clumped_minority_routes_read_heterogeneous` (:798) fail because the
re-fitted boundary crossed a synthetic's values: REDESIGN the synthetic to
sit inside the NEW measured band (e.g. concentrate `cycling_heterogeneous`'s
`robs_by_route` onto fewer routes — recompute its mean normalized HHI by the
Step-1 formula and place it above the new threshold with the same margin
the old builder had over 2204; the builder doc comment quotes the new band)
— never nudge the fitted constant back toward a synthetic. The synthetic is
the liar's regression fixture (the diagnostics.rs:896-899 house rule), so
its values must trace to the new fit table.

- [ ] **Step 10: full verification + single-cause commit**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/diagnostics.rs
git commit -F - <<'EOF'
lab(diagnostics): re-fit HHI/slack on the dual-map post-leak-fix labeled set

Single cause: phase-0a haven-lurk-leak fix invalidated the 2026-06-11
labeled fit (old literals 2204 / 3); re-derived by the same labeled-run
method over trophic+frontier with held-out seeds 11/31/57/101 (fit tables
pasted in the doc comments from fit_heterogeneity.py output). Frontier
positive control read RiskEqualized 4/4 BEFORE any frontier reading was
recorded. No config/state hash touched; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

(If Step 7 reported a closed margin: the commit instead carries ONLY the doc
note recording the overlap + the deferred-trigger registration, with the
literals unchanged — adjust the message to say so.)

---

### Task 3.3: I1 — `haves_havenots.py` (workplace radius × hearing lag × end credits)

Spec §8 panels, W5. Per-hauler join of: workplace radius (mean endpoint
station radius over the hauler's accepted contracts — gossip-log `accept`
events, route decoded as `fr*n + tr` per `diagnostics::route_of`), hearing
lag (first hearing tick minus the alert's **BORN tick** — never the carried
`rob_tick`, which the inflation/lying channel corrupts), and end credits
(final window `per_craft_credits`). The PLAY-C3 confound is pre-registered
IN THE PANEL'S EMITTED TEXT, and the 6-station control is actually run.
Follows the `media_log.py` standalone-panel idioms (positional JSONL,
`load()` helper, "REPORTED, never gated" framing).

**Files**
- Create: `/home/john/jumpgate/python/analysis/haves_havenots.py`
- Create: `/home/john/jumpgate/python/tests/test_haves_havenots.py`

- [ ] **Step 1: failing test — born-tick join, radius math, rank correlation**

`python/tests/test_haves_havenots.py`:

```python
"""I1 panel math pins (world-gets-big spec §8 / W5)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import haves_havenots as hh


def test_hearing_lag_joins_on_born_tick_never_rob_tick():
    # The heard event carries a CORRUPTED rob_tick (inflation/lying stays
    # armed-dormant, spec §12): the lag must be 150-100=50, never touch 999.
    borns = [{"e": "born", "tick": 100, "alert": 1, "route": 0, "claimed": 5}]
    heards = [{
        "e": "heard", "tick": 150, "alert": 1, "carrier": "c3",
        "rob_tick": 999, "hops": 2, "claimed": 9,
    }]
    assert hh.hearing_lags(borns, heards) == {3: [50]}


def test_hearing_lag_takes_first_hearing_and_ignores_station_carriers():
    borns = [{"e": "born", "tick": 10, "alert": 7, "route": 0, "claimed": 5}]
    heards = [
        {"e": "heard", "tick": 90, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 40, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 20, "alert": 7, "carrier": "s0", "rob_tick": 10, "hops": 1, "claimed": 5},
    ]
    assert hh.hearing_lags(borns, heards) == {1: [30]}


def test_workplace_radius_is_floor_mean_of_accept_endpoints():
    # n=10 stations; route 29 = fr 2 -> to 9. radii in milli-AU.
    radii = [350, 444, 564, 716, 909, 1154, 1466, 1861, 2363, 3000]
    accepts = [
        {"e": "accept", "tick": 5, "route": 29, "hauler": 4},
        {"e": "accept", "tick": 9, "route": 29, "hauler": 4},
    ]
    # mean(564, 3000, 564, 3000) = 1782 exactly (FLOOR-safe case).
    assert hh.workplace_radius_milli(accepts, radii, 10) == {4: 1782}


def test_workplace_radius_skips_null_routes():
    assert hh.workplace_radius_milli(
        [{"e": "accept", "tick": 5, "route": None, "hauler": 0}], [1, 2], 2
    ) == {}


def test_spearman_perfect_monotone_and_tie_handling():
    assert hh.spearman([(1, 10), (2, 20), (3, 30)]) == 1.0
    assert hh.spearman([(1, 30), (2, 20), (3, 10)]) == -1.0
    assert hh.spearman([(1, 1), (1, 1), (1, 1)]) is None  # constant margin
    r = hh.spearman([(1, 10), (2, 10), (3, 30), (4, 40)])  # tied ys
    assert r is not None and 0.9 < r <= 1.0
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_haves_havenots.py`
Expected failure: `ModuleNotFoundError: No module named 'haves_havenots'`.

- [ ] **Step 2: implement `haves_havenots.py`**

```python
"""haves_havenots — I1 per-hauler knowledge-horizon panel (world-gets-big
spec §8 / W5).

FRAME (PDR-0006): every number printed here is a designer's WINDOW for the
console observe->steer->re-observe loop — never an acceptance gate.

Joins, PER HAULER:
  * workplace radius — FLOOR mean station radius (milli-AU) over the
    endpoint stations of the hauler's ACCEPTED contracts (gossip-log
    "accept" events; route = fr*n + tr per diagnostics::route_of),
  * hearing lag — first-hearing tick minus the alert's BORN tick. The join
    is on the BORN event tick, NEVER the carried rob_tick: the
    inflation/lying channel corrupts rob_tick and its consumers stay
    armed-dormant (spec §12),
  * end credits — final window per_craft_credits[row].

PRE-REGISTERED CONFOUND (PLAY-C3 / W5) — printed with every reading:
ASSIGN is position-blind, so a hauler's "workplace" is an emergent artifact
of dispatch order and the per-tier capacity ladder (qty 5/10/15 vs hull
capacity), never a chosen home. A radius x outcome correlation may be a
capacity-ladder read, not a locality read. The 6-station control run
(same panel on scenario_trophic, near-uniform radii) is the null map.

Usage:
    python3 python/analysis/haves_havenots.py GOSSIP_LOG \
        --windows RUN_JSONL --stdout RUN_STDOUT
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE  # phase-0b lockstep regex for the META line

CONFOUND_TEXT = (
    "PRE-REGISTERED CONFOUND (PLAY-C3 / W5): ASSIGN is position-blind — "
    "workplace radius is an artifact of dispatch order and the per-tier "
    "capacity ladder (qty 5/10/15 vs hull capacity), not a chosen home; a "
    "radius x outcome correlation may be a capacity-ladder read, not a "
    "locality read. Compare against the 6-station control (the null map). "
    "REPORTED, never gated — PDR-0006."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_meta(stdout_text):
    """META line -> dict (radii decoded to a list of ints); SystemExit when
    absent — a frontier run without META is a pre-phase-0b binary."""
    for line in stdout_text.splitlines():
        m = META_RE.match(line.strip())
        if m:
            d = m.groupdict()
            d["radii"] = [int(x) for x in d["radii"].strip("[]").split(",") if x]
            return d
    raise SystemExit("no META line in --stdout (re-run with the phase-0b+ runner)")


def workplace_radius_milli(accepts, radii, n_stations):
    """Per-hauler FLOOR-mean endpoint-station radius (milli-AU)."""
    per = {}
    for a in accepts:
        if a["route"] is None:
            continue
        fr, to = divmod(a["route"], n_stations)
        per.setdefault(a["hauler"], []).extend((radii[fr], radii[to]))
    return {c: sum(v) // len(v) for c, v in per.items()}


def hearing_lags(borns, heards):
    """Per-hauler first-hearing lags, joined on the alert's BORN tick (never
    the carried rob_tick — corruptible)."""
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
    """Spearman rank correlation (mean ranks for ties); None when < 3 pairs
    or either margin is constant."""
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
    ap.add_argument("--windows", required=True, help="the same run's --jsonl per-window file")
    ap.add_argument("--stdout", required=True, help="the same run's banked stdout (META source)")
    args = ap.parse_args()

    events = load(args.gossip_log)
    borns = [e for e in events if e["e"] == "born"]
    heards = [e for e in events if e["e"] == "heard"]
    accepts = [e for e in events if e["e"] == "accept"]
    windows = load(args.windows)
    meta = parse_meta(pathlib.Path(args.stdout).read_text())
    n_stations = int(meta["stations"])
    haulers = int(meta["haulers"])
    radii = meta["radii"]

    radius = workplace_radius_milli(accepts, radii, n_stations)
    lags = hearing_lags(borns, heards)
    final_credits = windows[-1]["per_craft_credits"] if windows else []

    print(
        f"haves_havenots (PDR-0006: windows, not gates) — scenario={meta['scenario']} "
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
        f"\nspearman(workplace radius, median hearing lag)  = "
        f"{'n/a' if sl is None else f'{sl:.3f}'} over {len(rl_pairs)} haulers"
    )
    print(
        f"spearman(workplace radius, end credits)        = "
        f"{'n/a' if sc is None else f'{sc:.3f}'} over {len(rc_pairs)} haulers"
    )
    print(f"\n{CONFOUND_TEXT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_haves_havenots.py`
→ `5 passed`.

- [ ] **Step 4: PROCEDURE — run the 6-station control (the null map), then the frontier read**

The control runs FIRST and is recorded beside every frontier I1 reading
(media on via knobs — `MediaCfg` live-start caps 16/8):

```bash
mkdir -p /tmp/wgb-i1
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario trophic --seed 7 --ticks 50000 \
  --set station_gossip_slots=16 --set craft_gossip_slots=8 \
  --jsonl /tmp/wgb-i1/trophic_s7.jsonl --gossip-log /tmp/wgb-i1/trophic_s7.gossip.jsonl \
  > /tmp/wgb-i1/trophic_s7.stdout
python3 python/analysis/haves_havenots.py /tmp/wgb-i1/trophic_s7.gossip.jsonl \
  --windows /tmp/wgb-i1/trophic_s7.jsonl --stdout /tmp/wgb-i1/trophic_s7.stdout \
  | tee /tmp/wgb-i1/i1_trophic_control.txt

cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 50000 \
  --set station_gossip_slots=16 --set craft_gossip_slots=8 \
  --jsonl /tmp/wgb-i1/frontier_s7.jsonl --gossip-log /tmp/wgb-i1/frontier_s7.gossip.jsonl \
  > /tmp/wgb-i1/frontier_s7.stdout
python3 python/analysis/haves_havenots.py /tmp/wgb-i1/frontier_s7.gossip.jsonl \
  --windows /tmp/wgb-i1/frontier_s7.jsonl --stdout /tmp/wgb-i1/frontier_s7.stdout \
  | tee /tmp/wgb-i1/i1_frontier_s7.txt
```

Expected control shape (recorded, not gated): trophic radii near-uniform →
tiny radius spread, correlations ≈ n/a or ≈ 0. Both outputs are banked by
Task 3.7.

- [ ] **Step 5: commit**

```bash
git add python/analysis/haves_havenots.py python/tests/test_haves_havenots.py
git commit -F - <<'EOF'
lab(panel): I1 haves_havenots — workplace radius x hearing lag x credits

Per-hauler knowledge-horizon join (spec s8/W5): radius from accept-event
endpoints, lag joined on the alert's BORN tick (never the corruptible
rob_tick), end credits from the final window. PLAY-C3 position-blind-
dispatch confound pre-registered in the panel's emitted text; the
6-station control is the recorded null map. Windows, never gates.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.4: coverage re-denominated to contract-endpoint stations (scenario-conditional)

Spec §8: the dark haven (station 6, no contract endpoint) makes the
CommonKnowledge all-stations coverage check
(`stations_with_news as usize == per_station_alerts.len()`,
diagnostics.rs:426) structurally unsatisfiable on frontier. Re-denominate to
contract-endpoint stations, derived from the run's own config
(`RunConfig.contracts: Vec<ContractInit>` with
`from_station_index`/`to_station_index`, config.rs:126-135, 426). On
`scenario_trophic` every station is an endpoint (sources {0,1,2}, dests
{3,4,5}, fuel sinks {0,1,2} — scenario.rs:186-211), so banked trophic MEDIA
readings are unchanged; that invariance is pinned by test.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs`
  (`media_classify` :400-431; new `endpoint_station_rows`; media tests
  :929-1000)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs`
  (`simulate` cfg block :113-136, MEDIA println :410-418, replay arm
  destructure :456-462)

- [ ] **Step 1: failing test — endpoint denomination + trophic invariance**

Add to the diagnostics test module (beside the media tests, after
`media_localized_is_the_alive_reading`):

```rust
    #[test]
    fn media_common_knowledge_denominates_on_contract_endpoint_stations() {
        // Frontier shape: a DARK station (no contract endpoint — the haven)
        // never holds news; coverage must be satisfiable over the ENDPOINT
        // set (spec §8). Escape 23/24 = 958‰ ≥ 950; alerts on rows 0,1 only.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 0],
                )
            })
            .collect();
        // All-stations denominator (the old read): the dark row blocks it.
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::Localized,
            "dark row counted -> coverage unsatisfiable"
        );
        // Endpoint denominator: row 2 is no contract endpoint.
        assert_eq!(
            media_classify(&samples, &[true, true, false]),
            MediaReading::CommonKnowledge,
            "endpoint coverage complete -> CommonKnowledge"
        );
    }

    #[test]
    fn media_empty_endpoint_set_never_reads_common_knowledge() {
        // A degenerate all-dark mask must not make coverage vacuously true.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 1],
                )
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[false, false, false]),
            MediaReading::Localized
        );
    }

    #[test]
    fn endpoint_station_rows_trophic_is_all_true() {
        // scenario_trophic: sources {0,1,2}, dests {3,4,5}, fuel sinks
        // {0,1,2} — every station is a contract endpoint, so the banked
        // trophic MEDIA readings are unchanged by the re-denomination.
        let cfg = crate::scenario::scenario_trophic(7);
        assert_eq!(endpoint_station_rows(&cfg), vec![true; 6]);
    }

    #[test]
    fn endpoint_station_rows_marks_a_contractless_station_dark() {
        let mut cfg = crate::scenario::scenario_trophic(7);
        cfg.stations.push(crate::config::StationInit {
            body_index: 1,
            initial_stock: [0, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        });
        let rows = endpoint_station_rows(&cfg);
        assert_eq!(rows.len(), 7);
        assert!(!rows[6], "no contract touches the new station -> dark");
    }
```

Update the six existing `media_*` tests (:929-1000) mechanically: every
`media_classify(&samples)` becomes
`media_classify(&samples, &[true, true, true])` (their synthetic
`per_station_alerts` are 3-wide; the all-true mask is the old behavior).

Run: `cargo test -p jumpgate-core --lib media_`
Expected failure: compile error
`error[E0061]: this function takes 1 argument but 2 arguments were supplied`
(at the first updated call site) and
`cannot find function 'endpoint_station_rows' in this scope`.

- [ ] **Step 2: implement in `diagnostics.rs`**

New pub fn (beside `route_of`):

```rust
/// Contract-endpoint station rows derived from the run's own config: row i
/// is `true` iff some seeded contract has it as `from_station_index` or
/// `to_station_index`. The coverage denominator for `media_classify` —
/// scenario-conditional by construction (spec §8: the dark haven makes
/// all-stations coverage structurally unsatisfiable on frontier; on
/// scenario_trophic every station is an endpoint, so readings are
/// unchanged — pinned by test).
pub fn endpoint_station_rows(cfg: &crate::config::RunConfig) -> Vec<bool> {
    let mut rows = vec![false; cfg.stations.len()];
    for k in &cfg.contracts {
        if let Some(r) = rows.get_mut(k.from_station_index) {
            *r = true;
        }
        if let Some(r) = rows.get_mut(k.to_station_index) {
            *r = true;
        }
    }
    rows
}
```

`media_classify` signature + CommonKnowledge clause (NoMedia / NewsDesert /
StaleEcho arms untouched; `stations_with_news` STAYS sampled and in JSONL —
the additive law — the classifier just stops consuming it):

```rust
/// Classify a windowed run's MEDIA propagation field (spec §9). Pure over
/// the samples, like `classify`; precedence is the listed reading order.
/// `endpoint_rows` is the coverage denominator (world-gets-big spec §8):
/// CommonKnowledge requires news at every CONTRACT-ENDPOINT station, not
/// every station — the dark haven is structurally newsless.
pub fn media_classify(samples: &[TrophicSample], endpoint_rows: &[bool]) -> MediaReading {
```

and replace the CommonKnowledge `if` (:423-429) with:

```rust
    let last = samples.last().expect("non-empty: born > 0");
    let endpoints: Vec<usize> = endpoint_rows
        .iter()
        .enumerate()
        .filter_map(|(i, &e)| e.then_some(i))
        .collect();
    if escaped_milli(samples) >= COMMON_KNOWLEDGE_ESCAPE_MILLI
        && !last.per_station_alerts.is_empty()
        && !endpoints.is_empty()
        && endpoints
            .iter()
            .all(|&i| last.per_station_alerts.get(i).copied().unwrap_or(0) > 0)
    {
        return MediaReading::CommonKnowledge;
    }
    MediaReading::Localized
```

- [ ] **Step 3: thread the mask through `trophic_run.rs`**

`simulate` already owns the cfg; return the mask rather than rebuilding cfg
in `main` (self-contained regardless of the phase-2 `--scenario` dispatch
shape). After the `apply_knob` loop and before `World::reset`:

```rust
    let endpoint_rows = diagnostics::endpoint_station_rows(&cfg);
```

extend `simulate`'s return tuple with `endpoint_rows` (last position), and
update both call sites:

```rust
    let (samples, hashes, world, endpoint_rows) = match simulate(&args, Some(&mut jsonl_sink)) {
```

(replay arm: `let (_, hashes2, _, _) = match simulate(&args, None)`), then
the MEDIA println (:417) passes it:

```rust
        diagnostics::media_classify(&samples, &endpoint_rows),
```

(Exact tuple shape: match whatever `simulate` returns after phases 0b–2;
the mask is appended, nothing else moves.)

- [ ] **Step 4: run + expected pass, full suite, clippy**

```bash
cargo test -p jumpgate-core --lib media_     # all media_* + 2 endpoint tests pass
cargo test -p jumpgate-core --lib endpoint_  # 2 passed
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Also verify the trophic invariance live (recorded): re-run one banked
baseline seed and diff its MEDIA line against the phase-0b banked stdout —
expected byte-identical.

- [ ] **Step 5: commit**

```bash
git add crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
lab(media): coverage denominated over contract-endpoint stations

CommonKnowledge's coverage check counted ALL stations; the frontier dark
haven (no contract endpoint, spec s3) made it structurally unsatisfiable.
media_classify now takes an endpoint mask derived from the run's own
config (endpoint_station_rows over RunConfig.contracts). scenario_trophic
is all-endpoints, so banked trophic readings are unchanged (pinned by
test). stations_with_news stays sampled — JSONL keys untouched.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.5: I2 — radial-zone panel, `fuel_starve` discriminator, per-row refuel fill share

Spec §8 / W10. Two parts: (a) a tiny runner addition — `Refueled` events
written to the `--gossip-log` JSONL (the run's only machine-readable
per-event surface; the lockstep rule: emit + reader land in this one
commit); (b) `radial_zones.py`, the standalone zone panel.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs`
  (`write_gossip_log` match :190-236)
- Create: `/home/john/jumpgate/python/analysis/radial_zones.py`
- Create: `/home/john/jumpgate/python/tests/test_radial_zones.py`

- [ ] **Step 1: failing test — discriminator readings, fill-share FLOOR math, zone series**

`python/tests/test_radial_zones.py`:

```python
"""I2 radial-zone panel pins (world-gets-big spec §8 / W10)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import radial_zones as rz


def test_fuel_starve_readings_cover_the_pre_registered_table():
    # NoStockout: never dry.
    assert rz.fuel_starve([5, 4, 3, 2], [9, 9, 9, 9]) == "NoStockout"
    # BoomBust: dry then RECOVERED and ends wet.
    assert rz.fuel_starve([5, 0, 3, 4], [9, 5, 7, 8]) == "BoomBust"
    # DeathSpiral: ends dry AND final-quarter traffic collapsed
    # (quarters [9,9,1,0]; 0*4 < 9).
    assert rz.fuel_starve([5, 2, 0, 0], [9, 9, 1, 0]) == "DeathSpiral"
    # Stockout (residual): ends dry but the lane is still alive.
    assert rz.fuel_starve([5, 0, 0, 0], [5, 5, 5, 5]) == "Stockout"
    # ShortRun: quarters undefined under 4 windows.
    assert rz.fuel_starve([5, 0, 1], [1, 1, 1]) == "ShortRun"


def test_fill_share_is_floor_permille_of_remaining_headroom():
    # before 250 -> after 850: (850-250)*1000 // (1000-250) = 800.
    ev = {"e": "refuel", "tick": 9, "craft": 4, "station": 2,
          "units": 12, "price_micros": 7, "before_permille": 250,
          "after_permille": 850}
    assert rz.fill_permille(ev) == 800
    # Full tank before (defensive: resolve_refuels skips these) -> None.
    full = dict(ev, before_permille=1000, after_permille=1000)
    assert rz.fill_permille(full) is None


def test_zone_series_sums_stock_and_routes_touching_the_zone():
    w = {
        "tick": 2000,
        "per_route_robs": [0, 1, 0, 2],          # n=2 stations
        "per_route_traffic": [3, 1, 0, 5],
        "per_station_fuel_stock": [7, 11],
        "per_station_fuel_price": [5000, 9000],
        "per_station_alerts": [1, 0],
    }
    z = rz.zone_series([w], [1])  # zone = station 1 only
    # routes touching station 1: 0->1 (idx 1), 1->0 (idx 2), 1->1 (idx 3).
    assert z == [{
        "tick": 2000, "traffic": 6, "robs": 3,
        "stock": 11, "price_max": 9000, "alerts": 0,
    }]


def test_parse_zones():
    assert rz.parse_zones("0,1,2|3,4,5|6|7,8,9") == [
        [0, 1, 2], [3, 4, 5], [6], [7, 8, 9]
    ]
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_radial_zones.py`
Expected failure: `ModuleNotFoundError: No module named 'radial_zones'`.

- [ ] **Step 2: the `Refueled` gossip-log arm (runner)**

In `write_gossip_log`'s match (trophic_run.rs, after the `ContractAccepted`
arm, before the `_ => continue` catch-all — the catch-all is exactly why
this arm must be added explicitly, the chronicle_subject lesson):

```rust
            EventKind::Refueled {
                craft,
                station,
                units,
                price_micros,
                tank_before_permille,
                tank_after_permille,
            } => serde_json::json!({
                "e": "refuel", "tick": e.tick.0, "craft": craft.slot,
                "station": station.slot, "units": units,
                "price_micros": price_micros,
                "before_permille": tank_before_permille,
                "after_permille": tank_after_permille,
            }),
```

Update the fn doc comment's event list to name "Refueled (`refuel`)". This
is additive: trophic logs (RefuelCfg default-off) simply contain no
`refuel` lines, and `media_log.py` filters by `e` so it is untouched.

- [ ] **Step 3: implement `radial_zones.py`**

```python
"""radial_zones — I2 per-zone boom-bust / fuel-scarcity panel
(world-gets-big spec §8 / W10).

FRAME (PDR-0006): windows, never gates. The fuel_starve discriminator is
the OD-4 measurement — EITHER answer is a finding:
  NoStockout  — zone fuel stock never hit 0,
  BoomBust    — stock hit 0, RECOVERED (>0 later), and ends >0 — the
                scarcity arc cycles like the predation arc,
  DeathSpiral — ends at 0 AND final-quarter zone traffic*4 < the peak
                quarter — the lane died with the fuel,
  Stockout    — residual: ends dry but the lane is still alive (scarcity
                without collapse, incl. recovered-then-dry tails),
  ShortRun    — fewer than 4 windows (quarters undefined).

Fill share (per craft row): resolve_refuels fills in DENSE CRAFT-ROW ORDER —
under scarce stock low rows drink first, so a fill-share gradient by row is
a RATIONING ARTIFACT (row-order rationing, spec §8), not strategy. Named in
the emitted text.

Zone defaults are the frontier tiers (spec §3): core 0,1,2 | mid 3,4,5 |
haven 6 | frontier 7,8,9. Scenario-conditional: pass --zones for other maps
(e.g. --zones "0,1,2|3,4,5" on the 6-station control).

Usage:
    python3 python/analysis/radial_zones.py RUN_JSONL \
        --gossip-log GOSSIP_JSONL [--zones "0,1,2|3,4,5|6|7,8,9"]
"""
import argparse
import json

ZONE_NAMES = ["core", "mid", "haven", "frontier"]

RATIONING_TEXT = (
    "row-order rationing (RECORDED, never gated — PDR-0006): resolve_refuels "
    "fills in dense craft-row order; under scarce stock low rows drink "
    "first, so a fill-share gradient by row is a rationing artifact, not "
    "strategy."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_zones(spec):
    return [[int(x) for x in part.split(",") if x] for part in spec.split("|")]


def zone_series(windows, zone):
    """Per-window zone aggregates: traffic/robs over routes TOUCHING the
    zone (either endpoint), summed fuel stock, max fuel price, alerts."""
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
    """The OD-4 death-spiral-vs-boom-bust discriminator (docstring table)."""
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
    """FLOOR permille of remaining headroom this refuel filled; None when
    the tank was already full (resolve_refuels skips those — defensive)."""
    before, after = ev["before_permille"], ev["after_permille"]
    if before >= 1000:
        return None
    return (after - before) * 1000 // (1000 - before)


def fill_share_rows(refuels):
    """Per craft row: (n events, total units, median fill permille)."""
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
        rows.append((craft, per[craft]["n"], per[craft]["units"],
                     fills[(len(fills) - 1) // 2]))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("windows", help="trophic_run --jsonl per-window file")
    ap.add_argument("--gossip-log", help="same run's --gossip-log (refuel events)")
    ap.add_argument("--zones", default="0,1,2|3,4,5|6|7,8,9")
    args = ap.parse_args()

    windows = load(args.windows)
    zones = parse_zones(args.zones)
    refuels = (
        [e for e in load(args.gossip_log) if e["e"] == "refuel"]
        if args.gossip_log else []
    )
    print(
        f"radial_zones (PDR-0006: windows, not gates) — {len(windows)} windows, "
        f"{len(refuels)} refuel events, zones={args.zones}"
    )
    for zi, zone in enumerate(zones):
        name = ZONE_NAMES[zi] if zi < len(ZONE_NAMES) else f"zone{zi}"
        series = zone_series(windows, zone)
        stock = [s["stock"] for s in series]
        traffic = [s["traffic"] for s in series]
        reading = fuel_starve(stock, traffic)
        print(f"\n-- zone {name} (stations {zone}) — fuel_starve={reading} "
              "(RECORDED; either answer is a finding — OD-4) --")
        print("  window_close  traffic  robs  fuel_stock  price_max  alerts")
        for s in series:
            print(
                f"  {s['tick']:>12}  {s['traffic']:>7}  {s['robs']:>4}  "
                f"{s['stock']:>10}  {s['price_max']:>9}  {s['alerts']:>6}"
            )
    print("\n-- per-row refuel fill share --")
    rows = fill_share_rows(refuels)
    if not rows:
        print("  no refuel events (zero-refuel sentinel — the MEDIA precedent)")
    else:
        print("  row  refuels  units  median_fill_permille")
        for craft, n, units, med in rows:
            print(f"  {craft:>3}  {n:>7}  {units:>5}  {med:>20}")
    print(f"  {RATIONING_TEXT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: run + expected pass; live verification of the refuel lines**

```bash
PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_radial_zones.py   # 4 passed
cargo test --workspace                       # runner still compiles, nothing regressed
cargo clippy --all-targets -- -D warnings
# Lockstep verification (recorded): a frontier run emits refuel lines, a
# trophic run emits none (RefuelCfg default-off inertness).
mkdir -p /tmp/wgb-i2
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 50000 \
  --jsonl /tmp/wgb-i2/frontier_s7.jsonl --gossip-log /tmp/wgb-i2/frontier_s7.gossip.jsonl \
  > /tmp/wgb-i2/frontier_s7.stdout
grep -c '"e":"refuel"' /tmp/wgb-i2/frontier_s7.gossip.jsonl     # expected: > 0
python3 python/analysis/radial_zones.py /tmp/wgb-i2/frontier_s7.jsonl \
  --gossip-log /tmp/wgb-i2/frontier_s7.gossip.jsonl | tee /tmp/wgb-i2/i2_frontier_s7.txt
```

- [ ] **Step 5: commit (emit + reader, one lockstep commit)**

```bash
git add crates/jumpgate-core/examples/trophic_run.rs \
        python/analysis/radial_zones.py python/tests/test_radial_zones.py
git commit -F - <<'EOF'
lab(panel): I2 radial zones — fuel_starve discriminator + refuel fill share

Refueled events now flow to --gossip-log (explicit arm past the
catch-all; additive — trophic logs stay byte-identical, RefuelCfg off).
radial_zones.py reads per-zone traffic/robs/fuel stock/price/alerts and
prints the OD-4 death-spiral-vs-boom-bust discriminator (pre-registered
reading table pinned by pytest) plus per-row fill share with row-order
rationing named in the emitted text. Windows, never gates.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.6: the headline 20-seed × 6-arm grid (W4 flip-share VALUE, rate-normalized cross-map reads)

§11 W4: "flip-share VALUE: gossip-vs-ring over 20 clean seeds × both anchor
arms". The grid runs on `scenario_frontier`. **The six arms (pre-registered;
every knob set EXPLICITLY so arm identity never rides a factory default):**

| arm | `hauler_belief_scoring` | `station_gossip_slots` | `craft_gossip_slots` | `staleness_from_rob_tick` | active ASSIGN read |
|---|---|---|---|---|---|
| `blind-born`  | false | 0  | 0 | false | none (reward argmax) |
| `blind-rob`   | false | 0  | 0 | true  | none — A/A twin |
| `ring-born`   | true  | 0  | 0 | false | RouteEvidence ring |
| `ring-rob`    | true  | 0  | 0 | true  | ring — A/A twin |
| `gossip-born` | true  | 16 | 8 | false | gossip, born/heard-anchored staleness |
| `gossip-rob`  | true  | 16 | 8 | true  | gossip, rob-tick-anchored staleness |

The anchor knob is consumed ONLY on the gossip read (economy.rs:585-612
swapped read: `buf.count_route_recent(..., staleness_from_rob_tick)`;
media-off falls back to the ring, blind skips scoring entirely). The
blind/ring born-vs-rob twins are therefore **A/A instrument controls**:
identical dynamics expected; any RESULT divergence = wiring bug — fix the
instrument before reading W4 (recorded, not a ship gate). Gossip caps 16/8
are the `MediaCfg` documented live-start values (config.rs:359-362).

**Pre-registered:** 20 seeds = `7 11 13 23 29 31 37 41 42 43 47 53 57 59 61
67 71 73 99 101`. CLEAN seed = `blind-born` verdict != `PermanentPeace`
(the no-information baseline defines the peace basin — the media rung's
basin-chaos lesson). W4 value read = gossip−ring delta in median final
hauler credits per anchor over clean seeds; registered alternative: "mixing
persists → the deferred dispatch-locality lever" (spec §12). Cross-map reads
vs the banked post-fix trophic baseline are rate-normalized
distribution-vs-distribution, NEVER same-seed paired (GEO-C3).

**Files**
- Create: `/home/john/jumpgate/python/analysis/w4_grid.py`
- Create: `/home/john/jumpgate/python/tests/test_w4_grid.py`

- [ ] **Step 1: failing test — clean rule, A/A comparator, value/rate math**

`python/tests/test_w4_grid.py`:

```python
"""W4 grid reader pins (world-gets-big spec §8/§11 W4)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import w4_grid


def cell(verdict="Alive", robs="6", trips="40", credits=None, haulers=3):
    return {
        "result": {
            "seed": "7", "ticks": "50000", "verdict": verdict, "cycled": "true",
            "hetero": "true", "disperse": "true", "fuel_empty": "0",
            "robs": robs, "trips": trips, "purchases": "2",
        },
        "meta": {"haulers": str(haulers)},
        "windows": [{"per_craft_credits": credits or [30, 10, 20, 999]}],
    }


def test_clean_seeds_rule_is_blind_born_not_permanent_peace():
    cells = {
        ("blind-born", 7): cell(verdict="Alive"),
        ("blind-born", 11): cell(verdict="PermanentPeace"),
        # A peace reading on ANOTHER arm must not dirty the seed.
        ("gossip-born", 7): cell(verdict="PermanentPeace"),
    }
    assert w4_grid.clean_seeds(cells, [7, 11]) == [7]


def test_run_value_takes_hauler_slice_median_and_per_trip_rate():
    v = w4_grid.run_value(cell())
    # hauler rows 0..3 -> [30, 10, 20]; median 20; 20*1000 // 40 trips = 500.
    assert v == {"median_credits": 20, "laden_trips": 40, "credits_per_trip_milli": 500}


def test_aa_twin_divergence_names_the_differing_fields():
    cells = {
        ("blind-born", 7): cell(robs="6"),
        ("blind-rob", 7): cell(robs="7"),
        ("ring-born", 7): cell(),
        ("ring-rob", 7): cell(),
    }
    bad = w4_grid.aa_twin_divergences(cells, [7])
    assert len(bad) == 1
    fam, seed, fields = bad[0]
    assert (fam, seed) == ("blind", 7)
    assert fields == {"robs": ("6", "7")}


def test_quartiles_are_lower_index_integers():
    assert w4_grid.quartiles([5, 1, 9, 3, 7]) == (3, 5, 7)
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_w4_grid.py`
Expected failure: `ModuleNotFoundError: No module named 'w4_grid'`.

- [ ] **Step 2: implement `w4_grid.py`**

```python
"""w4_grid — headline 20-seed x 6-arm frontier grid reader
(world-gets-big spec §8 ensembles / §11 W4).

FRAME (PDR-0006): REPORTED, NEVER GATED. The W4 question: does the
gossip-vs-ring flip share finally carry VALUE now the world is bigger than
the news? Registered alternative: mixing persists -> the deferred
dispatch-locality lever (spec §12).

THE SIX ARMS (pre-registered; anchor = staleness_from_rob_tick, consumed
ONLY on the gossip read — blind/ring born-vs-rob twins are A/A instrument
controls; any RESULT divergence is a wiring bug to fix BEFORE reading W4):
  blind-born blind-rob ring-born ring-rob gossip-born gossip-rob

CLEAN SEED (pre-registered): blind-born verdict != PermanentPeace — the
no-information baseline defines the peace basin. Value reads pool CLEAN
seeds; the all-seeds pool is printed beside as context.

CROSS-MAP (GEO-C3): vs the banked post-fix trophic baseline, rate-normalized
distribution-vs-distribution (credits per laden trip, robs per 1000 laden
trips) — NEVER same-seed paired deltas; scenario_frontier is a new world.

Usage:
    python3 python/analysis/w4_grid.py /tmp/wgb-grid \
        [--seeds 7 11 ...] [--compare /tmp/wgb-trophic-baseline]
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE, RESULT_RE

ARMS = ["blind-born", "blind-rob", "ring-born", "ring-rob", "gossip-born", "gossip-rob"]

# The exact sweep --knobset strings (kept here so the grid is re-runnable
# from this file's docstring alone; every knob explicit).
ARM_KNOBSETS = [
    "blind-born:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "blind-rob:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "ring-born:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "ring-rob:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "gossip-born:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=false",
    "gossip-rob:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=true",
]

SEEDS = [7, 11, 13, 23, 29, 31, 37, 41, 42, 43, 47, 53, 57, 59, 61, 67, 71, 73, 99, 101]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def load_cell(out_dir, arm, seed):
    """One (arm, seed) cell from the sweep's banked stdout + windows."""
    stdout = (out_dir / f"{arm}_s{seed}.stdout").read_text()
    result = meta = None
    for line in stdout.splitlines():
        m = RESULT_RE.match(line.strip())
        if m:
            result = m.groupdict()
        m = META_RE.match(line.strip())
        if m:
            meta = m.groupdict()
    if result is None:
        raise SystemExit(f"no RESULT in {arm}_s{seed}.stdout")
    return {
        "result": result,
        "meta": meta,
        "windows": load(out_dir / f"{arm}_s{seed}.jsonl"),
    }


def clean_seeds(cells, seeds):
    """CLEAN = blind-born verdict != PermanentPeace (pre-registered)."""
    return [
        s for s in seeds
        if cells[("blind-born", s)]["result"]["verdict"] != "PermanentPeace"
    ]


def run_value(cell):
    """Median final hauler credits + rate per laden trip (milli) for one run.
    The hauler count comes from META — never a mirrored constant."""
    haulers = int(cell["meta"]["haulers"])
    creds = sorted(cell["windows"][-1]["per_craft_credits"][:haulers])
    med = creds[(len(creds) - 1) // 2]
    trips = int(cell["result"]["trips"])
    return {
        "median_credits": med,
        "laden_trips": trips,
        "credits_per_trip_milli": med * 1000 // max(trips, 1),
    }


def aa_twin_divergences(cells, seeds):
    """blind/ring born-vs-rob twins must have IDENTICAL RESULT dicts (the
    anchor is unread off the gossip path). Returns the divergences."""
    bad = []
    for fam in ("blind", "ring"):
        for s in seeds:
            a = cells.get((f"{fam}-born", s))
            b = cells.get((f"{fam}-rob", s))
            if a is None or b is None:
                continue
            ar, br = a["result"], b["result"]
            if ar != br:
                bad.append((fam, s, {k: (ar[k], br[k]) for k in ar if ar[k] != br[k]}))
    return bad


def quartiles(xs):
    s = sorted(xs)
    n = len(s)
    return s[(n - 1) // 4], s[(n - 1) // 2], s[(3 * (n - 1)) // 4]


def arm_table(cells, seeds, label):
    print(f"\n-- per-arm value table ({label}: {len(seeds)} seeds) --")
    print("  arm          med(median_credits)  med(cr/trip_milli)  flips/decisions(pooled)  readings")
    out = {}
    for arm in ARMS:
        vals = [run_value(cells[(arm, s)]) for s in seeds]
        med_c = quartiles([v["median_credits"] for v in vals])[1] if vals else None
        med_r = quartiles([v["credits_per_trip_milli"] for v in vals])[1] if vals else None
        flips = decisions = 0
        readings = {}
        for s in seeds:
            ws = cells[(arm, s)]["windows"]
            if ws:
                flips += ws[-1]["assign_flips_cum"]
                decisions += ws[-1]["assign_decisions_cum"]
            v = cells[(arm, s)]["result"]["verdict"]
            readings[v] = readings.get(v, 0) + 1
        out[arm] = {"med_credits": med_c, "med_rate": med_r}
        print(f"  {arm:<12} {med_c!s:>19}  {med_r!s:>18}  {flips}/{decisions:<22} {readings}")
    for anchor in ("born", "rob"):
        g, r = out[f"gossip-{anchor}"], out[f"ring-{anchor}"]
        if g["med_credits"] is not None and r["med_credits"] is not None:
            print(
                f"  W4 VALUE delta ({anchor}-anchor): gossip-ring = "
                f"{g['med_credits'] - r['med_credits']} micros median final hauler "
                f"credits ({g['med_rate'] - r['med_rate']} milli per laden trip) — "
                "REPORTED, NEVER GATED; registered alternative: mixing persists "
                "-> the deferred dispatch-locality lever (spec §12)"
            )
    return out


def map_distributions(stdout_dir, pattern):
    """Per-run rate reads for one map dir of banked stdouts (cross-map)."""
    rates, rob_rates = [], []
    for p in sorted(pathlib.Path(stdout_dir).glob(pattern)):
        result = meta = None
        windows_path = p.with_suffix(".jsonl")
        for line in p.read_text().splitlines():
            m = RESULT_RE.match(line.strip())
            if m:
                result = m.groupdict()
            m = META_RE.match(line.strip())
            if m:
                meta = m.groupdict()
        if result is None or meta is None or not windows_path.exists():
            continue
        ws = load(windows_path)
        if not ws:
            continue
        haulers = int(meta["haulers"])
        creds = sorted(ws[-1]["per_craft_credits"][:haulers])
        med = creds[(len(creds) - 1) // 2]
        trips = max(int(result["trips"]), 1)
        rates.append(med * 1000 // trips)
        rob_rates.append(int(result["robs"]) * 1000 // trips)
    return rates, rob_rates


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("out_dir", help="the grid sweep's --out directory")
    ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    ap.add_argument(
        "--compare",
        help="banked post-fix trophic baseline sweep dir (cross-map read; "
        "rate-normalized distribution-vs-distribution, NEVER paired — GEO-C3)",
    )
    args = ap.parse_args()
    out_dir = pathlib.Path(args.out_dir)

    cells = {
        (arm, s): load_cell(out_dir, arm, s) for arm in ARMS for s in args.seeds
    }
    print(
        "w4_grid (PDR-0006: REPORTED, NEVER GATED) — "
        f"{len(ARMS)} arms x {len(args.seeds)} seeds"
    )
    bad = aa_twin_divergences(cells, args.seeds)
    if bad:
        print("\nA/A TWIN DIVERGENCE (wiring bug — fix the instrument BEFORE reading W4):")
        for fam, s, fields in bad:
            print(f"  {fam} seed={s}: {fields}")
    else:
        print("A/A twins (blind, ring): born-vs-rob RESULT identical on every seed (instrument sound)")

    clean = clean_seeds(cells, args.seeds)
    dirty = [s for s in args.seeds if s not in clean]
    print(f"\nclean seeds (blind-born != PermanentPeace): {len(clean)}/{len(args.seeds)}; dirty={dirty}")
    arm_table(cells, clean, "CLEAN")
    arm_table(cells, args.seeds, "ALL (context)")

    if args.compare:
        fr, fr_robs = map_distributions(out_dir, "gossip-born_s*.stdout")
        tr, tr_robs = map_distributions(args.compare, "baseline_s*.stdout")
        print(
            "\n-- cross-map (rate-normalized, distribution-vs-distribution; "
            "NEVER same-seed paired — GEO-C3) --"
        )
        print(f"  frontier gossip-born: credits/trip milli quartiles={quartiles(fr) if fr else None} "
              f"robs/1000trips quartiles={quartiles(fr_robs) if fr_robs else None}")
        print(f"  trophic  baseline:    credits/trip milli quartiles={quartiles(tr) if tr else None} "
              f"robs/1000trips quartiles={quartiles(tr_robs) if tr_robs else None}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_w4_grid.py`
→ `4 passed`.

- [ ] **Step 4: PROCEDURE — ordering check, then run the grid (120 runs, 50k ticks)**

First confirm the Task 3.2 Step-5 frontier positive-control reading is
already recorded in `/tmp/wgb-fit/frontier-control-first/sweep_stdout.txt`
(procedure order: the control reading precedes the headline frontier
ensemble; an order step, not a ship gate). Then:

```bash
mkdir -p /tmp/wgb-grid
python3 python/analysis/sweep_trophic.py --scenario frontier \
  --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101 \
  --ticks 50000 \
  --knobset "blind-born:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false" \
  --knobset "blind-rob:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true" \
  --knobset "ring-born:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false" \
  --knobset "ring-rob:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true" \
  --knobset "gossip-born:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=false" \
  --knobset "gossip-rob:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=true" \
  --out /tmp/wgb-grid | tee /tmp/wgb-grid/sweep_stdout.txt

python3 python/analysis/w4_grid.py /tmp/wgb-grid \
  --compare <PATH TO THE PHASE-0B BANKED POST-FIX TROPHIC BASELINE SWEEP DIR> \
  | tee /tmp/wgb-grid/w4_readout.txt
```

(The `--compare` path is the phase-0b 20-seed trophic bank; if that bank
predates Task 3.1's stdout banking, regenerate it first with
`python3 python/analysis/sweep_trophic.py --scenario trophic --seeds <same 20> --ticks 50000 --out /tmp/wgb-trophic-baseline`
— a deterministic re-run of the same seeds.) All readings RECORDED; banked
by Task 3.7.

- [ ] **Step 5: commit**

```bash
git add python/analysis/w4_grid.py python/tests/test_w4_grid.py
git commit -F - <<'EOF'
lab(grid): W4 headline reader — 6 arms x 20 seeds, flip-share VALUE

blind/ring/gossip x born/rob anchors (every knob explicit; blind+ring
rob twins registered as A/A instrument controls since the anchor is
consumed only on the gossip read). Clean-seed rule pre-registered
(blind-born != PermanentPeace), gossip-vs-ring value deltas per anchor,
cross-map reads rate-normalized distribution-vs-distribution (GEO-C3).
REPORTED, NEVER GATED (PDR-0006).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.7: the owner console packet (band re-judgment + W1–W12 readings)

Assemble the console-session materials (the OD-3 scheduled re-judgment plus
the rung's window readings) into the capture-practice posts directory.
Everything in the packet is RECORDED, never gated; /tmp is volatile so this
banking happens the same day the runs land.

**Files**
- Create: `/home/john/jumpgate/docs/superpowers/posts/2026-06-11-world-gets-big-console/packet.md`
- Create (copied banked outputs, same directory):
  `fit_trophic.txt fit_frontier.txt fit_pooled.txt frontier_control_first.txt
  i1_trophic_control.txt i1_frontier_s7.txt i2_frontier_s7.txt
  grid_sweep_stdout.txt w4_readout.txt`

- [ ] **Step 1: bank the run outputs (same-day capture practice)**

```bash
mkdir -p docs/superpowers/posts/2026-06-11-world-gets-big-console
cp /tmp/wgb-fit/fit_trophic.txt  docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_trophic.txt
cp /tmp/wgb-fit/fit_frontier.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_frontier.txt
cp /tmp/wgb-fit/fit_pooled.txt   docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_pooled.txt
cp /tmp/wgb-fit/frontier-control-first/sweep_stdout.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/frontier_control_first.txt
cp /tmp/wgb-i1/i1_trophic_control.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/i1_trophic_control.txt
cp /tmp/wgb-i1/i1_frontier_s7.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/i1_frontier_s7.txt
cp /tmp/wgb-i2/i2_frontier_s7.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/i2_frontier_s7.txt
cp /tmp/wgb-grid/sweep_stdout.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/grid_sweep_stdout.txt
cp /tmp/wgb-grid/w4_readout.txt       docs/superpowers/posts/2026-06-11-world-gets-big-console/w4_readout.txt
```

- [ ] **Step 2: write `packet.md`**

Full skeleton below; the `«measured: …»` cells are PASTED from the named
banked files (the same paste-only discipline as goldens — the planner and
the builder's memory are not data sources):

```markdown
# Owner console packet — world-gets-big (band re-judgment + W1–W12)

FRAME (PDR-0006): every number in this packet is RECORDED, never gated.
The session judges PLAY at the console; these are the windows beside it.

## 1. Agenda
1. Band re-judgment (OD-3 bundled consequence): the haven-lurk-leak fix
   changed the trophic band previously judged "a great story" (~86% of
   post-refuge draws predicted to become map-wide breakouts). Re-watch the
   post-fix band chronicles and re-judge.
2. Frontier first look: the §1 arc — the ship that ran dry, the lane nobody
   warned, the station that sold its last tank at four times the core price.
3. W4 anchor adoption call (born vs rob staleness anchor, gossip arms).
4. W11 read = the named pirate-fuel unification trigger (OD-6).

## 2. Pre-registered windows (spec §11, verbatim) and readings

| window | pre-registered text (spec §11) | measured | source |
|---|---|---|---|
| W1 | saturation leaves CommonKnowledge (escaped_milli < 950; desert map disambiguates) | «measured: grid MEDIA readings + escaped_milli» | grid_sweep_stdout.txt |
| W2 | median hearing lag > 2500 (registered alternative: rob-anchored staleness zeroes frontier evidence older than 4000 — "gossip degenerates toward blind on frontier routes") | «measured: MEDIA median_lag/p90_lag, gossip arms» | grid_sweep_stdout.txt |
| W3 | hub/backwater ratio > 3.0 | «measured: news-geography panel» | grid_sweep_stdout.txt |
| W4 | flip-share VALUE: gossip-vs-ring over 20 clean seeds × both anchor arms (registered alternative: mixing persists → the deferred dispatch-locality lever) | «measured: W4 VALUE deltas» | w4_readout.txt |
| W5 | I1 correlations with the position-blind-dispatch confound registered | «measured: spearman lines» | i1_frontier_s7.txt vs i1_trophic_control.txt |
| W6 | breakout share + landing distribution + lurk-dwell bimodality | «measured: LurkMoved breakout share (phase-2 instruments)» | grid windows JSONL |
| W7 | tier-2 service rate + regime onset vs the upgrade ladder | «measured: per-zone traffic, frontier zone» | i2_frontier_s7.txt |
| W8 | hauler per-leg burn/duty (the calibration input) | «measured: FUEL line fields» | grid_sweep_stdout.txt |
| W9 | strandings 0–2/run band + robbed→stranded chains + contract-age liveness | «measured: FUEL strandings/adrift_end + chronicle chains» | grid_sweep_stdout.txt |
| W10 | station fuel stock-out map + price gradient + fuel_starve discriminator | «measured: per-zone fuel_starve readings» | i2_frontier_s7.txt |
| W11 | fleet attrition + per-role pirate fuel low-water (the OD-6 trigger input) | «measured: per-role JSONL fuel rows» | grid windows JSONL |
| W12 | trophic arms bit-identical digest + fuel_empty=0 (the control stays a control) | «measured: phase-1 digest test + trophic baseline fuel_empty» | fit_trophic sweep |

## 3. Instrument re-fit (dual-map, post-leak-fix)
Frontier positive control read FIRST: «measured: N/4 RiskEqualized»
(frontier_control_first.txt). Fit tables: fit_trophic.txt /
fit_frontier.txt / fit_pooled.txt. Adopted constants: «paste the
diagnostics.rs literals + whether the margin was open». Held-out seeds
11/31/57/101 sides: «paste».

## 4. Band re-judgment materials (post-fix trophic vs pre-fix bank)
Post-fix 20-seed trophic baseline: the phase-0b bank. Pre-fix judged band:
docs/superpowers/posts/2026-06-11-media-rung-story (chronicles + panels).
Diff to watch: post-refuge relocation pattern (LurkMoved breakout share on
the band), verdict mix, robs/trips rates — rate-normalized
distribution-vs-distribution, never paired.
Chronicle regen commands:
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario trophic --seed 7 --ticks 50000 --chronicle
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed 7 --ticks 50000 --chronicle

## 5. Registered confounds & rules carried into the readings
* PLAY-C3: position-blind dispatch — radius correlations may be
  capacity-ladder reads (printed by the I1 panel itself).
* A/A twins: blind/ring born-vs-rob arms must be identical; status:
  «measured: w4_readout.txt A/A line».
* Clean-seed rule: blind-born ≠ PermanentPeace; clean count: «measured».
* GEO-C3: cross-map reads are rate-normalized distributions, never
  same-seed pairs.
* fuel_empty=0 flips meaning on frontier ("no stranding this seed" —
  texture); --assert-no-fuel-empty stays on trophic arms only.
```

- [ ] **Step 3: verify nothing volatile or generated is being staged**

```bash
git status --short docs/superpowers/posts/2026-06-11-world-gets-big-console
git diff --stat        # expect: only the new posts directory contents
```

Confirm `runs/` is untouched and nothing under `/tmp` is referenced as a
deliverable (all copies are now in the posts dir).

- [ ] **Step 4: commit, then hand to the owner**

```bash
git add docs/superpowers/posts/2026-06-11-world-gets-big-console
git commit -F - <<'EOF'
docs(console): world-gets-big owner packet — band re-judgment + W1-W12

Banked same-day (capture practice): dual-map fit tables, frontier
positive-control-first reading, I1/I2 panel outputs, the 20-seed x 6-arm
grid stdout + W4 readout, and packet.md with the spec-s11 pre-registered
window text beside every measured reading. RECORDED, never gated
(PDR-0006); the console session re-judges the post-leak-fix band (OD-3)
and reads W11 as the OD-6 trigger input.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

Then surface the packet path to the owner and update the tracking issue
(`filigree issue-search "world gets big"`, `filigree comment <id> --actor
<name>` with the packet path) — the console session itself is the owner's.
