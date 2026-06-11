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
//! config, contract, stores, ephemeris, integrator, ship, provenance, autopilot,
//! ingest, events, world, hash, replay) land in subsequent tasks, declared in
//! lib.rs ONLY once each file exists (no forward `pub mod` for not-yet-created
//! files).
#![forbid(unsafe_code)]

pub mod autopilot;
pub mod config;
pub mod contract;
pub mod diagnostics;
pub mod economy;
pub mod ephemeris;
pub mod events;
pub mod hash;
pub mod ids;
pub mod ingest;
pub mod integrator;
pub mod math;
pub mod media;
pub mod pirate;
pub mod provenance;
pub mod replay;
pub mod rng;
pub mod scenario;
pub mod ship;
pub mod stores;
pub mod time;
pub mod types;
pub mod world;

// Crate-root re-export surface. Downstream tasks (and the jumpgate-py facade)
// import seam/physics/config types through `jumpgate_core::{...}` rather than
// deep module paths; each module-providing task APPENDS its public symbols here
// as it lands (plan-2: Ephemeris/VelocityVerlet/Rk4/substep_count/gravity_accel/
// thrust_accel_and_burn/autopilot_command; plan-3: World/Observer/FullObserver/
// View/ActionLog/EventStream/state_hash/replay symbols).
pub use autopilot::{ARRIVAL_RADIUS, autopilot_command};
pub use config::{
    BaseSpec, BodyInit, ConfigHash, ContractInit, CorporationInit, CraftInit, DispatchCfg,
    GuidanceParams, MediaCfg, OrbitalElements, PriceCfg, ProducerInit, RunConfig, StationInit,
    SubstepCfg,
};
pub use contract::{Command, Event, EventKind, Integrator, StateView, command_sort_key};
pub use diagnostics::{Diagnosis, TrophicSample, Verdict, classify, sample_window};
pub use ephemeris::Ephemeris;
pub use events::{EventStream, FUEL_EMPTY_EPS, detect_boundary_events};
pub use hash::{FnvHasher, HASH_FORMAT_VERSION, HASH_MAGIC, state_hash, write_store_cursor};
pub use ids::{BodyId, ContractId, CraftId, SlotMap, StationId};
pub use ingest::{ActionLog, ingest_into};
pub use integrator::{Rk4, VelocityVerlet, gravity_accel, substep_count};
pub use math::{G_CANONICAL, Vec3, tsiolkovsky_dv};
pub use media::{GossipAlert, GossipBuffer, GossipNode, MediaDiag};
pub use pirate::{
    EngagementSnapshot, relocate_lurk_target, resolve_encounters, run_pirate_brains, strength,
    update_pirate_population,
};
pub use provenance::{PROVENANCE, Provenance};
pub use replay::{Recording, record_run, replay_run};
pub use rng::{RngStream, RngStreams};
pub use scenario::{apply_knob, scenario_trophic};
pub use ship::thrust_accel_and_burn;
pub use stores::{
    BodyStore, CraftRole, CraftStore, Effective, EffectiveMods, NavState, PirateState,
    UpgradeKind, UpgradeLevels, effective_params,
};
pub use time::{Dt, Tick, sim_time};
pub use types::{CommandKind, EntityRef, Lod, NavDest, RouteKey, Target};
pub use world::{FullObserver, Observer, ResetError, RouteEvidence, View, World};

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
