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

    def __init__(self, num_envs: int = 1, num_craft: int = 1) -> None:
        super().__init__()
        self.num_envs = num_envs
        self.num_craft = num_craft
        self._native = JumpgateEnv(num_envs, num_craft)

        obs_dim = self._native.obs_dim
        action_dim = self._native.action_dim
        n = num_envs * num_craft

        # Flat caller-provided buffers, allocated once.
        self._obs_buf = np.zeros(n * obs_dim, dtype=np.float32)
        self._action_buf = np.zeros(n * action_dim, dtype=np.float32)
        self._reward_buf = np.zeros(n, dtype=np.float32)
        self._terminated_buf = np.zeros(n, dtype=np.bool_)
        self._truncated_buf = np.zeros(n, dtype=np.bool_)

        # v1: num_envs == num_craft == 1, so spaces are single-agent.
        self.observation_space = gym.spaces.Box(
            low=-np.inf, high=np.inf, shape=(obs_dim,), dtype=np.float32
        )
        self.action_space = gym.spaces.Box(
            low=-1.0, high=1.0, shape=(action_dim,), dtype=np.float32
        )

    def reset(
        self,
        *,
        seed: Optional[int] = None,
        options: Optional[dict[str, Any]] = None,
    ) -> tuple[np.ndarray, dict[str, Any]]:
        super().reset(seed=seed)
        # seed becomes RunConfig.master_seed per env; deterministic default.
        # v1: master_seed seeds RngStreams but nothing draws from them, so the
        # seed is inert (see test_reset_is_deterministic forward-debt note).
        native_seed = 0 if seed is None else int(seed)
        self._native.reset(native_seed, self._obs_buf)
        info: dict[str, Any] = {}
        return self._obs_buf.copy(), info

    def step(
        self, action: np.ndarray
    ) -> tuple[np.ndarray, float, bool, bool, dict[str, Any]]:
        # Flatten the per-craft action into the flat caller buffer.
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
        info: dict[str, Any] = {
            "reward_components": {"total": reward},
        }
        return self._obs_buf.copy(), reward, terminated, truncated, info

    def close(self) -> None:
        self._native = None
