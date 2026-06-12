# OD-1 Refactor Surface — Resource/N_RESOURCES Grounding Extract

**Date grounded:** 2026-06-13  
**Branch:** `jumpgate-v1-design` (HEAD b446095)  
**Beat:** Every site that touches `Resource` / `N_RESOURCES` — checklist for the A1 runtime-goods commit

---

## 1. The `Resource` enum + `N_RESOURCES` + `index()` / `ALL`

`crates/jumpgate-core/src/economy.rs:8-24`

```rust
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

This is the ONLY exhaustive match on `Resource` variants in production code.  
`Resource::ALL` is used at `economy.rs:325` inside `update_prices` to construct `PriceUpdate` events:
```rust
kind: EventKind::PriceUpdate {
    station,
    resource: Resource::ALL[r],   // economy.rs:325
    price_micros: p,
},
```

---

## 2. Per-resource arrays in stores

### 2a. `StationStore` stock and price_micros

`economy.rs:42-47`

```rust
pub struct StationStore {
    pub ids: SlotMap<()>,
    pub body: Vec<BodyId>,
    pub stock: Vec<[i64; N_RESOURCES]>,         // economy.rs:45
    pub price_micros: Vec<[i64; N_RESOURCES]>,   // economy.rs:46
}
```

`StationStore::push` receives `stock: [i64; N_RESOURCES]` and `price_micros: [i64; N_RESOURCES]` (`economy.rs:62-63`). The slot == row invariant holds (never mid-run despawn).

### 2b. `EconCounters` mined/consumed

`economy.rs:215-218`

```rust
pub struct EconCounters {
    pub mined: [i64; N_RESOURCES],       // economy.rs:216
    pub consumed: [i64; N_RESOURCES],    // economy.rs:217
}
impl EconCounters {
    pub fn zero() -> Self {
        EconCounters { mined: [0; N_RESOURCES], consumed: [0; N_RESOURCES] }  // economy.rs:223
    }
}
```

These are HASHED (folded as words 20 in `write_economy_stores`, `hash.rs:400-404`).

### 2c. `PriceCfg` base_micros / cap arrays

`config.rs:143-145`

```rust
pub struct PriceCfg {
    pub base_micros: [i64; crate::economy::N_RESOURCES],   // config.rs:144
    pub cap: [i64; crate::economy::N_RESOURCES],            // config.rs:145
    pub slope_milli: i64,
    pub reprice_interval: u32,
}
```

Default initializes with `[0; N_RESOURCES]` and `[1; N_RESOURCES]` (`config.rs:155-156`).

### 2d. `StationInit` initial_stock / initial_price_micros

`config.rs:101-108`

```rust
pub struct StationInit {
    pub body_index: usize,
    pub initial_stock: [i64; crate::economy::N_RESOURCES],          // config.rs:103
    pub initial_price_micros: [i64; crate::economy::N_RESOURCES],   // config.rs:104
    pub sells_upgrades: bool,
}
```

### 2e. `CraftStore.cargo` — Resource carried on a craft

`stores.rs:177-179`

```rust
/// Loaded cargo: `Some((resource, qty))` while carrying a delivery, else `None`.
/// Distinct from `fuel_mass` (propellant) — traded Fuel is cargo in v1.
pub cargo: Vec<Option<(crate::economy::Resource, u32)>>,
```

Initialized `None` at `stores.rs:243` / `world.rs:310`. This is the only "in-transit" tracker for the resource identity check (see §8 below). After A2, a second column `hold: Vec<Vec<(Good, u32)>>` is added; `assert_resource_identity` must be updated to sum both.

---

## 3. Recipe — input/output (Resource, qty) pairs

`economy.rs:33-38`

```rust
pub struct Recipe {
    pub input: Option<(Resource, u32)>,
    pub output: Option<(Resource, u32)>,
    pub interval: u32,
}
```

Used extensively in `ProducerInit` (`config.rs:113-115`) and in `run_producers` (`economy.rs:263-271`). Resource is accessed via `.index()` at:
- `economy.rs:264`: `stations.stock[srow][r_in.index()] < q as i64`
- `economy.rs:270`: `stations.stock[srow][r_in.index()] -= q as i64`
- `economy.rs:271`: `counters.consumed[r_in.index()] += q as i64`
- `economy.rs:274`: `stations.stock[srow][r_out.index()] += q as i64`
- `economy.rs:275`: `counters.mined[r_out.index()] += q as i64`

### Recipe folding in hashes

State hash (`hash.rs:372-390`, `write_recipe_hash`):
```rust
fn write_recipe_hash(h: &mut FnvHasher, r: &crate::economy::Recipe) {
    match r.input {
        None => h.write_u64(0),
        Some((res, qty)) => { h.write_u64(1); h.write_u64(res.index() as u64); h.write_u64(qty as u64); }
    }
    match r.output { ... }
    h.write_u64(r.interval as u64);
}
```

Config hash (`config.rs:757-777`, `write_recipe`): identical structure, uses `ConfigFnv` hasher.

---

## 4. `ContractStore` — `resource: Vec<Resource>` column

`economy.rs:160-167`

```rust
pub struct ContractStore {
    pub ids: SlotMap<()>,
    pub status: Vec<ContractStatus>,
    pub corp: Vec<CorporationId>,
    pub resource: Vec<Resource>,   // economy.rs:162
    pub qty: Vec<u32>,
    ...
}
```

Hashed via `resource.index()` at `hash.rs:456`:
```rust
h.write_u64(world.contracts.resource[i].index() as u64);
```

Config hash via `k.resource.index() as u64` at `config.rs:622`.

`ContractInit.resource: crate::economy::Resource` at `config.rs:130`.

---

## 5. `EventKind` variants carrying `Resource`

`contract.rs:70-84`:
- `Production { producer, resource: Resource, qty }` — emitted by `run_producers`
- `Trade { station, resource: Resource, qty, price_micros }` — DEAD CODE in v1; the sole constructor is at `contract.rs:417` (test only). The spec recommended cut §1.2 says "DELETES the dead `EventKind::Trade`" in the same commit that adds TradeBought/TradeSold.
- `PriceUpdate { station, resource: Resource, price_micros }` — emitted by `update_prices`

These variants are NOT hashed (event stream is unhashed). But `chronicle_subject` in `trophic_run.rs:481-511` uses a catch-all `_ => None` arm today. The recommended cut says this wildcard is DELIBERATELY REVERSED to exhaustive match when new event kinds land (recommended cut Part 3 / Chronicle section).

---

## 6. State-hash fold of every per-resource array

### 6a. `write_economy_stores` — hash.rs:397-473 (words 20-24)

Word 20: `EconCounters` (2 mined + 2 consumed = 4 words total, loop `0..N_RESOURCES`):
```rust
for r in 0..N_RESOURCES { h.write_u64(world.econ.mined[r] as u64); }
for r in 0..N_RESOURCES { h.write_u64(world.econ.consumed[r] as u64); }
```
`hash.rs:400-404`

Word 21: stations — for each station sorted by id: `slot, gen, body(slot,gen)`, then `for r in 0..N_RESOURCES { h.write_u64(stock[i][r]); h.write_u64(price_micros[i][r]); }`  
`hash.rs:416-419`

Word 22: producers — each folds its recipe via `write_recipe_hash` (discriminant + res.index() + qty, both arms). `hash.rs:431`.

Word 24: contracts — each folds `resource.index()` at `hash.rs:456`.

### 6b. `write_craft_economy` — hash.rs:326-368 (word 17, cargo)

```rust
match world.ships.cargo[idx] {  // hash.rs:328-335 — word 17
    None => h.write_u64(0),
    Some((res, qty)) => {
        h.write_u64(1);
        h.write_u64(res.index() as u64);
        h.write_u64(qty as u64);
    }
}
```

**No N_RESOURCES count word is folded anywhere in the state hash.** The loops are `for r in 0..N_RESOURCES` — they write exactly `N_RESOURCES` words, and the count is NOT separately emitted. This is the load-bearing fact for hash-neutrality: changing `N_RESOURCES` from 2 to N expands the loop and moves all subsequent words, which changes the hash. The A1 commit MUST convert to Vecs (so the fold iterates `0..n_goods` where `n_goods` is config-derived) and write the COUNT as a leading delimiter word — this is how the spec says "folds write exactly n_goods words (byte-identical at 2)" while the eventual `GoodsCfg` fold "writes the COUNT first." These are two different folds: the GoodsCfg config fold gets a count; the existing per-resource state folds currently have no count (they're bare `for r in 0..N_RESOURCES` loops). For byte-identical hash at n=2: the Vec loop must produce the same byte sequence as the array loop, which it will as long as the ITERATION ORDER is identical (ascending index 0, 1, …, n-1) with no leading count word added to the existing state-hash loops.

**CONFIRMED: the config hash ALSO lacks a count word for per-resource arrays.** `config.rs:607-611` and `config.rs:630-633` — these are bare `for r in 0..N_RESOURCES` loops without a preceding count word. If adding a count word to the config fold, that moves the hash and triggers a GOLDEN_CONFIG_HASH re-pin (intentional for A3).

---

## 7. `config_hash` fold of per-resource arrays

`config.rs:597-636` (the economy tail, append-only, words 14-20):

```rust
h.write_u64(stations.len() as u64);   // config.rs:601
...
for s in stations {
    h.write_u64(s.body_index as u64);
    for r in 0..crate::economy::N_RESOURCES {
        h.write_u64(s.initial_stock[r] as u64);       // config.rs:607-610
        h.write_u64(s.initial_price_micros[r] as u64);
    }
}
...
for k in contracts {
    h.write_u64(k.resource.index() as u64);  // config.rs:622
    ...
}
h.write_u64(price_cfg.slope_milli as u64);
h.write_u64(price_cfg.reprice_interval as u64);
for r in 0..crate::economy::N_RESOURCES {
    h.write_u64(price_cfg.base_micros[r] as u64);   // config.rs:630-633
    h.write_u64(price_cfg.cap[r] as u64);
}
```

The current `GOLDEN_CONFIG_HASH` pinned at `config.rs:783` is `0x128c_1299_5c48_4fdc` (re-pinned at world-gets-big §5). The A1 commit is hash-neutral (no config change); A3 gets ONE re-pin.

---

## 8. `assert_resource_identity` and in-transit accounting

`world.rs:2870-2886` (test helper used in `phase1_gate_resource_accounting_identity_holds_every_tick` and in `scripted_dispatch_makes_stage1_loop_self_run`):

```rust
fn assert_resource_identity(world: &World, initial: &[i64; crate::economy::N_RESOURCES]) {
    for r in 0..crate::economy::N_RESOURCES {
        let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
        let in_transit: i64 = world
            .ships
            .cargo
            .iter()
            .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
            .sum();
        let lhs = stock + in_transit;
        let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
        assert_eq!(lhs, rhs, ...);
    }
}
```

**After A2 (format v6, `hold` column added):** the recommended cut says: "`assert_resource_identity`'s in-transit term gains the hold sum (today it iterates `ships.cargo` only, world.rs:2870-2884) **in this commit**." The `hold` column contains own-cargo trader inventory — it participates in the identity. This is a same-commit obligation in A2, not A1.

---

## 9. `resolve_refuels` — hardcoded `Resource::Fuel.index()`

`economy.rs:1013`:
```rust
let fuel_r = Resource::Fuel.index();
```
Used at `economy.rs:1023`, `economy.rs:1027`, `economy.rs:1051`, `economy.rs:1052`. After A1, `fuel_r` becomes `Good::FUEL.index()` or the named constant index. This is a call-site that will NOT compile after the newtype change if `Resource::Fuel` is removed.

Similar pinned index at `economy.rs:1261` (`docked_station_row` caller — World::reset refuel validation at `world.rs:229`).

---

## 10. `diagnostics.rs` — hardcoded `Resource::Fuel.index()` in `sample_trophic`

`diagnostics.rs:805-815`:
```rust
per_station_fuel_stock: world
    .stations
    .stock
    .iter()
    .map(|st| st[Resource::Fuel.index()])   // diagnostics.rs:809
    .collect(),
per_station_fuel_price: world
    .stations
    .price_micros
    .iter()
    .map(|pr| pr[Resource::Fuel.index()])   // diagnostics.rs:815
    .collect(),
```

These fields remain as Fuel-specific reads (they are existing instruments; the A0 group adds `per_station_stock`/`per_station_price` flat matrices as additive keys, the recommended cut §1.1). So these lines must be updated to use the Good-indexed equivalent post-A1.

---

## 11. `scenario.rs` — `stock()` helper and all scenario Resource references

**`scenario_trophic`** (`scenario.rs:197-201`):
```rust
let stock = |ore: i64, fuel: i64| -> [i64; crate::economy::N_RESOURCES] {
    let mut s = [0i64; crate::economy::N_RESOURCES];
    s[Resource::Ore.index()] = ore;
    s[Resource::Fuel.index()] = fuel;
    s
};
```
Used to fill `initial_stock` on all 6 StationInits at lines 204-209.  
Initial price array is `[0, 0]` (literal) at lines 204-209 — these are bare array literals that will need to change to Vec or `[0; n_goods]` after A1.

**`scenario_frontier`** (`scenario.rs:396-401`): identical `stock()` helper, same pattern. The `initial_price_micros` uses `[0, fuel_price(fuel)]` (two-element literal, `scenario.rs:412`). After A1, this becomes a Vec of length n_goods with FUEL index set.

`scenario_trophic` Producers: 9 `Recipe` instantiations using `Resource::Ore` and `Resource::Fuel` directly (lines 213-223, 436-450).  
`scenario_trophic`/`scenario_frontier` ContractInits: multiple `resource: Resource::Ore` and `resource: Resource::Fuel` fields at lines 245, 254, 472, 481 (trophic) and 472+ (frontier).

---

## 12. `apply_knob` arms touching resources

`scenario.rs:550-634`. The current `apply_knob` has NO arms for per-resource config values — `demand_low`, `demand_high`, etc. are not resource-indexed knobs. After A1, if a per-good pricing knob is needed it would extend this function. **No existing arm currently needs updating for A1.**

---

## 13. `trophic_run.rs` — JSONL emission of per-resource fields

Per-window JSONL keys (lines 296-304):
```rust
"per_station_lurking_pirates": s.per_station_lurking_pirates,
"per_station_fuel_stock": s.per_station_fuel_stock,    // trophic_run.rs:299
"per_station_fuel_price": s.per_station_fuel_price,   // trophic_run.rs:300
"refuels": s.refuels,
```
These keys STAY byte-identical (existing instruments). The A0 group adds NEW additive keys `per_station_stock` / `per_station_price` as flat matrices alongside them — the recommended cut §1.1: "existing `per_station_fuel_stock/price` stay byte-identical forever."

`gossip_log_event_json` at `trophic_run.rs:384-450`: wildcard `_ => None` at line 448. The recommended cut says the A0 commit reverses this to an exhaustive match for new events. The `ContractAccepted` arm at line 421-425 uses `route_of()` not a Resource field — unaffected. No resource fields appear in the current gossip log JSONL.

`chronicle_subject` at `trophic_run.rs:481-511`: wildcard `_ => None` at line 509. Same reversal scheduled.

---

## 14. `jumpgate-py/src/env.rs` — hardcoded array literals

`env.rs:403-406`:
```rust
initial_stock: [0, 0],
initial_price_micros: [0, 0],
```
All StationInits in the Python env use literal two-element arrays. These must change to Vecs/arrays of length n_goods after A1.

Recipe `Resource::Ore` and `Resource::Fuel` direct uses at lines 412, 416, 422, 426.  
`ContractInit` `resource: Resource::Ore` at lines 459-462.

---

## 15. Hash-field ordering and fold order that MUST be preserved

The fold order in `write_economy_stores` (hash.rs:397-473) is the canonical word order and **must not change**:

1. `mined[0]`, `mined[1]` (then `consumed[0]`, `consumed[1]`) — words 20
2. stations cursor, then per-station sorted by id: `slot, gen, body(slot,gen)`, then per-resource `(stock[r], price_micros[r])` for r=0..N — word 21
3. producers cursor, then per-producer sorted by id: `slot, gen, station(slot,gen)`, recipe — word 22
4. corporations cursor, then per-corp: `slot, gen, treasury_micros, home_station(slot,gen)` — word 23
5. contracts cursor, then per-contract: `slot, gen, status.rank(), corp(slot,gen), resource.index(), qty, from(slot,gen), to(slot,gen), reward_micros, escrow_micros, hauler(0|1,slot,gen)` — word 24

In `write_craft_economy` (hash.rs:326-368):
- Word 17: cargo None→0 | Some→(1, res.index(), qty)

**CRITICAL:** the per-resource sub-loop order within each station row is `r=0, r=1` (ascending index). This must be preserved exactly after A1 converts arrays to Vecs. If n_goods stays 2, the 4 words emitted per station (stock[0], price[0], stock[1], price[1]) are identical to the current array-index form.

**No count word** is written before the per-resource sub-loop in any state-hash fold. The A1 commit must NOT add one — that would break the hash-neutrality proof. The GoodsCfg CONFIG fold (A3) does get a count word, but that is a CONFIG hash change, not a STATE hash change.

---

## 16. Tests that reference Resource directly (will need pin/update)

Tests currently using `[i64; N_RESOURCES]` or `Resource::*`:

| Test | File:line | Nature |
|------|-----------|--------|
| `phase1_gate_resource_accounting_identity_holds_every_tick` | world.rs:2888 | uses `[0i64; N_RESOURCES]`, iterates `0..N_RESOURCES`, asserts `Resource::Ore/Fuel.index()` |
| `scripted_dispatch_makes_stage1_loop_self_run` | world.rs:2591 | inline `check_identity` with `0..N_RESOURCES` loop; `world.stations.stock[1][fuel]` direct index |
| `phase1_gate_replay_is_deterministic_state_hash_tick_by_tick` | world.rs:2931 | golden hash (no Resource directly, but behavior depends on fold) |
| `deliver_on_arrival_settles_escrow_and_holds_credit_identity` | world.rs:2658 | `Resource::Fuel.index()` direct |
| tests in `pirate.rs` | pirate.rs:818-1537 | multiple `Resource::Fuel/Ore` direct |
| `golden_trajectory` in commons-cut | crates/jumpgate-commons-cut/tests/golden_trajectory.rs | uses scenario_trophic; replay hash sensitivity |
| `replay_equivalence` | crates/jumpgate-core/tests/replay_equivalence.rs | state-hash identity |
| hash tests in `hash.rs` | hash.rs:544+ | `cargo resource` mutation test |

The recommended cut says "pinned-index tests per scenario lineage replace enum exhaustiveness" — i.e., after A1, tests no longer call `Resource::Ore.index()` as a constant but instead use the Good const indices (e.g., `Good::ORE` as a newtype, or the pinned constants `ORE = Good(0)`, `FUEL = Good(1)`).

---

## 17. `EventKind::Trade` — dead code, confirmed

`contract.rs:75-80` defines the variant. Only constructor: `contract.rs:417` (in a test). `trophic_run.rs` gossip log does NOT emit it. Recommended cut says DELETE it in the same commit that adds `TradeBought`/`TradeSold` (Part A §1.2). This is a breaking API change — any external code matching `EventKind` exhaustively would break, but there is none (the match in `trophic_run.rs:384` uses a wildcard for unrecognized events today).

---

## GOTCHAS

1. **No count word in state-hash per-resource loops — do NOT add one.** All `for r in 0..N_RESOURCES` loops in `write_economy_stores` and `write_craft_economy` emit bare words with no preceding length word. Adding a count word silently changes the hash and breaks the hash-neutrality proof. The GoodsCfg CONFIG fold is different — it SHOULD get a count (and will, in A3, causing the intentional GOLDEN_CONFIG_HASH re-pin).

2. **`Resource::ALL` is used to construct `PriceUpdate` events** (`economy.rs:325`). After the `Good(u16)` newtype, this becomes `Good::ALL[r]` (or iteration over a `GoodsCfg.goods` slice). Forgetting this leaves `PriceUpdate` events carrying a phantom `Resource` type while everything else uses `Good`.

3. **`assert_resource_identity` iterates `ships.cargo` only for in-transit goods** (`world.rs:2877`). After A2 adds the `hold` column for own-cargo traders, the identity breaks unless the hold sum is also included. This is a same-commit obligation in A2, not deferred.

4. **`initial_price_micros` in `scenario_trophic` uses two-element literal `[0, 0]` arrays** (`scenario.rs:204-209`) and `scenario_frontier` uses `[0, fuel_price(fuel)]` (`scenario.rs:412`). Both scenarios also construct `stock` helpers using `[0i64; N_RESOURCES]` fixed-size arrays. After A1 these become Vec-backed, which means the `stock()` helper changes signature. The existing scenario tests (`scenario_trophic_shape`, `scenario_frontier_shape`) use the seeded stocks by index — these tests need the new per-Good index constants, not `Resource::Ore.index()`.

5. **`resolve_refuels` hardcodes `Resource::Fuel.index()`** (`economy.rs:1013`) as `fuel_r` and uses it to index into `stations.stock[srow][fuel_r]` and `stations.price_micros[srow][fuel_r]`. World::reset also hardcodes `Resource::Fuel.index()` for the refuel validation check (`world.rs:229`). After A1, these become `Good::FUEL.index()` (the named constant). If the named constant is not declared or is declared at a different index, the refuel machinery silently reads the wrong column. Pin `Good::FUEL = Good(1)` explicitly and verify it matches the GoodsCfg order.
