export const meta = {
  name: 'commons-cut-phase5-run',
  description: 'Commons-miner cut — Phase 5 build (plan tasks 15-16: CutSummary + pre-registered run harness, observe-large pack diagnostics) via per-task TDD implement+verify+repair. Task 17 (run + record verdict) is main-loop.',
  phases: [ { title: 'T15 run harness' }, { title: 'T16 pack diagnostics' } ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-commons-miner-cut.md'

const LAWS = `
LAW 1 — READ THE PLAN (${PLAN}) and implement the named task FAITHFULLY. Lessons: rand_core::Rng not RngCore; STOCK_MAX=20; yield law stock*richness_cap/(STOCK_MAX*occupants) floored u64; git commit -F heredoc; clippy forbids needless_range_loop.
LAW 2 — COMPARABILITY (correctness, LAW from Phase 4): the DP/MC ceiling is a SINGLE deviator's take; population rungs (constant, closed-form) are TOTALS that MUST be divided by ship count to per-ship means before being passed to fraction_of_ceiling. The plan's execute() does this (\`/3.0\`, \`/nn\`) — preserve it. Mixing a single-ship ceiling with a population TOTAL would be a correctness bug.
LAW 3 — THE VERDICT IS A FINDING, NOT A TARGET. The #[ignore] run_the_cut must NOT assert GO or NO-GO — it computes and prints/returns the verdict; the only assertion is APPARATUS FAIRNESS (negative control = identical regions MUST be NO-GO). Do NOT rig the experiment toward any outcome. The non-ignored execute() smoke test asserts only that it runs + the negative control holds.
LAW 4 — MC IS A CONSERVATIVE LOWER-BOUND CEILING at N>3 (truncated bounded-depth greedy, deterministic field => ~zero-width CI). That is acceptable (it cannot false-GO; a truncation NO-GO at high N is distinguishable by deeper depth). Do NOT try to "fix" the zero-width CI by injecting artificial variance. Just wire it per the plan.
LAW 5 — STRICT TDD: failing test FIRST; RUN and SEE it fail; then implement; green. (git stash operator-blocked — neuter+restore.)
LAW 6 — COMMIT exactly the task's named files with explicit \`git add\` (NEVER -A/.). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Trailer REQUIRED:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
LAW 7 — DO NOT MODIFY jumpgate-core. Rust 2024 \`gen\`->\`generation\`. clippy \`-p jumpgate-commons-cut --all-targets -- -D warnings\` clean. golden_trajectory must still pass unchanged.
LAW 8 — Report ONLY what you actually ran (paste real test-result lines). Subagents fabricate; the main loop re-verifies. Do NOT fabricate a verdict — if you run the #[ignore] experiment, paste its REAL printed output; if you did not run it, say so. blocker != none if something genuinely cannot hold.
`

const IMPL_SCHEMA = { type:'object', additionalProperties:false, required:['done','commit_sha','new_tests','deviations','blocker'], properties:{
  done:{type:'boolean'}, commit_sha:{type:'string'}, new_tests:{type:'string'}, deviations:{type:'string'}, blocker:{type:'string'} } }
const VERIFY_SCHEMA = { type:'object', additionalProperties:false, required:['passed','suite_result','clippy_result','commit_present','apparatus_fair','problems'], properties:{
  passed:{type:'boolean'}, suite_result:{type:'string'}, clippy_result:{type:'string'}, commit_present:{type:'boolean'}, apparatus_fair:{type:'boolean',description:'the run does NOT assert a GO/NO-GO outcome; negative control NO-GO holds; per-ship comparability preserved; golden intact + core untouched'}, problems:{type:'string'} } }

const TASKS = [
  { n:15, phase:'T15 run harness', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/report.rs', subject:'feat(commons-cut): CutSummary verdict struct + the pre-registered run (sweep + N-ladder + controls)' },
  { n:16, phase:'T16 pack diagnostics', files:'crates/jumpgate-commons-cut/src/report.rs', subject:'feat(commons-cut): observe-large pack diagnostics (reported, not a GO signal)' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  log(`Task ${t.n}: implementing`)
  const impl = await agent(
    `Implement Task ${t.n} of the commons-miner cut (Phase 5 build), strict TDD.\n${LAWS}\nFILES: ${t.files}\nWork in /home/john/jumpgate. Read the plan's Task ${t.n} section and implement faithfully (plan code may have bugs — fix to correctness; preserve per-ship comparability LAW 2 and the verdict-is-a-finding discipline LAW 3). Commit subject exactly: "${t.subject}" + trailer. Return structured result.`,
    { label:`impl:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
  )
  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${t.n} BLOCKED: ${impl?impl.blocker:'null'}`); results.push({task:t.n, impl, verify:null, halted:true}); break
  }
  log(`Task ${t.n}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `INDEPENDENT VERIFIER for Task ${t.n} (commons-miner cut, Phase 5 build). Trust nothing; re-run in /home/john/jumpgate.\n${LAWS}\nReport verbatim:\n1. \`cargo test -p jumpgate-commons-cut\` — result line; the task's new non-ignored test(s) present+passing; READ THE BODY. For T15 confirm: (a) execute() smoke test runs + asserts the negative control NO-GO (apparatus fairness), (b) the #[ignore] run_the_cut does NOT assert GO/NO-GO (the verdict is a finding), (c) per-ship comparability preserved (population totals divided by ship count before fraction_of_ceiling). For T16 confirm pack_diagnostics is reporting-only (no GO assertion). Claimed: ${impl.new_tests}\n2. \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.\n3. \`git show --stat HEAD\`: subject=="${t.subject}", trailer, only the task's files, no forbidden files.\n4. apparatus_fair + golden intact + core untouched.\npassed=true only if all hold. Return verdict with real outputs.`,
    { label:`verify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
  )
  if (!verify || !verify.passed) {
    log(`Task ${t.n} verify FAILED: ${verify?verify.problems:'null'} — one repair`)
    const repair = await agent(
      `Task ${t.n} FAILED verification. Fix in /home/john/jumpgate; keep subject "${t.subject}" + trailer.\n${LAWS}\nPROBLEMS: ${verify?verify.problems:'verifier null'}\nPreserve per-ship comparability + verdict-is-a-finding. Re-run cargo test + clippy + golden; report real results. blocker != none if unresolvable.`,
      { label:`repair:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
    )
    log(`Task ${t.n} repair ${repair?repair.commit_sha:'null'} blocker=${repair?repair.blocker:'null'}; re-verifying`)
    if (repair && repair.blocker && repair.blocker !== 'none') { results.push({task:t.n, impl, repair, verify, halted:true}); log(`Task ${t.n} BLOCKER — HALT`); break }
    verify = await agent(
      `RE-VERIFY Task ${t.n} after repair. cargo test, clippy --all-targets -D warnings, apparatus fairness (no outcome assertion, negative control NO-GO, per-ship comparability), golden intact, core untouched, commit clean. Report verbatim.`,
      { label:`reverify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
    )
    results.push({task:t.n, impl, repair, verify, halted: !verify||!verify.passed})
    if (!verify || !verify.passed) { log(`Task ${t.n} STILL FAILING — HALT`); break }
  } else { log(`Task ${t.n} GREEN — ${verify.suite_result}`); results.push({task:t.n, impl, verify, halted:false}) }
}
return { phase:'5-run-build', results }
