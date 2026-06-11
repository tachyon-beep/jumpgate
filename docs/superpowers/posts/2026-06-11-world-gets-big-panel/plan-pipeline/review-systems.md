# Systems Review: world-gets-big implementation plan

Reviewer: Systems Agent
Plan: /tmp/wgb-plan/assembled-plan.md (assembled from draft-0a, draft-0b, draft-1, draft-2a, draft-2b, draft-3)
Spec: docs/superpowers/specs/2026-06-11-world-gets-big-design.md (APPROVED; OD-1..7 resolved in §13)
HEAD: e7e490e
Grounding: /tmp/wgb-plan/ground-*.md


## Dependency Analysis

**Components changed (phase-by-phase):**
- Phase 0a: `pirate.rs` (haven-lurk filter), `trophic_run.py` (band constants)
- Phase 0b: `events.rs` (FUEL_EMPTY_EPS), `scenario.rs` (fuel_mass init sites), `trophic_run.py` (FuelDiag, FUEL anchored line), `trophic_run_test.py`
- Phase 1: `events.rs` (eps 1e-9→1e-11), `economy.rs` (ASSIGN fuel-eligibility filter), `world.rs`/`scenario.rs` (fuel_mass init), `trophic_run.py` (FuelDiag fields), `trophic_run_test.py`, Python harness
- Phase 2a: `scenario.rs` (scenario_frontier factory), `economy.rs` (live pricing + RefuelCfg), `world.rs` (refuel verb stages 1c3b/1d2), `run.py` (--scenario flag), `ephemeris.rs` (guard)
- Phase 2b: `pirate.rs` (LurkMoved event), `chronicle logic`, `trophic_run.py` (TrophicSample frontier fields, FUEL refuel fields), `trophic_run_test.py`, `FRONTIER_TRAJECTORY_GOLDEN`, `fuel_capacity_scale` knob
- Phase 3: sweep passthrough, dual-map HHI/slack re-fit, I1/I2 panels, coverage re-denominator, headline grid, console session packet

**Dependency chain (simulate return tuple — the most brittle implicit contract):**
```
simulate() at HEAD → (samples, hashes, world)
    ↓ Phase 0b Task 0b.2
simulate() → (samples, hashes, world, meta: MetaFacts)
    ↓ Phases 2b Tasks 2.7, 2.10 (trophic_run.py edits)
    ↓ Phase 3 Task 3.4
simulate() → (samples, hashes, world, meta, endpoint_rows)
    ↓
panels (I1, I2), headline grid, sweep driver
    ↓
owner console session
```

**Ripple risk:** High — the tuple is destructured at every call site; each phase extension adds a positional slot that all subsequent consumers must honour.


## Feedback Loops

| Potential Loop | Type | Risk | Plan Coverage |
|---|---|---|---|
| eps raised (Task 1.1) then filter added (Task 1.2.6): if misordered, all haulers fail filter → no contracts dispatched → world appears dead | Reinforcing dead-world | CRITICAL | Prerequisite note only — no automated enforcement |
| Hauler strands (fuel ≤ eps, Idle forever) → never dispatched again → contracts pile up Offered → W9 fires | Balancing (but one-directional) | Medium | W9 window records it; no recovery mechanism |
| Dispatch filter + low-density frontier: if refuel verb fails silently → crafts drain → more strand → threshold of dead-run reached with no alert | Reinforcing | Medium | FuelDiag diagnostic; no circuit-breaker |

No dangerous runaway loops identified beyond the ordering issue. The pirate / hauler predator-prey loop is unchanged by this plan.


## Historical Pattern Match

| Pattern | Match Level | Concern |
|---|---|---|
| "Implicit contract on tuple shape" | Yes | simulate() return value grows across 3 phases; no single authoritative spec of final shape |
| "Ordering dependency implicit in prose" | Yes | eps-then-filter ordering is documented as a prerequisite note, not enforced |
| "Analytic prior replaced by measurement later" | Partial | v_e=1.0 golden created in Task 2.10, then overwritten by Task 2.12 calibration — two commits, one cargo test failure window in between |
| "Procedural calibration step without automated guard" | Partial | Phase 3 positive-control ordering: species-injection must precede first frontier read; human-execution-only |


## Failure Mode Analysis

| Change | Silent Failure | Loud Failure | Idempotent? |
|---|---|---|---|
| eps re-bake (Task 1.1) | None — changes a constant, all tests recompile | cargo test fails if golden stale | Yes |
| ASSIGN fuel filter (Task 1.2.6) applied before eps lowered | Yes — world dead, no contracts dispatched, no error | None (Rust compiles, tests pass on isolated fixture) | Yes |
| simulate() tuple extended (Tasks 0b.2, 3.4) | Compile error if destructure pattern not updated everywhere | cargo test fails | Yes |
| v_e=1.0 golden (Task 2.10) before calibration (Task 2.12) | No — cargo test fails after 2.12 changes FRONTIER_HAULER_EXHAUST_VELOCITY | Loud: test fails | Yes (re-pin is a literal replacement) |
| FUEL_RE optional tail (Task 2.9) | Panel raises TypeError if refuels field read without None guard | TypeError at runtime | Yes (parse is pure read) |
| Coverage re-denominator (Task 3.4) | Wrong coverage metric silently if denominator is swapped but old data banked | None | Yes |
| Phase 0a haven-lurk fix without band re-judgment | Band constants fit on leaked data remain live; console session never happens | None | N/A — requires owner action |


## Integration Point Stress

| Integration Point | Failure Modes | Plan Coverage |
|---|---|---|
| `FUEL_EMPTY_EPS` constant site (events.rs:16) | Mis-ordered eps+filter change → all haulers excluded | Prerequisite note; no test guard |
| simulate() return tuple (trophic_run.py, run.py) | Extra positional return slot not destructured → wrong variable binding | Additive field law; Task 3.4 says "match whatever simulate returns" |
| FUEL anchored line regex (FUEL_RE) | New optional tail groups return None on banked output | Lockstep rule (regex+println in same commit); None-handling unconfirmed in panel code |
| RefuelCfg in CraftStore | pending_refuel all-None hash assert (parallel to pending_upgrade) | Task 1.3 asserts all-None before any refuel verb |
| scenario_frontier factory | ephemeris_window=120,000 > trophic 100,000; silent clamp could truncate orbit data | ground-scenario-config.md notes clamp; plan adds guard |
| Task 3.4 endpoint_rows | Depends on simulate() shape from phase 0b onward being stable | "Match whatever simulate returns after phases 0b–2" — shape not pinned |


## Timing and Ordering Assumptions

| Assumption | Location in Plan | What Breaks It | Severity |
|---|---|---|---|
| Task 1.1 (eps re-bake) MUST precede Task 1.2.6 (ASSIGN filter) | Phase 1 intro prerequisite note | Worker implementing tasks linearly 1.2.1..1.2.6 without reading intro | CRITICAL |
| Task 2.10 golden at v_e=1.0 is a TRANSIENT golden (replaced by Task 2.12) | Phase 2b architecture note | cargo test between 2.10 and 2.12 passes at wrong v_e; 2.12 changes constant → test fails until re-pinned | MAJOR |
| Phase 3 positive-control injection BEFORE first frontier read (Task 3.2 Step 5) | Task 3.2 procedural instruction | Worker skips injection, records frontier reads on unvalidated instrument | Minor |
| Band re-judgment session AFTER Phase 0a fix (spec §6, OD-3) | Phase 0a commit message says "scheduled"; spec §9 landing order | No explicit task requiring owner sign-off before rung close | MAJOR |
| simulate() tuple shape stable after Phase 0b before Phase 3.4 extends it | Task 3.4 wording | Any Phase 2b edit to simulate() not accounted for in Task 3.4's destructure | MAJOR |


---

## CRITICAL Issues (1)

### C1 — Dispatch filter (Task 1.2.6) can silently blacklist all trophic haulers if misordered relative to eps re-bake (Task 1.1)

**Plan location:** Phase 1 intro "prerequisite" note; Task 1.2.6 filter step.

**Evidence:**
- `crates/jumpgate-core/src/events.rs:16`: `pub const FUEL_EMPTY_EPS: f64 = 1e-9;` (HEAD)
- `crates/jumpgate-core/src/scenario.rs:113,126`: `fuel_mass: 1.0e-9` for every trophic hauler at spawn
- `crates/jumpgate-core/src/events.rs:50`: edge predicate is `fuel_prev > FUEL_EMPTY_EPS` (strict greater-than)
- `/tmp/wgb-plan/ground-fuel-edge.md`: "eps-straddle gotcha" — at eps=1e-9, `1.0e-9 > 1e-9 == FALSE`

The proposed ASSIGN filter in Task 1.2.6 is `fuel_mass > FUEL_EMPTY_EPS`. If this filter is applied while `FUEL_EMPTY_EPS` is still 1e-9, every trophic hauler spawns with `fuel_mass = 1.0e-9 = FUEL_EMPTY_EPS`, so `1.0e-9 > 1.0e-9 = FALSE`. Every Idle hauler fails the eligibility check on every tick. No contracts are ever dispatched. The trophic world runs silently to termination with no contracts delivered and no error raised — existing tests will pass because they use isolated fixtures, not scenario_trophic at the system level.

The ordering is documented as a prose prerequisite in the Phase 1 introduction, but nothing enforces it: Rust compiles either way, no intermediate test covers the system-level dispatch count before the full trophic golden is re-run.

**Fix required:**
Add an integration test in the Phase 1 test suite (or Task 1.2.6 acceptance criteria) that asserts `scenario_trophic(7)` produces at least N contracts dispatched within M ticks when run with the new filter active. This test will fail if Task 1.2.6 is applied before Task 1.1 because no hauler clears the eligibility gate. Alternatively, restructure the task numbering to make Task 1.1 a numbered sub-step of the ASSIGN filter task (i.e., 1.2.6 cannot start until 1.2.5+1 = 1.2.5.eps is done), making the dependency explicit in the task tree rather than a prose note.

A secondary safeguard: the FuelDiag diagnostics added in Phase 0b (Task 0b.4) should show all-zero `duty` readings if every hauler is ineligible — add a low-watermark assertion on `duty` in the Phase 1 trophic_run_test to catch a dead world.


---

## MAJOR Issues (3)

### M1 — v_e bake golden re-pin: Task 2.10 creates a transient golden at analytic prior; Task 2.12's explicit re-pin step was not visible in the plan

**Plan location:** Phase 2b Tasks 2.10 and 2.12; Phase 2b architecture note ("re-pinned once more by the v_e bake, cause documented").

**Evidence:**
- `/tmp/wgb-plan/draft-2b.md` lines 1–1187: Task 2.10 creates `FRONTIER_TRAJECTORY_GOLDEN` at `FRONTIER_HAULER_EXHAUST_VELOCITY = 1.0` (analytic prior). Task 2.10 doc comment says: "The calibration v_e bake is the one scheduled re-pin."
- Task 2.12 (calibration ensemble + v_e derivation) falls in lines 1188–1428, which were not readable due to file truncation at the time of initial review.
- The phase architecture note establishes the sequencing intent ("analytic prior → calibration ensemble → re-pin"), but does not enumerate the exact Steps for Task 2.12 that mirror Task 2.10's Step 1 ("run `print_golden_frontier`, paste literal").

**Risk:**
After Task 2.12 changes `FRONTIER_HAULER_EXHAUST_VELOCITY` from 1.0 to the derived calibrated value, `FRONTIER_TRAJECTORY_GOLDEN` is stale. `cargo test --workspace` will fail immediately at that commit until the golden literal is updated. If Task 2.12 neglects to include an explicit step to re-run `print_golden_frontier` and paste the new hash, the implementor must infer the step — a plausible omission under time pressure.

**Fix required:**
Task 2.12 must include an explicit numbered step: "After deriving `FRONTIER_HAULER_EXHAUST_VELOCITY = <calibrated value>` and updating the constant, run `cargo test -- print_golden_frontier --nocapture`, copy the printed literal, and replace `FRONTIER_TRAJECTORY_GOLDEN` in the same commit. The commit message must follow the single-cause golden re-pin convention with a provenance comment: `// v_e calibrated from fuel_capacity_scale=100 ensemble, k=<measured>×worst-leg-burn`."

This step should be present in the plan even if it was in the truncated section — verify Task 2.12's step list before implementation begins.


### M2 — `simulate()` return tuple shape is an implicit contract across three phases with no pinned final specification

**Plan location:** Phase 0b Task 0b.2 (adds MetaFacts), Phase 2b Tasks 2.7 and 2.10 (trophic_run.py edits), Phase 3 Task 3.4 (adds endpoint_rows).

**Evidence:**
- `/tmp/wgb-plan/assembled-plan.md` Phase 0b: "`simulate()` now returns `(samples, hashes, world, meta: MetaFacts)`"
- `/tmp/wgb-plan/draft-3.md` Task 3.4: "extend `simulate`'s return tuple with `endpoint_rows` (last position); update both call sites; match whatever `simulate` returns after phases 0b–2; the mask is appended, nothing else moves"
- Python's tuple positional destructuring: `samples, hashes, world, meta = simulate(...)` at each call site; adding a slot without updating every destructure binds the wrong name to the last variable (Python does not raise an error on extra elements if the assignment uses `*rest` unpacking, but raises `ValueError: too many values to unpack` for exact-arity destructures).

**Risk:**
The phrase "match whatever `simulate` returns after phases 0b–2" places the burden on the Task 3.4 implementor to know the correct arity. If any Phase 2b task silently touches `simulate()`'s return (e.g., adding a diagnostic tuple for the refuel verb), the arity seen by Task 3.4 may differ from the arity the plan assumes. A mismatch causes either a `ValueError` (loud, recoverable) or, if `*` unpacking is used, silent wrong-variable binding (silent, dangerous — `endpoint_rows` receives a MetaFacts object).

**Fix required:**
Phase 3 introduction or Task 3.4 preconditions must state the exact expected tuple at that point: `(samples, hashes, world, meta, endpoint_rows)` — five elements, in order, with types. Any intermediate phase that touches `simulate()`'s return must update this precondition statement. The Task 3.4 destructure in the plan should show the literal Python line `samples, hashes, world, meta, endpoint_rows = simulate(cfg, ticks)` so that "too many values to unpack" is a loud compile-equivalent error rather than a silent wrong binding.


### M3 — Band console re-judgment (spec OD-3) not confirmed as a named explicit task in the visible plan

**Plan location:** Phase 0a commit message ("console re-judgment scheduled"); spec §6 OD-3; Phase 3 Task 3.7 (unread — draft-3.md truncated at line 1138 of 1967).

**Evidence:**
- Spec §6: "haven-lurk leak fix changes the judged band — ~86% of post-refuge draws become map-wide breakouts; the legacy console band was fit on leaked data. OD-3: owner console re-judgment session scheduled after Phase 0a."
- Spec §9 landing order: "Phase 3: sweep, I1/I2 panels, headline grid, owner console session."
- Phase 0a commit message in the plan notes the re-judgment as "scheduled" without naming the specific task.
- Task 3.7 title is referenced in the Phase 3 task list as "owner console session packet" but its full content (draft-3.md lines 1139–1967) was not readable due to truncation.

**Risk:**
If Task 3.7 describes the console session packet as a documentation or artifact delivery task only — generating the packet for the owner to consume later — rather than as a task that requires owner sign-off before the rung is marked closed, the rung can be declared complete without the re-judgment ever happening. The HHI/slack constants (fit on leaked data, phase 3 Task 3.2 says "re-fit on corrected data") would be the only mitigation, but the band judgment itself (which governs what constitutes "good play" for this rung) remains unevaluated against the corrected world.

**Fix required:**
Task 3.7 (or its immediately preceding task) must include an explicit acceptance criterion: "Owner reviews the console session packet and records a judgment (APPROVED / NEEDS-WORK / DEFER) before filigree closes this rung." The filigree issue for the world-gets-big rung should reference this judgment as a required owner action, not an optional output. Verify Task 3.7's full text before implementation begins.


---

## MINOR Issues (4)

### m1 — W9 liveness window records stranding but has no threshold for "effectively dead run"

**Plan location:** Phase 1, W9 pre-registered window definition; Task 1.2.6 acceptance criteria.

**Evidence:**
- `/tmp/wgb-plan/assembled-plan.md` W9: "strandings 0–2/run band + robbed→stranded chains + contract-age liveness"; trigger for re-evaluation: "median ≥ ~2/20 haulers lost per run."
- Once a hauler strands at fuel ≤ eps, it is Idle but fails the dispatch eligibility filter permanently (fuel never replenishes without a refuel verb visit). A run where all haulers strand is a valid simulation output but produces a 0-contract-delivered world.

**Risk:**
W9 will record the stranding count but the plan offers no definition of what stranding rate makes a run's statistics meaningless for the panel analysis. A panel that averages a 0-contract run with a healthy run inflates variance and can produce misleading HHI/slack readings. This is accepted by design (PDR-0006: windows not gates), but the panel code has no filter for dead-run exclusion.

**Observation (not a fix mandate):** Consider adding a `min_contracts_delivered` filter in the panel sweep driver (Task 3.1) that tags runs with zero dispatched contracts as `DEGENERATE` in the JSONL output rather than silently including them in distribution fits.


### m2 — `FUEL_RE` optional tail groups return None on pre-refuel banked output; panel None-handling not confirmed

**Plan location:** Phase 2b Task 2.9 (FUEL anchored line refuel tail); Phase 3 panel code.

**Evidence:**
- `/tmp/wgb-plan/draft-2b.md` Task 2.9: `FUEL_RE` adds `(?:... refuels=(?P<refuels>\d+) refuel_spend_micros=(?P<refuel_spend_micros>\d+))?` as an optional tail. Named groups `refuels` and `refuel_spend_micros` return `None` when matched on pre-refuel banked FUEL output.
- Plan invokes the "version-gated parsing" principle: optional anchored lines read None on old output, never SystemExit.
- Phase 3 panel code (Tasks 3.3, 3.5) uses patterns like `int(run['fuel']['duty'])` for existing fields; the plan does not show a None-guard pattern for the new refuel fields.

**Risk:**
`int(run['fuel']['refuels'])` raises `TypeError: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'` when run against pre-refuel banked output. If the I1 or I2 panel includes a refuel column computed over a banked+live mixed ensemble, the panel raises on the first pre-refuel record.

**Fix required:**
All panel code reading refuel fields must use `int(run['fuel']['refuels'] or 0)` or an explicit `if run['fuel'] and run['fuel']['refuels'] is not None` guard. This pattern should appear as an explicit example in Task 2.9's "consumers" note and in the panel boilerplate in Task 3.3/3.5.


### m3 — Phase 3 frontier positive-control ordering is procedural-only with no automated enforcement

**Plan location:** Phase 3 Task 3.2 Step 5: "Run the hungry-roamer disease injection on `scenario_frontier` and read its restatement BEFORE any other frontier reading is taken or recorded."

**Evidence:**
- `/tmp/wgb-plan/ground-lab.md`: The positive-control injection is the instrument validation step — it confirms the panel can detect a known signal on frontier before trusting its null readings.
- The plan's ordering instruction is prose only. No test or fixture verifies that the restatement was read before any frontier HHI/slack numbers were accepted.

**Risk:**
A worker who runs frontier sweeps before the injection (e.g., to check sweep plumbing) may inadvertently anchor to frontier numbers before validation. Under PDR-0006, this is a procedural scientific integrity risk, not a code correctness risk. The fix is lightweight.

**Observation:** Add a one-line comment to the Task 3.2 output artifact (the panel notebook or script) at the top of the frontier section: `# POSITIVE CONTROL VERIFIED: <date, injection type, restatement text>`. This makes the validation evidence part of the artifact rather than an implicit precondition that may be skipped under time pressure.


### m4 — `FRONTIER_TIER_WIRING` dual-role station documentation is minimal

**Plan location:** Phase 2a Task 2.2 (`FRONTIER_TIER_WIRING` constant definition); spec §3 partitioned tier loops.

**Evidence:**
- `/tmp/wgb-plan/draft-2a.md` line ~335: `FRONTIER_TIER_WIRING = [(0,1,2,3),(3,4,5,4),(7,8,9,8)]`
- Spec §3: "Tier 1 mid: sources {3,4} → dest 5; Fuel return 5→4 (sink at 4)." Station 4 is both `src_b` and `sink` for tier 1. Station 8 is both `src_b` and `sink` for tier 2.
- The wiring invariant test asserts disjointness across tiers (not within tiers) — correct per spec, but a future writer who sees duplicate indices within a tier row may assume the test failing means they introduced a bug.

**Risk:**
No code correctness risk. Documentation clarity risk: the factory code and the wiring constant lack a single authoritative comment explaining that intra-tier `src_b == sink` is intentional (fuel return path), not a data entry error. Future writers or reviewers will be confused.

**Observation:** Add a single doc comment on `FRONTIER_TIER_WIRING`: `// src_b == sink is intentional for tiers 1 and 2 (fuel return path; station serves as both ore source and refuel sink for its tier).`


---

## Summary

- Dependency chain depth: 5 levels (simulate → phases 0b/2b/3.4 → panels → headline grid → console session)
- Potential feedback loops: 3 (ordering dead-world, stranding spiral, refuel-fail spiral)
- Historical pattern matches: 4 (implicit tuple contract, ordering-in-prose, analytic prior replacement, procedural calibration)
- Timing assumptions: 5 (eps-before-filter, transient golden, positive-control ordering, re-judgment before close, tuple shape stability)

| Severity | Count |
|---|---|
| CRITICAL | 1 |
| MAJOR | 3 |
| MINOR | 4 |


---

## Confidence Assessment

**Overall Confidence:** Moderate

| Finding | Confidence | Basis |
|---|---|---|
| C1 — eps/filter ordering | High | Grounded in source constants (events.rs:16, scenario.rs:113,126) + strict predicate (events.rs:50) + plan prerequisite note; arithmetic is exact |
| M1 — v_e golden re-pin | Moderate | Task 2.12 was in truncated file region; phase architecture note confirms intent but exact step list unverified |
| M2 — simulate() tuple | Moderate | Plan text confirms incremental additions; Task 3.4's exact wording "match whatever simulate returns" read directly; risk is structural |
| M3 — re-judgment not explicit task | Low-Moderate | Task 3.7 full content unread; finding is conditional on 3.7 not containing an explicit owner sign-off criterion |
| m1 — W9 no dead-run threshold | High | By design per PDR-0006; observation only |
| m2 — FUEL_RE None handling | Moderate | Pattern unconfirmed in panel code; risk is mechanical Python TypeError |
| m3 — positive-control ordering | High | Procedural risk confirmed by plan wording; no enforcement mechanism present |
| m4 — wiring doc | High | Code read directly; documentation gap only |


## Risk Assessment

**Implementation Risk:** Medium-High (one CRITICAL ordering issue with a clean fix; three MAJOR structural issues that require explicit verification before work begins)
**Reversibility:** Moderate (all issues are fixable before implementation; the CRITICAL issue becomes irreversible only if the misordered commit is pushed as a "working" trophic golden)

| Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|
| eps-before-filter misordering → silent dead trophic world | Critical | Possible (worker implements 1.2.x linearly) | Add dispatch-count integration test; restructure task numbering |
| simulate() tuple arity mismatch → wrong variable binding | Major | Possible (any Phase 2b edit to simulate()) | Pin exact final tuple shape in Phase 3 intro |
| v_e golden not re-pinned in Task 2.12 → cargo test fails | Major | Low (architecture note is clear; risk is step-list omission) | Verify Task 2.12 step list includes explicit re-pin instruction |
| Band re-judgment not required before rung close | Major | Low-Moderate (depends on Task 3.7 wording) | Verify Task 3.7 requires owner sign-off as acceptance criterion |
| FUEL_RE None guard missing | Minor | Possible (panel code not written yet) | Explicit guard pattern in Task 2.9 consumers note |


## Information Gaps

1. [ ] **Task 2.12 full step list** — draft-2b.md was truncated; the explicit v_e golden re-pin step in Task 2.12 was not confirmed. Verify before Phase 2b implementation begins.
2. [ ] **Task 3.7 full text** — draft-3.md was truncated at line 1138; Task 3.7 (owner console session) content unread. Verify it requires owner sign-off as an acceptance criterion.
3. [ ] **Panel code for Tasks 3.3/3.5** — not yet written; None-guard pattern for FUEL_RE refuel fields not confirmable until written. Address as a pre-commit checklist item.
4. [ ] **simulate() return tuple in Phase 2b tasks** — tasks 2.7 and 2.10 edit trophic_run.py; verify neither adds a positional return element to simulate() beyond what Phase 0b Task 0b.2 established.


## Caveats and Required Follow-ups

### Before Relying on This Analysis
- [ ] Read Task 2.12 and Task 3.7 in full before implementation begins (truncation in this review left two MAJOR findings at Moderate confidence)
- [ ] Re-run dependency analysis if any task scope changes between this review and implementation — the simulate() tuple chain is especially sensitive to scope creep in Phase 2b

### Assumptions Made
- Static code reading approximates runtime dispatch; no load test of refuel failure modes performed
- "Task 1.2.6 before Task 1.1" is the canonical misordering scenario; other orderings (e.g., Task 1.1 applied but with wrong constant value) are considered lower probability
- Python tuple ValueError is the loud failure mode for arity mismatch; `*rest` unpacking patterns would make it silent — assumed not used here based on existing panel code style

### Limitations
- This analysis does NOT cover security, test coverage, or code style
- Quantitative claims (FuelDiag duty readings, W9 threshold numbers) were not verified against running simulation output — that requires measurement
- The review did not examine the console session output format in Task 3.7 or the I2 panel (Task 3.5/3.6) — those sections were in the truncated region of draft-3.md
