# Reality Check — world-gets-big assembled-plan.md

**Reviewer:** Plan Review Reality Agent  
**Date:** 2026-06-10  
**Plan file:** `/tmp/wgb-plan/assembled-plan.md`  
**Spec (authoritative):** `/home/john/jumpgate/docs/superpowers/specs/2026-06-11-world-gets-big-design.md`  
**Codebase HEAD:** e7e490e  

---

## Reality Check

### Symbols

| Symbol | Status | Evidence |
|--------|--------|----------|
| `FUEL_EMPTY_EPS` (events.rs) | EXISTS | `events.rs:16` — `1e-9` |
| `fuel_just_emptied` | EXISTS | `events.rs:49-51` |
| `run_pirate_brains` | EXISTS | `pirate.rs:511` |
| `relocate_lurk_target` | EXISTS | `pirate.rs:452` |
| `nav_lurk` adoption site | EXISTS | `pirate.rs:578-583` |
| `run_purchase_policies` | EXISTS | `economy.rs:1012` |
| `resolve_purchases` | EXISTS | `economy.rs:853` |
| `resolve_failures` | EXISTS | `economy.rs:1194` |
| `settle_contract_failure` | EXISTS | `economy.rs:1262` |
| `FailureCause` | EXISTS | `economy.rs:1237` (pub enum) |
| `docked_at_vendor` | EXISTS | `economy.rs:931` |
| `vendor_world_fixture` | EXISTS | `economy.rs:1564` |
| `capacity_world_fixture` | EXISTS | `economy.rs:1749` |
| `two_body_contract_fixture` | EXISTS | `world.rs:2048` |
| `two_body_starved_contract_fixture` | EXISTS | `world.rs:2196`, fuel `1.06e-9` at `:2203` |
| `starved_two_body_contract_fixture` (economy.rs) | EXISTS | `economy.rs:2364`, fuel `1.06e-9` at `:2417` |
| `deliver_on_arrival_settles_escrow_and_holds_credit_identity` | EXISTS | `world.rs:2471` |
| `fuel_just_emptied_fires_only_on_depletion_edge` | EXISTS | `events.rs:179` |
| `replay_equivalence.rs` base fuel `5.0e-10` | EXISTS | `tests/replay_equivalence.rs:41` |
| `ingest_commands` | EXISTS | `ingest.rs:151` |
| `one_body_one_craft_cfg` | EXISTS | `ingest.rs:364` (ingest.rs test mod only) |
| **`one_body_one_craft_station_cfg`** (Task 1.2.1 Step 8, world.rs test) | **NOT FOUND** | Plan labels it a stand-in; actual fixture used by neighbour test `half_on_media_config_is_rejected` at `world.rs:1701` is `one_body_two_stations_one_miner()` at `world.rs:1648` |
| `buy_upgrade_writes_pending_intent_logs_and_emits_action_ingested` | EXISTS | `ingest.rs:440` |
| `pending_upgrade` (CraftStore field) | EXISTS | `stores.rs:203` |
| `pending_upgrade` empty() | EXISTS | `stores.rs:224` |
| `pending_upgrade` push() | EXISTS | `stores.rs:255` |
| `pending_upgrade` world.rs struct literal | EXISTS | `world.rs:252` |
| `pending_upgrade` world.rs per-craft loop | EXISTS | `world.rs:294` |
| all-None hash assert | EXISTS | `hash.rs:306-309` |
| `GOLDEN_CONFIG_HASH` | EXISTS | `config.rs:745` = `0xee02_df67_1889_78dc` |
| `config_hash_golden_anchor_is_stable` | EXISTS | `config.rs:796` |
| `print_golden_config` (ignored test) | EXISTS | `config.rs:1028` |
| `print_golden` (ignored test) | EXISTS | `hash.rs:1113` |
| `state_hash_golden_zero_world` | EXISTS | `hash.rs:1101` |
| `golden_zero_state_hash` | EXISTS | `hash.rs:1225` |
| `GOLDEN_ZERO_STATE_HASH` | EXISTS | `hash.rs:129` |
| `TrophicSample` (struct, last field `assign_counts_cum`) | EXISTS | `diagnostics.rs:66-139` |
| `sample_window` | EXISTS | `diagnostics.rs:437` |
| `WINDOW_TICKS` | EXISTS | `diagnostics.rs:24` — **NOT re-exported from lib.rs** (see Paths) |
| `OUTCOME_DISPERSION_MIN_MILLI` | EXISTS | `diagnostics.rs:61` |
| `sample_window_counts_purchases_and_reads_yard_treasury` | EXISTS | `diagnostics.rs:812` |
| `scenario_trophic` | EXISTS | `scenario.rs:70` |
| `scenario_trophic_shape` | EXISTS | `scenario.rs:338` |
| `NUM_HAULERS` | EXISTS | `scenario.rs:40` = `12` |
| `STATION_ORBIT_AU` | EXISTS | `scenario.rs:37` |
| `MediaCfg` fold precedent | EXISTS | `config.rs:690-713` |
| `ResetError::BadMediaCfg` | EXISTS | `world.rs:154` |
| `half_on_media_config_is_rejected` | EXISTS | `world.rs:1701` |
| `engagement_diag` (World field) | EXISTS | `world.rs:85` |
| `contract_id` helper | EXISTS | `economy.rs:747` |
| `EventKind` enum | EXISTS | `contract.rs:45` |
| `GossipHeard` (last variant before plan's append point) | EXISTS | `contract.rs:168` |
| `run_one` (sweep_trophic.py) returns tuple today | EXISTS | `sweep_trophic.py:89-118`, returns `(result, media, windows)` |
| `N_HAULERS` (sweep_trophic.py) | EXISTS | `sweep_trophic.py:72` = `12` |
| `RESULT_RE` | EXISTS | `sweep_trophic.py:51` |
| `MEDIA_RE` | EXISTS | `sweep_trophic.py:60` |
| `panel` (sweep_trophic.py) | EXISTS | `sweep_trophic.py:142` |
| `media_panel` | EXISTS | `sweep_trophic.py:221` |
| `voi_line` | EXISTS | `sweep_trophic.py:273` |
| `pirate_world_cfg` | EXISTS | `pirate.rs:1176` |
| `pirate_init` | EXISTS | `pirate.rs:1167` |
| `lying_low_pirate_seeks_hideout` | EXISTS | `pirate.rs:1793` |
| `fed_pirate_camps_hungry_pirate_roams` ends at | EXISTS | `pirate.rs:1791` |
| `--scenario` CLI flag | NOT FOUND | `trophic_run.rs:55-101` errors on unknown args — **plan marks this as "to be created"** in Phase 2; verified absent today |
| `fuel_capacity_scale` knob | NOT FOUND | grep clean across codebase — **plan marks this as "to be created"**; verified absent today |
| `pirate_max_reach_au` (RunConfig field) | EXISTS (silently inherited) | `scenario.rs` pirate_max_reach_au silently inherited by scenario_frontier from scenario_trophic |

### Paths

| Path | Status | Issue |
|------|--------|-------|
| `/home/john/jumpgate/crates/jumpgate-core/src/pirate.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/events.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/world.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/economy.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/config.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/hash.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/stores.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/scenario.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/autopilot.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/ingest.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/src/contract.rs` | EXISTS | None |
| `/home/john/jumpgate/crates/jumpgate-core/tests/replay_equivalence.rs` | EXISTS | None |
| `/home/john/jumpgate/python/analysis/sweep_trophic.py` | EXISTS | None |
| `/home/john/jumpgate/python/analysis/media_log.py` | EXISTS | None |
| `diagnostics::WINDOW_TICKS` import in scenario.rs tests | MISSING PATH | `lib.rs:55` exports only `Diagnosis, TrophicSample, Verdict, classify, sample_window` — `WINDOW_TICKS` is NOT re-exported; Task 2.3 Step 1 test requires explicit `use crate::diagnostics::WINDOW_TICKS;` import |

### Versions

No library version claims verified — plan uses only standard Rust/SB3 idioms already present in the codebase. No new external dependencies introduced.

### Conventions

| Rule | Compliance | Evidence |
|------|------------|----------|
| Config structs appended at RunConfig tail (MediaCfg precedent) | COMPLIANT | Plan's RefuelCfg follows MediaCfg insertion pattern at `config.rs:690-713` |
| Hash discipline: no HASH_FORMAT_VERSION bump | COMPLIANT | Plan calls for exactly one GOLDEN_CONFIG_HASH re-pin (RefuelCfg fields) |
| always-consume-then-gate idiom (BuyUpgrade precedent) | COMPLIANT | Plan preserves `pending_refuel[crow] = None` before gate checks |
| Lockstep rule (println + regex in same commit) | COMPLIANT | Phase 0b explicitly requires both in same commit |
| Phase 0b invariant: zero goldens move | COMPLIANT | Plan explicitly prohibits golden movement in Phase 0b |

---

## Summary

- **Hallucinations found:** 1 (fixture name stand-in mis-identified as concrete symbol)
- **Path issues:** 1 (missing `WINDOW_TICKS` import path in test)
- **Version mismatches:** 0
- **Convention violations:** 0

---

## Blocking Issues

### CRITICAL-1: `one_body_one_craft_station_cfg()` — hallucinated fixture name (Task 1.2.1, Step 8)

**Plan location:** Task 1.2.1, Step 8 — `fuel_empty_is_a_contract_failure_cause` test in world.rs  
**Evidence:** `world.rs:1701` — the neighbor test `half_on_media_config_is_rejected` calls `one_body_two_stations_one_miner()` at `world.rs:1648`. No function named `one_body_one_craft_station_cfg` exists anywhere in world.rs or its test module. A search over the codebase (`grep -rn "one_body_one_craft_station_cfg"`) returns zero matches.  
**Severity:** CRITICAL — a builder following the plan step literally will get `E0425: cannot find function one_body_one_craft_station_cfg in this scope`.  
**Concrete fix:** Replace `one_body_one_craft_station_cfg()` with `one_body_two_stations_one_miner()` in Task 1.2.1 Step 8. The plan's own mitigation note ("clone its fixture call verbatim") is correct but the stand-in name was never identified as such with the correct replacement. The test needs a two-station world because the contract references two distinct station entities; `one_body_two_stations_one_miner()` at `world.rs:1648` supplies that setup.

---

## Warnings

### MAJOR-1: `Refueled` event tank permille fields typed `i64`; `permille_floor()` returns `u32` (Task 1.1.3 / Task 1.2.2)

**Plan location:** Task 1.1.3 EventKind variant definition; Task 1.2.2 settle code inline computation; Task 1.2.2 test assertions  
**Evidence:** The spec-designated `permille_floor()` helper (Task 0b, Phase 0b instrument) returns `u32` (FLOOR-rounded). The plan's `Refueled { tank_before_permille: i64, tank_after_permille: i64 }` event definition uses `i64`. The settle code inline `((fuel / cap_eff) * 1000.0).floor() as i64` does not call `permille_floor`, creating a semantic inconsistency: the `TrophicSample` fuel fields (Phase 0b) consume `permille_floor → u32`, but the `Refueled` event payload would carry `i64`. Test assertions (`0`, `500`, `555`, `805`) are type-compatible with either type and will compile either way.  
**Risk:** Not a compile error. However, any code that reads `tank_after_permille` from the event stream and compares it against a TrophicSample fuel field (both describing the same physical quantity) will be comparing `i64` to `u32`, requiring explicit casts. More importantly, it bypasses the `permille_floor` seam, which is the plan's own designated rounding authority for fuel-permille values throughout Phase 0b.  
**Concrete fix:** Change field types to `u32` and use `permille_floor(fuel, cap_eff)` in the settle call-site. If `permille_floor` is not yet defined when this code is written (Phase 1 lands before Phase 0b?), define `permille_floor` first (Task 0b.2) and import it at the settle site.  
**Ordering note:** The plan's landing order is 0a → 0b → 1 → 2 → 3. `permille_floor` is a Phase 0b product; Task 1.2.2 is Phase 1. The plan therefore has `permille_floor` available when the settle code is written. Use it.

### MAJOR-2: ground-pirate.md §5 "STALE DOC TO FIX" timing contradicts spec §6 (Phase 0a vs Phase 2)

**Plan location:** Phase 0a, Task 0a.1 (haven-lurk fix)  
**Evidence:** `/tmp/wgb-plan/ground-pirate.md` §5 states: "Fix the stale 'none in reach → the NEAREST station' doc comment on `relocate_lurk_target` in the SAME COMMIT as the leak fix." The authoritative spec (`2026-06-11-world-gets-big-design.md`) §6 states: "the stale 'nearest station' marooned doc fixed in the same commit as the Phase 2 explicit-reach factory commit." The assembled plan correctly follows the spec (stale doc fix deferred to Phase 2). However, the grounding file used by implementors is directly contradictory.  
**Risk:** A builder consulting `ground-pirate.md` during Phase 0a will add the doc fix to the Phase 0a commit, violating the spec's intent that Phase 0a be minimal (just the single-line filter fix). This does not break compilation or tests but does violate the commit sequencing the spec prescribes.  
**Concrete fix:** Either (a) annotate the grounding file's §5 with a note "SUPERSEDED by spec §6 — defer to Phase 2 factory commit", or (b) add an explicit callout in Task 0a.1 of the plan: "Do NOT fix the stale `relocate_lurk_target` doc comment here — that is a Phase 2 deliverable."

### MINOR-1: `WINDOW_TICKS` missing import in `scenario_frontier_shape` test (Task 2.3, Step 1)

**Plan location:** Task 2.3, Step 1 — `scenario_frontier_shape` test in scenario.rs  
**Evidence:** `diagnostics.rs:24` defines `WINDOW_TICKS` as a module-private constant. `lib.rs:55` exports only `Diagnosis, TrophicSample, Verdict, classify, sample_window` from `diagnostics` — `WINDOW_TICKS` is NOT re-exported. The plan's test body references `WINDOW_TICKS` without showing a `use crate::diagnostics::WINDOW_TICKS;` import. `scenario.rs` test module accesses diagnostics through the crate root; without an explicit import, this is `E0425`.  
**Concrete fix:** Add `use crate::diagnostics::WINDOW_TICKS;` to the `#[cfg(test)]` import block at the top of scenario.rs's test module, OR re-export `WINDOW_TICKS` from `lib.rs` alongside `sample_window` (likely more useful since other tests will need it). The plan should show this import explicitly in Task 2.3 Step 1's import list.

### MINOR-2: Plan cites `pirate.rs:443-445` for stale doc comment; actual block is `:441-451`

**Plan location:** Phase 2, Task 2.1 (stale doc fix step)  
**Evidence:** `relocate_lurk_target` at `pirate.rs:452` has its doc block at approximately `:441-451`. The stale "nearest station" text is within that block at roughly `:443-445`. The plan's cite of `:443-445` is within the block but misses the function start line.  
**Risk:** Cosmetic — a builder searching for the doc comment will find it without difficulty. Off-by-a-few-lines in a doc comment context.  
**Concrete fix:** Change plan reference to `pirate.rs:441-451` (full doc block) or simply cite `pirate.rs:452` (the function declaration) and describe the doc as "the preceding doc comment."

### MINOR-3: `PriceCfg` import path not shown for `refuel_world_fixture` in economy.rs tests (Task 1.2.3, Step 1)

**Plan location:** Task 1.2.3, Step 1 — `refuel_world_fixture` helper in economy.rs test mod  
**Evidence:** The plan shows `cfg.price_cfg` mutation in `refuel_world_fixture` without identifying the required `use crate::config::PriceCfg;` import. The economy.rs test module imports many types already; `PriceCfg` is available at `crate::config::PriceCfg`. A builder following the plan will hit `E0412: cannot find type PriceCfg` if the import is absent from their test mod import block.  
**Risk:** Minor — easily resolved, and the builder is likely to check neighboring imports. But the plan's code block should be self-contained.  
**Concrete fix:** Add `use crate::config::PriceCfg;` (or confirm it's already in economy.rs test module's existing import list) in Task 1.2.3 Step 1.

---

## Confidence Assessment

**Overall Confidence:** High

| Finding | Confidence | Basis |
|---------|------------|-------|
| CRITICAL-1: `one_body_one_craft_station_cfg` not found | High | `grep -rn "one_body_one_craft_station_cfg"` across full codebase returned zero matches; `world.rs:1701` neighbor test confirmed to use `one_body_two_stations_one_miner` at `:1648` |
| MAJOR-1: `Refueled` i64 vs `permille_floor` u32 type inconsistency | High | `permille_floor` return type is deterministically `u32` from FLOOR rounding; plan's field definition uses `i64`; verified against diagnostics.rs field types |
| MAJOR-2: ground-pirate.md timing contradiction | High | Both files read directly; texts are directly contradictory on "same commit" timing |
| MINOR-1: `WINDOW_TICKS` not re-exported | High | `lib.rs:55` re-export list read directly; `WINDOW_TICKS` absent; `diagnostics.rs:24` confirmed module-private |
| MINOR-2: Line number offset for stale doc | Moderate | Based on reading pirate.rs around `:441-452`; exact line numbers of individual doc lines may vary by 1-2 after any subsequent edits |
| MINOR-3: `PriceCfg` import gap | Moderate | economy.rs test module import list not exhaustively verified; may already include `PriceCfg` from an existing import |

---

## Risk Assessment

**Implementation Risk:** Medium  
**Reversibility:** Easy (all fixes are additive or substitutional, no data migration)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Builder uses hallucinated `one_body_one_craft_station_cfg` → compile error + confusion | High | Certain (if CRITICAL-1 unaddressed) | Replace with `one_body_two_stations_one_miner` in plan |
| `Refueled` event permille fields use wrong type → semantic inconsistency with TrophicSample | Medium | Likely (if MAJOR-1 unaddressed) | Use `permille_floor → u32` consistently |
| Builder follows ground-pirate.md §5 in Phase 0a → commits stale doc fix to wrong commit | Low | Possible (grounding files are consulted) | Annotate ground file or add plan callout |
| `WINDOW_TICKS` missing import → compile error in scenario_frontier_shape test | Medium | Certain (if MINOR-1 unaddressed) | Add explicit import or re-export from lib.rs |
| Minor line-number discrepancy causes builder to search wrong area | Low | Unlikely | Clarify doc block span |

---

## Information Gaps

1. [ ] **`PriceCfg` in economy.rs test module existing import list**: Would confirm or refute MINOR-3. Reading the full economy.rs test module `use` block would resolve this.
2. [ ] **`LurkMoved` event threading**: The plan requires threading `&mut EventStream` through `run_pirate_brains` in Phase 2 to emit `LurkMoved`. The current signature at `pirate.rs:511` does not include `EventStream`. This threading is described as a Phase 2 deliverable — verified consistent with plan intent, but not independently verified that no intermediate phase inadvertently requires the event before Phase 2.
3. [ ] **`fuel_capacity_scale` / RefuelCfg field names**: Plan introduces `RefuelCfg { lot_mass: f64, price_per_lot: u32 }` as new fields. Field name correctness cannot be verified against existing code since these are "to be created." The lot_mass == 0.0 trophic inertness gate is the only externally constrainable behavior; correctness depends on the plan's own consistency.

---

## Caveats & Required Follow-ups

### Before Relying on This Analysis
- [ ] Verify CRITICAL-1 fix (`one_body_two_stations_one_miner`) by reading `world.rs:1648-1690` and confirming the fixture produces a usable two-station world for the `FuelEmpty` contract failure test
- [ ] Re-run Reality check after plan revisions (especially if Task 1.1.3 field types change)

### Assumptions Made
- Symbol-extraction verified via targeted grep; dynamic dispatch or macro-generated names may produce false negatives
- "Stand-in name" label in the plan was interpreted as the plan author admitting the fixture name is a placeholder — this interpretation is supported by the plan's own mitigation note but was not stated with full clarity in the plan text
- lib.rs re-export list is complete as read; no conditional compilation (`#[cfg(...)]`) re-exports were checked

### Limitations
- This analysis covers static symbol existence and import correctness only
- Runtime behavior (e.g., whether `permille_floor` rounds identically to `.floor() as i64` for the specific fuel values tested) is not verified
- Cross-phase ordering constraints (e.g., Phase 0b instrument being available when Phase 1 settle code is written) are assumed from the plan's own phase sequence, not independently verified from a dependency graph
