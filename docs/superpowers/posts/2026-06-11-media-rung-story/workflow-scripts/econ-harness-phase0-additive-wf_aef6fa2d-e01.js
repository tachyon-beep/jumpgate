export const meta = {
  name: 'econ-harness-phase0-additive',
  description: 'First economic loop harness — Phase 0 additive tasks 1-4 (Resource, economy ids, stores) via per-task TDD implement+verify',
  phases: [
    { title: 'Phase0-additive', detail: 'Tasks 1-4: Resource enum, economy ids, Station/Producer/Corp/Contract stores' },
  ],
}

const SHARED = `
PROJECT: Jumpgate v1 deterministic 3D Newtonian sim. Repo root: /home/john/jumpgate. Crate: jumpgate-core (Rust 2024, #![forbid(unsafe_code)]). Branch: jumpgate-v1-design (stay on it; do NOT create branches).

PLAN (source of truth): docs/superpowers/plans/2026-06-09-first-economic-loop-harness.md
SPEC (context): docs/superpowers/specs/2026-06-09-first-economic-loop-harness-design.md

NON-NEGOTIABLE LAWS:
1. LOCATE CODE BY SYMBOL, NEVER BY CITED LINE NUMBER. The plan cites file:line refs that are correct at planning time but go STALE as earlier tasks edit files. Use grep/Read to find the actual symbol (struct/fn/enum) before editing. If a cited line number disagrees with what you find, trust the symbol.
2. FOLLOW TDD EXACTLY, in order: (a) write the failing test, (b) run it and CONFIRM it fails for the stated reason, (c) write the minimal implementation, (d) run the test and CONFIRM it passes, (e) run the FULL crate suite 'cargo test -p jumpgate-core' and confirm NOTHING regressed, (f) commit.
3. COMMIT DISCIPLINE: 'git add' ONLY the specific files the task names (the working tree has untracked .claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf and a modified .gitignore — NEVER 'git add -A', NEVER add any of those). Use the task's exact commit message. Append this trailer to every commit message (two lines: a blank line then the trailer):

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

4. DO NOT touch determinism golden constants in this batch. Tasks 1-4 are purely additive (new types/stores) and MUST NOT move GOLDEN_CONFIG_HASH, GOLDEN_ZERO_STATE_HASH, the zero-world golden, or HASH_FORMAT_VERSION. If you find yourself needing to, STOP — you have misread the task; report it instead.
5. Report only what you actually did and actually observed. Do not claim a test run you did not perform. Paste real command results.
`

const IMPL_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    done: { type: 'boolean', description: 'true only if the task is fully implemented, tests pass, and committed' },
    commit_sha: { type: 'string', description: 'the git commit sha created, or "none"' },
    new_test_name: { type: 'string', description: 'the test function name added by this task' },
    full_suite_result: { type: 'string', description: 'verbatim final line of cargo test -p jumpgate-core (the lib "test result: ..." line)' },
    deviations: { type: 'string', description: 'any deviation from the plan text, stale line numbers corrected, or "none"' },
  },
  required: ['done', 'commit_sha', 'new_test_name', 'full_suite_result', 'deviations'],
}

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    passed: { type: 'boolean', description: 'true only if full suite is green AND clippy is clean AND the commit exists' },
    suite_result: { type: 'string', description: 'verbatim lib "test result:" line from cargo test -p jumpgate-core' },
    clippy_result: { type: 'string', description: 'PASS if clippy --all-targets -D warnings is clean, else the first error/warning' },
    commit_present: { type: 'boolean', description: 'true if git log shows the expected economy commit at HEAD' },
    problems: { type: 'string', description: 'concrete problems found, or "none"' },
  },
  required: ['passed', 'suite_result', 'clippy_result', 'commit_present', 'problems'],
}

phase('Phase0-additive')

const tasks = [1, 2, 3, 4]
const results = []

for (const n of tasks) {
  let impl = await agent(
    `${SHARED}\n\nYou are implementing **Task ${n}** of the plan. Open the plan file, find the "### Task ${n}:" section, and implement EXACTLY that task — every step in order (failing test → confirm fail → implement → confirm pass → full suite → commit). Read the source files the task references (locate symbols, not line numbers). When done, the crate must build, the new test must pass, the full 'cargo test -p jumpgate-core' must be green with nothing regressed, and you must have committed with the task's exact message + the required trailer. Return the structured result honestly.`,
    { label: `impl:T${n}`, phase: 'Phase0-additive', schema: IMPL_SCHEMA }
  )

  let verify = await agent(
    `${SHARED}\n\nYou are an INDEPENDENT verifier for **Task ${n}** (do NOT trust the implementer's claims — run the commands yourself). From the repo root run:\n1. 'cargo test -p jumpgate-core' — capture the lib "test result:" line; it must say 0 failed.\n2. 'cargo clippy --all-targets -- -D warnings' — must exit clean.\n3. 'git log --oneline -1' — confirm the HEAD commit is the Task ${n} economy commit.\n4. Confirm the determinism goldens are UNMOVED: 'grep -n "GOLDEN_CONFIG_HASH\\|GOLDEN_ZERO_STATE_HASH\\|HASH_FORMAT_VERSION\\|0x532d" crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/config.rs' and confirm HASH_FORMAT_VERSION is still 1 and the golden hex values are unchanged from 0x278c_5d91_b75a_9e5a / 0xf0dd_a1ba_f433_3735 / 0x532d_07bf_95a2_abc5.\nReport pass/fail with the real outputs.`,
    { label: `verify:T${n}`, phase: 'Phase0-additive', schema: VERIFY_SCHEMA }
  )

  if (!verify || !verify.passed) {
    log(`Task ${n} verify FAILED: ${verify ? verify.problems : 'verifier died'} — attempting one repair`)
    await agent(
      `${SHARED}\n\nTask ${n} was implemented but verification FAILED with: ${verify ? verify.problems : 'verifier returned nothing'} (suite: ${verify ? verify.suite_result : '?'}, clippy: ${verify ? verify.clippy_result : '?'}). Diagnose and FIX so that 'cargo test -p jumpgate-core' is fully green and 'cargo clippy --all-targets -- -D warnings' is clean, WITHOUT moving any determinism golden or the hash version. Amend or add a follow-up commit (specific files only, + trailer). Then confirm both commands are green.`,
      { label: `repair:T${n}`, phase: 'Phase0-additive' }
    )
    verify = await agent(
      `${SHARED}\n\nRe-verify **Task ${n}** after a repair. Run 'cargo test -p jumpgate-core', 'cargo clippy --all-targets -- -D warnings', and 'git log --oneline -2'. Confirm goldens unmoved (HASH_FORMAT_VERSION==1; 0x278c/0xf0dd/0x532d unchanged). Report honestly.`,
      { label: `reverify:T${n}`, phase: 'Phase0-additive', schema: VERIFY_SCHEMA }
    )
    if (!verify || !verify.passed) {
      throw new Error(`Task ${n} still failing after repair: ${verify ? verify.problems : 'verifier died'}. Halting Phase 0 additive batch for human inspection.`)
    }
  }

  results.push({ task: n, impl, verify })
  log(`Task ${n} GREEN — ${verify.suite_result}`)
}

return results
