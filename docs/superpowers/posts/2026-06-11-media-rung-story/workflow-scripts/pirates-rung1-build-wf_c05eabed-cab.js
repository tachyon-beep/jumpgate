export const meta = {
  name: 'pirates-rung1-build',
  description: 'Build pirates rung 1 per the approved spec/plan: sequential TDD tasks, each independently verified',
  phases: [
    { title: 'Build', detail: 'tasks T0-T7 built sequentially, TDD, committed' },
    { title: 'Verify', detail: 'independent gate-check after every task' },
  ],
}

const REPO = '/home/john/jumpgate'
const SPEC = `${REPO}/docs/superpowers/specs/2026-06-10-pirates-rung1-predation-and-upgrades-design.md`
const PLAN = `${REPO}/docs/superpowers/plans/2026-06-10-pirates-rung1-implementation.md`

const LAWS = `
NON-NEGOTIABLE PROJECT LAWS:
- Read THE SPEC (${SPEC}) and THE PLAN (${PLAN}) before touching anything. The plan's
  "Project laws" block binds you. Do ONLY your assigned task; prior tasks are already
  committed on this branch (check git log).
- Goldens: HASH_FORMAT_VERSION / GOLDEN_ZERO_STATE_HASH / GOLDEN_CONFIG_HASH move ONLY
  in their designated single-cause task, re-derived via the ignored print_golden tests —
  NEVER invented, NEVER copied from the plan or spec. Every other task must leave them
  bit-identical and must verify so (grep) before committing.
- TDD: write the failing test first, see it fail, implement, see it pass. For mechanic
  tests, prove discrimination by temp-revert where the plan says so.
- Gates before every commit: cargo test --workspace green; cargo clippy --all-targets
  -- -D warnings clean (NOT --lib, it is a no-op here); pytest only when py surfaces change.
- Rust 2024 (gen is reserved). Saturating/checked arithmetic on all new
  strength/level/credit math (spec section 8 totality discipline).
- git: NEVER 'git add -A' or '.'; stage explicit paths only; NEVER stage .gitignore,
  .claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf, runs/. git stash is
  operator-blocked (make WIP commits instead). Commit messages containing parentheses
  must be committed via 'git commit -F -' heredoc. Every commit ends with the trailer:
  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
- If the plan conflicts with code reality, adapt MINIMALLY, follow existing idioms, and
  note the deviation in the commit body. Do not expand scope. Do not "fix" neighboring
  code; incidental findings go in your final report, not into the diff.
- Your final message is a machine-read report, not prose for a human: state exactly what
  you committed (hashes), what you verified with which commands, and any deviations.
  Do NOT claim a gate passed unless you ran it and saw the output this session.`

const TASKS = [
  { id: 'T0', planTask: 'Task 0 (P0)', label: 'instrument',
    note: 'diagnostics.rs + classify (4-corner synthetic tests) + examples/trophic_run.rs over the CURRENT world. Goldens untouched. RECORD the measured laden_trips_per_window number from step 0.5 in your report — Task 6 calibration needs it.' },
  { id: 'T1', planTask: 'Task 1 (Commit A)', label: 'config-surface',
    note: 'THE config-golden single-cause task: only GOLDEN_CONFIG_HASH re-pins (re-derived via the ignored printer); both state goldens and HASH_FORMAT_VERSION must remain untouched. RunConfig tail append only — never reorder. The exhaustive destructure makes omissions a compile error; use it.' },
  { id: 'T2', planTask: 'Task 2 (Commit B)', label: 'state-v4',
    note: 'THE state-golden single-cause task: HASH_FORMAT_VERSION 3->4 + GOLDEN_ZERO_STATE_HASH + the manual zero-fold re-pinned (re-derived). GOLDEN_CONFIG_HASH must keep exactly the value Task 1 pinned. UpgradeLevels{hulls,escorts} is a FLEET LEDGER (counts of un-simulated ships) — carry the spec section 6 doc comment verbatim. Include the Class-3 transitively-pinned doc paragraph for the Piracy stream cursor.' },
  { id: 'T3', planTask: 'Task 3 (Commit C)', label: 'purchase-verb',
    note: 'Goldens FROZEN from here on (grep both before committing). Per-arm exact-integer tests are the point — the identities cannot catch wrong-price bugs. Fix the stale economy.rs:804-807 comment as the drive-by the plan names.' },
  { id: 'T4', planTask: 'Task 4 (Commit D)', label: 'encounters',
    note: 'First runtime RngStream::Piracy draws. Stage order 3b -> 3b2 -> 3b3 -> 3c is load-bearing (dock sanctuary at destination; rob-on-load legal at origin). Engagement emission sites log the kinematic snapshot (rel bearing + speed) per spec section 2. Temp-revert proof on escort_threshold_is_a_step.' },
  { id: 'T5', planTask: 'Task 5 (Commit E)', label: 'brains-evidence',
    note: 'route_evidence accessor takes the READER (media seam). Relocation fn signature must not accept traffic data (enforce dumbness by construction). Every scripted stage skips !scripted craft — grep-audit before commit.' },
  { id: 'T6', planTask: 'Task 6 (Commit F)', label: 'lab-scenario',
    note: 'scenario_trophic + sweep_trophic.py + the 50k-tick zero-FuelEmpty endurance window + the positive control (reach=inf MUST classify RiskEqualized — if it does not, STOP and report; do not tune around a broken instrument). Calibrate the food band from T0\'s measured laden_trips_per_window via spec section 4 formulas; show the arithmetic in the commit body. Do NOT do the console tuning loop (step 6.6) — that is owner-in-the-loop work after this workflow.' },
  { id: 'T7', planTask: 'Task 7 (Commit G)', label: 'gym',
    note: 'Steps 7.1, 7.2 and 7.4 only — SKIP step 7.3 (PPO training/ablation; the main loop runs it after verification). Action space stays Discrete(5); num_pirates=0 must leave every existing pytest AND the keystone learning smoke byte-identical (run the full python test suite).' },
]

function builderPrompt(t) {
  return `You are the builder for ${t.planTask} of the pirates-rung-1 plan in ${REPO} (branch jumpgate-v1-design).
${LAWS}
YOUR TASK: implement ${t.planTask} ("${t.label}") exactly as written in THE PLAN, informed by THE SPEC.
TASK-SPECIFIC NOTES: ${t.note}
Work through every checkbox step in order. Commit as the plan directs (single-cause discipline for golden tasks). Then report.`
}

function verifierPrompt(t, builderReport) {
  return `You are the INDEPENDENT verifier for ${t.planTask} ("${t.label}") of the pirates-rung-1 build in ${REPO}. Builders' reports are claims, not facts — re-run everything yourself.
${LAWS}
THE BUILDER REPORTED:
${builderReport}

VERIFY (run these yourself, capture real output):
1. git log --oneline -8 and git status --porcelain: the task's commits exist; working tree clean; NO forbidden files staged/committed (.claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf, .gitignore, runs/).
2. cargo test --workspace — green.
3. cargo clippy --all-targets -- -D warnings — clean.
4. Goldens discipline for THIS task: ${t.id === 'T1' ? 'GOLDEN_CONFIG_HASH changed in exactly one commit whose diff touches nothing unrelated; HASH_FORMAT_VERSION still 3; GOLDEN_ZERO_STATE_HASH still 0x1d44_b373_5ccd_33f7.' : t.id === 'T2' ? 'HASH_FORMAT_VERSION now 4 and GOLDEN_ZERO_STATE_HASH re-pinned in exactly one commit; GOLDEN_CONFIG_HASH unchanged from Task 1\'s value (git log -p the constant to confirm).' : 'grep proves HASH_FORMAT_VERSION and both GOLDEN constants are bit-identical to the previous task\'s state (git diff HEAD~N on the constants).'}
5. The task's NEW tests exist and are discriminating: pick the most load-bearing one, temporarily break its mechanic (in your working copy only), confirm the test FAILS, restore (git checkout -- the file), confirm green again.
6. If python surfaces changed: pytest python/tests -x.
Return verdict pass/fail with the actual command outputs that justify it. If fail: name the exact broken thing and the minimal fix.`
}

const VERDICT = {
  type: 'object', required: ['pass', 'evidence', 'failures'],
  properties: {
    pass: { type: 'boolean' },
    evidence: { type: 'string', description: 'commands run + key output lines' },
    failures: { type: 'array', items: { type: 'string' }, description: 'empty if pass; else exact broken things + minimal fixes' },
  },
}

const results = []
for (const t of TASKS) {
  phase('Build')
  log(`building ${t.id}: ${t.label}`)
  const report = await agent(builderPrompt(t), { label: `build:${t.id}`, phase: 'Build' })
  if (!report) throw new Error(`${t.id} builder died`)

  let verdict = await agent(verifierPrompt(t, report), { label: `verify:${t.id}`, phase: 'Verify', schema: VERDICT })
  if (!verdict) throw new Error(`${t.id} verifier died`)

  if (!verdict.pass) {
    log(`${t.id} FAILED verification — one repair attempt: ${verdict.failures.join(' | ')}`)
    const repair = await agent(
      `You are the repair agent for ${t.planTask} of the pirates-rung-1 build in ${REPO}.
${LAWS}
The independent verifier FAILED the task. Failures:\n${verdict.failures.join('\n')}\n\nVerifier evidence:\n${verdict.evidence}\n\nFix EXACTLY these failures, minimally, re-run the gates yourself, amend or add commits as appropriate (never rewrite the golden single-cause commits' causes), and report.`,
      { label: `repair:${t.id}`, phase: 'Build' })
    verdict = await agent(verifierPrompt(t, repair || 'repair agent died'), { label: `reverify:${t.id}`, phase: 'Verify', schema: VERDICT })
    if (!verdict || !verdict.pass) {
      results.push({ task: t.id, status: 'FAILED', detail: verdict ? verdict.failures : ['reverifier died'] })
      log(`${t.id} still failing after repair — ABORTING chain (later tasks depend on it)`)
      return { aborted_at: t.id, results }
    }
  }
  results.push({ task: t.id, status: 'ok', report: report.slice(0, 2000), evidence: verdict.evidence.slice(0, 1500) })
  log(`${t.id} verified ✓`)
}

return { aborted_at: null, results }