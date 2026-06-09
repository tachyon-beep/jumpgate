import numpy as np
import pytest
import gymnasium as gym

from jumpgate.gym_env import JumpgateGymEnv


def _make():
    return JumpgateGymEnv(num_envs=1, num_craft=1)


def _fixed_action(env):
    # Deterministic action so the run itself is the only variable across calls.
    return np.full(env.action_space.shape, 0.5, dtype=np.float32)


def test_spaces_match_native():
    env = _make()
    assert isinstance(env.observation_space, gym.spaces.Box)
    assert isinstance(env.action_space, gym.spaces.Box)
    assert env.observation_space.dtype == np.float32
    assert env.action_space.dtype == np.float32
    assert env.observation_space.shape == (env._native.obs_dim,)
    assert env.action_space.shape == (env._native.action_dim,)
    env.close()


def test_reset_returns_obs_info():
    env = _make()
    obs, info = env.reset(seed=7)
    assert isinstance(obs, np.ndarray)
    assert obs.dtype == np.float32
    assert obs.shape == env.observation_space.shape
    assert isinstance(info, dict)
    env.close()


def test_step_returns_five_tuple_with_correct_types():
    env = _make()
    env.reset(seed=7)
    obs, reward, terminated, truncated, info = env.step(_fixed_action(env))
    assert isinstance(obs, np.ndarray) and obs.dtype == np.float32
    assert obs.shape == env.observation_space.shape
    assert isinstance(reward, float)
    assert isinstance(terminated, (bool, np.bool_))
    assert isinstance(truncated, (bool, np.bool_))
    assert isinstance(info, dict)
    env.close()


def test_info_carries_reward_breakdown_not_obs():
    # Reward-component breakdown rides info, NEVER obs (spec 7.3).
    env = _make()
    env.reset(seed=7)
    _, _, _, _, info = env.step(_fixed_action(env))
    assert "reward_components" in info
    assert isinstance(info["reward_components"], dict)
    env.close()


def _run_obs_sequence(seed, n_steps=64):
    env = JumpgateGymEnv(num_envs=1, num_craft=1)
    obs, _ = env.reset(seed=seed)
    seq = [obs.copy()]  # copy: native rewrites the same buffer in place
    action = np.full(env.action_space.shape, 0.5, dtype=np.float32)
    for _ in range(n_steps):
        obs, _, _, _, _ = env.step(action)
        seq.append(obs.copy())
    env.close()
    return np.stack(seq)


def test_same_seed_bit_identical_obs_sequence():
    # Tier-B reproducibility through the binding (spec 8).
    a = _run_obs_sequence(seed=123)
    b = _run_obs_sequence(seed=123)
    assert np.array_equal(a, b), "same seed must yield a bit-identical obs sequence"


def test_reset_is_deterministic():
    # v1 determinism contract (spec 8): a fixed config + a fixed action stream
    # must reproduce bit-identically across runs. seed=42 is the canonical case.
    #
    # NOTE (v1 seed-invariance / v2 forward-debt): v1 has no seed-consuming path
    # -- all initial conditions are fixed RunConfig data, and master_seed only
    # seeds RngStreams that nothing draws from in v1. So two DIFFERENT seeds also
    # produce identical state; a "different seeds diverge" assertion would fail
    # unconditionally (memory: "VSL contest is seed-invariant"). We therefore
    # assert reproducibility, NOT divergence. v2 unlocks divergence by drawing a
    # per-craft positional perturbation from RngStream::Scenario in core
    # World::reset; only then does cross-seed divergence become testable.
    a = _run_obs_sequence(seed=42)
    b = _run_obs_sequence(seed=42)
    assert np.array_equal(a, b), (
        "reset(42) + fixed action stream must reproduce a bit-identical obs "
        "sequence across runs (Tier-B determinism through the binding)"
    )
