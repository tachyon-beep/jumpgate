# Synthesis — world-gets-big implementation plan review

**Plan:** `/tmp/wgb-plan/assembled-plan.md` (7742 lines, 5 phases: 0a → 0b → 1 → 2 → 3)
**Spec (authoritative):** `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` (APPROVED, OD-1..7 resolved)
**Codebase verified at:** HEAD `e7e490e`
**Reviewers synthesized:** Reality, Quality, Architecture, Systems
**Synthesizer adjudication:** every contested finding re-verified against `/home/john/jumpgate` source.

---

## Verdict: FIX-THEN-SHIP

The plan is structurally sound, well-phased, spec-faithful, and PDR-0006-clean (windows recorded, never gated; the only pass/fail surfaces are determinism checks and unit tests). No REDRAFT is warranted — no architectural flaw, no one-way door without mitigation, no spec misreading in the plan body. But there are **three CRITICAL defects** that will cause a silent or hard build failure if a builder executes the plan literally, and they must be fixed before the plan is handed off. All three are local plan-text edits; none require rethinking the design.

Reviewer verdict aggregation: Architecture = no blockers (Low-Medium risk). Reality = 1 blocker. Quality = 2 blockers. Systems = 1 blocker. After dedup and adjudication: **3 distinct CRITICAL**, **3 MAJOR**, plus minors folded down.

---

## CRITICAL (must fix before handoff)

### C1 — Hallucinated fixture `one_body_one_craft_station_cfg()` (Task 1.2.6, plan line 1914; also referenced Task 1.2.1 Step 8)
**Caught by:** Reality (CRITICAL-1), Quality (MAJOR-1) — same root issue, higher severity adopted.
**Verified:** `grep -rn "one_body_one_craft_station_cfg" crates/` → zero matches. The neighbouring `BadMediaCfg` reset-error test (`half_on_media_config_is_rejected`, world.rs:1701) uses `one_body_two_stations_one_miner()` (world.rs:1648).
**Why CRITICAL not MAJOR:** a builder copying the plan's literal `let mut cfg = one_body_one_craft_station_cfg();` at line 1914 gets `E0425: cannot find function`. It is a hard compile stop, not a lookup hint, and it appears in the *implementation* code block, not prose.
**Concrete edit:** In Task 1.2.6 (line 1914) and Task 1.2.1 Step 8, replace `one_body_one_craft_station_cfg()` with `one_body_two_stations_one_miner()` (world.rs:1648). Delete the "the name above is a stand-in" hedge and pin the concrete name + `world.rs:1648`. The two-station world is required because the reset-guard test references two distinct station entities.

### C2 — `Refueled` / `ContractFailed` chronicle arms have no numbered task (handoff note only, plan line 3200)
**Caught by:** Quality (CRITICAL-2). Architecture & Reality scoped past it; Systems implicitly via observability.
**Verified:** `chronicle_subject` ends `_ => None` at trophic_run.rs:262. Task 2.6 adds its own `chronicle_subject` arm for `LurkMoved` (line 4601); Task 2.7 adds a `print_chronicle` arm for the epilogue; Task 3.5 adds a `write_gossip_log` arm for `Refueled` (line 6988, a *different* match). **No numbered task adds the `chronicle_subject` arms for `Refueled` or `ContractFailed`** — they live only in a "Cross-section handoffs (named, not built here)" note. Spec §7 explicitly requires these printer-side ("the tragedy becomes visible"). Result: both events compile, emit into the event stream, and silently vanish from `--chronicle` with no test to catch it.
**Concrete edit:** Add a numbered task **Task 2.7b** (between Task 2.7 and Task 2.8), or fold two sub-steps into Tasks 1.2.3 (`Refueled` appended) and 1.2.6 (`ContractFailed` appended). The arms:
```rust
EventKind::Refueled { craft, .. } => Some(*craft),
EventKind::ContractFailed { hauler, .. } => Some(*hauler),
```
(use the actual field name on each variant — `ContractFailed` carries `hauler`, per the spec §7 payload and plan line 3002). Anchor with a runner-level red/green: `trophic_run --scenario frontier --seed 7 --ticks 10000 --chronicle 2>&1 | grep -c "refuel"` must go non-zero. Move the line-3200 handoff note from "not built here" to a pointer at the new task.

### C3 — eps-before-filter ordering has a prose prerequisite but no test guard; misordering yields a silent dead world (Task 1.1 → Task 1.2.6)
**Caught by:** Systems (C1). Architecture saw the dependency and judged the prose sufficient (Q-series, no blocker) — this is the one substantive reviewer conflict.
**Verified (arithmetic is exact):** `FUEL_EMPTY_EPS = 1e-9` (events.rs:16); every trophic hauler spawns `fuel_mass: 1.0e-9` (scenario.rs:113, 126); the ASSIGN filter is `fuel_mass > FUEL_EMPTY_EPS` (strict). With eps still `1e-9`, `1.0e-9 > 1.0e-9 == false` → every Idle hauler fails eligibility every tick → zero contracts dispatched → trophic world runs silently to termination, and existing unit tests pass because they use isolated fixtures.
**Adjudication:** The plan *does* document the ordering twice (line 1782 and line 2763: "Do not start this task until 1.1 is merged"), so Architecture is right that it is not undocumented. But Systems is right that the failure mode is **silent** (no compile error, no failing unit test, green `cargo test`), and the cross-branch digest (Task 1.2.7) is the only thing that would catch it — at phase-1 *exit*, far downstream, where the symptom (byte-divergent stdout) does not name the cause. A silent dead-world failure caught only at phase exit clears the CRITICAL bar. Conflict resolved toward the higher severity + the cheaper guard.
**Concrete edit:** Add to Task 1.2.6 acceptance criteria a system-level dispatch-count assertion: run `scenario_trophic(7)` for M ticks with the filter active and assert ≥ N contracts dispatched (equivalently, a FuelDiag low-watermark assertion that `duty` is non-zero across haulers — the FuelDiag instrument already lands in Phase 0b). This test fails loud the instant the filter is applied before the eps re-bake, naming the cause. Optionally also restate the dependency as a hard task-tree edge, not only an intro note.

---

## MAJOR (should fix before handoff; none block the design)

### M1 — `resolve_refuels` bypasses the `permille_floor` seam + `Refueled` permille fields typed `i64` not `u32` (Task 1.2.3, plan lines 2261-2262, 2382, 2391)
**Caught by:** Quality (CRITICAL-1) and Reality (MAJOR-1) — same issue, adjudicated **MAJOR** (Reality's rating, and Quality's own analysis shows the *values* are correct).
**Verified:** Task 0b.1 establishes `pub fn permille_floor(num, den) -> u32` as "the ONE f64→integer seam for fuel instruments" with NaN/negative/zero-denominator guards (plan lines 389, 363-371). Task 1.2.3 instead writes `((fuel / cap_eff) * 1000.0).floor() as i64` inline (lines 2382, 2391) and declares the event fields `i64` (lines 2261-2262). `economy.rs` does **not** currently import `diagnostics` (verified) — so the seam is genuinely bypassed, not merely re-spelled.
**Why MAJOR not CRITICAL:** for any valid clamped tank (`0 ≤ fuel ≤ cap_eff`, finite) the inline form is arithmetically identical to `permille_floor`; the divergence only appears on NaN/negative inputs that cannot reach a clamped tank. It compiles and the pinned test passes. The cost is a second rounding implementation that can drift from the seam in a future edit, plus an `i64`/`u32` type mismatch against the TrophicSample fuel fields describing the same physical quantity.
**Concrete edit:** (1) Change `Refueled.tank_before_permille` / `tank_after_permille` from `i64` to `u32` (lines 2261-2262). (2) Add `use crate::diagnostics::permille_floor;` to economy.rs and replace both inline casts (lines 2382, 2391) with `permille_floor(fuel, cap_eff)` and `permille_floor(ships.fuel_mass[crow], cap_eff)`. (3) Retype the test literals to `u32` (`0u32`, `500u32`, `555u32`, `805u32` at lines 2114-2115, 2146-2147) — values unchanged.

### M2 — Task 1.2.6 bundles three single-causes into one commit (PLAY-C1 filter + Refuel verb + ContractFailed narration)
**Caught by:** Architecture (MAJOR-1).
**Verified:** Task 1.2.6 touches economy.rs, ingest.rs, contract.rs, pirate.rs, world.rs in one commit for three independent concerns. Spec §9 *explicitly authorizes* this bundle ("dispatch fuel-eligibility + Refueled/ContractFailed + ingest verb, gated off at lot 0"). Plan call-site list for the `settle_contract_failure` signature change (world.rs:1012, pirate.rs:236) is **complete and correct** — verified no other callers exist, closing Architecture's information gap.
**Why MAJOR not CRITICAL:** spec-authorized; correct; risk is only forensic (a bug isolated to one concern requires reverting all three).
**Concrete edit:** Lowest-friction path — keep the bundle and add a commit-message note: `"spec §9 bundle: PLAY-C1 fuel filter + Refuel verb + ContractFailed narration — three concerns co-dependent at the lot_mass==0 gate."` (Splitting into three sequential commits is acceptable but unnecessary given the spec authorization.)

### M3 — `simulate()` return-tuple shape is an implicit positional contract grown across 3 phases with no pinned final spec (Tasks 0b.2 → 2.x → 3.4)
**Caught by:** Systems (M2).
**Verified (structurally):** the tuple grows `(samples, hashes, world)` → `+meta` (0b.2) → `+endpoint_rows` (3.4); Task 3.4 says "match whatever simulate returns after phases 0b–2." Python exact-arity destructure raises `ValueError` (loud) on mismatch; `*rest` would bind silently wrong. Existing panel style uses exact-arity (lower silent-risk), but the contract is unpinned.
**Why MAJOR not CRITICAL:** the dominant failure is loud (`ValueError`), and no Phase-2b task was found to actually mutate `simulate()`'s return — so the assumed arity likely holds. But "match whatever it returns" is an unpinned contract across a 5-deep dependency chain.
**Concrete edit:** In the Phase 3 intro (or Task 3.4 preconditions), pin the exact expected tuple with types and show the literal destructure line `samples, hashes, world, meta, endpoint_rows = simulate(cfg, ticks)`, with a note that any phase touching `simulate()`'s return must update this statement. This converts a possible silent wrong-binding into a guaranteed loud failure.

---

## REJECTED findings (with one-line reasons)

- **Quality MAJOR-2 (`<PRE>` placeholder fails if copied verbatim, Task 1.2.7):** Overstated. The `<PRE>` token is explicitly defined inline at line 3139 ("Record that commit hash when phase 1 starts; here `<PRE>` stands for it"), and the *executed* digest command at line 1746 already uses a real ref (`HEAD^`). At most a MINOR clarity nit, not a warning.
- **Systems M3 (band re-judgment not a named task / no owner sign-off before close):** Task 3.7 (line 7601) IS the named console-packet task and explicitly hands the band re-judgment to the owner ("the console session itself is the owner's"). PDR-0006 *forbids* gating the rung on a metric/owner verdict — the plan correctly schedules and surfaces the session rather than gating on it. Process-tracking nicety at most; not a plan defect.
- **Systems M1 (Task 2.12 v_e re-pin step may be missing — Moderate, file was truncated):** Resolved by reading the full task. Task 2.12 (line 5543) explicitly includes "re-derive the frontier trajectory golden (the one scheduled second pin)" plus the cross-cutting golden rule (line 7720) names it. The step Systems asked for is present.
- **Reality MAJOR-2 (ground-pirate.md §5 contradicts spec §6 on doc-fix timing):** This is a defect in a *grounding extract*, not in the plan. The assembled plan correctly defers the stale-doc fix to Phase 2 per spec §6. Out of scope for the plan; at most fix the ground file's annotation.
- **Architecture MINOR-2 (marooned doc fix placement Task 2.2 vs 2.3):** Spec §6 wording genuinely ambiguous; both readings defensible; zero correctness stakes. Not worth a fix.
- **Architecture MAJOR-2 (Task 1.2.1 ~11-file blast radius):** Not a defect — forced by Rust exhaustive-destructure discipline; the correct cost of the config-fold pattern, already mitigated by the per-site default note. Reclassified as expected cost, not a finding.
- **Reality MINOR-1 / MINOR-3, Quality MINOR-1, Architecture MINOR-3/MINOR-4, Systems m1/m2/m3/m4:** Genuine but minor (missing `use` imports the builder will resolve from neighbours; an untestable-by-construction no-clobber guard the plan already acknowledges; doc-comment line offsets; None-guards on not-yet-written panel code; calibration-placeholder sentinel; intra-tier `src_b==sink` doc clarity; positive-control ordering comment). None block handoff; bundle them into a builder pre-flight checklist rather than gating on them.

---

## Confidence

**Overall: High.** Every CRITICAL and MAJOR was re-verified against HEAD `e7e490e` source (fixture absence, eps/spawn-fuel arithmetic, `chronicle_subject` catch-all, `permille_floor` non-existence at HEAD, `economy.rs` import set, `settle_contract_failure` call sites, Task 2.12/3.7 full text). The single reviewer conflict (C3: Architecture "prose suffices" vs Systems "needs a guard") was resolved on the silent-failure criterion toward the higher severity. Residual uncertainty: none of the CRITICAL/MAJOR depends on an unread region; the minors were not each independently re-verified (accepted, low stakes).
