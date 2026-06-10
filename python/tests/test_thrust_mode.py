import numpy as np
from jumpgate.gym_env import JumpgateGymEnv

def _make(**kw):
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
    if kw:
        env.set_difficulty(**kw)
    return env

def test_thrust_mode_spaces():
    env = _make()
    assert env.observation_space.shape == (11,)
    assert env.action_space.shape == (3,)

def test_targets_vary_by_seed_and_obs_sees_them():
    a, _ = _make().reset(seed=1)
    b, _ = _make().reset(seed=2)
    assert not np.allclose(a[4:7], b[4:7]), "different seeds must yield different targets"
    assert np.linalg.norm(a[4:7]) > 0, "target must be visible in obs"

def test_same_seed_reproducible():
    e1, e2 = _make(), _make()
    o1, _ = e1.reset(seed=7); o2, _ = e2.reset(seed=7)
    act = np.array([0.5, 0.2, 0.0], dtype=np.float32)
    for _ in range(20):
        o1, r1, *_ = e1.step(act)
        o2, r2, *_ = e2.step(act)
    assert np.array_equal(o1, o2) and r1 == r2

def test_thrusting_toward_target_beats_coasting():
    env = _make()
    obs, _ = env.reset(seed=11)
    toward = obs[4:7] / (np.linalg.norm(obs[4:7]) + 1e-9)
    r_thrust = sum(env.step(toward.astype(np.float32))[1] for _ in range(30))
    env2 = _make(); env2.reset(seed=11)
    r_coast = sum(env2.step(np.zeros(3, np.float32))[1] for _ in range(30))
    assert r_thrust > r_coast, f"approach {r_thrust} must out-reward coasting {r_coast}"

def test_episode_truncates_and_autoresets():
    env = _make(time_limit=10)
    env.reset(seed=3)
    truncated = False
    for _ in range(12):
        _, _, term, trunc, _ = env.step(np.zeros(3, np.float32))
        truncated = truncated or trunc
        if term or trunc:
            break
    assert truncated, "time-limit must truncate"


def test_unseeded_resets_vary_targets_but_explicit_seed_reproduces():
    # SB3 VecEnvs reset() without a seed on every episode end; each such reset
    # must present a FRESH target (the old None->0 mapping silently trained on
    # one fixed scenario forever), while an explicit seed restores exact
    # reproducibility.
    env = _make()
    a, _ = env.reset(seed=7)
    b, _ = env.reset()  # unseeded: fresh derived scenario
    c, _ = env.reset()  # unseeded again: fresh again
    assert not np.allclose(a[4:7], b[4:7]), "unseeded reset must change the target"
    assert not np.allclose(b[4:7], c[4:7]), "every unseeded reset must differ"
    d, _ = env.reset(seed=7)  # explicit: back to the seed-7 scenario exactly
    assert np.array_equal(a, d), "explicit seed must reproduce bit-identically"
