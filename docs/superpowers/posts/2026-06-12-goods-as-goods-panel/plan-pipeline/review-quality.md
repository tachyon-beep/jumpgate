# Quality Review — Goods as Goods Rung A (assembled-plan.md)

**Reviewer:** Quality / QA agent  
**Date:** 2026-06-13  
**Spec:** `docs/superpowers/specs/2026-06-12-goods-as-goods-design.md` (D1-D8 + OD-1..7)  
**Panel cut:** `docs/superpowers/posts/2026-06-12-goods-as-goods-panel/synthesis-recommended-cut.md` (17 CRITICALs)  
**Grounding:** `ground-contracts-conservation.md`, `ground-lab.md`, `ground-economy-verbs.md`, `ground-craft-stores.md`, `ground-scenario-config.md`

---

## Quality Review

### Test Strategy

**Test approach:** TDD — every behavioral step specifies a failing test before implementation. Mechanical refactor steps (A1a, A1b, A3.3) specify compile-error failures as the failing signal. Digest cross-checks serve as integration tests at phase exits.

| Phase/Task | Test Type | File | Command | Status |
|---|---|---|---|---|
| A0.1 per-station matrices | Unit (compile-fail first) | `diagnostics.rs` tests | `cargo test sample_window_has_per_station_stock_and_price_matrices` | OK |
| A0.2 gossip-log rows | Unit (4 tests) | `trophic_run.rs` tests | `cargo test gossip_log_encodes_contract_fulfilled_as_deliver` etc. | OK |
| A0.3 RefuelDenied event | Unit (compile-fail first) | `trophic_run.rs` tests | `cargo test chronicle_subject_threads_refuel_denied_to_craft` | OK |
| A0.4 BAZAAR line | Python unit | `test_sweep_parsing.py` | `pytest test_v5_bazaar_line_parses_and_older_reads_none` | OK |
| A0.5 META goods= tail | Python unit | `test_sweep_parsing.py` | `pytest test_meta_goods_tail_is_none_for_trophic_frontier` | OK |
| A0.6 digest baseline | Integration (sha256 + replay-check) | `runs/` (gitignored) | manual runs + sha256sum | OK |
| A1.1 Good(u16) newtype | Unit (compile-fail) | `economy.rs` tests | `cargo test good_ore_and_fuel_pinned_indices` | OK |
| A1.1 cross-branch proof (A1a) | Hash-neutrality (ignored test) | `hash.rs` | `cargo test print_trophic_tick_hashes_1000 --ignored` | OK |
| A1.2 GoodsCfg, BadGoodsCfg | Unit (compile-fail) | `world.rs` tests | `cargo test bad_goods_cfg` | OK |
| A1.2 cross-branch proof (A1b) | Hash-neutrality (2 scenarios) | `hash.rs` | `cargo test print_frontier_tick_hashes_2000 --ignored` | OK |
| A2.1 hold column fold | Unit (compile-fail) | `hash.rs` tests | `cargo test hold_column_folds_into_state_hash_v6` | OK |
| A2.2 identity includes hold | Unit (compile-fail at runtime) | `world.rs` tests | `cargo test hold_participates_in_resource_identity` | **CRITICAL-1 gap** |
| A2.3 transient columns | Unit (debug-assert) | `hash.rs` tests | `cargo test pending_trade_columns_assert_fires_on_leftover_intent` | OK |
| A2.4 Food consumption | Unit | `scenario.rs` tests | `cargo test scenario_bazaar_has_food_consumption_recipe` | OK |
| A2.5 digest cross-check | Integration | `/tmp/` | sha256sum comparison | OK |
| A3.1 update_prices generalized | Unit | `economy.rs` tests | `cargo test update_prices_runs_for_all_goods_and_respects_cap_zero` | **MAJOR-1 gap** |
| A3.2 config hash | Unit | `config.rs` tests | `cargo test goods_cfg_arb_cfg_exchange_cfg_are_config_hashed` | **CRITICAL-2 / duplicate-struct issue** |
| A3.3 pending columns | Unit (debug-assert) | `hash.rs` tests | `cargo test pending_trade_buy_not_none_at_hash_point_panics` | **MAJOR-2: duplicates A2.3** |
| A3.4 TradeBought/TradeSold | Unit | `contract.rs` tests | `cargo test economy_event_kinds_are_copy_and_partial_eq` | **CRITICAL-3: Trade delete conflict** |
| A3.5 trade policy | Unit (3 tests) | `economy.rs` tests | `cargo test trade_policy_*` | OK |
| A3.6 resolve_trade_buys/sells | Unit (5 tests incl. credit identity) | `economy.rs` tests | `cargo test trade_buy_settles_*` + `credit_identity_holds_across_trade_buy_and_sell` | **MAJOR-3: identity extension duplicate** |
| A3.7 EXCHANGE line | Python unit | `test_sweep_parsing.py` | `pytest test_exchange_line_is_parsed` | **MINOR-1: module path wrong** |
| A4.1 OfferWithdrawn | Unit | `contract.rs` tests | `cargo test offer_withdrawn_event_is_copy_and_partial_eq` | OK |
| A4.2 ArbitrageCfg | Unit | `config.rs` tests | `cargo test arbitrage_cfg_is_folded_and_inert_by_default` | **CRITICAL-2: second config re-pin** |
| A4.3 REPOST off | Unit | `economy.rs` tests | `cargo test repost_structural_off_posts_nothing` | OK |
| A4.4 arbitrage poster | Unit | `economy.rs` tests | `cargo test arbitrage_poster_posts_when_spread_clears_transport` | OK |
| A5 scenario_bazaar | Integration | trophic_run + BAZAAR line | bazaar smoke run | **MAJOR-4: no explicit test for ASSIGN empty-hold gate L3-M3** |
| A5 WA3/WA5 joint-read | Lab read | panel script | console | **CRITICAL-4: WA3/WA5 joint-read not encoded in A5 instrument** |
| A6 WA windows | Lab reads | panel script | console | WA1/WA2/WA4 OK, WA3 see CRITICAL-4 |

**Coverage:** Not numerically specified; structural coverage (happy path + skip arms + identity invariants) is good in A3.6. Phase-exit digests are the integration gate.

---

### Observability

| Error Path | Logging / Observability | Status |
|---|---|---|
| REPOST structural off proof | ONLY a cargo test + within-build digest described; no explicit "before vs after" test command in A4.3 | PARTIAL |
| Exchange drain | EXCHANGE stdout line added (A3.7); drain_per_100k=0 placeholder throughout — treasury drawdown is NOT observable at phase exit | PARTIAL |
| RefuelDenied emit | Event + chronicle arm (A0.3); NoStock/CannotAfford/TankFull reasons emitted | OK |
| TradeBought/TradeSold events | Gossip-log arms added (A3.4) | OK |
| OfferWithdrawn event | Chronicle arm (A4.1); no gossip-log row (by design, returns None) | OK |
| Arbitrage poster inert on trophic/frontier | Digest comparison at A1/A2/A5 phase exits | OK |
| WA3 own-trade share | `TradeBought`/`TradeSold` gossip-log rows readable; but no instrument for the joint-read (see CRITICAL-4) | PARTIAL |
| Exchange battery drain alert | OD-2: "consumption-minted money named trigger if console shows heat death" — no printed alert when Exchange treasury < threshold | MINOR |

---

### Edge Cases

| Feature | Edge Cases Addressed | Missing / Gaps |
|---|---|---|
| Price < 1 guard (L2-C3) | Cloned into every settle arm (buy/sell/fence) | OK |
| Hold capacity gate | Integer milli-mass arithmetic at uniform 1000 in A3.6 | OK |
| GoodsCfg length=0 | `BadGoodsCfg` rejects before tick 0 (A1.2) | OK |
| Stock <= 0 skip | Present in resolve_trade_buys, run_trade_policies | OK |
| Stale Exchange corp row | `.id_at()` guard in buy/sell settle | OK |
| Exchange saturating sell | `trade_sell_exchange_saturates_when_broke` test | OK |
| Pirate skip in trade policy | `trade_policy_skips_pirate` test | OK |
| Day-0 wallet = 0 | Noted; all craft start in wage-hauling mode | OK |
| ASSIGN empty-hold gate (L3-M3) | Mentioned in A3.5 text ("same-phase obligation") but **no test** exists for it in any task | MAJOR-4 |
| Corp committed-scratch over-posting | `committed[corp]` scratch noted in A4.4; tested by `arbitrage_poster_posts_when_spread_clears_transport` indirectly | MINOR |
| n_goods cap at GoodsCfg fold | COUNT FIRST anti-aliasing delimiter specified | OK |
| Food == Good(2) index pinned | `good_food_has_index_2` test | OK |
| Toll_milli > 1000 validation | Rung B; not in plan scope | N/A |

---

### Security

No raw SQL, eval, shell=True, or dangerouslySetInnerHTML patterns. All arithmetic is integer (no floats in hot paths per PDR). No hardcoded secrets.

| Pattern | Location | Status |
|---|---|---|
| Integer overflow | Saturating mul/add used throughout | OK |
| OOB array index | `.get(r)` / `.id_at()` guards on Vec accesses | OK |
| `i64 as u16` truncation in Good fold | `good.0 as u64` in hash fold (A2.1 step 3) — `Good(u16)` → safe | OK |

---

### Production Readiness

| Element | Status | Notes |
|---|---|---|
| Behavior digests at phase exits | A0 (baseline), A2 (cross-check), A5 (rung exit) specified | OK |
| Replay-check in A0 baseline | 4 runs required | OK |
| Exchange drain observable | PARTIAL — drain_per_100k=0 placeholder; no alert | PARTIAL |
| Rollback plan | Implicit: A1 is hash-neutral (rollable); A2 v6 bump is a single-cause commit; GOLDEN_CONFIG_HASH re-pin is documented | OK |
| WA windows RECORDED, never gated | Spec §5 window text says "recorded, never gated"; plan respects this for WA1-WA4 | OK (see CRITICAL-4 for WA3) |
| Config inert defaults for trophic/frontier | `scan_interval=0`, `exchange.active=false`, `demand_low/high=0` are all structural offs | OK |
| No behavior-change commit before A0 baseline | Plan enforces "A0 first, baseline pinned, then A1+" | OK |

---

## Summary

- **Test gaps:** 4 (CRITICAL-1, CRITICAL-3, MAJOR-1, MAJOR-4)
- **Observability gaps:** 3 (CRITICAL-4, MAJOR-3, MINOR-1)
- **Edge cases missing:** 2 (MAJOR-4, MINOR-2)
- **Security issues:** 0
- **Structural / sequencing issues:** 2 (CRITICAL-2, CRITICAL-3)

---

## Blocking Issues

### CRITICAL-1: A2.2 `assert_resource_identity` hold extension is placed in the WRONG commit and A3.6 re-extends it (double obligation)

**Location:** Plan phase A2, Task A2.2 (plan line ~3033); also Phase A3, Task A3.6 Step 4 (plan line ~5132).

**Evidence:** 
- A2.2 Step 2 (plan line ~3090) presents `assert_resource_identity` using the old `N_RESOURCES` constant and `[i64; N_RESOURCES]` signature, which already conflicts with A1b's Vec migration (A1b Step 10 converts this to `&[i64]` and `Vec<i64>`). The A2.2 implementation shown still references `use crate::economy::N_RESOURCES;` and `for r in 0..N_RESOURCES` — but A1b removes `N_RESOURCES`.
- A3.6 Step 4 (plan line ~5132) re-extends `assert_resource_identity` again with a **different signature** — still using `&[i64; crate::economy::N_RESOURCES]`. This is the third version of the same function in three different tasks, creating a sequencing conflict.

**Impact:** One of these steps will encounter a compilation error or silently overwrite the other's work. The synthesis cut (Part 1.2) says the hold sum is "a same-commit obligation in A2" — the plan splits it across A2.2 and A3.6 while also migrating the signature in A1b.

**Fix:** Consolidate: the Vec-migrated signature from A1b (`&[i64]`) must be the only version. A2.2 should present the final Vec-based body, and A3.6 Step 4 must be removed entirely (or reduced to an assertion that A2.2 already did this). Verify the A2.2 body uses `for r in 0..initial.len()` not `for r in 0..N_RESOURCES`. The A3.6 step 4 body should be deleted from the plan.

---

### CRITICAL-2: TWO GOLDEN_CONFIG_HASH re-pins for rung A — plan allows both A3.2 AND A4.2 to each do a single re-pin, contradicting the spec's "ONE per rung" rule

**Location:** A3.2 title ("the ONE rung-A GOLDEN_CONFIG_HASH re-pin", plan line ~3850); A4.2 Step 3 adds a second re-pin ("GOLDEN_CONFIG_HASH re-pin — single-cause commit", plan line ~5563).

**Evidence:** 
- Spec §6: "one GOLDEN_CONFIG_HASH re-pin per rung, cause-documented."
- Synthesis cut §1.1 (A3): "ONE GOLDEN_CONFIG_HASH re-pin" for rung A.
- A3.2 adds GoodsCfg + ArbitrageCfg + ExchangeCfg + arb_premium_micros in ONE config commit and re-pins.
- A4.2 then adds `ArbitrageCfg` **again** as a separate struct with different fields (`qty_ladder`, `transport_micros` as a Vec) and re-pins a second time.
- Both tasks define `ArbitrageCfg` with different field sets: A3.2 version has `{scan_interval, wage_flat_micros, wage_share_milli, max_posts_per_scan}`; A4.2 version has `{scan_interval, transport_micros: Vec<i64>, wage_share_milli, qty_ladder: Vec<u32>, max_posts_per_scan: usize}`. These are **structurally incompatible definitions of the same struct in the same crate**.

**Impact:** The plan will fail to compile as written — two `ArbitrageCfg` struct definitions cannot coexist. The config hash is re-pinned twice, violating the spec's "one per rung" discipline.

**Fix:** Merge the two `ArbitrageCfg` definitions into one complete struct in A3.2. A4.2 should reference the A3.2 definition and confirm it's already present, not redefine it. The `transport_micros: Vec<i64>` and `qty_ladder: Vec<u32>` fields must be included in A3.2's definition. The GOLDEN_CONFIG_HASH is re-pinned exactly once (in A3.2).

---

### CRITICAL-3: `EventKind::Trade` is deleted TWICE — once in A0.3 and once in A3.4

**Location:** A0.3 Step 3 ("In `EventKind`, REMOVE the `Trade` variant", plan line ~601); A3.4 Step 2 ("DELETE: Trade { ... }", plan line ~4290).

**Evidence:**
- A0.3 explicitly removes `Trade` from the enum in `contract.rs` and removes it from all exhaustive matches.
- A3.4 Step 2 again attempts to delete `Trade`, with its own updated copy of `economy_event_kinds_are_copy_and_partial_eq` that still references `Resource::Ore` (which is deprecated post-A1a and deleted post-A1b).
- A3.4 Step 2's test also imports `use crate::economy::{Good, Resource};` — but `Resource` is removed in A1b.

**Impact:** A3.4 will encounter a compile error on `Resource::Ore` (removed post-A1b) and will try to delete an already-deleted variant. If executed as written, it will produce a confusing error that looks like a test failure rather than a plan sequencing bug.

**Fix:** A3.4 Step 2 must remove the `Trade` deletion (already done in A0.3). The test `economy_event_kinds_are_copy_and_partial_eq` in A3.4 must use `Good::ORE` not `Resource::Ore`. A3.4 should only add `TradeBought`/`TradeSold` and update the exhaustive matches.

---

### CRITICAL-4: WA3/WA5 joint-read rule is NOT encoded in any plan instrument or panel script step

**Location:** Spec §5 WA3 + WA5; synthesis cut Part 3 "carry the WA5 caveats (the WA3 joint read)"; spec §6b panel record "WA3 and WA5 are a joint read (own-trade share IS the pirate food supply)".

**Evidence:**
- The spec and panel record are unambiguous: "WA3 and WA5 are a joint read" — own-trade share (WA3) IS the pirate food supply indicator, so you cannot read WA5 (trophic preservation) without simultaneously reading WA3 (channel mix). If WA3 share is low, WA5 reads are confounded.
- The plan's A6 phase (lab / science) was not read in detail (it references `draft-a6.md`) but the A5 summary for scenario_bazaar does not include any instrument or panel script that encodes this joint constraint.
- The synthesis cut (Part 3) says: "carry the WA5 caveats (closed re-fit, outcomes_disperse saturation, the WA3 joint read)" — this is not a window gate, but it must appear as a mandatory co-reported column in any WA5 sweep output. No such co-reporting obligation is specified in the observable plan tasks.
- The WA3 metric requires `TradeBought`/`TradeSold` gossip-log rows joined by craft to derive per-craft channel share over a run window. This join is not defined anywhere in the plan's Python panel descriptions.

**Impact:** A builder reading this plan will produce WA5 verdict-mix output without the WA3 co-read. The panel's binding constraint ("never read WA5 without WA3") will be silently violated. The window metric will be uninterpretable.

**Fix:** In Phase A6 (lab), explicitly add:
1. A `wa3_channel_mix` reader: script-side join of `TradeBought`/`TradeSold` rows per craft → `own_trade_share` column in the sweep output.
2. A co-report obligation: every WA5 row in sweep output must carry a `wa3_own_trade_share` column alongside the verdict.
3. A failing test in the panel script (`test_wa5_output_has_wa3_column`) that asserts the joint read is present in sweep output.

---

## Warnings (MAJOR)

### MAJOR-1: A3.1 `update_prices` test fixture references non-existent methods and types

**Location:** A3.1 Step 1 (plan line ~3714), the failing test body.

**Evidence:**
- The test calls `StationStore::empty_with_goods(n)` (plan line ~3729) — no such method exists in `ground-craft-stores.md` or `ground-economy-verbs.md`. `StationStore` has `push(body, stock, price_micros)` and an implied `empty()`, but no `empty_with_goods(n)` constructor.
- The test calls `station.push_goods(/* body_id placeholder */ 0, vec![...], vec![...])` (plan line ~3730) — also non-existent; the method is `push(body: BodyId, stock: Vec<i64>, price_micros: Vec<i64>)`.
- The test imports `use crate::economy::{EconCounters, Good, GoodsCfg, GoodSpec, PriceCfg, StationStore}` — `GoodsCfg` and `GoodSpec` are config structs (in `config.rs`), not `economy.rs`. The import path is wrong post-A1.
- `update_prices` signature in the test passes `&goods_cfg` as a third argument, but in the existing code it takes `(stations, price_cfg, tick, events)`. Step 2 adds the parameter — but the Step 1 failing test must fail at compile because the function with the new signature doesn't exist yet, which is correct intent but the fixture helper method names are wrong and will cause a different compile error.

**Fix:** Replace `StationStore::empty_with_goods(n)` with the actual constructor pattern (create an empty `StationStore`, then push a station with `Vec<i64>` stock/price). Replace `push_goods(0, ...)` with `push(BodyId{slot:0,generation:0}, vec![...], vec![...])`. Fix imports for `GoodsCfg`/`GoodSpec` to come from `crate::config::`.

---

### MAJOR-2: A2.3 and A3.3 define structurally incompatible `TradeBuyIntent` / `pending_trade_buy` column types that will conflict

**Location:** A2.3 (plan line ~3167) defines `TradeBuyIntent { station_row: usize, good: Good, qty: u32 }` and `pending_trade_buy: Vec<Option<TradeBuyIntent>>`; A3.3 (plan line ~4120) redefines `pending_trade_buy: Vec<Option<(Good, u32, StationId)>>` as a tuple.

**Evidence:**
- A2.3 defines `TradeBuyIntent` as a named struct in `stores.rs` and uses it in the hash debug assert.
- A3.3 Step 2 re-adds `pending_trade_buy: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>` as a tuple type, inconsistent with A2.3's named struct.
- A3.5 (run_trade_policies, plan line ~4606) sets `ships.pending_trade_buy[crow] = Some((good, qty, sid))` — the tuple form.
- A3.6 (resolve_trade_buys, plan line ~4935) destructures `let Some((good, qty, src_sid)) = ships.pending_trade_buy[crow]` — also the tuple form.
- If A2.3 lands first with the named struct, A3.3 can't re-add the same field with a different type without removing A2.3's definition first.

**Impact:** One of two outcomes: (a) A2.3 is meant to be a PLACEHOLDER task that A3.3 supersedes, but there's no "remove A2.3's type in A3.3" step, or (b) both are meant to coexist with the named struct, but A3.5/A3.6 use tuple destructuring that won't compile against the named struct. Either way, the plan will not compile as written.

**Fix:** Choose one canonical type. The synthesis cut (Part 1.2) says `pending_trade_buy/sell` follow the "pending_upgrade payload shape" — the A3.3 tuple `(Good, u32, StationId)` is simpler and matches what A3.5/A3.6 use. Either: (a) Remove A2.3 entirely (the columns are not needed until A3.3), or (b) Change A2.3's type to match the tuple form used in A3.3/A3.5/A3.6. The debug_assert test in A2.3 can be moved to A3.3 if A2.3 is removed. Note that `TradeSellIntent { station_row: usize }` in A2.3 becomes `StationId` in A3.3 — same conflict.

---

### MAJOR-3: `credit_identity_holds_across_trade_buy_and_sell` (A3.6) is placed AFTER the Exchange legs are live but the credit identity test in A3.6 does NOT extend the existing world-level credit identity test

**Location:** A3.6 Step 1 (plan line ~4882), test `credit_identity_holds_across_trade_buy_and_sell`.

**Evidence:**
- The existing credit identity test (`phase2_credit_identity_holds_every_tick`, world.rs:3315) runs `Σtreasury+Σcredits+Σescrow` every tick on a full world. It does NOT include Exchange legs because exchange.active=false on trophic/frontier.
- A3.6's new test uses a custom `credit_identity_trade_fixture()` function (not defined in the plan — plan line ~4888 just calls it). The fixture is not shown; the builder must invent it.
- The existing world-level identity test will NOT cover the Exchange legs automatically because the world it runs has `exchange.active=false`. The plan does not add a step to extend `phase2_credit_identity_holds_every_tick` with a bazaar-scenario world.
- Ground-contracts-conservation.md §8 lists six lawful legs — the Exchange buy/sell legs are NEW legs not in this list. The test `credit_identity_holds_across_trade_buy_and_sell` tests a standalone fixture, but the world-level test remains unextended.

**Fix:** Add a step in A3.6 (or A5 when scenario_bazaar fires) to extend `phase2_credit_identity_holds_every_tick` (or add a bazaar variant of it) that runs on a world with `exchange.active=true` and verifies the Σtreasury+Σcredits+Σescrow invariant every tick. The fixture `credit_identity_trade_fixture()` must be defined in the plan (not just referenced by name).

---

### MAJOR-4: ASSIGN empty-hold gate (L3-M3) has no test anywhere in the plan

**Location:** Mentioned in A3.5 (plan line ~3395): "ASSIGN gates package claims on an empty hold (L3-M3: keeps the prey taxonomy exact in rung B and the no-double-rob invariant true)" and as "a same-phase obligation". No test is written for it in any task.

**Evidence:**
- The synthesis cut (Part 1.2) explicitly calls L3-M3 as a named correction: "ASSIGN gates package claims on an empty hold... keeps the prey taxonomy exact in rung B".
- This is a behavioral gate (an Idle hauler with goods in hold must not ASSIGN to a new package contract) that changes existing ASSIGN logic. It is not covered by existing tests (the grounding extracts show `scripted_assign_filters_dry_tank_craft_play_c1` and `scripted_assign_filters_oversized_contracts` — neither tests the hold-nonempty gate).
- Without this gate, a capitalized own-trader could load goods, then ASSIGN a new contract before selling, holding both own-goods and contract cargo simultaneously — breaking the rung-B prey taxonomy.

**Fix:** Add a failing test in A3.5 or A5: `scripted_assign_skips_craft_with_nonempty_hold` — build a world with a scripted hauler with a non-empty hold, verify ASSIGN does not set `ships.contract[crow]`. Add the corresponding implementation step that adds the hold-nonempty gate to `run_scripted_dispatch`'s ASSIGN arm.

---

## Warnings (MINOR)

### MINOR-1: A3.7 EXCHANGE instrument uses wrong module path and inconsistent fixture style

**Location:** A3.7 Steps 1-3 (plan line ~5262).

**Evidence:**
- Step 1 uses `from jumpgate.sweep_trophic import parse_run_output` but the existing module path is `from python.analysis import sweep_trophic` (or `import sweep_trophic` with `PYTHONPATH` set). The `jumpgate` package namespace doesn't match the project's Python structure shown in `ground-lab.md §9` which says `python/analysis/sweep_trophic.py`.
- Step 3 adds to `python/jumpgate/sweep_trophic.py` but all other tasks consistently use `python/analysis/sweep_trophic.py`.
- The V6 fixture `FIXTURE_V6_WITH_EXCHANGE` is a one-off string with a different format than the versioned `V5_STDOUT = V4_STDOUT + (...)` pattern used in A0.4/A0.5 — will not be reusable as a baseline for future version checks.

**Fix:** Correct the module path to `python/analysis/sweep_trophic.py` and the pytest command to use `PYTHONPATH=/home/john/jumpgate/python pytest ...`. Rename the fixture to `V6_STDOUT = V5_STDOUT + ("EXCHANGE treasury_micros=... drain_per_100k=0\n")` to follow the additive-versioned pattern.

---

### MINOR-2: A4.3 REPOST early-return "proof" is stated as an expectation that the test already passes — not a TDD failing test

**Location:** A4.3 Step 1 (plan line ~5659): "Expected: PASS already (the Schmitt trigger `stock < 0` never fires with non-negative stock...)".

**Evidence:**
- Every other task in the plan specifies a failing test first. A4.3 Step 1 explicitly says the test passes before the prelude is added. The comment says "The test documents the invariant" — this is documentation-as-test, not TDD.
- The within-build digest proof ("proven by within-build digest on trophic+frontier seed 7") described in the commit message has no actual verification step in the task. The step only runs `cargo test --workspace`, not the full before/after digest comparison the commit message claims.

**Fix:** Add a concrete digest comparison step to A4.3: before applying the prelude, capture `sha256sum` of trophic+frontier stdout+JSONL; after the prelude, capture again and `diff`. The commit message currently makes a claim ("proven by within-build digest") that no step in the task actually executes. If this proof is genuine, the verification step must be present.

---

### MINOR-3: A3.2 `config_hash` exhaustive destructure adds `goods_cfg` but A1b ALSO adds it — potential double-add

**Location:** A1.2 Step 3 (plan line ~2175): "Add `goods` field to `RunConfig`" and add to config_hash exhaustive destructure with `let _ = goods;`. A3.2 Step 4 then "extends the exhaustive destructure to fold them."

**Evidence:**
- A1b already adds `pub goods: GoodsCfg` to `RunConfig` and binds it in the destructure with `let _ = goods;`.
- A3.2 adds `pub goods_cfg: GoodsCfg` (a DIFFERENT field name: `goods` in A1b vs `goods_cfg` in A3.2). This will compile as two separate fields unless one is meant to replace the other.
- A3.2 also adds `pub exchange: ExchangeCfg` and `pub arbitrage: ArbitrageCfg` — but A4.2 re-adds `ArbitrageCfg` (CRITICAL-2 above).

**Fix:** Harmonize field names: use `goods` (A1b) or `goods_cfg` (A3.2) consistently. The field added in A1b must be the same field folded in A3.2 — not two separate fields. If A1b adds `goods: GoodsCfg` as a stub with `let _ = goods;`, then A3.2 must fold `goods` (not `goods_cfg`). Verify all struct literal sites use the same field name.

---

## Confidence Assessment

**Overall Confidence:** High — all findings are grounded in direct plan text with cross-references to spec, synthesis cut, and ground extracts.

| Finding | Confidence | Basis |
|---|---|---|
| CRITICAL-1: assert_resource_identity triple-definition conflict | High | Plan lines 3090, 5132, A1b Step 10 — three inconsistent implementations verified |
| CRITICAL-2: two ArbitrageCfg definitions / two config re-pins | High | A3.2 plan line ~3960 and A4.2 plan line ~5509 — structurally incompatible field sets read directly |
| CRITICAL-3: Trade deletion in both A0.3 and A3.4 | High | A0.3 plan line ~601, A3.4 plan line ~4291 — both explicitly delete the variant |
| CRITICAL-4: WA3/WA5 joint-read not encoded | High | Spec §6b and synthesis cut Part 3 explicitly name the joint-read; no corresponding instrument task found in plan |
| MAJOR-1: update_prices test uses non-existent methods | High | `empty_with_goods`, `push_goods` not in any ground extract |
| MAJOR-2: TradeBuyIntent vs tuple type conflict | High | A2.3 plan line ~3243 (named struct) vs A3.5/A3.6 plan lines ~4606/4935 (tuple destructuring) |
| MAJOR-3: credit identity test uses undefined fixture | Moderate | `credit_identity_trade_fixture()` called but not defined; world-level test not extended |
| MAJOR-4: ASSIGN empty-hold gate has no test | High | Synthesis cut §1.2 names it; no test found in any task |
| MINOR-1: A3.7 module path wrong | High | All other tasks use `python/analysis/`; A3.7 uses `python/jumpgate/` |
| MINOR-2: A4.3 digest proof not executed | High | Commit message claims proof; no verification step exists |
| MINOR-3: RunConfig field name inconsistency | Moderate | `goods` vs `goods_cfg` — depends on which phase lands first |

---

## Risk Assessment

**Implementation Risk:** High  
**Reversibility:** Moderate (A1 is hash-neutral so rollable; A2 v6 bump creates a breaking change that requires re-deriving goldens)

| Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|
| Plan compiles only after manual reconciliation of 3 CRITICALs | High | Certain if plan is executed as-written | Fix CRITICAL-1, CRITICAL-2, CRITICAL-3 before starting A2/A3 |
| WA5 reads are silently confounded by missing WA3 joint read | High | Certain (no instrument added) | Fix CRITICAL-4 in A6 lab tasks |
| ASSIGN allows own-traders to carry double cargo into rung B | Medium | Likely without L3-M3 gate test | Fix MAJOR-4 in A3.5 |
| Credit identity not tested end-to-end with Exchange active | Medium | Possible (fixture undefined) | Fix MAJOR-3 in A3.6 |
| Builder spends time debugging A3.1 test compile errors | Low | Likely (wrong method names) | Fix MAJOR-1 |

---

## Information Gaps

1. [ ] **Phase A6 lab tasks not read** — `draft-a6.md` was not read (only referenced via the assembled plan through A5). If A6 encodes the WA3/WA5 joint read, CRITICAL-4 may be partially addressed. Recommend reading A6 to confirm.
2. [ ] **`scenario_bazaar` full factory function** — the Food sink placement logic (rows 3, 4, 8) is asserted by the plan but not independently verified against the actual frontier-band topology. Plan-time checks on whether those rows correspond to sink-topology stations are not shown.
3. [ ] **`credit_identity_trade_fixture` definition** — the fixture function referenced by MAJOR-3 is not defined anywhere in the read plan text. It may exist in a draft or be expected to be written by the builder.
4. [ ] **Phase A5 full content** — phases A5 and A6 were read only in summary (assembled plan cuts off at A4.4). If A5 contains the ASSIGN empty-hold gate test or WA3 joint-read instrument, MAJOR-4 and CRITICAL-4 may be addressed.

---

## Caveats & Required Follow-ups

### Before Executing This Plan

- [ ] Resolve CRITICAL-2: merge `ArbitrageCfg` definitions into one complete struct in A3.2 before building
- [ ] Resolve CRITICAL-3: strip the `EventKind::Trade` deletion from A3.4 (it already lands in A0.3)
- [ ] Resolve CRITICAL-1: choose the canonical `assert_resource_identity` signature (Vec-based per A1b) and remove the duplicate extensions in A2.2 and A3.6 Step 4
- [ ] Add WA3/WA5 joint-read instrument and co-report obligation (CRITICAL-4) to A6 before the rung-A exit console session
- [ ] Add ASSIGN empty-hold gate test (MAJOR-4) to A3.5

### Assumptions Made

- `GoodSpec.name` type is `&'static str` in A1b but `String` in A3.2 — the plan is inconsistent; this analysis assumes `String` (heap-allocated) is correct for the final form since goods names come from config, not string literals
- The `A6` phase is the designated lab/science phase; its tasks are assumed to contain the WA panel scripts even if not read here

### Limitations

- This analysis does NOT verify symbol existence (Reality reviewer's scope)
- This analysis does NOT cover architectural patterns (Architecture reviewer's scope)
- Run-time behavior of the arbitrage poster's `committed[]` scratch correctness is inferred from spec text, not verified against existing economy engine flow
