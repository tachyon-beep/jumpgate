"""Pirates-rung gym ablation — REPORTED, NEVER GATED (PDR-0006).

A null result is a PLAYER finding at these prices — it triggers NOTHING:
it does not by itself send anyone back to the world design, and the existing
trader learning smoke stays the only gate, on the untouched 0-pirate scenario.

Protocol (spec §11): train PPO on the pirates variant (num_pirates=2,
horizon 5000, obs 34) twice with the SAME policy class — once contact-aware,
once with the contact dims (20-33) zero-masked — then evaluate both on
held-out seeds and report:
  * Δcredits delta (contact-aware − masked, per-episode mean ± sd);
  * route-share shift: per-slot accept-share histograms side by side, plus
    the mean nearest-contact distance AT the accept decision (does the
    contact-aware player route around lurks?);
  * per-decision-index robbery cost: negative-reward macro-steps bucketed by
    decision index (the early-episode-ransom note — a robbery early in the
    episode forfeits more of the horizon).

Run:
    PYTHONPATH=python python3 python/train/eval_pirates_ablation.py \
        --steps 20000
"""
import argparse
import pathlib
import sys

import gymnasium as gym
import numpy as np
from stable_baselines3 import PPO
from stable_baselines3.common.vec_env import DummyVecEnv, VecMonitor

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from train_trader import GAMMA, FreshSeedOnReset  # noqa: E402

from jumpgate.gym_env import JumpgateGymEnv

HEADER = (
    "REPORTED, NEVER GATED (PDR-0006): a null result is a PLAYER finding at "
    "these prices — it triggers NOTHING."
)

# Contact dims appended by the pirates variant (spec §11): 20..34.
CONTACT_LO, CONTACT_HI = 20, 34
NUM_PIRATES = 2
# Held-out eval seeds: the trader-rung convention (training uses 50_000+).
HELD_OUT = range(10_000, 10_020)


class ZeroMaskContacts(gym.ObservationWrapper):
    """The ablation arm: SAME env, SAME policy class, contact dims zeroed.

    The masked player sees the identical world (pirates still rob it) but is
    blind to the contact evidence — any Δcredits gap is therefore the value
    of READING contacts, not of the pirates' existence."""

    def observation(self, obs):
        obs = obs.copy()
        obs[CONTACT_LO:CONTACT_HI] = 0.0
        return obs


def make_env(idx: int, masked: bool):
    def _f():
        env = JumpgateGymEnv(
            num_envs=1, num_craft=1, mode="trader", num_pirates=NUM_PIRATES
        )
        if masked:
            env = ZeroMaskContacts(env)
        return FreshSeedOnReset(env, base_seed=50_000 + idx)

    return _f


def train_arm(masked: bool, steps: int, seed: int, n_envs: int) -> PPO:
    venv = VecMonitor(DummyVecEnv([make_env(i, masked) for i in range(n_envs)]))
    model = PPO(
        "MlpPolicy", venv, gamma=GAMMA, n_steps=256, batch_size=128,
        learning_rate=3e-4, ent_coef=0.003, seed=seed, device="cpu", verbose=0,
    )
    model.learn(total_timesteps=steps)
    return model


def eval_arm(model: PPO, masked: bool, seeds=HELD_OUT):
    """One full episode per held-out seed. Returns the report rows."""
    env = JumpgateGymEnv(
        num_envs=1, num_craft=1, mode="trader", num_pirates=NUM_PIRATES
    )
    returns = []
    slot_accepts = np.zeros(5, dtype=np.int64)  # 0 = wait
    near_dists_at_accept = []  # scaled log-dist of the nearest contact
    robbery_cost_by_index: dict[int, float] = {}
    for seed in seeds:
        obs, _ = env.reset(seed=seed)
        total, decision = 0.0, 0
        for _ in range(64):
            policy_obs = obs.copy()
            if masked:
                policy_obs[CONTACT_LO:CONTACT_HI] = 0.0
            act, _ = model.predict(policy_obs, deterministic=True)
            act = int(np.asarray(act).reshape(-1)[0])
            slot_accepts[act] += 1
            if act > 0 and obs[CONTACT_LO] > 0.5:
                near_dists_at_accept.append(float(obs[CONTACT_LO + 4]))
            obs, r, _, trunc, _ = env.step(act)
            total += r
            if r < 0.0:
                robbery_cost_by_index[decision] = (
                    robbery_cost_by_index.get(decision, 0.0) + r
                )
            decision += 1
            if trunc:
                break
        returns.append(total)
    env.close()
    return (
        np.asarray(returns),
        slot_accepts,
        np.asarray(near_dists_at_accept),
        robbery_cost_by_index,
    )


def report(tag: str, rows) -> float:
    returns, slots, near, rob = rows
    share = slots / max(1, slots.sum())
    print(f"\n--- {tag} ---")
    print(f"held-out Δcredits/episode: {returns.mean():.3f} +/- {returns.std():.3f}")
    print(f"action share [wait, slot1..4]: {np.array2string(share, precision=3)}")
    if near.size:
        print(
            "nearest-contact log-dist at accept: "
            f"{near.mean():.3f} +/- {near.std():.3f} (n={near.size})"
        )
    if rob:
        rows = ", ".join(f"{k}: {v:.3f}" for k, v in sorted(rob.items()))
        print(f"robbery cost by decision index (credits): {rows}")
    else:
        print("robbery cost by decision index: none observed")
    return float(returns.mean())


def main():
    p = argparse.ArgumentParser(description=HEADER)
    p.add_argument("--steps", type=int, default=20_000,
                   help="PPO decisions per arm")
    p.add_argument("--seed", type=int, default=0)
    p.add_argument("--n-envs", type=int, default=8)
    args = p.parse_args()

    print(HEADER)
    print(f"arms: contact-aware vs zero-masked | steps/arm {args.steps} | "
          f"num_pirates {NUM_PIRATES} | held-out seeds "
          f"{HELD_OUT.start}..{HELD_OUT.stop - 1}")

    aware = train_arm(masked=False, steps=args.steps, seed=args.seed,
                      n_envs=args.n_envs)
    masked = train_arm(masked=True, steps=args.steps, seed=args.seed,
                       n_envs=args.n_envs)

    m_aware = report("contact-aware", eval_arm(aware, masked=False))
    m_masked = report("zero-masked", eval_arm(masked, masked=True))

    print(f"\nΔcredits delta (aware − masked): {m_aware - m_masked:+.3f}")
    print(HEADER)


if __name__ == "__main__":
    main()
