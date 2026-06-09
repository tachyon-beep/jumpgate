//! jumpgate-py — PyO3/maturin ML + Gymnasium facade over `jumpgate-core`.
//!
//! This crate is the ONLY place `unsafe` is permitted (PyO3 FFI codegen). The
//! core engine stays `#![forbid(unsafe_code)]`. The native module is named
//! `_native`; Python imports it as `jumpgate._native`. `JumpgateEnv` and the
//! frame-relative obs path arrive in the gym-binding task.
use pyo3::prelude::*;

mod env;
mod obs;

pub use env::JumpgateEnv;

/// Scaffold smoke function: returns the core's scaffold value across the FFI
/// boundary, proving the cdylib links jumpgate-core and the abi3 module loads.
#[pyfunction]
fn scaffold_ok() -> u64 {
    jumpgate_core::scaffold_ok()
}

/// The native extension module. Python: `from jumpgate import _native`.
#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(scaffold_ok, m)?)?;
    m.add_class::<JumpgateEnv>()?;
    Ok(())
}
