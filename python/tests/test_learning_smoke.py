"""Learning-happens smoke (slow): PPO must measurably close mean final
distance-to-target vs a random policy on the easiest stage within 40k steps.

A CHECK that training moves the needle, not a performance gate. Two wiring
adaptations vs the plan listing (thresholds and step budget untouched):

- The native env AUTO-RESETS on done and overwrites the returned obs with the
  NEW episode's initial obs (SB3 VecEnv contract), so the final distance must
  be read from the last in-episode obs (truncation) or bounded by the scaled
  arrival radius (termination) — never from the post-auto-reset obs.
- JumpgateGymEnv.reset(seed=None) maps to native seed 0, and DummyVecEnv
  calls env.reset() (no seed) after every done; a fresh-seed wrapper keeps
  per-episode target diversity during training.
"""
import gymnasium as gym
import numpy as np
import pytest
from stable_baselines3 import PPO
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

from jumpgate.gym_env import JumpgateGymEnv

DIST_MIN, DIST_MAX = 0.001, 0.003
ARRIVAL_RADIUS = 1.0e-4  # FlightCfg default (AU); obs rel-pos is scaled /DIST_MAX


def _base_env():
    e = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
    e.set_difficulty(target_dist_min=DIST_MIN, target_dist_max=DIST_MAX,
                     star_mass=0.0, time_limit=200)
    return e


class _FreshSeedOnReset(gym.Wrapper):
    """reset(seed=None) -> fresh seed (see module docstring)."""

    def __init__(self, env, base_seed: int):
        super().__init__(env)
        self._rng = np.random.default_rng(base_seed)

    def reset(self, *, seed=None, options=None):
        if seed is None:
            seed = int(self._rng.integers(0, 2**31 - 1))
        return self.env.reset(seed=seed, options=options)


@pytest.mark.slow
def test_ppo_reduces_distance_on_easiest_stage():
    venv = VecMonitor(DummyVecEnv(
        [(lambda i=i: _FreshSeedOnReset(_base_env(), base_seed=100 + i)) for i in range(4)]
    ))

    def mean_final_dist(model_or_none, episodes=20):
        env = _base_env()
        dists = []
        for ep in range(episodes):
            obs, _ = env.reset(seed=1000 + ep)
            last_dist = float(np.linalg.norm(obs[4:7]))
            done = term = False
            while not done:
                if model_or_none is None:
                    act = env.action_space.sample()
                else:
                    act, _ = model_or_none.predict(obs, deterministic=True)
                obs, _, term, trunc, _ = env.step(act)
                done = term or trunc
                if not done:
                    last_dist = float(np.linalg.norm(obs[4:7]))
            # The post-done obs belongs to the auto-reset NEW episode. On
            # termination the true final distance is <= the arrival radius
            # (scaled); on truncation use the last in-episode distance.
            if term:
                dists.append(min(last_dist, ARRIVAL_RADIUS / DIST_MAX))
            else:
                dists.append(last_dist)
        return float(np.mean(dists))

    before = mean_final_dist(None)
    m = PPO("MlpPolicy", venv, n_steps=512, batch_size=128, seed=0, verbose=0)
    m.learn(total_timesteps=40_000)
    after = mean_final_dist(m)
    assert after < before * 0.7, f"PPO must measurably close distance: {before:.4f} -> {after:.4f}"
