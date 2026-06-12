# Phase A0 — Instruments First + Digest Baseline

**Scope:** scenario-blind instrument additions landing BEFORE any behavior change;
deletion of the dead `EventKind::Trade` corpse; and the behavior-digest baseline
bank that every later phase compares against.

**Ordering law (immutable):** A0.1 → A0.2 → A0.3 → A0.4 → A0.5 → baseline
pinned at A0.5's commit tip. No mechanics may merge until the baseline is banked.

---

### Task A0.1: per-station flat stock/price matrices in TrophicSample + JSONL

**Why:** WA1 survival-by-market reads need per-station stock and price for ALL
goods (today only the Fuel column is sampled). These are additive JSONL keys;
existing `per_station_fuel_stock`/`per_station_fuel_price` stay byte-identical
forever per spec §6 and the synthesis cut §1.1.

**Files:**
- Modify: `crates/jumpgate-core/src/diagnostics.rs` — `TrophicSample` struct
  (after `per_station_fuel_price` ~line 230), `sample_window` function (~line 800)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` — `sample_json` (~line 254)

---

- [ ] **Step 1: failing test — per_station_stock/price fields absent from TrophicSample**

  Confirm the fields do not yet exist so the test catches compilation failure:

  ```rust
  // crates/jumpgate-core/src/diagnostics.rs  — inside the #[cfg(test)] mod at end of file
  #[test]
  fn sample_window_has_per_station_stock_and_price_matrices() {
      use crate::{scenario_trophic, World};
      let (world, _) = World::reset(scenario_trophic(7)).expect("reset");
      let s = sample_window(&world, crate::time::Tick(0));
      // After A0.1 these fields exist and have n_stations entries.
      let n = world.stations.ids.len();
      assert_eq!(s.per_station_stock.len(), n,
          "per_station_stock: one row per station");
      assert_eq!(s.per_station_price.len(), n,
          "per_station_price: one row per station");
      // Each row covers all resources (N_RESOURCES columns today).
      // Fuel column must equal the existing per_station_fuel_stock scalar.
      let fuel_r = crate::economy::Resource::Fuel.index();
      for (row, stock) in s.per_station_stock.iter().enumerate() {
          assert_eq!(stock[fuel_r], s.per_station_fuel_stock[row],
              "per_station_stock fuel column matches existing fuel scalar at row {row}");
      }
  }
  ```

  Run (expected: compile error — fields `per_station_stock`/`per_station_price` do not exist):

  ```
  cargo test -p jumpgate-core sample_window_has_per_station_stock_and_price_matrices
  ```

  Expected failure text:
  ```
  error[E0609]: no field `per_station_stock` on type `TrophicSample`
  ```

- [ ] **Step 2: add fields to `TrophicSample`**

  In `crates/jumpgate-core/src/diagnostics.rs`, locate the block ending with:

  ```rust
      pub per_station_fuel_price: Vec<i64>,
      /// Windowed `Refueled` event reads.
      pub refuels: u32,
  ```

  Add AFTER `per_station_fuel_price` and BEFORE `refuels`:

  ```rust
      // --- goods-as-goods lab fields (rung A, A0; additive — every pre-goods
      // JSONL key above is byte-untouched). per_station_fuel_stock/price remain
      // the scalar fuel columns; these flat matrices carry ALL resources. ---
      /// Per-station stock at the sample point: `[station_row][resource_index]`.
      /// Sized n_stations × N_RESOURCES. Fuel column equals per_station_fuel_stock.
      pub per_station_stock: Vec<Vec<i64>>,
      /// Per-station price_micros at the sample point: `[station_row][resource_index]`.
      /// Sized n_stations × N_RESOURCES. Fuel column equals per_station_fuel_price.
      pub per_station_price: Vec<Vec<i64>>,
  ```

- [ ] **Step 3: populate the fields in `sample_window`**

  In `crates/jumpgate-core/src/diagnostics.rs`, locate the `sample_window` return
  expression. After the existing `per_station_fuel_price` population lines (~line 811)
  and before `refuels`, add:

  ```rust
          per_station_stock: world
              .stations
              .stock
              .iter()
              .map(|st| st.to_vec())
              .collect(),
          per_station_price: world
              .stations
              .price_micros
              .iter()
              .map(|pr| pr.to_vec())
              .collect(),
  ```

  Note: `stations.stock[srow]` is `[i64; N_RESOURCES]` today (pre-A1 the type is a fixed
  array); `.to_vec()` converts it. After A1's Vec migration the expression is
  unchanged because `.to_vec()` works on both `&[i64]` slices and `Vec<i64>`.

- [ ] **Step 4: emit fields in `sample_json` in trophic_run.rs**

  In `crates/jumpgate-core/examples/trophic_run.rs`, locate `sample_json`. After
  the existing `"per_station_fuel_price"` and `"refuels"` entries, insert the new
  keys:

  ```rust
          // goods-as-goods lab keys (rung A, A0) — ADDITIVE: every pre-goods key
          // above is byte-untouched. per_station_fuel_stock/price remain.
          "per_station_stock": s.per_station_stock,
          "per_station_price": s.per_station_price,
  ```

  Place these immediately after `"per_station_fuel_price": s.per_station_fuel_price,`
  and before `"refuels": s.refuels,`.

- [ ] **Step 5: run the test — expect pass**

  ```
  cargo test -p jumpgate-core sample_window_has_per_station_stock_and_price_matrices
  ```

  Expected output:
  ```
  test diagnostics::tests::sample_window_has_per_station_stock_and_price_matrices ... ok
  ```

- [ ] **Step 6: clippy clean**

  ```
  cargo clippy --all-targets -- -D warnings
  ```

  Expected: no warnings or errors.

- [ ] **Step 7: full workspace test**

  ```
  cargo test --workspace
  ```

  Expected: all tests pass.

- [ ] **Step 8: commit**

  ```
  git add crates/jumpgate-core/src/diagnostics.rs \
          crates/jumpgate-core/examples/trophic_run.rs
  git commit -F - <<'EOF'
  feat(a0): per-station stock/price matrices in TrophicSample + JSONL

  Adds per_station_stock and per_station_price flat matrices (n_stations ×
  N_RESOURCES) as additive JSONL keys in TrophicSample and sample_json.
  Existing per_station_fuel_stock/per_station_fuel_price scalar keys are
  untouched byte-for-byte (WA1 survival-by-market requires all-goods
  coverage; the fuel scalars are the legacy column read).

  This is A0 — instruments only, no behavior change.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A0.2: gossip-log enhancements — accept gains resource/reward; new deliver and lie_low rows; rob gains pirate field

**Why:** WA2/WA4 panel joins need the `"deliver"` row (ContractFulfilled
currently falls through to `_ => None` at trophic_run.rs:448). WB2 needs
`"lie_low"`. The accept row needs `resource` + `reward` for per-good route
traffic. The rob row needs `pirate` for engagement attribution panels.

**Files:**
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` — `gossip_log_event_json`
  (~line 384) and its tests (~line 853)

---

- [ ] **Step 1: failing tests — missing gossip-log rows**

  ```rust
  // In trophic_run.rs #[cfg(test)] mod tests — add these tests:

  #[test]
  fn gossip_log_encodes_contract_fulfilled_as_deliver() {
      use jumpgate_core::{scenario_trophic, ContractId, CraftId, World};
      let world = World::reset(scenario_trophic(7))
          .expect("trophic resolves")
          .0;
      let e = Event {
          tick: Tick(100),
          kind: EventKind::ContractFulfilled {
              contract: ContractId { slot: 1, generation: 0 },
              hauler: CraftId { slot: 3, generation: 0 },
          },
      };
      let row = gossip_log_event_json(&world, &e)
          .expect("ContractFulfilled must produce a deliver row");
      assert_eq!(row["e"].as_str(), Some("deliver"));
      assert_eq!(row["tick"].as_u64(), Some(100));
      assert_eq!(row["hauler"].as_u64(), Some(3));
  }

  #[test]
  fn gossip_log_encodes_pirate_lie_low_as_lie_low() {
      use jumpgate_core::{CraftId, World, scenario_trophic};
      let world = World::reset(scenario_trophic(7))
          .expect("trophic resolves")
          .0;
      let e = Event {
          tick: Tick(55),
          kind: EventKind::PirateLieLow {
              pirate: CraftId { slot: 7, generation: 0 },
              until: Tick(155),
          },
      };
      let row = gossip_log_event_json(&world, &e)
          .expect("PirateLieLow must produce a lie_low row");
      assert_eq!(row["e"].as_str(), Some("lie_low"));
      assert_eq!(row["tick"].as_u64(), Some(55));
      assert_eq!(row["pirate"].as_u64(), Some(7));
      assert_eq!(row["until"].as_u64(), Some(155));
  }

  #[test]
  fn gossip_log_accept_row_has_resource_and_reward() {
      use jumpgate_core::{ContractId, CraftId, World, scenario_trophic};
      let world = World::reset(scenario_trophic(7))
          .expect("trophic resolves")
          .0;
      let e = Event {
          tick: Tick(20),
          kind: EventKind::ContractAccepted {
              contract: ContractId { slot: 0, generation: 0 },
              hauler: CraftId { slot: 2, generation: 0 },
          },
      };
      // The world has contracts from scenario_trophic; slot 0 may not be live —
      // so we only assert the keys exist when a route is resolvable.
      // For a non-existent contract the row is still emitted with route=null.
      let row = gossip_log_event_json(&world, &e)
          .expect("ContractAccepted always emits an accept row");
      assert_eq!(row["e"].as_str(), Some("accept"));
      // The new keys must be present (null is ok for a stale contract).
      assert!(row.get("resource").is_some(), "accept row must have 'resource' key");
      assert!(row.get("reward").is_some(), "accept row must have 'reward' key");
  }

  #[test]
  fn gossip_log_rob_row_has_pirate_field() {
      use jumpgate_core::{ContractId, CraftId, World, scenario_trophic};
      let world = World::reset(scenario_trophic(7))
          .expect("trophic resolves")
          .0;
      let e = Event {
          tick: Tick(30),
          kind: EventKind::Robbed {
              pirate: CraftId { slot: 8, generation: 0 },
              hauler: CraftId { slot: 2, generation: 0 },
              contract: ContractId { slot: 0, generation: 0 },
              value_micros: 1_000_000,
          },
      };
      let row = gossip_log_event_json(&world, &e)
          .expect("Robbed always emits a rob row");
      assert_eq!(row["e"].as_str(), Some("rob"));
      assert_eq!(row["pirate"].as_u64(), Some(8),
          "rob row must carry pirate slot");
  }
  ```

  Run (expected: compile OK but tests fail — rows return `None` or miss keys):

  ```
  cargo test -p jumpgate-core gossip_log_encodes_contract_fulfilled_as_deliver
  cargo test -p jumpgate-core gossip_log_encodes_pirate_lie_low_as_lie_low
  cargo test -p jumpgate-core gossip_log_accept_row_has_resource_and_reward
  cargo test -p jumpgate-core gossip_log_rob_row_has_pirate_field
  ```

  Expected failure text for the first:
  ```
  thread '...' panicked at '...: ContractFulfilled must produce a deliver row'
  ```

- [ ] **Step 2: replace `gossip_log_event_json` with exhaustive match**

  In `crates/jumpgate-core/examples/trophic_run.rs`, replace the entire
  `gossip_log_event_json` function body (lines 384-450) with the following.
  The old `_ => None` wildcard is REMOVED; every variant is handled explicitly
  per the synthesis cut's exhaustive-match mandate.

  ```rust
  fn gossip_log_event_json(world: &World, e: &Event) -> Option<serde_json::Value> {
      match e.kind {
          EventKind::AlertBorn {
              alert_seq,
              route,
              pirate,
              hauler,
              truth_value_micros,
              claimed_value_micros,
          } => Some(serde_json::json!({
              "e": "born", "tick": e.tick.0, "alert": alert_seq, "route": route,
              "pirate": pirate.slot, "hauler": hauler.slot,
              "truth": truth_value_micros, "claimed": claimed_value_micros,
          })),
          EventKind::GossipHeard {
              carrier,
              alert_seq,
              route,
              claimed_value_micros,
              hops,
              rob_tick,
              ..
          } => {
              let carrier = match carrier {
                  GossipNode::Station(s) => format!("s{}", s.slot),
                  GossipNode::Craft(c) => format!("c{}", c.slot),
              };
              Some(serde_json::json!({
                  "e": "heard", "tick": e.tick.0, "alert": alert_seq,
                  "carrier": carrier, "route": route, "hops": hops,
                  "claimed": claimed_value_micros, "rob_tick": rob_tick.0,
              }))
          }
          EventKind::Robbed { pirate, contract, .. } => Some(serde_json::json!({
              "e": "rob", "tick": e.tick.0,
              "pirate": pirate.slot,
              "route": diagnostics::route_of(world, contract),
          })),
          EventKind::ContractAccepted { contract, hauler } => {
              // Accept row gains resource + reward keys (A0, WA2/WA4 joins).
              // Reads contracts store directly — offered_contracts() accessor
              // does not expose resource/qty.
              let k = world
                  .contracts
                  .ids
                  .dense_index(contract.slot, contract.generation);
              let (resource, reward) = k
                  .map(|kidx| {
                      let r = format!("{:?}", world.contracts.resource[kidx]);
                      let w = world.contracts.reward_micros[kidx];
                      (serde_json::Value::String(r), serde_json::json!(w))
                  })
                  .unwrap_or((serde_json::Value::Null, serde_json::Value::Null));
              Some(serde_json::json!({
                  "e": "accept", "tick": e.tick.0,
                  "route": diagnostics::route_of(world, contract),
                  "hauler": hauler.slot,
                  "resource": resource,
                  "reward": reward,
              }))
          }
          // "deliver" row: required by WA2/WA4 joins. Previously fell through to
          // _ => None. StationId precedent: use slot (matching Refueled).
          EventKind::ContractFulfilled { contract, hauler } => {
              let k = world
                  .contracts
                  .ids
                  .dense_index(contract.slot, contract.generation);
              let (resource, reward) = k
                  .map(|kidx| {
                      let r = format!("{:?}", world.contracts.resource[kidx]);
                      let w = world.contracts.reward_micros[kidx];
                      (serde_json::Value::String(r), serde_json::json!(w))
                  })
                  .unwrap_or((serde_json::Value::Null, serde_json::Value::Null));
              Some(serde_json::json!({
                  "e": "deliver", "tick": e.tick.0,
                  "route": diagnostics::route_of(world, contract),
                  "hauler": hauler.slot,
                  "resource": resource,
                  "reward": reward,
              }))
          }
          EventKind::Refueled {
              craft,
              station,
              units,
              price_micros,
              tank_before_permille,
              tank_after_permille,
          } => Some(serde_json::json!({
              "e": "refuel", "tick": e.tick.0, "craft": craft.slot,
              "station": station.slot, "units": units,
              "price_micros": price_micros,
              "before_permille": tank_before_permille,
              "after_permille": tank_after_permille,
          })),
          EventKind::LurkMoved {
              pirate,
              to_station,
              breakout,
          } => Some(serde_json::json!({
              "e": "lurk_moved", "tick": e.tick.0, "pirate": pirate.slot,
              "to_station": to_station, "breakout": breakout,
          })),
          // "lie_low" row: required by WB2. Previously fell through to _ => None.
          EventKind::PirateLieLow { pirate, until } => Some(serde_json::json!({
              "e": "lie_low", "tick": e.tick.0, "pirate": pirate.slot,
              "until": until.0,
          })),
          // Variants that are world-scoped or per-tick noise have no gossip row.
          // This exhaustive list prevents future variants from silently vanishing.
          EventKind::Arrival { .. }
          | EventKind::FuelEmpty { .. }
          | EventKind::ThrustApplied { .. }
          | EventKind::ActionIngested { .. }
          | EventKind::Reward { .. }
          | EventKind::Wake { .. }
          | EventKind::Production { .. }
          | EventKind::PriceUpdate { .. }
          | EventKind::ContractOffered { .. }
          | EventKind::ContractFailed { .. }
          | EventKind::DrivenOff { .. }
          | EventKind::HaulerKilled { .. }
          | EventKind::PirateLeft { .. }
          | EventKind::PirateSpawned { .. }
          | EventKind::UpgradePurchased { .. } => None,
      }
  }
  ```

  Note: `EventKind::Trade` is still present in the enum at this task — it is handled
  by the exhaustive None arm group above (it is effectively dead anyway; its deletion
  is A0.3). After A0.3 removes the variant, remove `Trade` from the None arm group
  in that commit.

- [ ] **Step 3: run the new tests — expect pass**

  ```
  cargo test -p jumpgate-core gossip_log_encodes_contract_fulfilled_as_deliver
  cargo test -p jumpgate-core gossip_log_encodes_pirate_lie_low_as_lie_low
  cargo test -p jumpgate-core gossip_log_accept_row_has_resource_and_reward
  cargo test -p jumpgate-core gossip_log_rob_row_has_pirate_field
  ```

  Expected: all four pass.

- [ ] **Step 4: verify the existing gossip-log tests still pass**

  ```
  cargo test -p jumpgate-core gossip_log
  ```

  Expected: `gossip_log_encodes_lurk_moved_for_w6` and
  `gossip_log_encodes_refueled_for_i2` both still pass alongside the new four.

- [ ] **Step 5: clippy + full workspace**

  ```
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
  ```

  Expected: clean.

- [ ] **Step 6: commit**

  ```
  git add crates/jumpgate-core/examples/trophic_run.rs
  git commit -F - <<'EOF'
  feat(a0): gossip-log exhaustive match — deliver/lie_low rows + accept/rob enrichment

  Replaces the _ => None wildcard in gossip_log_event_json with an exhaustive
  match per the synthesis-cut Chronicle mandate. New rows:
  - "deliver" (ContractFulfilled) — required for WA2/WA4 panel joins
  - "lie_low" (PirateLieLow) — required for WB2
  Enriched rows:
  - "accept" gains "resource" + "reward" keys (per-good route traffic)
  - "rob" gains "pirate" slot (engagement attribution panels)

  All new rows are hash-neutral (events are outside state_hash). The Trade
  variant remains in the exhaustive None group pending A0.3 deletion.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A0.3: exhaustive match in `chronicle_subject` + `RefuelDenied` event + remove `EventKind::Trade` corpse

**Why:** Three related concerns land in one commit because they share the
`chronicle_subject` exhaustive-match replacement:
1. The `_ => None` wildcard in `chronicle_subject` (trophic_run.rs:509) silently
   swallows variants. The synthesis cut mandates exhaustive match.
2. `RefuelDenied` (panel critical L4-C2) is a scenario-blind instrument event
   for the WB4 middle beat. It fires at resolve_refuels' silent `continue` sites.
3. `EventKind::Trade` is a dead variant (no production emit; sole constructor is
   a test at contract.rs:417). The synthesis cut requires its deletion in A0
   (its replacement TradeBought/TradeSold lands in A3).

**Ordering note:** `EventKind::Trade` is removed in this commit. After removal,
the exhaustive None arm group in `gossip_log_event_json` (from A0.2) must also
drop the `EventKind::Trade` variant — both files change in this commit.

**Files:**
- Modify: `crates/jumpgate-core/src/contract.rs` — remove `Trade` variant; add
  `RefuelDenied` variant; fix `economy_event_kinds_are_copy_and_partial_eq` test
- Modify: `crates/jumpgate-core/src/economy.rs` — emit `RefuelDenied` at resolve_refuels'
  three continue sites (~lines 1024-1046)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` — exhaustive
  `chronicle_subject`; remove `Trade` from gossip-log None arm

---

- [ ] **Step 1: failing test — chronicle_subject swallows RefuelDenied**

  ```rust
  // In trophic_run.rs #[cfg(test)] mod tests:
  #[test]
  fn chronicle_subject_threads_refuel_denied_to_craft() {
      // RefuelDenied must produce a chronicle line for the stranded-ship arc (WB4).
      // Before A0.3 this variant doesn't exist; the test catches it at compile.
      use jumpgate_core::CraftId;
      // StationId for the denied station
      use jumpgate_core::StationId;
      let craft = CraftId { slot: 3, generation: 0 };
      let station = StationId { slot: 1, generation: 0 };
      let kind = EventKind::RefuelDenied {
          craft,
          station,
          reason: jumpgate_core::RefuelDeniedReason::NoStock,
      };
      assert_eq!(chronicle_subject(&kind), Some(craft),
          "RefuelDenied must thread into craft life arc");
  }
  ```

  Run (expected: compile error — `EventKind::RefuelDenied` and
  `RefuelDeniedReason` do not exist):

  ```
  cargo test -p jumpgate-core chronicle_subject_threads_refuel_denied_to_craft
  ```

  Expected failure text:
  ```
  error[E0599]: no variant `RefuelDenied` found for enum `EventKind`
  ```

- [ ] **Step 2: failing test — Trade variant deletion breaks test**

  Verify the test at contract.rs:391 that constructs `EventKind::Trade` will be
  detected:

  ```
  cargo test -p jumpgate-core economy_event_kinds_are_copy_and_partial_eq
  ```

  Expected: passes now (Trade still exists). This test WILL fail after Trade
  removal unless we update it in the same commit.

- [ ] **Step 3: add `RefuelDeniedReason` and `RefuelDenied` to `EventKind` in contract.rs**

  In `crates/jumpgate-core/src/contract.rs`, directly BEFORE the `EventKind` enum,
  add the reason enum:

  ```rust
  /// Why a refuel attempt was silently skipped (A0 instrument; hash-neutral).
  /// Each variant corresponds to a `continue` site in `resolve_refuels`.
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub enum RefuelDeniedReason {
      /// Station stock was zero or negative (`stock <= 0` guard, economy.rs:1028).
      NoStock,
      /// Craft wallet too low to buy one unit (`afford < 1`, economy.rs:1044).
      CannotAfford,
      /// Tank already full — need rounds to zero (`need < 1`, economy.rs:1040).
      TankFull,
  }
  ```

  In `EventKind`, REMOVE the `Trade` variant (lines 75-80 in contract.rs):

  ```rust
  // DELETE these lines:
  Trade {
      station: StationId,
      resource: Resource,
      qty: u32,
      price_micros: i64,
  },
  ```

  ADD after `LurkMoved` (the last world-gets-big variant):

  ```rust
      // --- Goods-as-goods events (rung A; hash-neutral like all events) ---
      /// A craft's pending refuel was silently skipped because one of three
      /// preconditions failed (A0 instrument; WB4 middle beat). The `craft` is the
      /// one holding `pending_refuel = Some(_)`. `station` is the dock station.
      RefuelDenied {
          craft: CraftId,
          station: StationId,
          reason: RefuelDeniedReason,
      },
  ```

  In `crates/jumpgate-core/src/lib.rs`, the existing re-export is:
  ```rust
  pub use contract::{Command, Event, EventKind, Integrator, StateView, command_sort_key};
  ```
  Add `RefuelDeniedReason` to that list:
  ```rust
  pub use contract::{Command, Event, EventKind, Integrator, RefuelDeniedReason, StateView, command_sort_key};
  ```

- [ ] **Step 4: fix `economy_event_kinds_are_copy_and_partial_eq` test in contract.rs**

  In `crates/jumpgate-core/src/contract.rs`, inside the test at ~line 391, REPLACE
  the `trade` construction and its assertions. The test intent (proving `Copy` +
  `PartialEq`) must be preserved with a different variant.

  Remove these lines:
  ```rust
  let trade = EventKind::Trade {
      station,
      resource: Resource::Ore,
      qty: 3,
      price_micros: 1_000_000,
  };
  ```
  and:
  ```rust
  let trade_copy = trade;
  ```
  and:
  ```rust
  assert_eq!(trade, trade_copy);
  ```
  and:
  ```rust
  assert_ne!(production, trade);
  ```

  Replace with (using `ContractOffered` as the Copy+PartialEq witness — it exists
  and is a world-scoped event variant):

  ```rust
  let offered2 = EventKind::ContractOffered { contract };
  let offered2_copy = offered2;
  assert_eq!(offered2, offered2_copy);
  assert_ne!(production, offered2);
  ```

  The full updated test body (inside `fn economy_event_kinds_are_copy_and_partial_eq`)
  will look like:

  ```rust
  #[test]
  fn economy_event_kinds_are_copy_and_partial_eq() {
      use crate::economy::Resource;
      use crate::ids::{ContractId, CraftId, ProducerId, StationId};

      let producer = ProducerId { slot: 1, generation: 1 };
      let station = StationId { slot: 2, generation: 1 };
      let contract = ContractId { slot: 3, generation: 1 };
      let hauler = CraftId { slot: 4, generation: 1 };

      let production = EventKind::Production {
          producer,
          resource: Resource::Ore,
          qty: 5,
      };
      // Trade is deleted (A0.3); ContractOffered is the Copy+PartialEq proof witness.
      let offered2 = EventKind::ContractOffered { contract };
      let price_update = EventKind::PriceUpdate {
          station,
          resource: Resource::Fuel,
          price_micros: 2_000_000,
      };
      let offered = EventKind::ContractOffered { contract };
      let accepted = EventKind::ContractAccepted { contract, hauler };
      let fulfilled = EventKind::ContractFulfilled { contract, hauler };

      // Copy: binding by assignment leaves the original usable.
      let production_copy = production;
      let offered2_copy = offered2;
      let price_copy = price_update;
      let offered_copy = offered;
      let accepted_copy = accepted;
      let fulfilled_copy = fulfilled;

      // PartialEq: copies equal originals.
      assert_eq!(production, production_copy);
      assert_eq!(offered2, offered2_copy);
      assert_eq!(price_update, price_copy);
      assert_eq!(offered, offered_copy);
      assert_eq!(accepted, accepted_copy);
      assert_eq!(fulfilled, fulfilled_copy);

      // PartialEq: distinct variants differ.
      assert_ne!(production, offered2);
      assert_ne!(price_update, offered);
      assert_ne!(accepted, fulfilled);

      // Wrap one in an Event to confirm the stream type still derives.
      let ev = Event { tick: Tick(7), kind: accepted };
      assert_eq!(ev, ev);
  }
  ```

- [ ] **Step 5: emit `RefuelDenied` at the three silent-continue sites in economy.rs**

  In `crates/jumpgate-core/src/economy.rs`, in the `resolve_refuels` function
  (~line 1013). Find the craft identity resolution before the Refueled emit. We
  need the station resolution to emit `RefuelDenied`. The three guard sites each
  need an emit before their `continue`.

  The `station` resolution (currently after `units` is computed, ~line 1070) must
  be hoisted to be available at the early-exit guards. The existing `craft`
  resolution (`let craft = ships.ids_at(crow);`) must similarly be available.

  Replace the guard block from `let unit_price = ...` through `if afford < 1 { continue; }`:

  ```rust
      let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
          continue;
      };

      // Resolve craft and station identities early so RefuelDenied can be emitted
      // at each guard site (A0 instrument, WB4 middle beat).
      let craft = ships.ids_at(crow);
      let station_id = stations
          .ids
          .id_at(srow)
          .map(|(slot, generation)| StationId { slot, generation });

      let unit_price = stations.price_micros[srow][fuel_r];
      if unit_price < 1 {
          // price<1 guard: no deny event (the station is effectively not selling;
          // this is a config state, not a craft-level denial to narrate).
          continue;
      }
      let stock = stations.stock[srow][fuel_r];
      if stock <= 0 {
          if let Some(station) = station_id {
              events.emit(Event {
                  tick,
                  kind: EventKind::RefuelDenied {
                      craft,
                      station,
                      reason: crate::contract::RefuelDeniedReason::NoStock,
                  },
              });
          }
          continue;
      }
      let port_row = refuel.corp_index as usize;
      if corporations.ids.id_at(port_row).is_none() {
          continue;
      }

      let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
      let cap_eff = eff.fuel_capacity;
      let fuel = ships.fuel_mass[crow];
      let need = ((cap_eff - fuel) / lot).floor() as i64;
      if need < 1 {
          if let Some(station) = station_id {
              events.emit(Event {
                  tick,
                  kind: EventKind::RefuelDenied {
                      craft,
                      station,
                      reason: crate::contract::RefuelDeniedReason::TankFull,
                  },
              });
          }
          continue;
      }
      let afford = ships.credits_micros[crow].max(0) / unit_price;
      if afford < 1 {
          if let Some(station) = station_id {
              events.emit(Event {
                  tick,
                  kind: EventKind::RefuelDenied {
                      craft,
                      station,
                      reason: crate::contract::RefuelDeniedReason::CannotAfford,
                  },
              });
          }
          continue;
      }
  ```

  Remove the now-redundant `let craft = ships.ids_at(crow);` line that appeared
  lower in the function (at ~line 1070) since it is hoisted above.

  The `station_id` resolution for the `Refueled` emit at ~line 1071 was:
  ```rust
      if let Some(station) = stations.ids.id_at(srow).map(|(slot, generation)| StationId { slot, generation }) {
  ```
  Replace it to use the already-resolved `station_id`:
  ```rust
      if let Some(station) = station_id {
  ```

- [ ] **Step 6: add `RefuelDenied` chronicle arm + remove Trade from gossip-log None group**

  In `crates/jumpgate-core/examples/trophic_run.rs`:

  In `chronicle_subject`, REMOVE the `_ => None` wildcard. Replace the function
  body with an exhaustive match. Add `RefuelDenied` as a craft arm. Add Trade's
  former None arm group entries explicitly (Trade is now deleted; confirm the list
  matches the current enum). The new `chronicle_subject`:

  ```rust
  /// The craft a chronicle line belongs to. Per-tick noise (ThrustApplied,
  /// ActionIngested) and world-scoped economy events (offers, prices, production)
  /// have no chronicle subject. This match is EXHAUSTIVE — the wildcard is
  /// intentionally absent so that adding a new EventKind variant forces a
  /// deliberate decision here (synthesis-cut Part 3 Chronicle policy reversal).
  fn chronicle_subject(kind: &EventKind) -> Option<CraftId> {
      match *kind {
          EventKind::Arrival { craft, .. }
          | EventKind::FuelEmpty { craft }
          | EventKind::Wake { craft }
          | EventKind::Reward { craft, .. }
          | EventKind::UpgradePurchased { craft, .. } => Some(craft),
          EventKind::ContractAccepted { hauler, .. }
          | EventKind::ContractFulfilled { hauler, .. } => Some(hauler),
          // World-gets-big §7: refuel and failure thread into the craft's life arc.
          EventKind::Refueled { craft, .. } => Some(craft),
          EventKind::ContractFailed { hauler, .. } => Some(hauler),
          // Goods-as-goods A0: the WB4 middle beat (robbed→broke→RefuelDenied→ADRIFT).
          EventKind::RefuelDenied { craft, .. } => Some(craft),
          EventKind::Robbed { pirate, .. }
          | EventKind::DrivenOff { pirate, .. }
          | EventKind::HaulerKilled { pirate, .. }
          | EventKind::PirateLieLow { pirate, .. }
          | EventKind::PirateLeft { pirate }
          | EventKind::PirateSpawned { pirate }
          | EventKind::LurkMoved { pirate, .. } => Some(pirate),
          // Craft hearings thread into the carrier's arc; station hearings feed
          // the panels (a station-thread chronicle is a named deferral).
          // AlertBorn shadows Robbed: no arm.
          EventKind::GossipHeard {
              carrier: GossipNode::Craft(c),
              ..
          } => Some(c),
          // No craft subject for these world-scoped and noise variants.
          EventKind::GossipHeard { .. }
          | EventKind::AlertBorn { .. }
          | EventKind::ThrustApplied { .. }
          | EventKind::ActionIngested { .. }
          | EventKind::Production { .. }
          | EventKind::PriceUpdate { .. }
          | EventKind::ContractOffered { .. } => None,
      }
  }
  ```

  In `gossip_log_event_json`, remove `EventKind::Trade { .. }` from the exhaustive
  None arm group added in A0.2 (Trade no longer exists in the enum):

  Change:
  ```rust
          | EventKind::HaulerKilled { .. }
          | EventKind::PirateLeft { .. }
          | EventKind::PirateSpawned { .. }
          | EventKind::UpgradePurchased { .. } => None,
  ```
  to:
  ```rust
          | EventKind::HaulerKilled { .. }
          | EventKind::PirateLeft { .. }
          | EventKind::PirateSpawned { .. }
          | EventKind::UpgradePurchased { .. }
          | EventKind::RefuelDenied { .. } => None,
  ```

  (RefuelDenied has no gossip-log row; PirateLieLow now has a row and is
  handled above; the None group is everything without a row.)

- [ ] **Step 7: run all new and affected tests**

  ```
  cargo test -p jumpgate-core chronicle_subject_threads_refuel_denied_to_craft
  cargo test -p jumpgate-core economy_event_kinds_are_copy_and_partial_eq
  cargo test -p jumpgate-core gossip_log
  ```

  Expected: all pass.

- [ ] **Step 8: clippy + full workspace**

  ```
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
  ```

  Expected: clean. The `Trade` removal must not leave any dead-import warnings.

- [ ] **Step 9: commit (single-cause: Trade deletion + RefuelDenied + exhaustive chronicles)**

  ```
  git add crates/jumpgate-core/src/contract.rs \
          crates/jumpgate-core/src/economy.rs \
          crates/jumpgate-core/src/lib.rs \
          crates/jumpgate-core/examples/trophic_run.rs
  git commit -F - <<'EOF'
  feat(a0): RefuelDenied event + exhaustive chronicles + remove dead Trade variant

  Three changes forced into one commit by the exhaustive-match mandate:

  1. Removes EventKind::Trade (dead — sole constructor was a test helper at
     contract.rs:417; zero production emitters). The test economy_event_kinds_-
     are_copy_and_partial_eq is updated to use ContractOffered as the Copy+PartialEq
     proof witness, preserving the test intent.

  2. Adds RefuelDenied{craft, station, reason} + RefuelDeniedReason{NoStock,
     CannotAfford, TankFull} — emitted at resolve_refuels' three formerly-silent
     continue sites. This is the WB4 middle beat instrument (L4-C2 fix). Hash-neutral.

  3. Replaces _ => None wildcards in chronicle_subject and the gossip-log None
     group with exhaustive matches (synthesis-cut Part 3 policy reversal). The
     old doc comment "future variants default to skipped" is removed.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A0.4: BAZAAR anchored stdout line + JSONL transport table row + sweep_trophic.py V5 fixture

**Why:** The synthesis cut (Part 3) requires the BAZAAR anchored line to be
config-gated (optional-presence), regex and fixture land in the SAME commit
(lockstep rule). The WA1 anti-mirroring fix (L4-F4) requires the transport table
echoed as a no-tick JSONL tail row per the `fuel_role` precedent.

The BAZAAR line is a future anchored line for `scenario_bazaar`. It does NOT print
in trophic/frontier runs (the gate is `cfg.bazaar_mode`, a field that does not yet
exist — the structural off). This task adds the infrastructure in anticipation;
the line will fire when scenario_bazaar activates it in A3.

However, because we need the regex in sweep_trophic.py BEFORE scenario_bazaar
prints anything, we add BAZAAR_RE as an optional (False) entry in ANCHORED now,
with a V5 fixture. The Rust side adds the conditional print and the transport-table
tail row as a no-op for trophic/frontier (gate keeps it silent).

**Files:**
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` — add BAZAAR print
  (gated on `meta.bazaar_mode`); transport-table tail row in JSONL
- Modify: `python/analysis/sweep_trophic.py` — add BAZAAR_RE + ANCHORED entry
- Modify: `python/tests/test_sweep_parsing.py` — add V5 fixture + test

---

- [ ] **Step 1: failing test — V5 BAZAAR fixture not yet parsed**

  In `python/tests/test_sweep_parsing.py`, add at the end:

  ```python
  # V5: adds optional BAZAAR anchored line (rung A, scenario_bazaar; config-gated
  # so trophic/frontier stdout stays byte-identical). Regex lands in same commit
  # as the Rust println! (lockstep rule).
  V5_STDOUT = V4_STDOUT + (
      "BAZAAR seed=7 scenario=bazaar exchange_treasury_micros=1234567890 "
      "trade_buys=0 trade_sells=0 arb_posts=0 arb_withdrawals=0\n"
  )


  def test_v5_bazaar_line_parses_and_older_reads_none():
      parsed = sweep.parse_stdout(V5_STDOUT)
      assert parsed["bazaar"] is not None
      assert parsed["bazaar"]["exchange_treasury"] == "1234567890"
      assert parsed["bazaar"]["trade_buys"] == "0"
      for legacy_text in (V1_STDOUT, V2_STDOUT, V3_STDOUT, V4_STDOUT):
          legacy = sweep.parse_stdout(legacy_text)
          assert legacy["bazaar"] is None, "bazaar is None for pre-bazaar stdout"
  ```

  Run (expected: `AttributeError` or `KeyError` — `bazaar` not in ANCHORED):

  ```
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v5_bazaar_line_parses_and_older_reads_none
  ```

  Expected failure text:
  ```
  KeyError: 'bazaar'
  ```

- [ ] **Step 2: add BAZAAR_RE to sweep_trophic.py**

  In `python/analysis/sweep_trophic.py`, after the FUEL_RE block and before the
  `ANCHORED` dict, add:

  ```python
  # The BAZAAR line (rung A, scenario_bazaar; config-gated — absent from
  # trophic/frontier stdout). Regex lands in the SAME commit as the Rust println!
  # (lockstep rule). Optional in ANCHORED: pre-bazaar banked outputs parse as None.
  BAZAAR_RE = re.compile(
      r"^BAZAAR seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
      r"exchange_treasury_micros=(?P<exchange_treasury>-?\d+) "
      r"trade_buys=(?P<trade_buys>\d+) trade_sells=(?P<trade_sells>\d+) "
      r"arb_posts=(?P<arb_posts>\d+) arb_withdrawals=(?P<arb_withdrawals>\d+)$"
  )
  ```

  In `ANCHORED`, add:

  ```python
  ANCHORED = {
      "result": (True, RESULT_RE),
      "media": (True, MEDIA_RE),
      "meta": (False, META_RE),
      "fuel": (False, FUEL_RE),
      "bazaar": (False, BAZAAR_RE),   # rung A, scenario_bazaar; config-gated
  }
  ```

- [ ] **Step 3: add the Rust BAZAAR print + transport-table JSONL row**

  In `crates/jumpgate-core/examples/trophic_run.rs`:

  First, add a `bazaar_mode: bool` field to `MetaFacts`:

  ```rust
  struct MetaFacts {
      scenario: &'static str,
      stations: usize,
      haulers: usize,
      pirates_initial: usize,
      station_radii_milli_au: Vec<u32>,
      bazaar_mode: bool,   // A0: gate for BAZAAR anchored line
  }
  ```

  In `simulate`, where `MetaFacts` is constructed, set:

  ```rust
  let meta = MetaFacts {
      scenario: scenario_name,
      stations: cfg.stations.len(),
      haulers: cfg
          .craft
          .iter()
          .filter(|c| c.role != CraftRole::Pirate)
          .count(),
      pirates_initial: cfg
          .craft
          .iter()
          .filter(|c| c.role == CraftRole::Pirate)
          .count(),
      station_radii_milli_au: cfg
          .stations
          .iter()
          .map(|s| diagnostics::permille_floor(cfg.bodies[s.body_index].elements.a, 1.0))
          .collect(),
      bazaar_mode: false, // set true in scenario_bazaar (A3); trophic/frontier stay silent
  };
  ```

  In `main`, after the existing ASSIGN block and before the gossip-log write,
  add the gated BAZAAR print and the transport-table JSONL tail row:

  ```rust
  // BAZAAR anchored line (rung A, scenario_bazaar; lockstep: regex in same commit).
  // Config-gated: silent for trophic/frontier so banked baseline stays byte-identical.
  if meta.bazaar_mode {
      println!(
          "BAZAAR seed={} scenario={} exchange_treasury_micros={} \
           trade_buys={} trade_sells={} arb_posts={} arb_withdrawals={}",
          args.seed,
          meta.scenario,
          0i64,  // placeholder; scenario_bazaar populates via world accessor in A3
          0u64,  // trade_buys
          0u64,  // trade_sells
          0u64,  // arb_posts
          0u64,  // arb_withdrawals
      );
  }
  ```

  For the transport-table JSONL tail row (WA1 anti-mirroring, L4-F4 fix), add
  inside the `if let Some(mut w) = jsonl_writer` block in `main`, after the
  per-role FUEL rows:

  ```rust
  // Transport-table tail row (WA1 anti-mirroring, synthesis-cut L4-F4):
  // echoes the factory-time integer transport table so the rejection arithmetic
  // reads run-emitted numbers, never mirrored Python constants.
  // No "tick" key — window consumers gate on `"tick" in row`.
  // For trophic/frontier, the table is empty (no bazaar config); the row is
  // still emitted (with an empty array) so the parser contract is unconditional.
  writeln!(
      w,
      "{}",
      serde_json::json!({
          "transport_table": serde_json::Value::Array(Vec::new()),
          // route_costs will be a Vec<{from, to, cost_micros}> in A3 when
          // the bazaar transport table is built; empty here is the structural off.
      })
  )
  .expect("jsonl write");
  ```

- [ ] **Step 4: run the V5 parsing test**

  ```
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v5_bazaar_line_parses_and_older_reads_none
  ```

  Expected: passes.

- [ ] **Step 5: verify all existing parsing tests still pass**

  ```
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py
  ```

  Expected: all tests pass.

- [ ] **Step 6: clippy + full workspace**

  ```
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
  ```

  Expected: clean.

- [ ] **Step 7: commit**

  ```
  git add crates/jumpgate-core/examples/trophic_run.rs \
          python/analysis/sweep_trophic.py \
          python/tests/test_sweep_parsing.py
  git commit -F - <<'EOF'
  feat(a0): BAZAAR anchored line (config-gated) + transport-table JSONL tail row + V5 fixture

  Lockstep: BAZAAR_RE in sweep_trophic.py + BAZAAR println! in trophic_run.rs
  land in the same commit. The line is config-gated (bazaar_mode=false for
  trophic/frontier) so existing banked stdout stays byte-identical. ANCHORED
  marks it optional=False so pre-bazaar parses return None.

  Adds the transport-table no-tick JSONL tail row (WA1 anti-mirroring, L4-F4):
  the factory-time integer transport table is echoed from the run so WA1
  rejection arithmetic never mirrors Python constants. Empty array for
  trophic/frontier (structural off).

  V5 fixture in test_sweep_parsing.py covers the new BAZAAR line and verifies
  older outputs parse as None.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A0.5: META optional `goods=` tail field

**Why:** The synthesis cut §1.1 states META gains an optional tail `goods=`
(optional group — META_RE is `$`-anchored). This lets future bazaar runs report
their goods count in the META line without breaking old regex parsers. The
field prints only when `n_goods > N_RESOURCES` (i.e., in bazaar mode).

**Files:**
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` — optional tail in
  META println!
- Modify: `python/analysis/sweep_trophic.py` — extend META_RE with optional
  `(?: goods=\d+)?`
- Modify: `python/tests/test_sweep_parsing.py` — verify goods=None for trophic/frontier

---

- [ ] **Step 1: update META_RE in sweep_trophic.py**

  In `python/analysis/sweep_trophic.py`, replace:

  ```python
  META_RE = re.compile(
      r"^META seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
      r"stations=(?P<stations>\d+) haulers=(?P<haulers>\d+) "
      r"pirates_initial=(?P<pirates_initial>\d+) "
      r"station_radii_milli_au=\[(?P<radii>[0-9, ]*)\]$"
  )
  ```

  With:

  ```python
  META_RE = re.compile(
      r"^META seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
      r"stations=(?P<stations>\d+) haulers=(?P<haulers>\d+) "
      r"pirates_initial=(?P<pirates_initial>\d+) "
      r"station_radii_milli_au=\[(?P<radii>[0-9, ]*)\]"
      r"(?: goods=(?P<goods>\d+))?$"
  )
  ```

  Note: the final `$` moves to after the optional `goods` group. The regex
  still requires the radii field; `goods` is absent from trophic/frontier output.

- [ ] **Step 2: verify existing META tests still pass**

  ```
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v2_meta_line_parses
  ```

  Expected: passes (V2 stdout has no goods= tail; the group matches None).

- [ ] **Step 3: add the goods= tail to the Rust META println!**

  In `crates/jumpgate-core/examples/trophic_run.rs`, `MetaFacts` needs `n_goods`:

  ```rust
  struct MetaFacts {
      scenario: &'static str,
      stations: usize,
      haulers: usize,
      pirates_initial: usize,
      station_radii_milli_au: Vec<u32>,
      bazaar_mode: bool,
      n_goods: usize,   // A0.5: for optional META goods= tail
  }
  ```

  In `simulate`, set `n_goods: crate::economy::N_RESOURCES` (today's constant; in
  A1+ this becomes the dynamic count from GoodsCfg):

  ```rust
  n_goods: crate::economy::N_RESOURCES,
  ```

  In the META println! (line ~710), replace the existing `println!` call with:

  ```rust
  if meta.bazaar_mode {
      println!(
          "META seed={} scenario={} stations={} haulers={} pirates_initial={} \
           station_radii_milli_au={:?} goods={}",
          args.seed,
          meta.scenario,
          meta.stations,
          meta.haulers,
          meta.pirates_initial,
          meta.station_radii_milli_au,
          meta.n_goods,
      );
  } else {
      println!(
          "META seed={} scenario={} stations={} haulers={} pirates_initial={} \
           station_radii_milli_au={:?}",
          args.seed,
          meta.scenario,
          meta.stations,
          meta.haulers,
          meta.pirates_initial,
          meta.station_radii_milli_au,
      );
  }
  ```

- [ ] **Step 4: add a test that goods= is None for trophic/frontier**

  In `python/tests/test_sweep_parsing.py`, add:

  ```python
  def test_meta_goods_tail_is_none_for_trophic_frontier():
      # trophic/frontier META lines have no goods= tail; parser must return None.
      for text in (V2_STDOUT, V3_STDOUT, V4_STDOUT):
          parsed = sweep.parse_stdout(text)
          assert parsed["meta"] is not None
          assert parsed["meta"]["goods"] is None, \
              "goods= must be None for pre-bazaar META lines"
  ```

  Run:

  ```
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_meta_goods_tail_is_none_for_trophic_frontier
  ```

  Expected: passes.

- [ ] **Step 5: clippy + full workspace + all Python tests**

  ```
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/
  ```

  Expected: clean.

- [ ] **Step 6: commit (this is the LAST A0 commit — baseline pinned here)**

  ```
  git add crates/jumpgate-core/examples/trophic_run.rs \
          python/analysis/sweep_trophic.py \
          python/tests/test_sweep_parsing.py
  git commit -F - <<'EOF'
  feat(a0): META optional goods= tail (config-gated, bazaar_mode only)

  Extends META_RE with an optional `(?: goods=\d+)?` group so pre-bazaar
  banked stdout parses goods=None. The Rust META println! emits the goods=
  tail only when bazaar_mode=true; trophic/frontier stay byte-identical.

  This is the final A0 commit. The behavior-digest baseline is pinned at
  this commit tip per spec §6 and the synthesis-cut digest rule.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A0.6: behavior-digest baseline bank

**Why:** Spec §6 and synthesis-cut Part 3 require the digest baseline (sha256 over
stdout + window-JSONL + gossip-log per scenario/seed) to be pinned at the last A0
commit BEFORE any mechanic merges. Every subsequent phase (A1+) runs this same
procedure and compares. Any divergence is a determinism break requiring bisection.

**Pinned seeds:** 7, 23 (the WGB phase-1 precedent; the baseline uses the same
seeds as the banked phase-1 exit digests in `plans/2026-06-11-world-gets-big-implementation.md`).

**Scenarios:** `trophic` and `frontier` — the two pre-existing scenarios.
`scenario_bazaar` does not exist yet; it will be added in A3 and gets its own
initial digest at that point.

**Run length:** 50k ticks (WA5 bank comparability; `diagnostics::WINDOW_TICKS = 2000`
→ 25 windows).

**Output dir:** `runs/2026-06-13-gag-a0-baseline/` (never staged).

**Post summary:** `docs/superpowers/posts/2026-06-13-gag-a0-baseline-digest.md`
(capture practice: same-day bank).

---

- [ ] **Step 1: verify you are at the A0.5 commit tip**

  ```
  git log --oneline -1
  ```

  Expected: the commit message starts with `feat(a0): META optional goods= tail`.
  If not, DO NOT proceed — the baseline must be pinned at the instruments-complete
  tip.

- [ ] **Step 2: build the release binary**

  ```
  cargo build -p jumpgate-core --release
  ```

  Expected: compiles clean. The binary is at
  `target/release/examples/trophic_run`.

- [ ] **Step 3: create the baseline output directory**

  ```
  mkdir -p /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline
  ```

  This directory is in `runs/` which is gitignored (never stage it).

- [ ] **Step 4: run trophic scenario, seeds 7 and 23**

  ```
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 7 --ticks 50000 --scenario trophic \
    --jsonl /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.jsonl \
    --gossip-log /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl \
    > /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.out

  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 23 --ticks 50000 --scenario trophic \
    --jsonl /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.jsonl \
    --gossip-log /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.gossip.jsonl \
    > /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.out
  ```

- [ ] **Step 5: run frontier scenario, seeds 7 and 23**

  ```
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 7 --ticks 50000 --scenario frontier \
    --jsonl /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.jsonl \
    --gossip-log /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.gossip.jsonl \
    > /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.out

  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 23 --ticks 50000 --scenario frontier \
    --jsonl /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.jsonl \
    --gossip-log /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.gossip.jsonl \
    > /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.out
  ```

- [ ] **Step 6: compute sha256 digests for all 12 files (stdout + window-JSONL + gossip-log per scenario/seed)**

  Window-JSONL files contain all rows; the digest is over the full file (the
  sweep parser's window filter is reader-side only — the raw file is the ground
  truth). Run:

  ```
  sha256sum \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.out \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.out \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.gossip.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.out \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.gossip.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.out \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.jsonl \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.gossip.jsonl \
    > /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/SHA256SUMS
  ```

  Then display:

  ```
  cat /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/SHA256SUMS
  ```

  Paste the output verbatim into the post summary file in the next step.

- [ ] **Step 7: verify determinism — replay-check both scenarios**

  ```
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 7 --ticks 50000 --scenario trophic --replay-check

  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 23 --ticks 50000 --scenario trophic --replay-check

  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 7 --ticks 50000 --scenario frontier --replay-check

  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed 23 --ticks 50000 --scenario frontier --replay-check
  ```

  Each must print:
  ```
  replay-check OK: 50 (tick, state_hash) samples bit-identical (every 1000 ticks)
  ```

  If any says `replay-check FAILED`, STOP. Do not proceed to A1. Bisect the
  A0 commits to find the nondeterminism source.

- [ ] **Step 8: verify the RESULT lines read correctly (spot-check MEDIA line exists)**

  ```
  grep "^RESULT\|^MEDIA\|^META\|^FUEL" \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.out
  grep "^RESULT\|^MEDIA\|^META\|^FUEL" \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.out
  ```

  Expected: each file has exactly one RESULT, one MEDIA, one META, one FUEL line.
  No BAZAAR line (bazaar_mode=false for trophic/frontier).

- [ ] **Step 9: verify new gossip-log rows appear**

  ```
  grep '"e":"deliver"' \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl \
    | head -3
  grep '"e":"lie_low"' \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl \
    | head -3
  grep '"pirate"' \
    /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl \
    | grep '"e":"rob"' | head -3
  ```

  Expected: deliver rows appear (trophic has ContractFulfilled events);
  lie_low rows appear (trophic has PirateLieLow events); rob rows now carry
  `"pirate"` key.

  If deliver/lie_low rows are absent, check the `gossip_log_event_json` exhaustive
  match is in place and the A0.2 commit landed.

- [ ] **Step 10: check window JSONL has per_station_stock/price**

  ```
  python3 -c "
  import json
  with open('/home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.jsonl') as f:
      rows = [json.loads(l) for l in f if l.strip()]
  windows = [r for r in rows if 'tick' in r]
  print('window count:', len(windows))
  w0 = windows[0]
  print('per_station_stock present:', 'per_station_stock' in w0)
  print('per_station_price present:', 'per_station_price' in w0)
  print('per_station_stock shape:', len(w0['per_station_stock']), 'x', len(w0['per_station_stock'][0]))
  transport_rows = [r for r in rows if 'transport_table' in r]
  print('transport_table tail rows:', len(transport_rows))
  "
  ```

  Expected output shape (values will differ by seed/scenario):
  ```
  window count: 25
  per_station_stock present: True
  per_station_price present: True
  per_station_stock shape: 6 x 2
  transport_table tail rows: 1
  ```

  (trophic has 6 stations, 2 resources = N_RESOURCES today; transport_table has 1 tail row.)

- [ ] **Step 11: write the baseline summary to docs/superpowers/posts/**

  Create `docs/superpowers/posts/2026-06-13-gag-a0-baseline-digest.md` with:
  - Git commit hash (`git rev-parse HEAD`)
  - The full SHA256SUMS output pasted verbatim
  - The four replay-check OK lines pasted verbatim
  - Window count and shape from the python spot-check above

  Write the file to that path (do NOT use Write tool — create it with:)

  ```
  mkdir -p /home/john/jumpgate/docs/superpowers/posts/
  git rev-parse HEAD > /tmp/a0_tip.txt
  cat /tmp/a0_tip.txt
  ```

  Then write the summary. Example skeleton (builder fills the literal hash values
  from their actual run):

  ```
  # A0 Instrument Baseline Digest — 2026-06-13

  Commit tip: <paste git rev-parse HEAD output>
  Pinned at: last A0 commit (feat(a0): META optional goods= tail)
  Run length: 50k ticks / 25 windows (W=2000)
  Scenarios: trophic (s7, s23), frontier (s7, s23)

  ## SHA256SUMS (stdout + window-JSONL + gossip-log per scenario/seed)

  <paste verbatim sha256sum output from Step 6>

  ## Replay-check

  All four runs: replay-check OK, 50 (tick, state_hash) samples bit-identical.

  ## Window JSONL spot-check (trophic s7)

  - window count: 25
  - per_station_stock: present, 6 x 2 matrix (n_stations x N_RESOURCES)
  - per_station_price: present, 6 x 2 matrix
  - transport_table tail row: present (1 row, empty array — trophic structural off)

  ## Gossip-log new row spot-check (trophic s7)

  - "deliver" rows: present (ContractFulfilled now emits gossip-log row)
  - "lie_low" rows: present (PirateLieLow now emits gossip-log row)
  - "rob" rows: carry "pirate" field
  - "accept" rows: carry "resource" + "reward" fields

  ## Comparison protocol for A1+

  For each later-phase commit, repeat this procedure (replacing --scenario and
  --seed as needed). Any file-level sha256 divergence vs this baseline is a
  determinism break. For hash-neutral commits (A1 runtime-goods refactor),
  ALL 12 digests must be identical. For behavior-changing commits (A3+),
  record the new digests under a new dated directory.
  ```

- [ ] **Step 12: stage and commit the summary (NOT the runs/ files)**

  ```
  git add docs/superpowers/posts/2026-06-13-gag-a0-baseline-digest.md
  git commit -F - <<'EOF'
  docs(a0): bank behavior-digest baseline at A0 tip

  Pins the sha256 digests (stdout + window-JSONL + gossip-log) for
  scenario_trophic and scenario_frontier at seeds 7 and 23 over 50k ticks.
  All four replay-checks passed. This is the comparison baseline for every
  A1+ phase digest check.

  Per capture practice: banked same-day under docs/superpowers/posts/.
  The raw files are in runs/2026-06-13-gag-a0-baseline/ (gitignored).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

## Phase summary

| Task | Commit message prefix | Hash impact |
|------|----------------------|-------------|
| A0.1 | feat(a0): per-station stock/price matrices | none (event-stream + JSONL only) |
| A0.2 | feat(a0): gossip-log exhaustive match | none |
| A0.3 | feat(a0): RefuelDenied event + exhaustive chronicles + remove dead Trade | none |
| A0.4 | feat(a0): BAZAAR anchored line + transport-table JSONL tail row | none |
| A0.5 | feat(a0): META optional goods= tail | none |
| A0.6 | docs(a0): bank behavior-digest baseline at A0 tip | n/a (docs only) |

All A0 commits are hash-neutral by construction: events are outside `state_hash`
(contract.rs:97-98; hash.rs:195 never reads `world.events`); JSONL keys are additive;
Python regex additions are reader-side only; no state columns are touched. The v6
HASH_FORMAT_VERSION bump (one cause: the `hold` column) is in A2.
