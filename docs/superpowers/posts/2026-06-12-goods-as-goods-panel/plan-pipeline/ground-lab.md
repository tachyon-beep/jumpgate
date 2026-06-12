# Ground Extract — Lab Bench + Digest Procedure

> Grounded at HEAD b446095 (branch jumpgate-v1-design).
> Every claim was read this session; file:line citations are for that HEAD.

---

## 1. TrophicSample — full field list

`diagnostics.rs:129-235`. `Default` derive; all fields are integers ("samples are hash-adjacent evidence, never float analytics").

Field groups in struct order:

**Core trophic (original):** `tick`, `active_pirates`, `lying_low`, `laden_in_transit`, `laden_trips`, `robs`, `drivenoffs`, `purchases_hull`, `purchases_escort`, `per_route_robs: Vec<u32>` (n_stations²), `per_route_accepts: Vec<u32>`, `per_route_traffic: Vec<u32>`, `yard_treasury_micros: i64`, `per_craft_credits: Vec<i64>`, `engagement_phase_milli: Vec<u32>`

**Media lab (additive, `:164`):** `gossip_born`, `gossip_first_heard`, `gossip_born_cum`, `gossip_escaped_cum`, `alerts_carried`, `stations_with_news`, `per_station_alerts: Vec<u32>`, `per_station_contacts: Vec<u32>`, `heard_lag_ticks: Vec<u32>`, `heard_hops: Vec<u32>`, `alerts_evicted_cum: u64`, `assign_decisions_cum: u64`, `assign_flips_cum: u64`, `assign_counts_cum: Vec<u64>`

**Fuel lab (additive, `:203`):** `per_craft_role: Vec<u32>`, `per_craft_thrust_ticks: Vec<u64>`, `per_craft_burn_milli: Vec<u32>`, `per_craft_min_tank_permille: Vec<u32>`, `leg_burn_permille: Vec<u32>`

**World-gets-big pirate partition (additive, `:215`):** `per_station_lurking_pirates: Vec<u32>`, `pirates_commuting: u32`, `pirates_at_haven: u32`, `per_station_fuel_stock: Vec<i64>`, `per_station_fuel_price: Vec<i64>`, `refuels: u32`, `refuel_units: u64`, `refuel_spend_micros: i64`

---

## 2. sample_window

`diagnostics.rs:541`: `pub fn sample_window(world: &World, window_start: Tick) -> TrophicSample`

- Called every `WINDOW_TICKS` (= 2000, `:29`) by `trophic_run.rs`; `window_start` is the previous close tick.
- Events: `tick > window_start` (strictly, `:560`) up to `world.tick()`.
- Instantaneous snapshots at close: `per_craft_credits`, `per_station_fuel_stock/price`, `per_station_lurking_pirates`.
- Run-cumulative fields (`gossip_born_cum`, `gossip_escaped_cum`, etc.) scan from `Tick(0)` on every call.
- **WINDOW_TICKS = 2000; 50k-tick run = 25 windows** (`:28-29`).

---

## 3. Additive-field precedent

Every field group arrives in a comment block (`diagnostics.rs:164-166`):
```
// --- media lab fields (media rung cut 1, spec §9; windows, not gates).
// Additive: every pre-media JSONL key is untouched. ---
```
Fuel uses identical wording at `:203`. New goods-keyed fields for WA panels MUST follow this block pattern. The existing `per_station_fuel_stock`/`per_station_fuel_price` keys are stated byte-identical forever (recommended-cut §1.1 "A0").

---

## 4. per_route HHI denominators — station-pair-only today

`route_of` (`diagnostics.rs:827-838`): index = `from_row * n_stations + to_row`. Vectors are length `n_stations²`. **No goods dimension.** `risk_is_heterogeneous` (`diagnostics.rs:355-411`) aggregates all goods.

For per-good route-concentration (WA self-averaging panel), compute script-side from the accept-row `resource` key in the gossip-log — NOT from new per-good route vectors in TrophicSample. Python pattern (`sweep_trophic.py:182-188`):
```python
def occupied_hhi_milli(w):
    robs = [r for r, t in zip(w["per_route_robs"], w["per_route_traffic"]) if t > 0]
    total = sum(robs)
    if total == 0:
        return None
    return sum(r * r for r in robs) * 1000 // (total * total)
```

---

## 5. classify / predation_collapsed / verdict

`diagnostics.rs:280-300` (classify), `311-321` (predation_collapsed):

```rust
let verdict = if predation_collapsed(samples) {  // PermanentPeace FIRST
    Verdict::PermanentPeace
} else if !cycled { Verdict::NoCycle
} else if !risk_heterogeneous { Verdict::RiskEqualized
} else if !outcomes_disperse { Verdict::DecisionNotTranslating
} else { Verdict::Alive };
```

`predation_collapsed`: `early >= 4 && late == 0` (split at midpoint, counting `robs + drivenoffs`). Seed-7 lesson: lie-low duty cycling oscillates `active_pirates` without predator-prey coupling, so `cycled` alone is a false witness.

---

## 6. Anchored stdout lines — exact formats

All in `trophic_run.rs`:

| Line | Location | Format |
|------|----------|--------|
| META | :710-719 | `META seed={} scenario={} stations={} haulers={} pirates_initial={} station_radii_milli_au={:?}` |
| RESULT | :721-734 | `RESULT seed={} ticks={} verdict={:?} cycled={} risk_heterogeneous={} outcomes_disperse={} fuel_empty={} robs={} laden_trips={} purchases={}` |
| MEDIA | :757-765 | `MEDIA seed={} born={} escaped_milli={} median_lag={} p90_lag={} reading={:?}` |
| FUEL | :775-788 | `FUEL seed={} hauler_duty_milli={} hauler_burn_total_milli={} hauler_median_leg_burn_permille={} hauler_min_tank_permille={} refuels={} refuel_spend_micros={} strandings={} adrift_end={}` |
| LIVENESS | :789-792 | `LIVENESS max_open_contract_age={} open_contracts={}` |
| ASSIGN | :800-811 | `ASSIGN seed={} decisions={} flips={} flip_milli={} counts={:?}` |

**Lockstep rule** (`:738-740`): new anchored line and its matching regex land in the SAME commit.

---

## 7. JSONL emission keys

`trophic_run.rs:254-306` (`sample_json`). Window rows carry `"tick"`. Non-window tail rows (NO `"tick"`): META-row keys (`meta_seed`, `meta_scenario`, `meta_stations`, `meta_haulers`, `meta_pirates_initial`, `meta_station_radii_milli_au`), and per-role FUEL rows (`fuel_role`, `duty_milli`, `burn_total_milli`, `median_leg_burn_permille`, `min_tank_permille`). Consumer pattern: `[r for r in rows if "tick" in r]`.

The **transport-table tail row** (WA1 anti-mirroring, recommended-cut §3 L4-F4) does NOT exist yet — must be added in A0 as a no-tick JSONL row (fuel_role precedent, `:641-655`).

---

## 8. gossip-log row shapes

`trophic_run.rs:384-450` (`gossip_log_event_json`):

- `"born"`: `{e, tick, alert, route, pirate, hauler, truth, claimed}`
- `"heard"`: `{e, tick, alert, carrier ("s<row>"|"c<slot>"), route, hops, claimed, rob_tick}`
- `"rob"`: `{e, tick, route}`
- `"accept"`: `{e, tick, route, hauler}`
- `"refuel"`: `{e, tick, craft, station, units, price_micros, before_permille, after_permille}`
- `"lurk_moved"`: `{e, tick, pirate, to_station, breakout}` — pinned by test `:858-879`

**A0 additions (not yet present):** `"deliver"` (ContractFulfilled — required for WA2/WA4 joins, currently unhandled in the `_ => None` catch at `:384-450`), `"lie_low"` (PirateLieLow), accept row gains `resource` + `reward` keys.

---

## 9. sweep_trophic.py parsing contract + version-gating

Regexes: `RESULT_RE` (`:53-58`), `MEDIA_RE` (`:62-66`), `META_RE` (`:71-76`), `FUEL_RE` (`:83-90`). FUEL_RE uses nested optional groups for tail fields: `(?: refuels=...)?` then `(?: strandings=...)?`. Absent = `None`. The `ANCHORED` dict (`:92-97`) marks RESULT+MEDIA required, META+FUEL optional.

`parse_stdout` (`:144-157`) scans all lines, fills a dict. `run_one` (`:170-174`) aborts only on missing required lines. JSONL version gate (`:176-178`): `parsed["windows"] = [r for r in rows if "tick" in r]`.

The WA plan adds `"bazaar"` and `"crate"` entries to `ANCHORED` as optional.

---

## 10. test_sweep_parsing.py pin style

`python/tests/test_sweep_parsing.py`: versioned stdout fixture strings V1..V4 (V1 = RESULT+MEDIA only; V2 adds META+FUEL; V3 adds refuel tail; V4 adds strandings tail). Tests assert `parsed["field"] == "string_value"`. Negative version gate asserts `legacy["field"] is None`.

The WA plan appends V5 (BAZAAR line) and V6 (CRATE line) fixtures, following the V3→V4 extension pattern.

---

## 11. WGB phase-1 behavior-digest procedure

`plans/2026-06-11-world-gets-big-implementation.md` Task 1.2.7 (~line 3260-3285). Command shape:

```bash
git worktree add /tmp/wgb-pre-phase1 <PRE>
( cd /tmp/wgb-pre-phase1 && for S in 7 23; do
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --seed $S --ticks 2000 \
      --jsonl /home/john/jumpgate/runs/<dir>/base-s$S.jsonl \
      > /home/john/jumpgate/runs/<dir>/base-s$S.out; done )
for S in 7 23; do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed $S --ticks 2000 --jsonl runs/<dir>/head-s$S.jsonl \
    > runs/<dir>/head-s$S.out; done
for S in 7 23; do
  diff runs/<dir>/base-s$S.jsonl runs/<dir>/head-s$S.jsonl && \
  diff runs/<dir>/base-s$S.out  runs/<dir>/head-s$S.out; done
git worktree remove /tmp/wgb-pre-phase1
```

**Expected: every diff exits 0 with no output.** Any divergence is a determinism break — STOP, bisect commit-by-commit, never rationalize.

The spec (recommended-cut §6) extends this to `sha256 over stdout ∪ window-JSONL ∪ gossip-log per (scenario, seed)`. Phase-1 used `sha256sum` for JSONL (plan line ~1789). The WA digest adds `--gossip-log` to the runner call and includes those files in the hash set. Both builds must be `--release`.

Banked phase-1 exit digests (plan line 3305-3308): seed-7 JSONL `7e7be289...`, seed-23 JSONL `c38ed1c8...`, seed-7 stdout `744f284c...`, seed-23 stdout `a54384b1...`.

---

## 12. Panel idioms as WA templates

### w4_grid.py

20-seed ensemble (`w4_grid.py:35-38`):
```python
SEEDS = [7, 11, 13, 23, 29, 31, 37, 41, 42, 43, 47, 53, 57, 59, 61, 67, 71, 73, 99, 101]
```
Clean-seed filter (`w4_grid.py:74-79`): `blind-born verdict != PermanentPeace`. Hauler-prefix slice uses `int(cell["meta"]["haulers"])` NOT a constant. Quartile: `sorted_xs[(n-1)//4], [(n-1)//2], [3*(n-1)//4]` (lower-index, `:116-123`).

### haves_havenots.py — gossip-log + windows + stdout join

Loads three files, joins by alert-seq, reports per-hauler table. `parse_meta` decodes META line radii. Spearman with mean-rank tie breaking (`:80-110`).

### radial_zones.py — zone-aggregate + fuel_starve discriminator

Zone spec: `|`-delimited station-index groups (`:98` default `"0,1,2|3,4,5|6|7,8,9"`). `zone_series` aggregates routes touching the zone (`if fr in zset or to in zset`, `:35`). `fuel_starve` (`diagnostics.rs` equivalent in Python `:52-65`): NoStockout / BoomBust / DeathSpiral / Stockout / ShortRun — based on stock-series zeros vs traffic-quartile collapse.

All three panels: in `python/analysis/`, imported via `sys.path.insert(0, .../analysis)`. Tests use in-memory synthetic dicts, not real runs.

---

## 13. 20-seed ensemble defaults + two run lengths

Panel consensus (recommended-cut §3 "Run lengths DL5-1"):
- **50k ticks** — WA5 bank comparability (`population_cycles` raw alternation count, unnormalized, `diagnostics.rs:327-345`)
- **100k ticks** — per-good/WB/WC reads
- **20-seed W4 ladder** — every registered per-good read (WA1/WA2/WA3/WA4)

`sweep_trophic.py` default: `--ticks 50_000` (`:404`). The WA panel sweep wrapper passes `--ticks 100000` for per-good reads. The standard seed list is the SEEDS constant from w4_grid.py.

---

## GOTCHAS

1. **Route vectors have no goods dimension today.** `per_route_robs/accepts/traffic` are length `n_stations²` and aggregate all goods. Per-good HHI is computed script-side from the gossip-log accept-row `resource` key. Do NOT add `n_goods × n_stations²` per-good route vectors to TrophicSample.

2. **Lockstep rule: anchored stdout line + regex in the SAME commit.** Adding `BAZAAR` println! without simultaneously adding `BAZAAR_RE` to `ANCHORED` and a V5 fixture to `test_sweep_parsing.py` silently drops data from every sweep run.

3. **PermanentPeace is first in the verdict chain** (`diagnostics.rs:288`). WA panel scripts must use `clean_seeds` (filter `blind-born != PermanentPeace`) before reading WA5 verdict distributions — seeds where the war ended before market dynamics play out contaminate the distribution.

4. **Instruments (A0) must land before mechanics; digest baseline pinned at last A0 commit.** Running the cross-branch digest on a mixed A0+A1 commit will fail because the baseline JSONL has new A0 fields but the HEAD event counts are changed by the mechanics. The commit ordering is immutable: A0 first, baseline pinned, then A1+.

5. **Hauler-prefix slice `per_craft_credits[:haulers]` requires `meta["haulers"]` from the META line,** not a module-level constant. When Experiment C adds fleet-scale knobs, the slice length changes. The w4_grid.py fix (`:82-86`) is the precedent; new WA panels must read hauler count from the META line. Hard-coding `haulers = 12` or `haulers = 25` is the bug caught by L5-C2.
