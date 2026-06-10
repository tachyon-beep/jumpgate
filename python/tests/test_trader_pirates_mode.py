"""Trader pirates-mode (pirates rung 1, Commit G) wrapper tests.

Obs layout with ``num_pirates > 0`` (TRADER_OBS_DIM 20 -> 34, spec §11):
the unchanged 20-dim trader block, then K=2 pirate-contact blocks at dims
20-33, stride 7: ``[present, unit_bearing xyz, log1p(d/0.01)/5.5,
strength/4.0, active]``, contacts sorted by distance. Raw evidence only —
positions, capability magnitude, lying-low visibility — never a score.

Action space stays Discrete(5) (NO purchase actions this rung); reward stays
Δcredits with NO shaping anywhere. ``num_pirates=0`` (the default) keeps
every existing test and the keystone learning smoke byte-identical.
"""

import gymnasium as gym
import numpy as np

from jumpgate.gym_env import JumpgateGymEnv

CONTACT_BASE = 20
CONTACT_STRIDE = 7
CONTACT_SLOTS = 2
PIRATES_OBS_DIM = CONTACT_BASE + CONTACT_SLOTS * CONTACT_STRIDE  # 34
LOG_DIST_SCALE = 5.5
STRENGTH_SCALE = 4.0
DIST_REF_AU = 0.01
# Credits obs dim 17 scale: TRADER_CREDITS_SCALE micros -> credits.
CREDITS_OBS_TO_CR = 30.0


def _make(num_pirates: int) -> JumpgateGymEnv:
    return JumpgateGymEnv(
        num_envs=1, num_craft=1, mode="trader", num_pirates=num_pirates
    )


def _contact(obs: np.ndarray, k: int) -> np.ndarray:
    base = CONTACT_BASE + k * CONTACT_STRIDE
    return obs[base : base + CONTACT_STRIDE]


def _contact_dist_au(block: np.ndarray) -> float:
    """Invert log1p(d/0.01)/5.5 back to AU."""
    return float(np.expm1(block[4] * LOG_DIST_SCALE) * DIST_REF_AU)


def test_pirates_obs_layout_dims_20_33():
    env = _make(2)
    obs, _ = env.reset(seed=3)
    assert obs.shape == (PIRATES_OBS_DIM,)
    # Action space UNCHANGED: Discrete(5), no purchase actions this rung.
    assert isinstance(env.action_space, gym.spaces.Discrete)
    assert env.action_space.n == 5
    # The trader block is still the trader block (board + own dims live).
    assert all(obs[4 * j] in (0.0, 1.0) for j in range(4))
    assert 0.0 <= obs[19] <= 1.0

    # Both contacts present (2 pirates minted, neither is the observer).
    for k in range(CONTACT_SLOTS):
        block = _contact(obs, k)
        assert block[0] == 1.0, f"contact {k} present flag"
        # Unit bearing: a unit vector (pirates spawn at distance > 0).
        bearing_norm = float(np.linalg.norm(block[1:4]))
        assert abs(bearing_norm - 1.0) < 1e-5, f"contact {k} bearing norm {bearing_norm}"
        # Scaled log-distance: positive, O(1).
        assert 0.0 < block[4] < 2.0, f"contact {k} log-dist {block[4]}"
        # Strength: base pirate strength 1, no escorts -> 1/4.0.
        assert abs(block[5] - 1.0 / STRENGTH_SCALE) < 1e-6
        # Active flag: fresh pirates are on the field (lie_low_until = 0).
        assert block[6] == 1.0
    # Contacts sorted by distance ascending.
    d0 = _contact_dist_au(_contact(obs, 0))
    d1 = _contact_dist_au(_contact(obs, 1))
    assert d0 <= d1, f"contacts must be distance-sorted: {d0} > {d1}"


def test_num_pirates_zero_is_the_untouched_trader_env():
    # num_pirates=0 (explicit AND default) => obs_dim 20 and bit-identical
    # obs to an env constructed without the kwarg at the same seed.
    env_default = JumpgateGymEnv(num_envs=1, num_craft=1, mode="trader")
    env_zero = _make(0)
    a, _ = env_default.reset(seed=11)
    b, _ = env_zero.reset(seed=11)
    assert a.shape == (20,)
    assert b.shape == (20,)
    assert np.array_equal(a, b), "num_pirates=0 must be byte-identical"
    # And stepping stays identical too (no pirate machinery may run).
    for act in (1, 0, 2):
        oa, ra, ta, ka, _ = env_default.step(act)
        ob, rb, tb, kb, _ = env_zero.step(act)
        assert np.array_equal(oa, ob)
        assert ra == rb and ta == tb and ka == kb


def test_seed_varies_initial_lurk_stations():
    # Memorization guard (spec §5/§11): the pirate->station lurk map is drawn
    # from the Piracy stream at reset, so two seeds must show the agent
    # different contact geometry as pirates head to different lurks.
    env = _make(2)

    def contact_trace(seed: int) -> np.ndarray:
        obs, _ = env.reset(seed=seed)
        rows = [obs[CONTACT_BASE:PIRATES_OBS_DIM].copy()]
        for _ in range(4):  # let the lurk transits diverge
            obs, _, _, _, _ = env.step(0)
            rows.append(obs[CONTACT_BASE:PIRATES_OBS_DIM].copy())
        return np.concatenate(rows)

    a = contact_trace(101)
    b = contact_trace(202)
    assert not np.allclose(a, b), "two seeds must produce different lurk geometry"
    # Determinism control: the same seed reproduces bit-identically.
    c = contact_trace(101)
    assert np.array_equal(a, c)


def test_robbery_debits_delta_credits_no_shaping():
    # Two co-equal assertions (spec §11/§3):
    #  1. reward == Δcredits on EVERY macro-step (the obs credits dim is the
    #     wallet; any shaping term anywhere would break the identity);
    #  2. somewhere across the seed scan a robbery lands inside an episode
    #     and shows up as a NEGATIVE macro-step reward (the ransom debit +
    #     the forfeited payout — nothing else can debit the trader).
    env = _make(2)
    robbery_seen = False
    for seed in range(24):
        obs, _ = env.reset(seed=seed)
        prev_credits = float(obs[17]) * CREDITS_OBS_TO_CR
        for _ in range(32):
            rewards = [
                obs[4 * j + 1] if obs[4 * j] > 0.5 else -1.0 for j in range(4)
            ]
            act = (int(np.argmax(rewards)) + 1) if max(rewards) > 0 else 0
            obs, r, term, trunc, _ = env.step(act)
            assert not term
            if trunc:
                break  # obs is already the NEXT episode's reset obs
            credits_now = float(obs[17]) * CREDITS_OBS_TO_CR
            assert abs(r - (credits_now - prev_credits)) < 1e-4, (
                f"seed {seed}: reward {r} != Δcredits "
                f"{credits_now - prev_credits} (shaping leak?)"
            )
            if r < 0.0:
                robbery_seen = True
            prev_credits = credits_now
        if robbery_seen:
            break
    assert robbery_seen, (
        "no robbery debit found across 24 seeds — the predation field is "
        "not reaching the gym episode"
    )
