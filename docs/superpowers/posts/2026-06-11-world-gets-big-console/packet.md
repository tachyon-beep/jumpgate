# Owner console packet - world-gets-big (band re-judgment + W1-W12)

FRAME (PDR-0006): every number in this packet is RECORDED, never gated.
The session judges PLAY at the console; these are the windows beside it.

## 1. Agenda

1. Band re-judgment (OD-3 bundled consequence): the haven-lurk-leak fix changed
   the trophic band previously judged "a great story". Re-watch the post-fix
   band chronicles and re-judge.
2. Frontier first look: the section-1 arc - the ship that ran dry, the lane
   nobody warned, the station that sold its last tank at four times the core
   price.
3. W4 anchor adoption call (born vs rob staleness anchor, gossip arms).
4. W11 read = the named pirate-fuel unification trigger (OD-6).

## 2. Pre-registered windows and readings

| window | pre-registered text | measured | source |
|---|---|---|---|
| W1 | saturation leaves CommonKnowledge (escaped_milli < 950; desert map disambiguates) | gossip-born: Localized 12, CommonKnowledge 8, escaped_milli quartiles (875, 934, 962). gossip-rob: CommonKnowledge 9, Localized 11, escaped_milli quartiles (857, 937, 972). | grid_sweep_stdout.txt, grid_aux_summary.txt |
| W2 | median hearing lag > 2500 (registered alternative: rob-anchored staleness zeroes frontier evidence older than 4000 - "gossip degenerates toward blind on frontier routes") | gossip-born median_lag quartiles (5694, 6963, 9228), p90 quartiles (15309, 19932, 24136). gossip-rob median_lag quartiles (5714, 6332, 9087), p90 quartiles (15329, 19625, 22682). | grid_aux_summary.txt |
| W3 | hub/backwater ratio > 3.0 | gossip-born mostly above 3.0 with two exceptions in the printed panel (2.9, 2.8); gossip-rob printed ratios are >= 3.0, with large hub skew in several seeds (for example 15.1). | grid_sweep_stdout.txt |
| W4 | flip-share VALUE: gossip-vs-ring over 20 clean seeds x both anchor arms (registered alternative: mixing persists -> the deferred dispatch-locality lever) | Clean seeds 18/20, dirty [31, 99]. A/A twins sound. Clean deltas: born-anchor +713500 micros median final hauler credits (-244789 milli per laden trip); rob-anchor +622275 micros (-350703 milli per laden trip). | w4_readout.txt |
| W5 | I1 correlations with the position-blind-dispatch confound registered | Frontier seed 7: Spearman radius/median lag = 0.166, radius/end credits = 0.152 over 20 haulers. Trophic control seed 7: -0.098 and 0.818 over 12 haulers. PLAY-C3 confound named in both panels. | i1_frontier_s7.txt, i1_trophic_control.txt |
| W6 | breakout share + landing distribution + lurk-dwell bimodality | Proxy snapshot only, not a LurkMoved chronicle panel: final_lurker_zone_totals core=194, mid=265, haven=0, frontier=357; frontier_lurker_share_permille quartiles (166, 400, 600); final pirates_at_haven quartiles (1, 1, 3); final pirates_commuting quartiles (1, 1, 2). | grid_aux_summary.txt |
| W7 | tier-2 service rate + regime onset vs the upgrade ladder | Frontier seed 7 zone panel: frontier zone first shows traffic at window 48000 (traffic 3, robs 2), then 50000 (traffic 5, robs 1, fuel_stock 25, price_max 10000). | i2_frontier_s7.txt |
| W8 | hauler per-leg burn/duty (the calibration input) | Grid hauler fuel tail summary: duty_milli quartiles (546, 557, 565), burn_total_milli (3206, 3269, 3318), median_leg_burn_permille (2, 2, 2), min_tank_permille (936, 942, 944). | grid_sweep_stdout.txt, grid_aux_summary.txt |
| W9 | strandings 0-2/run band + robbed->stranded chains + contract-age liveness | fuel_empty_counts {0: 120}; max_open_contract_age quartiles (1077, 1257, 1439); open_contracts quartiles (20, 20, 20). Stranding chains were not separately chronicle-banked in this packet. | grid_aux_summary.txt |
| W10 | station fuel stock-out map + price gradient + fuel_starve discriminator | seed 7 radial readings: core NoStockout, mid NoStockout, haven Stockout, frontier NoStockout. Per-row refuel fill share recorded with row-order rationing note. | i2_frontier_s7.txt |
| W11 | fleet attrition + per-role pirate fuel low-water (the OD-6 trigger input) | Tail fuel rows: hauler min_tank_permille quartiles (936, 942, 944), pirate min_tank_permille quartiles (877, 897, 911); pirate duty_milli quartiles (107, 117, 124), burn_total_milli (664, 730, 773). | grid_aux_summary.txt |
| W12 | trophic arms bit-identical digest + fuel_empty=0 (the control stays a control) | Regenerated 20-seed trophic baseline: fuel_empty=0 for all baseline runs; verdict mix RiskEqualized 18, Alive 2. Trophic control: RiskEqualized 20/20, fuel_empty=0 for all runs. The Phase-1 digest artifact is not reprinted here. | trophic_baseline_summary.txt, fit_trophic.txt |

## 3. Instrument re-fit (dual-map, post-leak-fix)

Frontier positive control read FIRST: 4/4 RiskEqualized
(`frontier_control_first.txt`).

Fit tables:

- `fit_trophic.txt`: threshold candidate 1058, slack candidate 4; both margins
  closed.
- `fit_frontier.txt`: HHI threshold candidate 3911 with `margin_open=True`;
  slack candidate 5 with closed margin.
- `fit_pooled.txt`: threshold candidate 2110 and slack candidate 5; closed
  pooled fit, not adopted.

Adopted constants stayed unchanged in `crates/jumpgate-core/src/diagnostics.rs`:
`HHI_NORM_MIN_MILLI = 2204` and
`HOT_PERSISTENCE_SLACK_CHANGES = 3`. The closed dual-map fit was recorded as a
deferred trigger, not a Phase-3 literal change.

Held-out sides are printed in the fit files. The frontier held-out clumped
baseline seeds 101/11/31/57 all read boundary side `clumped`; frontier
held-out equalized controls split against the current boundary, which is why
the scenario-conditional threshold work was deferred.

## 4. Band re-judgment materials

Post-fix 20-seed trophic baseline is banked in the same-day trophic baseline
run and summarized in `trophic_baseline_summary.txt`. Pre-fix judged band remains in
`docs/superpowers/posts/2026-06-11-media-rung-story`.

Diff to watch at the console: post-refuge relocation pattern, verdict mix,
robs/trips rates, and rate-normalized distribution-vs-distribution reads.
Never pair same seed across trophic and frontier.

Chronicle regen commands:

```bash
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario trophic --seed 7 --ticks 50000 --chronicle
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 50000 --chronicle
```

## 5. Registered confounds and rules carried into the readings

- PLAY-C3: position-blind dispatch - radius correlations may be
  capacity-ladder reads; the I1 panel prints this directly.
- A/A twins: blind/ring born-vs-rob arms are identical on every seed in
  `w4_readout.txt`.
- Clean-seed rule: blind-born != PermanentPeace; clean count 18/20.
- GEO-C3: cross-map reads are rate-normalized distributions, never same-seed
  pairs. `w4_readout.txt` records frontier gossip-born credits/trip quartiles
  (15654127, 18722972, 23941932) vs trophic baseline (12805755, 19716312,
  51759530).
- On frontier, `fuel_empty=0` reads as texture ("no stranding this seed"), not
  a ship gate. `--assert-no-fuel-empty` stays on trophic arms only.
