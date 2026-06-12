# Phase A3 — Boards, the Exchange, and the own-trade verbs

> **Scope:** per-good live pricing generalization; GoodsCfg + ArbitrageCfg + ExchangeCfg config commit
> (the one rung-A GOLDEN_CONFIG_HASH re-pin); TradeBuy/TradeSell pending columns + scripted policy
> intent stage + settle stage; TradeBought/TradeSold events + chronicle arms + gossip-log rows;
> deletion of the dead EventKind::Trade; deterministic no-op skip arms enumerated; credit-identity
> test extended with Exchange legs.
>
> **Prerequisite:** A2 landed (hold column in state, v6 bump complete, assert_resource_identity
> includes hold sum). A3 builds only config + economy behavior on top of that stable substrate.
>
> **GOLDEN discipline:** this phase contains exactly ONE GOLDEN_CONFIG_HASH re-pin (A3.2 config
> commit). No GOLDEN_STATE_HASH re-pin — events are unhashed, and new config fields do not touch
> the state encoding. The behavior digest baseline was pinned at the last A0 commit; any state-hash
> drift here is a bug.

---

### Task A3.1: per-good live pricing generalization

Generalizes `update_prices` and `PriceUpdate` events to emit a `Good`-indexed
`resource` field after A1 has made all resource arrays Vec-backed. After A1,
`Resource::ALL` is replaced by iteration over `GoodsCfg.goods` and a `Good(u16)`
constant. This task lands the A1 → A3 bridge: a `Good::ALL` slice accessor and the
`update_prices` loop rewritten in terms of `n_goods`, with the `cap == 0` dead-good
skip preserved.

**This task is hash-neutral on trophic/frontier** because trophic/frontier configs
have the same n_goods and same cap values as today; the formula is identical. The
only change visible in the event stream is that `PriceUpdate.resource` is now typed
`Good(u16)` — events are unhashed (contract.rs:97–98), so no state-hash movement.

#### Files

- Modify: `crates/jumpgate-core/src/economy.rs` — `update_prices`, `Resource::ALL` → `Good::ALL`
- Modify: `crates/jumpgate-core/src/contract.rs` — `PriceUpdate` field type `Resource` → `Good`
- Modify: `crates/jumpgate-core/src/economy.rs` — any other `Resource::ALL` iteration over the price
  loop (economy.rs:325 is the only known site per ground-economy-verbs.md §2 gotcha)

> **Cross-beat gotcha:** `Resource::ALL` at economy.rs:325 is used to construct `PriceUpdate`
> events. After A1's `Good(u16)` newtype, this becomes `Good::ALL` or iteration over
> `GoodsCfg.goods`. The planner notes this as a same-commit obligation in A3.1.
> The cap==0 skip (economy.rs:309–311) is PRESERVED verbatim — a Good with cap==0 is
> the structural off for the pricer and must remain a silent continue.

- [ ] **Step 1: failing test — `update_prices_runs_for_all_goods_and_respects_cap_zero`**

```rust
// In crates/jumpgate-core/src/economy.rs, append to the #[cfg(test)] mod tests block.
//
// This test proves (a) the pricer runs for ALL n_goods resources, not just the
// two-element Resource::ALL array, and (b) cap==0 goods stay at their initial
// price. After A1 the fixture can have 3 goods; before A3.1's generalization the
// loop iterates Resource::ALL (len=2) and the third good is never updated.
#[test]
fn update_prices_runs_for_all_goods_and_respects_cap_zero() {
    use crate::economy::{EconCounters, Good, GoodsCfg, GoodSpec, PriceCfg, StationStore};
    use crate::contract::EventKind;
    use crate::events::EventStream;
    use crate::time::Tick;

    // Three goods: Ore(0), Fuel(1), Widget(2). Widget has cap==0 (dead, never priced).
    let goods_cfg = GoodsCfg {
        goods: vec![
            GoodSpec { name: "Ore".into(),    unit_mass_milli: 1000 },
            GoodSpec { name: "Fuel".into(),   unit_mass_milli: 1000 },
            GoodSpec { name: "Widget".into(), unit_mass_milli: 1000 },
        ],
    };
    let n = goods_cfg.goods.len();
    let mut station = StationStore::empty_with_goods(n); // A1 post-refactor constructor
    // Push one station with stock=0 for all goods.
    let _sid = station.push_goods(/* body_id placeholder */ 0, vec![0i64; n], vec![0i64; n]);

    let price_cfg = PriceCfg {
        base_micros:      vec![100_000, 50_000, 99_999], // Widget base irrelevant
        cap:              vec![100, 100, 0],              // Widget cap==0 → NEVER priced
        slope_milli:      1800,
        reprice_interval: 1,
    };
    // Seed station prices: Ore and Fuel start at their zero-stock price (base*2).
    // Widget starts at 1 (some nonzero sentinel that must NOT change after update).
    station.price_micros[0] = vec![0, 0, 1]; // initial_price_micros will be set by reset; simulate by direct write

    let mut events = EventStream::new();
    update_prices(&mut station, &price_cfg, &goods_cfg, Tick(1), &mut events);

    // Ore (cap>0, stock=0): should update to base*2 = 200_000.
    assert_eq!(station.price_micros[0][0], 200_000, "Ore price should update at stock=0");
    // Fuel (cap>0, stock=0): should update to base*2 = 100_000.
    assert_eq!(station.price_micros[0][1], 100_000, "Fuel price should update at stock=0");
    // Widget (cap==0): must remain at the initial sentinel value 1 — never priced.
    assert_eq!(station.price_micros[0][2], 1, "Widget (cap=0) must not be re-priced");

    // Exactly 2 PriceUpdate events emitted (Ore + Fuel), never Widget.
    let price_updates: Vec<_> = events.iter()
        .filter(|e| matches!(e.kind, EventKind::PriceUpdate { .. }))
        .collect();
    assert_eq!(price_updates.len(), 2, "only live-priced goods emit PriceUpdate");
}
```

Run: `cargo test -p jumpgate-core update_prices_runs_for_all_goods_and_respects_cap_zero`

Expected failure: compile error because `update_prices` still takes `&PriceCfg` with
`[i64; N_RESOURCES]` arrays, and `GoodsCfg` / `Good::ALL` do not exist yet (pre-A1
context) — or if A1 landed, a logic failure where Widget gets repriced.

- [ ] **Step 2: implementation — generalize `update_prices` over `n_goods`**

After A1 has changed `PriceCfg` arrays to `Vec<i64>` and introduced `Good(u16)` +
`GoodsCfg`, update `update_prices` to:
- Accept `goods_cfg: &GoodsCfg` as a new parameter
- Iterate `for r in 0..goods_cfg.goods.len()` instead of `for r in 0..N_RESOURCES`
- Preserve the `if price_cfg.cap[r] == 0 { continue; }` guard verbatim
- Replace `Resource::ALL[r]` in the `PriceUpdate` emit with `Good(r as u16)`

```rust
// crates/jumpgate-core/src/economy.rs — replace update_prices signature + body
pub fn update_prices(
    stations: &mut StationStore,
    price_cfg: &crate::config::PriceCfg,
    goods_cfg: &crate::config::GoodsCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    for row in 0..stations.ids.len() {
        for r in 0..goods_cfg.goods.len() {
            if price_cfg.cap[r] == 0 {
                continue;
            }
            let s = stations.stock[row][r].max(0).min(price_cfg.cap[r]);
            let p = (price_cfg.base_micros[r]
                * (2000 - s * price_cfg.slope_milli / price_cfg.cap[r])
                / 1000)
                .max(0);
            if p != stations.price_micros[row][r] {
                stations.price_micros[row][r] = p;
                if let Some(station) = stations
                    .ids
                    .id_at(row)
                    .map(|(slot, generation)| StationId { slot, generation })
                {
                    events.emit(Event {
                        tick,
                        kind: EventKind::PriceUpdate {
                            station,
                            resource: Good(r as u16), // A1 newtype replaces Resource::ALL[r]
                            price_micros: p,
                        },
                    });
                }
            }
        }
    }
}
```

Update the `World::step` call site in `world.rs` (stage 3d) to pass `&self.config.goods_cfg`
as the new third argument.

Update `EventKind::PriceUpdate` in `contract.rs` to use `resource: Good` instead of
`resource: Resource`.

- [ ] **Step 3: run test + clippy**

```
cargo test -p jumpgate-core update_prices_runs_for_all_goods_and_respects_cap_zero
cargo clippy --all-targets -- -D warnings
```

Expected: test passes, no warnings.

- [ ] **Step 4: commit**

```bash
git add crates/jumpgate-core/src/economy.rs \
        crates/jumpgate-core/src/contract.rs \
        crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(economy): generalize update_prices over all goods, cap==0 skip preserved

Replaces the Resource::ALL[r] two-element iteration with a goods_cfg.goods.len()
loop. PriceUpdate.resource is now Good(u16). The cap==0 dead-good skip is
preserved verbatim — zero-cap goods are never re-priced.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.2: GoodsCfg, ArbitrageCfg, ExchangeCfg config — the ONE rung-A GOLDEN_CONFIG_HASH re-pin

Adds the three new config structs to `RunConfig`, extends `config_hash`'s exhaustive
destructure to fold them, and re-pins GOLDEN_CONFIG_HASH. This is the single config
commit for rung A (L5-C4: no posture, no greed, no fence config — those are rung B).

New structs (all fields inert by default):

- `GoodsCfg { goods: Vec<GoodSpec> }` where `GoodSpec { name: String, unit_mass_milli: u32 }`
  — `name` is NEVER folded (OD-7 spec: "name (never folded)"); `unit_mass_milli` IS folded.
- `ArbitrageCfg { scan_interval: u32, wage_flat_micros: i64, wage_share_milli: u32,
  max_posts_per_scan: u32 }` — `scan_interval == 0` is the structural inert gate.
- `ExchangeCfg { corp_index: u32, active: bool }` — `active: false` default; the Exchange corp
  index for goods-leg money transfers.

The `CorporationInit` struct gains `arb_premium_micros: i64` (default 0, folded at tail of
the per-corp loop in config_hash).

#### Files

- Modify: `crates/jumpgate-core/src/config.rs` — add structs, extend `RunConfig`, extend
  `config_hash()` exhaustive destructure, re-pin `GOLDEN_CONFIG_HASH`, extend `sample()`

- [ ] **Step 1: failing test — config hash changes after adding new structs**

```rust
// In crates/jumpgate-core/src/config.rs, tests mod — a canary that fails until the
// new fields are in sample() and the golden is re-pinned.
#[test]
fn goods_cfg_arb_cfg_exchange_cfg_are_config_hashed() {
    // Build a RunConfig that differs only in ArbitrageCfg.scan_interval.
    // If the field is NOT folded, both hashes are equal — test fails.
    let base = sample();
    let mut modified = sample();
    // ArbitrageCfg with scan_interval=1 (non-default).
    modified.arbitrage = crate::config::ArbitrageCfg {
        scan_interval: 1,
        wage_flat_micros: 0,
        wage_share_milli: 0,
        max_posts_per_scan: 0,
    };
    assert_ne!(
        base.config_hash(),
        modified.config_hash(),
        "ArbitrageCfg.scan_interval must move the config hash"
    );

    // ExchangeCfg.active must also move the hash.
    let mut ex_modified = sample();
    ex_modified.exchange = crate::config::ExchangeCfg { corp_index: 0, active: true };
    assert_ne!(
        base.config_hash(),
        ex_modified.config_hash(),
        "ExchangeCfg.active must move the config hash"
    );
}
```

Run: `cargo test -p jumpgate-core goods_cfg_arb_cfg_exchange_cfg_are_config_hashed`

Expected failure: compile error — `ArbitrageCfg`, `ExchangeCfg` fields on `RunConfig`
don't exist yet.

- [ ] **Step 2: add GoodsCfg, ArbitrageCfg, ExchangeCfg structs**

```rust
// crates/jumpgate-core/src/config.rs — insert before RunConfig struct

/// Per-good property table (OD-7 minimal-live: name + unit_mass_milli).
/// `name` is NEVER folded into config_hash (display only).
/// `unit_mass_milli` is folded — it gates the integer milli-mass capacity check.
/// Further columns (value_density, perishability) land with their first reader.
#[derive(Clone, Debug)]
pub struct GoodSpec {
    /// Human-readable name for console/chronicle display. NOT folded.
    pub name: String,
    /// Mass per unit in milli-kg (1000 = 1 kg). Uniform 1000 in v1.
    pub unit_mass_milli: u32,
}

/// Goods configuration: the list of all tradeable goods in index order.
/// `Good(r as u16)` is the dense index into this vec.
/// GoodsCfg is folded with a COUNT FIRST in config_hash (anti-aliasing delimiter,
/// config fold discipline L5-F7). The fold is: goods.len() then per-good
/// unit_mass_milli (name never folded).
#[derive(Clone, Debug, Default)]
pub struct GoodsCfg {
    pub goods: Vec<GoodSpec>,
}

/// Exchange corporation config (OD-2): one config-named corp is the goods money
/// counterparty at every station including the haven.
/// `active: false` default → the Exchange verb settle arms are no-ops (inert gate,
/// analogous to RefuelCfg.lot_mass == 0.0).
/// `corp_index` is the dense corporation row index (Yard/Port idiom).
#[derive(Clone, Copy, Debug)]
pub struct ExchangeCfg {
    /// Dense corporation row index receiving/paying trade money.
    pub corp_index: u32,
    /// When false, all TradeBuy/TradeSell settle arms are deterministic no-ops.
    pub active: bool,
}

impl Default for ExchangeCfg {
    fn default() -> Self {
        ExchangeCfg { corp_index: 0, active: false }
    }
}

/// Arbitrage poster config (stage 1b2 slot, OD-2/spec §1.2).
/// `scan_interval == 0` is the structural inert gate: the poster returns
/// immediately without scanning, preserving bit-identical behavior on
/// trophic/frontier (the RefuelCfg.lot_mass precedent).
#[derive(Clone, Copy, Debug)]
pub struct ArbitrageCfg {
    /// Ticks between poster scans. 0 = poster is OFF (the structural inert gate).
    pub scan_interval: u32,
    /// Fixed transport-floor component of posted wage (micros).
    pub wage_flat_micros: i64,
    /// Fraction of spread surplus added to wage: `surplus * wage_share_milli / 1000`.
    pub wage_share_milli: u32,
    /// Maximum contracts posted per scan across all routes and corps.
    pub max_posts_per_scan: u32,
}

impl Default for ArbitrageCfg {
    fn default() -> Self {
        ArbitrageCfg {
            scan_interval: 0,
            wage_flat_micros: 0,
            wage_share_milli: 500,
            max_posts_per_scan: 32,
        }
    }
}
```

- [ ] **Step 3: extend CorporationInit with arb_premium_micros**

```rust
// crates/jumpgate-core/src/config.rs — CorporationInit
#[derive(Clone, Debug)]
pub struct CorporationInit {
    pub treasury_micros: i64,
    pub home_station_index: usize,
    /// Arbitrage premium floor this corp requires above transport cost before
    /// posting. 0 = will post whenever spread > transport (default).
    pub arb_premium_micros: i64,
}
```

Update all construction sites of `CorporationInit` (world.rs reset, scenario files,
test fixtures) to include `arb_premium_micros: 0` — compile exhaustiveness will catch
any missed sites.

- [ ] **Step 4: add fields to RunConfig and extend config_hash exhaustive destructure**

In `RunConfig`:
```rust
// After refuel field, append-only:
/// Goods configuration: property table for all tradeable goods.
/// Inert default (empty goods list — trophic/frontier behavior preserved).
pub goods_cfg: GoodsCfg,
/// Exchange configuration: the money counterparty for goods trades.
pub exchange: ExchangeCfg,
/// Arbitrage poster configuration.
pub arbitrage: ArbitrageCfg,
```

In `config_hash()`, extend the exhaustive destructure to bind `goods_cfg`, `exchange`,
`arbitrage`, and in the per-corp loop bind `arb_premium_micros`. Fold at the CONFIG
tail (after RefuelCfg, append-only — CONFIG_FIELD_ORDER words 27..=30):

```rust
// GOODS-AS-GOODS RUNG A (TAIL, append-only — CONFIG_FIELD_ORDER 27..=30).
// Exhaustive destructures: a new field is a compile error until folded.
let GoodsCfg { goods } = goods_cfg;
// COUNT FIRST (anti-aliasing delimiter, config fold discipline):
h.write_u64(goods.len() as u64);
for g in goods {
    let GoodSpec { name: _, unit_mass_milli } = g; // name NEVER folded
    h.write_u64(*unit_mass_milli as u64);
}
let ExchangeCfg { corp_index: ex_corp, active } = exchange;
h.write_u64(*ex_corp as u64);
h.write_u64(*active as u64);
let ArbitrageCfg { scan_interval, wage_flat_micros, wage_share_milli, max_posts_per_scan } = arbitrage;
h.write_u64(*scan_interval as u64);
h.write_u64(*wage_flat_micros as u64);
h.write_u64(*wage_share_milli as u64);
h.write_u64(*max_posts_per_scan as u64);
// arb_premium_micros is folded in the per-corp loop above (extend that loop):
// for c in corporations { h.write_u64(c.treasury_micros); h.write_u64(home_station_index); h.write_u64(c.arb_premium_micros as u64); }
```

In the per-corporation loop in `config_hash`, add:
```rust
for c in corporations {
    h.write_u64(c.treasury_micros as u64);
    h.write_u64(c.home_station_index as u64);
    h.write_u64(c.arb_premium_micros as u64); // NEW — rung A
}
```

Extend `sample()` in the test module with the new fields (exhaustiveness compiles
it in):
```rust
// In sample() RunConfig literal — add after refuel:
goods_cfg: GoodsCfg::default(),
exchange: ExchangeCfg::default(),
arbitrage: ArbitrageCfg::default(),
```

And in `sample()`'s `corporations` vec, the single `CorporationInit` gains:
```rust
CorporationInit { treasury_micros: 0, home_station_index: 0, arb_premium_micros: 0 }
```

- [ ] **Step 5: re-pin GOLDEN_CONFIG_HASH**

**DO NOT invent a literal.** Run the `print_golden_config` test to derive the new value:

```bash
cargo test -p jumpgate-core print_golden_config -- --ignored --nocapture
```

The test prints two lines; capture the `GOLDEN_CONFIG_HASH = 0x....` line. Then
open `crates/jumpgate-core/src/config.rs` and replace the existing constant with the
printed value, adding a provenance comment:

```rust
const GOLDEN_CONFIG_HASH: u64 = 0x????_????_????_????; // RE-PINNED: +GoodsCfg+ExchangeCfg+ArbitrageCfg+arb_premium_micros folded at config tail (goods-as-goods rung A). Was 0x128c_1299_5c48_4fdc.
```

Then verify:
```bash
cargo test -p jumpgate-core config_hash_golden_anchor_is_stable
```

- [ ] **Step 6: run workspace tests**

```
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all tests pass including `goods_cfg_arb_cfg_exchange_cfg_are_config_hashed`
and `config_hash_golden_anchor_is_stable`. No state-hash test movement (the config
hash and the state hash are separate spaces — config.rs:474 note).

- [ ] **Step 7: commit (single-cause — the one rung-A config re-pin)**

```bash
git add crates/jumpgate-core/src/config.rs \
        crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/scenario.rs
# (any other file that constructs CorporationInit or RunConfig)
git commit -F - <<'EOF'
feat(config): add GoodsCfg+ExchangeCfg+ArbitrageCfg — rung-A config re-pin

One GOLDEN_CONFIG_HASH re-pin for rung A. GoodsCfg folds count then
unit_mass_milli (name never folded, OD-7). ExchangeCfg and ArbitrageCfg
fold at config tail. CorporationInit gains arb_premium_micros. All defaults
inert; trophic/frontier behavior-identical proven by behavior digest.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.3: TradeBuy/TradeSell pending columns + debug_asserts

Adds `pending_trade_buy` and `pending_trade_sell` columns to `CraftStore`, initializes
them in `reset()` / `empty()` / `push()`, and extends the `state_hash` debug_asserts
to cover them. This task is hash-neutral and state-neutral — both columns are transient
and never folded. The asserts are the load-bearing correctness guard that makes
stage-ordering bugs loud.

#### Files

- Modify: `crates/jumpgate-core/src/stores.rs` — add columns to `CraftStore`
- Modify: `crates/jumpgate-core/src/world.rs` — `reset()` pushes new columns
- Modify: `crates/jumpgate-core/src/hash.rs` — extend the all-None `debug_assert!` block

- [ ] **Step 1: failing test — debug_assert fires when pending_trade_buy is not consumed**

```rust
// crates/jumpgate-core/src/hash.rs, in tests mod.
// This is a debug-build test: it verifies that state_hash panics when the new column
// has a Some value at hash time (the stage-ordering contract).
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "pending_trade_buy must be fully consumed")]
fn pending_trade_buy_not_none_at_hash_point_panics() {
    use crate::world::World;
    use crate::scenario::scenario_trophic; // reuse a minimal fixture

    let (mut world, _) = World::reset(scenario_trophic()).expect("reset ok");
    // Force a Some into the column (simulating a broken settle stage).
    // pending_trade_buy holds Option<(Good, u32, StationId)> — use any sentinel.
    world.ships.pending_trade_buy[0] = Some((crate::economy::Good(0), 1, world.stations.ids.id_at(0).map(|(s,g)| crate::ids::StationId { slot: s, generation: g }).unwrap_or_default()));
    // state_hash must debug_assert-panic.
    let _ = crate::hash::state_hash(&world);
}
```

Run: `cargo test -p jumpgate-core pending_trade_buy_not_none_at_hash_point_panics`

Expected failure: compile error — `ships.pending_trade_buy` does not exist yet.

- [ ] **Step 2: add columns to CraftStore**

```rust
// crates/jumpgate-core/src/stores.rs — in CraftStore, after pending_refuel:
/// TRANSIENT own-trade BUY intent written by run_trade_policies (stage 1c3x)
/// and consumed unconditionally by resolve_trade_buys (stage 1dx) the same tick.
/// Payload: (good_index, qty, source_station_id). None at every state-hash point.
pub pending_trade_buy: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>,
/// TRANSIENT own-trade SELL intent written by run_trade_policies (stage 1c3x)
/// and consumed unconditionally by resolve_trade_sells (stage 1dx) the same tick.
/// Payload: destination StationId (goods and qty read from hold). None at every hash point.
pub pending_trade_sell: Vec<Option<crate::ids::StationId>>,
```

Extend `CraftStore::empty()` to initialize both vecs as `Vec::new()`.

Extend `CraftStore::push()` to append `None` for each new column:
```rust
self.pending_trade_buy.push(None);
self.pending_trade_sell.push(None);
```

Extend `World::reset()`'s per-craft push loop to append `None` for both columns:
```rust
ships.pending_trade_buy.push(None);
ships.pending_trade_sell.push(None);
```

- [ ] **Step 3: extend state_hash debug_asserts**

```rust
// crates/jumpgate-core/src/hash.rs — in state_hash(), after the pending_refuel assert:
debug_assert!(
    world.ships.pending_trade_buy.iter().all(Option::is_none),
    "pending_trade_buy must be fully consumed (all None) at every state-hash point"
);
debug_assert!(
    world.ships.pending_trade_sell.iter().all(Option::is_none),
    "pending_trade_sell must be fully consumed (all None) at every state-hash point"
);
```

- [ ] **Step 4: run test + workspace**

```
cargo test -p jumpgate-core pending_trade_buy_not_none_at_hash_point_panics
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: the should_panic test passes; all other tests pass.

- [ ] **Step 5: commit**

```bash
git add crates/jumpgate-core/src/stores.rs \
        crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/hash.rs
git commit -F - <<'EOF'
feat(stores): add pending_trade_buy/sell transient columns + debug_asserts

Both columns are None at every state-hash point (hash-neutral). The
debug_assert! block in state_hash is extended to catch stage-ordering bugs
loudly. Same discipline as pending_upgrade / pending_refuel.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.4: TradeBought/TradeSold events + delete dead EventKind::Trade

Adds `TradeBought` and `TradeSold` to `EventKind`, deletes the dead `EventKind::Trade`
variant (contract.rs:75–80, only constructor at contract.rs:417 in a test), updates
the test `economy_event_kinds_are_copy_and_partial_eq` to replace the `Trade`
comparison with `TradeBought`, and adds chronicle arms + gossip-log arms in the same
commit. The `_ => None` wildcards in `chronicle_subject` and `gossip_log_event_json`
are made exhaustive (the deliberate policy reversal per synthesis cut Part 3).

**Single-emit discipline:** `TradeBought` is emitted exactly once — in
`resolve_trade_buys` after all accounting legs settle. `TradeSold` is emitted exactly
once — in `resolve_trade_sells`.

#### Files

- Modify: `crates/jumpgate-core/src/contract.rs` — delete `Trade`, add `TradeBought`/`TradeSold`
- Modify: `crates/jumpgate-core/src/trophic_run.rs` — `chronicle_subject`,
  `gossip_log_event_json`, make exhaustive
- Modify: `crates/jumpgate-core/src/economy.rs` — update test using `Trade`

- [ ] **Step 1: failing test — TradeBought/TradeSold are in EventKind and are Copy+PartialEq**

```rust
// crates/jumpgate-core/src/contract.rs — replace the existing
// economy_event_kinds_are_copy_and_partial_eq test body:
#[test]
fn economy_event_kinds_are_copy_and_partial_eq() {
    use crate::ids::{CraftId, StationId};
    use crate::economy::{Good, Resource};
    use crate::time::Tick;

    let production = EventKind::Production {
        producer: crate::ids::ProducerId { slot: 0, generation: 0 },
        resource: Resource::Ore,
        qty: 5,
    };
    let trade_bought = EventKind::TradeBought {
        craft: CraftId { slot: 0, generation: 0 },
        station: StationId { slot: 0, generation: 0 },
        good: Good(0),
        qty: 3,
        price_micros: 100_000,
    };
    let trade_bought_copy = trade_bought; // Copy
    assert_eq!(trade_bought, trade_bought_copy); // PartialEq
    assert_ne!(production, trade_bought); // distinct variants
}
```

Run: `cargo test -p jumpgate-core economy_event_kinds_are_copy_and_partial_eq`

Expected failure: compile error — `TradeBought` variant doesn't exist yet; `Trade`
still does and the old test referenced it.

- [ ] **Step 2: delete Trade, add TradeBought/TradeSold**

```rust
// crates/jumpgate-core/src/contract.rs — in EventKind enum:
// DELETE:
//   Trade { station: StationId, resource: Resource, qty: u32, price_micros: i64 },
// ADD (new group, hash-neutral like all events per contract.rs:169 idiom):

// --- Goods-as-goods rung A (hash-neutral like all events) ---
/// A scripted craft bought goods from a station (stock -> hold transfer settled).
/// Emitted exactly once: in resolve_trade_buys after all accounting legs.
TradeBought {
    craft: CraftId,
    station: StationId,
    good: crate::economy::Good,
    qty: u32,
    price_micros: i64,
},
/// A scripted craft sold goods to a station (hold -> stock transfer settled).
/// Emitted exactly once: in resolve_trade_sells after all accounting legs.
TradeSold {
    craft: CraftId,
    station: StationId,
    good: crate::economy::Good,
    qty: u32,
    price_micros: i64,
},
```

- [ ] **Step 3: make chronicle_subject exhaustive, add TradeBought/TradeSold arms**

```rust
// crates/jumpgate-core/src/trophic_run.rs — chronicle_subject function.
// REMOVE the `_ => None` arm and the doc comment saying variants default to skipped.
// ADD arms for all new variants. The full match must be exhaustive:
EventKind::TradeBought { craft, .. } => Some(*craft),
EventKind::TradeSold   { craft, .. } => Some(*craft),
// Variants with no craft subject — explicit None arms:
EventKind::Production { .. }    => None,
EventKind::PriceUpdate { .. }   => None,
EventKind::ContractOffered { .. } => None,
// (All existing arms already present; add the new variants, remove the wildcard.)
```

The function must compile exhaustively — if any `EventKind` variant is missing from
the match, Rust will refuse to compile.

- [ ] **Step 4: add gossip_log_event_json arms for TradeBought/TradeSold**

```rust
// crates/jumpgate-core/src/trophic_run.rs — gossip_log_event_json.
// Add arms BEFORE removing the _ => None wildcard. The "buy" and "sell" rows
// encode craft.slot, station.slot, good.0, qty, price_micros (all i64 safe).
EventKind::TradeBought { craft, station, good, qty, price_micros } => Some(format!(
    r#"{{"e":"buy","t":{},"craft":{},"station":{},"good":{},"qty":{},"price_micros":{}}}"#,
    tick.0, craft.slot, station.slot, good.0, qty, price_micros
)),
EventKind::TradeSold { craft, station, good, qty, price_micros } => Some(format!(
    r#"{{"e":"sell","t":{},"craft":{},"station":{},"good":{},"qty":{},"price_micros":{}}}"#,
    tick.0, craft.slot, station.slot, good.0, qty, price_micros
)),
```

After adding TradeBought/TradeSold arms, make this match exhaustive too (remove the
`_ => None` wildcard; all remaining non-gossip-log variants get explicit `=> None`).

- [ ] **Step 5: run tests**

```
cargo test -p jumpgate-core economy_event_kinds_are_copy_and_partial_eq
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. The dead `Trade` variant is gone; the new test proves Copy+PartialEq
on `TradeBought`.

- [ ] **Step 6: commit (same commit as delete Trade + add TradeBought/TradeSold)**

```bash
git add crates/jumpgate-core/src/contract.rs \
        crates/jumpgate-core/src/trophic_run.rs \
        crates/jumpgate-core/src/economy.rs
git commit -F - <<'EOF'
feat(events): add TradeBought/TradeSold, delete dead EventKind::Trade

Chronicle arms and gossip-log arms land in the same commit (policy: no
silently-swallowed variants). Both chronicle_subject and gossip_log_event_json
are now exhaustive (wildcard removed, deliberate policy reversal of the
documented "default-skip" behavior). TradeBought/TradeSold are unhashed.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.5: run_trade_policies (stage 1c3x) — scripted intent writer

Adds `run_trade_policies`: the scripted own-trade policy intent writer analogous to
`run_refuel_policies` (stage 1c3b). Writes `pending_trade_buy` OR `pending_trade_sell`
based on the two-mode policy decision (spec D6, synthesis cut §1.2 "two-mode policy"):

- **Own-trade path:** wallet ≥ buy_cost + trade_reserve AND best_trade_net > best_wage_net → write
  `pending_trade_buy`
- **Sell path:** craft has a live hold (non-empty) AND docked at a station → write `pending_trade_sell`
- Pirates and `!scripted` craft are always skipped (the refuel-policy precedent).

**ASSIGN empty-hold gate (L3-M3):** ASSIGN only assigns package contracts to craft
with an empty hold. This is enforced in the ASSIGN arm of `run_scripted_dispatch`,
not here — but noted as a same-phase obligation.

**Day-0 wallets are 0** (world.rs:311), so all day-0 craft are in wage-hauling mode
(WA3 story opens at share 0 — that IS the reading).

> **Cross-beat gotcha:** the ingest arm does NOT skip — it merely writes intent.
> The scripted policy stage skips pirates and !scripted (the refuel precedent).
> Skipping pirates here prevents rung-A pirates from becoming own-traders, which
> would break D7's channel split silently.

#### Files

- Modify: `crates/jumpgate-core/src/economy.rs` — new `run_trade_policies` function
- Modify: `crates/jumpgate-core/src/world.rs` — call at stage 1c3x (after 1c3b)
- Modify: `crates/jumpgate-core/src/ingest.rs` — add `TradeBuy`/`TradeSell` CommandKind arms

- [ ] **Step 1: failing test — run_trade_policies writes pending_trade_buy for a capitalized docked craft**

```rust
// crates/jumpgate-core/src/economy.rs — append to tests mod.
#[test]
fn trade_policy_writes_buy_intent_for_capitalized_docked_craft() {
    use crate::economy::{Good, GoodsCfg, GoodSpec, ArbitrageCfg, ExchangeCfg, run_trade_policies};
    use crate::stores::{CraftRole, CraftStore};
    use crate::time::Tick;

    // Build a minimal fixture: one scripted Idle hauler docked at station 0,
    // with a large wallet and station 0 having stock + a spread to station 1.
    // After run_trade_policies, pending_trade_buy[0] must be Some.
    let mut fixture = trade_policy_fixture(
        /* wallet_micros */ 10_000_000,
        /* station0_stock */ 50,
        /* station0_price_micros */ 200_000,
        /* station1_price_micros */ 400_000, // spread positive
        /* trade_reserve_micros */ 1_000_000,
    );
    assert!(fixture.ships.pending_trade_buy[0].is_none(), "precondition: no intent yet");
    run_trade_policies(
        &mut fixture.ships,
        &fixture.craft_cfg,
        &fixture.stations,
        &fixture.bodies,
        &fixture.eph,
        &fixture.exchange_cfg,
        &fixture.arbitrage_cfg,
        &fixture.goods_cfg,
        Tick(1),
    );
    assert!(
        fixture.ships.pending_trade_buy[0].is_some(),
        "capitalized docked craft with positive spread must write buy intent"
    );
    // pending_trade_sell remains None (hold is empty — no goods to sell).
    assert!(fixture.ships.pending_trade_sell[0].is_none());
}

#[test]
fn trade_policy_skips_pirate() {
    use crate::stores::CraftRole;
    use crate::economy::run_trade_policies;
    use crate::time::Tick;

    let mut fixture = trade_policy_fixture(10_000_000, 50, 200_000, 400_000, 1_000_000);
    fixture.ships.role[0] = CraftRole::Pirate;
    run_trade_policies(
        &mut fixture.ships, &fixture.craft_cfg, &fixture.stations,
        &fixture.bodies, &fixture.eph, &fixture.exchange_cfg,
        &fixture.arbitrage_cfg, &fixture.goods_cfg, Tick(1),
    );
    assert!(fixture.ships.pending_trade_buy[0].is_none(), "pirate must be skipped");
}

#[test]
fn trade_policy_skips_when_broke() {
    use crate::economy::run_trade_policies;
    use crate::time::Tick;

    // wallet=0 → cannot afford a buy.
    let mut fixture = trade_policy_fixture(0, 50, 200_000, 400_000, 1_000_000);
    run_trade_policies(
        &mut fixture.ships, &fixture.craft_cfg, &fixture.stations,
        &fixture.bodies, &fixture.eph, &fixture.exchange_cfg,
        &fixture.arbitrage_cfg, &fixture.goods_cfg, Tick(1),
    );
    assert!(fixture.ships.pending_trade_buy[0].is_none(), "broke craft must not buy");
}
```

Run: `cargo test -p jumpgate-core trade_policy_writes_buy_intent`

Expected failure: compile error — `run_trade_policies` does not exist.

- [ ] **Step 2: implement run_trade_policies**

```rust
// crates/jumpgate-core/src/economy.rs

/// Two-mode scripted trade policy intent stage — stage 1c3x, goods-as-goods rung A.
/// Writes `pending_trade_buy` or `pending_trade_sell` for scripted non-pirate craft.
/// Pirates and !scripted craft are always skipped (the run_refuel_policies precedent).
///
/// SELL path: if the craft has a non-empty hold AND is docked, write pending_trade_sell
/// for the current dock station. Sell always wins over buy (deliver before restocking).
///
/// BUY path: if wallet >= buy_cost + trade_reserve AND best_trade_net > best_wage_net,
/// write pending_trade_buy for the best (good, qty, station) triple.
///
/// Day-0 wallets are 0 → all craft start in wage-hauling mode (WA3 story intent).
/// Never clobbers an existing ingest intent (the BuyUpgrade/Refuel precedent).
#[allow(clippy::too_many_arguments)]
pub fn run_trade_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    exchange: &crate::config::ExchangeCfg,
    arbitrage: &crate::config::ArbitrageCfg,
    goods_cfg: &crate::config::GoodsCfg,
    tick: Tick,
) {
    // Structural off: if Exchange is inactive, no intent is ever written.
    if !exchange.active {
        return;
    }
    let n_goods = goods_cfg.goods.len();
    let prev = Tick(tick.0.saturating_sub(1));
    for crow in 0..ships.ids.len() {
        // Skip !scripted craft (gym-exclusion gate — the refuel precedent).
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        // Skip pirates — rung-A pirates are never own-traders (D7 channel split).
        if ships.role[crow] == CraftRole::Pirate {
            continue;
        }
        // Never clobber an existing ingest intent.
        if ships.pending_trade_buy[crow].is_some() || ships.pending_trade_sell[crow].is_some() {
            continue;
        }
        // Only Idle craft make policy decisions (Haulers are already committed).
        if ships.role[crow] != CraftRole::Idle {
            continue;
        }
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };

        // SELL PATH: if hold is non-empty, deliver goods at this station.
        if !ships.hold[crow].is_empty() {
            if let Some(sid) = stations.ids.id_at(srow).map(|(s, g)| StationId { slot: s, generation: g }) {
                ships.pending_trade_sell[crow] = Some(sid);
            }
            continue; // sell intent written; do not also write a buy intent
        }

        // BUY PATH: find the best (good, source_qty) at this dock.
        // best_trade_net = spread * qty - transport[route] (spec synthesis §1.2).
        // v1 simplification: use (dest_price - src_price) * qty as the net proxy;
        // full transport-table comparison added when ArbitrageCfg.wage_flat_micros
        // is used in the poster (A3.6). Here we just gate on spread > 0.
        let mut best_good: Option<(Good, u32, i64)> = None; // (good, qty, net)
        for r in 0..n_goods {
            let src_price = stations.price_micros[srow][r];
            if src_price < 1 {
                continue; // L2-C3: price<1 guard on every buy settle arm
            }
            let stock = stations.stock[srow][r];
            if stock <= 0 {
                continue;
            }
            // Capacity: how many units fit in hold? (uniform unit_mass_milli=1000 v1)
            let hold_used: u32 = ships.hold[crow].iter().map(|(_, q)| q).sum();
            let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
            let cap = eff.base_cargo_capacity; // units (milli-mass = cap * 1000)
            if hold_used >= cap {
                continue;
            }
            let free_units = (cap - hold_used) as i64;
            let afford_units = ships.credits_micros[crow].max(0) / src_price;
            if afford_units < 1 {
                continue;
            }
            let qty = free_units.min(stock).min(afford_units) as u32;
            if qty == 0 {
                continue;
            }
            // Find the best destination price across all OTHER stations.
            let best_dest_price: i64 = (0..stations.ids.len())
                .filter(|&dr| dr != srow)
                .map(|dr| stations.price_micros[dr][r])
                .max()
                .unwrap_or(0);
            let spread = best_dest_price - src_price;
            if spread <= 0 {
                continue;
            }
            let net = spread * qty as i64;
            // Also check wallet headroom: must cover buy_cost + trade_reserve.
            let buy_cost = qty as i64 * src_price;
            let trade_reserve = arbitrage.wage_flat_micros; // reuse as reserve proxy v1
            if ships.credits_micros[crow] < buy_cost + trade_reserve {
                continue;
            }
            if best_good.map_or(true, |(_, _, prev_net)| net > prev_net) {
                best_good = Some((Good(r as u16), qty, net));
            }
        }
        if let Some((good, qty, _)) = best_good {
            if let Some(sid) = stations.ids.id_at(srow).map(|(s, g)| StationId { slot: s, generation: g }) {
                ships.pending_trade_buy[crow] = Some((good, qty, sid));
            }
        }
    }
}
```

- [ ] **Step 3: add TradeBuy/TradeSell ingest arms**

```rust
// crates/jumpgate-core/src/ingest.rs — in the CommandKind match:
CommandKind::TradeBuy { good, qty, station } => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_trade_buy[i] = Some((good, qty, station));
    }
}
CommandKind::TradeSell { station } => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_trade_sell[i] = Some(station);
    }
}
```

Add the new arms to `CommandKind` in `contract.rs` or wherever it is defined.

- [ ] **Step 4: wire into World::step at stage 1c3x**

```rust
// crates/jumpgate-core/src/world.rs — after stage 1c3b (resolve_refuels):
// (1c3x) scripted trade policies (goods-as-goods rung A): write
//        pending_trade_buy or pending_trade_sell for scripted non-pirate
//        craft based on the two-mode policy decision. Consumed by
//        stages 1dx (resolve_trade_buys, resolve_trade_sells) this tick.
crate::economy::run_trade_policies(
    &mut self.ships,
    &self.config.craft,
    &self.stations,
    &self.bodies,
    &self.eph,
    &self.config.exchange,
    &self.config.arbitrage,
    &self.config.goods_cfg,
    next,
);
```

- [ ] **Step 5: run tests + clippy**

```
cargo test -p jumpgate-core trade_policy_writes_buy_intent_for_capitalized_docked_craft
cargo test -p jumpgate-core trade_policy_skips_pirate
cargo test -p jumpgate-core trade_policy_skips_when_broke
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. State-hash tests (golden_zero_state_hash, frontier_trajectory)
are unaffected because the new columns are transient and never folded.

- [ ] **Step 6: commit**

```bash
git add crates/jumpgate-core/src/economy.rs \
        crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/ingest.rs \
        crates/jumpgate-core/src/contract.rs
git commit -F - <<'EOF'
feat(economy): run_trade_policies stage 1c3x — scripted two-mode intent writer

Pirates and !scripted craft skipped (D7 channel split). Sell path wins over buy
path (non-empty hold triggers pending_trade_sell). Buy path gates: price<1 guard
(L2-C3), stock>0, wallet headroom including trade_reserve. Exchange.active=false
is the structural inert gate; trophic/frontier behavior-identical.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.6: resolve_trade_buys and resolve_trade_sells (stage 1dx) — settle stages

The two settle stages for the own-trade verbs. Both follow the
always-consume-then-gate idiom of `resolve_refuels`. The goods leg is a
**stock ↔ hold TRANSFER** (no `consumed[]` counter — the `try_load` shape at
economy.rs:826–827). The money leg is a pure **wallet ↔ Exchange treasury** transfer
inside Σtreasury+Σcredits+Σescrow. The `price < 1` guard is cloned from
economy.rs:1023–1025 (L2-C3) into BOTH settle functions before any integer division.

**Six deterministic no-op skip arms enumerated (resolve_trade_buys):**
1. Undocked (docked_station_row returns None)
2. Zero stock (stations.stock[srow][good_r] <= 0)
3. Wallet short (credits_micros < price × qty)
4. Hold full (used_milli + qty * unit_mass_milli > capacity * 1000)
5. Price < 1 (L2-C3 guard before division)
6. Stale Exchange corp row (corporations.ids.id_at(ex_row).is_none())

**Four deterministic no-op skip arms enumerated (resolve_trade_sells):**
1. Undocked
2. Empty hold (nothing to sell)
3. Price < 1 at destination
4. Stale Exchange corp row

**Credit identity:** these legs are named:
- BUY: `ships.credits_micros[crow] -= cost` + `corporations.treasury_micros[ex_row] += cost`
- SELL: `corporations.treasury_micros[ex_row] -= revenue` (saturating) + `ships.credits_micros[crow] += revenue`

The SELL Exchange leg is **saturating** (the Exchange treasury may not cover the
payment — spec §1.2: "saturating at the Exchange — goods still unload, contract
still completes"). The credit identity is extended to include the Exchange treasury.

> **Cross-beat gotcha:** Trade goods leg is a TRANSFER with NO `consumed[]` counter
> increment (ground-economy-verbs.md §5, panel consensus #5). Adding a `consumed[]`
> increment here corrupts the resource identity.

> **Cross-beat gotcha:** `try_load` uses tick-(t-1) ephemeris (economy.rs:800). The
> own-trade buy settle must use the same t-1 frame for the dock predicate.

#### Files

- Modify: `crates/jumpgate-core/src/economy.rs` — add `resolve_trade_buys`, `resolve_trade_sells`
- Modify: `crates/jumpgate-core/src/world.rs` — call both at stage 1dx (after 1d2)
- Modify: `crates/jumpgate-core/src/world.rs` — extend `assert_resource_identity` to include hold sum

- [ ] **Step 1: failing tests — settle stages settle correctly**

```rust
// crates/jumpgate-core/src/economy.rs — append to tests mod.

#[test]
fn trade_buy_settles_stock_to_hold_no_consumed_counter() {
    // A craft with 10_000_000 credits docked at a station with 50 units of Good(0)
    // at price 200_000 and pending_trade_buy = Some((Good(0), 3, station_id)).
    // After resolve_trade_buys:
    //   - station stock[0] -= 3
    //   - craft hold gains (Good(0), 3)
    //   - craft credits -= 3 * 200_000 = 600_000
    //   - Exchange treasury += 600_000
    //   - econ.consumed[0] UNCHANGED (TRANSFER, not a trophic sink)
    //   - TradeBought event emitted
    //   - pending_trade_buy[0] == None
    let (mut world, initial) = trade_buy_fixture(
        /* wallet */ 10_000_000,
        /* stock */ 50,
        /* price */ 200_000,
        /* qty intent */ 3,
    );
    let consumed_before = world.econ.consumed.clone();
    let ex_treasury_before = world.corporations.treasury_micros[0];

    crate::economy::resolve_trade_buys(
        &mut world.ships,
        &mut world.stations,
        &world.bodies,
        &world.eph,
        &mut world.corporations,
        &world.config.exchange,
        &world.config.goods_cfg,
        world.tick(),
        &mut world.events,
    );

    // Intent consumed.
    assert!(world.ships.pending_trade_buy[0].is_none(), "intent must be consumed");
    // Stock decremented.
    assert_eq!(world.stations.stock[0][0], 47, "station stock must decrease by 3");
    // Hold loaded.
    assert_eq!(world.ships.hold[0], vec![(crate::economy::Good(0), 3u32)]);
    // Wallet debited.
    assert_eq!(world.ships.credits_micros[0], 10_000_000 - 600_000);
    // Exchange credited.
    assert_eq!(world.corporations.treasury_micros[0], ex_treasury_before + 600_000);
    // consumed[] UNCHANGED (not a trophic sink).
    assert_eq!(world.econ.consumed, consumed_before, "consumed counter must not change on goods transfer");
    // TradeBought event emitted.
    let bought_events: Vec<_> = world.events.iter()
        .filter(|e| matches!(e.kind, crate::contract::EventKind::TradeBought { .. }))
        .collect();
    assert_eq!(bought_events.len(), 1);

    // Resource identity must hold (hold sum included — the A2 same-commit obligation).
    assert_resource_identity_with_hold(&world, &initial);
}

#[test]
fn trade_buy_skips_deterministically() {
    // Six skip arms: undocked, stock=0, wallet-short, hold-full, price<1, stale-corp.
    for arm in ["undocked", "stock-0", "wallet-short", "hold-full", "price-0", "stale-corp"] {
        let mut world = trade_buy_skip_fixture(arm);
        let snapshot = world_snapshot(&world); // capture credits, stock, hold, treasury
        crate::economy::resolve_trade_buys(
            &mut world.ships,
            &mut world.stations,
            &world.bodies,
            &world.eph,
            &mut world.corporations,
            &world.config.exchange,
            &world.config.goods_cfg,
            world.tick(),
            &mut world.events,
        );
        // Intent must be consumed (always-consume-then-gate).
        assert!(world.ships.pending_trade_buy[0].is_none(), "{arm}: intent not consumed");
        // No credit movement.
        assert_eq!(snapshot, world_snapshot(&world), "{arm}: skip must not move any state");
        // No TradeBought event.
        assert!(!world.events.iter().any(|e| matches!(e.kind, crate::contract::EventKind::TradeBought { .. })),
            "{arm}: no event on skip");
    }
}

#[test]
fn trade_sell_settles_hold_to_stock_exchange_pays() {
    // A craft with hold [(Good(0), 5)] docked at a station where Good(0) price=300_000.
    // Exchange treasury = 5_000_000 (covers the payment).
    // After resolve_trade_sells:
    //   - craft hold becomes empty
    //   - station stock[0] += 5
    //   - Exchange treasury -= 5 * 300_000 = 1_500_000
    //   - craft credits += 1_500_000
    //   - pending_trade_sell[0] == None
    let (mut world, initial) = trade_sell_fixture(
        /* hold */ vec![(crate::economy::Good(0), 5u32)],
        /* dest_price */ 300_000,
        /* ex_treasury */ 5_000_000,
    );
    let credits_before = world.ships.credits_micros[0];
    crate::economy::resolve_trade_sells(
        &mut world.ships,
        &mut world.stations,
        &world.bodies,
        &world.eph,
        &mut world.corporations,
        &world.config.exchange,
        &world.config.goods_cfg,
        world.tick(),
        &mut world.events,
    );

    assert!(world.ships.pending_trade_sell[0].is_none());
    assert_eq!(world.ships.hold[0].len(), 0, "hold must be empty after sell");
    assert_eq!(world.stations.stock[0][0], 5, "station stock += 5");
    assert_eq!(world.ships.credits_micros[0], credits_before + 1_500_000);
    assert_eq!(world.corporations.treasury_micros[0], 5_000_000 - 1_500_000);

    let sold: Vec<_> = world.events.iter()
        .filter(|e| matches!(e.kind, crate::contract::EventKind::TradeSold { .. }))
        .collect();
    assert_eq!(sold.len(), 1);

    assert_resource_identity_with_hold(&world, &initial);
}

#[test]
fn trade_sell_exchange_saturates_when_broke() {
    // Exchange treasury = 0: sell still unloads goods and pays 0 credits (saturating).
    let (mut world, _initial) = trade_sell_fixture(
        vec![(crate::economy::Good(0), 5u32)],
        300_000,
        /* ex_treasury */ 0,
    );
    let credits_before = world.ships.credits_micros[0];
    crate::economy::resolve_trade_sells(
        &mut world.ships, &mut world.stations, &world.bodies, &world.eph,
        &mut world.corporations, &world.config.exchange, &world.config.goods_cfg,
        world.tick(), &mut world.events,
    );
    // Goods still unloaded.
    assert_eq!(world.ships.hold[0].len(), 0, "goods unloaded even when Exchange broke");
    // Credits unchanged (saturating — no negative treasury draw).
    assert_eq!(world.ships.credits_micros[0], credits_before, "no credit movement when Exchange broke");
    // Exchange treasury cannot go negative.
    assert_eq!(world.corporations.treasury_micros[0], 0, "Exchange treasury saturates at 0");
}

#[test]
fn credit_identity_holds_across_trade_buy_and_sell() {
    // Runs a multi-tick loop through buy + transit + sell, checking
    // Σtreasury + Σcredits + Σescrow == t0 every tick.
    // This is the credit-identity extension for the Exchange legs.
    // (fixture: one hauler, one source station, one sink station, Exchange corp,
    //  exchange.active=true, goods_cfg with 1 good.)
    let (mut world, t0_sum) = credit_identity_trade_fixture();
    for _ in 0..50 {
        world.step();
        let total: i64 = world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>();
        assert_eq!(total, t0_sum, "credit identity violated after trade step");
    }
}
```

Run: `cargo test -p jumpgate-core trade_buy_settles_stock_to_hold`

Expected failure: compile error — `resolve_trade_buys` does not exist yet.

- [ ] **Step 2: implement resolve_trade_buys**

```rust
// crates/jumpgate-core/src/economy.rs

/// Own-trade BUY settle stage — stage 1dx, goods-as-goods rung A.
/// Consumes EVERY `pending_trade_buy` intent this tick (always-consume-then-gate).
/// Goods leg: stock ↔ hold TRANSFER. NO `consumed[]` counter (not a trophic sink).
/// Money leg: wallet -> Exchange treasury (pure transfer).
/// Six deterministic skip arms: undocked, zero stock, wallet short, hold full,
/// price<1 (L2-C3), stale Exchange corp.
#[allow(clippy::too_many_arguments)]
pub fn resolve_trade_buys(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    exchange: &crate::config::ExchangeCfg,
    goods_cfg: &crate::config::GoodsCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    // Structural off: if Exchange is inactive, consume all intents and return.
    if !exchange.active {
        for intent in ships.pending_trade_buy.iter_mut() { *intent = None; }
        return;
    }
    let ex_row = exchange.corp_index as usize;
    let prev = Tick(tick.0.saturating_sub(1));

    for crow in 0..ships.ids.len() {
        let Some((good, qty, src_sid)) = ships.pending_trade_buy[crow] else {
            continue;
        };
        // ALWAYS consume the intent unconditionally first (always-consume-then-gate).
        ships.pending_trade_buy[crow] = None;

        // (1) Undocked skip.
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };
        // Verify the dock matches the intended source station.
        let Some(dock_sid) = stations.ids.id_at(srow).map(|(s, g)| StationId { slot: s, generation: g }) else {
            continue;
        };
        if dock_sid != src_sid {
            continue; // station moved between policy write and settle (shouldn't happen in v1 but guard it)
        }

        let good_r = good.0 as usize;
        if good_r >= goods_cfg.goods.len() {
            continue;
        }
        let unit_mass_milli = goods_cfg.goods[good_r].unit_mass_milli as i64;

        // (2) Price < 1 guard (L2-C3 — before any integer division).
        let unit_price = stations.price_micros[srow][good_r];
        if unit_price < 1 {
            continue;
        }
        // (3) Zero stock skip.
        let stock = stations.stock[srow][good_r];
        if stock <= 0 {
            continue;
        }
        // (4) Stale Exchange corp skip.
        if corporations.ids.id_at(ex_row).is_none() {
            continue;
        }
        // (5) Integer capacity and afford calculation.
        let hold_used_milli: i64 = ships.hold[crow]
            .iter()
            .map(|(g, q)| *q as i64 * goods_cfg.goods[g.0 as usize].unit_mass_milli as i64)
            .sum();
        let cap_eff = effective_params(&ships.spec[crow], &ships.mods[crow]).base_cargo_capacity as i64;
        let free_milli = cap_eff * 1000 - hold_used_milli;
        if free_milli < unit_mass_milli {
            // (6) Hold full skip.
            continue;
        }
        let max_by_cap = (free_milli / unit_mass_milli) as i64;
        let afford = ships.credits_micros[crow].max(0) / unit_price;
        if afford < 1 {
            // (7) Wallet short skip.
            continue;
        }
        let units = (qty as i64).min(stock).min(max_by_cap).min(afford);
        if units < 1 {
            continue;
        }
        let cost = units.saturating_mul(unit_price);

        // GOODS LEG (TRANSFER — no consumed[] counter, the try_load precedent):
        stations.stock[srow][good_r] -= units;
        // Merge into hold (canonical: ascending Good, no zero qty).
        if let Some(entry) = ships.hold[crow].iter_mut().find(|(g, _)| *g == good) {
            entry.1 = entry.1.saturating_add(units as u32);
        } else {
            ships.hold[crow].push((good, units as u32));
            ships.hold[crow].sort_unstable_by_key(|(g, _)| g.0);
        }

        // MONEY LEG (pure wallet -> Exchange treasury transfer):
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(cost);
        corporations.treasury_micros[ex_row] =
            corporations.treasury_micros[ex_row].saturating_add(cost);

        // NAV re-derivation: if already Seeking (rare — trust the intent stage chose
        // the right dock), leave nav unchanged; the craft will move after sell.

        // EVENT (single-emit site):
        let craft = ships.ids_at(crow);
        events.emit(Event {
            tick,
            kind: EventKind::TradeBought {
                craft,
                station: dock_sid,
                good,
                qty: units as u32,
                price_micros: unit_price,
            },
        });
    }
}
```

- [ ] **Step 3: implement resolve_trade_sells**

```rust
// crates/jumpgate-core/src/economy.rs

/// Own-trade SELL settle stage — stage 1dx, goods-as-goods rung A.
/// Consumes EVERY `pending_trade_sell` intent this tick (always-consume-then-gate).
/// Goods leg: hold ↔ stock TRANSFER. NO `consumed[]` counter.
/// Money leg: Exchange treasury -> wallet (saturating — Exchange may be broke;
/// goods still transfer per spec §1.2 "saturating at the Exchange").
/// Four skip arms: undocked, empty hold, price<1 (L2-C3), stale Exchange corp.
#[allow(clippy::too_many_arguments)]
pub fn resolve_trade_sells(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    exchange: &crate::config::ExchangeCfg,
    goods_cfg: &crate::config::GoodsCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    if !exchange.active {
        for intent in ships.pending_trade_sell.iter_mut() { *intent = None; }
        return;
    }
    let ex_row = exchange.corp_index as usize;
    let prev = Tick(tick.0.saturating_sub(1));

    for crow in 0..ships.ids.len() {
        let Some(dest_sid) = ships.pending_trade_sell[crow] else {
            continue;
        };
        ships.pending_trade_sell[crow] = None; // always consume

        // (1) Undocked skip.
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };
        let Some(dock_sid) = stations.ids.id_at(srow).map(|(s, g)| StationId { slot: s, generation: g }) else {
            continue;
        };
        if dock_sid != dest_sid {
            continue;
        }
        // (2) Empty hold skip.
        if ships.hold[crow].is_empty() {
            continue;
        }
        // (4) Stale Exchange corp skip.
        if corporations.ids.id_at(ex_row).is_none() {
            continue;
        }

        // Sell each good in the hold at this station's price.
        // Drain the hold, accumulate revenue.
        let hold_snapshot: Vec<(Good, u32)> = ships.hold[crow].drain(..).collect();
        for (good, qty) in hold_snapshot {
            let good_r = good.0 as usize;
            if good_r >= goods_cfg.goods.len() {
                continue;
            }
            // (3) Price < 1 guard (L2-C3).
            let unit_price = stations.price_micros[srow][good_r];
            if unit_price < 1 {
                // Put back (can't sell at zero price — keep the goods).
                ships.hold[crow].push((good, qty));
                continue;
            }
            let revenue = (qty as i64).saturating_mul(unit_price);

            // GOODS LEG (TRANSFER — no consumed[] counter):
            stations.stock[srow][good_r] =
                stations.stock[srow][good_r].saturating_add(qty as i64);

            // MONEY LEG (saturating — Exchange may be broke):
            let pay = revenue.min(corporations.treasury_micros[ex_row].max(0));
            corporations.treasury_micros[ex_row] =
                corporations.treasury_micros[ex_row].saturating_sub(pay);
            ships.credits_micros[crow] =
                ships.credits_micros[crow].saturating_add(pay);

            // EVENT (single-emit site — one per good lot sold):
            let craft = ships.ids_at(crow);
            events.emit(Event {
                tick,
                kind: EventKind::TradeSold {
                    craft,
                    station: dock_sid,
                    good,
                    qty,
                    price_micros: unit_price,
                },
            });
        }
        // Re-sort hold to canonical form (ascending Good) after any not-sold items.
        ships.hold[crow].sort_unstable_by_key(|(g, _)| g.0);
    }
}
```

- [ ] **Step 4: extend assert_resource_identity to include hold sum (A2 obligation)**

The cross-beat gotcha (ground-craft-stores.md §GOTCHA) says: "After A2 adds the hold
column for own-cargo traders, the resource accounting identity breaks unless hold sum
is also included — this is a same-commit obligation in A2." If A2 did not yet land this,
it lands here (the first commit that actually reads hold in production code).

```rust
// crates/jumpgate-core/src/world.rs — in assert_resource_identity:
fn assert_resource_identity(world: &World, initial: &[i64; crate::economy::N_RESOURCES]) {
    for r in 0..crate::economy::N_RESOURCES {
        let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
        let in_transit: i64 = world
            .ships
            .cargo
            .iter()
            .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
            .sum();
        // A3 extension: hold sum (own-trade goods in flight between buy and sell).
        let in_hold: i64 = world
            .ships
            .hold
            .iter()
            .flat_map(|h| h.iter())
            .filter_map(|(g, q)| (g.0 as usize == r).then_some(*q as i64))
            .sum();
        let lhs = stock + in_transit + in_hold;
        let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
        assert_eq!(
            lhs, rhs,
            "resource identity for r={r}: {lhs} != {rhs} (stock+in_transit+in_hold vs initial+mined-consumed)"
        );
    }
}
```

- [ ] **Step 5: wire into World::step at stage 1dx**

```rust
// crates/jumpgate-core/src/world.rs — after stage 1d2 (resolve_refuels):
// (1dx-a) own-trade buy settle (goods-as-goods rung A): consume every
//         pending_trade_buy written by this tick's ingest or scripted
//         trade policy. AFTER resolve_refuels, PRE-physics.
//         Goods leg: stock->hold TRANSFER (no consumed[] counter).
//         Money leg: wallet->Exchange treasury.
crate::economy::resolve_trade_buys(
    &mut self.ships,
    &mut self.stations,
    &self.bodies,
    &self.eph,
    &mut self.corporations,
    &self.config.exchange,
    &self.config.goods_cfg,
    next,
    &mut self.events,
);

// (1dx-b) own-trade sell settle (goods-as-goods rung A): consume every
//         pending_trade_sell. Goods leg: hold->stock TRANSFER.
//         Money leg: Exchange treasury->wallet (saturating).
crate::economy::resolve_trade_sells(
    &mut self.ships,
    &mut self.stations,
    &self.bodies,
    &self.eph,
    &mut self.corporations,
    &self.config.exchange,
    &self.config.goods_cfg,
    next,
    &mut self.events,
);
```

- [ ] **Step 6: run all tests**

```
cargo test -p jumpgate-core trade_buy_settles_stock_to_hold_no_consumed_counter
cargo test -p jumpgate-core trade_buy_skips_deterministically
cargo test -p jumpgate-core trade_sell_settles_hold_to_stock_exchange_pays
cargo test -p jumpgate-core trade_sell_exchange_saturates_when_broke
cargo test -p jumpgate-core credit_identity_holds_across_trade_buy_and_sell
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. State-hash goldens unchanged (new events are unhashed; hold column
was already folded in A2; the only change to hashed state is stock/hold/credits/treasury
which the identity test covers, not the golden).

- [ ] **Step 7: commit**

```bash
git add crates/jumpgate-core/src/economy.rs \
        crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(economy): resolve_trade_buys/sells stage 1dx — own-trade settle stages

Goods leg is a stock<->hold TRANSFER (no consumed[] counter, the try_load
precedent). Money leg is wallet<->Exchange treasury (sell leg saturating per
spec §1.2). Six buy skip arms + four sell skip arms, all with price<1 guard
(L2-C3). Credit identity test extended with Exchange legs. assert_resource_identity
extended to include hold sum (A2 obligation completed here).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A3.7: Exchange drain instrument (A0 standing read)

Adds the Exchange-drain printed line as a standing read instrument per OD-2 and
synthesis cut §1.2 ("drain printed as a standing read — 'we're not standing up the
economy out of whole cloth, we're laying the tubes'"). This is scenario-blind
(it reads the Exchange treasury from TrophicSample/console output) and lands in the
A0 instrument group — but as the config is already set (A3.2), it lands here after
the Exchange corp exists.

> **Lockstep rule:** the anchored stdout line `EXCHANGE` and its matching regex in
> `sweep_trophic.py` must land in the **same commit**. Adding the `println!` without
> simultaneously adding the regex to `ANCHORED` in `sweep_trophic.py` and appending a
> versioned fixture to `test_sweep_parsing.py` silently drops the data from every
> sweep run.

#### Files

- Modify: `python/jumpgate/trophic_run.rs` (or the Rust console printer) — add EXCHANGE line
- Modify: `python/jumpgate/sweep_trophic.py` — add EXCHANGE to ANCHORED regex map
- Modify: `python/tests/test_sweep_parsing.py` — append V6 fixture line

- [ ] **Step 1: failing test — EXCHANGE line is parsed by sweep_trophic**

```python
# python/tests/test_sweep_parsing.py — append a new fixture with the EXCHANGE line.
FIXTURE_V6_WITH_EXCHANGE = """\
META seed=1 ticks=1000 stations=10 haulers=12 pirates=3
EXCHANGE treasury_micros=5000000000 drain_per_100k=0
VERDICT boom_bust
"""

def test_exchange_line_is_parsed():
    from jumpgate.sweep_trophic import parse_run_output
    result = parse_run_output(FIXTURE_V6_WITH_EXCHANGE)
    assert result["exchange_treasury_micros"] == 5_000_000_000
    assert result["exchange_drain_per_100k"] == 0
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_exchange_line_is_parsed`

Expected failure: `KeyError: 'exchange_treasury_micros'` — the EXCHANGE line is not
emitted yet.

- [ ] **Step 2: add EXCHANGE line to the Rust console printer**

In `trophic_run.rs` (the per-run stdout printer, after the META line and before
VERDICT), add:

```rust
// EXCHANGE line: standing read for OD-2 drain monitoring.
// Emitted when exchange_cfg.active is true; zero is printed when inactive
// (the inactive case is also a valid read: "Exchange is not live this run").
let ex_row = cfg.exchange.corp_index as usize;
let ex_treasury = if ex_row < world.corporations.treasury_micros.len() {
    world.corporations.treasury_micros[ex_row]
} else {
    0
};
// drain_per_100k: treasury_at_start minus treasury_now, normalized to 100k ticks.
// treasury_at_start is available from the run summary struct; print 0 for now
// (full drain tracking requires a baseline from reset — forward-hooked for console calibration).
println!("EXCHANGE treasury_micros={} drain_per_100k=0", ex_treasury);
```

- [ ] **Step 3: add EXCHANGE to sweep_trophic.py ANCHORED regex**

```python
# python/jumpgate/sweep_trophic.py — in the ANCHORED dict:
"EXCHANGE": re.compile(
    r"EXCHANGE\s+treasury_micros=(?P<exchange_treasury_micros>-?\d+)"
    r"\s+drain_per_100k=(?P<exchange_drain_per_100k>-?\d+)"
),
```

- [ ] **Step 4: run tests**

```
PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_exchange_line_is_parsed
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: Python test passes; Rust tests unaffected.

- [ ] **Step 5: commit (lockstep: println + regex + fixture in same commit)**

```bash
git add crates/jumpgate-core/src/trophic_run.rs \
        python/jumpgate/sweep_trophic.py \
        python/tests/test_sweep_parsing.py
git commit -F - <<'EOF'
feat(instrument): add EXCHANGE standing read — treasury + drain_per_100k

OD-2 solvency honesty: the Exchange-drain is printed as a standing read
(never a gate). Lockstep: println! + ANCHORED regex + V6 fixture land
together. drain_per_100k=0 placeholder; full drain tracking hooked for
console calibration.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

## Summary of tasks

| ID | Title | Key obligation |
|----|-------|---------------|
| A3.1 | Per-good live pricing generalization | `update_prices` over `goods_cfg.goods.len()`; `cap==0` skip preserved; `Resource::ALL` → `Good(r)` in PriceUpdate |
| A3.2 | GoodsCfg + ArbitrageCfg + ExchangeCfg config + GOLDEN_CONFIG_HASH re-pin | ONE config re-pin; GoodsCfg count-first fold; name never folded; all defaults inert; arb_premium_micros per-corp |
| A3.3 | pending_trade_buy/sell columns + debug_asserts | Hash-neutral transient columns; all-None asserts in state_hash |
| A3.4 | TradeBought/TradeSold events + delete EventKind::Trade | Same commit: event variants + chronicle arms + gossip-log arms + exhaustive matches; dead Trade deleted |
| A3.5 | run_trade_policies stage 1c3x | Pirates + !scripted skipped; sell-before-buy; price<1 guard; wallet headroom; Exchange.active inert gate |
| A3.6 | resolve_trade_buys + resolve_trade_sells stage 1dx | Stock↔hold TRANSFER (no consumed[]); money=wallet↔Exchange treasury; sell saturating; 6+4 skip arms; assert_resource_identity extended with hold sum |
| A3.7 | Exchange drain instrument | EXCHANGE println + sweep regex + fixture in same commit (lockstep rule) |
