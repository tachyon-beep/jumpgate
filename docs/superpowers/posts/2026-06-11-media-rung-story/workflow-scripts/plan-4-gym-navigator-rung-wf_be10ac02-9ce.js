export const meta = {
  name: 'plan-4-gym-navigator-rung',
  description: 'Execute Plan 4 (Tasks 16-18: frame-relative obs, JumpgateEnv PyO3, gym wrapper + determinism test) TDD, then two independent verifiers re-run the maturin+pytest gate',
  phases: [
    { title: 'Implement', detail: 'sequential implementer, Tasks 16-18, commit-per-task on green; reconcile plan drift + PyO3 linkage' },
    { title: 'Verify', detail: 'determinism-reviewer (Tier-B through binding) + adversarial skeptic (clean maturin rebuild + pytest)' },
  ],
}

const IMPL_SCHEMA = {
  type: 'object', additionalProperties: false,
  required: ['tasks_completed','files_changed','commits','core_untouched','deviations_from_plan','maturin_build','pytest_result','determinism_test','obs_dim','action_dim','cargo_py_tests','clippy','env_throughput_note','blocked','blocker_detail','notes'],
  properties: {
    tasks_completed: { type: 'array', items: { type: 'integer' } },
    files_changed: { type: 'array', items: { type: 'string' } },
    commits: { type: 'array', items: { type: 'object', additionalProperties: false, required: ['task','sha','message'], properties: { task: {type:'integer'}, sha: {type:'string'}, message: {type:'string'} } } },
    core_untouched: { type: 'boolean', description: 'jumpgate-core NOT modified (Plan-4 is py + python only)' },
    deviations_from_plan: { type: 'string', description: 'esp. the reset->Result fix and any PyO3 linkage resolution' },
    maturin_build: { type: 'string', description: 'raw result of maturin develop --release' },
    pytest_result: { type: 'string', description: 'e.g. "6 passed"' },
    determinism_test: { type: 'string', description: 'test_reset_is_deterministic + test_same_seed_bit_identical_obs_sequence result' },
    obs_dim: { type: 'integer' },
    action_dim: { type: 'integer' },
    cargo_py_tests: { type: 'string', description: 'cargo test -p jumpgate-py result, or note if extension-module linkage prevented it' },
    clippy: { type: 'string' },
    env_throughput_note: { type: 'string', description: 'any steps/sec observation if measurable (RAID R6); "unmeasured" if not' },
    blocked: { type: 'boolean' },
    blocker_detail: { type: 'string' },
    notes: { type: 'string' },
  },
}

const VERDICT_SCHEMA = {
  type: 'object', additionalProperties: false,
  required: ['verdict','maturin_release_build_ok','import_ok','pytest_passed','same_seed_reproducible','core_goldens_intact','obs_frame_relative_guard_present','obs_copy_semantics_ok','refutation_attempts','evidence'],
  properties: {
    verdict: { type: 'string', enum: ['CONFIRM','REFUTE'] },
    maturin_release_build_ok: { type: 'boolean' },
    import_ok: { type: 'boolean' },
    pytest_passed: { type: 'string' },
    same_seed_reproducible: { type: 'boolean' },
    core_goldens_intact: { type: 'boolean' },
    obs_frame_relative_guard_present: { type: 'boolean' },
    obs_copy_semantics_ok: { type: 'boolean', description: 'wrapper returns obs.copy() so the determinism test is not vacuous' },
    refutation_attempts: { type: 'string' },
    evidence: { type: 'string' },
  },
}

const IMPL_PROMPT = `You are executing **jumpgate v1 Plan 4 — the gym observation surface (the NAVIGATOR FIRST TRAINABLE RUNG)** in the jumpgate workspace (cwd /home/john/jumpgate, branch jumpgate-v1-design). This proves an agent can EXIST and be TRAINED (PDR-0002 first rung) — physics-only obs/action/reward; it is NOT the ecosystem gym.

READ FIRST and follow EXACTLY, task by task: \`docs/superpowers/plans/2026-06-08-jumpgate-v1-plan-4-observation-gym.md\` (Tasks 16, 17, 18), each strict TDD (write failing test → confirm fail → minimal impl → confirm pass). The plan gives complete code for every step.

CRITICAL DRIFT — the plan was written 2026-06-08, BEFORE the prelude/guidance/Plan-A landings. Reconcile:
1. **\`World::reset\` now returns \`Result<(World, ConfigHash), ResetError>\`** (guidance landed). The plan's env.rs code \`let (w, _hash) = World::reset(cfg);\` (Task 17 Step 5, in both \`new()\` and \`reset()\`) will NOT compile — change BOTH to \`let (w, _hash) = World::reset(cfg).expect("resolvable cfg");\`.
2. Core symbols are otherwise stable: it is \`CraftStore\` not \`ShipStore\`; \`effective_params\` is now 2-arg but Plan-4 does NOT call it directly (it reads \`view.craft_fuel_capacity(ego)\` via StateView — the §5.5 seam, still identity in v1).
3. VERIFY exact \`View\`/\`StateView\` accessor names by reading crates/jumpgate-core/src/{contract.rs,world.rs} before wiring the gatherer (\`craft_pos\`/\`craft_vel\`/\`craft_fuel\`/\`craft_fuel_capacity\`/\`craft_ids\`/\`recent_events\`/\`tick\`/\`dt\`/\`project\`/\`FullObserver\`). If a name differs, rebind ONLY \`write_obs_frame_relative\` — never \`write_obs_parts\` or the tests. Confirm \`EventKind::Arrival\` field name via the compiler (the plan matches \`Arrival { craft: c, .. }\`).

PyO3 LINKAGE GOTCHA (the plan flags it): \`cargo test -p jumpgate-py\` may FAIL TO LINK on Linux because \`extension-module\` suppresses libpython. If this blocks the cargo red/green for the pure fns (write_obs_parts / decode_action / compute_reward), resolve PRAGMATICALLY — the standard fix is to make \`extension-module\` a NON-default Cargo.toml feature that maturin enables via the pyproject \`features = ["pyo3/extension-module"]\` (already in the plan's pyproject Step 1), so \`cargo test\` links while the maturin build stays correct. If you do this, RECORD it in deviations_from_plan (it contradicts the plan's "keep always-on" note — but an unlinkable test cannot be a TDD gate). Do NOT get stuck; do NOT silently weaken any test.

DO NOT MODIFY jumpgate-core. Plan-4 lives in crates/jumpgate-py/ + python/ only. If you believe you must touch core, STOP, set blocked=true, and report why (it would risk the determinism goldens).

TOOLCHAIN (exact):
- Build: \`/home/john/jumpgate/archive/.venv/bin/python -m maturin develop --release --manifest-path /home/john/jumpgate/crates/jumpgate-py/Cargo.toml\`  (--release is LOAD-BEARING: Tier-B FP profile must reach the cdylib, spec §6).
- Test: \`/home/john/jumpgate/archive/.venv/bin/python -m pytest /home/john/jumpgate/python/tests/test_gym_smoke.py -q\`
- Import smoke: \`/home/john/jumpgate/archive/.venv/bin/python -c "import jumpgate._native as n; e=n.JumpgateEnv(2,1); print(e.obs_dim, e.action_dim)"\`

THE REAL GATE (capture RAW output):
- maturin develop --release succeeds (exit 0, "Installed jumpgate").
- \`pytest test_gym_smoke.py -q\` → **6 passed**, including \`test_reset_is_deterministic\` AND \`test_same_seed_bit_identical_obs_sequence\` (Tier-B reproducibility through the binding). If a determinism test fails, that is a REAL Tier-B break — bisect with core state_hash, do NOT weaken the test.
- \`cargo clippy -p jumpgate-py --all-targets\` clean.
- core untouched: \`git diff --stat\` shows NO crates/jumpgate-core changes.

If you can cheaply observe env steps/sec during the determinism run, note it (RAID R6 — the RL bottleneck); otherwise "unmeasured".

COMMIT per task with the plan's exact commit messages (they already include the Co-Authored-By trailer), ONLY when that task's gate is green. A separate verification phase re-runs maturin + pytest independently — do NOT fabricate; paste real command output. Return the structured result with real commit shas.`

const DET_PROMPT = `Independently verify that **jumpgate Plan 4 (gym navigator rung)** — just implemented on branch jumpgate-v1-design, cwd /home/john/jumpgate — reproduces DETERMINISTICALLY through the binding (Tier B) and preserves core determinism. Do NOT trust the implementer.

The claim: \`JumpgateEnv\` (PyO3) + the gym wrapper produce a bit-identical obs sequence for a fixed seed + fixed action stream, frame-relative obs (§7.2), with core untouched.

Verify WITH EVIDENCE (run commands; use /home/john/jumpgate/archive/.venv/bin/python):
1. The extension is built: re-run \`... -m pytest /home/john/jumpgate/python/tests/test_gym_smoke.py -q\` (the implementer's --release build is installed). Capture "N passed". Confirm \`test_reset_is_deterministic\` and \`test_same_seed_bit_identical_obs_sequence\` PASS specifically (run them with -v).
2. Confirm the wrapper returns \`self._obs_buf.copy()\` in BOTH reset and step (read python/jumpgate/gym_env.py) — if it returns the live buffer, the determinism test is VACUOUS (every entry aliases one mutated buffer). This is the single most important check.
3. Confirm the §7.2 frame-relative guard exists in crates/jumpgate-py/src/obs.rs (the debug_assert that no absolute ~AU coordinate crosses the f32 boundary; \`rel_to_f32\`).
4. Confirm jumpgate-core is UNTOUCHED: \`git diff --stat 5fe2e7e..HEAD -- crates/jumpgate-core\` is empty, and \`cargo test -p jumpgate-core golden\` still passes (both state goldens 0xf0dd_a1ba_f433_3735 / 0x532d_07bf_95a2_abc5).
5. Spot the determinism mechanism: same-seed reproducibility rests on the core being deterministic and the obs being a pure function of state; sanity-check the obs is genuinely frame-relative (not leaking absolute coords).
Actively try to REFUTE "deterministic through the binding". Set verdict=REFUTE if any check fails. Return the structured verdict.`

const SKEPTIC_PROMPT = `Adversarial re-verification of **Plan 4 (gym navigator rung)** on branch jumpgate-v1-design, cwd /home/john/jumpgate. ASSUME the implementer fabricated its green gates; trust only output you produce yourself. Use /home/john/jumpgate/archive/.venv/bin/python.

1. CLEAN REBUILD to defeat any cached/stale install: \`... -m maturin develop --release --manifest-path /home/john/jumpgate/crates/jumpgate-py/Cargo.toml\` — capture the tail (must end "Installed jumpgate", exit 0).
2. Import + construct: \`... -c "from jumpgate._native import JumpgateEnv; e=JumpgateEnv(1,1); print(e.obs_dim, e.action_dim)"\` — capture the printed dims.
3. Full pytest: \`... -m pytest /home/john/jumpgate/python/tests/test_gym_smoke.py -q\` — record the "N passed" line (expect 6 passed).
4. \`cargo clippy -p jumpgate-py --all-targets\` — paste the tail (clean?).
5. \`git status --porcelain\` (working tree clean except known infra: .gitignore/.claude/.filigree.conf/.mcp.json/AGENTS.md/CLAUDE.md) and \`git log --oneline -5\` (Plan-4 commits present). Confirm \`git diff --stat 5fe2e7e..HEAD -- crates/jumpgate-core\` is EMPTY (core untouched).
Report verdict=CONFIRM only if YOU personally saw the maturin build succeed AND 6 pytest pass AND core untouched. Return the structured verdict.`

phase('Implement')
const impl = await agent(IMPL_PROMPT, { label: 'plan-4-impl', schema: IMPL_SCHEMA })

if (!impl || impl.blocked) {
  log(`Implementer halted (blocked=${impl?.blocked}); skipping verification for human review. Detail: ${impl?.blocker_detail || 'n/a'}`)
  return { impl, det: null, skeptic: null, halted: true }
}
if (!impl.core_untouched) {
  log(`WARNING: implementer reports jumpgate-core was modified — verifiers will check goldens, flagging for human review.`)
}

phase('Verify')
const det = await agent(DET_PROMPT, { label: 'determinism-review', agentType: 'axiom-determinism-and-replay:determinism-reviewer', schema: VERDICT_SCHEMA })
const skeptic = await agent(SKEPTIC_PROMPT, { label: 'adversarial-gate', schema: VERDICT_SCHEMA })

return { impl, det, skeptic, halted: false }