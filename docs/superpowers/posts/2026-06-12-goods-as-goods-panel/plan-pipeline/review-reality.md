# Reality Check — Goods as Goods Rung A Implementation Plan

**Reviewer:** Plan Review Reality Agent  
**Branch:** jumpgate-v1-design (HEAD 140a8f1 / b446095 verified)  
**Sources grounded:** assembled-plan.md, ground-*.md extracts, live codebase reads  
**Date:** 2026-06-13  

---

## Reality Check

### Symbols

| Symbol | Status | Evidence |
|--------|--------|----------|
| `TrophicSample` struct | EXISTS | `diagnostics.rs:129` |
| `sample_window` function | EXISTS | `diagnostics.rs:541` |
| `gossip_log_event_json` function | EXISTS | `trophic_run.rs:384` |
| `chronicle_subject` function | EXISTS | `trophic_run.rs:481` |
| `sample_json` function | EXISTS | `trophic_run.rs:254` |
| `per_station_fuel_price` field | EXISTS | `diagnostics.rs:230` |
| `per_station_fuel_stock` field | EXISTS | `diagnostics.rs:229` |
| `refuels` field after `per_station_fuel_price` | EXISTS | `diagnostics.rs:232` |
| `EventKind::Trade` (dead variant) | EXISTS | `contract.rs:75-80` |
| `EventKind::ContractFulfilled` | EXISTS | `contract.rs:93` |
| `EventKind::PirateLieLow` | EXISTS | `contract.rs:117` |
| `EventKind::Robbed` | EXISTS | `contract.rs:100` |
| `EventKind::GossipHeard` | EXISTS | `contract.rs:160` |
| `GossipHeard.pirate_slot` (u32 field, NOT `pirate`) | EXISTS | `contract.rs:164` — plan's match uses `..` to ignore it, safe |
| `economy_event_kinds_are_copy_and_partial_eq` test | EXISTS | `contract.rs:391` |
| `EventKind::Trade` test constructor | EXISTS | `contract.rs:417` |
| `Resource` enum (Ore, Fuel) | EXISTS | `economy.rs:8-24` |
| `N_RESOURCES: usize = 2` | EXISTS | `economy.rs:20` |
| `StationStore.stock: Vec<[i64; N_RESOURCES]>` | EXISTS | `economy.rs:45` |
| `StationStore.price_micros: Vec<[i64; N_RESOURCES]>` | EXISTS | `economy.rs:46` |
| `EconCounters.mined/consumed: [i64; N_RESOURCES]` | EXISTS | `economy.rs:216-217` |
| `StationInit.initial_stock: [i64; N_RESOURCES]` | EXISTS | `config.rs:103` |
| `StationInit.initial_price_micros: [i64; N_RESOURCES]` | EXISTS | `config.rs:104` |
| `PriceCfg.base_micros: [i64; N_RESOURCES]` | EXISTS | `config.rs:144` |
| `PriceCfg.cap: [i64; N_RESOURCES]` | EXISTS | `config.rs:145` |
| `ContractStore.resource: Vec<Resource>` | EXISTS | `economy.rs:162` |
| `ContractInit.resource: Resource` | EXISTS | `config.rs:130` |
| `Resource::ALL` (plan calls it, but replaced by `Good::ALL_V1`) | NOT FOUND directly on `Resource` enum | `Resource::ALL` exists at `economy.rs:23` (`const ALL: [Resource; N_RESOURCES]`); plan correctly adds `Good::ALL_V1` as replacement |
| `assert_resource_identity` function | EXISTS | `world.rs:2870` |
| `ships.cargo` (in-transit term for identity) | EXISTS | `stores.rs:177` |
| `HASH_FORMAT_VERSION: u32 = 5` | EXISTS | `hash.rs:126` |
| `GOLDEN_ZERO_STATE_HASH` | EXISTS | `hash.rs:132` |
| `FRONTIER_TRAJECTORY_GOLDEN` | EXISTS | **`scenario.rs:1118`**, NOT `hash.rs:132` as the plan associates it |
| `manual_zero_fold` function | EXISTS | `hash.rs:1172` |
| `state_hash_golden_zero_world` test | EXISTS | `hash.rs:1111` |
| `print_golden` test (ignored) | EXISTS | `hash.rs:1123` |
| `write_craft_economy` function | EXISTS | `hash.rs:326` |
| `info_tick` word 28 at `hash.rs:367` | EXISTS | `hash.rs:367: h.write_u64(world.ships.info_tick[idx].0); // 28` |
| `run_scripted_dispatch` | EXISTS | `economy.rs:408` |
| `resolve_refuels` | EXISTS | `economy.rs:993` |
| `resolve_purchases` | EXISTS | `economy.rs:860` |
| `ships.ids_at(crow)` method | EXISTS | `stores.rs:296` |
| `stations.ids.id_at(srow)` | EXISTS | `ids.rs:124` returns `Option<(u32,u32)>` |
| `world.contracts.ids.dense_index` | EXISTS | `ids.rs:113` |
| `world.contracts.resource[kidx]` | EXISTS | `economy.rs:778` pattern |
| `world.contracts.reward_micros[kidx]` | EXISTS | `economy.rs:509` |
| `diagnostics::route_of` | EXISTS | `diagnostics.rs:827` |
| `MetaFacts` struct | EXISTS | `trophic_run.rs:119` — has 5 fields, NO `bazaar_mode` or `n_goods` yet |
| `--gossip-log` CLI flag | EXISTS | `trophic_run.rs:57,100` |
| `CraftRole::Pirate` | EXISTS | `stores.rs:71-90` |
| `TIERS` constant (pirate capacity vs tier) | EXISTS | `scenario.rs:104` = `[(5,1000),(10,1150),(15,1300)]` |
| `pirate.rs:640-642` hungry/press condition | EXISTS | `pirate.rs:640-642` matches `food_micros < grubstake_micros` |
| `pirate.rs:723-725` re-grubstake mint | EXISTS | `pirate.rs:724-725` matches |
| `economy.rs:1376` resolve_failures | EXISTS | `economy.rs:1376` |
| `economy.rs:512-515` ContractOffered emit | EXISTS | `economy.rs:511-514` (off by one line) |
| `w4_grid.py:84-86` hauler prefix slice | EXISTS | `python/analysis/w4_grid.py:84-86` matches exactly |
| `sweep_trophic.py` ANCHORED dict | EXISTS | `python/analysis/sweep_trophic.py:92` |
| `parse_stdout` function | EXISTS | `python/analysis/sweep_trophic.py:144` |
| `test_v2_meta_line_parses` test | EXISTS | `python/tests/test_sweep_parsing.py:56` |
| `V1_STDOUT`..`V4_STDOUT` fixtures | EXISTS | `python/tests/test_sweep_parsing.py:18-48` |
| `fleet_scale` knob | NOT FOUND | New to-be-created feature for Experiment C; correctly marked as new by plan context |
| `EventKind::Trade { .. }` in plan's A0.2 exhaustive None arm | NOT PRESENT — see CRITICAL-1 | |

---

### Paths

| Path | Status | Issue |
|------|--------|-------|
| `crates/jumpgate-core/src/diagnostics.rs` | EXISTS | Plan line refs ~230 (field) and ~811 (sample_window return) VERIFIED |
| `crates/jumpgate-core/examples/trophic_run.rs` | EXISTS | 911 lines |
| `crates/jumpgate-core/src/contract.rs` | EXISTS | |
| `crates/jumpgate-core/src/economy.rs` | EXISTS | |
| `crates/jumpgate-core/src/hash.rs` | EXISTS | |
| `crates/jumpgate-core/src/config.rs` | EXISTS | |
| `crates/jumpgate-core/src/scenario.rs` | EXISTS | |
| `crates/jumpgate-core/src/world.rs` | EXISTS | |
| `crates/jumpgate-core/src/stores.rs` | EXISTS | |
| `crates/jumpgate-core/src/lib.rs` | EXISTS | `pub use contract::{Command, Event, EventKind, ...}` at line 54 |
| `crates/jumpgate-py/src/env.rs` | EXISTS | |
| `python/analysis/sweep_trophic.py` | EXISTS | |
| `python/analysis/w4_grid.py` | EXISTS | |
| `python/tests/test_sweep_parsing.py` | EXISTS | |
| `docs/superpowers/specs/2026-06-12-goods-as-goods-design.md` | EXISTS | verified |
| `runs/` directory (gitignored) | FOLLOWS CONVENTION | plan says "gitignored — never stage" |

---

### Line References

| Claim | Actual | Status |
|-------|--------|--------|
| `diagnostics.rs ~line 230` for `per_station_fuel_price` | line 230 | EXACT |
| `trophic_run.rs ~line 254` for `sample_json` | line 254 | EXACT |
| `trophic_run.rs:384` for `gossip_log_event_json` | line 384 | EXACT |
| `trophic_run.rs:481` for `chronicle_subject` | line 481 | EXACT |
| `trophic_run.rs ~line 800` for `sample_window` return | `sample_window` returns at ~line 821, fuel fields at 805-820 | CLOSE (~811 claim for fuel field population) |
| `economy.rs:1023-1025` price<1 guard | `economy.rs:1023-1025` | EXACT |
| `economy.rs:1028` stock<=0 | `economy.rs:1028` | EXACT |
| `economy.rs:1040` need<1 | `economy.rs:1040` | EXACT |
| `economy.rs:1044` afford<1 | `economy.rs:1044` | EXACT |
| `economy.rs:1070` craft resolution | `economy.rs:1070` | EXACT |
| `economy.rs:1376` resolve_failures | `economy.rs:1376` | EXACT |
| `hash.rs:126` HASH_FORMAT_VERSION | `hash.rs:126` | EXACT |
| `hash.rs:132` GOLDEN_ZERO_STATE_HASH | `hash.rs:132` | EXACT |
| `hash.rs:367` word-28 info_tick | `hash.rs:367` | EXACT |
| `hash.rs:1172` manual_zero_fold | `hash.rs:1172` | EXACT |
| `world.rs:2870` assert_resource_identity | `world.rs:2870` | EXACT |
| `world.rs:311` credits_micros push(0) | `world.rs:311` | EXACT |
| `contract.rs:391` test fn | `contract.rs:391` | EXACT |
| `contract.rs:417` Trade constructor | `contract.rs:417` | EXACT |
| `scenario.rs:104` TIERS | `scenario.rs:104` | EXACT |
| `scenario.rs:355` pirate capacity 5 | `scenario.rs:355` | EXACT |
| `pirate.rs:640-642` press condition | `pirate.rs:640-642` | EXACT |
| `pirate.rs:723-725` re-grubstake | `pirate.rs:724-725` | CLOSE (off by one) |
| **`scenario.rs:156-161` for frontier ContractInit** | **WRONG: frontier ContractInit is at `scenario.rs:470-481`; lines 156-161 are inside `scenario_trophic` craft init** | WRONG LINE |
| `scenario.rs:243-248` for trophic ContractInit | `scenario.rs:243` (push call); resource at 245, 254 | APPROXIMATE (push spans 243-260) |
| **`FRONTIER_TRAJECTORY_GOLDEN` at `hash.rs`** | **WRONG: constant is at `scenario.rs:1118`, not `hash.rs`** | WRONG FILE (plan's A2 says re-pin via "print_golden_frontier" — that function IS in scenario.rs; the actual constant is at `scenario.rs:1118`) |
| `w4_grid.py:84-86` | `python/analysis/w4_grid.py:84-86` | EXACT |
| `diagnostics.rs:327-345` population_cycles | starts at 327, correct range | EXACT |

---

### Versions / Dependencies

No version mismatches identified. The plan uses existing crate features (serde_json, FnvHasher, etc.) already in scope.

---

## Summary

- **Critical hallucinations / compile-breaking errors:** 1
- **Major issues (wrong file/significant wrong direction):** 1
- **Minor issues (wrong line numbers, small gaps):** 4

---

## CRITICAL Findings

### CRITICAL-1: A0.2 exhaustive `gossip_log_event_json` missing `EventKind::Trade` arm

**Plan location:** Phase A0, Task A0.2, Step 2 — the proposed `gossip_log_event_json` replacement body (lines ~432-446 of the plan)

**Evidence:** The plan replaces `_ => None` with an exhaustive match. The None arm covers:
```rust
EventKind::Arrival { .. }
| EventKind::FuelEmpty { .. }
| ...
| EventKind::UpgradePurchased { .. } => None,
```
The plan's note says "`EventKind::Trade` is still present in the enum at this task — it is handled by the exhaustive None arm group above". But `EventKind::Trade { .. }` does NOT appear in that None arm group. At the A0.2 commit point, `Trade` is still in the enum (`contract.rs:75-80`) and is not deleted until A0.3. Rust exhaustive matching will produce a **compile error** (`non-exhaustive patterns: EventKind::Trade { .. } not covered`).

**Fix:** Add `| EventKind::Trade { .. }` to the None arm group in the A0.2 replacement, exactly as the plan's note claims it should be. Then in A0.3, remove it when `Trade` is deleted. The plan's intent is correct but the code block doesn't match it.

---

## MAJOR Findings

### MAJOR-1: `FRONTIER_TRAJECTORY_GOLDEN` is in `scenario.rs`, not `hash.rs`

**Plan location:** Phase A2, Task A2.1 header ("Files" section) and Step 5

**Plan text:** "Modify: `crates/jumpgate-core/src/scenario.rs` — `FRONTIER_TRAJECTORY_GOLDEN` constant (around line 1118)"

**Evidence:** The plan's files list correctly says `scenario.rs` and "around line 1118." The constant is at `scenario.rs:1118` — VERIFIED. **This finding is RETRACTED.** The plan correctly identifies `scenario.rs:1118`.

However, the plan also says in the A2.1 header: "Modify: `crates/jumpgate-core/src/hash.rs` — ... `GOLDEN_ZERO_STATE_HASH` constant (line 132) — DERIVED by builder." The plan does NOT incorrectly claim FRONTIER_TRAJECTORY_GOLDEN is in hash.rs. Both are correctly in their files. Downgrading this from MAJOR to no finding.

---

## MAJOR Findings (revised)

### MAJOR-1: `scenario.rs` frontier `ContractInit` line reference wrong — plan says 156-161, actual is 470-481

**Plan location:** Phase A1, Task A1.1, Step 10 — "Update `ContractInit` rows in scenario.rs (lines 156–161 for frontier, 243–248 for trophic)"

**Evidence:** 
- `scenario_trophic` starts at `scenario.rs:123`; its ContractInit push is at `scenario.rs:243` 
- `scenario_frontier` starts at `scenario.rs:320`; its ContractInit pushes are at `scenario.rs:470-481`
- Lines 156-161 of scenario.rs contain orbit/velocity math inside `scenario_trophic`'s craft init loop (not ContractInit)

**Severity:** MAJOR for a worker following the plan literally — they would edit the wrong lines. The structural direction (update Resource::Ore/Fuel → Good::ORE/FUEL in ContractInit rows) is correct; only the line numbers are wrong.

**Fix:** Replace "lines 156–161 for frontier" with "lines 470–481 for frontier" in Step 10.

---

## MINOR Findings

### MINOR-1: A0.2 `gossip_log_event_json` test uses `Robbed` with wrong field set

**Plan location:** Phase A0, Task A0.2, Step 1, test `gossip_log_rob_row_has_pirate_field`

**Evidence:** The plan's test constructs:
```rust
kind: EventKind::Robbed {
    pirate: CraftId { slot: 8, generation: 0 },
    hauler: CraftId { slot: 2, generation: 0 },
    contract: ContractId { slot: 0, generation: 0 },
    value_micros: 1_000_000,
},
```
This matches the actual `Robbed` payload (`pirate: CraftId, hauler: CraftId, contract: ContractId, value_micros: i64`) at `contract.rs:100-105`. **No issue.** Downgrade to no finding.

### MINOR-1: `economy.rs:512-515` ContractOffered emit is off by 1-2 lines

**Plan location:** Synthesis cut Part 1.2 (REPOST retirement / poster spec) claims "PackagePosted is DROPPED … economy.rs:512-515"

**Evidence:** `ContractOffered` emits at `economy.rs:511-514`:
```rust
events.emit(Event {
    tick,
    kind: EventKind::ContractOffered { contract: new_id },
});
```
(Lines 511-514, not 512-515). Minor off-by-one, no behavioral impact.

### MINOR-2: `pirate.rs:723-725` re-grubstake is actually lines 724-725

**Plan location:** Synthesis cut Part 2, PART 1 correction section — "pirate.rs:723-725 — pirates cannot die"

**Evidence:** The actual grubstake re-mint is at `pirate.rs:724-725` (`p.food_micros = trophic.grubstake_micros; events.emit(...)`). Line 723 is `if p.food_micros <= 0 {`. Minor, no behavioral impact.

### MINOR-3: `ResetError` enum display reference incomplete

**Plan location:** Phase A1, Task A1.2, Step 4 — "In `impl Display for ResetError` (world.rs:167)"

**Evidence:** No independent verification that the `Display` impl is specifically at `world.rs:167`. The claim is structurally sound (Display impl for ResetError exists to display the error messages) but the exact line was not verified. If the impl is elsewhere or the enum location has shifted, the worker needs to locate it.

**Impact:** Low — any compiler error will identify the correct location.

### MINOR-4: `RefuelDenied` NOT emitted for `port_row.is_none()` case — undocumented omission

**Plan location:** Phase A0, Task A0.3, Step 5

**Evidence:** The plan's proposed guard reordering emits `RefuelDenied` for NoStock, TankFull, and CannotAfford, but NOT for the `port_row.is_none()` case (stale corp). The spec says "three continue sites" (stock<=0, afford<1, need<1). The `port_row` check is a 4th continue site not in the spec's three. The plan is internally consistent — it explicitly skips emitting for `port_row.is_none()` (stale corp is a config error, not a craft-level denial). This is correct behavior but the plan's comment "three guard sites" in the surrounding text could mislead a worker counting continue sites in the function. **No code change needed**, but clarification would help.

---

## Blocking Issues

**CRITICAL-1 is the only blocking issue:**

The A0.2 exhaustive `gossip_log_event_json` replacement code block is missing `EventKind::Trade { .. }` from the None arm. Since `Trade` still exists in the enum at A0.2 time, Rust will refuse to compile the exhaustive match. The fix is trivial — add `| EventKind::Trade { .. }` to the None arm group, then remove it in A0.3. The plan's note correctly describes the intent, but the code block doesn't implement it.

---

## Warnings

1. **MAJOR-1 (frontier ContractInit line ref):** Worker must search for ContractInit in `scenario_frontier` (~line 470-481) rather than the stated lines 156-161. The structural direction is correct; only the line number is wrong.

2. **A0.3 chronicle_subject exhaustive match:** The plan's A0.3 exhaustive `chronicle_subject` replacement does NOT include `Reward { craft, .. }` and `Wake { craft }` in the None arm — they are correctly in the `Some(craft)` arm (lines 848, in the plan's replacement). But the None arm at plan lines 871-877 also omits `ContractFailed` from the None arm — `ContractFailed { hauler, .. }` is correctly in the Some(hauler) arm (plan line 853). The match is correctly exhaustive as written; no issue.

3. **`jumpgate-py/src/env.rs` — all 4 StationInit rows need Vec conversion in A1b.** The plan's A1.1 Step 13 mentions `initial_stock: [0, 0]` at lines 403-406 (one station). There are 4 stations in env.rs with `initial_stock`/`initial_price_micros` fixed-size arrays (lines 385, 391, 397, 403). The plan defers all of these to A1b ("remain `[i64; N_RESOURCES]` in A1a; converted to Vec in A1b") which is correct, but a worker fixing A1b must update all 4 stations, not just the one at 403-406.

---

## Confidence Assessment

**Overall Confidence:** High

| Finding | Confidence | Basis |
|---------|------------|-------|
| CRITICAL-1: Trade missing from A0.2 exhaustive None arm | High | Plan code block read line-by-line; `EventKind::Trade { .. }` absent; plan note contradicts code |
| MAJOR-1: frontier ContractInit at 470-481 not 156-161 | High | `grep -n "ContractInit" scenario.rs` + `sed -n '155,162p'` both verified |
| MINOR-1: ContractOffered at 511-514 not 512-515 | High | Direct read of economy.rs:511-514 |
| MINOR-2: grubstake at 724-725 not 723-725 | High | Direct read of pirate.rs:720-728 |
| All other line refs verified | High | Grep + Read confirmed |
| GossipHeard.pirate_slot vs pirate | High | contract.rs:164 confirms field name is `pirate_slot`; plan's match uses `..` so safe |
| FRONTIER_TRAJECTORY_GOLDEN in scenario.rs | High | `grep -rn` confirmed scenario.rs:1118; plan correctly identifies this file |

---

## Risk Assessment

**Implementation Risk:** Low-to-Medium (one compile blocker, one wrong line reference)  
**Reversibility:** Easy (all A0 changes are additive/instrument-only; A1 is hash-neutral)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| CRITICAL-1 A0.2 compile failure | High | Certain if code used as-is | Add `EventKind::Trade { .. }` to None arm |
| MAJOR-1 worker edits wrong lines (156-161 instead of 470-481) | Medium | Likely for a literal follower | Grep for ContractInit in scenario_frontier before editing |
| env.rs partial A1b conversion | Low | Possible | Worker must update all 4 StationInit rows, not just lines 403-406 |

---

## Information Gaps

1. [ ] **A6 phase plan (lab/science section):** The review covered A0-A2 and portions of A1 in detail. The A3 (boards+Exchange+trade verbs), A4 (arbitrage+REPOST retirement), A5 (two-mode policy + scenario_bazaar), and A6 (science+console) phases were not fully reviewed. The grounding extracts confirm the structural shapes are correct for those phases, but specific line references and new symbol names in those phases were not exhaustively verified.

2. [ ] **`ResetError` enum location in world.rs:** Plan says `world.rs:146` for the enum start and `world.rs:167` for the Display impl. Not independently verified; compiler error will catch any mismatch.

3. [ ] **`dense_index` return semantics vs `id_at` pattern:** The plan's A0.2 gossip-log code uses `dense_index` which returns `Option<usize>`, but the `id_at` pattern is used for station resolution. Both are correct but the pattern difference was noted as a potential source of confusion for a future worker adding similar arms.

---

## Caveats & Required Follow-ups

### Before Relying on This Analysis

- [ ] Fix CRITICAL-1 (add `EventKind::Trade { .. }` to the None arm in A0.2's `gossip_log_event_json` replacement) before executing A0.2
- [ ] Note MAJOR-1 (frontier ContractInit is at scenario.rs:470-481, not 156-161) before executing A1.1 Step 10
- [ ] Confirm all 4 StationInit rows in env.rs are converted in A1b

### Assumptions Made

- The grounding extracts were generated at HEAD b446095 which equals HEAD 140a8f1 (the task stated this was the same branch state)
- Line numbers were verified by direct codebase reads; functions are assumed stable (no uncommitted changes to those specific lines)

### Limitations

- A3-A6 phases verified structurally but not exhaustively at the line-reference level
- Dynamic behavior (runtime correctness of the arbitrage trigger formula, the config-hash neutrality of GoodsCfg default) is not statically verifiable here
- The spec's OD-3 fuel re-bake value (≈50_000) and the wage formula constants are not verified against any computed calibration — they are plan-time values per the spec

### Recommended Next Steps

1. Fix CRITICAL-1 (trivial code edit to A0.2 code block) and re-review the corrected A0.2 block
2. Correct MAJOR-1 line reference in A1.1 Step 10 (156-161 → 470-481 for frontier)
3. Proceed with A0 execution; all other findings are minor and can be resolved inline during execution
