# Quality Review — world-gets-big implementation plan

Spec: `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` (APPROVED, OD-1..7 resolved)
Plan: `/tmp/wgb-plan/assembled-plan.md`
Grounding: `/tmp/wgb-plan/ground-*.md`
HEAD at drafting: `e7e490e`

---

## Test Strategy

**Test approach:** TDD throughout. Every task opens with a failing test step before
any implementation code is written. Commands are exact (`cargo test -p
jumpgate-core <name>`). Test file paths are named in each task's **Files**
section. No blanket "add tests" instructions appear.

| Task | Test Type | Test File | Command | Status |
|------|-----------|-----------|---------|--------|
| 0a.1 Haven-lurk leak | Unit | `pirate.rs` tests mod | `cargo test -p jumpgate-core pirate_lurk_excludes_haven` | OK |
| 0b.1 permille_floor | Unit | `diagnostics.rs` tests mod | `cargo test -p jumpgate-core permille_floor` | OK |
| 0b.2 META anchored line | Runner (stdout) | `trophic_run.rs` + sweep | manual run + grep | OK |
| 0b.3 FuelDiag | Unit | `diagnostics.rs` tests | `cargo test -p jumpgate-core fuel_diag` | OK |
| 0b.4 TrophicSample fields | Unit | `diagnostics.rs` tests | `cargo test -p jumpgate-core sample_window` | OK |
| 0b.5 FUEL line | Runner | `trophic_run.rs` | manual run | OK |
| 1.1 eps re-bake | Unit | `scenario.rs` + `economy.rs` | `cargo test -p jumpgate-core edge` | OK |
| 1.2 hash-neutrality audit | Cross-branch digest | — | `git worktree` + sha256 | OK |
| 1.2.1 RefuelCfg | Unit | `scenario.rs` tests | `cargo test -p jumpgate-core refuel_cfg` | OK (see MAJOR-3) |
| 1.2.2 pending_refuel | Unit | `stores.rs` + `world.rs` | `cargo test -p jumpgate-core pending_refuel` | OK |
| 1.2.3 resolve_refuels | Unit | `economy.rs` tests | `cargo test -p jumpgate-core refuel_settles` | OK (see CRITICAL-1) |
| 1.2.4 run_refuel_policies | Unit | `economy.rs` tests | `cargo test -p jumpgate-core refuel_writes_pending` | OK (see MINOR-1) |
| 1.2.5 FUEL-C1 dv_remaining | Unit | `economy.rs` tests | `cargo test -p jumpgate-core refuel_rederives` | OK |
| 1.2.6 PLAY-C1 + ContractFailed | Unit | `economy.rs` tests | `cargo test -p jumpgate-core scripted_assign_filters_dry` | OK |
| 1.2.7 phase-1 exit digest | Cross-branch digest | — | `git worktree` + sha256 | OK (see MAJOR-4) |
| 2.1–2.5 frontier factory | Unit | `scenario.rs` tests | `cargo test -p jumpgate-core scenario_frontier` | OK |
| 2.6 LurkMoved | Unit | `pirate.rs` tests | `cargo test -p jumpgate-core lurk_moves_emit` | OK |
| 2.7 craft_role + epilogue | Unit + runner | `world.rs` + `trophic_run.rs` | `cargo test -p jumpgate-core craft_role` | OK |
| 2.8–2.9 TrophicSample frontier | Unit | `diagnostics.rs` tests | `cargo test -p jumpgate-core trophic_sample` | OK |
| 2.10 frontier golden | Unit (printer + pinned) | `scenario.rs` tests | `cargo test -p jumpgate-core golden_frontier` | OK |
| 2.11 fuel_capacity_scale knob | Unit | `scenario.rs` tests | `cargo test -p jumpgate-core fuel_capacity_scale_knob` | OK |
| 2.12 calibration bake | Manual ensemble + pin | `scenario.rs` | runner + re-pin | OK |
| 3.1–3.7 science + console | Runner + Python | various | `pytest` + sweeps | OK |
| Refueled/ContractFailed chronicle arms | None assigned | — | — | MISSING (see CRITICAL-2) |

**Coverage:** Not specified numerically, but unit tests cover the core happy path
and all named skip arms for refuel settle; cross-branch digest is the
determinism proof.

**Gaps:**
- `Refueled` and `ContractFailed` chronicle arms (`chronicle_subject` in
  `trophic_run.rs`) have no dedicated numbered task with TDD steps — see CRITICAL-2.

---

## Observability

The plan correctly establishes `FuelDiag` (Task 0b.3) as the per-craft
diagnostics instrument, anchored stdout lines (Task 0b.2/0b.5), and the
FUEL/FUEL_RE JSONL rows (Task 2.9). Error paths in `resolve_refuels` are
deterministic skip arms, not exceptions; the settle function is called on every
tick so silenced-by-skip is acceptable given the event-emit on success.

| Error Path | Logging/Instrumentation | Status |
|------------|------------------------|--------|
| Undocked refuel intent | deterministic `continue`; no event | OK (by design) |
| Dry dock (stock 0) | deterministic `continue`; no event | OK (by design) |
| Wallet short | deterministic `continue`; no event | OK |
| Stale corp row | deterministic `continue`; no event | OK (corp-index precedent) |
| FuelEmpty / ContractFailed | `ContractFailed` event emitted | OK in event stream |
| `ContractFailed` in chronicle | NOT captured — no chronicle arm | MISSING (CRITICAL-2) |
| `Refueled` in chronicle | NOT captured — no chronicle arm | MISSING (CRITICAL-2) |
| LurkMoved in chronicle | Covered by Task 2.6 | OK |
| ADRIFT epilogue | Covered by Task 2.7 | OK |
| FuelDiag per-craft fields | Task 0b.3 | OK |

**Windows-not-gates compliance:** The plan is consistent throughout. Every
measurement uses pre-registered band windows (spec §11). No new pass/fail gates
are introduced beyond the existing `--assert-no-fuel-empty` trophic-only flag,
which is correctly scoped and cited. PDR-0006 framing is explicit at every lab
task.

---

## Determinism Discipline

| Proof | Placement | Status |
|-------|-----------|--------|
| eps re-bake band-neutrality | Phase-1 Task 1.2 (immediately after Task 1.1 eps commit) | OK |
| Phase-1 exit cross-branch 2000-tick digest | Task 1.2.7 (phase-1 exit, lot-0 inertness) | OK (see MAJOR-4) |
| RefuelCfg golden re-pin | Task 1.2.1 Step 14 (single-cause commit) | OK |
| Frontier trajectory golden | Task 2.10 (printer + pinned test) | OK |
| Calibration-driven v_e bake + re-pin | Task 2.12 (follows calibration ensemble) | OK |
| HASH_FORMAT_VERSION stays 5 | Stated throughout, never bumped | OK |
| Golden literals typed from plan | PROHIBITED — plan requires paste from `#[ignore]` printer | OK |

---

## Fixture Redesign (eps re-bake)

The plan (Task 1.1) documents per-fixture arithmetic for both families:

- **Family A** (fixtures with `fuel_mass = FUEL_EMPTY_EPS`): math documented at spec §4
  item 1 — `FUEL_EMPTY_EPS 1e-9 → 1e-11` means the fixture must be updated to
  `fuel_mass: 1e-11`. Named fixtures are listed individually.
- **Family B** (fixtures with `prev_fuel = fuel_mass`): the prev-fuel copy-forward
  discipline is documented; each fixture that sets both fields is updated
  consistently.

This is principled per-fixture redesign, not a blanket nudge. The plan correctly
warns "do NOT touch fixtures that don't reference the old eps."

---

## Calibration → Bake Chain Ordering

| Step | Task | Status |
|------|------|--------|
| Instrument lands (`FuelDiag`, FUEL line) | Phase 0b (before Phase 1) | OK |
| Refuel mechanic ships (gated off at lot=0) | Phase 1 | OK — instrument precedes mechanic |
| `fuel_capacity_scale` knob arm | Task 2.11 (Phase 2 second half) | OK |
| Calibration ensemble run (100x tank) | Task 2.12 Step 1 | OK — knob precedes ensemble |
| v_e bake derived from ensemble output | Task 2.12 Step 2 | OK — ensemble precedes bake |
| Frontier trajectory golden re-pinned | Task 2.12 Step 4 (single-cause commit) | OK |

Calibration-before-bake ordering is correct.

---

## CRITICAL Findings

### CRITICAL-1: `resolve_refuels` bypasses `permille_floor()` seam — type mismatch and "one seam" violation

**Plan location:** Task 1.2.3, Step 3 implementation, lines 2382 and 2391 of assembled-plan.md

**Evidence:**
- Task 0b.1 (`assembled-plan.md:345-424`) establishes `permille_floor(num: f64, den: f64) -> u32` in `diagnostics.rs` as "the ONE f64→integer seam for fuel instruments (world-gets-big phase 0b; spec §7 pins FLOOR). Milli-AU radii ride the same form. … the phase-1 `tank_before_permille`, rides the same seam later." The doc comment on `permille_floor` names phase-1 explicitly.
- Task 1.2.3 Step 3 (`assembled-plan.md:2382`) writes: `let tank_before_permille = ((fuel / cap_eff) * 1000.0).floor() as i64;`
- Line 2391: `let tank_after_permille = ((ships.fuel_mass[crow] / cap_eff) * 1000.0).floor() as i64;`
- The `Refueled` event variant definition (lines 2259-2263) declares both fields as `i64`, which is consistent with the inline cast but inconsistent with the `permille_floor() -> u32` return type.
- `ground-fuel-edge.md` GOTCHA 1 states: "no f64→permille precedent exists; for `tank_before_permille` the pinned form should be `permille_floor(fuel, cap_eff) as i64`; pin it with a test."
- The plan pins the inline form in its own test `refuel_tank_permille_is_floor_rounded` (lines 2129-2153), which means future readers have two competing "seams" that can diverge if either is edited.

**Risk:** The seam principle exists to ensure a single audit point. With the inline form in `resolve_refuels`, any future change to `permille_floor` (e.g., clamping NaN differently) does not automatically propagate to the event payload. The f64 expressions in lines 2382 and 2391 are arithmetically equivalent to the current `permille_floor` implementation, but they are not linked to it, and they independently re-implement the rounding without the NaN/negative/zero-denominator guards present in `permille_floor`.

**Concrete fix:**
1. Change the `Refueled` event variant field types from `i64` to `u32`:
   ```rust
   tank_before_permille: u32,
   tank_after_permille: u32,
   ```
2. In `resolve_refuels` Step 3, replace both inline computations:
   ```rust
   // Before:
   let tank_before_permille = ((fuel / cap_eff) * 1000.0).floor() as i64;
   // After:
   use crate::diagnostics::permille_floor;
   let tank_before_permille = permille_floor(fuel, cap_eff);
   // ...after the tank write:
   let tank_after_permille = permille_floor(ships.fuel_mass[crow], cap_eff);
   ```
3. Update `refuel_tank_permille_is_floor_rounded` test assertions to use `u32` literals (the values remain the same: 555, 805).
4. Update the test in `refuel_settles_quantized_with_four_legs_and_exact_event` (line 2114-2115): `tank_before_permille: 0u32, tank_after_permille: 500u32`.

Note: Adding a `use crate::diagnostics::permille_floor;` import to `economy.rs` is appropriate since `economy.rs` already reads from `diagnostics` for `TrophicSample`.

---

### CRITICAL-2: `Refueled` and `ContractFailed` chronicle arms have no dedicated numbered task

**Plan location:** `assembled-plan.md:3198-3204` (Cross-section handoffs), plus Phase 3 Task 3.5 line 6984

**Evidence:**
- Line 3200: "Chronicle arms for `Refueled`/`ContractFailed` in `trophic_run.rs` (`chronicle_subject`'s catch-all `_ => None` at :262 means the new variants compile but silently vanish from the chronicle) — owned by the lab/chronicle section (spec §7 printer-side, phase 0b/2 beats)." This is a cross-section handoff note, not a numbered task.
- Task 2.6 (line 4359) covers `LurkMoved` with full TDD steps including the `chronicle_subject` arm.
- Task 2.7 (line 4646) covers the per-craft epilogue with full TDD steps.
- Phase 3 Task 3.5 (line 6984) adds a `Refueled` arm to `write_gossip_log` only (a separate match in the runner), not to `chronicle_subject`.
- `ground-events-chronicle.md` GOTCHA 1: "`chronicle_subject` ends `_ => None` (trophic_run.rs:262): adding Refueled/ContractFailed/LurkMoved to the enum compiles clean but they never print — you must add match arms in BOTH `chronicle_subject` and its consumers."
- Spec §7 states: "chronicle printer-side: role, workplace radius, tank permille, credits, `ADRIFT since t=…`" — implies the refuel events should appear in the per-craft chronicle trace when using `--chronicle`.

**Risk:** Without a numbered task with TDD steps, the builder can close Phase 1 and Phase 2 with `Refueled` and `ContractFailed` events confirmed in the event stream but silently absent from `--chronicle` output. The spec §7 printer-side requirement is not verifiably satisfied. This is a correctness gap detectable only by manual `--chronicle` inspection; it has no unit test to catch it.

**Concrete fix:**
Add a dedicated task between Task 2.7 (epilogue) and Task 2.8 (TrophicSample frontier fields) — call it Task 2.7b:

```
### Task 2.7b: chronicle_subject arms for Refueled and ContractFailed

Files:
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (chronicle_subject :242-264)

Step 1: Write the failing tests (runner-level, so red/green are run commands):
  cargo run -q -p jumpgate-core --release --example trophic_run \
    --seed 7 --ticks 10000 --chronicle 2>&1 | grep -c "refueled at"
  Expected failure: 0 (the arm is absent; Refueled events exist in the frontier run but chronicle_subject drops them).

Step 2: Add the match arms in chronicle_subject for both variants:
  EventKind::Refueled { craft, .. } => Some(craft),
  EventKind::ContractFailed { craft, .. } => Some(craft),

Step 3: Re-run — expected non-zero count for --scenario frontier with refuel live.
```

Alternatively, fold the two `chronicle_subject` arms into Task 1.2.3 (when
`Refueled` is appended to `EventKind`) and Task 1.2.6 (when `ContractFailed` is
appended), each as a numbered sub-step immediately after the variant is added. The
ground-events-chronicle GOTCHA explicitly names this as a same-commit discipline.

---

## MAJOR Findings

### MAJOR-1: Task 1.2.1 Step 8 reset error test uses a stand-in fixture name

**Plan location:** `assembled-plan.md`, Task 1.2.1, Steps 8-9

**Evidence:** The plan states: "Use whatever station-bearing fixture the neighbouring `BadMediaCfg` test uses — clone its fixture call verbatim; the name above is a stand-in for that exact fixture." The named fixture `one_body_one_craft_station_cfg()` may or may not exist at `world.rs:1700` in the actual codebase — the plan defers the lookup to build time without providing the concrete name.

**Risk:** Low compilation risk (the builder is instructed to look it up), but the plan is not self-contained. If the wrong fixture is chosen, the reset error test may pass vacuously (e.g., if the fixture has no `RefuelCfg` field, the test proves nothing about the `lot_mass > 0` reset guard). The "clone verbatim" instruction without naming the target is an ambiguous spec.

**Concrete fix:** Before the plan is handed to the builder, resolve the fixture name by running `grep -n "BadMediaCfg" /home/john/jumpgate/crates/jumpgate-core/src/world.rs` and replacing the stand-in text with the actual fixture name and its file:line. The plan should read "clone `<fixture_name>` from world.rs:<line>" not "clone whatever the neighbouring test uses."

---

### MAJOR-2: `<PRE>` placeholder in Task 1.2.7 cross-branch digest will fail if copied verbatim

**Plan location:** `assembled-plan.md`, Task 1.2.7, Step 3

**Evidence:** The bash command block reads:
```
git worktree add /tmp/wgb-pre-phase1 <PRE>
```
where the plan's prose says `<PRE>` is "the last commit BEFORE phase 1's first commit." This is not a literal SHA and will produce a `git` error if the builder copies the command verbatim.

**Risk:** Medium. A builder running under time pressure may copy the block without reading the prose above it, causing the digest step to fail at the worktree-add line. This is the ONLY determinism proof for Phase 1 (the trophic-inertness gate plus lot-0 inertness); if it is skipped or broken, refuel ships with no inertness gate are never proven bit-identical to the pre-phase-1 baseline.

**Concrete fix:** Add an explicit resolution step:
```
Step 2 (pre-flight): Identify the PRE sha:
  git log --oneline | grep -m1 "phase-0b"
  # The commit immediately before the first phase-1 commit. Substitute its sha
  # for every <PRE> below.
```
Alternatively, compute the SHA at plan-write time from the current HEAD and paste it literally with a provenance comment.

---

## MINOR Findings

### MINOR-1: `run_refuel_policies` no-clobber guard is untestable by construction; no compensating test

**Plan location:** `assembled-plan.md`, Task 1.2.4, Step 5 note

**Evidence:** The plan acknowledges: "the no-clobber guard cannot be black-box-distinguished — the intent payload is the unit type, so an overwrite of `Some(())` with `Some(())` is unobservable." No compensating white-box test is offered. The guard's only observable effect is the ABSENCE of duplicate policy-writes when a craft already has intent — but since both states are `Some(())`, the guard cannot be distinguished from no-guard by any event or state comparison.

**Risk:** Low. The no-clobber is a policy property, not a correctness property — the settle stage is idempotent regardless of whether the intent was written once or twice. However, the plan names this guard as an explicit invariant without any test anchoring it.

**Concrete fix (partial):** Add a white-box test that: (1) manually sets `pending_refuel[0] = Some(())` before `world.step()`, (2) runs one tick while the craft is in-flight (undocked, so policy cannot fire), and (3) asserts that after the step, `pending_refuel[0]` is `None` (the settle stage consumed the manually-written intent, and the policy did NOT re-write it). This exercises the intent-consumed-without-policy-overwrite path and serves as a partial guard.

---

## Summary

| Category | Count |
|----------|-------|
| Critical | 2 |
| Major | 2 |
| Minor | 1 |

**Blocking Issues (must be resolved before execution):**
- CRITICAL-1: `resolve_refuels` bypasses the established `permille_floor()` seam, creating a type mismatch (`i64` vs `u32`) and a second independent rounding implementation that can diverge from the seam on edge inputs (NaN, negative values, zero denominator).
- CRITICAL-2: `Refueled` and `ContractFailed` chronicle arms have no numbered task and no TDD steps; the `_ => None` catch-all in `chronicle_subject` will silently swallow both new event variants through the end of Phase 2.

**Warnings:**
- MAJOR-1: Stand-in fixture name in Task 1.2.1 Step 8; builder must look it up.
- MAJOR-2: `<PRE>` placeholder in Task 1.2.7 cross-branch digest; builder must resolve to a literal SHA.
- MINOR-1: `run_refuel_policies` no-clobber guard has no test; acknowledged by plan but not compensated.

---

## Confidence Assessment

**Overall Confidence:** High

| Finding | Confidence | Basis |
|---------|------------|-------|
| CRITICAL-1 permille_floor seam bypass | High | Verified against Task 0b.1 full text (lines 345-424) and Task 1.2.3 Step 3 implementation text (lines 2382, 2391); `ground-fuel-edge.md` GOTCHA 1 names the exact seam call; the event variant definition (lines 2259-2263) uses `i64` confirming the type inconsistency |
| CRITICAL-2 chronicle arms absent | High | Exhaustive search of assembled-plan.md for "chronicle_subject", "Refueled", "ContractFailed", "arm" confirms no numbered task owns TDD steps for these two arms; Task 2.6 (LurkMoved) and Task 2.7 (epilogue) are present as comparators; `ground-events-chronicle.md` GOTCHA 1 explicitly names the silent-swallow risk |
| MAJOR-1 stand-in fixture name | High | Plan text at Task 1.2.1 explicitly says "stand-in for that exact fixture" |
| MAJOR-2 `<PRE>` placeholder | High | Verified: `<PRE>` appears literally in the bash block at Task 1.2.7 Step 3; it is not a valid git ref |
| MINOR-1 no-clobber untestable | High | Plan itself acknowledges this at Task 1.2.4 Step 5 note; no compensating test exists in the task |
| False-alarm retracted: `ids_at` API | High | Confirmed `ids_at` exists at `stores.rs:289` — previous draft finding retracted |
| False-alarm retracted: `apply_knob` for `fuel_capacity_scale` | High | Task 2.11 (lines 5447-5541) is a fully-formed TDD task — previous draft finding retracted |
| False-alarm retracted: ADRIFT epilogue | High | Task 2.7 (lines 4646-4816) covers the per-craft epilogue with TDD — previous draft finding retracted |

---

## Risk Assessment

**Implementation Risk:** Medium (two critical issues, both correctable before build starts)
**Reversibility:** Easy (all findings are plan amendments, no code written yet)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Two `permille` implementations diverge silently in a future refactor | High | Certain without fix (two codepaths with same formula but different guards) | Apply CRITICAL-1 fix before Phase 1 starts |
| `--chronicle` output silently omits refuel and failure events | High | Certain without fix (catch-all proven to swallow) | Insert Task 2.7b or fold arms into Tasks 1.2.3/1.2.6 before Phase 2 closes |
| Phase-1 exit digest step fails at `git worktree add <PRE>` | Medium | Likely if blocks are copy-pasted | Apply MAJOR-2 fix (resolve SHA) |
| Wrong reset-error fixture means guard test is vacuous | Low | Possible | Apply MAJOR-1 fix (name concrete fixture) |
| No-clobber guard regresses undetected | Low | Possible but consequence is cosmetic | Apply MINOR-1 compensating test |

---

## Information Gaps

The following would improve this analysis:

1. [ ] **Fixture inventory at world.rs:~1700** — The plan defers the concrete fixture name for Task 1.2.1 Step 8 to build-time lookup. Resolving it now would eliminate MAJOR-1.
2. [ ] **Confirmed SHA of the last pre-phase-1 commit** — Would eliminate MAJOR-2 without requiring the builder to run an extra git command.
3. [ ] **Cross-check: does `economy.rs` already import `diagnostics`?** — If yes, adding `use crate::diagnostics::permille_floor;` in `resolve_refuels` is a one-line fix; if not, the fix is still straightforward but requires an import.

---

## Caveats and Required Follow-ups

### Before Relying on This Analysis
- [ ] Confirm that `permille_floor` is not re-exported from a location already imported by `economy.rs` (would simplify the CRITICAL-1 fix).
- [ ] Verify the `chronicle_subject` function signature accepts `EventKind` variants by-pattern (not by trait) — this is the assumed form based on ground-events-chronicle.md, but the catch-all `_ => None` confirms it.

### Assumptions Made
- `permille_floor` in `diagnostics.rs` is not already re-exported from `lib.rs` into `economy.rs`'s scope.
- The cross-section handoff note at line 3200 is the ONLY place `Refueled`/`ContractFailed` chronicle arms are discussed — confirmed by exhaustive grep.
- Task 2.7b does not already exist in a later draft section — confirmed by full task heading search (no Task 2.7b found).

### Limitations
- This analysis does NOT assess symbol existence (Reality reviewer scope).
- This analysis does NOT assess architecture patterns (Architecture reviewer scope).
- The `ground-*.md` files may lag behind plan revision; findings are anchored to assembled-plan.md line references.
