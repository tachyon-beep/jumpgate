# Tactical Flight, Rung 1 — a PPO agent learns Newtonian thrust-to-target

> **FRAME.** This is a **make-it-work** proof, not a theorem. Success is **empirical**:
> wire a real PPO agent through the existing gym and *watch it learn* to pilot a craft in
> the Newtonian sim. GAME science (a capability that makes the game playable), never game
> SCIENCE. No presolvability gate anywhere near this.

Date: 2026-06-10   Status: DRAFT (awaiting owner review)   Author: acting-PM (Claude)
Relates: `jumpgate-3497ef9e6e` (navigator first-rung). Builds the **tactical layer, Rung 1**
of the two-layer agent architecture (strategic = *what to do*; tactical = *how to fly*,
a capability ladder from autopilot-delegated to learned-thrust).

---

## 1. Goal & success (empirical)

A PPO agent, training through the existing PyO3 gym, learns to **pilot a craft via direct
thrust** to a **randomized target** in the real Newtonian sim. Success = the agent's
arrival rate climbs over training, across a curriculum of **increasingly long sprints with
gravity woven in**, and we can **watch** a craft go from flailing to flying (trajectory
render + learning curves). That is the whole bar: *it learns, and we can see it.*

## 2. What is already real (proven, not assumed)

Ran from Python this session: `gym.reset()/step()` drives the real Rust sim — a craft
thrusts (fuel depletes), gravity curves its orbit, obs come back, reproducible. The
autopilot/`Destination` path is the **capability-ladder Rung 0** (kept, not discarded).
This spec builds **Rung 1** (learned thrust). Confirmed gaps the run exposed: action is a
waypoint (not raw thrust); 28/32 obs values are always zero; reward is a flat time penalty
with no goal.

## 3. Scope boundary (what this does NOT do)

No strategic layer (job/target selection), no cargo, no danger/pirates, no multi-agent, no
hierarchy, **no LSTM/Transformer** (MLP — the task is fully observed/Markovian; recurrence
is earned later when hidden state/partial-observability actually arrives).

## 4. Components

### A. Core — `CommandKind::Thrust` (the ONLY core change)
- Additive enum variant `Thrust { thrust: Vec3 }` (world-frame thrust intent).
- Ingest clamps `|thrust|` to the craft's max thrust, fuel-limits it, and applies it as the
  craft's commanded thrust for the tick **through the same channel the autopilot's output
  uses, bypassing the autopilot controller**. Deterministic f64; recorded in the command
  stream so replay is bit-identical.
- Determinism: existing goldens untouched (they use `Destination`); a new thrust scenario
  gets its own record/replay test. No hash-format change (a new `Command` variant is an
  input, not hashed state).
- Tests: a `Thrust` command accelerates the craft along the vector by ~`F/m·dt`; fuel
  depletes; zero-thrust is a no-op; over-budget thrust is clamped; record→replay identical.

### B. Gym env (`env.rs`) — the flight task
- `control_mode`: `Waypoint` (Rung 0, existing) | `Thrust` (Rung 1, new). This proof uses
  `Thrust`. Keeping both bakes the ladder into the env.
- **Action** (thrust mode): `[tx,ty,tz] ∈ [-1,1]³` → scaled to max thrust → `CommandKind::Thrust`.
  `ACTION_DIM = 3` in thrust mode.
- **Target**: a goal position drawn from the **seed at reset**, at a curriculum-controlled
  distance + random direction. Held in the env (not core) — this is the per-episode
  variation that fixes the static-env gap, entirely env-side.
- **Obs** (thrust mode): own velocity (3) + fuel fraction (1) + target rel-pos (3) +
  target rel-vel (3) = the real, meaningful signal (replaces the zero padding). Frame-
  relative with the existing f32-boundary guard.
- **Reward** (the part to run to ground — v1):
  - **Potential-based shaping**, `Φ(s) = −distance_to_target`, reward `+= γΦ(s′) − Φ(s)`.
    Potential-based *specifically* so it cannot be farmed by dithering near the target.
  - **Arrival bonus** (large `+`) on reaching the target sphere **at low relative speed**
    (rendezvous, not flyby — mirrors the core arrival-speed gate).
  - **Costs**: `−` fuel spent, `−` a tiny per-tick time penalty.
- `terminated` = low-speed arrival; `truncated` = time limit.
- **Difficulty knob** (curriculum): target distance + gravity strength (e.g. craft-to-star
  distance or star mass), settable from Python via a reset option / setter.

### C. Python training (`python/`, SB3)
- PPO + MLP policy over `JumpgateGymEnv` (thrust mode). Add `stable-baselines3` + `torch`
  to deps.
- **Curriculum scheduler**: ramp difficulty (first target distance — the "increasingly
  long sprints" — then gravity strength) as the **rolling arrival rate** crosses thresholds;
  ratchet on *success*, not a fixed schedule.
- **Logging**: arrival rate, episode return, fuel-per-arrival, over training (CSV +
  optional tensorboard).

### D. Watch-it-learn tooling
- A rollout + render script: load a policy, run episodes, plot 3D trajectories
  (matplotlib) — a *random* policy vs a *trained* policy, side by side, so flailing→flying
  is visible. This is the deliverable that satisfies "watch it learn."

## 5. Determinism

The core stays deterministic; thrust application is deterministic; replay of a recorded
action stream is bit-identical (the lab property is preserved). Training stochasticity is
policy-side only — the env *transition* given `(state, action)` is deterministic. New
scenarios only; existing goldens untouched.

## 6. Reward failure modes to watch (named, so we recognise them at the console)

- **Dithering** near the target to farm shaping → prevented by potential-based shaping.
- **Fuel-refusal** (learns that not moving avoids the fuel penalty) → arrival bonus must
  outweigh the fuel saved by sitting still; watch for a policy that never thrusts.
- **Flyby/overshoot** (reaches the sphere at high speed) → the low-speed arrival gate.

## 7. Testing

- **Rust**: `Thrust` unit tests (§4A); env obs correctness (target-relative vectors),
  reward sign (potential shaping), arrival detection; a thrust-scenario record/replay
  determinism test.
- **Python**: a thrust-mode smoke (reset/step/obs-shape/reward-not-constant); a **short
  training smoke** — PPO measurably reduces mean distance-to-target on the easiest stage
  within a few k steps (a *learning-happens* check, NOT a performance gate).

## 8. Build → train → watch loop

Build the machinery (TDD), then **train and watch**: run the curriculum, read the learning
curves + trajectory renders, and **tune the reward/curriculum until a craft reliably flies
to target across the sprints**. "Make it work" = a trained agent visibly flying. The reward
is found at the console by watching, not declared up front.
