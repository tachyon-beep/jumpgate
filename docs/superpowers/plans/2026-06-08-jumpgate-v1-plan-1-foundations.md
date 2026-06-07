# jumpgate v1 — Plan 1: Foundations Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **READ FIRST:** `2026-06-08-jumpgate-v1-plan-0-contract-surface.md` — canonical signatures, workspace layout, and plan-level conventions (it wins on any conflict).

**Goal:** Build the jumpgate v1 rung-3 deterministic (Tier B) 3D Newtonian space core — on-rails bodies, gravity-feeling thrust/fuel/mass craft flown by an in-engine autopilot under a navigator macro-action — exposed as a reproducible Gymnasium env, with a per-tick state-hash replay-equivalence contract.

**Architecture:** Two crates: a pure-Rust `jumpgate-core` (`#![forbid(unsafe_code)]`) that is the sole authoritative writer (SoA stores, tick-indexed ephemeris, velocity-Verlet behind an Integrator trait with accel-keyed integer substepping, Tsiolkovsky variable-mass craft, autopilot guidance, one typed Command/Target ingestion path, a typed Event stream, FNV-1a state hashing, and log-replay), and a `jumpgate-py` PyO3 cdylib facade that writes frame-relative f32 observations into caller-provided buffers and presents the Gymnasium 5-tuple. All facades read through one `StateView` trait that exposes command+event history, not just physics; the engine is shaped (Target sum, Event typing, observer-parameterized projection, effective-param accessor, slot-map ids, Lod seam) so combat/upgrades/fog-of-war drop in without a contract break.

**Tech Stack:** Rust 2021 edition (rustc/cargo 1.95; edition 2021 is deliberate — `gen` is a reserved keyword in edition 2024 but is used as a struct field in `CraftId`/`BodyId`/slot-map generations); jumpgate-core deps: rand_chacha (pinned, ChaCha8Rng) + rand_core only; no serde/glam/rayon in the hashed path; hand-rolled f64 Vec3. jumpgate-py: pyo3 0.23 + numpy 0.23 (abi3-py312, extension-module). Build via /home/john/jumpgate/archive/.venv/bin/python -m maturin develop. Python test deps already present: gymnasium 1.2.3, numpy 2.4.6, torch 2.9.1. Workspace-root clippy.toml with disallowed-methods. FNV-1a hashing hand-rolled over f64::to_bits little-endian.

**This plan covers Tasks 1–6.** Prerequisite: (none — start here, after Plan 0).

---

### Task 1: Workspace + lint + toolchain scaffold

**Goal:** A buildable two-crate Cargo workspace with the determinism lint floor wired *and activated* so `cargo build`, `cargo test -p jumpgate-core`, and `cargo clippy --all-targets -- -D warnings` all pass on (nearly) empty crates. This is the foundation every later task builds on; it encodes the Tier-B lint bans (§6, §4.4 lever invariant rule 4), the pinned-dependency floor committed to ONE rand_chacha/rand_core/rand version family (`rand_chacha="=0.10.0"` / `rand_core="=0.10.1"` / `rand="=0.10.1"`, all sharing `rand_core 0.10.1`), the `#![forbid(unsafe_code)]` boundary for core, the FFI `unsafe`-allowed boundary for `jumpgate-py`, and the maturin/abi3-py312 packaging seam.

**Version-family single source of truth (PLAN-LEVEL — do not re-pin downstream):** The entire workspace targets the rand 0.10 family. `rand_chacha = "=0.10.0"` pulls `rand_core 0.10.1`; `rand = "=0.10.1"` *also* resolves `rand_core 0.10.1` (verified: `cargo tree` shows a single `rand_core v0.10.1` node for all three). Task 5 (rng.rs) MUST NOT run any `cargo update --precise` re-pin and MUST be written against this 0.10 API — there is no 0.3.x re-pin anywhere. The 0.10-family idioms every later task (5, 6, 7-12, 14, 17) uses are fixed here: the value trait is `rand_chacha::rand_core::Rng` (NOT `RngCore`), seeding is `SeedableRng::from_seed([u8;32])` or `SeedableRng::seed_from_u64(u64)`, and entropy/thread RNG is reached only via `rand::rng()` / `rand::make_rng()` / a `SysRng` source (all banned by clippy here).

**Contract types in play:** none (scaffold only). No `Vec3`/`World`/`Command` yet — those land in later tasks. The only "code" here is one trivial `pub fn` + a `#[test]` in core (proving the crate compiles and the harness runs) plus a second `#[cfg(test)]` module that *activates and proves* the determinism lint is live, and one trivial `#[pyfunction]`/`#[pymodule]` in py to prove the FFI/abi3 build works.

**Verified facts (probed against the live toolchain at draft time — empirically re-confirmed during repair; do not second-guess):**
- `rustc`/`cargo` `1.95.0` is the active stable toolchain on this machine.
- `rand_chacha = "=0.10.0"` resolves and pulls `rand_core 0.10.1`. `rand = "=0.10.1"` resolves the SAME `rand_core 0.10.1` (whole family aligned on one `rand_core` node — verified by `cargo tree`). In 0.10 the value RNG trait is `rand_chacha::rand_core::Rng` (importing `RngCore` and calling `next_u64` fails to compile: "trait `Rng` ... is implemented but not in scope"). Both `SeedableRng::from_seed([u8;32])` AND `SeedableRng::seed_from_u64(u64)` exist and compile (closes Task 5's OPEN FACT — the `from_seed`-over-hash fallback is NOT needed). `ChaCha8Rng`, `ChaCha20Rng`, and the `ChaChaRng = ChaCha20Rng` alias all exist.
- CRITICAL lint-path fact: in the 0.10 family the 0.9-era entropy entry points are RENAMED/REMOVED — `rand::thread_rng` → `rand::rng()`; `*::from_entropy` REMOVED (entropy now enters only via `from_rng(&mut SysRng)` / `make_rng`). Therefore banning the literal 0.9 paths (`rand::thread_rng`, `StdRng::from_entropy`, `ChaCha20Rng::from_entropy`) is INERT under the pinned family — clippy silently ignores method paths it cannot resolve, recreating the exact false-assurance bug this task exists to prevent. The paths that actually RESOLVE and FIRE in 0.10 are `rand::rng` and `rand::make_rng` (both verified: clippy emits `warning: use of a disallowed method 'rand::rng'` / `'rand::make_rng'`).
- A clippy `disallowed-methods` ban on a path from a crate the linted target does not depend on is INERT. `cargo clippy --all-targets` lints test/bench/example targets WITH dev-deps, so adding `rand` as a `[dev-dependencies]` and placing the canary violation in a `#[cfg(test)]` module makes the ban live (verified: with `rand="=0.10.1"` as a dev-dep ONLY and `rand::rng()`/`rand::make_rng()` inside a `#[cfg(test)] mod`, `cargo clippy --all-targets` fires both bans).
- `pyo3 = "=0.23"` resolves `0.23.5`; `numpy = "=0.23"` resolves `0.23.0`. With features `["extension-module","abi3-py312"]` a `cdylib`+`rlib` crate builds under plain `cargo build` with NO Python interpreter on PATH (abi3 stable ABI).
- maturin `develop`/`build`: `-m`/`--manifest-path` takes a **Cargo.toml** path ("Path to Cargo.toml" per `maturin develop --help`), NOT a pyproject.toml. The pyproject.toml is discovered relative to the crate; pass it the crate's `Cargo.toml`.

#### Files

- **Create:** `/home/john/jumpgate/Cargo.toml`
- **Create:** `/home/john/jumpgate/clippy.toml`
- **Create:** `/home/john/jumpgate/rust-toolchain.toml`
- **Create:** `/home/john/jumpgate/crates/jumpgate-core/Cargo.toml`
- **Create:** `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs`
- **Create:** `/home/john/jumpgate/crates/jumpgate-py/Cargo.toml`
- **Create:** `/home/john/jumpgate/crates/jumpgate-py/src/lib.rs`
- **Create:** `/home/john/jumpgate/crates/jumpgate-py/pyproject.toml`
- **Create:** `/home/john/jumpgate/python/jumpgate/__init__.py`
- **Modify:** `/home/john/jumpgate/.gitignore`
- **Test:** `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` (two inline `#[cfg(test)]` modules — the scaffold's failing-then-passing test, and the lint-activation canary)

#### Steps

- [ ] **Step 1: Write the root workspace manifest.**
  Create `/home/john/jumpgate/Cargo.toml` with `resolver = "3"`, both members, and a `[workspace.dependencies]` block pinning every shared dep with `=` exact versions (Tier-B reproducibility: floating versions can change FP codegen across `cargo update`). The whole rand family is pinned to ONE `rand_core` (0.10.1). `[workspace.lints]` is intentionally omitted — lint floor is enforced via `clippy.toml` + the per-crate `#![forbid(unsafe_code)]` attribute.
  ```toml
  [workspace]
  resolver = "3"
  members = [
      "crates/jumpgate-core",
      "crates/jumpgate-py",
  ]

  [workspace.package]
  edition = "2021"
  version = "0.1.0"
  license = "MIT OR Apache-2.0"

  [workspace.dependencies]
  # Pinned EXACT for Tier-B same-binary reproducibility, ALL on the rand 0.10
  # family so a single rand_core 0.10.1 node is shared (verified via cargo tree).
  # ChaCha8Rng from a pinned rand_chacha is version-stable; StdRng is not.
  # `rand` is the determinism LINT activator (dev-dep in core) — banning its
  # entropy entry points is inert unless the crate actually depends on it.
  rand_chacha = "=0.10.0"
  rand_core = "=0.10.1"
  rand = "=0.10.1"
  # PyO3 0.23 + numpy 0.23 per the shared type contract (verbatim). abi3-py312
  # so the cdylib needs no interpreter at build time.
  pyo3 = { version = "=0.23.5", default-features = false }
  numpy = "=0.23.0"

  [profile.release]
  # Deterministic-friendly release profile. No fast-math; default opt.
  lto = false
  codegen-units = 1
  ```

- [ ] **Step 2: Write the determinism lint floor (`clippy.toml`) with paths that ACTUALLY FIRE on the 0.10 family.**
  Create `/home/john/jumpgate/clippy.toml`. These are the §6 / §4.4-rule-4 bans: no wall-clock, no entropy-seeded or thread-local RNG, no env reads. The RNG paths are the 0.10-family names (`rand::rng`, `rand::make_rng`) — the 0.9-era `rand::thread_rng` / `*::from_entropy` do NOT resolve under the pinned family and would be silently ignored (false assurance). MEMORY note: this is a binary/lib mix; the lint is verified via `cargo clippy --all-targets`, NEVER `--lib`. All paths verified to resolve and fire under clippy 1.95 with the pinned deps present.
  ```toml
  # Tier-B determinism floor: ban every non-reproducible entropy/clock/env source.
  # Enforced via `cargo clippy --all-targets -- -D warnings`.
  #
  # RNG PATHS ARE 0.10-FAMILY NAMES. In rand 0.10 `thread_rng` was renamed to
  # `rand::rng()` and the `from_entropy` constructors were removed (entropy now
  # enters only via `from_rng(&mut SysRng)` / `make_rng`). Banning the old 0.9
  # names would be INERT (clippy ignores unresolvable paths) — a false floor.
  # Verified: clippy 1.95 emits "use of a disallowed method `rand::rng`" /
  # "`rand::make_rng`" on a violation; the std bans fire identically.
  disallowed-methods = [
      { path = "std::time::SystemTime::now", reason = "wall clock breaks Tier-B replay; derive time from tick*dt" },
      { path = "std::time::Instant::now", reason = "wall clock breaks Tier-B replay; derive time from tick*dt" },
      { path = "rand::rng", reason = "thread-local entropy RNG (0.9 `thread_rng`) breaks replay; use a named ChaCha8Rng sub-stream seeded from the master u64" },
      { path = "rand::make_rng", reason = "entropy-seeded RNG (0.10 replacement for `from_entropy`) breaks replay; seed deterministically via from_seed/seed_from_u64" },
      { path = "std::env::var", reason = "env reads make runs config-dependent off the hashed run-config" },
      { path = "std::env::vars", reason = "env reads make runs config-dependent off the hashed run-config" },
  ]
  ```

- [ ] **Step 3: Pin the toolchain (`rust-toolchain.toml`).**
  Create `/home/john/jumpgate/rust-toolchain.toml`. Tier-B is "same-binary / same-machine bit-reproducible," so the channel is pinned to the exact version verified on this machine (`1.95.0`). Include `clippy` and `rustfmt` components.
  ```toml
  [toolchain]
  channel = "1.95.0"
  components = ["clippy", "rustfmt"]
  profile = "minimal"
  ```

- [ ] **Step 4: Write the core crate manifest — pinned RNG deps PLUS the lint-activator dev-dep.**
  Create `/home/john/jumpgate/crates/jumpgate-core/Cargo.toml`. Core's runtime deps are only the pinned RNG crates (the only third-party code allowed in the `#![forbid(unsafe_code)]` engine — no pyo3, numpy, or serde). `rand` is added as a **dev-dependency**: it is NOT used by engine code, but its presence in the test target is what makes the `rand::rng`/`rand::make_rng` clippy bans LIVE under `cargo clippy --all-targets` (a ban on a method from an undepended crate is inert). The canary test in Step 5b exercises this.
  ```toml
  [package]
  name = "jumpgate-core"
  edition.workspace = true
  version.workspace = true
  license.workspace = true
  description = "jumpgate deterministic Newtonian space core (Tier-B replayable)"

  [dependencies]
  rand_chacha = { workspace = true }
  rand_core = { workspace = true }

  [dev-dependencies]
  # Lint ACTIVATOR ONLY. Banning rand::rng / rand::make_rng (clippy.toml) is inert
  # unless this crate depends on `rand`. As a dev-dep, the ban goes live for test
  # targets under `cargo clippy --all-targets`; engine code never references rand.
  rand = { workspace = true }

  [lints.rust]
  unsafe_code = "forbid"
  ```

- [ ] **Step 5: Write the FAILING scaffold test in core `lib.rs`.**
  Create `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` with the crate-level `#![forbid(unsafe_code)]`, a trivial `pub fn`, and a test that asserts the WRONG value so we observe a real failure first (strict TDD).
  ```rust
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
          assert_eq!(scaffold_ok(), 2);
      }
  }
  ```

- [ ] **Step 5b: Add the lint-activation canary as a SEPARATE `#[cfg(test)]` module (initially commented, to prove it fires before silencing).**
  Append to `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` a second test module whose body deliberately calls the banned entropy constructors, so we can demonstrate the ban is LIVE, then comment the violating lines out (keeping the proof in-tree as documentation). This is the artifact that prevents the "inert ban / false determinism assurance" failure mode permanently.
  ```rust
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
  ```

- [ ] **Step 6: Run the core test and SEE IT FAIL.**
  ```
  cargo test -p jumpgate-core scaffold_compiles_and_runs -- --nocapture
  ```
  EXPECTED: a single test runs and fails with an assertion mismatch, e.g.
  ```
  ---- tests::scaffold_compiles_and_runs stdout ----
  assertion `left == right` failed
    left: 1
   right: 2
  test result: FAILED. 0 passed; 1 failed; 0 ignored; ...
  ```
  (Confirms the workspace compiles AND the harness actually executes the test.)

- [ ] **Step 7: Make the test pass (minimal fix).**
  Edit the assertion in `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` to the correct expected value.
  ```rust
      #[test]
      fn scaffold_compiles_and_runs() {
          assert_eq!(scaffold_ok(), 1);
      }
  ```

- [ ] **Step 8: Run the core test and SEE IT PASS.**
  ```
  cargo test -p jumpgate-core scaffold_compiles_and_runs -- --nocapture
  ```
  EXPECTED:
  ```
  test tests::scaffold_compiles_and_runs ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] **Step 8b: PROVE the lint ban is live (one-off verification, then revert).**
  Temporarily uncomment the two banned lines in `lint_activation_canary` and run the floor; observe both bans fire as errors; then re-comment so the gate passes.
  ```
  cargo clippy --all-targets -- -D warnings
  ```
  EXPECTED (with the lines uncommented): exit non-zero with
  ```
  error: use of a disallowed method `rand::rng`
  error: use of a disallowed method `rand::make_rng`
  ```
  Re-comment both lines (restore Step 5b state). This one-off proves the dev-dep activation works; the committed tree keeps them commented so Step 15 passes clean.

- [ ] **Step 9: Write the py crate manifest (FFI / unsafe-allowed boundary).**
  Create `/home/john/jumpgate/crates/jumpgate-py/Cargo.toml`. This crate is `cdylib + rlib`, depends on `jumpgate-core` plus pyo3/numpy with the FFI features, and DELIBERATELY has no `unsafe_code = "forbid"` lint (PyO3 codegen requires `unsafe`). `extension-module` + `abi3-py312` let it build with no interpreter on PATH.
  ```toml
  [package]
  name = "jumpgate-py"
  edition.workspace = true
  version.workspace = true
  license.workspace = true
  description = "jumpgate PyO3/maturin ML+gym facade (FFI; unsafe allowed here, not in core)"

  [lib]
  name = "jumpgate"
  crate-type = ["cdylib", "rlib"]

  [dependencies]
  jumpgate-core = { path = "../jumpgate-core" }
  pyo3 = { workspace = true, features = ["extension-module", "abi3-py312"] }
  numpy = { workspace = true }
  ```

- [ ] **Step 10: Write the py crate `lib.rs` with the `_native` pymodule stub.**
  Create `/home/john/jumpgate/crates/jumpgate-py/src/lib.rs`. The module name MUST be `_native` (matches `pyproject.toml` `module-name = "jumpgate._native"`). A single `#[pyfunction]` proves the FFI/abi3 path links; `JumpgateEnv` lands in the gym-binding task.
  ```rust
  //! jumpgate-py — PyO3/maturin ML + Gymnasium facade over `jumpgate-core`.
  //!
  //! This crate is the ONLY place `unsafe` is permitted (PyO3 FFI codegen). The
  //! core engine stays `#![forbid(unsafe_code)]`. The native module is named
  //! `_native`; Python imports it as `jumpgate._native`. `JumpgateEnv` and the
  //! frame-relative obs path arrive in the gym-binding task.
  use pyo3::prelude::*;

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
      Ok(())
  }
  ```

- [ ] **Step 11: Write the maturin packaging config (`pyproject.toml`).**
  Create `/home/john/jumpgate/crates/jumpgate-py/pyproject.toml`. Maturin backend; native module `jumpgate._native`; `abi3-py312`; pure-Python source lives at the repo `python/` dir relative to this manifest (`../../python`).
  ```toml
  [build-system]
  requires = ["maturin>=1.12,<2.0"]
  build-backend = "maturin"

  [project]
  name = "jumpgate"
  version = "0.1.0"
  description = "jumpgate deterministic Newtonian space sim + Gymnasium env"
  requires-python = ">=3.12"
  classifiers = [
      "Programming Language :: Rust",
      "Programming Language :: Python :: 3.12",
  ]

  [tool.maturin]
  module-name = "jumpgate._native"
  python-source = "../../python"
  features = ["pyo3/extension-module"]
  ```

- [ ] **Step 12: Create the pure-Python package stub.**
  Create `/home/john/jumpgate/python/jumpgate/__init__.py`. Re-exports the native module so `import jumpgate; jumpgate._native` and `from jumpgate import _native` both work.
  ```python
  """jumpgate — deterministic Newtonian space sim with a Gymnasium env.

  The native engine is the compiled extension `jumpgate._native` (built by
  maturin from crates/jumpgate-py). The Gymnasium wrapper (`gym_env.py`)
  arrives in the gym-binding task.
  """
  from . import _native  # noqa: F401  (re-export; built by maturin)

  __all__ = ["_native"]
  ```

- [ ] **Step 13: Extend `.gitignore` for Rust + Python build artifacts.**
  The repo `.gitignore` currently contains only `archive/`. Use Edit on `/home/john/jumpgate/.gitignore` to replace its single line with the full set (preserve `archive/`).
  Replace the existing content `archive/` with:
  ```gitignore
  archive/

  # Rust
  /target/
  **/target/
  Cargo.lock.orig

  # Python / maturin
  __pycache__/
  *.py[cod]
  *.so
  *.pyd
  .pytest_cache/
  *.egg-info/
  build/
  dist/
  .venv/
  ```
  Note: `Cargo.lock` is intentionally NOT ignored — it is committed (pinning the lock is part of the Tier-B reproducibility floor).

- [ ] **Step 14: Verify the whole workspace builds (both crates, abi3 link).**
  ```
  cargo build --workspace
  ```
  EXPECTED: both crates compile; final line resembles
  ```
  Finished `dev` profile [unoptimized + debuginfo] target(s) in ...s
  ```
  with no errors. (Verified at draft time: pyo3 0.23.5 + numpy 0.23.0 abi3-py312 builds with no Python interpreter on PATH.)

- [ ] **Step 15: Verify the determinism lint floor passes on the scaffold (canary commented).**
  MEMORY: binary/lib mix — lint via `--all-targets`, never `--lib`/`--bins`.
  ```
  cargo clippy --all-targets -- -D warnings
  ```
  EXPECTED: exit 0, final line
  ```
  Finished `dev` profile [unoptimized + debuginfo] target(s) in ...s
  ```
  with no `disallowed method` and no warning-as-error output. (The committed canary lines are commented, so the floor is satisfied; Step 8b already proved they fire when active. Later tasks that touch RNG/time inherit the enforcement automatically.)

- [ ] **Step 16: Verify the full core test suite is green.**
  ```
  cargo test -p jumpgate-core
  ```
  EXPECTED:
  ```
  test tests::scaffold_compiles_and_runs ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
  (The `lint_activation_canary` module contains no `#[test]` fns, so the count stays 1.)

- [ ] **Step 17: Verify the maturin build wiring (recommended end-to-end check).**
  Use the project venv (do not create a new one). Pass maturin the crate's **Cargo.toml** via `-m` (`--manifest-path` = "Path to Cargo.toml"; passing pyproject.toml here is an error). This confirms `pyproject.toml`, `module-name`, and `python-source` are wired correctly end-to-end.
  ```
  /home/john/jumpgate/archive/.venv/bin/python -m maturin develop -m /home/john/jumpgate/crates/jumpgate-py/Cargo.toml
  ```
  EXPECTED: ends with a line like `📦 Built wheel ...` / `🛠 Installed jumpgate-0.1.0`. Then confirm the import + FFI round-trip:
  ```
  /home/john/jumpgate/archive/.venv/bin/python -c "import jumpgate; print(jumpgate._native.scaffold_ok())"
  ```
  EXPECTED output: `1`
  (If maturin reports a target-dir lock or interpreter mismatch, this step is non-blocking for the scaffold gate — Steps 14/15/16 are the binding acceptance gates. Record the failure for the gym-binding task rather than reworking the scaffold.)

- [ ] **Step 18: Commit the scaffold.**
  Confirm a clean tree of only the intended files, then commit.
  ```
  cd /home/john/jumpgate && git add Cargo.toml Cargo.lock clippy.toml rust-toolchain.toml crates/ python/ .gitignore && git status --short
  ```
  EXPECTED: only the eight created files + `Cargo.lock` + modified `.gitignore` staged; `archive/` and `target/` NOT listed.
  ```
  git commit -m "$(cat <<'EOF'
  Task 1: workspace + lint + toolchain scaffold

  Two-crate Cargo workspace (jumpgate-core #![forbid(unsafe_code)];
  jumpgate-py PyO3/maturin cdylib, unsafe allowed for FFI). Pins the
  Tier-B determinism floor on ONE rand 0.10 family (rand_chacha=0.10.0,
  rand_core=0.10.1, rand=0.10.1 sharing rand_core 0.10.1). clippy
  disallowed-methods bans SystemTime/Instant/env reads plus the 0.10
  entropy entry points rand::rng / rand::make_rng (0.9 thread_rng/
  from_entropy don't resolve here). `rand` added as a core dev-dep so
  those bans actually fire under --all-targets (verified). Toolchain
  pinned to 1.95.0. cargo build/test -p jumpgate-core/clippy
  --all-targets all green.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: commit succeeds with the file list shown.

#### Acceptance gate for Task 1
All four must hold:
1. `cargo build --workspace` → Finished, no errors.
2. `cargo clippy --all-targets -- -D warnings` → exit 0 with the canary commented; AND (one-off, Step 8b) uncommenting the canary makes the `rand::rng` / `rand::make_rng` bans fire as errors — proving the entropy ban is LIVE, not inert.
3. `cargo test -p jumpgate-core` → `1 passed; 0 failed`.
4. `cargo tree -p jumpgate-core` shows a single `rand_core v0.10.1` node (no second rand_core version in the lockfile) — the whole rand family is aligned.

Carry-forward for later tasks:
- The `scaffold_ok()` fns in both crates are placeholders to be deleted as real modules land (math.rs first, etc.); the `_native` module name and `python-source = "../../python"` are now load-bearing contract points the gym-binding task must not rename.
- The rand 0.10 family pin and the `Rng` (not `RngCore`) trait + `from_seed`/`seed_from_u64` API are the SINGLE source of truth: Task 5 (rng.rs) writes against this API and runs NO `cargo update --precise` re-pin; Tasks 6/7-12/14/17 must use 0.10 idioms.
- The `rand` dev-dep + `lint_activation_canary` module are load-bearing: do not remove them without re-proving the entropy ban another way.
- Module-ordering rule for every later task: `lib.rs` declares `pub mod` ONLY for files that exist at that task's completion. The acyclic target order is `math -> time -> types -> ids -> config -> contract -> stores -> ...` (the seam primitives `Lod`/`NavDest`/`Target`/`EntityRef`/`CommandKind` live in a `types.rs` created BEFORE `stores.rs` consumes them and BEFORE `contract.rs` builds traits on them). A standalone cross-task contract-surface document (every type/method/const that crosses a task boundary, with the providing task obligated to define+test each downstream-called method) is produced BEFORE Task 3; it governs the SlotMap / ShipStore / View-accessor / commands_flat surfaces. The `Integrator` trait is defined exactly ONCE in `contract.rs`; `integrator.rs` writes only `impl crate::contract::Integrator for ...` (no second trait, no re-export). The FNV-1a hash has a single authoritative `HASH_FIELD_ORDER` enumeration + `HASH_VERSION` const + a golden zero-state hash test (so the Task 11 prev_fuel/prev_inside_dest additions can't silently shift the hash). The `Lod` seam ships exercised: a `Wake` `EventKind` variant + a (trivial) Lod-dispatch branch in `World::step`. None of these belong in Task 1 — listed here so the providing tasks honor them.


---

### Task 2: Core math: f64 Vec3 + canonical units

**Goal:** A hand-rolled `f64` `Vec3` with the full op set and a fixed-order `to_bits`, plus canonical-unit constants, all unit-tested. No glam, no unsafe.

**Depends on:** Task 1 (workspace + `crates/jumpgate-core/` with `src/lib.rs` carrying `#![forbid(unsafe_code)]`).

**Contract types in play:** `Vec3`.

#### Files

- **Create:** `crates/jumpgate-core/src/math.rs`
- **Modify:** `crates/jumpgate-core/src/lib.rs` (add `pub mod math;`)
- **Test:** `crates/jumpgate-core/src/math.rs` (inline `#[cfg(test)] mod tests`)

---

- [ ] **Step 1: Create `math.rs` with the failing test module only (no impl yet).**

  Create `crates/jumpgate-core/src/math.rs` with the full test suite but a deliberately empty type surface so it fails to compile (TDD: the test names the contract before the code exists). Write the file:

  ```rust
  //! Core math: hand-rolled f64 `Vec3` and canonical-unit constants.
  //!
  //! Vec3 is hand-rolled (not glam) so the crate stays `#![forbid(unsafe_code)]`
  //! and so `to_bits()` owns a FIXED field order (x,y,z) for the Tier-B state hash.
  //! f64 throughout: no SIMD, no mantissa loss at solar-system scale. The only
  //! precision boundary is the f32 OBSERVATION downcast, which lives in jumpgate-py.

  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Vec3 {
      pub x: f64,
      pub y: f64,
      pub z: f64,
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn new_and_fields() {
          let v = Vec3::new(1.0, 2.0, 3.0);
          assert_eq!(v.x, 1.0);
          assert_eq!(v.y, 2.0);
          assert_eq!(v.z, 3.0);
      }

      #[test]
      fn zero_const() {
          assert_eq!(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.0));
      }

      #[test]
      fn add_sub_roundtrip() {
          let a = Vec3::new(1.0, 2.0, 3.0);
          let b = Vec3::new(10.0, 20.0, 30.0);
          assert_eq!(a.add(b), Vec3::new(11.0, 22.0, 33.0));
          assert_eq!(a.add(b).sub(b), a);
      }

      #[test]
      fn scale_scales_each_component() {
          let a = Vec3::new(1.0, -2.0, 3.0);
          assert_eq!(a.scale(2.0), Vec3::new(2.0, -4.0, 6.0));
          assert_eq!(a.scale(0.0), Vec3::ZERO);
      }

      #[test]
      fn dot_known_value() {
          let a = Vec3::new(1.0, 2.0, 3.0);
          let b = Vec3::new(4.0, -5.0, 6.0);
          // 1*4 + 2*-5 + 3*6 = 4 - 10 + 18 = 12
          assert_eq!(a.dot(b), 12.0);
      }

      #[test]
      fn length_three_four_zero_is_five() {
          let v = Vec3::new(3.0, 4.0, 0.0);
          assert_eq!(v.length_sq(), 25.0);
          assert_eq!(v.length(), 5.0);
      }

      #[test]
      fn normalize_unit_length() {
          let v = Vec3::new(3.0, 4.0, 0.0).normalize_or_zero();
          assert!((v.length() - 1.0).abs() < 1e-12);
          assert_eq!(v, Vec3::new(0.6, 0.8, 0.0));
      }

      #[test]
      fn normalize_of_zero_is_zero() {
          assert_eq!(Vec3::ZERO.normalize_or_zero(), Vec3::ZERO);
          // a vector below the epsilon floor also returns ZERO (no NaN)
          let tiny = Vec3::new(1e-300, 0.0, 0.0);
          assert_eq!(tiny.normalize_or_zero(), Vec3::ZERO);
      }

      #[test]
      fn to_bits_field_order_is_x_then_y_then_z() {
          let v = Vec3::new(1.0, 2.0, 3.0);
          assert_eq!(
              v.to_bits(),
              [1.0f64.to_bits(), 2.0f64.to_bits(), 3.0f64.to_bits()]
          );
      }

      #[test]
      fn to_bits_distinguishes_signed_zero() {
          // f64 to_bits preserves the sign bit: -0.0 != +0.0 in the hash encoding.
          let pos = Vec3::new(0.0, 0.0, 0.0).to_bits();
          let neg = Vec3::new(-0.0, 0.0, 0.0).to_bits();
          assert_ne!(pos[0], neg[0]);
      }

      #[test]
      fn g_canonical_is_gaussian_constant_squared() {
          // k = 0.01720209895 (Gaussian grav const); G_CANONICAL = k^2.
          assert_eq!(G_CANONICAL, 0.01720209895_f64 * 0.01720209895_f64);
      }
  }
  ```

  Then run it and confirm it fails to compile (the impl, `ZERO`, methods, and `G_CANONICAL` do not exist yet):

  ```
  cargo test -p jumpgate-core math -- --nocolor
  ```

  EXPECTED: compile errors such as `error[E0599]: no function or associated item named 'new' found for struct 'Vec3'` and `cannot find value 'G_CANONICAL' in this scope`. (Test compilation fails — this is the red state.)

- [ ] **Step 2: Wire the module into `lib.rs`.**

  Add the module declaration so the crate sees `math.rs`. Edit `crates/jumpgate-core/src/lib.rs` to add the line (keep the existing `#![forbid(unsafe_code)]` crate attribute at the top):

  ```rust
  pub mod math;
  ```

  Re-run to confirm the failure is now the missing `Vec3` impl (module resolves, body does not):

  ```
  cargo test -p jumpgate-core math -- --nocolor
  ```

  EXPECTED: still failing to compile, now with `no function or associated item named 'new'` / `cannot find value 'G_CANONICAL'` (NOT `file not found for module 'math'`). Still red.

- [ ] **Step 3: Implement the `Vec3` impl and constants (minimal code to pass).**

  In `crates/jumpgate-core/src/math.rs`, insert the impl block and the constants immediately after the `Vec3` struct definition (above the `#[cfg(test)]` module). Add this code:

  ```rust
  impl Vec3 {
      /// The zero vector. Associated const so `Vec3::ZERO` reads cleanly.
      pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };

      #[inline]
      pub fn new(x: f64, y: f64, z: f64) -> Vec3 {
          Vec3 { x, y, z }
      }

      #[inline]
      pub fn add(self, o: Vec3) -> Vec3 {
          Vec3 { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z }
      }

      #[inline]
      pub fn sub(self, o: Vec3) -> Vec3 {
          Vec3 { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z }
      }

      #[inline]
      pub fn scale(self, s: f64) -> Vec3 {
          Vec3 { x: self.x * s, y: self.y * s, z: self.z * s }
      }

      #[inline]
      pub fn dot(self, o: Vec3) -> f64 {
          self.x * o.x + self.y * o.y + self.z * o.z
      }

      #[inline]
      pub fn length_sq(self) -> f64 {
          self.dot(self)
      }

      #[inline]
      pub fn length(self) -> f64 {
          self.length_sq().sqrt()
      }

      /// Returns the unit vector, or `ZERO` if the length is below `NORMALIZE_EPS`
      /// (avoids dividing by ~0 and producing NaN/Inf).
      #[inline]
      pub fn normalize_or_zero(self) -> Vec3 {
          let len = self.length();
          if len < NORMALIZE_EPS {
              Vec3::ZERO
          } else {
              self.scale(1.0 / len)
          }
      }

      /// Fixed field order for hashing: x then y then z.
      #[inline]
      pub fn to_bits(self) -> [u64; 3] {
          [self.x.to_bits(), self.y.to_bits(), self.z.to_bits()]
      }
  }

  /// Length floor below which `normalize_or_zero` returns `ZERO` (NaN guard).
  const NORMALIZE_EPS: f64 = 1e-12;

  // ---- Canonical units (AU, M_sun, day). G folded so quantities sit near unity. ----

  /// Gravitational parameter in canonical units: AU^3 / (M_sun * day^2).
  /// Equals the square of the Gaussian gravitational constant k = 0.01720209895,
  /// i.e. the heliocentric G*M_sun expressed in (AU, M_sun, day).
  pub const G_CANONICAL: f64 = 0.01720209895_f64 * 0.01720209895_f64;

  /// One astronomical unit in metres (SI), for facade-boundary conversion only.
  pub const AU_IN_METERS: f64 = 1.495_978_707e11;
  /// One solar mass in kilograms (SI), for facade-boundary conversion only.
  pub const M_SUN_IN_KG: f64 = 1.988_47e30;
  /// One day in seconds (SI), for facade-boundary conversion only.
  pub const DAY_IN_SECONDS: f64 = 86_400.0;
  ```

  Run the math tests and confirm green:

  ```
  cargo test -p jumpgate-core math -- --nocolor
  ```

  EXPECTED: `test result: ok. 11 passed; 0 failed; 0 ignored` (the 11 tests from Step 1).

- [ ] **Step 4: Confirm the whole core crate still builds and lints clean (no unsafe, no disallowed methods).**

  Run the full crate test pass plus clippy across all targets (so the test module is linted, per the `clippy --all-targets` rule):

  ```
  cargo test -p jumpgate-core -- --nocolor && cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```

  EXPECTED: tests print `test result: ok.` for the crate; clippy finishes with `Finished` and no warnings/errors (no `#[forbid(unsafe_code)]` violation, no `disallowed-methods` hit — `Vec3` uses only arithmetic and `f64::sqrt`/`to_bits`).

- [ ] **Step 5: Commit.**

  Stage and commit the math module and the `lib.rs` wiring:

  ```
  git add crates/jumpgate-core/src/math.rs crates/jumpgate-core/src/lib.rs && git commit -m "$(cat <<'EOF'
  Add hand-rolled f64 Vec3 + canonical-unit constants

  Implements the contract Vec3 (ZERO, new, add, sub, scale, dot, length,
  length_sq, normalize_or_zero, to_bits in fixed x,y,z order) plus
  G_CANONICAL (Gaussian constant squared) and SI conversion constants for
  facade boundaries. No glam, no unsafe. 11 unit tests cover algebraic
  identities, length(3,4,0)==5, to_bits field order, signed-zero
  distinction, and zero-normalization NaN guard.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

  EXPECTED: a single commit recorded; `git status` clean for these two files.


---

### Task 3: Cross-task contract surface + time/ids/types/config primitives + FNV canonical anchor (hash.rs)

**Goal:** Close the systemic "task-local authoring" root cause and land the lower seam layers of the type contract so every downstream task has resolvable, acyclic dependencies and a single authoritative hash specification.

This task does five things:
1. **(Root-cause fix)** Produces a single **cross-task contract-surface document** listing every type/method/const that flows across task boundaries, with the providing task and the RULE that the providing task must define+test every method any downstream task calls.
2. Lands the **time primitives** (`Tick`, `Dt`, `sim_time`).
3. Lands the **id layer** (`CraftId`, `BodyId`, `SlotMap`) — moved earlier than the original plan so `types.rs` can resolve `EntityRef`.
4. Lands the **seam primitive types** in a new `types.rs` (`Lod`, `EntityRef`, `Target`, `NavDest`, `CommandKind`) so Task 4's `stores.rs` (`NavState{dest:NavDest}`, `lod:Vec<Lod>`) has its dependencies BEFORE `contract.rs` (Task 6) builds traits on top — breaking the stores<->contract cycle.
5. Lands the **single hashed `RunConfig`** (`config_hash`) AND the **authoritative per-tick STATE-hash specification** in `hash.rs`: `FnvHasher`, `HASH_MAGIC`, `HASH_FORMAT_VERSION`, a numbered `HASH_FIELD_ORDER` doc enumerating every hashed field with the task that introduces it, and a **golden-hash test** asserting a fixed input sequence hashes to a hardcoded value. This is the drift-lock anchor: when Task 11 adds `prev_fuel`/`prev_inside_dest` to the hash, the canonical order forces an explicit `HASH_FIELD_ORDER` edit and a golden-value bump, so the change cannot silently alter the hash uncaught.

**Module dependency order at this task's exit (acyclic):**
`math` (Tasks 1-2) -> `time` -> `ids` -> `types` -> `config` -> `hash`.

> **Ordering note (deviation from the literal fix-list sequence, justified):** the fix list states `math -> time -> types(new) -> ids -> config`. But the contract fixes `EntityRef = Craft(CraftId) | Body(BodyId)` and `NavDest = Position(Vec3) | Entity(EntityRef)`, so `types.rs` *cannot* compile before `ids.rs` exists. The only acyclic resolution is `ids` BEFORE `types`. `ids.rs` is therefore created in Task 3 (not Task 4 as in the original broken plan). `config.rs` imports only `math::Vec3` and `time::Dt` (it does NOT import `ids`), so it may follow `types` without creating a cycle. The cross-task doc (Step 0) records this reconciliation explicitly.

**Files:**
- Create: `docs/superpowers/specs/contract-surface.md`
- Create: `crates/jumpgate-core/src/time.rs`
- Create: `crates/jumpgate-core/src/ids.rs`
- Create: `crates/jumpgate-core/src/types.rs`
- Create: `crates/jumpgate-core/src/config.rs`
- Create: `crates/jumpgate-core/src/hash.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: inline `#[cfg(test)]` modules in `time.rs`, `ids.rs`, `types.rs`, `config.rs`, `hash.rs`

**Depends on Task 2** for `crate::math::Vec3` (with `Vec3::new`, `Vec3::ZERO`, and `Vec3::to_bits() -> [u64; 3]`). Do not redefine `Vec3`; import it.

**Starting state of `lib.rs` (reality check, corrected):** after Tasks 1-2, `crates/jumpgate-core/src/lib.rs` declares **only** `pub mod math;` (plus the crate-level `#![forbid(unsafe_code)]`). It does NOT declare `pub mod ids;` — that file does not exist yet and is created in THIS task. At the end of every step below, `lib.rs` declares only modules whose files already exist, so the workspace compiles at this task's exit.

---

- [ ] **Step 0: Write the cross-task contract-surface document (root-cause fix).**

  Create `docs/superpowers/specs/contract-surface.md`. This is the single source of truth for every symbol that crosses a task boundary, the task that PROVIDES it, and the consuming tasks. It encodes the RULE that closes the SlotMap/ShipStore/View-accessor/commands_flat gaps systemically.

  ````markdown
  # Jumpgate cross-task contract surface

  Authoritative list of every type, method, and const that flows across task
  boundaries. Derived verbatim from the SHARED TYPE CONTRACT. This document is
  the parent fix for the per-call-site gaps (SlotMap, ShipStore, View-accessor,
  commands_flat): it makes the dependency surface explicit so each task can be
  authored against the whole, not task-locally.

  ## RULE (binding on every task)

  The task that PROVIDES a symbol MUST define **every method any downstream task
  calls on that symbol**, and that providing task's own `#[cfg(test)]` suite MUST
  cover those methods. A method that a later task needs but the providing task
  did not implement+test is a contract gap and blocks the providing task's commit.

  ## Module dependency order (acyclic; enforced by lib.rs decl order)

  math -> time -> ids -> types -> config -> hash -> rng -> contract -> stores
       -> ephemeris -> integrator -> ship -> autopilot -> ingest -> events
       -> world -> replay -> (jumpgate-py)

  Reconciliation note: `EntityRef`/`NavDest` (in `types`) depend on
  `CraftId`/`BodyId` (in `ids`), so `ids` precedes `types`. `config` imports only
  `math` + `time`, so it follows `types` without a cycle. `contract` (Integrator,
  StateView, Command, Event, Lod re-export point) is defined ONCE and built on
  top of `types`/`ids`/`stores`; concrete impls (`integrator`, `world`) import
  from `contract` and never redeclare a same-shaped trait.

  ## Symbol -> providing task -> consumers

  | Symbol | Provided by | Key methods/fields downstream relies on | Consumers |
  |---|---|---|---|
  | `Vec3` | Task 2 (`math`) | `new`,`ZERO`,`add`,`sub`,`scale`,`dot`,`length`,`length_sq`,`normalize_or_zero`,`to_bits` | every physics/config/hash task |
  | `G_CANONICAL` | Task 2 (`math`) | const | integrator, ephemeris |
  | `Tick`,`Dt`,`sim_time` | Task 3 (`time`) | `Tick(u64)`; `Dt::new/get/bits`; `sim_time(Tick,Dt)` | config, hash, world, ephemeris, events, replay |
  | `CraftId`,`BodyId` | Task 3 (`ids`) | tuple `{slot,gen}`; `Ord`/`Hash` derives | types, stores, contract, world, hash, py |
  | `SlotMap<T>` | Task 3 (`ids`) | `new`,`len`,`is_empty`,`cursor`,`insert`,`get`,`remove`,`gen_of` | stores, world; `cursor()` is HASHED state |
  | `Lod` | Task 3 (`types`) | `Player`/`NpcInteraction`/`Nothing` | stores (`lod:Vec<Lod>`), contract, world dispatch |
  | `EntityRef`,`Target`,`NavDest`,`CommandKind` | Task 3 (`types`) | enum variants | contract (`Command`,`Event`), stores (`NavState`), ingest |
  | `BaseSpec`,`OrbitalElements`,`BodyInit`,`CraftInit`,`SubstepCfg`,`RunConfig`,`ConfigHash` | Task 3 (`config`) | `RunConfig::config_hash`; field access | world::reset, ephemeris, replay, py |
  | `FnvHasher`,`HASH_MAGIC`,`HASH_FORMAT_VERSION`,`HASH_FIELD_ORDER` | Task 3 (`hash`) | `new`,`write_u64`,`finish`; consts; canonical order | Task 13 `state_hash`, replay, world |
  | `RngStreams`,`RngStream` | Task 5 (`rng`) | `from_master`,`stream` | world |
  | `Command`,`Event`,`EventKind`,`command_sort_key`,`Integrator`,`StateView`,`Observer`,`FullObserver`,`View` | Task 6 (`contract`) | one definition each | integrator, ingest, events, world, py |
  | `NavState`,`ShipStore`,`BodyStore`,`Effective`,`effective_params` | Task 4 (`stores`) | field access; `effective_params(&BaseSpec)` | world, integrator, autopilot, ship |

  ## Hash-ownership invariant

  There are TWO distinct FNV-1a hashes, never sharing state or magic:
  - **config hash** (`RunConfig::config_hash`, Task 3 `config.rs`): hashes
    immutable initial conditions once. Uses a LOCAL fold with a `"CONFIG_1"` tag.
  - **per-tick STATE hash** (`state_hash`, Task 13 `hash.rs`): hashes evolving
    world state each tick via the shared `FnvHasher` seeded with `HASH_MAGIC`.
    Its canonical field order is `HASH_FIELD_ORDER` (Task 3, this task), and ANY
    task that adds a hashed field MUST append to `HASH_FIELD_ORDER`, bump
    `HASH_FORMAT_VERSION`, and update the golden-hash test.
  ````

  No build step here; this is a documentation deliverable. Verify it exists:

  ```
  test -f docs/superpowers/specs/contract-surface.md && echo "contract-surface present"
  ```

  EXPECTED: prints `contract-surface present`.

- [ ] **Step 1: Add the `time` module declaration to `lib.rs`.**

  Open `crates/jumpgate-core/src/lib.rs`. It currently declares only `pub mod math;`. Add directly after it:

  ```rust
  pub mod time;
  ```

  Do not build yet — the next step creates `time.rs`.

- [ ] **Step 2: Write a FAILING test + skeleton for `Tick`, `Dt`, `sim_time` in `time.rs`.**

  Create `crates/jumpgate-core/src/time.rs` with the type/signature skeleton (bodies `unimplemented!()`) and the test module:

  ```rust
  //! Time primitives. `tick: u64` is authoritative (spec §6); `dt` is fixed at
  //! init and stored as its u64 bit pattern; `sim_time = (tick as f64) * dt` is a
  //! DERIVED helper, never authoritative state, and `dt` is NEVER a step() arg.

  /// Authoritative integer tick. `sim_time` is derived from it where needed.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
  pub struct Tick(pub u64);

  /// Fixed timestep. Stores f64 but exposes its exact u64 bit pattern for hashing.
  #[derive(Clone, Copy, Debug)]
  pub struct Dt(f64);

  impl Dt {
      pub fn new(_dt: f64) -> Dt {
          unimplemented!()
      }
      pub fn get(self) -> f64 {
          unimplemented!()
      }
      pub fn bits(self) -> u64 {
          unimplemented!()
      }
  }

  /// Derived: (tick as f64) * dt. Computed only where needed, not stored.
  pub fn sim_time(_tick: Tick, _dt: Dt) -> f64 {
      unimplemented!()
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn dt_get_round_trips() {
          assert_eq!(Dt::new(0.5).get(), 0.5);
      }

      #[test]
      fn dt_bits_are_the_f64_bit_pattern() {
          assert_eq!(Dt::new(0.25).bits(), 0.25f64.to_bits());
      }

      #[test]
      fn dt_bits_distinguish_different_dts() {
          assert_ne!(Dt::new(0.25).bits(), Dt::new(0.5).bits());
      }

      #[test]
      fn sim_time_is_tick_times_dt() {
          let dt = Dt::new(0.5);
          assert_eq!(sim_time(Tick(0), dt), 0.0);
          assert_eq!(sim_time(Tick(4), dt), 2.0);
          assert_eq!(sim_time(Tick(10), dt), 5.0);
      }
  }
  ```

  Run the time tests and confirm RED:

  ```
  cargo test -p jumpgate-core time:: -- --nocolor
  ```

  EXPECTED: build succeeds, tests panic with `not implemented`; summary like `test result: FAILED. 0 passed; 4 failed`.

- [ ] **Step 3: Implement `Dt` and `sim_time`; make the time tests pass.**

  In `crates/jumpgate-core/src/time.rs`, replace the four bodies:

  ```rust
  impl Dt {
      pub fn new(dt: f64) -> Dt {
          Dt(dt)
      }
      pub fn get(self) -> f64 {
          self.0
      }
      pub fn bits(self) -> u64 {
          self.0.to_bits()
      }
  }

  /// Derived: (tick as f64) * dt. Computed only where needed, not stored.
  pub fn sim_time(tick: Tick, dt: Dt) -> f64 {
      (tick.0 as f64) * dt.get()
  }
  ```

  Run again:

  ```
  cargo test -p jumpgate-core time:: -- --nocolor
  ```

  EXPECTED: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 4: Add the `ids` module declaration to `lib.rs`.**

  In `crates/jumpgate-core/src/lib.rs`, add directly after `pub mod time;`:

  ```rust
  pub mod ids;
  ```

  Do not build yet — the next step creates `ids.rs`.

- [ ] **Step 5: Write a FAILING test + skeleton for `CraftId`, `BodyId`, `SlotMap` in `ids.rs`.**

  Create `crates/jumpgate-core/src/ids.rs`. `SlotMap` must define every method downstream tasks call (`new`, `len`, `is_empty`, `cursor`, `insert`, `get`, `remove`, `gen_of`) per the contract-surface RULE. The `cursor()` (high-water of slots ever minted) is HASHED state and is intentionally monotone (does not decrease on `remove`), so a future mid-run `Spawn` cannot rewrite prior ticks' hashes.

  ```rust
  //! Generational slot-map ids. `CraftId`/`BodyId` are `{slot, gen}` so a deleted
  //! entity can't be confused with its replacement (spec §4.3). `SlotMap::cursor()`
  //! is HASHED state (spec §6): it is the monotone high-water of slots ever minted,
  //! constant after `reset` in v1 but present so a future `Spawn` doesn't rewrite
  //! every prior tick's hash.

  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub struct CraftId {
      pub slot: u32,
      pub gen: u32,
  }

  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub struct BodyId {
      pub slot: u32,
      pub gen: u32,
  }

  /// Generational slot-map: dense values + per-slot generation + free list + a
  /// monotone `cursor` high-water. `cursor()` is included in the per-tick hash.
  pub struct SlotMap<T> {
      values: Vec<Option<T>>,
      gens: Vec<u32>,
      free: Vec<u32>,
      cursor: u64,
  }

  impl<T> SlotMap<T> {
      pub fn new() -> Self {
          unimplemented!()
      }
      pub fn len(&self) -> usize {
          unimplemented!()
      }
      pub fn is_empty(&self) -> bool {
          unimplemented!()
      }
      /// Monotone high-water of slots ever minted; HASHED state.
      pub fn cursor(&self) -> u64 {
          unimplemented!()
      }
      /// Returns `(slot, gen)` of the inserted value.
      pub fn insert(&mut self, _value: T) -> (u32, u32) {
          unimplemented!()
      }
      pub fn get(&self, _slot: u32, _gen: u32) -> Option<&T> {
          unimplemented!()
      }
      /// Removes; bumps the slot generation; pushes the slot to the free list.
      /// Does NOT decrease `cursor`.
      pub fn remove(&mut self, _slot: u32, _gen: u32) -> Option<T> {
          unimplemented!()
      }
      pub fn gen_of(&self, _slot: u32) -> Option<u32> {
          unimplemented!()
      }
  }

  impl<T> Default for SlotMap<T> {
      fn default() -> Self {
          Self::new()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn id_ordering_is_total_and_derivable() {
          let a = CraftId { slot: 0, gen: 0 };
          let b = CraftId { slot: 0, gen: 1 };
          let c = CraftId { slot: 1, gen: 0 };
          assert!(a < b && b < c);
          let mut v = vec![c, a, b];
          v.sort();
          assert_eq!(v, vec![a, b, c]);
      }

      #[test]
      fn new_is_empty_with_zero_cursor() {
          let m: SlotMap<u32> = SlotMap::new();
          assert_eq!(m.len(), 0);
          assert!(m.is_empty());
          assert_eq!(m.cursor(), 0);
      }

      #[test]
      fn insert_returns_fresh_slot_gen_and_advances_cursor() {
          let mut m: SlotMap<u32> = SlotMap::new();
          assert_eq!(m.insert(10), (0, 0));
          assert_eq!(m.insert(20), (1, 0));
          assert_eq!(m.len(), 2);
          assert_eq!(m.cursor(), 2);
      }

      #[test]
      fn get_returns_value_for_live_id_and_none_for_stale_gen() {
          let mut m: SlotMap<u32> = SlotMap::new();
          let (s, g) = m.insert(99);
          assert_eq!(m.get(s, g), Some(&99));
          assert_eq!(m.get(s, g + 1), None);
      }

      #[test]
      fn remove_bumps_gen_reuses_slot_but_keeps_cursor_monotone() {
          let mut m: SlotMap<u32> = SlotMap::new();
          let (s0, g0) = m.insert(1);
          assert_eq!((s0, g0), (0, 0));
          assert_eq!(m.remove(s0, g0), Some(1));
          assert_eq!(m.get(s0, g0), None); // stale
          // slot reused, generation bumped
          let (s1, g1) = m.insert(2);
          assert_eq!(s1, 0);
          assert_eq!(g1, 1);
          // cursor counts slots ever minted; it does NOT shrink on remove and
          // does NOT advance on free-list reuse.
          assert_eq!(m.cursor(), 1);
      }
  }
  ```

  Run RED:

  ```
  cargo test -p jumpgate-core ids:: -- --nocolor
  ```

  EXPECTED: build succeeds; panics with `not implemented`; e.g. `test result: FAILED. 1 passed; 4 failed` (the ordering test needs no impl, so it may pass).

- [ ] **Step 6: Implement `SlotMap`; make the ids tests pass.**

  In `crates/jumpgate-core/src/ids.rs`, replace the `impl<T> SlotMap<T>` block bodies:

  ```rust
  impl<T> SlotMap<T> {
      pub fn new() -> Self {
          SlotMap {
              values: Vec::new(),
              gens: Vec::new(),
              free: Vec::new(),
              cursor: 0,
          }
      }
      pub fn len(&self) -> usize {
          self.values.iter().filter(|v| v.is_some()).count()
      }
      pub fn is_empty(&self) -> bool {
          self.len() == 0
      }
      /// Monotone high-water of slots ever minted; HASHED state.
      pub fn cursor(&self) -> u64 {
          self.cursor
      }
      pub fn insert(&mut self, value: T) -> (u32, u32) {
          if let Some(slot) = self.free.pop() {
              let i = slot as usize;
              self.values[i] = Some(value);
              (slot, self.gens[i])
          } else {
              let slot = self.values.len() as u32;
              self.values.push(Some(value));
              self.gens.push(0);
              self.cursor += 1; // only fresh slots advance the high-water
              (slot, 0)
          }
      }
      pub fn get(&self, slot: u32, gen: u32) -> Option<&T> {
          let i = slot as usize;
          if i < self.values.len() && self.gens[i] == gen {
              self.values[i].as_ref()
          } else {
              None
          }
      }
      pub fn remove(&mut self, slot: u32, gen: u32) -> Option<T> {
          let i = slot as usize;
          if i < self.values.len() && self.gens[i] == gen && self.values[i].is_some() {
              let taken = self.values[i].take();
              self.gens[i] = self.gens[i].wrapping_add(1);
              self.free.push(slot);
              taken
          } else {
              None
          }
      }
      pub fn gen_of(&self, slot: u32) -> Option<u32> {
          self.gens.get(slot as usize).copied()
      }
  }
  ```

  Run again:

  ```
  cargo test -p jumpgate-core ids:: -- --nocolor
  ```

  EXPECTED: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 7: Add the `types` module declaration to `lib.rs`.**

  In `crates/jumpgate-core/src/lib.rs`, add directly after `pub mod ids;`:

  ```rust
  pub mod types;
  ```

  Do not build yet — the next step creates `types.rs`.

- [ ] **Step 8: Write the seam primitive types + tests in `types.rs`.**

  Create `crates/jumpgate-core/src/types.rs`. These are the primitive seam enums Task 4's `stores.rs` and Task 6's `contract.rs` build on. They are pure data with no methods, so they are implemented directly (no RED stub needed — the tests assert variant identity and the contract-mandated shapes). `Lod` is defined exactly per the contract (`Player`/`NpcInteraction`/`Nothing`); v1 dispatch/wake (the Lod seam exercise) is wired in the `world` task, not here.

  ```rust
  //! Primitive seam types shared across the contract (spec §4.4). Split into their
  //! own module so `stores.rs` (Task 4: `NavState{dest:NavDest}`, `lod:Vec<Lod>`)
  //! resolves them BEFORE `contract.rs` (Task 6) builds `Command`/`Event`/traits on
  //! top — this breaks the stores<->contract cycle. These are pure data: no methods.

  use crate::ids::{BodyId, CraftId};
  use crate::math::Vec3;

  /// Level-of-detail seam (spec §3 must-shape). v1 implements `Player` behaviour;
  /// the dispatch + wake-event hook lives in `world.rs`. The other variants exist
  /// so the seam is shaped, not so they are built in v1.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Lod {
      Player,
      NpcInteraction,
      Nothing,
  }

  /// Entity address: a craft OR a body. Generational ids keep stale refs distinct.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum EntityRef {
      Craft(CraftId),
      Body(BodyId),
  }

  /// Command address sum (spec §4.4): widened from day one so spawn / world-sim
  /// interventions / time-scoped commands are not foreclosed.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Target {
      Entity(EntityRef),
      World,
      Sim,
  }

  /// Navigator destination: an absolute position OR an entity to chase.
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub enum NavDest {
      Position(Vec3),
      Entity(EntityRef),
  }

  /// v1's ONLY command kind. `burn_budget`: optional scalar Δv cap.
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub enum CommandKind {
      Destination {
          dest: NavDest,
          burn_budget: Option<f64>,
      },
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn lod_has_the_three_contract_variants() {
          // Compiles iff exactly these variants exist; v1 default behaviour = Player.
          let all = [Lod::Player, Lod::NpcInteraction, Lod::Nothing];
          assert_eq!(all[0], Lod::Player);
          assert_ne!(Lod::Player, Lod::Nothing);
      }

      #[test]
      fn entity_ref_distinguishes_craft_from_body() {
          let c = EntityRef::Craft(CraftId { slot: 0, gen: 0 });
          let b = EntityRef::Body(BodyId { slot: 0, gen: 0 });
          assert_ne!(c, b);
      }

      #[test]
      fn target_carries_all_scopes() {
          let e = Target::Entity(EntityRef::Body(BodyId { slot: 2, gen: 1 }));
          assert_ne!(e, Target::World);
          assert_ne!(Target::World, Target::Sim);
      }

      #[test]
      fn navdest_supports_position_and_entity() {
          let p = NavDest::Position(Vec3::new(1.0, 2.0, 3.0));
          let en = NavDest::Entity(EntityRef::Craft(CraftId { slot: 1, gen: 0 }));
          assert_ne!(p, en);
          assert_eq!(p, NavDest::Position(Vec3::new(1.0, 2.0, 3.0)));
      }

      #[test]
      fn command_kind_destination_holds_dest_and_optional_budget() {
          let k = CommandKind::Destination {
              dest: NavDest::Position(Vec3::ZERO),
              burn_budget: Some(0.5),
          };
          match k {
              CommandKind::Destination { dest, burn_budget } => {
                  assert_eq!(dest, NavDest::Position(Vec3::ZERO));
                  assert_eq!(burn_budget, Some(0.5));
              }
          }
      }
  }
  ```

  Run the types tests:

  ```
  cargo test -p jumpgate-core types:: -- --nocolor
  ```

  EXPECTED: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 9: Add the `config` module declaration to `lib.rs`.**

  In `crates/jumpgate-core/src/lib.rs`, add directly after `pub mod types;`:

  ```rust
  pub mod config;
  ```

  Do not build yet — the next step creates `config.rs`.

- [ ] **Step 10: Write FAILING tests + skeleton for the config structs and `ConfigHash` in `config.rs`.**

  Create `crates/jumpgate-core/src/config.rs`. Define every config struct fully (needed for tests to compile) but leave `config_hash` as `unimplemented!()` so the hash tests are RED first.

  ```rust
  //! The single hashed run-config struct (spec §6). Initial conditions — body set,
  //! craft count, per-ship base spec, master seed, dt, softening, substep params —
  //! live HERE, recorded and folded into the CONFIG hash. This config hash is
  //! DISTINCT from the per-tick STATE hash (`hash.rs`): this one hashes immutable
  //! initial conditions ONCE with its own `"CONFIG_1"` tag; that one hashes the
  //! evolving world each tick via the shared `FnvHasher` seeded with `HASH_MAGIC`.
  //! Different magic/purpose; never conflate or share state.

  use crate::math::Vec3;
  use crate::time::Dt;

  /// Nominal ("base") ship numbers. Physics reads EFFECTIVE values via an accessor
  /// (Task 4 `stores::effective_params`); v1 effective == base.
  #[derive(Clone, Debug)]
  pub struct BaseSpec {
      pub base_dry_mass: f64,
      pub base_max_thrust: f64,
      pub base_exhaust_velocity: f64,
      pub base_fuel_capacity: f64,
  }

  /// Classical Kepler conic elements (radians for angles), solved once at init.
  #[derive(Clone, Debug)]
  pub struct OrbitalElements {
      pub a: f64,
      pub e: f64,
      pub i: f64,
      pub raan: f64,
      pub argp: f64,
      pub m0: f64,
  }

  #[derive(Clone, Debug)]
  pub struct BodyInit {
      pub mass: f64,
      pub elements: OrbitalElements,
  }

  #[derive(Clone, Debug)]
  pub struct CraftInit {
      pub spec: BaseSpec,
      pub pos: Vec3,
      pub vel: Vec3,
      pub fuel_mass: f64,
  }

  /// N substeps = pure fn of QUANTIZED total local acceleration magnitude (Task 7).
  #[derive(Clone, Copy, Debug)]
  pub struct SubstepCfg {
      pub accel_bin_base: f64,
      pub max_substeps: u32,
  }

  #[derive(Clone, Debug)]
  pub struct RunConfig {
      /// gym reset(seed) OVERWRITES this per episode.
      pub master_seed: u64,
      pub dt: Dt,
      /// epsilon in (r^2 + eps^2)^1.5 gravity softening.
      pub softening: f64,
      pub substep_cfg: SubstepCfg,
      /// ticks precomputed in the ephemeris window.
      pub ephemeris_window: u64,
      pub bodies: Vec<BodyInit>,
      pub craft: Vec<CraftInit>,
  }

  /// The CONFIG hash (immutable initial conditions). NOT the per-tick state hash.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct ConfigHash(pub u64);

  impl RunConfig {
      /// FNV-1a over master_seed, dt.bits(), softening.to_bits(), substep cfg, the
      /// ephemeris window, and every numeric field of every body/craft in a FIXED
      /// order (counts folded in first so two scenarios with different cardinality
      /// can never collide). DISTINCT from the per-tick state hash.
      pub fn config_hash(&self) -> ConfigHash {
          unimplemented!()
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      fn sample() -> RunConfig {
          RunConfig {
              master_seed: 42,
              dt: Dt::new(0.5),
              softening: 1e-4,
              substep_cfg: SubstepCfg { accel_bin_base: 2.0, max_substeps: 64 },
              ephemeris_window: 10_000,
              bodies: vec![BodyInit {
                  mass: 1.0,
                  elements: OrbitalElements {
                      a: 1.0, e: 0.0167, i: 0.0, raan: 0.0, argp: 1.0, m0: 0.5,
                  },
              }],
              craft: vec![CraftInit {
                  spec: BaseSpec {
                      base_dry_mass: 1.0,
                      base_max_thrust: 0.01,
                      base_exhaust_velocity: 3.0,
                      base_fuel_capacity: 0.5,
                  },
                  pos: Vec3::new(1.0, 0.0, 0.0),
                  vel: Vec3::new(0.0, 1.0, 0.0),
                  fuel_mass: 0.5,
              }],
          }
      }

      #[test]
      fn same_config_same_hash() {
          assert_eq!(sample().config_hash(), sample().config_hash());
      }

      #[test]
      fn changing_seed_changes_hash() {
          let mut c = sample();
          c.master_seed = 43;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_dt_changes_hash() {
          let mut c = sample();
          c.dt = Dt::new(0.25);
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_softening_changes_hash() {
          let mut c = sample();
          c.softening = 2e-4;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_a_body_field_changes_hash() {
          let mut c = sample();
          c.bodies[0].elements.e = 0.02;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_a_craft_field_changes_hash() {
          let mut c = sample();
          c.craft[0].spec.base_max_thrust = 0.02;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_craft_position_changes_hash() {
          let mut c = sample();
          c.craft[0].pos = Vec3::new(1.5, 0.0, 0.0);
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_substep_cfg_changes_hash() {
          let mut c = sample();
          c.substep_cfg.max_substeps = 128;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_ephemeris_window_changes_hash() {
          let mut c = sample();
          c.ephemeris_window = 20_000;
          assert_ne!(sample().config_hash(), c.config_hash());
      }

      #[test]
      fn changing_cardinality_changes_hash() {
          // An extra all-zero craft must still change the hash, because counts are
          // folded in BEFORE field values.
          let mut c = sample();
          c.craft.push(CraftInit {
              spec: BaseSpec {
                  base_dry_mass: 0.0,
                  base_max_thrust: 0.0,
                  base_exhaust_velocity: 0.0,
                  base_fuel_capacity: 0.0,
              },
              pos: Vec3::new(0.0, 0.0, 0.0),
              vel: Vec3::new(0.0, 0.0, 0.0),
              fuel_mass: 0.0,
          });
          assert_ne!(sample().config_hash(), c.config_hash());
      }
  }
  ```

  Run RED:

  ```
  cargo test -p jumpgate-core config:: -- --nocolor
  ```

  EXPECTED: build succeeds; panics with `not implemented`; e.g. `test result: FAILED. 0 passed; 10 failed`.

- [ ] **Step 11: Implement `config_hash` with a LOCAL FNV-1a fold; make config tests pass.**

  In `crates/jumpgate-core/src/config.rs`, insert the helper just above `impl RunConfig` and replace the method body. This FNV is LOCAL to the config hash (distinct purpose, distinct tag) — it deliberately does NOT use the shared `FnvHasher` from `hash.rs`.

  ```rust
  // FNV-1a 64-bit, folding one u64 at a time as 8 little-endian bytes. LOCAL to
  // the CONFIG hash; the per-tick STATE hash (hash.rs) is a separate hasher with a
  // different seed magic. The two hash spaces must never alias.
  const CONFIG_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const CONFIG_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

  struct ConfigFnv {
      state: u64,
  }

  impl ConfigFnv {
      fn new() -> Self {
          let mut h = ConfigFnv { state: CONFIG_FNV_OFFSET };
          h.write_u64(0x434f_4e46_4947_5f31); // "CONFIG_1" tag, distinct space
          h
      }

      fn write_u64(&mut self, v: u64) {
          for b in v.to_le_bytes() {
              self.state ^= b as u64;
              self.state = self.state.wrapping_mul(CONFIG_FNV_PRIME);
          }
      }

      fn finish(self) -> u64 {
          self.state
      }
  }
  ```

  ```rust
      pub fn config_hash(&self) -> ConfigHash {
          let mut h = ConfigFnv::new();
          // Scalars in fixed order.
          h.write_u64(self.master_seed);
          h.write_u64(self.dt.bits());
          h.write_u64(self.softening.to_bits());
          h.write_u64(self.substep_cfg.accel_bin_base.to_bits());
          h.write_u64(self.substep_cfg.max_substeps as u64);
          h.write_u64(self.ephemeris_window);
          // Counts folded BEFORE field values so cardinality changes always move
          // the hash even if the new elements are all-zero.
          h.write_u64(self.bodies.len() as u64);
          h.write_u64(self.craft.len() as u64);
          // Bodies in declaration order; each field in fixed order.
          for b in &self.bodies {
              h.write_u64(b.mass.to_bits());
              h.write_u64(b.elements.a.to_bits());
              h.write_u64(b.elements.e.to_bits());
              h.write_u64(b.elements.i.to_bits());
              h.write_u64(b.elements.raan.to_bits());
              h.write_u64(b.elements.argp.to_bits());
              h.write_u64(b.elements.m0.to_bits());
          }
          // Craft in declaration order; spec, pos, vel, fuel in fixed order.
          for c in &self.craft {
              h.write_u64(c.spec.base_dry_mass.to_bits());
              h.write_u64(c.spec.base_max_thrust.to_bits());
              h.write_u64(c.spec.base_exhaust_velocity.to_bits());
              h.write_u64(c.spec.base_fuel_capacity.to_bits());
              let p = c.pos.to_bits();
              h.write_u64(p[0]);
              h.write_u64(p[1]);
              h.write_u64(p[2]);
              let v = c.vel.to_bits();
              h.write_u64(v[0]);
              h.write_u64(v[1]);
              h.write_u64(v[2]);
              h.write_u64(c.fuel_mass.to_bits());
          }
          ConfigHash(h.finish())
      }
  ```

  Run again:

  ```
  cargo test -p jumpgate-core config:: -- --nocolor
  ```

  EXPECTED: `test result: ok. 10 passed; 0 failed`.

- [ ] **Step 12: Add the `hash` module declaration to `lib.rs`.**

  In `crates/jumpgate-core/src/lib.rs`, add directly after `pub mod config;`:

  ```rust
  pub mod hash;
  ```

  Do not build yet — the next step creates `hash.rs`.

- [ ] **Step 13: Write a FAILING test + skeleton for `FnvHasher` and the canonical-order anchor in `hash.rs`.**

  Create `crates/jumpgate-core/src/hash.rs`. This task lands the **per-tick STATE-hash specification** as the drift-lock anchor: the `FnvHasher`, `HASH_MAGIC`, `HASH_FORMAT_VERSION`, the numbered `HASH_FIELD_ORDER` doc enumerating every hashed field with its introducing task, and a **golden-hash test** over a fixed input sequence. Task 13 (`state_hash`) will USE this hasher and follow `HASH_FIELD_ORDER`; it does not redefine it. World does not exist yet, so the golden test pins a fixed `(magic, version, sample fields)` u64 sequence — the same sequence Task 13's zero-init `state_hash` will reproduce.

  ```rust
  //! Per-tick STATE-hash specification + the shared FNV-1a hasher (spec §6).
  //! Landed early as the DRIFT-LOCK ANCHOR: the canonical field order
  //! (`HASH_FIELD_ORDER`) is authoritative here, so a later task that adds a
  //! hashed field (e.g. Task 11 adds prev_fuel/prev_inside_dest) MUST append to
  //! `HASH_FIELD_ORDER`, bump `HASH_FORMAT_VERSION`, and update the golden test —
  //! the change cannot silently alter the hash uncaught.
  //!
  //! DISTINCT from the CONFIG hash (`config::RunConfig::config_hash`): that one
  //! folds immutable initial conditions ONCE with a "CONFIG_1" tag. This one
  //! hashes evolving world state each tick seeded with `HASH_MAGIC`.
  //!
  //! ## HASH_FIELD_ORDER — canonical per-tick state-hash field order
  //!
  //! `state_hash` (Task 13) writes EXACTLY these u64 words in EXACTLY this order.
  //! Every f64 is encoded via `f64::to_bits()`; every word is folded
  //! little-endian by `FnvHasher::write_u64`. Numbering is stable; APPEND only.
  //!
  //!  1. HASH_MAGIC                              (Task 3, header)
  //!  2. HASH_FORMAT_VERSION as u64              (Task 3, header)
  //!  3. tick.0                                   (Task 3, time)
  //!  4. body_store.ids.cursor()                  (Task 4/13, slot-map high-water)
  //!  5. ship_store.ids.cursor()                  (Task 4/13, slot-map high-water)
  //!  -- bodies, sorted by BodyId (slot, gen):
  //!  6.   body.slot as u64, body.gen as u64      (Task 13)
  //!  7.   body.mass.to_bits()                    (Task 13)
  //!     (body POSITION is derived from tick via ephemeris, NOT stored, so it is
  //!      NOT hashed independently — it is a pure function of tick already hashed)
  //!  -- craft, sorted by CraftId (slot, gen):
  //!  8.   craft.slot as u64, craft.gen as u64    (Task 13)
  //!  9.   pos.x,pos.y,pos.z to_bits()            (Task 13)
  //! 10.   vel.x,vel.y,vel.z to_bits()            (Task 13)
  //! 11.   fuel_mass.to_bits()                    (Task 13)
  //! 12.   nav discriminant as u64 (+ resolved dest/dv_remaining bits)  (Task 13)
  //! 13.   lod discriminant as u64               (Task 13)
  //! -- APPEND BELOW THIS LINE (bump HASH_FORMAT_VERSION + golden test on change):
  //! 14.   prev_fuel[i].to_bits()                 (Task 11, event edge-detect state)
  //! 15.   prev_inside_dest[i] as u64             (Task 11, event edge-detect state)

  /// Header magic for the per-tick STATE hash (little-endian, spec §6).
  pub const HASH_MAGIC: u64 = 0x4a55_4d50_4741_5445; // "JUMPGATE"
  /// Bump whenever HASH_FIELD_ORDER changes (e.g. Task 11 appends fields).
  pub const HASH_FORMAT_VERSION: u32 = 1;

  /// Shared FNV-1a 64-bit hasher for the per-tick state hash. Folds each u64 as 8
  /// little-endian bytes. `new()` seeds with `HASH_MAGIC` then the version word.
  pub struct FnvHasher {
      state: u64,
  }

  const STATE_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const STATE_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

  impl FnvHasher {
      pub fn new() -> Self {
          unimplemented!()
      }
      /// Folds one u64 as 8 little-endian bytes (HASH_FIELD_ORDER words).
      pub fn write_u64(&mut self, _v: u64) {
          unimplemented!()
      }
      pub fn finish(self) -> u64 {
          unimplemented!()
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
      fn new_seeds_with_magic_then_version() {
          // A fresh hasher already absorbed (HASH_MAGIC, HASH_FORMAT_VERSION).
          let a = FnvHasher::new().finish();
          let mut b = FnvHasher::new();
          // Writing nothing leaves it equal to the seeded state.
          assert_eq!(a, b.clone_finish_via_extra_write_check());
          let _ = &mut b;
      }

      // The above helper would require interior plumbing; keep the test simple:
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

      /// GOLDEN HASH. This pins the canonical encoding of the HASH_FIELD_ORDER
      /// header + a zero-initialized single-body single-craft state slice (the
      /// same words Task 13's zero-init `state_hash` will reproduce). If this value
      /// changes, the canonical hash encoding changed — that is ONLY allowed
      /// alongside a HASH_FORMAT_VERSION bump and a HASH_FIELD_ORDER edit.
      #[test]
      fn golden_zero_state_hash() {
          let mut h = FnvHasher::new();
          // header (words 1-2) are already folded by new(); now the rest of a
          // minimal zero-init slice per HASH_FIELD_ORDER words 3..=13:
          h.write_u64(0); // 3. tick
          h.write_u64(0); // 4. body cursor
          h.write_u64(0); // 5. ship cursor
          // one body (slot 0, gen 0, mass 0.0):
          h.write_u64(0); // body slot
          h.write_u64(0); // body gen
          h.write_u64(0.0f64.to_bits()); // body mass
          // one craft (slot 0, gen 0; zero pos/vel/fuel; nav Idle=0; lod Player=0):
          h.write_u64(0); // craft slot
          h.write_u64(0); // craft gen
          h.write_u64(0.0f64.to_bits()); // pos.x
          h.write_u64(0.0f64.to_bits()); // pos.y
          h.write_u64(0.0f64.to_bits()); // pos.z
          h.write_u64(0.0f64.to_bits()); // vel.x
          h.write_u64(0.0f64.to_bits()); // vel.y
          h.write_u64(0.0f64.to_bits()); // vel.z
          h.write_u64(0.0f64.to_bits()); // fuel_mass
          h.write_u64(0); // nav discriminant (Idle)
          h.write_u64(0); // lod discriminant (Player)
          // Hardcoded golden value: fill in from the first GREEN run (Step 14).
          assert_eq!(h.finish(), GOLDEN_ZERO_STATE_HASH);
      }
  }
  ```

  Note: the `golden_zero_state_hash` test references `GOLDEN_ZERO_STATE_HASH` and the first test references a nonexistent helper — both are intentional compile/fail bait removed in the next step. Also remove the placeholder helper test now by deleting the `new_seeds_with_magic_then_version` test body's `clone_finish_via_extra_write_check` line before running; the remaining tests must compile. Concretely, before running RED, delete the entire `new_seeds_with_magic_then_version` test (it was illustrative only) so the module compiles, leaving `fresh_hasher_is_deterministic`, `write_order_matters`, `writing_changes_the_hash`, and `golden_zero_state_hash`.

  After deleting that one test, run RED:

  ```
  cargo test -p jumpgate-core hash:: -- --nocolor 2>&1 | head -30
  ```

  EXPECTED: a COMPILE error for the missing `GOLDEN_ZERO_STATE_HASH` const (cannot find value). This is the expected RED for a golden test whose value is not yet known.

- [ ] **Step 14: Implement `FnvHasher`, capture the golden value, and make hash tests pass.**

  In `crates/jumpgate-core/src/hash.rs`, replace the three method bodies:

  ```rust
  impl FnvHasher {
      pub fn new() -> Self {
          let mut h = FnvHasher { state: STATE_FNV_OFFSET };
          h.write_u64(HASH_MAGIC);                 // HASH_FIELD_ORDER word 1
          h.write_u64(HASH_FORMAT_VERSION as u64); // HASH_FIELD_ORDER word 2
          h
      }
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
  ```

  Add the golden const just below `HASH_FORMAT_VERSION` (placeholder value first):

  ```rust
  /// Golden per-tick hash of the minimal zero-init slice under HASH_FIELD_ORDER
  /// words 1..=13. Pinned so any change to the canonical encoding is caught.
  /// Captured from the first run of `golden_zero_state_hash`; if HASH_FIELD_ORDER
  /// or HASH_FORMAT_VERSION changes, recapture AND bump the version.
  pub const GOLDEN_ZERO_STATE_HASH: u64 = 0; // placeholder; replaced below
  ```

  Now capture the real value. Temporarily change the golden assertion to print:

  ```rust
          // TEMP capture line (remove after recording the value):
          assert_eq!(h.finish(), 0, "GOLDEN = {:#018x}", h.finish());
  ```

  Run it and read the printed hex from the failure message:

  ```
  cargo test -p jumpgate-core hash::tests::golden_zero_state_hash -- --nocolor 2>&1 | grep -o '0x[0-9a-f]\{16\}' | tail -1
  ```

  EXPECTED: prints a 16-hex-digit value, e.g. `0x________________`. Copy that exact value into `GOLDEN_ZERO_STATE_HASH`, then restore the assertion to `assert_eq!(h.finish(), GOLDEN_ZERO_STATE_HASH);` and remove the TEMP line.

  Run the hash tests for real:

  ```
  cargo test -p jumpgate-core hash:: -- --nocolor
  ```

  EXPECTED: `test result: ok. 4 passed; 0 failed` — including `golden_zero_state_hash` now matching the recorded constant.

- [ ] **Step 15: Run the whole core crate test suite and clippy to confirm no regression.**

  ```
  cargo test -p jumpgate-core -- --nocolor
  ```

  EXPECTED: all tests pass. This task contributes 4 (time) + 5 (ids) + 5 (types) + 10 (config) + 4 (hash) = 28 tests, plus whatever Tasks 1-2 contributed. No `FAILED`.

  Then lint all targets (binary/library crate — `--lib` is a no-op here per the MEMORY note, so lint `--all-targets` to cover inline test modules):

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```

  EXPECTED: `Finished` with no warnings. In particular no `disallowed-methods` hits (this task uses no `SystemTime`/`Instant::now`/`thread_rng`/env reads).

- [ ] **Step 16: Confirm `lib.rs` declares ONLY modules that exist at this task's exit.**

  Verify the module set is exactly the files created so far (no forward references to Task 4+ files like `contract`/`stores`/`rng`):

  ```
  grep -n '^pub mod' crates/jumpgate-core/src/lib.rs
  ```

  EXPECTED output is exactly these six lines, in order:

  ```
  pub mod math;
  pub mod time;
  pub mod ids;
  pub mod types;
  pub mod config;
  pub mod hash;
  ```

  If any line names a module whose file does not yet exist (e.g. `ids` from a stale Task-1/2 edit, `contract`, `stores`), remove it — the workspace MUST compile at this task's exit.

- [ ] **Step 17: Commit.**

  ```
  git add docs/superpowers/specs/contract-surface.md crates/jumpgate-core/src/time.rs crates/jumpgate-core/src/ids.rs crates/jumpgate-core/src/types.rs crates/jumpgate-core/src/config.rs crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  Task 3: cross-task contract surface + time/ids/types/config + FNV anchor

  - docs/.../contract-surface.md: authoritative symbol->providing-task->consumers
    table + the RULE (provider defines+tests every method downstream calls) +
    acyclic module order + hash-ownership invariant (root-cause fix)
  - time.rs: Tick(u64), Dt (stores f64; get()/bits()), sim_time helper
  - ids.rs: CraftId/BodyId {slot,gen}; generational SlotMap with monotone
    cursor() (HASHED state) — moved earlier so types.rs resolves EntityRef
  - types.rs: Lod, EntityRef, Target, NavDest, CommandKind seam primitives so
    stores.rs (Task 4) resolves before contract.rs (Task 6) builds on top
  - config.rs: RunConfig + components; config_hash() via LOCAL "CONFIG_1" FNV-1a
    fold (DISTINCT from per-tick state hash)
  - hash.rs: per-tick STATE-hash anchor: FnvHasher, HASH_MAGIC,
    HASH_FORMAT_VERSION, numbered HASH_FIELD_ORDER (append-only) + golden
    zero-state hash test so later field additions (Task 11) can't drift silently
  - lib.rs: declares only modules that exist at this task's exit

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

  EXPECTED: commit succeeds; `git status` shows a clean tree for these files.

---

**Notes for the implementer:**
- **lib.rs reality (corrected):** the original plan falsely claimed lib.rs already declared `pub mod ids;` after Tasks 1-2. It does NOT — after Tasks 1-2 lib.rs declares only `pub mod math;`. This task creates `ids.rs` and declares it. Step 16 enforces the invariant that lib.rs only ever names existing files.
- **Module order is acyclic:** `math -> time -> ids -> types -> config -> hash`. `ids` precedes `types` because `EntityRef`/`NavDest` reference `CraftId`/`BodyId`; this is the documented deviation from the fix-list's literal `types -> ids` wording (which cannot compile against the fixed contract). `config` imports only `math`+`time`, so it follows `types` with no cycle. The full acyclic chain is recorded in `contract-surface.md`.
- **Two hashes, never conflated:** `config_hash` (this task, `config.rs`) uses a LOCAL fold tagged `"CONFIG_1"`. The per-tick STATE hash (Task 13, uses `hash::FnvHasher`) is seeded with `HASH_MAGIC` and follows `HASH_FIELD_ORDER`. They never share state or magic.
- **HASH_FIELD_ORDER is the drift-lock:** it is authoritative and APPEND-ONLY. When Task 11 adds `prev_fuel`/`prev_inside_dest` to the state hash, the implementer MUST append words 14-15 (already stubbed in the doc), bump `HASH_FORMAT_VERSION`, and recapture `GOLDEN_ZERO_STATE_HASH`. The golden test fails loudly otherwise — determinism tests prove same-run reproducibility, not correct field coverage, which is exactly why the golden test exists.
- **SlotMap completeness (RULE):** `SlotMap` defines AND tests `new/len/is_empty/cursor/insert/get/remove/gen_of` here because Task 4 `stores.rs` and Task 13 `state_hash` call them. `cursor()` is monotone (counts fresh slots minted, unaffected by `remove`/free-list reuse) so a future mid-run `Spawn` cannot rewrite prior ticks' hashes.
- **Body position is NOT independently hashed:** it is derived from `tick` via the ephemeris (a pure function of an already-hashed `tick`), so HASH_FIELD_ORDER hashes only body `mass` + ids, not body `pos`. This is noted inline in the doc so Task 13 does not double-count it.
- The config/state hashes use `to_bits()` on every `f64` so `-0.0` vs `+0.0` and NaN payloads hash by exact bit pattern (Tier-B rule, spec §6).
- This task introduces no RNG, time, or env-clock calls, so `clippy disallowed-methods` has nothing to flag.

---

### Task 4: Generational slot-map + per-type stores skeleton

A deterministic generational `SlotMap<T>` with a hashable `cursor()` plus the dense-index navigation methods (`iter_ids`/`dense_index`/`id_at`) that Tasks 10–13 read through, the `CraftId`/`BodyId` id types, and the `ShipStore`/`BodyStore` SoA layouts with their constructor/accessor impl blocks (`empty`/`push`/`ids_at`/`index_of`/`craft_pos_by_id`/`craft_fuel_capacity`) plus the `effective_params` accessor (v1: `effective == base`). Strict TDD: every behavior gets a failing test first, run-it-fails, minimal impl, run-it-passes.

**Depends on Task 3.** Task 3 lands the crate skeleton `crates/jumpgate-core` (`Cargo.toml` + `lib.rs`) with the `math.rs`, `time.rs`, `types.rs`, and `config.rs` modules. This task consumes `Vec3` (from `math.rs`), `BaseSpec` (from `config.rs`), and the seam types `NavDest` / `Lod` (from `types.rs` — the primitive-seam module created in Task 3 per the acyclic ordering `math -> time -> types -> ids -> config -> contract -> stores`). Those names/signatures are taken verbatim from the shared type contract; do not redefine them here. NOTE: `contract.rs` does NOT exist yet (it is built in Task 6, AFTER this task) — do not import from `crate::contract` anywhere in Task 4, and do not declare `pub mod contract` in `lib.rs` at this task's exit.

**Cross-task contract surface this task PROVIDES** (every downstream caller's needs must be defined and tested HERE):
- `SlotMap<T>`: `new`/`len`/`is_empty`/`cursor`/`insert`/`get`/`remove` (existing) + `iter_ids` (live `(slot,gen)` iterator), `dense_index(slot,gen) -> Option<usize>`, `id_at(idx) -> Option<(u32,u32)>`. None-on-stale-gen semantics for all three. Consumed by Tasks 10–13.
- `ShipStore`: `empty()`, `push(spec,pos,vel,fuel) -> CraftId`, `ids_at(idx) -> CraftId`, `index_of(id) -> Option<usize>`, `craft_pos_by_id(id) -> Option<Vec3>`, `craft_fuel_capacity(id) -> Option<f64>`. Consumed by Tasks 10/16.
- `ShipStore` carries `prev_fuel: Vec<f64>` and `prev_inside_dest: Vec<bool>` SoA arrays (reserved-for-hash; the copy-at-end-of-step and the FNV write land in later World/hash tasks — NOT here).
- v1 invariant pinned by this task: **`slot == dense row index`**. All craft are minted at reset with no mid-run despawn, so slots allocate contiguously and the SoA arrays stay row-aligned with the slot number. `push` asserts this.

#### Files
- **Create:** `crates/jumpgate-core/src/ids.rs`
- **Create:** `crates/jumpgate-core/src/stores.rs`
- **Modify:** `crates/jumpgate-core/src/lib.rs` (wire the two new modules + re-exports)
- **Test:** `crates/jumpgate-core/src/ids.rs` (`#[cfg(test)] mod tests`)
- **Test:** `crates/jumpgate-core/src/stores.rs` (`#[cfg(test)] mod tests`)

---

- [ ] **Step 1: Wire the two new modules into the crate**

  Add the module declarations and re-exports to `crates/jumpgate-core/src/lib.rs`. Create empty stub files so the crate compiles before any tests are written. `lib.rs` already declares `pub mod math; pub mod time; pub mod types; pub mod config;` from Task 3 (in acyclic order). We append `ids` (after `time`, before `config` consumers — but module declaration order in `lib.rs` does not affect compilation, only the dependency edges in the code do) and `stores`. Append to `crates/jumpgate-core/src/lib.rs`:
  ```rust
  pub mod ids;
  pub mod stores;

  pub use ids::{BodyId, CraftId, SlotMap};
  pub use stores::{BodyStore, Effective, NavState, ShipStore, effective_params};
  ```

  Create `crates/jumpgate-core/src/ids.rs` with only:
  ```rust
  //! Generational slot-map ids. The `cursor()` (slots-ever-allocated high-water)
  //! is HASHED state per the Tier-B determinism contract (§6). v1 invariant:
  //! `slot == dense row index` (no mid-run despawn → contiguous slots).
  ```

  Create `crates/jumpgate-core/src/stores.rs` with only:
  ```rust
  //! Per-type Struct-of-Arrays stores keyed by generational slot-map ids.
  ```

  Run:
  ```
  cargo build -p jumpgate-core
  ```
  EXPECTED: `Finished` (no errors; two empty modules compile).

- [ ] **Step 2: Failing test — `CraftId`/`BodyId` exist, are `Copy`/`Ord`, and sort by (slot, gen)**

  The contract requires both ids derive `Ord` (canonical hashing order is bodies-then-craft by sorted id). Add to the bottom of `crates/jumpgate-core/src/ids.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn ids_are_copy_and_ord() {
          let a = CraftId { slot: 0, gen: 0 };
          let b = CraftId { slot: 0, gen: 1 };
          let c = CraftId { slot: 1, gen: 0 };
          // Copy: using `a` after passing it by value must still work.
          let _copy = a;
          assert!(a < b, "same slot, lower gen sorts first");
          assert!(b < c, "lower slot sorts before higher slot regardless of gen");
          assert_eq!(a, a);

          let x = BodyId { slot: 2, gen: 5 };
          let y = BodyId { slot: 2, gen: 5 };
          assert_eq!(x, y);
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids:: -- --nocapture
  ```
  EXPECTED: compile error — `cannot find type CraftId`/`BodyId in this scope`.

- [ ] **Step 3: Implement `CraftId` and `BodyId`**

  Insert above the `#[cfg(test)]` module in `crates/jumpgate-core/src/ids.rs`:
  ```rust
  /// Generational id for a craft slot. `Ord` is derived as (slot, gen) lexicographic
  /// (field order: slot first), which is the canonical state-hash ordering.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub struct CraftId {
      pub slot: u32,
      pub gen: u32,
  }

  /// Generational id for a body slot. Same ordering contract as `CraftId`.
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
  pub struct BodyId {
      pub slot: u32,
      pub gen: u32,
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids:: -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 4: Failing test — `SlotMap` insert returns a key, `get` reads it back, `len`/`cursor` start at 0**

  `SlotMap<T>` is generic (stores `()` for both `ShipStore` and `BodyStore`). `insert(value) -> (u32 slot, u32 gen)` is the implementation API the drafter requires; `new`/`len`/`cursor` are the contract-pinned signatures. Add this test inside the existing `mod tests` in `crates/jumpgate-core/src/ids.rs`:
  ```rust
      #[test]
      fn insert_get_len_cursor() {
          let mut sm: SlotMap<u32> = SlotMap::new();
          assert_eq!(sm.len(), 0);
          assert_eq!(sm.cursor(), 0);

          let (s0, g0) = sm.insert(100);
          assert_eq!((s0, g0), (0, 0));
          assert_eq!(sm.len(), 1);
          assert_eq!(sm.cursor(), 1);
          assert_eq!(sm.get(s0, g0), Some(&100));

          let (s1, g1) = sm.insert(200);
          assert_eq!((s1, g1), (1, 0));
          assert_eq!(sm.len(), 2);
          assert_eq!(sm.cursor(), 2);
          assert_eq!(sm.get(s1, g1), Some(&200));

          // wrong generation reads nothing.
          assert_eq!(sm.get(s0, 99), None);
          // out-of-range slot reads nothing.
          assert_eq!(sm.get(7, 0), None);
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::insert_get_len_cursor -- --nocapture
  ```
  EXPECTED: compile error — `cannot find type SlotMap` / `no function or associated item named new`.

- [ ] **Step 5: Implement `SlotMap` (new/insert/get/len/cursor)**

  Insert above the `#[cfg(test)]` module in `crates/jumpgate-core/src/ids.rs`. `cursor` is the slots-ever-allocated high-water mark (length of the backing vectors); it never shrinks, so it is deterministic and monotonic and is folded into the per-tick state hash. `free` is the LIFO free list of reclaimed slots.
  ```rust
  /// Generational slot-map. Dense `values` + parallel `gens` + LIFO `free` list.
  /// `cursor()` is the slots-ever-allocated high-water (== values.len()); it is
  /// HASHED state so a future mid-run `Spawn` does not retroactively change the
  /// hash of every prior tick (§6).
  pub struct SlotMap<T> {
      values: Vec<Option<T>>,
      gens: Vec<u32>,
      free: Vec<u32>,
  }

  impl<T> SlotMap<T> {
      pub fn new() -> Self {
          SlotMap {
              values: Vec::new(),
              gens: Vec::new(),
              free: Vec::new(),
          }
      }

      /// Number of currently-occupied slots.
      pub fn len(&self) -> usize {
          self.values.iter().filter(|v| v.is_some()).count()
      }

      /// `true` when no slot is occupied.
      pub fn is_empty(&self) -> bool {
          self.len() == 0
      }

      /// Slots-ever-allocated high-water mark; included in the per-tick hash.
      pub fn cursor(&self) -> u64 {
          self.values.len() as u64
      }

      /// Insert a value, returning its `(slot, gen)`. Reuses a freed slot if one
      /// exists (LIFO), otherwise grows the backing vectors (advancing `cursor`).
      pub fn insert(&mut self, value: T) -> (u32, u32) {
          if let Some(slot) = self.free.pop() {
              let s = slot as usize;
              self.values[s] = Some(value);
              (slot, self.gens[s])
          } else {
              let slot = self.values.len() as u32;
              self.values.push(Some(value));
              self.gens.push(0);
              (slot, 0)
          }
      }

      /// Read a value, validating the generation. A stale `(slot, gen)` -> `None`.
      pub fn get(&self, slot: u32, gen: u32) -> Option<&T> {
          let s = slot as usize;
          if s >= self.values.len() || self.gens[s] != gen {
              return None;
          }
          self.values[s].as_ref()
      }
  }

  impl<T> Default for SlotMap<T> {
      fn default() -> Self {
          Self::new()
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::insert_get_len_cursor -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 6: Failing test — `remove` bumps gen, invalidates the old id, reuses the slot; replacement at same slot is distinct**

  This is the core generational-safety guarantee from the drafter notes: deletion invalidates the old id but NOT the replacement at the same slot. Add to `mod tests` in `crates/jumpgate-core/src/ids.rs`:
  ```rust
      #[test]
      fn remove_invalidates_old_id_not_replacement() {
          let mut sm: SlotMap<u32> = SlotMap::new();
          let (s0, g0) = sm.insert(100);
          assert_eq!(sm.remove(s0, g0), Some(100));
          // double-remove of the stale id is a no-op.
          assert_eq!(sm.remove(s0, g0), None);
          // old id is now stale.
          assert_eq!(sm.get(s0, g0), None);
          assert_eq!(sm.len(), 0);
          // cursor is a high-water mark: removal does NOT shrink it.
          assert_eq!(sm.cursor(), 1);

          // reinserting reuses slot 0 but with a bumped generation.
          let (s1, g1) = sm.insert(200);
          assert_eq!(s1, s0, "freed slot is reused");
          assert_eq!(g1, g0 + 1, "generation bumped on reuse");
          // the replacement is live...
          assert_eq!(sm.get(s1, g1), Some(&200));
          // ...but the old id still does NOT resolve to it.
          assert_eq!(sm.get(s0, g0), None);
          assert_eq!(sm.cursor(), 1, "reused slot does not advance cursor");
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::remove_invalidates_old_id_not_replacement -- --nocapture
  ```
  EXPECTED: compile error — `no method named remove`.

- [ ] **Step 7: Implement `SlotMap::remove`**

  The generation is bumped at remove time (so the freed-but-not-yet-reused id is already stale), and the slot is pushed onto the free list. Insert into the `impl<T> SlotMap<T>` block in `crates/jumpgate-core/src/ids.rs`, after `get`:
  ```rust
      /// Remove a value, validating the generation. Bumps the slot's generation and
      /// frees the slot for reuse. A stale `(slot, gen)` -> `None` (no-op).
      pub fn remove(&mut self, slot: u32, gen: u32) -> Option<T> {
          let s = slot as usize;
          if s >= self.values.len() || self.gens[s] != gen {
              return None;
          }
          let taken = self.values[s].take();
          if taken.is_some() {
              self.gens[s] = self.gens[s].wrapping_add(1);
              self.free.push(slot);
          }
          taken
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::remove_invalidates_old_id_not_replacement -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 8: Failing test — `dense_index` / `id_at` / `iter_ids` (the downstream navigation surface)**

  Tasks 10–13 index the SoA arrays by dense row, look up the typed id at a row, and iterate live ids. `SlotMap` is generic over `T`, so it cannot return `CraftId`; `id_at` returns the `(slot,gen)` tuple (matching `insert`), and the typed store wraps it. All three honor None-on-stale-gen semantics. Under the v1 `slot == dense row` invariant, `dense_index(slot,gen)` returns `Some(slot as usize)` for a live id. Add to `mod tests` in `crates/jumpgate-core/src/ids.rs`:
  ```rust
      #[test]
      fn dense_index_id_at_iter_ids() {
          let mut sm: SlotMap<u32> = SlotMap::new();
          let (s0, g0) = sm.insert(10);
          let (s1, g1) = sm.insert(20);

          // dense_index: live id -> its row (slot==row in v1); stale/oob -> None.
          assert_eq!(sm.dense_index(s0, g0), Some(0));
          assert_eq!(sm.dense_index(s1, g1), Some(1));
          assert_eq!(sm.dense_index(s0, 99), None, "stale gen -> None");
          assert_eq!(sm.dense_index(7, 0), None, "out-of-range slot -> None");

          // id_at: row -> (slot,gen) of the live occupant; empty/oob row -> None.
          assert_eq!(sm.id_at(0), Some((s0, g0)));
          assert_eq!(sm.id_at(1), Some((s1, g1)));
          assert_eq!(sm.id_at(2), None, "out-of-range row -> None");

          // iter_ids: yields every live (slot,gen) in ascending slot order.
          let live: Vec<(u32, u32)> = sm.iter_ids().collect();
          assert_eq!(live, vec![(s0, g0), (s1, g1)]);

          // after a remove, the freed row is skipped by iter_ids and id_at -> None.
          assert_eq!(sm.remove(s0, g0), Some(10));
          assert_eq!(sm.id_at(0), None, "removed row -> None");
          assert_eq!(sm.dense_index(s0, g0), None, "stale id after remove -> None");
          let live_after: Vec<(u32, u32)> = sm.iter_ids().collect();
          assert_eq!(live_after, vec![(s1, g1)]);
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::dense_index_id_at_iter_ids -- --nocapture
  ```
  EXPECTED: compile error — `no method named dense_index` / `id_at` / `iter_ids`.

- [ ] **Step 9: Implement `dense_index` / `id_at` / `iter_ids`**

  Insert into the `impl<T> SlotMap<T>` block in `crates/jumpgate-core/src/ids.rs`, after `remove`. `dense_index` is the slot itself under the v1 `slot == row` invariant, gated on liveness + generation. `id_at` validates the row holds a live value. `iter_ids` filters to live slots and emits `(slot, gen)`.
  ```rust
      /// Dense SoA row index for a live `(slot, gen)`. Under the v1 `slot == row`
      /// invariant this is the slot itself. Stale gen, removed slot, or out-of-range
      /// slot -> `None`.
      pub fn dense_index(&self, slot: u32, gen: u32) -> Option<usize> {
          let s = slot as usize;
          if s >= self.values.len() || self.gens[s] != gen || self.values[s].is_none() {
              return None;
          }
          Some(s)
      }

      /// The live `(slot, gen)` occupying dense row `idx`, or `None` if the row is
      /// empty/freed or out of range. Generic over `T`, so it returns the raw tuple;
      /// typed stores wrap it into `CraftId`/`BodyId`.
      pub fn id_at(&self, idx: usize) -> Option<(u32, u32)> {
          if idx >= self.values.len() || self.values[idx].is_none() {
              return None;
          }
          Some((idx as u32, self.gens[idx]))
      }

      /// Iterate every live `(slot, gen)` in ascending slot order.
      pub fn iter_ids(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
          self.values
              .iter()
              .enumerate()
              .filter_map(move |(i, v)| v.as_ref().map(|_| (i as u32, self.gens[i])))
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::dense_index_id_at_iter_ids -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 10: Failing test — `cursor` is deterministic across an identical insert/remove sequence**

  `cursor()` feeds the per-tick hash, so two slot-maps driven by the same operation sequence must report identical cursors. Add to `mod tests` in `crates/jumpgate-core/src/ids.rs`:
  ```rust
      #[test]
      fn cursor_is_deterministic() {
          fn drive() -> (u64, usize) {
              let mut sm: SlotMap<u32> = SlotMap::new();
              let a = sm.insert(1);
              let b = sm.insert(2);
              sm.remove(a.0, a.1);
              let _c = sm.insert(3); // reuses a's slot, does not grow cursor
              sm.remove(b.0, b.1);
              let _d = sm.insert(4); // reuses b's slot
              (sm.cursor(), sm.len())
          }
          let first = drive();
          let second = drive();
          assert_eq!(first, second, "same op sequence -> same (cursor, len)");
          assert_eq!(first.0, 2, "two slots ever allocated -> cursor == 2");
          assert_eq!(first.1, 2, "two live entries");
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib ids::tests::cursor_is_deterministic -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed` (implementation from Steps 5/7 already satisfies this; this test pins the determinism contract so a future change to `cursor` semantics breaks loudly).

- [ ] **Step 11: Failing test — `NavState` and `Effective` exist with the contract shape**

  `NavState` is the resolved field the autopilot reads (NOT a `Command`); `NavDest` comes from `types.rs` (Task 3) — NOT `contract.rs`, which does not exist until Task 6. `Effective` is the physics-facing param struct (carries `fuel_capacity`, which backs the `craft_fuel_capacity` accessor Task 16 needs). Add to the bottom of `crates/jumpgate-core/src/stores.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::math::Vec3;
      use crate::types::NavDest;

      #[test]
      fn navstate_and_effective_shapes() {
          let idle = NavState::Idle;
          let seeking = NavState::Seeking {
              dest: NavDest::Position(Vec3::new(1.0, 2.0, 3.0)),
              dv_remaining: 0.5,
          };
          // both variants are constructible and Copy (used by value in the integrator).
          let _copy = idle;
          let _copy2 = seeking;

          let eff = Effective {
              dry_mass: 1.0,
              max_thrust: 2.0,
              exhaust_velocity: 3.0,
              fuel_capacity: 4.0,
          };
          assert_eq!(eff.dry_mass, 1.0);
          assert_eq!(eff.max_thrust, 2.0);
          assert_eq!(eff.exhaust_velocity, 3.0);
          assert_eq!(eff.fuel_capacity, 4.0);
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::navstate_and_effective_shapes -- --nocapture
  ```
  EXPECTED: compile error — `cannot find type NavState` / `Effective in this scope`.

- [ ] **Step 12: Implement `NavState` and `Effective`**

  Insert above the `#[cfg(test)]` module in `crates/jumpgate-core/src/stores.rs`. `NavDest` is `Copy` (per contract), so `NavState` derives `Copy`. NOTE the imports point at `crate::types`, NOT `crate::contract`:
  ```rust
  use crate::config::BaseSpec;
  use crate::ids::SlotMap;
  use crate::math::Vec3;
  use crate::types::NavDest;

  /// Resolved navigation state the autopilot reads each tick. This is the RESOLVED
  /// field (set by command ingestion), NOT a `Command` — the autopilot never reads
  /// the command stream directly.
  #[derive(Clone, Copy, Debug)]
  pub enum NavState {
      Idle,
      Seeking { dest: NavDest, dv_remaining: f64 },
  }

  /// Effective ship parameters = base × component-mods × wear. In v1 the mod and
  /// wear factors are identity, so `effective == base`. The integrator and autopilot
  /// read ONLY through this accessor — never `BaseSpec` directly (§5.5 seam).
  #[derive(Clone, Copy, Debug)]
  pub struct Effective {
      pub dry_mass: f64,
      pub max_thrust: f64,
      pub exhaust_velocity: f64,
      pub fuel_capacity: f64,
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::navstate_and_effective_shapes -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 13: Failing test — `effective_params(spec)` is the identity map (effective == base)**

  Add to `mod tests` in `crates/jumpgate-core/src/stores.rs`:
  ```rust
      #[test]
      fn effective_equals_base_in_v1() {
          use crate::config::BaseSpec;
          let spec = BaseSpec {
              base_dry_mass: 10.0,
              base_max_thrust: 250.0,
              base_exhaust_velocity: 30.0,
              base_fuel_capacity: 40.0,
          };
          let eff = effective_params(&spec);
          assert_eq!(eff.dry_mass, spec.base_dry_mass);
          assert_eq!(eff.max_thrust, spec.base_max_thrust);
          assert_eq!(eff.exhaust_velocity, spec.base_exhaust_velocity);
          assert_eq!(eff.fuel_capacity, spec.base_fuel_capacity);
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::effective_equals_base_in_v1 -- --nocapture
  ```
  EXPECTED: compile error — `cannot find function effective_params`.

- [ ] **Step 14: Implement `effective_params`**

  Insert above the `#[cfg(test)]` module in `crates/jumpgate-core/src/stores.rs`, after the `Effective` struct:
  ```rust
  /// The ONLY accessor the integrator/autopilot read for ship params. v1: identity.
  pub fn effective_params(spec: &BaseSpec) -> Effective {
      Effective {
          dry_mass: spec.base_dry_mass,
          max_thrust: spec.base_max_thrust,
          exhaust_velocity: spec.base_exhaust_velocity,
          fuel_capacity: spec.base_fuel_capacity,
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::effective_equals_base_in_v1 -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 15: Failing test — `ShipStore`/`BodyStore` SoA layouts construct via `empty()` and stay length-parallel (incl. the two prev-* hash arrays)**

  The stores are plain SoA `Vec`s per the contract; `ids: SlotMap<()>` is the slot/gen authority. `empty()` builds a zero-craft store with all arrays empty. This test builds an empty store, asserts every parallel array (including `prev_fuel` and `prev_inside_dest`) starts empty, and constructs a one-body store. `Lod` comes from `types.rs` (Task 3). Add to `mod tests` in `crates/jumpgate-core/src/stores.rs`:
  ```rust
      #[test]
      fn stores_construct_soa_parallel() {
          let ship = ShipStore::empty();
          assert_eq!(ship.ids.len(), 0);
          let n = ship.ids.len();
          assert_eq!(ship.pos.len(), n);
          assert_eq!(ship.vel.len(), n);
          assert_eq!(ship.fuel_mass.len(), n);
          assert_eq!(ship.spec.len(), n);
          assert_eq!(ship.nav.len(), n);
          assert_eq!(ship.lod.len(), n);
          // the two prev-* arrays reserved for the edge-triggered-event hash path
          // (Task 11 copies into them; hash.rs writes them) start empty and parallel.
          assert_eq!(ship.prev_fuel.len(), n);
          assert_eq!(ship.prev_inside_dest.len(), n);

          let mut body = BodyStore {
              ids: SlotMap::new(),
              mass: Vec::new(),
              eph_index: Vec::new(),
          };
          let (bslot, bgen) = body.ids.insert(());
          let bid = BodyId { slot: bslot, gen: bgen };
          body.mass.push(1.0);
          body.eph_index.push(0);
          assert_eq!(bid.slot, bslot);
          assert_eq!(body.mass.len(), body.ids.len());
          assert_eq!(body.eph_index.len(), body.ids.len());
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::stores_construct_soa_parallel -- --nocapture
  ```
  EXPECTED: compile error — `cannot find type ShipStore` / `BodyStore` / `Lod` / `no function named empty`.

- [ ] **Step 16: Implement `ShipStore` (incl. prev-* arrays) and `BodyStore`, plus `ShipStore::empty()`**

  `Lod` and `BaseSpec` are Task-3 types (`types.rs`, `config.rs`); `CraftId`/`BodyId` from `ids.rs`. Extend the `use` block at the top of `crates/jumpgate-core/src/stores.rs` to bring in `Lod`, `BodyId`, and `CraftId` (`CraftId`/`BodyId` are used by the accessors added in Step 18 and the body test). Replace the existing import block from Step 12 with:
  ```rust
  use crate::config::BaseSpec;
  use crate::ids::{BodyId, CraftId, SlotMap};
  use crate::math::Vec3;
  use crate::types::{Lod, NavDest};
  ```
  Then add the two structs above the `#[cfg(test)]` module:
  ```rust
  /// SoA store for mobile craft. `ids` is the slot/gen authority; every other Vec
  /// is indexed by the same dense row (v1 invariant: `slot == row`) and must stay
  /// length-parallel. `prev_fuel` / `prev_inside_dest` snapshot the previous tick's
  /// values for edge-triggered event detection (Task 11 copies into them at the end
  /// of `World::step`; hash.rs folds them into the per-tick hash in canonical order).
  pub struct ShipStore {
      pub ids: SlotMap<()>,
      pub pos: Vec<Vec3>,
      pub vel: Vec<Vec3>,
      pub fuel_mass: Vec<f64>,
      pub spec: Vec<BaseSpec>,
      pub nav: Vec<NavState>,
      pub lod: Vec<Lod>,
      pub prev_fuel: Vec<f64>,
      pub prev_inside_dest: Vec<bool>,
  }

  /// SoA store for massive on-rails bodies. `eph_index` maps a body slot to its
  /// row in the precomputed ephemeris table (§5.4).
  pub struct BodyStore {
      pub ids: SlotMap<()>,
      pub mass: Vec<f64>,
      pub eph_index: Vec<usize>,
  }

  impl ShipStore {
      /// A zero-craft store with every SoA array empty. All craft are minted via
      /// `push` at reset; there is no mid-run despawn in v1, so slots allocate
      /// contiguously and `slot == row` holds.
      pub fn empty() -> Self {
          ShipStore {
              ids: SlotMap::new(),
              pos: Vec::new(),
              vel: Vec::new(),
              fuel_mass: Vec::new(),
              spec: Vec::new(),
              nav: Vec::new(),
              lod: Vec::new(),
              prev_fuel: Vec::new(),
              prev_inside_dest: Vec::new(),
          }
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::stores_construct_soa_parallel -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 17: Failing test — `push` / `ids_at` / `index_of` / `craft_pos_by_id` / `craft_fuel_capacity`**

  These are the typed-id store accessors Tasks 10/16 read through. `push(spec,pos,vel,fuel)` mints a `CraftId`, appends every SoA array (initializing `nav=Idle`, `lod=Player`, `prev_fuel=fuel`, `prev_inside_dest=false`) keeping them length-parallel, and enforces `slot == row`. `index_of` resolves a typed id to its dense row (stale id -> `None`). `craft_fuel_capacity` reads through `effective_params` (single source of truth — capacity comes from `spec`, never a divergent copy). Add to `mod tests` in `crates/jumpgate-core/src/stores.rs`:
  ```rust
      #[test]
      fn shipstore_push_and_accessors() {
          let mut ship = ShipStore::empty();
          let spec = BaseSpec {
              base_dry_mass: 10.0,
              base_max_thrust: 250.0,
              base_exhaust_velocity: 30.0,
              base_fuel_capacity: 40.0,
          };
          let id0 = ship.push(spec.clone(), Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, 40.0);
          let id1 = ship.push(spec.clone(), Vec3::new(2.0, 0.0, 0.0), Vec3::ZERO, 20.0);
          assert_eq!(id0, CraftId { slot: 0, gen: 0 });
          assert_eq!(id1, CraftId { slot: 1, gen: 0 });

          // every SoA array stayed length-parallel, including the prev-* pair.
          let n = ship.ids.len();
          assert_eq!(n, 2);
          assert_eq!(ship.pos.len(), n);
          assert_eq!(ship.vel.len(), n);
          assert_eq!(ship.fuel_mass.len(), n);
          assert_eq!(ship.spec.len(), n);
          assert_eq!(ship.nav.len(), n);
          assert_eq!(ship.lod.len(), n);
          assert_eq!(ship.prev_fuel.len(), n);
          assert_eq!(ship.prev_inside_dest.len(), n);

          // ids_at wraps the dense row into a typed CraftId.
          assert_eq!(ship.ids_at(0), id0);
          assert_eq!(ship.ids_at(1), id1);

          // index_of resolves a live typed id to its row; stale -> None.
          assert_eq!(ship.index_of(id0), Some(0));
          assert_eq!(ship.index_of(id1), Some(1));
          let stale = CraftId { slot: 0, gen: 99 };
          assert_eq!(ship.index_of(stale), None, "stale gen -> None");

          // craft_pos_by_id reads the row's position; stale -> None.
          assert_eq!(ship.craft_pos_by_id(id0), Some(Vec3::new(1.0, 0.0, 0.0)));
          assert_eq!(ship.craft_pos_by_id(id1), Some(Vec3::new(2.0, 0.0, 0.0)));
          assert_eq!(ship.craft_pos_by_id(stale), None);

          // craft_fuel_capacity reads through effective_params (spec is the single
          // source of truth), NOT current fuel_mass.
          assert_eq!(ship.craft_fuel_capacity(id0), Some(40.0));
          assert_eq!(ship.craft_fuel_capacity(id1), Some(40.0));
          assert_eq!(ship.craft_fuel_capacity(stale), None);

          // initial nav/prev-* defaults set by push.
          assert!(matches!(ship.nav[0], NavState::Idle));
          assert_eq!(ship.prev_fuel[1], 20.0);
          assert!(!ship.prev_inside_dest[0]);
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::shipstore_push_and_accessors -- --nocapture
  ```
  EXPECTED: compile error — `no method named push` / `ids_at` / `index_of` / `craft_pos_by_id` / `craft_fuel_capacity`.

- [ ] **Step 18: Implement the `ShipStore` accessors**

  Insert into the `impl ShipStore` block in `crates/jumpgate-core/src/stores.rs`, after `empty()`. `push` enforces the v1 `slot == row` invariant via `debug_assert!` BEFORE appending — that assert is what makes dense indexing by slot safe. `index_of` delegates to the slot-map's `dense_index`; `ids_at` wraps `id_at`; `craft_fuel_capacity` reads through `effective_params` so capacity has one source of truth (`spec`).
  ```rust
      /// Append a craft, returning its typed `CraftId`. Initializes `nav = Idle`,
      /// `lod = Player`, and the prev-* snapshots (`prev_fuel = fuel`,
      /// `prev_inside_dest = false`). Enforces the v1 `slot == row` invariant.
      pub fn push(&mut self, spec: BaseSpec, pos: Vec3, vel: Vec3, fuel: f64) -> CraftId {
          let (slot, gen) = self.ids.insert(());
          debug_assert_eq!(
              slot as usize,
              self.pos.len(),
              "v1 invariant violated: slot must equal dense row (no mid-run despawn)"
          );
          self.pos.push(pos);
          self.vel.push(vel);
          self.fuel_mass.push(fuel);
          self.spec.push(spec);
          self.nav.push(NavState::Idle);
          self.lod.push(Lod::Player);
          self.prev_fuel.push(fuel);
          self.prev_inside_dest.push(false);
          CraftId { slot, gen }
      }

      /// The typed `CraftId` occupying dense row `idx`. Panics if `idx` is not a
      /// live row (callers iterate `0..ids.len()` over a no-despawn v1 store).
      pub fn ids_at(&self, idx: usize) -> CraftId {
          let (slot, gen) = self
              .ids
              .id_at(idx)
              .expect("ids_at called with a non-live dense row");
          CraftId { slot, gen }
      }

      /// Dense SoA row for a live `CraftId`, or `None` for a stale/unknown id.
      pub fn index_of(&self, id: CraftId) -> Option<usize> {
          self.ids.dense_index(id.slot, id.gen)
      }

      /// Position of a live craft by id, or `None` if the id is stale.
      pub fn craft_pos_by_id(&self, id: CraftId) -> Option<Vec3> {
          self.index_of(id).map(|i| self.pos[i])
      }

      /// Effective fuel capacity of a live craft by id, read through
      /// `effective_params` (capacity's single source of truth is `spec`). `None`
      /// for a stale id.
      pub fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
          self.index_of(id)
              .map(|i| effective_params(&self.spec[i]).fuel_capacity)
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core --lib stores::tests::shipstore_push_and_accessors -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 19: Full module test run + clippy gate**

  Run the whole crate test suite and the lint that catches determinism-banned methods in test modules too (binary/lib crate: must use `--all-targets`, not `--lib`, per the project's clippy note):
  ```
  cargo test -p jumpgate-core --lib ids:: stores::
  ```
  EXPECTED: `test result: ok.` with all Task-4 tests passing (ids: 5 tests — `ids_are_copy_and_ord`, `insert_get_len_cursor`, `remove_invalidates_old_id_not_replacement`, `dense_index_id_at_iter_ids`, `cursor_is_deterministic`; stores: 4 tests — `navstate_and_effective_shapes`, `effective_equals_base_in_v1`, `stores_construct_soa_parallel`, `shipstore_push_and_accessors`).

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings (no `SystemTime`/`Instant::now`/`thread_rng` usage introduced; SoA stores are plain `Vec`; no import of the not-yet-existing `crate::contract`).

- [ ] **Step 20: Commit**

  Stage only the files this task owns and commit. (On `main`: create a branch first if the project workflow requires it; otherwise commit directly per the orchestration convention.)
  ```
  git add crates/jumpgate-core/src/ids.rs crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  Task 4: generational slot-map + per-type stores skeleton

  - CraftId/BodyId (Copy, Ord by (slot,gen)) for canonical hash order
  - SlotMap<T>: insert/get/remove with gen validation; cursor() high-water
    mark is hashed state and never shrinks on remove; dense_index/id_at/
    iter_ids navigation surface (None-on-stale-gen) for Tasks 10-13
  - ShipStore/BodyStore SoA layouts incl. prev_fuel/prev_inside_dest hash
    arrays; empty()/push/ids_at/index_of/craft_pos_by_id/craft_fuel_capacity
    accessors (Tasks 10/16); v1 slot==row invariant asserted in push
  - NavState resolved field; Effective + effective_params identity accessor
    (v1 effective == base); seam types imported from types.rs (Task 3),
    NOT contract.rs (Task 6, not yet built)

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: one commit created with the three files.

---

**Notes for the implementer**
- `SlotMap<T>::insert` returns a `(u32, u32)` `(slot, gen)` tuple rather than a typed id because both `ShipStore` and `BodyStore` use `SlotMap<()>` and must mint two distinct nominal id types (`CraftId`/`BodyId`) from the same generic map. `id_at` likewise returns the tuple; the typed store (`ids_at`) wraps it. Callers wrap the tuple into the appropriate id struct.
- The contract pins only `new`/`len`/`cursor` as `SlotMap`'s public signatures; `insert`/`get`/`remove`/`is_empty`/`Default`/`dense_index`/`id_at`/`iter_ids` are the implementation API the downstream tasks require. None contradicts the contract.
- **v1 `slot == row` invariant:** all craft are minted at reset and never despawned mid-run, so slots allocate contiguously and the SoA arrays stay aligned with the slot number. `ShipStore::push` asserts this with `debug_assert_eq!(slot as usize, self.pos.len())`. This is the single decision that makes `dense_index`/`index_of` correct; do NOT introduce compaction or swap-remove (that would reorder rows and break both determinism and the spec's "plain SoA Vec" directive).
- `cursor()` is the slots-ever-allocated high-water mark (`values.len()`), deterministic and monotonic — it must be folded into the per-tick state hash by the hash.rs task, so removal must NOT shrink it (asserted in Steps 6 and 10).
- **Hash-reservation only:** `prev_fuel` / `prev_inside_dest` are added as fields HERE, but the copy-at-end-of-`World::step` and the FNV write live in the later World/hash tasks — they reference `World`, which does not exist at Task 4. Do not implement that logic here. Their slot in the canonical HASH_FIELD_ORDER is owned by the hash.rs task.
- **Module ordering / no forward import:** seam types (`Lod`, `NavDest`) come from `crate::types` (Task 3's `types.rs`), NOT `crate::contract`, which is built in Task 6 AFTER this task. `lib.rs` at this task's exit declares only modules whose files exist at this point (`math`, `time`, `types`, `config`, `ids`, `stores`, plus the `rng` module if Task 3 landed it) — it must NOT declare `pub mod contract`.
- `craft_fuel_capacity` reads capacity through `effective_params(&spec[idx])` so `BaseSpec` is the single source of truth; there is no independent mutable capacity array that could desync. If a later task needs mutable capacity (refuel-to-cap), that task adds the array — do not over-build it now.
- All code is `#![forbid(unsafe_code)]`-clean: pure `Vec`/`Option` logic, no unsafe, no time/RNG/env calls.


---

### Task 5: Named RNG sub-streams (ChaCha8)

Derive separate, replay-stable `ChaCha8Rng` instances from one master `u64` seed via a **fixed per-stream salt derivation** — never by drawing from a shared parent (spec §6 / line 191: a shared parent couples stream-creation order to draw order and kills replay). Implements the `RngStreams` / `RngStream` contract types verbatim.

**Version family (workspace-wide, NON-NEGOTIABLE):** `rand_chacha = "=0.10.0"`, `rand_core = "=0.10.1"`. This is the single family the determinism contract was authored against (Task 1 verified-facts block) and the one EVERY subsequent task (6, 7-12, 14, 17) must use. The `rand` crate is NOT a dependency of the core crate — `ChaCha8Rng` plus the `rand_core` traits are sufficient.

**Verified API facts (rand_core 0.10.1 / rand_chacha 0.10.0, confirmed against a throwaway probe crate before writing this task):**
- `rand_core::SeedableRng::seed_from_u64(u64)` EXISTS in 0.10 and is the correct seeder. (The fix-list "else `from_seed([u8;32])` over a hash" fallback is therefore NOT needed and is not used here.)
- `next_u64()` is provided by the **`rand_core::Rng`** trait, NOT `rand_core::RngCore`. In 0.10 `RngCore` exposes only the fallible `try_next_u64()`. Tests MUST `use rand_core::Rng;` to call `next_u64()` — importing `RngCore` for this (the 0.3.x idiom) does NOT compile in 0.10.
- `seed_from_u64` is not on any clippy `disallowed_methods` list; only `thread_rng` / `from_entropy` are banned (Task 2). The derivation below uses only `seed_from_u64`, so clippy stays clean.
- Within one `RngStreams` instance the two streams never alias: `SALT_INTERVENTION != SALT_SCENARIO` ⇒ `master^SALT_INTERVENTION != master^SALT_SCENARIO` for every `master`. (A cross-master collision is mathematically possible — `master_b = master_a ^ SALT_INTERVENTION ^ SALT_SCENARIO` — but that requires two distinct masters and never affects single-episode replay, which is what determinism depends on.)

**Files**
- Create: `crates/jumpgate-core/src/rng.rs`
- Modify: `crates/jumpgate-core/src/lib.rs` (add `pub mod rng;`)
- Modify: `Cargo.toml` (ensure `rand_chacha` / `rand_core` pinned in `[workspace.dependencies]`)
- Modify: `crates/jumpgate-core/Cargo.toml` (ensure both deps inherited)
- Test: `crates/jumpgate-core/src/rng.rs` (inline `#[cfg(test)] mod tests`)

**Depends on:** Task 3 (config/scaffold). Assumes `jumpgate-core` already compiles and `lib.rs` exists with module declarations. `rng.rs` has NO intra-crate dependencies (it imports only `rand_chacha` / `rand_core`), so it slots in without touching the math→time→types→ids→config→contract→stores ordering.

---

- [ ] **Step 1: Pin `rand_chacha` and `rand_core` in the workspace manifest.**
  Open `Cargo.toml` (workspace root). Under `[workspace.dependencies]`, ensure these exact pinned lines are present (add them if Task 1/3 did not; if a different version is already pinned, CHANGE it to these — the whole workspace is one family). `ChaCha8Rng` is the version-stable generator we depend on (`StdRng` is not version-stable), which is why we pin `rand_chacha`, per spec line 191.
  ```toml
  [workspace.dependencies]
  rand_chacha = "=0.10.0"
  rand_core = "=0.10.1"
  ```
  (Leave any other existing `[workspace.dependencies]` entries untouched. Do NOT add a `rand` entry — the core crate does not use it.)

- [ ] **Step 2: Inherit both deps in the core crate manifest.**
  Open `crates/jumpgate-core/Cargo.toml`. Under `[dependencies]`, ensure exactly:
  ```toml
  [dependencies]
  rand_chacha = { workspace = true }
  rand_core = { workspace = true }
  ```
  Resolve the lockfile by building (do NOT use `cargo update --precise` — the `"=0.10.0"`/`"=0.10.1"` exact-version requirements already force the lockfile to these versions; an explicit `--precise` re-pin is what introduced the 0.3.x regression and is removed):
  ```
  cargo build -p jumpgate-core
  ```
  EXPECTED: builds clean. Then verify the resolved versions:
  ```
  cargo tree -p jumpgate-core -i rand_chacha && cargo tree -p jumpgate-core -i rand_core
  ```
  EXPECTED output contains: `rand_chacha v0.10.0` and `rand_core v0.10.1`. If either resolves to anything else, the `=` pin in Step 1 is missing or wrong — fix the manifest, do not patch the lockfile by hand.

- [ ] **Step 3: Write a failing test module + minimal type skeleton in `rng.rs`.**
  Create `crates/jumpgate-core/src/rng.rs` with the contract types declared but the body of `stream()` left as `unimplemented!()`, plus the full test suite. This compiles but the tests panic — proving the tests actually exercise the code path. Note the imports: `use rand_core::SeedableRng;` for seeding (non-test), and `use rand_core::Rng;` inside `mod tests` for `next_u64()`.
  ```rust
  //! Named RNG sub-streams (Task 5).
  //!
  //! One master `u64` seed → several SEPARATE `ChaCha8Rng` instances, each seeded
  //! by a FIXED derivation `master ^ SALT[stream]`. Streams are never drawn from a
  //! shared parent: that would couple stream-creation order to draw order and break
  //! Tier-B replay (spec §6 / line 191). Distinct salts give independent sequences;
  //! the same `(master, stream)` always reproduces the same sequence.
  //!
  //! Pinned to rand_chacha 0.10.0 / rand_core 0.10.1. In this family `seed_from_u64`
  //! lives on `rand_core::SeedableRng`, and the infallible `next_u64` lives on
  //! `rand_core::Rng` (NOT `RngCore`, which only exposes the fallible `try_next_u64`).

  use rand_chacha::ChaCha8Rng;
  use rand_core::SeedableRng;

  /// The named sub-streams available in v1. `Intervention` carries lever/perturbation
  /// randomness; `Scenario` carries initial-condition / loadout randomness.
  #[derive(Clone, Copy)]
  pub enum RngStream {
      Intervention,
      Scenario,
  }

  /// Per-stream salt constants. Fixed forever (changing one changes replay identity
  /// for that stream). Two unrelated 64-bit constants so `master ^ SALT` never aliases
  /// across the two streams for any single master (since the salts differ).
  const SALT_INTERVENTION: u64 = 0x9E37_79B9_7F4A_7C15;
  const SALT_SCENARIO: u64 = 0xC2B2_AE3D_27D4_EB4F;

  impl RngStream {
      /// Fixed salt for this stream. `const fn` so the derivation is unambiguous and
      /// has no runtime state.
      const fn salt(self) -> u64 {
          match self {
              RngStream::Intervention => SALT_INTERVENTION,
              RngStream::Scenario => SALT_SCENARIO,
          }
      }
  }

  /// Holds one independent `ChaCha8Rng` per named stream, all derived from a single
  /// master seed. Construction order is irrelevant — each stream is a pure function
  /// of `(master, stream)`.
  pub struct RngStreams {
      intervention: ChaCha8Rng,
      scenario: ChaCha8Rng,
  }

  impl RngStreams {
      /// Seed every named stream from `master` via its fixed salt derivation.
      /// `seed_from_u64` (rand_core 0.10) deterministically expands the u64 into the
      /// 32-byte ChaCha seed; pinned versions make this reproducible across runs.
      pub fn from_master(master: u64) -> Self {
          RngStreams {
              intervention: ChaCha8Rng::seed_from_u64(master ^ RngStream::Intervention.salt()),
              scenario: ChaCha8Rng::seed_from_u64(master ^ RngStream::Scenario.salt()),
          }
      }

      /// Borrow the named stream's generator for drawing.
      pub fn stream(&mut self, _which: RngStream) -> &mut ChaCha8Rng {
          unimplemented!("Step 5")
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      // 0.10: `next_u64` is on `rand_core::Rng`, NOT `RngCore`.
      use rand_core::Rng;

      fn draw_n(rng: &mut ChaCha8Rng, n: usize) -> Vec<u64> {
          (0..n).map(|_| rng.next_u64()).collect()
      }

      #[test]
      fn same_master_reproduces_each_stream() {
          let mut a = RngStreams::from_master(42);
          let mut b = RngStreams::from_master(42);
          assert_eq!(
              draw_n(a.stream(RngStream::Intervention), 8),
              draw_n(b.stream(RngStream::Intervention), 8),
              "Intervention sequence must reproduce for the same master"
          );
          assert_eq!(
              draw_n(a.stream(RngStream::Scenario), 8),
              draw_n(b.stream(RngStream::Scenario), 8),
              "Scenario sequence must reproduce for the same master"
          );
      }

      #[test]
      fn distinct_streams_differ() {
          let mut s = RngStreams::from_master(42);
          let iv = draw_n(s.stream(RngStream::Intervention), 8);
          let sc = draw_n(s.stream(RngStream::Scenario), 8);
          assert_ne!(iv, sc, "Intervention and Scenario must not produce the same sequence");
      }

      #[test]
      fn streams_are_independent_of_draw_order() {
          // Draining Intervention must not perturb Scenario: the streams are separate
          // ChaCha8Rng instances, NOT siblings off a shared parent.
          let mut drained = RngStreams::from_master(7);
          for _ in 0..1000 {
              drained.stream(RngStream::Intervention).next_u64();
          }
          let sc_after_drain = draw_n(drained.stream(RngStream::Scenario), 8);

          let mut fresh = RngStreams::from_master(7);
          let sc_fresh = draw_n(fresh.stream(RngStream::Scenario), 8);

          assert_eq!(
              sc_after_drain, sc_fresh,
              "Scenario must be unaffected by draws from Intervention"
          );
      }

      #[test]
      fn different_masters_differ_on_every_stream() {
          // Strengthened: divergence must hold on BOTH named streams, not just one.
          // A weak single-stream check could pass while the other stream silently
          // collapsed to a master-independent sequence.
          let mut a = RngStreams::from_master(1);
          let mut b = RngStreams::from_master(2);
          assert_ne!(
              draw_n(a.stream(RngStream::Intervention), 8),
              draw_n(b.stream(RngStream::Intervention), 8),
              "Different masters must yield different Intervention sequences"
          );

          let mut a2 = RngStreams::from_master(1);
          let mut b2 = RngStreams::from_master(2);
          assert_ne!(
              draw_n(a2.stream(RngStream::Scenario), 8),
              draw_n(b2.stream(RngStream::Scenario), 8),
              "Different masters must yield different Scenario sequences"
          );
      }

      #[test]
      fn golden_first_draws_are_pinned() {
          // VERSION/API-DRIFT GUARD. "Same run reproduces" only proves a run agrees
          // with itself; it cannot catch a silent rand_chacha/rand_core bump that
          // changes the byte stream. These hardcoded constants were captured against
          // rand_chacha=0.10.0 / rand_core=0.10.1 and pin the actual sequence.
          // If this test fails, the RNG version family changed and EVERY recorded
          // replay's state hashes are invalidated — that is a deliberate, reviewed
          // event, not a number to silently re-baseline.
          let mut s = RngStreams::from_master(0);
          let iv0 = s.stream(RngStream::Intervention).next_u64();
          let sc0 = s.stream(RngStream::Scenario).next_u64();
          assert_eq!(iv0, 0xa6ab_1181_2ab1_c509, "Intervention[master=0] first draw drifted");
          assert_eq!(sc0, 0x4f53_8dce_87ab_d2df, "Scenario[master=0] first draw drifted");
      }
  }
  ```

- [ ] **Step 4: Wire the module into `lib.rs`.**
  Open `crates/jumpgate-core/src/lib.rs` and add the module declaration alongside the other `pub mod` lines (keep them alphabetically grouped if the file is ordered that way). Declare ONLY this module here — do not add declarations for files later tasks create.
  ```rust
  pub mod rng;
  ```

- [ ] **Step 5: Run the tests and confirm they FAIL (red).**
  ```
  cargo test -p jumpgate-core rng -- --nocapture
  ```
  EXPECTED: all five `rng::tests::*` tests panic with `not implemented: Step 5` (the `unimplemented!()` in `stream()`), e.g.
  ```
  test rng::tests::same_master_reproduces_each_stream ... FAILED
  ...
  test result: FAILED. 0 passed; 5 failed; 0 ignored
  ```

- [ ] **Step 6: Implement `stream()` (minimal, green).**
  Replace the `unimplemented!` body in `crates/jumpgate-core/src/rng.rs`:
  ```rust
      /// Borrow the named stream's generator for drawing.
      pub fn stream(&mut self, which: RngStream) -> &mut ChaCha8Rng {
          match which {
              RngStream::Intervention => &mut self.intervention,
              RngStream::Scenario => &mut self.scenario,
          }
      }
  ```

- [ ] **Step 7: Run the tests and confirm they PASS (green).**
  ```
  cargo test -p jumpgate-core rng -- --nocapture
  ```
  EXPECTED:
  ```
  test rng::tests::different_masters_differ_on_every_stream ... ok
  test rng::tests::distinct_streams_differ ... ok
  test rng::tests::golden_first_draws_are_pinned ... ok
  test rng::tests::same_master_reproduces_each_stream ... ok
  test rng::tests::streams_are_independent_of_draw_order ... ok
  test result: ok. 5 passed; 0 failed; 0 ignored
  ```
  If `golden_first_draws_are_pinned` is the only failure, the resolved `rand_chacha`/`rand_core` versions are NOT 0.10.0/0.10.1 — return to Step 2 and fix the pin; do not edit the golden constants.

- [ ] **Step 8: Confirm no banned-RNG lint regression.**
  The core crate bans `thread_rng` / `from_entropy` via `clippy.toml` (Task 2). Our derivation uses only `seed_from_u64`, so clippy must stay clean.
  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings; specifically no `disallowed_methods` hit for `from_entropy`/`thread_rng`.

- [ ] **Step 9: Commit.**
  ```
  git add Cargo.toml Cargo.lock crates/jumpgate-core/Cargo.toml crates/jumpgate-core/src/rng.rs crates/jumpgate-core/src/lib.rs
  git commit -m "feat(core): named ChaCha8 RNG sub-streams via fixed salt derivation

Task 5: RngStreams derives separate ChaCha8Rng instances from one master
seed (master ^ per-stream salt), never from a shared parent, so stream
draws are independent of creation/draw order (Tier-B replay, spec §6).
Pinned to rand_chacha=0.10.0 / rand_core=0.10.1 (0.10 API: seed_from_u64
on SeedableRng, next_u64 on rand_core::Rng). Golden-value test pins the
actual byte stream so a version bump fails loudly.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```
  EXPECTED: commit succeeds; `git status` clean for these paths.

---

**Note on the fix-list items that touch other tasks (NOT applied here, flagged for the owning task):**
- *Cross-task contract-surface doc, types.rs split, acyclic module ordering, lib.rs declaring only existing modules, FNV HASH_FIELD_ORDER/HASH_VERSION/golden-hash, single `contract::Integrator`, Lod `Wake` + dispatch in `World::step`* — these belong to Tasks 3/4/6/8/11 and the new pre-Task-3 contract-surface document. Task 5 contributes nothing to the FNV hash input (RngStreams state is not hashed; only the master seed is, via `RunConfig::config_hash` in Task 3), and adds no cross-task symbols beyond `RngStreams`/`RngStream`/`RngStream::salt` whose only downstream consumer is `World` (Task 7+). Both public methods (`from_master`, `stream`) are exercised by this task's own suite, satisfying the provider-covers-its-surface rule.
- *Seed-divergence concern relocation to Task 18:* the seed→sequence divergence property is fully covered here (the strengthened `different_masters_differ_on_every_stream` + the golden test). What Task 18 should additionally own is the END-TO-END consequence — that two `World::reset` calls with different `master_seed` produce different per-tick state hashes, and that the same seed reproduces the recorded hash trace under `replay_run`. That is a `World`/replay-level assertion (it needs the full step pipeline), not an `rng.rs` unit test, so it correctly lives in Task 18, not here.

---

### Task 6: Shared contract types (the drift lock)

**Goal:** Land the cross-cutting shared **DTOs and read/integrate traits** that the command/event/replay format and all three facades agree on — `Command`, `command_sort_key`, `EventKind`/`Event`, and the `Integrator` and `StateView` traits — compiled with stubbed/trivial bodies so every downstream task implements against the *real* types. Getting these wrong is a contract break across every caller, which is why this task is the "drift lock." The primitive seam *enums* (`Lod`, `NavDest`, `Target`, `EntityRef`, `CommandKind`) are **not** defined here — they were split into `types.rs` in Task 3 so `stores.rs` (Task 4) can consume them without a cycle; this task **imports** them. The only real logic is `command_sort_key`, driven by TDD.

**Drift-lock anchor (read before editing):** Per the spec (§5.2, §4.4) the `Integrator` trait and the `StateView` trait are each defined **exactly once, here in `contract.rs`**. `integrator.rs` (Task 8) writes `use crate::contract::Integrator;` and supplies **only** impls — it must NOT re-declare the trait and must NOT `pub use` re-export it. A second same-shaped trait would silently mis-bind: concrete types would implement the wrong one while `World` holds `Box<dyn contract::Integrator>`. Same single-definition rule applies to `StateView` (impl'd for `World` in Task 12).

**Files**
- Create: `crates/jumpgate-core/src/contract.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/contract.rs` (inline `#[cfg(test)] mod tests`)

**Depends on:**
- Task 3 (`types.rs`: `Lod`, `NavDest`, `Target`, `EntityRef`, `CommandKind`).
- Task 4 (`math::Vec3`, `ids::{CraftId, BodyId}`, `time::{Tick, Dt}`).

**Module-ordering assumption (flag for the Task 3 repairer; do NOT fix here):** the fix-#6 sequence as literally written, `math -> time -> types -> ids`, is **inverted** — `types.rs` defines `EntityRef = Craft(CraftId) | Body(BodyId)` and `NavDest = Position(Vec3) | Entity(EntityRef)`, so it references `CraftId`/`BodyId`/`Vec3` and **cannot** precede `ids`/`math` (Rust has no forward declaration). The correct acyclic order is `math -> time -> ids -> types -> config -> contract -> stores`. This task assumes Task 3 lands `types.rs` in that order; by the time `contract.rs` builds, `types.rs` already compiles, so Task 6 itself is unblocked either way.

**Provides → consumed-by (cross-task contract surface for this module):**
- `Command`, `command_sort_key` → consumed by `ingest.rs` (Task 9, canonical-order apply) and `replay.rs` (Task 17).
- `EventKind` (incl. new `Wake` variant), `Event` → consumed by `events.rs` (Task 10/11) and `replay.rs`.
- `Integrator` → consumed by `integrator.rs` (Task 8, impls only) and `world.rs` (Task 12, `Box<dyn Integrator>`).
- `StateView` (incl. new `craft_fuel_capacity`) → impl'd for `World` in Task 12; read by `jumpgate-py` obs/reward.
- Per the fix-#4 RULE (the provider defines every method downstream calls **and** its own test suite covers them), every exported method below is exercised by the `Dummy`/`DummyView` tests in Steps 4/6 — including the two new surface elements added in this repair.

**Notes on derives (load-bearing — copy exactly):**
- `Command`, `EventKind`, `Event` are `PartialEq` only (no `Eq`) — `Vec3`/`f64` flows through them (via `NavDest`/`burn_budget`/`dv`/`value`). They are still `Copy` (every field is `Copy`, including the new `Wake { craft: CraftId }`).
- `command_sort_key` returns `(u8, u32, u32)` = `(scope_rank, slot, gen)` with `Sim=0, World=1, Entity=2`. A Craft and a Body with identical `slot`/`gen` deliberately map to the same key; ordering stays deterministic because callers use the **stable** `sort_by_key`. Do NOT fold a Craft/Body discriminator into `scope_rank` — the signature and rank scheme are pinned by the contract.

**This repair vs the original Task 6 (what changed):**
1. The 5 seam enums (`Lod`, `NavDest`, `Target`, `EntityRef`, `CommandKind`) are **imported from `crate::types`**, not defined here (fix #6 — resolves the contract↔stores cycle).
2. `StateView` gains `craft_fuel_capacity(&self, id: CraftId) -> Option<f64>` (fix #2), backed in the real impl by `effective_params(&spec).fuel_capacity` — never `base_fuel_capacity` (§5.5 forbids readers touching `base_*`).
3. `EventKind` gains a `Wake { craft: CraftId }` variant (fix #10); the LOD-dispatch branch that *emits* it lives in `World::step` (Task 12), not here.
4. The four prev-state / nav accessors stay **off** `StateView` (fix #3) — see the NOTE below.

**NOTE for the Task 11 repairer (fix #3 — decision made here, do NOT add to StateView):** `detect_boundary_events` needs previous-tick `prev_fuel` / `prev_inside_dest` (and nav) bookkeeping to detect `Arrival`/`FuelEmpty` edges. That state is **internal event-detection bookkeeping, not a facade read-surface** — it must NOT be added to `StateView`. The contract's `detect_boundary_events(world: &World, …)` already takes `&World`, so Task 11 reads those arrays directly off `World` (or pre-extracts them as `&[f64]`/`&[bool]` params). Adding them to `StateView` would leak internal state across the facade boundary. Relatedly, when Task 11 adds `prev_fuel` / `prev_inside_dest` arrays to the hashed state, it MUST extend the single authoritative `HASH_FIELD_ORDER` list (the fix-#7 anchor) — there is no hash field order in `contract.rs` to touch here.

---

- [ ] **Step 1: Create `contract.rs` importing the seam enums, defining the DTOs/traits, with a `todo!()` stub for the sort key.**

  Create `crates/jumpgate-core/src/contract.rs` with exactly this content. The seam enums come from `crate::types` (Task 3); `command_sort_key`'s body is `todo!()` so the next step's test fails first:

  ```rust
  //! Shared contract types — the cross-cutting DTOs and read/integrate traits
  //! that the ingestion path, event stream, replay format, and all facades
  //! agree on.
  //!
  //! This module is the "drift lock": downstream tasks implement against these
  //! exact names/signatures. Bodies are stubbed where logic does not yet exist.
  //!
  //! Single-definition rule (spec §5.2/§4.4): the `Integrator` and `StateView`
  //! traits are defined ONLY here. `integrator.rs` (Task 8) imports
  //! `crate::contract::Integrator` and writes impls only — it must not
  //! re-declare or `pub use` re-export the trait.
  //!
  //! The primitive seam enums (`Lod`, `NavDest`, `Target`, `EntityRef`,
  //! `CommandKind`) live in `crate::types` (Task 3) so `stores.rs` can consume
  //! them without a contract<->stores cycle; this module imports them.

  use crate::ids::{BodyId, CraftId};
  use crate::math::Vec3;
  use crate::time::{Dt, Tick};
  use crate::types::{CommandKind, EntityRef, Lod, NavDest, Target};

  // ---- command DTO ----

  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Command {
      pub target: Target,
      pub kind: CommandKind,
  }

  /// Total, deterministic ordering across World/Sim/Entity scopes for canonical
  /// apply. Returns `(scope_rank, slot, gen)` with `Sim=0, World=1, Entity=2`.
  pub fn command_sort_key(c: &Command) -> (u8, u32, u32) {
      todo!("implement canonical command ordering")
  }

  // ---- event stream ----

  #[derive(Clone, Copy, Debug, PartialEq)]
  pub enum EventKind {
      Arrival { craft: CraftId, dest: NavDest },
      FuelEmpty { craft: CraftId },
      ThrustApplied { craft: CraftId, dv: f64 },
      ActionIngested { target: Target },
      Reward { craft: CraftId, value: f64 },
      /// Emitted by the LOD-dispatch seam in `World::step` on a
      /// Dormant -> Active transition (the §3.2 wake hook). The
      /// emitting branch is Task 12; the variant is pinned here.
      Wake { craft: CraftId },
  }

  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct Event {
      pub tick: Tick,
      pub kind: EventKind,
  }

  // ---- integrator trait (DEFINED ONCE; Task 8 supplies impls only) ----

  /// Verlet needs body pos at BOTH t_n and t_{n+1}; impls take an ephemeris
  /// sampler. `accel_at` returns gravity(softened) + thrust at a sub-tick.
  pub trait Integrator {
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64 /*sub_t in days*/) -> Vec3,
          dt: f64,
          n_substeps: u32,
      ) -> (Vec3, Vec3);
      fn name(&self) -> &'static str;
  }

  // ---- state-access read trait (DEFINED ONCE; Task 12 impls for World) ----

  /// Read trait ALL facades read through. Carries intent (cmd + event history),
  /// not just physics. Methods reference only ids / Tick / Dt / Vec3 / Command /
  /// Event / Lod, so the trait compiles standalone (no `World` yet).
  pub trait StateView {
      fn tick(&self) -> Tick;
      fn dt(&self) -> Dt;
      fn craft_ids(&self) -> Vec<CraftId>;
      fn craft_pos(&self, id: CraftId) -> Option<Vec3>;
      fn craft_vel(&self, id: CraftId) -> Option<Vec3>;
      fn craft_fuel(&self, id: CraftId) -> Option<f64>;
      /// Effective fuel capacity. The real impl (Task 12) reads
      /// `effective_params(&spec).fuel_capacity` — NEVER `base_fuel_capacity`
      /// (§5.5: physics/readers go through the effective-param accessor).
      fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64>;
      fn body_ids(&self) -> Vec<BodyId>;
      fn body_pos(&self, id: BodyId, tick: Tick) -> Option<Vec3>;
      fn recent_commands(&self, since: Tick) -> &[Command];
      fn recent_events(&self, since: Tick) -> &[Event];
      fn lod(&self, id: CraftId) -> Option<Lod>;
  }
  ```

- [ ] **Step 2: Wire the module into `lib.rs`.**

  Add the module declaration to `crates/jumpgate-core/src/lib.rs`, keeping declarations alphabetical with the already-present modules. At this task's completion the modules that EXIST are exactly: `config`, `ids`, `math`, `time`, `types` (from Tasks 3–4) plus the new `contract`. Declare ONLY modules that exist — do not pre-declare `stores`/`integrator`/etc. (created in later tasks), or the workspace won't compile at this task's exit.

  ```rust
  pub mod contract;
  ```

  The resulting module block should read:

  ```rust
  pub mod config;
  pub mod contract;
  pub mod ids;
  pub mod math;
  pub mod time;
  pub mod types;
  ```

- [ ] **Step 3: Confirm the crate compiles with the stub.**

  Run:

  ```
  cargo build -p jumpgate-core
  ```

  EXPECTED: `Finished \`dev\` profile [unoptimized + debuginfo] target(s)` (no errors; the `todo!()` is allowed in a compiled-but-unreached body). A `warning: unused variable: \`c\`` on `command_sort_key` is acceptable at this step. If the build instead fails with `unresolved import \`crate::types\``, Task 3 has not landed `types.rs` (or landed it after `ids`/`math` in the wrong order) — that is a Task 3 defect, not a Task 6 one (see the ordering assumption above).

- [ ] **Step 4: Add the `command_sort_key` total-order test (RED).**

  Append this inline test module to the bottom of `crates/jumpgate-core/src/contract.rs`. Note the seam enums are imported from `crate::types`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::ids::{BodyId, CraftId};
      use crate::math::Vec3;
      use crate::types::{CommandKind, EntityRef, Lod, NavDest, Target};

      fn dest_cmd(target: Target) -> Command {
          Command {
              target,
              kind: CommandKind::Destination {
                  dest: NavDest::Position(Vec3::ZERO),
                  burn_budget: None,
              },
          }
      }

      #[test]
      fn command_sort_key_total_order() {
          let sim = dest_cmd(Target::Sim);
          let world = dest_cmd(Target::World);
          let craft_a = dest_cmd(Target::Entity(EntityRef::Craft(CraftId {
              slot: 5,
              gen: 0,
          })));
          let craft_b = dest_cmd(Target::Entity(EntityRef::Craft(CraftId {
              slot: 2,
              gen: 1,
          })));
          let body = dest_cmd(Target::Entity(EntityRef::Body(BodyId {
              slot: 3,
              gen: 0,
          })));

          // Scope ranks: Sim=0, World=1, Entity=2.
          assert_eq!(command_sort_key(&sim), (0, 0, 0));
          assert_eq!(command_sort_key(&world), (1, 0, 0));
          assert_eq!(command_sort_key(&craft_a), (2, 5, 0));
          assert_eq!(command_sort_key(&craft_b), (2, 2, 1));
          assert_eq!(command_sort_key(&body), (2, 3, 0));

          // Sorting a shuffled mix yields a total, deterministic order:
          // Sim, World, then entities by (slot, gen).
          let mut v = vec![craft_a, body, sim, craft_b, world];
          v.sort_by_key(command_sort_key);
          let keys: Vec<(u8, u32, u32)> = v.iter().map(command_sort_key).collect();
          assert_eq!(
              keys,
              vec![(0, 0, 0), (1, 0, 0), (2, 2, 1), (2, 3, 0), (2, 5, 0)]
          );
      }
  }
  ```

  Run:

  ```
  cargo test -p jumpgate-core contract -- --nocolor
  ```

  EXPECTED: failure — the test panics in the `todo!()`:
  `test contract::tests::command_sort_key_total_order ... FAILED`
  with `not yet implemented: implement canonical command ordering`, and `test result: FAILED. 0 passed; 1 failed`.

- [ ] **Step 5: Implement `command_sort_key` (GREEN).**

  Replace the `todo!()` body in `crates/jumpgate-core/src/contract.rs`:

  ```rust
  pub fn command_sort_key(c: &Command) -> (u8, u32, u32) {
      match c.target {
          Target::Sim => (0, 0, 0),
          Target::World => (1, 0, 0),
          Target::Entity(EntityRef::Craft(id)) => (2, id.slot, id.gen),
          Target::Entity(EntityRef::Body(id)) => (2, id.slot, id.gen),
      }
  }
  ```

  Run:

  ```
  cargo test -p jumpgate-core contract -- --nocolor
  ```

  EXPECTED: `test contract::tests::command_sort_key_total_order ... ok` and `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Add the enum round-trip and trait-impl compile tests (covering every new surface element).**

  Append these tests inside the existing `mod tests` block in `crates/jumpgate-core/src/contract.rs` (after `command_sort_key_total_order`). Per the fix-#4 RULE, these cover the two surface elements this repair added: the `Wake` `EventKind` variant and `StateView::craft_fuel_capacity`.

  ```rust
  #[test]
  fn enums_round_trip_via_partial_eq() {
      let c = CraftId { slot: 7, gen: 2 };

      // Command equality (PartialEq, holds f64 via burn_budget).
      let cmd = Command {
          target: Target::Entity(EntityRef::Craft(c)),
          kind: CommandKind::Destination {
              dest: NavDest::Entity(EntityRef::Craft(c)),
              burn_budget: Some(1.5),
          },
      };
      assert_eq!(cmd, cmd);
      assert_ne!(cmd.target, Target::World);

      // Event equality (PartialEq).
      let e1 = Event {
          tick: Tick(10),
          kind: EventKind::Arrival {
              craft: c,
              dest: NavDest::Position(Vec3::new(1.0, 2.0, 3.0)),
          },
      };
      let e2 = Event {
          tick: Tick(10),
          kind: EventKind::FuelEmpty { craft: c },
      };
      assert_eq!(e1, e1);
      assert_ne!(e1, e2);

      // New surface (fix #10): the Wake variant is Copy + PartialEq and
      // distinct from other kinds.
      let wake = Event {
          tick: Tick(10),
          kind: EventKind::Wake { craft: c },
      };
      let wake_copy = wake; // Copy
      assert_eq!(wake, wake_copy);
      assert_ne!(wake, e2);

      // Lod is Eq.
      assert_eq!(Lod::Player, Lod::Player);
      assert_ne!(Lod::Player, Lod::Nothing);
  }

  /// Trivial integrator: forward-Euler-ish, proves the trait is object-safe and
  /// implementable against the real signature.
  struct Dummy;
  impl Integrator for Dummy {
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64) -> Vec3,
          dt: f64,
          _n_substeps: u32,
      ) -> (Vec3, Vec3) {
          let a = accel_at(pos, 0.0);
          (pos.add(vel.scale(dt)), vel.add(a.scale(dt)))
      }
      fn name(&self) -> &'static str {
          "dummy"
      }
  }

  #[test]
  fn integrator_trait_is_implementable_and_object_safe() {
      let integ = Dummy;
      let obj: &dyn Integrator = &integ; // object-safety check
      assert_eq!(obj.name(), "dummy");

      let zero_accel = |_p: Vec3, _t: f64| Vec3::ZERO;
      let (p, v) = obj.step_craft(
          Vec3::ZERO,
          Vec3::new(1.0, 0.0, 0.0),
          &zero_accel,
          2.0,
          1,
      );
      assert_eq!(p, Vec3::new(2.0, 0.0, 0.0)); // pos += vel*dt
      assert_eq!(v, Vec3::new(1.0, 0.0, 0.0)); // vel unchanged (zero accel)
  }

  /// Trivial StateView backed by owned Vecs — proves the read trait is usable
  /// without `World`, including the slice-returning intent methods AND the new
  /// `craft_fuel_capacity` accessor (fix #2).
  struct DummyView {
      commands: Vec<Command>,
      events: Vec<Event>,
  }
  impl StateView for DummyView {
      fn tick(&self) -> Tick {
          Tick(0)
      }
      fn dt(&self) -> Dt {
          Dt::new(1.0)
      }
      fn craft_ids(&self) -> Vec<CraftId> {
          Vec::new()
      }
      fn craft_pos(&self, _id: CraftId) -> Option<Vec3> {
          None
      }
      fn craft_vel(&self, _id: CraftId) -> Option<Vec3> {
          None
      }
      fn craft_fuel(&self, _id: CraftId) -> Option<f64> {
          None
      }
      fn craft_fuel_capacity(&self, id: CraftId) -> Option<f64> {
          // Trivial backing: a known id resolves to a capacity, others None.
          // The real impl (Task 12) returns effective_params(&spec).fuel_capacity.
          if id == (CraftId { slot: 0, gen: 0 }) {
              Some(100.0)
          } else {
              None
          }
      }
      fn body_ids(&self) -> Vec<BodyId> {
          Vec::new()
      }
      fn body_pos(&self, _id: BodyId, _tick: Tick) -> Option<Vec3> {
          None
      }
      fn recent_commands(&self, _since: Tick) -> &[Command] {
          &self.commands
      }
      fn recent_events(&self, _since: Tick) -> &[Event] {
          &self.events
      }
      fn lod(&self, _id: CraftId) -> Option<Lod> {
          Some(Lod::Player)
      }
  }

  #[test]
  fn state_view_trait_is_implementable_standalone() {
      let view = DummyView {
          commands: vec![dest_cmd(Target::World)],
          events: vec![Event {
              tick: Tick(1),
              kind: EventKind::ActionIngested {
                  target: Target::World,
              },
          }],
      };
      let obj: &dyn StateView = &view; // object-safety check
      assert_eq!(obj.tick(), Tick(0));
      assert_eq!(obj.dt().get(), 1.0);
      assert_eq!(obj.recent_commands(Tick(0)).len(), 1);
      assert_eq!(obj.recent_events(Tick(0)).len(), 1);
      assert_eq!(obj.lod(CraftId { slot: 0, gen: 0 }), Some(Lod::Player));

      // New surface (fix #2): craft_fuel_capacity is Option-typed and present.
      assert_eq!(
          obj.craft_fuel_capacity(CraftId { slot: 0, gen: 0 }),
          Some(100.0)
      );
      assert_eq!(obj.craft_fuel_capacity(CraftId { slot: 9, gen: 9 }), None);
  }
  ```

  Run:

  ```
  cargo test -p jumpgate-core contract -- --nocolor
  ```

  EXPECTED: all four tests pass —
  `test contract::tests::command_sort_key_total_order ... ok`
  `test contract::tests::enums_round_trip_via_partial_eq ... ok`
  `test contract::tests::integrator_trait_is_implementable_and_object_safe ... ok`
  `test contract::tests::state_view_trait_is_implementable_standalone ... ok`
  `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 7: Lint the crate including the test module.**

  Per project memory, `clippy --lib` is a no-op here (binary/lib mix); use `--all-targets` so the inline `#[cfg(test)]` module is linted too.

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```

  EXPECTED: `Finished` with no warnings or errors. (Trait methods are not dead-code-checked, so the unused `_id` / `_since` params on the test impls are fine; they are justified by the real signatures.)

- [ ] **Step 8: Commit.**

  ```
  git add crates/jumpgate-core/src/contract.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  feat(core): land shared contract DTOs + Integrator/StateView traits (drift lock)

  Add contract.rs: the cross-cutting DTOs and read/integrate traits the
  facades, replay, event and ingestion paths agree on. Command + canonical
  command_sort_key (Sim=0, World=1, Entity=2 then slot/gen); EventKind/Event
  (incl. a Wake variant for the LOD wake hook, emitted in World::step later);
  the Integrator and StateView traits, each DEFINED ONCE here (the spec's
  drift-lock anchor) so integrator.rs/world.rs impl against these exact
  shapes. StateView gains craft_fuel_capacity (Option<f64>; real impl reads
  the effective-param accessor, never base_*).

  The primitive seam enums (Lod, NavDest, Target, EntityRef, CommandKind) are
  IMPORTED from crate::types (Task 3), not defined here, breaking the
  contract<->stores cycle. Prev-state/nav event-detection bookkeeping stays
  off StateView (internal, handed to detect_boundary_events via &World).
  Wire `pub mod contract;` into lib.rs. Bodies trivial; downstream tasks
  implement against these real types.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```

  EXPECTED: a commit is created on the current feature branch (not `main`). Verify with `git log --oneline -1`.

---

**Files referenced (absolute paths):**
- `/home/john/jumpgate/crates/jumpgate-core/src/contract.rs` (created)
- `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` (modified)

**Type-contract names used verbatim:** Defined here — `Command`, `command_sort_key`, `EventKind` (incl. new `Wake`), `Event`, `Integrator`, `StateView` (incl. new `craft_fuel_capacity`). Imported from `crate::types` (Task 3) — `Lod`, `NavDest`, `Target`, `EntityRef`, `CommandKind`. Task-4 dependencies — `math::Vec3` (incl. `Vec3::ZERO`, `Vec3::new`, `add`, `scale`), `ids::{CraftId, BodyId}` (incl. `slot`/`gen` fields, `PartialEq`/`Eq`/`Ord`), `time::{Tick, Dt}` (incl. `Dt::new`, `Dt::get`).