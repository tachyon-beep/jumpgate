"""Trader Rung-1 baseline policies + eval harness (spec §1, §7.3).

Two scripted policies over the 20-dim trader obs (4 slots x [present, reward,
d_pickup, d_haul] then own state):

- random_valid: uniform over PRESENT board slots, wait iff the board is empty.
  The gated comparator — PPO must beat its mean held-out credits (smoke test).
- greedy_reward: argmax of the slot reward dim, wait iff empty. REPORTED, not
  gated: beating greedy means the policy reads geometry, not just reward.

Run: PYTHONPATH=python python3 python/train/baselines.py
"""

import numpy as np

from jumpgate.gym_env import JumpgateGymEnv

BOARD_SLOTS = 4
HELD_OUT_SEEDS = list(range(10_000, 10_025))  # NEVER train on these
# An episode is ~3-40 decisions (horizon-truncated); this is a safety bound,
# not a tuning knob.
MAX_DECISIONS = 512


def random_valid(obs, rng) -> int:
    """Uniform over present slots; wait (0) iff none present."""
    present = [j for j in range(BOARD_SLOTS) if obs[4 * j] > 0.5]
    if not present:
        return 0
    return int(rng.choice(present)) + 1


def greedy_reward(obs) -> int:
    """Accept the highest-reward present slot; wait (0) iff the board is empty."""
    rewards = [
        obs[4 * j + 1] if obs[4 * j] > 0.5 else -1.0 for j in range(BOARD_SLOTS)
    ]
    return (int(np.argmax(rewards)) + 1) if max(rewards) > 0.0 else 0


def rollout_policy(env_seed: int, policy_fn, env=None) -> float:
    """Roll one full episode under policy_fn(obs) -> action; return credits.

    Pass `env` to reuse one native env across rollouts (explicit-seed reset
    restores bit-identical scenarios, so reuse does not leak state).
    """
    if env is None:
        env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
    obs, _ = env.reset(seed=env_seed)
    total = 0.0
    for _ in range(MAX_DECISIONS):
        obs, r, term, trunc, _ = env.step(policy_fn(obs))
        total += r
        if term or trunc:
            break
    return total


def evaluate(policy_fn, seeds) -> tuple[float, float]:
    """Mean and sd of episode credits for policy_fn over the given seeds."""
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
    credits = [rollout_policy(s, policy_fn, env=env) for s in seeds]
    return float(np.mean(credits)), float(np.std(credits))


def main() -> None:
    rng = np.random.default_rng(0)
    rnd_mean, rnd_sd = evaluate(lambda obs: random_valid(obs, rng), HELD_OUT_SEEDS)
    grd_mean, grd_sd = evaluate(greedy_reward, HELD_OUT_SEEDS)
    print(f"random-valid over {len(HELD_OUT_SEEDS)} held-out seeds: "
          f"mean {rnd_mean:.3f} +/- {rnd_sd:.3f} credits")
    print(f"greedy-reward over {len(HELD_OUT_SEEDS)} held-out seeds: "
          f"mean {grd_mean:.3f} +/- {grd_sd:.3f} credits")


if __name__ == "__main__":
    main()
