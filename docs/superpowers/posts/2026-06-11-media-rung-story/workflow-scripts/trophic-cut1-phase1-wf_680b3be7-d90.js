export const meta = {
  name: 'trophic-cut1-phase1',
  description: 'Build Phase 1 of Trophic Cut 1: foundations (hash format v3) + the aliveness discriminator',
  phases: [
    { title: 'Foundations' },
    { title: 'Diagnostics' },
    { title: 'Verify' },
  ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-trophic-cut-1-implementation.md'
const SPEC = 'docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md'

const IMPL_SCHEMA = {
  type: 'object',
  properties: {
    files_changed: { type: 'array', items: { type: 'string' } },
    tests_added: { type: 'array', items: { type: 'string' } },
    commands_run: { type: 'array', items: { type: 'string' } },
    build_passed: { type: 'boolean' },
    tests_passed: { type: 'boolean' },
    golden_changes: { type: 'string' },
    blockers: { type: 'array', items: { type: 'string' } },
    notes: { type: 'string' },
  },
  required: ['build_passed', 'tests_passed', 'golden_changes', 'notes'],
}

const VERIFY_SCHEMA = {
  type: 'object',
  properties: {
    test_pass: { type: 'boolean' },
    clippy_clean: { type: 'boolean' },
    hash_format_version: { type: 'string' },
    goldens_changed: { type: 'array', items: { type: 'string' } },
    failures: { type: 'string' },
    notes: { type: 'string' },
  },
  required: ['test_pass', 'clippy_clean', 'hash_format_version', 'failures'],
}

phase('Foundations')
const t1 = await agent(
  `Implement Task 1 of the plan at ${PLAN} (read the plan AND the spec at ${SPEC} first, especially the "Determinism / golden protocol" section).

Working dir is /home/john/jumpgate; the crate is jumpgate-core. Use strict TDD: write failing tests, implement minimally, run cargo until green.

Task 1 lands the trophic foundations into the canonical hash:
- CraftRole::Pirate (appended rank() = 2; Idle=0, Hauler=1 unchanged)
- CraftStore columns: risk_appetite: Vec<i32> (default 0) and pirate: Vec<Option<PirateState>> where PirateState { food_micros: i64, notoriety: u32, lie_low_until: Tick }. Default/push these in EVERY constructor and push path.
- RngStream::Piracy appended with a new fixed SALT_PIRACY constant (do NOT reorder existing salts)
- EventKind additive variants: Robbed { pirate, hauler, contract, value_micros }, DrivenOff { pirate, hauler }, HaulerKilled { pirate, hauler }, PirateLieLow { pirate, until }, PirateLeft { pirate }, PirateSpawned { pirate }
- RouteKey(pub StationId, pub StationId) in types.rs (Copy + Eq + Hash + Ord)
- hash.rs: fold risk_appetite and pirate (self-delimiting: tag 0 for None, tag 1 + fields for Some) into the craft state hash; bump HASH_FORMAT_VERSION from 2 to 3.

CRITICAL GOLDEN PROTOCOL — follow exactly:
- Adding state to the hash WILL move the determinism goldens. This is ONE named cause (format v3 adds trophic state). Bump HASH_FORMAT_VERSION to 3, then run cargo test -p jumpgate-core. The golden assertions will FAIL printing the ACTUAL computed hashes. Re-pin the golden constants to those ACTUAL printed values. NEVER type a hash from imagination — only paste values that cargo test actually printed. Re-run until green.
- Confirm record_then_replay_is_bit_identical still passes (v3 on both sides).
- Make NO behavioural change to existing scenarios — only the format bump moves them.

Run cargo test -p jumpgate-core and cargo clippy --all-targets -p jumpgate-core until both are clean. Report exactly which goldens moved and to what, in golden_changes. Your final message IS the structured report.`,
  { label: 'T1:foundations', phase: 'Foundations', schema: IMPL_SCHEMA },
)

phase('Diagnostics')
const t2 = await agent(
  `Implement Task 2 of the plan at ${PLAN} (read the plan AND spec ${SPEC} §3 first). Task 1 is already done (CraftRole::Pirate, RouteKey in types.rs, etc. exist). Working dir /home/john/jumpgate, crate jumpgate-core. Strict TDD.

Create crates/jumpgate-core/src/diagnostics.rs (and add the mod to lib.rs) with:
- TrophicSample { tick, active_pirates: u32, lying_low: u32, active_hauler_density: u32, robs_this_tick: u32, per_route_risk: Vec<(RouteKey, i64)>, avg_cargo_in_flight_micros: i64 }
- HaulerLedger { craft: CraftId, risk_appetite: i32, wealth_micros: i64, deliveries: u32, robs_suffered: u32, alive: bool }
- Verdict { BarMet, NoCycle, RiskEqualized, DecisionNotTranslating }
- Diagnosis { cycled: bool, risk_heterogeneous: bool, outcomes_disperse: bool, verdict: Verdict }
- fn classify(series: &[TrophicSample], ledger: &[HaulerLedger]) -> Diagnosis:
  * cycled = anti-phase amplitude of active_pirates vs active_hauler_density above a noise floor, neither pinned at 0 nor saturated (use integer/fixed-point math; no floats in the hashable sim path, but classify() is a pure analysis fn so f64 is acceptable here — keep it deterministic and pure)
  * risk_heterogeneous = cross-route variance of per_route_risk AND its temporal autocorrelation both above named-const thresholds
  * outcomes_disperse = variance of wealth_micros above a floor AND association with risk_appetite (rank correlation)
  * verdict per the spec §3 matrix
- Put all thresholds as named const at the top (diagnostic thresholds, NOT acceptance gates).

This is the KEYSTONE instrument — test it hard with the four corners:
1. anti-phase oscillating series + dispersed appetite-tracking ledger -> BarMet
2. flat/equilibrium series -> NoCycle
3. oscillating but uniform per-route risk (variance ~ 0) -> RiskEqualized
4. oscillating, heterogeneous persistent risk, but flat ledger -> DecisionNotTranslating
Plus a purity test (same input -> same output).

Run cargo test -p jumpgate-core and cargo clippy --all-targets -p jumpgate-core until clean. Final message IS the structured report.`,
  { label: 'T2:diagnostics', phase: 'Diagnostics', schema: IMPL_SCHEMA },
)

phase('Verify')
const v = await agent(
  `Independently verify the current working tree of /home/john/jumpgate. Do NOT edit code; only run commands and report facts.
Run:
1. cargo test -p jumpgate-core 2>&1 | tail -40  -> set test_pass true only if it reports 0 failures
2. cargo clippy --all-targets -p jumpgate-core 2>&1 | tail -20  -> clippy_clean true only if no warnings/errors
3. grep -rn "HASH_FORMAT_VERSION" crates/jumpgate-core/src/  -> report the value in hash_format_version
4. git diff --stat  -> list which golden/hash constants and files changed in goldens_changed
Report the actual numbers. If anything failed, put the real failing output in failures. Final message IS the structured report.`,
  { label: 'verify', phase: 'Verify', schema: VERIFY_SCHEMA },
)

return { t1, t2, verify: v }
