export const meta = {
  name: 'tactical-flight-rung1',
  description: 'Build the tactical-flight Rung-1 stack: core Thrust command, thrust-mode gym env, SB3 PPO training + curriculum + renderer',
  phases: [
    { title: 'Foundations', detail: 'T0 docs banners + T1 core Thrust + T2 obs writer (parallel)' },
    { title: 'Env', detail: 'T3 thrust-mode env + python wrapper' },
    { title: 'Bind', detail: 'T4 rebuild native + python behaviour tests' },
    { title: 'TrainStack', detail: 'T5 PPO+curriculum+learning-smoke, T6 renderer' },
    { title: 'FinalVerify', detail: 'independent re-run of every gate' },
  ],
}

const PLAN = 'docs/superpowers/plans/2026-06-10-tactical-flight-rung1-implementation.md'
const SPEC = 'docs/superpowers/specs/2026-06-10-tactical-flight-rung1-design.md'

const COMMON = `Working dir /home/john/jumpgate (branch jumpgate-v1-design). Read the plan at ${PLAN} (and spec at ${SPEC}) FIRST - the plan contains complete code; follow it exactly, adapting only where the tree differs from the plan's "Ground truth" notes. KNOWN DRIFT: a parked trophic WIP commit already moved HASH_FORMAT_VERSION to 3 and re-pinned GOLDEN_ZERO_STATE_HASH to 0x1d44_b373_5ccd_33f7, and CraftStore already has risk_appetite/pirate columns plus EventKind/RngStream additions. Treat EVERY currently-pinned golden constant as IMMUTABLE: if a golden test fails after your change, you broke an existing encoding - fix your code, NEVER edit a golden constant, and report golden_constants_touched honestly.
RULES: strict TDD (write failing test, run it to see it fail, implement, run green). Do NOT git commit, git add, or git stash anything (the main loop owns commits; stash is operator-blocked). Do not touch files outside your task's file list. Rust 2024: 'gen' is a reserved keyword. Report ACTUAL command outputs (paste the test-result lines); your final message IS the structured report, not prose for a human.`

const IMPL = {
  type: 'object',
  properties: {
    files_changed: { type: 'array', items: { type: 'string' } },
    tests_run: { type: 'array', items: { type: 'string' } },
    all_green: { type: 'boolean' },
    golden_constants_touched: { type: 'boolean' },
    blockers: { type: 'array', items: { type: 'string' } },
    notes: { type: 'string' },
  },
  required: ['files_changed', 'all_green', 'golden_constants_touched', 'notes'],
}

const VERIFY = {
  type: 'object',
  properties: {
    pass: { type: 'boolean' },
    failures: { type: 'string' },
    golden_diff: { type: 'string' },
    notes: { type: 'string' },
  },
  required: ['pass', 'failures'],
}

async function verified(phaseTitle, verifyPrompt, repairHint) {
  let v = await agent(verifyPrompt, { label: 'verify', phase: phaseTitle, schema: VERIFY })
  let rounds = 0
  while ((!v || !v.pass) && rounds < 2) {
    rounds++
    log(`${phaseTitle}: verification failed (round ${rounds}) - dispatching repair`)
    await agent(
      `${COMMON}

REPAIR TASK for phase "${phaseTitle}". An independent verification failed.
Failure report:
${v ? v.failures : '(verifier died - re-run the phase gates yourself and fix what fails)'}
${repairHint}
Fix the actual defect. NEVER weaken a test assertion, never raise/lower a threshold to pass, never edit a golden constant. Re-run the failing commands until genuinely green.`,
      { label: `repair-${rounds}`, phase: phaseTitle, schema: IMPL },
    )
    v = await agent(verifyPrompt, { label: `reverify-${rounds}`, phase: phaseTitle, schema: VERIFY })
  }
  return v
}

// ---------- Phase: Foundations (T0 + T1 + T2 in parallel; disjoint files) ----------
phase('Foundations')
const [t0, t1, t2] = await parallel([
  () => agent(
    `${COMMON}

Implement Task 0 ONLY (deferral banners on the two trophic docs). Edit exactly the two files named in the task, prepending the exact banner block from the plan directly under each title line. No other changes. tests_run may be empty.`,
    { label: 'T0:docs', phase: 'Foundations', schema: IMPL },
  ),
  () => agent(
    `${COMMON}

Implement Task 1 ONLY: CommandKind::Thrust -> NavState::DirectThrust. The plan gives the complete change-set (types.rs, stores.rs, autopilot.rs pass-through arm, both ingest paths, world.rs dest-resolution arm, hash.rs folds with NEW tags only) and the exact tests including the two world.rs end-to-end tests and thrust_mode_record_then_replay_is_bit_identical. The compiler's exhaustive-match errors are your worklist - fix every site it names (the trophic WIP may have added match sites the plan didn't list). Gate: cargo test -p jumpgate-core AND cargo clippy --all-targets -p jumpgate-core green/clean with ZERO golden-constant edits (run: git diff crates/jumpgate-core/src/hash.rs and confirm no +/- line touches a GOLDEN or 0x... pinned constant; paste that confirmation in notes).`,
    { label: 'T1:core-thrust', phase: 'Foundations', schema: IMPL },
  ),
  () => agent(
    `${COMMON}

Implement Task 2 ONLY: the 10-dim scaled thrust-mode obs writer in crates/jumpgate-py/src/obs.rs (THRUST_OBS_DIM, VEL_SCALE, write_obs_thrust_mode + the two tests, complete code in the plan). Touch ONLY obs.rs. Gate: cargo test -p jumpgate-py green (note: if jumpgate-py fails to COMPILE for reasons outside obs.rs - e.g. core drift - report it as a blocker rather than fixing other files).`,
    { label: 'T2:obs-writer', phase: 'Foundations', schema: IMPL },
  ),
])

const v1 = await verified(
  'Foundations',
  `Independently verify the working tree at /home/john/jumpgate (do NOT edit code; run commands, report facts).
1. cargo test -p jumpgate-core 2>&1 | tail -25 -> pass requires 0 failed
2. cargo test -p jumpgate-py 2>&1 | tail -15 -> 0 failed
3. cargo clippy --all-targets 2>&1 | tail -10 -> no warnings/errors
4. git diff -- crates/jumpgate-core/src/hash.rs | grep -E '^[+-].*(GOLDEN|0x[0-9a-f_]{8})' -> put the output (or 'empty') in golden_diff; pass requires NO changed pinned-constant lines (new tag constants for the new variant are acceptable ONLY if clearly additive '+' lines, never '-' lines on existing constants)
5. grep -l 'DEFERRED (2026-06-10' docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md docs/superpowers/plans/2026-06-10-trophic-cut-1-implementation.md -> both files must match
6. cargo test -p jumpgate-core thrust_mode_record_then_replay 2>&1 | grep 'test result' -> the new replay test exists and passes
Your final message IS the structured report.`,
  'Likely defects: a missed exhaustive-match site, a hash fold that renumbered existing tags (must append, never renumber), or the world.rs end-to-end test fixture not satisfying the reset guard (mirror reset_accepts_resolvable_thrusting_craft).',
)
if (!v1 || !v1.pass) {
  return { halted: 'Foundations failed verification after 2 repair rounds', v1, t0, t1, t2 }
}

// ---------- Phase: Env (T3, sequential - depends on T1+T2) ----------
phase('Env')
const t3 = await agent(
  `${COMMON}

Implement Task 3 ONLY: the thrust-mode env (FlightCfg, flight_reward, is_arrival, draw_target, JumpgateEnv wiring: control_mode/configure()/reset/step with auto-reset, mode-aware dims) in crates/jumpgate-py/src/env.rs, plus the python wrapper changes in python/jumpgate/gym_env.py (mode ctor arg, set_difficulty, is_success in info). The plan has complete code + the four unit tests.
RNG NOTE (overrides the plan's rand::Rng sketch): the workspace pins the rand 0.10 family exactly for determinism (read the comment at the top of /home/john/jumpgate/Cargo.toml). For draw_target, use jumpgate_core's RngStreams/RngStream::Scenario (add them to core's lib.rs pub-use list if not already exported - additive only) and generate uniforms from rand_core::RngCore::next_u64 mapped to f64 via ((x >> 11) as f64) * (1.0 / (1u64 << 53) as f64) - do NOT add the 'rand' crate to jumpgate-py; if a trait import is needed, depend on the workspace-pinned rand_core only.
Gate: cargo test -p jumpgate-py AND cargo clippy --all-targets -p jumpgate-py green/clean; existing waypoint-mode tests still pass.`,
  { label: 'T3:env-thrust-mode', phase: 'Env', schema: IMPL },
)

const v2 = await verified(
  'Env',
  `Independently verify /home/john/jumpgate (no code edits):
1. cargo test -p jumpgate-py 2>&1 | tail -20 -> 0 failed, and the flight_reward / is_arrival / draw_target / thrust_obs tests all appear
2. cargo clippy --all-targets 2>&1 | tail -10 -> clean
3. cargo test -p jumpgate-core 2>&1 | tail -5 -> still 0 failed (core untouched or only additive pub-use)
4. grep -n 'is_success' python/jumpgate/gym_env.py -> present
Your final message IS the structured report.`,
  'Likely defects: rand_core API mismatch (use next_u64 mapping, not random_range), borrow conflicts in step() when reading world state for reward (copy values out first, mirroring the arrivals pattern at env.rs:230), or forgetting auto-reset on terminated||truncated.',
)
if (!v2 || !v2.pass) {
  return { halted: 'Env failed verification after 2 repair rounds', v2, t3 }
}

// ---------- Phase: Bind (T4 - rebuild native + python tests) ----------
phase('Bind')
const t4 = await agent(
  `${COMMON}

Implement Task 4 ONLY: rebuild the native module and write+run the python behaviour tests.
Rebuild: try 'cd crates/jumpgate-py && maturin develop --release'; if it refuses (no venv), use 'maturin build --release' then 'unzip -o ../../target/wheels/jumpgate-*.whl jumpgate/_native.abi3.so -d ../../python/' (this matches the existing python/jumpgate/_native.abi3.so layout; confirm the .so mtime changed).
Then create python/tests/test_thrust_mode.py exactly per the plan and run:
PYTHONPATH=python python3 -m pytest python/tests/test_thrust_mode.py python/tests/test_gym_smoke.py -v
Gate: ALL pass (old waypoint smoke must still pass - default mode is waypoint). If a test fails, the defect may be in Task 3's env.rs - you may fix env.rs (then REBUILD the .so before re-running; a stale .so silently tests old code).`,
  { label: 'T4:bind+pytests', phase: 'Bind', schema: IMPL },
)

const v3 = await verified(
  'Bind',
  `Independently verify /home/john/jumpgate (no code edits):
1. ls -la python/jumpgate/_native.abi3.so -> report mtime (should be from today's rebuild)
2. PYTHONPATH=python python3 -m pytest python/tests/test_thrust_mode.py python/tests/test_gym_smoke.py -v 2>&1 | tail -25 -> ALL passed
3. cargo test -p jumpgate-py 2>&1 | tail -5 -> still green
Your final message IS the structured report.`,
  'If pytest fails but cargo tests pass, suspect a STALE .so (rebuild via maturin and re-extract) or a wrapper/native dim mismatch (obs/action buffer sizes must come from the post-configure getters).',
)
if (!v3 || !v3.pass) {
  return { halted: 'Bind failed verification after 2 repair rounds', v3, t4 }
}

// ---------- Phase: TrainStack (T5 + T6 in parallel; disjoint files) ----------
phase('TrainStack')
const [t5, t6] = await parallel([
  () => agent(
    `${COMMON}

Implement Task 5 ONLY: python/train/curriculum.py, python/train/train_flight.py, python/tests/test_learning_smoke.py - complete code in the plan. DEPS ARE ALREADY INSTALLED (torch 2.9.1+cu128 with CUDA, stable_baselines3 2.7.1) - do not pip install anything.
THEN RUN THE LEARNING SMOKE (the keystone): PYTHONPATH=python python3 -m pytest python/tests/test_learning_smoke.py -v 2>&1 | tail -15
It must GENUINELY pass (PPO closes mean final distance to < 0.7x random baseline after 40k steps; a few minutes on this GPU). If it fails: debug the WIRING (obs indices, reward sign/scale, action decode, VecEnv auto-reset, VecNormalize) - the 0.7 threshold and 40k step budget may NOT be weakened. You may tune PPO hyperparams (lr, n_steps, batch, ent_coef) and FlightCfg reward weights within the plan's spirit; report every change you make in notes. Also sanity-check the SB3 'env_method' reach-through noted in the plan (set_difficulty must land on the base env; if it does not, use the documented unwrap fallback).
Paste the actual pytest tail in notes. all_green=true ONLY if the smoke passed.`,
    { label: 'T5:train-stack', phase: 'TrainStack', schema: IMPL },
  ),
  () => agent(
    `${COMMON}

Implement Task 6 ONLY: python/train/render.py per the plan - and you MUST fix the plan's flagged pos-reconstruction lines: obs[4:7] is rel-pos SCALED by dist_scale (= the stage's target_dist_max passed to set_difficulty), so reconstruct pos = target_unscaled - nobs[4:7] * dist_scale, with target_unscaled = obs0[4:7] * dist_scale captured at reset; DELETE the dead 'if False' placeholder line entirely - final code must be clean.
Smoke it without waiting for training: create a throwaway untrained model in /tmp (python: PPO('MlpPolicy', <thrust env>).save('/tmp/untrained_flight')) then run: PYTHONPATH=python python3 python/train/render.py /tmp/untrained_flight.zip /tmp/traj_smoke.png -> the PNG must exist and the script must print both outcomes. Paste the actual output in notes.`,
    { label: 'T6:renderer', phase: 'TrainStack', schema: IMPL },
  ),
])

// ---------- Final independent verification of EVERYTHING ----------
phase('FinalVerify')
const vFinal = await verified(
  'FinalVerify',
  `Final independent verification of /home/john/jumpgate (no code edits). Run and report:
1. cargo test -p jumpgate-core 2>&1 | tail -6
2. cargo test -p jumpgate-py 2>&1 | tail -6
3. cargo clippy --all-targets 2>&1 | tail -6
4. PYTHONPATH=python python3 -m pytest python/tests/ -v -m "not slow" 2>&1 | tail -20
5. THE KEYSTONE - re-run the learning smoke yourself (do not trust prior reports): PYTHONPATH=python python3 -m pytest python/tests/test_learning_smoke.py -v 2>&1 | tail -12
6. ls -la /tmp/traj_smoke.png python/train/render.py python/train/train_flight.py python/train/curriculum.py
7. git diff -- crates/jumpgate-core/src/hash.rs | grep -E '^[-].*(GOLDEN|0x[0-9a-f_]{8})' -> must be empty (no removed pinned constants); put output in golden_diff
pass=true ONLY if every gate above is genuinely green. Paste the real tails in failures/notes.`,
  'Route the failure: cargo/golden issues -> core or obs code; pytest behaviour -> env.rs or wrapper (rebuild the .so after any env.rs fix); learning smoke -> reward/obs wiring or hyperparams (threshold may not be weakened).',
)

return {
  t0, t1, t2, t3, t4, t5, t6,
  foundations: v1, env: v2, bind: v3, final: vFinal,
  done: !!(vFinal && vFinal.pass),
}
