"""Render flight trajectories: random policy vs trained policy, side by side.
    PYTHONPATH=python python3 python/train/render.py runs/flight_final.zip out.png [runs/flight_vecnorm.pkl]

If the model was trained under VecNormalize (train_flight.py always is), pass
the saved vecnorm pkl — the policy must see NORMALIZED obs or it is crippled
off-distribution (measured: 0/20 arrivals raw vs 20/20 normalized). Geometry
reconstruction below always uses the RAW obs.
"""

import sys

import numpy as np
import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
from stable_baselines3 import PPO  # noqa: E402
from stable_baselines3.common.vec_env import DummyVecEnv, VecNormalize  # noqa: E402

from jumpgate.gym_env import JumpgateGymEnv  # noqa: E402

# The stage distance used for renders. Passed to set_difficulty() as
# target_dist_max, which is ALSO the obs rel-pos divisor (dist_scale) —
# obs[4:7] = target rel-pos / dist_scale (Task 2 scaling).
DIST_SCALE = 0.003


def _load_obs_normalizer(vecnorm_path):
    """Load VecNormalize stats for obs normalization only (policy input)."""
    carrier = DummyVecEnv(
        [lambda: JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")]
    )
    vn = VecNormalize.load(vecnorm_path, carrier)
    vn.training = False
    return vn


def rollout(model, seed, normalizer=None, max_steps=2000):
    env = JumpgateGymEnv(num_envs=1, num_craft=1, mode="thrust")
    env.set_difficulty(
        target_dist_min=DIST_SCALE, target_dist_max=DIST_SCALE, star_mass=0.0
    )
    obs, _ = env.reset(seed=seed)
    # At t0 the craft sits at the start-frame origin, so the unscaled rel-pos
    # IS the target in that frame: target = obs0[4:7] * dist_scale.
    target = obs[4:7].astype(np.float64) * DIST_SCALE
    pts = [np.zeros(3)]
    for _ in range(max_steps):
        if model is None:
            act = env.action_space.sample()
        else:
            policy_obs = (
                normalizer.normalize_obs(obs) if normalizer is not None else obs
            )
            act = model.predict(policy_obs, deterministic=True)[0]
        nobs, _, term, trunc, _ = env.step(act)
        if term or trunc:
            # The native env auto-resets on episode end: nobs already belongs
            # to the NEXT episode, so reconstruct nothing from it.
            return np.array(pts), target, bool(term)
        # Craft pos in start frame = target − unscaled rel-pos.
        pos = target - nobs[4:7].astype(np.float64) * DIST_SCALE
        pts.append(pos.copy())
        obs = nobs
    return np.array(pts), target, False


def panel(ax, traj, target, arrived, title):
    ax.plot(*traj.T)
    ax.scatter(*target, marker="*", s=200)
    ax.scatter(0, 0, 0, marker="o")
    ax.set_title(f"{title} — {'ARRIVED' if arrived else 'failed'} ({len(traj)} ticks)")


def main():
    model_path, out = sys.argv[1], sys.argv[2]
    normalizer = _load_obs_normalizer(sys.argv[3]) if len(sys.argv) > 3 else None
    fig = plt.figure(figsize=(14, 6))
    t1, tgt1, ok1 = rollout(None, seed=5)
    panel(fig.add_subplot(121, projection="3d"), t1, tgt1, ok1, "random policy")
    t2, tgt2, ok2 = rollout(PPO.load(model_path), seed=5, normalizer=normalizer)
    panel(fig.add_subplot(122, projection="3d"), t2, tgt2, ok2, "trained policy")
    fig.savefig(out, dpi=120)
    print(
        f"wrote {out}: random={'ARRIVED' if ok1 else 'failed'}, "
        f"trained={'ARRIVED' if ok2 else 'failed'}"
    )


if __name__ == "__main__":
    main()
