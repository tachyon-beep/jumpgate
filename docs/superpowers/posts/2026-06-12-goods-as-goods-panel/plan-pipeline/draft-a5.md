# Phase A5 — Two-mode policy + `scenario_bazaar`

**Section scope:** the per-trip channel decision at the ASSIGN write site; the
`scenario_bazaar` factory on the frontier 10-station band geometry with clumped
per-good topology; factory invariant tests; `GOLDEN_CONFIG_HASH` re-pin.

**Dependencies:** A0 instruments landed; A1 `Good(u16)` + Vec columns; A2 hold
column + v6 hash bump; A3 single config commit with `GoodsCfg`, `ArbitrageCfg`,
`ExchangeCfg`, `CraftInit` trade-policy dials.

**Rung-B prohibition:** no `fence_discount_milli`, no `toll_milli`, no
`greed_milli`, no `JetsamStore`, no `CrateCfg` — all rung B.

---

### Task A5.1: two-mode policy at the ASSIGN write site

**What it does.** Replace the single-mode package claim at `economy.rs:639-641`
with a scored comparison: if the craft is sufficiently capitalized AND
`best_trade_net > best_wage_net`, write `pending_trade_buy` instead of claiming
a package contract; otherwise claim the package (unchanged path). Empty-hold gate
added for package claims (panel L3-M3). Pirates and `!scripted` craft are skipped
in the new policy arm (the refuel-policy precedent; without this rung-A pirates
become own-traders and D7's split silently breaks).

The transport cost for both sides comes from the **factory-time integer table**
baked in `scenario_bazaar` (a `Vec<Vec<i64>>` of `transport_micros[from][to]`
folded in config — the same static ring-radius geometry used for contract wages).
Both `best_wage_net` and `best_trade_net` subtract the same table entry so the
comparison is PDR-0007-compliant and f64-free.

**Threshold:** `wallet ≥ buy_cost + trade_reserve_micros` (a new `CraftInit`
field from A3; default `0` keeps trophic/frontier identical). `buy_cost =
price[good] × qty` where `qty` is the smallest registered lot that clears the
transport floor. `best_trade_net = spread × qty − transport[from][to]` using the
public board's best ask/bid; `best_wage_net = best_offered_reward −
transport[from][to]` for any Offered contract the craft is eligible for.

**Files:**

- Modify: `crates/jumpgate-core/src/economy.rs` — `run_scripted_dispatch` ASSIGN
  block (economy.rs:520-650); add `run_trade_policies` (new function,
  stage 1c3x); add `resolve_trade_buys` / `resolve_trade_sells` (stage 1dx)
- Modify: `crates/jumpgate-core/src/stores.rs` — add `pending_trade_buy:
  Vec<Option<(Good, u32, StationId)>>` and `pending_trade_sell:
  Vec<Option<(Good, u32, StationId)>>` columns to `CraftStore`
- Modify: `crates/jumpgate-core/src/config.rs` — add `trade_reserve_micros: i64`
  to `CraftInit` (default `0`; no new fold word needed until A3 folds it;
  task asserts default leaves trophic/frontier unchanged)
- Modify: `crates/jumpgate-core/src/hash.rs` — add `debug_assert!` for
  `pending_trade_buy` / `pending_trade_sell` all-None at hash points
  (hash.rs:309-318 pattern)
- Modify: `crates/jumpgate-core/src/world.rs` — add `[1c3x]`
  `run_trade_policies` and `[1dx]` `resolve_trade_buys`/`resolve_trade_sells`
  call sites after `run_refuel_policies` / `resolve_refuels` respectively
  (world.rs:833, world.rs:867)

**Step 1: failing test — pending columns exist and are all-None at hash points**

```rust
// economy.rs (test module)
#[test]
fn pending_trade_columns_exist_and_are_always_none_at_hash_point() {
    use crate::scenario::scenario_trophic;
    use crate::world::World;
    use crate::hash::state_hash;

    let (mut w, _) = World::reset(scenario_trophic(7)).expect("reset");
    // After reset: both new columns must exist on the CraftStore and be all-None.
    assert!(
        w.ships.pending_trade_buy.iter().all(Option::is_none),
        "pending_trade_buy must be all-None at reset"
    );
    assert!(
        w.ships.pending_trade_sell.iter().all(Option::is_none),
        "pending_trade_sell must be all-None at reset"
    );
    // Step 50 ticks; the debug_assert! inside state_hash fires if any intent
    // leaks across a tick boundary.
    let mut cmds = Vec::new();
    for _ in 0..50 {
        w.step(&mut cmds);
        // state_hash debug_asserts both columns all-None; if they aren't, this panics
        let _ = state_hash(&w);
    }
}
```

Run: `cargo test -p jumpgate-core pending_trade_columns_exist_and_are_always_none_at_hash_point`

Expected failure: `error[E0609]: no field 'pending_trade_buy' on type 'CraftStore'`

**Step 2: add `pending_trade_buy` / `pending_trade_sell` to `CraftStore`**

In `crates/jumpgate-core/src/stores.rs`, add after the existing transient columns
(after `pending_refuel`, before `gossip`):

```rust
    // --- Rung-A trade-intent columns (TRANSIENT, NOT hashed; all-None debug_assert
    //     in state_hash mirrors the pending_upgrade / pending_refuel discipline) ---
    /// Own-trade BUY intent: (good, qty, source_station). Written by
    /// `run_trade_policies` (stage 1c3x); consumed by `resolve_trade_buys` (1dx).
    pub pending_trade_buy: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>,
    /// Own-trade SELL intent: (good, qty, destination_station). Written by
    /// `run_trade_policies` (stage 1c3x); consumed by `resolve_trade_sells` (1dx).
    pub pending_trade_sell: Vec<Option<(crate::economy::Good, u32, crate::ids::StationId)>>,
```

In `CraftStore::empty()` (`stores.rs:229-254`):
```rust
    pending_trade_buy: Vec::new(),
    pending_trade_sell: Vec::new(),
```

In `CraftStore::push()` (`stores.rs:261-292`), after the `pending_refuel.push(None)` line:
```rust
    self.pending_trade_buy.push(None);
    self.pending_trade_sell.push(None);
```

In `world.rs` reset loop (`world.rs:290-339`), after the `pending_refuel.push(None)` line:
```rust
    ships.pending_trade_buy.push(None);
    ships.pending_trade_sell.push(None);
```

**Step 3: add `debug_assert!` in `state_hash`**

In `crates/jumpgate-core/src/hash.rs`, after the `pending_refuel` assert
(hash.rs:315-318):

```rust
    debug_assert!(
        world.ships.pending_trade_buy.iter().all(Option::is_none),
        "pending_trade_buy must be fully consumed (all None) at every state-hash point"
    );
    debug_assert!(
        world.ships.pending_trade_sell.iter().all(Option::is_none),
        "pending_trade_sell must be fully consumed (all None) at every state-hash point"
    );
```

Run: `cargo test -p jumpgate-core pending_trade_columns_exist_and_are_always_none_at_hash_point`

Expected pass: test passes; 50-tick run on trophic does not panic.

**Step 4: failing test — two-mode policy scores package vs trade, chooses package when broke**

```rust
#[test]
fn two_mode_policy_chooses_package_when_wallet_below_reserve() {
    // A craft with wallet < trade_reserve stays on the package path (wage-hauling
    // when broke — D6). The ASSIGN write site must not write pending_trade_buy
    // when the wallet threshold is not met.
    use crate::scenario::scenario_bazaar;
    use crate::world::World;
    use crate::hash::state_hash;

    let cfg = scenario_bazaar(42);
    let (mut w, _) = World::reset(cfg).expect("reset");

    // Day-0 wallets are 0 (world.rs:311); all craft should stay in package-claim
    // mode at tick 0 and not write pending_trade_buy.
    let mut cmds = Vec::new();
    w.step(&mut cmds);
    assert!(
        w.ships.pending_trade_buy.iter().all(Option::is_none),
        "day-0 wallet=0 craft must not write pending_trade_buy (broke => wage path)"
    );
    // state_hash must not fire the debug_assert
    let _ = state_hash(&w);
}
```

Run: `cargo test -p jumpgate-core two_mode_policy_chooses_package_when_wallet_below_reserve`

Expected failure: `error[E0425]: cannot find function 'scenario_bazaar'` (or compile error because `scenario_bazaar` does not exist yet — the test is the spec for the dependency order; it will pass once A5.2 lands the factory, but must compile before the policy arm is wired in).

**Step 5: add `trade_reserve_micros` to `CraftInit`**

In `crates/jumpgate-core/src/config.rs`, add to `CraftInit`:
```rust
    /// Credits held in reserve before considering own-trade (rung-A two-mode
    /// policy). Default 0 leaves trophic/frontier unchanged. A craft only enters
    /// own-trade mode when `credits_micros >= buy_cost + trade_reserve_micros`.
    pub trade_reserve_micros: i64,
```

Add `trade_reserve_micros: 0` to every existing `CraftInit` struct literal in the
codebase (trophic, frontier factories, all test fixtures). Rust exhaustive struct
literal enforcement will flag every site as a compile error.

Run: `cargo build -p jumpgate-core` — expect compile errors listing every
`CraftInit { ... }` literal missing the new field; add `trade_reserve_micros: 0`
to each. This field is **not folded in config_hash until A3's single config
commit** — its presence does not move any golden.

**Step 6: add `run_trade_policies` (stage 1c3x)**

In `crates/jumpgate-core/src/economy.rs`, add the new function after
`run_refuel_policies`:

```rust
/// Own-trade intent stage (rung A, stage 1c3x). Writes `pending_trade_buy` for
/// craft that are capitalized AND find a better net on own-trade vs the best
/// offered package. Skips pirates and `!scripted` craft (D7 split discipline —
/// the refuel-policy precedent).
///
/// The policy uses the FACTORY-TIME `transport_micros` table from `ArbitrageCfg`
/// (a config-folded integer table, not a live ephemeris read — the L1-M3/L2 tie
/// breaks towards the static table). Both `best_wage_net` and `best_trade_net`
/// subtract the same table value so the comparison is PDR-0007-compliant.
pub fn run_trade_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    contracts: &ContractStore,
    arb: &crate::config::ArbitrageCfg,
    exchange_cfg: &crate::config::ExchangeCfg,
    goods_cfg: &crate::config::GoodsCfg,
    shipyard: &crate::config::ShipyardCfg,
    tick: Tick,
) {
    // Inert gate: arbitrage scanner must be live and the Exchange corp registered.
    if arb.scan_interval == 0 { return; }
    if !exchange_cfg.active { return; }

    let prev = Tick(tick.0.saturating_sub(1));
    let capacity = |crow: usize| -> u32 {
        crate::economy::cargo_capacity(&ships.spec[crow], ships.upgrades[crow], shipyard)
    };

    for crow in 0..ships.ids.len() {
        // Skip pirates — own-trade is dark (D7); the policy gate is the structural
        // mechanism (not a hold check) so it fires even when hold is empty.
        if ships.role[crow] == CraftRole::Pirate { continue; }
        // Skip gym-controlled craft.
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) { continue; }
        // Only Idle craft may choose their next trip.
        if ships.role[crow] != CraftRole::Idle { continue; }
        // Never clobber an ingest-written intent.
        if ships.pending_trade_buy[crow].is_some() { continue; }
        if ships.pending_refuel[crow].is_some() { continue; }

        let wallet = ships.credits_micros[crow];
        let reserve = craft_cfg.get(crow).map_or(0, |c| c.trade_reserve_micros);

        // --- best_wage_net: best offered package reward minus transport cost ---
        let mut best_wage_net: i64 = i64::MIN;
        for kidx in 0..contracts.ids.len() {
            if contracts.status[kidx] != ContractStatus::Offered { continue; }
            if contracts.qty[kidx] > capacity(crow) { continue; }
            let from = contracts.from_station[kidx];
            let to   = contracts.to_station[kidx];
            let frow = match stations.ids.dense_index(from.slot, from.generation) {
                Some(r) => r, None => continue,
            };
            let trow = match stations.ids.dense_index(to.slot, to.generation) {
                Some(r) => r, None => continue,
            };
            let transport = arb.transport_micros
                .get(frow)
                .and_then(|row| row.get(trow))
                .copied()
                .unwrap_or(0);
            let net = contracts.reward_micros[kidx].saturating_sub(transport);
            if net > best_wage_net { best_wage_net = net; }
        }

        // --- best_trade_net: best spread × smallest_lot − transport for any good ---
        let mut best_trade: Option<(Good, u32, usize, usize)> = None; // (good, qty, from, to)
        let mut best_trade_net: i64 = i64::MIN;

        for (gi, _spec) in goods_cfg.goods.iter().enumerate() {
            let good = Good(gi as u16);
            // Find the cheapest source station (non-haven) for this good.
            for from_row in 0..stations.ids.len() {
                let ask = stations.price_micros[from_row][gi];
                if ask < 1 { continue; }
                let source_stock = stations.stock[from_row][gi];
                // Smallest registered lot from ArbitrageCfg.qty_ladder.
                let qty = arb.qty_ladder.iter().copied().next().unwrap_or(0);
                if qty == 0 { continue; }
                if source_stock < qty as i64 { continue; }
                let buy_cost = ask.saturating_mul(qty as i64);
                if wallet < buy_cost.saturating_add(reserve) { continue; }
                // Find best destination sink (highest bid).
                for to_row in 0..stations.ids.len() {
                    if to_row == from_row { continue; }
                    let bid = stations.price_micros[to_row][gi];
                    if bid < 1 { continue; }
                    let dest_stock = stations.stock[to_row][gi];
                    // Only deliver where stock is below cap (there is demand).
                    if let Some(cap) = arb.stock_cap_hint.get(gi).copied() {
                        if dest_stock >= cap { continue; }
                    }
                    let sell_proceeds = bid.saturating_mul(qty as i64);
                    let spread = sell_proceeds.saturating_sub(buy_cost);
                    let transport = arb.transport_micros
                        .get(from_row)
                        .and_then(|r| r.get(to_row))
                        .copied()
                        .unwrap_or(0);
                    let net = spread.saturating_sub(transport);
                    if net > best_trade_net {
                        best_trade_net = net;
                        best_trade = Some((good, qty, from_row, to_row));
                    }
                }
            }
        }

        // --- channel decision (D6): own-trade iff capitalized AND trade wins ---
        // Ties go to the package path (wage path is always available; own-trade
        // needs a strictly better net).
        if best_trade_net > best_wage_net {
            if let Some((good, qty, from_row, _to_row)) = best_trade {
                // Resolve source StationId from dense row.
                if let Some(sid) = stations.ids.id_at(from_row) {
                    ships.pending_trade_buy[crow] = Some((good, qty, sid));
                }
            }
        }
        // If wage path wins (or trade is not better), ASSIGN is unchanged below.
    }
}
```

**Step 7: modify ASSIGN to gate package claims on an empty hold (panel L3-M3)**

In `run_scripted_dispatch` at the candidate-consideration loop (economy.rs:563),
add after the capacity filter:
```rust
            // Empty-hold gate (rung A, panel L3-M3): an own-trader with live
            // hold must not claim a package — keeps the dark/public prey
            // taxonomy exact and the no-double-rob invariant true.
            if !ships.hold[crow].is_empty() { continue; }
```

**Step 8: add `resolve_trade_buys` (stage 1dx)**

```rust
/// Settle stage for own-trade BUY intents (rung A, stage 1dx). Always-consume-
/// then-gate (the `pending_upgrade` shape). Transfers stock → hold on the docked
/// station; debits wallet; credits Exchange treasury. Emits `TradeBought`.
///
/// Price-guard (panel L2-C3): skip if `unit_price < 1`. Transfer only (no
/// `consumed[]` counter — goods are NOT trophic sinks, unlike Fuel; the
/// `try_load` shape at economy.rs:826-827).
pub fn resolve_trade_buys(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    exchange_cfg: &crate::config::ExchangeCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    let prev = Tick(tick.0.saturating_sub(1));
    let exchange_row = exchange_cfg.corp_index as usize;

    for crow in 0..ships.ids.len() {
        // Always consume the intent first.
        let Some((good, qty, sid)) = ships.pending_trade_buy[crow].take() else { continue; };

        // Resolve station row.
        let Some(srow) = stations.ids.dense_index(sid.slot, sid.generation) else { continue; };

        // Dock predicate: prev-frame position, matching try_load discipline
        // (economy.rs:800: `Tick(tick.0.saturating_sub(1))`).
        if !crate::economy::docked_station_row_at(
            &ships, crow, &stations, &bodies, eph, srow, prev,
        ) { continue; }

        // Exchange corp validity.
        if corporations.ids.id_at(exchange_row).is_none() { continue; }

        let unit_price = stations.price_micros[srow][good.0 as usize];
        // L2-C3: unit_price < 1 guard on every buy/sell settle.
        if unit_price < 1 { continue; }

        let stock = stations.stock[srow][good.0 as usize];
        if stock < qty as i64 { continue; }

        let total_cost = unit_price.saturating_mul(qty as i64);
        if ships.credits_micros[crow] < total_cost { continue; }

        // Capacity check: milli-mass gate.
        let used_milli: i64 = ships.hold[crow]
            .iter()
            .map(|(_, q)| *q as i64)
            .sum::<i64>()
            .saturating_mul(1000); // uniform unit_mass_milli = 1000
        let cap_milli = ships.spec[crow].base_cargo_capacity as i64 * 1000;
        if used_milli + (qty as i64).saturating_mul(1000) > cap_milli { continue; }

        // --- Two write legs: stock↔hold TRANSFER, no consumed[] (try_load shape) ---
        stations.stock[srow][good.0 as usize] -= qty as i64;
        // Insert into hold in canonical ascending-Good order.
        let h = &mut ships.hold[crow];
        match h.iter().position(|(g, _)| g.0 >= good.0) {
            Some(pos) if h[pos].0 == good => h[pos].1 += qty,
            Some(pos) => h.insert(pos, (good, qty)),
            None => h.push((good, qty)),
        }

        // Money leg: wallet → Exchange (saturating; Exchange may accumulate debt).
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(total_cost);
        corporations.treasury_micros[exchange_row] =
            corporations.treasury_micros[exchange_row].saturating_add(total_cost);

        events.push(Event {
            tick,
            kind: EventKind::TradeBought {
                craft: ships.ids.id_at(crow).unwrap_or_default(),
                station: sid,
                good,
                qty,
                total_cost_micros: total_cost,
            },
        });
    }
}
```

**Step 9: add `resolve_trade_sells` (stage 1dx, after resolve_trade_buys)**

```rust
/// Settle stage for own-trade SELL intents (rung A, stage 1dx). Transfers
/// hold → stock; credits wallet from Exchange (saturating — Exchange may be
/// depleted); emits `TradeSold`.
pub fn resolve_trade_sells(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    exchange_cfg: &crate::config::ExchangeCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    let prev = Tick(tick.0.saturating_sub(1));
    let exchange_row = exchange_cfg.corp_index as usize;

    for crow in 0..ships.ids.len() {
        let Some((good, qty, sid)) = ships.pending_trade_sell[crow].take() else { continue; };
        let Some(srow) = stations.ids.dense_index(sid.slot, sid.generation) else { continue; };

        if !crate::economy::docked_station_row_at(
            &ships, crow, &stations, &bodies, eph, srow, prev,
        ) { continue; }

        if corporations.ids.id_at(exchange_row).is_none() { continue; }

        let unit_price = stations.price_micros[srow][good.0 as usize];
        if unit_price < 1 { continue; }

        // Verify craft actually holds this good.
        let hold_qty = ships.hold[crow]
            .iter()
            .find(|(g, _)| *g == good)
            .map(|(_, q)| *q)
            .unwrap_or(0);
        if hold_qty < qty { continue; }

        let proceeds = unit_price.saturating_mul(qty as i64);

        // --- Transfer hold → stock (no consumed[]) ---
        stations.stock[srow][good.0 as usize] =
            stations.stock[srow][good.0 as usize].saturating_add(qty as i64);
        let h = &mut ships.hold[crow];
        if let Some(pos) = h.iter().position(|(g, _)| *g == good) {
            if h[pos].1 <= qty {
                h.remove(pos);
            } else {
                h[pos].1 -= qty;
            }
        }

        // Money leg: Exchange → wallet (saturating — spec OD-2: Exchange is a battery).
        corporations.treasury_micros[exchange_row] =
            corporations.treasury_micros[exchange_row].saturating_sub(proceeds);
        ships.credits_micros[crow] =
            ships.credits_micros[crow].saturating_add(proceeds);

        events.push(Event {
            tick,
            kind: EventKind::TradeSold {
                craft: ships.ids.id_at(crow).unwrap_or_default(),
                station: sid,
                good,
                qty,
                proceeds_micros: proceeds,
            },
        });
    }
}
```

**Step 10: wire stages in `world.rs`**

After `run_refuel_policies` call at world.rs:833:
```rust
        // [1c3x] rung-A own-trade intent: scripted capitalized craft that find a
        // better net on own-trade than any offered package write pending_trade_buy.
        // Pirates and !scripted craft are skipped (D7 discipline).
        crate::economy::run_trade_policies(
            &mut self.ships,
            &self.config.craft,
            &self.stations,
            &self.contracts,
            &self.config.arb,
            &self.config.exchange,
            &self.config.goods_cfg,
            &self.config.shipyard,
            next,
        );
```

After `resolve_refuels` at world.rs:867:
```rust
        // [1dx] settle own-trade buy intents (must run after resolve_refuels so
        // pending_refuel is already consumed; PRE-physics so ship pos = prev frame).
        crate::economy::resolve_trade_buys(
            &mut self.ships,
            &mut self.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &self.config.exchange,
            next,
            &mut self.events,
        );
        // [1dx] settle own-trade sell intents (after buy settles so hold is updated).
        crate::economy::resolve_trade_sells(
            &mut self.ships,
            &mut self.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &self.config.exchange,
            next,
            &mut self.events,
        );
```

**Step 11: add `TradeBought`/`TradeSold` event variants; delete dead `EventKind::Trade`**

In `crates/jumpgate-core/src/contract.rs`, the `EventKind` enum (contract.rs:75-80):

Delete `Trade` variant. Add:
```rust
    TradeBought {
        craft: CraftId,
        station: StationId,
        good: crate::economy::Good,
        qty: u32,
        total_cost_micros: i64,
    },
    TradeSold {
        craft: CraftId,
        station: StationId,
        good: crate::economy::Good,
        qty: u32,
        proceeds_micros: i64,
    },
```

The test `economy_event_kinds_are_copy_and_partial_eq` (contract.rs:417) uses
`EventKind::Trade` to prove `Copy + PartialEq`. Replace with `TradeBought {..}`:
```rust
// Replace: let _: EventKind = EventKind::Trade;
let a = EventKind::TradeBought {
    craft: CraftId::default(),
    station: StationId::default(),
    good: crate::economy::Good(0),
    qty: 1,
    total_cost_micros: 1_000,
};
let b = a;
assert_eq!(a, b, "TradeBought is Copy + PartialEq");
```

**Step 12: run tests; expect pass**

```
cargo test -p jumpgate-core -- two_mode_policy_chooses_package_when_wallet_below_reserve pending_trade_columns_exist_and_are_always_none_at_hash_point
cargo test -p jumpgate-core -- economy_event_kinds_are_copy_and_partial_eq
```

Expected: all pass. Trophic/frontier behavior digest unchanged (new stages are
gated by `exchange_cfg.active` which defaults `false`).

**Step 13: commit**

```
git add crates/jumpgate-core/src/economy.rs \
        crates/jumpgate-core/src/stores.rs \
        crates/jumpgate-core/src/config.rs \
        crates/jumpgate-core/src/hash.rs \
        crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/contract.rs
git commit -F - <<'EOF'
feat(rung-a): two-mode policy at ASSIGN write site + pending trade columns

Replaces single-mode package claim with scored channel comparison: capitalized
crafts write pending_trade_buy when best_trade_net > best_wage_net (D6). Both
sides subtract the same factory-time transport table (PDR-0007 compliant, f64-
free). Empty-hold gate added for package claims (panel L3-M3). Pirates and
!scripted craft skipped (D7 split discipline). Deletes dead EventKind::Trade;
adds TradeBought/TradeSold. New pending columns have matching debug_assert! in
state_hash. Exchange.active=false keeps trophic/frontier bit-identical.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A5.2: `scenario_bazaar` factory

**What it does.** A new `pub fn scenario_bazaar(seed: u64) -> RunConfig` factory
in `scenario.rs` on the frontier 10-station band geometry. Key differences from
`scenario_frontier`:

- Zero `ContractInit` rows (`contracts: vec![]` — arbitrage replaces restock, D4)
- `dispatch_cfg.demand_low = 0`, `demand_high = 0` (REPOST structural off)
- `dispatch_cfg.stagger_period = 16` (ASSIGN cadence unchanged)
- `ephemeris_window: 240_000` (spec §1.3)
- 10 goods via `GoodsCfg` (Ore, Fuel, Food, Alloys, Medicine, Machinery,
  Luxuries, Electronics, Textiles, Chemicals — Ore and Fuel at indices 0/1,
  matching Good::ORE / Good::FUEL)
- Fuel base_micros re-baked to `50_000` per OD-3a (the WA4-reachability
  arithmetic: a 5-unit fuel package at spread 10× base covers transport cost of
  a typical ring leg)
- All other goods base_micros at `200_000` (the PER_UNIT_BASE_MICROS wage scale)
- **Clumped per-good topology**: partitioned `mix(seed, k)` k-ranges assign each
  good to 2-3 sources and 2-3 sinks, disjoint across goods (FRONTIER_TIER_WIRING
  shape). Sources have producers (output-only recipe), sinks have consumers
  (input-only recipe). Food consumers use interval 80 (same as the Fuel sink shape
  in frontier). Haven (station row 6) excluded from all source/sink assignments.
- Differentiated initial stocks: sources near cap (80% of price cap), sinks at 0
- **Exchange corp** added as `corporations[0]` (the Yard/Port merge, OD-2):
  `treasury_micros` sized to the measured drain budget (see below), `home_station_index: 2`
- `ExchangeCfg { corp_index: 0, active: true }` — the Exchange is live
- `ArbitrageCfg` with `scan_interval: 16`, `wage_share_milli: 500`,
  `wage_flat_micros: 200_000`, `qty_ladder: [5, 10, 15]`,
  `max_posts_per_scan: 9`, `transport_micros` table from ring geometry
- Pirate capacity stays 5 (unchanged)
- `hideout_body_index: 7` (the seam, matching frontier — NOT 6 as in trophic)
- Per-role-block craft replication pattern so `fleet_scale` knob stays headcount-
  safe (panel L5-C2): haulers block first, then pirates block, each block
  replicated as a unit
- Trader holds sized: `base_cargo_capacity: 15` (3× the pirate capacity of 5,
  so the toll grid is live in rung B; smallest toll × 15 units ≥ 1 micro)

**Exchange treasury sizing (OD-2 solvency battery):**
Worst drain ≈ 5.4e9 micros / 100k ticks (from synthesis §1.2 M1 measurement).
Seed with `5_400_000_000` micros. Print drain as anchored BAZAAR read at the
runner. This is a lossy battery with Yard fee revenue merged in — when the
Exchange runs dry, `resolve_trade_sells` saturates at zero (the spec's named
behavior: goods still unload, contract still completes).

**Transport table construction (ring geometry, factory-time integers):**
The tier-reward precedent at scenario.rs:466-467 uses per-tier integer rewards
derived from ring geometry. The transport table is analogous: a symmetric
`n × n` matrix where `transport_micros[i][j]` is proportional to the arc
distance between station body indices, scaled so a typical same-tier leg costs
approximately `100_000` micros (half the base wage `200_000`). Implementation:
```
arc_ticks(i, j) = |body_index[i] - body_index[j]|  (ring distance, simplified)
transport_micros[i][j] = 100_000 * arc_ticks / max_arc_ticks
```
All values are non-negative integers. The table is folded in `ArbitrageCfg`'s
config hash (count words first).

**Files:**

- Modify: `crates/jumpgate-core/src/scenario.rs` — add `pub fn scenario_bazaar`
  and `pub const BAZAAR_*` constants; add `"bazaar"` arm to
  `trophic_run.rs:148-156`
- Modify: `python/jumpgate/trophic_run.py` (or equivalent runner) — add
  `"bazaar"` to `--scenario` enum
- Modify: `crates/jumpgate-core/src/config.rs` — add `ArbitrageCfg`,
  `ExchangeCfg`, `GoodsCfg`, `GoodSpec` structs; add fields to `RunConfig`;
  extend `config_hash` fold (count-first per GoodsCfg discipline)

**Step 1: failing test — scenario_bazaar shape invariants**

```rust
// scenario.rs tests
#[test]
fn scenario_bazaar_shape() {
    let cfg = scenario_bazaar(7);

    // Frontier band geometry: 1 star + 10 station bodies.
    assert_eq!(cfg.bodies.len(), 11, "star + 10 station bodies");
    let axes: Vec<f64> = cfg.bodies[1..].iter().map(|b| b.elements.a).collect();
    assert_eq!(axes, FRONTIER_ORBIT_AU.to_vec(), "bodies ride FRONTIER_ORBIT_AU");

    // 10 stations.
    assert_eq!(cfg.stations.len(), 10);

    // REPOST disabled.
    assert_eq!(cfg.dispatch_cfg.demand_low, 0);
    assert_eq!(cfg.dispatch_cfg.demand_high, 0);

    // Zero ContractInit rows (arbitrage replaces restock, D4).
    assert_eq!(cfg.contracts.len(), 0, "no pre-seeded contracts: arbitrage replaces repost");

    // 240k ephemeris window.
    assert_eq!(cfg.ephemeris_window, 240_000);

    // hideout at the seam (body 7 == FRONTIER_HAVEN_STATION), NOT outermost.
    assert_eq!(cfg.trophic.hideout_body_index, 7);

    // Pirate capacity = 5 (unchanged).
    for c in cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate) {
        assert_eq!(c.spec.base_cargo_capacity, 5, "pirate cargo capacity unchanged");
    }

    // Hauler holds sized for rung-B toll grid (capacity > pirate capacity).
    for c in cfg.craft.iter().filter(|c| c.role == CraftRole::Idle) {
        assert!(
            c.spec.base_cargo_capacity > 5,
            "hauler capacity > 5 so toll grid is live in rung B"
        );
    }

    // Exchange corp is registered and active.
    assert!(cfg.exchange.active, "Exchange active in bazaar");
    let ecorp = cfg.exchange.corp_index as usize;
    assert!(ecorp < cfg.corporations.len(), "exchange corp_index in range");
    assert!(
        cfg.corporations[ecorp].treasury_micros >= 5_400_000_000,
        "Exchange battery sized >= worst-drain budget"
    );

    // Fuel base re-baked to ~50_000 (OD-3a WA4 reachability).
    let fuel_idx = crate::economy::Good::FUEL.0 as usize;
    assert_eq!(
        cfg.price_cfg.base_micros[fuel_idx], 50_000,
        "fuel base_micros re-baked for WA4 reachability"
    );

    // All trade goods at 200_000 (wage scale).
    for (i, base) in cfg.price_cfg.base_micros.iter().enumerate() {
        if i == fuel_idx { continue; } // Fuel is the re-baked exception
        assert_eq!(*base, 200_000, "good {i} base_micros at wage scale");
    }

    // World resolves.
    let (w, _h) = World::reset(cfg).expect("scenario_bazaar must resolve");
    let pirates = w.ships.pirate.iter().filter(|p| p.is_some()).count();
    assert_eq!(pirates, FRONTIER_NUM_PIRATES, "pirate pool minted correctly");
}
```

Run: `cargo test -p jumpgate-core scenario_bazaar_shape`

Expected failure: `error[E0425]: cannot find function 'scenario_bazaar'`

**Step 2: add config structs (`ArbitrageCfg`, `ExchangeCfg`, `GoodsCfg`, `GoodSpec`)**

In `crates/jumpgate-core/src/config.rs`, append after `RefuelCfg`:

```rust
/// Per-good property table (OD-7 minimal-live). `name` is NEVER folded (it is
/// cosmetic only — folding it would couple the hash to string choices). The
/// `unit_mass_milli` drives the capacity gate: `used_milli + q*mass_milli <=
/// capacity*1000`. Uniform `1000` at v1 reduces exactly to today's unit compare.
#[derive(Clone, Debug)]
pub struct GoodSpec {
    /// Display name. NOT folded in config_hash.
    pub name: &'static str,
    /// Mass per unit in milli-units (1000 = one unit). Uniform at v1.
    pub unit_mass_milli: u32,
}

/// Goods configuration (rung A). The count word is written FIRST in the config
/// fold (anti-aliasing delimiter — prevents a 5-good config from colliding with
/// a 10-good config whose first 5 entries are identical, L5-F7 discipline).
#[derive(Clone, Debug)]
pub struct GoodsCfg {
    pub goods: Vec<GoodSpec>,
}

impl Default for GoodsCfg {
    fn default() -> Self {
        GoodsCfg { goods: vec![] }
    }
}

/// Exchange corp settings (OD-2): the single seller/buyer-of-record for all
/// goods at every station including the haven. `active: false` = Exchange off
/// (structural gate for trophic/frontier bit-identity). `corp_index` is the
/// dense row index of the Exchange corporation (Yard/Port/Exchange merged in
/// scenario_bazaar).
#[derive(Clone, Copy, Debug)]
pub struct ExchangeCfg {
    pub corp_index: u32,
    /// Exchange is active and participates in trade settles. Default `false`.
    pub active: bool,
}

impl Default for ExchangeCfg {
    fn default() -> Self {
        ExchangeCfg { corp_index: 0, active: false }
    }
}

/// Arbitrage config: the Exchange poster's scan parameters plus the factory-time
/// transport-cost table and the two-mode policy's wage-derivation knobs.
/// `scan_interval == 0` is the structural off for the poster (trophic/frontier
/// inert). All monetary values in microcredits.
#[derive(Clone, Debug)]
pub struct ArbitrageCfg {
    /// Ticks between poster scans. 0 = off (structural inert gate).
    pub scan_interval: u32,
    /// Fixed wage floor per route (microcredits). Added to surplus-share wage.
    pub wage_flat_micros: i64,
    /// Surplus-share parameter: posted wage gets this fraction (milli) of
    /// (spread - transport) above the floor. Per OD-4: dangerous lanes earn
    /// more through pure price mechanics.
    pub wage_share_milli: u32,
    /// Factory-time transport cost table (microcredits): `transport_micros[from][to]`
    /// is a non-negative integer; symmetric. Folded count-first in config_hash.
    /// NOT a runtime ephemeris read — L1-M3/L2 tie resolved in favour of the
    /// static table (kills the WB1 distance-oscillation confound).
    pub transport_micros: Vec<Vec<i64>>,
    /// Lot-size ladder (units). Smallest-first; the poster tries the smallest
    /// lot that clears `spread*qty > transport + premium`.
    pub qty_ladder: Vec<u32>,
    /// Maximum packages posted per scan across all routes.
    pub max_posts_per_scan: usize,
    /// Minimum surplus above transport before posting (per-corp config; the
    /// arbitrage premium in §1.2). Premium ∝ 1/cap per good (L1-m4).
    pub arb_premium_micros: Vec<i64>,
    /// Stock capacity hint per good for the own-trade "demand exists" filter in
    /// `run_trade_policies`. Indexed by Good.0. Not folded (cosmetic hint only).
    pub stock_cap_hint: Vec<i64>,
}

impl Default for ArbitrageCfg {
    fn default() -> Self {
        ArbitrageCfg {
            scan_interval: 0,
            wage_flat_micros: 0,
            wage_share_milli: 0,
            transport_micros: vec![],
            qty_ladder: vec![],
            max_posts_per_scan: 0,
            arb_premium_micros: vec![],
            stock_cap_hint: vec![],
        }
    }
}
```

Add the new fields to `RunConfig` (config.rs:433-465), after `refuel`:
```rust
    // Goods-as-goods rung (folded AFTER refuel, append-only). Defaults leave the
    // goods machinery inert (empty goods list, Exchange inactive, scanner off).
    pub goods_cfg: GoodsCfg,
    pub exchange: ExchangeCfg,
    pub arb: ArbitrageCfg,
```

Add to the `CONFIG_FIELD_ORDER` comment:
```
///  27. goods_cfg: count, then per-good (unit_mass_milli only; name NOT folded)
///  28. exchange: corp_index, active
///  29. arb: scan_interval, wage_flat_micros, wage_share_milli, transport table
///           (count rows, per row: count cols, per col: value), qty_ladder
///           (count, values), max_posts_per_scan, arb_premium_micros (count, values)
```

Add `config_hash` fold at the tail of `RunConfig::config_hash` (after the
`RefuelCfg` destructure at config.rs:749-751):

```rust
        // 27. GoodsCfg — count FIRST (anti-aliasing delimiter, L5-F7), then
        //     per-good unit_mass_milli. `name` is NEVER folded (cosmetic).
        let GoodsCfg { goods } = &self.goods_cfg;
        h.write_u64(goods.len() as u64); // count word first
        for spec in goods {
            let GoodSpec { name: _, unit_mass_milli } = spec;
            h.write_u64(*unit_mass_milli as u64);
        }

        // 28. ExchangeCfg.
        let ExchangeCfg { corp_index, active } = self.exchange;
        h.write_u64(corp_index as u64);
        h.write_u64(active as u64);

        // 29. ArbitrageCfg — scan_interval, wage knobs, transport table
        //     (count-rows, per-row count-cols + values), qty_ladder, max_posts,
        //     arb_premium_micros (count + values). stock_cap_hint NOT folded
        //     (cosmetic hint used only in run_trade_policies; not identity-
        //     bearing for cross-branch determinism).
        let ArbitrageCfg {
            scan_interval,
            wage_flat_micros,
            wage_share_milli,
            transport_micros,
            qty_ladder,
            max_posts_per_scan,
            arb_premium_micros,
            stock_cap_hint: _, // NOT folded: cosmetic hint
        } = &self.arb;
        h.write_u64(*scan_interval as u64);
        h.write_u64(*wage_flat_micros as u64);
        h.write_u64(*wage_share_milli as u64);
        h.write_u64(transport_micros.len() as u64);
        for row in transport_micros {
            h.write_u64(row.len() as u64);
            for &v in row {
                h.write_u64(v as u64);
            }
        }
        h.write_u64(qty_ladder.len() as u64);
        for &q in qty_ladder { h.write_u64(q as u64); }
        h.write_u64(*max_posts_per_scan as u64);
        h.write_u64(arb_premium_micros.len() as u64);
        for &p in arb_premium_micros { h.write_u64(p as u64); }
```

Add defaults to `sample()` (config.rs:785-831):
```rust
            goods_cfg: GoodsCfg::default(),
            exchange: ExchangeCfg::default(),
            arb: ArbitrageCfg::default(),
```

**Step 3: re-pin `GOLDEN_CONFIG_HASH` (single-cause commit)**

Run:
```
cargo test -p jumpgate-core -- config_hash_golden_anchor_is_stable --nocapture 2>&1 | grep -E "GOLDEN|drifted|got="
```

If the test fails with the drift message, run the print fixture:
```
cargo test -p jumpgate-core -- print_golden --ignored --nocapture 2>&1
```

This is the `config_hash_golden_anchor_is_stable` test — copy the printed
`ConfigHash(0x...)` value and update `GOLDEN_CONFIG_HASH` in config.rs:783.
Add a comment citing the cause:
```rust
const GOLDEN_CONFIG_HASH: u64 = 0x<NEW_VALUE>; // RE-PINNED: +GoodsCfg/ExchangeCfg/ArbitrageCfg folded at config tail (goods-as-goods rung A). Was 0x128c_1299_5c48_4fdc.
```

Run: `cargo test -p jumpgate-core config_hash_golden_anchor_is_stable`
Expected: passes.

**Commit the GOLDEN_CONFIG_HASH re-pin as a SEPARATE single-cause commit:**

```
git add crates/jumpgate-core/src/config.rs
git commit -F - <<'EOF'
fix(config-hash): re-pin GOLDEN_CONFIG_HASH after goods-as-goods config tail

GoodsCfg / ExchangeCfg / ArbitrageCfg folds added at tail of RunConfig::config_hash
(append-only discipline). Was 0x128c_1299_5c48_4fdc.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

**Step 4: implement `scenario_bazaar` factory**

In `crates/jumpgate-core/src/scenario.rs`, add constants then the factory:

```rust
/// Bazaar goods set (10 goods: Ore at 0, Fuel at 1, then 8 trade goods).
/// Indices are STABLE (append-only) — the first two must match Good::ORE/FUEL.
pub const BAZAAR_GOODS: &[(&str, u32)] = &[
    ("Ore",         1000),
    ("Fuel",        1000),
    ("Food",        1000),
    ("Alloys",      1000),
    ("Medicine",    1000),
    ("Machinery",   1000),
    ("Luxuries",    1000),
    ("Electronics", 1000),
    ("Textiles",    1000),
    ("Chemicals",   1000),
];

/// Number of haulers in the bazaar scenario (2 per station, matching frontier).
pub const BAZAAR_NUM_HAULERS: usize = 20;
/// Number of pirates in the bazaar scenario (2:1 predator:prey design, as frontier).
pub const BAZAAR_NUM_PIRATES: usize = 10;

/// Haven station row in the bazaar (same seam law as frontier: body 7, NOT outermost).
pub const BAZAAR_HAVEN_STATION: usize = FRONTIER_HAVEN_STATION; // = 6

/// Number of source stations and sink stations per good (clumped topology).
/// 2 sources and 2 sinks per good — enough for independent Schmitt triggers,
/// few enough to prevent self-averaging (panel L1-C2/L4-C3/L5-C3).
pub const BAZAAR_SOURCES_PER_GOOD: usize = 2;
pub const BAZAAR_SINKS_PER_GOOD: usize = 2;

/// Fuel base_micros re-baked for WA4 reachability (OD-3a). A 5-unit package at
/// spread 10× base = 500_000 > transport ~100_000 — the arbitrage trigger clears.
pub const BAZAAR_FUEL_BASE_MICROS: i64 = 50_000;

/// Trade good base_micros (wage scale, consensus #12).
pub const BAZAAR_TRADE_BASE_MICROS: i64 = 200_000;

/// Exchange treasury battery budget (OD-2 solvency battery — sized to worst-drain).
pub const BAZAAR_EXCHANGE_TREASURY_MICROS: i64 = 5_400_000_000;

/// Price cap per good (controls update_prices curve slope + own-trade demand
/// filter in run_trade_policies). Fuel cap matches frontier (40). Trade goods
/// cap = 40 (uniform; the structural-off cap==0 rule means all must be > 0).
pub const BAZAAR_GOOD_CAP: i64 = 40;

/// Build the 10-station bazaar scenario on the frontier band geometry.
/// The 10-station ring, pirate pool, and physics constants are identical to
/// `scenario_frontier`; what changes is the goods set, producers, pricing,
/// corp structure, and the absence of pre-seeded contracts.
pub fn scenario_bazaar(seed: u64) -> RunConfig {
    let n_goods = BAZAAR_GOODS.len(); // 10
    let n_stations = FRONTIER_ORBIT_AU.len(); // 10

    // --- bodies: identical to scenario_frontier ---
    let mut bodies = Vec::with_capacity(1 + n_stations);
    bodies.push(BodyInit { mass: STAR_MASS, elements: OrbitalElements::zero() });
    for (k, &a) in FRONTIER_ORBIT_AU.iter().enumerate() {
        let m0_raw = mix(seed, (k + 1) as u64);
        let m0 = std::f64::consts::TAU * u64_to_unit_f64(m0_raw);
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    // --- craft: per-role blocks (panel L5-C2 headcount-safe replication) ---
    // Haulers first (rows 0..BAZAAR_NUM_HAULERS), then pirates.
    // Hauler capacity = 15 (3× pirate capacity of 5; toll grid live in rung B).
    let hauler_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: FRONTIER_HAULER_EXHAUST_VELOCITY, // 42.5 calibrated
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 15,
    };
    let pirate_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 20.0,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    let co_orbit = |body_index: usize| -> (Vec3, Vec3) {
        let el = &bodies[body_index].elements;
        let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
        let v_circ = (mu / el.a).sqrt();
        let pos = Vec3::new(el.a * el.m0.cos(), el.a * el.m0.sin(), 0.0);
        let vel = Vec3::new(-v_circ * el.m0.sin(), v_circ * el.m0.cos(), 0.0);
        (pos, vel)
    };
    let mut craft = Vec::with_capacity(BAZAAR_NUM_HAULERS + BAZAAR_NUM_PIRATES);
    // Hauler block (rows 0..BAZAAR_NUM_HAULERS):
    for k in 0..BAZAAR_NUM_HAULERS {
        let (pos, vel) = co_orbit(1 + (k % n_stations));
        craft.push(CraftInit {
            spec: hauler_spec.clone(),
            pos, vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Idle,
            scripted: true,
            trade_reserve_micros: 0,
        });
    }
    // Pirate block (rows BAZAAR_NUM_HAULERS..):
    for _ in 0..BAZAAR_NUM_PIRATES {
        let (pos, vel) = co_orbit(1 + BAZAAR_HAVEN_STATION);
        craft.push(CraftInit {
            spec: pirate_spec.clone(),
            pos, vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Pirate,
            scripted: true,
            trade_reserve_micros: 0,
        });
    }

    // --- clumped per-good topology (panel L1-C2/L4-C3/L5-C3) ---
    // Assign each good a disjoint set of source and sink station rows using
    // partitioned mix(seed, k) k-ranges. The haven (BAZAAR_HAVEN_STATION) is
    // excluded from all source/sink assignments.
    //
    // K-range partition: good g uses source k-range [g*100..g*100+50) and sink
    // k-range [g*100+50..g*100+100). For each role, we draw BAZAAR_SOURCES_PER_GOOD
    // distinct station indices from [0..n_stations] \ {BAZAAR_HAVEN_STATION}.
    let eligible_stations: Vec<usize> = (0..n_stations)
        .filter(|&s| s != BAZAAR_HAVEN_STATION)
        .collect();
    let n_elig = eligible_stations.len(); // 9

    let pick_stations = |k_base: u64, count: usize| -> Vec<usize> {
        let mut result = Vec::with_capacity(count);
        let mut k = k_base;
        while result.len() < count {
            let idx = (mix(seed, k) % n_elig as u64) as usize;
            let station = eligible_stations[idx];
            if !result.contains(&station) {
                result.push(station);
            }
            k += 1;
        }
        result
    };

    // Per-good source/sink assignment.
    let mut good_sources: Vec<Vec<usize>> = Vec::with_capacity(n_goods);
    let mut good_sinks:   Vec<Vec<usize>> = Vec::with_capacity(n_goods);
    for g in 0..n_goods {
        let src = pick_stations(g as u64 * 100, BAZAAR_SOURCES_PER_GOOD);
        // Sinks use a different k-range base and exclude the sources.
        let mut sinks = pick_stations(g as u64 * 100 + 50, BAZAAR_SINKS_PER_GOOD + 2);
        sinks.retain(|s| !src.contains(s));
        sinks.truncate(BAZAAR_SINKS_PER_GOOD);
        if sinks.len() < BAZAAR_SINKS_PER_GOOD {
            // Fallback: any eligible non-source station.
            for &s in &eligible_stations {
                if !src.contains(&s) && !sinks.contains(&s) {
                    sinks.push(s);
                }
                if sinks.len() >= BAZAAR_SINKS_PER_GOOD { break; }
            }
        }
        good_sources.push(src);
        good_sinks.push(sinks);
    }

    // --- stations: initial stock differentiated (sources near cap, sinks at 0) ---
    // price_micros seeded from the demand-deflation curve at stock = initial_stock.
    let good_price = |good_idx: usize, stock: i64| -> i64 {
        let base = if good_idx == 1 { BAZAAR_FUEL_BASE_MICROS } else { BAZAAR_TRADE_BASE_MICROS };
        let s = stock.clamp(0, BAZAAR_GOOD_CAP);
        (base * (2000 - s * 1800 / BAZAAR_GOOD_CAP) / 1000).max(0)
    };

    let mut station_inits: Vec<StationInit> = Vec::with_capacity(n_stations);
    for srow in 0..n_stations {
        let body_index = srow + 1; // body k+1 hosts station row k
        let mut initial_stock = vec![0i64; n_goods];
        let mut initial_price = vec![0i64; n_goods];
        for g in 0..n_goods {
            let stock = if good_sources[g].contains(&srow) {
                // Source: 80% of cap as initial stock (seeded differentiation).
                (BAZAAR_GOOD_CAP * 4 / 5).max(1)
            } else {
                0
            };
            initial_stock[g] = stock;
            initial_price[g] = good_price(g, stock);
        }
        // Vendors: tier dests (2, 5, 9) and haven (6), matching frontier.
        let sells_upgrades = matches!(srow, 2 | 5 | 9) || srow == BAZAAR_HAVEN_STATION;
        station_inits.push(StationInit {
            body_index,
            initial_stock,
            initial_price_micros: initial_price,
            sells_upgrades,
        });
    }

    // --- producers: sources produce (output-only), sinks consume (input-only) ---
    // Food consumers use interval 80 (the Fuel sink shape from frontier).
    // Other goods use interval 40 for sources, 80 for sinks.
    let mut producers = Vec::new();
    for g in 0..n_goods {
        let good = Good(g as u16);
        let src_interval = if g == 1 { 60u32 } else { 40 }; // Fuel = refiner cadence
        let snk_interval = 80u32;
        for &srow in &good_sources[g] {
            producers.push(ProducerInit {
                station_index: srow,
                recipe: Recipe {
                    input: None,
                    output: Some((good, 5)),
                    interval: src_interval,
                },
            });
        }
        for &srow in &good_sinks[g] {
            producers.push(ProducerInit {
                station_index: srow,
                recipe: Recipe {
                    input: Some((good, 5)),
                    output: None,
                    interval: snk_interval,
                },
            });
        }
    }

    // --- corporations: Exchange (merged Yard+Port+Exchange, OD-2) is corp 0 ---
    // In scenario_bazaar the Exchange handles all goods trade; there is no
    // separate Yard or Port (the REPOST mechanics are retired; upgrade revenue
    // and refuel revenue fold into the Exchange battery).
    let corporations = vec![
        CorporationInit {
            treasury_micros: BAZAAR_EXCHANGE_TREASURY_MICROS,
            home_station_index: 2,
        },
    ];

    // --- transport cost table (ring geometry, factory-time integers) ---
    // Arc distance between station body indices gives a symmetric integer table.
    // A same-tier leg (adjacent bodies) costs ~100_000; the max arc (0→9) costs
    // up to ~450_000. All values are non-negative.
    let mut transport_micros = vec![vec![0i64; n_stations]; n_stations];
    for i in 0..n_stations {
        for j in 0..n_stations {
            let arc = (i as i64 - j as i64).unsigned_abs() as i64;
            transport_micros[i][j] = arc * 50_000; // 50_000 per hop
        }
    }

    // ArbitrageCfg: scan every 16 ticks (same as stagger_period), wage_share
    // 500‰, qty ladder [5, 10, 15].
    let arb_premium_micros = vec![50_000i64; n_goods]; // flat premium floor per good
    let stock_cap_hint = vec![BAZAAR_GOOD_CAP; n_goods];
    let arb = ArbitrageCfg {
        scan_interval: 16,
        wage_flat_micros: 200_000,
        wage_share_milli: 500,
        transport_micros,
        qty_ladder: vec![5, 10, 15],
        max_posts_per_scan: 9,
        arb_premium_micros,
        stock_cap_hint,
    };

    // --- GoodsCfg ---
    let goods_cfg = GoodsCfg {
        goods: BAZAAR_GOODS
            .iter()
            .map(|&(name, mass_milli)| GoodSpec { name, unit_mass_milli: mass_milli })
            .collect(),
    };

    // --- PriceCfg: all goods live (cap > 0 for every good) ---
    let base_micros: Vec<i64> = (0..n_goods)
        .map(|g| if g == 1 { BAZAAR_FUEL_BASE_MICROS } else { BAZAAR_TRADE_BASE_MICROS })
        .collect();
    let caps: Vec<i64> = vec![BAZAAR_GOOD_CAP; n_goods];

    // --- trophic: identical to frontier constants (the same band) ---
    let trophic = TrophicCfg {
        engage_radius_au: 5.0e-4,
        upkeep_per_tick: 12,
        food_per_unit_micros: 15_000,
        grubstake_micros: 100_000,
        ransom_cap_micros: 6_000_000,
        starve_lie_low_ticks: 4_000,
        hideout_body_index: 7, // the seam (same as frontier)
        pirate_max_reach_au: 0.6,
        hauler_belief_scoring: true,
        hauler_buy_policy: BuyPolicy::EscortFirst,
        ..TrophicCfg::default()
    };

    RunConfig {
        master_seed: seed,
        dt: Dt::new(0.25),
        softening: 1.0e-4,
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        ephemeris_window: 240_000,
        bodies,
        craft,
        guidance: GuidanceParams::default(),
        stations: station_inits,
        producers,
        corporations,
        contracts: vec![], // D4: arbitrage replaces repost
        price_cfg: PriceCfg {
            base_micros,
            cap: caps,
            slope_milli: 1800,
            reprice_interval: 1,
        },
        dispatch_cfg: DispatchCfg {
            demand_low: 0,   // REPOST structural off (economy.rs:491-516)
            demand_high: 0,  // REPOST structural off
            stagger_period: 16,
            contract_reward_micros: 0,
            contract_qty: 0,
        },
        trophic,
        shipyard: ShipyardCfg { corp_index: 0, ..ShipyardCfg::default() },
        media: MediaCfg::default(),
        refuel: RefuelCfg { lot_mass: 5.0e-11, corp_index: 0 }, // Exchange = Yard+Port
        goods_cfg,
        exchange: ExchangeCfg { corp_index: 0, active: true },
        arb,
    }
}
```

**Step 5: add `"bazaar"` arm to `trophic_run.rs`**

In `crates/jumpgate-sim/src/trophic_run.rs` (or wherever the scenario match
lives at trophic_run.rs:148-156):

```rust
    "bazaar" => ("bazaar", scenario_bazaar(args.seed)),
```

Add `scenario_bazaar` to the `use crate::scenario::` import line.

**Step 6: run the shape test**

```
cargo test -p jumpgate-core scenario_bazaar_shape
```

Expected: passes. Fix any compilation errors from `PriceCfg`/`StationInit` field
shape changes (after A1's Vec migration these become `Vec<i64>` instead of fixed
arrays — adapt accordingly; the test spec remains the same).

**Step 7: commit**

```
git add crates/jumpgate-core/src/scenario.rs \
        crates/jumpgate-core/src/config.rs \
        crates/jumpgate-sim/src/trophic_run.rs
git commit -F - <<'EOF'
feat(rung-a): scenario_bazaar factory — 10-good clumped topology on frontier band

Frontier 10-station band geometry, ephemeris_window=240k, zero ContractInit
rows (arbitrage replaces repost, D4), REPOST structural off (demand=0/0). Ten
goods: Ore/Fuel unchanged + Food/Alloys/Medicine/Machinery/Luxuries/Electronics/
Textiles/Chemicals. Fuel re-baked to 50_000 (OD-3a WA4 reachability). All trade
goods at 200_000 base_micros (wage scale). Clumped per-good topology via
partitioned mix(seed,k) k-ranges, 2 sources + 2 sinks per good (panel L1-C2),
haven excluded. Exchange corp (merged Yard+Port, OD-2) as corp 0 with 5.4B
treasury battery. Per-role-block craft layout (panel L5-C2): hauler block first,
pirate block second. Hauler capacity=15 (toll grid live in rung B, L3-C3).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A5.3: factory invariant tests

**What they test:**
1. Every good has ≥1 source station and ≥1 sink station (WA1 cannot stall if
   goods can never be produced or consumed)
2. Source and sink k-ranges are disjoint for the same good (no station is both
   source and sink for the same good — the clumped topology law)
3. Haven is excluded from all contract endpoints AND from source/sink assignments
4. REPOST is structurally off (`demand_low == demand_high == 0`)
5. No rung-B config knobs are present (`ExchangeCfg` has no `fence_discount_milli`
   etc.; posture/greed absent from any config struct — L5-C4)

**Files:**

- Modify: `crates/jumpgate-core/src/scenario.rs` — add tests to the `mod tests` block

**Step 1: write tests**

```rust
#[test]
fn scenario_bazaar_every_good_has_source_and_sink() {
    use crate::economy::{Good, N_RESOURCES};
    let cfg = scenario_bazaar(7);
    let n_goods = cfg.goods_cfg.goods.len();
    assert!(n_goods >= 2, "at least Ore + Fuel");

    for g in 0..n_goods {
        let good = Good(g as u16);
        let sources: Vec<usize> = cfg.producers.iter()
            .filter(|p| p.recipe.output.map_or(false, |(res, _)| res == good)
                     && p.recipe.input.is_none())
            .map(|p| p.station_index)
            .collect();
        let sinks: Vec<usize> = cfg.producers.iter()
            .filter(|p| p.recipe.input.map_or(false, |(res, _)| res == good)
                     && p.recipe.output.is_none())
            .map(|p| p.station_index)
            .collect();
        assert!(
            !sources.is_empty(),
            "good {g} ({}) must have >= 1 source producer",
            cfg.goods_cfg.goods[g].name
        );
        assert!(
            !sinks.is_empty(),
            "good {g} ({}) must have >= 1 sink producer",
            cfg.goods_cfg.goods[g].name
        );
    }
}

#[test]
fn scenario_bazaar_source_sink_disjoint_per_good() {
    use crate::economy::Good;
    let cfg = scenario_bazaar(7);
    let n_goods = cfg.goods_cfg.goods.len();

    for g in 0..n_goods {
        let good = Good(g as u16);
        let sources: std::collections::HashSet<usize> = cfg.producers.iter()
            .filter(|p| p.recipe.output.map_or(false, |(res, _)| res == good)
                     && p.recipe.input.is_none())
            .map(|p| p.station_index)
            .collect();
        let sinks: std::collections::HashSet<usize> = cfg.producers.iter()
            .filter(|p| p.recipe.input.map_or(false, |(res, _)| res == good)
                     && p.recipe.output.is_none())
            .map(|p| p.station_index)
            .collect();
        let overlap: std::collections::HashSet<_> = sources.intersection(&sinks).collect();
        assert!(
            overlap.is_empty(),
            "good {g} ({}): source-sink overlap at stations {:?} (clumped topology requires disjoint)",
            cfg.goods_cfg.goods[g].name,
            overlap
        );
    }
}

#[test]
fn scenario_bazaar_haven_excluded_from_sources_sinks_and_contracts() {
    use crate::economy::Good;
    let cfg = scenario_bazaar(7);
    let haven = BAZAAR_HAVEN_STATION;

    // No producer (source or sink) at the haven.
    for p in &cfg.producers {
        assert_ne!(
            p.station_index, haven,
            "haven must not host any producer (source or sink)"
        );
    }

    // No contract endpoints at the haven (contracts is empty in bazaar;
    // assert as a guard against accidental pre-seeded rows).
    for k in &cfg.contracts {
        assert_ne!(k.from_station_index, haven, "haven is not a contract from-endpoint");
        assert_ne!(k.to_station_index, haven, "haven is not a contract to-endpoint");
    }
}

#[test]
fn scenario_bazaar_repost_structurally_off() {
    let cfg = scenario_bazaar(7);
    assert_eq!(
        cfg.dispatch_cfg.demand_low, 0,
        "demand_low must be 0 (REPOST structural off)"
    );
    assert_eq!(
        cfg.dispatch_cfg.demand_high, 0,
        "demand_high must be 0 (REPOST structural off)"
    );
    // Zero pre-seeded contracts.
    assert_eq!(
        cfg.contracts.len(), 0,
        "no pre-seeded contracts in bazaar (arbitrage replaces repost)"
    );
}

#[test]
fn scenario_bazaar_no_rung_b_knobs() {
    // L5-C4: rung-B config (posture, greed, fence discount, toll) must NOT
    // be present in any rung-A config struct. We verify at the type level by
    // checking that ExchangeCfg and ArbitrageCfg have no fence/greed/toll
    // fields (this test compiles only if those fields are absent — it is a
    // static check via exhaustive destructure).
    let cfg = scenario_bazaar(7);
    // Exhaustive destructure of ExchangeCfg: if a rung-B field is added to
    // this struct, this will be a compile error (the intent of L5-C4).
    let ExchangeCfg { corp_index: _, active: _ } = cfg.exchange;
    // Exhaustive destructure of ArbitrageCfg: no fence/toll/greed/posture.
    let ArbitrageCfg {
        scan_interval: _,
        wage_flat_micros: _,
        wage_share_milli: _,
        transport_micros: _,
        qty_ladder: _,
        max_posts_per_scan: _,
        arb_premium_micros: _,
        stock_cap_hint: _,
    } = cfg.arb;
    // If this compiles and runs, no rung-B fields have been added.
}
```

**Step 2: run tests**

```
cargo test -p jumpgate-core -- scenario_bazaar_every_good_has_source_and_sink scenario_bazaar_source_sink_disjoint_per_good scenario_bazaar_haven_excluded_from_sources_sinks_and_contracts scenario_bazaar_repost_structurally_off scenario_bazaar_no_rung_b_knobs
```

Expected: all pass. If disjoint test fails, adjust the `pick_stations` k-range
logic in the factory to guarantee disjointness (the fallback path should already
ensure this, but verify).

**Step 3: run the full workspace test suite**

```
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. No warnings.

**Step 4: commit**

```
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
test(rung-a): scenario_bazaar factory invariant tests

Five invariant tests: every good has >= 1 source and >= 1 sink; source/sink
rows disjoint per good (clumped topology law); haven excluded from all source/
sink assignments and contract endpoints; REPOST structurally off (demand=0/0,
contracts empty); no rung-B knobs in rung-A config structs (L5-C4 exhaustive-
destructure static check).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A5.4: `fleet_scale` per-role-block replication

**What it does.** Implements the `fleet_scale` sweep knob for `scenario_bazaar`
so that `Experiment C`'s fleet-scale arms replicate craft **per role block** (all
haulers × N, then all pirates × N), keeping the `per_craft_credits[:haulers]`
prefix slice valid (panel L5-C2). Verified against the grounding: hauler count
from the META line `meta['haulers']`, not a module-level constant
(w4_grid.py:82-86).

**Files:**

- Modify: `crates/jumpgate-core/src/scenario.rs` — add `fleet_scale` arm to
  `apply_knob` so it replicates hauler block then pirate block; add test

**Step 1: failing test**

```rust
#[test]
fn scenario_bazaar_fleet_scale_replicates_per_role_block() {
    let mut cfg = scenario_bazaar(7);
    let n_haulers_base = cfg.craft.iter().filter(|c| c.role == CraftRole::Idle).count();
    let n_pirates_base = cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count();

    // Apply fleet_scale = 2: should double EACH role block independently.
    crate::scenario::apply_knob(&mut cfg, "fleet_scale", "2").expect("fleet_scale knob");

    let n_haulers = cfg.craft.iter().filter(|c| c.role == CraftRole::Idle).count();
    let n_pirates = cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count();

    assert_eq!(n_haulers, n_haulers_base * 2, "haulers scaled 2x");
    assert_eq!(n_pirates, n_pirates_base * 2, "pirates scaled 2x");

    // Role block order: all haulers first, then all pirates.
    let first_pirate_row = cfg.craft.iter().position(|c| c.role == CraftRole::Pirate);
    let last_hauler_row = cfg.craft.iter().rposition(|c| c.role == CraftRole::Idle);
    if let (Some(fp), Some(lh)) = (first_pirate_row, last_hauler_row) {
        assert!(
            lh < fp,
            "all haulers (last at row {lh}) must precede all pirates (first at row {fp})"
        );
    }

    // The per_craft_credits[:haulers] prefix slice invariant: the first n_haulers
    // rows are all Idle (hauler) role.
    for row in 0..n_haulers {
        assert_eq!(
            cfg.craft[row].role,
            CraftRole::Idle,
            "craft row {row} must be Idle (hauler) for prefix slice invariant"
        );
    }
}
```

Run: `cargo test -p jumpgate-core scenario_bazaar_fleet_scale_replicates_per_role_block`

Expected failure: `assert!(false, "fleet_scale knob")` — the knob arm does not
yet handle per-role-block replication for bazaar.

**Step 2: extend `apply_knob` with per-role-block `fleet_scale` for `scenario_bazaar`**

The existing `fleet_scale` arm in `apply_knob` (`scenario.rs:621-630`) scales
EVERY craft's fuel capacity and fuel mass. That is the existing semantics for
frontier. Add a NEW `fleet_scale_count` arm (distinct from the existing `fleet_scale`
f64 knob) that replicates role blocks:

```rust
        "fleet_scale_count" => {
            let scale: u32 = p(name, value)?;
            if scale == 0 {
                return Err(format!("fleet_scale_count must be >= 1, got 0"));
            }
            if scale == 1 {
                return Ok(()); // identity
            }
            // Replicate per-role block (panel L5-C2): haulers first, then pirates.
            // The existing craft vec is already role-blocked (scenario_bazaar
            // construction law: hauler block then pirate block).
            let haulers: Vec<CraftInit> = cfg.craft
                .iter()
                .filter(|c| c.role == CraftRole::Idle)
                .cloned()
                .collect();
            let pirates: Vec<CraftInit> = cfg.craft
                .iter()
                .filter(|c| c.role == CraftRole::Pirate)
                .cloned()
                .collect();
            let mut new_craft = Vec::with_capacity(
                haulers.len() * scale as usize + pirates.len() * scale as usize
            );
            for _ in 0..scale { new_craft.extend(haulers.iter().cloned()); }
            for _ in 0..scale { new_craft.extend(pirates.iter().cloned()); }
            cfg.craft = new_craft;
            Ok(())
        }
```

**Step 3: run test**

```
cargo test -p jumpgate-core scenario_bazaar_fleet_scale_replicates_per_role_block
```

Expected: passes.

**Step 4: commit**

```
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(rung-a): fleet_scale_count knob — per-role-block replication for Experiment C

Adds fleet_scale_count knob to apply_knob: replicates hauler block N times then
pirate block N times (per-role-block discipline, panel L5-C2). Keeps the
per_craft_credits[:haulers] prefix slice invariant valid for w4_grid.py's hauler-
count reads. Distinct from the existing float fleet_scale knob (fuel/tank scaling).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```

---

### Task A5.5: workspace clean pass

**What it does.** Runs the full workspace test suite + clippy to verify the
entire A5 phase compiles cleanly, all pre-existing goldens still pass, and the
trophic/frontier behavior digest is unchanged (Exchange inactive, REPOST
structural-off gates prevent any behavior change on those scenarios).

**Step 1: full workspace build and test**

```
cargo build --workspace 2>&1 | head -40
cargo test --workspace 2>&1 | tail -30
```

Expected: all tests pass including:
- `config_hash_golden_anchor_is_stable` (re-pinned in A5.2)
- `state_hash_golden_zero_world` (unchanged — no v6 bump in A5)
- `golden_zero_state_hash` (unchanged)
- `frontier_trajectory_golden` (unchanged — frontier is bit-identical)
- `scenario_frontier_shape`, `scenario_trophic_shape` (unchanged)
- All new A5 tests

**Step 2: clippy clean**

```
cargo clippy --all-targets -- -D warnings
```

Expected: no warnings.

**Step 3: verify trophic behavior digest unchanged**

Run a within-build digest comparison on trophic scenario across a few seeds to
confirm the new stages (run_trade_policies, resolve_trade_buys/sells) are inert
when `exchange.active == false`:

```bash
for seed in 7 42 100; do
    cargo run -p jumpgate-sim --release -- \
        --scenario trophic --seed $seed --ticks 5000 \
        > /tmp/trophic_${seed}_a5.out 2>&1
done
```

Compare sha256 of stdout with the pre-A5 baseline (pinned from the A0 baseline
commit). If the baseline is unavailable, use a cross-seed comparison: all 3 seeds
should produce the same verdict distribution as before A5.

**Step 4: commit (if any trivial fixes needed from the clean pass)**

Only commit if actual fixes are needed. If all tests pass in step 1 without
changes, no commit is needed for this task — the prior task commits are the
deliverables.

If fixes are needed:
```
git add <fixed files>
git commit -F - <<'EOF'
fix(rung-a): A5 workspace clean-pass fixes

<describe specific fixes>

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
```
