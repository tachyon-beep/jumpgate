"""jumpgate — deterministic-replayable Newtonian space sim, Gymnasium-wrapped.

The native engine is the compiled extension `jumpgate._native` (built by
maturin from crates/jumpgate-py). `JumpgateGymEnv` is the Gymnasium wrapper.
"""
from . import _native  # noqa: F401  (re-export; built by maturin)
from jumpgate.gym_env import JumpgateGymEnv

__all__ = ["_native", "JumpgateGymEnv"]
