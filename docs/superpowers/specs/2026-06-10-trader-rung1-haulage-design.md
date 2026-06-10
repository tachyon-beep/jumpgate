# Trader Rung 1 — Contract Haulage (strategic layer over the live economy)

**Date:** 2026-06-10
**Status:** Approved direction (owner: "Continue on with the trader arc … use ultracode to crack on")
**Predecessor:** Tactical Flight Rung 1 (closed, jumpgate-3497ef9e6e) — PPO provably learns
Newtonian thrust-to-rendezvous through the real sim.
**Frame:** PDR-0006. This is a GAME rung: the agent learns to *trade* — read a contract
board, weigh reward against route geometry and remaining time, and earn more credits than
a policy that can't. DRL is a player; the economy is the game.

## 1. What this rung proves

Wire PPO to the **strategic layer** and watch it learn the concept of *taking a job for
profit*: accept a delivery contract, let the ship fly itself (autopilot Rung 0), get paid
on delivery, and learn that **which** contract you take — and when to decline — determines
how rich you end up. The proof bar is the same "prove it works" bar as the flight rung:

> A PPO policy must earn significantly more credits per episode than a random-valid-choice
> baseline on held-out seeds, within a minutes-scale training budget. (Comparison against a
> greedy highest-reward baseline is *reported*, not gated — beating greedy means the policy
> reads geometry, which is the interesting game-science result.)

## 2. Two-layer architecture (unchanged)

- **Strategic (this rung, learned):** which contract to accept, or wait. One decision at a
  time, at decision points.
- **Tactical (delegated, free):** `resolve_contracts`/`try_load` already dispatch the craft
  via `NavState::Seeking` — deadhead to pickup, load, haul, deliver, escrow→credits payout.
  Zero new flight code. The learned thrust pilot (Rung 1) composes in later via
  jumpgate-c260947d7a; not on this critical path.

## 3. What exists (verified in code, 2026-06-10)

- Full contract lifecycle: `Offered→Accepted→CargoLoaded→InTransit→Completed/Failed`,
  escrow settlement into `ships.credits_micros`, credit identity Σtreasury+Σcredits+Σescrow
  invariant (economy.rs, tested in world.rs).
- `CommandKind::AcceptContract` through the single ingest path: records intent iff the
  contract is `Offered` and unclaimed; everything else is deferred to `resolve_contracts`.
- `run_scripted_dispatch` REPOST: order-up-to with hysteresis keeps a route's contracts
  flowing as destination stock drains (sinks consume → demand re-fires).
- Producers (miners/sinks), linear demand-deflation pricing, orbiting stations (bodies on
  Kepler orbits, live co-orbiting rendezvous arrival).

## 4. Gaps found (and their resolutions)

1. **ASSIGN steals the agent's decision** — `run_scripted_dispatch` part (b) auto-claims
   *any* `Idle` craft for the lowest Offered contract and has **no config gate** (the inert
   `DispatchCfg::default()` only disables REPOST). Resolution: `stagger_period == 0` ⇒
   ASSIGN disabled entirely (REPOST unaffected). Hash-neutral: no existing config uses 0
   (it is currently `.max(1)`-coerced), `DispatchCfg` folding is unchanged.
2. **No economy read surface for the gym** — `StateView` carries physics only and World's
   economy stores are `pub(crate)`. Resolution: four narrow read-only `World` accessors
   (§6.1). No hash impact, no determinism impact.
3. **jumpgate-2c0c2d92bb** (filed P3): propellant exhaustion mid-deadhead (status
   `Accepted`) locks escrow forever — `resolve_failures` only handles `InTransit`.
   Resolution for this rung: size `fuel_capacity` ≥ 3× the measured greedy-rollout burn so
   it cannot trigger. The bug remains filed; it becomes critical-path only when fuel gets
   scarce (the fuel-economy rung).

## 5. The environment (semi-MDP, macro-step)

One Python `step()` = **one strategic decision**, not one tick. The native env advances
world ticks internally between decisions (10k ticks/s makes this cheap). Episodes are
~15–40 decisions over a `horizon`-tick budget.

### 5.1 Decision points

The agent is consulted when its craft is **idle** (role `Idle`, no bound contract). After
an accept, the world auto-steps until the contract resolves terminally (delivery or
failure) or the horizon hits. After a wait, the world advances `WAIT_TICKS = 8` ticks.
An accept of an empty/stale slot ingests as a no-op (craft stays idle) and behaves as a
1-tick wait — deterministic, no special casing.

### 5.2 Action space — `Discrete(M+1)`, M = 4 board slots

- `0` = wait (advance 8 ticks).
- `j ∈ 1..=4` = issue `AcceptContract` for board slot `j-1`.

Board slots are the lowest-`M` `Offered` contracts in dense row order (deterministic).
The native action buffer stays f32 (`action_dim = 1`, value = the index, decoded with
`round()`); the Python wrapper exposes `gym.spaces.Discrete(M+1)` in trader mode.

### 5.3 Observation — `TRADER_OBS_DIM = 20`, fixed global scales, **no VecNormalize**

Flight-rung lesson (run 6): running obs normalization whipsaws under non-stationarity;
build stationary obs by construction instead. All scales are compile-time constants.

Per board slot `j` (4 dims × 4 slots = 16):

| dim | value | scale |
|-----|-------|-------|
| 0 | present (1.0 / 0.0) | — |
| 1 | `reward_micros / TRADER_REWARD_SCALE` | `TRADER_REWARD_SCALE = 3_000_000` (max seeded route reward) |
| 2 | dist(craft → from_station) | `TRADER_DIST_SCALE = 2.4` AU |
| 3 | dist(from_station → to_station) | `TRADER_DIST_SCALE` |

Own state (4 dims): `fuel_frac`, `credits_micros / (10 * TRADER_REWARD_SCALE)`,
`busy` (1.0 if non-idle — always 0.0 at decision points in v1, reserved),
`time_remaining_frac = (horizon − tick) / horizon`.

Empty slots write zeros. Station positions are sampled at the current tick (orbits move).

### 5.4 Reward — Δcredits, the game's own currency

`reward = (credits_micros_after − credits_micros_before) / 1e6` accumulated over the
macro-step. No shaping, no fuel term, no time penalty: time cost is real (a slow trip
forfeits later contracts inside the fixed horizon) and the agent sees
`time_remaining_frac`. PPO `gamma = 0.999` (near-undiscounted episodic credit total —
the SMDP per-decision discount must not punish long-but-lucrative trips).

### 5.5 Episode

- `terminated` = never (continuing task). `truncated` = tick ≥ `horizon` (default 2000).
  Truncation mid-trip pays nothing for the unfinished contract — *don't start what you
  can't finish* is a real strategic skill, learnable from `time_remaining_frac`.
- Auto-reset on truncation (SB3 VecEnv contract), same derived-seed scheme as thrust mode
  (`master ^ episode_counter`).
- `info["episode_credits"]` carries the final credit total for logging.

## 6. Scenario (trader template, built in jumpgate-py)

One star (1 M_sun). Four bodies on circular orbits, `a = {0.35, 0.55, 0.8, 1.1}` AU, one
station each. **Initial mean anomalies are derived from the reset seed** (deterministic
per-seed, varied across episodes) — the anti-memorization requirement: geometry differs
every episode, so the policy must read the board, not replay a script.

- Producers: Ore miners at stations 0 and 2; Ore demand-sinks at stations 1 and 3.
- One corporation, treasury large enough that escrow never reverts an accept.
- Four seeded routes with **rate-mispriced rewards** (reward NOT proportional to expected
  trip time, so rate-maximization is a judgment, and orbital phase shifts which route is
  best): R0: 0→1 @ 1.0 cr; R1: 2→3 @ 1.2 cr; R2: 0→3 @ 1.6 cr; R3: 2→1 @ 3.0 cr.
- `DispatchCfg { stagger_period: 0 /* ASSIGN OFF */, demand_low/demand_high tuned so each
  route reposts soon after completion (board usually holds 2–4 offers) }`.
- One agent craft. Craft spec lifted from the proven world.rs forage-loop fixture; horizon
  calibrated so a greedy rollout completes ≥ 6 deliveries; fuel ≥ 3× greedy-rollout burn.
- Exact stock/qty/interval numbers are build-time calibrations, locked by tests
  (board-stays-stocked over a scripted long rollout), not spec constants.

## 7. Changes by layer

### 7.1 jumpgate-core (minimal, hash-neutral, golden-untouched)

1. `run_scripted_dispatch`: `if dispatch.stagger_period == 0 { return; }` placed between
   REPOST and ASSIGN (REPOST still runs). Doc on `DispatchCfg::stagger_period`: `0` =
   manual/RL acceptance only.
2. Four read-only `World` accessors (plain `impl World`, not `StateView` — no trait churn):
   - `pub fn offered_contracts(&self) -> Vec<(ContractId, i64, StationId, StationId)>`
     (id, reward_micros, from, to; dense row order; `Offered` + unclaimed only)
   - `pub fn station_pos(&self, id: StationId) -> Option<Vec3>` (station's body at current tick)
   - `pub fn craft_credits(&self, id: CraftId) -> Option<i64>`
   - `pub fn craft_is_idle(&self, id: CraftId) -> Option<bool>` (role `Idle` ∧ no contract)

No store layout, hash fold, or stepping change ⇒ `HASH_FORMAT_VERSION` and both goldens
untouched (verified by diff grep at land time).

### 7.2 jumpgate-py

- `obs.rs`: `TRADER_OBS_DIM = 20`, scales, `write_obs_trader(...)`.
- `env.rs`: mode 2; `TraderCfg { horizon, wait_ticks, board_slots(=4 const) }`;
  `trader_config_template(seed)`; `reset_trader_episode`; `step_trader` (the macro-step
  loop of §5.1); auto-reset. `configure(mode=2, time_limit=horizon)` reuses the existing
  kwarg (no new configure args in v1).

### 7.3 python/

- `gym_env.py`: `"trader"` mode → `Discrete(M+1)` action space, f32 index marshalling,
  `info["episode_credits"]` passthrough.
- `train/baselines.py`: random-valid and greedy-highest-reward rollout policies + eval
  harness (N episodes, held-out seeds, mean±sd credits).
- `train/train_trader.py`: PPO MlpPolicy, `gamma=0.999`, **no VecNormalize**, 8 envs,
  CSV log (decision, tick, credits, action, stage of episode), `--log-path` discipline.
- `tests/test_trader_mode.py`: obs shape/scales; accept→delivery→Δcredits flow on a fixed
  seed; explicit-seed reproducibility; unseeded resets vary geometry.
- `tests/test_trader_learning_smoke.py`: **the keystone gate** — short-budget PPO beats
  the random-valid baseline mean credits on held-out seeds (margin + budget calibrated at
  build, target < ~2 min of pytest).

## 8. What this rung is NOT (YAGNI)

- No merchant Buy/Sell verbs (that is Trader Rung 2: owning cargo and price risk against
  the live `update_prices` curve — new core CommandKinds, designed after this lands).
- No competitor haulers, no pirates, no refuel, no multi-craft fleets, no LSTM (full
  observability here; memory earns entry with partial observability later).
- No curriculum in v1 — one scenario class, seed-varied. If the smoke shows it's needed,
  stage horizon/board-size, but do not pre-build it.

## 9. Risks

- **Sparse-ish reward:** ~one payout per 2–4 decisions is far denser than per-tick
  sparsity (the SMDP framing does the compression), but if PPO stalls, the fallback is a
  small per-delivery completion bonus already inherent (payout) — next lever is shorter
  horizon, not shaping. Watch entropy collapse per flight-rung lesson.
- **Board churn during a trip:** offers present at accept time may be gone at the next
  decision; obs is rebuilt fresh each decision point — Markov per-slot features, no
  cross-step slot identity assumed.
- **Macro-step wall-clock variance:** a step costs 8–450 ticks of native time; SB3
  rollout collection is unaffected (it counts steps, not ticks).
