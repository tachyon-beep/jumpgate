# Tactical Flight Rung 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A PPO+MLP agent learns to pilot a craft via direct thrust to a randomized target in the real Newtonian sim — and we watch it learn (curves + trajectory renders).

**Architecture:** One additive core change (`CommandKind::Thrust` → `NavState::DirectThrust`, riding the exact autopilot→`thrust_accel_and_burn` channel). The gym env grows a thrust control-mode: seeded per-episode targets, 10-dim target-relative obs, potential-based shaped reward, curriculum knobs. Python gets the first real training stack (SB3 PPO, curriculum scheduler, trajectory renderer). Core determinism untouched: existing goldens must not move.

**Tech Stack:** Rust 2024 (`jumpgate-core`, `jumpgate-py`/PyO3/maturin), Python 3.12, gymnasium, stable-baselines3 (torch), matplotlib.

**Spec:** `docs/superpowers/specs/2026-06-10-tactical-flight-rung1-design.md`

---

## Ground truth (verified this session — do not rediscover)

- The step loop (`world.rs:417-466`) per craft: reads `NavState` → `autopilot_command(nav, …) -> (thrust_dir, throttle)` (`autopilot.rs:35`) → `thrust_accel_and_burn(&eff, fuel, dir, throttle, dt)` (`ship.rs:27`) → integrator. **`Thrust` must inject as a `NavState` variant consumed by `autopilot_command`** — ingest is the only nav write path (lever invariant, `ingest.rs:1-4`).
- `NavState` is `{Idle, Seeking{dest, dv_remaining}}` (`stores.rs:13-16`). `CommandKind` is `{Destination, AcceptContract, SetRole}` (`types.rs:53-67`). `command_sort_key` keys on target only (`contract.rs:33-40`) — a new kind needs no sort change.
- `World::reset` rejects `a_max_empty·dt² ≥ ARRIVAL_RADIUS/(2·k_brake)` (= 1e-4 at defaults). The proven-resolvable craft spec is `dry=1e-12, thrust=1e-17, v_e=1e-3, fuel=1e-12` (a_max=1e-5, 10× margin) — see `env.rs:56-70`. **Keep dry/thrust; make `exhaust_velocity` & fuel a curriculum knob** (long sprints need more Δv: 0.5 AU at a=1e-5 needs ~6.3e-3 AU/day; v_e=1e-3 tank gives only ~6.9e-4).
- Obs f32-boundary guard `MAX_REL_AU = 1.0` (`obs.rs:38`) — thrust-mode obs must scale rel-pos by a `dist_scale` or orbital-scale targets trip it.
- Gym runs end-to-end from Python today (proven): `PYTHONPATH=python python3 -c "from jumpgate.gym_env import JumpgateGymEnv"`. Native module: `python/jumpgate/_native.abi3.so`; rebuild with `maturin develop --release` from `crates/jumpgate-py/`.
- Goldens that must NOT move: config `0x278c_5d91_b75a_9e5a`, zero-state `0x65d7_af3b_9a8a_8276`, recorded-run; `HASH_FORMAT_VERSION = 2`. New NavState/Command variants get NEW tags; existing encodings unchanged → goldens stable. Run `cargo test -p jumpgate-core` to prove it after every core task.
- WIP commit `2e1e1ad` holds stopped trophic-cut-1 foundations — **do not build on it, do not revert it**; it is parked.

## File structure

- `crates/jumpgate-core/src/types.rs` — `CommandKind::Thrust`
- `crates/jumpgate-core/src/stores.rs` — `NavState::DirectThrust`
- `crates/jumpgate-core/src/autopilot.rs` — pass-through arm
- `crates/jumpgate-core/src/ingest.rs` — both ingestion paths
- `crates/jumpgate-core/src/world.rs` — dest-resolution arm (one match)
- `crates/jumpgate-core/src/hash.rs` — fold new variants (new tags only)
- `crates/jumpgate-py/src/env.rs` — control mode, configure(), seeded target, reward, termination
- `crates/jumpgate-py/src/obs.rs` — 10-dim thrust-mode obs writer
- `python/jumpgate/gym_env.py` — mode-aware wrapper + `configure()`
- `python/train/train_flight.py` — PPO training entry
- `python/train/curriculum.py` — success-ratcheted difficulty scheduler
- `python/train/render.py` — trajectory render (random vs trained)
- `python/tests/test_thrust_mode.py` — env behaviour tests
- `python/tests/test_learning_smoke.py` — "learning happens" check (slow-marked)

---

## Task 0: Park the deferred trophic docs (hygiene)

**Files:** Modify `docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md`, `docs/superpowers/plans/2026-06-10-trophic-cut-1-implementation.md`

- [ ] **Step 1:** Prepend to BOTH files, directly under the title line:

```markdown
> **⚠ DEFERRED (2026-06-10, owner).** Superseded in sequence by the DRL pivot: agents must
> LEARN risk (PPO+LSTM), not carry a hardcoded `risk_appetite` scalar — that scalar was the
> computed-answer reflex again. Do NOT resume this build as written. The trophic world
> returns AFTER the tactical-flight rung proves the training pipeline
> (`2026-06-10-tactical-flight-rung1-design.md`). Phase-1 foundations are parked at WIP
> commit `2e1e1ad` (pirate columns/events/RngStream::Piracy — salvageable later).
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md docs/superpowers/plans/2026-06-10-trophic-cut-1-implementation.md
git commit -m "docs(trophic-cut-1): mark spec+plan DEFERRED pending the DRL tactical-flight pivot"
```

## Task 1: `CommandKind::Thrust` → `NavState::DirectThrust` (the one core change)

**Files:**
- Modify: `crates/jumpgate-core/src/types.rs:53-67`, `crates/jumpgate-core/src/stores.rs:13-16`,
  `crates/jumpgate-core/src/autopilot.rs:46-88`, `crates/jumpgate-core/src/ingest.rs` (both paths),
  `crates/jumpgate-core/src/world.rs:417-422`, `crates/jumpgate-core/src/hash.rs`
- Test: inline `#[cfg(test)]` in each touched module

- [ ] **Step 1: Write the failing tests** (in `stores.rs`/`autopilot.rs`/`world.rs` test mods):

```rust
// autopilot.rs tests
#[test]
fn direct_thrust_passes_vector_through() {
    let nav = NavState::DirectThrust { throttle_vec: Vec3::new(0.6, 0.0, 0.0) };
    let (dir, throttle) = autopilot_command(
        nav, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4,
    );
    assert_eq!(dir, Vec3::new(1.0, 0.0, 0.0));
    assert!((throttle - 0.6).abs() < 1e-12);
}
#[test]
fn direct_thrust_overlong_vector_clamps_to_full_throttle() {
    let nav = NavState::DirectThrust { throttle_vec: Vec3::new(3.0, 4.0, 0.0) }; // |v|=5
    let (dir, throttle) = autopilot_command(
        nav, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4,
    );
    assert!((dir.length() - 1.0).abs() < 1e-12);
    assert_eq!(throttle, 1.0);
}
#[test]
fn direct_thrust_zero_vector_coasts() {
    let nav = NavState::DirectThrust { throttle_vec: Vec3::ZERO };
    let (_dir, throttle) = autopilot_command(
        nav, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4,
    );
    assert_eq!(throttle, 0.0);
}

// world.rs tests — end-to-end through step()
#[test]
fn thrust_command_accelerates_craft_and_burns_fuel() {
    // Build the economy-free resolvable fixture (mirror reset_accepts_resolvable_thrusting_craft),
    // ingest CommandKind::Thrust { throttle_vec: Vec3::new(1.0, 0.0, 0.0) } targeting the craft,
    // step twice. Assert: vel.x increased by ~ a_max*dt per tick (within integrator tolerance),
    // fuel_mass strictly decreased, and a ThrustApplied event was emitted.
}
#[test]
fn thrust_command_persists_until_replaced() {
    // Ingest one Thrust command, then step 3 ticks with empty cmd vec.
    // Assert fuel decreases on ALL three ticks (held stick), then ingest
    // Thrust{ZERO} and assert fuel constant on the next tick.
}
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p jumpgate-core direct_thrust` → FAIL (no variant).

- [ ] **Step 3: Implement** (complete change-set):

```rust
// types.rs — append to CommandKind (after SetRole):
    /// Direct thrust intent (tactical Rung 1): world-frame throttle vector,
    /// |v| in [0,1] = throttle fraction (over-length clamps to 1 at the
    /// autopilot pass-through). Persists as NavState::DirectThrust until replaced.
    Thrust { throttle_vec: crate::math::Vec3 },

// stores.rs — append to NavState:
    /// Direct thrust (no destination, no Δv budget — fuel is the budget).
    DirectThrust { throttle_vec: NavVec },
// (use the same Vec3 type Seeking's NavDest machinery uses; plain `crate::math::Vec3`.)

// autopilot.rs — third match arm in autopilot_command (before/after Seeking):
        NavState::DirectThrust { throttle_vec } => {
            let mag = throttle_vec.length();
            if mag <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            (throttle_vec.normalize_or_zero(), mag.min(1.0))
        }

// ingest.rs — ingest_commands(): new arm in the CommandKind match:
                CommandKind::Thrust { throttle_vec } => {
                    world.set_nav(id, NavState::DirectThrust { throttle_vec });
                }
// ingest_into() (slice path): add the analogous (Craft, Thrust) arm setting
// ship.nav[idx] = NavState::DirectThrust { throttle_vec }; emit action_ingested.

// world.rs step() — dest-resolution match (line ~417) gains:
                NavState::DirectThrust { .. } => (pos, Vec3::ZERO), // unused (autopilot ignores dest)
// NOTE world.rs:455 — the dv_remaining decrement is already guarded by
// `if let NavState::Seeking`, so DirectThrust needs no change there; the
// ThrustApplied event fires for any throttle > 0 (correct).

// hash.rs — find the NavState fold (grep "Seeking" in hash.rs) and add a
// DirectThrust arm with the NEXT tag value (existing tags UNCHANGED), folding
// the three f64 components. Same for any Command/CommandKind fold (grep
// "CommandKind" in hash.rs): new variant -> new tag, fold three f64s.
// The compiler's exhaustive-match errors are the worklist: fix every site it names.
```

- [ ] **Step 4: Run** `cargo test -p jumpgate-core` → ALL green, including every pre-existing golden (config `0x278c…`, zero-state `0x65d7…`, `record_then_replay_is_bit_identical`). **If any golden moved, the hash fold changed an existing encoding — fix the fold; do NOT re-pin.**

- [ ] **Step 5: Replay determinism for the new mode** — add to the replay test module (next to `record_then_replay_is_bit_identical`):

```rust
#[test]
fn thrust_mode_record_then_replay_is_bit_identical() {
    // Same harness as record_then_replay_is_bit_identical, but the recorded
    // run feeds varying CommandKind::Thrust commands (e.g. tick t: throttle_vec
    // = Vec3::new(((t % 7) as f64) / 7.0, ((t % 3) as f64) / 3.0, 0.0)) for 50
    // ticks. Record state hashes per tick; replay from the log; assert the hash
    // sequences are bit-identical.
}
```

- [ ] **Step 6:** `cargo test -p jumpgate-core && cargo clippy --all-targets -p jumpgate-core` → green/clean.

- [ ] **Step 7: Commit**

```bash
git add crates/jumpgate-core/src/types.rs crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/autopilot.rs crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/hash.rs
git commit -m "feat(core): CommandKind::Thrust -> NavState::DirectThrust (tactical Rung 1, replay-safe, goldens untouched)"
```

## Task 2: Thrust-mode observation writer (10-dim, scaled)

**Files:** Modify `crates/jumpgate-py/src/obs.rs`

- [ ] **Step 1: Failing tests** (in `obs.rs` test mod):

```rust
#[test]
fn thrust_obs_layout_and_scaling() {
    let mut out = [9.0f32; THRUST_OBS_DIM]; // 10; sentinel proves overwrite
    write_obs_thrust_mode(
        Vec3::new(0.01, 0.0, 0.0),          // own vel (AU/day)
        0.5,                                 // fuel frac
        Vec3::new(0.2, -0.1, 0.0),           // target rel pos (AU)
        Vec3::new(-0.01, 0.0, 0.0),          // target rel vel
        0.4,                                 // dist_scale
        &mut out,
    );
    assert_eq!(out[0], (0.01f64 / VEL_SCALE) as f32);    // vel block scaled
    assert_eq!(out[3], 0.5);                              // fuel frac raw
    assert_eq!(out[4], (0.2f64 / 0.4) as f32);            // rel pos / dist_scale
    assert_eq!(out[7], (-0.01f64 / VEL_SCALE) as f32);    // rel vel scaled
}
#[test]
#[should_panic(expected = "crossed the f32 observation boundary")]
#[cfg(debug_assertions)]
fn thrust_obs_guard_fires_on_unscaled_absolute() {
    let mut out = [0.0f32; THRUST_OBS_DIM];
    // rel pos of 10 AU with dist_scale 1.0 -> scaled value 10 > MAX_REL_AU: guard trips.
    write_obs_thrust_mode(Vec3::ZERO, 0.5, Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, 1.0, &mut out);
}
```

- [ ] **Step 2:** `cargo test -p jumpgate-py thrust_obs` → FAIL.

- [ ] **Step 3: Implement** in `obs.rs`:

```rust
/// Thrust-mode obs: own vel (3, /VEL_SCALE) + fuel frac (1) + target rel-pos
/// (3, /dist_scale) + target rel-vel (3, /VEL_SCALE). All O(1) post-scaling;
/// the rel_to_f32 guard still applies to every scaled component.
pub const THRUST_OBS_DIM: usize = 10;
/// Velocity normalizer (AU/day): ~circular-orbit speed at 1 AU, so orbital
/// velocities land O(1).
pub const VEL_SCALE: f64 = 0.02;

pub fn write_obs_thrust_mode(
    own_vel: Vec3,
    fuel_frac: f32,
    target_rel_pos: Vec3,
    target_rel_vel: Vec3,
    dist_scale: f64,
    out: &mut [f32],
) {
    debug_assert_eq!(out.len(), THRUST_OBS_DIM);
    debug_assert!(dist_scale > 0.0);
    out[0] = rel_to_f32(own_vel.x / VEL_SCALE);
    out[1] = rel_to_f32(own_vel.y / VEL_SCALE);
    out[2] = rel_to_f32(own_vel.z / VEL_SCALE);
    out[3] = fuel_frac;
    out[4] = rel_to_f32(target_rel_pos.x / dist_scale);
    out[5] = rel_to_f32(target_rel_pos.y / dist_scale);
    out[6] = rel_to_f32(target_rel_pos.z / dist_scale);
    out[7] = rel_to_f32(target_rel_vel.x / VEL_SCALE);
    out[8] = rel_to_f32(target_rel_vel.y / VEL_SCALE);
    out[9] = rel_to_f32(target_rel_vel.z / VEL_SCALE);
}
```

(`rel_to_f32`'s guard bound `MAX_REL_AU = 1.0` stays; scaling keeps honest values
inside it. A craft >1 dist_scale from target post-scale is possible if it flies
away — widen the guard call-site tolerance ONLY if a real run trips it; start strict.
If that proves noisy in early training, clamp scaled components to ±MAX_REL_AU
instead of widening the guard — clamping is visible in obs, guard-widening hides bugs.)

- [ ] **Step 4:** `cargo test -p jumpgate-py` → green. Commit:

```bash
git add crates/jumpgate-py/src/obs.rs
git commit -m "feat(py-obs): 10-dim scaled thrust-mode observation writer"
```

## Task 3: Thrust-mode env — configure, seeded target, reward, termination

**Files:** Modify `crates/jumpgate-py/src/env.rs`, `python/jumpgate/gym_env.py`

- [ ] **Step 1: Failing Rust unit tests** (env.rs test mod — pure fns, no GIL):

```rust
#[test]
fn flight_reward_potential_shaping_rewards_approach() {
    let cfg = FlightCfg::default(); // gamma 0.99
    // moved from 0.5 to 0.4 distance (scale 1.0): shaping = gamma*(-0.4) - (-0.5) > 0
    let r = flight_reward(&cfg, 0.5, 0.4, 0.0, false, 0.0);
    assert!(r > 0.0, "approach must be net-positive: {r}");
    let r_away = flight_reward(&cfg, 0.4, 0.5, 0.0, false, 0.0);
    assert!(r_away < 0.0, "retreat must be net-negative: {r_away}");
}
#[test]
fn flight_reward_arrival_bonus_dominates() {
    let cfg = FlightCfg::default();
    let r = flight_reward(&cfg, 0.01, 0.0, 0.0, true, 0.0);
    assert!(r > 1.0, "arrival must dwarf per-tick terms: {r}");
}
#[test]
fn arrival_requires_low_relative_speed() {
    let cfg = FlightCfg::default();
    assert!(is_arrival(&cfg, cfg.arrival_radius * 0.5, cfg.arrival_speed * 0.5));
    assert!(!is_arrival(&cfg, cfg.arrival_radius * 0.5, cfg.arrival_speed * 2.0)); // flyby
    assert!(!is_arrival(&cfg, cfg.arrival_radius * 2.0, 0.0));
}
#[test]
fn target_draw_is_seed_deterministic_and_distance_bounded() {
    let cfg = FlightCfg { target_dist_min: 0.001, target_dist_max: 0.01, ..FlightCfg::default() };
    let a = draw_target(&cfg, 42, 0);
    let b = draw_target(&cfg, 42, 0);
    let c = draw_target(&cfg, 43, 0);
    assert_eq!(a, b, "same seed -> same target");
    assert_ne!(a, c, "different seed -> different target");
    let d = a.length();
    assert!((0.001..=0.01).contains(&d), "distance {d} out of band");
}
```

- [ ] **Step 2:** `cargo test -p jumpgate-py flight` → FAIL.

- [ ] **Step 3: Implement in `env.rs`:**

```rust
/// Tactical-flight curriculum + reward config (all settable from Python).
#[derive(Clone, Copy, Debug)]
pub struct FlightCfg {
    pub target_dist_min: f64,   // AU
    pub target_dist_max: f64,   // AU
    pub star_mass: f64,         // M_sun; 0.0 = gravity off (stage 0)
    pub exhaust_velocity: f64,  // curriculum Δv knob (reset-guard-safe: thrust/mass unchanged)
    pub fuel_capacity: f64,     // scales with sprint length
    pub time_limit: u64,        // ticks
    pub arrival_radius: f64,    // AU
    pub arrival_speed: f64,     // AU/day (rendezvous gate)
    pub gamma: f64,             // MUST equal PPO gamma (potential-shaping invariant)
    pub fuel_weight: f64,       // reward cost per unit fuel
    pub time_penalty: f64,      // per-tick cost
    pub arrival_bonus: f64,
}
impl Default for FlightCfg {
    fn default() -> Self {
        FlightCfg {
            target_dist_min: 0.001, target_dist_max: 0.005,  // stage-0 short hops
            star_mass: 0.0, exhaust_velocity: 0.1, fuel_capacity: 1.0e-12,
            time_limit: 400, arrival_radius: 1.0e-4, arrival_speed: 5.0e-4,
            gamma: 0.99, fuel_weight: 1.0e9, time_penalty: 0.001, arrival_bonus: 10.0,
        }
    }
}

/// Potential-based shaping (Φ = −d/dist_scale, normalized) + arrival bonus −
/// fuel − time. Potential-based SPECIFICALLY (Ng et al. form γΦ(s')−Φ(s)) so
/// shaping cannot be farmed by dithering.
pub fn flight_reward(
    cfg: &FlightCfg, prev_dist: f64, cur_dist: f64,
    fuel_spent: f64, arrived: bool, _dt: f64,
) -> f64 {
    let scale = cfg.target_dist_max.max(1e-12);
    let phi_prev = -(prev_dist / scale);
    let phi_cur = -(cur_dist / scale);
    let shaping = cfg.gamma * phi_cur - phi_prev;
    let bonus = if arrived { cfg.arrival_bonus } else { 0.0 };
    shaping + bonus - cfg.fuel_weight * fuel_spent - cfg.time_penalty
}

pub fn is_arrival(cfg: &FlightCfg, dist: f64, rel_speed: f64) -> bool {
    dist <= cfg.arrival_radius && rel_speed <= cfg.arrival_speed
}

/// Seeded target draw: uniform direction (Marsaglia via the core ChaCha8
/// Scenario stream — rand::Rng is already in jumpgate-core's tree) at a
/// uniform distance in [min, max]. `env_idx` decorrelates vectorized envs.
pub fn draw_target(cfg: &FlightCfg, seed: u64, env_idx: u64) -> Vec3 {
    use rand::Rng;
    let mut streams = jumpgate_core::RngStreams::from_master(seed.wrapping_add(env_idx));
    let rng = streams.stream(jumpgate_core::RngStream::Scenario);
    let dist = rng.random_range(cfg.target_dist_min..=cfg.target_dist_max);
    loop {
        let v = Vec3::new(
            rng.random_range(-1.0..=1.0),
            rng.random_range(-1.0..=1.0),
            rng.random_range(-1.0..=1.0),
        );
        let l = v.length();
        if l > 1e-9 && l <= 1.0 {
            return v.scale(dist / l);
        }
    }
}
```

(If `RngStreams`/`RngStream` are not pub-exported from jumpgate-core's lib.rs,
add them to the existing `pub use` list — additive. If `rand` is not a direct
dep of jumpgate-py, add the workspace `rand` to its Cargo.toml.)

Then wire `JumpgateEnv`:
- `control_mode: u8` field (0 = waypoint, 1 = thrust) + `flight: FlightCfg` + per-env `target_abs: Vec<Vec3>` + `prev_dist: Vec<f64>` + `ticks_in_episode: Vec<u64>`.
- New pymethod `configure(&mut self, mode: u8, kwargs…)` setting every `FlightCfg` field (plain f64/u64 args; no dict parsing) — also rebuilds `self.template` craft spec with `cfg.exhaust_velocity`/`cfg.fuel_capacity` and star mass `cfg.star_mass`. Returns the new obs/action dims.
- `obs_dim`/`action_dim` getters: thrust mode → `(THRUST_OBS_DIM, 3)`.
- `reset(seed, out_obs)`: thrust mode → also `draw_target` per env (`target_abs[env] = craft_pos + draw_target(...)`), zero `ticks_in_episode`, `prev_dist` = initial distance, write thrust-mode obs (`dist_scale = cfg.target_dist_max`). Target rel vel = `ZERO - craft_vel` (static target v1).
- `step(...)`: thrust mode → decode `[tx,ty,tz]` (clamp each to [-1,1]) into `CommandKind::Thrust`, step the world, compute `cur_dist`/`rel_speed`, reward via `flight_reward` (fuel_spent from the prev_fuel snapshot — same pattern as waypoint mode), `terminated = is_arrival`, `truncated = ticks_in_episode >= time_limit`, write thrust obs, update `prev_dist`. **On terminated||truncated, auto-reset that env** (draw a fresh target with a fresh derived seed, e.g. `seed = master ^ episode_counter`) so SB3's VecEnv contract (auto-reset per sub-env) holds; expose `episode_counter` so the smoke test can assert it advances.

- [ ] **Step 4:** `cargo test -p jumpgate-py && cargo clippy --all-targets -p jumpgate-py` → green/clean.

- [ ] **Step 5: Python wrapper** (`gym_env.py`): add `mode="waypoint"|"thrust"` ctor arg + `set_difficulty(**kwargs)` passing through to native `configure`, re-deriving spaces/buffers from the returned dims (action space stays `Box(-1, 1, (3,))` in thrust mode).

- [ ] **Step 6: Commit**

```bash
git add crates/jumpgate-py/src/env.rs crates/jumpgate-py/Cargo.toml python/jumpgate/gym_env.py
git commit -m "feat(py-env): thrust control mode — seeded targets, shaped reward, rendezvous termination, curriculum configure()"
```

## Task 4: Rebuild native + Python behaviour tests

**Files:** Create `python/tests/test_thrust_mode.py`

- [ ] **Step 1:** Rebuild: `cd crates/jumpgate-py && maturin develop --release` (expected: `Installed jumpgate-0.1.0`). If `maturin develop` refuses (no venv), fall back to `maturin build --release` + `pip install --user --force-reinstall target/wheels/jumpgate-*.whl`.

- [ ] **Step 2: Write the tests:**

```python
import numpy as np
from jumpgate.gym_env import JumpgateGymEnv

def _make(**kw):
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
    if kw:
        env.set_difficulty(**kw)
    return env

def test_thrust_mode_spaces():
    env = _make()
    assert env.observation_space.shape == (10,)
    assert env.action_space.shape == (3,)

def test_targets_vary_by_seed_and_obs_sees_them():
    a, _ = _make().reset(seed=1)
    b, _ = _make().reset(seed=2)
    assert not np.allclose(a[4:7], b[4:7]), "different seeds must yield different targets"
    assert np.linalg.norm(a[4:7]) > 0, "target must be visible in obs"

def test_same_seed_reproducible():
    e1, e2 = _make(), _make()
    o1, _ = e1.reset(seed=7); o2, _ = e2.reset(seed=7)
    act = np.array([0.5, 0.2, 0.0], dtype=np.float32)
    for _ in range(20):
        o1, r1, *_ = e1.step(act)
        o2, r2, *_ = e2.step(act)
    assert np.array_equal(o1, o2) and r1 == r2

def test_thrusting_toward_target_beats_coasting():
    env = _make()
    obs, _ = env.reset(seed=11)
    toward = obs[4:7] / (np.linalg.norm(obs[4:7]) + 1e-9)
    r_thrust = sum(env.step(toward.astype(np.float32))[1] for _ in range(30))
    env2 = _make(); env2.reset(seed=11)
    r_coast = sum(env2.step(np.zeros(3, np.float32))[1] for _ in range(30))
    assert r_thrust > r_coast, f"approach {r_thrust} must out-reward coasting {r_coast}"

def test_episode_truncates_and_autoresets():
    env = _make(time_limit=10)
    env.reset(seed=3)
    truncated = False
    for _ in range(12):
        _, _, term, trunc, _ = env.step(np.zeros(3, np.float32))
        truncated = truncated or trunc
        if term or trunc:
            break
    assert truncated, "time-limit must truncate"
```

- [ ] **Step 3:** `PYTHONPATH=python python3 -m pytest python/tests/test_thrust_mode.py python/tests/test_gym_smoke.py -v` → ALL pass (old waypoint smoke must still pass: mode default is waypoint).

- [ ] **Step 4: Commit**

```bash
git add python/tests/test_thrust_mode.py
git commit -m "test(py): thrust-mode env behaviour — seeded targets, reproducibility, reward direction, truncation"
```

## Task 5: Training stack — PPO + curriculum + logging

**Files:** Create `python/train/train_flight.py`, `python/train/curriculum.py`, `python/tests/test_learning_smoke.py`

- [ ] **Step 1: Deps.** `python3 -c "import stable_baselines3, torch"` — if missing: `pip install --user stable-baselines3` (pulls torch; CPU build is fine). Record installed versions in the commit message.

- [ ] **Step 2: `python/train/curriculum.py`:**

```python
"""Success-ratcheted curriculum: increasingly long sprints, gravity woven in.

Ratchets on ROLLING ARRIVAL RATE crossing a threshold — never on a fixed
schedule. Stage parameters feed JumpgateGymEnv.set_difficulty()."""
from dataclasses import dataclass

@dataclass(frozen=True)
class Stage:
    name: str
    target_dist_min: float
    target_dist_max: float
    star_mass: float
    exhaust_velocity: float
    time_limit: int

STAGES = [
    #      name              dmin    dmax   star  v_e   ticks
    Stage("hop-no-gravity",  0.001,  0.005, 0.0,  0.1,   400),
    Stage("sprint",          0.005,  0.05,  0.0,  0.1,  1000),
    Stage("well-shallow",    0.005,  0.05,  0.3,  0.1,  1000),
    Stage("well-full",       0.02,   0.2,   1.0,  0.2,  2500),
    Stage("orbital",         0.1,    0.5,   1.0,  0.3,  6000),
]
PROMOTE_AT = 0.8     # rolling arrival rate to advance
WINDOW = 200         # episodes in the rolling window

class Curriculum:
    def __init__(self):
        self.idx, self.results = 0, []
    @property
    def stage(self):
        return STAGES[self.idx]
    def record(self, arrived: bool) -> bool:
        """Record an episode; return True if we just promoted."""
        self.results.append(arrived)
        recent = self.results[-WINDOW:]
        if (len(recent) == WINDOW
                and sum(recent) / WINDOW >= PROMOTE_AT
                and self.idx < len(STAGES) - 1):
            self.idx += 1
            self.results = []
            return True
        return False
```

- [ ] **Step 3: `python/train/train_flight.py`:**

```python
"""Train PPO (MLP) to fly thrust-mode jumpgate. Logs arrival-rate/return/fuel
per rollout to CSV; checkpoints per stage promotion. Run:
    PYTHONPATH=python python3 python/train/train_flight.py --steps 2000000
"""
import argparse, csv, pathlib
import numpy as np
from stable_baselines3 import PPO
from stable_baselines3.common.callbacks import BaseCallback
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor, VecNormalize
from jumpgate.gym_env import JumpgateGymEnv
from curriculum import Curriculum

GAMMA = 0.99  # MUST match FlightCfg.gamma (potential-shaping invariant)

def make_env(stage):
    def _f():
        env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
        env.set_difficulty(
            target_dist_min=stage.target_dist_min, target_dist_max=stage.target_dist_max,
            star_mass=stage.star_mass, exhaust_velocity=stage.exhaust_velocity,
            time_limit=stage.time_limit, gamma=GAMMA,
        )
        return env
    return _f

class CurriculumCallback(BaseCallback):
    """Feeds episode outcomes to the curriculum; rebuilds envs on promotion;
    logs (step, stage, arrival_rate, ep_return) rows to runs/flight_log.csv."""
    def __init__(self, cur: Curriculum, log_path):
        super().__init__()
        self.cur, self.rows = cur, []
        self.log_path = pathlib.Path(log_path)
    def _on_step(self) -> bool:
        for info in self.locals.get("infos", []):
            ep = info.get("episode")
            if ep is None:
                continue
            arrived = bool(info.get("is_success", info.get("terminated", False)))
            promoted = self.cur.record(arrived)
            recent = self.cur.results[-50:]
            rate = (sum(recent) / len(recent)) if recent else 0.0
            self.rows.append([self.num_timesteps, self.cur.stage.name, f"{rate:.3f}", f"{ep['r']:.3f}"])
            if promoted:
                print(f"PROMOTED -> {self.cur.stage.name} at {self.num_timesteps} steps")
                self.model.save(f"runs/flight_{self.cur.stage.name}_entry")
                self.training_env.env_method("set_difficulty",
                    target_dist_min=self.cur.stage.target_dist_min,
                    target_dist_max=self.cur.stage.target_dist_max,
                    star_mass=self.cur.stage.star_mass,
                    exhaust_velocity=self.cur.stage.exhaust_velocity,
                    time_limit=self.cur.stage.time_limit, gamma=GAMMA)
        return True
    def _on_training_end(self):
        self.log_path.parent.mkdir(exist_ok=True)
        with open(self.log_path, "w", newline="") as f:
            csv.writer(f).writerows([["step", "stage", "arrival_rate", "ep_return"], *self.rows])

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--steps", type=int, default=2_000_000)
    p.add_argument("--n-envs", type=int, default=8)
    p.add_argument("--seed", type=int, default=0)
    args = p.parse_args()

    cur = Curriculum()
    venv = VecNormalize(VecMonitor(DummyVecEnv([make_env(cur.stage) for _ in range(args.n_envs)])),
                        norm_obs=True, norm_reward=True, gamma=GAMMA)
    model = PPO("MlpPolicy", venv, gamma=GAMMA, n_steps=2048, batch_size=256,
                learning_rate=3e-4, ent_coef=0.01, seed=args.seed, verbose=1)
    model.learn(total_timesteps=args.steps, callback=CurriculumCallback(cur, "runs/flight_log.csv"))
    model.save("runs/flight_final"); venv.save("runs/flight_vecnorm.pkl")

if __name__ == "__main__":
    main()
```

(`is_success`: set `info["is_success"] = terminated` in the gym wrapper's step
return when terminated — one-line addition to `gym_env.py`; SB3 Monitor passes
it through. NOTE: `set_difficulty` via `env_method` reaches through
Monitor/Normalize wrappers to the base env — verify it lands (DummyVecEnv
`env_method` calls the innermost gym.Env attribute); if wrapped lookup fails,
unwrap via `venv.unwrapped.envs[i].unwrapped`.)

- [ ] **Step 4: `python/tests/test_learning_smoke.py`** — the "learning happens" check (slow-marked, ~2-4 min CPU; a CHECK that training moves the needle, not a performance gate):

```python
import numpy as np, pytest
from stable_baselines3 import PPO
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor
from jumpgate.gym_env import JumpgateGymEnv

@pytest.mark.slow
def test_ppo_reduces_distance_on_easiest_stage():
    def f():
        e = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
        e.set_difficulty(target_dist_min=0.001, target_dist_max=0.003,
                         star_mass=0.0, time_limit=200)
        return e
    venv = VecMonitor(DummyVecEnv([f] * 4))

    def mean_final_dist(model_or_none, episodes=20):
        env = f(); dists = []
        for ep in range(episodes):
            obs, _ = env.reset(seed=1000 + ep)
            done = False
            while not done:
                if model_or_none is None:
                    act = env.action_space.sample()
                else:
                    act, _ = model_or_none.predict(obs, deterministic=True)
                obs, _, term, trunc, _ = env.step(act)
                done = term or trunc
            dists.append(np.linalg.norm(obs[4:7]))
        return float(np.mean(dists))

    before = mean_final_dist(None)
    m = PPO("MlpPolicy", venv, n_steps=512, batch_size=128, seed=0, verbose=0)
    m.learn(total_timesteps=40_000)
    after = mean_final_dist(m)
    assert after < before * 0.7, f"PPO must measurably close distance: {before:.4f} -> {after:.4f}"
```

- [ ] **Step 5:** Run it: `PYTHONPATH=python python3 -m pytest python/tests/test_learning_smoke.py -v -m slow` → PASS. **This is the moment the pipeline is proven to learn. If it fails, STOP and debug (reward sign, obs scaling, action decode) — do not proceed to Task 6 with a non-learning pipeline.**

- [ ] **Step 6: Commit**

```bash
git add python/train/train_flight.py python/train/curriculum.py python/tests/test_learning_smoke.py
git commit -m "feat(train): SB3 PPO flight training — success-ratcheted curriculum, CSV telemetry, learning-happens smoke"
```

## Task 6: Watch it learn — trajectory renderer

**Files:** Create `python/train/render.py`

- [ ] **Step 1:**

```python
"""Render flight trajectories: random policy vs trained policy, side by side.
    PYTHONPATH=python python3 python/train/render.py runs/flight_final.zip out.png
"""
import sys
import numpy as np
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from stable_baselines3 import PPO
from jumpgate.gym_env import JumpgateGymEnv

def rollout(model, seed, max_steps=2000):
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
    env.set_difficulty(target_dist_min=0.003, target_dist_max=0.003, star_mass=0.0)
    obs, _ = env.reset(seed=seed)
    target = obs[4:7].copy()            # rel pos at t0 == target in start frame
    pts, pos = [np.zeros(3)], np.zeros(3)
    for _ in range(max_steps):
        act = env.action_space.sample() if model is None \
              else model.predict(obs, deterministic=True)[0]
        nobs, _, term, trunc, _ = env.step(act)
        pos = target - nobs[4:7] * np.linalg.norm(target) / max(np.linalg.norm(obs[4:7]), 1e-12) \
              if False else pos  # placeholder-free: reconstruct from rel-pos delta below
        pos = target - nobs[4:7]        # craft pos in start frame = target - rel
        pts.append(pos.copy()); obs = nobs
        if term or trunc:
            return np.array(pts), target, bool(term)
    return np.array(pts), target, False

def panel(ax, traj, target, arrived, title):
    ax.plot(*traj.T); ax.scatter(*target, marker="*", s=200)
    ax.scatter(0, 0, 0, marker="o")
    ax.set_title(f"{title} — {'ARRIVED' if arrived else 'failed'} ({len(traj)} ticks)")

def main():
    model_path, out = sys.argv[1], sys.argv[2]
    fig = plt.figure(figsize=(14, 6))
    t1, tgt1, ok1 = rollout(None, seed=5)
    panel(fig.add_subplot(121, projection="3d"), t1, tgt1, ok1, "random policy")
    t2, tgt2, ok2 = rollout(PPO.load(model_path), seed=5)
    panel(fig.add_subplot(122, projection="3d"), t2, tgt2, ok2, "trained policy")
    fig.savefig(out, dpi=120)
    print(f"wrote {out}: random={'ARRIVED' if ok1 else 'failed'}, trained={'ARRIVED' if ok2 else 'failed'}")

if __name__ == "__main__":
    main()
```

NOTE the obs rel-pos is scaled by `dist_scale` (Task 2); multiply `nobs[4:7]`
by the stage's `target_dist_max` to undo before plotting (use the value passed
to `set_difficulty`). Fix the two `pos` lines accordingly — final code must
reconstruct `pos = target − rel_pos_unscaled`; delete the dead `if False` line.

- [ ] **Step 2:** Smoke it on the learning-smoke artifact or an untrained model: `python3 python/train/render.py <model> /tmp/traj.png` → file exists, prints the two outcomes.

- [ ] **Step 3: Commit**

```bash
git add python/train/render.py
git commit -m "feat(train): trajectory renderer — random vs trained, the watch-it-learn artifact"
```

## Task 7: The first real run (build → train → WATCH)

- [ ] **Step 1:** Full verification sweep first: `cargo test -p jumpgate-core && cargo test -p jumpgate-py && cargo clippy --all-targets && PYTHONPATH=python python3 -m pytest python/tests/ -v -m "not slow"` → all green.
- [ ] **Step 2:** Launch the real training run (background, ~hours on CPU): `PYTHONPATH=python python3 python/train/train_flight.py --steps 2000000 --n-envs 8 2>&1 | tee runs/train.log`
- [ ] **Step 3:** Read the windows as it runs: `runs/flight_log.csv` arrival-rate trend per stage; render checkpoints as they appear. **The bar (spec §1): arrival rate climbs across stages, and the render shows flailing→flying.** Tuning (reward weights, stage thresholds, PPO hyperparams) happens HERE, at the console, by watching — expect iterations; that is the design, not a failure.
- [ ] **Step 4:** When stage progression is demonstrable, commit run artifacts (log CSV + a flailing-vs-flying render PNG) under `runs/` if the owner wants them in-tree (ASK first — `runs/` may belong in `.gitignore`), update `jumpgate-3497ef9e6e` in filigree, and report to the owner with the curves + renders.

---

## Self-review (done at write time)

- **Spec coverage:** §4A→Task 1; §4B→Tasks 2-4; §4C→Task 5; §4D→Task 6; §8 loop→Task 7; §6 failure modes→Task 4's reward-direction test + Task 5's smoke + Task 7 console watch. Deferral hygiene→Task 0.
- **Placeholders:** none; the one near-placeholder (render pos-reconstruction) is explicitly flagged with the exact fix required before commit.
- **Type consistency:** `THRUST_OBS_DIM=10`/`VEL_SCALE` (Task 2) consumed in Task 3 obs writes and Task 4/6 index assumptions (`obs[4:7]` = scaled rel-pos); `FlightCfg.gamma`=`GAMMA`=0.99 across Tasks 3/5; `set_difficulty` kwargs match `FlightCfg` fields and `Stage` fields.
- **Known risks, named:** SB3 `env_method` reach-through on promotion (verify-or-unwrap note in Task 5); auto-reset semantics must match SB3 VecEnv expectations (Task 3 wires it; Task 4 tests truncation); torch install size; reset-guard caps acceleration so episode lengths are long at orbital scale (time limits sized for it in STAGES).
