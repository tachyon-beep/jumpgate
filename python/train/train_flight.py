"""Train PPO (MLP) to fly thrust-mode jumpgate. Logs arrival-rate/return/fuel
per rollout to CSV; checkpoints per stage promotion. Run:
    PYTHONPATH=python python3 python/train/train_flight.py --steps 2000000
"""
import argparse
import csv
import pathlib

import gymnasium as gym
import numpy as np
from stable_baselines3 import PPO
from stable_baselines3.common.callbacks import BaseCallback
from stable_baselines3.common.running_mean_std import RunningMeanStd
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor, VecNormalize

from jumpgate.gym_env import JumpgateGymEnv
from curriculum import Curriculum

GAMMA = 0.99  # MUST match FlightCfg.gamma (potential-shaping invariant)


class FreshSeedOnReset(gym.Wrapper):
    """Feed a fresh master seed on every unseeded reset.

    JumpgateGymEnv.reset(seed=None) maps to native seed 0, and SB3's
    DummyVecEnv calls env.reset() (no seed) after every done — without this
    wrapper every training episode after the first would redraw the identical
    seed-0 target, collapsing per-episode target diversity to a single point.
    """

    def __init__(self, env, base_seed: int):
        super().__init__(env)
        self._rng = np.random.default_rng(base_seed)

    def reset(self, *, seed=None, options=None):
        if seed is None:
            seed = int(self._rng.integers(0, 2**31 - 1))
        return self.env.reset(seed=seed, options=options)


def make_env(stage, idx: int):
    def _f():
        env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
        env.set_difficulty(
            target_dist_min=stage.target_dist_min, target_dist_max=stage.target_dist_max,
            star_mass=stage.star_mass, exhaust_velocity=stage.exhaust_velocity,
            time_limit=stage.time_limit, gamma=GAMMA,
            arrival_radius=stage.arrival_radius, arrival_speed=stage.arrival_speed,
            time_penalty=stage.time_penalty,
        )
        return FreshSeedOnReset(env, base_seed=10_000 + idx)
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
            rate = self.cur.rolling_rate()
            self.rows.append([self.num_timesteps, self.cur.stage.name, f"{rate:.3f}", f"{ep['r']:.3f}"])
            if promoted:
                print(f"PROMOTED -> {self.cur.stage.name} at {self.num_timesteps} steps")
                self.model.save(f"runs/flight_{self.cur.stage.name}_entry")
                # env_method reaches through VecNormalize/VecMonitor to the
                # base JumpgateGymEnv (verified: DummyVecEnv.env_method
                # getattr-walks gym wrappers); takes effect at next reset.
                self.training_env.env_method(
                    "set_difficulty",
                    target_dist_min=self.cur.stage.target_dist_min,
                    target_dist_max=self.cur.stage.target_dist_max,
                    star_mass=self.cur.stage.star_mass,
                    exhaust_velocity=self.cur.stage.exhaust_velocity,
                    time_limit=self.cur.stage.time_limit, gamma=GAMMA,
                    arrival_radius=self.cur.stage.arrival_radius,
                    arrival_speed=self.cur.stage.arrival_speed,
                    time_penalty=self.cur.stage.time_penalty)
                # Reset VecNormalize running statistics: obs/return scaling fit
                # to the OLD stage poisons the value function on the new one
                # (measured as the run-1 sprint decay). One transient
                # value-loss spike, self-corrects within a rollout.
                vn = self.model.get_vec_normalize_env()
                if vn is not None:
                    vn.obs_rms = RunningMeanStd(shape=vn.observation_space.shape)
                    vn.ret_rms = RunningMeanStd(shape=())
                    vn.returns[:] = 0.0
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
    venv = VecNormalize(
        VecMonitor(DummyVecEnv([make_env(cur.stage, i) for i in range(args.n_envs)])),
        norm_obs=True, norm_reward=True, gamma=GAMMA)
    # LR anneals 3e-4 -> 3e-5 (progress_remaining walks 1 -> 0); ent_coef
    # lowered 0.01 -> 0.003: the rendezvous endgame needs precision, and at
    # 0.01 the entropy bonus dominated once the task reward compressed
    # (run-1 sprint: policy std GREW 0.96 -> 1.64 while arrivals decayed).
    model = PPO("MlpPolicy", venv, gamma=GAMMA, n_steps=2048, batch_size=256,
                learning_rate=lambda pr: 3e-5 + pr * (3e-4 - 3e-5),
                ent_coef=0.003, seed=args.seed, verbose=1)
    model.learn(total_timesteps=args.steps, callback=CurriculumCallback(cur, "runs/flight_log.csv"))
    model.save("runs/flight_final")
    venv.save("runs/flight_vecnorm.pkl")


if __name__ == "__main__":
    main()
