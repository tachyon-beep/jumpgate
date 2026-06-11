export const meta = {
  name: 'commons-cut-phase3-dp',
  description: 'Commons-miner cut — Phase 3 DP ceiling (plan tasks 9-11: encode/decode + open-loop BR, closed-loop BR + phantom-ceiling check, planner upper bound) via per-task TDD implement+verify+repair',
  phases: [ { title: 'T9 encode+BR' }, { title: 'T10 closed-loop+phantom' }, { title: 'T11 planner bound' } ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-commons-miner-cut.md'

const LAWS = `
LAW 1 — READ THE PLAN (${PLAN}) and implement the named task FAITHFULLY (complete code per step). Phase-1/2 LESSONS: use \`rand_core::Rng\` not RngCore; STOCK_MAX=20; the yield law in any reasoning MUST match dynamics.rs (stock*richness_cap/(STOCK_MAX*occupants), floored, u64); commit via \`git commit -F\` heredoc (parens in messages); clippy may force \`for (i, x) in v.iter().enumerate()\` over indexed loops (needless_range_loop).
LAW 2 — STRICT TDD: failing test FIRST; RUN and SEE the specific failure; then implement; green. (git stash operator-blocked — neuter+restore to confirm red.)
LAW 3 — THE PHANTOM-CEILING CROSS-CHECK IS THE LOAD-BEARING GUARD (Task 10): the computed closed-loop BR value V0 MUST EXACTLY EQUAL the realized rollout of the chosen policy. If they differ, the DP or the realize logic is WRONG — fix the bug, NEVER weaken the assertion to a tolerance or skip it. Same for the tiny-instance brute-force/known-optimum check (Task 9) and closed-loop<=open-loop + planner>=selfish (Task 10/11): these are correctness invariants — a violation is a bug, not a test to relax.
LAW 4 — INTEGER-ONLY in the DP/dynamics (no f64; f64 only in later measurement). The DP must be deterministic + exact: keep instances tiny (N=3, M in {2,3}, short horizon) so backward induction is exact; the encode() debug_assert that state fits in u64 must hold.
LAW 5 — COMMIT exactly the task's named files with explicit \`git add <paths>\` (NEVER -A/.). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Trailer REQUIRED:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
LAW 6 — DO NOT MODIFY jumpgate-core. Rust 2024: \`gen\`->\`generation\`.
LAW 7 — clippy: \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.
LAW 8 — The golden trajectory test (tests/golden_trajectory.rs) MUST STILL PASS unchanged — the DP uses step() but must not alter dynamics. If golden moves, STOP and report.
LAW 9 — Report ONLY what you actually ran (paste real test-result lines). Subagents fabricate; the main loop re-verifies. If a correctness invariant genuinely cannot be made to hold, return blocker != none (do NOT fake it green).
`

const IMPL_SCHEMA = { type:'object', additionalProperties:false, required:['done','commit_sha','new_tests','deviations','blocker'], properties:{
  done:{type:'boolean'}, commit_sha:{type:'string'}, new_tests:{type:'string'}, deviations:{type:'string'}, blocker:{type:'string'} } }
const VERIFY_SCHEMA = { type:'object', additionalProperties:false, required:['passed','suite_result','clippy_result','commit_present','invariants_hold','problems'], properties:{
  passed:{type:'boolean'}, suite_result:{type:'string'}, clippy_result:{type:'string'}, commit_present:{type:'boolean'}, invariants_hold:{type:'boolean',description:'the correctness invariants genuinely hold (T9 DP==brute-force; T10 phantom V0==realized + closed<=open; T11 planner>=selfish) AND golden intact + core untouched'}, problems:{type:'string'} } }

const TASKS = [
  { n:9, phase:'T9 encode+BR', files:'crates/jumpgate-commons-cut/src/lib.rs, crates/jumpgate-commons-cut/src/dp.rs', subject:'feat(commons-cut): state encode/decode + open-loop best-response DP value' },
  { n:10, phase:'T10 closed-loop+phantom', files:'crates/jumpgate-commons-cut/src/dp.rs (+ src/policies.rs only if a helper is genuinely needed)', subject:'feat(commons-cut): closed-loop best-response ceiling + phantom-ceiling cross-check' },
  { n:11, phase:'T11 planner bound', files:'crates/jumpgate-commons-cut/src/dp.rs', subject:'feat(commons-cut): planner upper bound (labelled coordination headroom, reported-only)' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  log(`Task ${t.n}: implementing`)
  const impl = await agent(
    `Implement Task ${t.n} of the commons-miner cut (Phase 3, the DP ceiling), strict TDD.\n${LAWS}\nFILES: ${t.files}\nWork in /home/john/jumpgate. Read the plan's Task ${t.n} section and implement faithfully (the plan's DP code may contain bugs as Phase 1/2 did — the correctness invariants in LAW 3 are your guard; fix bugs to make them GENUINELY hold). Commit subject exactly: "${t.subject}" + trailer. Return structured result.`,
    { label:`impl:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
  )
  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${t.n} BLOCKED: ${impl?impl.blocker:'null'}`); results.push({task:t.n, impl, verify:null, halted:true}); break
  }
  log(`Task ${t.n}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `INDEPENDENT VERIFIER for Task ${t.n} (commons-miner cut, Phase 3 DP). Trust nothing; re-run in /home/john/jumpgate.\n${LAWS}\nReport verbatim:\n1. \`cargo test -p jumpgate-commons-cut\` — result line; the task's new test(s) present+passing; READ THE BODY and confirm the CORRECTNESS INVARIANT is genuinely asserted (T9: DP value == an independent brute-force/known optimum on a tiny instance; T10: phantom check asserts V0 == realized EXACTLY, and closed-loop <= open-loop; T11: planner >= selfish BR). Claimed: ${impl.new_tests}\n2. \`cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings\` clean.\n3. \`git show --stat HEAD\`: subject=="${t.subject}", trailer, only the task's files, no forbidden files.\n4. invariants_hold: the correctness invariants are GENUINE (not weakened to a tolerance, not skipped) AND golden_trajectory still passes unchanged AND jumpgate-core untouched.\npassed=true only if all hold. Return verdict with real outputs.`,
    { label:`verify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
  )
  if (!verify || !verify.passed) {
    log(`Task ${t.n} verify FAILED: ${verify?verify.problems:'null'} — one repair`)
    const repair = await agent(
      `Task ${t.n} FAILED verification. Fix in /home/john/jumpgate; keep subject "${t.subject}" + trailer.\n${LAWS}\nPROBLEMS: ${verify?verify.problems:'verifier null'}\nThe phantom-ceiling / DP-vs-brute-force / closed<=open / planner>=selfish invariants must GENUINELY hold — fix the underlying DP bug, do NOT weaken them. If genuinely unresolvable, return blocker != none with the exact mismatch numbers. Re-run cargo test + clippy + golden, report real results.`,
      { label:`repair:T${t.n}`, phase:t.phase, schema:IMPL_SCHEMA, agentType:'general-purpose' },
    )
    log(`Task ${t.n} repair ${repair?repair.commit_sha:'null'} blocker=${repair?repair.blocker:'null'}; re-verifying`)
    if (repair && repair.blocker && repair.blocker !== 'none') { results.push({task:t.n, impl, repair, verify, halted:true}); log(`Task ${t.n} BLOCKER — HALT for main loop`); break }
    verify = await agent(
      `RE-VERIFY Task ${t.n} after repair. cargo test, clippy --all-targets -D warnings, the correctness invariant genuinely holds, golden intact, core untouched, commit clean. Report verbatim.`,
      { label:`reverify:T${t.n}`, phase:t.phase, schema:VERIFY_SCHEMA, agentType:'general-purpose' },
    )
    results.push({task:t.n, impl, repair, verify, halted: !verify||!verify.passed})
    if (!verify || !verify.passed) { log(`Task ${t.n} STILL FAILING — HALT`); break }
  } else { log(`Task ${t.n} GREEN — ${verify.suite_result}`); results.push({task:t.n, impl, verify, halted:false}) }
}
return { phase:'3-dp', results }
