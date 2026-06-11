export const meta = {
  name: 'plan-A-effectivemods-seam',
  description: 'Execute Person+Ship Plan A (EffectiveMods seam) TDD, then two independent adversarial determinism verifiers',
  phases: [
    { title: 'Implement', detail: 'one sequential implementer, 4 TDD tasks, commit-per-task on green' },
    { title: 'Verify', detail: 'determinism-reviewer + adversarial skeptic re-run every gate from scratch' },
  ],
}

const IMPL_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['tasks_completed','files_changed','commits','golden_moved','hash_format_version','gate_results','effective_params_signature','call_sites_migrated','blocked','blocker_detail','notes'],
  properties: {
    tasks_completed: { type: 'array', items: { type: 'integer' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    commits: { type: 'array', items: { type: 'object', additionalProperties: false, required: ['task','sha','message'], properties: { task: {type:'integer'}, sha: {type:'string'}, message: {type:'string'} } } },
    golden_moved: { type: 'boolean', description: 'MUST be false; true means a hashed field was touched' },
    hash_format_version: { type: 'integer', description: 'MUST be 1' },
    gate_results: { type: 'object', additionalProperties: false, required: ['lib_tests','integration_tests','clippy'], properties: { lib_tests: {type:'string'}, integration_tests: {type:'string'}, clippy: {type:'string'} } },
    effective_params_signature: { type: 'string' },
    call_sites_migrated: { type: 'array', items: { type: 'string' } },
    blocked: { type: 'boolean' },
    blocker_detail: { type: 'string' },
    notes: { type: 'string' },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['verdict','golden_unchanged','hash_format_version_is_1','no_hashed_field_touched','plain_multiply_no_fma','mods_not_in_state_hash','all_call_sites_migrated','lib_test_result','integration_test_result','clippy_result','refutation_attempts','evidence'],
  properties: {
    verdict: { type: 'string', enum: ['CONFIRM','REFUTE'] },
    golden_unchanged: { type: 'boolean' },
    hash_format_version_is_1: { type: 'boolean' },
    no_hashed_field_touched: { type: 'boolean' },
    plain_multiply_no_fma: { type: 'boolean' },
    mods_not_in_state_hash: { type: 'boolean' },
    all_call_sites_migrated: { type: 'boolean' },
    lib_test_result: { type: 'string' },
    integration_test_result: { type: 'string' },
    clippy_result: { type: 'string' },
    refutation_attempts: { type: 'string' },
    evidence: { type: 'string' },
  },
}

const IMPL_PROMPT = `You are executing **Person+Ship Plan A — the EffectiveMods seam** in the jumpgate Rust workspace (crate \`jumpgate-core\`, branch \`jumpgate-v1-design\`, cwd /home/john/jumpgate). This is the single most irreversible seam in the program; it MUST be behaviour-preserving and BIT-IDENTICAL.

READ FIRST and follow EXACTLY, task by task: \`docs/superpowers/plans/2026-06-08-jumpgate-person-ship-plan-A-crewmods-seam.md\`. Tasks 1-4, each strict TDD: write the failing test → run it to confirm it FAILS → write the minimal implementation (the plan gives complete code verbatim) → run to confirm it PASSES.

CURRENT REALITY (the plan's line numbers are STALE — rely on the COMPILER to find every call site, never line numbers):
- \`effective_params\` is currently \`pub fn effective_params(spec: &BaseSpec) -> Effective\` at stores.rs:29 (single-arg, not yet migrated).
- ALL call sites that must migrate to the 2-arg form (each is a compile error until fixed): stores.rs (\`craft_fuel_capacity\` + the \`effective_equals_base_in_v1\` test), world.rs (there are FOUR calls: ~216, ~271, ~398, ~460 — step burn, second step path, project capacity, StateView capacity), ingest.rs (~168 Δv budget), ship.rs (~59 test fixture \`eff_fixture\`), autopilot.rs (~102 test). Fix EVERY compile error the signature change produces — do not stop at the three the plan's prose lists.
- lib.rs:59 re-export is currently: \`pub use stores::{BodyStore, CraftStore, Effective, NavState, effective_params};\` — add \`EffectiveMods\`.
- \`World::reset\` returns \`Result\` (guidance already landed): the new world.rs test \`identity_mods_preserve_trajectory\` must use \`let (mut world, _) = World::reset(cfg).expect("resolvable cfg");\`.
- hash.rs: \`HASH_FORMAT_VERSION = 1\`; goldens \`GOLDEN_ZERO_STATE_HASH = 0xf0dd_a1ba_f433_3735\` and the cfg-with-craft golden \`0x532d_07bf_95a2_abc5\`.

HARD INVARIANTS — violating ANY means STOP immediately, do NOT commit that task, set blocked=true and report detail. Do NOT work around:
1. Do NOT bump HASH_FORMAT_VERSION (stays 1).
2. Do NOT edit or re-baseline ANY golden literal. If a golden TEST fails, a hashed field was touched by mistake — STOP and report which; never "fix" by re-pinning.
3. \`effective_params\` applies \`thrust_factor\` with a PLAIN \`*\` (single rounding) — NO \`mul_add\` / NO FMA.
4. \`mods\` must NOT be folded into \`state_hash\` / HASH_FIELD_ORDER — it is derived/unhashed, exactly like \`prev_fuel\`.
5. \`EffectiveMods\` is the GENERAL modifier bundle exactly as the plan specifies (thrust_factor + reserved room for wear/component); do NOT rename it crew-only.

GATES (run yourself; capture RAW output):
- per task: that task's named tests (red then green).
- on Task 4: \`cargo test -p jumpgate-core\` (full lib), \`cargo test -p jumpgate-core --test replay_equivalence --test physics_sanity\`, \`cargo test -p jumpgate-core golden\`, \`cargo clippy --all-targets -- -D warnings\` (NOT --lib — binary crate, --lib is a no-op).

COMMIT per task using the plan's exact commit messages, appending the trailer:
  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
— but ONLY when that task's gates are green AND no golden moved. If anything fails, STOP, do not commit, report blocked.

A separate verification phase will INDEPENDENTLY re-run every gate after you. Do NOT fabricate results — report exactly what the commands printed (paste the pass/fail summary lines). Return the structured result (commits with real shas from \`git rev-parse HEAD\`, raw gate summaries, golden_moved=false, hash_format_version=1).`

const DET_PROMPT = `Independently verify that **Person+Ship Plan A (EffectiveMods seam)** — just implemented on branch \`jumpgate-v1-design\`, crate \`jumpgate-core\`, cwd /home/john/jumpgate — is behaviour-preserving and determinism-NEUTRAL. Do NOT trust the implementer; re-derive everything from the repo.

The claim under test: threading an identity-valued \`EffectiveMods\` through \`effective_params\` changed NOTHING observable — both state goldens byte-unchanged, HASH_FORMAT_VERSION still 1, full suite green. Plan: \`docs/superpowers/plans/2026-06-08-jumpgate-person-ship-plan-A-crewmods-seam.md\`.

Verify WITH EVIDENCE (run the commands):
1. \`git log --oneline -6\` then \`git show\` / \`git diff\` the Plan-A commits — inspect for ANY change to a hashed field, HASH_FIELD_ORDER entry, or HASH_FORMAT_VERSION.
2. grep hash.rs: confirm \`HASH_FORMAT_VERSION\` == 1 and the two golden literals are EXACTLY \`0xf0dd_a1ba_f433_3735\` and \`0x532d_07bf_95a2_abc5\` (unedited).
3. Confirm \`effective_params\` multiplies \`thrust_factor\` with a plain \`*\` (no \`mul_add\`/FMA), and \`mods\` does NOT appear in \`state_hash\` / HASH_FIELD_ORDER in hash.rs.
4. Re-run from scratch and capture raw counts: \`cargo test -p jumpgate-core\`; \`cargo test -p jumpgate-core --test replay_equivalence --test physics_sanity\`; \`cargo test -p jumpgate-core golden\`; \`cargo clippy --all-targets -- -D warnings\`.
5. \`grep -rn 'effective_params(' crates/\` — confirm EVERY call uses the 2-arg form.
Actively try to REFUTE "bit-identical / determinism-neutral". Set verdict=REFUTE if any check fails. Return the structured verdict.`

const SKEPTIC_PROMPT = `Adversarial re-verification of **Plan A (EffectiveMods seam)** on branch \`jumpgate-v1-design\`, crate \`jumpgate-core\`, cwd /home/john/jumpgate. ASSUME the implementer may have fabricated its green gates; trust only output you produce yourself.

1. Re-run ALL gates and capture raw output: \`cargo test -p jumpgate-core\` (record the "N passed; M failed" line); \`cargo test -p jumpgate-core --test replay_equivalence --test physics_sanity\`; \`cargo clippy --all-targets -- -D warnings\` (paste the tail).
2. Confirm the 3 new tests exist AND pass: \`effective_mods_identity_is_unit\`, \`effective_scales_with_thrust_factor\` (stores.rs), \`identity_mods_preserve_trajectory\` (world.rs); and that \`effective_equals_base_in_v1\` was updated to the 2-arg call.
3. \`grep -rn 'effective_params(' crates/\` — confirm NO single-arg call remains (every call has 2 args).
4. Confirm clippy is genuinely clean (run it; paste the last lines).
5. \`git status\` — confirm the working tree is clean post-commits, and \`git log --oneline -5\` shows the Plan-A commits.
Report verdict=CONFIRM only if you PERSONALLY saw green on every gate. Return the structured verdict.`

phase('Implement')
const impl = await agent(IMPL_PROMPT, { label: 'plan-A-impl', schema: IMPL_SCHEMA })

if (!impl || impl.blocked || impl.golden_moved || impl.hash_format_version !== 1) {
  log(`Implementer halted or tripped an invariant (blocked=${impl?.blocked} golden_moved=${impl?.golden_moved} hfv=${impl?.hash_format_version}); skipping verification for human review.`)
  return { impl, det: null, skeptic: null, halted: true }
}

phase('Verify')
const det = await agent(DET_PROMPT, { label: 'determinism-review', agentType: 'axiom-determinism-and-replay:determinism-reviewer', schema: VERDICT_SCHEMA })
const skeptic = await agent(SKEPTIC_PROMPT, { label: 'adversarial-gate', schema: VERDICT_SCHEMA })

return { impl, det, skeptic, halted: false }