# WGB Trophic Baseline, Phase 0b

Recorded: 2026-06-12

Run-time HEAD: `db4dca2` (`feat(lab): add role-split FUEL instrumentation`)

Phase-0a haven-lurk fix ancestry: `eff6db1` (`fix(pirate): exclude haven from post-refuge lurk`). The follow-up cleanup commit `6089cd0` is also below this baseline HEAD.

Windows recorded, never gated (PDR-0006). This baseline replaces every pre-haven-lurk-fix banked trophic baseline for cross-arm comparison; old banked state-hash trajectories are NOT comparable (the fix shifts Piracy draws; goldens unchanged).

## Shape

- 20 seeds: `0..19`.
- 50,000 ticks per run.
- 25 diagnostic windows per run (`WINDOW_TICKS = 2000`).
- Instrument format v2: META + FUEL anchored stdout lines; version-gated parsing pinned by `python/tests/test_sweep_parsing.py`.
- Per-seed JSONL shape: 1 meta row + 25 window rows + 2 fuel-role rows = 28 rows.
- Full per-seed stdout and JSONL live in `runs/wgb_baseline/`, deliberately uncommitted because `runs/` is gitignored and volatile-class.

## Commands

```bash
cd /home/john/jumpgate
cargo build --release -p jumpgate-core --examples
mkdir -p runs/wgb_baseline
for seed in $(seq 0 19); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed "$seed" --ticks 50000 \
    --jsonl "runs/wgb_baseline/trophic_s${seed}.jsonl" \
    --replay-check --assert-no-fuel-empty \
    > "runs/wgb_baseline/trophic_s${seed}.stdout"
done
grep -h '^META \|^RESULT \|^MEDIA \|^FUEL \|^ASSIGN ' \
  runs/wgb_baseline/trophic_s*.stdout > runs/wgb_baseline/anchored_lines.txt
wc -l runs/wgb_baseline/anchored_lines.txt
```

```bash
python3 python/analysis/sweep_trophic.py \
  --seeds 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 \
  --ticks 50000 \
  --knobset baseline \
  --knobset "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000" \
  --out runs/wgb_baseline/sweep \
  | tee runs/wgb_baseline/sweep_summary.txt
```

## Seed 7 Anchored Sample

META:
`META seed=7 scenario=trophic stations=6 haulers=12 pirates_initial=6 station_radii_milli_au=[350, 560, 770, 980, 1190, 1400]`

RESULT:
`RESULT seed=7 ticks=50000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=93 laden_trips=682 purchases=42`

MEDIA:
`MEDIA seed=7 born=0 escaped_milli=0 median_lag=0 p90_lag=0 reading=NoMedia`

FUEL:
`FUEL seed=7 hauler_duty_milli=670 hauler_burn_total_milli=5022 hauler_median_leg_burn_permille=6 hauler_min_tank_permille=576`

ASSIGN:
`ASSIGN seed=7 decisions=787 flips=0 flip_milli=0 counts=[1667, 4574, 1803, 417, 152, 0, 0]`

## Summary Files

- `anchored_lines.txt`: 100 anchored lines, 5 per seed.
- `sweep_summary.txt`: baseline plus hungry-roamer control panel output.
