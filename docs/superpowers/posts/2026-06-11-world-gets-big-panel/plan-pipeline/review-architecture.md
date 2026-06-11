# Architecture Review — World-Gets-Big (WGB) Implementation Plan

**Plan:** `/tmp/wgb-plan/assembled-plan.md`
**Spec:** `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` (APPROVED, OD-1..7 resolved)
**Codebase HEAD:** e7e490e
**Grounding extracts:** `/tmp/wgb-plan/ground-*.md`
**Reviewer focus:** Five specific architectural questions from the requester + blast-radius and
one-way-door checks across all tasks.

---

## Architecture Review

### Blast Radius

**Files touched (estimate by phase):**

| Phase | Task | Core Files | Config/Test | Weighted Score |
|-------|------|-----------|-------------|----------------|
| 0a | Haven-lurk fix | pirate.rs (2x) | — | 2 |
| 0b.1 | permille_floor | diagnostics.rs (2x) | tests (0.5x) | 2.5 |
| 0b.2-0b.5 | META/FUEL lines, FuelDiag, TrophicSample, role splits | world.rs (2x), diagnostics.rs (2x), economy.rs (2x) | tests (0.5x×3) | ~9 |
| 1.1 | eps re-bake | events.rs (2x), world.rs (2x), economy.rs (2x), ingest.rs (2x) | fixtures (0.5x×2) | 9 |
| 1.2.1 | RefuelCfg + GOLDEN re-pin | config.rs (2x), 10 literal files (2x×10) | — | 22 |
| 1.2.2 | pending_refuel sizing | stores.rs (2x), world.rs (2x), hash.rs (2x) | — | 6 |
| 1.2.3 | resolve_refuels stage 1d2 | economy.rs (2x), contract.rs (2x) | tests (0.5x) | 4.5 |
| 1.2.4 | run_refuel_policies stage 1c3b | economy.rs (2x) | — | 2 |
| 1.2.5 | FUEL-C1 dv-rederive | economy.rs (2x) | — | 2 |
| 1.2.6 | ASSIGN filter+verb+narration | economy.rs (2x), contract.rs (2x), ingest.rs (2x), pirate.rs (2x), types.rs (2x), world.rs (2x) | — | 12 |
| 2.x | scenario_frontier | world.rs (2x), config.rs (2x), runner (2x), pirate.rs (2x) | tests (0.5x×2) | 10 |

**Heaviest single-commit weighted score:** Task 1.2.1 at ~22 (forced by Rust exhaustive-destructure discipline).

**Overall plan weighted score (summed, deduplicated by file):** ~45–50 across all phases.
**Risk level:** High (individual commits range Low→High; the plan is well-phased, so no single PR should carry the full score).

**Recommendation:** Proceed as phased. The phase boundaries (0a → 0b → 1 → 2) already constitute natural PR splits. Do not land all phases in one PR.

---

### One-Way Doors

| Change | Risk | Mitigation in Plan? |
|--------|------|---------------------|
| GOLDEN_CONFIG_HASH re-pin (Task 1.2.1) | Medium | Yes — re-pin procedure documented (printer test `print_golden_config`; never hand-computed) |
| HASH_FORMAT_VERSION | N/A | No bump required; refuel is unhashed transient. Explicitly stated. |
| FUEL_EMPTY_EPS 1e-9→1e-11 (Task 1.1) | Medium | Yes — fixture redesign arithmetic documented; cross-branch digest gated at Task 1.2.7 |
| FailureCause derive attrs (Task 1.2.6) | Low | Yes — additive only; no existing callers break |
| `settle_contract_failure` signature change (Task 1.2.6) | Low | Yes — pirate.rs call site explicitly listed with updated args |
| `scenario_frontier` as new factory (Task 2.3) | Low | Yes — dead-pricing commit precedes armed commit (Tasks 2.3→2.4) |

No unmitigated one-way doors found.

---

### Five Specific Architectural Questions

#### Q1: Does `pending_refuel` genuinely follow the `pending_upgrade` transient precedent?

**Verdict: SOUND, with one specific condition that must be verified at implementation time.**

Evidence from grounding:

- `CraftStore::empty()` at stores.rs:203+ — `pending_upgrade: Vec<Option<UpgradeKind>>` is the template.
  The plan (Task 1.2.2) adds `pending_refuel: Vec<Option<()>>` at all THREE sizing sites:
  `CraftStore::empty()`, `CraftStore::push()`, and `World::reset`'s hand-built struct literal plus
  per-craft loop. This matches the three-site discipline exactly.

- `hash.rs:303-309` — the `debug_assert` that all `pending_upgrade` slots are `None` at hash time.
  The plan extends this assert to cover `pending_refuel` in the same task.

- `pending_refuel` is NOT added to `hash.rs`'s hashed fields. Correct.

- The `lot_mass == 0` path in `resolve_refuels` must consume any stray intent (`ships.pending_refuel[crow] = None`) BEFORE the early-return. The plan's Task 1.2.3 four-leg structure is:
  1. Consume intent (`= None`)
  2. Gate on `lot_mass == 0` → early-return (AFTER consume)
  3. Gate on docked-at-vendor
  4. Apply refuel

  The plan places the early-return AFTER the consume. This is the always-consume-then-gate idiom
  from `resolve_purchases` (economy.rs:866-875). Correct.

**One condition to verify:** The plan must ensure the inertness early-return in `run_refuel_policies`
ALSO leaves `pending_refuel` clean. Since `run_refuel_policies` only *writes* `Some(())` (it does not
read pre-existing state), returning early before writing is safe — no stale `Some` can be created by
the policy stage. The invariant is preserved.

**No issues.**

---

#### Q2: Is the stage insertion (1c3b/1d2) coherent with the documented stage order?

**Verdict: SOUND.**

Evidence from `ground-world-stages.md` and world.rs:750-800:

- Stage 1c3 (`run_purchase_policies`) ends at world.rs:766.
- Stage 1d (`resolve_purchases`) spans world.rs:776-786.
- Body snapshot (`next.bodies`) is at world.rs:789.

The plan inserts:
- `run_refuel_policies` at stage 1c3b → after line 766, before line 776. Correct slot.
- `resolve_refuels` at stage 1d2 → after line 786, before line 789. Correct slot.

The ordering principle is: policy stages (1c*) collect intents; resolve stages (1d*) act on them.
Inserting refuel between existing stages in both bands is coherent with this principle.

The body snapshot at 789 occurs AFTER `resolve_refuels`, meaning fuel-mass changes from refueling
are visible in `next.bodies` as intended by the spec (§5: "fuel_mass write must precede body
snapshot so physics stage 2 sees the correct mass").

`resolve_purchases` (786) passes `&mut self.events` (verified at world.rs:785). The same borrow
pattern works for `resolve_refuels` — no Rust lifetime issue.

**No issues.**

---

#### Q3: Does `scenario_frontier` stay a pure factory (no engine special-cases leaking in)?

**Verdict: SOUND. The two-commit discipline (dead→armed) enforces factory purity structurally.**

Evidence:

- Task 2.3 builds `scenario_frontier` with `PriceCfg { base_micros: [0,0], cap: [0,0] }` (prices
  structurally off — `cap[r] == 0` is the existing switch in `update_prices`) and
  `RefuelCfg::default()` (`lot_mass: 0.0` → inertness gate fires, no refuel behavior).

- Task 2.4 is a separate commit that sets `RefuelCfg { lot_mass: ..., corp_index: ... }` and
  real `PriceCfg` values. The engine sees both scenarios via the same code paths — no
  `if scenario == Frontier` branches in the engine.

- The `--scenario` flag (Task 2.5) is in the runner layer, not in the factory or engine.
  The factory is called once at scenario setup; the flag only controls which factory is called.

- `FRONTIER_ORBIT_AU` and `FRONTIER_TIER_WIRING` are named constants in the factory file.
  They do not touch the engine's core structs.

- `ephemeris_window` concern: the trophic scenario uses `100_000`. The frontier scenario should set
  this high enough or use the same value. The plan (Task 2.3) inherits from `scenario_trophic`'s
  defaults; Task 2.5 adds an explicit guard that warns if the runner's step count exceeds the
  ephemeris window. This is belt-and-suspenders but correct.

**No issues.**

---

#### Q4: Is the trophic-inertness gate (`lot_mass == 0`) structurally sound?

**Verdict: SOUND. Both stages gate correctly, and the consume-before-gate discipline is preserved.**

The gate fires in two places:

1. `run_refuel_policies` (Task 1.2.4): If `cfg.refuel.lot_mass == 0.0`, return early *without*
   writing any `Some(())` to `pending_refuel`. No stale intent is created. Safe.

2. `resolve_refuels` (Task 1.2.3): For each craft, consume (`= None`) then check `lot_mass == 0.0`
   → `continue`. Any manually-ingested `Some(())` from the Command layer is cleared before the
   gate check. Safe.

The "lot_mass == 0 means no refuel" contract is enforced at both the intent-generation stage and
the resolution stage. A world configured with `lot_mass: 0.0` (the trophic default) will never
apply refuel effects AND will never accumulate un-consumed intents at hash time.

The assertion at `hash.rs:306-309` (extended in Task 1.2.2) will catch any regression where a
`Some(())` survives to a hash point.

**No issues.**

---

#### Q5: Single-cause golden commits respected? Any task coupling two causes into one commit?

**Verdict: TWO ISSUES FOUND — one MAJOR, one MINOR.**

**MAJOR-1: Task 1.2.6 bundles three distinct causes into one commit.**

The plan's Task 1.2.6 touches 6 files in a single commit:

| File | Cause |
|------|-------|
| `economy.rs` | PLAY-C1: `fuel_mass > FUEL_EMPTY_EPS` filter in ASSIGN |
| `ingest.rs` | New `CommandKind::Refuel` verb |
| `contract.rs` | `#[derive(Clone, Copy, Debug, PartialEq)]` on `FailureCause` |
| `economy.rs` + `pirate.rs` | `settle_contract_failure` gains `tick, events` params + `ContractFailed` narration |
| `types.rs` | Supporting type change for Refuel verb |
| `world.rs` | Stage wiring for new verb |

Three logically independent concerns:
- (A) ASSIGN dispatch eligibility (fuel filter)
- (B) `CommandKind::Refuel` verb (the ingest surface)
- (C) `ContractFailed` narration (failure observability, involves signature change to `settle_contract_failure`)

The spec §9 explicitly bundles these: *"dispatch fuel-eligibility + Refueled/ContractFailed + ingest verb, gated off at lot 0"*. This is a known and intentional spec-level bundling, not an oversight by the plan author. However, it violates the single-cause commit principle that has governed this project (precedent: commits 88a5d85 and 1795c57).

The practical risk: a bug isolated to the `ContractFailed` narration signature change requires reverting the fuel-eligibility filter AND the Refuel verb simultaneously.

**Concrete fix options (pick one):**
- Split into three sequential commits:
  1. `CommandKind::Refuel` verb + ingest path (B)
  2. ASSIGN fuel-eligibility filter (A)
  3. `settle_contract_failure` signature + `ContractFailed` event + `FailureCause` derives (C)
- OR retain as-is but document the deliberate spec override at the commit message level
  (e.g., "spec §9 bundle: three concerns co-dependent at the lot_mass==0 gate") so future
  forensics understand the bundling was intentional.

The second option is the lower-friction path given the spec already authorizes it.

**Plan location:** Task 1.2.6, "PLAY-C1 + Refuel verb + ContractFailed narration" section.

---

**MINOR-1: Task 1.2.1 has a blast radius of ~11 files but is structurally unavoidable.**

The RefuelCfg fold + `GOLDEN_CONFIG_HASH` re-pin touches config.rs plus ~10 `RunConfig` literal
files. This is forced by Rust's exhaustive-struct-destructure discipline: every `RunConfig {..}`
literal site must add the new field, and there is no way to add a config field without touching
all literal sites.

This is not a design flaw — it is the correct cost of the discipline. It is flagged here only
because the blast radius (weighted ~22) is unusually high for a single commit and requires
careful review that each literal site gets the correct default (not a value accidentally copied
from a neighboring field).

**Concrete mitigation:** The plan should specify the exact default value for `RefuelCfg` at each
literal site (not just at the struct's `Default` impl). A one-line comment in the task description
pointing implementers to check `RefuelCfg::default()` vs per-scenario overrides is sufficient.

**Plan location:** Task 1.2.1, "RefuelCfg fold + GOLDEN re-pin" section.

---

### Complexity Assessment

**Tracer bullet opportunity:** No. Phase 0a (haven-lurk fix) is the immediate integration test —
it validates the trophic path end-to-end before Phase 1 adds new behavior. The phase structure
already implements the tracer-bullet discipline: unhashed diagnostics (Phase 0b) before new
behavior (Phase 1) before new scenario (Phase 2).

**Custom code vs libraries:** No reinvented wheels found. `permille_floor` (Task 0b.1) is a
project-specific integer conversion with defined rounding semantics (FLOOR, not round). No
library provides this in the way the project needs it. Correct to implement inline.

**"Why Now?" flags:** None. Every task in the plan has a direct dependency on either:
- The trophic-inertness gate (lot_mass==0 as the off-switch is required for the factory to land
  safely), or
- The eps re-bake (required before refuel can be tested — the old eps makes fuel-empty
  arithmetically unfireable on band tanks), or
- The frontier scenario (the stated goal of the rung).

No premature abstractions, no future-proofing, no scope creep detected.

---

### Pattern Alignment

| Pattern | Plan Approach | Project Standard | Aligned? |
|---------|---------------|------------------|----------|
| Transient field sizing | Three sites: empty/push/reset | `pending_upgrade` precedent | Yes |
| Hash exclusion | Not in hash.rs field list; debug_assert all-None | `pending_upgrade`/`media_diag` precedent | Yes |
| Always-consume-then-gate | consume first, then gate | `resolve_purchases` (economy.rs:866-875) | Yes |
| Config field addition | RunConfig exhaustive literal update + GOLDEN re-pin | MediaCfg fold precedent | Yes |
| Stage naming | 1c3b / 1d2 (letter-digit suffix) | Existing 1b2, 1c2, 1c3 | Yes |
| Single-cause commits | Task 1.2.6 bundles three causes | Commits 88a5d85, 1795c57 | Partial (spec override) |
| Diagnostics unhashed | FuelDiag, permille_floor all unhashed | media_diag, assign_diag | Yes |
| Factory purity | Dead-then-armed two-commit sequence | scenario_trophic precedent | Yes |
| GOLDEN re-pin procedure | printer test only, never hand-computed | Documented in spec §9 | Yes |

---

### Additional Specific Findings

**MINOR-2: Marooned doc fix placement ambiguity (Task 2.2 vs spec §6)**

Spec §6 says: *"Reach 0.6 set EXPLICITLY in both factories; the stale 'nearest station' marooned
doc fixed in the same commit."*

The plan puts the doc fix in Task 2.2 (trophic factory explicit-reach commit) rather than
waiting for Task 2.3 (frontier factory explicit-reach commit). A strict reading of "both
factories" suggests the doc fix should land after BOTH factories have explicit reach — i.e., in
Task 2.3 or as a separate Task 2.2b.

The plan's interpretation (fix doc with first explicit-reach commit) is defensible: the doc is
stale regardless of which factory you're looking at. But if the spec author intended the fix to
be proof that both factories have been audited for this pattern, landing it in 2.3 would be
more consistent with the intent.

**Concrete fix:** No change required if the interpretation is confirmed. If the spec author
intended "fix in the commit that sets reach in the second factory," move the doc fix to Task 2.3.

**Plan location:** Task 2.2, "Explicit reach in trophic factory" note.

---

**MINOR-3: `FRONTIER_HAULER_EXHAUST_VELOCITY` calibration has no enforcement gate**

The plan sets `FRONTIER_HAULER_EXHAUST_VELOCITY = 1.0` as a placeholder in Task 2.3 and defers
calibration to Phase 2 Task 2.6 (the `fuel_capacity_scale=100` ensemble). There is no
compile-time or test-time gate that prevents committing Task 2.3 through 2.5 with the
placeholder value and then forgetting Task 2.6.

This is not a structural bug — the inertness gate ensures refuel is off until Task 2.4 arms it,
so the calibration only matters post-arming. But the risk is that Task 2.6 is treated as
optional and the golden is re-pinned against an uncalibrated v_e.

**Concrete mitigation:** Add a `#[cfg(debug_assertions)] compile_error!` or a failing test that
asserts `FRONTIER_HAULER_EXHAUST_VELOCITY != 1.0` (a sentinel value check) until Task 2.6
replaces the placeholder. Remove the assertion in Task 2.6.

Alternatively: document the calibration as a pre-condition for the Task 2.4 armed commit
(not Task 2.6), so the golden re-pin in Task 2.4 already reflects the calibrated value.

**Plan location:** Task 2.3, `FRONTIER_HAULER_EXHAUST_VELOCITY` assignment.

---

**MINOR-4: `FRONTIER_TIER_WIRING` intra-tier sink==source overlap undocumented in the invariant test**

`FRONTIER_TIER_WIRING = [(0,1,2,3), (3,4,5,4), (7,8,9,8)]`

Station 4 is both `src_b` (tier-1) and `fuel_sink` (tier-1). Station 8 is both `src_b` (tier-2)
and `fuel_sink` (tier-2). This is intentional per spec §3 ("Fuel return 5→4", "9→8").

The plan's wiring invariant test (Task 2.1) asserts inter-tier disjointness of `dest` and
`fuel_sink` but NOT intra-tier uniqueness (which would fail by design). A maintainer reading the
test in isolation cannot distinguish "we didn't test intra-tier because it's impossible" from
"we didn't test intra-tier because we forgot."

**Concrete mitigation:** Add a comment to the invariant test: *"Intra-tier sink==source_b overlap
is intentional (see spec §3: fuel return station); this test only asserts inter-tier dest/sink
disjointness."*

**Plan location:** Task 2.1, wiring invariant test description.

---

## Summary

| Severity | Count | Issues |
|----------|-------|--------|
| CRITICAL | 0 | — |
| MAJOR | 2 | Task 1.2.6 triple-cause bundle; Task 1.2.1 11-file blast radius |
| MINOR | 4 | Marooned doc placement ambiguity; unchecked calibration placeholder; wiring test undocumented overlap; (Task 1.2.1 literal-site default note) |

---

## Blocking Issues

None. No one-way doors without mitigation. No structural soundness failures.

The MAJOR findings are:
- **MAJOR-1** (Task 1.2.6): Acknowledged spec-authorized bundle; risk is forensic difficulty, not
  correctness. Not blocking if the commit message documents the bundle.
- **MAJOR-2** (Task 1.2.1): Forced by compile discipline; not avoidable. Not blocking.

---

## Recommendations

1. **Task 1.2.6 commit message** (MAJOR-1): Add explicit note that the bundle is spec §9
   authorized: *"spec §9 bundle: PLAY-C1 fuel filter + Refuel verb + ContractFailed narration
   — three concerns gated off together at lot_mass==0."* This preserves forensic clarity without
   splitting the commit.

2. **Task 1.2.1 literal sites** (MAJOR-2): Add a line to the task description: *"Each literal
   site receives `refuel: RefuelCfg::default()`; verify no site accidentally copies a non-default
   value from a neighbor."*

3. **Task 2.3 calibration sentinel** (MINOR-3): Consider either (a) a failing sentinel assertion
   on the placeholder value, or (b) moving calibration to be a pre-condition of Task 2.4 (armed
   commit) rather than a separate Task 2.6.

4. **Task 2.1 wiring test comment** (MINOR-4): One-line comment explaining the intentional
   intra-tier sink==source overlap.

5. **Task 2.2 doc fix placement** (MINOR-2): Confirm with spec author whether "same commit"
   means "first explicit-reach commit" or "second (frontier) explicit-reach commit." Low stakes
   either way.

---

## Confidence Assessment

**Overall Confidence: High**

| Finding | Confidence | Basis |
|---------|------------|-------|
| Q1: pending_refuel transient precedent sound | High | Three sizing sites confirmed in stores.rs; assert location confirmed in hash.rs:303-309; consume-before-gate verified against economy.rs:866-875 |
| Q2: Stage insertion 1c3b/1d2 coherent | High | Stage sequence verified against world.rs:750-800 and ground-world-stages.md; slot boundaries confirmed at lines 766, 786, 789 |
| Q3: scenario_frontier pure factory | High | Dead-then-armed two-commit structure verified; no engine special-cases in plan; runner flag in Task 2.5 confirmed separate from factory |
| Q4: Trophic-inertness gate sound | High | Both stages gate at lot_mass==0; consume-before-gate discipline confirmed for resolve path; hash assert will catch regressions |
| Q5: Single-cause violations | High | Task 1.2.6 bundle is explicit in plan text; three-file causes enumerated; spec §9 authorization confirmed |
| Blast-radius classification | Moderate | Weighted by heuristic table; Task 1.2.1 literal-site count estimated from pattern (not manually counted across all 10 files) |
| Marooned doc placement (MINOR-2) | Moderate | Spec §6 wording is genuinely ambiguous; both interpretations are defensible |
| Calibration enforcement gap (MINOR-3) | High | Task 2.6 has no gate; Task 2.3 uses literal placeholder value |

---

## Risk Assessment

**Implementation Risk: Low-Medium**
**Reversibility: Easy** (all changes are additive or reconfiguration; no data migrations; GOLDEN re-pin is the highest-stakes irreversibility and is well-documented)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Task 1.2.6 bundle makes a bug require reverting all three concerns | Medium | Low | Spec authorizes bundle; document in commit message |
| Task 1.2.1 literal site gets wrong default value | Medium | Low | Code review; Rust exhaustive-destructure will catch missing fields but not wrong values |
| Calibration placeholder v_e escapes into golden | Medium | Medium | Sentinel assertion or move calibration before armed commit |
| FUEL-C1 dv-rederive uses wrong fuel_mass (pre- vs post-write) | High | Very Low | Plan explicitly says post-write; verified that tank write precedes tsiolkovsky call in resolve_refuels |
| Cross-branch digest (Task 1.2.7) passes on wrong baseline | Medium | Low | Plan specifies 2000-tick digest; baseline is the current trophic band; bit-identical stdout is the gate |

---

## Information Gaps

1. [ ] **Exact count of RunConfig literal sites** — estimated at ~10 from the MediaCfg fold
   precedent. The actual count could be verified with `grep -rn 'RunConfig {' src/` but was not
   exhaustively confirmed. If the count is higher, Task 1.2.1 blast radius is underestimated.

2. [ ] **`settle_contract_failure` call sites beyond pirate.rs:236** — the plan lists pirate.rs
   as the only call site. If there are other callers, the signature change in Task 1.2.6 touches
   more files than identified. A `grep -rn 'settle_contract_failure'` should be run before
   implementation.

3. [ ] **`fuel_capacity_scale` knob implementation** — the plan references this knob in Task 2.6
   calibration but it does not exist yet (confirmed: not in `apply_knob` as of HEAD). It must
   be added before Task 2.6 is runnable. The plan does not include a task for adding this knob.

---

## Caveats & Required Follow-ups

### Before Relying on This Analysis
- [ ] Confirm `settle_contract_failure` has no call sites beyond pirate.rs:236
      (`grep -rn 'settle_contract_failure' /home/john/jumpgate/crates/`)
- [ ] Confirm `fuel_capacity_scale` knob is planned somewhere (possibly an implicit sub-step
      of Task 2.6 not described in the plan)
- [ ] Confirm spec author's intent on marooned doc fix placement (Task 2.2 vs 2.3)

### Assumptions Made
- `RunConfig` literal site count estimated from MediaCfg precedent pattern
- Borrow safety of `world.rs` at stage 1d2 inferred from existing `resolve_purchases` call at line 785
- `FRONTIER_HAULER_EXHAUST_VELOCITY = 1.0` is confirmed as a placeholder, not a final value

### Scope Boundaries
- This review covers: architecture patterns, blast radius, one-way doors, complexity management,
  pattern alignment, single-cause commit discipline.
- This review does NOT cover: symbol existence verification, test coverage adequacy, security
  patterns, systemic effects across other rung interactions.
