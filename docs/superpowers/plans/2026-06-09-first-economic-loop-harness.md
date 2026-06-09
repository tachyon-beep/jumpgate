# First Economic Loop — Deterministic Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the first demand-driven economic loop (mine → refine → haul-under-contract → consume, with stock-driven price) as a deterministic, replayable correctness harness on the live `jumpgate-core` substrate.

**Architecture:** All economy state is new struct-of-arrays stores in a new `economy.rs`, minted at `World::reset` and advanced by new deterministic stages inserted into `World::step`. The single non-additive cost is one `HASH_FORMAT_VERSION` bump (1→2) folding all hashed economy state. Money is `i64` microcredits. Stations are Bodies; haulers dock via the already-live co-orbiting rendezvous arrival. NOT a place DRL is expected to win (PDR-0005) — success = conservation + replay.

**Tech Stack:** Rust 2024, `jumpgate-core` (`#![forbid(unsafe_code)]`), FNV-1a hashing, generational `SlotMap` ids. `cargo test -p jumpgate-core`, `cargo clippy --all-targets`.

**Spec:** `docs/superpowers/specs/2026-06-09-first-economic-loop-harness-design.md`. **Decisions (owner 2026-06-09):** Ore→Fuel→consume cargo-only; linear config-tuned deflation; stations are Bodies; one plan, three phases.

---

## Canonical types (defined by the tasks below — referenced everywhere, names are fixed)

These are the exact names/signatures later tasks rely on. They are introduced by the cited task; do not rename.

- `Resource` (Task 1): `enum Resource { Ore, Fuel }` — `Copy`; `Resource::ALL: [Resource; 2]`; `fn index(self) -> usize`. `pub const N_RESOURCES: usize = 2`.
- Ids (Task 2, in `ids.rs`): `StationId`, `ProducerId`, `CorporationId`, `ContractId` — each `{ slot: u32, generation: u32 }`, minted via `SlotMap`, mirroring `CraftId`.
- `Recipe` (Task 3): `struct Recipe { input: Option<(Resource, u32)>, output: Option<(Resource, u32)>, interval: u32 }`.
- `ContractStatus` (Task 6): `enum ContractStatus { Offered, Accepted, CargoLoaded, InTransit, Delivered, Completed, Failed }` — `Copy`; `fn rank(self) -> u8` for hashing.
- `CraftRole` (Task 8): `enum CraftRole { Idle, Hauler }` — `Copy`.
- Stores (Tasks 3–7): `StationStore`, `ProducerStore`, `CorporationStore`, `ContractStore` in `economy.rs`.
- New `EventKind` variants (Task 9): `Production{producer, resource, qty}`, `Trade{station, resource, qty, price_micros}`, `PriceUpdate{station, resource, price_micros}`, `ContractOffered{contract}`, `ContractAccepted{contract, hauler}`, `ContractFulfilled{contract, hauler}`.
- New `CommandKind` variants (Task 10): `AcceptContract{contract}`, `SetRole{role}`.
- Money: every credit value is `i64` **microcredits**. The only float↔int boundary is `price_micros = (price_f64 * 1_000_000.0).round() as i64`, isolated in one helper `to_micros` (Task 11, Stage 2 only).

---

# PHASE 0 — PRELUDE (stores exist, hash bumped, still an inert physics world)

Goal of phase: every economy store exists and is minted empty/from-config, folded into both hashes, with goldens re-pinned in single-cause commits. The world still behaves exactly as today when no economy is configured. **Determinism-critical — do this phase first and alone.**

### Task 1: `Resource` enum

**Files:**
- Create: `crates/jumpgate-core/src/economy.rs`
- Modify: `crates/jumpgate-core/src/lib.rs` (add `pub mod economy;`)

- [ ] **Step 1: Write the failing test** (in `economy.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_index_is_stable_and_dense() {
        assert_eq!(Resource::Ore.index(), 0);
        assert_eq!(Resource::Fuel.index(), 1);
        assert_eq!(Resource::ALL.len(), N_RESOURCES);
        for (i, r) in Resource::ALL.iter().enumerate() {
            assert_eq!(r.index(), i);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core economy::tests::resource_index -- --nocapture`
Expected: FAIL — `economy` module / `Resource` not found.

- [ ] **Step 3: Write minimal implementation** (top of `economy.rs`)

```rust
//! Economy stores + systems (the first demand-driven loop, deterministic harness).
//! All economy state is hashed (HASH_FIELD_ORDER words appended in this phase) and
//! all money is i64 microcredits. Stations are Bodies; haulers dock via the live
//! co-orbiting rendezvous arrival (events.rs).

/// The commodity set for the v1 thin loop. `index()` is the canonical dense order
/// used by every per-station resource array and by the state hash. APPEND-ONLY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Ore,
    Fuel,
}

pub const N_RESOURCES: usize = 2;

impl Resource {
    pub const ALL: [Resource; N_RESOURCES] = [Resource::Ore, Resource::Fuel];
    pub fn index(self) -> usize {
        match self {
            Resource::Ore => 0,
            Resource::Fuel => 1,
        }
    }
}
```

Add to `lib.rs` (alongside the other `pub mod` lines): `pub mod economy;`

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core economy::tests::resource_index`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/lib.rs
git commit -m "feat(economy): Resource enum (Ore, Fuel) + dense index"
```

### Task 2: economy id types

**Files:**
- Modify: `crates/jumpgate-core/src/ids.rs`

- [ ] **Step 1: Write the failing test** (append to `ids.rs` tests)

```rust
#[test]
fn economy_ids_are_distinct_generational() {
    let mut sm: SlotMap<()> = SlotMap::new();
    let (slot, generation) = sm.insert(());
    let s = StationId { slot, generation };
    let c = ContractId { slot, generation };
    assert_eq!(s.slot, c.slot); // same numeric slot...
    // ...but they are different *types*: this test compiles only if both exist.
    let _p = ProducerId { slot: 0, generation: 0 };
    let _co = CorporationId { slot: 1, generation: 0 };
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core ids::tests::economy_ids_are_distinct`
Expected: FAIL — `StationId` etc. not found.

- [ ] **Step 3: Write minimal implementation**

Mirror the existing `CraftId`/`BodyId` definitions in `ids.rs`. For each of `StationId`, `ProducerId`, `CorporationId`, `ContractId`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StationId {
    pub slot: u32,
    pub generation: u32,
}
// ...identical shape for ProducerId, CorporationId, ContractId.
```

(Match whatever derives `CraftId` carries in this file — copy them exactly so sorting/`Ord` works for canonical hash iteration.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core ids::tests::economy_ids_are_distinct`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/ids.rs
git commit -m "feat(economy): StationId/ProducerId/CorporationId/ContractId"
```

### Task 3: `StationStore` + `ProducerStore` + `Recipe` (empty constructors)

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn station_and_producer_stores_start_empty_and_parallel() {
    let s = StationStore::empty();
    assert_eq!(s.ids.len(), 0);
    assert_eq!(s.body.len(), 0);
    assert_eq!(s.stock.len(), 0);
    assert_eq!(s.price_micros.len(), 0);

    let p = ProducerStore::empty();
    assert_eq!(p.ids.len(), 0);
    assert_eq!(p.station.len(), 0);
    assert_eq!(p.recipe.len(), 0);
    assert_eq!(p.last_fired.len(), 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core economy::tests::station_and_producer`
Expected: FAIL — types not found.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::ids::{BodyId, ContractId, CorporationId, CraftId, ProducerId, SlotMap, StationId};
use crate::time::Tick;

/// A producer's recipe: optional input consumed, optional output produced, every
/// `interval` ticks (all-or-nothing). Mining = (None, Some(Ore)); refine =
/// (Some(Ore), Some(Fuel)); demand sink = (Some(Fuel), None).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub input: Option<(Resource, u32)>,
    pub output: Option<(Resource, u32)>,
    pub interval: u32,
}

/// Stations: a market (per-resource integer stock + integer micro-price) attached
/// to a Body (its orbital position). slot == row, no mid-run despawn (v1 invariant).
pub struct StationStore {
    pub ids: SlotMap<()>,
    pub body: Vec<BodyId>,
    pub stock: Vec<[i64; N_RESOURCES]>,
    pub price_micros: Vec<[i64; N_RESOURCES]>,
}

impl StationStore {
    pub fn empty() -> Self {
        StationStore { ids: SlotMap::new(), body: Vec::new(), stock: Vec::new(), price_micros: Vec::new() }
    }
    /// Append a station; returns its StationId. Enforces slot == row.
    pub fn push(&mut self, body: BodyId, stock: [i64; N_RESOURCES], price_micros: [i64; N_RESOURCES]) -> StationId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(slot as usize, self.body.len(), "station slot == row");
        self.body.push(body);
        self.stock.push(stock);
        self.price_micros.push(price_micros);
        StationId { slot, generation }
    }
}

pub struct ProducerStore {
    pub ids: SlotMap<()>,
    pub station: Vec<StationId>,
    pub recipe: Vec<Recipe>,
    pub last_fired: Vec<Tick>,
}

impl ProducerStore {
    pub fn empty() -> Self {
        ProducerStore { ids: SlotMap::new(), station: Vec::new(), recipe: Vec::new(), last_fired: Vec::new() }
    }
    pub fn push(&mut self, station: StationId, recipe: Recipe) -> ProducerId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(slot as usize, self.station.len(), "producer slot == row");
        self.station.push(station);
        self.recipe.push(recipe);
        self.last_fired.push(Tick(0));
        ProducerId { slot, generation }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core economy::tests::station_and_producer`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/economy.rs
git commit -m "feat(economy): StationStore + ProducerStore + Recipe (empty ctors)"
```

### Task 4: `CorporationStore` + `ContractStore` + `ContractStatus`

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn corp_and_contract_stores_start_empty() {
    let c = CorporationStore::empty();
    assert_eq!(c.ids.len(), 0);
    assert_eq!(c.treasury_micros.len(), 0);

    let k = ContractStore::empty();
    assert_eq!(k.ids.len(), 0);
    assert_eq!(k.status.len(), 0);
    // status rank is total + distinct (used by the state hash).
    assert_eq!(ContractStatus::Offered.rank(), 0);
    assert_ne!(ContractStatus::Failed.rank(), ContractStatus::Completed.rank());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core economy::tests::corp_and_contract`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractStatus {
    Offered,
    Accepted,
    CargoLoaded,
    InTransit,
    Delivered,
    Completed,
    Failed,
}

impl ContractStatus {
    /// Stable discriminant for self-delimiting state-hash folding. APPEND-ONLY.
    pub fn rank(self) -> u8 {
        match self {
            ContractStatus::Offered => 0,
            ContractStatus::Accepted => 1,
            ContractStatus::CargoLoaded => 2,
            ContractStatus::InTransit => 3,
            ContractStatus::Delivered => 4,
            ContractStatus::Completed => 5,
            ContractStatus::Failed => 6,
        }
    }
}

pub struct CorporationStore {
    pub ids: SlotMap<()>,
    pub treasury_micros: Vec<i64>,
    pub home_station: Vec<StationId>,
}

impl CorporationStore {
    pub fn empty() -> Self {
        CorporationStore { ids: SlotMap::new(), treasury_micros: Vec::new(), home_station: Vec::new() }
    }
    pub fn push(&mut self, treasury_micros: i64, home_station: StationId) -> CorporationId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(slot as usize, self.treasury_micros.len(), "corp slot == row");
        self.treasury_micros.push(treasury_micros);
        self.home_station.push(home_station);
        CorporationId { slot, generation }
    }
}

/// Delivery contracts: move `qty` of `resource` from `from_station` to `to_station`
/// for `reward_micros` (escrowed at accept). status enum + escrow are the hashed
/// lifecycle. `hauler` is set on accept.
pub struct ContractStore {
    pub ids: SlotMap<()>,
    pub status: Vec<ContractStatus>,
    pub corp: Vec<CorporationId>,
    pub resource: Vec<Resource>,
    pub qty: Vec<u32>,
    pub from_station: Vec<StationId>,
    pub to_station: Vec<StationId>,
    pub reward_micros: Vec<i64>,
    pub escrow_micros: Vec<i64>,
    pub hauler: Vec<Option<CraftId>>,
}

impl ContractStore {
    pub fn empty() -> Self {
        ContractStore {
            ids: SlotMap::new(), status: Vec::new(), corp: Vec::new(), resource: Vec::new(),
            qty: Vec::new(), from_station: Vec::new(), to_station: Vec::new(),
            reward_micros: Vec::new(), escrow_micros: Vec::new(), hauler: Vec::new(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn push(&mut self, corp: CorporationId, resource: Resource, qty: u32,
                from_station: StationId, to_station: StationId, reward_micros: i64) -> ContractId {
        let (slot, generation) = self.ids.insert(());
        self.status.push(ContractStatus::Offered);
        self.corp.push(corp);
        self.resource.push(resource);
        self.qty.push(qty);
        self.from_station.push(from_station);
        self.to_station.push(to_station);
        self.reward_micros.push(reward_micros);
        self.escrow_micros.push(0);
        self.hauler.push(None);
        ContractId { slot, generation }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core economy::tests::corp_and_contract`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/economy.rs
git commit -m "feat(economy): CorporationStore + ContractStore + ContractStatus"
```

### Task 5: economy initial-conditions in `RunConfig` + fold into `config_hash` (re-pin 0x278c)

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs`

- [ ] **Step 1: Write the failing test** (append to `config.rs` tests)

```rust
#[test]
fn changing_an_economy_field_changes_config_hash() {
    let mut c = sample();
    c.stations.push(StationInit { body_index: 0, initial_stock: [10, 0], initial_price_micros: [0, 0] });
    assert_ne!(sample().config_hash(), c.config_hash());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core config::tests::changing_an_economy_field`
Expected: FAIL — `stations`/`StationInit` not found.

- [ ] **Step 3: Write minimal implementation**

Add init structs (all data, integer money) and `RunConfig` fields:

```rust
#[derive(Clone, Debug)]
pub struct StationInit { pub body_index: usize, pub initial_stock: [i64; crate::economy::N_RESOURCES], pub initial_price_micros: [i64; crate::economy::N_RESOURCES] }
#[derive(Clone, Debug)]
pub struct ProducerInit { pub station_index: usize, pub recipe: crate::economy::Recipe }
#[derive(Clone, Debug)]
pub struct CorporationInit { pub treasury_micros: i64, pub home_station_index: usize }
#[derive(Clone, Debug)]
pub struct ContractInit { pub corp_index: usize, pub resource: crate::economy::Resource, pub qty: u32, pub from_station_index: usize, pub to_station_index: usize, pub reward_micros: i64 }
```

Add to `RunConfig` (after `guidance`):

```rust
    pub stations: Vec<StationInit>,
    pub producers: Vec<ProducerInit>,
    pub corporations: Vec<CorporationInit>,
    pub contracts: Vec<ContractInit>,
    /// Stage-2 deflation curve constants (linear: price = base*(2 - stock/cap*k)).
    pub price_cfg: PriceCfg,
```

```rust
#[derive(Clone, Copy, Debug)]
pub struct PriceCfg { pub base_micros: [i64; crate::economy::N_RESOURCES], pub cap: [i64; crate::economy::N_RESOURCES], pub slope_milli: i64 /* k*1000, e.g. 1800 == 1.8 */ }
impl Default for PriceCfg { fn default() -> Self { PriceCfg { base_micros: [0; crate::economy::N_RESOURCES], cap: [1; crate::economy::N_RESOURCES], slope_milli: 1800 } } }
```

In `config_hash`, extend the exhaustive destructure (the compiler will force this) and fold at the TAIL (after guidance), counts-first, in CONFIG_FIELD_ORDER. Append to the `CONFIG_FIELD_ORDER` doc comment. Example fold (integer fields fold directly; `Resource`/`Recipe` fold via discriminant + payload):

```rust
        // economy (TAIL, append-only). Counts first so cardinality always moves the hash.
        h.write_u64(stations.len() as u64);
        h.write_u64(producers.len() as u64);
        h.write_u64(corporations.len() as u64);
        h.write_u64(contracts.len() as u64);
        for s in stations {
            h.write_u64(s.body_index as u64);
            for r in 0..crate::economy::N_RESOURCES { h.write_u64(s.initial_stock[r] as u64); h.write_u64(s.initial_price_micros[r] as u64); }
        }
        for p in producers {
            h.write_u64(p.station_index as u64);
            write_recipe(&mut h, &p.recipe); // helper: input discriminant+payload, output discriminant+payload, interval
        }
        for c in corporations { h.write_u64(c.treasury_micros as u64); h.write_u64(c.home_station_index as u64); }
        for k in contracts {
            h.write_u64(k.corp_index as u64);
            h.write_u64(k.resource.index() as u64);
            h.write_u64(k.qty as u64);
            h.write_u64(k.from_station_index as u64);
            h.write_u64(k.to_station_index as u64);
            h.write_u64(k.reward_micros as u64);
        }
        h.write_u64(price_cfg.slope_milli as u64);
        for r in 0..crate::economy::N_RESOURCES { h.write_u64(price_cfg.base_micros[r] as u64); h.write_u64(price_cfg.cap[r] as u64); }
```

Add a free `fn write_recipe(h: &mut ConfigFnv, r: &crate::economy::Recipe)` that folds `input`/`output` as `0/1` discriminant then `(resource.index(), qty)` and `interval`.

Update EVERY existing `RunConfig { .. }` literal in the codebase (the compiler lists them) to include the new fields — for the existing fixtures use empty vecs + `PriceCfg::default()`:

```rust
            stations: vec![], producers: vec![], corporations: vec![], contracts: vec![], price_cfg: PriceCfg::default(),
```

- [ ] **Step 3b: Re-pin the config golden (single cause)**

The existing `config_hash_golden_anchor_is_stable` test (config.rs:251) will now fail. Confirm it fails for the RIGHT reason (added economy fold, all-empty in `sample()` still changes the byte stream because counts are folded), then run:

Run: `cargo test -p jumpgate-core config::tests -- --nocapture` (observe the new value in the failure), then update `GOLDEN_CONFIG_HASH` to the new value with a comment `// RE-PINNED: +economy fold. Was 0x278c_5d91_b75a_9e5a.`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p jumpgate-core config::`
Expected: PASS (all change-detection tests + the re-pinned golden).

- [ ] **Step 5: Commit (single cause)**

```bash
git add crates/jumpgate-core/src/config.rs
git commit -m "feat(economy): fold economy initial conditions into config_hash; re-pin GOLDEN_CONFIG_HASH (+economy)"
```

### Task 6: hauler economy columns on `CraftStore`

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs`

- [ ] **Step 1: Write the failing test** (append to `stores.rs` tests)

```rust
#[test]
fn push_initializes_hauler_columns_idle_empty() {
    let mut ship = CraftStore::empty();
    let spec = BaseSpec { base_dry_mass: 10.0, base_max_thrust: 250.0, base_exhaust_velocity: 30.0, base_fuel_capacity: 40.0 };
    ship.push(spec, Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, 40.0);
    assert_eq!(ship.role[0], CraftRole::Idle);
    assert_eq!(ship.cargo[0], None);
    assert_eq!(ship.credits_micros[0], 0);
    assert_eq!(ship.contract[0], None);
    assert_eq!(ship.role.len(), ship.ids.len());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core stores::tests::push_initializes_hauler`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
/// Economic role of a craft. v1: Idle or Hauler. Hashed (HASH_FIELD_ORDER).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CraftRole { Idle, Hauler }
impl CraftRole { pub fn rank(self) -> u8 { match self { CraftRole::Idle => 0, CraftRole::Hauler => 1 } } }
```

Add to `CraftStore` (length-parallel columns): `pub role: Vec<CraftRole>`, `pub cargo: Vec<Option<(crate::economy::Resource, u32)>>`, `pub credits_micros: Vec<i64>`, `pub contract: Vec<Option<crate::ids::ContractId>>`. Initialize in BOTH `empty()` (empty vecs) and `push()` (`CraftRole::Idle`, `None`, `0`, `None`) and in `World::reset`'s inline `CraftStore { .. }` literal (world.rs:144) — add the four fields there too. These are hashed economy state (see Task 7).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core stores::tests::push_initializes_hauler`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs
git commit -m "feat(economy): hauler role/cargo/credits/contract columns on CraftStore"
```

### Task 7: fold all economy state into `state_hash`; bump `HASH_FORMAT_VERSION` 1→2; re-pin both state goldens

**Files:**
- Modify: `crates/jumpgate-core/src/hash.rs`
- Modify: `crates/jumpgate-core/src/world.rs` (expose economy stores `pub(crate)` like `ships`/`bodies`)

This is the single allowed dual-golden move (RAID R2). Do it alone.

- [ ] **Step 1: Write/extend the failing test**

Extend `recompute_with_cursors` (hash.rs:334, the executable parity spec) to also fold, AFTER the per-craft words: per-craft `role.rank()`, cargo (discriminant 0/1 then `(resource.index(), qty)`), `credits_micros as u64`, contract (`0/1` then slot/generation); then, in sorted-id order, each station (slot, gen, body slot/gen, per-resource stock+price), each producer (slot, gen, station slot/gen, recipe), each corporation (slot, gen, treasury, home_station slot/gen), each contract (slot, gen, status.rank(), corp, resource.index(), qty, from/to station, reward, escrow, hauler 0/1+id). The existing `cursor_participates_in_state_hash` test (hash.rs:406) asserts `state_hash == recompute_with_cursors`; it will fail until Step 3 mirrors the same words into `state_hash`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core hash::tests::cursor_participates_in_state_hash`
Expected: FAIL (parity broken — recompute folds economy, state_hash does not yet).

- [ ] **Step 3: Implement — mirror the words into `state_hash`, bump the version, extend the doc**

In `hash.rs`: (a) bump `pub const HASH_FORMAT_VERSION: u32 = 2;` (b) append the new words to the `HASH_FIELD_ORDER` module doc (numbers 16+), with the exact economy fold order; (c) add the identical economy-folding code to `state_hash` that you added to `recompute_with_cursors`, reading `world.ships.role[..]` etc. and the economy stores (require `world.stations`/`producers`/`corporations`/`contracts` be `pub(crate)` — add them in Task 12, or add the fields now as empty stores; see note). Fold enums discriminant-first; iterate every store in sorted-id order.

> NOTE: economy stores live on `World` from Task 12. To keep this task self-contained, add the four economy stores to `World` as `pub(crate)` empty stores HERE (minted empty in `reset`), fold them, and let Task 12 populate them from config. An empty store folds only its zero cursor — deterministic.

- [ ] **Step 3b: Re-pin BOTH state goldens (single cause)**

`golden_zero_state_hash` (hash.rs:481) and `state_hash_golden_zero_world` (hash.rs:419) now fail. Confirm they fail for the version bump + appended words. For `golden_zero_state_hash`, append the new zero words to the hand-built hasher to match. For `state_hash_golden_zero_world`, run the `print_golden` ignored test:

Run: `cargo test -p jumpgate-core hash::tests::print_golden -- --ignored --nocapture`
Paste the printed value into `state_hash_golden_zero_world`'s assert and update `GOLDEN_ZERO_STATE_HASH` from the hand-built hasher's value. Comment both: `// RE-PINNED: HASH_FORMAT_VERSION 1->2 (+economy state words).`

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p jumpgate-core hash::`
Expected: PASS — parity test green, both goldens re-pinned, version == 2.

- [ ] **Step 5: Commit (single cause — the one dual-golden move)**

```bash
git add crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/world.rs
git commit -m "feat(economy)!: HASH_FORMAT_VERSION 1->2 — fold all economy state; re-pin both state goldens"
```

### Task 8: PHASE-0 GATE — full suite + clippy + determinism unchanged with no economy

- [ ] **Step 1: Run the whole core suite**

Run: `cargo test -p jumpgate-core`
Expected: PASS (all pre-existing tests + new economy/store/hash/config tests).

- [ ] **Step 2: Clippy clean (incl. test modules)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Confirm an economy-free world still replays bit-identically**

Add a test in `hash.rs` or a determinism test module: reset two worlds from an economy-free config, step both 20 ticks with empty commands, assert equal `state_hash` each tick. (Proves the bump did not introduce nondeterminism for the legacy path.) Run it; expected PASS. Commit.

```bash
git add -A && git commit -m "test(economy): phase-0 gate — suite+clippy green, economy-free replay stable"
```

---

# PHASE 1 — STAGE 1: the closed loop at a FIXED price

Goal: producers fire, a corp posts a delivery contract, a scripted hauler accepts it, loads, flies to the destination station-body, docks via rendezvous arrival, delivers, and is paid from escrow — with the resource accounting identity and global credit identity holding every tick. Price is frozen.

### Task 9: new `EventKind` variants (hash-neutral)

**Files:** Modify `crates/jumpgate-core/src/contract.rs`

- [ ] **Step 1: Failing test** (append to `contract.rs` tests): construct each new `EventKind` variant and assert `Copy` + `PartialEq` (mirror `enums_round_trip_via_partial_eq`). Reference ids from `crate::ids`.
- [ ] **Step 2: Run** `cargo test -p jumpgate-core contract::tests` → FAIL (variants missing).
- [ ] **Step 3: Implement** — add to `enum EventKind` (all `Copy`, ids + scalars only — NO `Vec`/`String`/`NavDest` payloads):

```rust
    Production { producer: ProducerId, resource: Resource, qty: u32 },
    Trade { station: StationId, resource: Resource, qty: u32, price_micros: i64 },
    PriceUpdate { station: StationId, resource: Resource, price_micros: i64 },
    ContractOffered { contract: ContractId },
    ContractAccepted { contract: ContractId, hauler: CraftId },
    ContractFulfilled { contract: ContractId, hauler: CraftId },
```
Add imports: `use crate::economy::Resource; use crate::ids::{ContractId, CorporationId, ProducerId, StationId};`. These are NOT in `HASH_FIELD_ORDER` (events are a stream), so no golden moves.
- [ ] **Step 4: Run** `cargo test -p jumpgate-core contract::tests` → PASS.
- [ ] **Step 5: Commit** `feat(economy): EventKind variants (Production/Trade/PriceUpdate/Contract*)`.

### Task 10: new `CommandKind` variants + ingestion

**Files:** Modify `crates/jumpgate-core/src/types.rs`, `crates/jumpgate-core/src/ingest.rs`

- [ ] **Step 1: Failing test** — in `ingest.rs` tests, issue an `AcceptContract` command targeting a craft and assert it is logged + an `ActionIngested` emitted (mirror the existing destination-ingest test).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — add to `enum CommandKind`: `AcceptContract { contract: crate::ids::ContractId }` and `SetRole { role: crate::stores::CraftRole }`. In `ingest_commands`, handle the new kinds: `AcceptContract` resolves to setting the craft's `contract` column + role Hauler (validate the contract is `Offered` and unassigned; deterministic skip otherwise) — but defer the actual contract state transition to the `resolve_contracts` stage; here just record intent by setting `ships.contract[i]`/`role[i]`. `command_sort_key` already orders these (target-based). Keep the single ingestion path.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): AcceptContract/SetRole commands through the single ingest path`.

### Task 11: `run_producers` stage

**Files:** Modify `crates/jumpgate-core/src/economy.rs` (system fn), `crates/jumpgate-core/src/world.rs` (call it + audited counters)

- [ ] **Step 1: Failing test** (in `economy.rs`): build a `StationStore` with one station, a `ProducerStore` with a miner `Recipe{input:None, output:Some((Ore,5)), interval:1}`; call `run_producers(&mut stations, &producers, &mut counters, tick, &mut events)`; assert station Ore stock rose by 5 and `counters.mined[Ore]==5` and a `Production` event was emitted. Add a refine case (Ore→Fuel) and an all-or-nothing skip case (insufficient input → no change).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** `run_producers` — iterate producers in sorted `ProducerId` order; fire if `(tick - last_fired) >= interval`; all-or-nothing: if `input` present and station stock < qty, skip; else apply `stock -= input`, `stock += output`, update `last_fired`, bump `counters.mined`/`counters.consumed` for source/sink legs, emit `Production`. Define `struct EconCounters { pub mined: [i64; N_RESOURCES], pub consumed: [i64; N_RESOURCES] }` (hashed? NO — derived audit counters; keep OUT of state_hash but document as transitively pinned by stock+events. Alternatively fold them — simplest correct choice: DO fold them, they are mutable state. Decision: fold them in Task 7's word list — if you reached here without them folded, add them now with a follow-up single-cause version note. Prefer folding.)

> Determinism note: counters MUST be hashed OR provably derived. Simplest: hash them. If Task 7 didn't include them, this task triggers a second (small, named) `HASH_FORMAT_VERSION` note — avoid by including `EconCounters` in Task 7's fold from the start.

- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): run_producers (all-or-nothing firing) + audited mined/consumed counters`.

### Task 12: wire economy stores into `World` + mint from config at `reset`

**Files:** Modify `crates/jumpgate-core/src/world.rs`

- [ ] **Step 1: Failing test** — reset a world from a config with 2 stations + 1 miner; assert `world` exposes them (via a test accessor or `pub(crate)` reads): station count == 2, producer count == 1, counters zero.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — add `pub(crate) stations: StationStore`, `producers: ProducerStore`, `corporations: CorporationStore`, `contracts: ContractStore`, `econ: EconCounters` to `World` (if not added in Task 7). In `reset`, after spawning bodies, mint stations (resolve `body_index` → the minted `BodyId`), producers (resolve `station_index`), corporations, contracts (status Offered) from the `RunConfig` init vecs. Validate indices; an out-of-range index → a new `ResetError::BadEconomyRef { .. }` arm before tick 0.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): mint economy stores from RunConfig at reset (+BadEconomyRef guard)`.

### Task 13: insert `run_producers` into `World::step`

**Files:** Modify `crates/jumpgate-core/src/world.rs`

- [ ] **Step 1: Failing test** — reset with a miner (interval 1), step 3 ticks, assert station Ore stock == 15 and `econ.mined[Ore]==15`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — in `step`, immediately AFTER `ingest_commands` (world.rs:245) and BEFORE the physics loop, call `economy::run_producers(&mut self.stations, &self.producers, &mut self.econ, next, &mut self.events)`. Use `next` as the firing tick (consistent with event tick stamping).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): run_producers stage in World::step`.

### Task 14: contract accept (scripted) + load at origin

**Files:** Modify `crates/jumpgate-core/src/economy.rs`, `world.rs`

- [ ] **Step 1: Failing test** — one Offered contract (move 5 Fuel A→B, reward 1_000_000µ), one corp with treasury, one Idle craft at station A's body. Issue `AcceptContract`. Step. Assert: contract status `Accepted`→(after load)`CargoLoaded`, `escrow_micros == reward`, corp treasury debited by reward, craft `cargo == Some((Fuel,5))`, station A Fuel stock dropped by 5, craft nav now Seeking station-B-body.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** a `resolve_contracts`-pre stage (or fold into one `step_contracts`): on a craft whose `contract` is set and status `Offered`: transition `Offered→Accepted` (escrow: debit corp treasury by reward into `escrow_micros`; if treasury insufficient → revert assignment, status stays Offered, deterministic), set `hauler`, emit `ContractAccepted`. Then `Accepted→CargoLoaded`: if craft is at `from_station` body (use `arrival`/proximity or simply require co-location at accept for v1) and station has stock: move `qty` from station stock into craft `cargo`, set craft nav to Seek `to_station`'s body via `set_nav(Seeking{dest: NavDest::Entity(Body(to_body)), dv_remaining})`, status `CargoLoaded→InTransit`. (Keep transitions in sorted ContractId order.)
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): contract accept + escrow + load cargo + dispatch hauler`.

### Task 15: deliver on Arrival + settle escrow

**Files:** Modify `crates/jumpgate-core/src/world.rs`, `economy.rs`

- [ ] **Step 1: Failing test** — continue the Task-14 fixture; step until the hauler reaches station B (rendezvous Arrival event for that craft fires). Assert: cargo unloaded into station B stock (+5 Fuel), contract `Delivered→Completed`, `escrow_micros` paid out to craft `credits_micros` (escrow→0), `ContractFulfilled` emitted, and the global credit identity holds (`Σtreasury+Σcredits+Σescrow == initial`).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — after `detect_boundary_events` (world.rs:336), scan the just-detected events for `Arrival { craft, .. }`; for a craft with an `InTransit` contract whose `to_station` body matches the arrival dest: unload `cargo` into `to_station` stock, status `InTransit→Delivered→Completed`, pay `escrow_micros` → craft `credits_micros`, zero escrow, clear craft `cargo`/`contract`/role, emit `ContractFulfilled`. Resolve in sorted ContractId order for determinism.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): deliver-on-arrival + escrow settlement (credit identity holds)`.

### Task 16: failure path — out of fuel mid-contract → `Failed`, escrow returns

**Files:** Modify `crates/jumpgate-core/src/world.rs`, `economy.rs`

- [ ] **Step 1: Failing test** — a hauler with too little `fuel_mass` to reach B; step until `FuelEmpty` fires while `InTransit`. Assert: contract `→Failed`, `escrow_micros` returned to corp treasury (escrow→0), craft cargo retained or dumped per rule (v1: cargo lost — document), role cleared; credit identity still holds.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — on a `FuelEmpty { craft }` event for a craft with an `InTransit` contract: status `→Failed`, refund `escrow_micros`→corp, zero escrow, clear contract handle. (v1: cargo is lost — note in the resource accounting test: cargo loss is NOT a leak because in-transit cargo was already debited from origin stock and is tracked in `in_transit_cargo`; on Failed it leaves the accounting via an explicit `consumed`-equivalent loss counter, OR keep it as a Failed-cargo audit term. Simplest: on Failed, add lost cargo to `econ.consumed[r]` so the identity stays exact.)
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): contract Failed path on FuelEmpty — escrow refund, accounted cargo loss`.

### Task 17: scripted dispatch + repost (Stage-1 loop closure)

**Files:** Modify `crates/jumpgate-core/src/economy.rs`, `world.rs`

- [ ] **Step 1: Failing test** — a full fixture (miner→Ore, refiner Ore→Fuel at station A, demand sink consuming Fuel at station B, a corp, an Idle hauler). Step ~N ticks with NO external commands. Assert the loop self-runs: at least one contract reaches `Completed`, station B Fuel stock saw deliveries, and the resource accounting identity holds every tick.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — a deterministic scripted policy stage (step (7)): if a corp has demand at a station (sink stock below a threshold) and no open contract for it, `ContractStore::push` an Offered contract (emit `ContractOffered`); for each Idle hauler, the scripted policy emits an `AcceptContract` for the lowest-`ContractId` Offered contract through the ingest path (resolved next tick — one path). Keep all selection in sorted-id order (no RNG).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): scripted dispatch + repost — Stage-1 loop self-runs`.

### Task 18: PHASE-1 GATE — accounting identity + determinism + clippy

- [ ] **Step 1: Accounting-identity test** — a helper `assert_resource_identity(world)` checking, per resource, `Σstation.stock + Σin_transit_cargo == initial + econ.mined - econ.consumed`. Call it every tick over a 200-tick run of the full Stage-1 fixture. Run → PASS.
- [ ] **Step 2: Determinism test** (per `digest-tests-are-determinism-not-golden`) — two worlds from the same Stage-1 config + same (empty) inputs produce bit-identical `state_hash` sequences over 200 ticks. Run → PASS.
- [ ] **Step 3:** `cargo test -p jumpgate-core` and `cargo clippy --all-targets -- -D warnings` → both green.
- [ ] **Step 4: Commit** `test(economy): phase-1 gate — accounting identity + replay determinism green`.

---

# PHASE 2 — STAGE 2: demand-deflation pricing (close the price loop, keep it stable)

Goal: turn on `update_prices` so price falls as stock rises; close the homeostatic cycle without a limit cycle.

### Task 19: `update_prices` stage (linear deflation) + `to_micros` boundary

**Files:** Modify `crates/jumpgate-core/src/economy.rs`, `world.rs`

- [ ] **Step 1: Failing test** — a station with Fuel stock at 0 → price == `base*2`; at `cap` → price == `base*(2 - slope)`; monotone decreasing in stock. Use integer math: `price_micros = base_micros * (2000 - min(stock,cap)*slope_milli/cap) / 1000` (clamp ≥ 0). Assert exact integer values for a few points.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** `update_prices(&mut stations, &price_cfg, tick, &mut events)` — for each station/resource, recompute integer `price_micros` from stock via the linear rule; emit `PriceUpdate` only when the value changes. The single float↔int boundary helper `to_micros` lives here (used only if any config curve is expressed in float; prefer pure-integer). Deterministic, sorted-id order.
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): update_prices linear deflation (integer micro-price)`.

### Task 20: invoke `update_prices` from the step path on a tick-gated clock

**Files:** Modify `crates/jumpgate-core/src/world.rs`, `config.rs` (a `reprice_interval` in `PriceCfg`)

- [ ] **Step 1: Failing test** — with `reprice_interval = 4`, prices update only on ticks that are multiples of 4; a determinism test confirms the schedule is in the recorded run (same config → same price sequence).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** — add `reprice_interval: u32` to `PriceCfg` (fold into config_hash — re-pin config golden again ONLY if you add it after Task 5; prefer adding it in Task 5 to avoid a second re-pin). In `step`, after `resolve_contracts` and before `copy-forward`, call `update_prices` when `next.0 % reprice_interval == 0`. (NOT lazily on read — the archived open-loop bug.)
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): tick-gated reprice clock invoked from World::step`.

### Task 21: hysteresis deadband + staggered dispatch (stability)

**Files:** Modify `crates/jumpgate-core/src/economy.rs`

- [ ] **Step 1: Failing test** — drive the closed Stage-2 loop 1000 ticks; assert price/stock for the traded resource settle into a bounded band (max-min over the last 200 ticks below a threshold) rather than a growing oscillation. Also assert dispatch does not fire all haulers on the same tick (staggered).
- [ ] **Step 2: Run** → FAIL (raw closed loop oscillates).
- [ ] **Step 3: Implement** — (a) hysteresis: only re-post/re-price-trigger contracts when stock crosses a deadband around the demand threshold (two thresholds, low/high), not a single point; (b) staggered dispatch: a deterministic per-hauler phase offset (e.g. `hauler.slot % stagger_period`) so accepts spread across ticks. Constants in `PriceCfg`/a new `DispatchCfg` (config-hashed; add in Task 5 if possible).
- [ ] **Step 4: Run** → PASS.
- [ ] **Step 5: Commit** `feat(economy): hysteresis deadband + staggered dispatch — Stage-2 stable`.

### Task 22: PHASE-2 GATE — full harness

- [ ] **Step 1:** Re-run the resource accounting identity + global credit identity over a 1000-tick Stage-2 run → PASS.
- [ ] **Step 2:** Replay determinism over 1000 ticks (bit-identical `state_hash` sequence, two worlds) → PASS.
- [ ] **Step 3:** Stability regression test (Task 21) green; `cargo test -p jumpgate-core`; `cargo clippy --all-targets -- -D warnings` → all green.
- [ ] **Step 4: Commit** `test(economy): phase-2 gate — full demand-deflation harness conserves + replays + stable`.
- [ ] **Step 5: Close the issue**

```bash
filigree close jumpgate-fe825a65f3 --actor <name>
```

---

## Self-review (author)

**Spec coverage:** producers/recipes (T3,11), station market stock+price (T3,19), corporation+treasury (T4,12), delivery contract lifecycle+escrow (T4,14,15,16), hauler role+cargo via live rendezvous arrival (T6,14,15), Stage 1 fixed price (T9–18), Stage 2 demand deflation + reprice clock + hysteresis/staggered dispatch (T19–22), the HASH_FORMAT_VERSION 1→2 single dual-golden move (T7), config economy fold + 0x278c re-pin (T5), integer microcredits (throughout), resource accounting identity + credit identity gates (T18,T22). All spec sections map to a task.

**Determinism budget:** ONE config re-pin (T5) — add `reprice_interval`/dispatch constants in T5 to avoid a second; ONE `HASH_FORMAT_VERSION` bump folding ALL economy state incl. `EconCounters` (T7 — include counters from the start to avoid a second bump in T11). EventKind/CommandKind additions hash-neutral (T9,T10). These are the only golden moves; each is single-cause.

**Open risk to watch at execution:** the exact economy fold ORDER must be identical in `state_hash` and `recompute_with_cursors` (T7) — they are the parity pair; the `cursor_participates_in_state_hash` test is the guard. Reality-check every `world.rs`/`hash.rs` line number at execution (they may have shifted); rely on the compiler + the parity test, not the cited line numbers.
