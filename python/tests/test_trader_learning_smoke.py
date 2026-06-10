"""Trader learning smoke — THE KEYSTONE GATE (plan Task 7, spec §1/§7.3).

A short-budget PPO must earn more credits per episode than the random-valid
baseline on held-out seeds. Greedy-highest-reward is REPORTED for context,
never gated (beating greedy = reading geometry, the game-science result —
tracked at the full 60k-step run, not here).

Calibration (measured on this host, 2026-06-10; margin formula untouched):
- Each accept macro-step advances ~600-700 world ticks (~60ms), so wall-clock
  is decision-bound: 6_000 decisions = 485s train. 2_048 decisions with
  n_steps=128 (updates every 1024 decisions instead of 4096) trains in ~125s
  and already clears the bar — PPO seeds 0/1/2 all eval to 4.650 +/- 0.218
  held-out credits (deterministic policy), vs random-valid 3.680 +/- 1.085
  and greedy 4.660 +/- 0.262. Bar = 1.15 * random + 0.05 = 4.282: PPO must
  close most of the random->greedy gap, not merely twitch above noise.
- Everything is seeded (env geometry, SB3, CPU torch), so this test is
  deterministic, not statistically flaky.
"""
import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "train"))

from baselines import evaluate, greedy_reward, random_valid  # noqa: E402
from train_trader import train  # noqa: E402

HELD_OUT = range(10_000, 10_020)  # never seen in training (train uses 50_000+)


def test_ppo_beats_random_baseline():
    # /tmp log path: smokes must NEVER clobber runs/trader_log.csv
    # (flight-rung lesson #7).
    model = train(steps=2_048, seed=0, n_steps=128,
                  log_path="/tmp/trader_smoke.csv")

    def ppo_policy(obs):
        act, _ = model.predict(obs, deterministic=True)
        return int(act)

    rng = np.random.default_rng(0)
    ppo_mean, ppo_sd = evaluate(ppo_policy, HELD_OUT)
    rnd_mean, rnd_sd = evaluate(lambda obs: random_valid(obs, rng), HELD_OUT)
    grd_mean, grd_sd = evaluate(greedy_reward, HELD_OUT)  # reported, not gated
    print(f"\nPPO {ppo_mean:.3f}+/-{ppo_sd:.3f} | random {rnd_mean:.3f}"
          f"+/-{rnd_sd:.3f} | greedy {grd_mean:.3f}+/-{grd_sd:.3f} (reported)")
    assert ppo_mean > rnd_mean * 1.15 + 0.05, (
        f"PPO must beat random-valid: ppo={ppo_mean:.3f} "
        f"random={rnd_mean:.3f} bar={rnd_mean * 1.15 + 0.05:.3f}"
    )
