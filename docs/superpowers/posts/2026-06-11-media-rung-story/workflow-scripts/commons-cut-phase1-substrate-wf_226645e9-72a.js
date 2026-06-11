export const meta = {
  name: 'commons-cut-phase1-substrate',
  description: 'Commons-miner cut — Phase 1 substrate (plan tasks 1-5: crate scaffold, types, seeded scenario, integer tick + gradient check, golden trajectory hash) via per-task TDD implement+verify+repair',
  phases: [
    { title: 'T1 scaffold' }, { title: 'T2 state types' }, { title: 'T3 rng_bridge' },
    { title: 'T4 dynamics' }, { title: 'T5 golden hash' },
  ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-commons-miner-cut.md'
const SPEC = 'docs/superpowers/specs/2026-06-10-commons-miner-cut-design.md'

const LAWS = `
LAW 1 — READ THE PLAN (${PLAN}) and implement the named task FAITHFULLY. It contains complete code per step; transcribe + adapt only where the plan explicitly flags (the Task-4 gradient check may raise STOCK_MAX; the Task-5 golden hash is pinned once). Read ${SPEC} for context.
LAW 2 — STRICT TDD: write the failing test FIRST; RUN it and SEE the specific failure; only then implement; run green. (git stash is operator-blocked — confirm red by temporarily neutering the impl, then restore.)
LAW 3 — INTEGER-ONLY in arena state + transitions; NO f64 in dynamics/state (f64 is allowed ONLY in measurement/reporting, none in Phase 1). No in-tick RNG; ships iterate in index order; simultaneous update (read tick-start state, apply, then yields/decrement).
LAW 4 — COMMIT exactly the task's named files with explicit \`git add <paths>\` (NEVER \`git add -A\`/\`.\`). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Message MUST end with:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  (Use \`git commit -F\` with a heredoc if the message has parens/special chars — a -m string with unbalanced parens breaks the shell.)
LAW 5 — DO NOT MODIFY jumpgate-core. The cut only DEPENDS on it (rng). The only core-adjacent edit is adding the crate to the ROOT Cargo.toml workspace members (Task 1). jumpgate-core tests/goldens must stay untouched.
LAW 6 — Rust 2024 reserves \`gen\`: use \`generation\`.
LAW 7 — clippy gate: \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` must be clean.
LAW 8 — Report ONLY what you actually ran (paste the real \`test result:\` line + clippy outcome). Subagents have fabricated gate claims here before; the main loop re-verifies everything.
`

const IMPL_SCHEMA = { type:'object', additionalProperties:false, required:['done','commit_sha','new_tests','deviations','blocker'], properties:{
  done:{type:'boolean'}, commit_sha:{type:'string'}, new_tests:{type:'string'}, deviations:{type:'string'}, blocker:{type:'string',description:'blocker needing the main loop, or "none"'} } }
const VERIFY_SCHEMA = { type:'object', additionalProperties:false, required:['passed','suite_result','clippy_result','commit_present','core_untouched','problems'], properties:{
  passed:{type:'boolean'}, suite_result:{type:'string'}, clippy_result:{type:'string'}, commit_present:{type:'boolean'}, core_untouched:{type:'boolean',description:'jumpgate-core was NOT modified (only root Cargo.toml members + the new crate)'}, problems:{type:'string'} } }

const TASKS = [
  { n:1, phase:'T1 scaffold', files:'Cargo.toml (root members), crates/jumpgate-commons-cut/Cargo.toml, crates/jumpgate-commons-cut/src/lib.rs', subject:'feat(commons-cut): scaffold crate + Region/Ship integer types' },
  { n:2, phase:'T2 state types', files:'crates/jumpgate-commons-cut/src/lib.rs', subject:'feat(commons-cut): ArenaConfig/ArenaState/Action types' },
  { n:3, phase:'T3 rng_bridge', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/rng_bridge.rs', subject:'feat(commons-cut): deterministic seeded scenario setup (field_correlation axis)' },
  { n:4, phase:'T4 dynamics', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/dynamics.rs', subject:'feat(commons-cut): integer tick — crowd-split yield, depletion, regen, transit (+gradient gate)' },
  { n:5, phase:'T5 golden hash', files:'crates/jumpgate-commons-cut/tests/golden_trajectory.rs', subject:'test(commons-cut): pinned golden trajectory hash — determinism control' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  log(`Task ${t.n}: implementing`)
  const impl = await agent(
    `Implement Task ${t.n} of the commons-miner cut, strict TDD.\n${LAWS}\nFILES (the task's scope): ${t.files}\nWork in /home/john/jumpgate. Read the plan's Task ${t.n} section and implement it faithfully (it has complete code). Commit with subject exactly: "${t.subject}" + the trailer. Return the structured result.`,
    { label:`impl:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
  )
  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${t.n} BLOCKED: ${impl?impl.blocker:'null'}`); results.push({task:t.n, impl, verify:null, halted:true}); break
  }
  log(`Task ${t.n}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `INDEPENDENT VERIFIER for Task ${t.n} of the commons-miner cut. Trust nothing; re-run in /home/john/jumpgate.\n${LAWS}\nRun & report verbatim:\n1. \`cargo test -p jumpgate-commons-cut\` — the \`test result:\` line; the task's new test(s) present+passing (read the body — substantive?). Claimed: ${impl.new_tests}\n2. \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` — clean.\n3. \`git show --stat HEAD\`: subject == "${t.subject}", trailer present, ONLY the task's files (${t.files}), NO forbidden files.\n4. CORE UNTOUCHED: confirm HEAD did not modify crates/jumpgate-core/** (only root Cargo.toml + the new crate). For Task 4: confirm the gradient check passed (>=4 distinct yields; if STOCK_MAX was raised to 50, dependent test constants were re-derived). For Task 5: confirm the golden hash literal is the real pinned value (not 0x000...0) and stable.\npassed=true only if all hold. Return the verdict with real outputs.`,
    { label:`verify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
  )
  if (!verify || !verify.passed) {
    log(`Task ${t.n} verify FAILED: ${verify?verify.problems:'null'} — one repair`)
    const repair = await agent(
      `Task ${t.n} FAILED verification. Fix in /home/john/jumpgate; keep the single-cause subject "${t.subject}" + trailer (amend or fixup).\n${LAWS}\nPROBLEMS: ${verify?verify.problems:'verifier null'}\nRe-run cargo test + clippy and report real results. Return the impl-shaped result.`,
      { label:`repair:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
    )
    log(`Task ${t.n} repair ${repair?repair.commit_sha:'null'}; re-verifying`)
    verify = await agent(
      `RE-VERIFY Task ${t.n} after repair. \`cargo test -p jumpgate-commons-cut\`, \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\`, commit clean (trailer, named files, no forbidden), jumpgate-core untouched. Report verbatim.`,
      { label:`reverify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
    )
    results.push({task:t.n, impl, repair, verify, halted: !verify||!verify.passed})
    if (!verify || !verify.passed) { log(`Task ${t.n} STILL FAILING — HALT`); break }
  } else {
    log(`Task ${t.n} GREEN — ${verify.suite_result}`); results.push({task:t.n, impl, verify, halted:false})
  }
}
return { phase:'1-substrate', results }
