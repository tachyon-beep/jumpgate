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

  Update `ContractInit` rows in scenario.rs (lines 156–161 for frontier, 243–248 for trophic):

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
      pub name: &'static str,
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
                  GoodSpec { name: "Ore",  unit_mass_milli: 1000 },
                  GoodSpec { name: "Fuel", unit_mass_milli: 1000 },
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
