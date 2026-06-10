# Trader Rung 1 (Contract Haulage) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PPO learns contract haulage — accept the right delivery contract (or wait) and
earn more credits than a random-valid baseline — through the live deterministic economy,
with flight delegated to autopilot Rung 0.

**Architecture:** Semi-MDP gym mode 2 ("trader") in jumpgate-py: one Python step = one
strategic decision; the native env macro-steps world ticks between decision points.
Reward = Δcredits. Two hash-neutral core changes (ASSIGN gate + read accessors).

**Tech Stack:** Rust (jumpgate-core, jumpgate-py/PyO3), Python (gymnasium, SB3 PPO),
maturin build, pytest.

**Spec:** `docs/superpowers/specs/2026-06-10-trader-rung1-haulage-design.md` — READ IT
FIRST. All constants (`TRADER_OBS_DIM=20`, `M=4`, `WAIT_TICKS=8`, scales, scenario
geometry, route rewards) are defined there and in Task 3 below.

**Non-negotiable invariants (project law):**
- Goldens untouched: `HASH_FORMAT_VERSION = 3`, `GOLDEN_ZERO_STATE_HASH =
  0x1d44_b373_5ccd_33f7`, `GOLDEN_CONFIG_HASH` — no task may edit hash.rs fold order,
  store layouts, or stepping. Verify with `git diff` grep at the end.
- Determinism: no `std::time`, no map-order iteration, no float-from-env; all randomness
  through seeds.
- Never `git add -A`/`.`; explicit paths only. Never stage `.gitignore`, `.claude/`,
  `CLAUDE.md`, `AGENTS.md`, `.mcp.json`, `.filigree.conf`.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`;
  use `git commit -F` heredoc when the message has parens.
- Rust 2024 (`gen` is reserved). Lint with `cargo clippy --all-targets` (not `--lib`).

---

### Task 1: Core — ASSIGN gate (`stagger_period == 0` disables scripted acceptance)

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (in `run_scripted_dispatch`, between the
  REPOST loop and the ASSIGN loop, currently near the `// (b) ASSIGN` comment)
- Modify: `crates/jumpgate-core/src/config.rs` (doc comment on `DispatchCfg::stagger_period`)

- [ ] **Step 1: Write the failing test** in `economy.rs` `mod tests` (or world.rs tests if
  fixture access is easier — follow the existing dispatch-test pattern, e.g. the Task-21
  stagger tests):

```rust
#[test]
fn stagger_period_zero_disables_assign_but_not_repost() {
    // Build the minimal dispatch fixture used by the existing stagger tests:
    // 1 corp, 2 stations, 1 seeded TERMINAL contract on a route whose destination
    // stock is below demand_low (so REPOST fires), 1 Idle craft.
    // With stagger_period == 0:
    //   - REPOST still pushes a fresh Offered contract (count grows),
    //   - ASSIGN does NOT bind the Idle craft (role stays Idle, contract None).
    // (Mirror the arrange code of the nearest existing run_scripted_dispatch test;
    // assert ships.role[0] == CraftRole::Idle && ships.contract[0].is_none()
    // && contracts.ids.len() == n_before + 1.)
}
```

- [ ] **Step 2:** `cargo test -p jumpgate-core stagger_period_zero` → FAIL (craft gets bound).
- [ ] **Step 3: Implement.** In `run_scripted_dispatch`, immediately before the ASSIGN
  loop (`let stagger = ...`):

```rust
// stagger_period == 0: scripted acceptance OFF (manual / RL-issued AcceptContract
// only). REPOST above is unaffected — the board keeps flowing; nothing claims it.
if dispatch.stagger_period == 0 {
    return;
}
```

  And update the `DispatchCfg::stagger_period` doc in config.rs: add
  `/// `0` disables scripted acceptance entirely (manual / RL `AcceptContract` only).`
- [ ] **Step 4:** `cargo test -p jumpgate-core` → all green (the `.max(1)` coercion is now
  dead for the 0 case but harmless; existing tests with stagger 1/2 unaffected).
- [ ] **Step 5: Commit** `feat(core): stagger_period=0 disables scripted ASSIGN (RL-manual acceptance)`

### Task 2: Core — read-only economy accessors on `World`

**Files:**
- Modify: `crates/jumpgate-core/src/world.rs` (new `impl World` methods near the existing
  pub surface; tests in world.rs `mod tests`)
- Modify: `crates/jumpgate-core/src/lib.rs` ONLY if `StationId`/`ContractId` are not
  already re-exported (check `pub use` list first).

- [ ] **Step 1: Failing test** (world.rs tests; reuse the economy fixture from
  `escrow settled to craft credits` test around line 1609):

```rust
#[test]
fn trader_read_accessors_expose_board_and_wallet() {
    // Fixture: 2 stations, 1 corp, 1 Offered contract, 1 craft.
    // offered_contracts(): exactly one row (id, reward_micros, from, to) matching the
    //   seeded ContractInit; after the craft accepts + resolve binds it, the list is EMPTY
    //   (Offered+unclaimed only).
    // station_pos(): Some(pos) equal to the station's body position at the current tick
    //   (compare against StateView::body_pos); None for a stale id.
    // craft_credits(): 0 before delivery; craft_is_idle(): true before accept,
    //   false after ingest of AcceptContract.
}
```

- [ ] **Step 2:** run → FAIL (methods don't exist).
- [ ] **Step 3: Implement** in `impl World`:

```rust
/// Offered + unclaimed contracts (the strategic board), dense row order.
pub fn offered_contracts(&self) -> Vec<(ContractId, i64, StationId, StationId)> {
    (0..self.contracts.ids.len())
        .filter(|&k| {
            self.contracts.status[k] == crate::economy::ContractStatus::Offered
                && self.contracts.hauler[k].is_none()
                // a craft holding accept-INTENT (pre-resolve) also claims the slot:
                && !(0..self.ships.ids.len()).any(|r| {
                    self.ships.contract[r].map_or(false, |c| {
                        self.contracts.ids.dense_index(c.slot, c.generation) == Some(k)
                    })
                })
        })
        .map(|k| {
            let (slot, generation) = self.contracts.ids.id_at(k).expect("live row");
            (
                ContractId { slot, generation },
                self.contracts.reward_micros[k],
                self.contracts.from_station[k],
                self.contracts.to_station[k],
            )
        })
        .collect()
}

/// Station's body position at the current tick (orbits move).
pub fn station_pos(&self, id: StationId) -> Option<Vec3> {
    let row = self.stations.ids.dense_index(id.slot, id.generation)?;
    let body = self.stations.body[row];
    let brow = self.bodies.ids.dense_index(body.slot, body.generation)?;
    Some(self.eph.body_pos(self.bodies.eph_index[brow], self.tick))
}

pub fn craft_credits(&self, id: CraftId) -> Option<i64> {
    self.ships.index_of(id).map(|r| self.ships.credits_micros[r])
}

/// Idle = available for a strategic decision (no role, no bound contract).
pub fn craft_is_idle(&self, id: CraftId) -> Option<bool> {
    self.ships.index_of(id).map(|r| {
        self.ships.role[r] == crate::stores::CraftRole::Idle
            && self.ships.contract[r].is_none()
    })
}
```

  (Adjust to the real private-field names/borrows — e.g. `self.tick` vs `self.tick()`.
  Read the existing accessor impls first and mirror them exactly.)
- [ ] **Step 4:** `cargo test -p jumpgate-core` green; `cargo clippy --all-targets` clean.
- [ ] **Step 5: Commit** `feat(core): read-only trader accessors (board/station-pos/credits/idle)`

### Task 3: jumpgate-py — trader scenario template + configure(mode=2)

**Files:**
- Modify: `crates/jumpgate-py/src/env.rs`

- [ ] **Step 1: Constants + TraderCfg + template.** Add:

```rust
pub const TRADER_BOARD_SLOTS: usize = 4;          // M
pub const TRADER_ACTION_DIM: usize = 1;           // f32 index, Discrete(M+1) in Python
pub const TRADER_WAIT_TICKS: u64 = 8;
pub const TRADER_REWARD_SCALE: f64 = 3_000_000.0; // micros; max seeded route reward
pub const TRADER_DIST_SCALE: f64 = 2.4;           // AU; max pairwise station distance
pub const TRADER_CREDITS_SCALE: f64 = 30_000_000.0; // 10 * reward scale

#[derive(Clone, Copy, Debug)]
pub struct TraderCfg {
    pub horizon: u64, // ticks per episode (reuses configure's time_limit kwarg)
}
impl Default for TraderCfg { fn default() -> Self { TraderCfg { horizon: 2000 } } }
```

  `trader_config_template(seed: u64, num_craft: usize) -> RunConfig`: 1 star (1 M_sun);
  4 bodies, circular (`e=0,i=0`), `a = [0.35, 0.55, 0.8, 1.1]`, `m0` for body k derived
  from the seed: `m0 = u64_to_unit_f64(splitmix-style mix of (seed, k)) * std::f64::consts::TAU`
  (write a tiny `fn mix(seed: u64, k: u64) -> u64` using the 0x9E3779B97F4A7C15 stride +
  xor-shift — deterministic, no new dep; do NOT use RngStreams here since this is
  pre-reset config, not world state). 4 stations (body_index 1..=4 — body 0 is the star;
  station k on body k+1), initial Ore stock `[40, 0, 40, 0]` per station (miners' homes
  stocked), prices irrelevant in v1 (`initial_price_micros: [0,0]`).
  Producers: Ore miner `(None → Ore 5, interval 40)` at stations 0 and 2; Ore sink
  `(Ore 5 → None, interval 60)` at stations 1 and 3.
  1 corp: `treasury_micros: 1_000_000_000`, home station 0.
  4 seeded ContractInit (qty 5 each): (from 0→to 1, 1_000_000), (2→3, 1_200_000),
  (0→3, 1_600_000), (2→1, 3_000_000).
  `DispatchCfg { demand_low: 10, demand_high: 20, stagger_period: 0, contract_reward_micros: 0, contract_qty: 0 }`.
  Craft: copy the spec values from the world.rs forage-loop test fixture (the test that
  proves accept→load→deliver→pay end-to-end — read it; do NOT invent thrust/mass numbers;
  the reset anti-tunnel guard `a_max_empty*dt² < R/(2·k_brake)` must pass), spawn pos =
  near body 1 (e.g. body-1's `a` on +x with matching circular velocity — compute
  `v_circ = sqrt(G_CANONICAL * 1.0 / a)`), `fuel_mass = fuel_capacity` sized ≥ 3× a greedy
  rollout's burn (calibrate in Task 6's smoke; start generous, e.g. capacity 10× the
  fixture's).
  `dt: Dt::new(1.0)`, `softening 1.0e-4`, `substep_cfg` and `ephemeris_window` copied from
  `config_template`, `ephemeris_window` ≥ horizon + slack (e.g. 100_000 as today).

- [ ] **Step 2: Mode plumbing.** `control_mode = 2` in `configure(mode, ...)`: accept
  `mode == 2`; map `time_limit` kwarg → `self.trader.horizon`; set
  `self.obs_dim = TRADER_OBS_DIM; self.action_dim = TRADER_ACTION_DIM`; the trader
  template is built fresh at each reset from the seed (do NOT mutate `self.template`,
  which belongs to modes 0/1 — add a `control_mode == 2` branch in `reset()` that calls
  `trader_config_template(seed + env, num_craft)` directly). Error message updates from
  "mode must be 0 or 1" to include 2.
- [ ] **Step 3:** `cargo build -p jumpgate-py` compiles; unit test:

```rust
#[test]
fn trader_template_resolves_and_seed_varies_geometry() {
    let a = trader_config_template(1, 1);
    let b = trader_config_template(1, 1);
    let c = trader_config_template(2, 1);
    // deterministic per seed:
    assert_eq!(a.bodies[1].elements.m0, b.bodies[1].elements.m0);
    // varied across seeds:
    assert_ne!(a.bodies[1].elements.m0, c.bodies[1].elements.m0);
    // resolvable (anti-tunnel guard passes, economy refs in range):
    jumpgate_core::World::reset(a).expect("resolvable trader cfg");
}
```

- [ ] **Step 4: Commit** `feat(py): trader scenario template + mode-2 plumbing`

### Task 4: jumpgate-py — trader obs + macro-step

**Files:**
- Modify: `crates/jumpgate-py/src/obs.rs` — `pub const TRADER_OBS_DIM: usize = 20;` and:

```rust
/// Trader-mode obs: 4 board slots × [present, reward, d_pickup, d_haul] + own
/// [fuel_frac, credits, busy, time_remaining_frac]. FIXED global scales (no
/// running normalization — flight-rung lesson #2/#run-6).
pub fn write_obs_trader(
    board: &[(f64 /*reward_micros*/, f64 /*d_pickup_au*/, f64 /*d_haul_au*/)], // len <= 4
    fuel_frac: f32,
    credits_micros: f64,
    busy: bool,
    time_remaining_frac: f32,
    out: &mut [f32], // len TRADER_OBS_DIM
) { /* slot dims 0..16 (zeros for absent), own dims 16..20, scales per spec §5.3 */ }
```

- Modify: `crates/jumpgate-py/src/env.rs` — state fields (reuse where possible:
  `ticks_in_episode`, `master_seed`, `episode_counter` already exist; add
  `trader: TraderCfg`, `board_ids: Vec<jumpgate_core::ContractId>` (slot→id mapping
  captured at obs-write time), `prev_credits: Vec<i64>`).

- [ ] **Step 1: `reset_trader_episode(env, seed, out)`** — rebuild world from
  `trader_config_template(derived_seed, num_craft)`, zero episode clock and
  `prev_credits`, **advance to the first decision point** (the craft starts idle and
  seeded contracts exist, so this is immediate), capture `board_ids`, write obs.
- [ ] **Step 2: `step_trader(act, obs, rew, term, trunc)`** — per env:

```text
choice = act[a_base].round() clamped to [0, M]
cmds = if choice >= 1 and board_ids[choice-1] is Some(cid):
           [Command { target: craft, kind: AcceptContract { contract: cid } }]
       else []                                   # wait or empty slot
credits_before = world.craft_credits(craft)
ticks_advanced = 0
loop:
    world.step(&mut cmds)   # cmds only on the FIRST iteration (then empty)
    ticks_in_episode += 1; ticks_advanced += 1
    if ticks_in_episode >= trader.horizon: truncated = true; break
    idle = world.craft_is_idle(craft)
    if accepted_this_step:                      # accept path: run until trip resolves
        if idle: break                          # delivered, failed, or ingest-skipped
    else:                                       # wait path: fixed advance
        if ticks_advanced >= TRADER_WAIT_TICKS: break
reward = (credits_now - credits_before) as f64 / 1e6
term = false; trunc = truncated
rebuild board (offered_contracts, take first M, store board_ids), write obs
if truncated: auto-reset (same derived-seed scheme as thrust mode:
    episode_counter += 1; fresh_seed = master_seed ^ episode_counter;
    reset_trader_episode(env, fresh_seed, obs))   # flags still report old episode
```

  Wire `control_mode == 2` dispatch into `step()` and `reset()` beside the thrust arms.
- [ ] **Step 3: Rust unit tests** (env.rs `mod tests`, no Python needed):
  - `trader_macro_step_accept_pays_delta_credits`: fixed seed; find the slot whose route
    is 0→1 (craft spawns at body 1 = station 0's body — check via `station_pos`); accept
    it; assert the macro-step returns with `rew > 0` (escrow settled) and craft idle.
  - `trader_wait_advances_eight_ticks`: choice 0 advances exactly `TRADER_WAIT_TICKS`
    ticks (or to horizon) with `rew == 0`.
  - `trader_truncates_at_horizon_and_autoresets`: drive waits to the horizon; assert
    `trunc` then `episode_counter == 1` and a fresh board obs.
- [ ] **Step 4:** `cargo test -p jumpgate-py` green; `cargo clippy --all-targets` clean.
- [ ] **Step 5: Commit** `feat(py): trader obs (20-dim fixed-scale) + semi-MDP macro-step`

### Task 5: Python wrapper — Discrete action space + tests

**Files:**
- Modify: `python/jumpgate/gym_env.py`
- Create: `python/tests/test_trader_mode.py`

- [ ] **Step 1:** `_MODES = {"waypoint": 0, "thrust": 1, "trader": 2}`. In
  `_rebuild_spaces_and_buffers`, trader mode sets
  `self.action_space = gym.spaces.Discrete(5)` (M+1) while keeping the f32 action buffer
  of length 1; `step()` marshals `int(action)` → `self._action_buf[0] = float(action)`.
  Obs space stays Box(20,). `info["episode_credits"]`: maintain a running
  `self._episode_credits += reward` in the wrapper, attach on every step, zero on reset.
- [ ] **Step 2: Tests** (`test_trader_mode.py`):

```python
def test_trader_obs_shape_and_action_space():
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
    obs, _ = env.reset(seed=3)
    assert obs.shape == (20,)
    assert env.action_space.n == 5
    assert 0.0 <= obs[19] <= 1.0          # time_remaining_frac
    assert obs[0] in (0.0, 1.0)           # slot-0 present flag

def test_trader_accept_eventually_pays():
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
    obs, _ = env.reset(seed=3)
    total, steps = 0.0, 0
    # greedy: always accept the highest-reward present slot
    for _ in range(64):
        rewards = [obs[4*j + 1] if obs[4*j] > 0.5 else -1.0 for j in range(4)]
        act = (int(np.argmax(rewards)) + 1) if max(rewards) > 0 else 0
        obs, r, term, trunc, info = env.step(act)
        total += r
        if trunc: break
    assert total > 0.0, "a full episode of greedy accepts must earn credits"

def test_trader_explicit_seed_reproduces_and_unseeded_varies():
    # same explicit seed -> identical first obs; two unseeded resets -> different obs
    # (geometry is seed-derived). Mirror test_thrust_mode's seed test shape.
```

- [ ] **Step 3:** Build + run: `maturin build --release -m crates/jumpgate-py/Cargo.toml`
  then `unzip -o target/wheels/jumpgate-*.whl 'jumpgate/_native.abi3.so' -d python/`;
  `PYTHONPATH=python python3 -m pytest python/tests/test_trader_mode.py -v` → PASS.
- [ ] **Step 4: Commit** `feat(py): trader gym mode (Discrete(5)) + mode tests`

### Task 6: Baselines + training script

**Files:**
- Create: `python/train/baselines.py` — `rollout_policy(env_seed, policy_fn) -> credits`;
  `random_valid(obs, rng)` (uniform over present slots, wait iff none),
  `greedy_reward(obs)` (argmax slot-1 reward dim, wait iff empty board);
  `evaluate(policy_fn, seeds: list[int]) -> (mean, sd)`; `__main__` prints both baselines
  over 25 held-out seeds (seeds 10_000..10_024).
- Create: `python/train/train_trader.py` — PPO MlpPolicy, `gamma=0.999`,
  `n_steps=512, batch_size=128, ent_coef=0.003, learning_rate=3e-4` (constant — only
  anneal if smoke shows instability), 8 DummyVecEnv envs (NO VecNormalize — spec §5.3),
  VecMonitor, `--steps` (default 60_000 decisions), `--seed`, `--log-path`
  (default `runs/trader_log.csv`; smokes MUST pass /tmp paths — flight-rung lesson #7).
  CSV rows per episode: `step, ep_return(credits), ep_len(decisions)`. Save
  `runs/trader_final.zip`. Wrap envs in the same `FreshSeedOnReset` pattern as
  train_flight.py (import or copy — copy is fine, it is 12 lines; keep base_seed
  10_000+idx DISTINCT from eval's held-out seeds: use 50_000+idx).
- [ ] Run baselines once, record numbers in the commit message.
- [ ] **Commit** `feat(train): trader baselines (random-valid/greedy) + PPO trainer`

### Task 7: Learning smoke (the keystone gate)

**Files:**
- Create: `python/tests/test_trader_learning_smoke.py`

```python
# Gate: PPO with a tiny budget must beat the random-valid baseline on held-out seeds.
# Calibrate budget so the test runs in ~2 min; tighten margin only if stable.
def test_ppo_beats_random_baseline():
    train(steps=6_000, seed=0, log_path="/tmp/trader_smoke.csv")   # short-horizon cfg ok
    ppo_mean = evaluate(ppo_policy, seeds=range(10_000, 10_020))
    rnd_mean = evaluate(random_valid, seeds=range(10_000, 10_020))
    assert ppo_mean > rnd_mean * 1.15 + 0.05
```

- [ ] If 6k decisions is insufficient, raise to ≤ 20k before weakening the margin; if it
  still fails, STOP and report (do not ship a hollow gate).
- [ ] **Commit** `test(train): trader learning smoke — PPO > random-valid baseline`

### Task 8: Land-time verification (main loop, NOT subagents)

- [ ] `cargo test --workspace` all green; `cargo clippy --all-targets -- -D warnings` clean.
- [ ] `git diff main -- crates/jumpgate-core/src/hash.rs` → empty; grep confirms
  `HASH_FORMAT_VERSION = 3` and both golden literals unchanged.
- [ ] Full pytest: `PYTHONPATH=python python3 -m pytest python/tests -v`.
- [ ] `python3 python/train/baselines.py` — record random/greedy numbers.
- [ ] Full training run `--steps 60000` + report curve (credits/episode over decisions),
  PPO vs both baselines on 25 fresh held-out seeds (seeds 20_000..20_024).
