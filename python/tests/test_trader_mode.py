"""Trader-mode (strategic Rung 1) wrapper tests.

Obs layout (TRADER_OBS_DIM = 20, fixed global scales, spec §5.3):
4 board slots × [present, reward, d_pickup, d_haul] at dims 4j..4j+4,
then own [fuel_frac, credits, busy, time_remaining_frac] at dims 16..20.
Action space is Discrete(5): 0 = wait, j = accept board slot j-1.
"""

import gymnasium as gym
import numpy as np

from jumpgate.gym_env import JumpgateGymEnv


def _make():
    return JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")


def test_trader_obs_shape_and_action_space():
    env = _make()
    obs, _ = env.reset(seed=3)
    assert obs.shape == (20,)
    assert isinstance(env.action_space, gym.spaces.Discrete)
    assert env.action_space.n == 5
    assert 0.0 <= obs[19] <= 1.0  # time_remaining_frac
    assert obs[0] in (0.0, 1.0)  # slot-0 present flag
    # Seeded scenario starts with a 4-route board: every slot present.
    assert all(obs[4 * j] == 1.0 for j in range(4))


def test_trader_accept_eventually_pays():
    env = _make()
    obs, _ = env.reset(seed=3)
    total, info = 0.0, {}
    # Greedy: always accept the highest-reward present slot, wait iff empty.
    for _ in range(64):
        rewards = [obs[4 * j + 1] if obs[4 * j] > 0.5 else -1.0 for j in range(4)]
        act = (int(np.argmax(rewards)) + 1) if max(rewards) > 0 else 0
        obs, r, term, trunc, info = env.step(act)
        total += r
        assert not term, "trader mode is a continuing task: never terminated"
        # episode_credits rides info on EVERY step and tracks the running sum
        # (on the truncation step it is the episode's final total).
        assert np.isclose(info["episode_credits"], total)
        if trunc:
            break
    assert trunc, "the horizon must truncate within 64 greedy decisions"
    assert total > 0.0, "a full episode of greedy accepts must earn credits"


def test_trader_explicit_seed_reproduces_and_unseeded_varies():
    # Mirrors test_thrust_mode: SB3 VecEnvs reset() without a seed on every
    # episode end; each such reset must present FRESH seed-derived geometry,
    # while an explicit seed restores exact reproducibility.
    env = _make()
    a, _ = env.reset(seed=7)
    b, _ = env.reset()  # unseeded: fresh derived scenario
    c, _ = env.reset()  # unseeded again: fresh again
    assert not np.allclose(a, b), "unseeded reset must change the geometry"
    assert not np.allclose(b, c), "every unseeded reset must differ"
    d, _ = env.reset(seed=7)  # explicit: back to the seed-7 scenario exactly
    assert np.array_equal(a, d), "explicit seed must reproduce bit-identically"
