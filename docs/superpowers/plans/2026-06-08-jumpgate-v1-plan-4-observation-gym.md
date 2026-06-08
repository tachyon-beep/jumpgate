# jumpgate v1 — Plan 4: Observation & gym Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **READ FIRST:** `2026-06-08-jumpgate-v1-plan-0-contract-surface.md` — canonical signatures, workspace layout, and plan-level conventions (it wins on any conflict).

**Goal:** Build the jumpgate v1 rung-3 deterministic (Tier B) 3D Newtonian space core — on-rails bodies, gravity-feeling thrust/fuel/mass craft flown by an in-engine autopilot under a navigator macro-action — exposed as a reproducible Gymnasium env, with a per-tick state-hash replay-equivalence contract.

**Architecture:** Two crates: a pure-Rust `jumpgate-core` (`#![forbid(unsafe_code)]`) that is the sole authoritative writer (SoA stores, tick-indexed ephemeris, velocity-Verlet behind an Integrator trait with accel-keyed integer substepping, Tsiolkovsky variable-mass craft, autopilot guidance, one typed Command/Target ingestion path, a typed Event stream, FNV-1a state hashing, and log-replay), and a `jumpgate-py` PyO3 cdylib facade that writes frame-relative f32 observations into caller-provided buffers and presents the Gymnasium 5-tuple. All facades read through one `StateView` trait that exposes command+event history, not just physics; the engine is shaped (Target sum, Event typing, observer-parameterized projection, effective-param accessor, slot-map ids, Lod seam) so combat/upgrades/fog-of-war drop in without a contract break.

**Tech Stack:** Rust 2024 edition (rustc/cargo 1.95; the slot-map generation field is named `generation` to sidestep the edition-2024 reserved keyword; a toolchain/edition/RNG-pin bump is a reviewed determinism rebaseline — see spec §6 and `provenance.rs`); jumpgate-core deps: rand_chacha (pinned, ChaCha8Rng) + rand_core only; no serde/glam/rayon in the hashed path; hand-rolled f64 Vec3. jumpgate-py: pyo3 0.23 + numpy 0.23 (abi3-py312, extension-module). Build via /home/john/jumpgate/archive/.venv/bin/python -m maturin develop --release (the `--release` is REQUIRED so the Tier-B FP determinism profile reaches the training cdylib — `maturin develop` defaults to a debug build; see spec §6). Python test deps already present: gymnasium 1.2.3, numpy 2.4.6, torch 2.9.1. Workspace-root clippy.toml with disallowed-methods. FNV-1a hashing hand-rolled over f64::to_bits little-endian.

**This plan covers Tasks 16–18.** Prerequisite: Plan 1, Plan 2, Plan 3 complete.

---

### Task 16: Frame-relative observation extraction into buffers

Implements the §7.2 hard invariant: positional/velocity observations are frame-relative. The f64 subtraction (`p_craft - p_target`, `v_craft - v_target`) happens in Rust over core f64 values; ONLY the small relative delta is downcast to f32. A debug assertion guards that no raw absolute coordinate (magnitude ~AU scale) ever crosses the f32 boundary. The observation is a fixed ego block + a presence-masked variable-length entity set (v1 emits zero live neighbors, but the schema reserves slots and is versioned so combat's variable-N neighbors never force a contract break).

The module is factored into two layers so the substance is unit-testable without ever constructing a `View`:
- `write_obs_parts(...)` — a pure fn over plain f64/f32 values that does the f64->f32 delta downcast, the absolute-coordinate guard, the fixed layout, and presence-mask zeroing. ALL three required tests target this fn with synthetic values.
- `write_obs_frame_relative(view, ego, out)` — a thin gatherer that pulls values out of `View` and calls `write_obs_parts`. Its end-to-end correctness is covered later by the Python gym smoke test (same-seed -> identical obs), NOT here.

This is pure Rust and lives in `jumpgate-py` but uses NO PyO3 yet — it is unit-tested via `cargo test`.

**Files**
- Create: `crates/jumpgate-py/src/obs.rs`
- Modify: `crates/jumpgate-py/src/lib.rs`
- Test: `crates/jumpgate-py/src/obs.rs` (inline `#[cfg(test)] mod tests`)

**Depends on:** Task 12 (`View`, `Observer`, `project`).

**Contract types in play:** `View`, `StateView`, `Vec3`, `CraftId`, `write_obs_frame_relative`.

**Preconditions / cross-task dependencies (verify before the commands below run):**
- `crates/jumpgate-py/Cargo.toml` declares `crate-type = ["cdylib", "rlib"]` and enables the PyO3 `extension-module` feature UNCONDITIONALLY (always on, alongside `abi3-py312`): `pyo3 = { workspace = true, features = ["extension-module", "abi3-py312"] }`. Because `extension-module` suppresses linking libpython, a plain `cargo test -p jumpgate-py` test binary may FAIL to link on Linux (the classic PyO3 gotcha) even though `obs.rs` itself is PyO3-free. This is a real build concern the implementer must resolve — do NOT assume `cargo test` "links without libpython." The pragmatic path: exercise the PURE obs logic (`write_obs_parts`, the `OBS_DIM` layout) via unit tests that do not require the pyo3/libpython link where possible, and exercise the full PyO3 surface through the Python smoke/determinism test (Task 18, via `maturin develop --release` + pytest), which is the linkage-correct path. Do NOT make `extension-module` optional/non-default to dodge this — the locked Cargo.toml keeps it always-on by design.
- The gatherer reads `View` via accessors shaped like the `StateView` contract (`craft_pos`, `craft_vel`, `craft_fuel`, plus `craft_fuel_capacity` for effective fuel capacity — `StateView` exposes capacity directly via the `craft_fuel_capacity` accessor, the §5.5 effective-param seam, where effective == base in v1; no `View` `Effective`/`BaseSpec` detour is needed, and plan-4's own later step calls `view.craft_fuel_capacity(ego)`). These accessor NAMES are owned by Task 12's `View`. If Task 12 named them differently, ONLY the gatherer (`write_obs_frame_relative`) rebinds; the tested helper `write_obs_parts` is unaffected. Do not invent a rich `View` API.

---

- [ ] **Step 1: Add the `obs` module to the py crate lib.**
  In `crates/jumpgate-py/src/lib.rs`, add the module declaration so the new file compiles. Add it near the top of the file (alongside the existing `mod env;` if present):
  ```rust
  mod obs;
  ```
  This is the only change to `lib.rs` for this task — `obs.rs` contains no `#[pyclass]`/`#[pymethods]`, so nothing is registered in the `#[pymodule]`.

- [ ] **Step 2: Create `obs.rs` with the layout constants and a stub helper (does NOT compile the tests yet).**
  Create `crates/jumpgate-py/src/obs.rs` with the versioned schema constants and an unimplemented `write_obs_parts` so the next step's tests have a symbol to call:
  ```rust
  //! Frame-relative observation extraction (§7.2 hard invariant).
  //!
  //! The f64 subtraction (p_craft - p_target, v_craft - v_target) happens HERE
  //! over core f64 values; ONLY the small relative delta is downcast to f32.
  //! A debug assertion guards that no raw absolute coordinate (~AU scale) is
  //! ever cast to f32. Layout = fixed ego block + presence-masked entity set.
  //!
  //! This module is PyO3-free and unit-tested via `cargo test` without Python.

  use jumpgate_core::math::Vec3;

  /// Bumped whenever the obs layout changes. The variable-N entity set is
  /// reserved (zeroed-absent) in v1 so combat's neighbors never force a break.
  pub const SCHEMA_VERSION: u32 = 1;

  /// Ego block: own velocity in the target frame (3) + fuel fraction (1).
  /// Own position relative to the frame target is trivially ~zero and is NOT
  /// emitted (emitting it would risk a raw ego coord crossing the boundary).
  pub const EGO_LEN: usize = 4;

  /// Reserved neighbor slots. v1 emits zero LIVE neighbors but writes every
  /// slot as masked-absent, so "presence mask zeros for absent slots" is real.
  pub const MAX_NEIGHBORS: usize = 4;

  /// Per-neighbor: presence flag (1) + relative pos (3) + relative vel (3).
  pub const ENTITY_STRIDE: usize = 7;

  /// Total observation width.
  pub const OBS_DIM: usize = EGO_LEN + MAX_NEIGHBORS * ENTITY_STRIDE;

  /// Tripwire bound (AU). Any frame-relative delta passed to the f32 boundary
  /// must be smaller than this in magnitude; an absolute solar-system coord
  /// (~tens of AU) trips it, catching the §7.2 "forgot to subtract" bug.
  /// Recalibrated when live neighbors arrive (the schema is versioned).
  pub const MAX_REL_AU: f64 = 1.0;

  /// Downcast a single frame-relative scalar to f32, guarding the boundary.
  #[inline]
  fn rel_to_f32(v: f64) -> f32 {
      debug_assert!(
          v.abs() < MAX_REL_AU,
          "absolute coordinate {v} crossed the f32 observation boundary \
           (>= MAX_REL_AU {MAX_REL_AU} AU); obs must be frame-relative (§7.2)"
      );
      v as f32
  }

  /// Pure obs writer over plain values (no `View`). Does the delta downcast,
  /// the boundary guard, the fixed layout, and presence-mask zeroing.
  ///
  /// `ego_vel_in_frame` is the craft velocity already expressed relative to the
  /// frame target (an f64 delta). `ego_fuel_frac` is dimensionless (O(1)).
  /// `neighbors` are (rel_pos, rel_vel) f64 deltas for LIVE neighbors only;
  /// any beyond `MAX_NEIGHBORS` are ignored, remaining slots are masked-absent.
  pub fn write_obs_parts(
      _ego_vel_in_frame: Vec3,
      _ego_fuel_frac: f32,
      _neighbors: &[(Vec3, Vec3)],
      _out: &mut [f32],
  ) {
      unimplemented!()
  }
  ```
  Compile-check only (the body is `unimplemented!()`):
  ```
  cargo build -p jumpgate-py 2>&1 | tail -5
  EXPECTED: Compiling jumpgate-py ... / Finished ... (warnings about unused vars are OK)
  ```

- [ ] **Step 3: Write the three failing tests against `write_obs_parts`.**
  Append to `crates/jumpgate-py/src/obs.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use jumpgate_core::math::Vec3;

      // Test A: a small relative delta between two tens-of-AU positions retains
      // sub-meter precision in f32, whereas the absolute coord would lose >km.
      #[test]
      fn relative_delta_retains_sub_meter_precision() {
          // Two craft ~10 AU out, separated by 0.0001 AU (~14_960 km).
          let p_craft = 10.000_1_f64; // AU, one axis
          let p_target = 10.000_0_f64; // AU
          let delta_au = p_craft - p_target; // 1e-4 AU

          // Frame-relative path: subtract in f64, downcast the small delta.
          let rel_f32 = delta_au as f32;
          // 1 AU = 1.495_978_707e11 m.
          const AU_M: f64 = 1.495_978_707e11;
          let err_m = ((rel_f32 as f64) - delta_au).abs() * AU_M;
          assert!(err_m < 1.0, "frame-relative err {err_m} m must be sub-meter");

          // Absolute path (what we must NOT do): downcast the ~10 AU coord.
          let abs_f32 = p_craft as f32;
          let abs_err_m = ((abs_f32 as f64) - p_craft).abs() * AU_M;
          assert!(
              abs_err_m > 1_000.0,
              "absolute coord err {abs_err_m} m should exceed a km (proves the \
               frame-relative transform is load-bearing)"
          );
      }

      // Test B: the debug-assert guard fires if a raw absolute (~tens of AU)
      // coordinate is fed to the boundary. Only meaningful in debug builds.
      #[test]
      #[should_panic(expected = "crossed the f32 observation boundary")]
      #[cfg(debug_assertions)]
      fn guard_fires_on_absolute_coordinate() {
          let mut out = [0.0f32; OBS_DIM];
          // ego velocity-in-frame holds a raw absolute coord (~10 AU) — the bug.
          let abs_vel = Vec3::new(10.0, 0.0, 0.0);
          write_obs_parts(abs_vel, 0.5, &[], &mut out);
      }

      // Test C: with zero live neighbors, every reserved entity slot is written
      // as masked-absent (presence flag 0 + zeroed payload).
      #[test]
      fn presence_mask_zeros_for_absent_slots() {
          let mut out = [9.0f32; OBS_DIM]; // sentinel: prove we overwrite.
          let ego_vel = Vec3::new(0.001, -0.002, 0.0003); // small frame-rel deltas
          write_obs_parts(ego_vel, 0.75, &[], &mut out);

          // Ego block written.
          assert_eq!(out[0], 0.001_f32);
          assert_eq!(out[1], -0.002_f32);
          assert_eq!(out[2], 0.000_3_f32);
          assert_eq!(out[3], 0.75_f32); // fuel fraction

          // Every neighbor slot is masked-absent: flag 0 + zeroed payload.
          for n in 0..MAX_NEIGHBORS {
              let base = EGO_LEN + n * ENTITY_STRIDE;
              assert_eq!(out[base], 0.0_f32, "slot {n} presence flag must be 0");
              for k in 1..ENTITY_STRIDE {
                  assert_eq!(out[base + k], 0.0_f32, "slot {n} payload[{k}] must be 0");
              }
          }
      }
  }
  ```
  Run them — they MUST fail (the impl is `unimplemented!()`):
  ```
  cargo test -p jumpgate-py obs:: -- --nocapture
  EXPECTED: test result: FAILED. (panics: "not implemented" from unimplemented!()
            in tests A and C; test B's should_panic catches the wrong message)
  ```

- [ ] **Step 4: Implement `write_obs_parts` to pass the tests.**
  Replace the stub body in `crates/jumpgate-py/src/obs.rs`:
  ```rust
  pub fn write_obs_parts(
      ego_vel_in_frame: Vec3,
      ego_fuel_frac: f32,
      neighbors: &[(Vec3, Vec3)],
      out: &mut [f32],
  ) {
      debug_assert_eq!(out.len(), OBS_DIM, "obs buffer must be OBS_DIM wide");

      // --- Ego block: velocity-in-frame (guarded delta) + fuel fraction. ---
      out[0] = rel_to_f32(ego_vel_in_frame.x);
      out[1] = rel_to_f32(ego_vel_in_frame.y);
      out[2] = rel_to_f32(ego_vel_in_frame.z);
      out[3] = ego_fuel_frac; // dimensionless O(1); no boundary guard needed.

      // --- Presence-masked entity set: fill all reserved slots. ---
      for n in 0..MAX_NEIGHBORS {
          let base = EGO_LEN + n * ENTITY_STRIDE;
          match neighbors.get(n) {
              Some(&(rel_pos, rel_vel)) => {
                  out[base] = 1.0; // present
                  out[base + 1] = rel_to_f32(rel_pos.x);
                  out[base + 2] = rel_to_f32(rel_pos.y);
                  out[base + 3] = rel_to_f32(rel_pos.z);
                  out[base + 4] = rel_to_f32(rel_vel.x);
                  out[base + 5] = rel_to_f32(rel_vel.y);
                  out[base + 6] = rel_to_f32(rel_vel.z);
              }
              None => {
                  // Masked-absent: flag 0 + zeroed payload.
                  for k in 0..ENTITY_STRIDE {
                      out[base + k] = 0.0;
                  }
              }
          }
      }
  }
  ```
  Run the tests — they MUST pass:
  ```
  cargo test -p jumpgate-py obs:: -- --nocapture
  EXPECTED: test result: ok. 3 passed; 0 failed
  ```

- [ ] **Step 5: Add the thin gatherer `write_obs_frame_relative` (the contract signature).**
  Append to `crates/jumpgate-py/src/obs.rs` the public contract fn that reads `View` and delegates to `write_obs_parts`. This does the f64 subtraction over core values and computes the fuel fraction; it emits zero live neighbors in v1 (`&[]`), so every reserved slot is masked-absent.
  ```rust
  use jumpgate_core::contract::StateView;
  use jumpgate_core::ids::CraftId;
  use jumpgate_core::world::View;

  /// Frame-relative obs extraction from a projected `View` into `out`.
  ///
  /// Reads the ego craft's pos/vel/fuel from the view, computes the
  /// frame-relative velocity (v_craft - v_target; v1 frame target is the
  /// ego craft itself, so the ego delta is its own velocity in core units
  /// kept small by canonical units), the fuel fraction, and writes a v1
  /// zero-neighbor (presence-masked) observation. End-to-end correctness is
  /// covered by the Python gym smoke test, not by a unit test here.
  ///
  /// ACCESSOR NAMES below (`craft_vel`, `craft_fuel`, fuel-capacity) are owned
  /// by Task 12's `View`; if they differ, rebind ONLY this fn — `write_obs_parts`
  /// is unaffected.
  pub fn write_obs_frame_relative(view: &View, ego: CraftId, out: &mut [f32]) {
      // Velocity in the ego frame. v1 uses an ego-centric frame; the velocity
      // delta is already small in canonical units.
      let ego_vel = view.craft_vel(ego).unwrap_or(Vec3::ZERO);

      // Fuel fraction = fuel_mass / fuel_capacity (dimensionless, O(1)).
      let fuel_mass = view.craft_fuel(ego).unwrap_or(0.0);
      let capacity = view.craft_fuel_capacity(ego).unwrap_or(1.0);
      let fuel_frac = if capacity > 0.0 {
          (fuel_mass / capacity) as f32
      } else {
          0.0
      };

      // v1: zero live neighbors; all reserved slots written masked-absent.
      let neighbors: &[(Vec3, Vec3)] = &[];
      write_obs_parts(ego_vel, fuel_frac, neighbors, out);
  }
  ```
  Compile-check (no new test runs the gatherer; it must build against `View`):
  ```
  cargo build -p jumpgate-py 2>&1 | tail -5
  EXPECTED: Finished ...
  ```
  If this fails ONLY because a `View`/`StateView` accessor name differs (e.g. `craft_fuel_capacity` is named otherwise in Task 12), rebind the accessor calls in THIS fn to match — do not touch `write_obs_parts` or the tests.

- [ ] **Step 6: Run the full crate test + clippy to confirm nothing regressed.**
  ```
  cargo test -p jumpgate-py 2>&1 | tail -10
  EXPECTED: test result: ok. 3 passed; 0 failed (plus any pre-existing env tests)
  ```
  ```
  cargo clippy -p jumpgate-py --all-targets 2>&1 | tail -10
  EXPECTED: Finished ... (no warnings from obs.rs)
  ```

- [ ] **Step 7: Commit.**
  ```
  git add crates/jumpgate-py/src/obs.rs crates/jumpgate-py/src/lib.rs
  git commit -m "$(cat <<'EOF'
  Add frame-relative observation extraction (Task 16)

  Implements the §7.2 hard invariant: f64 subtraction in Rust, only the
  small relative delta downcast to f32, with a debug-assert guard that no
  absolute (~AU scale) coordinate crosses the boundary. Layout = fixed ego
  block + presence-masked, versioned variable-N entity set (zero live
  neighbors in v1). Factored into a pure unit-tested `write_obs_parts` plus
  a thin `write_obs_frame_relative` gatherer over `View`.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  EXPECTED: [<branch> <hash>] Add frame-relative observation extraction (Task 16)
            2 files changed
  ```


---

### Task 17: PyO3 Gymnasium env (caller buffers, 5-tuple seam, per-craft)

Build `JumpgateEnv`: a PyO3 `#[pyclass]` holding `Vec<World>` (one per env), that on `reset(seed)` rebuilds every world with `master_seed` derived per-env and writes the initial frame-relative observation into a caller-provided numpy buffer, and on `step(action)` decodes a per-craft navigator macro-action from a flat `f32` buffer into a `Command{target:Entity(Craft), kind:Destination{..}}`, advances each world one tick, and writes obs / reward / terminated / truncated into four caller-provided buffers. No 5-tuple and no `info` are assembled in Rust (that is the Python wrapper's job, task 18); `terminated` (Arrival/task-success) and `truncated` (time-limit) are kept DISTINCT per §7.3. Engine writes into caller buffers (one memcpy/step pattern) — NO zero-copy views into sim memory (§7.3 FFI rule).

**Architectural linchpin (do not deviate):** the action decode and reward are pure free functions `decode_action` and `compute_reward`, NOT logic buried inside `#[pymethods]`. The unit test exercises these with no GIL and no live numpy, giving a real red→green cycle. `reset`/`step` stay thin: index the flat buffer, call the pure fns, call `write_obs_frame_relative`.

**Cross-task contract surface this task CONSUMES (per the cross-task contract-surface document built before Task 3 — symbols are listed with their PROVIDING task so the import paths are unambiguous):**

- From crate `jumpgate_core` (the core crate; Task 15's `jumpgate-py/Cargo.toml` declares the dep `jumpgate-core` → crate path `jumpgate_core`): `World` (Task: world.rs), `RunConfig`, `Dt`, `SubstepCfg`, `BodyInit`, `CraftInit`, `BaseSpec`, `OrbitalElements` (config.rs), `Vec3` (math.rs), `Command`, `Target`, `EntityRef`, `CommandKind`, `NavDest`, `Event`, `EventKind` (contract.rs / types.rs seam types re-exported at crate root), `CraftId` (ids.rs), `FullObserver` (world.rs), and the read trait `StateView` (contract.rs). `StateView` MUST be imported because `World::tick / dt / craft_ids / craft_pos / craft_fuel / recent_events` are trait methods, not inherent methods — calling them without the trait in scope is a compile error. (`World::reset / step / project` ARE inherent and need no trait import.)
- From THIS crate (`jumpgate-py`), module `obs` (Task 16): `OBS_DIM` and `write_obs_frame_relative`. These are defined in `crates/jumpgate-py/src/obs.rs`, NOT in `jumpgate_core` — import them as `use crate::obs::{OBS_DIM, write_obs_frame_relative};`. They must NEVER be confused with a core symbol or hardcoded as a magic number; `OBS_DIM` is the single source of truth that matches `write_obs_frame_relative`'s layout.

**Pinned decisions (no placeholders):**
- `action_dim == 4`: `[dest_x, dest_y, dest_z, burn_budget]`. Destination is **ego-relative** (consistent with §7.2 frame-relative philosophy — this is why `ego_pos` is passed into `decode_action`): `dest_abs = ego_pos + Vec3::new(a[0],a[1],a[2]) as f64`. Burn-budget sentinel: `a[3] < 0.0 ⇒ None`, else `Some(a[3] as f64)`.
- `obs_dim` is sourced from `crate::obs::OBS_DIM` (Task 16, same crate) — NEVER a magic number (it must match `write_obs_frame_relative`'s layout).
- Per-env seed: `master_seed = seed.wrapping_add(env_idx as u64)` so vectorized envs are distinct and reproducible (a single shared seed would make all envs identical).
- Flat-buffer index math: per-craft block `base = (env*num_craft + craft)*action_dim` for actions / `*obs_dim` for obs; reward / terminated / truncated are sized `num_envs*num_craft` and indexed `env*num_craft + craft`.
- Reward: `compute_reward(prev_fuel, cur_fuel, arrived, dt)` = `-(prev_fuel - cur_fuel)` (fuel spent penalty) `- 0.001*dt` (time penalty) `+ if arrived { 1.0 } else { 0.0 }` (task-success bonus, §7.4). All f64, downcast to f32 at the buffer boundary.
- Config template: `new()` builds a minimal `RunConfig` from contract types — one `BodyInit` (a central star) and `num_craft` identical `CraftInit`. No scenario-builder fn is referenced (none exists in the contract or tasks 14/16). The `SubstepCfg.accel_ref` value is calibrated to the redesigned reference-acceleration scale from Task 8: characteristic gravity at 1 AU from a 1 M☉ star is `a = G_CANONICAL·M/r² ≈ 2.96e-4 AU/day²`, so `accel_ref = 3.0e-4` keeps the substep count near 1 at cruise and only escalates on close approach. (A value like `1e-6` would saturate `max_substeps` every tick — wrong scale.)

**Files**
- Create: `crates/jumpgate-py/src/env.rs`
- Modify: `crates/jumpgate-py/src/lib.rs`
- Test: `crates/jumpgate-py/src/env.rs` (the `#[cfg(test)] mod tests` exercising `decode_action` + `compute_reward`)

`EventKind::Arrival` is matched (via `matches!`, so new `EventKind` variants such as the Task-10 `Wake` do not break this code) to detect task-success.

---

- [ ] **Step 1: Register `JumpgateEnv` in the `#[pymodule]` (lib.rs)**

Replace the body of `crates/jumpgate-py/src/lib.rs` so the module declares `obs` (Task 16) and `env`, and registers the class. (Task 15 created a minimal `lib.rs` with the `#[pymodule] _native`; Task 16 added `mod obs;`; this wires `env.rs` in.)

```rust
//! PyO3 extension module `jumpgate._native`.
//! unsafe is ALLOWED in this crate (FFI); the core crate forbids it.

use pyo3::prelude::*;

mod env;
mod obs;

pub use env::JumpgateEnv;

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<JumpgateEnv>()?;
    Ok(())
}
```

This will not compile yet (`mod env;` references a file we create next) — that is expected; Step 2 creates `env.rs`. (`mod obs;` already exists from Task 16.)

---

- [ ] **Step 2: Write a FAILING unit test for `decode_action` (env.rs)**

Create `crates/jumpgate-py/src/env.rs` containing ONLY the imports and the test module so the first `cargo test` fails on an undefined function (`decode_action`). Note the split imports: seam/physics types from `jumpgate_core`; `OBS_DIM` + `write_obs_frame_relative` from THIS crate's `obs` module (Task 16). `StateView` is imported because the `World` accessors used in Step 5 are trait methods.

```rust
//! `JumpgateEnv`: the Gymnasium PyO3 facade. Holds one `World` per env, writes
//! frame-relative obs / reward / terminated / truncated into caller buffers.
//! Pure `decode_action` / `compute_reward` are unit-tested without the GIL.

use crate::obs::{write_obs_frame_relative, OBS_DIM};
use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, CraftId, CraftInit, Dt, EntityRef, Event, EventKind,
    FullObserver, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg, Target, Vec3, World,
};
use numpy::{PyReadonlyArray1, PyReadwriteArray1};
use pyo3::prelude::*;

pub const ACTION_DIM: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_action_builds_ego_relative_destination_with_budget() {
        let ego = Vec3::new(1.0, 2.0, 3.0);
        let craft = CraftId { slot: 0, generation: 0 };
        let a = [10.0f32, 0.0, 0.0, 5.0];
        let cmd = decode_action(&a, ego, craft);
        assert_eq!(cmd.target, Target::Entity(EntityRef::Craft(craft)));
        match cmd.kind {
            CommandKind::Destination { dest, burn_budget } => {
                match dest {
                    NavDest::Position(p) => {
                        assert!((p.x - 11.0).abs() < 1e-6);
                        assert!((p.y - 2.0).abs() < 1e-6);
                        assert!((p.z - 3.0).abs() < 1e-6);
                    }
                    NavDest::Entity(_) => panic!("expected Position dest"),
                }
                assert_eq!(burn_budget, Some(5.0));
            }
        }
    }

    #[test]
    fn decode_action_negative_budget_is_none() {
        let ego = Vec3::ZERO;
        let craft = CraftId { slot: 0, generation: 0 };
        let a = [0.0f32, 0.0, 0.0, -1.0];
        let cmd = decode_action(&a, ego, craft);
        match cmd.kind {
            CommandKind::Destination { burn_budget, .. } => assert_eq!(burn_budget, None),
        }
    }
}
```

Run it. NOTE: the locked `jumpgate-py/Cargo.toml` enables `extension-module` UNCONDITIONALLY, so this `cargo test -p jumpgate-py` invocation is NOT guaranteed to link as written on Linux — `extension-module` suppresses libpython linking and the test binary may fail to link. Treat this as a real build concern: prefer exercising the pure `decode_action` logic via tests that avoid the pyo3/libpython link where possible, and validate the full env surface through the Python smoke/determinism test (Task 18). Do not assume this command links cleanly out of the box:

```
cargo test -p jumpgate-py decode_action -- --nocolor
```

EXPECTED: a compile error `cannot find function `decode_action` in this scope` (red).

---

- [ ] **Step 3: Implement `decode_action` to make it GREEN (env.rs)**

Add the pure free function above the `#[cfg(test)]` module.

```rust
/// Decode one craft's flat action slice into the navigator macro-command.
/// `slice` is exactly `ACTION_DIM` long: `[dx, dy, dz, burn_budget]`.
/// Destination is EGO-RELATIVE (§7.2): `dest_abs = ego_pos + (dx,dy,dz)`.
/// `burn_budget < 0.0` decodes to `None`.
fn decode_action(slice: &[f32], ego_pos: Vec3, ego: CraftId) -> Command {
    let offset = Vec3::new(slice[0] as f64, slice[1] as f64, slice[2] as f64);
    let dest_abs = ego_pos.add(offset);
    let raw_budget = slice[3] as f64;
    let burn_budget = if raw_budget < 0.0 { None } else { Some(raw_budget) };
    Command {
        target: Target::Entity(EntityRef::Craft(ego)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(dest_abs),
            burn_budget,
        },
    }
}
```

Run:

```
cargo test -p jumpgate-py decode_action -- --nocolor
```

EXPECTED: `test result: ok. 2 passed; 0 failed` (green).

---

- [ ] **Step 4: Write a FAILING unit test for `compute_reward`, then implement it (env.rs)**

Add these two tests inside the existing `#[cfg(test)] mod tests`.

```rust
    #[test]
    fn compute_reward_penalizes_fuel_and_time() {
        // spent 2.0 fuel, dt = 1.0 day, not arrived
        let r = compute_reward(10.0, 8.0, false, 1.0);
        assert!((r - (-2.0 - 0.001)).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn compute_reward_adds_arrival_bonus() {
        // no fuel spent, dt = 1.0, arrived
        let r = compute_reward(10.0, 10.0, true, 1.0);
        assert!((r - (1.0 - 0.001)).abs() < 1e-9, "got {r}");
    }
```

Run (red — `compute_reward` undefined):

```
cargo test -p jumpgate-py compute_reward -- --nocolor
```

EXPECTED: compile error `cannot find function `compute_reward``.

Now add the pure function above the test module (next to `decode_action`):

```rust
/// v1 fuel-constrained transfer reward (§7.4), computed in f64.
/// Penalize fuel spent and elapsed time; bonus on task success (Arrival).
fn compute_reward(prev_fuel: f64, cur_fuel: f64, arrived: bool, dt: f64) -> f64 {
    let fuel_spent = prev_fuel - cur_fuel;
    let arrival_bonus = if arrived { 1.0 } else { 0.0 };
    -fuel_spent - 0.001 * dt + arrival_bonus
}
```

Run:

```
cargo test -p jumpgate-py compute_reward -- --nocolor
```

EXPECTED: `test result: ok. 2 passed; 0 failed` (green).

---

- [ ] **Step 5: Add the config template + `#[pyclass] JumpgateEnv` with `new`/`reset`/`step`/getters (env.rs)**

Add the helper that builds the per-env `RunConfig` template and the full `#[pymethods]` block. These call only the pure fns from Steps 3–4 and contract methods (inherent `reset`/`step`/`project` + `StateView` accessors). Insert above the `#[cfg(test)]` module.

Note the borrow discipline in `step`: arrival events are copied into an owned `Vec<Event>` (cheap — `Event` is `Copy`) BEFORE the per-craft loop, releasing the immutable borrow of `self.worlds[env]` so the loop can re-borrow it for `craft_fuel` / `project` without a borrow-checker conflict.

```rust
/// Build the v1 scenario config: one central star + `num_craft` identical craft.
/// `master_seed` is overwritten per-env in `reset`.
fn config_template(num_craft: usize) -> RunConfig {
    let star = BodyInit {
        mass: 1.0, // 1 M_sun in canonical units
        elements: OrbitalElements {
            a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0,
        },
    };
    let spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-7,
        base_exhaust_velocity: 1.0e-3,
        base_fuel_capacity: 1.0e-9,
    };
    let craft = (0..num_craft)
        .map(|_| CraftInit {
            spec: spec.clone(),
            pos: Vec3::new(1.0, 0.0, 0.0),   // 1 AU from the star
            vel: Vec3::new(0.0, 0.0172, 0.0), // ~circular at 1 AU: sqrt(G_CANONICAL) AU/day
            fuel_mass: 1.0e-9,
        })
        .collect();
    RunConfig {
        master_seed: 0,
        dt: Dt::new(1.0),
        softening: 1.0e-4,
        // accel_ref calibrated to Task-8 reference accel: gravity at 1 AU
        // from 1 M_sun is ~2.96e-4 AU/day^2, so 3.0e-4 keeps substeps near 1 at
        // cruise and escalates only on close approach (not the saturating 1e-6).
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        ephemeris_window: 100_000,
        bodies: vec![star],
        craft,
    }
}

/// Vectorized Gymnasium env: `num_envs` independent `World`s, each with
/// `num_craft` craft. Writes frame-relative obs / per-craft reward / terminated /
/// truncated into caller-provided numpy buffers (one memcpy per step, §7.3).
#[pyclass]
pub struct JumpgateEnv {
    worlds: Vec<World>,
    prev_fuel: Vec<f64>, // (num_envs * num_craft) snapshot for reward
    num_envs: usize,
    num_craft: usize,
    obs_dim: usize,
    action_dim: usize,
    time_limit: u64, // truncation horizon in ticks
    template: RunConfig,
}

#[pymethods]
impl JumpgateEnv {
    #[new]
    fn new(num_envs: usize, num_craft: usize) -> Self {
        let template = config_template(num_craft);
        let worlds = (0..num_envs)
            .map(|i| {
                let mut cfg = template.clone();
                cfg.master_seed = i as u64; // overwritten in reset
                let (w, _hash) = World::reset(cfg);
                w
            })
            .collect();
        JumpgateEnv {
            worlds,
            prev_fuel: vec![0.0; num_envs * num_craft],
            num_envs,
            num_craft,
            obs_dim: OBS_DIM,
            action_dim: ACTION_DIM,
            time_limit: 1000,
            template,
        }
    }

    #[getter]
    fn obs_dim(&self) -> usize {
        self.obs_dim
    }

    #[getter]
    fn action_dim(&self) -> usize {
        self.action_dim
    }

    /// Rebuild every world with `seed` (the gym seed BECOMES the master seed,
    /// distinct per env), then write the initial obs into `out_obs`.
    fn reset(&mut self, seed: u64, mut out_obs: PyReadwriteArray1<f32>) {
        let out = out_obs.as_slice_mut().expect("out_obs must be contiguous");
        for env in 0..self.num_envs {
            let mut cfg = self.template.clone();
            cfg.master_seed = seed.wrapping_add(env as u64);
            let (world, _hash) = World::reset(cfg);
            self.worlds[env] = world;

            let view = self.worlds[env].project(&FullObserver);
            let ids = self.worlds[env].craft_ids();
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let obs_base = flat * self.obs_dim;
                write_obs_frame_relative(
                    &view,
                    ids[craft],
                    &mut out[obs_base..obs_base + self.obs_dim],
                );
                self.prev_fuel[flat] =
                    self.worlds[env].craft_fuel(ids[craft]).unwrap_or(0.0);
            }
        }
    }

    /// Decode per-craft actions, advance each world one tick, then write
    /// obs / reward / terminated / truncated. `terminated` = Arrival (success);
    /// `truncated` = time-limit; kept DISTINCT (§7.3). No 5-tuple in Rust.
    fn step(
        &mut self,
        action: PyReadonlyArray1<f32>,
        mut out_obs: PyReadwriteArray1<f32>,
        mut out_reward: PyReadwriteArray1<f32>,
        mut out_terminated: PyReadwriteArray1<bool>,
        mut out_truncated: PyReadwriteArray1<bool>,
    ) {
        let act = action.as_slice().expect("action must be contiguous");
        let obs = out_obs.as_slice_mut().expect("out_obs contiguous");
        let rew = out_reward.as_slice_mut().expect("out_reward contiguous");
        let term = out_terminated.as_slice_mut().expect("out_terminated contiguous");
        let trunc = out_truncated.as_slice_mut().expect("out_truncated contiguous");

        for env in 0..self.num_envs {
            let ids = self.worlds[env].craft_ids();

            // Decode one command per craft from the flat action buffer.
            let mut cmds: Vec<Command> = Vec::with_capacity(self.num_craft);
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let a_base = flat * self.action_dim;
                let ego_pos = self.worlds[env].craft_pos(ids[craft]).unwrap_or(Vec3::ZERO);
                cmds.push(decode_action(
                    &act[a_base..a_base + self.action_dim],
                    ego_pos,
                    ids[craft],
                ));
            }

            let before_tick = self.worlds[env].tick();
            self.worlds[env].step(&mut cmds);
            let dt = self.worlds[env].dt().get();
            let now_tick = self.worlds[env].tick();

            // Copy arrival-window events into an OWNED Vec BEFORE the per-craft
            // loop. `recent_events` returns `&[Event]` borrowed from
            // `self.worlds[env]`; collecting to owned (Event is Copy) releases
            // that immutable borrow so the loop below can re-borrow the world
            // for `craft_fuel` / `project` without a borrow-checker conflict.
            let arrivals: Vec<Event> = self.worlds[env].recent_events(before_tick).to_owned();

            let view = self.worlds[env].project(&FullObserver);
            let ids = self.worlds[env].craft_ids();
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let cur_fuel = self.worlds[env].craft_fuel(ids[craft]).unwrap_or(0.0);

                let arrived = arrivals.iter().any(|e| {
                    matches!(e.kind, EventKind::Arrival { craft: c, .. } if c == ids[craft])
                });

                let r = compute_reward(self.prev_fuel[flat], cur_fuel, arrived, dt);
                self.prev_fuel[flat] = cur_fuel;

                let obs_base = flat * self.obs_dim;
                write_obs_frame_relative(
                    &view,
                    ids[craft],
                    &mut obs[obs_base..obs_base + self.obs_dim],
                );
                rew[flat] = r as f32;
                term[flat] = arrived;
                trunc[flat] = now_tick.0 >= self.time_limit;
            }
        }
    }
}
```

This step adds no test; it is verified to compile in Step 6's build. (The pure-fn unit tests from Steps 2–4 still pass and are not touched.)

Re-run the unit tests to confirm nothing regressed:

```
cargo test -p jumpgate-py -- --nocolor
```

EXPECTED: `test result: ok. 4 passed; 0 failed`.

---

- [ ] **Step 6: Build the extension with maturin (integration verification, not a red/green cycle)**

Confirm the `#[pymethods]` FFI surface compiles and links against libpython through maturin, and that the class is importable.

```
/home/john/jumpgate/archive/.venv/bin/python -m maturin develop --release -m crates/jumpgate-py/Cargo.toml
```

> **`--release` is load-bearing for Tier B (spec §6).** `maturin develop` defaults to a DEBUG build (`codegen-units=256`, `opt-level=0`), which never gets the workspace `[profile.release]` FP-determinism profile (`codegen-units=1`, no fast-math). The cdylib that runs RL training and any core-golden cross-check MUST be the release build, or per-tick hashes pinned from the core release tests will not match the training binary (Tier B is same-binary only). Use a plain debug `maturin develop` only for fast non-determinism iteration; all reproducibility assertions run against `--release`.

EXPECTED: ends with `📦 Built wheel ...` then `🛠 Installed jumpgate-...` and exit code 0.

Then a one-line import + construct smoke check (`obs_dim` is `OBS_DIM` from Task 16; `action_dim` is 4):

```
/home/john/jumpgate/archive/.venv/bin/python -c "import jumpgate._native as n; e=n.JumpgateEnv(2,1); print(e.obs_dim, e.action_dim)"
```

EXPECTED: prints `<OBS_DIM> 4` (the `OBS_DIM` value defined in Task 16; `action_dim` is 4), exit code 0.

---

- [ ] **Step 7: Commit**

```
git add crates/jumpgate-py/src/env.rs crates/jumpgate-py/src/lib.rs
git commit -m "$(cat <<'EOF'
Add JumpgateEnv PyO3 gym facade (caller buffers, per-craft, 5-tuple seam)

JumpgateEnv holds Vec<World> (one per env); reset(seed) rebuilds each world
with a per-env master seed and writes initial frame-relative obs; step(action)
decodes a per-craft navigator macro-action into Command{Destination}, advances
each world, and writes obs/reward/terminated/truncated into caller buffers.
terminated (Arrival) and truncated (time-limit) kept distinct (§7.3).
OBS_DIM / write_obs_frame_relative are imported from this crate's obs module
(Task 16), not jumpgate_core. StateView is in scope for the World accessors.
decode_action / compute_reward are pure free fns, unit-tested without the GIL.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

EXPECTED: `2 files changed`, commit created on the current feature branch (branch first if on `main`).


---

### Task 18: Python gym wrapper + smoke/determinism test + maturin build

A `gymnasium.Env` subclass wrapping the native `JumpgateEnv` (from Task 17), plus a Python test asserting the Gymnasium 5-tuple shapes/dtypes and same-seed-identical-obs reproducibility *through the binding* (Tier-B determinism, §8). This is the end-to-end DRL-thesis validation surface.

**Depends on Task 17** (the `JumpgateEnv` `#[pyclass]` with `reset(seed, out_obs) -> None`, `step(action, out_obs, out_reward, out_terminated, out_truncated) -> None`, and `obs_dim`/`action_dim` getters; the `crates/jumpgate-py/Cargo.toml` cdylib crate; `crates/jumpgate-py/src/lib.rs` `#[pymodule] _native`).

**Contract types in play:** `JumpgateEnv` (native, from Task 17). The native env **writes into caller-provided buffers and returns nothing** (§7.3 FFI rule); the wrapper allocates the flat buffers once and assembles the tuples.

> Load-bearing invariant (§8): the native rewrites the *same* `out_obs` buffer every call. The wrapper MUST return `self._obs_buf.copy()` (and the test MUST store copies), or the determinism test is vacuous — every list entry would alias one mutated buffer, so same-seed "passes" trivially and divergence can never be detected. Every obs returned to Python is a fresh copy.

> **v1 SEED-INVARIANCE (forward-debt, do NOT assert divergence).** v1 has **no seed-consuming path**: every initial condition is fixed config data (`RunConfig.bodies` / `RunConfig.craft` from the Task 17 scenario template), and `RunConfig.master_seed` only seeds the `RngStreams` (Intervention/Scenario), which nothing draws from in v1. Therefore two resets with *different* seeds produce **bit-identical** state, and a `different-seeds-diverge` assertion would fail unconditionally (matches the project memory note "VSL contest is seed-invariant"). The honest v1 determinism test is **same-config reproducibility**: `reset(42)` + N steps must reproduce identically across runs. Cross-seed divergence is **v2 forward-debt**: add a master-RNG positional perturbation in the core `World::reset` (draw a small per-craft position/velocity jitter from `RngStream::Scenario` and apply it to `CraftInit` before building `ShipStore`) so the seed actually moves initial state; only then does a divergence test become meaningful. The seam already exists (`RunConfig.master_seed` → `RngStreams::from_master`); v1 just leaves the draw unwired.

**Files**
- Create: `/home/john/jumpgate/python/jumpgate/__init__.py`
- Create: `/home/john/jumpgate/python/jumpgate/gym_env.py`
- Create: `/home/john/jumpgate/python/tests/test_gym_smoke.py`
- Modify: `/home/john/jumpgate/crates/jumpgate-py/pyproject.toml`
- Test: `/home/john/jumpgate/python/tests/test_gym_smoke.py`

---

- [ ] **Step 1: Pin the maturin mixed-layout config in `pyproject.toml`**

  Set the `[tool.maturin]` table so the compiled extension is `jumpgate._native` and the pure-Python source (the wrapper) lives under `python/`. This is the seam that lets `gym_env.py` do `from jumpgate._native import JumpgateEnv`. Edit `/home/john/jumpgate/crates/jumpgate-py/pyproject.toml` so its `[tool.maturin]` table reads exactly (leave `[build-system]`/`[project]` from Task 17 intact, but ensure `dependencies` includes gymnasium and numpy).

  TWO REALITY FIXES baked in below:
  1. `requires = ["maturin>=1.12,<2.0"]` — the confirmed-available toolchain is maturin 1.12 (`/home/john/jumpgate/archive/.venv/bin/python -m maturin --version` → `maturin 1.12.0`). Do **not** lower this bound to `>=1.7`; the plan's verified-facts pin is 1.12.
  2. NO `manifest-path` key under `[tool.maturin]`. `manifest-path` is a **CLI-only** option (`maturin develop -m/--manifest-path <PATH>`), not a `pyproject.toml` config key — maturin auto-discovers the manifest from the `pyproject.toml` location, and an unrecognized key is at best ignored / at worst a warning. It is supplied on the command line in Step 2 instead.

  ```toml
  [build-system]
  requires = ["maturin>=1.12,<2.0"]
  build-backend = "maturin"

  [project]
  name = "jumpgate"
  version = "0.1.0"
  requires-python = ">=3.12"
  dependencies = ["numpy>=2.0", "gymnasium>=1.2"]

  [tool.maturin]
  python-source = "../../python"
  module-name = "jumpgate._native"
  features = ["pyo3/extension-module"]
  ```

  Then verify the pure-Python package directory exists for maturin to pick up:

  ```bash
  mkdir -p /home/john/jumpgate/python/jumpgate /home/john/jumpgate/python/tests
  ```

  EXPECTED: no output (directories created); `pyproject.toml` `[tool.maturin]` has `module-name = "jumpgate._native"` and `python-source = "../../python"` and NO `manifest-path` key; `[build-system].requires` reads `maturin>=1.12,<2.0`.

- [ ] **Step 2: Build the native extension into the venv with maturin**

  The Python test cannot even import until the compiled extension is installed into the project venv. Build it now (the manifest is passed on the CLI — this is where `-m` belongs, not in `pyproject.toml`):

  ```bash
  /home/john/jumpgate/archive/.venv/bin/python -m maturin develop --release --manifest-path /home/john/jumpgate/crates/jumpgate-py/Cargo.toml
  ```

  > **`--release` is required for the determinism test below.** The same-seed bit-identical obs-sequence assertion (and any core-golden cross-check) is only meaningful on the Tier-B FP profile, which `maturin develop` does NOT apply by default (it builds debug). See spec §6.

  EXPECTED: ends with a line containing `Installed jumpgate-0.1.0` (or `🛠 Installed jumpgate`), exit code 0.

  Confirm the native symbols are importable:

  ```bash
  /home/john/jumpgate/archive/.venv/bin/python -c "from jumpgate._native import JumpgateEnv; e=JumpgateEnv(1,1); print(e.obs_dim, e.action_dim)"
  ```

  EXPECTED: two integers printed (e.g. `32 4`), exit code 0. (The exact values come from Task 17's obs/action schema; record them — call them `OBS_DIM` and `ACTION_DIM` below.)

- [ ] **Step 3: Write the failing smoke + determinism test (TDD red)**

  The wrapper module does not exist yet, so this test fails at import. The final test is `test_reset_is_deterministic` (same config, two runs, bit-identical) — NOT a different-seeds-diverge test (v1 is seed-invariant; see the forward-debt note above). Create `/home/john/jumpgate/python/tests/test_gym_smoke.py`:

  ```python
  import numpy as np
  import pytest
  import gymnasium as gym

  from jumpgate.gym_env import JumpgateGymEnv


  def _make():
      return JumpgateGymEnv(num_envs=1, num_craft=1)


  def _fixed_action(env):
      # Deterministic action so the run itself is the only variable across calls.
      return np.full(env.action_space.shape, 0.5, dtype=np.float32)


  def test_spaces_match_native():
      env = _make()
      assert isinstance(env.observation_space, gym.spaces.Box)
      assert isinstance(env.action_space, gym.spaces.Box)
      assert env.observation_space.dtype == np.float32
      assert env.action_space.dtype == np.float32
      assert env.observation_space.shape == (env._native.obs_dim,)
      assert env.action_space.shape == (env._native.action_dim,)
      env.close()


  def test_reset_returns_obs_info():
      env = _make()
      obs, info = env.reset(seed=7)
      assert isinstance(obs, np.ndarray)
      assert obs.dtype == np.float32
      assert obs.shape == env.observation_space.shape
      assert isinstance(info, dict)
      env.close()


  def test_step_returns_five_tuple_with_correct_types():
      env = _make()
      env.reset(seed=7)
      obs, reward, terminated, truncated, info = env.step(_fixed_action(env))
      assert isinstance(obs, np.ndarray) and obs.dtype == np.float32
      assert obs.shape == env.observation_space.shape
      assert isinstance(reward, float)
      assert isinstance(terminated, (bool, np.bool_))
      assert isinstance(truncated, (bool, np.bool_))
      assert isinstance(info, dict)
      env.close()


  def test_info_carries_reward_breakdown_not_obs():
      # Reward-component breakdown rides info, NEVER obs (spec 7.3).
      env = _make()
      env.reset(seed=7)
      _, _, _, _, info = env.step(_fixed_action(env))
      assert "reward_components" in info
      assert isinstance(info["reward_components"], dict)
      env.close()


  def _run_obs_sequence(seed, n_steps=64):
      env = JumpgateGymEnv(num_envs=1, num_craft=1)
      obs, _ = env.reset(seed=seed)
      seq = [obs.copy()]  # copy: native rewrites the same buffer in place
      action = np.full(env.action_space.shape, 0.5, dtype=np.float32)
      for _ in range(n_steps):
          obs, _, _, _, _ = env.step(action)
          seq.append(obs.copy())
      env.close()
      return np.stack(seq)


  def test_same_seed_bit_identical_obs_sequence():
      # Tier-B reproducibility through the binding (spec 8).
      a = _run_obs_sequence(seed=123)
      b = _run_obs_sequence(seed=123)
      assert np.array_equal(a, b), "same seed must yield a bit-identical obs sequence"


  def test_reset_is_deterministic():
      # v1 determinism contract (spec 8): a fixed config + a fixed action stream
      # must reproduce bit-identically across runs. seed=42 is the canonical case.
      #
      # NOTE (v1 seed-invariance / v2 forward-debt): v1 has no seed-consuming path
      # -- all initial conditions are fixed RunConfig data, and master_seed only
      # seeds RngStreams that nothing draws from in v1. So two DIFFERENT seeds also
      # produce identical state; a "different seeds diverge" assertion would fail
      # unconditionally (memory: "VSL contest is seed-invariant"). We therefore
      # assert reproducibility, NOT divergence. v2 unlocks divergence by drawing a
      # per-craft positional perturbation from RngStream::Scenario in core
      # World::reset; only then does cross-seed divergence become testable.
      a = _run_obs_sequence(seed=42)
      b = _run_obs_sequence(seed=42)
      assert np.array_equal(a, b), (
          "reset(42) + fixed action stream must reproduce a bit-identical obs "
          "sequence across runs (Tier-B determinism through the binding)"
      )
  ```

  Run it (expect failure on missing wrapper):

  ```bash
  /home/john/jumpgate/archive/.venv/bin/python -m pytest /home/john/jumpgate/python/tests/test_gym_smoke.py -q
  ```

  EXPECTED: collection error / `ModuleNotFoundError: No module named 'jumpgate.gym_env'` — all tests error (red).

- [ ] **Step 4: Create the package `__init__.py` re-exporting the wrapper**

  Create `/home/john/jumpgate/python/jumpgate/__init__.py`:

  ```python
  """jumpgate: deterministic-replayable Newtonian space sim, Gymnasium-wrapped."""

  from jumpgate.gym_env import JumpgateGymEnv

  __all__ = ["JumpgateGymEnv"]
  ```

  EXPECTED: file created, no command to run yet.

- [ ] **Step 5: Implement the `gymnasium.Env` wrapper (TDD green)**

  Allocate the flat buffers once (sized by the `num_envs * num_craft * dim` product per §7.3), define spaces from the native getters, and return copies. Create `/home/john/jumpgate/python/jumpgate/gym_env.py`:

  ```python
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
  ```

  EXPECTED: file created.

- [ ] **Step 6: Run the test suite (TDD green)**

  ```bash
  /home/john/jumpgate/archive/.venv/bin/python -m pytest /home/john/jumpgate/python/tests/test_gym_smoke.py -q
  ```

  EXPECTED: `6 passed` (final line `6 passed in N.NNs`), exit code 0. (Six tests: spaces, reset-returns, five-tuple, info-breakdown, same-seed-identical, reset-is-deterministic.)

  If `test_reset_is_deterministic` or `test_same_seed_bit_identical_obs_sequence` fails: this is a real Tier-B determinism break in the core, NOT a seeding gap — the same config + same action stream diverged across two runs. Bisect with the core's per-tick `state_hash` (spec §6) and the `replay_run` first-differing-tick report; do NOT weaken the test. (Do not re-add a different-seeds-diverge test: v1 is seed-invariant by design and such a test fails unconditionally — see the Step 3 forward-debt note.)

  If `test_info_carries_reward_breakdown_not_obs` fails because Task 17 exposes richer reward components, extend the `reward_components` dict in `gym_env.py` to mirror them (still in `info`, never `obs`).

- [ ] **Step 7: Confirm reproducibility end-to-end and commit**

  Run only the two determinism tests verbosely to record the headline result:

  ```bash
  /home/john/jumpgate/archive/.venv/bin/python -m pytest "/home/john/jumpgate/python/tests/test_gym_smoke.py::test_reset_is_deterministic" "/home/john/jumpgate/python/tests/test_gym_smoke.py::test_same_seed_bit_identical_obs_sequence" -v
  ```

  EXPECTED: `PASSED` for both `test_reset_is_deterministic` and `test_same_seed_bit_identical_obs_sequence`, exit code 0.

  Then commit (branch first if on `main`):

  ```bash
  git -C /home/john/jumpgate add python/jumpgate/__init__.py python/jumpgate/gym_env.py python/tests/test_gym_smoke.py crates/jumpgate-py/pyproject.toml
  git -C /home/john/jumpgate commit -m "$(cat <<'EOF'
  Task 18: Gymnasium wrapper + smoke/determinism test through the binding

  - JumpgateGymEnv: gymnasium.Env owning native JumpgateEnv; allocates flat
    buffers once; returns obs.copy() so collected sequences don't alias the
    in-place native buffer (Tier-B determinism, spec 8).
  - 5-tuple (obs f32, reward, terminated/truncated bool, info); reward
    breakdown in info, never obs (spec 7.3).
  - test_gym_smoke: shapes/dtypes, same-seed bit-identical obs sequence, and
    test_reset_is_deterministic (fixed config + fixed action stream reproduces
    bit-identically). v1 is seed-invariant (no seed-consuming path); cross-seed
    divergence is v2 forward-debt via a Scenario-RNG positional perturbation in
    core World::reset -- so NO different-seeds-diverge test (would fail
    unconditionally).
  - pyproject.toml: maturin>=1.12,<2.0 (confirmed toolchain); manifest-path is
    CLI-only, passed to `maturin develop -m`, not a [tool.maturin] key.
  - Built via `maturin develop --release --manifest-path crates/jumpgate-py/Cargo.toml`
    (`--release` required: Tier-B FP profile must reach the training cdylib, spec §6).

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

  EXPECTED: commit succeeds; `git -C /home/john/jumpgate log --oneline -1` shows the Task 18 message.