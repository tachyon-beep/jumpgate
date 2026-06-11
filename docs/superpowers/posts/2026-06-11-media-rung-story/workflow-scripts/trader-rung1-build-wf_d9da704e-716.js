export const meta = {
  name: 'trader-rung1-build',
  description: 'Build the trader rung: core gates, semi-MDP gym mode 2, wrapper, baselines, PPO smoke',
  phases: [
    { title: 'Core', detail: 'ASSIGN gate + read accessors (Plan Tasks 1-2)' },
    { title: 'NativeEnv', detail: 'trader template + obs + macro-step (Tasks 3-4)' },
    { title: 'Wrapper', detail: 'Discrete gym mode + wheel + tests (Task 5)' },
    { title: 'Training', detail: 'baselines + trainer + learning smoke (Tasks 6-7)' },
    { title: 'Review', detail: 'adversarial review of each landed phase' },
  ],
}

const CTX = `You are an implementation agent in /home/john/jumpgate (Rust workspace + python/). Branch jumpgate-v1-design.
READ FIRST, in order:
1. docs/superpowers/specs/2026-06-10-trader-rung1-haulage-design.md (the spec)
2. docs/superpowers/plans/2026-06-10-trader-rung1-haulage-implementation.md (the plan — your task numbers refer to it)
Then read the actual code files your task touches BEFORE editing (the plan says "mirror existing patterns" — that is mandatory, e.g. real field names, borrow patterns, test fixtures).

PROJECT LAWS (violating any of these is a failed task):
- GOLDENS UNTOUCHED: never edit crates/jumpgate-core/src/hash.rs fold order, HASH_FORMAT_VERSION (=3), GOLDEN_ZERO_STATE_HASH (0x1d44_b373_5ccd_33f7), GOLDEN_CONFIG_HASH, store SoA layouts, or World::step stage order. Never invent hash literals.
- Determinism: no wall-clock, no HashMap iteration order reaching state, all randomness seed-derived.
- Rust 2024 edition ('gen' is a reserved keyword). Lint: cargo clippy --all-targets (NOT --lib).
- git: NEVER 'git add -A' or 'git add .' — explicit file paths only. NEVER stage .gitignore, .claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf. Commit messages end with trailer line:
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  Use 'git commit -F -' heredoc when the message contains parentheses.
- Honesty: your report is re-verified by the main loop. Never claim a test ran if it did not; paste real output excerpts. If blocked, say so in 'blockers' rather than papering over.`

const REPORT = {
  type: 'object',
  required: ['task', 'commits', 'tests_run', 'blockers'],
  properties: {
    task: { type: 'string' },
    commits: { type: 'array', items: { type: 'string' }, description: 'SHA + subject per commit made' },
    files_changed: { type: 'array', items: { type: 'string' } },
    tests_run: { type: 'array', items: { type: 'object', required: ['cmd', 'result'], properties: { cmd: { type: 'string' }, result: { type: 'string', description: 'real pass/fail counts pasted from output' } } } },
    key_decisions: { type: 'array', items: { type: 'string' }, description: 'deviations from plan or calibration values chosen, with reasons' },
    blockers: { type: 'array', items: { type: 'string' } },
  },
}

const REVIEW = {
  type: 'object',
  required: ['verdict', 'findings'],
  properties: {
    verdict: { type: 'string', enum: ['clean', 'issues-found'] },
    findings: { type: 'array', items: { type: 'object', required: ['severity', 'where', 'issue'], properties: { severity: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] }, where: { type: 'string' }, issue: { type: 'string' }, suggested_fix: { type: 'string' } } } },
  },
}

// ---- Phase 1: core changes (sequential — small, same crate) ----
phase('Core')
const core = await agent(`${CTX}

YOUR TASK: Implement Plan Task 1 (ASSIGN gate: stagger_period==0 disables scripted acceptance in run_scripted_dispatch, REPOST unaffected; doc update on DispatchCfg::stagger_period) AND Plan Task 2 (four read-only World accessors: offered_contracts / station_pos / craft_credits / craft_is_idle) following the plan steps exactly (TDD: failing test first, then impl, then green).
The plan's code sketches use guessed field access — verify against the real code (world.rs private fields like 'tick' may need self.tick vs a method; eph.body_pos signature; ids.id_at / dense_index / index_of helpers) and adjust. Important correctness detail for offered_contracts: a contract is OFF the board if status != Offered, OR hauler is set, OR any craft holds accept-intent for it (ships.contract column points at it).
Run: cargo test -p jumpgate-core, cargo clippy --all-targets -- -D warnings (workspace-wide clippy must stay clean for the crates you touched).
Commit per plan (one commit per task, explicit paths, trailer).`, { label: 'impl:core-tasks-1-2', phase: 'Core', schema: REPORT })

if (core && core.blockers && core.blockers.length) log(`CORE BLOCKERS: ${core.blockers.join(' | ')}`)

// ---- Phase 2: native env (long) + core review in parallel ----
phase('NativeEnv')
const [native, coreReview] = await parallel([
  () => agent(`${CTX}

YOUR TASK: FINISH Plan Task 4 (Task 3 is ALREADY COMMITTED as 2622ad2 by a predecessor agent killed mid-Task-4 by a rate limit). CURRENT WIP STATE you inherit (verify with git status / git diff FIRST):
- Committed: core Tasks 1-2 (8445025, 72c6a78: accessors offered_contracts/station_pos/craft_credits/craft_is_idle; stagger_period==0 disables ASSIGN) + py Task 3 (2622ad2: trader template + mode-2 plumbing).
- UNCOMMITTED WIP: large env.rs + obs.rs diffs (the Task-4 obs writer + macro-step, partially done) AND a core crates/jumpgate-core/src/economy.rs fix to try_load (frame-mixing bug: craft pos is the tick t-1 pre-physics state but body_pos was sampled at tick t; a body on a fast inner orbit moves ~100x ARRIVAL_RADIUS per tick so the load gate starved forever; the fix samples body_pos at t-1). cargo test -p jumpgate-core is GREEN (182) with the fix applied; cargo test -p jumpgate-py is 22 passed / 1 FAILED: env::tests::trader_macro_step_accept_pays_delta_credits.
- Do NOT stage .gitignore (unrelated local changes). Do NOT revert the WIP wholesale — build on it.
YOUR STEPS, in order:
1. Independently VERIFY the try_load frame fix (read World::step stage order — resolve_contracts is pre-physics; check what frame the swept-arrival detection and autopilot resolve against). If correct: add a focused core regression test (fast-orbit body + perfectly co-located hauler that fails to load WITHOUT the fix — prove by temporarily reverting the fix) and commit the core fix + its test SEPARATELY FIRST as 'fix(core): try_load co-location compares craft and body in the same frame' (single-cause discipline, trailer, -F heredoc). If the fix is WRONG, revert it and solve the real cause; either way record the verdict in key_decisions.
2. Diagnose the failing trader_macro_step_accept_pays_delta_credits: instrument the macro-step (eprintln + 'cargo test -- --nocapture', remove after) to find where the trip stalls (never loads? never arrives? horizon/fuel/stock?). Calibrate the template until the accept path genuinely pays; record final calibration values in key_decisions.
3. Complete the rest of Plan Task 4 (all three rust unit tests passing) per the instructions below.
Key invariants from the spec (section 5): one Python step = one decision; accept path runs world ticks until the craft is idle again or horizon; wait path advances exactly TRADER_WAIT_TICKS=8; reward = delta credits_micros/1e6; terminated always false, truncated at horizon; auto-reset mirrors the thrust-mode scheme (master_seed ^ episode_counter). Obs: TRADER_OBS_DIM=20, FIXED global scales from the plan constants, zeros for absent slots, station positions sampled at the CURRENT tick.
Scenario calibration: lift the craft spec from the world.rs forage-loop test fixture (find the test proving accept->load->deliver->pay; copy its BaseSpec numbers, do NOT invent). The reset anti-tunnel guard must pass (World::reset returns Ok). Make the rust unit tests from plan Task 4 Step 3 pass for real — especially trader_macro_step_accept_pays_delta_credits (this proves the whole loop end-to-end in Rust before Python ever sees it). If the macro-step never pays (autopilot too slow for horizon 2000, fuel too small, stock missing), CALIBRATE the template (exhaust_velocity / fuel / stock / horizon) until it does, and record the values in key_decisions.
Run: cargo test -p jumpgate-py, cargo test -p jumpgate-core (must stay green), cargo clippy --all-targets -- -D warnings.
Commit per plan (Task 3 commit, then Task 4 commit).`, { label: 'impl:native-env-3-4', phase: 'NativeEnv', schema: REPORT }),
  () => agent(`${CTX}

YOU ARE A READ-ONLY ADVERSARIAL REVIEWER (no edits, no commits). Review the two most recent core commits (git log, the ASSIGN gate + trader accessors). Hunt specifically for:
1. Determinism leaks (iteration order, float ops in new code paths).
2. Hash/golden impact: any change that could alter state_hash or config_hash for EXISTING configs (the gate must be hash-neutral because no existing config uses stagger_period 0).
3. ASSIGN-gate placement: does stagger_period==0 also accidentally skip REPOST or any later stage? Is the early-return placed AFTER the full REPOST loop?
4. offered_contracts correctness: O(n*m) intent scan — does it correctly hide a contract a craft holds intent on pre-resolve? Generation checks on dense_index?
5. Borrowing/perf landmines for a per-decision call cadence.
Report findings with severity; P0 = would corrupt determinism/goldens or steal the agent decision.`, { label: 'review:core', phase: 'Review', schema: REVIEW }),
])

if (coreReview) log(`core review: ${coreReview.verdict} (${(coreReview.findings || []).length} findings)`)
if (native && native.blockers && native.blockers.length) log(`NATIVE BLOCKERS: ${native.blockers.join(' | ')}`)

// ---- Phase 3: python wrapper + native review in parallel ----
phase('Wrapper')
const [wrapper, nativeReview] = await parallel([
  () => agent(`${CTX}

YOUR TASK: Implement Plan Task 5 (python wrapper trader mode + tests). Native mode 2 is landed — read the landed env.rs configure/step_trader to confirm the exact FFI behaviour (obs_dim/action_dim returned by configure(2), action marshalling expectations).
Steps: update python/jumpgate/gym_env.py (_MODES gains "trader": 2; Discrete(5) action space in trader mode while keeping the f32 buffer; int->float marshalling in step; info["episode_credits"] running sum, zeroed on reset). Create python/tests/test_trader_mode.py per the plan (3 tests; the greedy-accept test must genuinely earn credits over an episode).
BUILD THE WHEEL first so python sees the new native code: maturin build --release -m crates/jumpgate-py/Cargo.toml && unzip -o target/wheels/jumpgate-*.whl 'jumpgate/_native.abi3.so' -d python/
Run: PYTHONPATH=python python3 -m pytest python/tests/ -v  (ALL python tests, not just the new file — thrust-mode tests must stay green).
Commit per plan (explicit paths: python/jumpgate/gym_env.py python/tests/test_trader_mode.py).`, { label: 'impl:wrapper-5', phase: 'Wrapper', schema: REPORT }),
  () => agent(`${CTX}

YOU ARE A READ-ONLY ADVERSARIAL REVIEWER (no edits, no commits). Review the landed native trader-mode commits (Plan Tasks 3-4: trader template, obs writer, macro-step). Hunt specifically for:
1. SEMI-MDP correctness: does the accept path break ONLY when the craft is idle-or-horizon? Can it livelock (craft never idle => loop to horizon is fine, but loop past horizon is a bug)? Off-by-one on ticks_in_episode vs horizon?
2. Reward accounting: credits delta measured across the WHOLE macro-step (before first tick to after last)? Auto-reset must not zero credits BEFORE the reward is computed.
3. Obs staleness: board_ids captured at the same tick as the obs written? Slot->ContractId mapping used at the NEXT step's decode — can a stale id be accepted (fine: ingest skips) or a WRONG live id be accepted (bug: slot reindexed)?
4. Anti-memorization: m0 seed-derivation actually varies geometry across episodes AND auto-reset derives fresh seeds; explicit-seed reproducibility preserved.
5. Flight-rung regressions: modes 0/1 untouched (template mutation leakage between modes — Task 3 was told NOT to mutate self.template for trader).
6. Fixed scales: no VecNormalize dependence; obs writer scales match the spec table.
Report findings with severity; P0 = wrong rewards/decision-stealing/mode regression.`, { label: 'review:native-env', phase: 'Review', schema: REVIEW }),
])

if (nativeReview) log(`native review: ${nativeReview.verdict} (${(nativeReview.findings || []).length} findings)`)
if (wrapper && wrapper.blockers && wrapper.blockers.length) log(`WRAPPER BLOCKERS: ${wrapper.blockers.join(' | ')}`)

// ---- Phase 4: training stack + wrapper review in parallel ----
phase('Training')
const [training, wrapperReview] = await parallel([
  () => agent(`${CTX}

YOUR TASK: Implement Plan Task 6 (python/train/baselines.py + python/train/train_trader.py) AND Plan Task 7 (python/tests/test_trader_learning_smoke.py — the keystone gate). The wrapper is landed and the wheel is built (verify with: PYTHONPATH=python python3 -c "from jumpgate.gym_env import JumpgateGymEnv; e=JumpgateGymEnv(mode='trader'); print(e.action_space)").
Plan specifics: PPO MlpPolicy gamma=0.999, NO VecNormalize, 8 DummyVecEnv + VecMonitor, FreshSeedOnReset pattern copied from train_flight.py with base_seed 50_000+idx; --log-path discipline (smokes write /tmp, never runs/trader_log.csv). Baselines: random-valid and greedy-highest-reward over 25 held-out seeds (10_000..10_024); run them ONCE and put the real numbers in key_decisions.
Smoke calibration (Task 7): start at 6_000 decisions; raise toward 20_000 before weakening the margin (PPO mean > 1.15 * random mean + 0.05). Torch is CPU-only on this host (CUDA error 804 -> fallback, expected). Target smoke wall-clock ~2 min; if even 20k decisions cannot beat random, DO NOT ship a hollow gate — report it as a blocker with the curves/numbers you measured.
Run: PYTHONPATH=python python3 -m pytest python/tests/ -v (everything green including your smoke).
Commit per plan (two commits, explicit paths).`, { label: 'impl:training-6-7', phase: 'Training', schema: REPORT }),
  () => agent(`${CTX}

YOU ARE A READ-ONLY ADVERSARIAL REVIEWER (no edits, no commits). Review the landed python wrapper commit (gym_env.py trader mode + test_trader_mode.py). Hunt for:
1. Discrete->buffer marshalling: numpy int types, 0-d arrays from SB3 predict, action validation (out-of-range index must not panic the native layer).
2. info["episode_credits"]: zeroed on reset including AUTO-reset (native auto-resets on truncation — the wrapper cannot see it except via trunc flag; is the running sum handled correctly across the boundary)?
3. Buffer aliasing (spec 7.3/8 discipline): obs copies returned, not the live buffer.
4. Test honesty: does the greedy test ACTUALLY assert earned credits > 0 over a real episode, or could it vacuously pass (e.g. break before any delivery)?
Report findings with severity.`, { label: 'review:wrapper', phase: 'Review', schema: REVIEW }),
])

if (wrapperReview) log(`wrapper review: ${wrapperReview.verdict} (${(wrapperReview.findings || []).length} findings)`)
if (training && training.blockers && training.blockers.length) log(`TRAINING BLOCKERS: ${training.blockers.join(' | ')}`)

return {
  core, coreReview,
  native, nativeReview,
  wrapper, wrapperReview,
  training,
}