export const meta = {
  name: 'commons-cut-phase2-policies',
  description: 'Commons-miner cut — Phase 2 policies (plan tasks 6-8: Policy trait + Constant + rollout, randomizing closed-form + fit, myopic + ladder monotonicity) via per-task TDD implement+verify+repair',
  phases: [ { title: 'T6 trait+constant' }, { title: 'T7 closed-form' }, { title: 'T8 myopic' } ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-commons-miner-cut.md'

const LAWS = `
LAW 1 — READ THE PLAN (${PLAN}) and implement the named task FAITHFULLY (complete code per step). Phase-1 LESSONS to apply: (a) use \`rand_core::Rng\` NOT \`rand_core::RngCore\` for next_u32/next_u64 (matches core's rng.rs); rand_core is already a direct dep. (b) STOCK_MAX stayed 20 (gradient passed) — plan test constants assuming STOCK_MAX=20 are correct. (c) The yield law in any policy MUST match dynamics.rs exactly: per_ship = stock*richness_cap/(STOCK_MAX*occupants), floored, u64.
LAW 2 — STRICT TDD: failing test FIRST; RUN and SEE the specific failure; then implement; green. (git stash is operator-blocked — confirm red by neutering the impl, then restore.)
LAW 3 — INTEGER-ONLY in arena state/transitions/policies' yield reasoning; NO f64 in policy decisions or dynamics. No in-tick entropy beyond the policy's OWN seeded coin (ClosedForm carries a seeded Rng keyed by seed/tick/ship — deterministic). Ships in index order.
LAW 4 — COMMIT exactly the task's named files with explicit \`git add <paths>\` (NEVER -A/.). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Use \`git commit -F\` heredoc (avoid -m with parens). Trailer REQUIRED:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  (Cargo.lock may be staged with the task IF a dep edge changed; otherwise leave it.)
LAW 5 — DO NOT MODIFY jumpgate-core.
LAW 6 — Rust 2024 reserves \`gen\`: use \`generation\`.
LAW 7 — clippy: \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.
LAW 8 — The golden trajectory test (tests/golden_trajectory.rs) MUST STILL PASS unchanged — policies must not perturb dynamics. If it moves, you broke determinism: STOP and report, do not re-pin.
LAW 9 — TASK 8 ladder monotonicity (constant <= closed-form <= myopic): this is a REAL correctness test. If it fails, a policy or the fit is BUGGY — fix the bug, do NOT weaken/force the assertion. Report as a blocker if you cannot make it genuinely pass.
LAW 10 — Report ONLY what you actually ran (paste real test-result lines). Subagents have fabricated gate claims; the main loop re-verifies.
`

const IMPL_SCHEMA = { type:'object', additionalProperties:false, required:['done','commit_sha','new_tests','deviations','blocker'], properties:{
  done:{type:'boolean'}, commit_sha:{type:'string'}, new_tests:{type:'string'}, deviations:{type:'string'}, blocker:{type:'string'} } }
const VERIFY_SCHEMA = { type:'object', additionalProperties:false, required:['passed','suite_result','clippy_result','commit_present','golden_intact','problems'], properties:{
  passed:{type:'boolean'}, suite_result:{type:'string'}, clippy_result:{type:'string'}, commit_present:{type:'boolean'}, golden_intact:{type:'boolean',description:'golden_trajectory test still passes unchanged + jumpgate-core untouched'}, problems:{type:'string'} } }

const TASKS = [
  { n:6, phase:'T6 trait+constant', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/policies.rs', subject:'feat(commons-cut): Policy trait + Constant rung + rollout harness' },
  { n:7, phase:'T7 closed-form', files:'crates/jumpgate-commons-cut/src/policies.rs', subject:'feat(commons-cut): randomizing best-closed-form rung + train/eval grid fit' },
  { n:8, phase:'T8 myopic', files:'crates/jumpgate-commons-cut/src/policies.rs', subject:'feat(commons-cut): per-seed-myopic rung + ladder monotonicity sanity' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  log(`Task ${t.n}: implementing`)
  const impl = await agent(
    `Implement Task ${t.n} of the commons-miner cut (Phase 2 policies), strict TDD.\n${LAWS}\nFILES: ${t.files}\nWork in /home/john/jumpgate. Read the plan's Task ${t.n} section and implement faithfully. Commit subject exactly: "${t.subject}" + trailer. Return structured result.`,
    { label:`impl:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
  )
  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${t.n} BLOCKED: ${impl?impl.blocker:'null'}`); results.push({task:t.n, impl, verify:null, halted:true}); break
  }
  log(`Task ${t.n}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `INDEPENDENT VERIFIER for Task ${t.n} (commons-miner cut, Phase 2). Trust nothing; re-run in /home/john/jumpgate.\n${LAWS}\nReport verbatim:\n1. \`cargo test -p jumpgate-commons-cut\` — result line; the task's new test(s) present+passing (read body — substantive? for T8 confirm the monotonicity test genuinely runs constant/closed-form/myopic and asserts the ordering). Claimed: ${impl.new_tests}\n2. \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.\n3. \`git show --stat HEAD\`: subject=="${t.subject}", trailer, ONLY ${t.files} (+Cargo.lock if a dep changed), no forbidden files.\n4. golden_intact: \`cargo test -p jumpgate-commons-cut --test golden_trajectory\` still passes UNCHANGED (determinism preserved) AND jumpgate-core not modified by HEAD.\npassed=true only if all hold. Return verdict with real outputs.`,
    { label:`verify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
  )
  if (!verify || !verify.passed) {
    log(`Task ${t.n} verify FAILED: ${verify?verify.problems:'null'} — one repair`)
    const repair = await agent(
      `Task ${t.n} FAILED verification. Fix in /home/john/jumpgate; keep subject "${t.subject}" + trailer.\n${LAWS}\nPROBLEMS: ${verify?verify.problems:'verifier null'}\nIf the failure is the ladder-monotonicity test (T8), it signals a real policy/fit bug — fix the bug, do NOT weaken the assertion; if genuinely unresolvable, return blocker != none. Re-run cargo test + clippy + golden, report real results. Return impl-shaped result.`,
      { label:`repair:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
    )
    log(`Task ${t.n} repair ${repair?repair.commit_sha:'null'} blocker=${repair?repair.blocker:'null'}; re-verifying`)
    if (repair && repair.blocker && repair.blocker !== 'none') { results.push({task:t.n, impl, repair, verify, halted:true}); log(`Task ${t.n} repair BLOCKER — HALT`); break }
    verify = await agent(
      `RE-VERIFY Task ${t.n} after repair. cargo test -p jumpgate-commons-cut, clippy --all-targets -D warnings, golden_trajectory intact, jumpgate-core untouched, commit clean. Report verbatim.`,
      { label:`reverify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
    )
    results.push({task:t.n, impl, repair, verify, halted: !verify||!verify.passed})
    if (!verify || !verify.passed) { log(`Task ${t.n} STILL FAILING — HALT`); break }
  } else { log(`Task ${t.n} GREEN — ${verify.suite_result}`); results.push({task:t.n, impl, verify, halted:false}) }
}
return { phase:'2-policies', results }
