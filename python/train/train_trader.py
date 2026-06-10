"""Train PPO (MLP) on trader mode: pick the right contract (or wait), get
paid on delivery. Reward IS Delta-credits; NO VecNormalize (spec §5.3 — fixed
global obs scales by construction; the flight rung measured running
normalization whipsawing under non-stationarity). Logs per-episode
(step, ep_return in credits, ep_len in decisions) to CSV. Run:
    PYTHONPATH=python python3 python/train/train_trader.py --steps 60000
"""
import argparse
import csv
import pathlib

import gymnasium as gym
import numpy as np
from stable_baselines3 import PPO
from stable_baselines3.common.callbacks import BaseCallback
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

from jumpgate.gym_env import JumpgateGymEnv

# Near-undiscounted episodic credit total: the SMDP per-decision discount must
# not punish long-but-lucrative trips (spec §5.4).
GAMMA = 0.999


class FreshSeedOnReset(gym.Wrapper):
    """Feed a fresh master seed on every unseeded reset.

    SB3's DummyVecEnv calls env.reset() (no seed) after every done — without
    this wrapper every training episode after the first would redraw the
    same derived-seed scenario sequence from a fixed base, collapsing
    per-episode geometry diversity (flight-rung lesson)."""

    def __init__(self, env, base_seed: int):
        super().__init__(env)
        self._rng = np.random.default_rng(base_seed)

    def reset(self, *, seed=None, options=None):
        if seed is None:
            seed = int(self._rng.integers(0, 2**31 - 1))
        return self.env.reset(seed=seed, options=options)


def make_env(idx: int):
    def _f():
        env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
        # base_seed 50_000+idx: DISTINCT from the held-out eval seeds
        # (10_000..10_024) and from the flight rung's 10_000+idx bases.
        return FreshSeedOnReset(env, base_seed=50_000 + idx)
    return _f


class EpisodeLogCallback(BaseCallback):
    """Collects (step, ep_return(credits), ep_len(decisions)) per finished
    episode from VecMonitor infos; writes the CSV at training end."""

    def __init__(self, log_path):
        super().__init__()
        self.rows = []
        self.log_path = pathlib.Path(log_path)

    def _on_step(self) -> bool:
        for info in self.locals.get("infos", []):
            ep = info.get("episode")
            if ep is not None:
                self.rows.append(
                    [self.num_timesteps, f"{ep['r']:.4f}", int(ep["l"])]
                )
        return True

    def _on_training_end(self):
        self.log_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.log_path, "w", newline="") as f:
            csv.writer(f).writerows(
                [["step", "ep_return", "ep_len"], *self.rows]
            )


def train(steps: int = 60_000, seed: int = 0,
          log_path="runs/trader_log.csv", n_envs: int = 8,
          n_steps: int = 512) -> PPO:
    """Build envs + PPO, learn for `steps` decisions, return the model.

    Smokes MUST pass a /tmp log_path (flight-rung lesson #7: never let a test
    clobber runs/trader_log.csv)."""
    venv = VecMonitor(DummyVecEnv([make_env(i) for i in range(n_envs)]))
    # Constant LR (anneal only if a smoke shows instability — plan Task 6);
    # ent_coef 0.003 per the flight rung's entropy-collapse lesson;
    # device="cpu": MlpPolicy + this host (CUDA unusable, error 804).
    model = PPO("MlpPolicy", venv, gamma=GAMMA, n_steps=n_steps,
                batch_size=128, learning_rate=3e-4, ent_coef=0.003,
                seed=seed, device="cpu", verbose=0)
    model.learn(total_timesteps=steps, callback=EpisodeLogCallback(log_path))
    return model


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--steps", type=int, default=60_000,
                   help="total strategic decisions (semi-MDP steps)")
    p.add_argument("--n-envs", type=int, default=8)
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--log-path", default="runs/trader_log.csv")
    args = p.parse_args()

    model = train(steps=args.steps, seed=args.seed,
                  log_path=args.log_path, n_envs=args.n_envs)
    pathlib.Path("runs").mkdir(exist_ok=True)
    model.save("runs/trader_final")


if __name__ == "__main__":
    main()
