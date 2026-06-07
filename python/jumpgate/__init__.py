"""jumpgate — deterministic Newtonian space sim with a Gymnasium env.

The native engine is the compiled extension `jumpgate._native` (built by
maturin from crates/jumpgate-py). The Gymnasium wrapper (`gym_env.py`)
arrives in the gym-binding task.
"""
from . import _native  # noqa: F401  (re-export; built by maturin)

__all__ = ["_native"]
