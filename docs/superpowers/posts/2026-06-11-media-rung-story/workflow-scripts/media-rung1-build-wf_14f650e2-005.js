export const meta = {
  name: 'media-rung1-build',
  description: 'Build media cut-1 (gossip epidemiology) task-by-task: builder + independent verifier per plan task',
  phases: [
    { title: 'T0 lab note' },
    { title: 'T1 escrow bug' },
    { title: 'T2 metric calibration' },
    { title: 'T3 RngStream::Media' },
    { title: 'T4 config golden A' },
    { title: 'T5 state golden B v5' },
    { title: 'T6 mechanics C' },
    { title: 'T7 read swap D' },
    { title: 'T8 instruments E' },
    { title: 'T9 verification + sweep' },
  ],
}

const PLAN = 'docs/superpowers/plans/2026-06-11-media-rung1-gossip-implementation.md'
const SPEC = 'docs/superpowers/specs/2026-06-11-media-rung1-gossip-design.md'

const LAWS = `
PROJECT LAWS (verbatim, non-negotiable — violations are CRITICAL):
- Repo: /home/john/jumpgate, branch jumpgate-v1-design. Work ONLY there.
- THE PLAN is ${PLAN}; THE SPEC is ${SPEC}. Read your task's section of the plan IN FULL first, then the spec sections it cites. The plan is authoritative on order and content.
- Golden literals move ONLY in plan Tasks 4 and 5, re-derived via the ignored print_golden / print_golden_config tests (cargo test -p jumpgate-core <name> -- --ignored --nocapture) — NEVER invented, never copied from the plan. Every other task must leave HASH_FORMAT_VERSION, GOLDEN_ZERO_STATE_HASH, the zero-world state_hash golden, and GOLDEN_CONFIG_HASH untouched.
- TDD: write the failing test, run it to see it fail, implement, run it green. Actually RUN every command — never report a result you did not observe.
- cargo clippy --all-targets -- -D warnings (NEVER --lib, it is a no-op in this crate layout). Rust edition 2024: 'gen' is a reserved keyword.
- git: NEVER 'git add -A' or 'git add .' — explicit paths only. NEVER stage .gitignore, .claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf, or anything under runs/. 'git stash' is operator-blocked — never call it. Commit messages containing parentheses MUST use git commit -F - with a heredoc. Every commit ends with the trailer line exactly: Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
- PDR-0006: windows never gates; no kill-criterion vocabulary; reward stays delta-credits; no shaping; no per-craft taste scalars; route_evidence returns a RAW count; pirates are information-blind.
- filigree CLI: pass --actor claude on every verb.
- If a plan step is impossible as written (compile reality differs), implement the minimal faithful correction, and record the deviation explicitly in your report — do NOT silently improvise scope.
`

const BUILD_SCHEMA = {
  type: 'object',
  required: ['completed', 'commits', 'tests_observed', 'deviations', 'notes'],
  properties: {
    completed: { type: 'boolean', description: 'every checkbox of the task done and committed' },
    commits: { type: 'array', items: { type: 'string' }, description: 'short hash + subject of each commit made' },
    tests_observed: { type: 'string', description: 'the exact final test/clippy command outputs you OBSERVED (counts, pass/fail)' },
    deviations: { type: 'array', items: { type: 'string' }, description: 'any departure from the plan text, with reason' },
    notes: { type: 'string', description: 'recorded numbers/readings the plan asks to RECORD, and anything the next task must know' },
  },
}

const VERDICT_SCHEMA = {
  type: 'object',
  required: ['pass', 'critical', 'summary'],
  properties: {
    pass: { type: 'boolean' },
    critical: { type: 'array', items: { type: 'string' }, description: 'issues that MUST be fixed before the next task' },
    summary: { type: 'string', description: 'what you independently verified, with observed outputs' },
  },
}

const TASKS = [
  { n: 0, phase: 'T0 lab note', hint: 'docs only; read runs/pirates_ablation.log; one commit.' },
  { n: 1, phase: 'T1 escrow bug', hint: 'economy.rs resolve_failures + settle debug_assert; close filigree jumpgate-2c0c2d92bb via CLI after committing.' },
  { n: 2, phase: 'T2 metric calibration', hint: 'release-runs take ~1-2 min each; RECORD the measured HHI numbers in your notes; if labels are inseparable, STOP per plan step 2.2 and report. Includes baking runs/media_baseline (bench, never staged). Close filigree jumpgate-50c6a8a3bd.' },
  { n: 3, phase: 'T3 RngStream::Media', hint: 'rng.rs only; golden first-draw captured from the observed failure message.' },
  { n: 4, phase: 'T4 config golden A', hint: 'MediaCfg + fold + apply_knob + every RunConfig literal the compiler names (incl. jumpgate-py/src/env.rs); GOLDEN_CONFIG_HASH re-pinned via print_golden_config ONLY.' },
  { n: 5, phase: 'T5 state golden B v5', hint: 'media.rs structs + CraftStore.gossip + World fields + reset init/validation + hash words 30-32 + HASH_FORMAT_VERSION 5; BOTH state goldens re-pinned via print_golden ONLY.' },
  { n: 6, phase: 'T6 mechanics C', hint: 'the biggest task: events, mint at the Robbed settlement (seed = contract reward read BEFORE settle + ransom), origin-pier deposit, edge-triggered exchange (pre-refresh info_tick predicate), eviction, chronicle arm. ALL behavior behind media_live(); all four goldens must be byte-unchanged at the end.' },
  { n: 7, phase: 'T7 read swap D', hint: 'route_evidence body swap only (signature unchanged) + deaf-control trace-identity test (6k ticks, may take a couple minutes in debug).' },
  { n: 8, phase: 'T8 instruments E', hint: 'TrophicSample additive fields + media_classify + synthetics incl. StaleEcho trap + MEDIA line and MEDIA_RE in the SAME commit + --gossip-log + media_log.py + media_default_is_inert test.' },
  { n: 9, phase: 'T9 verification + sweep', hint: 'NO code commits expected. Run the full battery, the cross-branch inert diff vs runs/media_baseline, media-live replay-check, M-DEAD/M-ORACLE controls, and the band sweep; RECORD every reading verbatim in notes (these go to the owner). The sweep is ~12 release runs of 50k ticks — expect tens of minutes; run them.' },
]

const results = []
for (const t of TASKS) {
  phase(t.phase)
  let build = await agent(
    `You are the BUILDER for plan Task ${t.n} of the media cut-1 build.\n${LAWS}\nTask-specific guidance: ${t.hint}\n\nExecute plan Task ${t.n} (the section '## Task ${t.n}:' in ${PLAN}) step by step, checking off each step mentally. Run every command yourself and observe its output. Commit exactly as the plan specifies. Prior tasks' reports for context: ${JSON.stringify(results.slice(-2))}\n\nYour final output is the structured build report.`,
    { label: `build:T${t.n}`, phase: t.phase, schema: BUILD_SCHEMA },
  )
  if (!build) { log(`T${t.n}: builder died`); return { failed_at: t.n, results } }

  let verdict = null
  for (let round = 0; round < 3; round++) {
    verdict = await agent(
      `You are the INDEPENDENT VERIFIER for plan Task ${t.n} of the media cut-1 build. Trust NOTHING the builder claimed — subagents have fabricated test results in this project before.\n${LAWS}\n\nThe builder reported: ${JSON.stringify(build)}\n\nVerify independently:\n1. git log/show the claimed commits — do they exist, touch only the plan's files, carry the exact trailer, and stage nothing forbidden (.claude/, CLAUDE.md, AGENTS.md, .mcp.json, .gitignore, .filigree.conf, runs/)?\n2. Re-run the task's decisive tests YOURSELF (cargo test for the touched crate at minimum; clippy --all-targets -- -D warnings for code tasks) and quote the observed output.\n3. grep the four golden literals + HASH_FORMAT_VERSION; confirm they match the plan's expectation FOR THIS TASK (unchanged, except re-pins in Tasks 4/5 which must differ from the old values and be accompanied by the print_golden discipline).\n4. Check the task's checkboxes against reality (files exist, tests named in the plan exist and pass, recorded numbers present where the plan says RECORD).\n5. Spot-check one substantive claim adversarially (e.g. run the new test with the fix reverted in your head / read the diff for the actual logic).\nWorking tree must be left clean of unintended changes (git status; untracked runs/ and .filigree artifacts are expected and fine).\nReport the structured verdict. pass=true ONLY if you observed everything green.`,
      { label: `verify:T${t.n}:r${round}`, phase: t.phase, schema: VERDICT_SCHEMA },
    )
    if (!verdict) { log(`T${t.n}: verifier died, retrying once`); continue }
    if (verdict.pass) break
    log(`T${t.n} round ${round}: verifier found ${verdict.critical.length} critical issue(s)`)
    if (round === 2) break
    build = await agent(
      `You are the FIXER for plan Task ${t.n} of the media cut-1 build.\n${LAWS}\nTask-specific guidance: ${t.hint}\n\nThe independent verifier REJECTED the task with these critical issues:\n${JSON.stringify(verdict.critical, null, 2)}\nVerifier summary: ${verdict.summary}\nPrior build report: ${JSON.stringify(build)}\n\nFix every critical issue properly (amend or follow-up commits per the same git discipline; if a golden was mis-derived, re-derive via the ignored print tests). Run the tests yourself. Report the structured build report covering the FIXED state.`,
      { label: `fix:T${t.n}:r${round}`, phase: t.phase, schema: BUILD_SCHEMA },
    )
    if (!build) { log(`T${t.n}: fixer died`); return { failed_at: t.n, results, verdict } }
  }
  results.push({ task: t.n, build, verdict })
  if (!verdict || !verdict.pass) {
    log(`T${t.n}: NOT verified after fix rounds — stopping for main-loop intervention`)
    return { failed_at: t.n, results }
  }
  log(`T${t.n} verified: ${verdict.summary.slice(0, 120)}`)
}
return { failed_at: null, results }