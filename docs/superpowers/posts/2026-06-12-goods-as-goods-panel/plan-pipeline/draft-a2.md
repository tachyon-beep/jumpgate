# Phase A2 — Format v6: the `hold` column + Food as Good(2)

> Section of the goods-as-goods rung-A plan. Phase A2 follows A1 (runtime
> goods representation, hash-neutral) and A0 (instruments). It has exactly ONE
> state-hash cause (the `hold` column) and ONE new good (Food, `Good(2)`, with
> a consumption recipe). Every golden literal is derived by running the
> print-fixture; no literal is invented here.

---

### Task A2.1: `hold` column skeleton — struct, push, empty, reset

**Files**

- Modify: `crates/jumpgate-core/src/stores.rs`
  - `CraftStore` struct (around line 215, after `gossip` field)
  - `CraftStore::empty()` (around line 229–254)
  - `CraftStore::push()` (around line 261–292)
- Modify: `crates/jumpgate-core/src/world.rs`
  - `World::reset` craft mint loop (around line 334–338, after `gossip.push`)
- Modify: `crates/jumpgate-core/src/hash.rs`
  - `write_craft_economy` (after line 367, after `info_tick.0` word 28)
  - `manual_zero_fold` (after the word-28 comment, around line 1207)
  - `HASH_FORMAT_VERSION` (line 126: 5 → 6)
  - `GOLDEN_ZERO_STATE_HASH` constant (line 132) — DERIVED by builder
  - `state_hash_golden_zero_world` assertion (line 1118) — DERIVED by builder
- Modify: `crates/jumpgate-core/src/scenario.rs`
  - `FRONTIER_TRAJECTORY_GOLDEN` constant (around line 1118) — DERIVED by builder

> This is the ONLY commit that bumps HASH_FORMAT_VERSION. All golden literals
> are re-derived by the builder running the print fixtures named in step 5.

- [ ] **Step 1: Write the failing test for the hold column**

Add the following test to `crates/jumpgate-core/src/hash.rs` (inside the
`#[cfg(test)]` block, after `golden_zero_state_hash`):

```rust
#[test]
fn hold_column_folds_into_state_hash_v6() {
    // The `hold` column must be folded after word 28 (info_tick) in
    // write_craft_economy. A non-empty hold must produce a different hash
    // than an empty hold. HASH_FORMAT_VERSION must be 6.
    assert_eq!(
        HASH_FORMAT_VERSION,
        6,
        "A2 requires HASH_FORMAT_VERSION=6; update the version before this test passes"
    );
    let (mut w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
    let base = state_hash(&w);

    // Mutate the hold of craft 0 to contain one unit of Good(0).
    // Before A2 this field does not exist — this test fails to compile.
    w.ships.hold[0].push((crate::economy::Good(0), 1));
    let with_hold = state_hash(&w);
    assert_ne!(
        base, with_hold,
        "a non-empty hold must move the state hash (hold fold is missing or wrong)"
    );

    // An empty Vec must produce the same hash as the base (pirate-row discipline).
    w.ships.hold[0].clear();
    let cleared = state_hash(&w);
    assert_eq!(
        base, cleared,
        "clearing the hold must restore the original hash"
    );
}
```

Run the test (expected failure — does not compile yet because `ships.hold` does not exist):

```
cargo test -p jumpgate-core hold_column_folds_into_state_hash_v6 2>&1 | head -20
```

Expected output contains:
```
error[E0609]: no field `hold` found in type `CraftStore`
```

- [ ] **Step 2: Add `hold` field to `CraftStore`**

In `crates/jumpgate-core/src/stores.rs`, in the `CraftStore` struct after the
`gossip` field (around line 215):

```rust
    // --- Goods-rung columns (HASHED v6+) ---
    /// Owned-cargo hold for own-trade craft; canonical ascending-Good no-zero-qty form.
    /// Pirates get `Vec::new()` — they never become own-traders (D6/D7).
    /// Fold: count-first after word 28 in `write_craft_economy`.
    pub hold: Vec<Vec<(crate::economy::Good, u32)>>,
```

In `CraftStore::empty()` (around line 252, after `gossip: Vec::new()`):

```rust
            hold: Vec::new(),
```

In `CraftStore::push()` (around line 290, after `self.gossip.push(None)`):

```rust
        // Goods-rung (v6): all craft start with an empty hold (pirates never
        // fill theirs; the empty-vec count word keeps the fold uniform).
        self.hold.push(Vec::new());
```

In `World::reset` craft mint loop in `crates/jumpgate-core/src/world.rs`
(after the `ships.gossip.push(...)` block at around line 338):

```rust
            // Goods-rung hold (v6): empty for all craft including pirates.
            ships.hold.push(Vec::new());
```

- [ ] **Step 3: Add the hold fold to `write_craft_economy` in `hash.rs`**

In `crates/jumpgate-core/src/hash.rs`, in `write_craft_economy` after the
`info_tick.0` line (after line 367):

```rust
    // HASH_FIELD_ORDER v6 hold: count-first, then (good.0, qty) per entry.
    // Canonical form: ascending Good, no zero-qty entries. Pirates hold zeros
    // (Vec::new()); the count word (0) is the self-delimiting boundary.
    let hold = &world.ships.hold[idx];
    h.write_u64(hold.len() as u64);
    for (good, qty) in hold {
        h.write_u64(good.0 as u64);
        h.write_u64(*qty as u64);
    }
```

Also add the same fold to `recompute_with_cursors` in the same file (after the
`super::write_craft_economy(&mut h, w, idx);` call at around line 761 — the
hold fold is INSIDE `write_craft_economy` which is shared, so no extra call is
needed here; verify `recompute_with_cursors` calls `write_craft_economy` not
an inlined copy).

- [ ] **Step 4: Update `manual_zero_fold` in `hash.rs`**

In `manual_zero_fold()` (around line 1207, after the `// 28. info_tick` line):

```rust
        h.write_u64(0); // 28. info_tick
        // v6 hold: one craft row, empty hold -> count word 0.
        h.write_u64(0); // v6 hold count (empty vec on zero-init world)
```

- [ ] **Step 5: Bump `HASH_FORMAT_VERSION` and re-derive ALL goldens**

In `hash.rs` line 126, change:

```rust
pub const HASH_FORMAT_VERSION: u32 = 5;
```

to:

```rust
/// v6: + goods-rung hold column (per-craft `hold: Vec<Vec<(Good, u32)>>`, word after 28).
pub const HASH_FORMAT_VERSION: u32 = 6;
```

**The builder MUST run the print fixtures and paste their output — never invent literals.**

Run the zero-world fixture:

```
cargo test -p jumpgate-core -- print_golden --ignored --nocapture 2>&1
```

This prints two lines, e.g.:
```
GOLDEN=0x<new_value>
GOLDEN_ZERO_STATE_HASH=0x<new_value>
```

Update in `hash.rs`:
- Line 132 `GOLDEN_ZERO_STATE_HASH`: paste the `GOLDEN_ZERO_STATE_HASH=0x...` value.
  Add a provenance comment: `// RE-PINNED: HASH_FORMAT_VERSION 5->6 (+hold column after word 28). Was 0x0f20_843f_ccfd_8c70.`
- Line 1118 `state_hash_golden_zero_world` assertion: paste the `GOLDEN=0x...` value.
  Add a provenance comment: `// RE-PINNED: HASH_FORMAT_VERSION 5->6 (+hold column after word 28). Was 0x274b_6874_3b8d_2700.`

Run the frontier trajectory fixture:

```
cargo test -p jumpgate-core -- print_golden_frontier --ignored --nocapture 2>&1
```

This prints one line:
```
FRONTIER_TRAJECTORY_GOLDEN=0x<new_value>
```

Update in `scenario.rs` (around line 1118):
- `FRONTIER_TRAJECTORY_GOLDEN`: paste the new value.
  Add a provenance comment: `// RE-PINNED: v5->v6 (+hold). Was 0x050de98bd4b6793c.`

- [ ] **Step 6: Run the full test suite and verify all pass**

```
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass. In particular:
- `hold_column_folds_into_state_hash_v6` passes
- `golden_zero_state_hash` passes (manual_zero_fold matches GOLDEN_ZERO_STATE_HASH)
- `state_hash_golden_zero_world` passes
- `frontier_trajectory_golden` passes
- No other tests regress

Also run:
```
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 7: Commit (single-cause: the hold column)**

```bash
git add \
  crates/jumpgate-core/src/stores.rs \
  crates/jumpgate-core/src/world.rs \
  crates/jumpgate-core/src/hash.rs \
  crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(hash): v6 — per-craft `hold` column, HASH_FORMAT_VERSION 5->6

Adds `hold: Vec<Vec<(Good, u32)>>` to CraftStore (SoA, all roles incl.
pirates which hold Vec::new()). The count-first fold after word 28
(info_tick) in write_craft_economy is the ONE cause of the v6 bump.
All three goldens re-derived via print fixtures; manual_zero_fold
updated with the empty-hold count word. No new behavior; no config
change; no GOLDEN_CONFIG_HASH re-pin.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A2.2: `assert_resource_identity` — extend in-transit sum to include `hold`

**Files**

- Modify: `crates/jumpgate-core/src/world.rs`
  - `assert_resource_identity` helper (around line 2870)
  - `phase1_gate_resource_accounting_identity_holds_every_tick` test (around line 2888)

The cross-task obligation in the recommended cut §1.2 is explicit: the hold
sum must be included in-transit accounting in the SAME commit as A2. After
A1's Vec-backed stocks, the Good newtype index is used (Good(0)=Ore, Good(1)=Fuel).

- [ ] **Step 1: Write the failing test extension**

Add the following test alongside `phase1_gate_resource_accounting_identity_holds_every_tick`
in `crates/jumpgate-core/src/world.rs`:

```rust
#[test]
fn hold_participates_in_resource_identity() {
    // After A2, assert_resource_identity must count units in ships.hold[]
    // as in-transit goods. This test manually places a Good(0) unit into
    // a craft hold and verifies the identity still holds with the updated
    // helper. Without the hold sum, the identity equation is unbalanced.
    use crate::economy::N_RESOURCES;
    let (mut world, _) =
        World::reset(full_stage1_self_running_fixture()).expect("resolvable cfg");
    let initial: Vec<i64> = (0..N_RESOURCES)
        .map(|r| world.stations.stock.iter().map(|s| s[r]).sum())
        .collect();
    // Place one unit of Good(0)=Ore into craft 0's hold and simultaneously
    // remove it from station 0's stock to keep the identity balanced — then
    // verify assert_resource_identity does not panic.
    if world.stations.stock[0][0] > 0 {
        world.stations.stock[0][0] -= 1;
        world.ships.hold[0].push((crate::economy::Good(0), 1));
        // If assert_resource_identity doesn't sum hold, it will assert-fail here.
        assert_resource_identity(&world, &initial);
        // Restore.
        world.ships.hold[0].clear();
        world.stations.stock[0][0] += 1;
    }
}
```

Run (expected failure — `assert_resource_identity` does not yet sum `hold`):

```
cargo test -p jumpgate-core hold_participates_in_resource_identity 2>&1 | tail -10
```

Expected output contains:
```
thread '...' panicked at 'resource identity for r=0
```

- [ ] **Step 2: Update `assert_resource_identity` to include `hold`**

In `crates/jumpgate-core/src/world.rs`, replace the `assert_resource_identity`
function body (around line 2870):

```rust
fn assert_resource_identity(world: &World, initial: &[i64]) {
    use crate::economy::N_RESOURCES;
    for r in 0..N_RESOURCES {
        let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
        let in_cargo: i64 = world
            .ships
            .cargo
            .iter()
            .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
            .sum();
        // v6: own-cargo hold also participates in the identity.
        let in_hold: i64 = world
            .ships
            .hold
            .iter()
            .flat_map(|h| h.iter())
            .filter_map(|(good, q)| (good.0 as usize == r).then_some(*q as i64))
            .sum();
        let lhs = stock + in_cargo + in_hold;
        let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
        assert_eq!(
            lhs, rhs,
            "resource identity for r={r}: {lhs} != {rhs} (stock+cargo+hold vs initial+mined-consumed)"
        );
    }
}
```

Also update the signature of `assert_resource_identity` call sites — the
initial array type changes from `&[i64; N_RESOURCES]` to `&[i64]` (a slice,
so both `Vec<i64>` and `[i64; N]` callers work). Update the call in
`phase1_gate_resource_accounting_identity_holds_every_tick` to build initial
as a `Vec<i64>`:

```rust
let initial: Vec<i64> = (0..crate::economy::N_RESOURCES)
    .map(|r| world.stations.stock.iter().map(|s| s[r]).sum())
    .collect();
```

And in any other test that calls `assert_resource_identity`, update initial
construction to `Vec<i64>` with the same pattern (grep for the call sites first:
`scripted_dispatch_makes_stage1_loop_self_run` at world.rs:2591 also calls it
via the inline `check_identity` closure — update that closure too).

- [ ] **Step 3: Run the test suite**

```
cargo test -p jumpgate-core 2>&1 | tail -20
```

Expected: all tests pass including the new `hold_participates_in_resource_identity`
and the existing identity tests.

- [ ] **Step 4: Commit**

```bash
git add crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
fix(world): extend assert_resource_identity to include hold (v6 obligation)

The in-transit accounting identity must sum ships.hold[] alongside
ships.cargo[] after the v6 hold column lands. Own-cargo traders' held
goods participate in stock+mined-consumed conservation the same way
in-flight contract cargo does.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A2.3: `pending_trade_buy` / `pending_trade_sell` transient columns + debug asserts

**Files**

- Modify: `crates/jumpgate-core/src/stores.rs`
  - `CraftStore` struct — add two `Vec<Option<...>>` transient columns
  - `CraftStore::empty()` — add two `Vec::new()`
  - `CraftStore::push()` — push `None` for each
- Modify: `crates/jumpgate-core/src/world.rs`
  - `World::reset` craft mint loop — push `None` for each
- Modify: `crates/jumpgate-core/src/hash.rs`
  - `state_hash` — add two `debug_assert!` before `h.finish()`

The recommended cut §1.2 says `pending_trade_buy/sell` follow the same
pending-column discipline as `pending_upgrade/pending_refuel`. These columns
carry intent written by `run_trade_policies` (stage 1c3x) and consumed by
`resolve_trade_buys/sells` (stage 1dx). The asserts fire in debug builds if
any intent is un-consumed at a hash point, catching stage-ordering bugs loudly.

The payload type for `pending_trade_buy` is a buy intent: the station at which
to buy, the Good to buy, and the quantity. For `pending_trade_sell` it is the
station at which to sell (Good and qty are read from the hold at settle time,
or the full hold is sold). Define minimal payload structs here.

- [ ] **Step 1: Write the failing test**

Add to `crates/jumpgate-core/src/hash.rs` inside the `#[cfg(test)]` block:

```rust
#[test]
fn pending_trade_columns_assert_fires_on_leftover_intent() {
    // A pending_trade_buy/sell left unconsumed at a hash point must trip
    // the debug_assert. In release builds this test is vacuous; in debug it
    // catches stage-ordering bugs.
    #[cfg(debug_assertions)]
    {
        let (mut w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        // Simulate a leftover pending_trade_buy (the stage consume forgot to clear).
        use crate::economy::Good;
        w.ships.pending_trade_buy[0] = Some(crate::stores::TradeBuyIntent {
            station_row: 0,
            good: Good(0),
            qty: 1,
        });
        let result = std::panic::catch_unwind(|| {
            crate::hash::state_hash(&w)
        });
        assert!(
            result.is_err(),
            "state_hash must debug_assert-panic when pending_trade_buy is non-None"
        );
        w.ships.pending_trade_buy[0] = None; // restore
    }
}
```

Run (expected failure — the columns don't exist yet):

```
cargo test -p jumpgate-core pending_trade_columns_assert_fires_on_leftover_intent 2>&1 | head -20
```

Expected:
```
error[E0609]: no field `pending_trade_buy` found in type `CraftStore`
```

- [ ] **Step 2: Define intent types and add columns to `CraftStore`**

In `crates/jumpgate-core/src/stores.rs`, after the `PirateState` struct
definition and before `CraftStore`:

```rust
/// Intent written by `run_trade_policies` (stage 1c3x) for a BUY settle.
/// Consumed and cleared by `resolve_trade_buys` (stage 1dx). TRANSIENT — must
/// be all-None at every state-hash point (debug_assert in hash.rs).
#[derive(Clone, Copy, Debug)]
pub struct TradeBuyIntent {
    pub station_row: usize,
    pub good: crate::economy::Good,
    pub qty: u32,
}

/// Intent written by `run_trade_policies` for a SELL settle.
/// The good and quantity are the full hold contents at settle time (or a
/// partial sell in future extensions — v1 sells the whole hold).
#[derive(Clone, Copy, Debug)]
pub struct TradeSellIntent {
    pub station_row: usize,
}
```

In the `CraftStore` struct, after the `pending_refuel` field:

```rust
    // --- Trade-mode transient columns (NOT hashed; all-None at hash points) ---
    /// Pending buy intent for own-trade mode (stage 1c3x → 1dx).
    pub pending_trade_buy: Vec<Option<TradeBuyIntent>>,
    /// Pending sell intent for own-trade mode (stage 1c3x → 1dx).
    pub pending_trade_sell: Vec<Option<TradeSellIntent>>,
```

In `CraftStore::empty()`, after `pending_refuel: Vec::new()`:

```rust
            pending_trade_buy: Vec::new(),
            pending_trade_sell: Vec::new(),
```

In `CraftStore::push()`, after `self.pending_refuel.push(None)`:

```rust
        self.pending_trade_buy.push(None);
        self.pending_trade_sell.push(None);
```

In `World::reset` craft mint loop in `world.rs`, after `ships.pending_refuel.push(None)`:

```rust
            ships.pending_trade_buy.push(None);
            ships.pending_trade_sell.push(None);
```

- [ ] **Step 3: Add `debug_assert!` guards in `state_hash`**

In `crates/jumpgate-core/src/hash.rs`, in `state_hash`, after the existing
`pending_refuel` assert (around line 317) and before `h.finish()`:

```rust
    // `pending_trade_buy/sell` are TRANSIENT trade intent (goods-rung v6):
    // written by run_trade_policies and consumed within the same tick.
    debug_assert!(
        world.ships.pending_trade_buy.iter().all(Option::is_none),
        "pending_trade_buy must be fully consumed (all None) at every state-hash point"
    );
    debug_assert!(
        world.ships.pending_trade_sell.iter().all(Option::is_none),
        "pending_trade_sell must be fully consumed (all None) at every state-hash point"
    );
```

- [ ] **Step 4: Run the test suite**

```
cargo test -p jumpgate-core 2>&1 | tail -20
```

Expected: all tests pass including `pending_trade_columns_assert_fires_on_leftover_intent`.

```
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add \
  crates/jumpgate-core/src/stores.rs \
  crates/jumpgate-core/src/world.rs \
  crates/jumpgate-core/src/hash.rs
git commit -F - <<'EOF'
feat(stores): pending_trade_buy/sell transient columns + debug asserts

Adds TradeBuyIntent / TradeSellIntent types and the two pending-trade
Vec columns to CraftStore (all-None transient, same discipline as
pending_upgrade/pending_refuel). Matching debug_assert!s in state_hash
catch stage-ordering bugs loudly rather than silently corrupting the
hash-point determinism.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A2.4: Food as `Good(2)` — the good definition and consumption recipe in `scenario_bazaar`

**Files**

- Modify: `crates/jumpgate-core/src/economy.rs`
  - Document that `Good` consts include `FOOD = Good(2)` (constant added in A1 alongside ORE/FUEL, or added here if A1 deferred it)
- Modify: `crates/jumpgate-core/src/scenario.rs`
  - `scenario_bazaar` factory: Food consumers (input-only Recipe, fuel-sink shape) at sink stations

The spec §3 says Food ships as a real good WITH a consumption recipe (input-only
Recipe, the fuel-sink shape). The recommended cut §1.3 says
"sink-side input-only recipes (incl. Food consumers) keep spreads re-opening."
Food is Good(2) in the global good ordering; it exists in the GoodsCfg for ALL
scenarios (good index 2 is Food globally) but trophic/frontier have no Food
producers or consumers, so their behavior digests are unaffected.

The key invariant: the Food Recipe uses `Good(2)` via the `Resource`-based
Recipe struct (post-A1: Recipe carries `Good` not `Resource`). The consumption
sinks in scenario_bazaar are standard input-only producers (interval N, qty Q,
input Some(Food), output None) — identical mechanically to the Fuel sinks in
scenario_frontier.

> Note: If A1 has not yet added `Good::FOOD = Good(2)` as a named const, this
> task adds it now. The const must appear in the same file as `Good(u16)`.

- [ ] **Step 1: Write the failing test**

Add to `crates/jumpgate-core/src/scenario.rs` inside the test module:

```rust
#[test]
fn scenario_bazaar_has_food_consumption_recipe() {
    // Food (Good(2)) must appear as input to at least one producer in the
    // bazaar scenario (the fuel-sink shape: input-only, no output). This
    // verifies WA1's supply is non-trivially demanded.
    use crate::economy::Good;
    let cfg = scenario_bazaar(7);
    let food_consumers = cfg
        .producers
        .iter()
        .filter(|p| matches!(p.recipe.input, Some((g, _)) if g == Good::FOOD))
        .filter(|p| p.recipe.output.is_none())
        .count();
    assert!(
        food_consumers >= 1,
        "scenario_bazaar must have at least one Food consumption sink (input-only recipe)"
    );
}

#[test]
fn good_food_has_index_2() {
    // Good::FOOD must be Good(2) — the globally pinned index.
    use crate::economy::Good;
    assert_eq!(Good::FOOD.0, 2, "Food must be index 2 in the Good ordering");
}
```

Run (expected failure — `scenario_bazaar` and `Good::FOOD` do not exist yet):

```
cargo test -p jumpgate-core scenario_bazaar_has_food_consumption_recipe good_food_has_index_2 2>&1 | head -20
```

Expected:
```
error[E0425]: cannot find function `scenario_bazaar` in this scope
```

- [ ] **Step 2: Ensure `Good::FOOD` constant exists**

In `crates/jumpgate-core/src/economy.rs`, in the `Good` impl block (added by A1
alongside `Good::ORE` and `Good::FUEL`):

```rust
    /// Food: Good(2). Exists globally; consumed at bazaar sink stations.
    /// No production recipe on trophic/frontier — those scenarios leave it at zero.
    pub const FOOD: Good = Good(2);
```

Verify `Good::ORE = Good(0)` and `Good::FUEL = Good(1)` are also present (A1
obligation). If A1 placed them, this step only adds FOOD.

- [ ] **Step 3: Add Food consumption producers to `scenario_bazaar`**

In `crates/jumpgate-core/src/scenario.rs`, in the `scenario_bazaar` producer
construction block (mirroring the Fuel sinks in `scenario_frontier`). The
bazaar food sinks go at a subset of designated station rows — the spec calls for
sink-side input-only recipes at stations that are not primary Food sources. Use
3 Food sinks at the same station rows used for Fuel sinks in frontier (rows 3,
4, 8 — the tier dest/sink geometry carries over by the frontier-band inheritance).

The `ProducerInit` shape (from `config.rs:113-115`):

```rust
pub struct ProducerInit {
    pub station_index: usize,
    pub recipe: Recipe,
}
```

And `Recipe` post-A1 uses `Good` not `Resource`:

```rust
pub struct Recipe {
    pub input: Option<(Good, u32)>,
    pub output: Option<(Good, u32)>,
    pub interval: u32,
}
```

In the scenario_bazaar producer block, add after the Ore miners and Fuel sinks:

```rust
    // Food consumption sinks (input-only, fuel-sink shape): qty 5, interval 80
    // at station rows 3, 4, 8 (the same tier-sink geometry as Fuel sinks in
    // scenario_frontier). Keeps the Food spread open after deliveries arrive.
    for sink_row in [3usize, 4, 8] {
        producers.push(ProducerInit {
            station_index: sink_row,
            recipe: Recipe {
                input: Some((Good::FOOD, 5)),
                output: None,
                interval: 80,
            },
        });
    }
```

- [ ] **Step 4: Run the new tests**

```
cargo test -p jumpgate-core scenario_bazaar_has_food_consumption_recipe good_food_has_index_2 2>&1 | tail -10
```

Expected:
```
test scenario_bazaar_has_food_consumption_recipe ... ok
test good_food_has_index_2 ... ok
```

Run the full test suite:

```
cargo test --workspace 2>&1 | tail -20
```

Expected: all tests pass.

```
cargo clippy --all-targets -- -D warnings 2>&1 | tail -10
```

Expected: zero warnings.

- [ ] **Step 5: Verify trophic/frontier behavior digests are unaffected**

The spec §6 says: "state HASHES move with v6, behavior digests must NOT [for
trophic/frontier]."

Run a short baseline digest for trophic scenario (seed 7, 1000 ticks) before
and after this commit and verify stdout + JSONL are byte-identical:

```bash
cargo build -q --release -p jumpgate-core --example trophic_run
cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario trophic --seed 7 --ticks 1000 \
    --jsonl /tmp/gag-a2-trophic-s7.jsonl > /tmp/gag-a2-trophic-s7.stdout
cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed 7 --ticks 1000 \
    --jsonl /tmp/gag-a2-frontier-s7.jsonl > /tmp/gag-a2-frontier-s7.stdout
```

This step is a manual verification check — the builder compares stdout and JSONL
from a pre-A2 build (captured on the A1 tip) against the A2 outputs. If the
outputs differ on trophic or frontier, the Food Recipe has been incorrectly
wired into those scenarios and must be fixed before committing.

Because trophic and frontier configs have no Food producers or consumers, and
GoodsCfg length change is a config-hash change (A3) not a state-hash change,
the only stdout difference allowed is the HASH_FORMAT_VERSION embedded in the
HASH_FORMAT_VERSION = 6 word (if the trophic_run binary prints it) — the
per-tick hash sequence will differ (v6 bumped) but the RESULT verdict and JSONL
window aggregates must be identical.

- [ ] **Step 6: Commit**

```bash
git add \
  crates/jumpgate-core/src/economy.rs \
  crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(scenario): Food as Good(2) + consumption sinks in scenario_bazaar

Adds Good::FOOD = Good(2) constant. scenario_bazaar gets 3 input-only
Food consumption producers (qty=5, interval=80, fuel-sink shape) at
station rows 3/4/8. scenario_trophic and scenario_frontier are
untouched; their behavior digests (verdict/JSONL) are unaffected by
the v6 hold fold because Food has no producers or consumers there.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A2.5: Behavior-digest cross-check — trophic/frontier unchanged at v6

**Files**

- No code changes. This task records the cross-branch digest comparison as an
  explicit step in the plan, making the verification observable in CI and in the
  builder's terminal output.

The spec §6 states: "state-hash sequence equality is available for exactly one
commit: the OD-1 runtime-goods representation commit (no bump). The exit
measurement [for trophic/frontier] is a behavior digest, never cross-bump hash
equality." The behavior digest is sha256 over stdout ∪ JSONL ∪ gossip-log per
(scenario, seed). Here we verify the stdout+JSONL digest is unchanged for
trophic and frontier after the v6 bump: the only permitted difference is
per-tick state hashes embedded in any HASH=... line (which is not an anchored
output line per `ANCHORED` in sweep_trophic.py).

- [ ] **Step 1: Capture A1-tip digests (before A2 commits)**

On the A1 tip (before any A2 commit), capture:

```bash
cargo build -q --release -p jumpgate-core --example trophic_run

for scenario in trophic frontier; do
  for seed in 7 11 13; do
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --scenario $scenario --seed $seed --ticks 2000 \
      --jsonl /tmp/gag-digest-baseline-${scenario}-s${seed}.jsonl \
      > /tmp/gag-digest-baseline-${scenario}-s${seed}.stdout
  done
done

sha256sum /tmp/gag-digest-baseline-*.stdout /tmp/gag-digest-baseline-*.jsonl \
  > /tmp/gag-digest-baseline.sha256
```

Store `/tmp/gag-digest-baseline.sha256` as the A1-tip reference.

- [ ] **Step 2: Capture A2-tip digests (after all A2.1–A2.4 commits)**

On the A2 tip:

```bash
cargo build -q --release -p jumpgate-core --example trophic_run

for scenario in trophic frontier; do
  for seed in 7 11 13; do
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --scenario $scenario --seed $seed --ticks 2000 \
      --jsonl /tmp/gag-digest-a2-${scenario}-s${seed}.jsonl \
      > /tmp/gag-digest-a2-${scenario}-s${seed}.stdout
  done
done

sha256sum /tmp/gag-digest-a2-*.stdout /tmp/gag-digest-a2-*.jsonl \
  > /tmp/gag-digest-a2.sha256
```

- [ ] **Step 3: Compare digests — verify trophic/frontier behavior is unchanged**

```bash
diff /tmp/gag-digest-baseline.sha256 /tmp/gag-digest-a2.sha256
```

Expected: the RESULT lines, JSONL window aggregates, and per-station fuel
stocks are identical for trophic and frontier. Per-tick state hashes embedded
in any internal format are allowed to differ (v6 changes them). If the RESULT
verdict or any JSONL window field differs, there is a bug in A2 that wires Food
into trophic/frontier and it must be fixed.

Note: if the A1 build is not available at the same revision, an equivalent
cross-check is: run trophic/frontier on the A2 tip and compare against banked
artifacts (stored in `docs/superpowers/posts/` if previously captured). The
invariant is that RESULT verdict and JSONL are semantically unchanged; only the
per-tick HASH_FORMAT_VERSION-sensitive state hashes move.

- [ ] **Step 4: Record the digest comparison result as a comment**

No commit required. The builder records the diff output (or "no diff in
anchored fields") in a short comment in the task tracking system or as a commit
message note in the next phase's commit.

---

## Summary of A2 commits (landing order)

1. **A2.1** `feat(hash): v6 — per-craft hold column, HASH_FORMAT_VERSION 5->6`
   - Single cause: hold fold after word 28.
   - ALL three goldens re-derived by running `print_golden` and `print_golden_frontier`.
   - `manual_zero_fold` updated with the empty-hold word.

2. **A2.2** `fix(world): extend assert_resource_identity to include hold`
   - Same-commit obligation from the recommended cut §1.2.
   - In-transit term gains `in_hold` sum.

3. **A2.3** `feat(stores): pending_trade_buy/sell transient columns + debug asserts`
   - `TradeBuyIntent` / `TradeSellIntent` types.
   - `pending_trade_buy/sell` Vec columns, all-None, debug_assert in state_hash.

4. **A2.4** `feat(scenario): Food as Good(2) + consumption sinks in scenario_bazaar`
   - `Good::FOOD = Good(2)` const.
   - 3 input-only Food consumption producers in scenario_bazaar.
   - trophic/frontier behavior digests verified unchanged (stdout+JSONL).

5. **A2.5** (verification step — no commit) behavior digest cross-check recorded.
