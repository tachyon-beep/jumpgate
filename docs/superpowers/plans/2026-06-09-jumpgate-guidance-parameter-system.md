# Guidance Parameter System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Migrate the guidance tuning constants (`K_BRAKE`, `V_CRUISE`, `V_ERR_EPS`) into a config-hashed `GuidanceParams` policy struct, make arrival detection robust to step size (swept test), and add a reset-time resolvability guard — closing the anti-tunnel hole and establishing the three-class determinism taxonomy in code.

**Architecture:** Three-class taxonomy — Class-1 module `const` (physical law), Class-2 config-hashed `RunConfig` field (run-level policy), Class-3 derived-from-hashed-inputs (no new hash slot). The per-ship cruise *speed* cap is derived at the autopilot from `GuidanceParams` + `Effective` (policy stays out of the pure `effective_params` seam); arrival becomes a swept segment-vs-moving-sphere closest-approach test; `World::reset` rejects configs whose worst-case braking would tunnel.

**Tech Stack:** Rust 2024, `crates/jumpgate-core` (binary-determinism Tier B), FNV-1a-over-`to_bits` hashing, no-FMA arithmetic discipline, `cargo test -p jumpgate-core` + `cargo clippy --all-targets`.

---

## Context the executor needs before starting

**Read first (authoritative spec):** `docs/superpowers/specs/2026-06-09-guidance-parameter-system-design.md` (decisions D1–D13, derivations, determinism arguments). This plan operationalizes it; the spec is the *why*, this is the *how*.

**Prerequisites already landed on `jumpgate-v1-design` (do NOT redo):**
- The prelude: `ShipStore → CraftStore` rename (`403b74d`) and `config_hash` exhaustive-destructure + `GOLDEN_CONFIG_HASH = 0x9767_52c4_8d05_053c` anchor (`b856b67`). `config.rs:config_hash` **already** binds `let RunConfig { master_seed, dt, softening, substep_cfg, ephemeris_window, bodies, craft } = self;` with no rest-pattern — you only **append** `guidance`.
- The μ correctness fix (`bf97147`, D12/§12.4/Q3) — **DONE**. The body-mass/ephemeris-μ work is already merged and its issue (`jumpgate-fca8c9e0c0`) closed. Do **not** re-open it; this plan's only remaining tie is to file the catalogue note (Task 7) reflecting it as resolved.

**Determinism discipline (applies to every task):**
- All float folding is `f64::to_bits()`; never hash a raw `f64`.
- **No `f64::mul_add` / FMA anywhere.** Products are left-to-right exactly as the existing code writes them. The recorded hashes were captured under this grouping.
- After any task that *could* move a hash, prove the three pinned goldens are byte-unchanged unless the task explicitly re-pins one: `GOLDEN_CONFIG_HASH` (`config.rs`, the `0x9767…` anchor — Task 2 deliberately re-pins it), `GOLDEN_ZERO_STATE_HASH = 0xf0dd_a1ba_f433_3735` (`hash.rs`), and the cfg-with-craft state golden `0x532d_07bf_95a2_abc5` (`hash.rs`, in `state_hash_golden_zero_world`).

**Line numbers are advisory.** The spec's §11 call-site line numbers predate the μ commit (`bf97147`), which shifted `hash.rs` and other test files. **Locate call sites by symbol with grep**, not by line number:
- `rg 'World::reset\(' crates/` — every `reset` caller (Task 4).
- `rg 'autopilot_command\(' crates/` — every autopilot caller (Task 3).
- `rg 'RunConfig \{' crates/` — every full struct literal needing `guidance:` (Task 2).
Treat the spec's counts (24 reset sites, 8 autopilot sites, 7 literals) as a checklist target; the compiler is the real driver — the build does not go green until every site is updated.

**File-structure map (what each task creates/modifies):**
| File | Responsibility | Tasks |
|---|---|---|
| `math.rs` | `tsiolkovsky_dv` helper (pure scalar Δv) | 1 |
| `config.rs` | `GuidanceParams` struct + `Default`; `RunConfig.guidance`; fold into `config_hash`; re-pin golden; perturbation tests; `CONFIG_FIELD_ORDER` doc | 2 |
| `autopilot.rs` | delete the 3 consts; `autopilot_command` reads `guidance` + `dt`; cruise cap = fraction × full-tank Δv; backstop `debug_assert` | 3 |
| `world.rs` | `reset → Result<_, ResetError>` + guard; `ResetError`; autopilot call site; `prev_pos` init + copy-forward | 3,4,6 |
| `ingest.rs` | live-path dv budget INFINITY → fuel-derived | 5 |
| `events.rs` | `ARRIVAL_SPEED` const; `arrival_swept` replaces `arrival_crossed`; resolve `c_prev`/`c_now`/`rel_speed` | 6 |
| `stores.rs` | `CraftStore.prev_pos` column (empty/push) | 6 |
| `lib.rs` | export `tsiolkovsky_dv`, `GuidanceParams`, `ResetError` | 1,2,4 |

---

## Task 1: `tsiolkovsky_dv` shared helper (D8 / §7)

A pure scalar Δv helper with one definition and two callers, so the rocket equation cannot drift between the live nav budget and the autopilot cruise cap. Re-pointing the existing inline `dv_from_fuel` at it is **bit-for-bit hash-neutral** (same operands, same grouping).

**Files:**
- Modify: `crates/jumpgate-core/src/math.rs` (add `tsiolkovsky_dv`)
- Modify: `crates/jumpgate-core/src/lib.rs` (export it on the `math` re-export line)
- Modify: `crates/jumpgate-core/src/ingest.rs` (`dv_from_fuel` delegates to the helper)
- Test: `crates/jumpgate-core/src/math.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing test for the helper**

In `math.rs` `#[cfg(test)] mod tests`, add:

```rust
#[test]
fn tsiolkovsky_dv_matches_rocket_equation() {
    // v_e * ln((dry + prop)/dry); left-to-right, ratio inside ln.
    let got = tsiolkovsky_dv(1.0e-2, 1.0e-9, 1.0e-9);
    let want = 1.0e-2 * ((1.0e-9_f64 + 1.0e-9) / 1.0e-9).ln(); // = 1e-2 * ln 2
    assert_eq!(got, want);
    assert_eq!(got.to_bits(), want.to_bits(), "must match bit-for-bit (grouping)");
}

#[test]
fn tsiolkovsky_dv_degenerate_inputs_return_zero() {
    assert_eq!(tsiolkovsky_dv(1.0e-2, 0.0, 1.0), 0.0, "no dry mass -> 0, not Inf");
    assert_eq!(tsiolkovsky_dv(1.0e-2, 1.0, 0.0), 0.0, "no propellant -> 0");
    assert_eq!(tsiolkovsky_dv(1.0e-2, -1.0, 1.0), 0.0, "negative dry -> 0");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jumpgate-core tsiolkovsky_dv`
Expected: FAIL — `cannot find function tsiolkovsky_dv in this scope`.

- [ ] **Step 3: Implement the helper**

Add to `math.rs` (module scope, near `G_CANONICAL`):

```rust
/// Ideal-rocket (Tsiolkovsky) Δv: `Δv = v_e · ln((dry + prop) / dry)`.
///
/// Precondition: `dry_mass > 0.0` (a massless dry hull has unbounded Δv and is
/// non-physical). Returns `0.0` for `dry_mass <= 0.0` or `propellant_mass <= 0.0`
/// (no tank / no budget) rather than NaN/Inf; a `debug_assert!` traps producer bugs.
///
/// Pinned numerics: the product is LEFT-TO-RIGHT (`v_e * ln(...)`), the mass ratio
/// is formed inside the `ln` argument, NO `mul_add`/FMA — the exact grouping the
/// recorded hashes were captured under (matches the prior inline `dv_from_fuel`).
pub fn tsiolkovsky_dv(exhaust_velocity: f64, dry_mass: f64, propellant_mass: f64) -> f64 {
    debug_assert!(dry_mass > 0.0, "tsiolkovsky_dv requires dry_mass > 0");
    if dry_mass <= 0.0 || propellant_mass <= 0.0 {
        0.0
    } else {
        exhaust_velocity * ((dry_mass + propellant_mass) / dry_mass).ln()
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jumpgate-core tsiolkovsky_dv`
Expected: PASS (both tests).

- [ ] **Step 5: Export the helper**

In `lib.rs`, change the math re-export from `pub use math::{G_CANONICAL, Vec3};` to:

```rust
pub use math::{G_CANONICAL, Vec3, tsiolkovsky_dv};
```

- [ ] **Step 6: Re-point `dv_from_fuel` at the helper (hash-neutral)**

In `ingest.rs`, replace the body of `dv_from_fuel` so it delegates (same operands, same grouping):

```rust
/// Fuel-derived Δv fallback when no explicit budget is given: Tsiolkovsky Δv via
/// the shared `math::tsiolkovsky_dv` helper, using effective params (§5.5). Bit-for-bit
/// identical to the prior inline form (same operands, same left-to-right grouping).
fn dv_from_fuel(ship: &CraftStore, idx: usize) -> f64 {
    let eff = crate::stores::effective_params(&ship.spec[idx]);
    crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ship.fuel_mass[idx])
}
```

- [ ] **Step 7: Verify the full suite is still green (hash-neutral proof)**

Run: `cargo test -p jumpgate-core` then `cargo clippy --all-targets`
Expected: all green, zero warnings. The existing ingest test `out_of_order_yields_same_navstate_as_presorted` and every replay/golden test pass unchanged — `dv_from_fuel` produces bit-identical Δv, so no `state_hash` moves.

- [ ] **Step 8: Commit**

```bash
git add crates/jumpgate-core/src/math.rs crates/jumpgate-core/src/lib.rs crates/jumpgate-core/src/ingest.rs
git commit -m "feat(core): tsiolkovsky_dv shared helper; dv_from_fuel delegates (hash-neutral) (D8)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `GuidanceParams` struct + config_hash fold + golden re-pin (D4 / D5 / D10-append / D13 / §4 / §9)

Introduce the Class-2 policy struct, append it as the last `RunConfig` field, fold its three fields at the **tail** of `config_hash` (existing byte-stream prefix stays identical), and deliberately re-pin `GOLDEN_CONFIG_HASH` (adding a folded field legitimately moves it — same discipline as the state golden).

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` (struct + `Default`; `RunConfig.guidance`; destructure append + 3 folds; `CONFIG_FIELD_ORDER` doc; re-pin golden; 3 perturbation tests; update `sample()`)
- Modify: `crates/jumpgate-core/src/lib.rs` (export `GuidanceParams`)
- Modify: every other full `RunConfig { .. }` literal (tests/fixtures) to add `guidance: GuidanceParams::default()`
- Test: `crates/jumpgate-core/src/config.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing perturbation + golden tests**

In `config.rs` `#[cfg(test)] mod tests`, add (these reference `c.guidance`, which does not exist yet → fail):

```rust
#[test]
fn changing_cruise_burn_fraction_changes_hash() {
    let mut c = sample();
    c.guidance.cruise_burn_fraction = 0.30;
    assert_ne!(sample().config_hash(), c.config_hash());
}

#[test]
fn changing_k_brake_changes_hash() {
    let mut c = sample();
    c.guidance.k_brake = 0.6;
    assert_ne!(sample().config_hash(), c.config_hash());
}

#[test]
fn changing_v_err_eps_changes_hash() {
    let mut c = sample();
    c.guidance.v_err_eps = 2.0e-4;
    assert_ne!(sample().config_hash(), c.config_hash());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p jumpgate-core --lib config`
Expected: FAIL to compile — `no field guidance on type RunConfig`.

- [ ] **Step 3: Define `GuidanceParams` + add the `RunConfig` field**

In `config.rs`, add the struct (near `SubstepCfg`):

```rust
/// Class-2 run-level guidance POLICY (config-hashed). Dimensionless tunables a
/// caller may legitimately vary per run; folded into `config_hash` so a changed
/// value yields a different config whose recordings are correctly rejected at the
/// replay config-hash guard. (In a future fleet layer this migrates to a per-fleet
/// attribute; v1 holds it run-level — see spec §13.)
#[derive(Clone, Copy, Debug)]
pub struct GuidanceParams {
    /// Closing-speed cap as a FRACTION of full-tank Tsiolkovsky Δv
    /// (`exhaust_velocity * ln((dry + capacity)/dry)`). Replaces the absolute
    /// `V_CRUISE = 2e-3`. Default 0.25 (D5 derivation note).
    pub cruise_burn_fraction: f64,
    /// Brake-early safety margin (< 1). Exact carryover of the old `K_BRAKE`.
    pub k_brake: f64,
    /// Velocity-matched deadband (canonical AU/day). Exact carryover of `V_ERR_EPS`.
    pub v_err_eps: f64,
}

impl Default for GuidanceParams {
    fn default() -> Self {
        GuidanceParams { cruise_burn_fraction: 0.25, k_brake: 0.5, v_err_eps: 1.0e-4 }
    }
}
```

Add `guidance` as the **last** field of `RunConfig` (after `craft`):

```rust
    pub bodies: Vec<BodyInit>,
    pub craft: Vec<CraftInit>,
    /// Class-2 guidance policy (D4). Folded at the TAIL of config_hash.
    pub guidance: GuidanceParams,
```

- [ ] **Step 4: Append `guidance` to the destructure and fold its three fields at the tail**

In `config_hash`, add `guidance` to the existing exhaustive destructure pattern (the compiler forces this), then fold three `to_bits` words **after** the craft loop, before `ConfigHash(h.finish())`:

```rust
        let RunConfig {
            master_seed,
            dt,
            softening,
            substep_cfg,
            ephemeris_window,
            bodies,
            craft,
            guidance, // NEW (D4): destructure forces folding below
        } = self;
```

…and at the tail (after the `for c in craft { … }` loop):

```rust
        // GUIDANCE (D4/D9) at the TAIL: the existing byte stream above stays
        // byte-identical; config_hash only EXTENDS. Order: cruise_burn_fraction,
        // k_brake, v_err_eps (CONFIG_FIELD_ORDER words below).
        h.write_u64(guidance.cruise_burn_fraction.to_bits());
        h.write_u64(guidance.k_brake.to_bits());
        h.write_u64(guidance.v_err_eps.to_bits());
        ConfigHash(h.finish())
```

- [ ] **Step 5: Add the `CONFIG_FIELD_ORDER` drift-lock doc (D13)**

Above `config_hash` (or at the top of the `impl RunConfig` block), add a doc block mirroring `HASH_FIELD_ORDER`, listing the fold order so a reorder is reviewable:

```rust
//! CONFIG_FIELD_ORDER (config_hash fold order — append-only; re-pin the golden on change):
//!  1. master_seed                       9.  per-body: mass + 6 elements
//!  2. dt.bits()                         10. per-craft: 4 spec + pos[3] + vel[3] + fuel
//!  3. softening.to_bits()               11. guidance.cruise_burn_fraction   (D4)
//!  4. substep_cfg.accel_ref.to_bits()   12. guidance.k_brake                (D4)
//!  5. substep_cfg.max_substeps          13. guidance.v_err_eps              (D4)
//!  6. ephemeris_window
//!  7. bodies.len()   8. craft.len()
```

- [ ] **Step 6: Add `guidance: GuidanceParams::default()` to every full `RunConfig` literal**

Run `rg 'RunConfig \{' crates/` and add the field to each full struct literal. Known sites (verify by grep; line numbers shifted post-μ): `config.rs` `sample()`, `replay_equivalence.rs` `base_config`, `physics_sanity.rs` `star_config` and the second literal, `world.rs` `one_body_one_craft` and `one_body_one_thrusting_craft`, `hash.rs` `cfg_with_craft_x`. The literal that uses `..rec.config.clone()` (spread) needs **no** edit. The compiler error `missing field guidance in initializer of RunConfig` is your checklist — fix until it is silent.

- [ ] **Step 7: Run perturbation tests + re-pin the golden**

Run: `cargo test -p jumpgate-core --lib config`
Expected: the 3 new perturbation tests PASS; `config_hash_golden_anchor_is_stable` **FAILS** with a `config_hash drifted` message showing the new value (the golden legitimately moved because a folded field was added).

Capture the new value from the failure message and re-pin it:

```rust
const GOLDEN_CONFIG_HASH: u64 = 0x____________; // RE-PINNED: +guidance fold (D4). Was 0x9767_52c4_8d05_053c.
```

Re-run: `cargo test -p jumpgate-core --lib config` → all green.

- [ ] **Step 8: Export `GuidanceParams`**

In `lib.rs`, add `GuidanceParams` to the `config` re-export:

```rust
pub use config::{
    BaseSpec, BodyInit, ConfigHash, CraftInit, GuidanceParams, OrbitalElements, RunConfig, SubstepCfg,
};
```

- [ ] **Step 9: Full suite + clippy; confirm state goldens unmoved**

Run: `cargo test -p jumpgate-core` then `cargo clippy --all-targets`
Expected: all green. `GOLDEN_ZERO_STATE_HASH` and `state_hash_golden_zero_world` (`0x532d…`) are **unchanged** (guidance is a config field; `state_hash` does not read it). Only `GOLDEN_CONFIG_HASH` moved, and you re-pinned it deliberately.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(core): GuidanceParams (Class-2 policy) folded into config_hash; re-pin GOLDEN_CONFIG_HASH (D4/D5/D13)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: wire `GuidanceParams` + `dt` into `autopilot_command`; delete the consts (D1 / D2 / §4)

Change the autopilot to read the cruise cap as `cruise_burn_fraction × full-tank Δv` (ship-dependent, trajectory-constant) and read `k_brake`/`v_err_eps` from policy. Delete the three module consts (`ARRIVAL_RADIUS` is kept). Add the `dt` parameter and the §6.6 backstop `debug_assert`. **The cruise-cap change re-derives the cruise-axis physics tests** (`physics_sanity.rs`); the per-tick state goldens do NOT move (golden configs use `Idle` nav, which never invokes the cap).

**Files:**
- Modify: `crates/jumpgate-core/src/autopilot.rs` (delete `K_BRAKE`/`V_CRUISE`/`V_ERR_EPS`; new signature; cap derivation; backstop; module doc; 8 in-module test call sites)
- Modify: `crates/jumpgate-core/src/world.rs` (the `autopilot_command(...)` call site passes `&self.config.guidance`, `dt`)
- Test: `crates/jumpgate-core/src/autopilot.rs` (carryover + cruise-cap tests)

- [ ] **Step 1: Write the failing carryover + cruise-cap tests**

In `autopilot.rs` tests, add (these use the new signature → fail to compile):

```rust
fn guidance() -> crate::config::GuidanceParams {
    crate::config::GuidanceParams::default()
}

#[test]
fn k_brake_and_v_err_eps_defaults_are_exact_carryover() {
    // Same braking scenario as `brakes_when_overspeeding_toward_dest`, asserted
    // bit-identical under default policy (k_brake=0.5, v_err_eps=1e-4 == old consts).
    let dest = Vec3::new(0.0, 0.0, 0.0);
    let pos = Vec3::new(0.01, 0.0, 0.0);
    let vel = Vec3::new(-1.0, 0.0, 0.0);
    let (dir, throttle) = autopilot_command(
        // dt = 1e-4, NOT 0.25: dt feeds ONLY the backstop debug_assert, and eff()
        // (a_max=0.5) trips it at 0.25 (0.5*0.0625 = 0.03125 >= R/(2k)=1e-4). 1e-4
        // passes (0.5*1e-8 = 5e-9 < 1e-4) and leaves the trajectory assertion intact.
        seeking(dest), pos, vel, dest, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4,
    );
    assert_eq!(throttle, 1.0);
    assert!(dir.dot(dest.sub(pos).normalize_or_zero()) < 0.0, "still brakes retrograde");
}

#[test]
fn cruise_cap_is_fraction_of_full_tank_dv() {
    // eff(): dry=1, max_thrust=1, v_e=1, capacity=1 => full_tank_dv = 1*ln(2) = 0.693147…
    // cap = 0.25 * full_tank_dv = 0.173287…; far & slow so v_des is cap-limited, not brake-limited.
    let full_tank_dv = crate::math::tsiolkovsky_dv(1.0, 1.0, 1.0);
    let expected_cap = 0.25 * full_tank_dv;
    let dest = Vec3::new(1.0e6, 0.0, 0.0); // very far: v_brake >> cap, so v_des == cap
    let pos = Vec3::ZERO;
    // Craft already moving at exactly the cap toward dest -> v_err ~ 0 (matched at cap).
    let vel = Vec3::new(expected_cap, 0.0, 0.0);
    let (_dir, throttle) = autopilot_command(
        seeking(dest), pos, vel, dest, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4, // dt: backstop-only
    );
    assert_eq!(throttle, 0.0, "at the cap with zero residual error -> within deadband, coast");
    // Bracket the cap magnitude (a factor-of-2 cap bug would suppress at the wrong
    // speed): moving FASTER than the cap toward dest must command a retrograde brake.
    let faster = Vec3::new(expected_cap * 2.0, 0.0, 0.0);
    let (dir2, throttle2) = autopilot_command(
        seeking(dest), pos, faster, dest, Vec3::ZERO, 1.0, &eff(), &guidance(), 1.0e-4,
    );
    assert_eq!(throttle2, 1.0);
    assert!(dir2.x < 0.0, "over the cap -> brake retrograde (pins cap magnitude, not just deadband)");
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p jumpgate-core --lib autopilot`
Expected: FAIL to compile — `autopilot_command` takes 7 args, not 9.

- [ ] **Step 3: Change the signature, delete the consts, derive the cap**

In `autopilot.rs`: delete `pub const K_BRAKE`, `pub const V_CRUISE`, `pub const V_ERR_EPS` (keep `ARRIVAL_RADIUS`). Update the module doc (lines referencing `V_CRUISE`/`K_BRAKE`/`V_ERR_EPS` as consts → "from `GuidanceParams`"). Add the import `use crate::config::GuidanceParams;`. Replace the signature + body:

```rust
pub fn autopilot_command(
    nav: NavState,
    pos: Vec3,
    vel: Vec3,
    dest_pos: Vec3,
    dest_vel: Vec3,
    fuel_mass: f64,
    eff: &Effective,
    guidance: &GuidanceParams, // NEW (D1/D4): run-level policy
    dt: f64,                   // NEW (D6 backstop only; feeds no trajectory arithmetic)
) -> (Vec3, f64) {
    match nav {
        NavState::Idle => (Vec3::ZERO, 0.0),
        NavState::Seeking { dv_remaining, .. } => {
            let rel_pos = dest_pos.sub(pos);
            let d = rel_pos.length();
            if d <= ARRIVAL_RADIUS || dv_remaining <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let dir = rel_pos.normalize_or_zero();
            let rel_vel = vel.sub(dest_vel);
            let a_max = eff.max_thrust / (eff.dry_mass + fuel_mass);
            // §6.6 backstop: the reset guard (Task 4) guarantees this for the
            // empty-tank worst case; live a_max <= empty-tank a_max, so this can
            // only fire if reset was bypassed or effective params drift above base.
            // Compiled out of release; feeds NO arithmetic, so it cannot affect the hash.
            debug_assert!(
                a_max * dt * dt < ARRIVAL_RADIUS / (2.0 * guidance.k_brake),
                "unbrakable config reached autopilot: a_max*dt^2={} >= R/(2K)={}",
                a_max * dt * dt,
                ARRIVAL_RADIUS / (2.0 * guidance.k_brake)
            );
            // Left-to-right product (NOT an FMA): 2 * k_brake * a_max * (d - eps).
            let v_brake = (2.0 * guidance.k_brake * a_max * (d - ARRIVAL_RADIUS)).sqrt();
            // Cruise cap = fraction of FULL-tank Δv (trajectory-constant, not
            // shrinking as fuel burns). full_tank_dv left-to-right, ratio inside ln.
            let full_tank_dv =
                crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, eff.fuel_capacity);
            let cruise_cap = guidance.cruise_burn_fraction * full_tank_dv;
            let v_des = dir.scale(cruise_cap.min(v_brake));
            let v_err = v_des.sub(rel_vel);
            if v_err.length() < guidance.v_err_eps {
                return (Vec3::ZERO, 0.0);
            }
            (v_err.normalize_or_zero(), 1.0)
        }
    }
}
```

> **Grouping note:** `cruise_cap = guidance.cruise_burn_fraction * full_tank_dv` is a single plain `*`; `cruise_cap.min(v_brake)` matches the old `V_CRUISE.min(v_brake)` shape. No `mul_add`.

- [ ] **Step 4: Update the 8 in-module autopilot test call sites**

Every existing `autopilot_command(...)` call in `autopilot.rs` tests gains two trailing args: `&guidance()` and a `dt`. **Pass `dt = 1.0e-4` to every `eff()`-based call site — NOT the `0.25` engine cadence.** The `eff()` fixture has `a_max = 0.5`, and at `dt = 0.25` the backstop `debug_assert!(a_max·dt² < R/(2·k_brake))` evaluates `0.5·0.0625 = 0.03125 ≥ 1e-4` and **panics in debug/test builds**, killing every test that reaches the `Seeking` branch. `dt` feeds *only* the `debug_assert`, so trajectory assertions are unchanged; at `1e-4` the bound is `0.5·1e-8 = 5e-9 < 1e-4` and passes. The fine-step loop in `arrives_with_low_relative_speed` already uses its own `dt = 1.0e-4` and is unaffected. Run `rg 'autopilot_command\(' crates/jumpgate-core/src/autopilot.rs` to enumerate all 8 sites; each must receive `dt = 1.0e-4`.

- [ ] **Step 5: Update the production call site in `world.rs`**

Find it with `rg 'autopilot_command\(' crates/jumpgate-core/src/world.rs`. The `World` already owns `config` and `dt` (`let dt = self.dt.get();` at the top of `step`). Change the call to:

```rust
            let (thrust_dir, throttle) = autopilot_command(
                self.ships.nav[ci], pos, vel, dest_pos, dest_vel, fuel, &eff,
                &self.config.guidance, dt,
            );
```

(No new `World` field; `self.config.guidance` and `dt` are already in scope.)

- [ ] **Step 6: Run autopilot tests**

Run: `cargo test -p jumpgate-core --lib autopilot`
Expected: PASS, including the two new tests and the updated existing ones.

- [ ] **Step 7: Re-derive the cruise-axis physics tests, confirm state goldens unmoved**

Run: `cargo test -p jumpgate-core`
- The per-tick state-hash goldens (`golden_zero_state_hash`, `state_hash_golden_zero_world` = `0x532d…`) and the config golden **stay green unchanged** (Idle-nav golden configs never invoke the cruise cap).
- `physics_sanity.rs` cruise-axis tests (`transfer_to_moving_body_rendezvous`, `transfer_arrival_tick_is_*`) exercise the new lower per-ship cap (`0.25 × full_tank_dv` < old `2e-3` for the reference ship → more ticks to arrive). Their asserts are *properties* (e.g. rel-speed-at-arrival ≪ circular speed, arrive within `max_ticks`), expected to hold; if any pins an exact arrival tick that shifted, **re-pin it to the newly measured value** (single-cause: this commit, the cruise-cap change). If a property genuinely breaks (e.g. `max_ticks` now too small), widen the test's tick budget — do **not** weaken the rel-speed property.

Then `cargo clippy --all-targets` → clean (watch for `dead_code` on now-unused imports after the const deletions).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(core): autopilot reads GuidanceParams + dt; cruise cap = fraction x full-tank dv; delete V_CRUISE/K_BRAKE/V_ERR_EPS consts (D1/D2/D5)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: reset-time resolvability guard + `ResetError` (D6 / §6)

`World::reset` becomes fallible and rejects any craft whose worst-case (empty-tank) braking cannot resolve the arrival sphere at the run's `dt` — turning a silent tunnel into a loud config error. Determinism-neutral: the guard runs before tick 0 and reads only config inputs already in `config_hash`.

**Files:**
- Modify: `crates/jumpgate-core/src/world.rs` (`ResetError` enum + `Display`; `reset → Result<(World, ConfigHash), ResetError>`; the guard loop)
- Modify: `crates/jumpgate-core/src/lib.rs` (export `ResetError`)
- Modify: every `World::reset(...)` caller (production uses `?`/`.expect`; tests use `.expect("resolvable config")`)
- Test: `crates/jumpgate-core/src/world.rs` (reject high-thrust; accept resolvable; golden-zero Ok)

- [ ] **Step 1: Write the failing guard tests**

In `world.rs` tests, add (uses `ResetError` + `Result` return → fail):

```rust
#[test]
fn reset_rejects_unbrakable_high_thrust_craft() {
    // dry=1e-9, max_thrust=1e-11 (10x the passing fixture) at dt=0.25:
    // a_max_empty = 1e-2, a_max*dt^2 = 6.25e-4 >= R=1e-4 -> REJECT.
    let cfg = one_body_one_thrusting_craft_with_thrust(1.0e-11); // helper: see Step 4
    match World::reset(cfg) {
        Err(ResetError::Unbrakable { craft_index, .. }) => assert_eq!(craft_index, 0),
        other => panic!("expected Unbrakable, got {other:?}"),
    }
}

#[test]
fn reset_accepts_resolvable_thrusting_craft() {
    // The real fixture: dry=1e-9, max_thrust=1e-12 -> a_max*dt^2 = 6.25e-5 < R -> Ok.
    assert!(World::reset(one_body_one_thrusting_craft()).is_ok());
}

#[test]
fn reset_rejects_zero_dry_mass_craft() {
    // dry = 0 -> a_max_empty = max_thrust/0 = INFINITY; the `dry > 0.0` /
    // `is_finite()` guard branch must reject (else a divide-by-zero ship slips through).
    let mut cfg = one_body_one_thrusting_craft();
    cfg.craft[0].spec.base_dry_mass = 0.0;
    assert!(matches!(World::reset(cfg), Err(ResetError::Unbrakable { craft_index: 0, .. })));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p jumpgate-core --lib world`
Expected: FAIL to compile — `ResetError` undefined; `World::reset` returns a tuple, not `Result`.

- [ ] **Step 3: Add `ResetError` and make `reset` fallible with the guard**

In `world.rs`, near `reset`, add:

```rust
/// A `RunConfig` rejected by `World::reset`'s resolvability guard (§6). Part of
/// the recorded contract surface (replay calls `reset` and asserts its hash), so
/// it is re-exported from `lib.rs` for the gym/FFI layer to match on.
#[derive(Clone, Debug, PartialEq)]
pub enum ResetError {
    /// Craft `craft_index`'s worst-case (empty-tank) braking cannot resolve the
    /// arrival sphere at this `dt`: `a_max_empty * dt^2 >= limit` (limit = R/(2·k_brake)).
    Unbrakable { craft_index: usize, a_max_empty: f64, dt: f64, limit: f64 },
}

impl std::fmt::Display for ResetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResetError::Unbrakable { craft_index, a_max_empty, dt, limit } => write!(
                f,
                "craft {craft_index} is unbrakable: a_max_empty*dt^2 = {} >= R/(2*k_brake) = {limit} \
                 (a_max_empty={a_max_empty}, dt={dt}); remedy: lower max_thrust, raise dry_mass, or shrink dt",
                a_max_empty * dt * dt
            ),
        }
    }
}

impl std::error::Error for ResetError {}
```

Change `reset`'s signature and add the guard before any store is built:

```rust
    pub fn reset(cfg: RunConfig) -> Result<(World, ConfigHash), ResetError> {
        let hash = cfg.config_hash();
        // §6 resolvability guard: reject any craft whose worst-case (empty-tank)
        // braking would tunnel the arrival sphere at this dt. Reads only inputs
        // already in config_hash (dt, base_max_thrust, base_dry_mass, guidance.k_brake),
        // runs before tick 0, persists no state -> determinism-neutral.
        let dt = cfg.dt.get();
        let limit = ARRIVAL_RADIUS / (2.0 * cfg.guidance.k_brake);
        // TODO(forward-debt, Person+Ship / spec §6.5): this reads the BASE max_thrust,
        // but the Task-3 autopilot backstop reads the EFFECTIVE a_max. When EffectiveMods
        // lands and multiplies max_thrust, a crew-boosted craft could pass here on base
        // values yet violate the runtime backstop. The Person line resolves `mods` at reset
        // BEFORE this guard; honour that ordering and read
        // effective_params(&c.spec, &reset_mods).max_thrust here. Identity in v1 (no mods yet).
        for (i, c) in cfg.craft.iter().enumerate() {
            let dry = c.spec.base_dry_mass;
            let a_max_empty = c.spec.base_max_thrust / dry;
            if !(dry > 0.0 && a_max_empty.is_finite() && a_max_empty * dt * dt < limit) {
                return Err(ResetError::Unbrakable { craft_index: i, a_max_empty, dt, limit });
            }
        }
        // … existing ephemeris precompute + body/craft store construction unchanged …
```

…and wrap the existing return: change `(world, hash)` to `Ok((world, hash))`. Add `use crate::autopilot::ARRIVAL_RADIUS;` if not already imported in `world.rs` (it is referenced in `step`'s copy-forward, so likely already in scope via `crate::autopilot::ARRIVAL_RADIUS` — match the existing reference style).

- [ ] **Step 4: Add the high-thrust test fixture helper**

In `world.rs` tests, add a parameterized variant of the existing `one_body_one_thrusting_craft` fixture that lets the test set `base_max_thrust` (locate the existing fixture with `rg 'fn one_body_one_thrusting_craft'` and mirror it):

```rust
fn one_body_one_thrusting_craft_with_thrust(max_thrust: f64) -> RunConfig {
    let mut cfg = one_body_one_thrusting_craft();
    cfg.craft[0].spec.base_max_thrust = max_thrust;
    cfg
}
```

- [ ] **Step 4b: Recalibrate the fixtures that fail the new guard (REQUIRED — verified by review)**

Two existing fixtures have unphysical thrust/mass ratios that the guard correctly rejects. They are latent tunnels today (their tests only check determinism/coast, never anti-tunnel), so making them resolvable is a correctness improvement, not a test weakening. Find each with `rg 'fn one_body_one_craft' crates/jumpgate-core/src/world.rs` and `rg 'fn base_config' crates/jumpgate-core/tests/replay_equivalence.rs` and confirm the live numbers before editing.

- **`one_body_one_craft` (world.rs) — coast fixture, free to lower.** Live numbers: `base_dry_mass = 1e-12`, `base_max_thrust = 1e-13`, `dt = 1.0` → `a_max_empty = 0.1`, `0.1·1² = 0.1 ≥ 1e-4` → rejected. The craft never `Seeking`s in any of its four tests (`reset_starts_at_tick_zero_and_hashes_config`, `step_advances_tick_and_coasts_under_gravity`, `dormant_craft_skips_physics`, `project_respects_observer_visibility_and_accessors`), so thrust magnitude is behaviourally irrelevant. Set `base_max_thrust = 1.0e-17` (`a_max_empty = 1e-5`, `1e-5·1² = 1e-5 < 1e-4` → accepts). Update the fixture comment: "thrust set below the resolvability ceiling (coast fixture; value is not behavioural)."

  ⚠ This changes `one_body_one_craft`'s `config_hash` and any per-tick `state_hash` its tests pin. Check: do those four tests assert a pinned hash? If yes, re-pin it (single-cause: fixture recalibration) — but coast trajectories under gravity do not read `max_thrust`, so the *state* trajectory is identical; only `config_hash` (which folds `base_max_thrust`) moves. Re-pin any `config_hash` assertion these tests make; the state trajectory and its hashes are unchanged.

- **`base_config` (replay_equivalence.rs) — ACTIVELY THRUSTS; coupled retune, highest-risk item in this plan.** Live numbers: `base_dry_mass = 1e-9`, `base_max_thrust = 1e-6`, `dt = 0.5` → `a_max_empty = 1000`, `1000·0.25 = 250 ≥ 1e-4` → rejected, and `discover_craft_id()` resets it so **all four replay-equivalence tests panic**. This craft is commanded to a destination and flies a real burn, so the retune is constrained: pick params that (a) pass the guard `a_max_empty·dt² < 1e-4` i.e. `max_thrust/dry < 1e-4/0.25 = 4e-4`, (b) keep a *meaningful* thrust burn (the test's stated intent: "big enough to exercise gravity + a thrust burn"), and (c) keep `record_then_replay` bit-identical. **Find this empirically — do NOT pin a value blind.** Start by lowering `base_max_thrust` toward `~1e-13` (with `dry = 1e-9` → `a_max_empty = 1e-4`, `1e-4·0.25 = 2.5e-5 < 1e-4`); if the burn becomes trivial, raise `dry_mass` and/or shrink `dt` instead of (or alongside) lowering thrust to preserve the burn while satisfying the ratio. After retuning, re-run the four replay-equivalence tests and confirm: the craft still executes a non-trivial burn AND arrives, and `record_then_replay_is_bit_identical` is green. `base_config` is not a pinned golden, so no golden re-pin — but record/replay bit-identity must still hold.

The companion guard test from Step 1 (`reset_accepts_resolvable_thrusting_craft` using `one_body_one_thrusting_craft`: `dry=1e-9`, `max_thrust=1e-12` → `a_max·dt²=6.25e-5 < 1e-4`) already passes the guard — leave it as-is.

- [ ] **Step 5: Update every `World::reset` caller**

Run `rg 'World::reset\(' crates/`. For each:
- **Production** (`replay.rs`, both sites — `record_run` / `replay_run`): use `?` if the enclosing fn returns a compatible `Result`, otherwise `.expect("config's own hash")` preserving the existing reset-hash assertion semantics. Check the existing signatures; the minimal change that keeps replay's contract is `.expect(...)` since these reset a config the caller just built. If `record_run`/`replay_run` already return `Result`, thread `ResetError` via `?` (you may need a `From<ResetError>` or a map). **Prefer the smallest change that compiles and preserves the asserted reset hash**; the existing `let (world, hash) = World::reset(cfg);` becomes `let (world, hash) = World::reset(cfg).expect("resolvable config");` unless the surrounding code is already fallible.
- **Tests/fixtures** (`world.rs`, `hash.rs`, `physics_sanity.rs`, `replay_equivalence.rs`): `let (world, hash) = World::reset(cfg).expect("resolvable config");`. The golden-zero config `cfg_with_craft_x` passes the guard (`a_max·dt² = 0.1 × 1e-4 = 1e-5 < R`), so the golden tests still reach tick 0 unchanged.

The compiler error `mismatched types: expected tuple, found Result` is your checklist.

- [ ] **Step 6: Run the guard tests + full suite**

Run: `cargo test -p jumpgate-core`
Expected: the two guard tests PASS. **Do NOT assume every pre-existing fixture passes unchanged** — `one_body_one_craft` and `replay_equivalence`'s `base_config` both required recalibration in Step 4b (they were latent tunnels). After Step 4b, every golden/replay/coast test is green again. If any *other* fixture trips the guard, it too was a latent tunnel — recalibrate it the same way (lower `max_thrust`, raise `dry_mass`, or shrink `dt`), and re-pin only its `config_hash` if it asserts one.

Then `cargo clippy --all-targets` → clean.

- [ ] **Step 7: Export `ResetError`**

In `lib.rs`, add to the `world` re-export:

```rust
pub use world::{FullObserver, Observer, ResetError, View, World};
```

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat(core): World::reset resolvability guard + ResetError (anti-tunnel half b) (D6)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: ingest dv-budget reconciliation — live path → fuel-derived (D9 / §8)

The live `World` ingest path defaults a missing `burn_budget` to `f64::INFINITY`; the slice path uses the fuel-derived Tsiolkovsky budget. Make the live path match, so the recorded Δv policy is path-independent and no non-finite word lands in `dv_remaining` (state-hash word 12). **This resolves filigree observation `jumpgate-obs-4d28955902`.**

**Files:**
- Modify: `crates/jumpgate-core/src/ingest.rs` (`ingest_commands` default)
- Test: `crates/jumpgate-core/src/ingest.rs` or `world.rs` (live no-budget command → finite fuel-derived dv)

- [ ] **Step 1: Write the failing test**

The live path needs a `&mut World`. Add this to the **`world.rs` `#[cfg(test)]` module** (where `World`, the fixtures, and the crate-internal `ships` field / `ids_at` accessor are all in scope — confirmed by review: `World.ships` is `pub(crate)`, `CraftStore::ids_at(row)` is `pub`):

```rust
#[test]
fn live_ingest_no_budget_uses_fuel_derived_dv_not_infinity() {
    let (mut world, _h) = World::reset(one_body_one_thrusting_craft()).expect("resolvable");
    let id = world.ships.ids_at(0); // typed CraftId for dense row 0 (no-despawn v1)
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(id)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(Vec3::new(1.0, 0.0, 0.0)),
            burn_budget: None, // no explicit budget -> must derive from fuel, not INFINITY
        },
    }];
    crate::ingest::ingest_commands(&mut world, Tick(0), &mut cmds);
    match world.ships.nav[0] {
        NavState::Seeking { dv_remaining, .. } => {
            assert!(dv_remaining.is_finite(), "dv must be finite, got {dv_remaining}");
            assert!(dv_remaining > 0.0, "fuelled craft has positive dv budget");
        }
        other => panic!("expected Seeking, got {other:?}"),
    }
}
```

> Requires `one_body_one_thrusting_craft` to start with positive `fuel_mass` (it does — it is a thrusting fixture). The assertion is the load-bearing part: `dv_remaining` must be **finite and positive**, never `INFINITY`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p jumpgate-core live_ingest_no_budget`
Expected: FAIL — `dv_remaining` is `f64::INFINITY` (`is_finite()` is false).

- [ ] **Step 3: Change the live default to fuel-derived**

In `ingest.rs` `ingest_commands`, replace the `burn_budget.unwrap_or(f64::INFINITY)` default. The live path must resolve the craft's effective params + current fuel before `set_nav`. Add a narrow `World` read accessor if one does not exist (mirror `set_nav`), or resolve via the existing store access:

```rust
        if let Target::Entity(EntityRef::Craft(id)) = cmd.target {
            let CommandKind::Destination { dest, burn_budget } = cmd.kind;
            // dv budget: explicit cap, else Tsiolkovsky fuel-derived (D9/M5) — path-
            // independent with the slice path, and never INFINITY into dv_remaining.
            let dv = burn_budget.unwrap_or_else(|| world.dv_from_fuel_for(id));
            world.set_nav(id, NavState::Seeking { dest, dv_remaining: dv });
        }
```

Add the helper to `World` (in `world.rs`, near `set_nav`), reusing the shared math helper and effective params (returns `0.0` for a stale id, matching `dv_from_fuel`'s degenerate handling):

```rust
    /// Fuel-derived Δv budget for a live craft (D9): `tsiolkovsky_dv` over effective
    /// params + current fuel. `0.0` for a stale id. The single source the live ingest
    /// path uses when no explicit `burn_budget` is given.
    pub(crate) fn dv_from_fuel_for(&self, id: CraftId) -> f64 {
        match self.ship_index(id) {
            Some(i) => {
                let eff = effective_params(&self.ships.spec[i]);
                crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, self.ships.fuel_mass[i])
            }
            None => 0.0,
        }
    }
```

- [ ] **Step 4: Run the test + full suite**

Run: `cargo test -p jumpgate-core`
Expected: the new test PASSES. **Determinism:** this is a behavioural `state_hash` change for the live no-budget path (a finite value replaces `INFINITY` in `dv_remaining`, word 12) — but **no** `HASH_FORMAT_VERSION` bump (field set/order unchanged) and **no** golden-zero change (the golden configs use `Idle` nav and never ingest a no-budget command). No committed recording exercises this path (§10.5), so the rebaseline is forward-discipline only. Confirm `golden_zero_state_hash`, `state_hash_golden_zero_world`, and `record_then_replay_is_bit_identical` are all still green.

Then `cargo clippy --all-targets` → clean.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "fix(core): live ingest dv budget INFINITY -> fuel-derived (path-independent) (D9); resolves obs-4d28955902

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: swept arrival detection (D7 / §5)

Replace the point-in-sphere arrival predicate with a deterministic swept segment-vs-(moving-)sphere closest-approach test in the target frame, gated by a relative-speed flyby/rendezvous check. Add an unhashed `prev_pos` craft column (transitively pinned, Class-3) to snapshot the chord start. **No `state_hash`/`config_hash`/golden change** — `prev_pos` is a pure copy-forward of `pos` (already hashed at the prior tick) and is not folded; the only behavioural delta is Arrival **event** timing.

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`CraftStore.prev_pos`; `empty()`; `push()`; doc block)
- Modify: `crates/jumpgate-core/src/world.rs` (`reset` inits `prev_pos`; `step` copy-forward; the world.rs ARRIVAL_RADIUS edge consumer relocates into the swept predicate)
- Modify: `crates/jumpgate-core/src/events.rs` (`ARRIVAL_SPEED` const; `arrival_swept` replaces `arrival_crossed`; `detect_boundary_events` resolves `c_prev`/`c_now`/`rel_speed`; repurpose the two in-module arrival tests; new swept tests)
- Test: `crates/jumpgate-core/src/events.rs` (swept behaviour) + an integration check in `physics_sanity.rs` for the moving-body case

- [ ] **Step 1: Add the `prev_pos` column (no behavioural change yet)**

In `stores.rs`:
- Extend the `CraftStore` doc block: list `prev_pos` alongside `prev_fuel`/`prev_inside_dest` as a copy-forward edge-detect snapshot, **not** folded into `state_hash`, transitively pinned (`prev_pos[t] == pos[t-1]`, hashed at word 9 at tick `t-1`).
- Add the field after `prev_inside_dest`: `pub prev_pos: Vec<Vec3>,`
- In `empty()`: add `prev_pos: Vec::new(),`
- In `push()`: add `self.prev_pos.push(pos);` (initialise to current `pos` so the tick-0 chord is zero-length) and extend the doc.

In `world.rs` `reset`'s `CraftStore { … }` literal: add `prev_pos: Vec::new(),`; in the per-craft loop add `ships.prev_pos.push(c.pos);` (mirroring `prev_fuel`).

Update the two `stores.rs` SoA-parallel tests (`stores_construct_soa_parallel`, `shipstore_push_and_accessors`) to assert `ship.prev_pos.len() == n`.

- [ ] **Step 2: Run — confirm still green (column added, unused)**

Run: `cargo test -p jumpgate-core`
Expected: green. The state goldens are **unchanged** (`prev_pos` is not folded into `state_hash`). `clippy --all-targets` may warn `field prev_pos is never read` — that is expected until Step 4 wires it; silence it only by the actual use in Step 4 (do not `#[allow]`).

- [ ] **Step 3: Add the copy-forward in `step`**

In `world.rs` `step`, in the copy-forward loop (the one that sets `prev_fuel`/`prev_inside_dest`, which runs **after** `detect_boundary_events`), add as the first line of the loop body:

```rust
            // TODO(spec §13, deferred): prev_inside_dest below is an ENDPOINT
            // point-in-sphere test, not the swept verdict. A pure chord-clip arrival
            // (closest approach inside R, neither endpoint inside) could re-fire the
            // once-only latch. Out of scope for v1 (the rel_speed gate suppresses the
            // flyby case; the rendezvous case has an endpoint inside R). Deriving
            // inside-prev from the chord is explicitly deferred.
            self.ships.prev_pos[ci] = self.ships.pos[ci];
```

**Ordering invariant (critical):** detection runs *before* this loop, so at detection tick `T` the chord is `[prev_pos = pos(T-1), pos = pos(T)]`. Do not move the copy-forward above detection.

- [ ] **Step 4: Write the failing swept tests**

In `events.rs` tests, add (reference `arrival_swept` + `ARRIVAL_SPEED`, which don't exist → fail). These call the predicate directly with constructed chords:

```rust
use crate::math::Vec3;

// Helper mirrors detect_boundary_events' call for a fixed Position target
// (c_prev == c_now == dest, dest_vel == 0).
fn swept_fixed(prev_pos: Vec3, pos: Vec3, dest: Vec3, vel: Vec3, prev_inside: bool) -> bool {
    let rel_speed = vel.sub(Vec3::ZERO).length();
    arrival_swept(prev_pos, pos, dest, dest, rel_speed, prev_inside)
}

#[test]
fn swept_fires_when_point_test_would_miss() {
    // Chord passes THROUGH the sphere at the origin between ticks; neither endpoint
    // is inside R=1e-4, but the closest approach is. Low rel_speed -> fires.
    let dest = Vec3::ZERO;
    let prev_pos = Vec3::new(-1.0e-3, 0.0, 0.0);
    let pos = Vec3::new(1.0e-3, 0.0, 0.0);
    let slow = Vec3::new(1.0e-5, 0.0, 0.0); // |v| < ARRIVAL_SPEED
    assert!(swept_fixed(prev_pos, pos, dest, slow, false));
}

#[test]
fn fast_flyby_does_not_fire() {
    // Same geometric clip, but rel_speed above the gate -> suppressed.
    let dest = Vec3::ZERO;
    let prev_pos = Vec3::new(-1.0e-3, 0.0, 0.0);
    let pos = Vec3::new(1.0e-3, 0.0, 0.0);
    let fast = Vec3::new(1.0, 0.0, 0.0); // |v| >> ARRIVAL_SPEED
    assert!(!swept_fixed(prev_pos, pos, dest, fast, false));
}

#[test]
fn velocity_matched_rendezvous_fires() {
    // rel_vel ~ 0 -> dd ~ 0 degenerate branch -> endpoint test fires (would NaN-out
    // without the dd<=DD_EPS guard, §5.3).
    let dest = Vec3::ZERO;
    let inside = Vec3::new(0.5e-4, 0.0, 0.0); // within R
    assert!(swept_fixed(inside, inside, dest, Vec3::new(1.0e-6, 0.0, 0.0), false));
}

#[test]
fn tick0_zero_length_chord_does_not_fire_outside() {
    // prev_pos == pos and outside R -> no spurious fire.
    let dest = Vec3::ZERO;
    let outside = Vec3::new(1.0e-2, 0.0, 0.0);
    assert!(!swept_fixed(outside, outside, dest, Vec3::ZERO, false));
}

#[test]
fn arrival_speed_gate_boundary_pins_comparison_direction() {
    // The gate is `rel_speed <= ARRIVAL_SPEED`. Pin the boundary so a future
    // flip to `<` / `>=` is caught. Geometry inside R, vary only rel_speed.
    let dest = Vec3::ZERO;
    let inside = Vec3::new(0.5e-4, 0.0, 0.0);
    // Just under the gate -> fires.
    assert!(swept_fixed(inside, inside, dest, Vec3::new(ARRIVAL_SPEED - 1e-9, 0.0, 0.0), false));
    // Strictly over the gate -> does not fire.
    assert!(!swept_fixed(inside, inside, dest, Vec3::new(ARRIVAL_SPEED + 1e-3, 0.0, 0.0), false));
}

#[test]
fn swept_latch_suppresses_repeat_when_prev_inside() {
    // Already delivered last tick (prev_inside = true) -> the once-only latch
    // suppresses a second fire even though geometry+speed would otherwise qualify.
    let dest = Vec3::ZERO;
    let inside = Vec3::new(0.5e-4, 0.0, 0.0);
    assert!(!swept_fixed(inside, inside, dest, Vec3::new(1.0e-6, 0.0, 0.0), true));
}
```

- [ ] **Step 5: Run to verify failure**

Run: `cargo test -p jumpgate-core --lib events`
Expected: FAIL — `arrival_swept` / `ARRIVAL_SPEED` not found.

- [ ] **Step 6: Implement `ARRIVAL_SPEED` + `arrival_swept`, rewire `detect_boundary_events`**

In `events.rs`, add the const (Class-1, D11) and the predicate, and delete `arrival_crossed`:

```rust
/// Relative-speed gate (canonical AU/day) distinguishing a velocity-matched
/// rendezvous (fires Arrival) from a fast flyby that merely grazes the sphere
/// (must NOT fire). Class-1 const (D11); affects ONLY Arrival event timing, never
/// state_hash. Starting value = the old V_CRUISE magnitude; pin by measurement (§10).
pub const ARRIVAL_SPEED: f64 = 2.0e-3;

/// Degeneracy epsilon for the swept chord (target-frame chord length^2 below this
/// is treated as a stationary rendezvous; §5.3).
const DD_EPS: f64 = 1.0e-30;

/// Swept arrival predicate (§5.2): closest approach of the craft↔target chord in
/// the TARGET frame, gated by rel_speed. `c_prev`/`c_now` are the target position
/// at tick T-1 / T (equal, for a fixed Position). All ops are Vec3 + scalar; NO FMA.
fn arrival_swept(
    prev_pos: Vec3,
    pos: Vec3,
    c_prev: Vec3,
    c_now: Vec3,
    rel_speed: f64,
    prev_inside: bool,
) -> bool {
    let a = prev_pos.sub(c_prev); // craft offset from target at chord start
    let b = pos.sub(c_now);       // craft offset from target at chord end
    let d = b.sub(a);
    let dd = d.dot(d);
    let r = ARRIVAL_RADIUS;
    let min_sq = if dd <= DD_EPS {
        b.dot(b) // degenerate / rendezvous: endpoint point-in-sphere (§5.3)
    } else {
        let t = ((-(a.dot(d))) / dd).max(0.0).min(1.0); // clamp closest-approach param to [0,1]
        let closest = a.add(d.scale(t));
        closest.dot(closest)
    };
    let inside_now = (min_sq <= r * r) && (rel_speed <= ARRIVAL_SPEED);
    inside_now && !prev_inside
}
```

In `detect_boundary_events`, the `Seeking` branch must resolve the target at **both** `Tick(T-1)` and `Tick(T)` and compute `rel_speed`, then call `arrival_swept`. `T >= 1` always (detection runs with `next`, so `Tick(T-1)` never underflows). Replace the dest-resolution + `arrival_crossed` block:

```rust
        if let NavState::Seeking { dest, .. } = ships.nav[idx] {
            let (c_prev, c_now, dest_vel) = match dest {
                NavDest::Position(p) => (p, p, Vec3::ZERO),
                NavDest::Entity(EntityRef::Body(body_id)) => {
                    let eidx = bodies.eph_index[body_id.slot as usize];
                    let prev_tick = Tick(tick.0 - 1);
                    (ephem.body_pos(eidx, prev_tick), ephem.body_pos(eidx, tick), ephem.body_vel(eidx, tick))
                }
                NavDest::Entity(EntityRef::Craft(_)) => continue,
            };
            let rel_speed = ships.vel[idx].sub(dest_vel).length();
            if arrival_swept(
                ships.prev_pos[idx], ships.pos[idx], c_prev, c_now, rel_speed,
                ships.prev_inside_dest[idx],
            ) {
                out.emit(Event { tick, kind: EventKind::Arrival { craft: id, dest } });
            }
        }
```

(`ARRIVAL_RADIUS` is already imported in `events.rs`; it is now the swept test's `R`, the relocated geometry consumer. `ephem.body_vel` exists at `ephemeris.rs` — confirm with `rg 'fn body_vel'`.)

- [ ] **Step 7: Repurpose the two in-module arrival tests**

`arrival_crossing_contract_documented` (uses a local `crossed` closure — keep, or rewrite to document the swept contract) and `arrival_crossed_uses_arrival_radius_constant` (calls the now-deleted `arrival_crossed` — **rewrite** to call `arrival_swept` with a zero-length chord at a point just inside `ARRIVAL_RADIUS`, low rel_speed, asserting it fires; and well outside, asserting it does not). Do not leave a reference to the deleted function.

- [ ] **Step 8: Run events tests**

Run: `cargo test -p jumpgate-core --lib events`
Expected: all swept tests PASS.

- [ ] **Step 9: Add the moving-body integration test + measure `ARRIVAL_SPEED`**

In `physics_sanity.rs`, add `moving_body_rendezvous_fires`: a craft co-moving with an orbiting `Body`, stepped to rendezvous, asserting an `Arrival` event for that craft appears in the stream (resolving `C_prev`/`C_now`/`dest_vel` via the ephemeris through the real `step` path). This runs on the μ-corrected ephemeris baseline (`bf97147`). Then run the flyby/rendezvous pair and **confirm `ARRIVAL_SPEED = 2e-3` cleanly separates** the real arrival (fires) from a fast flyby (does not). If a real rendezvous in `physics_sanity` arrives with rel_speed above `2e-3`, lower the const until the real-arrival tests fire and the flyby test still does not — then pin that value with a comment recording the measurement.

- [ ] **Step 10: Full suite + clippy; confirm no hash moved**

Run: `cargo test -p jumpgate-core` then `cargo clippy --all-targets`
Expected: all green, clean. **No** state/config golden moved (`prev_pos` unhashed; swept changes only event timing). If a test pinned the **event stream** for an arrival whose timing shifted, re-pin it (single-cause: swept detection); the per-tick state-hash chain does not move. No committed event-stream golden exists (§10.5), so this is forward-discipline only.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(core): swept arrival detection (segment-vs-moving-sphere) + ARRIVAL_SPEED gate + prev_pos column (anti-tunnel half a) (D7/D11)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: catalogue the deferred danger-set + close-out (D3 / D11 / D12 / §12)

Record the not-migrated tunables as tracked debt with their trip-conditions, reflect the already-resolved μ fix, retire stale prose, and resolve the folded-in observation. No engine behaviour change.

**Files:**
- Modify: `crates/jumpgate-core/src/autopilot.rs` (module doc — remove stale `V_CRUISE`/`K_BRAKE` const prose), `crates/jumpgate-core/src/world.rs` (fixture comments that say "absolute V_CRUISE" → "fractional cap")
- Filigree: catalogue issues + observation resolution

- [ ] **Step 1: Retire stale prose**

`rg -n 'V_CRUISE|K_BRAKE|V_ERR_EPS' crates/jumpgate-core/src/` — every remaining hit should be either the `GuidanceParams` field docs or a comment that now misdescribes the code. Update the autopilot module doc and the `world.rs` fixture comments (the `one_body_one_thrusting_craft` comment about the absolute cap) to describe the fractional per-ship cap. Run `cargo test -p jumpgate-core` to confirm doc-comment changes didn't break doc-tests.

- [ ] **Step 2: File catalogue debt as filigree issues (P4 backlog)**

Create one issue per deferred tunable, each recording class + live consumers + trip-condition + promotion cost (copy from spec §12):
- `ARRIVAL_RADIUS` — shared world geometry (Class-1); trip = first per-scenario arrival tolerance need; cost = `config_hash` fold + possible `prev_inside_dest` state fold (`HASH_FORMAT_VERSION` bump). Goes in a geometry/world config section, **not** `GuidanceParams`.
- Kepler iteration budget (Class-1) — trip = per-run solver fidelity; cost = Class-2 fold + full recorded-run rebaseline.
- `ARRIVAL_SPEED` (Class-1, new) — event-timing-only; promotion is event-stream-only, never a state golden re-derive.

Use `mcp__filigree__issue_create` (type `task`, priority 4, label e.g. `deferred-debt`/`taxonomy`).

- [ ] **Step 3: Resolve the folded-in observation**

`jumpgate-obs-4d28955902` (divergent `burn_budget` ingest defaults) is fixed by Task 5. Dismiss it (or promote-then-close) with a note citing the Task 5 commit, so it does not linger in the 14-day scratchpad.

- [ ] **Step 4: Final whole-suite verification**

Run: `cargo test -p jumpgate-core` and `cargo clippy --all-targets`
Expected: all green, zero warnings. Confirm the three goldens are at their intended values: `GOLDEN_CONFIG_HASH` re-pinned (Task 2), `GOLDEN_ZERO_STATE_HASH = 0xf0dd…` and `state_hash_golden_zero_world = 0x532d…` **unchanged**.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs(core): catalogue deferred taxonomy debt; retire stale V_CRUISE prose (D3/D11/D12)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-review (run by the plan author before handoff)

**1. Spec coverage** — every decision maps to a task:
- D1 (policy at autopilot) → Task 3. D2 (k_brake/v_err_eps carryover) → Task 3. D3 (`ARRIVAL_RADIUS` catalogued) → Task 7. D4/D5 (`GuidanceParams` + 0.25 default) → Task 2. D6 (reset guard + dt backstop) → Tasks 4 (guard) + 3 (backstop). D7 (swept) → Task 6. D8 (`tsiolkovsky_dv`) → Task 1. D9 (ingest reconciliation) → Task 5. D10 (config destructure) → **prelude, done**; Task 2 only appends. D11 (`ARRIVAL_SPEED`) → Task 6 + catalogue Task 7. D12 (μ correctness) → **done (`bf97147`)**; Task 7 catalogue note. D13 (config golden + `CONFIG_FIELD_ORDER`) → Task 2.

**2. Placeholder scan** — the only intentional blank is `GOLDEN_CONFIG_HASH = 0x____` in Task 2 Step 7, which is filled from the measured failure output (golden re-pin is inherently measure-then-pin, like the state golden); not a placeholder defect.

**3. Type consistency** — `tsiolkovsky_dv(exhaust_velocity, dry_mass, propellant_mass) -> f64` used identically in Tasks 1/3/5. `GuidanceParams { cruise_burn_fraction, k_brake, v_err_eps }` consistent across Tasks 2/3/4. `autopilot_command(.., eff, guidance, dt)` arg order consistent Task 3 ↔ world.rs call site ↔ test sites. `ResetError::Unbrakable { craft_index, a_max_empty, dt, limit }` consistent Task 4 def ↔ test. `arrival_swept(prev_pos, pos, c_prev, c_now, rel_speed, prev_inside)` consistent Task 6 def ↔ tests ↔ `detect_boundary_events`.

**Golden-discipline ledger (single-cause):** Task 2 re-pins **`GOLDEN_CONFIG_HASH` only**. Task 3 re-derives **cruise-axis trajectory/property tests only** (no state golden — Idle-nav goldens). Task 4: the guard itself is **determinism-neutral**, but Step 4b **recalibrates `one_body_one_craft` and `base_config`** (latent tunnels) — this moves *their* `config_hash` (folds `base_max_thrust`); re-pin any `config_hash` assertion those fixtures make. Their *state* trajectories are unchanged where the craft coasts (`max_thrust` unread on a coast); `base_config` record/replay bit-identity must be re-confirmed after retune. Neither is one of the three pinned goldens. Task 5 moves **state_hash for the live no-budget path only** (no golden, no recording). Task 6 moves **Arrival event timing only** (no hashed state). No `HASH_FORMAT_VERSION` bump anywhere in this plan. Tasks 1/7 are hash-neutral.

**Review provenance:** A 4-perspective plan-review (reality/architecture/quality/systems + synthesis) verified every symbol against source and found 3 blocking issues — all fixed above (Task 3 backstop `dt`, Task 4 fixture recalibration ×2, plus the corrected false green-checkpoint claims). The reviewers' down-classified non-issues (the `arrives_with_low_relative_speed` cruise-cap concern is moot — that test uses its own `dt=1e-4` and arrives in the brake regime; the chord-clip latch double-fire is spec-§13 deferred) are recorded as forward-debt comments, not blockers.
