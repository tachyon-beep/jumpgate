# Architecture Review — Goods as Goods Rung A (the bazaar)

> Reviewer: architecture + complexity lens  
> Spec: `docs/superpowers/specs/2026-06-12-goods-as-goods-design.md` (D1-D8, OD-1..7)  
> Panel cut: `docs/superpowers/posts/2026-06-12-goods-as-goods-panel/synthesis-recommended-cut.md` (17 CRITICALs)  
> Plan: `/tmp/gag-plan-a/assembled-plan.md` (10 888 lines, 7 landing phases A0-A6)  
> Codebase head: `b446095` (`jumpgate-v1-design`)

---

## Architecture Review

### Blast Radius

**Files touched (plan-declared):** ~25 Rust source files + ~8 Python files = **33 files**

| File | Type | Weight | Reason for change |
|------|------|--------|-------------------|
| `crates/jumpgate-core/src/economy.rs` | Core business logic | 2× | New verbs: `run_trade_policies`, `resolve_trade_buys/sells`, `run_scripted_dispatch` (poster), `update_prices` generalization |
| `crates/jumpgate-core/src/hash.rs` | Core — state-hash fold | 2× | v6 bump, `manual_zero_fold`, pending-column asserts |
| `crates/jumpgate-core/src/stores.rs` | Core — CraftStore layout | 2× | New `hold`, `pending_trade_buy/sell` columns |
| `crates/jumpgate-core/src/world.rs` | Core — tick orchestration | 2× | New stages 1c3x, 1dx, `assert_resource_identity`, `ResetError::BadGoodsCfg` |
| `crates/jumpgate-core/src/contract.rs` | Core — event types | 2× | Remove `Trade`; add `TradeBought`, `TradeSold`, `OfferWithdrawn`, `RefuelDenied` |
| `crates/jumpgate-core/src/config.rs` | Config schema | 1× | `GoodsCfg`, `ArbitrageCfg`, `ExchangeCfg`, `CorporationInit.arb_premium_micros`, `GOLDEN_CONFIG_HASH` re-pin |
| `crates/jumpgate-core/src/scenario.rs` | Config factory | 1× | `scenario_bazaar`, v6 golden re-pin `FRONTIER_TRAJECTORY_GOLDEN` |
| `crates/jumpgate-core/src/diagnostics.rs` | Instruments | 1× | per-station stock/price matrices |
| `crates/jumpgate-core/src/ingest.rs` | Core — command ingestion | 2× | `TradeBuy`/`TradeSell` `CommandKind` arms |
| `crates/jumpgate-core/src/pirate.rs` | Core — pirate brain | 2× | engagement eligibility extension (rung A unattackable own-traders) |
| `crates/jumpgate-core/examples/trophic_run.rs` | Infra/runner | 0.5× | exhaustive chronicle, gossip-log arms, BAZAAR line, transport-table JSONL |
| `crates/jumpgate-py/src/env.rs` | Gym binding | 1× | array → Vec conversions |
| `python/analysis/sweep_trophic.py` | Analysis | 0.5× | BAZAAR_RE, META_RE extension |
| `python/tests/test_sweep_parsing.py` | Test | 0.5× | V5 fixture |
| `crates/jumpgate-core/src/lib.rs` | Config | 0.5× | `RefuelDeniedReason` re-export |
| Multiple test modules | Test | 0.5× | New per-task tests (≈30 across phases) |

**Weighted score (heuristic):** core-business files (8 × 2 = 16) + config (1×) + factory (1×) + infra (0.5×) + gym (1×) + python (1×) + tests (0.5× × ~10 = 5) ≈ **26 weighted units**

**Risk level: Very High** (weighted > 15, files > 13)

**Recommendation:** The plan already phases this well (A0 → A6). The phase boundaries ARE the mitigation. Do NOT attempt to collapse phases. Each phase boundary is a merge-able, digest-verified unit. The blast radius is appropriate given the plan is an ordered, testable pipeline.

---

### One-Way Doors

| Change | Risk | Mitigation in Plan? |
|--------|------|---------------------|
| `HASH_FORMAT_VERSION` 5 → 6 (A2.1) | High — all existing trophic/frontier goldens move | YES — single-cause commit; `print_golden` + `print_golden_frontier` fixtures mandated; `manual_zero_fold` update required same-commit |
| `GOLDEN_CONFIG_HASH` re-pin (A3.2) | Medium — config tests break until re-pinned | YES — single-cause commit; `print_golden_config` fixture mandated; not invented |
| `FRONTIER_TRAJECTORY_GOLDEN` re-pin (A2.1) | Medium | YES — same commit as v6 bump; `print_golden_frontier` mandated |
| Removal of `EventKind::Trade` (A0.3) | Medium — API change | YES — handled twice (see CRITICAL-1 below) |
| `[i64; N_RESOURCES]` → `Vec<i64>` everywhere (A1b) | Medium — signature-breaking refactor | YES — cross-branch state-hash sequence equality mandated before commit |
| `scenario_bazaar` as a new runnable scenario | Low | YES — `trophic`/`frontier` stay bit-identical; bazaar isolated |
| `demand_low = demand_high = 0` in `scenario_bazaar` (REPOST structural off) | Low — retirement, not deletion | YES — verified at economy.rs:488-516; structural off documented; trophic/frontier untouched |
| `GoodsCfg` added to `RunConfig` (A1b + A3.2) | Low — additive | Partial — see CRITICAL-2 below |

**No unmitigated one-way doors identified for the core state-hash changes.** The golden discipline is rigorous. However two structural issues arise in the plan's own task sequencing — see Blocking Issues.

---

### Complexity Assessment

**Tracer bullet opportunity:** No — the plan already has an explicit integration spine: A0 instruments → A1 hash-neutral refactor → A2 format bump → A3 boards+verbs → A4 arbitrage → A5 scenario → A6 exit. Each phase exit is a runnable, digest-checkable world. This IS the tracer bullet pattern correctly applied.

**Custom code vs libraries:**

| Custom Code | Available Library | Recommendation |
|-------------|-------------------|----------------|
| `mix(seed, k)` for per-good topology in `scenario_bazaar` | None (project-internal; Rust inline) | Correct — reuses existing well-tested PRNG idiom (scenario.rs:108-113); no library needed |
| `sha256sum` for digest baseline (bash) | System `sha256sum` | Correct |
| Per-good spread computation in `run_trade_policies` (inline O(n_stations × n_goods) scan) | None | Adequate for v1; recognized in plan as "v1 simplification" |

No reinvented wheels detected.

**"Why Now?" flags:**

| Step | Concern | Assessment |
|------|---------|------------|
| `GoodSpec.unit_mass_milli` folded in A3.2 (first reader not yet present) | OD-7 "minimal-live": capacity gate reads it on every transfer | JUSTIFIED — capacity gate lands in A3.6 (`resolve_trade_buys` milli-mass check); folding before the reader exists is correct because the config hash must be stable once set |
| `Good::FOOD = Good(2)` const in A2.4 but `scenario_bazaar` producers land later in A5 | FOOD const created before its consuming factory | JUSTIFIED — needed to write the failing test in A2.4 that guards the factory |
| `pending_trade_buy/sell` columns added in A2.3 before `run_trade_policies` (A3.5) or `resolve_trade_buys` (A3.6) write/consume them | Columns exist but are all-None for one phase boundary | JUSTIFIED — debug_asserts validate they stay None; the pending-column idiom requires the column to exist before the policy stage can reference it; this is the `pending_upgrade` precedent |

No unjustified premature abstractions detected.

---

### Pattern Alignment

| Pattern | Plan Approach | Project Standard | Aligned? |
|---------|---------------|------------------|----------|
| Intent/settle split for new verbs | `run_trade_policies` (1c3x) writes `pending_trade_buy/sell`; `resolve_trade_buys/sells` (1dx) consumes | Established at `run_refuel_policies` / `resolve_refuels`, `run_purchase_policies` / `resolve_purchases` | YES — explicitly cited as precedent throughout |
| Transfer vs sink for goods | `TRANSFER (stock ↔ hold, no consumed[])` | Established by `try_load` (economy.rs:826-827) | YES — panel consensus #5, verified in `ground-economy-verbs.md §5` |
| Single-cause golden commits | Each format bump and config re-pin is one commit with one named cause | Established at v2-v5 bumps per `hash.rs` provenance comments | YES — rigorously enforced throughout |
| Exhaustive match on EventKind | Remove `_ => None` wildcards in chronicle and gossip-log | Established AFTER this plan (the plan itself creates the policy) | YES — explicitly stated as deliberate policy reversal in synthesis cut Part 3 |
| Corp dense-row index idiom | `ExchangeCfg { corp_index: u32 }` with `id_at(ex_row).is_none()` stale-skip | Established at `ShipyardCfg.corp_index`, `RefuelCfg.corp_index` | YES — same idiom cloned per `ground-economy-verbs.md §10` |
| Config tail append-only | GoodsCfg, ArbitrageCfg, ExchangeCfg folded after RefuelCfg | Established by RefuelCfg append at config.rs:749-751 | YES |
| Structural inert gate | `ExchangeCfg.active = false`, `ArbitrageCfg.scan_interval = 0` | Established by `RefuelCfg.lot_mass = 0.0` and `dispatch_cfg.demand_low = demand_high = 0` | YES |

**Pattern alignment is high.** The plan is architecturally coherent with the existing codebase idioms.

---

## Blocking Issues

### CRITICAL-1: `EventKind::Trade` is scheduled for deletion TWICE

**Evidence:**
- `assembled-plan.md:509-950` — Task A0.3 title: "remove `EventKind::Trade` corpse". Step 2 explicitly verifies `economy_event_kinds_are_copy_and_partial_eq` passes with `Trade` still present. Step 3 adds `RefuelDenied` to `EventKind`. Step 9 commit message reads: "Removes EventKind::Trade (dead — sole constructor was a test helper at contract.rs:417)".
- `assembled-plan.md:4232-4380` — Task A3.4 title: "TradeBought/TradeSold events + **delete dead EventKind::Trade**". Step 1 writes a failing test that references `TradeBought` and replaces the old `Trade`-referencing test. Step 2 instructions say "DELETE: Trade { station, resource, qty, price_micros }".

**Severity: CRITICAL** — If the builder executes A0.3 as written, `EventKind::Trade` is gone from the enum at the A0 tip. Phase A3.4 then instructs deletion of a variant that no longer exists, and also updates `economy_event_kinds_are_copy_and_partial_eq` with instructions that reference the old `Trade`-referencing test body — but that test body was already updated in A0.3 to use `ContractOffered` as the Copy+PartialEq witness (assembled-plan.md:665-729).

**Concrete failure mode:** A3.4 Step 2 instructions ("DELETE these lines: `Trade { station, resource, ... }`") will produce a compile error because the variant is already absent. The exhaustive-match mandate in A0.2/A0.3 means the None-arm group that included `Trade` was also already cleaned up. A builder following both tasks literally will hit a confusing "variant not found" error in the middle of A3.

**Required fix:** Remove the `EventKind::Trade` deletion from A3.4. The A3.4 task's actual job is only: add `TradeBought`/`TradeSold` to `EventKind`, update `economy_event_kinds_are_copy_and_partial_eq` to prove `TradeBought` has Copy+PartialEq (the test body already changed in A0.3, so the delta here is adding the `TradeBought` assertion, not replacing a `Trade` assertion). Update the A3.4 task title and preamble to remove the deletion framing.

**Alternatively** (if the plan intent was to keep `Trade` alive through A0): Move the `Trade` deletion out of A0.3 entirely. A0.3's three stated reasons are RefuelDenied, exhaustive chronicles, and Trade deletion. The first two are genuinely A0-grade (hash-neutral instrument changes); the Trade deletion is a breaking enum change best co-located with `TradeBought/TradeSold` addition in A3.4 since they are semantically related. This avoids a multi-phase dependency on a deleted variant.

---

### CRITICAL-2: `GoodsCfg` struct is defined THREE TIMES in the plan with incompatible field types

**Evidence:**
- `assembled-plan.md:2145-2172` (Task A1.2, Step 2) — defines `GoodSpec { name: &'static str, unit_mass_milli: u32 }` and `GoodsCfg { goods: Vec<GoodSpec> }` in `config.rs`. `name` is `&'static str`.
- `assembled-plan.md:3923-3938` (Task A3.2, Step 2) — re-defines `GoodSpec { name: String, unit_mass_milli: u32 }` and `GoodsCfg { goods: Vec<GoodSpec> }`. `name` is `String`.
- `assembled-plan.md:7480-7503` (Task A5.2, scenario_bazaar) — re-defines `GoodsCfg::default()` with `name: "Ore".into()` (implies `String`).

**Severity: CRITICAL** — `&'static str` vs `String` is not a cosmetic difference. A1b's definition using `&'static str` is hash-neutral correct (it cannot be `.into()`'d from a non-literal), but A3.2's definition using `String` breaks A1b's code that already compiled. The builder implementing A1b first will write code with `&'static str`, then A3.2 will rewrite the same struct with `String`, which is a silent semantic change in `config_hash`'s destructure and fold. The `GoodSpec` default in A1.2 uses string literals (`name: "Ore"`, `name: "Fuel"`) consistent with `&'static str`. The default in A3.2/A5.2 uses `.into()` (consistent with `String`). A `Clone` bound requirement differs between the two (`&'static str` is Copy; `String` is not).

**Additional complication:** A1b's `GoodsCfg::default()` impl uses `GoodSpec { name: "Ore", ... }` (a string literal, valid for `&'static str` but NOT for `String`). A3.2's rewrite uses `GoodSpec { name: "Ore".into(), ... }`. If A1b is implemented first (which it must be — A3 depends on A1), a builder must then change `GoodSpec.name` field type from `&'static str` to `String` in A3.2 AND update all A1b-era construction sites (the `GoodsCfg::default()` impl, the `GoodSpec` literals in `scenario_bazaar`). This is undocumented churn.

**Required fix:** Reconcile to a single definition. The panel spec (synthesis §3) says "name (never folded)" and the spirit is human-readable display. `String` is the appropriate choice for a property that will eventually accept config-file names. The A1b task should define `GoodSpec { name: String, ... }` from the start and use `.to_string()` or `.into()` in all literal construction sites. Remove the redundant struct definitions from A3.2 and A5.2 (they should reference the already-landed A1b type, not re-define it).

---

### CRITICAL-3: `pending_trade_buy/sell` columns are added TWICE with incompatible payload types

**Evidence:**
- `assembled-plan.md:3167-3340` (Task A2.3) — defines `TradeBuyIntent { station_row: usize, good: Good, qty: u32 }` and `TradeSellIntent { station_row: usize }` structs, then adds `pending_trade_buy: Vec<Option<TradeBuyIntent>>` and `pending_trade_sell: Vec<Option<TradeSellIntent>>` to `CraftStore`.
- `assembled-plan.md:4121-4228` (Task A3.3) — re-adds the same two columns with DIFFERENT payload types: `pending_trade_buy: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>` (a bare tuple using `StationId` not `station_row: usize`) and `pending_trade_sell: Vec<Option<crate::ids::StationId>>`.

**Severity: CRITICAL** — This is not an update; it is a redefinition. A builder implementing A2.3 first will create the `TradeBuyIntent` / `TradeSellIntent` structs and add the columns with struct payloads. A3.3 then instructs adding the same columns again with tuple payloads, which is a compile error (duplicate field in struct). The two tasks also have different hash.rs `debug_assert!` content — A2.3 uses the `TradeBuyIntent` struct constructor, A3.3 uses a bare tuple. The test in A3.3 Step 1 (`should_panic`) will compile-fail against A2.3's already-landed column.

**Root cause:** A2.3's placement in Phase A2 ("format v6 scaffolding") and A3.3's placement in Phase A3 ("boards, exchange, verbs") represent two drafters separately adding the same scaffolding. The synthesis cut §1.2 says these columns follow `pending_upgrade/pending_refuel` discipline — they should appear exactly once.

**Required fix:** Remove one of the two tasks entirely. The column payload type question (struct vs tuple, `station_row: usize` vs `StationId`) must be resolved to one definition. The `StationId`-based tuple in A3.3 is more consistent with existing pending-column payloads (which reference entity IDs, not raw rows). Remove A2.3 completely; fold any useful `debug_assert` infrastructure from A2.3's Step 3 into A3.3's Step 3 (they are identical in intent). The A2 summary (assembled-plan.md:3645) lists A2.3 as a required commit — update that summary.

---

## Major Issues

### MAJOR-1: A3.3 (pending columns) is placed in Phase A3 but its test probes A1b's scaffolding

**Evidence:** `assembled-plan.md:4135-4159` — A3.3 Step 1 writes a failing test that expects `ships.pending_trade_buy` to not exist. But A2.3 (a prior phase) ALSO adds that field. If A2.3 lands first (per phase order), the A3.3 Step 1 test passes immediately rather than failing as the test expects — the test's pedagogical purpose (TDD guard) is vacuous.

**Severity: MAJOR** — The TDD discipline embedded in the plan is undermined. The failing-test → green-test cycle is load-bearing for the plan's quality guarantee. This is a symptom of CRITICAL-3 above but has its own impact: a builder may conclude "test passes, all good" without noticing the duplicate column was already added with wrong types.

**Fix:** Resolves when CRITICAL-3 is fixed (keep only one column-addition task).

---

### MAJOR-2: A3.4's `chronicle_subject` exhaustive-match instructions conflict with A0.3's already-landed exhaustive match

**Evidence:**
- `assembled-plan.md:509-950` — A0.3 Step 6 replaces the entire `chronicle_subject` function with an exhaustive match covering ALL then-existing EventKind variants including a None arm for Production/PriceUpdate/etc.
- `assembled-plan.md:4315-4328` — A3.4 Step 3 instructs: "REMOVE the `_ => None` arm and the doc comment saying variants default to skipped. ADD arms for all new variants." This assumes the wildcard still exists post-A0.3.

**Severity: MAJOR** — A builder implementing A0.3 as written will have already removed the `_ => None` wildcard and created an exhaustive match. A3.4's instructions to "remove the wildcard" are then no-ops, and the instruction to "ADD arms for all new variants" is the correct action — but the framing implies the wildcard is still present, which will confuse the builder. If the builder searches for the wildcard and doesn't find it, they may think A0.3 didn't apply correctly.

**Fix:** A3.4 Step 3 should be rewritten as: "Add `TradeBought/TradeSold` arms to the ALREADY exhaustive `chronicle_subject` match (landed in A0.3). The wildcard is already gone. Failure to add these arms will be a compile error." Similarly for Step 4 (gossip-log exhaustive match).

---

### MAJOR-3: The withdrawal sweep (L1-C1) placement is at stage 1b2 but the spec's "Accepted-but-never-loaded" arm requires CraftStore write access not present in current `run_scripted_dispatch` signature

**Evidence:**
- `assembled-plan.md:6110-6568` (Task A4.5) — the withdrawal sweep runs in the same `run_scripted_dispatch` function (stage 1b2). The Accepted-never-loaded arm requires: "fail + escrow refund + release hauler" which means writing `ships.contract[crow] = None` and `ships.role[crow] = CraftRole::Idle`.
- `ground-economy-verbs.md §8` (REPOST mechanics) — `run_scripted_dispatch` currently does NOT take `ships: &mut CraftStore` as a parameter; it only takes `contracts: &mut ContractStore`, `corporations: &mut CorporationStore`, and the dispatch config. The synthesis cut Part 1.2 notes: "the withdrawal sweep belongs in the same `run_scripted_dispatch` function" and the A4.5 task adds `ships: &mut CraftStore` to the signature.
- `assembled-plan.md:6121` — confirms "update signature to include `ships: &mut CraftStore` for the Offered craft-intent clearing."

**Severity: MAJOR** — The signature change is noted in A4.5 but the implications are not fully threaded: (1) `World::step` at stage 1b2 calls `run_scripted_dispatch` and must be updated in the same commit; (2) any test that constructs a bare `run_scripted_dispatch` call will break on the new parameter. The plan does not check whether A4.5's signature change is co-landed with the world.rs call site update, nor does it explicitly verify that no existing test calls `run_scripted_dispatch` directly (the test module at `world.rs:2591` — `scripted_dispatch_makes_stage1_loop_self_run` — calls the full `World::step`, so it may be OK, but the isolation tests at `economy.rs:2415` and `economy.rs:2334` call the function directly and WILL break without a parameter update).

**Fix:** A4.5 must explicitly list `world.rs` and any direct `run_scripted_dispatch` test callers in its Files section, and require the signature update to be co-landed in the same commit. A grep for direct callers should be a required step.

---

## Minor Issues

### MINOR-1: `GoodSpec.name` field type inconsistency creates `Clone` bound ambiguity

Already covered as part of CRITICAL-2 but worth calling out independently: `&'static str` does not require a `Clone` bound (it is `Copy`), while `String` does. The `GoodsCfg` struct has `#[derive(Clone, Debug)]` in A1.2 but not `Copy`. With `&'static str` it could derive `Copy`; with `String` it cannot. A3.2's re-definition with `String` also adds `#[derive(Clone, Debug, Default)]` (the A1.2 definition has an explicit `impl Default`). These diverge.

### MINOR-2: A5.1 (two-mode policy at ASSIGN write site) duplicates ASSIGN gate logic already present in A3.5 (`run_trade_policies`)

**Evidence:** `assembled-plan.md:6722+` — A5.1 extends the ASSIGN write site in `run_scripted_dispatch` to compare `best_wage_net` vs `best_trade_net`. But A3.5 already implements `run_trade_policies` which makes the same two-mode decision (sell path / buy path) for docked craft. The ASSIGN slot handles moving craft; `run_trade_policies` handles docked craft. These are genuinely separate decision points (D6: "per-trip channel decision"). However, the transport-table subtraction formula `best_wage_net = ASSIGN score − transport[route]` in A5.1 must use the SAME transport table as `run_trade_policies`' spread computation — the plan does not explicitly verify this is the same config source.

**Fix:** Add a cross-check note in A5.1 verifying that the transport table used in the ASSIGN comparison reads from `ArbitrageCfg`'s transport table (the factory-time integer table, not an ephemeris read), same as A3.5. The synthesis cut §1.2 says "the PDR-0007 seam as one shared function" — confirm A5.1 routes through the same function or at minimum the same config field.

### MINOR-3: A2.4 adds Food (`Good(2)`) to `economy.rs` but does not verify it is absent from `scenario_trophic`/`scenario_frontier` GoodsCfg defaults

**Evidence:** A1.2 defines `GoodsCfg::default()` as a 2-element table (ORE, FUEL). `Good::FOOD = Good(2)` is added in A2.4. But `GoodsCfg::default()` still returns `vec![GoodSpec{Ore}, GoodSpec{Fuel}]`. A test that constructs `scenario_trophic(7)` and queries `cfg.goods.goods.len()` will get 2, not 3. Food consumers in `scenario_bazaar` read `Good::FOOD = Good(2)` but the bazaar GoodsCfg has n_goods from its own factory (must be ≥ 3). The plan does not include a test that asserts `scenario_bazaar`'s GoodsCfg has `n_goods >= 3` when Food consumers are present — A2.4's failing test checks that Food consumption producers exist but does NOT check that the config's `goods` Vec is long enough to include `Good(2)`.

**Fix:** A2.4 Step 3 (Food consumption producers) should co-land with the `scenario_bazaar` GoodsCfg definition that includes a Food entry. Alternatively add an explicit assertion: `assert!(cfg.goods.goods.len() > Good::FOOD.0 as usize)` in the test.

### MINOR-4: Arbitrage poster (A4.4) uses `(route_index + scan_index) % n_corps` rotation but A4.2 notes the transport table is "factory-time integer" — the plan does not specify where this table lives in config

**Evidence:** `assembled-plan.md:5464-5600` (Task A4.2) says "the per-good transport table" is needed by A4 but defers the exact config slot to that task. The synthesis cut (Part 1.2) says "transport[route] is a FACTORY-TIME integer table from phase-independent ring-radius geometry (the tier-reward precedent, scenario.rs:466-467), folded as config." The plan says the BAZAAR JSONL line echoes it as a no-tick tail row (A0.4). But Task A4.2 only says "confirmation + transport table" without specifying: is this a new `ArbitrageCfg` sub-field, a new `RunConfig` top-level field, or a per-station pair? The `ArbitrageCfg` struct as defined in A3.2 does not include a transport table field.

**Fix:** A4.2 must explicitly name the new config field for the transport table and verify it is included in `config_hash` (the A3.2 commit only folds `ArbitrageCfg { scan_interval, wage_flat_micros, wage_share_milli, max_posts_per_scan }` — no route-cost table). If the table is added in A4.2, A4.2 becomes a second config-hash movement, requiring a SECOND `GOLDEN_CONFIG_HASH` re-pin. The plan claims "ONE GOLDEN_CONFIG_HASH re-pin per rung" (A3.2). Resolution: either include the transport table in A3.2's config commit, or explicitly document that A4.2 introduces a second re-pin (breaking the one-re-pin discipline).

---

## Summary

- **Architectural concerns:** 0 (the architecture is sound; the intent/settle split, transfer vs sink, and golden discipline are well-applied)
- **One-way doors without mitigation:** 0 (all one-way doors have mandated print-fixture re-derivation)
- **Blocking issues (CRITICAL):** 3
- **Major issues:** 3
- **Minor issues:** 4

## Confidence Assessment

**Overall Confidence:** High

| Finding | Confidence | Basis |
|---------|------------|-------|
| CRITICAL-1: Trade deletion twice | High | Direct read of assembled-plan.md lines 509-950 (A0.3) and 4232-4380 (A3.4); both explicitly delete the same variant with different companion changes |
| CRITICAL-2: GoodsCfg/GoodSpec triple definition | High | Direct grep of assembled-plan.md: lines 2145, 3923, 7480 all define the struct; field types differ (`&'static str` vs `String`) |
| CRITICAL-3: pending_trade columns twice | High | Direct read of assembled-plan.md lines 3167-3340 (A2.3) and 4121-4228 (A3.3); column names identical; payload types different |
| MAJOR-1: A3.3 TDD guard vacuous | High | Follows logically from CRITICAL-3; the test guard assumes column does not yet exist |
| MAJOR-2: A3.4 chronicle instructions conflict with A0.3 | High | Both tasks claim to be the site of wildcard removal; only one can be first |
| MAJOR-3: run_scripted_dispatch signature impact | Moderate | Confirmed signature change required; specific test caller list not verified by direct file read (grounding extract covers `scripted_dispatch_makes_stage1_loop_self_run` but not all direct callers) |
| MINOR-2: Two-mode policy duplication concern | Moderate | Both A3.5 and A5.1 make channel decisions; the synthesis cut says they are different decision points (docked vs in-flight); may be intentional |
| MINOR-4: Transport table config placement ambiguous | Moderate | A4.2 task text is incomplete; the ArbitrageCfg struct as written in A3.2 lacks a route-cost field |

## Risk Assessment

**Implementation Risk:** High  
**Reversibility:** Moderate (hash bumps are one-way doors within a branch; the behavior-digest baseline enables cross-phase bisection)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| CRITICAL-1: Trade deletion causes compile failure mid-A3 | High | Certain (if both tasks executed as written) | Fix A3.4 to not re-delete; or move deletion to A3.4 only |
| CRITICAL-2: GoodsCfg type mismatch breaks A3.2 when building on A1b | High | Certain | Reconcile to `String` in A1.2; remove re-definitions from A3.2 and A5.2 |
| CRITICAL-3: pending column redefinition breaks A3.3 when building on A2.3 | High | Certain | Remove A2.3; consolidate into A3.3 |
| MAJOR-3: signature change breaks economy.rs direct callers | Medium | Likely | Add explicit caller inventory to A4.5 Files section |
| MINOR-4: second GOLDEN_CONFIG_HASH re-pin undocumented | Medium | Possible | Include transport table in A3.2's config commit, or document second re-pin explicitly |

## Information Gaps

1. [ ] **Direct `run_scripted_dispatch` callers in test modules** — a grep over `economy.rs` and `world.rs` test mods for `run_scripted_dispatch(` calls would confirm whether MAJOR-3 affects isolated unit tests or only integration tests. This reviewer did not read all test code.
2. [ ] **A4.2 full task content** — the assembled plan excerpt cuts off before the transport-table config slot is specified. The complete A4.2 content would resolve MINOR-4.
3. [ ] **Whether A2.3 was intentionally added as a "scaffold first" pattern** — if the plan author intended A2.3 as the canonical column definition and A3.3 as its user (not a redefinition), the payload type conflict still needs resolution but the architectural intent changes.

## Caveats & Required Follow-ups

### Before Relying on This Analysis
- [ ] Verify CRITICAL-1 by checking whether A0.3's commit instructions (lines 509-950) are meant to be skipped by a builder who also executes A3.4, or whether one of the two tasks supersedes the other.
- [ ] Confirm GoodSpec field type (`&'static str` vs `String`) was a deliberate choice in A1.2 or an oversight.
- [ ] Confirm A2.3 vs A3.3 relationship: if A2.3 is a draft placeholder that was superseded by A3.3, A2.3 should be marked as skip/deleted in the assembled plan.

### Assumptions Made
- The plan is executed in phase order (A0 → A6) by a single agent; concurrent multi-agent execution is not assumed.
- All task Steps within a phase are executed in order.
- The "single-cause commit" mandate means each task maps to exactly one commit (or a small set where the task explicitly names multiple commits, as in A1a/A1b).

### Limitations
- This review does NOT cover symbol existence verification (whether `scenario_trophic()`, `cfg_with_craft_x()`, etc. are callable with the signatures the plan assumes at each phase).
- This review does NOT cover test coverage quality (whether the 30+ new tests are sufficient).
- This review does NOT assess security or systemic effects (propagation through the gossip/media layers).
