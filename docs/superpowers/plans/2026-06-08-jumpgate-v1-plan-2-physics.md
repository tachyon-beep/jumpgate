# jumpgate v1 — Plan 2: Physics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
> **READ FIRST:** `2026-06-08-jumpgate-v1-plan-0-contract-surface.md` — canonical signatures, workspace layout, and plan-level conventions (it wins on any conflict).

**Goal:** Build the jumpgate v1 rung-3 deterministic (Tier B) 3D Newtonian space core — on-rails bodies, gravity-feeling thrust/fuel/mass craft flown by an in-engine autopilot under a navigator macro-action — exposed as a reproducible Gymnasium env, with a per-tick state-hash replay-equivalence contract.

**Architecture:** Two crates: a pure-Rust `jumpgate-core` (`#![forbid(unsafe_code)]`) that is the sole authoritative writer (SoA stores, tick-indexed ephemeris, velocity-Verlet behind an Integrator trait with accel-keyed integer substepping, Tsiolkovsky variable-mass craft, autopilot guidance, one typed Command/Target ingestion path, a typed Event stream, FNV-1a state hashing, and log-replay), and a `jumpgate-py` PyO3 cdylib facade that writes frame-relative f32 observations into caller-provided buffers and presents the Gymnasium 5-tuple. All facades read through one `StateView` trait that exposes command+event history, not just physics; the engine is shaped (Target sum, Event typing, observer-parameterized projection, effective-param accessor, slot-map ids, Lod seam) so combat/upgrades/fog-of-war drop in without a contract break.

**Tech Stack:** Rust 2021 edition (rustc/cargo 1.95; edition 2021 is deliberate — `gen` is a reserved keyword in edition 2024 but is used as a struct field in `CraftId`/`BodyId`/slot-map generations); jumpgate-core deps: rand_chacha (pinned, ChaCha8Rng) + rand_core only; no serde/glam/rayon in the hashed path; hand-rolled f64 Vec3. jumpgate-py: pyo3 0.23 + numpy 0.23 (abi3-py312, extension-module). Build via /home/john/jumpgate/archive/.venv/bin/python -m maturin develop. Python test deps already present: gymnasium 1.2.3, numpy 2.4.6, torch 2.9.1. Workspace-root clippy.toml with disallowed-methods. FNV-1a hashing hand-rolled over f64::to_bits little-endian.

**This plan covers Tasks 7–9.** Prerequisite: Plan 1 complete.

---

### Task 7: Ephemeris: Kepler-once → tick table + sub-tick interp (with a≤0 fixed-body guard)

Precompute body positions+velocities over the tick window from classical orbital elements once at init (Kepler solve via Newton iteration), store as per-body `Vec<Vec3>` pos + `Vec<Vec3>` vel tables, and expose O(1) `body_pos(tick)` array lookup plus deterministic `body_pos_subtick(tick, frac)` interpolation. v1 interpolation is LINEAR but the stored velocity table makes cubic-Hermite a drop-in (the seam is marked in code). No transcendentals on the lookup/interp path — Kepler runs once, at `precompute`.

**CRITICAL CORRECTNESS GUARD (this task's headline fix).** The spec's on-rails set is "star, planets" (§5.4): the **central star is itself a body in `RunConfig.bodies`** and it sits at the focus — it does not orbit, so its config uses `OrbitalElements { a: 0.0, .. }`. With no guard, `a = 0.0` makes mean motion `n = sqrt(μ / a³) = sqrt(μ/0) = +inf`, then `M = m0 + n·t = inf`, then `inf % TAU = NaN` in `solve_eccentric_anomaly`, and `kepler_state` returns `(NaN, NaN, NaN)`. That NaN is the central body's position; every craft reads it through gravity, every downstream body-relative term inherits it, and it is then `f64::to_bits()`-laundered into a **deterministic-but-wrong** FNV state hash (NaN has a fixed bit pattern, so replay still "passes" while the physics is silently garbage). The fix: a single `KEPLER_A_EPSILON` threshold, checked in **both** `precompute` (fast path: fill the whole column with `Vec3::ZERO`, skip transcendentals entirely) and `kepler_state` (defensive: any caller passing `a < ε` gets `(Vec3::ZERO, Vec3::ZERO)`). The existing tests all use `a = 1.0` and miss this entirely — Step 2 adds a dedicated `a = 0.0` test that asserts **finite (non-NaN) origin-pinned output across the window**.

**CROSS-TASK CONTRACT SURFACE (systemic rule, applied here).** Per the workspace contract-surface document produced before Task 3, the task that PROVIDES a symbol must define every method any downstream task calls AND cover those methods in its own test suite. Task 7 is the sole provider of `Ephemeris`. Downstream callers (Task 9 `VelocityVerlet`/`gravity_accel`, Task 10 `World`/`StateView::body_pos`) call exactly three methods: `Ephemeris::precompute`, `Ephemeris::body_pos`, `Ephemeris::body_pos_subtick`. All three are defined here and all three are exercised by this task's tests (including the `a = 0.0` central-body case and the clamp-past-window case). Do not let a downstream task add an `Ephemeris` method without adding it (and its test) here.

**Depends on Task 6** (which must already provide, in `jumpgate-core`): `math.rs` with `Vec3` + `G_CANONICAL`; `time.rs` with `Tick(u64)` and `Dt` (`Dt::get`, `Dt::bits`); `config.rs` (created earlier in the acyclic sequence math → time → types → ids → config → contract → stores) with `OrbitalElements { a, e, i, raan, argp, m0 }` and `BodyInit { mass, elements }`. This task does not redefine any of those — it imports them. It also does NOT touch `hash.rs`: `Ephemeris` tables are derived from `tick` and are not themselves appended to the per-tick state hash (only craft state + the SlotMap cursor are, per the HASH_FIELD_ORDER spec), so this task introduces no new hashed field and the golden-hash test is unaffected.

Files:
- Create: `crates/jumpgate-core/src/ephemeris.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/ephemeris.rs` (inline `#[cfg(test)] mod tests`)

Contract types in play: `Ephemeris`, `OrbitalElements`, `BodyInit`, `Dt` (plus `Vec3`, `Tick`, `G_CANONICAL` from earlier tasks).

---

- [ ] **Step 1: Add the `ephemeris` module declaration + re-export to `lib.rs`.**

  Open `crates/jumpgate-core/src/lib.rs` and add **only** the two lines below. Place the `pub mod ephemeris;` line in alphabetical position among the existing `pub mod` lines (after `pub mod config;`, before `pub mod hash;` — adjust to match whatever ordering the earlier tasks left). Do NOT add declarations for any module whose file does not yet exist at this task's completion — `lib.rs` must compile with exactly the files present now (ephemeris is the new one).

  ```rust
  pub mod ephemeris;
  pub use ephemeris::Ephemeris;
  ```

  This makes the new file compile as part of the crate. Do not add anything else.

- [ ] **Step 2: Create `ephemeris.rs` with the struct, a stub `precompute`, and the failing tests (including the `a = 0.0` guard test).**

  Write `crates/jumpgate-core/src/ephemeris.rs`. The struct stores, per body, a `Vec<Vec3>` of positions and a `Vec<Vec3>` of velocities sampled at integer ticks over `[0, window]` inclusive (so `window+1` samples), plus the sampling `dt`. `precompute` is a stub returning empty tables so the file compiles but the tests fail.

  ```rust
  //! On-rails body ephemeris: Kepler-solve ONCE at init into integer-tick
  //! position+velocity tables; O(1) tick lookup; deterministic sub-tick interp.
  //!
  //! No transcendentals on the lookup/interp path. The stored per-tick velocity
  //! table exists so cubic-Hermite interpolation is a drop-in replacement for the
  //! v1 LINEAR interp at `body_pos_subtick` (see the SEAM comment there).
  //!
  //! CENTRAL-BODY GUARD: the star sits at the common focus and is configured with
  //! `OrbitalElements { a: 0.0, .. }`. `a == 0.0` would make `n = sqrt(mu/a^3)`
  //! infinite and propagate NaN through every position; `KEPLER_A_EPSILON` pins
  //! any such body to the origin with zero velocity instead. See `precompute`
  //! and `kepler_state`.

  use crate::config::BodyInit;
  use crate::math::{Vec3, G_CANONICAL};
  use crate::time::{Dt, Tick};

  /// Semi-major axes below this (canonical AU) are treated as a body fixed at the
  /// focus (the central star), NOT propagated by Kepler. Guards `a == 0.0` from
  /// producing `n = sqrt(mu/a^3) = inf -> M = inf -> inf % TAU = NaN`.
  pub const KEPLER_A_EPSILON: f64 = 1e-12;

  /// Precomputed per-body position+velocity tables over a fixed tick window.
  pub struct Ephemeris {
      /// `pos[body_idx][tick]` — sampled at integer ticks `0..=window`.
      pos: Vec<Vec<Vec3>>,
      /// `vel[body_idx][tick]` — same indexing; reserved for cubic-Hermite seam.
      #[allow(dead_code)] // SEAM: read by the cubic-Hermite drop-in in body_pos_subtick.
      vel: Vec<Vec<Vec3>>,
      /// Number of integer-tick samples per body (== window + 1).
      n_samples: usize,
      /// Sampling timestep (days), folded into Kepler propagation at precompute.
      #[allow(dead_code)] // retained for the cubic-Hermite seam (basis on dt days).
      dt: Dt,
  }

  impl Ephemeris {
      /// Solve Kepler once per tick sample per body; store pos+vel tables.
      pub fn precompute(_bodies: &[BodyInit], dt: Dt, _window: u64) -> Ephemeris {
          // STUB — replaced in Step 4.
          Ephemeris { pos: Vec::new(), vel: Vec::new(), n_samples: 0, dt }
      }

      /// O(1) array lookup of a body position at an integer tick (clamped to window).
      pub fn body_pos(&self, body_idx: usize, tick: Tick) -> Vec3 {
          let i = (tick.0 as usize).min(self.n_samples.saturating_sub(1));
          self.pos[body_idx][i]
      }

      /// Deterministic sub-tick position between sample `tick` and `tick+1`.
      /// v1 = LINEAR; `frac` in [0,1]. SEAM: cubic-Hermite drops in here using
      /// `self.vel[..][tick]` and `self.vel[..][tick+1]` without changing callers.
      pub fn body_pos_subtick(&self, _body_idx: usize, _tick: Tick, _frac: f64) -> Vec3 {
          // STUB — replaced in Step 4.
          Vec3::ZERO
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::config::OrbitalElements;
      use crate::math::Vec3;
      use crate::time::{Dt, Tick};

      /// A circular (e=0) orbit at a=1 AU keeps a constant radius across the window.
      fn circular_body() -> BodyInit {
          BodyInit {
              mass: 0.0, // test bodies are massless probes; central mass is M_sun=1
              elements: OrbitalElements {
                  a: 1.0,
                  e: 0.0,
                  i: 0.0,
                  raan: 0.0,
                  argp: 0.0,
                  m0: 0.0,
              },
          }
      }

      /// The central star: sits at the focus, does NOT orbit -> a = 0.0.
      fn central_star() -> BodyInit {
          BodyInit {
              mass: 1.0, // M_sun in canonical units
              elements: OrbitalElements {
                  a: 0.0,
                  e: 0.0,
                  i: 0.0,
                  raan: 0.0,
                  argp: 0.0,
                  m0: 0.0,
              },
          }
      }

      #[test]
      fn circular_orbit_constant_radius() {
          let dt = Dt::new(1.0);
          let window = 400u64; // ~ a bit more than one 365-day orbit
          let eph = Ephemeris::precompute(&[circular_body()], dt, window);
          let r0 = eph.body_pos(0, Tick(0)).length();
          assert!((r0 - 1.0).abs() < 1e-9, "initial radius {r0} != 1 AU");
          for t in 0..=window {
              let r = eph.body_pos(0, Tick(t)).length();
              assert!(
                  (r - 1.0).abs() < 1e-9,
                  "radius drifted to {r} at tick {t} (e=0 must stay at 1 AU)"
              );
          }
      }

      #[test]
      fn subtick_endpoints_match_samples() {
          let dt = Dt::new(1.0);
          let eph = Ephemeris::precompute(&[circular_body()], dt, 10);
          let p_t = eph.body_pos(0, Tick(3));
          let p_t1 = eph.body_pos(0, Tick(4));
          let at0 = eph.body_pos_subtick(0, Tick(3), 0.0);
          let at1 = eph.body_pos_subtick(0, Tick(3), 1.0);
          assert_eq!(at0, p_t, "frac=0 must equal body_pos(tick)");
          assert_eq!(at1, p_t1, "frac=1 must equal body_pos(tick+1)");
      }

      #[test]
      fn precompute_is_bit_identical() {
          let dt = Dt::new(0.5);
          let window = 200u64;
          let bodies = [circular_body()];
          let a = Ephemeris::precompute(&bodies, dt, window);
          let b = Ephemeris::precompute(&bodies, dt, window);
          for t in 0..=window {
              let pa = a.body_pos(0, Tick(t));
              let pb = b.body_pos(0, Tick(t));
              assert_eq!(
                  pa.to_bits(),
                  pb.to_bits(),
                  "tick {t}: two precomputes from same config must be bit-identical"
              );
          }
      }

      /// HEADLINE GUARD TEST. The central star is configured with a = 0.0. Without
      /// the KEPLER_A_EPSILON guard, `n = sqrt(mu/a^3) = inf -> M = inf -> NaN`,
      /// which laundered into the FNV hash gives a deterministic-but-WRONG state.
      /// Assert the output is (a) finite everywhere (no NaN/inf) and (b) pinned to
      /// the origin, at integer ticks AND at a sub-tick fraction.
      #[test]
      fn central_body_a_zero_is_fixed_at_origin_and_finite() {
          let dt = Dt::new(1.0);
          let window = 50u64;
          // Mixed config: index 0 is the fixed star, index 1 is a real orbiter.
          let eph = Ephemeris::precompute(&[central_star(), circular_body()], dt, window);

          for t in 0..=window {
              let p = eph.body_pos(0, Tick(t));
              assert!(
                  p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                  "central body produced non-finite pos {p:?} at tick {t} \
                   (a=0 NaN leak — KEPLER_A_EPSILON guard missing)"
              );
              assert_eq!(
                  p,
                  Vec3::ZERO,
                  "central body (a=0) must be pinned to origin, got {p:?} at tick {t}"
              );
          }

          // Sub-tick interpolation of a fixed body must also be the origin, finite.
          let mid = eph.body_pos_subtick(0, Tick(10), 0.5);
          assert!(
              mid.x.is_finite() && mid.y.is_finite() && mid.z.is_finite(),
              "central body sub-tick non-finite {mid:?}"
          );
          assert_eq!(mid, Vec3::ZERO, "central body sub-tick must stay at origin");

          // The co-resident real orbiter must be unaffected: r stays ~1 AU, finite.
          for t in 0..=window {
              let p = eph.body_pos(1, Tick(t));
              assert!(
                  p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                  "orbiter contaminated by NaN at tick {t}: {p:?}"
              );
              assert!(
                  (p.length() - 1.0).abs() < 1e-9,
                  "orbiter radius drifted to {} at tick {t}",
                  p.length()
              );
          }
      }

      /// Contract-surface coverage: lookups past the window must clamp, not panic
      /// or index OOB (downstream Verlet reads t_{n+1} at the last tick).
      #[test]
      fn lookup_past_window_clamps_to_last_sample() {
          let dt = Dt::new(1.0);
          let window = 5u64;
          let eph = Ephemeris::precompute(&[circular_body()], dt, window);
          let last = eph.body_pos(0, Tick(window));
          let past = eph.body_pos(0, Tick(window + 1000));
          assert_eq!(past, last, "past-window lookup must clamp to the final sample");
          let sub_past = eph.body_pos_subtick(0, Tick(window + 1000), 1.0);
          assert_eq!(sub_past, last, "past-window sub-tick must clamp to final sample");
      }
  }
  ```

- [ ] **Step 3: Run the tests and confirm they FAIL.**

  ```
  cargo test -p jumpgate-core ephemeris -- --nocapture
  ```

  EXPECTED: the suite compiles, then fails. The radius/identity/guard tests index into empty tables (panic: index out of bounds) or hit the `Vec3::ZERO` stub mismatch (`subtick_endpoints_match_samples`). `central_body_a_zero_is_fixed_at_origin_and_finite` and `lookup_past_window_clamps_to_last_sample` panic on the empty-table index. Net: `test result: FAILED. 0 passed; 5 failed`.

- [ ] **Step 4: Implement Kepler propagation + linear interp, WITH the `a < KEPLER_A_EPSILON` fixed-body guard (minimal real code).**

  Replace the two STUB method bodies and add the private Kepler helpers. Propagation: mean anomaly `M(t) = m0 + n·t` with mean motion `n = sqrt(μ / a³)` and `μ = G_CANONICAL · M_central`; the central body is `M_sun = 1` in canonical units, so `μ = G_CANONICAL`. Solve `M = E − e·sin E` for eccentric anomaly `E` by Newton iteration (fixed iteration count → identical on replay). Build perifocal position+velocity, then rotate by `argp → i → raan`. Sample at every integer tick `0..=window`. **For any body with `a < KEPLER_A_EPSILON`, skip Kepler entirely and fill its columns with `Vec3::ZERO` — this is the central-star fixed-body path and the NaN guard.**

  Replace the `precompute` body with:

  ```rust
      /// Solve Kepler once per tick sample per body; store pos+vel tables.
      /// Bodies with `a < KEPLER_A_EPSILON` are FIXED at the focus (the central
      /// star): they skip the transcendental path entirely and are written as
      /// `Vec3::ZERO`. This both models the star correctly AND guards the
      /// `n = sqrt(mu/a^3) = inf -> M = NaN` blow-up that would otherwise be
      /// laundered into the deterministic state hash.
      pub fn precompute(bodies: &[BodyInit], dt: Dt, window: u64) -> Ephemeris {
          let n_samples = (window as usize) + 1;
          let dt_days = dt.get();
          // Central gravitational parameter in canonical units (M_sun = 1).
          let mu = G_CANONICAL;

          let mut pos = Vec::with_capacity(bodies.len());
          let mut vel = Vec::with_capacity(bodies.len());

          for body in bodies {
              let e = &body.elements;
              let mut pcol = Vec::with_capacity(n_samples);
              let mut vcol = Vec::with_capacity(n_samples);

              if e.a < KEPLER_A_EPSILON {
                  // FIXED body (central star): pinned to the focus, no propagation.
                  for _ in 0..n_samples {
                      pcol.push(Vec3::ZERO);
                      vcol.push(Vec3::ZERO);
                  }
              } else {
                  // Mean motion n = sqrt(mu / a^3). Computed ONCE per body.
                  let n_motion = (mu / (e.a * e.a * e.a)).sqrt();
                  for k in 0..n_samples {
                      let t_days = (k as f64) * dt_days;
                      let m = e.m0 + n_motion * t_days;
                      let (p, v) = kepler_state(e.a, e.e, e.i, e.raan, e.argp, m, mu);
                      pcol.push(p);
                      vcol.push(v);
                  }
              }

              pos.push(pcol);
              vel.push(vcol);
          }

          Ephemeris { pos, vel, n_samples, dt }
      }
  ```

  Replace the `body_pos_subtick` body with:

  ```rust
      /// Deterministic sub-tick position between sample `tick` and `tick+1`.
      /// v1 = LINEAR; `frac` in [0,1]. SEAM: cubic-Hermite drops in here using
      /// `self.vel[body_idx][i]` and `self.vel[body_idx][i+1]` (Hermite basis on
      /// `dt.get()` days) without changing this signature or any caller.
      pub fn body_pos_subtick(&self, body_idx: usize, tick: Tick, frac: f64) -> Vec3 {
          let i = (tick.0 as usize).min(self.n_samples.saturating_sub(1));
          let j = (i + 1).min(self.n_samples - 1);
          let a = self.pos[body_idx][i];
          let b = self.pos[body_idx][j];
          // LINEAR: a + (b - a) * frac.
          a.add(b.sub(a).scale(frac))
      }
  ```

  Add these private free functions at module scope (below the `impl`, above `#[cfg(test)]`):

  ```rust
  /// Solve Kepler's equation M = E - e*sin(E) for eccentric anomaly E.
  /// Fixed iteration budget => identical FP path on replay (no convergence-count
  /// branch that could differ between runs).
  fn solve_eccentric_anomaly(m: f64, e: f64) -> f64 {
      // Wrap mean anomaly into [-pi, pi] for a stable Newton seed.
      let two_pi = std::f64::consts::TAU;
      let mut mw = m % two_pi;
      if mw > std::f64::consts::PI {
          mw -= two_pi;
      } else if mw < -std::f64::consts::PI {
          mw += two_pi;
      }
      let mut ecc = mw; // seed
      for _ in 0..16 {
          let f = ecc - e * ecc.sin() - mw;
          let fp = 1.0 - e * ecc.cos();
          ecc -= f / fp;
      }
      ecc
  }

  /// Perifocal -> inertial state for classical elements at mean anomaly `m`.
  /// Returns (position, velocity) in canonical units.
  ///
  /// DEFENSIVE GUARD: `a < KEPLER_A_EPSILON` returns `(Vec3::ZERO, Vec3::ZERO)`
  /// so this function is NaN-safe for any caller (mirrors the `precompute` fast
  /// path; the two guards must agree).
  #[allow(clippy::too_many_arguments)] // arg list mirrors OrbitalElements verbatim.
  fn kepler_state(a: f64, e: f64, i: f64, raan: f64, argp: f64, m: f64, mu: f64) -> (Vec3, Vec3) {
      if a < KEPLER_A_EPSILON {
          // Fixed body at the focus; no transcendentals, no inf/NaN.
          return (Vec3::ZERO, Vec3::ZERO);
      }

      let ecc = solve_eccentric_anomaly(m, e);
      let cos_e = ecc.cos();
      let sin_e = ecc.sin();
      let sqrt_1me2 = (1.0 - e * e).sqrt();

      // Perifocal-frame position (PQW): x along periapsis, y 90deg ahead.
      let xp = a * (cos_e - e);
      let yp = a * sqrt_1me2 * sin_e;

      // Perifocal-frame velocity. r = a(1 - e cosE); Edot = n / (1 - e cosE),
      // with n = sqrt(mu/a^3). Folded: vx = -(sqrt(mu*a)/r)*sinE,
      //                               vy =  (sqrt(mu*a)/r)*sqrt(1-e^2)*cosE.
      let r = a * (1.0 - e * cos_e);
      let sqrt_mu_a = (mu * a).sqrt();
      let vxp = -sqrt_mu_a / r * sin_e;
      let vyp = sqrt_mu_a / r * sqrt_1me2 * cos_e;

      // Rotation PQW -> inertial: Rz(raan) * Rx(i) * Rz(argp).
      let cos_o = raan.cos();
      let sin_o = raan.sin();
      let cos_i = i.cos();
      let sin_i = i.sin();
      let cos_w = argp.cos();
      let sin_w = argp.sin();

      // Combined rotation matrix rows (standard orbital-elements transform).
      let r11 = cos_o * cos_w - sin_o * sin_w * cos_i;
      let r12 = -cos_o * sin_w - sin_o * cos_w * cos_i;
      let r21 = sin_o * cos_w + cos_o * sin_w * cos_i;
      let r22 = -sin_o * sin_w + cos_o * cos_w * cos_i;
      let r31 = sin_w * sin_i;
      let r32 = cos_w * sin_i;

      let pos = Vec3::new(
          r11 * xp + r12 * yp,
          r21 * xp + r22 * yp,
          r31 * xp + r32 * yp,
      );
      let vel = Vec3::new(
          r11 * vxp + r12 * vyp,
          r21 * vxp + r22 * vyp,
          r31 * vxp + r32 * vyp,
      );
      (pos, vel)
  }
  ```

  Note: `solve_eccentric_anomaly` / `kepler_state` are the ONLY transcendental site and they live in `precompute`; `body_pos` and `body_pos_subtick` are pure array lookup + add/sub/scale, satisfying the "no transcendentals on the hot path" contract. The `a < KEPLER_A_EPSILON` branch in `precompute` means the star never even reaches `kepler_state`, but the `kepler_state` guard is retained so the function is independently NaN-safe (contract-surface rule: a provided symbol must be safe for every downstream caller, not just the one in this file).

- [ ] **Step 5: Run the tests and confirm they PASS.**

  ```
  cargo test -p jumpgate-core ephemeris -- --nocapture
  ```

  EXPECTED: `test result: ok. 5 passed; 0 failed`.
  - `circular_orbit_constant_radius`: e=0 ⇒ `r = a(1 − e·cosE) = a = 1 AU` at every sample.
  - `subtick_endpoints_match_samples`: linear interp gives `frac=0 → a`, `frac=1 → b` exactly.
  - `precompute_is_bit_identical`: deterministic fixed-iteration f64 path ⇒ identical `to_bits()`.
  - `central_body_a_zero_is_fixed_at_origin_and_finite`: the `a < KEPLER_A_EPSILON` branch fills the star's column with `Vec3::ZERO` (finite, origin-pinned) and the co-resident orbiter stays at r=1 AU (no NaN contamination). **This test fails loudly if the guard is ever removed** — `sqrt(mu/0)` would make the assert see NaN.
  - `lookup_past_window_clamps_to_last_sample`: `.min(n_samples-1)` / `saturating_sub` clamp past-window indices.

- [ ] **Step 6: Lint the new module (including tests).**

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```

  EXPECTED: `Finished` with no warnings. (`--all-targets` is required to lint the inline `#[cfg(test)]` module — `--lib` alone would skip it per the project note: this is a binary/lib crate.) The `#[allow(clippy::too_many_arguments)]` on `kepler_state` and the `#[allow(dead_code)]` on the `vel`/`dt` cubic-Hermite seam fields keep this clean without restructuring or deleting the seam. Do NOT delete the `vel` field to silence a dead-field lint — a later task wires it into `VelocityVerlet` substepping.

- [ ] **Step 7: Commit.**

  ```
  git add crates/jumpgate-core/src/ephemeris.rs crates/jumpgate-core/src/lib.rs
  git commit -m "Task 7: ephemeris Kepler-once -> tick table + sub-tick interp

  precompute() solves Kepler (Newton, fixed 16 iters) once per tick sample
  into per-body pos+vel tables; body_pos() is O(1) lookup; body_pos_subtick()
  is deterministic LINEAR interp with a cubic-Hermite seam over the stored
  velocity table. No transcendentals on the lookup/interp path.

  Adds KEPLER_A_EPSILON guard: bodies with a<eps (the central star, a=0.0)
  are pinned to the focus as Vec3::ZERO instead of producing
  sqrt(mu/0)=inf -> M=NaN, which would be laundered into the state hash.
  Guarded in both precompute and kepler_state; covered by a dedicated a=0.0
  finite/origin test plus a past-window clamp test.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

  EXPECTED: one commit created on the current feature branch (not `main` directly — branch first if on `main`).

---

**Design notes for the implementer (not steps):**
- **The `a=0.0` star is the live config, not a hypothetical.** The spec's §5.4 on-rails set is "star, planets"; the star is a `BodyInit` at the focus. `RunConfig.bodies[0]` will carry `OrbitalElements { a: 0.0, .. }`. The guard is therefore exercised on every real run, not just in tests — which is exactly why the NaN was so dangerous (it would have shipped silently behind a "passing" deterministic hash).
- `μ = G_CANONICAL` assumes the central body is the star at `M_sun = 1` in canonical units (AU, M☉, day). All non-central `OrbitalElements` are heliocentric. Moons-of-planets are out of v1 scope (the contract's `BodyInit` carries no parent reference), so every orbiting body shares the common focus.
- The velocity table is stored but unused by v1 linear interp on purpose: it is the cubic-Hermite drop-in seam (§5.4 "linear, or cubic-Hermite using stored body velocity"). The `#[allow(dead_code)]` on `vel`/`dt` documents the seam; a later task wires it into `VelocityVerlet` substepping.
- `n_samples = window + 1` so that `body_pos_subtick(tick=window-1, frac=1)` and Verlet's `t_{n+1}` read at the last tick never index past the table; lookups past the window clamp to the final sample (`saturating_sub`/`.min`), as `lookup_past_window_clamps_to_last_sample` verifies.
- Determinism: the fixed 16-iteration Newton loop has no data-dependent early-exit, so the f64 operation sequence is identical across runs and the per-tick state hash stays stable. Do not switch to a tolerance-based `while` loop.
- Contract-surface note: this task is the sole provider of `Ephemeris`. Its three public methods (`precompute`, `body_pos`, `body_pos_subtick`) are exactly what Tasks 9/10 call, and all three are covered by this suite (including the fixed-body and clamp edges). If a later task needs a new `Ephemeris` method, add it AND its test here, not at the call site.


---

### Task 8: Integrator impls + reference-accel substepping + softened gravity

Velocity-Verlet (two-eval moving field) and RK4 as **impls of the `Integrator` trait that is defined once in `contract.rs`** (Task 6), plus a pure `substep_count(total_accel_mag)` and a softened `gravity_accel` kernel. The integrator stays decoupled from thrust/fuel: it receives a scalar `n_substeps` and an `accel_at(pos, sub_t)` closure, so it never knows about thrust, fuel, or ephemeris internals. `substep_count` is a pure function of the QUANTIZED total local acceleration magnitude, so it is bit-identical on replay.

**Contract-surface obligations honoured here** (per the cross-task contract-surface doc produced before Task 3):
- `integrator.rs` PROVIDES the free fns `gravity_accel`, `substep_count` and the structs `VelocityVerlet`, `Rk4`. It is the OWNER of those four symbols and its test suite covers every one of them. Downstream callers (Task 13 `World::step`, Task 15 energy/blowup test, Task 17 production config) rely only on these signatures.
- `integrator.rs` does **not** own the `Integrator` trait — that is a contract.rs symbol (Task 6). This file `use crate::contract::Integrator;` and writes impls only. There is no second trait definition anywhere in the workspace.

**Files**
- Create: `crates/jumpgate-core/src/integrator.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/integrator.rs` (inline `#[cfg(test)] mod tests`)

**Contract types in play** (use verbatim, do not rename):
- `Integrator` (trait) — **imported from `crate::contract`** (defined in Task 6; NOT redefined here)
- `VelocityVerlet` (struct), `Rk4` (struct) — owned by this task
- `pub fn substep_count(total_accel_mag: f64, cfg: SubstepCfg) -> u32` — owned by this task
- `pub fn gravity_accel(p: Vec3, body_positions: &[(Vec3, f64)], softening: f64) -> Vec3` — owned by this task
- `SubstepCfg { pub accel_ref: f64, pub max_substeps: u32 }` (defined in `config.rs`, Task 7)
- `Vec3`, `G_CANONICAL` (from `math.rs`)

> **CONTRACT AMENDMENT (carried from the substep-redesign fix, originates in Task 7):** the `SubstepCfg` field formerly named `accel_bin_base: f64` is renamed to `accel_ref: f64` and re-typed in MEANING from "log base" to "reference acceleration in AU/day²". The log base is now FIXED at 2 inside `substep_count`. Task 7 defines `SubstepCfg { pub accel_ref: f64, pub max_substeps: u32 }`; Tasks 8/15/17 consume `accel_ref`. This task assumes that amended definition is already in `config.rs`.

**Design notes (load-bearing — read before coding):**
- `gravity_accel` uses the SOFTENED kernel `G·M / (r² + ε²)^1.5` summed over all bodies. A hard distance cutoff is FORBIDDEN (it is a force discontinuity that itself causes artifacts). `body_positions: &[(Vec3, f64)]` is `(body_position, body_mass)`. The acceleration contribution from one body is `G_CANONICAL * M * d / (|d|² + ε²)^1.5` where `d = body_pos − p`.
- The `Integrator::step_craft` closure signature is `&dyn Fn(Vec3, f64) -> Vec3` where the first arg is a candidate position and the second is `sub_t` (the sub-tick time offset, in DAYS, from the start of the tick). The closure returns TOTAL local acceleration (gravity + thrust) at that `(pos, sub_t)`. The integrator calls this closure; it does not compute gravity or thrust itself.
- VelocityVerlet MUST sample the field at BOTH `t_n` and `t_{n+1}` (it calls `accel_at` at the start-of-substep position/time AND at the end-of-substep position/time). A single-eval implementation silently degrades to O(dt) — tag this in a code comment so the collapse cannot be reintroduced.
- **substep_count is a REFERENCE-ACCELERATION schedule, not a log-base-binning one.** `n = 1 + floor(log2(max(1.0, mag / accel_ref)))`, clamped to `[1, max_substeps]`. `accel_ref` is a physical reference acceleration in AU/day². At/below `accel_ref`, exactly 1 substep; every doubling of `mag` above `accel_ref` adds exactly one substep. **Why this and not `floor(ln(ratio)/ln(base))`:** all production configs use a reference acceleration `< 1`; with the old log-base form `floor(ln(ratio)/ln(base))` goes NEGATIVE for every realistic acceleration, pinning `n` to 1 always — a false-green where the only test showing `n>1` used `base=10.0`, a regime never deployed. With `accel_ref = 1e-4`, a craft at 1 AU from a 1 M_sun body (`g ≈ 2.96e-4`) gets `n=2`; at 0.1 AU (`g ≈ 2.96e-2`) gets `n=9`. This is what makes Task 15's energy-blowup test actually exercise substepping.

---

- [ ] **Step 1: Create `integrator.rs` with a failing test for the softened gravity kernel.**

  Create `crates/jumpgate-core/src/integrator.rs` with only the import block, the `gravity_accel` stub, and the test module so it compiles but the test fails. The softening kernel: a body of mass `M` at distance `r` along +x pulls a craft at the origin with acceleration magnitude `G·M / (r²+ε²)^1.5 · r` in the +x direction.

  ```rust
  //! Integrators (velocity-Verlet + RK4) as impls of the `Integrator` trait
  //! (defined in `contract.rs`), the softened gravity kernel, and
  //! reference-acceleration deterministic substepping.
  //!
  //! This module OWNS: `gravity_accel`, `substep_count`, `VelocityVerlet`, `Rk4`.
  //! It does NOT own the `Integrator` trait — that lives in `contract.rs` and is
  //! imported below. There is exactly one `Integrator` trait in the workspace.

  use crate::config::SubstepCfg;
  use crate::contract::Integrator;
  use crate::math::{Vec3, G_CANONICAL};

  /// Softened gravitational acceleration at point `p` summed over `body_positions`
  /// (each `(body_pos, body_mass)`), using the kernel `G·M·d / (|d|² + ε²)^1.5`
  /// with `d = body_pos − p`. A hard distance cutoff is FORBIDDEN.
  pub fn gravity_accel(p: Vec3, body_positions: &[(Vec3, f64)], softening: f64) -> Vec3 {
      todo!()
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::math::Vec3;

      #[test]
      fn gravity_softened_matches_closed_form() {
          // One body of mass M at distance r along +x; craft at origin.
          let m = 3.0_f64;
          let r = 2.0_f64;
          let eps = 0.0_f64; // no softening => pure Newtonian
          let a = gravity_accel(Vec3::ZERO, &[(Vec3::new(r, 0.0, 0.0), m)], eps);
          let expected_mag = G_CANONICAL * m / (r * r); // along +x
          assert!((a.x - expected_mag).abs() < 1e-12, "ax={} expected={}", a.x, expected_mag);
          assert!(a.y.abs() < 1e-15 && a.z.abs() < 1e-15);

          // Softening strictly reduces the magnitude vs the unsoftened case.
          let eps2 = 1.0_f64;
          let a_soft = gravity_accel(Vec3::ZERO, &[(Vec3::new(r, 0.0, 0.0), m)], eps2);
          assert!(a_soft.x < a.x && a_soft.x > 0.0, "softened {} should be in (0, {})", a_soft.x, a.x);
      }
  }
  ```

- [ ] **Step 2: Wire `integrator` into `lib.rs` and run the failing test.**

  Add the module declaration and re-exports to `crates/jumpgate-core/src/lib.rs`. Place `pub mod integrator;` near the other module declarations and the `pub use` near the other re-export lines.

  **CRITICAL — do NOT re-export `Integrator` here.** The `Integrator` trait is owned and re-exported by `contract.rs` (Task 6's `pub use contract::Integrator;`). Re-exporting it again from `integrator` would create a second public path to the same trait and invites a concrete type implementing a same-shaped *duplicate*. This task re-exports ONLY the symbols it owns:

  ```rust
  pub mod integrator;
  pub use integrator::{gravity_accel, substep_count, Rk4, VelocityVerlet};
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator -- --nocapture
  ```
  EXPECTED: compile error — `substep_count`, `Rk4`, `VelocityVerlet` are not yet defined (the re-export references them) and the `use crate::contract::Integrator;` resolves (Task 6 already landed the trait). This confirms the wiring fails before the owned symbols exist.

- [ ] **Step 3: Add stubs for `substep_count`, `VelocityVerlet`, `Rk4` (impls of the imported trait) so the crate compiles and the gravity test runs and FAILS.**

  Append to `crates/jumpgate-core/src/integrator.rs` (above the `tests` module). Note there is **no `pub trait Integrator` block here** — only impls of the trait imported from `contract`.

  ```rust
  /// `N` = pure fn of the QUANTIZED total local acceleration magnitude
  /// (gravity + thrust). Identical on replay. Monotonic non-decreasing in
  /// `total_accel_mag`, clamped to `[1, cfg.max_substeps]`.
  ///
  /// Reference-acceleration schedule (fixed log base 2):
  ///   n = 1 + floor(log2(max(1, mag / cfg.accel_ref)))
  /// `cfg.accel_ref` is a physical reference acceleration in AU/day².
  pub fn substep_count(total_accel_mag: f64, cfg: SubstepCfg) -> u32 {
      todo!()
  }

  /// Default integrator: 1 force eval per substep, two field samples (t_n, t_{n+1}).
  pub struct VelocityVerlet;
  /// Golden/validation integrator: 4 force evals per substep.
  pub struct Rk4;

  impl Integrator for VelocityVerlet {
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64) -> Vec3,
          dt: f64,
          n_substeps: u32,
      ) -> (Vec3, Vec3) {
          let _ = (pos, vel, accel_at, dt, n_substeps);
          todo!()
      }
      fn name(&self) -> &'static str {
          "velocity_verlet"
      }
  }

  impl Integrator for Rk4 {
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64) -> Vec3,
          dt: f64,
          n_substeps: u32,
      ) -> (Vec3, Vec3) {
          let _ = (pos, vel, accel_at, dt, n_substeps);
          todo!()
      }
      fn name(&self) -> &'static str {
          "rk4"
      }
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::gravity_softened -- --nocapture
  ```
  EXPECTED: the test compiles and FAILS with a panic `not yet implemented` from `gravity_accel`'s `todo!()`.

- [ ] **Step 4: Implement `gravity_accel` (softened kernel sum) and re-run to pass.**

  Replace the `gravity_accel` body in `crates/jumpgate-core/src/integrator.rs`:

  ```rust
  pub fn gravity_accel(p: Vec3, body_positions: &[(Vec3, f64)], softening: f64) -> Vec3 {
      let eps_sq = softening * softening;
      let mut acc = Vec3::ZERO;
      for &(body_pos, mass) in body_positions {
          let d = body_pos.sub(p); // vector from craft to body
          let r_sq = d.length_sq() + eps_sq;
          // Softened kernel: G·M·d / (r² + ε²)^1.5. No hard cutoff.
          let inv = 1.0 / (r_sq * r_sq.sqrt());
          acc = acc.add(d.scale(G_CANONICAL * mass * inv));
      }
      acc
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::gravity_softened -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 5: Add a failing test for `substep_count` (physically-grounded inputs, determinism, monotonicity, cap).**

  This test uses `accel_ref` values in the real AU/day² regime that production configs deploy (`< 1`), and asserts the exact per-octave escalation — the property the old `base=10.0` test never checked. Grounding numbers (verified): with `accel_ref = 1e-4`, a 1 M_sun body at 1 AU gives `g = G_CANONICAL ≈ 2.96e-4` → `n = 2`; at 0.1 AU, `g ≈ 2.96e-2` → `n = 9`.

  Add to the `tests` module in `crates/jumpgate-core/src/integrator.rs`:

  ```rust
  #[test]
  fn substep_count_reference_accel_grounded() {
      // PRODUCTION-REGIME reference acceleration (AU/day^2), NOT a log base.
      let cfg = SubstepCfg { accel_ref: 1.0e-4, max_substeps: 64 };

      // Deterministic: same quantized input => same output, every call.
      let a = 3.21e-3_f64;
      assert_eq!(substep_count(a, cfg), substep_count(a, cfg));

      // At/below the reference accel, exactly 1 substep.
      assert_eq!(substep_count(0.0, cfg), 1);
      assert_eq!(substep_count(cfg.accel_ref * 0.5, cfg), 1);
      assert_eq!(substep_count(cfg.accel_ref, cfg), 1);

      // Each DOUBLING above accel_ref adds exactly one substep:
      // n = 1 + floor(log2(mag/accel_ref)).
      assert_eq!(substep_count(cfg.accel_ref * 2.0, cfg), 2);
      assert_eq!(substep_count(cfg.accel_ref * 4.0, cfg), 3);
      assert_eq!(substep_count(cfg.accel_ref * 8.0, cfg), 4);
      // Just under the next octave stays in the lower bin (floor behaviour).
      assert_eq!(substep_count(cfg.accel_ref * 3.999, cfg), 2);

      // Physically-grounded: gravity from a 1 M_sun body at 1 AU and 0.1 AU.
      let m = 1.0_f64;
      let g_1au = gravity_accel(Vec3::ZERO, &[(Vec3::new(1.0, 0.0, 0.0), m)], 0.0).length();
      let g_01au = gravity_accel(Vec3::ZERO, &[(Vec3::new(0.1, 0.0, 0.0), m)], 0.0).length();
      assert_eq!(substep_count(g_1au, cfg), 2, "1 AU should escalate past 1 substep");
      assert_eq!(substep_count(g_01au, cfg), 9, "0.1 AU close approach");

      // Monotonic non-decreasing across increasing acceleration; in range.
      let mut prev = 0u32;
      let mut mag = cfg.accel_ref * 0.25_f64;
      for _ in 0..40 {
          let n = substep_count(mag, cfg);
          assert!(n >= prev, "non-monotonic at mag={}: {} < {}", mag, n, prev);
          assert!(n >= 1 && n <= cfg.max_substeps, "out of range n={}", n);
          prev = n;
          mag *= 2.0;
      }

      // Huge acceleration saturates exactly at the cap.
      assert_eq!(substep_count(1.0e300, cfg), cfg.max_substeps);
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::substep_count_reference_accel -- --nocapture
  ```
  EXPECTED: FAILS with `not yet implemented` panic from `substep_count`'s `todo!()`.

- [ ] **Step 6: Implement `substep_count` (reference-accel, fixed log base 2, clamped) and re-run to pass.**

  The schedule: at/below `accel_ref`, 1 substep. Above it, `N = 1 + floor(log2(mag / accel_ref))` — one extra substep per octave (doubling) of acceleration above the reference. `floor` of a deterministic f64 `log2` makes `N` a pure step function of the input, identical on replay. Clamp to `[1, max_substeps]`.

  Replace the `substep_count` body in `crates/jumpgate-core/src/integrator.rs`:

  ```rust
  pub fn substep_count(total_accel_mag: f64, cfg: SubstepCfg) -> u32 {
      // Non-finite or non-positive accel => the floor of 1 substep.
      if !(total_accel_mag > 0.0) || !total_accel_mag.is_finite() {
          return 1;
      }
      // Reference-acceleration ratio. accel_ref is AU/day^2 (a physical scale),
      // NOT a log base. Production configs use accel_ref < 1, so clamping the
      // ratio to >= 1 keeps n >= 1 instead of pinning to 1 for every realistic
      // acceleration (the false-green the old log-base form produced).
      let ratio = (total_accel_mag / cfg.accel_ref).max(1.0);
      // Fixed log base 2: one extra substep per octave above the reference.
      let octaves = ratio.log2().floor();
      let n = 1.0 + octaves; // octaves >= 0 since ratio >= 1
      let n = n.max(1.0).min(cfg.max_substeps as f64);
      n as u32
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::substep_count_reference_accel -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 7: Add a failing test for VelocityVerlet on a coast (zero field) — exact straight-line motion.**

  With `accel_at` returning `Vec3::ZERO`, both Verlet and RK4 must reproduce exact uniform motion `pos + vel·dt`, regardless of `n_substeps`. Add to the `tests` module:

  ```rust
  #[test]
  fn verlet_coast_is_exact_straight_line() {
      let v = VelocityVerlet;
      let pos = Vec3::new(1.0, -2.0, 0.5);
      let vel = Vec3::new(0.3, 0.1, -0.2);
      let dt = 0.5_f64;
      let zero_field = |_p: Vec3, _t: f64| Vec3::ZERO;
      for &n in &[1u32, 4, 16] {
          let (p1, v1) = v.step_craft(pos, vel, &zero_field, dt, n);
          let expected = pos.add(vel.scale(dt));
          assert!((p1.sub(expected)).length() < 1e-12, "n={} pos drift {:?}", n, p1);
          assert!((v1.sub(vel)).length() < 1e-12, "n={} vel drift {:?}", n, v1);
      }
      assert_eq!(v.name(), "velocity_verlet");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::verlet_coast -- --nocapture
  ```
  EXPECTED: FAILS with `not yet implemented` from `VelocityVerlet::step_craft`'s `todo!()`.

- [ ] **Step 8: Implement `VelocityVerlet::step_craft` (two-eval moving field, substepped) and re-run to pass.**

  Velocity-Verlet per substep with timestep `h = dt / n_substeps`. Sample the field at the start of the substep (`a_n` at `(pos, t)`) AND at the end (`a_np1` at the drifted position `(pos + vel·h + ½·a_n·h², t+h)`). Using both samples is what keeps it second-order in a moving field — a single-eval form silently degrades to O(dt).

  Replace the `VelocityVerlet` `step_craft` body in `crates/jumpgate-core/src/integrator.rs`:

  ```rust
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64) -> Vec3,
          dt: f64,
          n_substeps: u32,
      ) -> (Vec3, Vec3) {
          let n = n_substeps.max(1);
          let h = dt / (n as f64);
          let mut p = pos;
          let mut v = vel;
          let mut t = 0.0_f64; // sub-tick time offset in days
          for _ in 0..n {
              // a_n: acceleration at the START of the substep (t_n).
              let a_n = accel_at(p, t);
              // Drift to the new position using a_n.
              let p_new = p.add(v.scale(h)).add(a_n.scale(0.5 * h * h));
              // a_{n+1}: acceleration at the END of the substep (t_{n+1}).
              // MOVING-FIELD CRITICAL: this SECOND eval (at p_new, t+h) is what
              // keeps Verlet O(dt^2). A single-eval form silently degrades to O(dt).
              let a_np1 = accel_at(p_new, t + h);
              // Kick: average the two accelerations.
              let v_new = v.add(a_n.add(a_np1).scale(0.5 * h));
              p = p_new;
              v = v_new;
              t += h;
          }
          (p, v)
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::verlet_coast -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 9: Add a failing test for `Rk4::step_craft` on a coast — exact straight line.**

  Add to the `tests` module:

  ```rust
  #[test]
  fn rk4_coast_is_exact_straight_line() {
      let r = Rk4;
      let pos = Vec3::new(0.2, 4.0, -1.0);
      let vel = Vec3::new(-0.5, 0.0, 0.25);
      let dt = 0.5_f64;
      let zero_field = |_p: Vec3, _t: f64| Vec3::ZERO;
      for &n in &[1u32, 4, 16] {
          let (p1, v1) = r.step_craft(pos, vel, &zero_field, dt, n);
          let expected = pos.add(vel.scale(dt));
          assert!((p1.sub(expected)).length() < 1e-12, "n={} pos drift {:?}", n, p1);
          assert!((v1.sub(vel)).length() < 1e-12, "n={} vel drift {:?}", n, v1);
      }
      assert_eq!(r.name(), "rk4");
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::rk4_coast -- --nocapture
  ```
  EXPECTED: FAILS with `not yet implemented` from `Rk4::step_craft`'s `todo!()`.

- [ ] **Step 10: Implement `Rk4::step_craft` (classic RK4 on the (pos, vel) ODE, substepped) and re-run to pass.**

  State `y = (pos, vel)`, derivative `(vel, accel_at(pos, t))`. Classic 4-stage RK4 per substep of size `h = dt / n_substeps`, sampling the field at `t`, `t + h/2` (twice), and `t + h`.

  Replace the `Rk4` `step_craft` body in `crates/jumpgate-core/src/integrator.rs`:

  ```rust
      fn step_craft(
          &self,
          pos: Vec3,
          vel: Vec3,
          accel_at: &dyn Fn(Vec3, f64) -> Vec3,
          dt: f64,
          n_substeps: u32,
      ) -> (Vec3, Vec3) {
          let n = n_substeps.max(1);
          let h = dt / (n as f64);
          let mut p = pos;
          let mut v = vel;
          let mut t = 0.0_f64;
          for _ in 0..n {
              // k1
              let k1_p = v;
              let k1_v = accel_at(p, t);
              // k2 at t + h/2
              let p2 = p.add(k1_p.scale(0.5 * h));
              let v2 = v.add(k1_v.scale(0.5 * h));
              let k2_p = v2;
              let k2_v = accel_at(p2, t + 0.5 * h);
              // k3 at t + h/2
              let p3 = p.add(k2_p.scale(0.5 * h));
              let v3 = v.add(k2_v.scale(0.5 * h));
              let k3_p = v3;
              let k3_v = accel_at(p3, t + 0.5 * h);
              // k4 at t + h
              let p4 = p.add(k3_p.scale(h));
              let v4 = v.add(k3_v.scale(h));
              let k4_p = v4;
              let k4_v = accel_at(p4, t + h);
              // weighted sum: (k1 + 2k2 + 2k3 + k4) / 6
              let sixth = h / 6.0;
              p = p
                  .add(k1_p.scale(sixth))
                  .add(k2_p.scale(2.0 * sixth))
                  .add(k3_p.scale(2.0 * sixth))
                  .add(k4_p.scale(sixth));
              v = v
                  .add(k1_v.scale(sixth))
                  .add(k2_v.scale(2.0 * sixth))
                  .add(k3_v.scale(2.0 * sixth))
                  .add(k4_v.scale(sixth));
              t += h;
          }
          (p, v)
      }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::rk4_coast -- --nocapture
  ```
  EXPECTED: `test result: ok. 1 passed`.

- [ ] **Step 11: Add a regression-guard test — near-circular orbit stays bounded over many steps (Verlet), with the reference-accel schedule actually engaging substeps.**

  A craft on a circular orbit around a central mass `M` at radius `R` has speed `sqrt(G·M/R)` perpendicular to the radius. Integrating many ticks, the radius must stay in a tight band (substepping keeps it bounded — "close enough", not exact). With `accel_ref = 1e-4`, a 1 AU orbit gets `n = 2` substeps, so this test genuinely exercises the substep path (it is no longer the always-`n=1` false-green of the old log-base config). Add to the `tests` module:

  ```rust
  #[test]
  fn near_circular_orbit_stays_bounded() {
      let v = VelocityVerlet;
      // Production-regime reference accel (AU/day^2): a 1 AU orbit gets n=2 here.
      let cfg = SubstepCfg { accel_ref: 1.0e-4, max_substeps: 64 };
      let m = 1.0_f64; // central mass (M_sun)
      let radius = 1.0_f64; // 1 AU
      let softening = 1.0e-6_f64;
      let body_pos = Vec3::ZERO;
      let mu = G_CANONICAL * m;
      let speed = (mu / radius).sqrt();
      let mut pos = Vec3::new(radius, 0.0, 0.0);
      let mut vel = Vec3::new(0.0, speed, 0.0);
      let dt = 1.0_f64; // 1 day per tick

      // Confirm the schedule actually escalates at this orbit (guards the redesign).
      let g0 = gravity_accel(pos, &[(body_pos, m)], softening).length();
      assert!(substep_count(g0, cfg) >= 2, "reference-accel schedule did not engage at 1 AU");

      let mut r_min = f64::INFINITY;
      let mut r_max = 0.0_f64;
      for _ in 0..2000 {
          // accel closure: softened gravity from the central body (no thrust here).
          let field = |p: Vec3, _t: f64| gravity_accel(p, &[(body_pos, m)], softening);
          // accel-keyed substeps from the QUANTIZED gravity magnitude at current pos.
          let g_mag = gravity_accel(pos, &[(body_pos, m)], softening).length();
          let n = substep_count(g_mag, cfg);
          let (p1, v1) = v.step_craft(pos, vel, &field, dt, n);
          pos = p1;
          vel = v1;
          let r = pos.length();
          r_min = r_min.min(r);
          r_max = r_max.max(r);
      }
      // Bounded, not golden: radius stays within ±5% of 1 AU over 2000 days.
      assert!(r_min > 0.95 && r_max < 1.05, "orbit unbounded: r in [{}, {}]", r_min, r_max);
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::near_circular -- --nocapture
  ```
  EXPECTED: PASSES immediately (all implementations from Steps 4/6/8 are in place). This is a regression guard, not a red-then-green step. If it FAILS on the `substep_count >= 2` assertion, the reference-accel redesign regressed; if it FAILS on the bound, the orbit is blowing up — lower `accel_ref` so a 1-AU orbit gets more substeps. Confirm green before continuing.

- [ ] **Step 12: Add a test — Verlet and RK4 agree to coarse tolerance on a partial orbit (coast under gravity).**

  Both integrators on the SAME softened-gravity field over a short arc must agree to a coarse tolerance (per the spec: "Verlet and RK4 agree to coarse tolerance on a coast"). Add to the `tests` module:

  ```rust
  #[test]
  fn verlet_and_rk4_agree_on_coast_arc() {
      let m = 1.0_f64;
      let softening = 1.0e-6_f64;
      let body_pos = Vec3::ZERO;
      let mu = G_CANONICAL * m;
      let radius = 1.0_f64;
      let speed = (mu / radius).sqrt();
      let pos0 = Vec3::new(radius, 0.0, 0.0);
      let vel0 = Vec3::new(0.0, speed, 0.0);
      let dt = 1.0_f64;
      let n = 32u32; // fine, fixed substeps for both => fair comparison
      let field = |p: Vec3, _t: f64| gravity_accel(p, &[(body_pos, m)], softening);

      let mut pv = (pos0, vel0);
      let mut pr = (pos0, vel0);
      let verlet = VelocityVerlet;
      let rk4 = Rk4;
      for _ in 0..50 {
          pv = verlet.step_craft(pv.0, pv.1, &field, dt, n);
          pr = rk4.step_craft(pr.0, pr.1, &field, dt, n);
      }
      let pos_gap = pv.0.sub(pr.0).length();
      let vel_gap = pv.1.sub(pr.1).length();
      // Coarse agreement: well under 1% of an AU / orbital speed after 50 days.
      assert!(pos_gap < 1.0e-3, "verlet vs rk4 pos gap {}", pos_gap);
      assert!(vel_gap < 1.0e-3, "verlet vs rk4 vel gap {}", vel_gap);
  }
  ```

  Run:
  ```
  cargo test -p jumpgate-core integrator::tests::verlet_and_rk4 -- --nocapture
  ```
  EXPECTED: PASSES (both integrators implemented). If the gap exceeds tolerance, raise `n` in the test (both must use the same `n`); do NOT loosen below a coarse 1e-3 without re-deriving — a large gap signals a real integrator bug.

- [ ] **Step 13: Run the full integrator test set and clippy.**

  ```
  cargo test -p jumpgate-core integrator -- --nocapture
  ```
  EXPECTED: `test result: ok. 6 passed` (gravity_softened, substep_count_reference_accel_grounded, verlet_coast, rk4_coast, near_circular, verlet_and_rk4).

  ```
  cargo clippy -p jumpgate-core --all-targets -- -D warnings
  ```
  EXPECTED: `Finished` with no warnings (this crate is a library/binary; `--all-targets` is required to lint the inline test module — `--lib` alone would skip tests). No `disallowed-methods` hits (no `SystemTime`/`Instant::now`/`thread_rng` introduced). Confirm there is exactly one `Integrator` trait definition in the workspace — `grep -rn "pub trait Integrator" crates/jumpgate-core/src` must return ONLY `contract.rs`.

- [ ] **Step 14: Commit.**

  ```
  git add crates/jumpgate-core/src/integrator.rs crates/jumpgate-core/src/lib.rs
  git commit -m "$(cat <<'EOF'
  Task 8: Integrator impls (Verlet two-eval + RK4) + reference-accel substepping + softened gravity

  - impls of contract::Integrator only; no duplicate trait body; Integrator NOT re-exported from this module
  - gravity_accel: softened kernel G·M·d/(|d|²+ε²)^1.5 summed over bodies; no hard cutoff
  - substep_count: n = 1 + floor(log2(max(1, mag/accel_ref))), fixed log base 2, clamped [1, max_substeps]; identical on replay. Reference-accel (AU/day²) schedule replaces the log-base form, which pinned n=1 for all production configs (accel_ref<1) — a false-green
  - VelocityVerlet::step_craft: two field evals per substep (t_n and t_{n+1}), tagged to prevent O(dt) collapse
  - Rk4::step_craft: classic 4-stage RK4 for golden/validation
  - integrator decoupled from thrust/fuel: receives scalar n_substeps + accel_at closure
  - tests: softened-kernel closed form, substep schedule on physically-grounded inputs (1 AU=>2, 0.1 AU=>9), exact coast (both), bounded near-circular orbit with substeps engaged, Verlet≈RK4 on a coast arc

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  )"
  ```
  EXPECTED: a commit on a non-default branch (already on `jumpgate-v1-design`). `git log --oneline -1` shows the Task 8 commit.

---

### Task 9: Ship variable-mass dynamics (Tsiolkovsky) + fuel-empty

**Goal:** Implement `thrust_accel_and_burn` — variable-mass thrust-to-acceleration with fuel consumption keyed on effective exhaust velocity, applied at substep granularity, with fuel-empty producing zero thrust. The function reads `Effective` only (never `base_*`). The `FuelEmpty` *event* is emitted later (Task 11); here we only produce the physical result and the zero-thrust-when-dry behaviour.

**Depends on:** Task 8 (`stores.rs`: `Effective`, `BaseSpec`, `effective_params`) and Task 1 (`math.rs`: `Vec3`).

**Files**
- Create: `crates/jumpgate-core/src/ship.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`
- Test: `crates/jumpgate-core/src/ship.rs` (inline `#[cfg(test)] mod tests`)

**Physics contract (from spec §5.5, drafter notes):**
- `thrust_force = throttle * eff.max_thrust`
- `accel = thrust_force * dir / (eff.dry_mass + fuel_mass)` (variable total mass)
- `fuel_consumed = thrust_force / eff.exhaust_velocity * dt`, clamped to available `fuel_mass`
- When `fuel_mass <= 0` (or `throttle <= 0`), thrust contributes zero accel and zero burn.
- `dir` is taken as given (caller passes a unit vector from the autopilot); we do not re-normalize here.

---

- [ ] **Step 1: Add the failing module wiring + first test (zero throttle → zero accel, zero burn).**

Create `crates/jumpgate-core/src/ship.rs` with the test module first, plus a stub that compiles but is wrong, so the test fails for a real reason. Write the file:

```rust
//! Ship variable-mass dynamics (Tsiolkovsky). Reads `Effective` only.

use crate::math::Vec3;
use crate::stores::Effective;

/// Variable-mass thrust step.
///
/// `dir` is the (already-unit) thrust direction supplied by the autopilot.
/// Returns `(accel, fuel_consumed)`:
/// - `accel`   = `throttle * eff.max_thrust * dir / (eff.dry_mass + fuel_mass)`
/// - `fuel_consumed` = `throttle * eff.max_thrust / eff.exhaust_velocity * dt`,
///   clamped to the available `fuel_mass`.
///
/// When `throttle <= 0` or `fuel_mass <= 0`, thrust contributes nothing:
/// returns `(Vec3::ZERO, 0.0)`.
pub fn thrust_accel_and_burn(
    eff: &Effective,
    fuel_mass: f64,
    thrust_dir: Vec3,
    throttle: f64,
    dt: f64,
) -> (Vec3, f64) {
    // STUB — intentionally wrong so the first test fails.
    let _ = (eff, fuel_mass, thrust_dir, throttle, dt);
    (Vec3::new(1.0, 0.0, 0.0), 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::effective_params;
    use crate::config::BaseSpec;

    fn eff_fixture() -> Effective {
        // dry_mass 1, max_thrust 1, exhaust_velocity 10, fuel_capacity 2
        effective_params(&BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 1.0,
            base_exhaust_velocity: 10.0,
            base_fuel_capacity: 2.0,
        })
    }

    #[test]
    fn zero_throttle_yields_zero_accel_and_zero_burn() {
        let eff = eff_fixture();
        let (a, consumed) =
            thrust_accel_and_burn(&eff, 2.0, Vec3::new(1.0, 0.0, 0.0), 0.0, 0.1);
        assert_eq!(a, Vec3::ZERO);
        assert_eq!(consumed, 0.0);
    }
}
```

- [ ] **Step 2: Wire the module into `lib.rs`.**

Add the module declaration and re-export so the crate compiles and other tasks can reach the symbol. In `crates/jumpgate-core/src/lib.rs`, add the `ship` module alongside the existing module list:

```rust
pub mod ship;
```

and add to the public re-exports block:

```rust
pub use ship::thrust_accel_and_burn;
```

- [ ] **Step 3: Run the first test and confirm it FAILS.**

```
cargo test -p jumpgate-core ship::tests::zero_throttle -- --nocapture
```

EXPECTED: a failing assertion, e.g.
`assertion ... left: Vec3 { x: 1.0, y: 0.0, z: 0.0 }, right: Vec3 { x: 0.0, y: 0.0, z: 0.0 }`
ending in `test result: FAILED. 0 passed; 1 failed`.

- [ ] **Step 4: Implement the real function (minimal).**

Replace the stub body in `crates/jumpgate-core/src/ship.rs`:

```rust
pub fn thrust_accel_and_burn(
    eff: &Effective,
    fuel_mass: f64,
    thrust_dir: Vec3,
    throttle: f64,
    dt: f64,
) -> (Vec3, f64) {
    // No thrust if commanded off or tank is dry.
    if throttle <= 0.0 || fuel_mass <= 0.0 {
        return (Vec3::ZERO, 0.0);
    }
    let thrust_force = throttle * eff.max_thrust;
    let total_mass = eff.dry_mass + fuel_mass;
    let accel = thrust_dir.scale(thrust_force / total_mass);
    // Variable-mass consumption: mdot = F / v_e; clamp to what's in the tank.
    let consumed = (thrust_force / eff.exhaust_velocity * dt).min(fuel_mass);
    (accel, consumed)
}
```

- [ ] **Step 5: Run the first test and confirm it PASSES.**

```
cargo test -p jumpgate-core ship::tests::zero_throttle -- --nocapture
```

EXPECTED: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Add the fuel-cannot-go-negative test.**

Append to the `tests` module in `crates/jumpgate-core/src/ship.rs`:

```rust
    #[test]
    fn fuel_consumed_clamped_to_available() {
        let eff = eff_fixture();
        // Tiny tank, huge dt: raw mdot*dt = 1.0/10.0*100.0 = 10.0 >> 0.001.
        let fuel = 0.001_f64;
        let (_a, consumed) =
            thrust_accel_and_burn(&eff, fuel, Vec3::new(1.0, 0.0, 0.0), 1.0, 100.0);
        assert!(consumed <= fuel, "consumed {consumed} must not exceed fuel {fuel}");
        assert!((consumed - fuel).abs() < 1e-12, "should consume exactly the tank");
    }
```

- [ ] **Step 7: Run the clamp test and confirm it PASSES.**

```
cargo test -p jumpgate-core ship::tests::fuel_consumed_clamped -- --nocapture
```

EXPECTED: `test result: ok. 1 passed; 0 failed`. (No prior failing run needed — the clamp branch already exists from Step 4; this test pins the behaviour against regression.)

- [ ] **Step 8: Add the accel-rises-as-fuel-drops test.**

Append to the `tests` module:

```rust
    #[test]
    fn accel_rises_as_fuel_drops_at_constant_throttle() {
        let eff = eff_fixture();
        let dir = Vec3::new(1.0, 0.0, 0.0);
        // Same throttle, less fuel => smaller total mass => larger accel.
        let (a_full, _) = thrust_accel_and_burn(&eff, 2.0, dir, 1.0, 0.1);
        let (a_low, _) = thrust_accel_and_burn(&eff, 0.5, dir, 1.0, 0.1);
        // dry=1,max_thrust=1: full -> 1/(1+2)=0.333..., low -> 1/(1+0.5)=0.666...
        assert!((a_full.x - (1.0 / 3.0)).abs() < 1e-12);
        assert!((a_low.x - (1.0 / 1.5)).abs() < 1e-12);
        assert!(a_low.length() > a_full.length(), "accel must rise as fuel drops");
    }
```

- [ ] **Step 9: Run the accel-rises test and confirm it PASSES.**

```
cargo test -p jumpgate-core ship::tests::accel_rises -- --nocapture
```

EXPECTED: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 10: Add the Tsiolkovsky-burn convergence test.**

This integrates many small steps, accumulating Δv from `accel·dt` and depleting fuel each step, then checks total fuel consumed against the Tsiolkovsky prediction `m0·(1 - exp(-Δv/v_e))` for the *achieved* Δv. (Euler mass-flow converges to the exact integral as dt→0; verified offline at rel-err ~1.7e-5 for these numbers.) Append to the `tests` module:

```rust
    #[test]
    fn known_burn_consumes_tsiolkovsky_fuel() {
        let eff = eff_fixture(); // dry=1, max_thrust=1, v_e=10, cap=2
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let dt = 0.001_f64;
        let fuel0 = 2.0_f64;

        let mut fuel = fuel0;
        let mut dv = 0.0_f64;
        for _ in 0..2000 {
            let (a, consumed) = thrust_accel_and_burn(&eff, fuel, dir, 1.0, dt);
            if consumed <= 0.0 {
                break; // tank dry
            }
            fuel -= consumed;
            dv += a.length() * dt;
        }
        let consumed_total = fuel0 - fuel;

        // Tsiolkovsky: m1 = m0 * exp(-dv / v_e); predicted consumed = m0 - m1.
        let m0 = eff.dry_mass + fuel0;
        let m1 = m0 * (-dv / eff.exhaust_velocity).exp();
        let pred_consumed = m0 - m1;

        let rel_err = (consumed_total - pred_consumed).abs() / pred_consumed;
        assert!(
            rel_err < 1e-3,
            "consumed {consumed_total} vs Tsiolkovsky {pred_consumed} (dv={dv}, rel_err={rel_err})"
        );
    }
```

- [ ] **Step 11: Run the Tsiolkovsky test and confirm it PASSES.**

```
cargo test -p jumpgate-core ship::tests::known_burn -- --nocapture
```

EXPECTED: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 12: Run the full ship test set and clippy (test modules included).**

```
cargo test -p jumpgate-core ship -- --nocapture
cargo clippy -p jumpgate-core --all-targets -- -D warnings
```

EXPECTED: first command `test result: ok. 4 passed; 0 failed`; clippy finishes with no warnings (note: `--all-targets`, not `--lib`, so the inline test module is linted — this is a binary/lib crate where `--lib` would skip test code).

- [ ] **Step 13: Commit.**

```
git add crates/jumpgate-core/src/ship.rs crates/jumpgate-core/src/lib.rs
git commit -m "Task 9: ship variable-mass thrust + fuel burn (Tsiolkovsky)

thrust_accel_and_burn reads Effective only; accel = F/(dry+fuel),
mdot = F/v_e clamped to tank; zero thrust when throttle<=0 or dry.
Tests: zero-throttle, fuel-clamp, accel-rises-as-fuel-drops, and a
multi-step burn matching Tsiolkovsky within 1e-3.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

EXPECTED: a commit on the current feature branch (branch first if on `main`).
