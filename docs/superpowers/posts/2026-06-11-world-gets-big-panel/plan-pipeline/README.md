# Plan pipeline — the world-gets-big implementation plan (2026-06-11)

The 20-agent workflow that turned the APPROVED spec
(`docs/superpowers/specs/2026-06-11-world-gets-big-design.md`, OD-1..7 resolved)
into `docs/superpowers/plans/2026-06-11-world-gets-big-implementation.md`.

Pipeline (script `wgb-plan-pipeline-wf_ee7ec369-aa6.js`):

1. **Ground** — 7 citation-grade readers (`ground-*.md`), one per code beat
   (fuel edge, economy verbs, scenario/config, world stages, pirate, lab,
   events/chronicle/ephemeris). Every claim carries a file:line verified at
   HEAD `e7e490e`.
2. **Draft** — 7 phase drafters (`draft-*.md`) writing code-complete TDD tasks
   under writing-plans discipline (0a leak fix / 0b instruments / 1 eps /
   1 refuel verb / 2 factory / 2 calibration / 3 science+console).
3. **Assemble** — one agent merged the sections (36 tasks, ~7.8k lines),
   scanned for placeholders and invented golden literals (found: 1 TODO-pattern
   sentence, 0 invented literals).
4. **Review** — 4 adversarial reviewers (axiom-planning reality/quality/
   architecture/systems, `review-*.md`): 4 CRITICAL + 9 MAJOR raw findings.
5. **Synthesize** — `synthesis.md`: FIX-THEN-SHIP; deduped to 3 CRITICAL +
   3 MAJOR (6 reviewer findings rejected with reasons, each re-verified
   against source).

Main-loop fixes applied to the shipped plan (all 6 + one coverage gap the
self-review caught):

- **C1** hallucinated fixture `one_body_one_craft_station_cfg()` → the real
  `one_body_two_stations_one_miner()` (world.rs:1648).
- **C2** `Refueled`/`ContractFailed` chronicle arms had no numbered task
  (would silently vanish from `--chronicle`) → new Task 1.2.6b + a frontier
  smoke-run grep in the phase-2 section verification.
- **C3** eps-before-filter ordering was prose-only; misordering = a silently
  dead trophic world → `trophic_world_still_dispatches_under_fuel_eligibility`
  tripwire test (Task 1.2.6 Step 5b).
- **M1** `resolve_refuels` re-spelled the FLOOR seam inline with `i64` fields →
  routed through `permille_floor` (Task 0b.1's seam), fields retyped `u32`.
- **M2** Task 1.2.6's three-cause bundle documented as the spec-§9-authorized
  bundle in its commit message.
- **M3** `simulate()` return tuple pinned as an explicit contract in the
  phase-3 intro (exact-arity destructures only).
- **Self-review catch:** W9's "max non-terminal contract age" clause had no
  instrument anywhere (ContractStore carries no accept tick) → Task 2.7
  Step 6b: runner-side event-stream bookkeeping + a `LIVENESS` anchored line.

Pull-quote numbers: 20 agents, ~2.3M tokens across the run (incl. a
session-limit kill resumed from the journal with 14/20 agents cached),
36 tasks, 5 phases, exactly one `GOLDEN_CONFIG_HASH` re-pin budgeted.
