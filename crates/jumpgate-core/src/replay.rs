//! Replay equivalence — the primary correctness surface (spec §8).
//!
//! `record_run` steps a fresh `World`, logging each tick's driver-produced
//! commands and the post-step `state_hash`, and stamps the config hash AND the
//! build provenance AT RECORD TIME. `replay_run` rebuilds the world from the
//! recorded config, rejects a provenance mismatch (Tier B is same-binary) and a
//! stored-vs-fresh config-hash mismatch, re-feeds the logged commands
//! tick-by-tick, and asserts per-tick hash equality, returning the first
//! differing tick on mismatch. Replay NEVER calls a driver/policy (spec §6).

use crate::config::{ConfigHash, RunConfig};
use crate::contract::{Command, StateView};
use crate::hash::state_hash;
use crate::ingest::ActionLog;
use crate::provenance::Provenance;
use crate::time::Tick;
use crate::world::World;

/// A recorded run: the exact config it ran under, the config hash captured at
/// record time, the tick-stamped action log, and the per-tick
/// `(post_step_tick, state_hash)` sequence.
pub struct Recording {
    pub config: RunConfig,
    pub log: ActionLog,
    pub hashes: Vec<(Tick, u64)>,
    /// `config.config_hash()` snapshotted when the run was recorded. Compared
    /// against a fresh `config.config_hash()` at replay so the guard is not
    /// tautological (see `replay_run`).
    pub config_hash: ConfigHash,
    /// Build/determinism trust-boundary stamp snapshotted at record time (spec
    /// §3.4/§6 "replay header records build metadata"). `replay_run` rejects a
    /// recording whose provenance differs from the replaying binary's, so a
    /// same-machine/same-binary Tier-B violation is detectable, not silent.
    pub provenance: Provenance,
}

/// Step a fresh world for `ticks` ticks, feeding `driver(pre_step_tick)` each
/// tick. The driver's commands are cloned into the log BEFORE `step` mutates
/// (sorts) them. Records one `(post_step_tick, state_hash)` per stepped tick and
/// stamps `config_hash` from the config the run actually used.
pub fn record_run(
    cfg: RunConfig,
    ticks: u64,
    mut driver: impl FnMut(Tick) -> Vec<Command>,
) -> Recording {
    let config_hash = cfg.config_hash();
    let (mut world, reset_hash) = World::reset(cfg.clone()).expect("resolvable config");
    debug_assert_eq!(
        reset_hash, config_hash,
        "World::reset must return the config's own hash"
    );

    let mut log = ActionLog {
        entries: Vec::new(),
        commands_flat: Vec::new(),
        config_hash,
    };
    let mut hashes: Vec<(Tick, u64)> = Vec::with_capacity(ticks as usize);

    for _ in 0..ticks {
        let pre_tick = world.tick();
        let mut cmds = driver(pre_tick);
        // Log the driver's commands faithfully BEFORE step reorders/consumes them.
        // Command is #[derive(Clone, Copy)], so *c is correct.
        for c in &cmds {
            log.record(pre_tick, *c);
        }
        world.step(&mut cmds);
        hashes.push((world.tick(), state_hash(&world)));
    }

    Recording {
        config: cfg,
        log,
        hashes,
        config_hash,
        provenance: Provenance::current(),
    }
}

/// Reject a provenance mismatch, then a config-hash mismatch; rebuild from
/// `rec.config` and re-feed `rec.log` tick-by-tick recomputing `state_hash`.
/// Returns `Ok(())` if every recorded hash matches, else
/// `Err(first_differing_tick)`. Both pre-conditions return `Err(Tick(0))`.
///
/// The provenance guard compares the STORED `rec.provenance` (captured at record
/// time) against `Provenance::current()` (the replaying binary). Tier B is
/// same-binary, so within one build these are equal and the guard is a no-op
/// in-contract; it fires only for an out-of-contract cross-build replay, making
/// that tier mismatch a clean rejection rather than a silent hash divergence.
///
/// The config-hash guard compares the STORED `rec.config_hash` (captured at
/// record time) against a FRESH `rec.config.config_hash()`. These disagree iff
/// `rec.config` was swapped after recording, so the `Err(Tick(0))` branch is
/// reachable and meaningful — not tautological.
///
/// NEVER calls a driver/policy — it only re-feeds the recorded log.
pub fn replay_run(rec: &Recording) -> Result<(), Tick> {
    // Trust-boundary guard FIRST: a recording from a different build (rustc /
    // edition / rand pin / hash-format) is out of Tier-B contract. Reject it as
    // a clean failure so the caller sees "wrong build", not a confusing per-tick
    // divergence. No tick was reproduced.
    if rec.provenance != Provenance::current() {
        return Err(Tick(0));
    }

    let fresh_hash: ConfigHash = rec.config.config_hash();
    if rec.config_hash != fresh_hash {
        // The hashes in this recording were generated under a config whose hash
        // was `rec.config_hash`; the config now present hashes differently. No
        // tick was reproduced.
        return Err(Tick(0));
    }

    let (mut world, reset_hash) = World::reset(rec.config.clone()).expect("resolvable config");
    debug_assert_eq!(
        reset_hash, fresh_hash,
        "World::reset must return the config's own hash"
    );

    for &(recorded_tick, recorded_hash) in &rec.hashes {
        let pre_tick = world.tick();
        // Re-feed exactly the logged commands for this pre-step tick.
        let mut cmds: Vec<Command> = rec.log.at(pre_tick).to_vec();
        world.step(&mut cmds);
        let got = state_hash(&world);
        debug_assert_eq!(
            world.tick(),
            recorded_tick,
            "replay tick cadence diverged from recording"
        );
        if got != recorded_hash {
            return Err(world.tick());
        }
    }

    Ok(())
}
