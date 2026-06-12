# Goods as Goods — Rung A (the bazaar) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the bazaar rung — 10 runtime goods with live per-good boards, the Exchange money counterparty, corp arbitrage packages replacing order-up-to restock, one-role/two-mode cargo craft, scenario_bazaar on the frontier band geometry, and the WA1-5 lab — per the APPROVED spec docs/superpowers/specs/2026-06-12-goods-as-goods-design.md (D1-D8 + OD-1..7 resolved) and the panel's recommended cut (docs/superpowers/posts/2026-06-12-goods-as-goods-panel/synthesis-recommended-cut.md, all 17 CRITICALs dispositioned).

**Architecture:** Seven landing phases (A0 instruments+digest baseline → A1 runtime-goods hash-neutral refactor → A2 v6 hold bump + Food → A3 boards+Exchange+trade verbs → A4 arbitrage replaces REPOST → A5 two-mode policy + scenario_bazaar → A6 science+console). Hash discipline: A1 is PROVABLY hash-neutral (state-hash sequence equality, no bump); exactly ONE format bump (v6, the hold column, single-cause); GOLDEN_CONFIG_HASH re-pins single-cause with pasted literals; trophic+frontier behavior digests (stdout+JSONL+gossip-log) match the A0 baseline at the rung exit; rung-B surfaces (jettison/fence/posture/greed/jetsam) are DEFECTS if present.

**Tech Stack:** Rust 2024 (jumpgate-core), examples/trophic_run.rs runner, Python panels (python/, pytest with PYTHONPATH), deterministic seeded ensembles.

---

## Phase A0 — Instruments First + Digest Baseline

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
          | EventKind::UpgradePurchased { .. }
          | EventKind::Trade { .. } => None,
      }
  }
  ```

  Note: `EventKind::Trade` is still present in the enum at this task — it is included
  in the exhaustive None arm group above (it is effectively dead anyway; its deletion
  is A0.3). After A0.3 removes the variant, remove `| EventKind::Trade { .. }` from
  the None arm group in that commit.

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

## Phase A1 — Runtime Goods Representation (OD-1)

# Phase A1 — Runtime Goods Representation (OD-1)

**Phase summary.** Replace the enum `Resource {Ore, Fuel}` + `N_RESOURCES: usize = 2` with a
`Good(pub u16)` newtype and a config-tail `GoodsCfg` block.  Every per-resource array
(`[i64; N_RESOURCES]`) becomes a `Vec<i64>` sized from the goods count at reset.  The state-hash
fold loops iterate `0..n_goods`; with `n_goods == 2` and ascending index order the byte sequence is
bit-identical to today's array loops — the commit is **provably hash-neutral**, verified by
per-tick state-hash sequence equality cross-branch on trophic and frontier.

**No format-version bump in this phase.**  The v6 bump has exactly one cause: the per-craft `hold`
column, which lands in A2.  This phase does not touch `HASH_FORMAT_VERSION`, `GOLDEN_ZERO_STATE_HASH`,
`state_hash_golden_zero_world`, `manual_zero_fold`, or `FRONTIER_TRAJECTORY_GOLDEN`.

**GOLDEN_CONFIG_HASH is NOT re-pinned in A1.**  `GoodsCfg` enters `RunConfig` and is folded in A3
(the one rung-A config commit); A1 itself adds no new field to `RunConfig` — the conversion is
purely mechanical.

---

## A1 split strategy

The mechanical surface is large (seven files, many call sites).  Split into two reviewable commits,
each independently hash-checked:

| Commit | Content | Hash proof |
|---|---|---|
| **A1a** | `Good(u16)` newtype in `economy.rs` + all call sites that *compile but stay functionally identical* | `cargo test --workspace` green + cross-branch sequence equality on trophic seed 7, 1 000 ticks |
| **A1b** | `[i64; N_RESOURCES]` → `Vec<i64>` everywhere (stores, config, scenario, env, hash folds, world.rs reset guard), plus `ResetError::BadGoodsCfg` + pinned-index tests | `cargo test --workspace` green + cross-branch sequence equality on trophic seed 7 AND frontier seed 7, 2 000 ticks |

---

### Task A1.1: Good(u16) newtype — economy.rs, hash.rs, config.rs, contract.rs call sites

**Scope:** introduce `Good(pub u16)` in `economy.rs` alongside the old `Resource` enum (kept alive
as a deprecated alias until A1b removes it), add named constants `ORE` and `FUEL`, and update every
`.index()` call site that the Rust compiler's type system will catch.  No array-to-Vec conversion
yet.  Hash-neutral by construction (all folds still call `.index()` on the same integer values).

**Files:**

- Modify: `crates/jumpgate-core/src/economy.rs` lines 8–24
- Modify: `crates/jumpgate-core/src/hash.rs` lines 326–390, 397–473 (fold sites: `res.index()`)
- Modify: `crates/jumpgate-core/src/config.rs` lines 144–145, 622, 764–776 (Recipe fold)
- Modify: `crates/jumpgate-core/src/contract.rs` lines 70–84 (EventKind Resource fields)
- Modify: `crates/jumpgate-core/src/diagnostics.rs` lines 805–815 (Fuel index reads)
- Modify: `crates/jumpgate-core/src/scenario.rs` lines 29 (use Economy::Resource), 197–209, 396–413 (stock helpers and initializers)
- Modify: `crates/jumpgate-py/src/env.rs` lines 403–406, 412, 416, 422, 426, 459–462

- [ ] **Step 1: Write the failing test (pinned-index contract)**

  In `crates/jumpgate-core/src/economy.rs` test module, add:

  ```rust
  // NEW test — A1 pinned-index contract.  Enum exhaustiveness is gone;
  // this test is the load-bearing substitute that fires if ORE/FUEL are
  // declared at the wrong index or GoodsCfg boot order changes.
  #[test]
  fn good_ore_and_fuel_pinned_indices() {
      // ORE must be index 0, FUEL must be index 1.  These are the canonical
      // dense order used by every per-resource array and by the state hash.
      // If either constant moves, every existing trophic/frontier golden
      // diverges and the cross-branch digest proof fails.
      assert_eq!(Good::ORE.index(), 0, "ORE must be index 0");
      assert_eq!(Good::FUEL.index(), 1, "FUEL must be index 1");
      // Good(u16) must implement Copy, Clone, Debug, PartialEq, Eq — required
      // by Recipe/ContractStore/EventKind which derive these.
      fn needs_copy<T: Copy>() {}
      fn needs_eq<T: Eq>() {}
      needs_copy::<Good>();
      needs_eq::<Good>();
  }
  ```

  Run:

  ```sh
  cargo test -p jumpgate-core good_ore_and_fuel_pinned_indices
  ```

  Expected failure: `error[E0412]: cannot find type 'Good' in this scope` (the type does not exist
  yet).

- [ ] **Step 2: Add `Good(u16)` newtype to `economy.rs`**

  In `crates/jumpgate-core/src/economy.rs`, REPLACE lines 6–24:

  ```rust
  /// Runtime goods newtype (OD-1).  Dense index `0..n_goods` is the canonical
  /// per-resource array key; the numeric value is the GoodsCfg order and is
  /// NEVER folded as a count word — only the value is emitted to the state hash.
  /// Named constants ORE/FUEL pin the v1 pair at indices 0 and 1 (tested by
  /// `good_ore_and_fuel_pinned_indices`); appending new goods is config-only.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct Good(pub u16);

  impl Good {
      /// Canonical dense index (0-based); used by every per-resource array and
      /// by the state-hash fold.
      #[inline]
      pub fn index(self) -> usize {
          self.0 as usize
      }

      /// v1 pinned goods.  Indices are VERIFIED by `good_ore_and_fuel_pinned_indices`.
      pub const ORE:  Good = Good(0);
      pub const FUEL: Good = Good(1);
  }

  /// Backward-compatible alias kept for migration in A1a; removed in A1b once all
  /// call sites are updated.  Declared after `Good` so `Resource::Ore.index()`
  /// still compiles, easing the mechanical conversion.
  #[allow(non_camel_case_types, dead_code)]
  #[deprecated(since = "0.0.0", note = "migrate to Good::ORE / Good::FUEL (A1)")]
  pub type Resource = Good;

  /// Backward-compat shim: re-export old names as associated consts.
  #[allow(dead_code)]
  impl Good {
      #[deprecated(since = "0.0.0", note = "use Good::ORE")]
      pub const Ore:  Good = Good::ORE;
      #[deprecated(since = "0.0.0", note = "use Good::FUEL")]
      pub const Fuel: Good = Good::FUEL;
  }

  /// Number of base goods in v1 (pinned; Experiment C raises this via config).
  /// Used ONLY for fixed-size array literals that survive until A1b converts them
  /// to Vecs; do NOT introduce new uses.
  pub const N_RESOURCES: usize = 2;
  ```

  Note: keeping `Resource` as a type alias means existing code (`Resource::Ore`,
  `Resource::Fuel`, `Resource::ALL`) continues to compile in A1a so the diff is
  reviewable in small slices.

- [ ] **Step 3: Update `Resource::ALL` usage in `update_prices` (economy.rs:325)**

  The grounding extract confirms `Resource::ALL[r]` at economy.rs:325.  With the type alias
  `Resource = Good` the `ALL` constant no longer exists.  Add an `ALL` constant to `Good`:

  In the `impl Good` block (economy.rs), after the `FUEL` const:

  ```rust
      /// All v1 base goods in canonical index order.  Used only by `update_prices`
      /// to build PriceUpdate events; code that needs a runtime count should read
      /// `n_goods` from GoodsCfg (A3).
      pub const ALL_V1: [Good; N_RESOURCES] = [Good::ORE, Good::FUEL];
  ```

  In `economy.rs:325` (inside `update_prices`), change:

  ```rust
  // OLD:
  resource: Resource::ALL[r],
  // NEW:
  resource: Good::ALL_V1[r],
  ```

  (The EventKind field name `resource` stays — it will be renamed `good` when the
  event variants are updated in A2/A4 alongside TradeBought/TradeSold.)

- [ ] **Step 4: Update `Recipe` to use `Good`**

  In `economy.rs`, the `Recipe` struct at line 33:

  ```rust
  // OLD:
  pub struct Recipe {
      pub input: Option<(Resource, u32)>,
      pub output: Option<(Resource, u32)>,
      pub interval: u32,
  }
  // NEW (unchanged compile result because Resource = Good, but explicit type):
  pub struct Recipe {
      pub input: Option<(Good, u32)>,
      pub output: Option<(Good, u32)>,
      pub interval: u32,
  }
  ```

  Because `Resource` is a type alias for `Good`, this is a no-op at the type level but
  makes the migration explicit and will cause a compile warning on the deprecated alias.

- [ ] **Step 5: Update `ContractStore.resource` field type in `economy.rs:162`**

  ```rust
  // OLD:
  pub resource: Vec<Resource>,
  // NEW:
  pub resource: Vec<Good>,
  ```

- [ ] **Step 6: Update `ContractInit.resource` in config.rs:130**

  ```rust
  // OLD:
  pub resource: crate::economy::Resource,
  // NEW:
  pub resource: crate::economy::Good,
  ```

- [ ] **Step 7: Update `EventKind` variants in contract.rs:70–84**

  The `Resource` fields in `Production`, `Trade`, and `PriceUpdate` variants become `Good`.
  Because `Resource = Good` in A1a this is again a compile-only change, but it removes the
  deprecated-alias path:

  ```rust
  Production {
      producer: ProducerId,
      resource: Good,          // was Resource
      qty: u32,
  },
  Trade {
      station: StationId,
      resource: Good,          // was Resource
      qty: u32,
      price_micros: i64,
  },
  PriceUpdate {
      station: StationId,
      resource: Good,          // was Resource
      price_micros: i64,
  },
  ```

- [ ] **Step 8: Update hash fold sites in hash.rs**

  Three fold sites reference `res.index()` on a `Resource`-typed value.  Because
  `Resource = Good` these compile, but update to explicit `Good` type:

  In `write_recipe_hash` (hash.rs:372–390), the `res.index()` calls already work.
  In `write_craft_economy` (hash.rs:328–334) the `cargo: Vec<Option<(Resource, u32)>>` path
  compiles because `Resource = Good`.  No byte-sequence change.

  In `write_economy_stores` (hash.rs:456): `world.contracts.resource[i].index()` — no change
  needed (type alias).

- [ ] **Step 9: Update diagnostics.rs:805–815 to use `Good::FUEL`**

  ```rust
  // OLD:
  .map(|st| st[Resource::Fuel.index()])
  // NEW:
  .map(|st| st[Good::FUEL.index()])
  ```

  (Two occurrences — stock and price.)  Add `use crate::economy::Good;` to the imports at
  the top of diagnostics.rs if it does not already import from economy.

- [ ] **Step 10: Update scenario.rs `use` statement and stock helpers**

  In `scenario.rs:29`:

  ```rust
  // OLD:
  use crate::economy::{Recipe, Resource};
  // NEW:
  use crate::economy::{Good, Recipe};
  ```

  The `stock()` helper closures in `scenario_trophic` (lines 197–201) and `scenario_frontier`
  (lines 396–401) reference `Resource::Ore.index()` and `Resource::Fuel.index()`.  Update to
  use `Good::ORE.index()` and `Good::FUEL.index()` — still compiling against the array form
  (A1b converts to Vec):

  ```rust
  // scenario_trophic stock helper (scenario.rs:197)
  let stock = |ore: i64, fuel: i64| -> [i64; crate::economy::N_RESOURCES] {
      let mut s = [0i64; crate::economy::N_RESOURCES];
      s[Good::ORE.index()]  = ore;
      s[Good::FUEL.index()] = fuel;
      s
  };
  ```

  ```rust
  // scenario_frontier stock helper (scenario.rs:396)
  let stock = |ore: i64, fuel: i64| -> [i64; crate::economy::N_RESOURCES] {
      let mut s = [0i64; crate::economy::N_RESOURCES];
      s[Good::ORE.index()]  = ore;
      s[Good::FUEL.index()] = fuel;
      s
  };
  ```

  `initial_price_micros: [0, fuel_price(fuel)]` at scenario.rs:412 is a positional literal
  that relies on the index 0=Ore, 1=Fuel order.  Leave it as-is in A1a (the positional
  literal is hash-neutral; it will become `vec![0, fuel_price(fuel)]` in A1b).

  Update `ContractInit` rows in scenario.rs (~lines 470–481 for frontier, 243–248 for trophic;
  note: `scenario_bazaar` has zero ContractInit rows so only `scenario_frontier` is affected here):

  ```rust
  // OLD:
  resource: Resource::Ore,
  // NEW:
  resource: Good::ORE,
  ```

  ```rust
  // OLD:
  resource: Resource::Fuel,
  // NEW:
  resource: Good::FUEL,
  ```

  Update the `ProducerInit` recipe fields in the same file (e.g. scenario.rs:213–223, 436–450):

  ```rust
  // OLD:
  output: Some((Resource::Ore, 5))
  // NEW:
  output: Some((Good::ORE, 5))
  ```

  (and similarly all `Resource::Fuel` → `Good::FUEL` in recipe tuples.)

- [ ] **Step 11: Update resolve_refuels in economy.rs (line 1013)**

  ```rust
  // OLD:
  let fuel_r = Resource::Fuel.index();
  // NEW:
  let fuel_r = Good::FUEL.index();
  ```

- [ ] **Step 12: Update World::reset refuel guard in world.rs (line 229)**

  ```rust
  // OLD:
  let fuel = crate::economy::Resource::Fuel.index();
  // NEW:
  let fuel = crate::economy::Good::FUEL.index();
  ```

- [ ] **Step 13: Update jumpgate-py env.rs Recipe and ContractInit fields**

  In `crates/jumpgate-py/src/env.rs`, update all `Resource::Ore` and `Resource::Fuel`
  references in recipe tuples and ContractInit:

  ```rust
  // OLD in recipe:
  output: Some((Resource::Ore, 5))
  // NEW:
  output: Some((Good::ORE, 5))
  ```

  ```rust
  // OLD in ContractInit:
  resource: Resource::Ore,
  // NEW:
  resource: Good::ORE,
  ```

  Add `use jumpgate_core::economy::Good;` (or adjust the existing use block).  The
  `initial_stock: [0, 0]` and `initial_price_micros: [0, 0]` literal arrays at env.rs:403–406
  remain `[i64; N_RESOURCES]` in A1a; converted to Vec in A1b.

- [ ] **Step 14: Run and verify green**

  ```sh
  cargo test --workspace 2>&1 | tail -20
  cargo clippy --all-targets -- -D warnings 2>&1 | grep -E "^error" | head -20
  ```

  Expected: all tests pass; no clippy errors.  Deprecation warnings for `Resource` alias usage
  are acceptable at this stage (all will be removed in A1b).

- [ ] **Step 15: Cross-branch state-hash sequence equality — trophic seed 7, 1 000 ticks**

  This is the A1a hash-neutrality proof.  Run the following on both the pre-A1a tip and this
  commit; the per-tick hash sequence must be bit-identical.

  ```sh
  # On the pre-A1a tip (jumpgate-v1-design):
  cargo build -p jumpgate-core --release 2>/dev/null
  cargo test -p jumpgate-core -- phase1_gate_replay_is_deterministic_state_hash_tick_by_tick \
      --nocapture 2>&1 | grep "^tick" | sha256sum
  ```

  Then build after A1a and run the same command.  The sha256 of the tick-hash stream must match.

  If your environment lacks a convenient cross-branch runner, the following inline Rust snippet
  in a temporary test in `hash.rs` captures the 1000-tick sequence for trophic seed 7:

  ```rust
  #[test]
  #[ignore = "A1a hash-neutrality probe — run before and after the commit, compare outputs"]
  fn print_trophic_tick_hashes_1000() {
      use crate::scenario::scenario_trophic;
      use crate::world::World;
      let (mut w, _) = World::reset(scenario_trophic(7)).expect("trophic seed 7 ok");
      let mut cmds = Vec::new();
      for t in 0..1_000u64 {
          w.step(&mut cmds);
          println!("tick={t} hash={:016x}", crate::hash::state_hash(&w));
      }
  }
  ```

  Run with:

  ```sh
  cargo test -p jumpgate-core -- print_trophic_tick_hashes_1000 --ignored --nocapture \
      2>/dev/null | grep "^tick" | sha256sum
  ```

  The sha256 must match the pre-A1a run exactly.

- [ ] **Step 16: Commit A1a**

  ```sh
  git add \
    crates/jumpgate-core/src/economy.rs \
    crates/jumpgate-core/src/hash.rs \
    crates/jumpgate-core/src/config.rs \
    crates/jumpgate-core/src/contract.rs \
    crates/jumpgate-core/src/diagnostics.rs \
    crates/jumpgate-core/src/scenario.rs \
    crates/jumpgate-core/src/world.rs \
    crates/jumpgate-py/src/env.rs
  git commit -F - <<'EOF'
  refactor(economy): Good(u16) newtype — call-site migration (A1a, hash-neutral)

  Introduces Good(pub u16) with named constants ORE/FUEL (indices 0/1),
  keeping a deprecated Resource=Good type alias so the diff is reviewable
  in two slices.  All Recipe/ContractInit/EventKind/fold call sites updated
  to use Good::ORE and Good::FUEL.  Arrays stay [i64; N_RESOURCES] until A1b.

  Hash-neutral: per-tick state-hash sequence identical to pre-A1a tip on
  trophic seed 7, 1 000 ticks (sha256 verified cross-branch).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A1.2: Vec-backed arrays, GoodsCfg stub, ResetError::BadGoodsCfg, pinned-index tests

**Scope:** convert every `[i64; N_RESOURCES]` to `Vec<i64>` (and `Vec<Good>`, `Vec<Option<(Good,u32)>>`
for the contract/cargo types), remove the deprecated `Resource` alias and `N_RESOURCES` constant,
add the `GoodsCfg` struct to `config.rs` (name and unit_mass_milli only; not yet folded into
config_hash — that happens in A3), add `ResetError::BadGoodsCfg`, and prove hash-neutrality on
both trophic and frontier.

**Files:**

- Modify: `crates/jumpgate-core/src/economy.rs` — `StationStore.stock/price_micros`, `EconCounters`, `StationStore::push`, all index sites
- Modify: `crates/jumpgate-core/src/config.rs` — `StationInit.initial_stock/initial_price_micros`, `PriceCfg.base_micros/cap`, add `GoodsCfg` struct, add `GoodsCfg` field to `RunConfig`
- Modify: `crates/jumpgate-core/src/hash.rs` — `write_economy_stores`, `write_craft_economy` fold loops (no count word added)
- Modify: `crates/jumpgate-core/src/world.rs` — `World::reset` reset loop (array init), `assert_resource_identity`, `ResetError::BadGoodsCfg`
- Modify: `crates/jumpgate-core/src/scenario.rs` — `stock()` helpers, `initial_price_micros` literals
- Modify: `crates/jumpgate-py/src/env.rs` — `initial_stock: [0, 0]` → Vec
- Create or update tests in `crates/jumpgate-core/src/world.rs` test module

- [ ] **Step 1: Write failing test — GoodsCfg validation**

  In `crates/jumpgate-core/src/world.rs` test module:

  ```rust
  #[test]
  fn bad_goods_cfg_zero_goods_is_rejected() {
      // World::reset must return ResetError::BadGoodsCfg when GoodsCfg has
      // zero goods — an n_goods=0 world cannot initialise stock Vecs.
      let mut cfg = crate::scenario::scenario_trophic(0);
      cfg.goods = crate::config::GoodsCfg { goods: vec![] };
      match crate::world::World::reset(cfg) {
          Err(crate::world::ResetError::BadGoodsCfg { reason }) => {
              assert!(reason.contains("zero"), "reason should mention zero goods: {reason}");
          }
          other => panic!("expected BadGoodsCfg, got {other:?}"),
      }
  }

  #[test]
  fn bad_goods_cfg_stock_length_mismatch_is_rejected() {
      // A StationInit with initial_stock length != n_goods must be rejected.
      let mut cfg = crate::scenario::scenario_trophic(0);
      // trophic has n_goods=2; inject a 3-element stock vec.
      cfg.stations[0].initial_stock = vec![0i64, 0, 0];
      match crate::world::World::reset(cfg) {
          Err(crate::world::ResetError::BadGoodsCfg { reason }) => {
              assert!(reason.contains("initial_stock"), "reason should cite initial_stock: {reason}");
          }
          other => panic!("expected BadGoodsCfg, got {other:?}"),
      }
  }
  ```

  Run:

  ```sh
  cargo test -p jumpgate-core bad_goods_cfg
  ```

  Expected failure: `error[E0609]: no field 'goods' on type 'RunConfig'` (GoodsCfg not yet added).

- [ ] **Step 2: Add `GoodsCfg` and `GoodSpec` to config.rs**

  After the `RefuelCfg` definition (around config.rs:430), insert:

  ```rust
  /// Minimal-live per-good property record (OD-7).  `name` is NEVER folded
  /// into any hash (display-only).  `unit_mass_milli` is read by the capacity
  /// gate on every transfer; uniform 1000 in v1 (one unit == one milli-mass).
  /// Additional columns (value_density, perishability, …) land each with their
  /// first reader (the INDUSTRY hook).
  #[derive(Clone, Debug)]
  pub struct GoodSpec {
      /// Human-readable name for console / chronicle output.  Not hashed.
      /// `String` (not `&'static str`) because names will eventually come from
      /// config files; using `String` keeps derive sets consistent and avoids
      /// lifetime annotations at every call site (C6 fix: architecture rationale).
      pub name: String,
      /// Mass per unit in milli-mass (1000 == 1 mass unit).
      pub unit_mass_milli: u32,
  }

  /// The ordered goods table.  `goods[i]` describes `Good(i as u16)`.
  /// `goods.len()` is the authoritative `n_goods` used to size every
  /// per-resource Vec at `World::reset`.  Folded into `config_hash` in A3
  /// (the one rung-A config commit); not yet folded here so A1b stays
  /// hash-neutral on the config side too.
  #[derive(Clone, Debug)]
  pub struct GoodsCfg {
      pub goods: Vec<GoodSpec>,
  }

  impl Default for GoodsCfg {
      /// v1 two-good table (ORE at 0, FUEL at 1).  Matches the v1 pinned indices.
      fn default() -> Self {
          GoodsCfg {
              goods: vec![
                  GoodSpec { name: "Ore".to_string(),  unit_mass_milli: 1000 },
                  GoodSpec { name: "Fuel".to_string(), unit_mass_milli: 1000 },
              ],
          }
      }
  }
  ```

- [ ] **Step 3: Add `goods` field to `RunConfig`**

  In `RunConfig` (config.rs:433), append after `refuel`:

  ```rust
      // Goods-as-goods rung A (folded AFTER refuel in A3, append-only). Default
      // is the v1 two-good table; n_goods = goods.goods.len() sizes all
      // per-resource Vecs at World::reset.
      pub goods: GoodsCfg,
  ```

  Update the **exhaustive destructure** in `config_hash` (config.rs:533) to include `goods`:

  ```rust
  let RunConfig {
      master_seed,
      dt,
      softening,
      substep_cfg,
      ephemeris_window,
      bodies,
      craft,
      guidance,
      stations,
      producers,
      corporations,
      contracts,
      price_cfg,
      dispatch_cfg,
      trophic,
      shipyard,
      media,
      refuel,
      goods,   // NEW (A1b): destructure forces folding in A3
  } = self;
  ```

  The `goods` variable is bound but NOT yet folded (it is used in A3's config-hash extension).
  Add a `let _ = goods;` immediately after the destructure to suppress the unused-variable warning
  until A3:

  ```rust
  let _ = goods; // folded in A3; bound here so adding A3's fold is a compile error until explicit
  ```

  Update the `sample()` function in the test module (config.rs:785) to append:

  ```rust
  goods: GoodsCfg::default(),
  ```

  This keeps `sample()` exhaustive (Rust struct literal completeness).

- [ ] **Step 4: Add `ResetError::BadGoodsCfg` to world.rs**

  In the `ResetError` enum (world.rs:146), append:

  ```rust
      /// The `RunConfig.goods` table is invalid: either zero goods, or a station's
      /// `initial_stock` / `initial_price_micros` Vec length does not equal
      /// `n_goods`.  Rejected before tick 0.
      BadGoodsCfg { reason: &'static str },
  ```

  Add the Display arm in `impl Display for ResetError` (world.rs:167):

  ```rust
  ResetError::BadGoodsCfg { reason } => {
      write!(f, "bad goods config: {reason}")
  }
  ```

- [ ] **Step 5: Convert `StationStore.stock/price_micros` to `Vec<Vec<i64>>`**

  In `economy.rs:42–47`, change:

  ```rust
  // OLD:
  pub stock: Vec<[i64; N_RESOURCES]>,
  pub price_micros: Vec<[i64; N_RESOURCES]>,
  ```

  ```rust
  // NEW:
  pub stock: Vec<Vec<i64>>,
  pub price_micros: Vec<Vec<i64>>,
  ```

  Update `StationStore::push` signature (economy.rs:60–71):

  ```rust
  // OLD:
  pub fn push(
      &mut self,
      body: BodyId,
      stock: [i64; N_RESOURCES],
      price_micros: [i64; N_RESOURCES],
  ) -> StationId {
  ```

  ```rust
  // NEW:
  pub fn push(
      &mut self,
      body: BodyId,
      stock: Vec<i64>,
      price_micros: Vec<i64>,
  ) -> StationId {
  ```

  The body of `push` is unchanged (it calls `.push(stock)` and `.push(price_micros)`).

- [ ] **Step 6: Convert `EconCounters` to `Vec<i64>`**

  In `economy.rs:215–224`:

  ```rust
  // OLD:
  pub struct EconCounters {
      pub mined: [i64; N_RESOURCES],
      pub consumed: [i64; N_RESOURCES],
  }
  impl EconCounters {
      pub fn zero() -> Self {
          EconCounters { mined: [0; N_RESOURCES], consumed: [0; N_RESOURCES] }
      }
  }
  ```

  ```rust
  // NEW:
  pub struct EconCounters {
      pub mined: Vec<i64>,
      pub consumed: Vec<i64>,
  }
  impl EconCounters {
      /// All-zero counters sized for `n_goods`.
      pub fn zero(n_goods: usize) -> Self {
          EconCounters { mined: vec![0i64; n_goods], consumed: vec![0i64; n_goods] }
      }
  }
  ```

  Note: `EconCounters::zero()` gains an argument.  All call sites in `World::reset` must pass
  `n_goods` (see Step 11).

- [ ] **Step 7: Convert `StationInit` and `PriceCfg` fields in config.rs**

  `StationInit.initial_stock` and `initial_price_micros` (config.rs:101–108):

  ```rust
  // OLD:
  pub initial_stock: [i64; crate::economy::N_RESOURCES],
  pub initial_price_micros: [i64; crate::economy::N_RESOURCES],
  ```

  ```rust
  // NEW:
  pub initial_stock: Vec<i64>,
  pub initial_price_micros: Vec<i64>,
  ```

  `PriceCfg.base_micros` and `cap` (config.rs:143–145):

  ```rust
  // OLD:
  pub base_micros: [i64; crate::economy::N_RESOURCES],
  pub cap: [i64; crate::economy::N_RESOURCES],
  ```

  ```rust
  // NEW:
  pub base_micros: Vec<i64>,
  pub cap: Vec<i64>,
  ```

  Update `Default for PriceCfg` (config.rs:152–160):

  ```rust
  impl Default for PriceCfg {
      fn default() -> Self {
          PriceCfg {
              base_micros: vec![0i64; crate::economy::N_GOODS_V1],
              cap: vec![1i64; crate::economy::N_GOODS_V1],
              slope_milli: 1800,
              reprice_interval: 1,
          }
      }
  }
  ```

  Add the constant `N_GOODS_V1` to economy.rs (to be used only in Default impls and tests; all
  runtime sizing uses `cfg.goods.goods.len()`):

  ```rust
  /// Number of goods in the v1 table.  Use for Default impls and old-lineage
  /// tests only; runtime sizing must read from GoodsCfg.
  pub const N_GOODS_V1: usize = 2;
  ```

  Update `config_hash` fold loops in config.rs:607–610 and config.rs:630–633 to use `.len()`:

  ```rust
  // OLD (config.rs:607):
  for r in 0..crate::economy::N_RESOURCES {
      h.write_u64(s.initial_stock[r] as u64);
      h.write_u64(s.initial_price_micros[r] as u64);
  }
  // NEW (still no count word — A3 adds it when GoodsCfg is folded):
  for r in 0..s.initial_stock.len() {
      h.write_u64(s.initial_stock[r] as u64);
      h.write_u64(s.initial_price_micros[r] as u64);
  }
  ```

  ```rust
  // OLD (config.rs:630):
  for r in 0..crate::economy::N_RESOURCES {
      h.write_u64(price_cfg.base_micros[r] as u64);
      h.write_u64(price_cfg.cap[r] as u64);
  }
  // NEW:
  for r in 0..price_cfg.base_micros.len() {
      h.write_u64(price_cfg.base_micros[r] as u64);
      h.write_u64(price_cfg.cap[r] as u64);
  }
  ```

  With n_goods still 2, the loop body is byte-identical to the old `N_RESOURCES` form.

- [ ] **Step 8: Update `write_economy_stores` fold loops in hash.rs**

  In `write_economy_stores` (hash.rs:397–473), the two sets of per-resource loops:

  **EconCounters loop (hash.rs:400–404):**

  ```rust
  // OLD:
  use crate::economy::N_RESOURCES;
  for r in 0..N_RESOURCES {
      h.write_u64(world.econ.mined[r] as u64);
  }
  for r in 0..N_RESOURCES {
      h.write_u64(world.econ.consumed[r] as u64);
  }
  ```

  ```rust
  // NEW (no count word — byte-identical at n_goods == 2):
  for v in &world.econ.mined {
      h.write_u64(*v as u64);
  }
  for v in &world.econ.consumed {
      h.write_u64(*v as u64);
  }
  ```

  **Per-station stock/price loop (hash.rs:416–419):**

  ```rust
  // OLD:
  for r in 0..N_RESOURCES {
      h.write_u64(world.stations.stock[i][r] as u64);
      h.write_u64(world.stations.price_micros[i][r] as u64);
  }
  ```

  ```rust
  // NEW (no count word; ascending index order preserved by Vec iteration):
  for (s, p) in world.stations.stock[i].iter().zip(world.stations.price_micros[i].iter()) {
      h.write_u64(*s as u64);
      h.write_u64(*p as u64);
  }
  ```

  Remove `use crate::economy::N_RESOURCES;` from `write_economy_stores` since it is no longer
  needed.

- [ ] **Step 9: Update `write_recipe_hash` and `write_craft_economy` — no structural change**

  `write_recipe_hash` (hash.rs:372–390) uses `res.index()` which is a `Good::index()` call —
  already updated in A1a.  No structural change needed.

  `write_craft_economy` (hash.rs:328–334) uses `cargo: Vec<Option<(Resource, u32)>>` (= `Good`).
  The fold is already correct (no loop over goods).

- [ ] **Step 10: Update `assert_resource_identity` in world.rs:2870**

  The function currently takes `&[i64; N_RESOURCES]`.  Convert to `&[i64]` (slice):

  ```rust
  // OLD:
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

  ```rust
  // NEW:
  fn assert_resource_identity(world: &World, initial: &[i64]) {
      let n = initial.len();
      for r in 0..n {
          let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
          let in_transit: i64 = world
              .ships
              .cargo
              .iter()
              .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
              .sum();
          let lhs = stock + in_transit;
          let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
          assert_eq!(
              lhs, rhs,
              "resource identity for r={r}: {lhs} != {rhs} (stock+in_transit vs initial+mined-consumed)"
          );
      }
  }
  ```

  Update the call site (`phase1_gate_resource_accounting_identity_holds_every_tick`,
  world.rs:2889–2898) to build a `Vec<i64>` instead of `[0i64; N_RESOURCES]`:

  ```rust
  // OLD:
  use crate::economy::{N_RESOURCES, Resource};
  let mut initial = [0i64; N_RESOURCES];
  for (r, slot) in initial.iter_mut().enumerate() {
      *slot = world.stations.stock.iter().map(|s| s[r]).sum();
  }
  ```

  ```rust
  // NEW:
  let n_goods = world.stations.stock.first().map(|v| v.len()).unwrap_or(0);
  let mut initial: Vec<i64> = (0..n_goods)
      .map(|r| world.stations.stock.iter().map(|s| s[r]).sum())
      .collect();
  ```

- [ ] **Step 11: Update `World::reset` stock Vec initialization**

  In `World::reset` (world.rs:192+), the reset block that seeds station stocks must now:
  (a) validate `n_goods` via `BadGoodsCfg`, and (b) convert `StationInit.initial_stock` to the
  Vec that `StationStore::push` expects.

  Immediately after the config hash computation (after `let hash = cfg.config_hash();`,
  world.rs:197):

  ```rust
  // Validate goods table (A1b): reject zero-goods configs and length mismatches
  // before minting any stores.
  let n_goods = cfg.goods.goods.len();
  if n_goods == 0 {
      return Err(ResetError::BadGoodsCfg { reason: "GoodsCfg has zero goods" });
  }
  for (si, s) in cfg.stations.iter().enumerate() {
      if s.initial_stock.len() != n_goods {
          return Err(ResetError::BadGoodsCfg {
              reason: "station initial_stock length != n_goods",
          });
      }
      if s.initial_price_micros.len() != n_goods {
          return Err(ResetError::BadGoodsCfg {
              reason: "station initial_price_micros length != n_goods",
          });
      }
      let _ = si; // si used in error messages if the & str is expanded later
  }
  if cfg.price_cfg.base_micros.len() != n_goods || cfg.price_cfg.cap.len() != n_goods {
      return Err(ResetError::BadGoodsCfg {
          reason: "PriceCfg base_micros or cap length != n_goods",
      });
  }
  ```

  Update `EconCounters::zero()` call (world.rs, wherever it appears):

  ```rust
  // OLD:
  let econ = EconCounters::zero();
  // NEW:
  let econ = EconCounters::zero(n_goods);
  ```

  The refuel guard in world.rs:228–244 currently indexes `cfg.price_cfg.base_micros[fuel]` and
  `s.initial_price_micros[fuel]` using the old `fuel` constant.  Update:

  ```rust
  // OLD:
  let fuel = crate::economy::Resource::Fuel.index();
  if cfg.price_cfg.base_micros[fuel] == 0 {
  ```

  ```rust
  // NEW:
  let fuel = crate::economy::Good::FUEL.index();
  if cfg.price_cfg.base_micros.get(fuel).copied().unwrap_or(0) == 0 {
  ```

  (`.get(fuel)` is safe because the BadGoodsCfg validation above already checked lengths.)

- [ ] **Step 12: Update scenario.rs `stock()` helpers and literal arrays**

  In `scenario_trophic` (scenario.rs:197–210), convert the `stock` helper and its callers:

  ```rust
  // NEW stock helper (returns Vec<i64>, length 2 matching N_GOODS_V1):
  let stock = |ore: i64, fuel: i64| -> Vec<i64> {
      let mut s = vec![0i64; crate::economy::N_GOODS_V1];
      s[crate::economy::Good::ORE.index()]  = ore;
      s[crate::economy::Good::FUEL.index()] = fuel;
      s
  };
  ```

  Update all `initial_price_micros: [0, 0]` literals in scenario_trophic (lines 204–209) to
  `initial_price_micros: vec![0i64, 0i64]`.

  In `scenario_frontier` (scenario.rs:396–433), apply the same conversion:

  ```rust
  // NEW stock helper:
  let stock = |ore: i64, fuel: i64| -> Vec<i64> {
      let mut s = vec![0i64; crate::economy::N_GOODS_V1];
      s[crate::economy::Good::ORE.index()]  = ore;
      s[crate::economy::Good::FUEL.index()] = fuel;
      s
  };
  ```

  The `station` helper (scenario.rs:409):

  ```rust
  // OLD:
  let station = |body_index: usize, ore: i64, fuel: i64, vendor: bool| StationInit {
      body_index,
      initial_stock: stock(ore, fuel),
      initial_price_micros: [0, fuel_price(fuel)],
      sells_upgrades: vendor,
  };
  // NEW:
  let station = |body_index: usize, ore: i64, fuel: i64, vendor: bool| StationInit {
      body_index,
      initial_stock: stock(ore, fuel),
      initial_price_micros: {
          let mut p = vec![0i64; crate::economy::N_GOODS_V1];
          p[crate::economy::Good::FUEL.index()] = fuel_price(fuel);
          p
      },
      sells_upgrades: vendor,
  };
  ```

  Add `goods: crate::config::GoodsCfg::default()` to both `RunConfig` constructors in
  `scenario_trophic` and `scenario_frontier` (the RunConfig struct literals at the ends of both
  factory functions).

- [ ] **Step 13: Update jumpgate-py env.rs array literals**

  In `crates/jumpgate-py/src/env.rs` (lines 403–406):

  ```rust
  // OLD:
  initial_stock: [0, 0],
  initial_price_micros: [0, 0],
  ```

  ```rust
  // NEW:
  initial_stock: vec![0i64, 0i64],
  initial_price_micros: vec![0i64, 0i64],
  ```

  Add `goods: jumpgate_core::config::GoodsCfg::default()` to the RunConfig literal in env.rs.

- [ ] **Step 14: Remove deprecated `Resource` alias and `N_RESOURCES`**

  With all call sites updated to `Good::ORE`/`Good::FUEL`/`Good::ALL_V1`, remove from economy.rs:

  - The `#[deprecated] pub type Resource = Good;` block
  - The `#[deprecated] pub const Ore/Fuel` consts
  - The `pub const N_RESOURCES: usize = 2;` constant

  Fix any compile errors that surface.  The Rust compiler will identify remaining uses.

- [ ] **Step 15: Run all tests and clippy**

  ```sh
  cargo test --workspace 2>&1 | tail -30
  cargo clippy --all-targets -- -D warnings 2>&1 | grep "^error" | head -20
  ```

  Expected: all tests pass; no clippy errors.

- [ ] **Step 16: Cross-branch state-hash sequence equality — trophic AND frontier, 2 000 ticks**

  This is the definitive A1b hash-neutrality proof.  Run the probe test from A1a on both tips:

  ```sh
  # trophic seed 7, 2 000 ticks:
  cargo test -p jumpgate-core -- print_trophic_tick_hashes_1000 --ignored --nocapture \
      2>/dev/null | grep "^tick" | sha256sum

  # frontier seed 7, 2 000 ticks (add an equivalent ignored test):
  ```

  Add a second probe test to hash.rs:

  ```rust
  #[test]
  #[ignore = "A1b hash-neutrality probe — frontier, run before/after, compare outputs"]
  fn print_frontier_tick_hashes_2000() {
      use crate::scenario::scenario_frontier;
      use crate::world::World;
      let (mut w, _) = World::reset(scenario_frontier(7)).expect("frontier seed 7 ok");
      let mut cmds = Vec::new();
      for t in 0..2_000u64 {
          w.step(&mut cmds);
          println!("tick={t} hash={:016x}", crate::hash::state_hash(&w));
      }
  }
  ```

  Run:

  ```sh
  cargo test -p jumpgate-core -- print_frontier_tick_hashes_2000 --ignored --nocapture \
      2>/dev/null | grep "^tick" | sha256sum
  ```

  Both sha256 values must match their pre-A1b counterparts.

  The A1b cross-branch proof is the **definitive hash-neutrality attestation** recorded in the
  commit message.  It is NOT a gate on any other metric — it is the determinism contract for
  this commit.

- [ ] **Step 17: Commit A1b**

  ```sh
  git add \
    crates/jumpgate-core/src/economy.rs \
    crates/jumpgate-core/src/config.rs \
    crates/jumpgate-core/src/hash.rs \
    crates/jumpgate-core/src/world.rs \
    crates/jumpgate-core/src/scenario.rs \
    crates/jumpgate-py/src/env.rs
  git commit -F - <<'EOF'
  refactor(economy): Vec-backed stocks, GoodsCfg stub, BadGoodsCfg (A1b, hash-neutral)

  Converts all [i64; N_RESOURCES] arrays to Vec<i64> (StationStore, EconCounters,
  StationInit, PriceCfg).  Adds GoodsCfg {goods: Vec<GoodSpec>} to RunConfig with
  default v1 two-good table; GoodsCfg is NOT yet folded into config_hash (A3).
  Adds ResetError::BadGoodsCfg rejecting n_goods=0 and length mismatches before
  tick 0.  Removes the deprecated Resource alias and N_RESOURCES constant.

  Hash-neutral: per-tick state-hash sequence bit-identical to pre-A1b tip on
  trophic seed 7 (2 000 ticks) AND frontier seed 7 (2 000 ticks) — sha256 verified
  cross-branch.  HASH_FORMAT_VERSION unchanged (still 5); v6 is the hold column (A2).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

## Hash-neutrality proof: exact cross-branch digest commands

The following is the **complete reproducible recipe** for the A1 cross-branch hash-neutrality proof.
Run this verbatim before and after the A1a/A1b commits and compare the sha256 values.

```sh
# --- Pre-A1 baseline (on branch jumpgate-v1-design before any A1 commit) ---
git stash                       # save working tree if needed
BASELINE_BRANCH=$(git rev-parse --abbrev-ref HEAD)
BASELINE_SHA=$(git rev-parse HEAD)

cargo test -p jumpgate-core -- print_trophic_tick_hashes_1000 --ignored --nocapture \
    2>/dev/null | grep "^tick" > /tmp/pre_a1_trophic_hashes.txt
sha256sum /tmp/pre_a1_trophic_hashes.txt

cargo test -p jumpgate-core -- print_frontier_tick_hashes_2000 --ignored --nocapture \
    2>/dev/null | grep "^tick" > /tmp/pre_a1_frontier_hashes.txt
sha256sum /tmp/pre_a1_frontier_hashes.txt

# --- Post-A1b (on the A1b commit) ---
# (apply and commit A1a, then A1b)
cargo test -p jumpgate-core -- print_trophic_tick_hashes_1000 --ignored --nocapture \
    2>/dev/null | grep "^tick" > /tmp/post_a1b_trophic_hashes.txt
sha256sum /tmp/post_a1b_trophic_hashes.txt

cargo test -p jumpgate-core -- print_frontier_tick_hashes_2000 --ignored --nocapture \
    2>/dev/null | grep "^tick" > /tmp/post_a1b_frontier_hashes.txt
sha256sum /tmp/post_a1b_frontier_hashes.txt

# Both sha256 lines must match their pre-A1 counterpart exactly.
diff /tmp/pre_a1_trophic_hashes.txt  /tmp/post_a1b_trophic_hashes.txt  && echo "TROPHIC OK"
diff /tmp/pre_a1_frontier_hashes.txt /tmp/post_a1b_frontier_hashes.txt && echo "FRONTIER OK"
```

Both "OK" lines must print.  Any diff means a state-hash fold was accidentally modified — check
the `write_economy_stores` and `write_craft_economy` functions first (count words and iteration order).

## Phase A2 — Format v6: the `hold` Column + Food as Good(2)

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

### Task A2.3: DELETED — superseded by A3.3 (synthesis C3)

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

3. **A2.4** `feat(scenario): Food as Good(2) + consumption sinks in scenario_bazaar`
   - `Good::FOOD = Good(2)` const.
   - 3 input-only Food consumption producers in scenario_bazaar.
   - trophic/frontier behavior digests verified unchanged (stdout+JSONL).

4. **A2.5** (verification step — no commit) behavior digest cross-check recorded.

## Phase A3 — Boards, the Exchange, and the Own-trade Verbs

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
    // NOTE (M2): GoodsCfg/GoodSpec imported from crate::config, not crate::economy.
    use crate::config::{GoodsCfg, GoodSpec};
    let goods_cfg = GoodsCfg {
        goods: vec![
            GoodSpec { name: "Ore".to_string(),    unit_mass_milli: 1000 },
            GoodSpec { name: "Fuel".to_string(),   unit_mass_milli: 1000 },
            GoodSpec { name: "Widget".to_string(), unit_mass_milli: 1000 },
        ],
    };
    let n = goods_cfg.goods.len();
    // NOTE (M2): use real StationStore constructors (empty + push), not the
    // non-existent empty_with_goods/push_goods methods.
    let mut station = StationStore::empty();
    station.push(
        crate::ids::BodyId { slot: 0, generation: 0 },
        vec![0i64; n],
        vec![0i64; n],
    );

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

Update the `World::step` call site in `world.rs` (stage 3d) to pass `&self.config.goods`
(NOTE C6: field is `goods`, not `goods_cfg`)
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
  **`GoodSpec` and `GoodsCfg` are defined HERE in A3.2 as the canonical definition; A1.2's
  definition is superseded by this one. Use `String` for `name` (not `&'static str`) per C6.**
- `ArbitrageCfg { scan_interval: u32, wage_flat_micros: i64, wage_share_milli: u32,
  transport_micros: Vec<Vec<i64>>, qty_ladder: Vec<u32>, max_posts_per_scan: usize,
  arb_premium_micros: Vec<i64> }` — `scan_interval == 0` is the structural inert gate.
  All-inert default (`scan_interval: 0`, empty Vecs). **This is the ONE complete definition
  for rung A (C1 canonical superset). A4.2 and A5.2 reference this struct, they do NOT
  redefine it.**
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
        transport_micros: vec![],
        qty_ladder: vec![],
        max_posts_per_scan: 0,
        arb_premium_micros: vec![],
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

- [ ] **Step 2: add ExchangeCfg and ArbitrageCfg structs**

> **NOTE (C6):** `GoodSpec` and `GoodsCfg` are already defined in A1.2
> (`crates/jumpgate-core/src/economy.rs`). Do NOT redefine them here — use
> `crate::economy::{GoodSpec, GoodsCfg}` or `use super::` as appropriate.

```rust
// crates/jumpgate-core/src/config.rs — insert before RunConfig struct
// GoodSpec / GoodsCfg come from economy.rs (A1.2); only add what is new here.

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

// NOTE (C1): ONE canonical ArbitrageCfg for rung A. A4.2 and A5.2 reference
// this struct and do NOT redefine it. This is the A5.2 superset field set.
/// Arbitrage poster config (stage 1b2 slot, OD-2/spec §1.2).
/// `scan_interval == 0` is the structural inert gate: the poster returns
/// immediately without scanning, preserving bit-identical behavior on
/// trophic/frontier (the RefuelCfg.lot_mass precedent).
#[derive(Clone, Debug)]
pub struct ArbitrageCfg {
    /// Ticks between poster scans. 0 = poster is OFF (the structural inert gate).
    pub scan_interval: u32,
    /// Fixed transport-floor component of posted wage (micros).
    pub wage_flat_micros: i64,
    /// Fraction of spread surplus added to wage: `surplus * wage_share_milli / 1000`.
    pub wage_share_milli: u32,
    /// Factory-time transport cost table: `transport_micros[from][to]` non-negative int.
    /// Folded count-first in config_hash. NOT runtime ephemeris (PDR-0007).
    pub transport_micros: Vec<Vec<i64>>,
    /// Lot-size ladder (units). Smallest-first.
    pub qty_ladder: Vec<u32>,
    /// Maximum contracts posted per scan across all routes.
    pub max_posts_per_scan: usize,
    /// Minimum surplus above transport before posting, per-corp (indexed by corp row).
    pub arb_premium_micros: Vec<i64>,
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
/// NOTE (C6): field name is `goods` (not `goods_cfg`) — A1b adds `goods: GoodsCfg`;
/// A3.2 folds the existing `goods` field. Do NOT add a second `goods_cfg` field.
pub goods: GoodsCfg,
/// Exchange configuration: the money counterparty for goods trades.
pub exchange: ExchangeCfg,
/// Arbitrage poster configuration.
pub arbitrage: ArbitrageCfg,
```

In `config_hash()`, extend the exhaustive destructure to bind `goods`, `exchange`,
`arbitrage`, and in the per-corp loop bind `arb_premium_micros`. Fold at the CONFIG
tail (after RefuelCfg, append-only — CONFIG_FIELD_ORDER words 27..=30):

```rust
// GOODS-AS-GOODS RUNG A (TAIL, append-only — CONFIG_FIELD_ORDER 27..=30).
// Exhaustive destructures: a new field is a compile error until folded.
let GoodsCfg { goods } = &self.goods; // NOTE (C6): field is `goods`, not `goods_cfg`
// COUNT FIRST (anti-aliasing delimiter, config fold discipline):
h.write_u64(goods.len() as u64);
for g in goods {
    let GoodSpec { name: _, unit_mass_milli } = g; // name NEVER folded
    h.write_u64(*unit_mass_milli as u64);
}
let ExchangeCfg { corp_index: ex_corp, active } = exchange;
h.write_u64(*ex_corp as u64);
h.write_u64(*active as u64);
let ArbitrageCfg {
    scan_interval, wage_flat_micros, wage_share_milli,
    transport_micros, qty_ladder, max_posts_per_scan, arb_premium_micros,
} = arbitrage;
h.write_u64(*scan_interval as u64);
h.write_u64(*wage_flat_micros as u64);
h.write_u64(*wage_share_milli as u64);
h.write_u64(transport_micros.len() as u64);
for row in transport_micros {
    h.write_u64(row.len() as u64);
    for &v in row { h.write_u64(v as u64); }
}
h.write_u64(qty_ladder.len() as u64);
for &q in qty_ladder { h.write_u64(q as u64); }
h.write_u64(*max_posts_per_scan as u64);
h.write_u64(arb_premium_micros.len() as u64);
for &p in arb_premium_micros { h.write_u64(p as u64); }
// Note: arb_premium_micros is on ArbitrageCfg (folded above). CorporationInit
// also carries arb_premium_micros (per-corp override) — fold in the per-corp loop:
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
goods: GoodsCfg::default(),  // NOTE (C6): field name is `goods`, not `goods_cfg`
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

### Task A3.4: TradeBought/TradeSold events + exhaustive match arms

Adds `TradeBought` and `TradeSold` to `EventKind` (note: `EventKind::Trade` was
already deleted in A0.3; this task does NOT delete it again). Updates the test
`economy_event_kinds_are_copy_and_partial_eq` to prove `TradeBought` is Copy+PartialEq,
and adds chronicle arms + gossip-log arms in the same commit. The matches in
`chronicle_subject` and `gossip_log_event_json` are already exhaustive after A0.3
removed the wildcard (C2 fix: the deletion in this task's Step 2 is removed — it
already landed in A0.3).

**Single-emit discipline:** `TradeBought` is emitted exactly once — in
`resolve_trade_buys` after all accounting legs settle. `TradeSold` is emitted exactly
once — in `resolve_trade_sells`.

#### Files

- Modify: `crates/jumpgate-core/src/contract.rs` — add `TradeBought`/`TradeSold`
- Modify: `crates/jumpgate-core/src/trophic_run.rs` — `chronicle_subject`,
  `gossip_log_event_json`, add new arms (matches already exhaustive after A0.3)
- Modify: `crates/jumpgate-core/src/economy.rs` — update test `economy_event_kinds_are_copy_and_partial_eq`

- [ ] **Step 1: failing test — TradeBought/TradeSold are in EventKind and are Copy+PartialEq**

```rust
// crates/jumpgate-core/src/contract.rs — replace the existing
// economy_event_kinds_are_copy_and_partial_eq test body:
// NOTE (C2): `Resource` is removed in A1b; use `crate::economy::Good` only.
#[test]
fn economy_event_kinds_are_copy_and_partial_eq() {
    use crate::ids::{CraftId, StationId};
    use crate::economy::Good;  // NOTE (C2): no Resource import; Resource::Ore → Good::ORE
    use crate::time::Tick;

    let production = EventKind::Production {
        producer: crate::ids::ProducerId { slot: 0, generation: 0 },
        resource: Good::ORE,   // NOTE (C2): was Resource::Ore
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

- [ ] **Step 2: add TradeBought/TradeSold to EventKind**

Note (C2): `EventKind::Trade` was already deleted in A0.3. Do NOT delete it again here.

```rust
// crates/jumpgate-core/src/contract.rs — in EventKind enum:
// ADD (new group, hash-neutral like all events per contract.rs:169 idiom).
// NOTE: Trade was already removed in A0.3; do not attempt to delete it here.

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

- [ ] **Step 3: add TradeBought/TradeSold arms in chronicle_subject**

Note (M1): the matches are already exhaustive after A0.3 removed the `_ => None`
wildcard. ADD arms for the new variants — a missing arm is a compile error.
Do NOT include any wildcard-removal instruction.

```rust
// crates/jumpgate-core/src/trophic_run.rs — chronicle_subject function.
// The matches are already exhaustive (A0.3 removed the wildcard).
// ADD arms for the new variants:
EventKind::TradeBought { craft, .. } => Some(*craft),
EventKind::TradeSold   { craft, .. } => Some(*craft),
// Variants with no craft subject — explicit None arms:
EventKind::Production { .. }    => None,
EventKind::PriceUpdate { .. }   => None,
EventKind::ContractOffered { .. } => None,
// (All existing arms already present; simply insert the two new arms above.)
```

The function compiles exhaustively — if any `EventKind` variant is missing from
the match, Rust will refuse to compile.

- [ ] **Step 4: add gossip_log_event_json arms for TradeBought/TradeSold**

Note (M1): the match is already exhaustive after A0.3 removed the wildcard. ADD
the new arms — do NOT include any wildcard-removal instruction.

```rust
// crates/jumpgate-core/src/trophic_run.rs — gossip_log_event_json.
// The matches are already exhaustive (A0.3 removed the wildcard).
// ADD arms for the new variants. The "buy" and "sell" rows encode
// craft.slot, station.slot, good.0, qty, price_micros (all i64 safe).
EventKind::TradeBought { craft, station, good, qty, price_micros } => Some(format!(
    r#"{{"e":"buy","t":{},"craft":{},"station":{},"good":{},"qty":{},"price_micros":{}}}"#,
    tick.0, craft.slot, station.slot, good.0, qty, price_micros
)),
EventKind::TradeSold { craft, station, good, qty, price_micros } => Some(format!(
    r#"{{"e":"sell","t":{},"craft":{},"station":{},"good":{},"qty":{},"price_micros":{}}}"#,
    tick.0, craft.slot, station.slot, good.0, qty, price_micros
)),
```

- [ ] **Step 5: run tests**

```
cargo test -p jumpgate-core economy_event_kinds_are_copy_and_partial_eq
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all pass. The new test proves Copy+PartialEq on `TradeBought` (Trade was
already removed in A0.3 — it is not touched here).

- [ ] **Step 6: commit (add TradeBought/TradeSold arms)**

```bash
git add crates/jumpgate-core/src/contract.rs \
        crates/jumpgate-core/src/trophic_run.rs \
        crates/jumpgate-core/src/economy.rs
git commit -F - <<'EOF'
feat(events): add TradeBought/TradeSold event variants and exhaustive arms

Chronicle arms and gossip-log arms land in the same commit (policy: no
silently-swallowed variants). Matches already exhaustive (A0.3 removed the
wildcard — ADD arms, do not remove again). TradeBought/TradeSold are unhashed.
EventKind::Trade was already removed in A0.3; this commit does not touch it.

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
    &self.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
    next,
);
```

- [ ] **Step 5: failing test — ASSIGN skips craft with non-empty hold (M5)**

Add this test to `crates/jumpgate-core/src/economy.rs` (or the ASSIGN unit test file):

```rust
// M5: ASSIGN empty-hold gate (L3-M3). A craft carrying own-trade goods in its
// hold must NOT receive a package contract — the two channels must stay separate.
#[test]
fn scripted_assign_skips_craft_with_nonempty_hold() {
    // Set up a world with one Offered contract and one craft with a non-empty hold.
    // After run_scripted_dispatch, the craft must NOT have accepted the contract.
    use crate::economy::{run_scripted_dispatch, Good};
    use crate::stores::{CraftRole, CraftStore};
    use crate::contract::{ContractStatus, ContractStore};
    // ... (use the run_scripted_dispatch test helper pattern from the REPOST test above)
    let mut contracts = ContractStore::empty();
    let mut ships = CraftStore::empty();
    // Push one craft with a non-empty hold (simulating an own-trade carry).
    {
        ships.ids.insert(());
        ships.role.push(CraftRole::Idle); // Idle = eligible for ASSIGN normally
        ships.contract.push(None);
        ships.credits_micros.push(10_000_000);
        // Non-empty hold: 5 units of Good(0).
        ships.hold.push(vec![(Good(0), 5u32)]);
        // ... populate remaining required CraftStore columns with safe defaults
    }
    // Push one Offered contract.
    let from_id = crate::ids::StationId { slot: 0, generation: 0 };
    let to_id = crate::ids::StationId { slot: 1, generation: 0 };
    let corp_id = crate::ids::CorporationId { slot: 0, generation: 0 };
    contracts.push(corp_id, crate::economy::Good::ORE, 5, from_id, to_id, 500_000);

    // ... (build minimal stations/bodies/dispatch cfg, run run_scripted_dispatch) ...
    // After dispatch: the craft's contract slot must remain None.
    assert!(
        ships.contract[0].is_none(),
        "ASSIGN must skip craft with non-empty hold (L3-M3: package contract requires empty hold)"
    );
    // The contract must still be Offered (not Accepted).
    assert_eq!(contracts.status[0], ContractStatus::Offered,
        "Offered contract must remain Offered when the only candidate has a non-empty hold");
}
```

Run: `cargo test -p jumpgate-core scripted_assign_skips_craft_with_nonempty_hold`
Expected: FAIL (hold check not yet implemented in ASSIGN arm).

- [ ] **Step 5b: implement the empty-hold guard in the ASSIGN arm**

In `crates/jumpgate-core/src/economy.rs`, in `run_scripted_dispatch`, in the ASSIGN
arm where candidate craft are evaluated, add the hold-empty gate:

```rust
// ASSIGN empty-hold gate (M5, panel L3-M3): a craft with non-empty hold is in
// own-trade mode (carrying bought goods). Assigning a package contract would
// mix channels — skip it.
if !ships.hold[crow].is_empty() {
    continue; // non-empty hold: craft is carrying own-trade goods
}
```

Add this guard before the `credits_micros` wallet check in the candidate evaluation
loop (the refuel-gate precedent: check structural predicates before monetary ones).

Run: `cargo test -p jumpgate-core scripted_assign_skips_craft_with_nonempty_hold`
Expected: PASS.

- [ ] **Step 5c: run tests + clippy**

```
cargo test -p jumpgate-core trade_policy_writes_buy_intent_for_capitalized_docked_craft
cargo test -p jumpgate-core trade_policy_skips_pirate
cargo test -p jumpgate-core trade_policy_skips_when_broke
cargo test -p jumpgate-core scripted_assign_skips_craft_with_nonempty_hold
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
(L2-C3), stock>0, wallet headroom including trade_reserve. ASSIGN empty-hold gate
added (L3-M3): craft carrying own-trade goods are skipped in the ASSIGN arm so the
package and own-trade channels stay separate. Exchange.active=false is the structural
inert gate; trophic/frontier behavior-identical.

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
  NOTE (C4): `assert_resource_identity` hold extension was landed in A2.2; A3.6 does NOT re-extend it.

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
        &world.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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
    assert_resource_identity(&world, &initial); // NOTE (C4): A2.2 version includes hold sum
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
            &world.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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
        &world.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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

    assert_resource_identity(&world, &initial); // NOTE (C4): A2.2 version includes hold sum
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
        &mut world.corporations, &world.config.exchange, &world.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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

// NOTE (C4): Step 4 (re-extend assert_resource_identity) has been DELETED.
// The hold sum extension was already landed in A2.2 with the Vec signature
// `fn assert_resource_identity(world: &World, initial: &[i64])`.
// A3.6 must NOT re-touch this function or reference N_RESOURCES.
// The A2.2 version already includes the hold sum — no re-extension here.

- [ ] **Step 4: wire into World::step at stage 1dx**

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
    &self.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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
    &self.config.goods,  // NOTE (C6): field is `goods`, not `goods_cfg`
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

- Modify: `crates/jumpgate-core/src/trophic_run.rs` (the Rust console printer) — add EXCHANGE line
- Modify: `python/analysis/sweep_trophic.py` — add EXCHANGE to ANCHORED regex map
  NOTE (MINOR-1): module path is `python/analysis/sweep_trophic.py`, NOT `python/jumpgate/sweep_trophic.py`
- Modify: `python/tests/test_sweep_parsing.py` — append V6 fixture using versioned-fixture pattern
  NOTE (MINOR-1): V6_STDOUT = V5_STDOUT + the new EXCHANGE line (additive); do not replace the V5 fixture

- [ ] **Step 1: failing test — EXCHANGE line is parsed by sweep_trophic**

```python
# python/tests/test_sweep_parsing.py — append a V6 fixture using the versioned-fixture
# pattern: V6_STDOUT = V5_STDOUT + the new EXCHANGE line (additive).
# NOTE (MINOR-1): module path is python/analysis/sweep_trophic.py, NOT python/jumpgate/.

# Build on the existing V5 fixture (add the EXCHANGE line):
V5_STDOUT = """\
META seed=1 ticks=1000 stations=10 haulers=12 pirates=3
VERDICT boom_bust
"""
V6_STDOUT = """\
META seed=1 ticks=1000 stations=10 haulers=12 pirates=3
EXCHANGE treasury_micros=5000000000 drain_per_100k=0
VERDICT boom_bust
"""

def test_exchange_line_is_parsed():
    from analysis.sweep_trophic import parse_run_output
    result = parse_run_output(V6_STDOUT)
    assert result["exchange_treasury_micros"] == 5_000_000_000
    assert result["exchange_drain_per_100k"] == 0

def test_v5_stdout_still_parses_without_exchange_line():
    # Backwards compatibility: V5 output (no EXCHANGE line) must still parse.
    from analysis.sweep_trophic import parse_run_output
    result = parse_run_output(V5_STDOUT)
    # exchange_treasury_micros absent or 0 (not a parse error).
    assert result.get("exchange_treasury_micros", 0) == 0
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_exchange_line_is_parsed`

Expected failure: `ModuleNotFoundError` or `KeyError: 'exchange_treasury_micros'` — the EXCHANGE regex is not yet in `python/analysis/sweep_trophic.py`.

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
# python/analysis/sweep_trophic.py — in the ANCHORED dict:
# NOTE (MINOR-1): file is python/analysis/sweep_trophic.py, not python/jumpgate/
"EXCHANGE": re.compile(
    r"EXCHANGE\s+treasury_micros=(?P<exchange_treasury_micros>-?\d+)"
    r"\s+drain_per_100k=(?P<exchange_drain_per_100k>-?\d+)"
),
```

- [ ] **Step 4: run tests**

```
PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_exchange_line_is_parsed
PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v5_stdout_still_parses_without_exchange_line
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: both Python tests pass; Rust tests unaffected.

- [ ] **Step 5: commit (lockstep: println + regex + fixture in same commit)**

```bash
git add crates/jumpgate-core/src/trophic_run.rs \
        python/analysis/sweep_trophic.py \
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
| A3.4 | TradeBought/TradeSold events + exhaustive match arms | Same commit: event variants + chronicle arms + gossip-log arms; matches already exhaustive (A0.3 removed wildcard, Trade deleted in A0.3 — NOT here) |
| A3.5 | run_trade_policies stage 1c3x | Pirates + !scripted skipped; sell-before-buy; price<1 guard; wallet headroom; Exchange.active inert gate |
| A3.6 | resolve_trade_buys + resolve_trade_sells stage 1dx | Stock↔hold TRANSFER (no consumed[]); money=wallet↔Exchange treasury; sell saturating; 6+4 skip arms; assert_resource_identity hold extension was landed in A2.2 (C4: no re-extension here) |
| A3.7 | Exchange drain instrument | EXCHANGE println + sweep regex + fixture in same commit (lockstep rule) |

## Phase A4 — Corp Arbitrage Replaces REPOST

# Phase A4 — Corp Arbitrage Replaces REPOST

> **Scope:** This phase implements the Exchange arbitrage poster (the replacement demand-generator that retires REPOST) and the extended withdrawal sweep. It covers: the `ArbitrageCfg` config struct landing in A3 (the `scan_interval > 0` inert gate); the arbitrage scan replacing the REPOST O(n²) body inside `run_scripted_dispatch` when `scan_interval > 0`; the full withdrawal sweep (Offered price-recheck + Accepted-never-loaded corp-solvency-recheck with escrow refund + hauler release); the structural REPOST disable in `scenario_bazaar` (`demand_low = demand_high = 0`); the `OfferWithdrawn` event + chronicle arm; and the Exchange corp battery sizing note and drain read (recorded, never gated).
>
> **Phase A4 depends on:** A3 landed (ArbitrageCfg, ExchangeCfg, GoodsCfg, and the per-good transport table in config; `scenario_bazaar` skeleton with `demand_low = demand_high = 0`).
>
> **Rung-B boundary (L5-C4):** no posture config, no greed config, no fence config, no `JetsamStore`, no jettison verb — all rung B.

---

### Task A4.1: `OfferWithdrawn` event and its chronicle arm

**What:** Add the `OfferWithdrawn { contract: ContractId, corp: CorporationId }` variant to `EventKind`. Add the `chronicle_subject` arm (returns the corp's craft row — see note below) and the `gossip_log_event_json` arm (returns `None`; withdrawn postings are not a gossip-log row). Replace the `_ => None` wildcard in `chronicle_subject` with an exhaustive match covering the new variant per the synthesis mandate. Add the `gossip_log_event_json` arm. Events are unhashed — no GOLDEN_STATE_HASH re-pin, no HASH_FORMAT_VERSION bump.

**Files:**
- Modify: `crates/jumpgate-core/src/contract.rs` — add `OfferWithdrawn` variant after `LurkMoved` (~line 201)
- Modify: `crates/jumpgate-bin/src/trophic_run.rs` — `chronicle_subject` (~line 481), `gossip_log_event_json` (~line 384)

---

- [ ] **Step 1: Failing test — `OfferWithdrawn` variant compiles and is Copy+PartialEq**

  ```rust
  // Add to economy_event_kinds_are_copy_and_partial_eq in contract.rs tests:
  #[test]
  fn offer_withdrawn_event_is_copy_and_partial_eq() {
      use crate::ids::CorporationId;
      let cid = ContractId { slot: 0, generation: 0 };
      let corp = CorporationId { slot: 1, generation: 0 };
      let ev = EventKind::OfferWithdrawn { contract: cid, corp };
      let ev_copy = ev;
      assert_eq!(ev, ev_copy);
      let prod = EventKind::Production {
          producer: crate::ids::ProducerId { slot: 0, generation: 0 },
          resource: Resource::Ore,
          qty: 1,
      };
      assert_ne!(ev, prod);
  }
  ```

  Run: `cargo test -p jumpgate-core offer_withdrawn_event_is_copy_and_partial_eq`
  Expected failure: `error[E0599]: no variant named OfferWithdrawn`

- [ ] **Step 2: Add `OfferWithdrawn` to `EventKind` in `contract.rs`**

  After `LurkMoved { pirate: CraftId, to_station: u32, breakout: bool },` (contract.rs ~line 200), add:

  ```rust
      // --- Goods-as-goods rung A events (hash-neutral like all events) ---
      /// An Exchange arbitrage posting was withdrawn — either the spread no longer
      /// clears at current prices (Offered recheck) or the corp cannot fund the
      /// pending buy after acceptance (Accepted-never-loaded solvency recheck).
      /// Emitted in stage 1b2 by `run_scripted_dispatch` / the withdrawal sweep.
      OfferWithdrawn {
          contract: ContractId,
          corp: CorporationId,
      },
  ```

  Also update `chronicle_subject` in `trophic_run.rs` to replace the `_ => None` arm with an exhaustive match. At line ~509, the function currently ends:

  ```rust
          _ => None,   // ← THE WILDCARD SWALLOW (line 509)
  ```

  Replace the entire match with an exhaustive one. The new arms needed:

  ```rust
          // Rung A (goods-as-goods): OfferWithdrawn has no craft subject — it is
          // a corp-level event; the chronicle skips it (returns None).
          EventKind::OfferWithdrawn { .. } => None,
          // NOTE: EventKind::Trade was deleted in A0.3 / A3.4. Do NOT add an arm for it.
  ```

  (The full exhaustive list must compile without `_ => None`. The builder should verify compilation replaces the wildcard entirely.)

  Also add to `gossip_log_event_json` (trophic_run.rs ~line 448), similarly replacing `_ => None` with an exhaustive match for all non-logged variants (including the new `OfferWithdrawn { .. } => None` arm).

  Run: `cargo test -p jumpgate-core offer_withdrawn_event_is_copy_and_partial_eq`
  Expected: PASS.
  Run: `cargo clippy --all-targets -- -D warnings`
  Expected: no warnings.

- [ ] **Step 3: Commit**

  ```bash
  git add crates/jumpgate-core/src/contract.rs \
          crates/jumpgate-bin/src/trophic_run.rs
  git commit -F - <<'EOF'
  feat(gag-a4): add OfferWithdrawn event + exhaustive chronicle match

  Adds EventKind::OfferWithdrawn{contract,corp} for the arbitrage
  withdrawal sweep (Offered price-recheck + Accepted-never-loaded
  solvency-recheck). Replaces the _ => None wildcard in
  chronicle_subject and gossip_log_event_json with exhaustive matches
  per synthesis mandate (policy reversal of the documented wildcard).
  Event is unhashed — no HASH_FORMAT_VERSION bump.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A4.2: `ArbitrageCfg` dependency confirmation + transport table

> **NOTE (C1): `ArbitrageCfg` was defined once in Task A3.2 as the canonical
> definition with the complete field set (C1 fix). This task does NOT redefine
> `ArbitrageCfg`. It is a dependency-confirmation task only: verify the A3.2
> struct is present and has the correct superset fields, then proceed to populate
> `scenario_bazaar`'s ArbitrageCfg with the transport table.**

**A4.2's sole job:** confirm A3.2 landed the complete `ArbitrageCfg` (including
`transport_micros: Vec<Vec<i64>>`, `qty_ladder`, `arb_premium_micros`), then wire
the `scenario_bazaar` factory (A5.2) to populate it with the PDR-0007 Hohmann-floor
table. No new struct definition. No GOLDEN_CONFIG_HASH re-pin (that was done in A3.2).

**Transport cost derivation (PDR-0007):** The transport table is a factory-time integer
precomputed from the ring-orbit geometry. For the frontier 10-station band, the per-hop
dv floor is derived from the Kepler delta-v between adjacent ring orbits. The spec
(synthesis Part 1.2) states "transport[route] is a FACTORY-TIME integer table from
phase-independent ring-radius geometry (the tier-reward precedent, scenario.rs:466-467)".
The concrete derivation: for a directed pair (from_body_au, to_body_au) use the impulsive
Hohmann approximation `dv ≈ √(GM/a_from)|1 - √(a_from/a_to)|` scaled to the cost
calibration `transport_micros[i][j] = (dv_norm * trade_base_micros).round() as i64`.
**The builder must compute this from FRONTIER_ORBIT_AU[i] entries at scenario factory
time; cite PDR-0007 in the doc comment.**

**Files:**
- Verify: `crates/jumpgate-core/src/config.rs` — confirm `ArbitrageCfg` has the A3.2
  canonical superset fields (transport_micros, qty_ladder, arb_premium_micros present)
- Modify: `crates/jumpgate-core/src/scenario.rs` — `scenario_trophic` and
  `scenario_frontier` get `ArbitrageCfg::default()` (inert, `scan_interval == 0`);
  `scenario_bazaar` populates the transport table (A5.2 task, cross-referenced here)

---

- [ ] **Step 1: Confirm `ArbitrageCfg` shape from A3.2**

  Run: `cargo test -p jumpgate-core goods_cfg_arb_cfg_exchange_cfg_are_config_hashed`
  Expected: PASS (struct was defined in A3.2).

  Verify the struct has all superset fields:
  ```bash
  grep -A 15 'pub struct ArbitrageCfg' crates/jumpgate-core/src/config.rs
  ```
  Expected: `transport_micros: Vec<Vec<i64>>`, `qty_ladder: Vec<u32>`,
  `arb_premium_micros: Vec<i64>` all present.

- [ ] **Step 2: Add `ArbitrageCfg::default()` to `scenario_trophic` and `scenario_frontier`**

  In `crates/jumpgate-core/src/scenario.rs`, wherever `scenario_trophic` and
  `scenario_frontier` construct `RunConfig`, confirm (or add) `arbitrage: ArbitrageCfg::default()`.
  The default has `scan_interval: 0` which is the structural inert gate — behavior-identical.

  Run: `cargo test --workspace`
  Expected: all pass; trophic/frontier behavior unchanged.

---

### Task A4.3: REPOST structural off — early-return prelude commit

**What:** The synthesis requires a hash-neutral early-return prelude so the O(n²) REPOST body stops executing over a growing contract board when `demand_low == demand_high == 0`. This commit proves behavior-identity by a within-build digest (stdout + JSONL + gossip-log sha256 at trophic seed 7 and frontier seed 7 before and after the prelude) — not a state-hash golden re-pin, because events are unhashed and the prelude is behavior-identical. The prelude is: if `dispatch.demand_low == 0 && dispatch.demand_high == 0 { /* skip REPOST body */ }`.

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` — add prelude before REPOST loop (~line 440)

---

- [ ] **Step 1: Failing test — REPOST early-return with demand 0/0 posts nothing**

  ```rust
  // In economy.rs tests, new test:
  #[test]
  fn repost_structural_off_posts_nothing_when_demand_zero() {
      // Build a minimal world with one route and some stock.
      // demand_low == demand_high == 0 → no burst ever fires.
      let mut contracts = ContractStore::empty();
      let corp = CorporationId { slot: 0, generation: 0 };
      let from = StationId { slot: 0, generation: 0 };
      let to = StationId { slot: 1, generation: 0 };
      // Seed one Completed contract on the route (the repost representative).
      let kid = contracts.ids.insert(());
      contracts.status.push(ContractStatus::Completed);
      contracts.corp.push(corp);
      contracts.resource.push(Resource::Ore);
      contracts.qty.push(5);
      contracts.from_station.push(from);
      contracts.to_station.push(to);
      contracts.reward_micros.push(1_000_000);
      contracts.escrow_micros.push(0);
      contracts.hauler.push(None);
      let n_before = contracts.ids.len();
      // Build minimal supporting stores (stock at 0 to trigger even a live dispatch).
      let mut stations = one_station([0, 0]);
      // Second station needed for `to` lookup.
      stations.ids.insert(());
      stations.stock.push([0, 0]);
      stations.price_micros.push([0, 0]);
      stations.body.push(BodyId { slot: 99, generation: 0 });
      let mut ships = CraftStore::empty();
      let mut diag = AssignDiag::default();
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          Tick(1), &mut events,
      );
      assert_eq!(contracts.ids.len(), n_before,
          "REPOST structural off: no new rows when demand_low==demand_high==0");
      assert!(events.is_empty(), "no ContractOffered events");
  }
  ```

  Run: `cargo test -p jumpgate-core repost_structural_off_posts_nothing_when_demand_zero`
  Expected: PASS already (the Schmitt trigger `stock < 0` never fires with non-negative stock, and `projected < max(0,0)` never fires with non-negative projected). The test documents the invariant.

  If this already passes, the prelude is a performance optimization, not a behavior change. Proceed to Step 2.

- [ ] **Step 2: Add REPOST early-return prelude in `economy.rs`**

  Before `let n = contracts.ids.len();` (economy.rs ~line 440), add:

  ```rust
      // REPOST structural off (goods-as-goods A4): when demand_low == demand_high == 0,
      // the Schmitt trigger can never fire (burst needs stock < 0; order-up-to needs
      // projected < 0; both are impossible with non-negative stock). The O(n²) route-key
      // scan would still execute over a growing arbitrage board — skip it explicitly.
      // Behavior-identical (proven by within-build digest on trophic+frontier seed 7).
      if dispatch.demand_low == 0 && dispatch.demand_high == 0 {
          // Skip the REPOST body; fall through to ASSIGN (stagger_period gate next).
          // NOTE: do NOT return here — ASSIGN still needs to run for any stagger_period > 0.
      } else {
      // (The entire REPOST body is wrapped in this else block.)
  ```

  Then close the `else` block after `}` on line ~518 (after `posts += 1;`).

  Run: `cargo test -p jumpgate-core repost_structural_off_posts_nothing_when_demand_zero`
  Expected: PASS.
  Run: `cargo test --workspace`
  Expected: all green. The REPOST body is now skipped for scenario_bazaar; trophic/frontier have `demand_low=10, demand_high=20` so they continue to run through the else branch unchanged.

- [ ] **Step 2b: within-build digest proof (MINOR-2 — execute the claimed proof)**

  The commit message claims "Proven by within-build digest on trophic+frontier seed 7."
  Execute this claim before committing:

  ```bash
  # BEFORE applying the prelude (on the HEAD before this change):
  cargo build -p jumpgate-core --release 2>/dev/null
  cargo run --release -p jumpgate-core --example trophic_run -- \
      --scenario trophic --seed 7 --ticks 50000 2>/dev/null \
      | sha256sum > /tmp/before_repost_trophic.sha256
  cargo run --release -p jumpgate-core --example trophic_run -- \
      --scenario frontier --seed 7 --ticks 50000 2>/dev/null \
      | sha256sum > /tmp/before_repost_frontier.sha256

  # AFTER applying the prelude (with this commit's changes):
  cargo build -p jumpgate-core --release 2>/dev/null
  cargo run --release -p jumpgate-core --example trophic_run -- \
      --scenario trophic --seed 7 --ticks 50000 2>/dev/null \
      | sha256sum > /tmp/after_repost_trophic.sha256
  cargo run --release -p jumpgate-core --example trophic_run -- \
      --scenario frontier --seed 7 --ticks 50000 2>/dev/null \
      | sha256sum > /tmp/after_repost_frontier.sha256

  # Compare:
  diff /tmp/before_repost_trophic.sha256 /tmp/after_repost_trophic.sha256
  diff /tmp/before_repost_frontier.sha256 /tmp/after_repost_frontier.sha256
  ```

  Expected: `diff` exits 0 (files identical) — the REPOST prelude is behavior-identical
  for trophic and frontier. If there is any divergence, STOP and bisect before committing.

- [ ] **Step 3: Commit**

  ```bash
  git add crates/jumpgate-core/src/economy.rs
  git commit -F - <<'EOF'
  perf(gag-a4): REPOST early-return prelude when demand_low==demand_high==0

  Adds an explicit early-exit for the REPOST body when both deadband
  edges are zero, preventing the O(n^2) route-key scan from iterating
  the growing arbitrage contract board. Behavior-identical: the Schmitt
  trigger (stock < demand_low, projected < max(high,low)) can never fire
  when both are 0. Proven by within-build digest on trophic+frontier
  seed 7 (no stdout/JSONL/gossip-log change). Hash-neutral (no state
  column touched). trophic (10/20) and frontier (10/20) are unaffected.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A4.4: Arbitrage poster in `run_scripted_dispatch`

**What:** When `dispatch.arbitrage.scan_interval > 0` AND `tick % arbitrage.scan_interval == 0`, run the O(n_routes) arbitrage scan as a replacement demand-generator in the REPOST slot. The scan: for each directed route (from_station × to_station per good), compute spread × qty − transport[route] − premium for the corp selected by `(route_index + scan_index) % n_corps`; if positive AND the corp has funding headroom (`treasury >= wage + price[from] * qty` after committed scratch), push one Offered contract via `ContractStore::push` and emit `ContractOffered`. Wage = `transport[route] + surplus × wage_share_milli / 1000`. `scan_index` is a stable tick-derived counter (increments by 1 per scan, wraps). `n_corps` is `corporations.ids.len()`. The scan binds `n = contracts.ids.len()` BEFORE pushing to avoid re-evaluating its own posts. Premium = `arb.qty_ladder[0]` is the unit for the smallest-first ladder.

**Critical details:**
- Route iteration is station-pair-major (outer: from_srow in 0..n_stations; inner: to_srow in 0..n_stations; skip from==to). Per-good loop inside. Route index = `from_srow * n_stations + to_srow`. This is O(n_stations² × n_goods) per scan tick.
- Corp rotation: `corp_for_route = (route_index + scan_index) % n_corps` where `route_index` is the loop iteration count and `scan_index = tick.0 / arb.scan_interval as u64`.
- qty ladder: use the smallest qty that fits (iterate ladder ascending). If none fits, skip the route.
- spread = `price[to][good] - price[from][good]` (integer micros). Must be > 0 for the trigger.
- transport = `arb.transport_micros.get(route_key).copied().unwrap_or(0)`. If table is empty or short, default to 0 (routes not in table are free — a degenerate config; inert gate via `scan_interval==0` prevents this in production).
- The unit_price guard: `if price[from][good] < 1 { continue; }` (L2-C3, cloned from resolve_refuels economy.rs:1023-1025).
- The `committed[corp_row]` scratch: declared as `Vec<i64>` of length `n_corps` before the scan, reset to 0 each scan tick. Each post adds `wage + price[from][good] * qty` to `committed[corp_row]`.
- Emit `ContractOffered` only (no `PackagePosted` — dropped per synthesis conflict resolution; the "post" gossip-log row is runner-enriched from ContractOffered + current prices at read time).

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` — extend `run_scripted_dispatch` signature with `arbitrage: &ArbitrageCfg, corporations: &CorporationStore` and add the scan body
- Modify: `crates/jumpgate-core/src/world.rs` — update the `run_scripted_dispatch` call site to pass the new args

---

- [ ] **Step 1: Failing test — arbitrage poster posts when spread clears transport**

  ```rust
  // In economy.rs tests:
  #[test]
  fn arbitrage_poster_posts_when_spread_clears_transport() {
      // Two stations: from (high ore stock, low price) → to (low ore stock, high price).
      // spread * qty > transport → one Offered row must appear.
      let mut stations = StationStore::empty();
      // Station 0 (from): ore price_micros = 100_000; fuel = 0.
      {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([100_000i64, 0i64]);
          stations.body.push(BodyId { slot: 0, generation: 0 });
      }
      // Station 1 (to): ore price = 500_000 (high); fuel = 0.
      {
          stations.ids.insert(());
          stations.stock.push([0, 0]);
          stations.price_micros.push([500_000i64, 0i64]);
          stations.body.push(BodyId { slot: 1, generation: 0 });
      }
      let mut corporations = CorporationStore::empty();
      {
          // One corp with ample treasury.
          corporations.ids.insert(());
          corporations.treasury_micros.push(10_000_000_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let corp_id = CorporationId { corporations.ids.id_at(0).map(|(s,g)| CorporationId{slot:s,generation:g}).unwrap() };
      // Actually use the id_at API:
      let (cs, cg) = corporations.ids.id_at(0).unwrap();
      let corp_id = CorporationId { slot: cs, generation: cg };

      let arb = ArbitrageCfg {
          scan_interval: 1,
          // transport = 50_000 micros for route 0→1.
          transport_micros: vec![0, 50_000, 50_000, 0], // 2x2 table
          wage_share_milli: 200, // 20% of surplus
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let mut contracts = ContractStore::empty();
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut ships = CraftStore::empty();
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();

      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations,
          Tick(1), &mut events,
      );

      // spread = 500_000 - 100_000 = 400_000; transport = 50_000;
      // surplus = 400_000 * 5 - 50_000 = 1_950_000 > 0 → one post.
      assert_eq!(contracts.ids.len(), 1, "one Offered row posted");
      assert_eq!(contracts.resource[0], Resource::Ore);
      assert_eq!(contracts.qty[0], 5);
      assert_eq!(contracts.from_station[0].slot, 0);
      assert_eq!(contracts.to_station[0].slot, 1);
      // wage = transport + surplus * wage_share_milli / 1000
      // surplus_unit = spread * qty - transport = 400_000 * 5 - 50_000 = 1_950_000
      let expected_wage = 50_000i64 + 1_950_000 * 200 / 1000;
      assert_eq!(contracts.reward_micros[0], expected_wage);
      // Check ContractOffered event emitted.
      let offered_count = events.iter().filter(|e| matches!(e.kind, EventKind::ContractOffered { .. })).count();
      assert_eq!(offered_count, 1, "one ContractOffered event");
  }
  ```

  Run: `cargo test -p jumpgate-core arbitrage_poster_posts_when_spread_clears_transport`
  Expected failure: signature mismatch (new args not yet added).

- [ ] **Step 2: Failing test — arbitrage poster skips when spread does not clear**

  ```rust
  #[test]
  fn arbitrage_poster_skips_when_spread_below_transport() {
      // spread * qty < transport → no post.
      let mut stations = StationStore::empty();
      {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([490_000i64, 0i64]); // from price high (narrow spread)
          stations.body.push(BodyId { slot: 0, generation: 0 });
      }
      {
          stations.ids.insert(());
          stations.stock.push([0, 0]);
          stations.price_micros.push([500_000i64, 0i64]); // to price
          stations.body.push(BodyId { slot: 1, generation: 0 });
      }
      let mut corporations = CorporationStore::empty();
      {
          corporations.ids.insert(());
          corporations.treasury_micros.push(10_000_000_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let arb = ArbitrageCfg {
          scan_interval: 1,
          transport_micros: vec![0, 500_000, 500_000, 0], // transport > spread*qty
          wage_share_milli: 200,
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let mut contracts = ContractStore::empty();
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut ships = CraftStore::empty();
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();

      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations,
          Tick(1), &mut events,
      );

      assert_eq!(contracts.ids.len(), 0, "no post when spread < transport");
  }
  ```

  Run: `cargo test -p jumpgate-core arbitrage_poster_skips_when_spread_below_transport`
  Expected failure: same compile error (args not present yet).

- [ ] **Step 3: Failing test — corp rotation assigns first refusal correctly**

  ```rust
  #[test]
  fn arbitrage_corp_rotation_assigns_first_refusal() {
      // Two corps, two routes. Corp rotation: route_0 → corp (0+scan_index)%2,
      // route_1 → corp (1+scan_index)%2. At scan_index=0, route_0→corp_0, route_1→corp_1.
      let mut stations = StationStore::empty();
      for price in [100_000i64, 500_000i64, 100_000i64, 500_000i64] {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([price, 0]);
          stations.body.push(BodyId { slot: stations.ids.len() as u32 - 1, generation: 0 });
      }
      let mut corporations = CorporationStore::empty();
      for _ in 0..2 {
          corporations.ids.insert(());
          corporations.treasury_micros.push(10_000_000_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let (cs0, cg0) = corporations.ids.id_at(0).unwrap();
      let (cs1, cg1) = corporations.ids.id_at(1).unwrap();
      let corp0 = CorporationId { slot: cs0, generation: cg0 };
      let corp1 = CorporationId { slot: cs1, generation: cg1 };
      let arb = ArbitrageCfg {
          scan_interval: 1,
          // 4-station table: routes 0→1, 0→2, 0→3, etc. Use a simple 50_000 transport.
          transport_micros: vec![0i64; 16], // 4x4 all-zero transport (routes will clear)
          wage_share_milli: 0,
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let mut contracts = ContractStore::empty();
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut ships = CraftStore::empty();
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();

      // Tick 1 → scan_index = 1/1 = 1.
      // Route index 0 (0→1): corp = (0+1)%2 = 1.
      // Route index 1 (0→2): corp = (1+1)%2 = 0.
      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations,
          Tick(1), &mut events,
      );

      // Verify at least the first posted route has corp1 assigned (route_index=0, scan_index=1).
      // Exact route depends on station-pair iteration order (from 0, to 1 = route_index 0).
      let first_post = contracts.corp.first().copied();
      assert_eq!(
          first_post, Some(corp1),
          "route_index=0 scan_index=1 → corp (0+1)%2=1"
      );
  }
  ```

  Run: `cargo test -p jumpgate-core arbitrage_corp_rotation_assigns_first_refusal`
  Expected failure: compile error.

- [ ] **Step 4: Implement the arbitrage poster in `run_scripted_dispatch`**

  Update function signature in `economy.rs`:

  ```rust
  #[allow(clippy::too_many_arguments)]
  pub fn run_scripted_dispatch(
      contracts: &mut ContractStore,
      stations: &StationStore,
      ships: &mut CraftStore,
      craft_cfg: &[crate::config::CraftInit],
      route_evidence: &crate::world::RouteEvidence,
      media_live: bool,
      staleness_from_rob_tick: bool,
      diag: &mut AssignDiag,
      dispatch: &crate::config::DispatchCfg,
      shipyard: &crate::config::ShipyardCfg,
      trophic: &crate::config::TrophicCfg,
      arbitrage: &crate::config::ArbitrageCfg,
      corporations: &CorporationStore,
      tick: Tick,
      events: &mut EventStream,
  ) {
  ```

  After the REPOST prelude (the `if demand_low==0 && demand_high==0 { } else { ... }` block added in A4.3), add the arbitrage scan. The full arbitrage body, placed between the REPOST block and the ASSIGN gate:

  ```rust
      // ARBITRAGE POSTER (goods-as-goods A4) — Exchange corp posts a sealed package
      // when spread × qty − transport > 0 AND the corp has funding headroom.
      // Inert gate: scan_interval == 0 skips entirely (structural off for
      // trophic/frontier). Rate gate: scan only on the configured interval tick.
      if arbitrage.scan_interval > 0
          && tick.0 % arbitrage.scan_interval as u64 == 0
          && !arbitrage.qty_ladder.is_empty()
          && !corporations.ids.is_empty()
      {
          let n_stations = stations.ids.len();
          let n_corps = corporations.ids.len();
          // scan_index increments each scan period (PDR-0007 corp-rotation).
          let scan_index = tick.0 / arbitrage.scan_interval as u64;
          // Bind row count BEFORE pushing so fresh rows are not re-evaluated this tick.
          let _n_pre = contracts.ids.len();
          // per-corp committed scratch: prevents over-posting against a single treasury.
          let mut committed: Vec<i64> = vec![0i64; n_corps];
          let mut posts_this_scan: usize = 0;
          let mut route_index: u64 = 0;
          'route: for from_srow in 0..n_stations {
              if stations.ids.id_at(from_srow).is_none() {
                  route_index += n_stations as u64;
                  continue;
              }
              let (from_slot, from_gen) = stations.ids.id_at(from_srow).unwrap();
              let from_id = StationId { slot: from_slot, generation: from_gen };
              for to_srow in 0..n_stations {
                  if to_srow == from_srow {
                      route_index += 1;
                      continue;
                  }
                  if posts_this_scan >= arbitrage.max_posts_per_scan {
                      break 'route;
                  }
                  if stations.ids.id_at(to_srow).is_none() {
                      route_index += 1;
                      continue;
                  }
                  let (to_slot, to_gen) = stations.ids.id_at(to_srow).unwrap();
                  let to_id = StationId { slot: to_slot, generation: to_gen };
                  let route_key = from_srow * n_stations + to_srow;
                  let transport = arbitrage.transport_micros
                      .get(route_key)
                      .copied()
                      .unwrap_or(0);
                  // Corp rotation (L1-C2): deterministic first-refusal.
                  let corp_row = ((route_index + scan_index) % n_corps as u64) as usize;
                  let Some((cs, cg)) = corporations.ids.id_at(corp_row) else {
                      route_index += 1;
                      continue;
                  };
                  let corp_id = CorporationId { slot: cs, generation: cg };
                  let treasury = corporations.treasury_micros[corp_row];
                  // Per-good scan on this route.
                  let n_goods = stations.price_micros[from_srow].len()
                      .min(stations.price_micros[to_srow].len());
                  for good_idx in 0..n_goods {
                      // L2-C3: price < 1 guard (cloned from resolve_refuels:1023-1025).
                      let price_from = stations.price_micros[from_srow][good_idx];
                      if price_from < 1 {
                          continue;
                      }
                      let price_to = stations.price_micros[to_srow][good_idx];
                      if price_to < 1 {
                          continue;
                      }
                      let spread = price_to.saturating_sub(price_from);
                      if spread <= 0 {
                          continue;
                      }
                      // Smallest-first qty ladder.
                      let Some(&qty) = arbitrage.qty_ladder.first() else { continue; };
                      // spread * qty − transport > 0?
                      let gross = spread.saturating_mul(qty as i64);
                      let surplus = gross.saturating_sub(transport);
                      if surplus <= 0 {
                          continue;
                      }
                      // Wage = transport floor + share of surplus (OD-4a).
                      let wage = transport.saturating_add(
                          surplus.saturating_mul(arbitrage.wage_share_milli as i64) / 1000,
                      );
                      // Funding headroom: treasury − committed >= wage + price_from * qty.
                      let buy_cost = price_from.saturating_mul(qty as i64);
                      let need = wage.saturating_add(buy_cost);
                      let available = treasury.saturating_sub(committed[corp_row]);
                      if available < need {
                          continue;
                      }
                      // Post.
                      committed[corp_row] = committed[corp_row].saturating_add(need);
                      // Good index → Good(u16) (after A1 refactor: Good(good_idx as u16)).
                      // For now (pre-A1) use Resource::from_index or the equivalent.
                      // The builder will adjust this to use Good(good_idx as u16) after A1.
                      let resource_or_good = {
                          // Post-A1 this becomes: Good(good_idx as u16)
                          // Pre-A1 this is a Resource; the type is whatever ContractStore.resource uses.
                          // Use Resource::from_index is not a real API — use the known order.
                          // Since A1 refactors Resource→Good, this line is a PLACEHOLDER
                          // that the builder MUST align with the actual type after A1 lands.
                          // If A1 is not yet landed: Resource::Ore (good_idx==0) / Resource::Fuel (1).
                          // Builder instruction: replace this block with Good(good_idx as u16) after A1.
                          if good_idx == 0 { Resource::Ore } else { Resource::Fuel }
                      };
                      let new_id = contracts.push(
                          corp_id,
                          resource_or_good,
                          qty,
                          from_id,
                          to_id,
                          wage,
                      );
                      events.emit(Event {
                          tick,
                          kind: EventKind::ContractOffered { contract: new_id },
                      });
                      posts_this_scan += 1;
                  }
                  route_index += 1;
              }
          }
      }
  ```

  Update `world.rs` call site: find the `run_scripted_dispatch(` call and add `&cfg.arbitrage, &world.corporations,` in the correct argument positions.

  Run: `cargo test -p jumpgate-core arbitrage_poster_posts_when_spread_clears_transport arbitrage_poster_skips_when_spread_below_transport arbitrage_corp_rotation_assigns_first_refusal`
  Expected: all three PASS.
  Run: `cargo test --workspace`
  Expected: all green.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/jumpgate-core/src/economy.rs \
          crates/jumpgate-core/src/world.rs
  git commit -F - <<'EOF'
  feat(gag-a4): arbitrage poster in run_scripted_dispatch (stage 1b2)

  Implements the Exchange arbitrage scan: when scan_interval > 0 and
  tick is a scan tick, iterates directed station pairs per good. Posts a
  ContractOffered row when spread*qty - transport[route] > 0 AND the
  selected corp has funding headroom (committed scratch per corp). Corp
  rotation (route_index + scan_index) % n_corps gives first refusal
  (L1-C2). Wage = transport + surplus * wage_share_milli / 1000 (OD-4a).
  L2-C3 price<1 guards cloned from resolve_refuels:1023-1025. Inert
  when scan_interval==0 (trophic, frontier, default). Emits
  ContractOffered only (PackagePosted dropped per synthesis conflict).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A4.5: Withdrawal sweep — Offered price-recheck and Accepted-never-loaded corp solvency

**What:** Extend the stage-1b2 scan (the same function body after the arbitrage poster) to run two withdrawal passes each scan tick:

**(a) Offered recheck (L1-C1a):** For Offered rows whose corp matches the Exchange, recompute `spread × qty − transport − premium ≤ 0` at current prices. If the spread has collapsed, fail the row: set `status = Failed`, emit `OfferWithdrawn`. No escrow motion (posting is free; escrow is 0 at Offered). No hauler to release (hauler is None at Offered, or if intent-claimed by a craft that hasn't yet been resolved, the craft's intent will be cleared in the Offered→Failed path — see note below).

**(b) Accepted-never-loaded recheck (L1-C1b):** For Accepted rows where `contracts.hauler[kidx].is_some()` AND the corp cannot fund the buy: `corporations.treasury_micros[corp_row] < buy_price_at_load` (= `price[from][good] * qty`). Execute the `settle_contract_failure` legs MINUS the cargo leg: escrow refund → corp treasury, release hauler (contract/role → None/Idle), status → Failed. Emit `OfferWithdrawn` (not `ContractFailed`, which is `FuelEmpty`-only). This prevents the permanent fleet attrition documented in the grounding extract §11(b).

**Note on Offered-with-craft-intent:** If a craft holds accept-intent (`ships.contract[r] == Some(cid)`) and the Offered row is withdrawn before `resolve_contracts` runs, the craft's intent is stale. The withdrawal sweep must also clear the craft-side intent: iterate `ships.contract` and set any matching slot to `None`, `ships.role` back to `Idle`. This is the mirror of the REVERT path in `resolve_contracts` (economy.rs:718-721).

**Files (M4: explicitly inventoried):**
- Modify: `crates/jumpgate-core/src/economy.rs` — add withdrawal sweep in `run_scripted_dispatch`, update signature to include `ships: &mut CraftStore` for the Offered craft-intent clearing
- Modify: `crates/jumpgate-core/src/world.rs` — **stage-1b2 call site** (the primary call site that gains the new `ships` arg); search with `grep -n 'run_scripted_dispatch(' crates/jumpgate-core/src/world.rs` to locate it
- Grep step (M4): before writing, enumerate ALL direct `run_scripted_dispatch(` callers:
  ```bash
  grep -rn 'run_scripted_dispatch(' crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs
  ```
  Any callers in test modules (`#[cfg(test)]` blocks) within `economy.rs` or `world.rs` are also call sites that must be updated in the SAME commit when the signature gains `ships: &mut CraftStore`. Do NOT land the signature change and leave any call site on the old signature — that is a compile error.

---

- [ ] **Step 1: Failing test — Offered price-recheck withdraws collapsed-spread rows**

  ```rust
  #[test]
  fn withdrawal_sweep_offered_recheck_clears_stale_post() {
      // An Offered row posted by the Exchange corp. After prices move so spread
      // < transport, the withdrawal sweep must mark it Failed and emit OfferWithdrawn.
      let mut contracts = ContractStore::empty();
      let mut corporations = CorporationStore::empty();
      {
          corporations.ids.insert(());
          corporations.treasury_micros.push(1_000_000_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let (cs, cg) = corporations.ids.id_at(0).unwrap();
      let corp_id = CorporationId { slot: cs, generation: cg };
      // Push an Offered row.
      let from_id = StationId { slot: 0, generation: 0 };
      let to_id = StationId { slot: 1, generation: 0 };
      let orig_wage = 400_000i64;
      let kid_idx = {
          let cid = contracts.push(corp_id, Resource::Ore, 5, from_id, to_id, orig_wage);
          contracts.ids.dense_index(cid.slot, cid.generation).unwrap()
      };
      assert_eq!(contracts.status[kid_idx], ContractStatus::Offered);
      // Stations: spread has now collapsed (from price ≈ to price).
      let mut stations = StationStore::empty();
      for p in [499_000i64, 500_000i64] {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([p, 0]);
          stations.body.push(BodyId { slot: stations.ids.len() as u32 - 1, generation: 0 });
      }
      // transport_micros for route 0→1 = 100_000 (well above the 1_000 spread * 5 = 5_000).
      let arb = ArbitrageCfg {
          scan_interval: 1,
          transport_micros: vec![0, 100_000, 100_000, 0],
          wage_share_milli: 200,
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut ships = CraftStore::empty();
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();

      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations, Tick(1), &mut events,
      );

      assert_eq!(contracts.status[kid_idx], ContractStatus::Failed,
          "Offered row with collapsed spread must be Failed");
      assert_eq!(contracts.escrow_micros[kid_idx], 0,
          "escrow stays 0 (posting was free)");
      let withdrawn = events.iter().any(|e| matches!(e.kind, EventKind::OfferWithdrawn { .. }));
      assert!(withdrawn, "OfferWithdrawn must be emitted");
  }
  ```

  Run: `cargo test -p jumpgate-core withdrawal_sweep_offered_recheck_clears_stale_post`
  Expected failure: compile error (no withdrawal sweep yet).

- [ ] **Step 2: Failing test — Accepted-never-loaded solvency recheck releases fleet**

  ```rust
  #[test]
  fn withdrawal_sweep_accepted_never_loaded_releases_hauler() {
      // An Accepted row where the corp treasury has drained below buy_price * qty.
      // The sweep must: escrow refund → corp treasury, release hauler, status = Failed.
      let mut contracts = ContractStore::empty();
      let mut corporations = CorporationStore::empty();
      {
          corporations.ids.insert(());
          // Treasury BELOW buy price for the load (100_000 * 5 = 500_000 needed).
          corporations.treasury_micros.push(100_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let (cs, cg) = corporations.ids.id_at(0).unwrap();
      let corp_id = CorporationId { slot: cs, generation: cg };

      // Hauler craft.
      let mut ships = CraftStore::empty();
      {
          ships.ids.insert(());
          ships.role.push(CraftRole::Hauler);
          ships.contract.push(None); // will be set below
          // ... (populate all required CraftStore columns with safe defaults)
      }
      let (hs, hg) = ships.ids.id_at(0).unwrap();
      let hauler_id = CraftId { slot: hs, generation: hg };

      // Push an Accepted contract.
      let from_id = StationId { slot: 0, generation: 0 };
      let to_id = StationId { slot: 1, generation: 0 };
      let escrow = 800_000i64;
      let kid_idx = {
          let cid = contracts.push(corp_id, Resource::Ore, 5, from_id, to_id, escrow);
          let ki = contracts.ids.dense_index(cid.slot, cid.generation).unwrap();
          // Manually transition to Accepted with escrow (simulating resolve_contracts accept).
          contracts.status[ki] = ContractStatus::Accepted;
          contracts.hauler[ki] = Some(hauler_id);
          contracts.escrow_micros[ki] = escrow;
          corporations.treasury_micros[0] -= escrow; // escrow was debited at accept
          ships.contract[0] = Some(cid);
          ki
      };
      // Corp treasury is now 100_000 - 800_000 = -700_000? No — treasury was 100_000 total.
      // Let's make it cleaner: treasury = 800_000 (just enough to escrow), drain to 50_000.
      // Reset: treasury = 50_000 after escrow was already debited.
      corporations.treasury_micros[0] = 50_000i64; // drained below buy_cost = 500_000.

      // Stations: from price = 100_000, qty = 5 → buy_cost = 500_000 > treasury 50_000.
      let mut stations = StationStore::empty();
      for p in [100_000i64, 800_000i64] {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([p, 0]);
          stations.body.push(BodyId { slot: stations.ids.len() as u32 - 1, generation: 0 });
      }
      let arb = ArbitrageCfg {
          scan_interval: 1,
          transport_micros: vec![0, 50_000, 50_000, 0],
          wage_share_milli: 200,
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();
      let initial_credit = corporations.treasury_micros[0]
          + ships.credits_micros.iter().sum::<i64>()
          + contracts.escrow_micros.iter().sum::<i64>();

      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations, Tick(1), &mut events,
      );

      assert_eq!(contracts.status[kid_idx], ContractStatus::Failed,
          "Accepted-never-loaded with insolvent corp must be Failed");
      assert_eq!(ships.role[0], CraftRole::Idle, "hauler must be released");
      assert!(ships.contract[0].is_none(), "hauler contract cleared");
      assert_eq!(contracts.escrow_micros[kid_idx], 0, "escrow refunded");
      // Credit identity: escrow was refunded to corp treasury.
      let final_credit = corporations.treasury_micros[0]
          + ships.credits_micros.iter().sum::<i64>()
          + contracts.escrow_micros.iter().sum::<i64>();
      assert_eq!(final_credit, initial_credit + escrow,
          "escrow refund preserves credit identity (escrow → treasury)");
      let withdrawn = events.iter().any(|e| matches!(e.kind, EventKind::OfferWithdrawn { .. }));
      assert!(withdrawn, "OfferWithdrawn emitted for insolvent accepted row");
  }
  ```

  Run: `cargo test -p jumpgate-core withdrawal_sweep_accepted_never_loaded_releases_hauler`
  Expected failure: compile error.

- [ ] **Step 3: Implement withdrawal sweep in `run_scripted_dispatch`**

  At the END of the arbitrage scan block (after the `}` closing `if arbitrage.scan_interval > 0 ...`), add the two withdrawal passes. These also run only when `scan_interval > 0` (the inert gate — the withdrawal sweep is part of the arbitrage machinery):

  ```rust
      // WITHDRAWAL SWEEP (L1-C1): two passes, both gated on scan_interval > 0.
      // (a) Offered price-recheck — collapse-spread rows fail and clear craft intents.
      // (b) Accepted-never-loaded solvency recheck — insolvent corp rows: escrow refund,
      //     release hauler.
      if arbitrage.scan_interval > 0
          && tick.0 % arbitrage.scan_interval as u64 == 0
      {
          let n_stations = stations.ids.len();
          // Pass (a): Offered rows whose spread no longer clears transport.
          for kidx in 0..contracts.ids.len() {
              if contracts.status[kidx] != ContractStatus::Offered {
                  continue;
              }
              // Only sweep Exchange-posted rows (hauler is None, escrow is 0).
              // We identify them as rows with no cargo and no hauler.
              if contracts.hauler[kidx].is_some() {
                  continue; // craft has intent-claimed but not yet resolved; skip.
              }
              let from = contracts.from_station[kidx];
              let to = contracts.to_station[kidx];
              let Some(from_srow) = stations.ids.dense_index(from.slot, from.generation) else {
                  // Dead station — withdraw the stale row.
                  contracts.status[kidx] = ContractStatus::Failed;
                  let cid = contract_id(contracts, kidx);
                  let corp = contracts.corp[kidx];
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
                  continue;
              };
              let Some(to_srow) = stations.ids.dense_index(to.slot, to.generation) else {
                  contracts.status[kidx] = ContractStatus::Failed;
                  let cid = contract_id(contracts, kidx);
                  let corp = contracts.corp[kidx];
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
                  continue;
              };
              let route_key = from_srow * n_stations + to_srow;
              let transport = arbitrage.transport_micros.get(route_key).copied().unwrap_or(0);
              // Determine good index from contract resource.
              // Post-A1: contracts.resource[kidx] is Good(u16); good_idx = good.0 as usize.
              // Pre-A1: resource.index() gives the column.
              let good_idx = contracts.resource[kidx].index();
              let price_from = stations.price_micros
                  .get(from_srow)
                  .and_then(|p| p.get(good_idx))
                  .copied()
                  .unwrap_or(0);
              let price_to = stations.price_micros
                  .get(to_srow)
                  .and_then(|p| p.get(good_idx))
                  .copied()
                  .unwrap_or(0);
              // L2-C3: price < 1 guard.
              if price_from < 1 || price_to < 1 {
                  contracts.status[kidx] = ContractStatus::Failed;
                  let cid = contract_id(contracts, kidx);
                  let corp = contracts.corp[kidx];
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
                  continue;
              }
              let spread = price_to.saturating_sub(price_from);
              let qty = contracts.qty[kidx];
              if qty == 0 {
                  contracts.status[kidx] = ContractStatus::Failed;
                  let cid = contract_id(contracts, kidx);
                  let corp = contracts.corp[kidx];
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
                  continue;
              }
              let gross = spread.saturating_mul(qty as i64);
              let surplus = gross.saturating_sub(transport);
              if surplus <= 0 {
                  // Spread collapsed — withdraw.
                  contracts.status[kidx] = ContractStatus::Failed;
                  // Clear any craft-side accept intent (the resolve_contracts REVERT mirror).
                  let cid = contract_id(contracts, kidx);
                  for crow in 0..ships.ids.len() {
                      if ships.contract[crow] == Some(cid) {
                          ships.contract[crow] = None;
                          ships.role[crow] = CraftRole::Idle;
                      }
                  }
                  let corp = contracts.corp[kidx];
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
              }
          }
          // Pass (b): Accepted-never-loaded rows — corp solvency check.
          for kidx in 0..contracts.ids.len() {
              if contracts.status[kidx] != ContractStatus::Accepted {
                  continue;
              }
              // Only rows with no cargo (ship.cargo is None = never loaded).
              // We identify "never loaded" as status == Accepted (CargoLoaded means loaded).
              let Some(hauler_id) = contracts.hauler[kidx] else { continue; };
              let Some(crow) = ships.index_of(hauler_id) else { continue; };
              // Check the craft has no cargo (definitely not loaded).
              if ships.cargo[crow].is_some() {
                  // Should not happen (cargo → CargoLoaded status), but be safe.
                  continue;
              }
              let from = contracts.from_station[kidx];
              let Some(from_srow) = stations.ids.dense_index(from.slot, from.generation) else {
                  continue;
              };
              let good_idx = contracts.resource[kidx].index();
              let price_from = stations.price_micros
                  .get(from_srow)
                  .and_then(|p| p.get(good_idx))
                  .copied()
                  .unwrap_or(0);
              if price_from < 1 {
                  continue; // price gate (L2-C3)
              }
              let qty = contracts.qty[kidx];
              let buy_cost = price_from.saturating_mul(qty as i64);
              // Corp solvency check.
              let corp = contracts.corp[kidx];
              let Some(corp_row) = corporations.ids.dense_index(corp.slot, corp.generation) else {
                  // Stale corp row: still refund escrow into the void and release hauler.
                  contracts.escrow_micros[kidx] = 0;
                  ships.contract[crow] = None;
                  ships.role[crow] = CraftRole::Idle;
                  contracts.hauler[kidx] = None;
                  contracts.status[kidx] = ContractStatus::Failed;
                  let cid = contract_id(contracts, kidx);
                  events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
                  continue;
              };
              if corporations.treasury_micros[corp_row] >= buy_cost {
                  continue; // Corp can still fund; leave Accepted.
              }
              // Insolvent: escrow refund + hauler release (no cargo leg — never loaded).
              let refund = contracts.escrow_micros[kidx];
              corporations.treasury_micros[corp_row] =
                  corporations.treasury_micros[corp_row].saturating_add(refund);
              contracts.escrow_micros[kidx] = 0;
              ships.contract[crow] = None;
              ships.role[crow] = CraftRole::Idle;
              contracts.hauler[kidx] = None;
              contracts.status[kidx] = ContractStatus::Failed;
              let cid = contract_id(contracts, kidx);
              events.emit(Event { tick, kind: EventKind::OfferWithdrawn { contract: cid, corp } });
          }
      }
  ```

  Run: `cargo test -p jumpgate-core withdrawal_sweep_offered_recheck_clears_stale_post withdrawal_sweep_accepted_never_loaded_releases_hauler`
  Expected: both PASS.

- [ ] **Step 4: No-zombie test — claimed-then-insolvent leaves no permanent fleet attrition**

  ```rust
  #[test]
  fn no_zombie_claimed_then_insolvent_corp_releases_fleet() {
      // An Accepted row (corp treasury drained), withdrawal sweep fires.
      // After the sweep: hauler is Idle, escrow is 0, contract is Failed.
      // The credit identity must be preserved.
      let mut contracts = ContractStore::empty();
      let mut corporations = CorporationStore::empty();
      let escrow_amount = 500_000i64;
      {
          corporations.ids.insert(());
          // Treasury after escrow was debited at accept: well below buy_cost.
          corporations.treasury_micros.push(1_000i64);
          corporations.home_station.push(StationId { slot: 0, generation: 0 });
      }
      let (cs, cg) = corporations.ids.id_at(0).unwrap();
      let corp_id = CorporationId { slot: cs, generation: cg };
      let mut ships = CraftStore::empty();
      // Build a minimal hauler row (all required columns must be pushed).
      {
          ships.ids.insert(());
          ships.role.push(CraftRole::Hauler);
          ships.contract.push(None);
          ships.cargo.push(None);
          ships.credits_micros.push(0);
          ships.fuel_mass.push(1.0e-9);
          ships.pos.push(crate::math::Vec3::zero());
          ships.vel.push(crate::math::Vec3::zero());
          ships.nav.push(crate::stores::NavState::Coast);
          ships.spec.push(crate::config::BaseSpec::default());
          ships.upgrades.push(crate::stores::UpgradeLevels::default());
          ships.info_tick.push(crate::time::Tick(0));
          ships.pending_upgrade.push(None);
          ships.pending_refuel.push(None);
          ships.gossip.push(None);
      }
      let (hs, hg) = ships.ids.id_at(0).unwrap();
      let hauler_id = CraftId { slot: hs, generation: hg };
      // Accepted contract row.
      let from_id = StationId { slot: 0, generation: 0 };
      let to_id = StationId { slot: 1, generation: 0 };
      let kid_idx = {
          let cid = contracts.push(corp_id, Resource::Ore, 5, from_id, to_id, 800_000);
          let ki = contracts.ids.dense_index(cid.slot, cid.generation).unwrap();
          contracts.status[ki] = ContractStatus::Accepted;
          contracts.hauler[ki] = Some(hauler_id);
          contracts.escrow_micros[ki] = escrow_amount;
          ships.contract[0] = Some(cid);
          ki
      };
      let mut stations = StationStore::empty();
      for p in [100_000i64, 800_000i64] {
          stations.ids.insert(());
          stations.stock.push([50, 0]);
          stations.price_micros.push([p, 0]);
          stations.body.push(BodyId { slot: stations.ids.len() as u32 - 1, generation: 0 });
      }
      // buy_cost = 100_000 * 5 = 500_000 > treasury 1_000.
      let arb = ArbitrageCfg {
          scan_interval: 1,
          transport_micros: vec![0, 50_000, 50_000, 0],
          wage_share_milli: 200,
          qty_ladder: vec![5],
          max_posts_per_scan: 64,
      };
      let initial_credit = corporations.treasury_micros[0]
          + ships.credits_micros.iter().sum::<i64>()
          + contracts.escrow_micros.iter().sum::<i64>();
      let dispatch = DispatchCfg { demand_low: 0, demand_high: 0, ..DispatchCfg::default() };
      let mut diag = AssignDiag::default();
      let route_evidence = crate::world::RouteEvidence::empty(4);
      let mut events = EventStream::default();
      let shipyard = ShipyardCfg::default();
      let trophic = TrophicCfg::default();

      run_scripted_dispatch(
          &mut contracts, &stations, &mut ships, &[], &route_evidence,
          false, false, &mut diag, &dispatch, &shipyard, &trophic,
          &arb, &corporations, Tick(1), &mut events,
      );

      assert_eq!(contracts.status[kid_idx], ContractStatus::Failed,
          "insolvent accepted row → Failed");
      assert_eq!(ships.role[0], CraftRole::Idle, "hauler released — no zombie");
      assert!(ships.contract[0].is_none(), "hauler contract cleared");
      assert_eq!(contracts.escrow_micros[kid_idx], 0, "escrow returned to treasury");
      // Credit identity holds.
      let final_credit = corporations.treasury_micros[0]
          + ships.credits_micros.iter().sum::<i64>()
          + contracts.escrow_micros.iter().sum::<i64>();
      assert_eq!(final_credit, initial_credit,
          "no-zombie: credit identity preserved through escrow refund");
  }
  ```

  Run: `cargo test -p jumpgate-core no_zombie_claimed_then_insolvent_corp_releases_fleet`
  Expected: PASS.
  Run: `cargo test --workspace && cargo clippy --all-targets -- -D warnings`
  Expected: all green.

- [ ] **Step 5: Commit**

  ```bash
  git add crates/jumpgate-core/src/economy.rs
  git commit -F - <<'EOF'
  feat(gag-a4): withdrawal sweep — Offered recheck + Accepted-never-loaded (L1-C1)

  Extends stage-1b2 scan with two withdrawal passes gated on
  scan_interval > 0:
  (a) Offered rows: recompute spread*qty - transport at current prices;
      collapse → Failed + OfferWithdrawn + craft-intent cleared.
  (b) Accepted-never-loaded rows: if corp.treasury < price[from]*qty,
      escrow refund → treasury, hauler released (Idle), Failed +
      OfferWithdrawn. Prevents the permanent fleet attrition documented
      in grounding extract §11(b). No-zombie test covers the
      claimed-then-insolvent path. Credit identity preserved.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A4.6: Exchange battery sizing note and drain read (recorded, never gated)

**What:** The synthesis (Part 1.2, Solvency honesty, OD-2) requires printing the Exchange treasury as an anchored stdout read. The Exchange corp is seeded with a sized battery computed from the measured drain window. The print is config-gated on `ExchangeCfg::active` (part of A3). This task adds the anchored BAZAAR drain line to `trophic_run.rs` and a comment in `scenario_bazaar.rs` documenting the battery sizing formula. **This is never a pass/fail gate — recorded only.**

**Anchored line format (synthesis Part 3):** `BAZAAR drain=<exchange_treasury_micros> ticks=<run_ticks>` — printed once at run-end when `cfg.exchange.active` is true. The regex must land in the same commit as the `println!` (lockstep rule).

**Files:**
- Modify: `crates/jumpgate-bin/src/trophic_run.rs` — add BAZAAR drain println at run end when `cfg.exchange.active`; add regex to `ANCHORED` array; append fixture to `test_sweep_parsing` (or create test)
- Modify: `crates/jumpgate-core/src/scenario.rs` — add battery sizing comment in `scenario_bazaar` corp init

---

- [ ] **Step 1: Failing test — BAZAAR line parsed when ExchangeCfg active**

  In `trophic_run.rs` tests (or the sweep parsing test file), add:

  ```rust
  #[test]
  fn bazaar_drain_line_is_parsed_by_anchored_regex() {
      // The BAZAAR line: "BAZAAR drain=5400000000 ticks=50000"
      let line = "BAZAAR drain=5400000000 ticks=50000";
      // The regex in ANCHORED must match this line format.
      let re = regex::Regex::new(r"^BAZAAR drain=(\d+) ticks=(\d+)$").unwrap();
      assert!(re.is_match(line), "BAZAAR line must match anchored regex");
      let caps = re.captures(line).unwrap();
      assert_eq!(&caps[1], "5400000000");
      assert_eq!(&caps[2], "50000");
  }
  ```

  Run: `cargo test -p jumpgate-bin bazaar_drain_line_is_parsed_by_anchored_regex`
  Expected failure: test module compile error (regex not yet in ANCHORED; test just validates format).

- [ ] **Step 2: Add BAZAAR drain println and regex in same commit**

  In `trophic_run.rs`, at the run-end print block (after the per-run stats, near where GOSSIP/META are printed), add:

  ```rust
  // BAZAAR drain read (OD-2 solvency honesty, goods-as-goods A4).
  // Printed only when exchange.active — the battery drain window for console judgment.
  // Never a gate; recorded only per synthesis Part 1.2.
  if cfg.exchange.active {
      if let Some(exch_row) = cfg.exchange.corp_index as usize {
          let drain = cfg_initial_exchange_treasury  // stored at run start
              - world.corporations.treasury_micros[exch_row];
          println!("BAZAAR drain={} ticks={}", drain, ticks_run);
      }
  }
  ```

  (The builder must store the initial Exchange treasury at run start — `let cfg_initial_exchange_treasury = world.corporations.treasury_micros[cfg.exchange.corp_index as usize];` before the step loop. The exact variable name follows the trophic_run.rs `initial_credits` precedent.)

  Add to the `ANCHORED` regex array (wherever it lives in trophic_run.rs):

  ```rust
  // BAZAAR drain read — Exchange treasury drain (goods-as-goods A4, config-gated).
  r"^BAZAAR drain=(?P<exchange_drain>\d+) ticks=(?P<ticks>\d+)$",
  ```

  Add or append to the sweep-parsing fixture test: a line `"BAZAAR drain=5400000000 ticks=50000"` that passes the ANCHORED regex.

  Add the battery sizing comment in `scenario_bazaar`:

  ```rust
  // Exchange corp (index N): seeded as a sized battery. Worst drain measured at
  // ~5.4e9 micros/100k ticks (synthesis solvency arithmetic, OD-2). Refuel recapture
  // ≤ 0.3e9/100k. Seed from a calibration run's measured drain window. Consumption-minted
  // money is the named trigger if the console shows universal late-game heat death.
  // PDR-0007: the battery models "laying the tubes," not a self-sustaining economy.
  CorporationInit { treasury_micros: 20_000_000_000, home_station_index: 0 },
  ```

  Run: `cargo test -p jumpgate-bin bazaar_drain_line_is_parsed_by_anchored_regex`
  Expected: PASS.
  Run: `cargo test --workspace`
  Expected: all green.

- [ ] **Step 3: Commit**

  ```bash
  git add crates/jumpgate-bin/src/trophic_run.rs \
          crates/jumpgate-core/src/scenario.rs
  git commit -F - <<'EOF'
  feat(gag-a4): Exchange battery sizing note + BAZAAR drain read (OD-2)

  Adds anchored stdout BAZAAR line (drain=<micros> ticks=<n>) printed
  at run-end when ExchangeCfg::active, with matching ANCHORED regex in
  the same commit (lockstep rule). Records Exchange treasury drain as a
  console judgment window — never a pass/fail gate per PDR-0006.
  Battery sizing comment in scenario_bazaar documents the measured
  ~5.4e9 micros/100k drain window and the consumption-minted trigger.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

## Phase A4 summary

| Task | ID | Commits |
|------|----|---------|
| `OfferWithdrawn` event + exhaustive chronicle match | A4.1 | 1 |
| `ArbitrageCfg` transport-table confirmation (struct + re-pin landed in A3.2) | A4.2 | 1 |
| REPOST early-return prelude (hash-neutral, behavior-identical) | A4.3 | 1 |
| Arbitrage poster in `run_scripted_dispatch` (stage 1b2) | A4.4 | 1 |
| Withdrawal sweep — Offered recheck + Accepted-never-loaded (L1-C1) | A4.5 | 1 |
| Exchange battery sizing note + BAZAAR drain read | A4.6 | 1 |

**Total: 6 single-cause commits.**

### Dependency map

```
A4.1 (OfferWithdrawn event)
  ↓ (used by withdrawal sweep)
A4.2 (ArbitrageCfg config + GOLDEN re-pin)
  ↓
A4.3 (REPOST prelude — behavior-identical)
  ↓
A4.4 (arbitrage poster)
  ↓
A4.5 (withdrawal sweep uses OfferWithdrawn from A4.1)
  ↓
A4.6 (drain read uses ExchangeCfg from A3)
```

### What is NOT in A4 (explicitly out)

- `TradeBought` / `TradeSold` events (landed in A4's sibling task alongside the own-trade verb and `EventKind::Trade` deletion — the cut specifies them landing "in the same commit that DELETES the dead EventKind::Trade")
- `JetsamStore`, jettison verb, fence stage (rung B, L5-C4)
- `greed_milli`, `posture_threshold` config knobs (rung B)
- Evidence-priced wages (OD-4c rejected)
- `pending_trade_buy` / `pending_trade_sell` intent columns (these land in the own-trade task, a different A4 sibling — the plan section here covers corp arbitrage/REPOST replacement only per the task brief)

## Phase A5 — Two-mode Policy + `scenario_bazaar`

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
            &self.config.arbitrage,   // NOTE (C1): field is `arbitrage`, not `arb`
            &self.config.exchange,
            &self.config.goods,       // NOTE (C6): field is `goods`, not `goods_cfg`
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

**Step 11: add `TradeBought`/`TradeSold` chronicle/gossip-log arms in A5.1 matches**

Note (M1/C2): `EventKind::Trade` was deleted in A0.3; `TradeBought`/`TradeSold` were
added in A3.4. The matches in `chronicle_subject` and `gossip_log_event_json` are
already exhaustive (A0.3 removed the wildcard). This step adds the match arms for
the new variants — a missing arm is a compile error. Do NOT remove a wildcard here
(there is none to remove) and do NOT delete `Trade` again (A0.3 already did it).

In `crates/jumpgate-core/src/contract.rs`: `TradeBought`/`TradeSold` are already
present from A3.4. Verify the enum compiles. Add:
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
!scripted craft skipped (D7 split discipline). EventKind::Trade was deleted in A0.3;
adds TradeBought/TradeSold arms. New pending columns have matching debug_assert! in
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

**Step 2: verify config structs from A3.2 (NOTE C1/C6 — no re-definition here)**

NOTE (C1, C6): `GoodSpec`, `GoodsCfg`, `ExchangeCfg`, and `ArbitrageCfg` are ALL
defined in **Task A3.2** (the ONE canonical definition). `GoodSpec.name` is `String`
(not `&'static str` — C6 fix). `ArbitrageCfg` has the superset field set from the
synthesis: `{scan_interval, wage_flat_micros, wage_share_milli, transport_micros:
Vec<Vec<i64>>, qty_ladder, max_posts_per_scan, arb_premium_micros}`. A5.2 does NOT
redefine any of these. The `RunConfig` field for goods is `goods: GoodsCfg` (not
`goods_cfg`). The `RunConfig` field for arbitrage is `arbitrage: ArbitrageCfg` (not `arb`).

Verify by running:
```bash
cargo test -p jumpgate-core goods_cfg_arb_cfg_exchange_cfg_are_config_hashed
cargo test -p jumpgate-core config_hash_golden_anchor_is_stable
```
Both should PASS (structs were folded in A3.2; golden was re-pinned in A3.2).
If they fail, A3.2 did not land yet — stop and complete A3.2 first.

**Step 3: GOLDEN_CONFIG_HASH — already done in A3.2 (NOTE C1 — no re-pin here)**

NOTE (C1): The GOLDEN_CONFIG_HASH was re-pinned in A3.2 (the ONE rung-A re-pin).
A5.2 does NOT re-pin it. If `config_hash_golden_anchor_is_stable` fails at this
point, the issue is in A3.2's execution, not A5.2.

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
    let n_goods = cfg.goods.goods.len();
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
            cfg.goods.goods[g].name
        );
        assert!(
            !sinks.is_empty(),
            "good {g} ({}) must have >= 1 sink producer",
            cfg.goods.goods[g].name
        );
    }
}

#[test]
fn scenario_bazaar_source_sink_disjoint_per_good() {
    use crate::economy::Good;
    let cfg = scenario_bazaar(7);
    let n_goods = cfg.goods.goods.len();

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
            cfg.goods.goods[g].name,
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

## Phase A6 — Science + Console (Rung-A Windows, Digest Exit)

# Phase A6 — Science + Console (Rung-A windows, digest exit)

> **Frame (PDR-0006):** every number produced here is a designer's WINDOW for
> the console observe→steer→re-observe loop — never an acceptance gate, never a
> build trigger. Recorded, not gated.
>
> **Ordering invariant:** A6 tasks run AFTER the last A0 commit (digest
> baseline pinned there) and AFTER A1-A5 mechanics. The A0 baseline is the
> rung-A exit reference; A6.0 pins it, then A6.1-A6.6 build the readers, then
> A6.7 runs the 20-seed ensemble and produces the console packet.
>
> **Scope reminder:** Rung A only. No jettison, fencing, JetsamStore, posture,
> greed, or rung-B config. BAZAAR/CRATE line — only BAZAAR lands here; CRATE
> is rung B.

---

### Task A6.0: Behavior digest baseline (A0 tip pinned)

**Files**

- Create: `runs/gag-a6-baseline/` (directory, builder creates at run time)
- Modify: nothing (procedure only — no source changes, no commit)

**Steps**

- [ ] **Step 1: Verify A0 is the last landed commit**

  ```
  git log --oneline -5
  ```

  Confirm the HEAD commit is the final A0 instruments commit (no A1+ mechanics
  present). If A1+ has already landed, the baseline must have been pinned at the
  A0 tip; check `runs/gag-a6-baseline/` for pre-existing digests and skip
  forward to A6.1.

- [ ] **Step 2: Build release binaries**

  ```bash
  cargo build -p jumpgate-core --release --example trophic_run 2>&1 | tail -5
  ```

  Expected: `Finished release profile`.

- [ ] **Step 3: Run A0-tip digest across both reference scenarios**

  For each `SCENARIO` in `trophic frontier`:

  ```bash
  mkdir -p runs/gag-a6-baseline
  for S in 7 23; do
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --scenario $SCENARIO --seed $S --ticks 2000 \
      --gossip-log runs/gag-a6-baseline/${SCENARIO}-base-s${S}.gossip.jsonl \
      --jsonl    runs/gag-a6-baseline/${SCENARIO}-base-s${S}.jsonl \
      > runs/gag-a6-baseline/${SCENARIO}-base-s${S}.out
  done
  ```

- [ ] **Step 4: Compute and record digests**

  ```bash
  for SCENARIO in trophic frontier; do
    for S in 7 23; do
      BASE=runs/gag-a6-baseline/${SCENARIO}-base-s${S}
      sha256sum ${BASE}.out ${BASE}.jsonl ${BASE}.gossip.jsonl
    done
  done | tee runs/gag-a6-baseline/BASELINE_DIGESTS.txt
  ```

  Paste the output into `runs/gag-a6-baseline/BASELINE_DIGESTS.txt`.
  This file is the rung-A exit reference. It must NOT be in the git staging
  area (`runs/` is `.gitignore`d by HOUSE RULES — never stage it).

- [ ] **Step 5: Sanity check — no A1+ behavior yet**

  The RESULT lines should show `verdict` values matching the pre-A5 banked
  runs. If trophic seed=7 shows `PermanentPeace` on a 2000-tick run that is
  expected (PermanentPeace can fire in short runs on some seeds). The digest
  pinning is the gate; absolute verdict is not.

---

### Task A6.1: WA1 survival-by-market reader

**Files**

- Create: `python/analysis/wa1_survival.py`
- Create: `python/tests/test_wa1_survival.py`

**Steps**

- [ ] **Step 1: Write the failing test first**

  `python/tests/test_wa1_survival.py`:

  ```python
  """WA1 survival-by-market reader pins (spec §5 WA1; windows, not gates).

  WA1 reads: per-station per-good minimum stock over the run (zero-stock
  run-length) + the consumer-starved hauler count (stalled-consumer read from
  JSONL). Either answer is a finding: localized starvation at the rim is
  expected and interesting; universal starvation means the market broke.
  The anti-mirroring (L4-F4) transport table tail row is read here, never
  mirrored as a Python constant.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa1_survival


  def _make_windows(n_stations, n_goods, stocks):
      """Synthetic window list with per_station_stock flat matrix.

      stocks: list of length n_stations * n_goods (row-major: station-major,
      good-minor), per window. Pass a list of lists (one per window).
      """
      return [
          {
              "tick": (i + 1) * 2000,
              "per_station_stock": stk,
              "per_station_price": [100_000] * (n_stations * n_goods),
          }
          for i, stk in enumerate(stocks)
      ]


  def test_zero_stock_run_length_all_good():
      # 2 stations, 2 goods, no zeros ever
      windows = _make_windows(2, 2, [
          [10, 20, 30, 40],
          [15, 25, 35, 45],
      ])
      result = wa1_survival.stock_runs(windows, n_stations=2, n_goods=2)
      # max zero-stock run-length = 0 for every (station, good)
      for row in result:
          assert row["max_zero_run"] == 0


  def test_zero_stock_run_length_detects_consecutive_zeros():
      # station 0, good 1 has zeros in windows 0 and 1 (run of 2)
      stocks = [
          [10, 0, 30, 40],
          [12, 0, 32, 42],
          [14, 5, 34, 44],
      ]
      windows = _make_windows(2, 2, stocks)
      result = wa1_survival.stock_runs(windows, n_stations=2, n_goods=2)
      by_key = {(r["station"], r["good"]): r for r in result}
      assert by_key[(0, 1)]["max_zero_run"] == 2
      assert by_key[(0, 0)]["max_zero_run"] == 0


  def test_stalled_consumer_count_from_jsonl():
      # deliver rows with same craft back-to-back should count stalls
      # stalled = craft has no deliver events in a window that has traffic
      hauler_slots = [0, 1, 2]
      windows = _make_windows(2, 2, [[10, 20, 30, 40]] * 3)
      # craft 0 never delivers; craft 1 delivers once; craft 2 delivers twice
      deliver_rows = [
          {"e": "deliver", "tick": 2001, "hauler": 1, "good": 0},
          {"e": "deliver", "tick": 2001, "hauler": 2, "good": 0},
          {"e": "deliver", "tick": 2002, "hauler": 2, "good": 1},
      ]
      result = wa1_survival.stalled_consumers(windows, deliver_rows, hauler_slots)
      # craft 0 stalled in all windows that have any deliver activity
      assert result["craft_0_deliver_count"] == 0
      assert result["craft_1_deliver_count"] == 1
      assert result["craft_2_deliver_count"] == 2


  def test_transport_table_tail_row_is_read_not_mirrored():
      # The factory transport table is echoed as a no-tick JSONL tail row
      # (L4-F4 anti-mirroring). wa1_survival must read it from the JSONL,
      # not from a module-level constant.
      tail_row = {
          "e": "transport_table",
          "routes": [[0, 1], [1, 0]],
          "transport_micros": [50000, 60000],
      }
      t = wa1_survival.read_transport_table([tail_row])
      assert t is not None
      assert t["transport_micros"] == [50000, 60000]


  def test_transport_table_absent_returns_none():
      rows = [{"e": "refuel", "tick": 100, "craft": 0, "station": 1,
               "units": 5, "price_micros": 10000,
               "before_permille": 800, "after_permille": 900}]
      assert wa1_survival.read_transport_table(rows) is None
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa1_survival.py -x 2>&1 | tail -20
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa1_survival'
  ```

- [ ] **Step 2: Write `python/analysis/wa1_survival.py`**

  ```python
  """wa1_survival — WA1 survival-by-market reader (spec §5 WA1).

  FRAME (PDR-0006): windows, not gates. Either answer is a finding:
  - localized starvation at rim stations = expected with clumped topology
  - universal starvation = the market broke
  Anti-mirroring (L4-F4): the factory transport table is read from the no-tick
  JSONL tail row emitted by the runner, never mirrored as a Python constant.

  Usage:
      python3 python/analysis/wa1_survival.py <windows.jsonl> \\
          [--gossip-log <gossip.jsonl>] [--n-stations N] [--n-goods G]
  """

  import argparse
  import json
  import pathlib
  import sys


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def stock_runs(windows, n_stations, n_goods):
      """Per-(station, good) max consecutive-zero-stock window run-length.

      Returns list of dicts: {station, good, max_zero_run, zero_windows, total_windows}.
      Zero windows = the consumer-starve raw count; max_zero_run = the worst
      localized drought.
      """
      results = []
      for st in range(n_stations):
          for g in range(n_goods):
              idx = st * n_goods + g
              cur_run = 0
              max_run = 0
              zero_count = 0
              for w in windows:
                  stock = w.get("per_station_stock", [])
                  if idx < len(stock) and stock[idx] == 0:
                      cur_run += 1
                      zero_count += 1
                      max_run = max(max_run, cur_run)
                  else:
                      cur_run = 0
              results.append({
                  "station": st,
                  "good": g,
                  "max_zero_run": max_run,
                  "zero_windows": zero_count,
                  "total_windows": len(windows),
              })
      return results


  def stalled_consumers(windows, deliver_rows, hauler_slots):
      """Per-craft deliver counts over the run.

      A craft with zero delivers is a stalled consumer: either broke (wage
      mode, waiting for work) or stranded. Either reading is a finding.
      Returns dict with craft_{slot}_deliver_count keys.
      """
      counts = {s: 0 for s in hauler_slots}
      for row in deliver_rows:
          if row.get("e") == "deliver":
              h = row.get("hauler")
              if h in counts:
                  counts[h] += 1
      return {f"craft_{s}_deliver_count": v for s, v in counts.items()}


  def read_transport_table(rows):
      """Read the factory transport table from the no-tick JSONL tail row.

      Returns the tail row dict if found, else None (anti-mirroring: L4-F4).
      The runner emits one row with e='transport_table' at run end; older
      runs without the row return None — version gate, no abort.
      """
      for row in rows:
          if row.get("e") == "transport_table":
              return row
      return None


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("windows", help="per-window JSONL file from trophic_run")
      ap.add_argument("--gossip-log", help="gossip-log JSONL with deliver rows")
      ap.add_argument("--n-stations", type=int, required=True)
      ap.add_argument("--n-goods", type=int, required=True)
      args = ap.parse_args()

      all_rows = load(args.windows)
      windows = [r for r in all_rows if "tick" in r]
      gossip = load(args.gossip_log) if args.gossip_log else []

      transport = read_transport_table(all_rows + gossip)
      if transport is None:
          print("WA1 transport table: absent (pre-A0 run — anti-mirroring: L4-F4 not yet wired)")
      else:
          print(f"WA1 transport table: {transport}")

      # Hauler slots: derive from per_craft_role if present (role 1 = hauler),
      # else assume all crafts in per_craft_credits are haulers.
      hauler_slots = []
      if windows and "per_craft_role" in windows[0]:
          for slot, role in enumerate(windows[0]["per_craft_role"]):
              if role == 1:
                  hauler_slots.append(slot)
      elif windows and "per_craft_credits" in windows[0]:
          # Fallback: no role info. Use META haulers count if available.
          hauler_slots = list(range(len(windows[0]["per_craft_credits"])))

      deliver_rows = [r for r in gossip if r.get("e") == "deliver"]
      stall = stalled_consumers(windows, deliver_rows, hauler_slots)
      zero_delivers = sum(1 for v in stall.values() if v == 0)
      total_haulers = len(hauler_slots)
      print(
          f"WA1 stalled consumers (zero delivers over run): "
          f"{zero_delivers}/{total_haulers} haulers "
          "(RECORDED, never gated — PDR-0006; zero = no deliver events or pre-A0)"
      )

      runs = stock_runs(windows, args.n_stations, args.n_goods)
      print(
          f"\nWA1 survival-by-market ({len(windows)} windows, "
          f"{args.n_stations} stations × {args.n_goods} goods) "
          "(RECORDED, never gated — PDR-0006):"
      )
      print(f"  {'station':>7}  {'good':>4}  {'max_zero_run':>12}  "
            f"{'zero_windows':>12}  {'total':>5}")
      for r in runs:
          flag = " <-- STARVATION" if r["max_zero_run"] > 0 else ""
          print(
              f"  {r['station']:>7}  {r['good']:>4}  {r['max_zero_run']:>12}  "
              f"{r['zero_windows']:>12}  {r['total_windows']:>5}{flag}"
          )

      # Summary: either answer is a finding
      starving = [r for r in runs if r["max_zero_run"] > 0]
      if not starving:
          print("\nWA1 reading: NoStarvation — all goods at all stations held stock "
                "above zero in every window (RECORDED; finding: market feeds the world)")
      else:
          stations_hit = {r["station"] for r in starving}
          all_stations = set(range(args.n_stations))
          rim = max(all_stations)
          rim_only = stations_hit <= {rim, rim - 1}
          if rim_only:
              print(f"\nWA1 reading: RimLocalized — starvation confined to "
                    f"rim stations {sorted(stations_hit)} (RECORDED; "
                    "finding: market feeds core; rim wants supply or a lane)")
          else:
              print(f"\nWA1 reading: Universal — starvation at stations "
                    f"{sorted(stations_hit)} (RECORDED; finding: market broke or "
                    "topology mismatch — owner's call)")


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa1_survival.py -v 2>&1 | tail -20
  ```

  Expected:
  ```
  test_zero_stock_run_length_all_good PASSED
  test_zero_stock_run_length_detects_consecutive_zeros PASSED
  test_stalled_consumer_count_from_jsonl PASSED
  test_transport_table_tail_row_is_read_not_mirrored PASSED
  test_transport_table_absent_returns_none PASSED
  5 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa1_survival.py python/tests/test_wa1_survival.py
  git commit -F - <<'EOF'
  feat(lab): WA1 survival-by-market reader — zero-stock run-lengths + stalled consumers

  Reads per-station × per-good max-consecutive-zero-stock run-length and the
  per-craft deliver count (stalled-consumer proxy) from the gossip-log deliver
  rows. Anti-mirroring: transport table read from the no-tick tail row emitted
  by the runner (L4-F4), never mirrored as a Python constant. Either answer is
  a finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.2: WA2 spread-closure reader

**Files**

- Create: `python/analysis/wa2_spread_closure.py`
- Create: `python/tests/test_wa2_spread_closure.py`

**Steps**

- [ ] **Step 1: Write the failing test first**

  `python/tests/test_wa2_spread_closure.py`:

  ```python
  """WA2 spread-closure reader pins (spec §5 WA2; windows, not gates).

  WA2: posted spread on a route decays after package delivery (arbitrage
  arbitrages). Join: post → accept → deliver rows in gossip-log, keyed by
  contract; measure price-at-post minus price-at-deliver per route per good;
  decay over successive contracts on the same route/good pair is the signal.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa2_spread_closure


  def _post(contract, tick, route, good, spread_micros):
      return {"e": "post", "tick": tick, "contract": contract,
              "route": route, "good": good, "spread_micros": spread_micros}


  def _accept(contract, tick):
      return {"e": "accept", "tick": tick, "contract": contract}


  def _deliver(contract, tick, spread_at_deliver):
      return {"e": "deliver", "tick": tick, "contract": contract,
              "spread_at_deliver": spread_at_deliver}


  def test_spread_closure_detects_decay():
      rows = [
          _post(1, 100, 0, 2, 80_000),
          _accept(1, 200),
          _deliver(1, 500, 30_000),
          _post(2, 600, 0, 2, 40_000),
          _accept(2, 700),
          _deliver(2, 900, 10_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      # route 0, good 2: spreads started at 80k, closed to 10k
      assert (0, 2) in result
      series = result[(0, 2)]
      assert series[0]["spread_at_post"] == 80_000
      assert series[1]["spread_at_post"] == 40_000


  def test_spread_closure_open_contract_excluded():
      # Contract without a deliver row = in-flight, excluded from the series
      rows = [
          _post(1, 100, 0, 0, 50_000),
          _accept(1, 200),
          # no deliver for contract 1
          _post(2, 300, 0, 0, 45_000),
          _accept(2, 400),
          _deliver(2, 600, 15_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      # only contract 2 completes; series length = 1
      assert (0, 0) in result
      assert len(result[(0, 0)]) == 1


  def test_spread_closure_no_deliver_rows_returns_empty():
      rows = [
          _post(1, 100, 0, 0, 60_000),
          _accept(1, 200),
      ]
      result = wa2_spread_closure.spread_series(rows)
      assert result == {}


  def test_decay_flag_detected():
      rows = [
          _post(1, 100, 3, 1, 100_000),
          _deliver(1, 500, 40_000),
          _post(2, 600, 3, 1, 60_000),
          _deliver(2, 900, 20_000),
          _post(3, 1000, 3, 1, 30_000),
          _deliver(3, 1200, 5_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      summary = wa2_spread_closure.summarize(result)
      # route 3, good 1 should show decay
      hit = [r for r in summary if r["route"] == 3 and r["good"] == 1]
      assert len(hit) == 1
      assert hit[0]["decaying"] is True
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa2_spread_closure.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa2_spread_closure'
  ```

- [ ] **Step 2: Write `python/analysis/wa2_spread_closure.py`**

  ```python
  """wa2_spread_closure — WA2 spread-closure reader (spec §5 WA2).

  FRAME (PDR-0006): windows, not gates.
  Decay is the signal: posted spread on a (route, good) pair should fall
  after delivery as the arbitrage opportunity self-eliminates. Any trend
  (rising = routes opening up, flat = persistent gap, falling = closure)
  is a finding.

  Join order: post → accept → deliver rows per contract_id. The "post"
  gossip-log row is runner-enriched (from the ContractOffered event + current
  prices at log time) with spread_micros; the "deliver" row (ContractFulfilled,
  A0 instrument) carries spread_at_deliver (price spread at delivery tick).
  Contracts without a deliver row (still in flight) are excluded.

  Usage:
      python3 python/analysis/wa2_spread_closure.py <gossip.jsonl>
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def spread_series(rows):
      """Build per-(route, good) series of completed contracts.

      Returns dict[(route, good)] = list of {contract, spread_at_post,
      spread_at_deliver, post_tick, deliver_tick} in post-tick order.
      """
      posts = {}
      accepts = {}
      delivers = {}
      for row in rows:
          e = row.get("e")
          c = row.get("contract")
          if c is None:
              continue
          if e == "post":
              posts[c] = row
          elif e == "accept":
              accepts[c] = row
          elif e == "deliver":
              delivers[c] = row

      result = {}
      for c, d in delivers.items():
          p = posts.get(c)
          if p is None:
              continue
          key = (p.get("route"), p.get("good"))
          if None in key:
              continue
          entry = {
              "contract": c,
              "spread_at_post": p.get("spread_micros", 0),
              "spread_at_deliver": d.get("spread_at_deliver", 0),
              "post_tick": p.get("tick", 0),
              "deliver_tick": d.get("tick", 0),
          }
          result.setdefault(key, []).append(entry)

      for key in result:
          result[key].sort(key=lambda x: x["post_tick"])
      return result


  def summarize(result):
      """Per-(route, good) decay summary rows for printing."""
      out = []
      for (route, good), series in sorted(result.items()):
          if len(series) < 2:
              decaying = None  # too few data points
          else:
              first_half = series[: len(series) // 2]
              second_half = series[len(series) // 2 :]
              first_avg = sum(s["spread_at_post"] for s in first_half) // len(first_half)
              second_avg = sum(s["spread_at_post"] for s in second_half) // len(second_half)
              decaying = second_avg < first_avg
          out.append({
              "route": route,
              "good": good,
              "n_contracts": len(series),
              "first_spread": series[0]["spread_at_post"] if series else None,
              "last_spread": series[-1]["spread_at_post"] if series else None,
              "decaying": decaying,
          })
      return out


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      result = spread_series(rows)
      summary = summarize(result)

      print(
          f"WA2 spread-closure ({len(result)} route×good pairs with completed contracts) "
          "(RECORDED, never gated — PDR-0006):"
      )
      if not summary:
          print("  no completed contracts in gossip log (pre-A2 run or no bazaar traffic)")
          return

      print(f"  {'route':>5}  {'good':>4}  {'n':>4}  {'first_spread':>12}  "
            f"{'last_spread':>11}  {'decaying':>8}")
      for r in summary:
          d = str(r["decaying"]) if r["decaying"] is not None else "?"
          print(
              f"  {r['route']:>5}  {r['good']:>4}  {r['n_contracts']:>4}  "
              f"{r['first_spread']!s:>12}  {r['last_spread']!s:>11}  {d:>8}"
          )

      decaying_count = sum(1 for r in summary if r["decaying"] is True)
      flat_count = sum(1 for r in summary if r["decaying"] is False)
      pending_count = sum(1 for r in summary if r["decaying"] is None)
      print(
          f"\nWA2 reading: decaying={decaying_count} flat={flat_count} "
          f"pending={pending_count} (decaying = arbitrage arbitrages; "
          "flat = persistent gap = priced-in transport or no competition; "
          "either is a finding — owner's call)"
      )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa2_spread_closure.py -v 2>&1 | tail -15
  ```

  Expected:
  ```
  test_spread_closure_detects_decay PASSED
  test_spread_closure_open_contract_excluded PASSED
  test_spread_closure_no_deliver_rows_returns_empty PASSED
  test_decay_flag_detected PASSED
  4 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa2_spread_closure.py python/tests/test_wa2_spread_closure.py
  git commit -F - <<'EOF'
  feat(lab): WA2 spread-closure reader — post/deliver join, decay detection per route×good

  Joins gossip-log post→deliver rows per contract; measures spread-at-post vs
  spread-at-deliver per (route, good) pair; flags routes where second-half
  average spread is below first-half (arbitrage arbitrages). Either trend is a
  finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.3: WA3 + WA5 joint reader (channel mix vs capitalization, trophic preservation)

**Files**

- Create: `python/analysis/wa3_wa5_joint.py`
- Create: `python/tests/test_wa3_wa5_joint.py`

**Steps**

- [ ] **Step 1: Failing test**

  `python/tests/test_wa3_wa5_joint.py`:

  ```python
  """WA3+WA5 joint reader pins (spec §5 WA3/WA5; panel joint-read warning).

  WA3 and WA5 are a JOINT READ: own-trade share IS the pirate food supply.
  Shrunken own-trade share → less prey → PermanentPeace masquerade.
  The panel's warning: read WA3 and WA5 side-by-side; a high WA3 own-trade
  share that correlates with PermanentPeace verdict on the SAME seeds is the
  prey-shrink confound, not a finding that bazaar killed boom-bust.

  WA5: verdict distribution-vs-frontier-bank, NEVER same-seed paired.
  The clean_seeds filter (blind-born != PermanentPeace) is mandatory before
  reading the verdict mix; PermanentPeace is first in the verdict chain
  (diagnostics.rs:288) and overrides cycled.

  This module holds the joint reader. It does NOT make a decision; it prints
  the side-by-side and names the confound.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa3_wa5_joint


  def _window(credits, trade_sold=0, trade_bought=0, robs=0, laden_trips=0):
      return {
          "tick": 2000,
          "per_craft_credits": credits,
          "trade_sold_count": trade_sold,
          "trade_bought_count": trade_bought,
          "robs": robs,
          "laden_trips": laden_trips,
      }


  def test_own_trade_share_zero_when_no_trades():
      windows = [_window([100, 200, 50])]
      share = wa3_wa5_joint.own_trade_share_milli(windows, n_haulers=3)
      assert share == 0


  def test_own_trade_share_is_trade_over_trade_plus_contract():
      # 3 trade_sold vs 7 laden_trips (contract delivers)
      windows = [_window([100, 200, 50], trade_sold=3, laden_trips=7)]
      share = wa3_wa5_joint.own_trade_share_milli(windows, n_haulers=3)
      # 3 / (3 + 7) = 0.3 = 300 milli
      assert share == 300


  def test_clean_seeds_filters_permanent_peace():
      cells = {
          7:  {"verdict": "Alive",         "own_trade_share_milli": 200},
          11: {"verdict": "PermanentPeace","own_trade_share_milli": 800},
          13: {"verdict": "NoCycle",       "own_trade_share_milli": 100},
      }
      clean = wa3_wa5_joint.clean_seeds(cells)
      assert clean == [7, 13]


  def test_prey_shrink_confound_flagged():
      # High own-trade share AND PermanentPeace on same seed = confound
      bazaar_cells = {
          7:  {"verdict": "PermanentPeace", "own_trade_share_milli": 750},
          11: {"verdict": "Alive",          "own_trade_share_milli": 150},
      }
      confound_seeds = wa3_wa5_joint.prey_shrink_confound_seeds(bazaar_cells, threshold_milli=500)
      assert confound_seeds == [7]


  def test_prey_shrink_confound_empty_when_no_pp():
      cells = {
          7:  {"verdict": "Alive", "own_trade_share_milli": 750},
          11: {"verdict": "Alive", "own_trade_share_milli": 300},
      }
      assert wa3_wa5_joint.prey_shrink_confound_seeds(cells, threshold_milli=500) == []


  def test_verdict_distribution_never_same_seed_paired():
      # WA5 compares bazaar distribution vs frontier bank as distributions,
      # never same-seed paired — the function must not accept a shared seed list
      # and must operate on independent sample bags.
      bazaar_bag = ["Alive", "NoCycle", "Alive", "Alive"]
      frontier_bag = ["Alive", "Alive", "NoCycle", "Alive"]
      dist = wa3_wa5_joint.verdict_distributions(bazaar_bag, frontier_bag)
      assert dist["bazaar"]["Alive"] == 3
      assert dist["frontier"]["Alive"] == 3
      assert "same_seed_pairing" not in dist  # must not exist


  def test_wa5_output_has_wa3_column():
      # M3 (synthesis): every WA5 verdict-mix row must carry own_trade_share_milli
      # alongside the verdict. This is a co-read, not a gate.
      # Simulate the cells dict that sweep_bazaar / wa3_wa5_joint produces for
      # WA5 input: each entry must have both "verdict" and "own_trade_share_milli".
      cells = {
          7:  {"verdict": "Alive",          "own_trade_share_milli": 200},
          11: {"verdict": "NoCycle",         "own_trade_share_milli": 350},
          13: {"verdict": "PermanentPeace",  "own_trade_share_milli": 800},
      }
      for seed, cell in cells.items():
          assert "own_trade_share_milli" in cell, (
              f"seed {seed}: WA5 verdict-mix row missing own_trade_share_milli "
              f"(M3 co-read — WA3 and WA5 are a joint read)"
          )
          assert "verdict" in cell, f"seed {seed}: WA5 cell missing verdict"
      # The WA3 column must survive the clean_seeds filter (it is NOT stripped).
      clean = wa3_wa5_joint.clean_seeds(cells)
      for seed in clean:
          assert "own_trade_share_milli" in cells[seed], (
              f"clean seed {seed}: own_trade_share_milli must be present on every "
              "clean cell passed to the WA5 distribution read"
          )
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa3_wa5_joint.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa3_wa5_joint'
  ```

- [ ] **Step 2: Write `python/analysis/wa3_wa5_joint.py`**

  ```python
  """wa3_wa5_joint — WA3 + WA5 joint reader (spec §5 WA3/WA5).

  FRAME (PDR-0006): windows, not gates.
  JOINT READ (panel directive): own-trade share (WA3) IS the pirate food
  supply. High own-trade share → fewer crate haulers on the public board →
  fewer targets → PermanentPeace masquerade. NEVER read WA5 without reading
  WA3 first and checking for the prey-shrink confound on the same seeds.

  WA5: verdict distribution-vs-frontier-bank, NEVER same-seed paired.
  Use clean_seeds() (blind-born != PermanentPeace) before reading the
  verdict mix — PermanentPeace is first in the verdict chain and overrides
  cycled even when boom-bust is live (diagnostics.rs:288).

  Usage:
      python3 python/analysis/wa3_wa5_joint.py <bazaar-out-dir> \\
          --frontier-dir <frontier-out-dir> [--seeds 7 11 13 ...]
  """

  import argparse
  import json
  import pathlib
  import sys
  from collections import Counter

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
  from sweep_trophic import META_RE, RESULT_RE


  SEEDS = [
      7, 11, 13, 23, 29, 31, 37, 41, 42, 43,
      47, 53, 57, 59, 61, 67, 71, 73, 99, 101,
  ]


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def own_trade_share_milli(windows, n_haulers):
      """Fraction of (own-trade sells) / (own-trade sells + contract delivers),
      milli, FLOOR. Returns 0 if no activity.

      WA3 signal: starts near 0 (all craft broke, wage mode) and rises as
      capital accumulates (rich craft go own-trade). The joint WA5 read:
      if this rises AND PermanentPeace appears on the same seeds, that is the
      prey-shrink confound (panel warning).
      """
      total_trade = sum(w.get("trade_sold_count", 0) for w in windows)
      total_contract = sum(w.get("laden_trips", 0) for w in windows)
      denom = total_trade + total_contract
      if denom == 0:
          return 0
      return total_trade * 1000 // denom


  def clean_seeds(cells):
      """CLEAN = blind-born (or baseline) verdict != PermanentPeace.

      Cells is dict[seed] -> {verdict, ...}.
      PermanentPeace is first in the verdict chain and overrides cycled
      (diagnostics.rs:288) — seeds where war ended before market dynamics
      play out contaminate the WA5 distribution.
      """
      return [s for s, c in sorted(cells.items()) if c["verdict"] != "PermanentPeace"]


  def prey_shrink_confound_seeds(bazaar_cells, threshold_milli=500):
      """Seeds where own-trade share >= threshold AND verdict == PermanentPeace.

      These are the prey-shrink confound seeds: the bazaar's own-trade share
      shrank the pool of crate-hauler targets, causing PermanentPeace through
      prey depletion rather than predator extinction. RECORDED, not a gate;
      owner judges whether to tune the capitalization curve or accept the read.
      """
      return [
          s for s, c in sorted(bazaar_cells.items())
          if c.get("verdict") == "PermanentPeace"
          and c.get("own_trade_share_milli", 0) >= threshold_milli
      ]


  def verdict_distributions(bazaar_bag, frontier_bag):
      """Compare verdict distributions as independent sample bags (NEVER same-seed paired).

      WA5 is a DISTRIBUTION comparison, not a per-seed comparison. The two
      bags are drawn from independent campaigns; the caller must not pair them
      by seed. Returns {bazaar: Counter, frontier: Counter}.
      """
      return {
          "bazaar": dict(Counter(bazaar_bag)),
          "frontier": dict(Counter(frontier_bag)),
      }


  def load_cell_stdout(path):
      """Parse one stdout file for RESULT + META."""
      result = meta = None
      for line in path.read_text().splitlines():
          stripped = line.strip()
          m = RESULT_RE.match(stripped)
          if m:
              result = m.groupdict()
          m = META_RE.match(stripped)
          if m:
              meta = m.groupdict()
      return result, meta


  def load_windows(path):
      return [r for r in load(path) if "tick" in r]


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("bazaar_dir", help="bazaar sweep output directory")
      ap.add_argument("--frontier-dir", help="frontier bank sweep directory for WA5")
      ap.add_argument("--arm", default="baseline",
                      help="stdout arm prefix (default: baseline)")
      ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
      ap.add_argument("--prey-shrink-threshold-milli", type=int, default=500,
                      help="own-trade share threshold for prey-shrink confound flag")
      args = ap.parse_args()

      bazaar_dir = pathlib.Path(args.bazaar_dir)

      # --- WA3: own-trade share per seed (bazaar) ---
      print(
          "WA3+WA5 JOINT READ (panel directive): own-trade share IS the pirate "
          "food supply — read side-by-side always (PDR-0006: RECORDED, NEVER GATED)"
      )
      print()

      bazaar_cells = {}
      for seed in args.seeds:
          stdout_path = bazaar_dir / f"{args.arm}_s{seed}.stdout"
          jsonl_path = bazaar_dir / f"{args.arm}_s{seed}.jsonl"
          if not stdout_path.exists() or not jsonl_path.exists():
              continue
          result, meta = load_cell_stdout(stdout_path)
          if result is None or meta is None:
              continue
          windows = load_windows(jsonl_path)
          n_haulers = int(meta.get("haulers", 0))
          share = own_trade_share_milli(windows, n_haulers)
          bazaar_cells[seed] = {
              "verdict": result["verdict"],
              "own_trade_share_milli": share,
              "robs": int(result.get("robs", 0)),
              "trips": int(result.get("trips", 0)),
          }

      print("WA3 own-trade share per seed (bazaar, 50k ticks):")
      print(f"  {'seed':>6}  {'verdict':>20}  {'trade_share‰':>12}  "
            f"{'robs':>6}  {'trips':>6}")
      for seed, c in sorted(bazaar_cells.items()):
          print(
              f"  {seed:>6}  {c['verdict']:>20}  {c['own_trade_share_milli']:>12}  "
              f"  {c['robs']:>4}  {c['trips']:>5}"
          )

      # Prey-shrink confound check
      confound = prey_shrink_confound_seeds(bazaar_cells, args.prey_shrink_threshold_milli)
      if confound:
          print(
              f"\n  PREY-SHRINK CONFOUND WARNING (panel directive): seeds {confound} "
              f"show PermanentPeace WITH own-trade share >= "
              f"{args.prey_shrink_threshold_milli}‰ — "
              "this is NOT a finding that bazaar killed boom-bust; it means the "
              "own-trade share shrank the crate-hauler prey pool. Owner's call: "
              "tune capitalization curve, or accept read as 'prey-limited regime'."
          )
      else:
          print(
              f"\n  Prey-shrink confound (threshold {args.prey_shrink_threshold_milli}‰): "
              "none detected on these seeds."
          )

      # --- WA5: verdict distribution vs frontier bank ---
      print()
      clean = clean_seeds(bazaar_cells)
      dirty = [s for s in args.seeds if s in bazaar_cells and s not in clean]
      bazaar_bag = [bazaar_cells[s]["verdict"] for s in clean if s in bazaar_cells]
      print(
          f"WA5 trophic preservation — clean seeds (bazaar baseline != PermanentPeace): "
          f"{len(clean)}/{len(bazaar_cells)}; excluded={dirty}"
      )
      print(f"  bazaar verdict distribution (clean, 50k): {Counter(bazaar_bag)}")

      if args.frontier_dir:
          frontier_dir = pathlib.Path(args.frontier_dir)
          frontier_bag = []
          for seed in args.seeds:
              stdout_path = frontier_dir / f"baseline_s{seed}.stdout"
              if not stdout_path.exists():
                  continue
              result, _ = load_cell_stdout(stdout_path)
              if result is not None and result["verdict"] != "PermanentPeace":
                  frontier_bag.append(result["verdict"])
          dist = verdict_distributions(bazaar_bag, frontier_bag)
          print(f"  frontier verdict distribution (bank, 50k): {Counter(frontier_bag)}")
          print(
              f"\n  WA5 reading: distribution-vs-distribution (NEVER same-seed paired). "
              "Comparable Alive fractions = trophic dynamics preserved through "
              "the demand-mechanism swap. Divergence = the market changed piracy ecology. "
              "Either is a finding (PDR-0006 — owner's call)."
          )
          # Alive fraction comparison
          def alive_frac(bag):
              if not bag:
                  return None
              return bag.count("Alive") * 1000 // len(bag)
          ba = alive_frac(bazaar_bag)
          fa = alive_frac(frontier_bag)
          print(f"  Alive‰: bazaar={ba} frontier={fa}")
      else:
          print("  (no --frontier-dir; WA5 distribution comparison skipped)")


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa3_wa5_joint.py -v 2>&1 | tail -20
  ```

  Expected:
  ```
  test_own_trade_share_zero_when_no_trades PASSED
  test_own_trade_share_is_trade_over_trade_plus_contract PASSED
  test_clean_seeds_filters_permanent_peace PASSED
  test_prey_shrink_confound_flagged PASSED
  test_prey_shrink_confound_empty_when_no_pp PASSED
  test_verdict_distribution_never_same_seed_paired PASSED
  test_wa5_output_has_wa3_column PASSED
  7 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa3_wa5_joint.py python/tests/test_wa3_wa5_joint.py
  git commit -F - <<'EOF'
  feat(lab): WA3+WA5 joint reader — own-trade share, prey-shrink confound, verdict distribution

  Encodes the panel's mandatory joint read: WA3 own-trade share and WA5 verdict
  distribution must be read side-by-side because own-trade share IS the pirate
  food supply (prey-shrink / PermanentPeace masquerade). clean_seeds filters
  PermanentPeace before the WA5 distribution read (diagnostics.rs:288 precedent).
  WA5 is distribution-vs-frontier-bank, NEVER same-seed paired (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.4: WA4 emergent-tanker reader

**Files**

- Create: `python/analysis/wa4_tanker.py`
- Create: `python/tests/test_wa4_tanker.py`

**Steps**

- [ ] **Step 1: Failing test**

  `python/tests/test_wa4_tanker.py`:

  ```python
  """WA4 emergent-tanker reader pins (spec §5 WA4; windows, not gates).

  WA4: fuel packages to non-refinery stations appear with zero fuel-specific
  dispatch code. The read: join gossip-log 'post' rows where good==FUEL_GOOD
  (good index from the META 'goods=' tail; defaults to 1 = the Fuel slot in
  scenario_bazaar) to the destination station, then filter out the three
  refinery stations. Any such package = a tanker event. The first tanker
  contract sequence (post → accept → deliver) is the console chronicle arc.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa4_tanker


  REFINERY_STATIONS = {2, 5, 9}  # scenario_bazaar refinery positions


  def _post(contract, tick, route, good, to_station):
      return {"e": "post", "tick": tick, "contract": contract,
              "route": route, "good": good, "to_station": to_station}


  def _accept(contract, tick, hauler):
      return {"e": "accept", "tick": tick, "contract": contract, "hauler": hauler}


  def _deliver(contract, tick):
      return {"e": "deliver", "tick": tick, "contract": contract}


  def test_tanker_detected_on_non_refinery_fuel_delivery():
      rows = [
          _post(1, 100, 3, 1, 4),   # good=1 (Fuel), to_station=4 (not refinery)
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert len(tankers) == 1
      assert tankers[0]["contract"] == 1
      assert tankers[0]["to_station"] == 4


  def test_no_tanker_when_fuel_goes_to_refinery():
      rows = [
          _post(1, 100, 3, 1, 2),   # good=1 (Fuel), to_station=2 (refinery)
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []


  def test_no_tanker_when_non_fuel_good_to_non_refinery():
      rows = [
          _post(1, 100, 3, 3, 4),   # good=3 (not Fuel), to_station=4
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []


  def test_first_tanker_is_earliest_by_post_tick():
      rows = [
          _post(2, 500, 5, 1, 3),   # later fuel tanker
          _accept(2, 600, 8),
          _deliver(2, 900),
          _post(1, 100, 3, 1, 4),   # earlier fuel tanker
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert len(tankers) == 2
      first = wa4_tanker.first_tanker(tankers)
      assert first["contract"] == 1  # earliest post_tick


  def test_tanker_undelivered_not_counted():
      # post + accept but no deliver = in-flight, not a confirmed tanker event
      rows = [
          _post(1, 100, 3, 1, 4),
          _accept(1, 200, 7),
          # no deliver
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa4_tanker.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa4_tanker'
  ```

- [ ] **Step 2: Write `python/analysis/wa4_tanker.py`**

  ```python
  """wa4_tanker — WA4 emergent-tanker reader (spec §5 WA4).

  FRAME (PDR-0006): windows, not gates.
  Signal: fuel packages posted to non-refinery stations, with zero
  fuel-specific dispatch code. The tanker is emergent because the poster
  (the Exchange corp) computes the same spread-clearing trigger for Fuel as
  for any other good — the refinery-to-rim price gradient is what makes the
  economics work.

  The read: gossip-log post rows where good == fuel_good_index AND
  to_station ∉ refinery_stations, joined to accept + deliver rows for
  completion confirmation. The fuel_good_index is read from the META goods=
  tail field (A0 instrument), defaulting to 1 (Fuel slot in scenario_bazaar).

  Usage:
      python3 python/analysis/wa4_tanker.py <gossip.jsonl> \\
          [--fuel-good 1] [--refinery-stations 2 5 9]
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def find_tankers(rows, fuel_good, refinery_stations):
      """Return completed fuel-package deliveries to non-refinery stations.

      A 'tanker event' = a contract where:
        - the 'post' row has good == fuel_good
        - the destination station is not a refinery
        - the contract has both an 'accept' and a 'deliver' row (completed)

      Returns list of dicts: {contract, post_tick, accept_tick, deliver_tick,
      to_station, hauler}.
      """
      posts = {}
      accepts = {}
      delivers = {}
      for row in rows:
          e = row.get("e")
          c = row.get("contract")
          if c is None:
              continue
          if e == "post":
              posts[c] = row
          elif e == "accept":
              accepts[c] = row
          elif e == "deliver":
              delivers[c] = row

      result = []
      for c, p in posts.items():
          if p.get("good") != fuel_good:
              continue
          if p.get("to_station") in refinery_stations:
              continue
          d = delivers.get(c)
          if d is None:
              continue  # in-flight, not confirmed
          a = accepts.get(c)
          result.append({
              "contract": c,
              "post_tick": p.get("tick", 0),
              "accept_tick": a["tick"] if a else None,
              "deliver_tick": d.get("tick", 0),
              "to_station": p.get("to_station"),
              "hauler": a["hauler"] if a else None,
              "route": p.get("route"),
          })

      result.sort(key=lambda x: x["post_tick"])
      return result


  def first_tanker(tankers):
      """The first (earliest post_tick) confirmed tanker event."""
      if not tankers:
          return None
      return min(tankers, key=lambda x: x["post_tick"])


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
      ap.add_argument("--fuel-good", type=int, default=1,
                      help="good index for Fuel (default 1; read from META goods= tail "
                           "when the A0 instrument is present)")
      ap.add_argument("--refinery-stations", type=int, nargs="+", default=[2, 5, 9],
                      help="station indices that are refineries in scenario_bazaar "
                           "(default: 2 5 9; read from META when the A0 instrument "
                           "carries station roles)")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      refinery_set = set(args.refinery_stations)

      tankers = find_tankers(rows, fuel_good=args.fuel_good,
                             refinery_stations=refinery_set)

      print(
          f"WA4 emergent tankers (fuel good={args.fuel_good}, "
          f"refinery stations={sorted(refinery_set)}) "
          "(RECORDED, never gated — PDR-0006):"
      )
      if not tankers:
          print(
              "  WA4 reading: NoTanker — no completed fuel packages to non-refinery "
              "stations observed. Either the fuel price gradient is too flat to clear "
              "the arbitrage trigger, or too few ticks. Recorded as a finding: "
              "the tanker is the WA4 test of price-driven emergence — its absence "
              "is equally informative (PDR-0006)."
          )
          return

      first = first_tanker(tankers)
      print(
          f"  WA4 reading: Tanker — {len(tankers)} confirmed fuel packages to "
          f"non-refinery stations."
      )
      print(
          f"  First tanker: contract={first['contract']} "
          f"post_tick={first['post_tick']} accept_tick={first['accept_tick']} "
          f"deliver_tick={first['deliver_tick']} "
          f"to_station={first['to_station']} hauler={first['hauler']}"
      )
      print(
          "  (zero fuel-specific dispatch code — pure price-driven emergence; "
          "the console chronicle arc starts here)"
      )
      print()
      print(f"  {'contract':>10}  {'post_tick':>9}  {'deliver_tick':>12}  "
            f"{'to_station':>10}  {'hauler':>6}  {'route':>5}")
      for t in tankers:
          print(
              f"  {t['contract']:>10}  {t['post_tick']:>9}  {t['deliver_tick']:>12}  "
              f"  {t['to_station']:>9}  {t['hauler']!s:>6}  {t['route']!s:>5}"
          )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa4_tanker.py -v 2>&1 | tail -15
  ```

  Expected:
  ```
  test_tanker_detected_on_non_refinery_fuel_delivery PASSED
  test_no_tanker_when_fuel_goes_to_refinery PASSED
  test_no_tanker_when_non_fuel_good_to_non_refinery PASSED
  test_first_tanker_is_earliest_by_post_tick PASSED
  test_tanker_undelivered_not_counted PASSED
  5 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa4_tanker.py python/tests/test_wa4_tanker.py
  git commit -F - <<'EOF'
  feat(lab): WA4 emergent-tanker reader — fuel packages to non-refinery stations, post→deliver join

  Joins gossip-log post/accept/deliver rows per contract; selects completed Fuel
  packages whose destination is not a refinery station. Any such event is the
  emergent tanker: zero fuel-specific dispatch code, pure price-driven emergence.
  Absence is equally a finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.5: Rung-A BAZAAR anchored line + per-good route-concentration panel

**Files**

- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (BAZAAR anchored line + JSONL per_station_stock/price)
- Modify: `python/analysis/sweep_trophic.py` (BAZAAR_RE + ANCHORED entry, same commit — lockstep rule)
- Modify: `python/tests/test_sweep_parsing.py` (V5 fixture, same commit — lockstep rule)
- Create: `python/analysis/wa_route_concentration.py`
- Create: `python/tests/test_wa_route_concentration.py`

**Steps**

- [ ] **Step 1: Write the failing parsing test for V5 (BAZAAR line)**

  In `python/tests/test_sweep_parsing.py`, append after the existing
  `test_v4_fuel_line_stranding_tail_parses_and_older_tails_read_none` test.
  The V5_STDOUT fixture uses a V4 base (frontier scenario with strandings):

  ```python
  V5_STDOUT = V4_STDOUT + (
      "BAZAAR seed=7 goods=10 exchange_drain_micros=-12345678 "
      "trade_sold=42 trade_bought=38 packages_posted=27 packages_delivered=24 "
      "own_trade_share_milli=350\n"
  )


  def test_v5_bazaar_line_parses_and_older_stdout_reads_none():
      parsed = sweep.parse_stdout(V5_STDOUT)
      assert parsed["bazaar"] is not None
      assert parsed["bazaar"]["goods"] == "10"
      assert parsed["bazaar"]["exchange_drain_micros"] == "-12345678"
      assert parsed["bazaar"]["trade_sold"] == "42"
      assert parsed["bazaar"]["own_trade_share_milli"] == "350"
      for legacy_text in (V1_STDOUT, V2_STDOUT, V3_STDOUT, V4_STDOUT):
          legacy = sweep.parse_stdout(legacy_text)
          assert legacy["bazaar"] is None
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v5_bazaar_line_parses_and_older_stdout_reads_none -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  KeyError: 'bazaar'
  ```
  (The `ANCHORED` dict doesn't have a `bazaar` key yet.)

- [ ] **Step 2: Write failing test for route-concentration panel**

  `python/tests/test_wa_route_concentration.py`:

  ```python
  """Per-good route-concentration panel pins (L4-C3/L5-C3 fix).

  Computed script-side from the gossip-log accept-row resource key.
  Route vectors have no goods dimension in TrophicSample (grounding §4);
  per-good HHI is derived from the accept rows, never from new per-good
  route vectors.

  Run in the same campaign as the WA5 threshold fit so the open/closed margin
  is interpretable.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa_route_concentration


  def _accept(route, good, hauler=0, tick=100):
      return {"e": "accept", "tick": tick, "route": route,
              "hauler": hauler, "resource": good}


  def test_per_good_hhi_uniform_routes():
      # 4 accepts on 4 different routes for good 0: perfectly distributed = low HHI
      rows = [_accept(r, 0) for r in range(4)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      assert 0 in result
      # each route has share 1/4; HHI = 4*(1/4)^2 = 1/4 = 250 milli
      assert result[0] == 250


  def test_per_good_hhi_concentrated_routes():
      # All 4 accepts on route 0 for good 1: fully concentrated = HHI 1000
      rows = [_accept(0, 1) for _ in range(4)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      assert 1 in result
      assert result[1] == 1000


  def test_per_good_hhi_excludes_unoccupied_routes():
      # 2 routes occupied by good 2
      rows = [_accept(0, 2, tick=100), _accept(0, 2, tick=101), _accept(3, 2, tick=102)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      # route 0: 2 accepts, route 3: 1 accept; HHI = (4+1)*1000/9 = 555
      assert 2 in result
      assert result[2] == (4 + 1) * 1000 // 9


  def test_good_not_in_accepts_excluded():
      rows = [_accept(0, 0)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=2)
      assert 1 not in result  # good 1 never accepted


  def test_ensemble_good_hhi_quartiles():
      # Two seeds, good 0: seed1 HHI 250, seed2 HHI 1000
      per_seed = {7: {0: 250}, 11: {0: 1000}}
      q = wa_route_concentration.ensemble_quartiles(per_seed, good=0)
      assert q is not None
      assert q[0] <= q[1] <= q[2]
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa_route_concentration.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa_route_concentration'
  ```

- [ ] **Step 3: Add BAZAAR anchored line to `trophic_run.rs` and add JSONL fields**

  The BAZAAR line and JSONL per_station_stock/price additions must land in the
  SAME commit as the BAZAAR_RE and V5 fixture (lockstep rule).

  In `trophic_run.rs`, in `sample_json` (after the existing `refuel_spend_micros`
  key, around line 302), add the per_station_stock and per_station_price fields
  following the additive block pattern:

  ```rust
          // --- bazaar lab fields (rung A, spec §5 WA1-4; windows, not gates).
          // Additive: every pre-bazaar key above is byte-untouched.
          // per_station_stock / per_station_price: n_stations × n_goods flat
          // matrices (station-major, good-minor). Existing
          // per_station_fuel_stock / per_station_fuel_price stay byte-identical.
          "per_station_stock": s.per_station_stock,
          "per_station_price": s.per_station_price,
          "trade_sold_count": s.trade_sold_count,
          "trade_bought_count": s.trade_bought_count,
  ```

  In `diagnostics.rs` `TrophicSample` struct, add after the last field of the
  world-gets-big group (after `refuel_spend_micros`, line ~234):

  ```rust
      // --- bazaar lab fields (rung A, spec §5 WA1-5; windows, not gates).
      // Additive: every pre-bazaar JSONL key is untouched. ---
      /// Per-station × per-good stock snapshot at the sample point.
      /// Flat matrix: station-major, good-minor. Length = n_stations × n_goods.
      /// Zero-len when the scenario has no bazaar goods (trophic/frontier
      /// are byte-identical: the field is present but empty — additive).
      pub per_station_stock: Vec<i64>,
      /// Per-station × per-good price snapshot. Same shape as per_station_stock.
      pub per_station_price: Vec<i64>,
      /// TradeSold events in the window (own-cargo sells, WA3 numerator).
      pub trade_sold_count: u32,
      /// TradeBought events in the window.
      pub trade_bought_count: u32,
  ```

  In `sample_window` (`diagnostics.rs:541`), populate these fields from the
  event stream (TradeSold/TradeBought counts, window-filtered) and from the
  world's station stock/price columns at the sample tick. When the scenario has
  no goods beyond Ore/Fuel, the Vec is empty — that is correct: trophic and
  frontier produce zero-length per_station_stock, which is byte-identical in
  their JSONL (field present, value `[]`).

  In `trophic_run.rs`, after the FUEL line (around line 788), add the BAZAAR
  anchored line. It is config-gated: only print when the world has more than 2
  goods (i.e., scenario_bazaar). In v1 the check is `n_goods > 2`; trophic and
  frontier both have n_goods=2 and thus never print BAZAAR.

  ```rust
      // The BAZAAR line (rung A, spec §5; a window, not a gate).
      // Config-gated: only printed when the scenario has bazaar goods (n_goods>2).
      // Lockstep rule: BAZAAR_RE in sweep_trophic.py lands in the SAME commit.
      let n_goods = world.n_goods(); // method on World returning GoodsCfg count
      if n_goods > 2 {
          let trade_sold_total: u64 = samples.iter().map(|s| u64::from(s.trade_sold_count)).sum();
          let trade_bought_total: u64 = samples.iter().map(|s| u64::from(s.trade_bought_count)).sum();
          let packages_posted: u64 = samples.iter().map(|s| u64::from(s.packages_posted)).sum();
          let packages_delivered: u64 = samples.iter().map(|s| u64::from(s.laden_trips)).sum();
          let total_trips = trade_sold_total + packages_delivered;
          let own_trade_share_milli = if total_trips > 0 {
              trade_sold_total.saturating_mul(1000) / total_trips
          } else {
              0
          };
          // Exchange drain: sum of per-window exchange_treasury_micros deltas.
          // Negative = net outflow from the Exchange (the solvency honesty read,
          // OD-2; printed as a standing read, not a gate).
          let exchange_drain = exchange_drain_micros(&samples);
          println!(
              "BAZAAR seed={} goods={} exchange_drain_micros={} \
               trade_sold={} trade_bought={} packages_posted={} \
               packages_delivered={} own_trade_share_milli={}",
              args.seed, n_goods, exchange_drain,
              trade_sold_total, trade_bought_total,
              packages_posted, packages_delivered, own_trade_share_milli,
          );
      }
  ```

  The `exchange_drain_micros` helper computes the run-cumulative Exchange
  treasury delta from the sample sequence:

  ```rust
  fn exchange_drain_micros(samples: &[TrophicSample]) -> i64 {
      // Sum of per-window exchange treasury deltas (negative = drain).
      // Uses the exchange_treasury_micros field added to TrophicSample in A3.
      // Zero sentinel when the field is absent (pre-bazaar scenarios).
      if samples.is_empty() {
          return 0;
      }
      let first = samples.first().map_or(0, |s| s.exchange_treasury_micros);
      let last = samples.last().map_or(0, |s| s.exchange_treasury_micros);
      last - first
  }
  ```

  **Note to builder:** `exchange_treasury_micros` is a new field in
  `TrophicSample` that must be added alongside the other bazaar fields. It
  holds the Exchange corp's treasury snapshot per window (A3 unlocks this;
  if A3 has not landed, zero-init the field). Also add `packages_posted` to
  `TrophicSample` (ContractOffered events in the window from the Exchange corp,
  which requires knowing the Exchange corp index — derive from config.

  **After confirming the impl builds**, add the BAZAAR_RE to
  `python/analysis/sweep_trophic.py` in the same commit.

  In `sweep_trophic.py`, add after `FUEL_RE`:

  ```python
  # The BAZAAR line (rung A, spec §5) — config-gated (only when n_goods > 2).
  # Lockstep rule: this regex and the Rust println! land in the SAME commit.
  # Optional: absent from trophic/frontier runs, reads None in parse_stdout.
  BAZAAR_RE = re.compile(
      r"^BAZAAR seed=(?P<seed>\d+) goods=(?P<goods>\d+) "
      r"exchange_drain_micros=(?P<exchange_drain_micros>-?\d+) "
      r"trade_sold=(?P<trade_sold>\d+) trade_bought=(?P<trade_bought>\d+) "
      r"packages_posted=(?P<packages_posted>\d+) "
      r"packages_delivered=(?P<packages_delivered>\d+) "
      r"own_trade_share_milli=(?P<own_trade_share_milli>\d+)$"
  )
  ```

  And in the `ANCHORED` dict, add:

  ```python
  ANCHORED = {
      "result": (True, RESULT_RE),
      "media": (True, MEDIA_RE),
      "meta": (False, META_RE),
      "fuel": (False, FUEL_RE),
      "bazaar": (False, BAZAAR_RE),  # config-gated: absent on trophic/frontier
  }
  ```

- [ ] **Step 4: Write `python/analysis/wa_route_concentration.py`**

  ```python
  """wa_route_concentration — rung-A per-good route-concentration panel.

  L4-C3 / L5-C3 fix: per-good route traffic and rob HHI beside WA5 verdict,
  from the accept-row 'resource' key in the gossip-log. Route vectors have no
  goods dimension today (grounding §4); this panel is entirely script-side.

  Run in the SAME campaign as the WA5 threshold fit (Part 3, DL5-2) so the
  open/closed margin is interpretable. The clumped-topology factory constraint
  (L1-C2) means goods should travel on a small subset of routes; high HHI per
  good is EXPECTED and is the design proof that clumped topology is working.
  A low HHI (< ~200) means self-averaging has started — the L5-C3 warning.

  Usage:
      python3 python/analysis/wa_route_concentration.py <gossip.jsonl> \\
          [--n-routes N]
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def per_good_hhi_milli(rows, n_routes):
      """Per-good occupied-route HHI (milli), from gossip-log accept rows.

      Only rows with e='accept' and a 'resource' key are counted (A0
      instrument: accept row gains resource+reward). Returns dict[good_index]
      -> HHI_milli for goods with at least one accept.
      """
      counts = {}  # good -> {route -> count}
      for row in rows:
          if row.get("e") != "accept":
              continue
          g = row.get("resource")
          r = row.get("route")
          if g is None or r is None:
              continue
          counts.setdefault(g, {}).setdefault(r, 0)
          counts[g][r] += 1

      result = {}
      for good, route_counts in counts.items():
          total = sum(route_counts.values())
          if total == 0:
              continue
          hhi = sum(c * c for c in route_counts.values()) * 1000 // (total * total)
          result[good] = hhi
      return result


  def ensemble_quartiles(per_seed, good):
      """Lower-index quartiles of per-good HHI across seeds.

      per_seed: dict[seed] -> dict[good -> hhi_milli]
      Returns (q1, median, q3) or None if fewer than 2 seeds have data.
      """
      vals = sorted(v[good] for v in per_seed.values() if good in v)
      n = len(vals)
      if n < 2:
          return None
      return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with accept rows (resource key required)")
      ap.add_argument("--n-routes", type=int, default=90,
                      help="n_stations^2 (default 90 = 10^2 for scenario_bazaar)")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      result = per_good_hhi_milli(rows, n_routes=args.n_routes)

      print(
          f"rung-A per-good route concentration (HHI, milli) "
          f"— {sum(1 for r in rows if r.get('e') == 'accept')} accept events "
          "(RECORDED, never gated — PDR-0006; L4-C3/L5-C3 fix):"
      )
      if not result:
          print(
              "  no accept rows with 'resource' key (pre-A0 run — the A0 instrument "
              "must add resource+reward to the accept row)"
          )
          return

      print(f"  {'good':>4}  {'HHI‰':>6}  reading")
      for good in sorted(result):
          hhi = result[good]
          if hhi >= 600:
              reading = "concentrated (clumped topology working)"
          elif hhi >= 200:
              reading = "moderate"
          else:
              reading = "low (self-averaging warning — L5-C3)"
          print(f"  {good:>4}  {hhi:>6}  {reading}")

      print(
          "\n  Interpretation (panel L4-C3/L5-C3): high HHI per good is EXPECTED "
          "under clumped topology — it is the design proof. Low HHI means a good "
          "is spreading over all routes (self-averaging); increase goods-topology "
          "concentration or run the DL5-2 threshold fit to check margin."
      )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 5: Run all tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py python/tests/test_wa_route_concentration.py -v 2>&1 | tail -30
  ```

  Expected:
  ```
  test_v5_bazaar_line_parses_and_older_stdout_reads_none PASSED
  test_per_good_hhi_uniform_routes PASSED
  test_per_good_hhi_concentrated_routes PASSED
  test_per_good_hhi_excludes_unoccupied_routes PASSED
  test_good_not_in_accepts_excluded PASSED
  test_ensemble_good_hhi_quartiles PASSED
  ```

  Also run Rust tests to confirm the TrophicSample additions compile and the
  JSONL emitter still passes the existing sample_json shape tests:

  ```bash
  cargo test -p jumpgate-core --all-targets -- sample 2>&1 | tail -15
  ```

  Expected: all existing sample tests pass.

- [ ] **Step 6: Commit (LOCKSTEP — Rust println! + Python regex + V5 fixture all in one commit)**

  ```bash
  git add \
    crates/jumpgate-core/examples/trophic_run.rs \
    crates/jumpgate-core/src/diagnostics.rs \
    python/analysis/sweep_trophic.py \
    python/tests/test_sweep_parsing.py \
    python/analysis/wa_route_concentration.py \
    python/tests/test_wa_route_concentration.py
  git commit -F - <<'EOF'
  feat(lab): BAZAAR anchored line + per_station_stock/price JSONL + route-concentration panel

  BAZAAR line (config-gated: n_goods > 2; absent on trophic/frontier) carries
  exchange drain, channel mix counts, and own_trade_share_milli. BAZAAR_RE lands
  in the same commit as the println! (lockstep rule); V5 fixture appended to
  test_sweep_parsing.py. per_station_stock / per_station_price fields added to
  TrophicSample (additive; empty vec on trophic/frontier — byte-identical).
  wa_route_concentration panel computes per-good HHI from gossip-log accept-row
  resource keys (script-side, L4-C3/L5-C3 fix).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.6: Per-station epilogue block + chronicle enrichment

**Files**

- Modify: `crates/jumpgate-core/examples/trophic_run.rs`

**Steps**

- [ ] **Step 1: Write failing test for the per-station epilogue output**

  Add to `crates/jumpgate-core/tests/` or as a doc-test; here we use an
  integration check that the chronicle printer produces per-station rows in the
  JSONL-free path. Because the chronicle is stdout-only, test by running a
  short trophic_run with `--chronicle` and checking the output contains the
  per-station epilogue marker string.

  In `crates/jumpgate-core/tests/chronicle_epilogue.rs`:

  ```rust
  /// Confirm that `--chronicle` output contains per-station epilogue lines.
  /// These are printer-only (PDR-0006: windows, never gates) and must be
  /// present whenever the chronicle flag is set on a scenario with stations.
  #[test]
  fn per_station_epilogue_appears_in_chronicle() {
      use std::process::Command;
      let out = Command::new(env!("CARGO_BIN_EXE_trophic_run"))
          .args([
              "--scenario", "trophic",
              "--seed", "7",
              "--ticks", "500",
              "--chronicle",
          ])
          .output()
          .expect("trophic_run");
      let stdout = String::from_utf8_lossy(&out.stdout);
      // Per-station epilogue block header must appear
      assert!(
          stdout.contains("=== per-station epilogue"),
          "no per-station epilogue in chronicle output:\n{stdout}"
      );
  }
  ```

  Run:
  ```bash
  cargo test -p jumpgate-core --all-targets -- per_station_epilogue 2>&1 | tail -15
  ```

  Expected failure:
  ```
  assertion `left == right` failed: no per-station epilogue in chronicle output
  ```

- [ ] **Step 2: Add per-station epilogue block to `trophic_run.rs`**

  In the `print_chronicle` function in `trophic_run.rs`, after the per-craft
  loop (after the closing `}` of the craft loop around line 611), add the
  per-station epilogue block. It receives a `&[TrophicSample]` reference
  (already available from the caller's `simulate` result):

  ```rust
  /// Per-station summary epilogue in the chronicle (synthesis cut Part 3).
  /// Threaded &[TrophicSample] for the WA1 protagonist (station starvation map).
  /// Printer-side only (PDR-0006: windows, never gates).
  fn print_station_epilogue(world: &World, samples: &[TrophicSample]) {
      let n_stations = world.n_stations();
      if n_stations == 0 || samples.is_empty() {
          return;
      }
      println!("=== per-station epilogue (PDR-0006: recorded, never gated) ===");
      let n_goods = world.n_goods();
      let last = samples.last().unwrap();

      for s in 0..n_stations {
          // Final stock + price snapshot from the last sample window.
          let stock_range: Vec<i64> = if n_goods > 0 && !last.per_station_stock.is_empty() {
              (0..n_goods)
                  .map(|g| last.per_station_stock.get(s * n_goods + g).copied().unwrap_or(0))
                  .collect()
          } else {
              // Pre-bazaar scenario: fall back to fuel-only read
              vec![
                  last.per_station_fuel_stock.get(s).copied().unwrap_or(0),
              ]
          };
          let price_range: Vec<i64> = if n_goods > 0 && !last.per_station_price.is_empty() {
              (0..n_goods)
                  .map(|g| last.per_station_price.get(s * n_goods + g).copied().unwrap_or(0))
                  .collect()
          } else {
              vec![
                  last.per_station_fuel_price.get(s).copied().unwrap_or(0),
              ]
          };
          // Count zero-stock windows per good (WA1 starvation map).
          let zero_runs: Vec<u32> = if n_goods > 0 && !samples[0].per_station_stock.is_empty() {
              (0..n_goods)
                  .map(|g| {
                      samples
                          .iter()
                          .filter(|w| {
                              let idx = s * n_goods + g;
                              w.per_station_stock.get(idx).copied().unwrap_or(0) == 0
                          })
                          .count() as u32
                  })
                  .collect()
          } else {
              vec![]
          };
          // Lurking pirates at this station in the final window.
          let lurking = last.per_station_lurking_pirates.get(s).copied().unwrap_or(0);
          println!(
              "  station {s}: final_stock={stock_range:?} final_price={price_range:?} \
               zero_stock_windows={zero_runs:?} lurking_pirates={lurking}"
          );
      }
  }
  ```

  In `print_chronicle`, call `print_station_epilogue(world, samples)` after the
  per-craft loop. Update the function signature to accept `samples`:

  ```rust
  fn print_chronicle(world: &World, samples: &[TrophicSample], gossip_min_micros: i64) {
      // ...existing per-craft loop unchanged...
      print_station_epilogue(world, samples);
  }
  ```

  In `main`, update the call site:

  ```rust
  if args.chronicle {
      print_chronicle(&world, &samples, args.chronicle_gossip_min_micros);
  }
  ```

- [ ] **Step 3: Run the test**

  ```bash
  cargo test -p jumpgate-core --all-targets -- per_station_epilogue 2>&1 | tail -10
  ```

  Expected:
  ```
  test per_station_epilogue_appears_in_chronicle ... ok
  ```

- [ ] **Step 4: Run full workspace tests to confirm no regressions**

  ```bash
  cargo test --workspace 2>&1 | tail -20
  ```

  Expected: all tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add \
    crates/jumpgate-core/examples/trophic_run.rs \
    crates/jumpgate-core/tests/chronicle_epilogue.rs
  git commit -F - <<'EOF'
  feat(chronicle): per-station epilogue block — starvation map + lurking pirates

  Adds per-station epilogue to the chronicle printer (synthesis cut Part 3):
  final stock/price per good, zero-stock window count per good (WA1 protagonist),
  and lurking pirate count. Threaded &[TrophicSample] into print_chronicle.
  Printer-side only (PDR-0006: windows, never gates).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.7: 20-seed ensemble + rung-A exit digest + console packet

**Files**

- Create: `python/analysis/sweep_bazaar.py` (the WA sweep runner)
- Create: `runs/gag-rung-a/` (builder creates at run time; never staged)
- No Rust changes (pure Python + shell procedure)

**Steps**

- [ ] **Step 1: Write the sweep runner**

  `python/analysis/sweep_bazaar.py`:

  ```python
  """sweep_bazaar — rung-A WA1-5 20-seed campaign runner.

  FRAME (PDR-0006): every number here is a designer's window, never a gate.
  Runs the bazaar scenario over the 20 SEEDS at both run lengths (50k for WA5
  bank-comparability; 100k for per-good/WA1-4 reads), aggregates the
  readers (WA1 survival, WA2 spread closure, WA3+WA5 joint, WA4 tanker,
  per-good route concentration), and prints the rung-A console packet.

  The 50k run is bank-comparable with frontier (WA5 distribution). The 100k
  run carries the per-good reads. Both use the 20-seed W4 SEEDS ladder.

  Exit condition (RECORDED, NEVER GATED — PDR-0006):
    1. sha256 digest of trophic + frontier vs the A0 baseline (from A6.0).
    2. WA1-5 readings printed.
    3. First-look chronicle materials banked to runs/gag-rung-a/.
  Any divergence in step 1 is a determinism break — STOP, bisect, never rationalize.

  Usage:
      python3 python/analysis/sweep_bazaar.py \\
          --out runs/gag-rung-a \\
          --baseline-dir runs/gag-a6-baseline \\
          [--seeds 7 11 13] [--ticks-short 50000] [--ticks-long 100000]
  """

  import argparse
  import hashlib
  import json
  import pathlib
  import subprocess
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
  from sweep_trophic import META_RE, RESULT_RE, parse_stdout, runner_cmd
  from w4_grid import SEEDS, quartiles, clean_seeds as w4_clean_seeds
  from wa1_survival import stock_runs, stalled_consumers, read_transport_table
  from wa2_spread_closure import spread_series, summarize as wa2_summarize
  from wa3_wa5_joint import (
      own_trade_share_milli,
      clean_seeds as wa3_clean_seeds,
      prey_shrink_confound_seeds,
      verdict_distributions,
      load_cell_stdout,
  )
  from wa4_tanker import find_tankers, first_tanker
  from wa_route_concentration import per_good_hhi_milli


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def sha256_file(path):
      h = hashlib.sha256()
      with open(path, "rb") as f:
          for chunk in iter(lambda: f.read(65536), b""):
              h.update(chunk)
      return h.hexdigest()


  def run_one_seed(scenario, seed, ticks, out_dir, arm="baseline"):
      """Run trophic_run for one (scenario, seed, ticks) cell."""
      jsonl = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.jsonl"
      gossip = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.gossip.jsonl"
      stdout_path = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.stdout"

      if stdout_path.exists() and jsonl.exists() and gossip.exists():
          # Already banked; skip re-run (idempotent sweep).
          stdout_text = stdout_path.read_text()
      else:
          cmd = runner_cmd(scenario, seed, ticks, jsonl, [])
          cmd += ["--gossip-log", str(gossip), "--chronicle"]
          proc = subprocess.run(cmd, capture_output=True, text=True)
          if proc.returncode != 0:
              sys.stderr.write(proc.stdout + proc.stderr)
              raise SystemExit(f"run failed: {scenario} seed={seed} ticks={ticks}")
          stdout_path.write_text(proc.stdout)
          stdout_text = proc.stdout

      parsed = parse_stdout(stdout_text)
      all_rows = load(jsonl)
      windows = [r for r in all_rows if "tick" in r]
      gossip_rows = load(gossip)
      return parsed, windows, gossip_rows, jsonl, gossip


  def verify_digest_vs_baseline(baseline_dir, out_dir, seeds=(7, 23)):
      """Cross-branch digest: trophic + frontier vs A0 baseline.

      Compares sha256 of stdout + JSONL + gossip-log for seeds 7 and 23
      at 2000 ticks (the same cells pinned in A6.0). Any divergence is a
      determinism break — print and abort; never rationalize.
      """
      print("\n=== rung-A exit digest (A0 baseline vs HEAD) ===")
      ok = True
      for scenario in ("trophic", "frontier"):
          for s in seeds:
              base = pathlib.Path(baseline_dir)
              head_dir = out_dir
              # Re-run at 2000 ticks for the digest comparison
              for ext in ("out", "jsonl", "gossip.jsonl"):
                  base_f = base / f"{scenario}-base-s{s}.{ext}"
                  head_f = head_dir / f"digest-{scenario}_s{s}_t2000.{ext}"
                  if not base_f.exists():
                      print(f"  SKIP {base_f.name} (no baseline — run A6.0 first)")
                      continue
                  if not head_f.exists():
                      # Need to produce the head file
                      jsonl_h = head_dir / f"digest-{scenario}_s{s}_t2000.jsonl"
                      gossip_h = head_dir / f"digest-{scenario}_s{s}_t2000.gossip.jsonl"
                      stdout_h = head_dir / f"digest-{scenario}_s{s}_t2000.out"
                      cmd = runner_cmd(scenario, s, 2000, jsonl_h, [])
                      cmd += ["--gossip-log", str(gossip_h)]
                      proc = subprocess.run(cmd, capture_output=True, text=True)
                      if proc.returncode != 0:
                          raise SystemExit(f"digest run failed: {scenario} seed={s}")
                      stdout_h.write_text(proc.stdout)
                  base_digest = sha256_file(base_f)
                  head_f_resolved = head_dir / f"digest-{scenario}_s{s}_t2000.{ext}"
                  head_digest = sha256_file(head_f_resolved)
                  match = "OK" if base_digest == head_digest else "DIVERGE"
                  print(f"  {match}  {scenario} s={s} {ext}: "
                        f"base={base_digest[:12]}... head={head_digest[:12]}...")
                  if match != "OK":
                      ok = False
      if not ok:
          print("\n  DETERMINISM BREAK: one or more files diverged from the A0 baseline.")
          print("  STOP — bisect commit-by-commit. Do NOT rationalize.")
      else:
          print("  All digest checks pass — rung-A mechanics are behavior-equivalent "
                "on trophic and frontier (the OD-1 hash-neutrality confirmation).")
      return ok


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("--out", default="runs/gag-rung-a")
      ap.add_argument("--baseline-dir", default="runs/gag-a6-baseline")
      ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
      ap.add_argument("--ticks-short", type=int, default=50_000,
                      help="50k for WA5 bank-comparability")
      ap.add_argument("--ticks-long", type=int, default=100_000,
                      help="100k for per-good reads (WA1-4)")
      ap.add_argument("--frontier-baseline-dir",
                      help="frontier bank sweep dir for WA5 comparison")
      args = ap.parse_args()

      out_dir = pathlib.Path(args.out)
      out_dir.mkdir(parents=True, exist_ok=True)

      print(
          "sweep_bazaar rung-A exit campaign "
          f"(PDR-0006: RECORDED, NEVER GATED) — "
          f"seeds={len(args.seeds)} ticks_short={args.ticks_short} "
          f"ticks_long={args.ticks_long}"
      )

      # ---- Step 1: Behavior digest ----
      ok = verify_digest_vs_baseline(
          args.baseline_dir, out_dir, seeds=(7, 23)
      )
      if not ok:
          raise SystemExit(1)

      # ---- Step 2: 50k ensemble (WA5 bank-comparability) ----
      print("\n=== 50k ensemble (20 seeds, WA5 verdict distribution) ===")
      cells_50k = {}
      for seed in args.seeds:
          parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
              "bazaar", seed, args.ticks_short, out_dir
          )
          haulers = int(parsed["meta"]["haulers"]) if parsed.get("meta") else 0
          share = own_trade_share_milli(windows, haulers)
          cells_50k[seed] = {
              "verdict": parsed["result"]["verdict"] if parsed.get("result") else "Unknown",
              "own_trade_share_milli": share,
              "robs": int(parsed["result"].get("robs", 0)) if parsed.get("result") else 0,
              "trips": int(parsed["result"].get("trips", 0)) if parsed.get("result") else 0,
              "windows": windows,
              "gossip": gossip_rows,
          }
          print(
              f"  seed={seed}: verdict={cells_50k[seed]['verdict']} "
              f"robs={cells_50k[seed]['robs']} trips={cells_50k[seed]['trips']} "
              f"own_trade‰={share}"
          )

      # WA3+WA5 joint
      confound = prey_shrink_confound_seeds(cells_50k, threshold_milli=500)
      if confound:
          print(f"\n  PREY-SHRINK CONFOUND WARNING: seeds {confound} — "
                "PermanentPeace with high own-trade share. See wa3_wa5_joint for detail.")
      clean = wa3_clean_seeds(cells_50k)
      bazaar_bag = [cells_50k[s]["verdict"] for s in clean]
      print(f"\nWA5 verdict distribution (clean seeds, n={len(clean)}): "
            f"{dict(sorted((v, bazaar_bag.count(v)) for v in set(bazaar_bag)))}")

      if args.frontier_baseline_dir:
          frontier_bag = []
          fdir = pathlib.Path(args.frontier_baseline_dir)
          for p in sorted(fdir.glob("baseline_s*.stdout")):
              result, _ = load_cell_stdout(p)
              if result and result["verdict"] != "PermanentPeace":
                  frontier_bag.append(result["verdict"])
          dist = verdict_distributions(bazaar_bag, frontier_bag)
          print(f"WA5 frontier distribution (bank): "
                f"{dict(sorted((v, frontier_bag.count(v)) for v in set(frontier_bag)))}")
          alive_b = bazaar_bag.count("Alive") * 1000 // max(len(bazaar_bag), 1)
          alive_f = frontier_bag.count("Alive") * 1000 // max(len(frontier_bag), 1)
          print(f"WA5 Alive‰: bazaar={alive_b} frontier={alive_f} "
                "(distribution-vs-distribution, NEVER same-seed paired)")

      # ---- Step 3: 100k ensemble (WA1-4 per-good reads) ----
      print("\n=== 100k ensemble (20 seeds, WA1-4 per-good reads) ===")
      all_hhi = {}
      tanker_total = 0
      first_tanker_global = None

      for seed in args.seeds:
          parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
              "bazaar", seed, args.ticks_long, out_dir
          )
          haulers = int(parsed["meta"]["haulers"]) if parsed.get("meta") else 0
          n_stations = int(parsed["meta"]["stations"]) if parsed.get("meta") else 10
          n_goods = int(parsed["bazaar"]["goods"]) if parsed.get("bazaar") else 0

          # WA1: survival
          if n_goods > 0:
              runs = stock_runs(windows, n_stations, n_goods)
              starving = [r for r in runs if r["max_zero_run"] > 0]
              print(
                  f"  WA1 seed={seed}: {len(starving)}/{n_stations * n_goods} "
                  f"station×good pairs had zero-stock windows"
              )

          # WA2: spread closure
          s_result = spread_series(gossip_rows)
          wa2_rows = wa2_summarize(s_result)
          decaying = sum(1 for r in wa2_rows if r["decaying"] is True)
          print(
              f"  WA2 seed={seed}: {decaying}/{len(wa2_rows)} route×good pairs "
              "show spread decay"
          )

          # WA4: tankers
          fuel_good = 1  # Fuel slot in scenario_bazaar
          refinery_set = {2, 5, 9}  # OD-3 refinery stations in scenario_bazaar
          tankers = find_tankers(gossip_rows, fuel_good=fuel_good,
                                 refinery_stations=refinery_set)
          tanker_total += len(tankers)
          ft = first_tanker(tankers)
          if ft is not None:
              if first_tanker_global is None or ft["post_tick"] < first_tanker_global["post_tick"]:
                  first_tanker_global = {**ft, "seed": seed}
          print(
              f"  WA4 seed={seed}: {len(tankers)} tanker events "
              f"({'first at t=' + str(ft['post_tick']) if ft else 'none'})"
          )

          # Per-good route concentration
          hhi = per_good_hhi_milli(gossip_rows, n_routes=n_stations * n_stations)
          all_hhi[seed] = hhi

      # Route concentration ensemble summary (L4-C3/L5-C3)
      if all_hhi:
          all_goods = sorted({g for h in all_hhi.values() for g in h})
          print("\nPer-good route concentration (HHI‰ quartiles, 100k, 20 seeds):")
          print(f"  {'good':>4}  {'q1':>6}  {'median':>6}  {'q3':>6}  reading")
          for g in all_goods:
              q = ensemble_quartiles_good(all_hhi, g)
              if q is None:
                  continue
              q1, med, q3 = q
              reading = ("concentrated" if med >= 600
                         else "moderate" if med >= 200
                         else "LOW (self-averaging — L5-C3)")
              print(f"  {g:>4}  {q1:>6}  {med:>6}  {q3:>6}  {reading}")

      # WA4 summary
      print(f"\nWA4 emergent tankers (100k, 20 seeds): {tanker_total} total tanker events")
      if first_tanker_global:
          print(
              f"  First tanker across ensemble: seed={first_tanker_global['seed']} "
              f"contract={first_tanker_global['contract']} "
              f"to_station={first_tanker_global['to_station']} "
              f"post_tick={first_tanker_global['post_tick']} "
              "(console chronicle: 'the market fixed the fuel desert')"
          )
      else:
          print("  WA4 reading: NoTanker across ensemble — recorded as a finding.")

      print(
          "\n=== rung-A exit complete (PDR-0006: RECORDED, NEVER GATED) ===\n"
          "Judgment sessions to bank same-day:\n"
          "  1. 'the market fixed the fuel desert' (WA4 tanker arc)\n"
          "  2. 'the trader who flew too close' (WA3 mode-flip arc)\n"
          "  3. trophic preservation read (WA5 distribution vs frontier bank)"
      )


  def ensemble_quartiles_good(all_hhi, good):
      vals = sorted(v[good] for v in all_hhi.values() if good in v)
      n = len(vals)
      if n < 2:
          return None
      return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 2: Run the ensemble (50k pass first, then 100k)**

  ```bash
  # 50k pass (WA5 bank-comparability)
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 50000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101
  ```

  If the digest step diverges, STOP — bisect commit-by-commit. Do NOT
  rationalize.

  ```bash
  # 100k pass (per-good reads)
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 100000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101
  ```

  The runner is idempotent: cells already in `runs/gag-rung-a/` are not re-run.

- [ ] **Step 3: Bank the console packet**

  Capture the full output of both passes to the same-day post directory:

  ```bash
  DATE=$(date +%Y-%m-%d)
  mkdir -p docs/superpowers/posts/${DATE}-bazaar-rung-a
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 100000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101 \
    2>&1 | tee docs/superpowers/posts/${DATE}-bazaar-rung-a/console-packet.txt
  ```

  Also run the chronicle for the first-tanker seed (the WA4 arc):

  ```bash
  # Identify the first-tanker seed from the console-packet above, then:
  TANKER_SEED=<seed from console-packet>
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario bazaar --seed $TANKER_SEED --ticks 100000 \
    --chronicle \
    --gossip-log runs/gag-rung-a/chronicle-bazaar-s${TANKER_SEED}.gossip.jsonl \
    --jsonl     runs/gag-rung-a/chronicle-bazaar-s${TANKER_SEED}.jsonl \
    > docs/superpowers/posts/${DATE}-bazaar-rung-a/chronicle-tanker-seed${TANKER_SEED}.txt
  ```

  The chronicle output is the "the market fixed the fuel desert" story artifact.
  Bank it same-day (the capture-story-artifacts standing directive).

- [ ] **Step 4: Commit the sweep runner**

  ```bash
  git add python/analysis/sweep_bazaar.py
  git commit -F - <<'EOF'
  feat(lab): sweep_bazaar — rung-A 20-seed campaign runner + exit digest

  20-seed × 50k+100k ensemble runner for WA1-5 and per-good route
  concentration (L4-C3/L5-C3). Behavior-digest step verifies trophic + frontier
  vs the A0 baseline before any WA reads (determinism gate; any divergence = STOP).
  Idempotent: banked cells in runs/ are not re-run.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

- [ ] **Step 5: Confirm full workspace tests still pass**

  ```bash
  cargo test --workspace 2>&1 | tail -15
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/ -q 2>&1 | tail -15
  ```

  Expected: all green.

- [ ] **Step 6: Record the WA1-5 readings as observations in filigree**

  After reading the console packet, use `filigree observation_create` (MCP) or
  `filigree observation create` (CLI) to record:
  - WA1 reading (starvation map): `file_path` = `runs/gag-rung-a/`, one
    observation per surprising finding (rim starvation → finding; universal →
    finding requiring owner attention).
  - WA4 first tanker tick and seed (if found): a positive finding.
  - WA5 Alive‰ comparison vs frontier bank.
  - Any prey-shrink confound seeds (WA3 warning).

  The owner reads the console packet and the chronicle materials, then judges
  the rung (PDR-0006). No gate. No threshold. The story arc is the criterion.

---

## Phase A6 task index

| ID | Title |
|----|-------|
| A6.0 | Behavior digest baseline (A0 tip pinned) |
| A6.1 | WA1 survival-by-market reader |
| A6.2 | WA2 spread-closure reader |
| A6.3 | WA3 + WA5 joint reader |
| A6.4 | WA4 emergent-tanker reader |
| A6.5 | BAZAAR anchored line + per_station_stock/price JSONL + route-concentration panel |
| A6.6 | Per-station epilogue block + chronicle enrichment |
| A6.7 | 20-seed ensemble + rung-A exit digest + console packet |

## Cross-task constraints (encoded in steps above)

- **Lockstep rule (A6.5):** BAZAAR anchored println! + BAZAAR_RE + V5 fixture
  in the same commit. No exceptions.
- **Joint-read mandate (A6.3):** WA3 and WA5 are never read in isolation;
  `wa3_wa5_joint.py` enforces this structurally.
- **Clean seeds (A6.3, A6.7):** `clean_seeds()` filters PermanentPeace before
  every WA5 distribution read. PermanentPeace is first in the verdict chain
  (diagnostics.rs:288) and overrides cycled.
- **Distribution-vs-distribution (A6.3, A6.7):** WA5 is never same-seed
  paired; the two bags are independent.
- **Hauler slice from META (A6.3, A6.7):** `int(meta["haulers"])` from the
  META line; never a module-level constant (L5-C2).
- **Digest-first (A6.7):** `verify_digest_vs_baseline` runs before any WA
  reads; divergence aborts the sweep.
- **No gates:** every window is RECORDED, NEVER GATED. No metric makes a step
  fail. The only binary check is the behavior digest (determinism, not a WA
  reading).
- **Reward surfaces untouched:** no A6 step modifies any reward function.
- **runs/ never staged:** all run outputs go to `runs/` (gitignored by
  HOUSE RULES). Story artifacts go to `docs/superpowers/posts/`.

## Cross-cutting checklist

- **A1 hash-neutrality proof rule:** The runtime-goods refactor (A1) is PROVABLY hash-neutral. Before merging A1, run the cross-branch state-hash sequence equality check on `scenario_trophic` seed 7 (1 000 ticks, commit A1a) AND on both `scenario_trophic` + `scenario_frontier` seed 7 (2 000 ticks, commit A1b). Any tick-level divergence is a defect — stop, bisect, fix. Do NOT bump `HASH_FORMAT_VERSION` in A1.

- **Single v6 bump rule:** There is EXACTLY ONE `HASH_FORMAT_VERSION` bump in rung A: the A2.1 commit that adds the `hold` column fold. Every golden literal that changes (GOLDEN_ZERO_STATE_HASH, state_hash_golden_zero_world assertion, FRONTIER_TRAJECTORY_GOLDEN) moves in that single commit. No other rung-A commit touches `HASH_FORMAT_VERSION` or any of those three golden constants.

- **Behavior-digest exit criterion vs A0 baseline:** A0.6 banks the baseline (sha256 over stdout + JSONL + gossip-log for `scenario_trophic` and `scenario_frontier` at seeds 7 and 23, 50 000 ticks) at the A0.5 commit tip. A2.5 verifies the digest is unchanged after the v6 bump (state hashes move; behavior digests must not). A6.7 re-verifies before any WA read. Any divergence is a determinism break — STOP, bisect commit-by-commit, do NOT rationalize.

- **Never-gate rule (windows recorded):** WA1 through WA5 readings are designer windows — recorded, never gated. No plan step may make a WA metric a pass/fail gate for proceeding (unit tests and the behavior-digest determinism check are exempted). Print the reading; bank it in `docs/superpowers/posts/`; let the owner judge.

- **WA3/WA5 joint-read rule:** WA3 (own-trade channel mix vs capitalization) and WA5 (trophic preservation) are NEVER read in isolation. Own-trade share is pirate food supply; high own-trade share in a PermanentPeace seed is the prey-shrink/PermanentPeace masquerade. `wa3_wa5_joint.py` enforces this structurally. `clean_seeds()` filters PermanentPeace before every WA5 distribution read. WA5 is always distribution-vs-distribution (bazaar bag vs frontier bank), NEVER same-seed paired.

- **No-rung-B-knobs rule:** rung-B surfaces (jettison verb, scoop verb, fence config, posture config, greed config, JetsamStore, CrateCfg sealed-crate states) are DEFECTS if present in any rung-A commit. A5.3 factory invariant tests include an exhaustive-destructure static check that `ExchangeCfg` and `ArbitrageCfg` carry none of those fields. Any review finding one is a CRITICAL.

- **Commit trailer:** every commit message in this plan ends with the exact line:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```
  Use `git commit -F - <<'EOF' ... EOF` (heredoc form) to preserve the trailer exactly.

- **Explicit git add paths:** every commit step lists the exact files to stage. Never use `git add -A` or `git add .`. Stage only the files listed in the commit's git add command.

- **Never stage runs/:** all run outputs go to `runs/` (gitignored by house rules). Never `git add runs/`. Story artifacts from the console packet go to `docs/superpowers/posts/<date>-<story>/` and are staged from there.
