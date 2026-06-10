"""Gymnasium wrapper around the native jumpgate._native.JumpgateEnv.

The native env writes into caller-provided flat buffers and returns nothing
(spec 7.3 FFI rule). This wrapper owns those buffers, assembles the Gymnasium
5-tuple, and returns a fresh copy of the obs buffer on every call so that
collected obs sequences do not alias one mutated buffer (spec 8 determinism).
"""

from typing import Any, Optional

import numpy as np
import gymnasium as gym

from jumpgate._native import JumpgateEnv


class JumpgateGymEnv(gym.Env):
    metadata = {"render_modes": []}

    _MODES = {"waypoint": 0, "thrust": 1, "trader": 2}

    def __init__(
        self,
        num_envs: int = 1,
        num_craft: int = 1,
        mode: str = "waypoint",
        num_pirates: int = 0,
    ) -> None:
        super().__init__()
        if mode not in self._MODES:
            raise ValueError(f"mode must be one of {sorted(self._MODES)}, got {mode!r}")
        self.num_envs = num_envs
        self.num_craft = num_craft
        self.mode = mode
        # Pirates variant (pirates rung spec §11, trader mode only): > 0
        # appends K=2 pirate-contact blocks to the obs (20 -> 34) and defaults
        # the horizon to 5000. obs_dim passthrough ONLY — the action space is
        # UNCHANGED (no purchase actions this rung).
        self.num_pirates = num_pirates
        self._native = JumpgateEnv(num_envs, num_craft, num_pirates)
        # Unseeded-reset seed derivation (see reset()): base from the last
        # explicit seed, golden-ratio stride per unseeded reset.
        self._seed_base = 0
        self._unseeded_resets = 0
        # Trader mode: running credit total for the current episode, attached
        # as info["episode_credits"] on every step (spec §5.5).
        self._episode_credits = 0.0

        if mode in ("thrust", "trader"):
            obs_dim, action_dim = self._native.configure(self._MODES[mode])
        else:
            obs_dim, action_dim = self._native.obs_dim, self._native.action_dim
        self._rebuild_spaces_and_buffers(obs_dim, action_dim)

    def _rebuild_spaces_and_buffers(self, obs_dim: int, action_dim: int) -> None:
        n = self.num_envs * self.num_craft

        # Flat caller-provided buffers, allocated once per (re)configure.
        self._obs_buf = np.zeros(n * obs_dim, dtype=np.float32)
        self._action_buf = np.zeros(n * action_dim, dtype=np.float32)
        self._reward_buf = np.zeros(n, dtype=np.float32)
        self._terminated_buf = np.zeros(n, dtype=np.bool_)
        self._truncated_buf = np.zeros(n, dtype=np.bool_)

        # v1: num_envs == num_craft == 1, so spaces are single-agent.
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf, shape=(obs_dim,), dtype=np.float32
        )
        if self.mode == "trader":
            # Strategic decision: 0 = wait, j = accept board slot j-1 (M=4).
            # The NATIVE action buffer stays f32 (action_dim = 1); step()
            # marshals the Discrete int into that buffer as a float index.
            self.action_space = gym.spaces.Discrete(5)
        else:
            self.action_space = gym.spaces.Box(
                low=-1.0, high=1.0, shape=(action_dim,), dtype=np.float32
            )

    def set_difficulty(self, **kwargs: Any) -> None:
        """Pass curriculum/reward knobs through to the native ``configure``.

        Accepts any subset of the native FlightCfg fields (target_dist_min,
        target_dist_max, star_mass, exhaust_velocity, fuel_capacity,
        time_limit, arrival_radius, arrival_speed, gamma, fuel_weight,
        time_penalty, arrival_bonus); omitted fields keep their values.
        Takes effect at the next reset (call reset() after changing it).
        """
        obs_dim, action_dim = self._native.configure(self._MODES[self.mode], **kwargs)
        self._rebuild_spaces_and_buffers(obs_dim, action_dim)

    def reset(
        self,
        *,
        seed: Optional[int] = None,
        options: Optional[dict[str, Any]] = None,
    ) -> tuple[np.ndarray, dict[str, Any]]:
        super().reset(seed=seed)
        # seed becomes RunConfig.master_seed per env (drives the thrust-mode
        # target draw). CRITICAL: SB3's VecEnvs call reset() with NO seed on
        # every episode end, preempting the native auto-reset; the old
        # `None -> 0` mapping therefore trained on ONE fixed scenario forever
        # (measured: the policy only arrived on targets near the single
        # trained direction). On unseeded resets, derive a fresh deterministic
        # seed from the last explicit seed + an episode stride instead.
        if seed is None:
            self._unseeded_resets += 1
            native_seed = (
                self._seed_base + self._unseeded_resets * 0x9E3779B97F4A7C15
            ) & 0xFFFFFFFFFFFFFFFF
        else:
            native_seed = int(seed)
            self._seed_base = native_seed
            self._unseeded_resets = 0
        self._native.reset(native_seed, self._obs_buf)
        self._episode_credits = 0.0
        info: dict[str, Any] = {}
        return self._obs_buf.copy(), info

    def step(
        self, action: np.ndarray
    ) -> tuple[np.ndarray, float, bool, bool, dict[str, Any]]:
        # Flatten the per-craft action into the flat caller buffer.
        if self.mode == "trader":
            # Discrete int -> f32 index (the native buffer stays f32;
            # the native step decodes it with round()). asarray+flatten accepts
            # ints, np scalars, AND shape-(1,) arrays (a batch dim kept from
            # model.predict) — int(np.array([3])) is a numpy hard error soon.
            self._action_buf[0] = float(np.asarray(action).reshape(-1)[0])
        else:
            self._action_buf[:] = np.asarray(action, dtype=np.float32).reshape(-1)
        self._native.step(
            self._action_buf,
            self._obs_buf,
            self._reward_buf,
            self._terminated_buf,
            self._truncated_buf,
        )
        reward = float(self._reward_buf[0])
        terminated = bool(self._terminated_buf[0])
        truncated = bool(self._truncated_buf[0])
        # Reward-component breakdown rides info, NEVER obs (spec 7.3).
        # `is_success` = task success (low-speed arrival), consumed by SB3's
        # Monitor for the curriculum's rolling arrival rate.
        info: dict[str, Any] = {
            "reward_components": {"total": reward},
            "is_success": terminated,
        }
        if self.mode == "trader":
            # Running credit total (reward IS Δcredits in trader mode); the
            # value attached on the truncation step is the episode's final
            # total (spec §5.5). Zeroed after episode end because the native
            # env auto-resets — the next step belongs to a fresh episode.
            self._episode_credits += reward
            info["episode_credits"] = self._episode_credits
            if terminated or truncated:
                self._episode_credits = 0.0
        return self._obs_buf.copy(), reward, terminated, truncated, info

    def close(self) -> None:
        self._native = None
