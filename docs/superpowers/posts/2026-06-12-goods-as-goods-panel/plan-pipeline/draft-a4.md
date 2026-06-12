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
          // Dead variant retained for hash stability — never emitted in production.
          EventKind::Trade { .. } => None,
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

### Task A4.2: `ArbitrageCfg` in config (A3 dependency confirmation + transport table)

> **Pre-condition:** A3 has landed and introduced `ArbitrageCfg` (with `scan_interval`, `wage_flat_micros`, `wage_share_milli`, `qty_ladder: Vec<u32>`, `max_posts_per_scan: usize`) and the per-route integer transport table (`transport_micros: Vec<i64>`, indexed `from_row * n_stations + to_row`). If A3 has NOT yet landed, this task lands the config struct first. If A3 IS landed, this task verifies the shape and adds the `ExchangeCfg::active` solvency drain read (anchored stdout, never gated).

**Transport cost derivation (PDR-0007):** The transport table is a factory-time integer precomputed from the ring-orbit geometry. For the frontier 10-station band, the per-hop dv floor is derived from the Kepler delta-v between adjacent ring orbits. The spec (synthesis Part 1.2) states "transport[route] is a FACTORY-TIME integer table from phase-independent ring-radius geometry (the tier-reward precedent, scenario.rs:466-467)". The concrete derivation: for a directed pair (from_body_au, to_body_au) use the impulsive Hohmann approximation `dv ≈ √(GM/a_from)|1 - √(a_from/a_to)|` scaled to the cost calibration `transport_micros = (dv_norm * trade_base_micros).round() as i64`. The exact coefficient is a factory-time constant stored in `ArbitrageCfg::transport_micros` — not runtime ephemeris. **The builder must compute this from FRONTIER_ORBIT_AU[i] entries at scenario factory time; cite PDR-0007 in the doc comment.**

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` — add `ArbitrageCfg` struct, add to `RunConfig`, fold at tail of `config_hash`
- Modify: `crates/jumpgate-core/src/scenario.rs` — add `ArbitrageCfg` to `scenario_bazaar` factory; `scenario_trophic` and `scenario_frontier` get `ArbitrageCfg::default()` (inert, `scan_interval == 0`)

---

- [ ] **Step 1: Failing test — `ArbitrageCfg` struct exists and folds into config_hash**

  ```rust
  // In config.rs tests:
  #[test]
  fn arbitrage_cfg_is_folded_and_inert_by_default() {
      let cfg = ArbitrageCfg::default();
      assert_eq!(cfg.scan_interval, 0, "default scan_interval must be 0 (inert gate)");
      assert!(cfg.qty_ladder.is_empty(), "default qty_ladder is empty (no posts)");
      // Config hash changes when scan_interval changes — proves folding works.
      let mut c1 = crate::config::RunConfig::sample();
      let mut c2 = c1.clone();
      c2.arbitrage.scan_interval = 1;
      let h1 = crate::config::config_hash(&c1);
      let h2 = crate::config::config_hash(&c2);
      assert_ne!(h1, h2, "scan_interval change must move config hash");
  }
  ```

  Run: `cargo test -p jumpgate-core arbitrage_cfg_is_folded_and_inert_by_default`
  Expected failure: `error[E0422]: cannot find struct, variant or union type ArbitrageCfg`

- [ ] **Step 2: Add `ArbitrageCfg` to `config.rs`**

  After `RefuelCfg` (config.rs ~line 413), add:

  ```rust
  /// Arbitrage posting config (goods-as-goods rung A, A4). `scan_interval == 0`
  /// is the structural inert gate — the whole arbitrage stage is skipped.
  /// The transport table is a factory-time Hohmann-floor integer keyed
  /// `from_station_row * n_stations + to_station_row` (PDR-0007: route cost
  /// floor from ring-orbit geometry, not runtime ephemeris).
  #[derive(Clone, Debug)]
  pub struct ArbitrageCfg {
      /// Ticks between arbitrage scans. 0 = inert (no posting). Default: 0.
      pub scan_interval: u32,
      /// Static per-unit wage floor from ring-orbit geometry (PDR-0007).
      /// Index: from_station_row * n_stations + to_station_row.
      /// Empty vec = no routes registered (inert).
      pub transport_micros: Vec<i64>,
      /// Posted wage = transport[route] + spread_surplus × wage_share_milli / 1000.
      pub wage_share_milli: u32,
      /// Fixed package size ladder (smallest-first). Each element is tried in order;
      /// first that clears spread AND fits hold is used.
      pub qty_ladder: Vec<u32>,
      /// Maximum arbitrage posts per scan pass (backstop against degenerate configs).
      pub max_posts_per_scan: usize,
  }

  impl Default for ArbitrageCfg {
      fn default() -> Self {
          ArbitrageCfg {
              scan_interval: 0,
              transport_micros: Vec::new(),
              wage_share_milli: 0,
              qty_ladder: Vec::new(),
              max_posts_per_scan: 64,
          }
      }
  }
  ```

  Add `arbitrage: ArbitrageCfg` to `RunConfig` struct after `refuel` field. Add the field to `sample()` exhaustive struct literal: `arbitrage: ArbitrageCfg::default(),`.

  Add to `config_hash` at tail after RefuelCfg fold:

  ```rust
  // ArbitrageCfg — goods-as-goods rung A.
  // Count first (anti-aliasing delimiter per synthesis L5-F7).
  h.write_u64(arbitrage.transport_micros.len() as u64);
  for &t in &arbitrage.transport_micros {
      h.write_u64(t as u64);
  }
  h.write_u64(arbitrage.scan_interval as u64);
  h.write_u64(arbitrage.wage_share_milli as u64);
  h.write_u64(arbitrage.qty_ladder.len() as u64);
  for &q in &arbitrage.qty_ladder {
      h.write_u64(q as u64);
  }
  h.write_u64(arbitrage.max_posts_per_scan as u64);
  ```

  The exhaustive destructure in `config_hash` must name the `arbitrage` field or this is a compile error.

  Run: `cargo test -p jumpgate-core arbitrage_cfg_is_folded_and_inert_by_default`
  Expected: PASS.

- [ ] **Step 3: GOLDEN_CONFIG_HASH re-pin — single-cause commit**

  The `config_hash_golden_anchor_is_stable` test will now fail because a new field was added. Re-derive by running:

  ```bash
  cargo test -p jumpgate-core -- config_hash_golden_anchor_is_stable --nocapture 2>&1 | tail -5
  ```

  The test will print the new hash. Paste it into `config.rs` at the `GOLDEN_CONFIG_HASH` constant and update the provenance comment:

  ```rust
  // RE-PINNED: +ArbitrageCfg{scan_interval,transport_micros,wage_share_milli,
  //            qty_ladder,max_posts_per_scan} folded at config tail (goods-as-goods A4).
  // Was 0x<previous>.
  const GOLDEN_CONFIG_HASH: u64 = 0x<NEW_VALUE_FROM_TEST>;
  ```

  Verify: `cargo test -p jumpgate-core config_hash_golden_anchor_is_stable`
  Expected: PASS.

  ```bash
  git add crates/jumpgate-core/src/config.rs
  git commit -F - <<'EOF'
  feat(gag-a4): ArbitrageCfg in config + GOLDEN_CONFIG_HASH re-pin

  Adds ArbitrageCfg{scan_interval,transport_micros,wage_share_milli,
  qty_ladder,max_posts_per_scan} at the RunConfig tail, folded with
  count-first discipline. scan_interval==0 is the structural inert gate.
  transport_micros is the factory-time PDR-0007 Hohmann-floor table.
  Re-pins GOLDEN_CONFIG_HASH (single cause: new config field).
  trophic/frontier get ArbitrageCfg::default() (scan_interval=0, inert).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

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

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` — add withdrawal sweep in `run_scripted_dispatch`, update signature to include `ships: &mut CraftStore` for the Offered craft-intent clearing
- Modify: `crates/jumpgate-core/src/world.rs` — call site update for new `ships` arg (already present in the function but only used by ASSIGN)

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
| `ArbitrageCfg` in config + GOLDEN_CONFIG_HASH re-pin | A4.2 | 1 |
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
