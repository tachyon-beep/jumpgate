# Commons-Miner Analytic Cut — Implementation Plan

> **⚠ RETIRED FRAME (PDR-0006, 2026-06-10).** The DRL-room / presolvability-gate / fraction-of-ceiling / "prove a learner beats a script or the optimum" framing in this document is **RETIRED** — v1 is judged as a **GAME by emergent play** (GAME science: the science of what makes a good game), not by proving settled theory with a video game (game SCIENCE). Genuine engineering and history here stand; read any gate/room/thesis framing as dead doctrine. See `docs/product/decisions/0006-judge-v1-as-a-game-not-a-presolvability-gate.md`.


> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone deterministic Rust probe that measures whether the full-information commons-miner game has learnable DRL room (≥10% of ceiling, flat-or-rising in N), and reports a pre-registered GO/NO-GO verdict.

**Architecture:** A new workspace crate `crates/jumpgate-commons-cut`, integer-only and deterministic, depending on `jumpgate-core` only for `RngStreams`/`RngStream::Scenario`. An integer arena sim (regions deplete + regen; crowd-split yield) drives a four-rung policy ladder (constant → randomizing best-closed-form → per-seed-myopic → single-agent closed-loop selfish best-response computed by backward-induction DP). The DP calibrates a Monte-Carlo best-response estimator carried up an N-ladder; the verdict is the fraction-of-ceiling curve vs the 10% gate. Determinism is enforced by a pinned golden trajectory hash.

**Tech Stack:** Rust 2024, `jumpgate-core` (RNG only), `rand_chacha` (transitively via core's seeded streams), FNV-1a for the golden trajectory hash (match core's hashing idiom).

**Spec:** `docs/superpowers/specs/2026-06-10-commons-miner-cut-design.md`. Read it before starting; this plan implements it. Pre-registered gate constant `GAP_FRAC_MIN = 0.10`; grade by fraction-of-ceiling, never Cohen's d.

---

## File structure (locked before tasks)

- `crates/jumpgate-commons-cut/Cargo.toml` — new crate manifest.
- `src/lib.rs` — module wiring + core types (`Region`, `Ship`, `ArenaConfig`, `ArenaState`, `Action`, constants).
- `src/rng_bridge.rs` — seeded scenario setup from `RngStreams::from_master`.
- `src/dynamics.rs` — the integer tick (simultaneous update), yield law, depletion, regen.
- `src/policies.rs` — the `Policy` trait + constant / closed-form / myopic rungs + the rollout harness.
- `src/dp.rs` — state encoding + backward-induction closed-loop single-agent BR ceiling + planner upper bound.
- `src/mc.rs` — Monte-Carlo BR estimator + DP calibration + confidence intervals.
- `src/gate.rs` — fraction-of-ceiling, the N-scaling verdict, controls.
- `src/report.rs` — `Verdict` summary struct + `#[ignore]` diagnostic entry points (ladder, sweep, pack diagnostics).
- `tests/golden_trajectory.rs` — the **non-ignored** determinism golden.

Convention (spec §3/§7): **integer-only state and transitions; no `f64` in arena state or transition.** `f64` is permitted only in *measurement/reporting* (fractions, CIs) downstream of the integer sim.

---

### Task 1: Crate scaffold + core types

**Files:**
- Create: `crates/jumpgate-commons-cut/Cargo.toml`
- Create: `crates/jumpgate-commons-cut/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)

- [ ] **Step 1: Add the crate to the workspace + manifest**

`crates/jumpgate-commons-cut/Cargo.toml`:
```toml
[package]
name = "jumpgate-commons-cut"
version = "0.1.0"
edition = "2024"

[dependencies]
jumpgate-core = { path = "../jumpgate-core" }
```

In the root `Cargo.toml`, add the member (keep the list sorted):
```toml
members = [
    "crates/jumpgate-commons-cut",
    "crates/jumpgate-core",
    "crates/jumpgate-py",
]
```

- [ ] **Step 2: Write the failing test** (in `src/lib.rs`)

```rust
//! Commons-miner analytic cut — a standalone deterministic probe measuring
//! learnable DRL room in the full-information commons-miner game (spec
//! 2026-06-10-commons-miner-cut-design.md). NOT part of the hashed World.

/// Global stock discretization. Region stock and richness_cap are in `0..=STOCK_MAX`.
/// Start at 20; the gradient check (Task 4) raises it to 50 if depletion flattens.
pub const STOCK_MAX: u32 = 20;

/// A mining region. All integer (determinism).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub stock: u32,        // 0..=richness_cap
    pub richness_cap: u32, // 1..=STOCK_MAX
    pub regen_per_tick: u32,
}

/// A mining ship. `region == None` means in transit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ship {
    pub region: Option<u8>,
    pub dest: u8,
    pub travel_ticks_remaining: u8,
    pub total_yield: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_and_ship_are_plain_integer_value_types() {
        let r = Region { stock: 10, richness_cap: 20, regen_per_tick: 0 };
        let s = Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 };
        assert_eq!(r.stock, 10);
        assert_eq!(s.region, Some(0));
        // Copy semantics (value types, no heap in the hot loop).
        let _r2 = r;
        let _s2 = s;
        assert_eq!(r, _r2);
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut`
Expected: FAIL — crate/types not yet compiling until Step 2's code is in place. (If you wrote Step 2 directly, it should compile and pass; the "failing" state is the pre-creation state.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut`
Expected: PASS (`1 passed`).

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/jumpgate-commons-cut/Cargo.toml crates/jumpgate-commons-cut/src/lib.rs
git commit -m "feat(commons-cut): scaffold crate + Region/Ship integer types"
```

---

### Task 2: ArenaConfig / ArenaState + Action enum

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/lib.rs`

- [ ] **Step 1: Write the failing test** (append to `src/lib.rs` `tests`)

```rust
    #[test]
    fn arena_state_holds_regions_and_ships_and_tick() {
        let cfg = ArenaConfig {
            regions: vec![
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 10, richness_cap: 10, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 3], vec![3, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8, 1u8]);
        assert_eq!(st.tick, 0);
        assert_eq!(st.regions.len(), 2);
        assert_eq!(st.ships.len(), 2);
        assert_eq!(st.ships[0].region, Some(0));
        assert_eq!(st.ships[1].region, Some(1));
        assert!(matches!(Action::Stay, Action::Stay));
        assert!(matches!(Action::MoveTo(1), Action::MoveTo(1)));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut`
Expected: FAIL — `ArenaConfig`, `ArenaState`, `Action` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/lib.rs`, above `tests`)

```rust
/// Static arena definition (seeded at construction; never mutated during a run).
#[derive(Clone, Debug)]
pub struct ArenaConfig {
    pub regions: Vec<Region>,
    pub travel: Vec<Vec<u8>>, // travel[i][j] = ticks to move from region i to j; [i][i] = 0
    pub horizon: u32,
}

/// Mutable arena state advanced by `dynamics::step`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaState {
    pub regions: Vec<Region>,
    pub ships: Vec<Ship>,
    pub tick: u32,
}

impl ArenaState {
    /// Build the initial state: each ship starts mining its assigned region.
    pub fn from_config(cfg: &ArenaConfig, ship_start_regions: &[u8]) -> Self {
        let ships = ship_start_regions
            .iter()
            .map(|&r| Ship { region: Some(r), dest: r, travel_ticks_remaining: 0, total_yield: 0 })
            .collect();
        ArenaState { regions: cfg.regions.clone(), ships, tick: 0 }
    }
}

/// A ship's per-decision action. Integer, Copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Stay,
    MoveTo(u8),
}
```

Also add the module declarations at the top of `lib.rs` (after the doc comment): `pub mod rng_bridge; pub mod dynamics; pub mod policies; pub mod dp; pub mod mc; pub mod gate; pub mod report;` — **but only uncomment each as its task lands** (Rust will not compile a declared-but-absent module). For Task 2, declare none yet.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut`
Expected: PASS (`2 passed`).

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs
git commit -m "feat(commons-cut): ArenaConfig/ArenaState/Action types"
```

---

### Task 3: Deterministic seeded scenario setup (rng_bridge)

**Files:**
- Create: `crates/jumpgate-commons-cut/src/rng_bridge.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (add `pub mod rng_bridge;`)

**Context:** `jumpgate_core::rng::{RngStreams, RngStream}` are `pub` (verified: `RngStreams::from_master`, `RngStream::Scenario` at `crates/jumpgate-core/src/rng.rs:53/21`). Use the `Scenario` stream for all initial-condition randomness. `field_correlation` controls heterogeneity: `corr = 0` → independent diverse caps; `corr = 1000` (per-mille) → all caps equal (the negative control).

- [ ] **Step 1: Write the failing test** (in `src/rng_bridge.rs`)

```rust
use crate::{ArenaConfig, STOCK_MAX};
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::RngCore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_config_diff_seed_diff() {
        let a = build_scenario(42, 3, 3, 0, 0);
        let b = build_scenario(42, 3, 3, 0, 0);
        let c = build_scenario(43, 3, 3, 0, 0);
        assert_eq!(a.regions, b.regions, "same seed -> identical caps (determinism)");
        assert_ne!(a.regions, c.regions, "different seed -> different caps");
        assert_eq!(a.regions.len(), 3);
    }

    #[test]
    fn full_correlation_makes_all_caps_equal() {
        let cfg = build_scenario(42, 3, 4, 0, 1000); // corr = 1000 per-mille = identical
        let cap0 = cfg.regions[0].richness_cap;
        assert!(cfg.regions.iter().all(|r| r.richness_cap == cap0),
            "field_correlation=1000 -> identical regions (negative control)");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut rng_bridge`
Expected: FAIL — `build_scenario` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/rng_bridge.rs`, above `tests`)

```rust
/// Build a deterministic `ArenaConfig` from a master seed. Integer-only.
/// `field_correlation` is per-mille in `0..=1000`: 0 = independent diverse caps,
/// 1000 = all caps identical (the negative control). `regen` sets every region's
/// regen_per_tick (0 = one-shot exhaustion).
pub fn build_scenario(
    master: u64,
    n_regions: u8,
    travel_ticks: u8,
    regen: u32,
    field_correlation: u32,
) -> ArenaConfig {
    let mut streams = RngStreams::from_master(master);
    let rng = streams.stream(RngStream::Scenario);

    // One shared "base richness" draw; per-region caps blend toward it by correlation.
    let base = 1 + (rng.next_u32() % STOCK_MAX); // 1..=STOCK_MAX
    let corr = field_correlation.min(1000);
    let regions = (0..n_regions)
        .map(|_| {
            let indep = 1 + (rng.next_u32() % STOCK_MAX); // 1..=STOCK_MAX
            // Integer blend: cap = (corr*base + (1000-corr)*indep) / 1000, clamped to >=1.
            let cap = ((corr as u64 * base as u64 + (1000 - corr) as u64 * indep as u64) / 1000)
                .max(1) as u32;
            Region { stock: cap, richness_cap: cap, regen_per_tick: regen } // start full
        })
        .collect::<Vec<_>>();

    // Fully-connected uniform travel matrix; [i][i] = 0.
    let travel = (0..n_regions as usize)
        .map(|i| (0..n_regions as usize).map(|j| if i == j { 0 } else { travel_ticks }).collect())
        .collect();

    ArenaConfig { regions, travel, horizon: 30 }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut rng_bridge`
Expected: PASS (`2 passed`). Add `pub mod rng_bridge;` to `lib.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/rng_bridge.rs
git commit -m "feat(commons-cut): deterministic seeded scenario setup (field_correlation axis)"
```

---

### Task 4: The integer tick + yield law + GRADIENT CHECK

**Files:**
- Create: `crates/jumpgate-commons-cut/src/dynamics.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod dynamics;`)

**Spec §3 yield law** (`occupants ≥ 1`): `per_ship_yield = (stock * richness_cap) / (STOCK_MAX * occupants)`, floored; total extraction = `per_ship_yield * occupants`; stock decremented by that (saturating); then regen up to `richness_cap`. **Simultaneous update**: read actions/positions from tick-start state, apply, then compute yields/decrement — never mutate stock mid-tick.

- [ ] **Step 1: Write the failing test** (in `src/dynamics.rs`)

```rust
use crate::{Action, ArenaState, Region, Ship, STOCK_MAX};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArenaConfig;

    fn one_region_state(stock: u32, cap: u32, occupants: usize) -> ArenaState {
        ArenaState {
            regions: vec![Region { stock, richness_cap: cap, regen_per_tick: 0 }],
            ships: (0..occupants)
                .map(|_| Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 })
                .collect(),
            tick: 0,
        }
    }

    #[test]
    fn yield_is_stock_times_cap_over_stockmax_times_occupants_floored() {
        // STOCK_MAX=20, full stock=20, cap=20, 1 occupant -> 20*20/(20*1)=20.
        let mut st = one_region_state(20, 20, 1);
        let actions = vec![Action::Stay];
        step(&mut st, &actions, &ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 });
        assert_eq!(st.ships[0].total_yield, 20);
        assert_eq!(st.regions[0].stock, 0, "20 mined out of 20 -> exhausted");
    }

    #[test]
    fn crowd_split_dilutes_per_ship_yield() {
        // full=20, cap=20, 2 occupants -> per_ship = 20*20/(20*2)=10 each; total extract 20.
        let mut st = one_region_state(20, 20, 2);
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 };
        step(&mut st, &[Action::Stay, Action::Stay], &cfg);
        assert_eq!(st.ships[0].total_yield, 10);
        assert_eq!(st.ships[1].total_yield, 10);
        assert_eq!(st.regions[0].stock, 0, "2x10 extracted from 20");
    }

    #[test]
    fn depletion_to_zero_then_zero_yield() {
        let mut st = one_region_state(2, 20, 1); // low stock -> 2*20/20 = 2
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 };
        step(&mut st, &[Action::Stay], &cfg);
        assert_eq!(st.ships[0].total_yield, 2);
        assert_eq!(st.regions[0].stock, 0);
        step(&mut st, &[Action::Stay], &cfg); // empty region -> 0
        assert_eq!(st.ships[0].total_yield, 2, "no further yield from empty region");
    }

    #[test]
    fn move_costs_transit_ticks_of_zero_yield() {
        let mut st = ArenaState {
            regions: vec![
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            ships: vec![Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 }],
            tick: 0,
        };
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0, 2], vec![2, 0]], horizon: 30 };
        step(&mut st, &[Action::MoveTo(1)], &cfg); // depart: travel=2, this tick in transit
        assert_eq!(st.ships[0].region, None, "in transit");
        assert_eq!(st.ships[0].total_yield, 0, "no mining while moving");
        step(&mut st, &[Action::Stay], &cfg); // 1 tick left
        assert_eq!(st.ships[0].region, None);
        step(&mut st, &[Action::Stay], &cfg); // arrived, mines region 1
        assert_eq!(st.ships[0].region, Some(1));
        assert_eq!(st.ships[0].total_yield, 20);
    }

    /// SPEC §3 GRADIENT CHECK — gate, not a runtime assertion. Yield must take >=4
    /// distinct values as a single region drains, else the depletion gradient is too
    /// flat for a live abandon-decision (raise STOCK_MAX). With STOCK_MAX=20, cap=20,
    /// 1 occupant, yields are 20,19,...  -> plenty of distinct values.
    #[test]
    fn depletion_gradient_has_enough_distinct_yield_values() {
        let mut st = one_region_state(STOCK_MAX, STOCK_MAX, 1);
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 100 };
        let mut seen = std::collections::BTreeSet::new();
        let mut prev = 0u64;
        for _ in 0..STOCK_MAX {
            step(&mut st, &[Action::Stay], &cfg);
            seen.insert(st.ships[0].total_yield - prev);
            prev = st.ships[0].total_yield;
            if st.regions[0].stock == 0 { break; }
        }
        assert!(seen.len() >= 4, "depletion gradient too flat ({} distinct yields); raise STOCK_MAX", seen.len());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut dynamics`
Expected: FAIL — `step` not defined. (If the gradient check fails with <4 distinct values, raise `STOCK_MAX` to 50 in `lib.rs` and re-run.)

- [ ] **Step 3: Write minimal implementation** (in `src/dynamics.rs`, above `tests`)

```rust
use crate::ArenaConfig;

/// Advance the arena one tick. SIMULTANEOUS update: actions are read against the
/// tick-start state, applied, then yields computed and stock decremented, then regen.
/// Ships iterate in index order (determinism). No RNG, no float.
pub fn step(state: &mut ArenaState, actions: &[Action], cfg: &ArenaConfig) {
    debug_assert_eq!(actions.len(), state.ships.len());

    // --- Phase 1: apply movement decisions (read tick-start positions) ---
    for (i, ship) in state.ships.iter_mut().enumerate() {
        match (ship.region, actions[i]) {
            (Some(here), Action::MoveTo(dest)) if dest != here => {
                let cost = cfg.travel[here as usize][dest as usize];
                ship.region = None;
                ship.dest = dest;
                ship.travel_ticks_remaining = cost.saturating_sub(1); // this tick is the first transit tick
                if ship.travel_ticks_remaining == 0 {
                    ship.region = Some(dest); // adjacent / zero-cost arrival
                }
            }
            (None, _) => {
                // In transit: count down; arrive when it hits zero.
                if ship.travel_ticks_remaining == 0 {
                    ship.region = Some(ship.dest);
                } else {
                    ship.travel_ticks_remaining -= 1;
                    if ship.travel_ticks_remaining == 0 {
                        ship.region = Some(ship.dest);
                    }
                }
            }
            _ => {} // Stay, or MoveTo current region: no change
        }
    }

    // --- Phase 2: count occupants per region (post-movement) ---
    let n_regions = state.regions.len();
    let mut occupants = vec![0u64; n_regions];
    for ship in &state.ships {
        if let Some(r) = ship.region {
            occupants[r as usize] += 1;
        }
    }

    // --- Phase 3: compute per-ship yields + total extraction per region ---
    let mut extraction = vec![0u64; n_regions];
    for ship in state.ships.iter_mut() {
        if let Some(r) = ship.region {
            let region = &state.regions[r as usize];
            let occ = occupants[r as usize].max(1);
            let per_ship = (region.stock as u64 * region.richness_cap as u64)
                / (STOCK_MAX as u64 * occ);
            ship.total_yield += per_ship;
            extraction[r as usize] += per_ship; // summed over occupants below via repeated add
        }
    }

    // --- Phase 4: decrement stock by total extraction, then regen ---
    for (r, region) in state.regions.iter_mut().enumerate() {
        let new_stock = (region.stock as u64).saturating_sub(extraction[r]) as u32;
        region.stock = (new_stock + region.regen_per_tick).min(region.richness_cap);
    }

    state.tick += 1;
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut dynamics`
Expected: PASS (`5 passed`). Add `pub mod dynamics;` to `lib.rs`. If the gradient check tripped, STOCK_MAX is now 50 and all yield numbers above scale — re-derive the expected constants in the tests before asserting.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/dynamics.rs
git commit -m "feat(commons-cut): integer tick — crowd-split yield, depletion, regen, transit (+gradient gate)"
```

---

### Task 5: Golden trajectory-hash determinism test (NON-IGNORED)

**Files:**
- Create: `crates/jumpgate-commons-cut/tests/golden_trajectory.rs`

**Spec §7:** the crate is outside core's RNG-lint perimeter, so the no-float/no-entropy discipline is enforced by a pinned golden hash of the `(tick, per-ship total_yield, per-region stock)` sequence for a fixed `(seed, policy, N, M, H)`. Use FNV-1a (match core's idiom).

- [ ] **Step 1: Write the failing test** (in `tests/golden_trajectory.rs`)

```rust
use jumpgate_commons_cut::{dynamics::step, rng_bridge::build_scenario, Action, ArenaState};

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic "always stay" trajectory hash. Pins the integer dynamics + scenario
/// seeding so any accidental float/entropy/order change is caught loudly (spec §7).
#[test]
fn golden_trajectory_is_pinned() {
    let cfg = build_scenario(12345, 3, 3, 0, 0);
    let mut st = ArenaState::from_config(&cfg, &[0u8, 1u8, 2u8]);
    let actions = vec![Action::Stay; st.ships.len()];
    let mut buf = Vec::new();
    for _ in 0..cfg.horizon {
        step(&mut st, &actions, &cfg);
        buf.extend_from_slice(&st.tick.to_le_bytes());
        for s in &st.ships { buf.extend_from_slice(&s.total_yield.to_le_bytes()); }
        for r in &st.regions { buf.extend_from_slice(&r.stock.to_le_bytes()); }
    }
    let h = fnv1a(&buf);
    // GOLDEN: re-pin ONCE on first green via `assert_eq!(h, h)` -> read the printed value.
    assert_eq!(h, 0x0000_0000_0000_0000u64,
        "trajectory hash drifted to {h:#018x} — a determinism break OR a deliberate, reviewed re-pin");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut --test golden_trajectory`
Expected: FAIL — the assert prints the real hash `0x....`.

- [ ] **Step 3: Pin the golden**

Replace the `0x0000_..._0000` literal with the printed value. (This is the one-time golden capture; thereafter the test guards determinism.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut --test golden_trajectory`
Expected: PASS. Re-run twice to confirm stability across processes.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/tests/golden_trajectory.rs
git commit -m "test(commons-cut): pinned golden trajectory hash — determinism control"
```

---

### Task 6: Policy trait + Constant rung + rollout harness

**Files:**
- Create: `crates/jumpgate-commons-cut/src/policies.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod policies;`)

**Spec §4:** rung 1 = constant ("mine your start region until empty, never move"). The rollout harness runs a homogeneous population under a policy and returns per-ship totals. Policies see only **current observables** (current stocks + current crowd), never future/oracle info — enforce by the `Observation` the trait receives. **Pinned tie-break: lowest region index** (spec §7).

- [ ] **Step 1: Write the failing test** (in `src/policies.rs`)

```rust
use crate::{Action, ArenaConfig, ArenaState};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rng_bridge::build_scenario};

    #[test]
    fn constant_policy_never_moves() {
        let cfg = build_scenario(7, 3, 3, 0, 0);
        let starts = vec![0u8, 1u8, 2u8];
        let p = Constant;
        let totals = rollout(&cfg, &starts, &p);
        assert_eq!(totals.len(), 3);
        // Constant never emits MoveTo, so every ship stays on its start region the whole run.
        let mut st = ArenaState::from_config(&cfg, &starts);
        for _ in 0..cfg.horizon {
            let acts = decide_all(&p, &st);
            assert!(acts.iter().all(|a| matches!(a, Action::Stay)), "constant only Stays");
            crate::dynamics::step(&mut st, &acts, &cfg);
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: FAIL — `Constant`, `rollout`, `decide_all`, `Policy` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/policies.rs`, above `tests`)

```rust
/// What a policy may observe at decision time: current state only (spec §4 ablation).
pub struct Observation<'a> {
    pub state: &'a ArenaState,
    pub ship_idx: usize,
}

/// A decision rule. Deterministic given (observation, rng if any) — but the ladder's
/// rungs 1/3/ceiling are deterministic; the closed-form (Task 7) carries its own
/// seeded coin for randomization.
pub trait Policy {
    fn decide(&self, obs: &Observation) -> Action;
}

/// Rung 1: never move.
pub struct Constant;
impl Policy for Constant {
    fn decide(&self, _obs: &Observation) -> Action { Action::Stay }
}

/// Decide one action per ship against the SAME tick-start state (simultaneous).
pub fn decide_all<P: Policy>(p: &P, st: &ArenaState) -> Vec<Action> {
    (0..st.ships.len())
        .map(|i| p.decide(&Observation { state: st, ship_idx: i }))
        .collect()
}

/// Run a homogeneous population under one policy for `horizon` ticks; return per-ship totals.
pub fn rollout<P: Policy>(cfg: &ArenaConfig, ship_starts: &[u8], p: &P) -> Vec<u64> {
    let mut st = ArenaState::from_config(cfg, ship_starts);
    for _ in 0..cfg.horizon {
        let acts = decide_all(p, &st);
        crate::dynamics::step(&mut st, &acts, cfg);
    }
    st.ships.iter().map(|s| s.total_yield).collect()
}

/// Per-region occupant counts of the CURRENT state (a shared observable helper).
pub fn occupant_counts(st: &ArenaState) -> Vec<u32> {
    let mut occ = vec![0u32; st.regions.len()];
    for s in &st.ships {
        if let Some(r) = s.region { occ[r as usize] += 1; }
    }
    occ
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: PASS. Add `pub mod policies;` to `lib.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/policies.rs
git commit -m "feat(commons-cut): Policy trait + Constant rung + rollout harness"
```

---

### Task 7: Best-closed-form reactive rung (randomizing) + train/eval fit

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/policies.rs`

**Spec §4 rung 2 (THE BAR):** "abandon when realized per-ship yield drops below τ; move to the highest observed stock-per-occupant region." **Must randomize** (move-with-probability `p`) — a deterministic threshold self-herds (mixed equilibrium, spec §4). Uses a seeded coin (the `Scenario` stream, separate draw, so it stays deterministic + replayable). Fit `(τ, p)` on **train seeds**, report on **disjoint eval seeds**.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn closed_form_abandons_below_threshold_and_targets_best_region() {
        // A ship on a near-empty region with a rich alternative should (sometimes) move.
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 1, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
                crate::Region { stock: crate::STOCK_MAX, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8]);
        // tau high (always-want-to-move), p=1000 (always move when triggered) -> deterministic move to region 1.
        let pol = ClosedForm { tau: crate::STOCK_MAX as u64, move_prob_milli: 1000, seed: 1 };
        let a = pol.decide(&Observation { state: &st, ship_idx: 0 });
        assert_eq!(a, Action::MoveTo(1), "abandons poor region for the richest");
    }

    #[test]
    fn closed_form_fit_returns_best_on_train_and_is_reused_on_eval() {
        let train: Vec<u64> = (100..104).collect();
        let eval: Vec<u64> = (200..204).collect();
        let fitted = fit_closed_form(3, 3, 0, 0, &train);
        // Determinism: fitting twice on the same train seeds gives the same params.
        let again = fit_closed_form(3, 3, 0, 0, &train);
        assert_eq!((fitted.tau, fitted.move_prob_milli), (again.tau, again.move_prob_milli));
        // The fitted policy is then evaluated on disjoint eval seeds (smoke: it runs).
        let cfg = build_scenario(eval[0], 3, 3, 0, 0);
        let totals = rollout(&cfg, &[0, 1, 2], &fitted);
        assert_eq!(totals.len(), 3);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: FAIL — `ClosedForm`, `fit_closed_form` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::RngCore;

/// Rung 2: randomizing anti-herding reactive rule. Abandon when the region's
/// stock-per-occupant projected yield is below `tau`; move (with prob move_prob_milli
/// per-mille) to the region with the highest current stock-per-occupant. Seeded coin
/// keyed by (seed, tick, ship_idx) -> deterministic + replayable.
pub struct ClosedForm {
    pub tau: u64,
    pub move_prob_milli: u32,
    pub seed: u64,
}

impl ClosedForm {
    fn stock_per_occ(st: &ArenaState, r: usize, occ: &[u32]) -> u64 {
        let region = &st.regions[r];
        let o = occ[r].max(1) as u64;
        (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o)
    }
}

impl Policy for ClosedForm {
    fn decide(&self, obs: &Observation) -> Action {
        let st = obs.state;
        let i = obs.ship_idx;
        let Some(here) = st.ships[i].region else { return Action::Stay }; // in transit: ride it out
        let occ = occupant_counts(st);
        let here_yield = Self::stock_per_occ(st, here as usize, &occ);
        if here_yield >= self.tau {
            return Action::Stay; // still good enough
        }
        // Find the best alternative by current stock-per-occupant (counting self as +1 there).
        let mut best = here as usize;
        let mut best_val = here_yield;
        for r in 0..st.regions.len() {
            if r == here as usize { continue; }
            let region = &st.regions[r];
            let o = (occ[r] + 1).max(1) as u64; // if I joined
            let v = (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o);
            // Pinned tie-break: strict > keeps the lowest index on ties (spec §7).
            if v > best_val {
                best_val = v;
                best = r;
            }
        }
        if best == here as usize { return Action::Stay; }
        // Seeded coin: hash (seed, tick, ship) into the Scenario stream space.
        let mut streams = RngStreams::from_master(self.seed ^ ((st.tick as u64) << 8) ^ (i as u64));
        let roll = streams.stream(RngStream::Scenario).next_u32() % 1000;
        if roll < self.move_prob_milli { Action::MoveTo(best as u8) } else { Action::Stay }
    }
}

/// Fit (tau, move_prob_milli) by grid search on TRAIN seeds (mean total yield).
/// Deterministic. Returns the best `ClosedForm` (seed pinned to 0xC0FFEE for eval coins).
pub fn fit_closed_form(
    n_regions: u8, travel: u8, regen: u32, field_corr: u32, train_seeds: &[u64],
) -> ClosedForm {
    let taus = [1u64, crate::STOCK_MAX as u64 / 4, crate::STOCK_MAX as u64 / 2, crate::STOCK_MAX as u64];
    let probs = [200u32, 500, 800, 1000];
    let mut best = ClosedForm { tau: taus[0], move_prob_milli: probs[0], seed: 0xC0FFEE };
    let mut best_mean = 0u64;
    for &tau in &taus {
        for &mp in &probs {
            let pol = ClosedForm { tau, move_prob_milli: mp, seed: 0xC0FFEE };
            let mut sum = 0u64;
            for &s in train_seeds {
                let cfg = crate::rng_bridge::build_scenario(s, n_regions, travel, regen, field_corr);
                let starts: Vec<u8> = (0..n_regions).collect();
                sum += rollout(&cfg, &starts, &pol).iter().sum::<u64>();
            }
            let mean = sum / train_seeds.len().max(1) as u64;
            if mean > best_mean {
                best_mean = mean;
                best = ClosedForm { tau, move_prob_milli: mp, seed: 0xC0FFEE };
            }
        }
    }
    best
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/policies.rs
git commit -m "feat(commons-cut): randomizing best-closed-form rung + train/eval grid fit"
```

---

### Task 8: Per-seed-myopic rung + ladder monotonicity sanity

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/policies.rs`

**Spec §4 rung 3:** greedy one-step-optimal on current observed state (others held at current position, no anticipation). Sanity: on a fixture, `constant ≤ closed-form ≤ myopic` (a violation = a rung bug, spec §12).

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn myopic_picks_the_one_step_best_action() {
        // One ship; region 0 nearly empty, region 1 full + adjacent (travel 0 -> arrives same... use travel 1).
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 1, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
                crate::Region { stock: crate::STOCK_MAX, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8]);
        // Staying yields ~1 this tick; moving yields 0 this tick (transit) but the myopic
        // 1-step horizon values only THIS tick -> myopic stays (greedy is myopic by design).
        let a = Myopic.decide(&Observation { state: &st, ship_idx: 0 });
        assert_eq!(a, Action::Stay, "1-step greedy values only the immediate tick");
    }

    #[test]
    fn ladder_is_monotone_on_a_fixture() {
        let seeds: Vec<u64> = (300..304).collect();
        let mean = |totals: Vec<u64>| totals.iter().sum::<u64>();
        let cf = fit_closed_form(3, 3, 0, 0, &seeds);
        let cfg = build_scenario(999, 3, 3, 0, 0);
        let c = mean(rollout(&cfg, &[0, 1, 2], &Constant));
        let f = mean(rollout(&cfg, &[0, 1, 2], &cf));
        let m = mean(rollout(&cfg, &[0, 1, 2], &Myopic));
        assert!(c <= f, "constant {c} <= closed-form {f}");
        assert!(f <= m, "closed-form {f} <= myopic {m}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: FAIL — `Myopic` not defined. (If `ladder_is_monotone` fails, that is a real signal — investigate whether the closed-form fit or myopic is buggy before forcing it green.)

- [ ] **Step 3: Write minimal implementation**

```rust
/// Rung 3: greedy one-step optimal. Evaluate each action's immediate-tick yield
/// against the current state (others frozen), pick the max. Pinned tie-break: Stay
/// wins ties, then lowest MoveTo index.
pub struct Myopic;
impl Policy for Myopic {
    fn decide(&self, obs: &Observation) -> Action {
        let st = obs.state;
        let i = obs.ship_idx;
        let Some(here) = st.ships[i].region else { return Action::Stay };
        let occ = occupant_counts(st);
        // Staying: my share of `here` this tick.
        let stay_val = {
            let region = &st.regions[here as usize];
            let o = occ[here as usize].max(1) as u64;
            (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o)
        };
        // Moving: 0 this tick (in transit). So 1-step greedy only ever beats Stay if Stay==0.
        if stay_val > 0 {
            return Action::Stay;
        }
        // here is empty: move to the region with the best immediate post-arrival share
        // (approximated as current stock-per-(occ+1); the move still costs transit, but a
        // 1-step myopic that's stuck at 0 prefers heading somewhere with future value).
        let mut best = here as usize;
        let mut best_val = 0u64;
        for r in 0..st.regions.len() {
            if r == here as usize { continue; }
            let region = &st.regions[r];
            let o = (occ[r] + 1) as u64;
            let v = (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o);
            if v > best_val {
                best_val = v;
                best = r;
            }
        }
        if best == here as usize { Action::Stay } else { Action::MoveTo(best as u8) }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut policies`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/policies.rs
git commit -m "feat(commons-cut): per-seed-myopic rung + ladder monotonicity sanity"
```

---

### Task 9: State encoding + exact backward-induction value (single-region-stock DP core)

**Files:**
- Create: `crates/jumpgate-commons-cut/src/dp.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod dp;`)

**Spec §4.1 + §6:** the exact ceiling carries the full joint state (all N ship positions + all M region stocks) over the horizon. This task builds the **state encode/decode + a memoized backward-induction value function** for ONE omniscient ship best-responding while the other N−1 follow a fixed reactive policy; the others' re-crowding (closed-loop) is Task 10. Keep N=3, M∈{2,3}, STOCK_MAX small enough that the state space fits (~54M at N=3/M=3 per spec; a `HashMap<u64, u64>` memo or a dense `Vec` keyed by encoded state).

- [ ] **Step 1: Write the failing test** (in `src/dp.rs`)

```rust
use crate::{ArenaConfig, ArenaState};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Region;

    #[test]
    fn encode_decode_roundtrips() {
        let cfg = ArenaConfig {
            regions: vec![
                Region { stock: 5, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 12, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 2], vec![2, 0]],
            horizon: 10,
        };
        let st = ArenaState::from_config(&cfg, &[0u8, 1u8]);
        let code = encode(&st, &cfg);
        let st2 = decode(code, &cfg, &[0u8, 1u8]);
        assert_eq!(st.regions.iter().map(|r| r.stock).collect::<Vec<_>>(),
                   st2.regions.iter().map(|r| r.stock).collect::<Vec<_>>());
        assert_eq!(st.ships.iter().map(|s| s.region).collect::<Vec<_>>(),
                   st2.ships.iter().map(|s| s.region).collect::<Vec<_>>());
    }

    #[test]
    fn single_ship_dp_value_matches_hand_rollout_on_tiny_instance() {
        // 1 ship, 1 region, full stock 20, cap 20, horizon 3, no regen, no moves possible.
        // Optimal = greedy stay: 20 + 0 + 0 = 20 (mined out tick 1).
        let cfg = ArenaConfig {
            regions: vec![Region { stock: 20, richness_cap: 20, regen_per_tick: 0 }],
            travel: vec![vec![0]],
            horizon: 3,
        };
        let v = best_response_value_open_loop(&cfg, &[0u8], 0);
        assert_eq!(v, 20);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: FAIL — `encode`/`decode`/`best_response_value_open_loop` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/dp.rs`)

```rust
use crate::dynamics::step;
use crate::{Action, Ship};
use std::collections::HashMap;

/// Encode the dynamic state (region stocks + ship positions) into a u64 key.
/// Layout: [stock_0..stock_{M-1}] each in 0..=STOCK_MAX (needs ceil(log2(STOCK_MAX+1)) bits),
/// then [pos_0..pos_{N-1}] each in 0..=M (M = "in transit"/None sentinel). Travel state
/// for transiting ships is folded by also encoding travel_ticks_remaining per ship.
pub fn encode(st: &ArenaState, cfg: &ArenaConfig) -> u64 {
    let m = cfg.regions.len() as u64;
    let stock_bits = 64 - (crate::STOCK_MAX as u64).leading_zeros() as u64; // bits per stock
    let mut key = 0u64;
    let mut shift = 0u64;
    for r in &st.regions {
        key |= (r.stock as u64) << shift;
        shift += stock_bits;
    }
    // pos in 0..=M (M = transit); plus travel_ticks_remaining (small).
    let pos_bits = 64 - (m + 1).leading_zeros() as u64;
    for s in &st.ships {
        let pos = s.region.map(|r| r as u64).unwrap_or(m);
        key |= pos << shift; shift += pos_bits;
        key |= (s.dest as u64) << shift; shift += pos_bits;
        key |= (s.travel_ticks_remaining as u64) << shift; shift += 4; // travel <= 5 fits in 4 bits
    }
    debug_assert!(shift <= 64, "state does not fit in u64 ({shift} bits) — shrink N/M/STOCK_MAX");
    key
}

/// Inverse of `encode` (positions/stocks; total_yield is reset to 0 — it is the DP's reward, not state).
pub fn decode(key: u64, cfg: &ArenaConfig, _starts: &[u8]) -> ArenaState {
    let m = cfg.regions.len() as u64;
    let stock_bits = 64 - (crate::STOCK_MAX as u64).leading_zeros() as u64;
    let mut shift = 0u64;
    let mut regions = cfg.regions.clone();
    for r in regions.iter_mut() {
        let mask = (1u64 << stock_bits) - 1;
        r.stock = ((key >> shift) & mask) as u32;
        shift += stock_bits;
    }
    let pos_bits = 64 - (m + 1).leading_zeros() as u64;
    let pos_mask = (1u64 << pos_bits) - 1;
    let n_ships = cfg.travel.len(); // placeholder; real N passed by caller in practice
    let mut ships = Vec::new();
    for _ in 0..n_ships {
        let pos = (key >> shift) & pos_mask; shift += pos_bits;
        let dest = ((key >> shift) & pos_mask) as u8; shift += pos_bits;
        let ttr = ((key >> shift) & 0xF) as u8; shift += 4;
        ships.push(Ship {
            region: if pos == m { None } else { Some(pos as u8) },
            dest, travel_ticks_remaining: ttr, total_yield: 0,
        });
    }
    ArenaState { regions, ships, tick: 0 }
}

/// Exact OPEN-LOOP best-response value for ship `me`: `me` chooses actions to maximize
/// its own total_yield over the horizon; the other ships are FROZEN at Stay. Backward
/// induction with memoization. (Closed-loop re-crowding is Task 10.)
pub fn best_response_value_open_loop(cfg: &ArenaConfig, starts: &[u8], me: usize) -> u64 {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    br_value(&st0, cfg, me, &mut memo)
}

fn br_value(st: &ArenaState, cfg: &ArenaConfig, me: usize, memo: &mut HashMap<(u64, u32), u64>) -> u64 {
    if st.tick >= cfg.horizon { return 0; }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) { return v; }
    // Candidate actions for `me`: Stay, or MoveTo each other region.
    let mut candidates = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() {
            if r != here as usize { candidates.push(Action::MoveTo(r as u8)); }
        }
    }
    let mut best = 0u64;
    for a in candidates {
        let mut next = st.clone();
        let mut acts = vec![Action::Stay; st.ships.len()]; // others frozen
        acts[me] = a;
        let before = next.ships[me].total_yield;
        step(&mut next, &acts, cfg);
        let reward = next.ships[me].total_yield - before;
        let v = reward + br_value(&next, cfg, me, memo);
        if v > best { best = v; }
    }
    memo.insert(key, best);
    best
}
```

> **Note for the implementer:** `decode`'s `n_ships` is derived from `cfg.travel.len()` as a stand-in; thread the real `N` through if `M != N` (they differ in general). The DP only ever round-trips states it `encode`d, so `decode` is used for debugging/tests, not the hot path — `br_value` clones live `ArenaState`s directly.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: PASS. Add `pub mod dp;` to `lib.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/dp.rs
git commit -m "feat(commons-cut): state encode/decode + open-loop best-response DP value"
```

---

### Task 10: Closed-loop best-response (reactive others re-crowd) + phantom-ceiling cross-check

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/dp.rs`

**Spec §4.1 (mandatory):** the N−1 others must **re-crowd in response** to the deviator (they run the fixed reactive `ClosedForm` rule against the live state, not frozen). The **phantom-ceiling cross-check**: roll the computed BR policy forward through a fresh sim with the reactive field and assert realized total ≈ computed `V₀`.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn closed_loop_br_value_equals_realized_rollout_phantom_check() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 2], vec![2, 0]],
            horizon: 8,
        };
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let (v0, realized) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        assert_eq!(v0, realized, "phantom-ceiling: computed V0 must equal realized rollout");
    }

    #[test]
    fn closed_loop_le_open_loop_recrowding_reduces_inherited_residual() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 8,
        };
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let open = best_response_value_open_loop(&cfg, &[0u8, 1u8], 0);
        let (closed, _) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        assert!(closed <= open, "closed-loop {closed} <= open-loop {open} (reacting field contests residuals)");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: FAIL — `best_response_value_closed_loop_checked` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::policies::{decide_all_except, ClosedForm, Observation, Policy};

/// Closed-loop BR: `me` best-responds while the OTHERS run `others` (reactive) against
/// the live state each tick. Returns (computed V0, realized rollout total) — they MUST
/// be equal (phantom-ceiling cross-check, spec §4.1).
pub fn best_response_value_closed_loop_checked(
    cfg: &ArenaConfig, starts: &[u8], me: usize, others: &ClosedForm,
) -> (u64, u64) {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    let v0 = br_value_cl(&st0, cfg, me, others, &mut memo);
    // Realized rollout: replay me's greedy-by-value choice forward.
    let realized = realize_cl(&st0, cfg, me, others, &mut memo);
    (v0, realized)
}

fn others_actions(st: &ArenaState, cfg: &ArenaConfig, me: usize, others: &ClosedForm) -> Vec<Action> {
    let _ = cfg;
    (0..st.ships.len())
        .map(|i| if i == me { Action::Stay } else { others.decide(&Observation { state: st, ship_idx: i }) })
        .collect()
}

fn br_value_cl(
    st: &ArenaState, cfg: &ArenaConfig, me: usize, others: &ClosedForm,
    memo: &mut HashMap<(u64, u32), u64>,
) -> u64 {
    if st.tick >= cfg.horizon { return 0; }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) { return v; }
    let mut candidates = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() {
            if r != here as usize { candidates.push(Action::MoveTo(r as u8)); }
        }
    }
    let mut best = 0u64;
    for a in candidates {
        let mut next = st.clone();
        let mut acts = others_actions(st, cfg, me, others);
        acts[me] = a;
        let before = next.ships[me].total_yield;
        step(&mut next, &acts, cfg);
        let reward = next.ships[me].total_yield - before;
        let v = reward + br_value_cl(&next, cfg, me, others, memo);
        if v > best { best = v; }
    }
    memo.insert(key, best);
    best
}

fn realize_cl(
    st0: &ArenaState, cfg: &ArenaConfig, me: usize, others: &ClosedForm,
    memo: &mut HashMap<(u64, u32), u64>,
) -> u64 {
    let mut st = st0.clone();
    while st.tick < cfg.horizon {
        // pick me's action that achieves the memoized best value
        let mut candidates = vec![Action::Stay];
        if let Some(here) = st.ships[me].region {
            for r in 0..st.regions.len() {
                if r != here as usize { candidates.push(Action::MoveTo(r as u8)); }
            }
        }
        let mut chosen = Action::Stay;
        let mut best = u64::MAX; // sentinel
        let target = br_value_cl(&st, cfg, me, others, memo);
        for a in candidates {
            let mut next = st.clone();
            let mut acts = others_actions(&st, cfg, me, others);
            acts[me] = a;
            let before = next.ships[me].total_yield;
            step(&mut next, &acts, cfg);
            let reward = next.ships[me].total_yield - before;
            let v = reward + br_value_cl(&next, cfg, me, others, memo);
            if v == target && best == u64::MAX { chosen = a; best = v; } // first (tie-break: Stay/lowest)
        }
        let mut acts = others_actions(&st, cfg, me, others);
        acts[me] = chosen;
        step(&mut st, &acts, cfg);
    }
    st.ships[me].total_yield
}
```

Add to `policies.rs` the helper used above (decide for all except `me` is inlined here; expose `decide_all_except` if you prefer — otherwise delete the unused import). Minimal: remove the `decide_all_except` import since `others_actions` inlines it.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: PASS (both phantom-check and closed≤open).

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/dp.rs crates/jumpgate-commons-cut/src/policies.rs
git commit -m "feat(commons-cut): closed-loop best-response ceiling + phantom-ceiling cross-check"
```

---

### Task 11: Planner upper bound (labelled, reported-only)

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/dp.rs`

**Spec §4.1:** compute the coordinated social-planner optimum as a **labelled upper bound only** (NOT the gate). It maximizes *summed* yield by jointly choosing all ships' actions. Sanity: `planner ≥ closed-loop selfish BR` (it's an upper bound).

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn planner_is_an_upper_bound_on_selfish_br() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 6,
        };
        let planner = planner_value(&cfg, &[0u8, 1u8]);
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let (selfish, _) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        // Planner maximizes TOTAL across ships; selfish is one ship's take -> planner total >= any single share.
        assert!(planner >= selfish, "planner total {planner} >= selfish single-ship {selfish}");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: FAIL — `planner_value` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
/// LABELLED UPPER BOUND ONLY (NOT the gate, spec §4.1): coordinated social-planner
/// optimum = max over JOINT action sequences of summed ship yield. Backward induction
/// over the joint action space. Reported as "coordination headroom — not learnable".
pub fn planner_value(cfg: &ArenaConfig, starts: &[u8]) -> u64 {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    planner_rec(&st0, cfg, &mut memo)
}

fn planner_rec(st: &ArenaState, cfg: &ArenaConfig, memo: &mut HashMap<(u64, u32), u64>) -> u64 {
    if st.tick >= cfg.horizon { return 0; }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) { return v; }
    // Joint action space = product over ships of {Stay, MoveTo(other regions)}.
    let per_ship: Vec<Vec<Action>> = (0..st.ships.len()).map(|i| {
        let mut c = vec![Action::Stay];
        if let Some(here) = st.ships[i].region {
            for r in 0..st.regions.len() { if r != here as usize { c.push(Action::MoveTo(r as u8)); } }
        }
        c
    }).collect();
    let mut best = 0u64;
    let mut idx = vec![0usize; st.ships.len()];
    loop {
        let acts: Vec<Action> = (0..st.ships.len()).map(|i| per_ship[i][idx[i]]).collect();
        let mut next = st.clone();
        let before: u64 = next.ships.iter().map(|s| s.total_yield).sum();
        step(&mut next, &acts, cfg);
        let after: u64 = next.ships.iter().map(|s| s.total_yield).sum();
        let v = (after - before) + planner_rec(&next, cfg, memo);
        if v > best { best = v; }
        // odometer over the joint index
        let mut k = 0;
        loop {
            if k == idx.len() { return { memo.insert(key, best); best }; }
            idx[k] += 1;
            if idx[k] < per_ship[k].len() { break; }
            idx[k] = 0; k += 1;
        }
    }
}
```

- [ ] **Step 2 note:** the joint action space is exponential in N — keep this for N=3 only. Beyond N=3 the planner upper bound is omitted (or replaced by a fluid relaxation), but it is reporting-only so its absence never blocks the gate.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut dp`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/dp.rs
git commit -m "feat(commons-cut): planner upper bound (labelled coordination headroom, reported-only)"
```

---

### Task 12: Fraction-of-ceiling + per-instance gate

**Files:**
- Create: `crates/jumpgate-commons-cut/src/gate.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod gate;`)

**Spec §5:** `frac = (ceiling_closed_loop_BR − best_closed_form) / (ceiling_closed_loop_BR − constant)`. All evaluated as MEAN over eval seeds. `GAP_FRAC_MIN = 0.10`. (This task: the per-instance fraction at the exact N=3 rung. The N-ladder + MC is Task 13.)

- [ ] **Step 1: Write the failing test** (in `src/gate.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_definition_is_ceiling_minus_bar_over_ceiling_minus_floor() {
        // ceiling=100, bar=95, floor=20 -> (100-95)/(100-20) = 5/80 = 0.0625 -> below 0.10.
        let f = fraction_of_ceiling(100.0, 95.0, 20.0);
        assert!((f - 0.0625).abs() < 1e-9);
        assert!(f < GAP_FRAC_MIN);
        // ceiling=100, bar=80, floor=20 -> 20/80 = 0.25 -> GO-eligible.
        assert!(fraction_of_ceiling(100.0, 80.0, 20.0) >= GAP_FRAC_MIN);
    }

    #[test]
    fn degenerate_zero_range_is_zero_frac_not_nan() {
        assert_eq!(fraction_of_ceiling(20.0, 20.0, 20.0), 0.0);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut gate`
Expected: FAIL — `fraction_of_ceiling`, `GAP_FRAC_MIN` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/gate.rs`)

```rust
/// Pre-registered gate threshold (spec §2). Do NOT move post-hoc.
pub const GAP_FRAC_MIN: f64 = 0.10;

/// Fraction-of-ceiling (spec §5). f64 is permitted here — this is MEASUREMENT, downstream
/// of the integer sim, never fed back into a transition. Degenerate range -> 0.0 (not NaN).
pub fn fraction_of_ceiling(ceiling: f64, bar: f64, floor: f64) -> f64 {
    let range = ceiling - floor;
    if range <= 0.0 { return 0.0; }
    ((ceiling - bar) / range).max(0.0)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut gate`
Expected: PASS. Add `pub mod gate;` to `lib.rs`.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/gate.rs
git commit -m "feat(commons-cut): fraction-of-ceiling definition + pre-registered GAP_FRAC_MIN"
```

---

### Task 13: MC best-response estimator + DP calibration + confidence interval

**Files:**
- Create: `crates/jumpgate-commons-cut/src/mc.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod mc;`)

**Spec §5:** an MC estimator of the closed-loop BR value (for N where the exact DP is infeasible), CALIBRATED against the exact DP at small N (the phantom-ceiling realized≈V₀ is the calibration), reporting a confidence interval across eval seeds. The MC BR uses a bounded-depth lookahead / sampled rollouts of the reactive field.

- [ ] **Step 1: Write the failing test** (in `src/mc.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArenaConfig;

    #[test]
    fn mc_br_approximates_exact_dp_at_small_n_within_ci() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 8,
        };
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let (exact, _) = crate::dp::best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        let est = mc_best_response(&cfg, &[0u8, 1u8], 0, &others, 64, 7);
        // The exact value should fall within the MC confidence interval (calibration).
        assert!(est.lo <= exact as f64 && exact as f64 <= est.hi,
            "exact {exact} must lie in MC CI [{:.1},{:.1}]", est.lo, est.hi);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut mc`
Expected: FAIL — `mc_best_response`, `Estimate` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/mc.rs`)

```rust
use crate::dynamics::step;
use crate::policies::{ClosedForm, Observation, Policy};
use crate::{Action, ArenaConfig, ArenaState};
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::RngCore;

/// A point estimate with a (mean ± 1.96·SE) confidence interval. f64 = measurement only.
pub struct Estimate { pub mean: f64, pub lo: f64, pub hi: f64 }

/// MC estimate of ship `me`'s closed-loop best-response value: `samples` independent
/// greedy-lookahead rollouts (depth `lookahead`), reactive others live. Deterministic
/// (seeded per sample). Returns mean + 95% CI across samples.
pub fn mc_best_response(
    cfg: &ArenaConfig, starts: &[u8], me: usize, others: &ClosedForm,
    samples: u32, base_seed: u64,
) -> Estimate {
    let mut vals = Vec::with_capacity(samples as usize);
    for s in 0..samples {
        let mut st = ArenaState::from_config(cfg, starts);
        let mut rng = RngStreams::from_master(base_seed ^ (s as u64));
        while st.tick < cfg.horizon {
            // greedy 1-step lookahead for `me` (sampled tie-break), reactive others
            let action = greedy_lookahead(&st, cfg, me, others, rng.stream(RngStream::Scenario), lookahead());
            let mut acts: Vec<Action> = (0..st.ships.len())
                .map(|i| if i == me { action } else { others.decide(&Observation { state: &st, ship_idx: i }) })
                .collect();
            acts[me] = action;
            step(&mut st, &acts, cfg);
        }
        vals.push(st.ships[me].total_yield as f64);
    }
    summarize(&vals)
}

fn lookahead() -> u32 { 3 }

fn greedy_lookahead(
    st: &ArenaState, cfg: &ArenaConfig, me: usize, others: &ClosedForm,
    rng: &mut impl RngCore, depth: u32,
) -> Action {
    let mut candidates = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() { if r != here as usize { candidates.push(Action::MoveTo(r as u8)); } }
    }
    let mut best = candidates[0];
    let mut best_v = -1i64;
    for a in candidates {
        let mut next = st.clone();
        let mut acts: Vec<Action> = (0..st.ships.len())
            .map(|i| if i == me { a } else { others.decide(&Observation { state: st, ship_idx: i }) })
            .collect();
        acts[me] = a;
        let before = next.ships[me].total_yield;
        step(&mut next, &acts, cfg);
        let mut v = (next.ships[me].total_yield - before) as i64;
        if depth > 1 && next.tick < cfg.horizon {
            // shallow continuation under the same greedy
            let cont = greedy_lookahead(&next, cfg, me, others, rng, depth - 1);
            let mut n2 = next.clone();
            let mut a2: Vec<Action> = (0..n2.ships.len())
                .map(|i| if i == me { cont } else { others.decide(&Observation { state: &next, ship_idx: i }) })
                .collect();
            a2[me] = cont;
            let b2 = n2.ships[me].total_yield;
            step(&mut n2, &a2, cfg);
            v += (n2.ships[me].total_yield - b2) as i64;
        }
        // sampled tie-break for diversity across MC samples
        if v > best_v || (v == best_v && (rng.next_u32() & 1) == 1) {
            best_v = v; best = a;
        }
    }
    best
}

fn summarize(vals: &[f64]) -> Estimate {
    let n = vals.len().max(1) as f64;
    let mean = vals.iter().sum::<f64>() / n;
    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let se = (var / n).sqrt();
    Estimate { mean, lo: mean - 1.96 * se, hi: mean + 1.96 * se }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut mc`
Expected: PASS. Add `pub mod mc;` to `lib.rs`. If the exact value falls outside the CI, the MC lookahead is too shallow — increase `lookahead()` or `samples` (the DP is ground truth; the MC must bracket it).

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/mc.rs
git commit -m "feat(commons-cut): MC best-response estimator + DP-calibrated confidence interval"
```

---

### Task 14: N-scaling verdict + controls

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/gate.rs`

**Spec §5/§8:** the gate is the N-curve. GO iff `frac(N) ≥ 0.10` AND flat-or-rising in N (CI-aware: don't GO when the CI straddles 0.10). NO-GO iff below at the smallest exact rung OR decaying toward the gate as N rises. Negative control (identical regions) must NO-GO by construction; positive control read as "room at small N" then checked up the ladder.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn verdict_go_requires_above_gate_and_non_decaying() {
        // frac flat-or-rising and all >= 0.10 -> GO
        let rising = vec![(3, 0.12, 0.10, 0.14), (6, 0.13, 0.11, 0.15), (12, 0.15, 0.13, 0.17)];
        assert_eq!(verdict(&rising), Verdict::Go);
        // decaying toward the gate as N rises -> NO-GO (LLN signature)
        let decaying = vec![(3, 0.30, 0.28, 0.32), (6, 0.18, 0.16, 0.20), (12, 0.09, 0.07, 0.11)];
        assert_eq!(verdict(&decaying), Verdict::NoGo);
        // below gate at smallest rung -> NO-GO
        let low = vec![(3, 0.04, 0.02, 0.06)];
        assert_eq!(verdict(&low), Verdict::NoGo);
        // CI straddles gate at smallest rung -> Inconclusive (widen sampling)
        let straddle = vec![(3, 0.11, 0.06, 0.16)];
        assert_eq!(verdict(&straddle), Verdict::Inconclusive);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut gate`
Expected: FAIL — `verdict`, `Verdict` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
/// The pre-registered verdict (spec §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict { Go, NoGo, Inconclusive }

/// Each rung: (N, frac_mean, frac_ci_lo, frac_ci_hi). Smallest N first (exact-DP rung).
pub fn verdict(curve: &[(u32, f64, f64, f64)]) -> Verdict {
    if curve.is_empty() { return Verdict::Inconclusive; }
    let (_, m0, lo0, hi0) = curve[0];
    // smallest exact rung below gate -> NO-GO
    if hi0 < GAP_FRAC_MIN { return Verdict::NoGo; }
    if lo0 < GAP_FRAC_MIN && hi0 >= GAP_FRAC_MIN && m0 < GAP_FRAC_MIN { return Verdict::NoGo; }
    if lo0 < GAP_FRAC_MIN { return Verdict::Inconclusive; } // CI straddles -> widen sampling
    // above gate at the smallest rung; now require flat-or-rising (no decay toward gate)
    let mut prev = m0;
    for &(_, m, _, _) in &curve[1..] {
        if m < prev - 0.02 { return Verdict::NoGo; } // decaying (LLN signature), 2pt tolerance
        prev = m;
    }
    // final rung must still clear the gate
    let last = curve.last().unwrap();
    if last.1 >= GAP_FRAC_MIN { Verdict::Go } else { Verdict::NoGo }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut gate`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/gate.rs
git commit -m "feat(commons-cut): N-scaling verdict (flat-or-rising, CI-aware) — the gate"
```

---

### Task 15: Verdict summary struct + observe-large pack diagnostics + the pre-registered run

**Files:**
- Create: `crates/jumpgate-commons-cut/src/report.rs`
- Modify: `crates/jumpgate-commons-cut/src/lib.rs` (`pub mod report;`)

**Spec §6/§10:** a machine-readable `CutSummary` (verdict + curve + controls + planner-headroom label) and `#[ignore]` diagnostic entry points that run the real experiment (the sweep × N-ladder) and the observe-large pack diagnostics. Packs existing is NOT a GO (spec §6).

- [ ] **Step 1: Write the failing test** (in `src/report.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_summary_carries_verdict_and_curve() {
        let s = CutSummary {
            verdict: crate::gate::Verdict::NoGo,
            curve: vec![(3, 0.04, 0.02, 0.06)],
            negative_control_nogo: true,
            planner_headroom_frac: 0.40,
        };
        assert_eq!(s.verdict, crate::gate::Verdict::NoGo);
        assert!(s.negative_control_nogo, "apparatus fairness: identical regions must NO-GO");
        assert!(s.planner_headroom_frac > s.curve[0].1, "planner upper bound exceeds selfish frac");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut report`
Expected: FAIL — `CutSummary` not defined.

- [ ] **Step 3: Write minimal implementation** (in `src/report.rs`)

```rust
use crate::gate::Verdict;

/// Machine-readable verdict (spec §10) — a harness asserts on this, not on println output.
#[derive(Clone, Debug, PartialEq)]
pub struct CutSummary {
    pub verdict: Verdict,
    /// (N, frac_mean, ci_lo, ci_hi) per ladder rung, smallest N first.
    pub curve: Vec<(u32, f64, f64, f64)>,
    /// Apparatus fairness: the identical-regions negative control must be NO-GO.
    pub negative_control_nogo: bool,
    /// The labelled coordination-headroom upper bound (reported, NOT gated).
    pub planner_headroom_frac: f64,
}

/// THE PRE-REGISTERED RUN (spec §2/§5). `#[ignore]` — invoked deliberately, not in CI.
/// Sweeps regen × field-correlation, builds the N-ladder (exact DP at N=3, MC above),
/// computes the verdict, prints it, and returns the CutSummary.
#[cfg(test)]
mod run {
    use super::*;
    use crate::dp::{best_response_value_closed_loop_checked, planner_value};
    use crate::gate::{fraction_of_ceiling, verdict};
    use crate::mc::mc_best_response;
    use crate::policies::{fit_closed_form, rollout, Constant};
    use crate::rng_bridge::build_scenario;

    #[test]
    #[ignore = "the pre-registered experiment — run deliberately: cargo test -p jumpgate-commons-cut --ignored run_the_cut"]
    fn run_the_cut() {
        let train: Vec<u64> = (1000..1008).collect();
        let eval: Vec<u64> = (2000..2008).collect();
        let summary = execute(&train, &eval, 0 /*one-shot regen*/, 0 /*independent corr*/);
        println!("CUT VERDICT: {:?}", summary.verdict);
        println!("  curve (N, frac, lo, hi): {:?}", summary.curve);
        println!("  negative_control_nogo: {}", summary.negative_control_nogo);
        println!("  planner_headroom_frac (NOT learnable): {:.3}", summary.planner_headroom_frac);
        // No assertion on Go/NoGo — the verdict is the finding. Only apparatus fairness is asserted.
        assert!(summary.negative_control_nogo, "negative control must NO-GO or the apparatus is rigged");
    }

    /// The experiment body, separated so it is unit-testable on tiny inputs.
    pub fn execute(train: &[u64], eval: &[u64], regen: u32, corr: u32) -> CutSummary {
        // N=3 exact rung (M=3).
        let cf = fit_closed_form(3, 3, regen, corr, train);
        let others = crate::policies::ClosedForm { tau: cf.tau, move_prob_milli: cf.move_prob_milli, seed: cf.seed };
        let (mut ceil_sum, mut bar_sum, mut floor_sum) = (0f64, 0f64, 0f64);
        for &s in eval {
            let cfg = build_scenario(s, 3, 3, regen, corr);
            let starts = [0u8, 1, 2];
            let (c, _) = best_response_value_closed_loop_checked(&cfg, &starts, 0, &others);
            ceil_sum += c as f64;
            bar_sum += rollout(&cfg, &starts, &cf).iter().sum::<u64>() as f64 / 3.0; // per-ship mean to match single-ship ceiling
            floor_sum += rollout(&cfg, &starts, &Constant).iter().sum::<u64>() as f64 / 3.0;
        }
        let n = eval.len() as f64;
        let frac3 = fraction_of_ceiling(ceil_sum / n, bar_sum / n, floor_sum / n);
        // (CI at N=3 is exact -> degenerate interval = the point.)
        let mut curve = vec![(3u32, frac3, frac3, frac3)];

        // MC-carried rungs (N=6,12,24 at fixed N/M=1 here, M=3): estimate ceiling via MC.
        for &nn in &[6u32, 12, 24] {
            let (mut cl, mut ch, mut bar, mut flo) = (0f64, 0f64, 0f64, 0f64);
            for &s in eval {
                let cfg = build_scenario(s, 3, 3, regen, corr); // M=3 fixed; N scales via starts
                let starts: Vec<u8> = (0..nn).map(|i| (i % 3) as u8).collect();
                let est = mc_best_response(&cfg, &starts, 0, &others, 128, s);
                cl += est.lo; ch += est.hi;
                bar += rollout(&cfg, &starts, &cf).iter().sum::<u64>() as f64 / nn as f64;
                flo += rollout(&cfg, &starts, &Constant).iter().sum::<u64>() as f64 / nn as f64;
            }
            let mean_ceiling = (cl + ch) / (2.0 * n);
            let frac_mean = fraction_of_ceiling(mean_ceiling, bar / n, flo / n);
            let frac_lo = fraction_of_ceiling(cl / n, bar / n, flo / n);
            let frac_hi = fraction_of_ceiling(ch / n, bar / n, flo / n);
            curve.push((nn, frac_mean, frac_lo.min(frac_hi), frac_lo.max(frac_hi)));
        }

        // Negative control: identical regions (corr=1000) must NO-GO.
        let neg = {
            let cfgn = build_scenario(eval[0], 3, 3, regen, 1000);
            let starts = [0u8, 1, 2];
            let (c, _) = best_response_value_closed_loop_checked(&cfgn, &starts, 0, &others);
            let bar = rollout(&cfgn, &starts, &cf).iter().sum::<u64>() as f64 / 3.0;
            let flo = rollout(&cfgn, &starts, &Constant).iter().sum::<u64>() as f64 / 3.0;
            fraction_of_ceiling(c as f64, bar, flo) < crate::gate::GAP_FRAC_MIN
        };

        let planner = {
            let cfg = build_scenario(eval[0], 3, 3, regen, corr);
            let p = planner_value(&cfg, &[0, 1, 2]) as f64;
            let flo = rollout(&cfg, &[0, 1, 2], &Constant).iter().sum::<u64>() as f64;
            fraction_of_ceiling(p, flo, 0.0) // headroom of the planner total over the floor total
        };

        CutSummary { verdict: verdict(&curve), curve, negative_control_nogo: neg, planner_headroom_frac: planner }
    }

    #[test]
    fn execute_runs_and_negative_control_holds_on_tiny_inputs() {
        let s = execute(&[1u64, 2], &[3u64], 0, 0);
        assert!(!s.curve.is_empty());
        // On independent regions the negative control (corr=1000) should be NO-GO by construction.
        assert!(s.negative_control_nogo);
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut report` (the non-ignored `execute_runs...` + `cut_summary...`)
Expected: PASS. Add `pub mod report;` to `lib.rs`. Then the real experiment: `cargo test -p jumpgate-commons-cut --ignored run_the_cut -- --nocapture` prints the verdict.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/lib.rs crates/jumpgate-commons-cut/src/report.rs
git commit -m "feat(commons-cut): CutSummary verdict struct + the pre-registered run (sweep + N-ladder + controls)"
```

---

### Task 16: Observe-large pack diagnostics (reported, NOT the gate)

**Files:**
- Modify: `crates/jumpgate-commons-cut/src/report.rs`

**Spec §6:** N=100–200, M=10–20, H≈5000, best-closed-form only. Diagnostics: pack count K(t) + autocorrelation, spatial-entropy oscillation, exodus/regroup lag, boom/bust period, camper-vs-mover payoff. **Pre-registered: packs existing is NOT a GO.**

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn pack_diagnostics_compute_on_a_fixture() {
        let series = vec![
            vec![10u32, 0, 0],  // all clumped on region 0
            vec![5, 5, 0],
            vec![0, 5, 5],      // dispersed/moved
        ];
        let d = pack_diagnostics(&series);
        assert!(d.mean_spatial_entropy >= 0.0);
        assert!(d.peak_pack_count >= 1);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jumpgate-commons-cut report`
Expected: FAIL — `pack_diagnostics`, `PackDiagnostics` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Observe-large diagnostics (spec §6). Pure measurement (f64 ok). NOT a GO signal.
#[derive(Clone, Debug)]
pub struct PackDiagnostics {
    pub peak_pack_count: u32,
    pub mean_spatial_entropy: f64,
    pub entropy_oscillation: f64, // max-min of per-tick entropy (concentrate<->disperse)
}

/// `occupancy[t][r]` = ship count at region r at tick t. A "pack" = a region above a
/// quarter-of-mean occupancy threshold.
pub fn pack_diagnostics(occupancy: &[Vec<u32>]) -> PackDiagnostics {
    let mut peak = 0u32;
    let mut entropies = Vec::new();
    for row in occupancy {
        let total: u32 = row.iter().sum();
        if total == 0 { entropies.push(0.0); continue; }
        let thresh = (total as f64 / row.len() as f64) / 4.0;
        peak = peak.max(row.iter().filter(|&&o| o as f64 >= thresh).count() as u32);
        // Shannon entropy of the occupancy distribution.
        let mut h = 0.0;
        for &o in row {
            if o == 0 { continue; }
            let p = o as f64 / total as f64;
            h -= p * p.ln();
        }
        entropies.push(h);
    }
    let mean = entropies.iter().sum::<f64>() / entropies.len().max(1) as f64;
    let osc = entropies.iter().cloned().fold(f64::MIN, f64::max)
        - entropies.iter().cloned().fold(f64::MAX, f64::min);
    PackDiagnostics { peak_pack_count: peak, mean_spatial_entropy: mean, entropy_oscillation: osc.max(0.0) }
}
```

Also add an `#[ignore]` `observe_large` entry point in the `run` module that builds N=120/M=12/H=5000, rolls best-closed-form recording per-tick occupancy, and prints `pack_diagnostics`. (No assertion — it is context, not the gate.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p jumpgate-commons-cut report`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/src/report.rs
git commit -m "feat(commons-cut): observe-large pack diagnostics (reported, not a GO signal)"
```

---

### Task 17: Phase gate — full suite, clippy, run the cut, record the verdict

**Files:**
- Create: `crates/jumpgate-commons-cut/RESULT.md` (the recorded verdict)

- [ ] **Step 1: Full green + clippy**

Run: `cargo test -p jumpgate-commons-cut` (all non-ignored) → green.
Run: `cargo clippy -p jumpgate-commons-cut --all-targets -- -D warnings` → clean.
Run: `cargo test -p jumpgate-commons-cut --test golden_trajectory` → the determinism golden holds.

- [ ] **Step 2: Run the pre-registered experiment**

Run: `cargo test -p jumpgate-commons-cut --ignored run_the_cut -- --nocapture`
Capture the printed `CUT VERDICT`, curve, negative-control flag, and planner-headroom.

- [ ] **Step 3: Run observe-large (context)**

Run: `cargo test -p jumpgate-commons-cut --ignored observe_large -- --nocapture`
Capture pack diagnostics.

- [ ] **Step 4: Record the verdict honestly** in `crates/jumpgate-commons-cut/RESULT.md`: the verdict (GO / NO-GO / Inconclusive), the frac curve with CIs, whether it decays with N, the negative control, the planner-headroom (labelled not-learnable), and the honest scope line: *"this measured the FULL-INFO commons game; it did NOT test the information game (Media / partial observability — spec §9)."* If NO-GO, state it is reported and honored per PDR-0005; surface the information-room bet as the owner's next decision.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-commons-cut/RESULT.md
git commit -m "test(commons-cut): phase gate — run the pre-registered cut + record the verdict"
```

---

## Self-review (author)

**Spec coverage:** arena model + yield/depletion/regen + gradient check (T4); field-correlation + regen axes (T3); integer determinism + golden hash + pinned tie-breaks (T4/T5/T6/T8); the four-rung ladder incl. randomizing closed-form (T6/T7/T8); closed-loop selfish BR ceiling + phantom-check + planner-upper-bound (T9/T10/T11); fraction-of-ceiling + 10% gate (T12); MC estimator + DP calibration + CI (T13); N-scaling verdict + controls (T14); CutSummary + the pre-registered run + sweep (T15); observe-large pack diagnostics (T16); phase gate + recorded verdict + honest NO-GO scoping (T17). All spec sections map to a task.

**Placeholder scan:** no TBD/TODO; every code step has real code. Two flagged implementer judgment calls are explicit, not placeholders: STOCK_MAX may rise to 50 after the T4 gradient check (re-derive dependent constants); the golden hash literal is captured once in T5.

**Type consistency:** `Region`/`Ship`/`ArenaConfig`/`ArenaState`/`Action` (T1/T2) used consistently; `Policy`/`Observation`/`rollout`/`occupant_counts` (T6) reused by T7/T8/T13; `ClosedForm`/`fit_closed_form` (T7) reused by T10/T13/T15; `encode`/`best_response_value_closed_loop_checked`/`planner_value` (T9–11) reused by T13/T15; `fraction_of_ceiling`/`GAP_FRAC_MIN`/`verdict`/`Verdict` (T12/T14) reused by T15; `CutSummary` (T15). Consistent.

**Known implementer caveats (call out at execution):** (a) `dp::decode` derives `n_ships` from `cfg.travel.len()` — fine for N=M instances and tests; thread real N if N≠M. (b) The exact DP is N=3-only; T13's MC carries the ladder above — verify MC brackets the exact DP (T13 test) before trusting any N>3 number. (c) The N=3 false-GO risk (spec §8) is real — the verdict's flat-or-rising requirement (T14) is the guard; do not quote the N=3 point alone.
