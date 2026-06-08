# jumpgate v1 — Plan 3: Engine & replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **READ FIRST:** `2026-06-08-jumpgate-v1-plan-0-contract-surface.md` — canonical signatures, workspace layout, and plan-level conventions (it wins on any conflict).

**Goal:** Build the jumpgate v1 rung-3 deterministic (Tier B) 3D Newtonian space core — on-rails bodies, gravity-feeling thrust/fuel/mass craft flown by an in-engine autopilot under a navigator macro-action — exposed as a reproducible Gymnasium env, with a per-tick state-hash replay-equivalence contract.

**Architecture:** Two crates: a pure-Rust `jumpgate-core` (`#![forbid(unsafe_code)]`) that is the sole authoritative writer (SoA stores, tick-indexed ephemeris, velocity-Verlet behind an Integrator trait with accel-keyed integer substepping, Tsiolkovsky variable-mass craft, autopilot guidance, one typed Command/Target ingestion path, a typed Event stream, FNV-1a state hashing, and log-replay), and a `jumpgate-py` PyO3 cdylib facade that writes frame-relative f32 observations into caller-provided buffers and presents the Gymnasium 5-tuple. All facades read through one `StateView` trait that exposes command+event history, not just physics; the engine is shaped (Target sum, Event typing, observer-parameterized projection, effective-param accessor, slot-map ids, Lod seam) so combat/upgrades/fog-of-war drop in without a contract break.

**Tech Stack:** Rust 2024 edition (rustc/cargo 1.95; the slot-map generation field is named `generation` to sidestep the edition-2024 reserved keyword; a toolchain/edition/RNG-pin bump is a reviewed determinism rebaseline — see spec §6 and `provenance.rs`); jumpgate-core deps: rand_chacha (pinned, ChaCha8Rng) + rand_core only; no serde/glam/rayon in the hashed path; hand-rolled f64 Vec3. jumpgate-py: pyo3 0.23 + numpy 0.23 (abi3-py312, extension-module). Build via /home/john/jumpgate/archive/.venv/bin/python -m maturin develop --release (the `--release` is REQUIRED so the Tier-B FP determinism profile reaches the training cdylib — `maturin develop` defaults to a debug build; see spec §6). Python test deps already present: gymnasium 1.2.3, numpy 2.4.6, torch 2.9.1. Workspace-root clippy.toml with disallowed-methods. FNV-1a hashing hand-rolled over f64::to_bits little-endian.

**This plan covers Tasks 10–15.** Prerequisite: Plan 1, Plan 2 complete.

---

### Task 10: Command ingestion path + canonical order + action log + autopilot

The single deterministic write path. `ingest_into` sorts incoming commands by `command_sort_key` (the total canonical order from `contract.rs`), resolves each `NavDest` into a resolved `NavState::Seeking { dest, dv_remaining }` on the target craft, appends every command to a tick-stamped `ActionLog`, and emits an `ActionIngested` event — the lever invariant's one-and-only write path (§4.4). The deterministic `autopilot_command` reads the *resolved* `NavState` field (never the `Command`) plus pos/vel/dest_pos/effective-params and returns `(thrust_dir, throttle)`: thrust toward the destination, throttle cut inside `ARRIVAL_RADIUS` or when `dv_remaining` is exhausted.

This task **defines `ActionLog` in its FINAL shape** — there is no Task-12/Task-14 retrofit. Per the cross-task contract-surface rule (the document produced before Task 3): the task that PROVIDES a symbol defines every method any downstream task calls, and tests it here. `ActionLog` is consumed by Task 12 (`World` holds it; `World::step` reads `since_commands` for `StateView::recent_commands`) and Task 14 (replay compares `config_hash` provenance, re-feeds `at(tick)`). Therefore this task lands: the `config_hash` field captured at construction, a `commands_flat` parallel `Vec<Command>` (so the slice-returning accessors satisfy the locked `StateView::recent_commands -> &[Command]`), `record`, `at`, and `since_commands` (both returning `&[Command]` slices). `record` is the single writer and pushes `entries` + `commands_flat` in lockstep, so the parallel vec can never desync. `ingest_into`, `autopilot_command`, and `ARRIVAL_RADIUS` are likewise final.

**Module-ordering note (acyclic dependency contract).** The seam primitives `NavDest`, `Target`, `EntityRef`, `CommandKind`, `Lod` live in `types.rs` (created Task 3); `Command`, `Event`, `EventKind`, `command_sort_key` live in `contract.rs` (Task 6). `ingest.rs` imports `Command`/`Event`/`EventKind`/`command_sort_key` from `crate::contract` and the primitives from `crate::types`. **The `action_ingested` event constructor lives in `ingest.rs`, NOT `events.rs`** — this breaks the Task-10↔Task-11 cycle (Task 10's `ingest_into` must not depend on a symbol defined in Task 11). `events.rs` (the `EventStream` container: `new`/`emit`/`since`) is created **here in Task 10** as the minimal stub `ingest_into` needs; Task 11 *extends* it with `detect_boundary_events` and the `Wake`/`Arrival`/`FuelEmpty` detectors. `EventStream` is therefore provided here and Task 11 only adds free functions over it.

**Files**
- Create: `crates/jumpgate-core/src/ingest.rs`
- Create: `crates/jumpgate-core/src/autopilot.rs`
- Create: `crates/jumpgate-core/src/events.rs` (minimal `EventStream` container; Task 11 extends it)
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/ingest.rs` (in-file `#[cfg(test)]`), `crates/jumpgate-core/src/autopilot.rs` (in-file `#[cfg(test)]`), `crates/jumpgate-core/src/events.rs` (in-file `#[cfg(test)]`)

**Depends on Task 9** for `ShipStore`, `NavState`, `Effective`, `effective_params`, `BaseSpec` and the `ShipStore` accessors `empty()`, `push(spec,pos,vel,fuel_mass)`, `ids_at(usize) -> CraftId`, `index_of(CraftId) -> Option<usize>`, `craft_pos_by_id(CraftId) -> Option<Vec3>`, and the `pub nav: Vec<NavState>` / `pub spec: Vec<BaseSpec>` / `pub fuel_mass: Vec<f64>` SoA fields. **These accessor names are fixed by the cross-task contract-surface document; if Task 9 named them differently, that is a Task-9 bug to fix there, not a rename here** — the contract's `ingest`/`autopilot` signatures stay verbatim. Contract/types already landed (Tasks 3/6): `Command`, `CommandKind`, `Target`, `EntityRef`, `NavDest`, `Event`, `EventKind`, `Tick`, `CraftId`, `BodyId`, `command_sort_key`, `Vec3`, `Ephemeris`, `ConfigHash`.

> **`World` coordination note:** the contract's `ingest_commands(world: &mut World, …)` cannot be built here because `World` lands in Task 12. This task implements the full ingestion *logic* as `ingest_into(ship: &mut ShipStore, eph: &Ephemeris, log: &mut ActionLog, events: &mut EventStream, tick, cmds: &mut Vec<Command>)`, which `World::step` (Task 12) calls and re-exports as the public `ingest_commands`. All sorting/resolution/logging/event behaviour is exercised here against the `ShipStore` subset; Task 12 only does the wiring.

---

- [ ] **Step 1: Create `events.rs` with the minimal `EventStream` container + a failing test.**

  This is the Task-10 stub that breaks the events↔ingest cycle. Task 11 will *append* `detect_boundary_events` and the `Wake`/`Arrival`/`FuelEmpty` logic; nothing here is rewritten by Task 11.

  Create `crates/jumpgate-core/src/events.rs`:

  ```rust
  //! Typed event record stream (§4.4): one tick-stamped append-only stream, one
  //! emit path, no reactivity. Task 10 lands the container; Task 11 adds the
  //! boundary detectors (Arrival / FuelEmpty / Wake). The `action_ingested`
  //! constructor deliberately lives in `ingest.rs`, not here, so the single
  //! ingestion path does not create a Task-10 -> Task-11 module cycle.

  use crate::contract::Event;
  use crate::time::Tick;

  /// Append-only, tick-ordered event stream. Emitters push in tick order.
  pub struct EventStream {
      pub events: Vec<Event>,
  }

  impl EventStream {
      pub fn new() -> Self {
          EventStream { events: Vec::new() }
      }

      pub fn emit(&mut self, e: Event) {
          self.events.push(e);
      }

      /// All events with `tick >= t`, in emission order.
      pub fn since(&self, t: Tick) -> &[Event] {
          // events is tick-monotonic by construction, so the first index with
          // tick >= t starts the contiguous tail.
          let start = self.events.partition_point(|e| e.tick < t);
          &self.events[start..]
      }
  }

  impl Default for EventStream {
      fn default() -> Self {
          EventStream::new()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::contract::EventKind;
      use crate::ids::CraftId;

      #[test]
      fn since_returns_tick_tail() {
          let mut s = EventStream::new();
          let cid = CraftId { slot: 0, generation: 1 };
          s.emit(Event { tick: Tick(1), kind: EventKind::FuelEmpty { craft: cid } });
          s.emit(Event { tick: Tick(3), kind: EventKind::FuelEmpty { craft: cid } });
          s.emit(Event { tick: Tick(3), kind: EventKind::FuelEmpty { craft: cid } });
          assert_eq!(s.since(Tick(0)).len(), 3);
          assert_eq!(s.since(Tick(3)).len(), 2);
          assert_eq!(s.since(Tick(4)).len(), 0);
      }
  }
  ```

  Register the module — add to `crates/jumpgate-core/src/lib.rs` (alongside the existing `mod`/`pub use` lines):

  ```rust
  pub mod events;
  pub use events::EventStream;
  ```

- [ ] **Step 2: Run the `EventStream` test — passes.**

  ```
  cargo test -p jumpgate-core events::tests::since_returns_tick_tail -- --nocolor
  ```
  EXPECTED: builds (Task 3/6 `Event`/`EventKind`/`Tick`/`CraftId` present) and `test result: ok. 1 passed`. If `Event`/`EventKind` are missing, EXPECTED is a compile error naming them — fix the Task 3/6 dependency before continuing.

- [ ] **Step 3: Create `autopilot.rs` with a "points toward dest" test.**

  Create `crates/jumpgate-core/src/autopilot.rs`:

  ```rust
  //! Deterministic guidance law (§5.6). Reads the RESOLVED `NavState` field
  //! (never the `Command`), returns `(thrust_dir, throttle)`. v1 law: thrust
  //! toward the destination; cut throttle inside `ARRIVAL_RADIUS` or when the
  //! remaining Δv budget is exhausted. Reads `Effective` (the §5.5 accessor
  //! output), never `BaseSpec` directly.

  use crate::math::Vec3;
  use crate::stores::{Effective, NavState};

  /// Distance (canonical AU) at which the autopilot declares "arrived" and cuts thrust.
  pub const ARRIVAL_RADIUS: f64 = 1.0e-4;

  /// Deterministic guidance. Returns `(thrust_dir, throttle)`.
  /// `thrust_dir` is a unit vector (or `Vec3::ZERO` when not thrusting);
  /// `throttle` is in `[0.0, 1.0]`.
  pub fn autopilot_command(
      nav: NavState,
      pos: Vec3,
      _vel: Vec3,
      dest_pos: Vec3,
      _eff: &Effective,
  ) -> (Vec3, f64) {
      match nav {
          NavState::Idle => (Vec3::ZERO, 0.0),
          NavState::Seeking { dv_remaining, .. } => {
              let to_dest = dest_pos.sub(pos);
              let dist = to_dest.length();
              if dist <= ARRIVAL_RADIUS || dv_remaining <= 0.0 {
                  (Vec3::ZERO, 0.0)
              } else {
                  (to_dest.normalize_or_zero(), 1.0)
              }
          }
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::config::BaseSpec;
      use crate::stores::effective_params;

      fn eff() -> Effective {
          effective_params(&BaseSpec {
              base_dry_mass: 1.0,
              base_max_thrust: 1.0,
              base_exhaust_velocity: 1.0,
              base_fuel_capacity: 1.0,
          })
      }

      #[test]
      fn points_toward_dest() {
          let pos = Vec3::new(0.0, 0.0, 0.0);
          let dest = Vec3::new(3.0, 0.0, 0.0);
          let nav = NavState::Seeking {
              dest: crate::types::NavDest::Position(dest),
              dv_remaining: 5.0,
          };
          let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
          assert_eq!(dir, Vec3::new(1.0, 0.0, 0.0));
          assert_eq!(throttle, 1.0);
      }
  }
  ```

  Register the module — add to `crates/jumpgate-core/src/lib.rs`:

  ```rust
  pub mod autopilot;
  pub use autopilot::{autopilot_command, ARRIVAL_RADIUS};
  ```

  > `NavDest` is imported from `crate::types` (the Task-3 seam-primitive module), not `crate::contract`. `NavState`/`Effective`/`effective_params`/`BaseSpec` come from Task 9 (`stores.rs`) and Task 3 (`config.rs`).

- [ ] **Step 4: Run it.**

  ```
  cargo test -p jumpgate-core autopilot::tests::points_toward_dest -- --nocolor
  ```
  EXPECTED: builds and `test result: ok. 1 passed` IF Task 9 types are present. If `NavState`/`Effective`/`effective_params` are missing, EXPECTED is a compile error naming them — fix the Task 9 dependency before continuing.

  > This is a thin guidance fn, so the impl from Step 3 already satisfies the first test; the tests below drive the cut/exhaustion branches against the same impl.

- [ ] **Step 5: Add "cuts at arrival radius" and "Δv exhaustion" and "idle" tests.**

  Append inside the `mod tests` block in `crates/jumpgate-core/src/autopilot.rs`:

  ```rust
      #[test]
      fn cuts_inside_arrival_radius() {
          let dest = Vec3::new(0.0, 0.0, 0.0);
          // pos is closer than ARRIVAL_RADIUS to dest.
          let pos = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
          let nav = NavState::Seeking {
              dest: crate::types::NavDest::Position(dest),
              dv_remaining: 5.0,
          };
          let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
          assert_eq!(dir, Vec3::ZERO);
          assert_eq!(throttle, 0.0);
      }

      #[test]
      fn dv_exhaustion_stops_thrust() {
          let pos = Vec3::new(0.0, 0.0, 0.0);
          let dest = Vec3::new(3.0, 0.0, 0.0); // far away, would otherwise thrust
          let nav = NavState::Seeking {
              dest: crate::types::NavDest::Position(dest),
              dv_remaining: 0.0, // budget gone
          };
          let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
          assert_eq!(dir, Vec3::ZERO);
          assert_eq!(throttle, 0.0);
      }

      #[test]
      fn idle_never_thrusts() {
          let (dir, throttle) = autopilot_command(
              NavState::Idle,
              Vec3::new(1.0, 2.0, 3.0),
              Vec3::ZERO,
              Vec3::new(9.0, 9.0, 9.0),
              &eff(),
          );
          assert_eq!(dir, Vec3::ZERO);
          assert_eq!(throttle, 0.0);
      }
  ```

- [ ] **Step 6: Run the full autopilot suite — passes.**

  ```
  cargo test -p jumpgate-core autopilot::tests -- --nocolor
  ```
  EXPECTED: `test result: ok. 4 passed`.

- [ ] **Step 7: Create `ingest.rs` with `ActionLog` in its FINAL shape + a `record`/`at`/`since_commands` test.**

  `ActionLog` is provided here once, complete, in its **single canonical shape** for the whole plan (Tasks 12 and 14 consume it, they do NOT redefine it). The `config_hash` field is captured at construction so Task 14's replay guard can compare *recorded provenance* against a *freshly-computed* hash (without it, both sides recompute from the same `cfg` and the guard is vacuous). It keeps a `commands_flat: Vec<Command>` parallel to `entries`, pushed in lockstep by the single writer `record`, because the locked `StateView::recent_commands` trait method returns `&[Command]` — an entries-only iterator cannot produce that borrowed slice. Because `record` is the ONLY writer and pushes both vecs together, the two can never desync, so the slice-backed accessors (`at`/`since_commands`) are safe.

  Create `crates/jumpgate-core/src/ingest.rs`:

  ```rust
  //! THE single ingestion path (lever invariant, §4.4). Sorts commands by the
  //! canonical `command_sort_key`, resolves each `NavDest` into a resolved
  //! `NavState::Seeking`, logs every command tick-stamped, and emits an
  //! `ActionIngested` event. No out-of-band store mutation happens anywhere else.

  use crate::contract::{command_sort_key, Command, Event, EventKind};
  use crate::config::ConfigHash;
  use crate::ephemeris::Ephemeris;
  use crate::events::EventStream;
  use crate::math::Vec3;
  use crate::stores::{NavState, ShipStore};
  use crate::time::Tick;
  use crate::types::{CommandKind, EntityRef, NavDest, Target};

  /// Tick-stamped append-only command log. Replay re-feeds these entries; the
  /// policy is never re-run (§6). `config_hash` is the provenance stamp of the
  /// `RunConfig` this log was recorded under — Task 14 compares it against a
  /// freshly-computed hash to detect a config/replay mismatch.
  pub struct ActionLog {
      pub entries: Vec<(Tick, Command)>,
      /// Parallel to `entries`, pushed in lockstep by `record` (the SINGLE writer),
      /// so `at`/`since_commands` return zero-copy `&[Command]`. The lockstep push
      /// is what makes the parallel vec safe — it can never desync from `entries`.
      pub commands_flat: Vec<Command>,
      pub config_hash: ConfigHash,
  }

  impl ActionLog {
      /// Construct a log stamped with the recording run's config hash.
      pub fn new(config_hash: ConfigHash) -> Self {
          ActionLog {
              entries: Vec::new(),
              commands_flat: Vec::new(),
              config_hash,
          }
      }

      /// The ONLY writer: pushes BOTH vecs in lockstep so they cannot diverge.
      pub fn record(&mut self, tick: Tick, cmd: Command) {
          self.entries.push((tick, cmd));
          self.commands_flat.push(cmd);
      }

      /// All commands logged exactly at `tick`, in insertion (canonical) order,
      /// as a zero-copy contiguous slice (entries are append-only, tick-monotone).
      pub fn at(&self, tick: Tick) -> &[Command] {
          let start = self.entries.partition_point(|(t, _)| *t < tick);
          let end = self.entries.partition_point(|(t, _)| *t <= tick);
          &self.commands_flat[start..end]
      }

      /// Every command logged at `tick >= since`, in insertion (canonical) order,
      /// as a zero-copy contiguous tail slice. Task 12's `StateView::recent_commands`
      /// (which the locked contract fixes as returning `&[Command]`) is built on this.
      pub fn since_commands(&self, since: Tick) -> &[Command] {
          let start = self.entries.partition_point(|(t, _)| *t < since);
          &self.commands_flat[start..]
      }
  }

  /// Construct an `ActionIngested` event. Lives HERE (not `events.rs`) so the
  /// single ingestion path owns its only event constructor and the module graph
  /// stays acyclic (Task 10 must not depend on a Task-11 symbol).
  fn action_ingested(tick: Tick, target: Target) -> Event {
      Event {
          tick,
          kind: EventKind::ActionIngested { target },
      }
  }

  /// Resolve a `NavDest` to a concrete world `Vec3` at `tick`.
  /// `Position` is already absolute; `Entity` is looked up via the ephemeris
  /// (bodies) or the ship store (craft). Returns `None` if the referent is gone.
  fn resolve_dest(
      dest: NavDest,
      tick: Tick,
      ship: &ShipStore,
      eph: &Ephemeris,
  ) -> Option<Vec3> {
      match dest {
          NavDest::Position(p) => Some(p),
          NavDest::Entity(EntityRef::Body(bid)) => {
              // v1 ephemeris is indexed positionally; the BodyId slot is the row.
              Some(eph.body_pos(bid.slot as usize, tick))
          }
          NavDest::Entity(EntityRef::Craft(cid)) => ship.craft_pos_by_id(cid),
      }
  }

  /// THE single ingestion path. Sorts `cmds` into canonical order in place,
  /// then for each command: resolves the destination, sets the target craft's
  /// `NavState::Seeking`, logs the command tick-stamped, and emits
  /// `ActionIngested`. Lever invariant: this is the only craft-nav write path.
  pub fn ingest_into(
      ship: &mut ShipStore,
      eph: &Ephemeris,
      log: &mut ActionLog,
      events: &mut EventStream,
      tick: Tick,
      cmds: &mut Vec<Command>,
  ) {
      // Canonical, total, deterministic ordering across all Target scopes.
      cmds.sort_by_key(command_sort_key);

      for cmd in cmds.iter() {
          // Log every command in canonical order (resolved values; §4.4 rule 2).
          log.record(tick, *cmd);

          match cmd.target {
              Target::Entity(EntityRef::Craft(cid)) => {
                  let CommandKind::Destination { dest, burn_budget } = cmd.kind;
                  if let Some(idx) = ship.index_of(cid) {
                      // dv budget: explicit cap, else Tsiolkovsky fuel-derived.
                      let dv = burn_budget.unwrap_or_else(|| dv_from_fuel(ship, idx));
                      // Validate the dest resolves now; drop silently if it does
                      // not. The autopilot recomputes the live dest each tick, so
                      // we store the dest reference (moving targets are tracked).
                      if resolve_dest(dest, tick, ship, eph).is_some() {
                          ship.nav[idx] = NavState::Seeking {
                              dest,
                              dv_remaining: dv,
                          };
                          events.emit(action_ingested(tick, cmd.target));
                      }
                  }
              }
              // World / Sim / Body targets: no v1 CommandKind acts on them yet,
              // but they are logged above so replay identity is preserved, and
              // the ingestion event is still emitted for the legibility stream.
              _ => {
                  events.emit(action_ingested(tick, cmd.target));
              }
          }
      }
  }

  /// Fuel-derived Δv fallback when no explicit budget is given:
  /// Tsiolkovsky Δv = v_e * ln((dry + fuel) / dry), using effective params (§5.5).
  fn dv_from_fuel(ship: &ShipStore, idx: usize) -> f64 {
      let eff = crate::stores::effective_params(&ship.spec[idx]);
      let fuel = ship.fuel_mass[idx];
      let dry = eff.dry_mass;
      if dry <= 0.0 || fuel <= 0.0 {
          0.0
      } else {
          eff.exhaust_velocity * ((dry + fuel) / dry).ln()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::config::{BaseSpec, BodyInit};
      use crate::ids::CraftId;
      use crate::time::Dt;

      fn cfg_hash() -> ConfigHash {
          // A stand-in provenance stamp; only its round-trip identity matters here.
          ConfigHash(0xABCD_0001)
      }

      fn empty_ephemeris() -> Ephemeris {
          // Zero bodies: NavDest::Position resolution needs no body lookup.
          Ephemeris::precompute(&[] as &[BodyInit], Dt::new(1.0), 1)
      }

      fn ship_store_with(n: usize) -> ShipStore {
          let mut store = ShipStore::empty();
          for _ in 0..n {
              store.push(
                  BaseSpec {
                      base_dry_mass: 1.0,
                      base_max_thrust: 1.0,
                      base_exhaust_velocity: 1.0,
                      base_fuel_capacity: 1.0,
                  },
                  Vec3::ZERO,
                  Vec3::ZERO,
                  0.5, // fuel_mass
              );
          }
          store
      }

      fn dest_for(id: CraftId, x: f64) -> Command {
          Command {
              target: Target::Entity(EntityRef::Craft(id)),
              kind: CommandKind::Destination {
                  dest: NavDest::Position(Vec3::new(x, 0.0, 0.0)),
                  burn_budget: Some(2.0),
              },
          }
      }

      #[test]
      fn log_records_queries_by_tick_and_since() {
          let mut log = ActionLog::new(cfg_hash());
          log.record(Tick(5), dest_for(CraftId { slot: 0, generation: 1 }, 1.0));
          log.record(Tick(5), dest_for(CraftId { slot: 1, generation: 1 }, 2.0));
          log.record(Tick(6), dest_for(CraftId { slot: 0, generation: 1 }, 3.0));
          assert_eq!(log.at(Tick(5)).len(), 2);
          assert_eq!(log.at(Tick(6)).len(), 1);
          assert_eq!(log.at(Tick(7)).len(), 0);
          assert_eq!(log.entries.len(), 3);
          // since_commands returns a zero-copy &[Command] tail slice; commands_flat
          // is pushed in lockstep with entries by the single writer, so no desync.
          assert_eq!(log.since_commands(Tick(0)).len(), 3);
          assert_eq!(log.since_commands(Tick(6)).len(), 1);
          assert_eq!(log.since_commands(Tick(7)).len(), 0);
          assert_eq!(log.commands_flat.len(), log.entries.len());
          // config_hash provenance is preserved verbatim for Task 14's guard.
          assert_eq!(log.config_hash, cfg_hash());
      }

      #[test]
      fn out_of_order_yields_same_navstate_as_presorted() {
          let eph = empty_ephemeris();

          // Build two identical stores; feed one shuffled, one pre-sorted.
          let mut store_a = ship_store_with(2);
          let mut store_b = ship_store_with(2);
          let id0 = store_a.ids_at(0);
          let id1 = store_a.ids_at(1);

          let mut shuffled = vec![dest_for(id1, 9.0), dest_for(id0, 4.0)];
          let mut presorted = shuffled.clone();
          presorted.sort_by_key(command_sort_key);

          let mut log_a = ActionLog::new(cfg_hash());
          let mut log_b = ActionLog::new(cfg_hash());
          let mut ev_a = EventStream::new();
          let mut ev_b = EventStream::new();

          ingest_into(&mut store_a, &eph, &mut log_a, &mut ev_a, Tick(0), &mut shuffled);
          ingest_into(&mut store_b, &eph, &mut log_b, &mut ev_b, Tick(0), &mut presorted);

          // Resolved NavState must be identical regardless of input order.
          for i in 0..2 {
              match (store_a.nav[i], store_b.nav[i]) {
                  (
                      NavState::Seeking { dest: da, dv_remaining: va },
                      NavState::Seeking { dest: db, dv_remaining: vb },
                  ) => {
                      assert_eq!(da, db, "dest mismatch at craft {i}");
                      assert_eq!(va, vb, "dv mismatch at craft {i}");
                  }
                  other => panic!("expected both Seeking at {i}, got {other:?}"),
              }
          }

          // The log is sorted into canonical order on both paths -> identical.
          assert_eq!(log_a.entries, log_b.entries);

          // dv budget honoured: burn_budget Some(2.0) -> dv_remaining 2.0.
          if let NavState::Seeking { dv_remaining, .. } = store_a.nav[0] {
              assert_eq!(dv_remaining, 2.0);
          } else {
              panic!("craft 0 not Seeking");
          }
      }

      #[test]
      fn ingest_emits_action_ingested_event() {
          let eph = empty_ephemeris();
          let mut store = ship_store_with(1);
          let id0 = store.ids_at(0);
          let mut log = ActionLog::new(cfg_hash());
          let mut ev = EventStream::new();
          let mut cmds = vec![dest_for(id0, 4.0)];
          ingest_into(&mut store, &eph, &mut log, &mut ev, Tick(3), &mut cmds);

          let emitted = ev.since(Tick(0));
          assert_eq!(emitted.len(), 1);
          assert_eq!(emitted[0].tick, Tick(3));
          match emitted[0].kind {
              EventKind::ActionIngested { target } => {
                  assert_eq!(target, Target::Entity(EntityRef::Craft(id0)));
              }
              other => panic!("expected ActionIngested, got {other:?}"),
          }
      }
  }
  ```

  Register the module — add to `crates/jumpgate-core/src/lib.rs`:

  ```rust
  pub mod ingest;
  pub use ingest::{ingest_into, ActionLog};
  ```

  > The test module references the Task-9 `ShipStore` accessors `empty()`, `push(...)`, `ids_at(usize)`, `index_of(CraftId)`, `craft_pos_by_id(CraftId)`, and the `pub nav` / `pub spec` / `pub fuel_mass` SoA fields. These names are fixed in the pre-Task-3 cross-task contract-surface document; a mismatch is a Task-9 defect, fixed in `stores.rs` — do not rename in this task and do not invent new `ShipStore` methods here.

- [ ] **Step 8: Run the full ingest suite — passes.**

  ```
  cargo test -p jumpgate-core ingest::tests -- --nocolor
  ```
  EXPECTED: `test result: ok. 3 passed` (`log_records_queries_by_tick_and_since`, `out_of_order_yields_same_navstate_as_presorted`, `ingest_emits_action_ingested_event`).

  > If the build instead fails on a missing `ShipStore` accessor or on `NavDest`/`Target` not being found in `crate::types`, that signals a Task-3/Task-9 contract-surface gap (a providing task did not define a symbol this task consumes). Fix it in the providing module before proceeding — do not work around it here.

- [ ] **Step 9: Run the whole crate test suite to confirm no regressions.**

  ```
  cargo test -p jumpgate-core -- --nocolor
  ```
  EXPECTED: `test result: ok.` with all prior tasks' tests plus the 8 added here (4 autopilot + 3 ingest + 1 events) passing; `0 failed`.

- [ ] **Step 10: Lint the new modules (catches banned methods in test code too).**

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings. (`--all-targets` is required — `--lib` is a no-op on linting `#[cfg(test)]` modules in this crate.)

- [ ] **Step 11: Commit.**

  ```
  git checkout -b task-10-ingest-autopilot
  git add crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/autopilot.rs crates/jumpgate-core/src/events.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  Task 10: command ingestion path + canonical order + action log + autopilot

  - ingest_into: the single deterministic write path. Sorts commands by
    command_sort_key, resolves NavDest, sets NavState::Seeking on the target
    craft, logs every command tick-stamped (ActionLog), emits ActionIngested.
  - ActionLog landed in FINAL shape: entries + commands_flat (parallel vec,
    pushed in lockstep by the single writer record, so no desync) + config_hash
    provenance stamp + record/at/since_commands (at/since_commands return
    &[Command] slices to satisfy the locked StateView::recent_commands -> &[Command]).
    Consumed verbatim by Task 12 (StateView::recent_commands) and Task 14
    (replay config-hash provenance guard).
  - action_ingested constructor placed in ingest.rs (NOT events.rs) to keep the
    module graph acyclic; events.rs lands the minimal EventStream container here
    (Task 11 extends it with detect_boundary_events + Wake/Arrival/FuelEmpty).
  - autopilot_command: deterministic v1 guidance reading the resolved NavState
    field; thrust toward dest, cut inside ARRIVAL_RADIUS or on dv exhaustion.
  - dv budget: explicit burn_budget cap, else Tsiolkovsky fuel-derived fallback.
  - imports reconciled to the acyclic module order: seam primitives (NavDest,
    Target, EntityRef, CommandKind) from crate::types; Command/Event/EventKind/
    command_sort_key from crate::contract.
  - World::ingest_commands wrapper deferred to Task 12 (World wiring).

  Tests: EventStream since-tail; out-of-order vec yields identical NavState +
  identical log as pre-sorted; autopilot points toward dest, cuts at arrival
  radius, stops on dv exhaustion, idle never thrusts; ActionIngested emitted at
  the correct tick; ActionLog config_hash + since_commands round-trip.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: a commit on branch `task-10-ingest-autopilot` listing 4 changed files.

---

### Task 11: Event layer: tick-boundary records (Arrival, FuelEmpty)

**Goal:** A tick-stamped `EventStream` (record buffer) plus pure boundary-detection predicates that emit `Arrival`/`FuelEmpty` against *quantized* state at the tick boundary, with **no reactivity** (an event never triggers another same-tick event; chains arise only across ticks because next tick a predicate reads the mutated state). The public `detect_boundary_events(...)` is a thin read-only wrapper over those predicates that reads the prior-tick snapshot stored on `ShipStore`.

**Files**
- Create: `crates/jumpgate-core/src/events.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/events.rs` (inline `#[cfg(test)] mod tests`)

**Depends on:**
- Task 4 (`stores.rs`): `ShipStore` and its index-aligned data arrays — `pos`, `fuel_mass`, `nav: Vec<NavState>`, plus the **two prior-tick snapshot arrays added in Task 4's hashed state**: `prev_fuel: Vec<f64>` and `prev_inside_dest: Vec<bool>`. Also requires a **live-id accessor** (index → `CraftId`) — the contract's `SlotMap` exposes `new/len/cursor` plus `id_at(idx) -> Option<(u32,u32)>`, and `ShipStore` wraps it as `ids_at(idx) -> CraftId` (the `expect`-internally form used to stamp events). This task names that requirement (`ShipStore::ids_at`); it does not fabricate the accessor.
- Ephemeris task (`ephemeris.rs`): `Ephemeris::body_pos(body_idx, tick)` for resolving `NavDest::Entity(Body)` destinations.
- `BodyStore` (Task 4): `eph_index: Vec<usize>` to map a `BodyId` to its ephemeris row when resolving an entity destination.
- Task 13 (`autopilot.rs`): `pub const ARRIVAL_RADIUS: f64`. To keep this task self-contained for TDD, the helper tests use literal radii; only Step 9 imports the real constant.

Contract types in play: `EventStream`, `Event`, `EventKind`, `detect_boundary_events`, `NavState`, `NavDest`, `EntityRef`, `CraftId`, `BodyId`.

**Design note (read before coding).** The contract's intent is `detect_boundary_events` records `Arrival`/`FuelEmpty` at the tick boundary against quantized state. The original plan called four `World` accessors (`craft_fuel_prev`, `craft_nav`, `craft_prev_inside_dest`, `resolved_dest_pos`) that **do not exist on `World`/`StateView`** — they were hallucinated. We correct this by reading the data **directly from `ShipStore`** (whose fields are `pub` and index-aligned) and from `Ephemeris`/`BodyStore`, rather than inventing `World` methods. This is also the systemic fix the reviewers flagged: a downstream task must call only methods its provider actually defines. The new signature is therefore:

```rust
pub fn detect_boundary_events(
    ships: &ShipStore,
    bodies: &BodyStore,
    ephem: &Ephemeris,
    tick: Tick,
    out: &mut EventStream,
);
```

This **decouples the detector from `World`** (a genuine improvement — it is now unit-testable from a plain `ShipStore` and is a pure read, enforced by `&` shared refs).

The two non-trivial requirements — (a) `Arrival` fires exactly on *crossing* `ARRIVAL_RADIUS`, and (b) `FuelEmpty` fires **once** on depletion, not every later tick — both require comparing the just-completed tick's quantized state against the *previous* tick's state. Task 4 stores that prior state on `ShipStore` as `prev_fuel` and `prev_inside_dest` (both index-aligned with `pos`/`fuel_mass`/`nav`). **HASH CROSS-REFERENCE:** those two arrays are part of the canonical hashed state; their exact position in the FNV-1a field order is owned by the hash task's `HASH_FIELD_ORDER` spec and golden-hash test — this task only *reads* them and must not change their layout or hash encoding.

We isolate the per-craft decision into two pure helper functions that take plain scalar/`Vec3` inputs (current + previous), unit-test those exhaustively with **no `World`/`ShipStore` needed**, and let `detect_boundary_events` be the thin glue that reads the stores and calls them.

These helpers (defined in this task) are referenced by the steps below:
```rust
/// Pure arrival predicate: within ARRIVAL_RADIUS of dest_pos now AND outside it
/// at the previous tick (a crossing, not a "still inside" repeat).
fn arrival_crossed(pos: Vec3, dest_pos: Vec3, prev_inside: bool) -> bool;

/// Pure fuel-empty predicate: at/below epsilon now AND strictly above epsilon at
/// the previous tick (the depletion edge, fires once).
fn fuel_just_emptied(fuel_now: f64, fuel_prev: f64) -> bool;

/// Quantization epsilon for fuel comparisons (fuel at/below this == "empty").
pub const FUEL_EMPTY_EPS: f64 = 1e-9;
```

**Out of scope for this task (other reviewers' fixes, deliberately NOT done here):** the `Wake` `EventKind` variant + the `Lod` dispatch branch live in `EventKind` (contract.rs) and `World::step` — `detect_boundary_events` emits only `Arrival`/`FuelEmpty`. The cross-task contract-surface doc, the rng version pin, module-ordering, the `Integrator` single-definition fix, and the `HASH_FIELD_ORDER` spec + golden-hash test all belong to their owning tasks.

---

- [ ] **Step 1: Add the `events` module declaration to `lib.rs` (red — won't compile yet).**
  Open `crates/jumpgate-core/src/lib.rs` and add, in the module-declaration block alongside the other `pub mod` lines:
  ```rust
  pub mod events;
  ```

- [ ] **Step 2: Create `events.rs` with `EventStream` + a failing `emit`/`since` test.**
  `EventStream` is a pure append-only buffer; `since(t)` returns the suffix of events whose `tick >= t` (events are appended in non-decreasing tick order by construction, so `since` is a tail slice found by the first index with `tick >= t`).
  ```rust
  //! Event layer: tick-stamped record buffer + tick-boundary detectors.
  //!
  //! Events are RECORDS, not handlers. No bus, no reactivity: detecting an event
  //! never triggers another same-tick event. Emergent chains arise only across
  //! ticks (next tick a predicate reads the mutated state). Detectors are pure
  //! reads — they never mutate any store.

  use crate::contract::{Event, EventKind};
  use crate::time::Tick;

  /// Quantization epsilon for fuel comparisons (fuel at/below this == "empty").
  pub const FUEL_EMPTY_EPS: f64 = 1e-9;

  /// Append-only, tick-stamped record buffer. Events are appended in
  /// non-decreasing `tick` order (the step loop only ever emits for the current
  /// tick), so `since` is a contiguous tail slice.
  pub struct EventStream {
      pub events: Vec<Event>,
  }

  impl EventStream {
      pub fn new() -> Self {
          EventStream { events: Vec::new() }
      }

      pub fn emit(&mut self, e: Event) {
          self.events.push(e);
      }

      /// All events with `tick >= t`, in emission order.
      pub fn since(&self, t: Tick) -> &[Event] {
          let start = self
              .events
              .iter()
              .position(|e| e.tick >= t)
              .unwrap_or(self.events.len());
          &self.events[start..]
      }
  }

  impl Default for EventStream {
      fn default() -> Self {
          Self::new()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::ids::CraftId;

      fn craft(slot: u32) -> CraftId {
          CraftId { slot, generation: 0 }
      }

      #[test]
      fn emit_and_since_returns_tail_by_tick() {
          let mut s = EventStream::new();
          s.emit(Event {
              tick: Tick(1),
              kind: EventKind::FuelEmpty { craft: craft(0) },
          });
          s.emit(Event {
              tick: Tick(3),
              kind: EventKind::FuelEmpty { craft: craft(1) },
          });
          s.emit(Event {
              tick: Tick(5),
              kind: EventKind::FuelEmpty { craft: craft(2) },
          });

          // since(0) returns everything.
          assert_eq!(s.since(Tick(0)).len(), 3);
          // since(3) drops the tick-1 event, keeps tick-3 and tick-5.
          let tail = s.since(Tick(3));
          assert_eq!(tail.len(), 2);
          assert_eq!(tail[0].tick, Tick(3));
          // since past the end is empty.
          assert_eq!(s.since(Tick(99)).len(), 0);
      }
  }
  ```

- [ ] **Step 3: Run the test — confirm it compiles and passes (green for the buffer).**
  ```
  cargo test -p jumpgate-core events::tests::emit_and_since -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed` (the `events::tests::emit_and_since_returns_tail_by_tick` line shows `ok`).

- [ ] **Step 4: Add a failing test for the `fuel_just_emptied` edge predicate.**
  Add this test inside the existing `mod tests` block. It pins the once-only edge semantics: fires on the depletion transition, not while already empty, not while still fuelled.
  ```rust
      #[test]
      fn fuel_just_emptied_fires_only_on_depletion_edge() {
          // Was fuelled, now empty -> edge, fires.
          assert!(fuel_just_emptied(0.0, 0.5));
          // At/below eps now, above eps before -> still the edge.
          assert!(fuel_just_emptied(FUEL_EMPTY_EPS, 0.5));
          assert!(fuel_just_emptied(FUEL_EMPTY_EPS * 0.5, 1.0));
          // Already empty last tick -> does NOT fire again.
          assert!(!fuel_just_emptied(0.0, 0.0));
          assert!(!fuel_just_emptied(0.0, FUEL_EMPTY_EPS));
          // Still fuelled -> does not fire.
          assert!(!fuel_just_emptied(0.4, 0.5));
      }
  ```
  This will fail to compile (`fuel_just_emptied` undefined). That is the red state.

- [ ] **Step 5: Implement `fuel_just_emptied` (minimal).**
  Add to `events.rs`, above the `#[cfg(test)]` block:
  ```rust
  /// Pure fuel-empty predicate. True iff fuel is at/below epsilon now AND was
  /// strictly above epsilon at the previous tick (the depletion edge, fires once).
  fn fuel_just_emptied(fuel_now: f64, fuel_prev: f64) -> bool {
      fuel_now <= FUEL_EMPTY_EPS && fuel_prev > FUEL_EMPTY_EPS
  }
  ```

- [ ] **Step 6: Run the fuel predicate test — confirm green.**
  ```
  cargo test -p jumpgate-core events::tests::fuel_just_emptied -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 7: Add a failing test for the `arrival_crossed` edge predicate.**
  Add inside `mod tests`. The first test uses literal radii (no dependency on Task 13's constant) to document the crossing contract; the second calls the real `arrival_crossed` and so depends on `ARRIVAL_RADIUS`.
  ```rust
      #[test]
      fn arrival_crossing_contract_documented() {
          use crate::math::Vec3;
          let dest = Vec3::new(10.0, 0.0, 0.0);
          let radius = 0.5_f64;

          // Helper mirrors arrival_crossed but with an explicit radius for the test.
          let crossed = |pos: Vec3, prev_inside: bool| {
              let inside = pos.sub(dest).length() <= radius;
              inside && !prev_inside
          };

          // Outside last tick, inside now -> crossing, fires.
          assert!(crossed(Vec3::new(10.2, 0.0, 0.0), false));
          // Inside last tick, inside now -> no new crossing.
          assert!(!crossed(Vec3::new(10.1, 0.0, 0.0), true));
          // Outside now -> never fires regardless of prior.
          assert!(!crossed(Vec3::new(11.0, 0.0, 0.0), false));
          assert!(!crossed(Vec3::new(11.0, 0.0, 0.0), true));
      }

      #[test]
      fn arrival_crossed_uses_arrival_radius_constant() {
          use crate::autopilot::ARRIVAL_RADIUS;
          use crate::math::Vec3;
          let dest = Vec3::new(0.0, 0.0, 0.0);
          // A point just inside ARRIVAL_RADIUS, coming from outside -> fires.
          let just_inside = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
          assert!(arrival_crossed(just_inside, dest, false));
          // Same point, already inside -> no repeat.
          assert!(!arrival_crossed(just_inside, dest, true));
          // A point well outside -> never.
          let outside = Vec3::new(ARRIVAL_RADIUS * 10.0 + 1.0, 0.0, 0.0);
          assert!(!arrival_crossed(outside, dest, false));
      }
  ```

- [ ] **Step 8: Implement `arrival_crossed` (minimal).**
  Add the import at the top of `events.rs` (alongside the existing `use` lines):
  ```rust
  use crate::autopilot::ARRIVAL_RADIUS;
  use crate::math::Vec3;
  ```
  Then add the predicate above the `#[cfg(test)]` block:
  ```rust
  /// Pure arrival predicate. True iff the craft is within ARRIVAL_RADIUS of
  /// dest_pos now AND was outside it at the previous tick (a crossing, fires once).
  fn arrival_crossed(pos: Vec3, dest_pos: Vec3, prev_inside: bool) -> bool {
      let inside = pos.sub(dest_pos).length() <= ARRIVAL_RADIUS;
      inside && !prev_inside
  }
  ```

- [ ] **Step 9: Run the arrival predicate tests — confirm green.**
  ```
  cargo test -p jumpgate-core events::tests::arrival -- --nocolor
  ```
  EXPECTED: `test result: ok. 2 passed; 0 failed` (`arrival_crossing_contract_documented` and `arrival_crossed_uses_arrival_radius_constant` both `ok`).

- [ ] **Step 10: Implement the public `detect_boundary_events` wrapper (the contract fn) over the real stores.**
  This is the thin, **pure-read** glue. It reads, per craft index, the just-completed-tick quantized state and the prior-tick snapshot Task 4 stores on `ShipStore` (`prev_fuel`, `prev_inside_dest`). It resolves an `Entity(Body)` destination through `BodyStore.eph_index` + `Ephemeris::body_pos`. It emits at most one `Arrival` and one `FuelEmpty` per craft per tick and mutates nothing.

  Add the imports at the top of `events.rs` (note the **corrected** split: `NavState` from `crate::stores`, `NavDest`/`EntityRef` from `crate::contract`; **no `StateView`** — the function no longer takes `&World`):
  ```rust
  use crate::contract::{EntityRef, NavDest};
  use crate::ephemeris::Ephemeris;
  use crate::stores::{BodyStore, NavState, ShipStore};
  ```
  Then add the function above the test block:
  ```rust
  /// Detect Arrival/FuelEmpty at the tick boundary against QUANTIZED state and
  /// record them into `out`. Pure read: never mutates any store (enforced by the
  /// shared `&` refs). No reactivity — each predicate reads only state, and
  /// emitting an event cannot trigger another same-tick event.
  ///
  /// Reads the prior-tick snapshot from `ShipStore::prev_fuel` /
  /// `ShipStore::prev_inside_dest` (both index-aligned, populated in Task 4 and
  /// part of the canonical hashed state). Resolves an entity destination via
  /// `BodyStore::eph_index` + `Ephemeris::body_pos`.
  pub fn detect_boundary_events(
      ships: &ShipStore,
      bodies: &BodyStore,
      ephem: &Ephemeris,
      tick: Tick,
      out: &mut EventStream,
  ) {
      for idx in 0..ships.ids.len() {
          // Index -> stable CraftId via the Task-4 live-id accessor (see Depends-on).
          let id = ships.ids_at(idx);

          // FuelEmpty edge: fuel at/below eps now, above eps at prior tick.
          if fuel_just_emptied(ships.fuel_mass[idx], ships.prev_fuel[idx]) {
              out.emit(Event {
                  tick,
                  kind: EventKind::FuelEmpty { craft: id },
              });
          }

          // Arrival edge: only meaningful while Seeking toward a resolved dest.
          if let NavState::Seeking { dest, .. } = ships.nav[idx] {
              let dest_pos = match dest {
                  NavDest::Position(p) => p,
                  NavDest::Entity(EntityRef::Body(body_id)) => {
                      ephem.body_pos(bodies.eph_index[body_id.slot as usize], tick)
                  }
                  // Entity(Craft) destinations are not a v1 nav target; skip.
                  NavDest::Entity(EntityRef::Craft(_)) => continue,
              };
              if arrival_crossed(ships.pos[idx], dest_pos, ships.prev_inside_dest[idx]) {
                  out.emit(Event {
                      tick,
                      kind: EventKind::Arrival { craft: id, dest },
                  });
              }
          }
      }
  }
  ```
  Note: `ShipStore::ids_at(idx) -> CraftId` is the Task-4 live-id accessor (it does the `SlotMap::id_at` `expect` internally); do NOT inline a fabricated `SlotMap` iterator here. If Task 4's accessor has a different name at integration time, only this one call site changes; the tested predicates do not.

- [ ] **Step 11: Confirm the no-mutation / no-reactivity property by inspection + a compile check.**
  `detect_boundary_events` takes shared `&` refs to every store — the compiler enforces it cannot mutate them. Run the whole module to confirm everything still builds and passes:
  ```
  cargo test -p jumpgate-core events -- --nocolor
  ```
  EXPECTED: `test result: ok. 4 passed; 0 failed` (the buffer test, the fuel-edge test, and the two arrival tests; `0 ignored`).

- [ ] **Step 12: Re-export from `lib.rs` and lint clean.**
  Add the public surface re-export next to the other module re-exports in `crates/jumpgate-core/src/lib.rs`:
  ```rust
  pub use events::{detect_boundary_events, EventStream, FUEL_EMPTY_EPS};
  ```
  Then run the full clippy gate (binary crate — must use `--all-targets`, not `--lib`, per project note):
  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings (no `disallowed-methods` hits — the detector uses no `SystemTime`/`Instant`/`thread_rng`; it is a pure read of `tick`-derived state).

- [ ] **Step 13: Commit.**
  ```
  git add crates/jumpgate-core/src/events.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  feat(core): event layer — tick-boundary Arrival/FuelEmpty records

  EventStream record buffer (emit/since) plus pure edge predicates
  arrival_crossed / fuel_just_emptied. detect_boundary_events reads the
  ShipStore/BodyStore/Ephemeris directly (no fabricated World accessors):
  events are records, no bus, no reactivity; FuelEmpty fires once on the
  depletion edge, Arrival once on radius crossing, both tick-stamped
  against quantized state and the prev_fuel/prev_inside_dest snapshot.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: a commit is created on the current feature branch with the two changed files.

---

### Task 12: World.step assembly + StateView + projection/observer

Assemble the `World` aggregate: `reset` (build ephemeris, seed rng, spawn craft/bodies, return config hash), `step` (ingest commands canonically → Lod-dispatch → per-craft substepped Verlet integrate with gravity+thrust+fuel → boundary events → `tick++`), the `StateView` read trait, and the observer-parameterized `project()`. `dt` is owned by `World` (from `RunConfig`) and is NEVER a `step()` argument. Body positions are always derived from `tick` via the ephemeris, never from mutable body state.

This task is the consumer that finally instantiates `World`, so it also (a) re-points the contract's `ingest_commands(world: &mut World, …)` at the real type, and calls `detect_boundary_events(&self.ships, &self.bodies, &self.eph, …)` with the by-store-reference signature Task 11 actually defines, (b) consumes the single canonical `ActionLog` (`entries`/`commands_flat`/`config_hash` + `since_commands`) defined once in Task 10 — it does NOT redefine the struct, and (c) exercises the Lod must-shape seam (skip physics for dormant craft, emit `Wake` on dormant→active).

**Two cross-task contract facts this task DEPENDS ON (must already exist at entry, per the contract-surface doc):**
- `SlotMap<T>` (Task 2, `ids.rs`) exposes `insert(value: T) -> (u32, u32)` returning `(slot, generation)`, `iter_ids() -> impl Iterator<Item = (u32, u32)>` (live `(slot, generation)` pairs), `dense_index(slot: u32, generation: u32) -> Option<usize>`, and `id_at(dense_index: usize) -> (u32, u32)`. Task 2's own test suite covers all four (per the PROVIDER-defines-and-tests rule).
- `ShipStore` (Task 4, `stores.rs`) carries the two ADDITIVE boundary-edge SoA arrays `prev_fuel: Vec<f64>` and `prev_inside_dest: Vec<bool>` (spec §5.5 blesses "new per-ship scalars attach as additional SoA arrays"). They are written by `detect_boundary_events` (Task 11) reading the *new* quantized state vs. the *previous* tick, and copy-forwarded at the end of `step()` here. They are folded into `state_hash` at the position recorded in `HASH_FIELD_ORDER` (the canonical numbered field-order doc established by the plan-level FNV fix; the golden-hash test in Task 7 pins their zero-init contribution).
- `EventKind::Wake { craft: CraftId }` exists (added to `contract.rs` in Task 6 per the plan-level Lod fix). `Integrator` is defined ONCE in `contract.rs`; this task imports `crate::contract::Integrator` (never a second same-shaped trait).

**Files**
- Create: `crates/jumpgate-core/src/world.rs`
- Modify: `crates/jumpgate-core/src/lib.rs` (wire the `world` module + re-exports)
- Modify: `crates/jumpgate-core/src/ingest.rs` (re-point `ingest_commands` at the real `World`; `ActionLog` itself is unchanged — its canonical shape is owned by Task 10)
- Test: `crates/jumpgate-core/src/world.rs` (`#[cfg(test)] mod tests`)

Dependencies in play (all from Tasks 1–11, reference by exact name): `Vec3`, `CraftId`, `BodyId`, `SlotMap`, `Tick`, `Dt`, `RunConfig`, `ConfigHash`, `RngStreams`, `Target`, `EntityRef`, `NavDest`, `Command`, `CommandKind`, `Event`, `EventKind`, `Lod`, `Integrator`, `StateView`, `NavState`, `ShipStore`, `BodyStore`, `Effective`, `effective_params`, `Ephemeris`, `VelocityVerlet`, `substep_count`, `gravity_accel`, `thrust_accel_and_burn`, `autopilot_command`, `ActionLog`, `EventStream`, `detect_boundary_events`. New types this task defines: `World`, `Observer`, `FullObserver`, `View` (with accessor methods).

---

- [ ] **Step 1: Stub the `world` module and wire it into `lib.rs` so the crate still compiles. `View` carries fuel_capacity and ships accessor methods.**

Create `crates/jumpgate-core/src/world.rs` with the type skeletons (no behavior yet) so subsequent failing tests have something to name. NOTE the `View` shape: each craft row is `(CraftId, pos, vel, fuel, fuel_capacity)` and `View` exposes `craft_vel` / `craft_fuel` / `craft_fuel_capacity` accessor methods — Task 16 (`write_obs_frame_relative`) calls these, so the PROVIDER (this task) defines and tests them rather than letting a downstream task index a flat tuple it can't name.

```rust
//! World aggregate: owns all stores + ephemeris + rng + logs, drives the tick.
use crate::config::{ConfigHash, RunConfig};
use crate::contract::{
    Command, EntityRef, Event, EventKind, Integrator, Lod, NavDest, StateView, Target,
};
use crate::ephemeris::Ephemeris;
use crate::events::EventStream;
use crate::ids::{BodyId, CraftId, SlotMap};
use crate::ingest::ActionLog;
use crate::integrator::{gravity_accel, substep_count, VelocityVerlet};
use crate::math::Vec3;
use crate::rng::RngStreams;
use crate::ship::thrust_accel_and_burn;
use crate::stores::{effective_params, BodyStore, NavState, ShipStore};
use crate::time::{Dt, Tick};
use crate::autopilot::autopilot_command;

/// The authoritative simulation aggregate. Single writer; all facades read via `StateView`.
pub struct World {
    ships: ShipStore,
    bodies: BodyStore,
    eph: Ephemeris,
    rng: RngStreams,
    log: ActionLog,
    events: EventStream,
    tick: Tick,
    dt: Dt,
    config: RunConfig,
}

/// Read filter applied at the single projection seam (`project`). v1: all-visible.
pub trait Observer {
    fn visible(&self, target: EntityRef) -> bool;
}
/// v1 default observer: everything is visible.
pub struct FullObserver;
impl Observer for FullObserver {
    fn visible(&self, _target: EntityRef) -> bool {
        true
    }
}

/// Projected, presence-masked snapshot the obs layer reads.
/// Each craft row: (id, pos, vel, fuel_mass, fuel_capacity). Accessor methods below
/// are the contract the obs layer (Task 16 `write_obs_frame_relative`) reads through.
pub struct View {
    pub tick: Tick,
    pub craft: Vec<(CraftId, Vec3, Vec3, f64, f64)>,
    /// (id, pos) for each visible body at `tick`, in sorted-id order.
    pub bodies: Vec<(BodyId, Vec3)>,
}

impl View {
    fn craft_row(&self, id: CraftId) -> Option<&(CraftId, Vec3, Vec3, f64, f64)> {
        self.craft.iter().find(|r| r.0 == id)
    }
    pub fn craft_pos(&self, id: CraftId) -> Option<Vec3> {
        self.craft_row(id).map(|r| r.1)
    }
    pub fn craft_vel(&self, id: CraftId) -> Option<Vec3> {
        self.craft_row(id).map(|r| r.2)
    }
    pub fn craft_fuel(&self, id: CraftId) -> Option<f64> {
        self.craft_row(id).map(|r| r.3)
    }
    pub fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
        self.craft_row(id).map(|r| r.4)
    }
    pub fn body_pos(&self, id: BodyId) -> Option<Vec3> {
        self.bodies.iter().find(|r| r.0 == id).map(|r| r.1)
    }
}
```

Add to `crates/jumpgate-core/src/lib.rs` (alongside the existing `pub mod` lines and re-exports). Do NOT re-export `Integrator` here — it lives once in `contract.rs` and is re-exported there:

```rust
pub mod world;
pub use world::{FullObserver, Observer, View, World};
```

Run: `cargo build -p jumpgate-core`
EXPECTED: `Finished` with no errors (warnings about unused imports/fields are acceptable at this stage).

---

- [ ] **Step 2: Re-point `ingest_commands` at the real `World`. (`ActionLog` is NOT redefined here — its single canonical shape was provided once in Task 10, Step 7.)**

The contract declares `pub fn ingest_commands(world: &mut World, tick: Tick, cmds: &mut Vec<Command>)` and `StateView::recent_commands(&self, since: Tick) -> &[Command]`. In Task 10 (`ingest.rs`) `World` did not yet exist, so it was drafted against a placeholder. Now make it real, *before* `reset`/`step` reference it.

`ActionLog` already exists in its FINAL single shape from Task 10 — `{ entries: Vec<(Tick, Command)>, commands_flat: Vec<Command>, config_hash: ConfigHash }` — with `new(config_hash)`, `record`, `at`, and `since_commands`. Do NOT redeclare it here (the prior drafts that re-defined it in Task 12/14 with conflicting shapes were the drift this fix collapses). The `commands_flat` parallel vec is pushed in lockstep with `entries` by the single writer `record`, so `recent_commands` can return a zero-copy `&[Command]` tail slice without any desync (the contract trait fixes `recent_commands -> &[Command]`, which an entries-only iterator could not satisfy). For reference, the Task-10 shape and its slice accessors are:

```rust
// Defined ONCE in Task 10 (ingest.rs) — shown here for reference, not re-declared:
pub struct ActionLog {
    pub entries: Vec<(Tick, Command)>,
    /// Parallel to `entries`, pushed in lockstep by `record` (single writer, so
    /// it can never desync), so `at`/`since_commands` return zero-copy &[Command].
    pub commands_flat: Vec<Command>,
    /// Provenance stamp captured at construction (Task 14's replay guard compares it).
    pub config_hash: crate::config::ConfigHash,
}
// impl ActionLog { new(config_hash), record(tick, cmd) -> pushes BOTH vecs,
//   at(tick) -> &[Command], since_commands(since) -> &[Command] } // all from Task 10.
```

Edit `crates/jumpgate-core/src/ingest.rs` only to re-point `ingest_commands` at the real `World` and write through its stores/log/events in canonical order. It performs exactly three writes per `Target::Entity(EntityRef::Craft(_))` command — set `NavState::Seeking`, log (which pushes both `entries` and `commands_flat`), and emit `ActionIngested`:

```rust
/// THE single ingestion path. Sorts by `command_sort_key` (total over World/Sim/
/// entity scopes), resolves each NavDest into a concrete `NavState::Seeking`,
/// logs (resolved values, never re-rolled intentions — the lever invariant),
/// and emits `ActionIngested`. v1's only `CommandKind` is `Destination`.
pub fn ingest_commands(world: &mut crate::world::World, tick: Tick, cmds: &mut Vec<Command>) {
    cmds.sort_by_key(command_sort_key);
    for &cmd in cmds.iter() {
        world.log_mut().record(tick, cmd);
        match cmd.target {
            Target::Entity(EntityRef::Craft(id)) => {
                if let CommandKind::Destination { dest, burn_budget } = cmd.kind {
                    let dv = burn_budget.unwrap_or(f64::INFINITY);
                    world.set_nav(id, NavState::Seeking { dest, dv_remaining: dv });
                }
            }
            // World/Sim/Body targets carry no v1 effect beyond logging (seam only).
            _ => {}
        }
        world.events_mut().emit(Event {
            tick,
            kind: EventKind::ActionIngested { target: cmd.target },
        });
    }
    cmds.clear();
}
```

> NOTE: `ingest_commands` needs three narrow mutators on `World` (`log_mut`, `events_mut`, `set_nav`) rather than touching private fields from another module. These are defined in Step 3 alongside `reset`. `NavDest`/`Vec3` imports above are retained for Task 10's existing validation helpers; if Task 10 did not use them, drop the unused `use` lines to satisfy clippy.

Run: `cargo build -p jumpgate-core`
EXPECTED: error `no method named log_mut/events_mut/set_nav found for struct World` (those land in Step 3). This confirms the call sites compile against the right `World` type and only the not-yet-written mutators are missing. (If Task 2/4 symbols are mis-named, the error will instead name `SlotMap`/`ShipStore` — fix those provider tasks, not this call site.)

---

- [ ] **Step 3: Write a FAILING test that `reset` builds a World and returns a deterministic config hash; `tick()` starts at 0.**

Append to `crates/jumpgate-core/src/world.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BaseSpec, BodyInit, CraftInit, OrbitalElements, SubstepCfg};
    use crate::contract::{CommandKind, StateView};

    fn one_body_one_craft() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(1.0),
            softening: 1e-4,
            substep_cfg: SubstepCfg { accel_ref: 1.0, max_substeps: 64 },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1.0, // 1 M_sun central star at the origin (a == 0.0 conic)
                elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-12,
                    base_max_thrust: 1e-9,
                    base_exhaust_velocity: 1e-3,
                    base_fuel_capacity: 1e-12,
                },
                // 1 AU out, on a roughly circular prograde orbit (v ~ sqrt(GM/r)).
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 0.0172, 0.0),
                fuel_mass: 1e-12,
            }],
        }
    }

    #[test]
    fn reset_starts_at_tick_zero_and_hashes_config() {
        let cfg = one_body_one_craft();
        let expected = cfg.config_hash();
        let (world, returned) = World::reset(cfg);
        assert_eq!(returned, expected, "reset must return RunConfig::config_hash()");
        assert_eq!(world.tick(), Tick(0));
        assert_eq!(world.dt().get(), 1.0);
        assert_eq!(world.craft_ids().len(), 1);
        assert_eq!(world.body_ids().len(), 1);
    }
}
```

Run: `cargo test -p jumpgate-core world::tests::reset_starts_at_tick_zero -- --nocapture`
EXPECTED: compile error — `World::reset` not found / `impl StateView for World` missing. Intended red state.

---

- [ ] **Step 4: Implement `World::reset`, the narrow mutators, and the `StateView` impl to make Steps 2 and 3 build/pass. Initialize the prev_fuel/prev_inside_dest boundary-edge arrays. Guard the a==0.0 star.**

Add to `crates/jumpgate-core/src/world.rs` (after the struct defs, before `mod tests`):

```rust
impl World {
    /// Build a World from a RunConfig: precompute ephemeris, seed rng from the
    /// master seed, spawn bodies then craft, and return the config hash.
    /// `seed` and `dt` come from `cfg`; nothing is read from the environment.
    pub fn reset(cfg: RunConfig) -> (World, ConfigHash) {
        let hash = cfg.config_hash();
        // Ephemeris::precompute (Task 9) must yield a FINITE position for an a==0.0
        // conic: a central star sits at the origin for all ticks (no NaN from a 0/0
        // mean-anomaly solve). The Task 7 gravity_accel softening (r^2 + eps^2)^1.5
        // then keeps accel finite even when a craft coincides with the star.
        let eph = Ephemeris::precompute(&cfg.bodies, cfg.dt, cfg.ephemeris_window);

        let mut bodies = BodyStore { ids: SlotMap::new(), mass: Vec::new(), eph_index: Vec::new() };
        for (i, b) in cfg.bodies.iter().enumerate() {
            bodies.ids.insert(());
            bodies.mass.push(b.mass);
            bodies.eph_index.push(i);
        }

        let mut ships = ShipStore {
            ids: SlotMap::new(),
            pos: Vec::new(),
            vel: Vec::new(),
            fuel_mass: Vec::new(),
            spec: Vec::new(),
            nav: Vec::new(),
            lod: Vec::new(),
            prev_fuel: Vec::new(),
            prev_inside_dest: Vec::new(),
        };
        for c in cfg.craft.iter() {
            ships.ids.insert(());
            ships.pos.push(c.pos);
            ships.vel.push(c.vel);
            ships.fuel_mass.push(c.fuel_mass);
            ships.spec.push(c.spec.clone());
            ships.nav.push(NavState::Idle);
            ships.lod.push(Lod::Player);
            // Boundary-edge previous state: at tick 0 prev == current, so no spurious
            // FuelEmpty/Arrival fires on the first step (edge detection needs a prior).
            ships.prev_fuel.push(c.fuel_mass);
            ships.prev_inside_dest.push(false);
        }

        let rng = RngStreams::from_master(cfg.master_seed);
        let dt = cfg.dt;
        let world = World {
            ships,
            bodies,
            eph,
            rng,
            log: ActionLog { entries: Vec::new(), commands_flat: Vec::new(), config_hash: hash },
            events: EventStream { events: Vec::new() },
            tick: Tick(0),
            dt,
            config: cfg,
        };
        (world, hash)
    }

    // --- narrow mutators the single ingestion path writes through (Step 2) ---
    pub(crate) fn log_mut(&mut self) -> &mut ActionLog {
        &mut self.log
    }
    pub(crate) fn events_mut(&mut self) -> &mut EventStream {
        &mut self.events
    }
    pub(crate) fn set_nav(&mut self, id: CraftId, nav: NavState) {
        if let Some(i) = self.ship_index(id) {
            self.ships.nav[i] = nav;
        }
    }

    fn ship_index(&self, id: CraftId) -> Option<usize> {
        self.ships.ids.dense_index(id.slot, id.generation)
    }
    fn body_index(&self, id: BodyId) -> Option<usize> {
        self.bodies.ids.dense_index(id.slot, id.generation)
    }
    fn craft_id_at(&self, dense_index: usize) -> CraftId {
        // SlotMap::id_at returns Option; delegate to the ShipStore wrapper
        // `ids_at`, which does the `expect` internally and returns CraftId.
        self.ships.ids_at(dense_index)
    }
}

impl StateView for World {
    fn tick(&self) -> Tick {
        self.tick
    }
    fn dt(&self) -> Dt {
        self.dt
    }
    fn craft_ids(&self) -> Vec<CraftId> {
        let mut v: Vec<CraftId> = self
            .ships
            .ids
            .iter_ids()
            .map(|(slot, generation)| CraftId { slot, generation })
            .collect();
        v.sort();
        v
    }
    fn craft_pos(&self, id: CraftId) -> Option<Vec3> {
        self.ship_index(id).map(|i| self.ships.pos[i])
    }
    fn craft_vel(&self, id: CraftId) -> Option<Vec3> {
        self.ship_index(id).map(|i| self.ships.vel[i])
    }
    fn craft_fuel(&self, id: CraftId) -> Option<f64> {
        self.ship_index(id).map(|i| self.ships.fuel_mass[i])
    }
    fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
        // effective_params (use crate::stores::effective_params, in scope above)
        // applies the spec's modifiers; fuel_capacity is the effective field.
        self.ship_index(id)
            .map(|i| effective_params(&self.ships.spec[i]).fuel_capacity)
    }
    fn body_ids(&self) -> Vec<BodyId> {
        let mut v: Vec<BodyId> = self
            .bodies
            .ids
            .iter_ids()
            .map(|(slot, generation)| BodyId { slot, generation })
            .collect();
        v.sort();
        v
    }
    fn body_pos(&self, id: BodyId, tick: Tick) -> Option<Vec3> {
        self.body_index(id).map(|i| self.eph.body_pos(self.bodies.eph_index[i], tick))
    }
    fn recent_commands(&self, since: Tick) -> &[Command] {
        self.log.since_commands(since)
    }
    fn recent_events(&self, since: Tick) -> &[Event] {
        self.events.since(since)
    }
    fn lod(&self, id: CraftId) -> Option<Lod> {
        self.ship_index(id).map(|i| self.ships.lod[i])
    }
}
```

Run: `cargo test -p jumpgate-core world::tests::reset_starts_at_tick_zero -- --nocapture`
EXPECTED: `test result: ok. 1 passed; 0 failed`

---

- [ ] **Step 5: Write a FAILING test that `step` advances the tick and a thrust-less craft coasts under gravity (body pos comes from ephemeris, not stored state), and the a==0.0 star sample is finite and stable.**

Append inside `mod tests` in `crates/jumpgate-core/src/world.rs`:

```rust
#[test]
fn step_advances_tick_and_coasts_under_gravity() {
    let cfg = one_body_one_craft();
    let (mut world, _) = World::reset(cfg);

    let start_r = world.craft_pos(world.craft_ids()[0]).unwrap().length();
    let body = world.body_ids()[0];
    let body_at_0 = world.body_pos(body, Tick(0)).unwrap();
    // a==0.0 star fix (Task 7/9): the sample must be FINITE, else the assert below
    // is NaN != NaN and the determinism claim is vacuous.
    assert!(body_at_0.x.is_finite() && body_at_0.y.is_finite() && body_at_0.z.is_finite());

    // No commands: the craft coasts (nav stays Idle, autopilot throttles 0).
    let mut empty: Vec<Command> = Vec::new();
    for _ in 0..10 {
        world.step(&mut empty);
    }

    assert_eq!(world.tick(), Tick(10), "10 steps -> tick 10");

    // Body position is derived from tick via ephemeris, never mutated in a store:
    // body_pos(t) must equal the ephemeris sample for that t regardless of stepping.
    let body_at_0_again = world.body_pos(body, Tick(0)).unwrap();
    assert_eq!(body_at_0, body_at_0_again, "body_pos(0) is tick-derived, not stateful");

    // The craft moved but did not blow up: radius stays within a sane band.
    let r = world.craft_pos(world.craft_ids()[0]).unwrap().length();
    assert!(r > 0.5 * start_r && r < 2.0 * start_r, "coast stayed bounded: r={r}");
}
```

Run: `cargo test -p jumpgate-core world::tests::step_advances_tick -- --nocapture`
EXPECTED: compile error — `World::step` not found. Intended red state.

---

- [ ] **Step 6: Implement `World::step` — ingest → Lod-dispatch → per-craft autopilot/integrate/fuel → boundary events (borrow-split) → copy-forward prev arrays → tick++.**

Add to the `impl World` block in `crates/jumpgate-core/src/world.rs`:

```rust
impl World {
    /// Advance one tick. `dt` is owned by the World, never an argument.
    /// (1) ingest commands canonically, (2) Lod-dispatch: skip physics for dormant
    /// (`Lod::Nothing`) craft and emit `Wake` on dormant->active, integrate the rest,
    /// (3) detect boundary events against the new quantized state, (4) copy-forward
    /// the boundary-edge arrays, (5) tick++.
    pub fn step(&mut self, cmds: &mut Vec<Command>) {
        let cur = self.tick;
        let dt = self.dt.get();
        let next = Tick(cur.0 + 1);

        // (1) single ingestion path (Step 2): sorts canonically, resolves NavDest
        //     into NavState, logs, emits ActionIngested.
        crate::ingest::ingest_commands(self, cur, cmds);

        // Snapshot body eph_index + mass to avoid borrowing self inside the closure.
        let body_indices: Vec<(usize, f64)> = (0..self.bodies.mass.len())
            .map(|i| (self.bodies.eph_index[i], self.bodies.mass[i]))
            .collect();
        let softening = self.config.softening;
        let substep_cfg = self.config.substep_cfg;
        let integrator = VelocityVerlet;
        let n_craft = self.ships.pos.len();

        for ci in 0..n_craft {
            // (2) Lod-dispatch must-shape seam. v1 implements `Player` (full physics).
            // `Nothing` = dormant / not ticked (spec §3.2): skip physics entirely.
            // A future tier that wakes a craft (Nothing -> Player) emits `Wake`.
            match self.ships.lod[ci] {
                Lod::Nothing => {
                    // Dormant: state is propagated closed-form elsewhere; do nothing here.
                    // (Seam exercised; analytic propagation deferred per spec.)
                    continue;
                }
                Lod::NpcInteraction => {
                    // Deferred tier; v1 falls through to Player-grade physics so the
                    // dispatch branch exists and is type-checked.
                }
                Lod::Player => {}
            }

            let eff = effective_params(&self.ships.spec[ci]);
            let pos = self.ships.pos[ci];
            let vel = self.ships.vel[ci];
            let fuel = self.ships.fuel_mass[ci];

            let dest_pos = match self.ships.nav[ci] {
                NavState::Seeking { dest, .. } => self.resolve_dest_pos(dest, cur),
                NavState::Idle => pos, // unused (throttle will be 0)
            };
            let (thrust_dir, throttle) =
                autopilot_command(self.ships.nav[ci], pos, vel, dest_pos, &eff);

            let (thrust_accel, fuel_consumed) =
                thrust_accel_and_burn(&eff, fuel, thrust_dir, throttle, dt);

            // accel_at(p, sub_t_days): softened gravity at the sub-tick instant the
            // body has moved to, plus the (tick-constant) thrust acceleration.
            let eph = &self.eph;
            let accel_at = |p: Vec3, sub_t: f64| -> Vec3 {
                let frac = sub_t / dt; // days into the tick -> fractional tick
                let body_positions: Vec<(Vec3, f64)> = body_indices
                    .iter()
                    .map(|&(eidx, m)| (eph.body_pos_subtick(eidx, cur, frac), m))
                    .collect();
                gravity_accel(p, &body_positions, softening).add(thrust_accel)
            };

            // N = pure fn of QUANTIZED total local acceleration magnitude.
            let total_accel_mag = accel_at(pos, 0.0).length();
            let n = substep_count(total_accel_mag, substep_cfg);

            let (new_pos, new_vel) = integrator.step_craft(pos, vel, &accel_at, dt, n);

            self.ships.pos[ci] = new_pos;
            self.ships.vel[ci] = new_vel;
            self.ships.fuel_mass[ci] = (fuel - fuel_consumed).max(0.0);

            if throttle > 0.0 {
                let dv = thrust_accel.length() * dt;
                if let NavState::Seeking { dest, dv_remaining } = self.ships.nav[ci] {
                    self.ships.nav[ci] =
                        NavState::Seeking { dest, dv_remaining: dv_remaining - dv };
                }
                let id = self.craft_id_at(ci);
                self.events.emit(Event { tick: next, kind: EventKind::ThrustApplied { craft: id, dv } });
            }
        }

        // (3) detect Arrival / FuelEmpty at the new boundary. MANDATORY borrow split:
        //     detect_boundary_events borrows `&self.ships`/`&self.bodies`/`&self.eph`
        //     (reads stores) AND writes the event sink; passing `&mut self.events`
        //     alongside `&self.*` field borrows is E0502. Take the EventStream out,
        //     run detection against the shared field borrows, put it back.
        let mut ev = std::mem::take(&mut self.events);
        crate::events::detect_boundary_events(&self.ships, &self.bodies, &self.eph, next, &mut ev);
        self.events = ev;

        // (4) copy-forward the boundary-edge arrays so next tick's detection has a
        //     prior. These arrays are folded into state_hash at the position fixed by
        //     HASH_FIELD_ORDER (Task 7 golden-hash test pins their zero-init value).
        for ci in 0..n_craft {
            self.ships.prev_fuel[ci] = self.ships.fuel_mass[ci];
            self.ships.prev_inside_dest[ci] = match self.ships.nav[ci] {
                NavState::Seeking { dest, .. } => {
                    let dp = self.resolve_dest_pos(dest, next);
                    self.ships.pos[ci].sub(dp).length() <= crate::autopilot::ARRIVAL_RADIUS
                }
                NavState::Idle => false,
            };
        }

        // (5) advance.
        self.tick = next;
    }

    /// Resolve a NavDest to a concrete position at `tick`
    /// (Entity bodies are tick-derived from the ephemeris).
    fn resolve_dest_pos(&self, dest: NavDest, tick: Tick) -> Vec3 {
        match dest {
            NavDest::Position(p) => p,
            NavDest::Entity(EntityRef::Body(b)) => self.body_pos(b, tick).unwrap_or(Vec3::ZERO),
            NavDest::Entity(EntityRef::Craft(c)) => self.craft_pos(c).unwrap_or(Vec3::ZERO),
        }
    }
}
```

> NOTE on `std::mem::take`: it requires `EventStream: Default`. Task 11 (`events.rs`) must `#[derive(Default)]` on `EventStream` (its single field `events: Vec<Event>` is `Default`); the contract-surface doc records this as a Task-11-provided requirement consumed here. If Task 11 did not derive it, add `impl Default for EventStream { fn default() -> Self { EventStream { events: Vec::new() } } }` there.
>
> NOTE on `detect_boundary_events`: its Task 11 signature takes the stores by reference, NOT the whole `World` — `detect_boundary_events(ships: &ShipStore, bodies: &BodyStore, ephem: &Ephemeris, tick: Tick, out: &mut EventStream)` (the param-narrowing form, matching Task 11's actual definition). It reads `ships.prev_fuel`/`prev_inside_dest` vs. the new quantized state and emits `FuelEmpty`/`Arrival`; it must NOT mutate the prev_* arrays — copy-forward is owned by step() (4) above, the single writer. (The contract-surface rule: the PROVIDER of `prev_*` is Task 4; the WRITER on the tick edge is Task 12 step(); the READER is Task 11.) Passing the three store fields separately (not `&self`) is what makes the `std::mem::take`/field-borrow split above sound.
>
> NOTE on the fuel/substep seam: `Integrator::step_craft` returns only `(pos, vel)` and `accel_at` is `Fn` (no per-substep side effects). v1 holds `thrust_accel` tick-constant (the navigator macro-action is fixed within a tick) and debits fuel once over `dt`. Finer substep mass-bleed is a deferred refinement that does not change the seam.

Run: `cargo test -p jumpgate-core world::tests::step_advances_tick -- --nocapture`
EXPECTED: `test result: ok. 1 passed; 0 failed`

---

- [ ] **Step 7: Write and pass a test that a commanded craft moves toward its destination and that `StateView` exposes the recorded command + the ActionIngested event.**

Append inside `mod tests` in `crates/jumpgate-core/src/world.rs`:

```rust
#[test]
fn commanded_craft_moves_toward_dest_and_history_is_visible() {
    use crate::contract::{EntityRef, NavDest, Target};
    let cfg = one_body_one_craft();
    let (mut world, _) = World::reset(cfg);
    let id = world.craft_ids()[0];

    let dest = NavDest::Position(Vec3::new(3.0, 0.0, 0.0));
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(id)),
        kind: CommandKind::Destination { dest, burn_budget: Some(0.05) },
    }];

    let r0 = world.craft_pos(id).unwrap().length();
    let d0 = world.craft_pos(id).unwrap().sub(Vec3::new(3.0, 0.0, 0.0)).length();

    world.step(&mut cmds); // tick 0 -> 1: ingest + integrate

    // History: the command was logged at tick 0 and is visible via StateView.
    let recent = world.recent_commands(Tick(0));
    assert_eq!(recent.len(), 1, "the issued command is recorded and exposed");
    assert!(
        matches!(recent[0].kind, CommandKind::Destination { .. }),
        "recorded command kind preserved"
    );

    // The single ingestion path emitted an ActionIngested event.
    let evs = world.recent_events(Tick(0));
    assert!(
        evs.iter().any(|e| matches!(
            e.kind,
            EventKind::ActionIngested { target: Target::Entity(EntityRef::Craft(_)) }
        )),
        "ingestion emits ActionIngested"
    );

    // Keep stepping; the craft should net-approach the destination.
    for _ in 0..20 {
        let mut none: Vec<Command> = Vec::new();
        world.step(&mut none);
    }
    let d1 = world.craft_pos(id).unwrap().sub(Vec3::new(3.0, 0.0, 0.0)).length();
    let r1 = world.craft_pos(id).unwrap().length();
    assert!(d1 < d0, "craft moved toward dest: {d0} -> {d1}");
    assert!(r1 > r0, "thrusting outward increased orbital radius: {r0} -> {r1}");
}
```

Run: `cargo test -p jumpgate-core world::tests::commanded_craft_moves -- --nocapture`
EXPECTED: `test result: ok. 1 passed; 0 failed`. (If it fails, the most likely cause is `ingest_commands` not performing all three writes — set `NavState::Seeking`, log/push `commands_flat`, emit `ActionIngested` — re-check Step 2.)

---

- [ ] **Step 8: Write a FAILING test for the Lod-dispatch seam — a dormant (`Lod::Nothing`) craft is not integrated.**

This exercises the must-shape Lod seam directly (spec §3.2). Append inside `mod tests`:

```rust
#[test]
fn dormant_craft_skips_physics() {
    let cfg = one_body_one_craft();
    let (mut world, _) = World::reset(cfg);
    let id = world.craft_ids()[0];
    let p0 = world.craft_pos(id).unwrap();

    // Force the craft dormant via the Lod seam (test-only mutator below).
    world.set_lod_for_test(id, Lod::Nothing);

    let mut empty: Vec<Command> = Vec::new();
    for _ in 0..10 {
        world.step(&mut empty);
    }
    // Dormant craft are not ticked: position is unchanged.
    assert_eq!(world.craft_pos(id).unwrap(), p0, "Lod::Nothing skips integration");
    assert_eq!(world.tick(), Tick(10));
}
```

Add the test-only mutator to the `impl World` block (kept `#[cfg(test)]` so it is not part of the production surface):

```rust
impl World {
    #[cfg(test)]
    fn set_lod_for_test(&mut self, id: CraftId, lod: Lod) {
        if let Some(i) = self.ship_index(id) {
            self.ships.lod[i] = lod;
        }
    }
}
```

Run: `cargo test -p jumpgate-core world::tests::dormant_craft_skips_physics -- --nocapture`
EXPECTED: `test result: ok. 1 passed; 0 failed` (the `Lod::Nothing => continue;` branch from Step 6 makes this pass on first run; the test locks the seam against regression).

---

- [ ] **Step 9: Write a FAILING test for `project(observer)` — FullObserver yields a full View with working accessors; a deny-all observer yields an empty one.**

Append inside `mod tests`:

```rust
struct DenyAll;
impl Observer for DenyAll {
    fn visible(&self, _t: crate::contract::EntityRef) -> bool {
        false
    }
}

#[test]
fn project_respects_observer_visibility_and_accessors() {
    let cfg = one_body_one_craft();
    let (world, _) = World::reset(cfg);
    let cid = world.craft_ids()[0];

    let full = world.project(&FullObserver);
    assert_eq!(full.tick, Tick(0));
    assert_eq!(full.craft.len(), 1, "FullObserver sees the one craft");
    assert_eq!(full.bodies.len(), 1, "FullObserver sees the one body");

    // View accessor methods (the contract Task 16's write_obs_frame_relative reads):
    assert_eq!(full.craft_pos(cid), world.craft_pos(cid));
    assert_eq!(full.craft_vel(cid), world.craft_vel(cid));
    assert_eq!(full.craft_fuel(cid), world.craft_fuel(cid));
    assert_eq!(full.craft_fuel_capacity(cid), Some(1e-12), "fuel_capacity surfaced");
    // Body position in the View is the tick-derived ephemeris sample.
    assert_eq!(full.bodies[0].1, world.body_pos(world.body_ids()[0], Tick(0)).unwrap());

    let none = world.project(&DenyAll);
    assert!(none.craft.is_empty() && none.bodies.is_empty(), "deny-all hides all entities");
}
```

Run: `cargo test -p jumpgate-core world::tests::project_respects_observer -- --nocapture`
EXPECTED: compile error — `World::project` not found. Intended red state.

---

- [ ] **Step 10: Implement `World::project` — the single observer/visibility seam, emitting the fuel_capacity column.**

Add to the `impl World` block in `crates/jumpgate-core/src/world.rs`:

```rust
impl World {
    /// Observer-parameterized projection. The presence mask is sourced from the
    /// single `visible(observer, entity)` predicate (all-true for `FullObserver`).
    /// This is the ONE location a future fog-of-war / per-faction filter edits.
    pub fn project<O: Observer>(&self, observer: &O) -> View {
        let mut craft = Vec::new();
        for id in self.craft_ids() {
            if observer.visible(EntityRef::Craft(id)) {
                let i = self.ship_index(id).expect("live id");
                // effective fuel_capacity rides the accessor seam (effective==base in v1).
                let cap = effective_params(&self.ships.spec[i]).fuel_capacity;
                craft.push((
                    id,
                    self.ships.pos[i],
                    self.ships.vel[i],
                    self.ships.fuel_mass[i],
                    cap,
                ));
            }
        }
        let mut bodies = Vec::new();
        let t = self.tick;
        for id in self.body_ids() {
            if observer.visible(EntityRef::Body(id)) {
                bodies.push((id, self.body_pos(id, t).expect("live id")));
            }
        }
        View { tick: t, craft, bodies }
    }
}
```

Run: `cargo test -p jumpgate-core world::tests::project_respects_observer -- --nocapture`
EXPECTED: `test result: ok. 1 passed; 0 failed`

---

- [ ] **Step 11: Run the full crate test suite + clippy to confirm no regressions, no banned methods, and that the hash field-order change is golden-pinned.**

Run: `cargo test -p jumpgate-core -- --nocapture`
EXPECTED: `test result: ok.` for the whole crate. This MUST include Task 7's golden-hash test (`state_hash` of a zero-init / fresh-`reset` world equals the hardcoded `HASH_FIELD_ORDER` value). Because this task added `prev_fuel`/`prev_inside_dest` to `ShipStore` — which Task 7 folds into the hash at their `HASH_FIELD_ORDER` position — that golden test is what proves the field-coverage change was intentional and not a silent drift. If the golden test fails, the hash field order or the prev_* init changed; update the golden constant deliberately and bump `HASH_VERSION`, do not paper over it.

Run: `cargo clippy -p jumpgate-core --all-targets -- -D warnings`
EXPECTED: `Finished` with no warnings and no `disallowed-methods` hits (no `SystemTime`/`Instant::now`/`thread_rng`/`from_entropy`/`env::var` introduced — `World` reads time only from `cfg.dt` and `tick`, rng only via `RngStreams::from_master`). `--all-targets` lints the test module too (this is a binary-free lib crate; `--lib` would skip the tests).

---

- [ ] **Step 12: Commit Task 12.**

Run:
```
git checkout -b task-12-world-step
git add crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/lib.rs crates/jumpgate-core/src/ingest.rs
git commit -m "$(cat <<'EOF'
Task 12: World.reset/step assembly + StateView + observer projection

Assemble the World aggregate (ShipStore/BodyStore/Ephemeris/RngStreams/
ActionLog/EventStream/tick/dt/config). reset() precomputes the ephemeris,
seeds rng from the master seed, spawns bodies+craft, initializes the
prev_fuel/prev_inside_dest boundary-edge arrays, and returns the config hash.
step() runs the canonical assembly: ingest_commands -> Lod-dispatch (skip
Lod::Nothing, integrate the rest) -> per-craft autopilot + substepped
VelocityVerlet integrate with softened gravity + thrust + fuel burn ->
boundary events (borrow-split via std::mem::take to avoid E0502) ->
copy-forward the boundary-edge arrays -> tick++. dt is owned by World, never a
step arg. Re-point ingest_commands at the real World and call
detect_boundary_events(&ships,&bodies,&eph,..) (Task 11's by-store-ref
signature); consume the canonical ActionLog from Task 10 (entries +
commands_flat + config_hash) so StateView::recent_commands is zero-copy.
Implement StateView (body_pos tick-derived from the ephemeris; commands/events
exposed; craft_fuel_capacity via effective_params) and the
observer-parameterized project() with View accessor methods (incl.
fuel_capacity) the obs layer reads through; FullObserver is all-visible.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```
EXPECTED: a commit on branch `task-12-world-step` with the three files.

---

### Task 13: FNV-1a per-tick state hash (incl. slot-map cursor) + authoritative HASH_FIELD_ORDER

**Goal:** A canonical per-tick state hash over `f64::to_bits()` in a fixed, **numbered** field-then-id order, including each store's slot-map allocator cursor, behind a versioned `MAGIC + FORMAT_VERSION` header. Implements §6's "State hash test surface": FNV-1a, explicit little-endian, no serde/json in the path. This task is also the **authoritative home of the `HASH_FIELD_ORDER` specification** — the single numbered list every other task that touches the hash input (Tasks 4, 5, 7, and the Task 11 fix that adds `prev_fuel`/`prev_inside_dest`) must edit and re-version. A **golden state-hash test** pins the hash of a zero-initialised world to a hardcoded value so any silent change to field coverage or order is caught here, not in production.

**Depends on:**
- Task 4 (`ids.rs` — `SlotMap<T>` with `new()`, `len()`, `cursor() -> u64`).
- Task 6 (`contract.rs` — `StateView` trait: `tick`, `body_ids`, `craft_ids`, `craft_pos`, `craft_vel`, `craft_fuel`).
- Task 12 (`world.rs` — `World` with `pub(crate)` fields `ships: ShipStore` and `bodies: BodyStore`, each `{ ids: SlotMap<()>, .. }`; `World::reset(cfg) -> (World, ConfigHash)`; `impl StateView for World`).

**Contract-surface symbols consumed (must already be defined + tested by their providing task — see the cross-task contract-surface doc):**
- `SlotMap::new`, `SlotMap::cursor` (Task 4).
- `StateView::{tick, body_ids, craft_ids, craft_pos, craft_vel, craft_fuel}` (Task 6).
- `World::reset`, `World.ships.ids`, `World.bodies.ids` (Task 12).
- `Vec3::to_bits`, `Vec3::new`, `Vec3::ZERO` (Task 1/3, `math.rs`).
- `Tick(pub u64)` field `.0` (Task 1/3, `time.rs`).

**Contract-surface symbols PROVIDED by this task (every downstream caller's call site must match these signatures; this task's own test module exercises each one):**
- `pub const HASH_MAGIC: u64`
- `pub const HASH_FORMAT_VERSION: u32`
- `pub struct FnvHasher` + `FnvHasher::new`, `FnvHasher::write_u64(&mut self, v: u64)`, `FnvHasher::finish(self) -> u64`
- `pub fn state_hash(world: &World) -> u64`
- `pub fn write_store_cursor<T>(h: &mut FnvHasher, store: &SlotMap<T>)`

Files:
- Modify: `crates/jumpgate-core/src/hash.rs` (already exists — the Task-3 anchor; Task 13 ADDS `state_hash` + `write_store_cursor`)
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/hash.rs` (`#[cfg(test)] mod tests`)

Design constraints (pinned before drafting):
- **FNV-1a is byte-wise.** `write_u64` decomposes the value into 8 little-endian bytes and runs the canonical per-byte loop `h = (h ^ byte) * PRIME`. Constants: offset basis `0xcbf29ce484222325`, prime `0x100000001b3`. NOT a word-at-a-time XOR-multiply.
- **`HASH_FIELD_ORDER` is the drift-lock.** The exact sequence of words mixed into `state_hash` is documented as a numbered list in a module-level doc comment, each entry tagged with the task that introduced it. Any task that adds a hashed field appends to this list AND bumps `HASH_FORMAT_VERSION` AND updates the golden-hash constant in Step 6. The golden test is the enforcement.
- **NavState (word 12) is the under-pinned field — test it explicitly.** The golden zero-world test only exercises `NavState::Idle`, so the entire multi-word `Seeking` encoding would otherwise be unpinned. Add a unit test asserting the encoding is self-delimiting and collision-free: a craft whose resolved dest is `NavDest::Position(Vec3::new(x,0,0))` and one whose resolved dest is `NavDest::Entity(Craft(CraftId{slot:x as u32, generation:0}))` MUST produce DIFFERENT `state_hash` values (the NavDest discriminant must be folded before the payload). Also add a `Seeking`-state golden so word 12's full word-stream is pinned, not just `Idle`'s single discriminant.
- **Bodies are on-rails.** Body positions derive from `tick` (already hashed), so `state_hash` does NOT hash body positions — only the `BodyStore` slot-map cursor and (sorted) body ids/masses-via-id. Ship state (pos/vel/fuel) is hashed in full.
- **TDD fail-first ordering for the cursor.** Step 4's first `state_hash` impl deliberately OMITS the slot-map cursors. The cursor-participation golden test (Step 6) is written to FAIL against that impl, then Step 7 adds the cursor writes to make it pass. This is the only sequence that honors fail-first for the cursor requirement.

- [ ] **Step 1: Extend the `hash` re-exports in `lib.rs` to add the not-yet-written symbols (expect a compile error on those, NOT on the module).**

  `crates/jumpgate-core/src/hash.rs` already exists (the Task-3 anchor) and `pub mod hash;` plus `FnvHasher`/`HASH_MAGIC`/`HASH_FORMAT_VERSION` are already declared and re-exported. Edit `crates/jumpgate-core/src/lib.rs` to APPEND the two symbols Task 13 adds (`state_hash`, `write_store_cursor`) to the existing `hash::*` re-export. (`pub mod hash;` must come after `pub mod world;` since `state_hash` reads `World` — confirm it does; do not re-add the module line if it is already present.)

  ```rust
  pub use hash::{state_hash, write_store_cursor, FnvHasher, HASH_FORMAT_VERSION, HASH_MAGIC};
  ```

  > Do NOT add `Integrator` or any `contract::*` re-export here; the `Integrator` trait is defined and re-exported solely by `contract.rs` (Task 6). This task touches only `hash::*` symbols.

  Run it and confirm it fails because `state_hash`/`write_store_cursor` are not defined yet (the file EXISTS — this is NOT an `E0583` missing-file error):

  ```
  cargo build -p jumpgate-core 2>&1 | head -5
  ```
  EXPECTED: an error such as ``error[E0432]: unresolved import `hash::state_hash` `` / `` `hash::write_store_cursor` `` (the functions are not defined until Steps 5/7). This confirms the re-export is wired against the right symbols.

- [ ] **Step 2: Confirm the pre-existing `hash.rs` anchor (`FnvHasher`/`HASH_MAGIC`/`HASH_FORMAT_VERSION`/`write_u64` + their tests already landed in Task 3).**

  `hash.rs` ALREADY EXISTS and is complete for the hasher + header layer — do **not** recreate it. The committed Task-3 anchor provides `HASH_MAGIC`, `HASH_FORMAT_VERSION`, `GOLDEN_ZERO_STATE_HASH`, the `FnvHasher` (with `new()`/`write_u64`/`finish`), and the hasher tests, plus the authoritative `HASH_FIELD_ORDER` doc list. Task 13's only NEW work is adding `state_hash(world: &World)` and `write_store_cursor` (Steps 4–7); it does NOT redeclare the file or change the header values. The committed anchor is reproduced below for reference (this is the verbatim landed shape — read it, do not rewrite it):

  ```rust
  //! Per-tick STATE-hash specification + the shared FNV-1a hasher (spec §6).
  //! Landed early as the DRIFT-LOCK ANCHOR: the canonical field order
  //! (`HASH_FIELD_ORDER`) is authoritative here, so a later task that adds a
  //! hashed field (e.g. Task 11 adds prev_fuel/prev_inside_dest) MUST append to
  //! `HASH_FIELD_ORDER`, bump `HASH_FORMAT_VERSION`, and update the golden test —
  //! the change cannot silently alter the hash uncaught.
  //!
  //! ## HASH_FIELD_ORDER — canonical per-tick state-hash field order
  //!
  //! `state_hash` (Task 13) writes EXACTLY these u64 words in EXACTLY this order.
  //! Every f64 is encoded via `f64::to_bits()`; every word is folded
  //! little-endian by `FnvHasher::write_u64`. Numbering is stable; APPEND only.
  //!
  //!  1. HASH_MAGIC                              (Task 3, header)
  //!  2. HASH_FORMAT_VERSION as u64              (Task 3, header)
  //!  3. tick.0                                  (Task 3, time)
  //!  4. body_store.ids.cursor()                 (Task 4/13, slot-map high-water)
  //!  5. ship_store.ids.cursor()                 (Task 4/13, slot-map high-water)
  //!
  //! Bodies, sorted by BodyId (slot, generation):
  //!
  //!  6. body.slot as u64, body.generation as u64       (Task 13)
  //!  7. body.mass.to_bits()                     (Task 13)
  //!     (body POSITION is derived from tick via ephemeris, NOT stored, so it is
  //!     NOT hashed independently — it is a pure function of tick already hashed)
  //!
  //! Craft, sorted by CraftId (slot, generation):
  //!
  //!  8. craft.slot as u64, craft.generation as u64     (Task 13)
  //!  9. pos.x,pos.y,pos.z to_bits()             (Task 13)
  //! 10. vel.x,vel.y,vel.z to_bits()             (Task 13)
  //! 11. fuel_mass.to_bits()                     (Task 13)
  //! 12. nav discriminant as u64 (+ resolved dest/dv_remaining bits)  (Task 13)
  //!     Encoding (Step 5/6 expand this placeholder): Idle => disc 0;
  //!     Seeking => disc 1, then the NavDest discriminant (Position => disc 0
  //!     then pos.x/.y/.z bits; Entity => disc 1, then (kind, slot, generation)), then
  //!     dv_remaining bits. The NavDest discriminant is folded BEFORE its payload
  //!     so Position(x,0,0) != Entity(slot=x). burn_budget is NOT hashed here.
  //! 13. lod discriminant as u64                 (Task 13)
  //!
  //! APPEND BELOW THIS LINE (bump HASH_FORMAT_VERSION + golden test on change):
  //!
  //! 14. prev_fuel[i].to_bits()                  (Task 11, event edge-detect state)
  //! 15. prev_inside_dest[i] as u64             (Task 11, event edge-detect state)

  /// Header magic for the per-tick STATE hash (little-endian, spec §6).
  pub const HASH_MAGIC: u64 = 0x4a55_4d50_4741_5445; // "JUMPGATE"
  /// Bump whenever HASH_FIELD_ORDER changes (e.g. Task 11 appends fields).
  pub const HASH_FORMAT_VERSION: u32 = 1;

  /// Golden per-tick hash of the minimal zero-init slice under HASH_FIELD_ORDER
  /// words 1..=13. Pinned so any change to the canonical encoding is caught.
  pub const GOLDEN_ZERO_STATE_HASH: u64 = 0xf0dd_a1ba_f433_3735;

  /// Shared FNV-1a 64-bit hasher for the per-tick state hash. Folds each u64 as 8
  /// little-endian bytes. `new()` seeds with `HASH_MAGIC` then the version word.
  pub struct FnvHasher {
      state: u64,
  }

  const STATE_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const STATE_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

  impl FnvHasher {
      pub fn new() -> Self {
          // Header-folding: seed with the FNV offset basis, then immediately fold
          // HASH_MAGIC and HASH_FORMAT_VERSION (HASH_FIELD_ORDER words 1 and 2).
          // A fresh hasher is therefore NOT the bare offset basis.
          let mut h = FnvHasher { state: STATE_FNV_OFFSET };
          h.write_u64(HASH_MAGIC);                 // HASH_FIELD_ORDER word 1
          h.write_u64(HASH_FORMAT_VERSION as u64); // HASH_FIELD_ORDER word 2
          h
      }
      /// Folds one u64 as 8 little-endian bytes (HASH_FIELD_ORDER words).
      pub fn write_u64(&mut self, v: u64) {
          for b in v.to_le_bytes() {
              self.state ^= b as u64;
              self.state = self.state.wrapping_mul(STATE_FNV_PRIME);
          }
      }
      pub fn finish(self) -> u64 {
          self.state
      }
  }

  impl Default for FnvHasher {
      fn default() -> Self {
          Self::new()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn fresh_hasher_is_deterministic() {
          assert_eq!(FnvHasher::new().finish(), FnvHasher::new().finish());
      }

      #[test]
      fn write_order_matters() {
          let mut a = FnvHasher::new();
          a.write_u64(1);
          a.write_u64(2);
          let mut b = FnvHasher::new();
          b.write_u64(2);
          b.write_u64(1);
          assert_ne!(a.finish(), b.finish());
      }

      #[test]
      fn writing_changes_the_hash() {
          let base = FnvHasher::new().finish();
          let mut h = FnvHasher::new();
          h.write_u64(0);
          assert_ne!(base, h.finish(), "even writing 0 must move the hash");
      }

      /// GOLDEN HASH over the FnvHasher directly (words 1..=13 of a zero-init
      /// single-body single-craft slice). Pins the canonical encoding; this is
      /// NOT a `state_hash(world)` golden (it predates `World`).
      #[test]
      fn golden_zero_state_hash() {
          let mut h = FnvHasher::new();
          // header (words 1-2) already folded by new(); words 3..=13 follow.
          h.write_u64(0); // 3. tick
          h.write_u64(0); // 4. body cursor
          h.write_u64(0); // 5. ship cursor
          h.write_u64(0); // body slot
          h.write_u64(0); // body generation
          h.write_u64(0.0f64.to_bits()); // body mass
          h.write_u64(0); // craft slot
          h.write_u64(0); // craft generation
          h.write_u64(0.0f64.to_bits()); // pos.x
          h.write_u64(0.0f64.to_bits()); // pos.y
          h.write_u64(0.0f64.to_bits()); // pos.z
          h.write_u64(0.0f64.to_bits()); // vel.x
          h.write_u64(0.0f64.to_bits()); // vel.y
          h.write_u64(0.0f64.to_bits()); // vel.z
          h.write_u64(0.0f64.to_bits()); // fuel_mass
          h.write_u64(0); // nav discriminant (Idle)
          h.write_u64(0); // lod discriminant (Player)
          assert_eq!(h.finish(), GOLDEN_ZERO_STATE_HASH);
      }
  }
  ```

  No edit is required for this layer. Confirm the anchor still builds and its tests pass before adding `state_hash`:

  ```
  cargo test -p jumpgate-core hash:: 2>&1 | tail -8
  ```
  EXPECTED: `test result: ok. 4 passed; 0 failed` (`fresh_hasher_is_deterministic`, `write_order_matters`, `writing_changes_the_hash`, `golden_zero_state_hash`). These already pass — `write_u64` is implemented and the header values are committed. Task 13 adds NO failing test at this layer; the red→green work begins at Step 4 (`state_hash`).

- [ ] **Step 3: (No work — `FnvHasher::write_u64` already exists.)**

  The byte-wise little-endian FNV-1a `write_u64` is part of the committed Task-3 anchor shown in Step 2 (`for b in v.to_le_bytes() { state ^= b; state = state.wrapping_mul(STATE_FNV_PRIME); }`). There is nothing to implement here. Task 13's first real implementation step is Step 4's `state_hash`. (This step is retained as a numbered placeholder so the downstream step numbering and cross-references are unchanged.)

- [ ] **Step 4: Write the failing `state_hash` determinism + perturbation + header tests (no impl yet).**

  Add to the `tests` module in `crates/jumpgate-core/src/hash.rs`. These build worlds through the PUBLIC `World::reset` API only (no field-poking), so they depend only on Task 6's `StateView` read surface and Task 12's `reset`. Use a local minimal config helper:

  ```rust
      use crate::config::{
          BaseSpec, BodyInit, CraftInit, OrbitalElements, RunConfig, SubstepCfg,
      };
      use crate::math::Vec3;
      use crate::time::Dt;
      use crate::world::World;

      fn base_spec() -> BaseSpec {
          BaseSpec {
              base_dry_mass: 1.0,
              base_max_thrust: 0.1,
              base_exhaust_velocity: 0.05,
              base_fuel_capacity: 0.5,
          }
      }

      fn cfg_with_craft_x(craft_x: f64) -> RunConfig {
          RunConfig {
              master_seed: 42,
              dt: Dt::new(0.01),
              softening: 1e-4,
              substep_cfg: SubstepCfg { accel_ref: 1.0, max_substeps: 16 },
              ephemeris_window: 64,
              bodies: vec![BodyInit {
                  mass: 1.0,
                  elements: OrbitalElements {
                      a: 1.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0,
                  },
              }],
              craft: vec![CraftInit {
                  spec: base_spec(),
                  pos: Vec3::new(craft_x, 0.0, 0.0),
                  vel: Vec3::ZERO,
                  fuel_mass: 0.5,
              }],
          }
      }

      #[test]
      fn identical_worlds_hash_equal() {
          let (wa, _) = World::reset(cfg_with_craft_x(2.0));
          let (wb, _) = World::reset(cfg_with_craft_x(2.0));
          assert_eq!(state_hash(&wa), state_hash(&wb));
      }

      #[test]
      fn perturbing_one_f64_changes_hash() {
          // Two worlds identical except one craft x-coordinate differs slightly.
          let (wa, _) = World::reset(cfg_with_craft_x(2.0));
          let (wb, _) = World::reset(cfg_with_craft_x(2.0 + 1e-9));
          assert_ne!(state_hash(&wa), state_hash(&wb));
      }

      #[test]
      fn header_words_are_present_but_not_the_whole_hash() {
          // The first three words must be MAGIC, VERSION, tick. Recompute the
          // header-only hash independently; state_hash mixes MORE after it, so it
          // must NOT equal the header-only hash (proves header present AND body
          // follows). This pins HASH_FIELD_ORDER entries 1-3.
          let (w, _) = World::reset(cfg_with_craft_x(2.0));
          // new() already folds HASH_MAGIC + HASH_FORMAT_VERSION (words 1-2); add
          // only the tick word. state_hash mixes MORE (cursors/bodies/craft) after
          // this, so it must differ.
          let mut header_only = FnvHasher::new();
          header_only.write_u64(0u64); // word 3: tick 0 after reset
          assert_ne!(state_hash(&w), header_only.finish());
      }

      #[test]
      fn seeking_navdest_discriminant_is_folded_before_payload() {
          // Word 12 must fold the NavDest discriminant BEFORE its payload, so a craft
          // Seeking Position(Vec3::new(x,0,0)) MUST hash differently from one Seeking
          // Entity(Craft(CraftId{slot: x as u32, generation: 0})). Build two otherwise-identical
          // worlds, set the single craft's nav through the ingestion path, and assert
          // the two state hashes differ. (Pins word 12's encoding, which the Idle-only
          // golden zero-world test would otherwise leave unexercised.)
          use crate::contract::{Command, CommandKind, Target};
          let x: f64 = 7.0;

          let (mut wp, _) = World::reset(cfg_with_craft_x(2.0));
          let cp = wp.craft_ids()[0];
          let mut cmds_p = vec![Command {
              tick: wp.tick(),
              target: Target::Entity(EntityRef::Craft(cp)),
              kind: CommandKind::Destination {
                  dest: NavDest::Position(Vec3::new(x, 0.0, 0.0)),
                  burn_budget: None,
              },
          }];
          crate::ingest::ingest_commands(&mut wp, wp.tick(), &mut cmds_p);

          let (mut we, _) = World::reset(cfg_with_craft_x(2.0));
          let ce = we.craft_ids()[0];
          let mut cmds_e = vec![Command {
              tick: we.tick(),
              target: Target::Entity(EntityRef::Craft(ce)),
              kind: CommandKind::Destination {
                  dest: NavDest::Entity(EntityRef::Craft(CraftId { slot: x as u32, generation: 0 })),
                  burn_budget: None,
              },
          }];
          crate::ingest::ingest_commands(&mut we, we.tick(), &mut cmds_e);

          assert_ne!(
              state_hash(&wp),
              state_hash(&we),
              "NavDest discriminant must be folded before payload (Position(x) != Entity(slot=x))"
          );
      }
  ```

  > NOTE: this test resolves `CraftId` through the crate-root or `crate::contract` import already in scope; if `Command`/`CommandKind`/`Target`/`CraftId` are not yet imported in the test module, add the `use` lines shown. The exact ingestion entry point (`ingest::ingest_commands`) is the single nav-write path from Task 10/12 — do not poke `world.ships.nav` directly.

  Add the `state_hash` stub above the test module (so the file compiles). Keep a SINGLE `use crate::world::World;` at module scope:

  ```rust
  use crate::world::World;

  /// Canonical per-tick state hash. Mixes the words listed in HASH_FIELD_ORDER
  /// (module doc) in that exact order. Body positions derive from `tick`
  /// (already hashed) so only body identity + store cursor participate; ship
  /// dynamic state is hashed in full.
  pub fn state_hash(_world: &World) -> u64 {
      unimplemented!()
  }
  ```

  Run it and confirm the `state_hash` tests fail (panic):

  ```
  cargo test -p jumpgate-core hash:: 2>&1 | tail -14
  ```
  EXPECTED: `identical_worlds_hash_equal`, `perturbing_one_f64_changes_hash`, `header_words_are_present_but_not_the_whole_hash`, and `seeking_navdest_discriminant_is_folded_before_payload` FAIL with `not implemented` (all four call `state_hash`); the four committed hasher tests (`fresh_hasher_is_deterministic`, `write_order_matters`, `writing_changes_the_hash`, `golden_zero_state_hash`) still pass. Overall `test result: FAILED`.

- [ ] **Step 5: Implement `state_hash` WITHOUT the slot-map cursors (passes determinism + perturbation + header; leaves cursors for fail-first).**

  Replace the `state_hash` stub in `crates/jumpgate-core/src/hash.rs`. Read everything through the `StateView` trait (Task 6) so it depends only on the public read surface. Sort ids via their `Ord` impls for canonical order. NOTE: per HASH_FIELD_ORDER words 4 and 5, the `BodyStore`/`ShipStore` cursors belong in the stream, but they are deliberately NOT written here yet — Step 7 adds them after the failing cursor golden test:

  ```rust
  use crate::contract::StateView;
  use crate::math::Vec3;
  use crate::stores::NavState;
  use crate::types::{EntityRef, NavDest};

  fn write_vec3(h: &mut FnvHasher, v: Vec3) {
      let [bx, by, bz] = v.to_bits();
      h.write_u64(bx);
      h.write_u64(by);
      h.write_u64(bz);
  }

  pub fn state_hash(world: &World) -> u64 {
      // new() ALREADY folds HASH_FIELD_ORDER words 1-2 (HASH_MAGIC, HASH_FORMAT_VERSION).
      // Do NOT re-write them here, or the digest will not match the committed anchor.
      let mut h = FnvHasher::new();
      h.write_u64(world.tick().0); // word 3: tick

      // HASH_FIELD_ORDER words 4-5 (body-store then ship-store cursor): added in
      // Step 7, BOTH immediately after the tick word and BEFORE the body loop.

      // HASH_FIELD_ORDER words 6-7: bodies, sorted by id — slot, generation, mass.
      // (Body POSITIONS are tick-derived via ephemeris, so they are NOT hashed.)
      let mut bodies = world.body_ids();
      bodies.sort();
      for b in bodies {
          h.write_u64(b.slot as u64);
          h.write_u64(b.generation as u64);
          let bi = world.bodies.ids.dense_index(b.slot, b.generation).unwrap();
          h.write_u64(world.bodies.mass[bi].to_bits()); // word 7: body mass
      }

      // HASH_FIELD_ORDER words 8-13: craft, sorted id, then full dynamic state.
      let mut craft = world.craft_ids();
      craft.sort();
      for c in craft {
          h.write_u64(c.slot as u64);
          h.write_u64(c.generation as u64);
          if let Some(p) = world.craft_pos(c) {
              write_vec3(&mut h, p);
          }
          if let Some(v) = world.craft_vel(c) {
              write_vec3(&mut h, v);
          }
          if let Some(f) = world.craft_fuel(c) {
              h.write_u64(f.to_bits());
          }
          // HASH_FIELD_ORDER word 12: NavState (discriminant-first, self-delimiting).
          // Read the dense row via the public SlotMap accessor (ship_index is private
          // to world.rs). The NavDest discriminant is folded BEFORE its payload so
          // Position(x,0,0) and Entity(slot=x) cannot collide.
          let idx = world.ships.ids.dense_index(c.slot, c.generation).unwrap();
          match world.ships.nav[idx] {
              NavState::Idle => h.write_u64(0),
              NavState::Seeking { dest, dv_remaining } => {
                  h.write_u64(1);
                  match dest {
                      NavDest::Position(p) => {
                          h.write_u64(0);
                          let [dx, dy, dz] = p.to_bits();
                          h.write_u64(dx);
                          h.write_u64(dy);
                          h.write_u64(dz);
                      }
                      NavDest::Entity(EntityRef::Craft(id)) => {
                          h.write_u64(1);
                          h.write_u64(0); // kind: craft
                          h.write_u64(id.slot as u64);
                          h.write_u64(id.generation as u64);
                      }
                      NavDest::Entity(EntityRef::Body(id)) => {
                          h.write_u64(1);
                          h.write_u64(1); // kind: body
                          h.write_u64(id.slot as u64);
                          h.write_u64(id.generation as u64);
                      }
                  }
                  h.write_u64(dv_remaining.to_bits());
              }
          }
          // HASH_FIELD_ORDER word 13: Lod discriminant (lod() is on StateView).
          if let Some(l) = world.lod(c) {
              h.write_u64(l as u64);
          }
      }

      h.finish()
  }
  ```

  Run it and confirm all eight tests pass:

  ```
  cargo test -p jumpgate-core hash:: 2>&1 | tail -10
  ```
  EXPECTED: `test result: ok. 8 passed; 0 failed` (4 committed hasher tests + `identical_worlds_hash_equal` + `perturbing_one_f64_changes_hash` + `header_words_are_present_but_not_the_whole_hash` + `seeking_navdest_discriminant_is_folded_before_payload`).

- [ ] **Step 6: Write the FAILING cursor-participation golden test for a zero-init world.**

  This is the systemic drift-lock: a hardcoded golden hash of a fully-known zero-init world that INCLUDES the cursor contribution. Step 5's impl omits the cursors, so the golden value below (computed by an independent recomputation that DOES write the cursors at positions 4 and 5) will NOT match Step 5's output — that is the fail-first signal. The independent recomputation also doubles as the executable spec for HASH_FIELD_ORDER.

  Add to the `tests` module:

  ```rust
      use crate::contract::StateView;
      use crate::ids::SlotMap;

      /// Independent recomputation of HASH_FIELD_ORDER, WITH the two store
      /// cursors written (words 4 and 5). This is the executable spec; if a
      /// field is added to `state_hash` without updating this helper, the golden
      /// test below diverges and forces the author to bump HASH_FORMAT_VERSION.
      fn recompute_with_cursors(w: &World) -> u64 {
          // Mirrors the committed HASH_FIELD_ORDER exactly: new() folds words 1-2,
          // then tick(3), body cursor(4), ship cursor(5), per-body slot/generation/mass(6-7),
          // per-craft(8-13). Do NOT re-write the header — new() already did.
          let mut h = FnvHasher::new();
          h.write_u64(w.tick().0); // word 3
          write_store_cursor(&mut h, &w.bodies.ids); // word 4
          write_store_cursor(&mut h, &w.ships.ids);  // word 5
          let mut bodies = w.body_ids();
          bodies.sort();
          for b in bodies {
              h.write_u64(b.slot as u64);
              h.write_u64(b.generation as u64);
              let bi = w.bodies.ids.dense_index(b.slot, b.generation).unwrap();
              h.write_u64(w.bodies.mass[bi].to_bits()); // word 7: body mass
          }
          let mut craft = w.craft_ids();
          craft.sort();
          for c in craft {
              h.write_u64(c.slot as u64);
              h.write_u64(c.generation as u64);
              let p = w.craft_pos(c).unwrap();
              let [px, py, pz] = p.to_bits();
              h.write_u64(px);
              h.write_u64(py);
              h.write_u64(pz);
              let v = w.craft_vel(c).unwrap();
              let [vx, vy, vz] = v.to_bits();
              h.write_u64(vx);
              h.write_u64(vy);
              h.write_u64(vz);
              h.write_u64(w.craft_fuel(c).unwrap().to_bits());
              // HASH_FIELD_ORDER word 12: NavState (discriminant-first, self-delimiting).
              // Map the sorted CraftId back to its dense row; ship_index is private to
              // world.rs, so resolve the row via the public SlotMap accessor.
              let idx = w.ships.ids.dense_index(c.slot, c.generation).unwrap();
              match w.ships.nav[idx] {
                  NavState::Idle => h.write_u64(0),
                  NavState::Seeking { dest, dv_remaining } => {
                      h.write_u64(1);
                      match dest {
                          NavDest::Position(p) => {
                              h.write_u64(0);
                              let [dx, dy, dz] = p.to_bits();
                              h.write_u64(dx);
                              h.write_u64(dy);
                              h.write_u64(dz);
                          }
                          NavDest::Entity(EntityRef::Craft(id)) => {
                              h.write_u64(1);
                              h.write_u64(0); // kind: craft
                              h.write_u64(id.slot as u64);
                              h.write_u64(id.generation as u64);
                          }
                          NavDest::Entity(EntityRef::Body(id)) => {
                              h.write_u64(1);
                              h.write_u64(1); // kind: body
                              h.write_u64(id.slot as u64);
                              h.write_u64(id.generation as u64);
                          }
                      }
                      h.write_u64(dv_remaining.to_bits());
                  }
              }
              // HASH_FIELD_ORDER word 13: Lod discriminant (lod() is on StateView).
              h.write_u64(w.lod(c).unwrap() as u64);
          }
          h.finish()
      }

      #[test]
      fn cursor_participates_in_state_hash() {
          // state_hash MUST include both store cursors (HASH_FIELD_ORDER 4, 5).
          // The independent recompute writes them; until Step 7 wires the cursors
          // into state_hash, the two digests diverge. Step 7 makes them equal.
          let (w, _) = World::reset(cfg_with_craft_x(2.0));
          assert_eq!(
              state_hash(&w),
              recompute_with_cursors(&w),
              "state_hash must mix both store cursors per HASH_FIELD_ORDER 4 and 5"
          );
      }

      #[test]
      fn write_store_cursor_is_cursor_sensitive() {
          // Self-contained unit guard on the helper itself: two SlotMaps whose
          // cursors differ must hash differently. Uses only contract methods
          // (new, cursor). A fresh map has cursor 0; we assert the helper writes
          // whatever cursor() returns by feeding two maps with distinct cursors.
          // Since v1 has no public mutator that advances cursor in this unit
          // scope, we assert the weaker-but-sufficient property that the helper
          // mixes the cursor word at all: an empty map's helper-hash differs from
          // a bare FnvHasher (i.e. a cursor word WAS written).
          let empty: SlotMap<()> = SlotMap::new();
          let mut with = FnvHasher::new();
          write_store_cursor(&mut with, &empty);
          assert_ne!(
              with.finish(),
              FnvHasher::new().finish(),
              "write_store_cursor must mix a cursor word into the hasher"
          );
      }
  ```

  This references `write_store_cursor` and `w.bodies.ids` / `w.ships.ids`, which do not exist yet, so the test module fails to COMPILE. That is the fail-first signal for the cursor.

  Run it and confirm the compile failure:

  ```
  cargo test -p jumpgate-core hash:: 2>&1 | tail -8
  ```
  EXPECTED: ``error[E0425]: cannot find function `write_store_cursor` in this scope`` (and/or ``error[E0609]: no field `bodies`/`ships` ``). Build FAILS.

  > Cross-task contract-surface note: `write_store_cursor` is PROVIDED by this task (Step 7). The store field paths `w.bodies.ids` / `w.ships.ids` are PROVIDED by Task 12 (`World { bodies: BodyStore, ships: ShipStore, .. }` with crate-visible fields; `BodyStore`/`ShipStore { ids: SlotMap<()>, .. }` per the `stores.rs` contract). If Task 12 named the world fields differently, the contract-surface doc is wrong and Task 12 must be fixed — do NOT silently rename here. The only requirement is that both store cursors are reachable as `&SlotMap<()>` and mixed.

- [ ] **Step 7: Add the cursor helper + wire both store cursors into `state_hash` (make Step 6 pass).**

  Add the `write_store_cursor` helper (PROVIDED symbol — generic over any `&SlotMap<T>`, so it serves both `BodyStore.ids` and `ShipStore.ids`) and wire the two cursors into `state_hash` at HASH_FIELD_ORDER positions 4 and 5. Add the import for `SlotMap` at module scope:

  ```rust
  use crate::ids::SlotMap;

  /// Mix a store's allocator cursor (high-water) into the hash. Present per §6 /
  /// HASH_FIELD_ORDER so a future mid-run Spawn does not invalidate prior-tick
  /// hashes. Generic so both BodyStore.ids and ShipStore.ids reuse it.
  pub fn write_store_cursor<T>(h: &mut FnvHasher, store: &SlotMap<T>) {
      h.write_u64(store.cursor());
  }
  ```

  Then edit `state_hash`: insert BOTH store-cursor writes immediately after the tick word and BEFORE the body loop — body-store cursor (HASH_FIELD_ORDER word 4) then ship-store cursor (word 5), matching the committed anchor order:

  ```rust
      h.write_u64(world.tick().0); // word 3

      // HASH_FIELD_ORDER words 4-5: both store allocator cursors (hashed state, §6),
      // body-store then ship-store, BEFORE any body/craft data.
      write_store_cursor(&mut h, &world.bodies.ids);
      write_store_cursor(&mut h, &world.ships.ids);

      // HASH_FIELD_ORDER words 6-7: bodies, sorted by id — slot, generation, mass.
      let mut bodies = world.body_ids();
      bodies.sort();
      for b in bodies {
          h.write_u64(b.slot as u64);
          h.write_u64(b.generation as u64);
          let bi = world.bodies.ids.dense_index(b.slot, b.generation).unwrap();
          h.write_u64(world.bodies.mass[bi].to_bits());
      }
  ```

  Run it and confirm every test passes:

  ```
  cargo test -p jumpgate-core hash:: 2>&1 | tail -12
  ```
  EXPECTED: `test result: ok. 10 passed; 0 failed` (4 committed hasher tests + 3 state_hash determinism/header + `seeking_navdest_discriminant_is_folded_before_payload` + `cursor_participates_in_state_hash` + `write_store_cursor_is_cursor_sensitive`).

- [ ] **Step 8: Add the hardcoded golden state-hash regression test for a zero-init world.**

  The Step 6 `recompute_with_cursors` proves `state_hash` MATCHES its own field list, but both could drift together. Pin the ACTUAL numeric digest of the canonical zero-init world so any reordering, added field, or version bump is caught by a hardcoded value (the systemic drift-lock the fix requires). First print the real value, then paste it in.

  Print the current digest:

  ```
  cargo test -p jumpgate-core hash::tests::print_golden -- --ignored --nocapture 2>&1 | grep GOLDEN
  ```

  To make that work, first add this helper test to the `tests` module:

  ```rust
      #[test]
      #[ignore = "prints the golden constant for state_hash_golden_zero_world"]
      fn print_golden() {
          let (w, _) = World::reset(cfg_with_craft_x(2.0));
          println!("GOLDEN=0x{:016x}", state_hash(&w));
      }
  ```

  Run the print command above, read the `GOLDEN=0x...` value, then add the regression test, substituting the printed value for `<PASTE>`:

  ```rust
      #[test]
      fn state_hash_golden_zero_world() {
          // Hardcoded digest of the canonical zero-init world (cfg_with_craft_x(2.0),
          // tick 0). Pins HASH_FIELD_ORDER + HASH_FORMAT_VERSION. If this changes,
          // a field was added/reordered or the version bumped: update HASH_FIELD_ORDER
          // (module doc), bump HASH_FORMAT_VERSION, and re-paste from `print_golden`.
          let (w, _) = World::reset(cfg_with_craft_x(2.0));
          assert_eq!(state_hash(&w), 0x<PASTE>u64);
      }
  ```

  Run it and confirm it passes with the pasted constant:

  ```
  cargo test -p jumpgate-core hash::tests::state_hash_golden_zero_world 2>&1 | tail -6
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`. (If it fails, the pasted constant is wrong — re-run `print_golden`.)

- [ ] **Step 9: Confirm no banned methods, clippy clean, and full crate still builds.**

  Run clippy across all targets (per the project note, `--lib` is a no-op here — use `--all-targets` to lint the test module) and a full crate test pass:

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings 2>&1 | tail -5
  cargo test -p jumpgate-core 2>&1 | tail -6
  ```
  EXPECTED: clippy prints `Finished` with no warnings (no `disallowed-methods` hits — `hash.rs` uses only `to_le_bytes`/`wrapping_mul`/`to_bits`/`sort`, none banned; no `SystemTime`/`Instant`/`thread_rng`); the full `jumpgate-core` suite reports `test result: ok` with all hash tests (11 active — 4 committed hasher tests, 3 state_hash determinism/header, `seeking_navdest_discriminant_is_folded_before_payload`, `cursor_participates_in_state_hash`, `write_store_cursor_is_cursor_sensitive`, plus `state_hash_golden_zero_world` from Step 8 — and 1 ignored `print_golden`) accounted for.

- [ ] **Step 10: Commit.**

  ```
  git add crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  feat(core): FNV-1a per-tick state hash + authoritative HASH_FIELD_ORDER

  Canonical state hash over f64::to_bits in a fixed, numbered field-then-id
  order: header (MAGIC + FORMAT_VERSION), tick, body-store cursor, sorted
  body ids, ship-store cursor, sorted craft id + pos/vel/fuel. Byte-wise
  little-endian FNV-1a; no serde/json in the hashed path. Both store cursors
  are hashed so a future mid-run Spawn cannot invalidate prior-tick hashes.

  HASH_FIELD_ORDER is documented as the authoritative numbered drift-lock in
  the module doc; a hardcoded golden state-hash of a zero-init world plus an
  independent field-order recomputation catch any silent change to field
  coverage or order and force a HASH_FORMAT_VERSION bump (the seam the Task 11
  prev_fuel/prev_inside_dest addition will edit).

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: a single commit on the current branch containing the two files; `git status` clean afterward.

---

### Task 14: Replay equivalence (record + log re-feed + first-diff report)

This is **the primary correctness test** (spec §8): record a run's action log + per-tick state hashes, then replay by **re-feeding the recorded log** (never the policy/driver), recomputing the hash each tick and asserting equality, reporting the **first differing tick** on mismatch. It exercises the whole determinism floor (spec §6) end to end.

The Task-14 corruption test is only meaningful if the recorded run actually *does something* that a corrupted command can perturb. Therefore the driver must address a **real craft** with a **real thrust-producing destination** — `Target::Entity(EntityRef::Craft(id))` — so the full `ingest -> nav -> thrust -> fuel -> hash` chain runs. A `Target::Sim` command is a no-op in `ingest_commands` (no craft is selected, no `NavState` is set), which would leave the craft coasting on pure gravity and make `corrupting_one_logged_command` produce an identical hash. We route to the craft and assert thrust fires.

**Files**
- Create: `crates/jumpgate-core/src/replay.rs`
- Create: `crates/jumpgate-core/tests/replay_equivalence.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/tests/replay_equivalence.rs`

**Depends on:** Task 13 (`hash.rs` → `state_hash(world: &World) -> u64`, `HASH_MAGIC`, `HASH_FORMAT_VERSION`). Also consumes, from earlier tasks, only these contract symbols:
- `World::reset(cfg) -> (World, ConfigHash)`, `World::step(&mut self, cmds: &mut Vec<Command>)`, `World as StateView` (for `craft_ids()` — needed to discover the deterministic craft id).
- `RunConfig`, `RunConfig::config_hash() -> ConfigHash`, `ConfigHash`.
- `Tick(pub u64)`, `Dt::new`.
- `ActionLog { pub entries: Vec<(Tick, Command)>, pub config_hash: ConfigHash, .. }` (single canonical shape from Task 10; also carries a `commands_flat` parallel vec so `at`/`since_commands` return `&[Command]`) with `record(&mut self, Tick, Command)` and `at(&self, Tick) -> &[Command]`.
- `Command`, `CommandKind::Destination`, `NavDest::Position`, `Target::Entity`, `EntityRef::Craft`, `Target::Sim`, `CraftId`, `Vec3`.
- `BaseSpec`, `BodyInit`, `OrbitalElements`, `CraftInit`, `SubstepCfg`.
- `EventKind::ThrustApplied` (asserted in a precondition test that the recorded run truly thrusts).

**CROSS-TASK CONTRACT SURFACE consumed here** (see the Task-3 contract-surface document; this task is a *consumer* of every symbol below and adds nothing other tasks consume except the two `Recording` fields + three `fn`s in the verbatim block):
- The single craft minted by `World::reset` in v1 is deterministically `CraftId { slot: 0, generation: 0 }` (slot-map allocates slot 0 first; a fresh slot starts at `generation == 0`). Task 14 does **not** hardcode this — it discovers the id via `World::reset(...).0.craft_ids()[0]` and *asserts* the stable value, so a divergence in Task 4's generation convention fails loudly here rather than silently mis-routing commands.
- `ingest_commands` treats `Target::Sim` as a no-op in v1 (no `CommandKind` variant is Sim-scoped); only `Target::Entity(EntityRef::Craft(_))` sets a `NavState::Seeking`. This is the property the corruption test relies on being *false* for craft-targeted commands.

**Contract types introduced here (verbatim):**
```rust
pub struct Recording {
    pub config: RunConfig,
    pub log: ActionLog,
    pub hashes: Vec<(Tick, u64)>,
    /// config_hash() captured AT RECORD TIME, before any post-hoc mutation of
    /// `config`. The replay guard compares THIS (stored) against a fresh
    /// `config.config_hash()` so the check is not tautological. (Fix: the stored
    /// hash, mirroring the hash ActionLog stamps at record time per Task 10.)
    pub config_hash: ConfigHash,
}
pub fn record_run(cfg: RunConfig, ticks: u64, driver: impl FnMut(Tick) -> Vec<Command>) -> Recording;
pub fn replay_run(rec: &Recording) -> Result<(), Tick>;
```

**Design decisions (pinned, so the implementer does not re-litigate):**
1. `record_run` clones the driver's per-tick `Vec<Command>` into `rec.log` **before** calling `World::step` (which sorts/mutates the vec in place via `ingest_commands`). Replay re-feeds the logged clone. Because `ingest_commands` applies in canonical `command_sort_key` order, re-feeding the same multiset reproduces identical state regardless of log order — but we log and re-feed faithfully anyway.
2. The hash recorded for an iteration is taken **after** `step`. Concretely, for each of `ticks` iterations we (a) read `t = world.tick()` (pre-step tick), (b) log the driver's commands at `t`, (c) `step`, (d) push `(world.tick(), state_hash(&world))`. So `rec.hashes[i].0` is the **post-step** tick and the hash is of the resulting state. Replay reproduces the identical sequence by the identical procedure, so the comparison is apples-to-apples.
3. `replay_run` rebuilds the world with `World::reset(rec.config.clone())`, **rejects a stored-vs-fresh config-hash mismatch first**, then steps `rec.hashes.len()` times re-feeding `rec.log.at(pre_step_tick)`, comparing each recomputed `(tick, hash)` against the recorded pair. First mismatch → `Err(that_tick)`. A config-hash mismatch → `Err(Tick(0))` (no tick reproduced). The guard compares `rec.config_hash` (**stored at record time**) against `rec.config.config_hash()` (**freshly computed from the config now in the recording**); swapping `rec.config` after recording makes these disagree and the branch fires — it is *not* dead.
4. `record_run` always records exactly `ticks` hash entries. v1 has no early termination inside record/replay.

---

- [ ] **Step 1: Add the failing test file (precondition + round-trip + corruption + config-mismatch).**
  Create `crates/jumpgate-core/tests/replay_equivalence.rs`. It references only contract symbols. The craft id is **discovered** from a throwaway `World::reset` and asserted stable, so the driver routes to a real craft. Write it complete now; it will fail to compile until `replay.rs` exists.
```rust
use jumpgate_core::{
    record_run, replay_run, BaseSpec, BodyInit, Command, CommandKind, CraftId, CraftInit, Dt,
    EntityRef, EventKind, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg, Target, Tick,
    Vec3, World,
};

/// A 2-body, 1-craft scenario big enough to exercise gravity + a thrust burn.
fn base_config() -> RunConfig {
    RunConfig {
        master_seed: 0x9E37_79B9_7F4A_7C15_u64, // arbitrary fixed seed (golden-ratio bits)
        dt: Dt::new(0.5),
        softening: 1e-4,
        substep_cfg: SubstepCfg { accel_ref: 1.0, max_substeps: 16 },
        ephemeris_window: 4096,
        bodies: vec![
            BodyInit {
                mass: 1.0,
                elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            },
            BodyInit {
                mass: 3.0e-6,
                elements: OrbitalElements { a: 1.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            },
        ],
        craft: vec![CraftInit {
            spec: BaseSpec {
                base_dry_mass: 1.0e-9,
                base_max_thrust: 1.0e-6,
                base_exhaust_velocity: 0.02,
                base_fuel_capacity: 1.0e-9,
            },
            pos: Vec3::new(1.2, 0.0, 0.0),
            vel: Vec3::new(0.0, 0.9, 0.0),
            fuel_mass: 5.0e-10,
        }],
    }
}

/// The single v1 craft is deterministically `CraftId { slot: 0, generation: 0 }`.
/// Discover it from a fresh reset rather than hardcoding, and assert the stable
/// value so a slot-map generation-convention drift (Task 4) fails HERE, loudly.
fn discover_craft_id() -> CraftId {
    let (world, _hash) = World::reset(base_config());
    let ids = world.craft_ids();
    assert_eq!(ids.len(), 1, "v1 scenario has exactly one craft");
    assert_eq!(
        ids[0],
        CraftId { slot: 0, generation: 0 },
        "first-minted craft must be slot 0 / generation 0 (slot-map convention from Task 4: a fresh slot starts at generation 0)"
    );
    ids[0]
}

/// Driver factory: command a destination on tick 0 ADDRESSED TO THE REAL CRAFT
/// (autopilot flies it, burning fuel), then issue no further commands.
/// Deterministic, no RNG, no clock. Routing to `Target::Entity(Craft(id))` (NOT
/// `Target::Sim`, which `ingest_commands` no-ops) is what makes the corruption
/// test causally meaningful.
fn transfer_driver(craft: CraftId) -> impl FnMut(Tick) -> Vec<Command> {
    move |tick: Tick| {
        if tick == Tick(0) {
            vec![Command {
                target: Target::Entity(EntityRef::Craft(craft)),
                kind: CommandKind::Destination {
                    dest: NavDest::Position(Vec3::new(-1.2, 0.0, 0.0)),
                    burn_budget: Some(0.01),
                },
            }]
        } else {
            Vec::new()
        }
    }
}

/// PRECONDITION: the recorded run must actually thrust. If it coasted (e.g. the
/// command was mis-routed to `Target::Sim`), the corruption test below would be
/// vacuous. We assert at least one `ThrustApplied` event by re-running the same
/// driver against a world we can read events from.
#[test]
fn recorded_run_actually_thrusts() {
    let craft = discover_craft_id();
    let mut driver = transfer_driver(craft);
    let (mut world, _hash) = World::reset(base_config());
    let mut saw_thrust = false;
    for _ in 0..50 {
        let pre = world.tick();
        let mut cmds = driver(pre);
        world.step(&mut cmds);
        if world
            .recent_events(pre)
            .iter()
            .any(|e| matches!(e.kind, EventKind::ThrustApplied { dv } if dv > 0.0 && {
                // craft binding is implicit (single craft); dv>0 proves a burn
                let _ = e;
                true
            }))
        {
            saw_thrust = true;
        }
    }
    assert!(
        saw_thrust,
        "craft-targeted destination must produce a ThrustApplied event; \
         a Target::Sim no-op would make the corruption test vacuous"
    );
}

#[test]
fn record_then_replay_is_bit_identical() {
    let craft = discover_craft_id();
    let rec = record_run(base_config(), 200, transfer_driver(craft));
    assert_eq!(rec.hashes.len(), 200, "one hash per stepped tick");
    assert_eq!(replay_run(&rec), Ok(()), "faithful re-feed must reproduce every tick hash");
}

#[test]
fn corrupting_one_logged_command_reports_first_differing_tick() {
    let craft = discover_craft_id();
    let mut rec = record_run(base_config(), 200, transfer_driver(craft));
    // Find the logged tick-0 craft-targeted destination command and corrupt its
    // destination. Because the command sets a NavState that drives thrust on the
    // very next step, the post-step-tick-1 hash diverges.
    let idx = rec
        .log
        .entries
        .iter()
        .position(|(t, c)| {
            *t == Tick(0)
                && matches!(c.kind, CommandKind::Destination { .. })
                && matches!(c.target, Target::Entity(EntityRef::Craft(_)))
        })
        .expect("driver logged a tick-0 craft-targeted destination command");
    rec.log.entries[idx].1 = Command {
        target: Target::Entity(EntityRef::Craft(craft)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(Vec3::new(99.0, 99.0, 99.0)), // different destination
            burn_budget: Some(0.01),
        },
    };
    // Re-feeding the corrupted log thrusts toward a different point; the recorded
    // hashes are the originals. First divergence = the first post-step tick.
    assert_eq!(replay_run(&rec), Err(Tick(1)));
}

#[test]
fn config_hash_mismatch_is_rejected() {
    // Swap `rec.config` for a DIFFERENT config AFTER recording, WITHOUT updating
    // the stored `rec.config_hash`. replay_run compares the stored hash (taken at
    // record time) against a fresh hash of the now-swapped config; they disagree,
    // so the guard fires and returns Err(Tick(0)) BEFORE any tick is reproduced.
    // This is the non-tautological guard: it proves a recording's hashes are bound
    // to the exact config they were generated under.
    let craft = discover_craft_id();
    let mut rec = record_run(base_config(), 50, transfer_driver(craft));
    let differing = RunConfig {
        master_seed: rec.config.master_seed ^ 0xABCD,
        softening: rec.config.softening * 2.0, // also perturb the gravity kernel
        ..rec.config.clone()
    };
    rec.config = differing; // config_hash field intentionally left stale
    assert_eq!(
        replay_run(&rec),
        Err(Tick(0)),
        "stored config-hash must reject a recording whose config was swapped"
    );
}
```

- [ ] **Step 2: Run the test to confirm it fails to compile (no `replay` symbols yet).**
```
cargo test -p jumpgate-core --test replay_equivalence -- --nocolor 2>&1 | head -40
```
  EXPECTED: a compile error, e.g. `error[E0432]: unresolved import` for `record_run` / `replay_run` (and possibly `Recording` if used). The seed literal `0x9E37_79B9_7F4A_7C15_u64` is a valid Rust integer literal, so there must be **no** lexer/tokenizer error. The test binary must NOT build yet.

- [ ] **Step 3: Implement `replay.rs`.**
  Create `crates/jumpgate-core/src/replay.rs`. `record_run` stamps `config_hash` at record time; `replay_run` compares stored-vs-fresh first, then re-feeds the log. Uses the chosen `rand_chacha = "=0.10.0"` family transitively via `World` only — no RNG idioms appear in this file.
```rust
//! Replay equivalence — the primary correctness surface (spec §8).
//!
//! `record_run` steps a fresh `World`, logging each tick's driver-produced
//! commands and the post-step `state_hash`, and stamps the config hash AT RECORD
//! TIME. `replay_run` rebuilds the world from the recorded config, rejects a
//! stored-vs-fresh config-hash mismatch, re-feeds the logged commands
//! tick-by-tick, and asserts per-tick hash equality, returning the first
//! differing tick on mismatch. Replay NEVER calls a driver/policy (spec §6).

use crate::config::{ConfigHash, RunConfig};
use crate::contract::Command;
use crate::hash::state_hash;
use crate::ingest::ActionLog;
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
    let (mut world, reset_hash) = World::reset(cfg.clone());
    debug_assert_eq!(
        reset_hash, config_hash,
        "World::reset must return the config's own hash"
    );

    let mut log = ActionLog { entries: Vec::new(), commands_flat: Vec::new(), config_hash };
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

    Recording { config: cfg, log, hashes, config_hash }
}

/// Rebuild from `rec.config`, reject a config-hash mismatch, then re-feed
/// `rec.log` tick-by-tick recomputing `state_hash`. Returns `Ok(())` if every
/// recorded hash matches, else `Err(first_differing_tick)`.
///
/// The config-hash guard compares the STORED `rec.config_hash` (captured at
/// record time) against a FRESH `rec.config.config_hash()`. These disagree iff
/// `rec.config` was swapped after recording, so the `Err(Tick(0))` branch is
/// reachable and meaningful — not tautological.
///
/// NEVER calls a driver/policy — it only re-feeds the recorded log.
pub fn replay_run(rec: &Recording) -> Result<(), Tick> {
    let fresh_hash: ConfigHash = rec.config.config_hash();
    if rec.config_hash != fresh_hash {
        // The hashes in this recording were generated under a config whose hash
        // was `rec.config_hash`; the config now present hashes differently. No
        // tick was reproduced.
        return Err(Tick(0));
    }

    let (mut world, reset_hash) = World::reset(rec.config.clone());
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
```

- [ ] **Step 4: Wire `replay` into the crate root.**
  In `crates/jumpgate-core/src/lib.rs`, add the module declaration alongside the other modules (preserving the canonical acyclic order math → time → ids → types → config → hash → rng → contract → stores → … → world → replay; `ids` BEFORE `types`, `hash` anchor early) and re-export the contract symbols. Add:
```rust
pub mod replay;
pub use replay::{record_run, replay_run, Recording};
```
  Do **not** re-export `Integrator` here or anywhere outside `contract` (it is defined exactly once in `contract.rs`; `integrator.rs` writes only impls via `use crate::contract::Integrator`).

- [ ] **Step 5: Run the precondition + round-trip tests.**
```
cargo test -p jumpgate-core --test replay_equivalence recorded_run_actually_thrusts -- --nocolor
cargo test -p jumpgate-core --test replay_equivalence record_then_replay_is_bit_identical -- --nocolor
```
  EXPECTED for each: `test result: ok. 1 passed; 0 failed`.
  If `recorded_run_actually_thrusts` fails with `saw_thrust == false`, the command is being mis-routed (a `Target::Sim` no-op or a wrong `CraftId`) — fix the routing/id before proceeding; a coasting craft makes the corruption test vacuous.

- [ ] **Step 6: Run the corruption + config-mismatch tests.**
```
cargo test -p jumpgate-core --test replay_equivalence corrupting_one_logged_command_reports_first_differing_tick -- --nocolor
cargo test -p jumpgate-core --test replay_equivalence config_hash_mismatch_is_rejected -- --nocolor
```
  EXPECTED for each: `test result: ok. 1 passed; 0 failed`.
  `corrupting_one_logged_command...` proves the full `ingest -> nav -> thrust -> fuel -> hash` chain diverges (`Err(Tick(1))`) when the tick-0 craft command is changed. `config_hash_mismatch_is_rejected` proves the stored-vs-fresh guard returns `Err(Tick(0))` — confirming the guard is reachable, not dead.

- [ ] **Step 7: Run the whole core test suite + clippy to confirm no regression.**
```
cargo test -p jumpgate-core -- --nocolor
cargo clippy -p jumpgate-core --all-targets -- -D warnings
```
  EXPECTED: `cargo test` ends `test result: ok.` for every binary (lib, `replay_equivalence` with **4 passed**, and the other tasks' suites). EXPECTED: clippy prints `Finished` with no `disallowed-methods` hits (replay/record use no `SystemTime`/`Instant::now`/`thread_rng`; the only RNG reaches them transitively through `World`, which already uses the pinned `rand_chacha = "=0.10.0"` family).

- [ ] **Step 8: Commit.**
```
git checkout -b task-14-replay-equivalence
git add crates/jumpgate-core/src/replay.rs crates/jumpgate-core/src/lib.rs crates/jumpgate-core/tests/replay_equivalence.rs
git commit -m "Task 14: replay equivalence — craft-targeted record, log re-feed, stored config-hash guard, first-differing-tick report

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
  EXPECTED: one commit on branch `task-14-replay-equivalence` with three files changed (2 created, 1 modified).

---

### Task 15: Physics sanity + autopilot transfer tests

Integration tests over the fully-assembled `World` (landed in Task 14). Four behaviours: (1) a near-circular orbit stays bounded over many orbits; (2) an eccentric close-approach trajectory does **not** blow up (substepping + softening keep it finite and bounded); (3) pure-coast specific-orbital-energy drift is bounded (sanity, not golden); (4) a fuel-budgeted autopilot transfer emits an `EventKind::Arrival` within its tick budget and does so **deterministically** (same `RunConfig` → same arrival tick). This task also resolves the §11 tuning items (`dt`, substep schedule, softening ε) **by measurement**, encoding the chosen v1 defaults in a single in-test config builder and documenting them so a follow-up can promote them into `config.rs`. Per the task's file scope, this task writes **only** the test file — it does not modify `config.rs`.

**Cross-task contract dependencies (read the contract-surface doc first):** this task is a pure downstream consumer. It reads the engine exclusively through the crate-root re-exports of `World`, `World`'s `StateView` impl, and the contract/seam/config types listed in the import block below — no internal/private types are touched. Three upstream facts this task's config and assertions depend on (all must already be landed and tested by their providing task before Task 15 can go green):

- **Imports come through the lib.rs re-export surface, NOT sub-module paths.** `Dt` and `Tick` live in `time.rs` (re-exported as `jumpgate_core::Dt` / `jumpgate_core::Tick`), **not** in `config.rs`. `World` is `jumpgate_core::World` (re-export), **not** `jumpgate_core::world::World`. The seam primitives `Lod`/`NavDest`/`Target`/`EntityRef`/`CommandKind` live in `types.rs` (Task 3) and `Command`/`Event`/`EventKind`/`StateView` in `contract.rs` (Task 6); both are re-exported at the crate root. This test imports **everything** from the crate root so the internal module layout (`types.rs` vs `contract.rs`, `time.rs` vs `config.rs`) cannot drift the test out of compile.
- **The substep formula (Task 8) is the canonical acceleration-keyed LOG2 bin** `N = clamp(1 + floor(log2(max(1.0, total_accel_mag / accel_ref))), 1, max_substeps)` over the **quantized total local acceleration magnitude (gravity + thrust)** (this is the form plan-2 Task 8 DEFINES, with tests enforcing log2). The eccentric test below depends on this formula *actually engaging* substepping at periapsis (it must not pass trivially at `N == 1`). With the v1-default `accel_ref = 1.0e-3` chosen here, the engagement is: apoapsis accel `≈ 8.2e-5` → `N == 1`; circular accel `≈ 3.0e-4` → `N == 1`; **periapsis accel `≈ 3.0e-2` (ratio `≈ 30`) → `N ≈ 5`** (`1 + floor(log2(30)) = 5`, well under `max_substeps = 64`). That is the measurement that makes case (2) a real test. CAVEAT: log2 is gentle — `N` here tops out near 5; it CANNOT reach the ~30 the old linear formula gave (`accel_ref` is a weak lever; `DT_DAYS` is the primary bounding lever — see the notes at the end of this task).
- **The `a == 0.0` star guard (Task 7) holds.** The single body in `star_config` uses Kepler elements with `a = 0.0`, which the ephemeris degenerates to a fixed point at the focus (the origin), giving the craft the clean central term `G*M / (r² + ε²)^1.5`. Task 7's `gravity_accel` must not divide by a zero `r` for the star's own slot and must not emit a NaN that the `is_finite()` assertions below would then catch as a false blowup. The eccentric periapsis here is `r_p = 0.1 AU` (never `r ≈ 0`), and softening `ε = 1.0e-3 AU` keeps `(r² + ε²)^1.5` strictly positive, so the only NaN source would be an upstream guard regression — which this test correctly surfaces.

`EventKind` gained a `Wake` variant in Task 8's Lod-dispatch fix; the arrival reader below uses `if let EventKind::Arrival { .. }`, which is forward-compatible with the added variant.

**Files**
- Create: `crates/jumpgate-core/tests/physics_sanity.rs`
- Modify: (none)
- Test: `crates/jumpgate-core/tests/physics_sanity.rs`

---

- [ ] **Step 1: Create the test file with the in-test v1-default config builder (the §11 tuning home).**
  Create `crates/jumpgate-core/tests/physics_sanity.rs`. This builder is the single place the four tests pull `dt` / substep schedule / softening ε from; the values here are the **measured v1 defaults** this task recommends promoting into `config.rs`. All imports come through the **crate-root re-export surface** (see the cross-task dependency note: `Dt`/`Tick` re-export from `time.rs`, `World` re-export, seam types from `types.rs`). Write exactly:

  ```rust
  //! Physics sanity + autopilot transfer integration tests.
  //!
  //! Bounded (not golden) checks over the full `World`:
  //!   1. near-circular orbit stays bounded over many orbits,
  //!   2. eccentric close-approach does NOT blow up (substepping + softening),
  //!   3. pure-coast specific-orbital-energy drift is bounded,
  //!   4. a fuel-budgeted autopilot transfer reaches its destination
  //!      deterministically (same config -> same arrival tick).
  //!
  //! RESOLVED §11 TUNING (v1 defaults, measured here; promote into config.rs later):
  //!   * dt              = 0.25 day
  //!   * softening (eps) = 1.0e-3 AU
  //!   * substep_cfg     = { accel_ref: 1.0e-3, max_substeps: 64 }
  //! Rationale: at 1 AU a near-circular orbit (period ~365 d) gets ~1460 ticks/orbit,
  //! Verlet stays well-bounded at N == 1. The acceleration-keyed LOG2 substep schedule
  //! (Task 8: N = clamp(1 + floor(log2(max(1.0, total_accel_mag / accel_ref))), 1, max_substeps))
  //! supplies extra accuracy ONLY where the field is steep: with accel_ref = 1e-3,
  //! the eccentric apoapsis (accel ~8.2e-5) and the circular orbit (accel ~3.0e-4)
  //! stay at N == 1, while the eccentric periapsis (accel ~3.0e-2, ratio ~30) climbs to N ~= 5
  //! -- so case (2) genuinely exercises substepping. CAVEAT: the log2 schedule is
  //! gentle (periapsis tops out near N ~= 5 here, ~9 at accel_ref=1e-4, ~12 at 1e-5;
  //! it CANNOT reach the ~30 substeps the old linear formula gave). accel_ref is a
  //! weak lever; the PRIMARY lever for keeping the e=0.9 orbit bounded is a smaller
  //! base DT_DAYS (and softening eps). Whether case (2) stays bounded under log2
  //! substepping is an empirical tuning question to settle by lowering DT_DAYS first.
  //!
  //! Upstream dependencies: Task 7's gravity_accel must honour the a == 0.0 star
  //! guard (no NaN from the star's own slot) and Task 8's substep formula must
  //! engage on the quantized total accel magnitude; the is_finite() asserts below
  //! would otherwise report an upstream regression as a (false) physics blowup.

  use jumpgate_core::{
      BaseSpec, BodyInit, Command, CommandKind, CraftInit, Dt, EntityRef, EventKind,
      G_CANONICAL, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg, Target,
      Tick, Vec3, World,
  };

  // ---- resolved v1 tuning defaults ----
  const DT_DAYS: f64 = 0.25;
  const SOFTENING: f64 = 1.0e-3;
  const SUBSTEP_CFG: SubstepCfg = SubstepCfg { accel_ref: 1.0e-3, max_substeps: 64 };

  /// A massive star pinned at the origin (a == 0 => Kepler conic degenerates to a
  /// fixed point at the focus), plus a caller-supplied set of craft. One body only,
  /// so the gravity field a craft feels is the clean central term G*M/(r^2+eps^2)^1.5.
  fn star_config(star_mass: f64, window: u64, craft: Vec<CraftInit>) -> RunConfig {
      RunConfig {
          master_seed: 0xJUMPGATE_SEED_PLACEHOLDER, // overwritten just below
          dt: Dt::new(DT_DAYS),
          softening: SOFTENING,
          substep_cfg: SUBSTEP_CFG,
          ephemeris_window: window,
          bodies: vec![BodyInit {
              mass: star_mass,
              elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
          }],
          craft,
      }
  }

  /// A coasting craft: zero fuel so the autopilot/thrust path is inert and the
  /// trajectory is pure gravity.
  fn coasting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
      CraftInit {
          spec: BaseSpec {
              base_dry_mass: 1.0e-12,        // ~negligible vs M_sun; craft exerts no gravity anyway
              base_max_thrust: 0.0,
              base_exhaust_velocity: 1.0e-2,
              base_fuel_capacity: 0.0,
          },
          pos,
          vel,
          fuel_mass: 0.0,
      }
  }
  ```

  NOTE: the literal `0xJUMPGATE_SEED_PLACEHOLDER` is intentionally invalid so the file does **not** compile yet — Step 2 replaces it with a real seed and the `star_config` helper gains a `seed` parameter. This is the deliberate red state.

- [ ] **Step 2: Run the file — confirm it fails to compile (red).**
  ```
  cargo test -p jumpgate-core --test physics_sanity -- --nocolor
  ```
  EXPECTED: a compile error referencing the invalid integer literal `0xJUMPGATE_SEED_PLACEHOLDER` (e.g. `error: invalid suffix ...` / `error[E0425]`). The test binary does not build. This is the expected red state. (If instead you see an unresolved-import error on any of the crate-root names — `Dt`, `Tick`, `World`, `NavDest`, etc. — that is a genuine upstream re-export gap, NOT this task's bug: the providing task's `lib.rs` is missing a `pub use`; fix it there, not here.)

- [ ] **Step 3: Fix the builder to take a real seed (green-the-scaffold).**
  Replace the `star_config` signature and the placeholder seed line so the helper threads a real `master_seed`:

  ```rust
  fn star_config(seed: u64, star_mass: f64, window: u64, craft: Vec<CraftInit>) -> RunConfig {
      RunConfig {
          master_seed: seed,
          dt: Dt::new(DT_DAYS),
  ```

  (Delete the `0xJUMPGATE_SEED_PLACEHOLDER` line entirely; keep the rest of `star_config` identical.) Then run:
  ```
  cargo test -p jumpgate-core --test physics_sanity -- --nocolor
  ```
  EXPECTED: `running 0 tests` then `test result: ok. 0 passed; 0 failed`. The scaffold compiles with no tests yet.

- [ ] **Step 4: Write the bounded near-circular-orbit test (red).**
  Append to the file. A craft at 1 AU with the exact circular speed `sqrt(G*M/r)` should orbit with bounded radius. Track min/max radius over ~10 orbits and assert the band stays within ±5% (sanity, not golden):

  ```rust
  #[test]
  fn circular_orbit_stays_bounded_over_many_orbits() {
      let m: f64 = 1.0; // M_sun
      let r0: f64 = 1.0; // AU
      let v_circ = (G_CANONICAL * m / r0).sqrt(); // AU/day
      // place at +x, velocity +y => prograde circular orbit in the z=0 plane
      let craft = vec![coasting_craft(
          Vec3::new(r0, 0.0, 0.0),
          Vec3::new(0.0, v_circ, 0.0),
      )];

      let period_days = std::f64::consts::TAU / (G_CANONICAL * m / (r0 * r0 * r0)).sqrt();
      let ticks_per_orbit = (period_days / DT_DAYS).ceil() as u64;
      let n_orbits: u64 = 10;
      let total_ticks = ticks_per_orbit * n_orbits;

      let (mut world, _cfg_hash) = World::reset(star_config(1, m, total_ticks + 8, craft));
      let cid = world.craft_ids()[0];

      let mut r_min = f64::INFINITY;
      let mut r_max = 0.0_f64;
      let mut cmds: Vec<Command> = Vec::new();
      for _ in 0..total_ticks {
          world.step(&mut cmds);
          let p = world.craft_pos(cid).expect("craft alive");
          assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "position went non-finite");
          let r = p.length();
          if r < r_min { r_min = r; }
          if r > r_max { r_max = r; }
      }
      // bounded: radius never drifts more than 5% off the initial circular radius
      assert!(r_min > 0.95 * r0, "orbit decayed inward: r_min = {r_min}");
      assert!(r_max < 1.05 * r0, "orbit grew outward: r_max = {r_max}");
  }
  ```

  Run only this test:
  ```
  cargo test -p jumpgate-core --test physics_sanity circular_orbit -- --nocolor
  ```
  EXPECTED (red→assess): the test runs. If the ±5% band fails (`orbit decayed inward` / `orbit grew outward`), that is the §11 measurement signal — proceed to Step 5 to tune.

- [ ] **Step 5: Tune dt/substep against the measured orbit, confirm green.**
  If Step 4 failed the band, the lever is the in-test tuning constants (this is the §11 work, not an engine change). Tighten `DT_DAYS` toward a smaller value (try `0.125`) and/or lower `SUBSTEP_CFG.accel_ref` so the circular orbit also picks up a substep or two, then re-run. (Note: under the log2 schedule the circular orbit at accel `≈ 3.0e-4` only reaches `N == 2` once `accel_ref ≤ ~1.5e-4`, so `DT_DAYS` is the stronger lever for the circular band.) Once the band holds, re-run:
  ```
  cargo test -p jumpgate-core --test physics_sanity circular_orbit -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`. Update the header `RESOLVED §11 TUNING` comment block (including the substep-engagement worked example) if the final `DT_DAYS` or `accel_ref` differs from the documented defaults.

- [ ] **Step 6: Write the eccentric close-approach no-blowup test (red).**
  Append. An eccentric orbit (`a = 1`, `e = 0.9` ⇒ apoapsis 1.9 AU, periapsis 0.1 AU) whips close to the star; substepping + softening must keep it finite and bounded (it must not tunnel to ~0 or fly off to infinity). The periapsis accel `≈ 3.0e-2` is ~100× the apoapsis accel (ratio `≈ 30`), so under the log2 schedule `N = clamp(1 + floor(log2(max(1.0, mag / accel_ref))), 1, max_substeps)` with `accel_ref = 1.0e-3` the substep count climbs from `N == 1` at apoapsis to `N ≈ 5` near periapsis (`1 + floor(log2(30)) = 1 + 4 = 5`) — i.e. this case **does** engage substepping, unlike a trivial `N == 1` pass, though only modestly (log2 is gentle; see Step 7 and the notes — `DT_DAYS` is the primary bounding lever). Use the vis-viva speed at apoapsis:

  ```rust
  #[test]
  fn eccentric_close_approach_does_not_blow_up() {
      let m: f64 = 1.0;
      let a: f64 = 1.0;   // semi-major axis (AU)
      let e: f64 = 0.9;   // high eccentricity => periapsis r_p = a(1-e) = 0.1 AU
      let r_apo = a * (1.0 + e);                 // 1.9 AU, start here
      // vis-viva: v^2 = G*M*(2/r - 1/a); at apoapsis velocity is purely tangential
      let v_apo = (G_CANONICAL * m * (2.0 / r_apo - 1.0 / a)).sqrt();
      let craft = vec![coasting_craft(
          Vec3::new(r_apo, 0.0, 0.0),
          Vec3::new(0.0, v_apo, 0.0),
      )];

      let period_days = std::f64::consts::TAU * (a * a * a / (G_CANONICAL * m)).sqrt();
      let total_ticks = (5.0 * period_days / DT_DAYS).ceil() as u64; // 5 orbits incl. 5 periapsis passes

      let (mut world, _h) = World::reset(star_config(2, m, total_ticks + 8, craft));
      let cid = world.craft_ids()[0];

      let mut r_min = f64::INFINITY;
      let mut r_max = 0.0_f64;
      let mut cmds: Vec<Command> = Vec::new();
      for _ in 0..total_ticks {
          world.step(&mut cmds);
          let p = world.craft_pos(cid).expect("craft alive");
          assert!(
              p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
              "close approach produced a non-finite position (blowup or upstream a==0 star NaN)"
          );
          let r = p.length();
          if r < r_min { r_min = r; }
          if r > r_max { r_max = r; }
      }
      // engaged substepping kept periapsis off the singularity ...
      assert!(r_min > 0.5 * a * (1.0 - e), "periapsis collapsed: r_min = {r_min}");
      // ... and did not get slingshot to escape (bound orbit stays near apoapsis scale)
      assert!(r_max < 3.0 * r_apo, "trajectory blew outward: r_max = {r_max}");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --test physics_sanity eccentric -- --nocolor
  ```
  EXPECTED (red→assess): test runs. If `r_min` collapses or a position goes non-finite, the substep schedule is not engaging enough at periapsis (Task 8 log2 formula tops out around `N ≈ 5` here — see Step 7; `accel_ref` is a weak lever) or `DT_DAYS` is too large or Task 7's `a == 0.0` star guard is regressed — proceed to Step 7 (lower `DT_DAYS` first).

- [ ] **Step 7: Tune the substep schedule for periapsis, confirm green.**
  If Step 6 blew up, the log2 schedule means `accel_ref` is a weak lever — lowering `SUBSTEP_CFG.accel_ref` from `1e-3` to `1e-4` only lifts periapsis from `N ≈ 5` to `N ≈ 9` (the schedule tops out near `N ≈ 12` at `1e-5`; it CANNOT reach the ~30 substeps the old linear formula gave, which would need ratio `= 2^29`). So the **primary lever is a smaller base `DT_DAYS`** (e.g. `0.125`), with softening ε as the singularity guard; nudge `accel_ref` down only as a secondary refinement, and/or raise `max_substeps`. Whether the `e = 0.9` orbit stays bounded under the gentler log2 substepping is the empirical tuning question to settle here — lower `DT_DAYS` first. Re-run:
  ```
  cargo test -p jumpgate-core --test physics_sanity eccentric -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`. Reflect any final substep values in the header comment block. Verify the formula is still engaging by sanity-checking that `r_min` lands near the analytic `r_p = 0.1` (within the bound), not pinned at the softening floor — if it pins at ε, the orbit is being absorbed and substepping is still too coarse.

- [ ] **Step 8: Write the bounded pure-coast energy-drift test (red).**
  Append. Specific orbital energy `E = v²/2 − G·M/r` is conserved exactly for a Kepler orbit; the integrator only approximates it. Assert the relative drift over a full orbit is small (sanity bound, e.g. < 1%):

  ```rust
  #[test]
  fn coast_specific_energy_drift_is_bounded() {
      let m: f64 = 1.0;
      let r0: f64 = 1.0;
      let v_circ = (G_CANONICAL * m / r0).sqrt();
      let craft = vec![coasting_craft(
          Vec3::new(r0, 0.0, 0.0),
          Vec3::new(0.0, v_circ, 0.0),
      )];

      let period_days = std::f64::consts::TAU / (G_CANONICAL * m / (r0 * r0 * r0)).sqrt();
      let total_ticks = (period_days / DT_DAYS).ceil() as u64; // one orbit

      let (mut world, _h) = World::reset(star_config(3, m, total_ticks + 8, craft));
      let cid = world.craft_ids()[0];

      let energy = |p: Vec3, v: Vec3| -> f64 {
          0.5 * v.length_sq() - G_CANONICAL * m / p.length()
      };
      let e0 = energy(
          world.craft_pos(cid).unwrap(),
          world.craft_vel(cid).unwrap(),
      );

      let mut cmds: Vec<Command> = Vec::new();
      for _ in 0..total_ticks {
          world.step(&mut cmds);
      }
      let e1 = energy(
          world.craft_pos(cid).unwrap(),
          world.craft_vel(cid).unwrap(),
      );

      let rel_drift = ((e1 - e0) / e0).abs();
      assert!(rel_drift < 1.0e-2, "energy drift too large over one orbit: {rel_drift}");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --test physics_sanity coast_specific_energy -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`. (If it fails marginally, loosen the bound to `2.0e-2` — this is a sanity check, not a golden conservation test, per §8.)

- [ ] **Step 9: Write the fuel-budgeted autopilot-transfer arrival test (red).**
  Append. A thrusting craft far from the star (weak gravity) is commanded to a nearby destination with a generous fuel mass + burn budget; assert an `EventKind::Arrival` for that craft is emitted within a tick budget. The destination is issued once at tick 0 through the single command-ingestion path (`Target::Entity(EntityRef::Craft(cid))` + `CommandKind::Destination`), and arrival events are read via `StateView::recent_events` (a `Tick` cursor from the crate-root re-export). The `if let EventKind::Arrival { .. }` is forward-compatible with the `EventKind::Wake` variant added by Task 8:

  ```rust
  /// A craft with real thrust + fuel, in a weak-gravity region so the autopilot's
  /// guidance dominates and the transfer is robust.
  fn thrusting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
      CraftInit {
          spec: BaseSpec {
              base_dry_mass: 1.0e-9,
              base_max_thrust: 1.0e-12,    // F so that F/(dry+fuel) ~ 1e-3 AU/day^2 (>> local g)
              base_exhaust_velocity: 1.0e-2,
              base_fuel_capacity: 1.0e-9,
          },
          pos,
          vel,
          fuel_mass: 1.0e-9,               // full tank => ample dv budget for a short hop
      }
  }

  /// Run a transfer to `dest` and return Some(arrival_tick) if an Arrival event for
  /// the (single) craft fired within `max_ticks`, else None.
  fn run_transfer(seed: u64, start: Vec3, dest: Vec3, budget: Option<f64>, max_ticks: u64)
      -> Option<u64>
  {
      let craft = vec![thrusting_craft(start, Vec3::ZERO)];
      let (mut world, _h) = World::reset(star_config(seed, 1.0, max_ticks + 8, craft));
      let cid = world.craft_ids()[0];

      // single ingestion path: command the destination once at tick 0
      let mut cmds = vec![Command {
          target: Target::Entity(EntityRef::Craft(cid)),
          kind: CommandKind::Destination { dest: NavDest::Position(dest), burn_budget: budget },
      }];
      world.step(&mut cmds); // tick 0 ingests + integrates

      let mut last_seen = Tick(0);
      loop {
          for ev in world.recent_events(last_seen) {
              if let EventKind::Arrival { craft: ac, .. } = ev.kind {
                  if ac == cid {
                      return Some(ev.tick.0);
                  }
              }
          }
          last_seen = world.tick();
          if world.tick().0 >= max_ticks {
              return None;
          }
          let mut none: Vec<Command> = Vec::new();
          world.step(&mut none);
      }
  }

  #[test]
  fn fueled_autopilot_transfer_reaches_destination() {
      // weak-gravity region (~5 AU): central accel ~ G*M/25 ~ 1.2e-5, thrust ~1e-3 dominates
      let start = Vec3::new(5.0, 0.0, 0.0);
      let dest = Vec3::new(5.5, 0.0, 0.0); // 0.5 AU hop
      let arrival = run_transfer(11, start, dest, Some(1.0), 4000);
      assert!(arrival.is_some(), "craft never emitted Arrival within budget");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --test physics_sanity fueled_autopilot_transfer_reaches -- --nocolor
  ```
  EXPECTED (red→assess): test runs. If `arrival.is_some()` fails, the thrust is too weak or `max_ticks` too small — proceed to Step 10.

- [ ] **Step 10: Tune thrust/budget for a robust arrival, confirm green.**
  If Step 9 returned `None`, raise `base_max_thrust` in `thrusting_craft` (e.g. `1.0e-11`) so thrust accel comfortably exceeds local gravity, and/or raise `max_ticks`. Keep the hop short (≤0.5 AU) and the budget generous (`Some(1.0)`). Re-run:
  ```
  cargo test -p jumpgate-core --test physics_sanity fueled_autopilot_transfer_reaches -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 11: Write the determinism test (same config → same arrival tick).**
  Append. Two transfers built from identical inputs (including the identical `master_seed`, which is the single u64 the determinism contract derives all named ChaCha8Rng sub-streams from) must produce the identical arrival tick — Tier-B reproducibility of the autopilot path:

  ```rust
  #[test]
  fn transfer_arrival_tick_is_deterministic() {
      let start = Vec3::new(5.0, 0.0, 0.0);
      let dest = Vec3::new(5.5, 0.0, 0.0);
      let a = run_transfer(11, start, dest, Some(1.0), 4000);
      let b = run_transfer(11, start, dest, Some(1.0), 4000);
      assert!(a.is_some(), "first run did not arrive");
      assert_eq!(a, b, "same config produced different arrival ticks: {a:?} vs {b:?}");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --test physics_sanity transfer_arrival_tick_is_deterministic -- --nocolor
  ```
  EXPECTED: `test result: ok. 1 passed; 0 failed`. (A mismatch here means the autopilot/integration path is reading a non-replayed source — escalate to the determinism owner; do not paper over it by loosening the assert.)

- [ ] **Step 12: Run the full file and confirm all five tests pass.**
  ```
  cargo test -p jumpgate-core --test physics_sanity -- --nocolor
  ```
  EXPECTED: `test result: ok. 5 passed; 0 failed; 0 ignored`.

- [ ] **Step 13: Lint the new test under the workspace clippy gate.**
  Test modules are only linted via `--all-targets` (per the project memo: `--lib` is a no-op for this binary-style crate). Run:
  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings emitted from `tests/physics_sanity.rs`. (Fix any `clippy::needless_range_loop` / `clippy::approx_constant` etc. in the test before committing.)

- [ ] **Step 14: Commit.**
  ```
  git add crates/jumpgate-core/tests/physics_sanity.rs
  git commit -m "$(cat <<'EOF'
  test(core): physics sanity + deterministic autopilot transfer

  Integration tests over the full World: bounded near-circular orbit over
  many orbits, eccentric close-approach no-blowup (substepping engages at
  periapsis: N~=5 at accel_ref=1e-3 under the log2 schedule + softening),
  bounded pure-coast
  energy drift, and a fuel-budgeted autopilot transfer that emits Arrival
  within budget deterministically. Resolves the §11 tuning items (dt=0.25
  day, eps=1e-3 AU, substep_cfg={accel_ref:1e-3, max_substeps:64}) by
  measurement; values documented in-file for promotion into config.rs.
  Imports route through the crate-root re-export surface.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: one new file committed; `git status` clean afterward.

**Notes for the implementer**
- This task writes the test file ONLY. The chosen `dt` / softening / substep values live in the in-test constants and the header comment; promoting them into `config.rs` (e.g. a `RunConfig::v1_default` or `Default`) is a separate change outside this task's file scope.
- All imports are through the **crate-root re-export surface** (`jumpgate_core::{...}`), deliberately, so the upstream `types.rs`-vs-`contract.rs` / `time.rs`-vs-`config.rs` module split cannot drift this consumer out of compile. An unresolved-import failure here is an upstream `pub use` gap in the providing task's `lib.rs`, not a Task 15 bug. In particular `Dt` and `Tick` re-export from `time.rs` (never `config.rs`) and `World` is the re-export `jumpgate_core::World` (never `jumpgate_core::world::World`). NOTE: the locked `lib.rs` already re-exports the plan-1 public symbols at the crate root (`Vec3`, `Tick`, `Dt`, the seam enums, `Command`/`Event`/`EventKind`/`StateView`, config types, etc.), so `use jumpgate_core::{...}` resolves them. Each plan-2/plan-3 module-providing task must APPEND its new symbols to `lib.rs`'s `pub use` block as those modules land (`Ephemeris`, `VelocityVerlet`, `substep_count`, `gravity_accel`, `thrust_accel_and_burn`, `autopilot_command`; `World`/`Observer`/`FullObserver`/`View`/`ActionLog`/`EventStream`/`state_hash`/`replay`).
- All assertions are **bounded sanity checks** (§8), not golden values — loosen tolerances if a check fails *marginally* for the right physical reason, but never loosen the determinism assert in Step 11 (a mismatch there is a real Tier-B bug).
- The eccentric case (Steps 6–7) is the one test whose *value* depends on an upstream behaviour: Task 8's acceleration-keyed substep formula must actually engage (`N > 1`) at periapsis and Task 7's `a == 0.0` star guard must hold. Under the canonical **log2** schedule `N = clamp(1 + floor(log2(max(1.0, mag / accel_ref))), 1, max_substeps)` (Task 8), the documented `accel_ref = 1.0e-3` puts periapsis (accel `≈ 3.0e-2`, ratio `≈ 30`) at only `N ≈ 5` while apoapsis/circular stay at `N == 1`. **Critical tuning caveat:** the log2 schedule is gentle — periapsis tops out near `N ≈ 5` at `accel_ref = 1e-3` (`N ≈ 9` at `1e-4`, `N ≈ 12` at `1e-5`); it CANNOT reach the ~30 substeps the old linear formula gave (that would need ratio `= 2^29`). So `accel_ref` is a weak lever here; the **primary lever** for keeping the `e = 0.9` orbit bounded is a smaller base `DT_DAYS` (and softening ε). Whether the eccentric test stays bounded under the gentler log2 substepping is an empirical Task-15 tuning question to settle by lowering `DT_DAYS` first; if `r_min` pins at the softening floor instead of near the analytic `r_p = 0.1`, lower `DT_DAYS` before touching `accel_ref`.
