export const meta = {
  name: 'commons-cut-phase4-gate',
  description: 'Commons-miner cut — Phase 4 gate+MC (plan tasks 12-14: fraction-of-ceiling, MC estimator DP-calibrated with CI, N-scaling verdict) via per-task TDD implement+verify+repair',
  phases: [ { title: 'T12 fraction' }, { title: 'T13 MC estimator' }, { title: 'T14 verdict' } ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-commons-miner-cut.md'

const LAWS = `
LAW 1 — READ THE PLAN (${PLAN}) and implement the named task FAITHFULLY. Phase-1/2/3 LESSONS: use \`rand_core::Rng\` not RngCore; STOCK_MAX=20; yield law = stock*richness_cap/(STOCK_MAX*occupants) floored u64; commit via \`git commit -F\` heredoc; clippy forbids needless_range_loop (use .iter().enumerate()).
LAW 2 — f64 IS NOW ALLOWED — but ONLY in measurement/reporting (fractions, CIs, verdict). NEVER feed f64 back into the integer sim/DP/dynamics. The arena + DP stay integer.
LAW 3 — STRICT TDD: failing test FIRST; RUN and SEE it fail; then implement; green. (git stash operator-blocked — neuter+restore.)
LAW 4 — THE MC CALIBRATION IS THE LOAD-BEARING GUARD (Task 13): the MC best-response estimate's confidence interval MUST BRACKET the EXACT closed-loop DP value at small N (the exact DP is ground truth). If it does not bracket, the MC is too shallow/biased — deepen the lookahead or raise samples until it genuinely brackets; do NOT widen the CI artificially or weaken the assertion. An MC that cannot bracket the DP is a real bug (return blocker if truly unresolvable).
LAW 5 — COMPARABILITY NOTE (for correctness): the DP ceiling is a SINGLE deviator's take; population rungs (constant/closed-form) are per-ship MEANS. fraction_of_ceiling is a pure (ceiling,bar,floor) f64 fn — the CALLER must pass per-ship-comparable values. Keep fraction_of_ceiling degenerate-safe (zero range -> 0.0, never NaN). The verdict (T14) only asserts guaranteed relations + the flat-or-rising / CI-straddle / decay logic.
LAW 6 — COMMIT exactly the task's named files with explicit \`git add\` (NEVER -A/.). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Trailer REQUIRED:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
LAW 7 — DO NOT MODIFY jumpgate-core. Rust 2024: \`gen\`->\`generation\`. clippy \`-p jumpgate-commons-cut --all-targets -- -D warnings\` clean.
LAW 8 — golden_trajectory test MUST still pass unchanged (the gate/MC must not touch dynamics). If it moves, STOP and report.
LAW 9 — Report ONLY what you actually ran (paste real test-result lines). Subagents fabricate; the main loop re-verifies. Fake nothing green; blocker != none if a load-bearing invariant cannot genuinely hold.
`

const IMPL_SCHEMA = { type:'object', additionalProperties:false, required:['done','commit_sha','new_tests','deviations','blocker'], properties:{
  done:{type:'boolean'}, commit_sha:{type:'string'}, new_tests:{type:'string'}, deviations:{type:'string'}, blocker:{type:'string'} } }
const VERIFY_SCHEMA = { type:'object', additionalProperties:false, required:['passed','suite_result','clippy_result','commit_present','invariants_hold','problems'], properties:{
  passed:{type:'boolean'}, suite_result:{type:'string'}, clippy_result:{type:'string'}, commit_present:{type:'boolean'}, invariants_hold:{type:'boolean',description:'T12 frac def + degenerate-safe; T13 MC CI genuinely brackets the exact DP (not artificially widened); T14 verdict logic correct on synthetic curves; golden intact + core untouched'}, problems:{type:'string'} } }

const TASKS = [
  { n:12, phase:'T12 fraction', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/gate.rs', subject:'feat(commons-cut): fraction-of-ceiling definition + pre-registered GAP_FRAC_MIN' },
  { n:13, phase:'T13 MC estimator', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/mc.rs', subject:'feat(commons-cut): MC best-response estimator + DP-calibrated confidence interval' },
  { n:14, phase:'T14 verdict', files:'crates/jumpgate-commons-cut/src/gate.rs', subject:'feat(commons-cut): N-scaling verdict (flat-or-rising, CI-aware) — the gate' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  log(`Task ${t.n}: implementing`)
  const impl = await agent(
    `Implement Task ${t.n} of the commons-miner cut (Phase 4, gate+MC), strict TDD.\n${LAWS}\nFILES: ${t.files}\nWork in /home/john/jumpgate. Read the plan's Task ${t.n} section and implement faithfully (plan code may have bugs as before — the LAW 4 MC-brackets-DP guard and the verdict logic are your guards; make them GENUINELY hold). Commit subject exactly: "${t.subject}" + trailer. Return structured result.`,
    { label:`impl:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
  )
  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${t.n} BLOCKED: ${impl?impl.blocker:'null'}`); results.push({task:t.n, impl, verify:null, halted:true}); break
  }
  log(`Task ${t.n}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `INDEPENDENT VERIFIER for Task ${t.n} (commons-miner cut, Phase 4). Trust nothing; re-run in /home/john/jumpgate.\n${LAWS}\nReport verbatim:\n1. \`cargo test -p jumpgate-commons-cut\` — result line; the task's new test(s) present+passing; READ THE BODY. For T13 confirm the test genuinely asserts the exact DP value lies WITHIN the MC CI (and the CI is not absurdly wide to force it). For T14 confirm the verdict cases (GO flat-or-rising; NO-GO below-gate / decaying; Inconclusive CI-straddle) are all exercised. Claimed: ${impl.new_tests}\n2. \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.\n3. \`git show --stat HEAD\`: subject=="${t.subject}", trailer, only the task's files, no forbidden files.\n4. invariants_hold + golden intact + core untouched.\npassed=true only if all hold. Return verdict with real outputs.`,
    { label:`verify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
  )
  if (!verify || !verify.passed) {
    log(`Task ${t.n} verify FAILED: ${verify?verify.problems:'null'} — one repair`)
    const repair = await agent(
      `Task ${t.n} FAILED verification. Fix in /home/john/jumpgate; keep subject "${t.subject}" + trailer.\n${LAWS}\nPROBLEMS: ${verify?verify.problems:'verifier null'}\nIf the MC CI fails to bracket the exact DP (T13), deepen lookahead/raise samples until it GENUINELY brackets — do NOT widen the CI to fake it. If unresolvable, blocker != none with the exact DP value vs MC [lo,hi]. Re-run cargo test + clippy + golden; report real results.`,
      { label:`repair:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
    )
    log(`Task ${t.n} repair ${repair?repair.commit_sha:'null'} blocker=${repair?repair.blocker:'null'}; re-verifying`)
    if (repair && repair.blocker && repair.blocker !== 'none') { results.push({task:t.n, impl, repair, verify, halted:true}); log(`Task ${t.n} BLOCKER — HALT`); break }
    verify = await agent(
      `RE-VERIFY Task ${t.n} after repair. cargo test, clippy --all-targets -D warnings, the load-bearing invariant genuinely holds (esp. T13 MC brackets exact DP), golden intact, core untouched, commit clean. Report verbatim.`,
      { label:`reverify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
    )
    results.push({task:t.n, impl, repair, verify, halted: !verify||!verify.passed})
    if (!verify || !verify.passed) { log(`Task ${t.n} STILL FAILING — HALT`); break }
  } else { log(`Task ${t.n} GREEN — ${verify.suite_result}`); results.push({task:t.n, impl, verify, halted:false}) }
}
return { phase:'4-gate', results }
