//! jumpgate-core — pure-Rust authoritative deterministic Newtonian space engine.
//!
//! Determinism contract (Tier B = same-binary / same-machine bit-reproducible):
//! integer `tick: u64` is authoritative; `dt` is fixed at init (never a step arg);
//! all RNG is named ChaCha8Rng sub-streams seeded from one master u64 (rand 0.10
//! family: `Rng` value trait, `from_seed`/`seed_from_u64`); actions are a typed
//! `Command` applied in canonical sorted order; a per-tick FNV-1a hash over
//! `f64::to_bits()` (including the slot-map allocator cursor) is the replay test
//! surface. `#![forbid(unsafe_code)]` — no `unsafe` in the engine.
//!
//! This file is the scaffold floor; the engine modules (math, time, types, ids,
//! config, contract, stores, ephemeris, integrator, ship, autopilot, ingest,
//! events, world, hash, replay) land in subsequent tasks, declared in lib.rs
//! ONLY once each file exists (no forward `pub mod` for not-yet-created files).
#![forbid(unsafe_code)]

pub mod math;
pub mod time;
pub mod ids;
pub mod types;
pub mod config;
pub mod hash;
pub mod stores;

pub use ids::{BodyId, CraftId, SlotMap};
pub use stores::{BodyStore, Effective, NavState, ShipStore, effective_params};

/// Scaffold smoke value. Proves the crate compiles and the test harness runs.
/// Replaced by real module wiring in later tasks.
pub fn scaffold_ok() -> u64 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_compiles_and_runs() {
        assert_eq!(scaffold_ok(), 1);
    }
}

/// Lint-activation canary. The `rand` dev-dependency exists ONLY to make the
/// clippy `disallowed-methods` bans on `rand::rng` / `rand::make_rng` resolve
/// and fire under `cargo clippy --all-targets`. Verified during authoring:
/// uncommenting either line below makes clippy emit
///   error: use of a disallowed method `rand::rng`     (-D warnings)
///   error: use of a disallowed method `rand::make_rng`
/// They are left commented so the floor passes; do NOT delete this module or
/// the `rand` dev-dep without re-confirming the ban another way — removing them
/// silently turns the entropy ban inert (the exact bug this guards against).
#[cfg(test)]
mod lint_activation_canary {
    #[allow(dead_code)]
    fn entropy_sources_are_banned() {
        // let _thread = rand::rng();                       // BANNED (was 0.9 thread_rng)
        // let _ent: rand::rngs::StdRng = rand::make_rng(); // BANNED (was 0.9 from_entropy)
    }
}
