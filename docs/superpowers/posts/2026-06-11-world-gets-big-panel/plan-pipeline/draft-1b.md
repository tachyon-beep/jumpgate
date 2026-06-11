# Phase 1 — the refuel verb (spec §5, §7 Refueled/ContractFailed, §9 phase-1 rest)

> Drafted against HEAD `e7e490e`. Prerequisite within phase 1: **Task 1.1 (the
> eps 1e-11 re-bake + fixture redesign, own commit — owned by the fuel-edge
> section)** lands BEFORE Task 1.2.6 (the PLAY-C1 dispatch filter compares
> `fuel_mass > FUEL_EMPTY_EPS`, and every trophic tank is exactly the OLD eps —
> with eps still 1e-9 the filter would blacklist every full-tank band hauler).
> Tasks 1.2.1–1.2.5 and 1.2.7 do not read the eps and may land before or after
> 1.1; the stated order below is the safe one.
>
> House rules every commit step obeys: `git add` EXPLICIT paths only (never
> `-A`, never `.`); never stage `runs/`; commit messages via `git commit -F -`
> heredoc ending with the exact trailer
> `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`;
> golden literals are NEVER typed from this plan — re-pins paste the output of
> the `print_golden_config` printer test, single-cause commits. Reward
> surfaces untouched. Windows are recorded, never gated.

---

### Task 1.2.1: `RefuelCfg { lot_mass, corp_index }` — config surface, exhaustive fold, half-on reset error, ONE golden re-pin

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` (struct after `MediaCfg` ~config.rs:398; `RunConfig` tail config.rs:407-436; `config_hash` destructure config.rs:503-521 + tail fold after the MediaCfg block config.rs:690-713; `CONFIG_FIELD_ORDER` doc config.rs:480-498; `sample()` config.rs:747-793; `GOLDEN_CONFIG_HASH` config.rs:745; new `changing_refuel_cfg_changes_config_hash` test cloning config.rs:989-999)
- Modify (RunConfig-literal compile fixes, `refuel: RefuelCfg::default()` appended after `media:` in each): `crates/jumpgate-core/src/scenario.rs` (~:229-253), `crates/jumpgate-core/src/economy.rs` (`vendor_world_fixture` ~:1564-1611, `starved_two_body_contract_fixture` ~:2364-2455), `crates/jumpgate-core/src/world.rs` (test fixtures incl. `one_body_one_thrusting_craft` ~:1282-1297, `two_body_starved_contract_fixture` ~:2196-2205), `crates/jumpgate-core/src/ingest.rs`, `crates/jumpgate-core/src/pirate.rs`, `crates/jumpgate-core/src/hash.rs`, `crates/jumpgate-core/src/diagnostics.rs`, `crates/jumpgate-core/tests/replay_equivalence.rs` (~:25-44), `crates/jumpgate-core/tests/physics_sanity.rs`, `crates/jumpgate-py/src/env.rs` (~:187-213, ~:464-490)
- Modify (commit 2): `crates/jumpgate-core/src/world.rs` (`ResetError` ~:142-155, `Display` ~:157-176, `World::reset` validation after the media half-on check ~:207-211; new reset-error test cloning the `BadMediaCfg` test at world.rs:1700-1712)

- [ ] **Step 1: failing test first.** In `crates/jumpgate-core/src/config.rs` tests, clone `changing_media_cfg_changes_config_hash` (config.rs:989-999):

```rust
    #[test]
    fn changing_refuel_cfg_changes_config_hash() {
        let base = sample().config_hash();
        let mut cfg = sample();
        cfg.refuel.lot_mass = 5e-11;
        assert_ne!(cfg.config_hash(), base, "lot_mass must be folded");
        let mut cfg = sample();
        cfg.refuel.corp_index = 4;
        assert_ne!(cfg.config_hash(), base, "corp_index must be folded");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core changing_refuel_cfg` → expected failure:** compile error `E0609: no field `refuel` on type `RunConfig`` (the exhaustive-destructure discipline working as designed).
- [ ] **Step 3: implement the struct + field + fold.** In `config.rs`, after the `MediaCfg` impl block (~:398):

```rust
/// The propellant-purchase verb (world-gets-big rung §5). Inert by default:
/// `lot_mass == 0.0` makes BOTH refuel stages (1c3b `run_refuel_policies`,
/// 1d2 `resolve_refuels`) deterministic no-ops — the named trophic-inertness
/// gate (scenario_trophic leaves this default-off; proven by the phase-exit
/// cross-branch digest, Task 1.2.7).
#[derive(Clone, Copy, Debug)]
pub struct RefuelCfg {
    /// Propellant mass per integer lot (same f64 unit as `fuel_mass`).
    /// `0.0` = the refuel verb is OFF. The settle decision is integer lots:
    /// `units = min(floor((cap_eff - fuel)/lot), stock[Fuel], credits/price)`.
    pub lot_mass: f64,
    /// Corporation (config index) credited with every refuel payment — the
    /// Port corp (the Yard precedent, `ShipyardCfg.corp_index`: dense
    /// slot == row; a stale/out-of-range row is a deterministic settle skip,
    /// never a one-legged debit). The frontier factory (phase 2) appends a
    /// `CorporationInit { treasury_micros: 0, .. }` Port corp and points this
    /// at it; on a lot-0 world this index is never read.
    pub corp_index: u32,
}

impl Default for RefuelCfg {
    fn default() -> Self {
        RefuelCfg { lot_mass: 0.0, corp_index: 0 }
    }
}
```

  On `RunConfig` (after `media`, config.rs:436):

```rust
    // World-gets-big rung (folded AFTER media, append-only). Default leaves the
    // refuel machinery inert (lot_mass == 0.0 => both refuel stages no-op).
    pub refuel: RefuelCfg,
```

  In `config_hash`: add `refuel, // NEW (world-gets-big): destructure forces folding below` to the top-level destructure (config.rs:521), extend the `CONFIG_FIELD_ORDER` doc list with `///  26. refuel: lot_mass.to_bits(), corp_index`, and append at the VERY tail, after the MediaCfg field writes (config.rs:713), before `ConfigHash(h.finish())`:

```rust
        // WORLD-GETS-BIG RUNG (TAIL, append-only — CONFIG_FIELD_ORDER 26). The
        // byte stream above stays byte-identical; this only EXTENDS it.
        // Exhaustive destructure: a NEW RefuelCfg field is a COMPILE ERROR here
        // until explicitly folded (the D10/M6 discipline).
        let RefuelCfg { lot_mass, corp_index } = refuel;
        h.write_u64(lot_mass.to_bits());
        h.write_u64(*corp_index as u64);
```

  Add `refuel: RefuelCfg::default(),` to `sample()` (config.rs:792).
- [ ] **Step 4: fix every RunConfig literal in the workspace.** Run `cargo build --workspace 2>&1 | grep -c E0063` — every error site is a struct literal missing the new field. Append `refuel: RefuelCfg::default(),` (or `refuel: jumpgate_core::config::RefuelCfg::default(),` in `crates/jumpgate-py/src/env.rs` :213/:490 — match the `media:` line's path style at each site) to EVERY listed literal: scenario.rs factory, economy.rs fixtures, world.rs fixtures, ingest.rs, pirate.rs, hash.rs, diagnostics.rs fixtures, tests/replay_equivalence.rs, tests/physics_sanity.rs, jumpgate-py env.rs. Re-run `cargo build --workspace` → clean.
- [ ] **Step 5: re-pin the config golden (the ONLY golden that moves this rung).** Run `cargo test -p jumpgate-core config_hash_golden_anchor` → expected failure: `config_hash drifted: re-pin only if intentional`. Then run

```
cargo test -p jumpgate-core --lib print_golden_config -- --ignored --nocapture
```

  and paste ITS printed hex (never hand-computed, never taken from this plan) into config.rs:745, keeping the provenance comment discipline on the literal's line:

```rust
    const GOLDEN_CONFIG_HASH: u64 = 0x<PASTE_PRINTED_VALUE>; // RE-PINNED: +RefuelCfg{lot_mass,corp_index} folded at config tail (world-gets-big §5). Was 0xee02_df67_1889_78dc.
```

- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core config` → all green including `changing_refuel_cfg_changes_config_hash` and the re-pinned anchor. `cargo test --workspace` green (state goldens untouched — the new field is config-side only; `HASH_FORMAT_VERSION` stays 5).
- [ ] **Step 7: commit (single cause: the RefuelCfg fold).**

```
git add crates/jumpgate-core/src/config.rs crates/jumpgate-core/src/scenario.rs \
  crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs \
  crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/pirate.rs \
  crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/diagnostics.rs \
  crates/jumpgate-core/tests/replay_equivalence.rs crates/jumpgate-core/tests/physics_sanity.rs \
  crates/jumpgate-py/src/env.rs
git commit -F - <<'EOF'
feat(world-gets-big): RefuelCfg folded at config tail (GOLDEN_CONFIG_HASH re-pinned, single cause)

lot_mass (0.0 = the named trophic-inertness gate) + corp_index (the Port
corp binding, Yard precedent). CONFIG_FIELD_ORDER 26; every RunConfig
literal gains refuel: RefuelCfg::default(). No HASH_FORMAT_VERSION bump;
zero state goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 8: failing test for the half-on reset error.** In `crates/jumpgate-core/src/world.rs` tests, next to the `BadMediaCfg` half-on test (world.rs:1700-1712):

```rust
    #[test]
    fn refuel_half_on_price_surface_is_a_reset_error() {
        // lot_mass > 0 with a dead Fuel price surface would make every refuel a
        // silent `unit_price < 1` no-op — a misconfiguration, rejected before
        // tick 0 (the BadMediaCfg half-on idiom).
        let mut cfg = one_body_one_craft_station_cfg();
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 5e-11, corp_index: 0 };
        // Arm 1: price_cfg.base_micros[Fuel] == 0 (the PriceCfg default).
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with base_micros[Fuel] == 0 must be rejected"
        );
        // Arm 2: base live, but a station's seeded initial_price_micros[Fuel] == 0.
        cfg.price_cfg.base_micros[crate::economy::Resource::Fuel.index()] = 5_000;
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with a zero seeded station Fuel price must be rejected"
        );
        // Control: both live -> resolves.
        for s in cfg.stations.iter_mut() {
            s.initial_price_micros[crate::economy::Resource::Fuel.index()] = 5_000;
        }
        assert!(World::reset(cfg).is_ok(), "fully-on refuel config resolves");
    }
```

  (Use whatever station-bearing fixture the neighbouring BadMediaCfg test uses — clone its fixture call verbatim; the name above is a stand-in for that exact fixture. If it has no station, push one `StationInit { body_index: 0, initial_stock: [0, 0], initial_price_micros: [0, 0], sells_upgrades: false }` plus a body as the BadEconomyRef tests do.)
- [ ] **Step 9: run `cargo test -p jumpgate-core refuel_half_on` → expected failure:** `E0599: no variant named `BadRefuelCfg``.
- [ ] **Step 10: implement.** New `ResetError` variant (after `BadMediaCfg`, world.rs:154):

```rust
    /// A half-on `RefuelCfg`: `lot_mass > 0` while the Fuel price surface is
    /// structurally dead (`price_cfg.base_micros[Fuel] == 0`, the cap-0/base-0
    /// update_prices skip) or any station's seeded
    /// `initial_price_micros[Fuel] == 0`. Every refuel would be a silent
    /// `unit_price < 1` settle skip — a misconfiguration, rejected before
    /// tick 0 (the media half-on idiom).
    BadRefuelCfg { reason: &'static str },
```

  Display arm (after the BadMediaCfg arm, world.rs:170-172):

```rust
            ResetError::BadRefuelCfg { reason } => {
                write!(f, "bad refuel config: {reason}")
            }
```

  Validation in `World::reset`, directly after the media half-on check (world.rs:211):

```rust
        // Refuel half-on validation (world-gets-big §5, the BadMediaCfg idiom):
        // a live lot size demands a live Fuel price surface, config-wide.
        if cfg.refuel.lot_mass > 0.0 {
            let fuel = crate::economy::Resource::Fuel.index();
            if cfg.price_cfg.base_micros[fuel] == 0 {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but price_cfg.base_micros[Fuel] == 0 (price surface dead)",
                });
            }
            if cfg.stations.iter().any(|s| s.initial_price_micros[fuel] == 0) {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but a station's seeded initial_price_micros[Fuel] == 0",
                });
            }
        }
```

- [ ] **Step 11: run + expected pass.** `cargo test -p jumpgate-core refuel_half_on` green; `cargo test -p jumpgate-core` green (no behavior change for lot-0 configs — every existing fixture).
- [ ] **Step 12: commit.**

```
git add crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): BadRefuelCfg reset error — the refuel half-on idiom

lot_mass > 0 with base_micros[Fuel] == 0 or any zero seeded station Fuel
price is rejected before tick 0 (the BadMediaCfg precedent). Hash-neutral.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.2: `CraftStore.pending_refuel: Vec<Option<()>>` — transient column at all THREE sizing sites + the all-None hash-point assert

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (field after `pending_upgrade` ~:203; `empty()` ~:224-248; `push()` ~:255-285; length-parallel test asserts ~:412-482)
- Modify: `crates/jumpgate-core/src/world.rs` (reset's hand-built literal ~:232-254 and per-craft loop ~:255-303 — reset does NOT use `push`)
- Modify: `crates/jumpgate-core/src/hash.rs` (extend the all-None debug_assert block hash.rs:301-309)

- [ ] **Step 1: failing test first.** Extend the store length-parallel tests in `stores.rs` (`stores_construct_soa_parallel` at :412 and the push test asserting `pending_upgrade.len()` at ~:482) with, in each, immediately after the `pending_upgrade` assert:

```rust
        assert_eq!(ship.pending_refuel.len(), ship.ids.len());
```

- [ ] **Step 2: run `cargo test -p jumpgate-core stores` → expected failure:** `E0609: no field `pending_refuel` on type `CraftStore``.
- [ ] **Step 3: implement the column at all three sites.** `stores.rs` field, directly after `pending_upgrade` (:203):

```rust
    /// TRANSIENT refuel intent (world-gets-big §5 — the `pending_upgrade`
    /// precedent, same strictness): written by ingest (`CommandKind::Refuel`)
    /// or the scripted stage 1c3b, consumed by `resolve_refuels` (stage 1d2)
    /// the SAME tick, so it is always `None` at every hash point —
    /// `state_hash` debug_asserts exactly that. NOT folded into
    /// HASH_FIELD_ORDER; no HASH_FORMAT_VERSION bump.
    pub pending_refuel: Vec<Option<()>>,
```

  `empty()` (:245, after `pending_upgrade: Vec::new(),`): `pending_refuel: Vec::new(),` — `push()` (:280, after `self.pending_upgrade.push(None);`): `self.pending_refuel.push(None);`. In `world.rs` reset: the hand-built struct literal gains `pending_refuel: Vec::new(),` (after :252's `pending_upgrade: Vec::new(),`) and the per-craft loop gains `ships.pending_refuel.push(None);` (after :294's `ships.pending_upgrade.push(None);` — reset bypasses `push`, all columns must stay length-parallel or the hash's dense-row unwraps panic).
- [ ] **Step 4: extend the hash-point assert.** In `hash.rs`, directly after the existing block at :306-309:

```rust
    // `pending_refuel` is TRANSIENT intent (world-gets-big §5): written and
    // consumed within one tick (stage 1d2), so it must be empty at EVERY hash
    // point. A `Some` here is a stage-ordering bug — fail loud in debug.
    debug_assert!(
        world.ships.pending_refuel.iter().all(Option::is_none),
        "pending_refuel must be fully consumed (all None) at every state-hash point"
    );
```

- [ ] **Step 5: run + expected pass.** `cargo test -p jumpgate-core stores` green; `cargo test -p jumpgate-core` green (goldens untouched: the column is never folded; `recompute_with_cursors` needs no mirror — nothing was added to the fold stream).
- [ ] **Step 6: commit.**

```
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/hash.rs
git commit -F - <<'EOF'
feat(world-gets-big): pending_refuel transient column (3 sizing sites + all-None hash assert)

The pending_upgrade precedent: stores empty()/push() + World::reset's
hand-built literal and per-craft loop; joins the all-None-at-every-hash-
point debug_assert. NOT hashed; no format bump; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.3: `resolve_refuels` at stage 1d2 — always-consume-then-gate settle, integer quantization, four legs, `Refueled` event, FLOOR-permille pinned

**Files:**
- Modify: `crates/jumpgate-core/src/contract.rs` (`EventKind` — append `Refueled` at the enum tail, after `GossipHeard` ~:168)
- Modify: `crates/jumpgate-core/src/economy.rs` (new `docked_station_row` next to `docked_at_vendor` ~:926-952; new `resolve_refuels` after `resolve_purchases` ~:924; tests: `refuel_world_fixture`, `refuel_settles_quantized_with_four_legs_and_exact_event`, `refuel_tank_permille_is_floor_rounded`, `assert_refuel_skipped`, `refuel_skips_deterministically`)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 1d2 call after `resolve_purchases` :776-786, before the physics body snapshot at :789)

- [ ] **Step 1: failing tests first.** In `economy.rs` `#[cfg(test)]`, the fixture (clone of `vendor_world_fixture`'s shape with the Fuel price surface live) plus the exact-settle test:

```rust
    /// Refuel fixture: the vendor fixture's one-body/one-station/one-craft dock
    /// with the Fuel price surface LIVE (base 5_000, cap 40 — the §5 frontier
    /// shape; cap[Ore] == 0 keeps Ore structurally dead), the reprice clock OFF
    /// (interval 0, world.rs guard) so the seeded price 5_000 is the settle
    /// price for exact-integer assertions, station Fuel stock 40, and
    /// `RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 }` — 4 lots fill the 1e-9
    /// tank; corp 0 doubles as the Port corp with treasury 0 so every credited
    /// micro is refuel money. The craft starts DRY (fuel_mass 0.0; prev == fuel
    /// at reset, so no spurious FuelEmpty edge).
    fn refuel_world_fixture() -> crate::config::RunConfig {
        let mut cfg = vendor_world_fixture(false);
        cfg.craft[0].fuel_mass = 0.0;
        cfg.stations[0].initial_stock = [0, 40];
        cfg.stations[0].initial_price_micros = [0, 5_000];
        cfg.price_cfg = crate::config::PriceCfg {
            base_micros: [0, 5_000],
            cap: [0, 40],
            slope_milli: 1800,
            reprice_interval: 0, // clock OFF: the seeded 5_000 is the settle price
        };
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 };
        cfg
    }

    #[test]
    fn refuel_settles_quantized_with_four_legs_and_exact_event() {
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        // The integer decision: need = floor((1e-9 - 0)/2.5e-10) = 4 lots;
        // afford = 12_000 / 5_000 = 2; stock = 40 => units = min(4, 40, 2) = 2.
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());

        let f = Resource::Fuel.index();
        assert_eq!(world.stations.stock[0][f], 38, "stock leg: -= units");
        assert_eq!(world.econ.consumed[f], 2, "sink leg: consumed[Fuel] += units");
        assert_eq!(world.ships.credits_micros[0], 2_000, "wallet leg: debited EXACTLY units*price");
        assert_eq!(world.corporations.treasury_micros[0], 10_000, "Port treasury credited the same (pure transfer)");
        assert_eq!(world.ships.fuel_mass[0], 5.0e-10, "tank leg: fuel += units*lot (one clamped write)");
        assert_eq!(world.ships.pending_refuel[0], None, "intent consumed");
        // Resource identity: the stock leg exits through `consumed`, exactly
        // like a producer input leg (Σstock + in_transit == initial + mined − consumed).
        let stock_now: i64 = world.stations.stock.iter().map(|s| s[f]).sum();
        assert_eq!(stock_now, 40 + world.econ.mined[f] - world.econ.consumed[f], "resource identity holds");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    craft: c,
                    units: 2,
                    price_micros: 5_000,
                    tank_before_permille: 0,
                    tank_after_permille: 500,
                    ..
                } if c == craft
            )),
            "Refueled emitted with the exact quantized payload"
        );
        // The transient-column invariant survives the hash point.
        let _ = crate::hash::state_hash(&world);
    }
```

  The FLOOR-rounding pin (no f64→permille FLOOR precedent exists in-tree — this test pins the form):

```rust
    #[test]
    fn refuel_tank_permille_is_floor_rounded() {
        // Pins the rounding form ((fuel / cap_eff) * 1000.0).floor(): 555.5
        // and 805.5 both FLOOR — never half-up, never round-to-nearest.
        use crate::world::World;
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].fuel_mass = 5.555e-10; // 555.5 permille of the 1e-9 tank
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 5_000; // afford exactly 1 unit
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        // need = floor((1e-9 - 5.555e-10)/2.5e-10) = floor(1.778) = 1; units = 1.
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    units: 1,
                    tank_before_permille: 555,
                    tank_after_permille: 805,
                    ..
                }
            )),
            "tank permilles are FLOOR-rounded against EFFECTIVE capacity"
        );
    }
```

  The skip catalogue (clone of `purchase_skips_deterministically` + `assert_purchase_skipped`, economy.rs:1623-1743):

```rust
    /// Skip-arm postcondition: zero movement on every leg, intent consumed,
    /// NO Refueled event (the assert_purchase_skipped pattern).
    fn assert_refuel_skipped(
        world: &mut crate::world::World,
        credits_before: i64,
        fuel_before: f64,
        stock_before: i64,
        arm: &str,
    ) {
        let f = Resource::Fuel.index();
        assert_eq!(world.ships.fuel_mass[0], fuel_before, "{arm}: tank untouched");
        assert_eq!(world.ships.credits_micros[0], credits_before, "{arm}: zero wallet movement");
        assert_eq!(world.stations.stock[0][f], stock_before, "{arm}: stock untouched");
        assert_eq!(world.econ.consumed[f], 0, "{arm}: no sink leg");
        assert_eq!(world.corporations.treasury_micros[0], 0, "{arm}: Port treasury untouched");
        assert_eq!(world.ships.pending_refuel[0], None, "{arm}: intent consumed");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "{arm}: NO Refueled event"
        );
    }

    #[test]
    fn refuel_skips_deterministically() {
        use crate::math::Vec3;
        use crate::world::World;
        let f = Resource::Fuel.index();

        // (a) UNDOCKED: ~10_000x ARRIVAL_RADIUS from the station body.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "undocked");

        // (b) STOCK 0: the dry dock (the stranding arc's substrate).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.stock[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 0, "stock-0");

        // (c) WALLET SHORT: one micro short of one unit.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 4_999, 0.0, 40, "wallet-short");

        // (d) TANK FULL: headroom < 1 lot (need == 0).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 1.0e-9, 40, "tank-full");

        // (e) UNIT PRICE < 1: live store row zeroed in-test (the curve cannot
        //     reach 0 at slope 1800; the settle never divides by a dead price).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.price_micros[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "price-0");

        // (f) STALE PORT-CORP ROW: corp_index out of range — never a
        //     one-legged debit (the Yard id_at liveness precedent).
        let mut cfg = refuel_world_fixture();
        cfg.refuel.corp_index = 7;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "stale-corp");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_settles` → expected failure:** `E0599: no variant named `Refueled` found for enum `EventKind`` (then, once the variant exists, the settle assertions fail — the stage doesn't exist).
- [ ] **Step 3: implement the event variant.** Append at the TAIL of `EventKind` in `contract.rs` (after `GossipHeard`, ~:168 — events are NOT hashed; append-only by convention):

```rust
    // --- World-gets-big events (refuel rung §5/§7; hash-neutral like all events) ---
    /// A craft bought propellant at a station dock (stage 1d2). `units` is the
    /// integer lot count actually settled (`min(need, stock, afford)`),
    /// `price_micros` the per-unit price read from the dock's live price row at
    /// settle. Tank permilles are FLOOR-rounded against EFFECTIVE capacity;
    /// `tank_after_permille` derives from the decided integer purchase (the
    /// same clamped write the tank leg performs).
    Refueled {
        craft: CraftId,
        station: StationId,
        units: i64,
        price_micros: i64,
        tank_before_permille: i64,
        tank_after_permille: i64,
    },
```

- [ ] **Step 4: implement the dock predicate + the settle stage** in `economy.rs`, after `docked_at_vendor` (~:952):

```rust
/// Any-station dock predicate (world-gets-big §5): the FIRST (lowest dense
/// row — deterministic tie-break for overlapping fixture bodies) station whose
/// body is within `ARRIVAL_RADIUS` of the craft, compared in the craft's frame
/// (`body_pos` at `prev == t-1`; the try_load precedent). Shared by
/// `run_refuel_policies` (stage 1c3b) and `resolve_refuels` (stage 1d2) so
/// policy intent and same-tick settle agree on what "docked" means. Unlike
/// `docked_at_vendor` there is NO `sells_upgrades` filter: every dock sells
/// propellant when the price surface is live.
fn docked_station_row(
    ships: &CraftStore,
    crow: usize,
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    prev: Tick,
) -> Option<usize> {
    (0..stations.ids.len()).find(|&srow| {
        let body = stations.body[srow];
        bodies
            .ids
            .dense_index(body.slot, body.generation)
            .is_some_and(|brow| {
                let bpos = eph.body_pos(bodies.eph_index[brow], prev);
                ships.pos[crow].sub(bpos).length() <= crate::autopilot::ARRIVAL_RADIUS
            })
    })
}

/// Refuel settle stage — stage 1d2, world-gets-big §5 (after
/// `resolve_purchases`, PRE-physics: the same-tick burn draws from the
/// refilled tank, and the stage-4 `prev_fuel` copy-forward is untouched —
/// Class-3 pinning and the FuelEmpty edge are undisturbed).
///
/// Consumes EVERY `pending_refuel` intent THIS tick (the transient-column
/// invariant `state_hash` debug_asserts). The integer decision precedes every
/// write: `need = floor((cap_eff − fuel)/lot)`; `afford = credits / price`
/// (price >= 1 by the skip); `units = min(need, stock[Fuel], afford)`; then
/// four legs — `stock -= units` · `consumed[Fuel] += units` (the sink leg the
/// resource identity demands) · wallet -> Port corp treasury (a pure transfer,
/// zero new identity legs) · `fuel_mass += units*lot` clamped to cap in ONE
/// write. Propellant mass lives OUTSIDE both identities by design: the traded
/// Fuel stock exits through `consumed` exactly like a producer input leg, and
/// the tank is not a resource store.
///
/// Deterministic no-op skips (every one a bare `continue` AFTER the intent is
/// consumed): undocked / `unit_price < 1` / stock 0 / stale Port-corp row /
/// tank full / wallet short. Top-to-full, threshold-free — no taste scalar.
#[allow(clippy::too_many_arguments)]
pub fn resolve_refuels(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    counters: &mut EconCounters,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    if refuel.lot_mass <= 0.0 {
        // The trophic-inertness gate (world-gets-big §5). Still consume any
        // manual-ingest intents so the all-None hash invariant holds: a Refuel
        // command against a lot-0 world is a deterministic no-op, never a
        // debug_assert panic at the next hash point.
        for slot in ships.pending_refuel.iter_mut() {
            *slot = None;
        }
        return;
    }
    let lot = refuel.lot_mass;
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Resource::Fuel.index();
    for crow in 0..ships.ids.len() {
        if ships.pending_refuel[crow].is_none() {
            continue;
        }
        // ALWAYS consume the intent this stage, settle or skip (the transient-
        // column invariant: `pending_refuel` is None at every hash point).
        ships.pending_refuel[crow] = None;
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue; // undocked
        };
        let unit_price = stations.price_micros[srow][fuel_r];
        if unit_price < 1 {
            continue; // dead/degenerate price row (also the afford div-by-zero guard)
        }
        let stock = stations.stock[srow][fuel_r];
        if stock <= 0 {
            continue; // dry dock
        }
        // The Port corp (the Yard precedent): a stale/out-of-range config index
        // is a deterministic skip — never a one-legged debit.
        let port_row = refuel.corp_index as usize;
        if corporations.ids.id_at(port_row).is_none() {
            continue;
        }
        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        let cap_eff = eff.fuel_capacity;
        let fuel = ships.fuel_mass[crow];
        // The integer decision precedes every write (world-gets-big §5).
        // (Rust float->int casts saturate, so a degenerate cap/lot ratio cannot UB.)
        let need = ((cap_eff - fuel) / lot).floor() as i64;
        if need < 1 {
            continue; // tank full (headroom < 1 lot)
        }
        let afford = ships.credits_micros[crow].max(0) / unit_price;
        if afford < 1 {
            continue; // wallet short
        }
        let units = need.min(stock).min(afford);
        let cost = units.saturating_mul(unit_price);
        // FLOOR-rounded tank permilles against EFFECTIVE capacity; `after`
        // derives from the decided integer purchase below.
        let tank_before_permille = ((fuel / cap_eff) * 1000.0).floor() as i64;
        // Four legs.
        stations.stock[srow][fuel_r] -= units;
        counters.consumed[fuel_r] = counters.consumed[fuel_r].saturating_add(units);
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(cost);
        corporations.treasury_micros[port_row] =
            corporations.treasury_micros[port_row].saturating_add(cost);
        // ONE clamped write — never an iterative per-lot accumulation.
        ships.fuel_mass[crow] = (fuel + units as f64 * lot).min(cap_eff);
        let tank_after_permille = ((ships.fuel_mass[crow] / cap_eff) * 1000.0).floor() as i64;
        let craft = ships.ids_at(crow);
        if let Some(station) = stations
            .ids
            .id_at(srow)
            .map(|(slot, generation)| StationId { slot, generation })
        {
            events.emit(Event {
                tick,
                kind: EventKind::Refueled {
                    craft,
                    station,
                    units,
                    price_micros: unit_price,
                    tank_before_permille,
                    tank_after_permille,
                },
            });
        }
    }
}
```

- [ ] **Step 5: wire stage 1d2 in `World::step`**, after the `resolve_purchases` call (world.rs:786), BEFORE the physics body snapshot (:789):

```rust
        // (1d2) refuel settle stage (world-gets-big §5): consume every Refuel
        //       intent written by this tick's ingest and stage 1c3b. AFTER
        //       resolve_purchases, PRE-physics: the same-tick burn draws from
        //       the refilled tank, and the dock predicate samples body_pos at
        //       `next - 1 == cur` (the try_load frame). `prev_fuel` is NOT
        //       touched here — the stage-4 copy-forward keeps Class-3 pinning
        //       and the FuelEmpty edge undisturbed. Inert at lot_mass == 0.0
        //       (the trophic-inertness gate).
        crate::economy::resolve_refuels(
            &mut self.ships,
            &mut self.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &mut self.econ,
            &self.config.refuel,
            next,
            &mut self.events,
        );
```

- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core refuel` → `refuel_settles_quantized_with_four_legs_and_exact_event`, `refuel_tank_permille_is_floor_rounded`, `refuel_skips_deterministically` all green. `cargo test -p jumpgate-core` green (every existing world is lot-0 → the stage early-returns; zero goldens move). `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 7: commit.**

```
git add crates/jumpgate-core/src/contract.rs crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): resolve_refuels at stage 1d2 — quantized four-leg settle + Refueled event

Always-consume-then-gate (pending_upgrade precedent); units =
min(floor((cap_eff-fuel)/lot), stock[Fuel], credits/price); legs: stock,
consumed[Fuel] sink, wallet->Port corp pure transfer, one clamped tank
write. FLOOR tank permilles pinned by test. prev_fuel untouched
(Class-3). Skips: undocked/price<1/stock-0/stale-corp/tank-full/wallet.
Inert at lot_mass == 0 (consumes stray intents, settles nothing).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.4: `run_refuel_policies` at stage 1c3b — scripted top-to-full intent for docked non-pirate craft

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (new `run_refuel_policies` after `run_purchase_policies` ~:1103; tests `refuel_policy_gates_deterministically`, `credit_identity_holds_across_refuels_and_policy_is_self_running`)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 1c3b call after `run_purchase_policies` :756-766, before `resolve_purchases` :776)

- [ ] **Step 1: failing tests first** (in `economy.rs` tests — note these write NO manual intent; the scripted stage must produce the whole arc):

```rust
    #[test]
    fn credit_identity_holds_across_refuels_and_policy_is_self_running() {
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000; // covers the full 4-lot fill
        let total = |w: &crate::world::World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        let t0 = total(&world);
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..50 {
            world.step(&mut empty);
            assert_eq!(total(&world), t0, "Σtreasury+Σcredits+Σescrow invariant every tick");
        }
        // Non-vacuity: the SCRIPTED policy wrote the intent (no command, no
        // manual store write) and the settle topped the dry tank to full.
        assert!(
            world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { units: 4, .. })),
            "policy-driven top-to-full refuel happened (4 lots, dry -> full)"
        );
        assert_eq!(world.ships.fuel_mass[0], 1.0e-9, "topped to capacity: 4 * 2.5e-10");
        assert_eq!(world.ships.credits_micros[0], 1_000_000 - 20_000, "4 units at 5_000");
    }

    #[test]
    fn refuel_policy_gates_deterministically() {
        use crate::world::World;
        let no_refuel = |world: &mut crate::world::World, arm: &str| {
            assert!(
                !world
                    .events_mut()
                    .since(Tick(0))
                    .iter()
                    .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
                "{arm}: the policy must not have produced a refuel"
            );
        };

        // (a) !scripted craft: gym-controlled rows are the ingest verb's job.
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].scripted = false;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "!scripted");

        // (b) pirate rows: per-class endurance spec this rung (spec §6, OD-6) —
        //     the policy is non-pirate by construction.
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].role = crate::stores::CraftRole::Pirate;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "pirate");

        // (c) headroom < 1 lot: a full tank writes no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "full-tank");

        // (d) wallet below one unit at the dock's live price: no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "wallet-short");

        // (e) undocked: no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = crate::math::Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut Vec::new());
        no_refuel(&mut world, "undocked");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_policy` → expected failure:** `credit_identity_holds_across_refuels_and_policy_is_self_running` fails at the non-vacuity assert — `policy-driven top-to-full refuel happened (4 lots, dry -> full)` (no stage writes the intent yet; the gates test passes vacuously and is armed by the implementation).
- [ ] **Step 3: implement** in `economy.rs`, after `run_purchase_policies` (~:1103):

```rust
/// Scripted refuel-intent stage — stage 1c3b, world-gets-big §5 (after
/// `run_purchase_policies`, before `resolve_purchases`). Writes the transient
/// `pending_refuel` intent for every scripted NON-PIRATE craft (pirates keep
/// the per-class endurance spec this rung — spec §6/OD-6) that is docked at
/// ANY station (`body_pos` at `prev == t-1`, the try_load frame), has tank
/// headroom for at least one lot, and holds a wallet covering ONE unit at the
/// dock's live Fuel price. Top-to-full, threshold-free: no taste scalar, no
/// target level — the 1d2 settle buys `min(need, stock, afford)` lots.
///
/// Inert by default: `lot_mass == 0.0` early-returns (the trophic-inertness
/// gate). Scripted stage: skips `!scripted` craft; never clobbers an intent
/// already written by this tick's ingest (the run_purchase_policies
/// discipline).
#[allow(clippy::too_many_arguments)]
pub fn run_refuel_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
) {
    if refuel.lot_mass <= 0.0 {
        return; // the trophic-inertness gate (world-gets-big §5)
    }
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Resource::Fuel.index();
    for crow in 0..ships.ids.len() {
        // Scripted stages skip gym-controlled craft (spec §5).
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        // Never clobber an intent already written by this tick's ingest.
        if ships.pending_refuel[crow].is_some() {
            continue;
        }
        if ships.role[crow] == CraftRole::Pirate {
            continue; // per-class endurance spec this rung (OD-6)
        }
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };
        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        // The SAME quantization expression as the 1d2 settle, so policy intent
        // and same-tick settle agree on "headroom >= 1 lot".
        let need = ((eff.fuel_capacity - ships.fuel_mass[crow]) / refuel.lot_mass).floor();
        if need < 1.0 {
            continue;
        }
        // Wallet covers ONE unit at the dock's live price (the settle re-gates
        // everything, including unit_price < 1).
        if ships.credits_micros[crow] < stations.price_micros[srow][fuel_r] {
            continue;
        }
        ships.pending_refuel[crow] = Some(());
    }
}
```

- [ ] **Step 4: wire stage 1c3b in `World::step`**, after the `run_purchase_policies` call (world.rs:766), before `resolve_purchases` (:776):

```rust
        // (1c3b) scripted refuel policies (world-gets-big §5): write the
        //        `pending_refuel` INTENT for docked, scripted, non-pirate craft
        //        with >= 1 lot of headroom and a wallet covering one unit at
        //        the dock's live price; consumed by stage 1d2 below the SAME
        //        tick — the column stays None at every hash point. Top-to-full,
        //        threshold-free. Inert at lot_mass == 0.0 (the trophic-
        //        inertness gate). PRE-physics: body_pos sampled at
        //        `next - 1 == cur` (the try_load frame).
        crate::economy::run_refuel_policies(
            &mut self.ships,
            &self.config.craft,
            &self.stations,
            &self.bodies,
            &self.eph,
            &self.config.refuel,
            next,
        );
```

- [ ] **Step 5: run + expected pass.** `cargo test -p jumpgate-core refuel` all green (including the Task-1.2.3 tests — they step exactly once with the intent pre-written, so the policy's no-clobber guard keeps their outcomes bit-identical). `cargo test -p jumpgate-core` green; `cargo clippy --all-targets -- -D warnings` clean.

  Note (untestable-by-construction, documented in code instead): the no-clobber guard cannot be black-box-distinguished — the intent payload is the unit type, so an overwrite of `Some(())` with `Some(())` is unobservable. The guard is kept because it is the `run_purchase_policies` discipline and protects any future payload.
- [ ] **Step 6: commit.**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): run_refuel_policies at stage 1c3b — scripted top-to-full refuel intent

Docked-at-ANY-station (t-1 frame), headroom >= 1 lot (the settle's exact
quantization expression), wallet covers one unit. Non-pirate by
construction (OD-6); skips !scripted; never clobbers ingest intent;
inert at lot_mass == 0. Credit identity pinned across self-running
refuels.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.5: FUEL-C1 — `resolve_refuels` re-derives `dv_remaining` for refueled Seeking craft

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (inside `resolve_refuels`, after the tank-leg write, before the `Refueled` emit; test `refuel_rederives_dv_for_seeking_craft_fuel_c1`)

- [ ] **Step 1: failing test first** (in `economy.rs` tests):

```rust
    #[test]
    fn refuel_rederives_dv_for_seeking_craft_fuel_c1() {
        use crate::types::NavDest;
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        // The escrow-lock trap (autopilot coast-at-zero, autopilot.rs:61):
        // Seeking with an exhausted budget and a dry tank. Without FUEL-C1 the
        // craft coasts FOREVER after the refill — nothing re-derives
        // dv_remaining after dispatch; it is only ever decremented.
        world.ships.nav[0] = NavState::Seeking {
            dest: NavDest::Position(crate::math::Vec3::new(0.5, 0.0, 0.0)),
            dv_remaining: 0.0,
        };
        world.step(&mut Vec::new());

        // The 1d2 refill: 4 lots, dry -> full — the same clamped-write expression.
        let refilled: f64 = (0.0 + 4.0 * 2.5e-10_f64).min(1.0e-9);
        // 1d2 is PRE-physics: the re-derived budget unlocked a SAME-TICK burn
        // drawn from the refilled tank (prev_fuel untouched until stage 4).
        let dv_applied: f64 = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .find_map(|e| match e.kind {
                EventKind::ThrustApplied { dv, .. } => Some(dv),
                _ => None,
            })
            .expect("the re-derived budget unlocked a same-tick burn");
        assert!(
            world.ships.fuel_mass[0] < refilled,
            "the same-tick burn drew from the REFILLED tank"
        );
        // Exact pin: dv_remaining == tsiolkovsky(refilled tank) − this tick's
        // burn — the SAME derivation both dispatch sites use (try_load /
        // ingest dv_from_fuel), then the step loop's single subtraction.
        let dv_full = crate::math::tsiolkovsky_dv(1e-2, 1e-9, refilled);
        match world.ships.nav[0] {
            NavState::Seeking { dv_remaining, .. } => {
                assert_eq!(
                    dv_remaining,
                    dv_full - dv_applied,
                    "budget re-derived from the refilled tank, then burned once"
                );
                assert!(dv_remaining > 0.0, "no more coast-at-zero with a full tank");
            }
            other => panic!("expected Seeking, got {other:?}"),
        }
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_rederives` → expected failure:** panics at `expect("the re-derived budget unlocked a same-tick burn")` — with `dv_remaining <= 0` the autopilot returns `(Vec3::ZERO, 0.0)` (autopilot.rs:61), so no `ThrustApplied` ever fires.
- [ ] **Step 3: implement.** In `resolve_refuels`, immediately after the tank-leg write (`ships.fuel_mass[crow] = …`) and the `tank_after_permille` line, BEFORE the `Refueled` emit:

```rust
        // FUEL-C1 (world-gets-big §5): re-derive the Δv budget for a refueled
        // craft that is currently Seeking — a pure function of hashed state
        // (effective spec + the just-written fuel_mass), the SAME tsiolkovsky
        // derivation both dispatch sites already use (economy::try_load,
        // ingest's dv_from_fuel). Closes the same-tick dispatch-then-refuel
        // race: the autopilot treats `dv_remaining <= 0` as a permanent coast
        // even with a full tank, locking the contract's escrow forever.
        if let NavState::Seeking { dest, .. } = ships.nav[crow] {
            let dv = crate::math::tsiolkovsky_dv(
                eff.exhaust_velocity,
                eff.dry_mass,
                ships.fuel_mass[crow],
            );
            ships.nav[crow] = NavState::Seeking { dest, dv_remaining: dv };
        }
```

  (This runs only on the settle path — `units >= 1` — so a skipped refuel never touches `nav`. Idle / DirectThrust craft are untouched by the `if let`.)
- [ ] **Step 4: run + expected pass.** `cargo test -p jumpgate-core refuel` all green (the earlier exact-settle tests use Idle craft — nav untouched, payloads unchanged). `cargo test -p jumpgate-core` green; zero goldens move (the write is to already-hashed `nav` state on a path no existing fixture reaches).
- [ ] **Step 5: commit.**

```
git add crates/jumpgate-core/src/economy.rs
git commit -F - <<'EOF'
feat(world-gets-big): FUEL-C1 — resolve_refuels re-derives dv_remaining for Seeking craft

A refueled craft mid-Seek gets its Δv budget re-derived from the refilled
tank (the shared tsiolkovsky derivation of both dispatch sites), closing
the dispatch-then-refuel coast-at-zero escrow lock (autopilot.rs:61).
Settle-path only; Idle/DirectThrust nav untouched.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.6: PLAY-C1 dispatch fuel-eligibility + `Refuel` ingest verb + `ContractFailed` narration (FuelEmpty-cause ONLY)

**Depends on Task 1.1 (eps 1e-9 → 1e-11, the fuel-edge section's commit):** the ASSIGN filter compares `fuel_mass > FUEL_EMPTY_EPS`; every band tank is exactly the OLD eps (scenario.rs:113/126), so with eps still 1e-9 the filter would blacklist every full-tank trophic hauler and break the phase-exit digest. Do not start this task until 1.1 is merged.

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (ASSIGN per-craft gates ~:533-545; `FailureCause` derives ~:1237; `settle_contract_failure` ~:1262-1309 signature + capture + emit; `resolve_failures` ~:1194-1231 signature; tests)
- Modify: `crates/jumpgate-core/src/contract.rs` (`EventKind::ContractFailed` appended after `Refueled`)
- Modify: `crates/jumpgate-core/src/types.rs` (`CommandKind::Refuel` after `BuyUpgrade` ~:76)
- Modify: `crates/jumpgate-core/src/ingest.rs` (`ingest_commands` arm ~:193-204; test) — the legacy `ingest_into` path's catch-all `_ =>` arm (:138-141) covers the new variant with NO edit (economy kinds fall through, logged + event only)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 3c `resolve_failures` call :1012-1018 gains `next, &mut self.events`)
- Modify: `crates/jumpgate-core/src/pirate.rs` (the Robbed `settle_contract_failure` call ~:236 gains `tick, events`)

- [ ] **Step 1: failing tests first.**

  (i) The ASSIGN filter (economy.rs tests, the `scripted_assign_filters_oversized_contracts` / `capacity_world_fixture` pattern):

```rust
    #[test]
    fn scripted_assign_filters_dry_tank_craft_play_c1() {
        use crate::world::World;
        // The capacity-fixture board with scripted ASSIGN ON (stagger 1) and a
        // claimable lot (qty 5 <= base capacity): the only hauler's TANK is the
        // sole eligibility variable under test.
        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        cfg.craft[0].fuel_mass = 0.0; // DRY tank (<= FUEL_EMPTY_EPS)
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..8 {
            world.step(&mut empty);
        }
        // World-truth feasibility, never claim-and-strand: the board keeps the
        // offer; the craft is the ADRIFT end state (role Idle ∧ dry tank).
        assert_eq!(world.contracts.status[0], ContractStatus::Offered, "dry tank: never claimed");
        assert_eq!(world.ships.role[0], CraftRole::Idle, "stays Idle forever");
        assert_eq!(world.ships.contract[0], None, "no binding written");

        // Control arm: the stock tank (1e-9 > the re-baked eps 1e-11, Task 1.1)
        // claims it — the filter, not the fixture, was the gate.
        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        for _ in 0..8 {
            world.step(&mut empty);
        }
        assert_ne!(world.contracts.status[0], ContractStatus::Offered, "live tank: claimed");
    }
```

  (ii) The ingest verb (ingest.rs tests, the `buy_upgrade_writes_pending_intent…` clone at :440-469):

```rust
    #[test]
    fn refuel_writes_pending_intent_logs_and_emits_action_ingested() {
        // The Refuel ingest arm is INTENT-ONLY (the BuyUpgrade template): it
        // writes the transient `pending_refuel` column and nothing else — the
        // settle (dock check, quantization, four legs) is deferred to
        // `resolve_refuels` (stage 1d2), which consumes the intent the same
        // tick. Top-to-full: the verb carries no quantity.
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::Refuel,
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        assert_eq!(world.ships.pending_refuel[0], Some(()), "intent column set");
        assert_eq!(world.ships.fuel_mass[0], world.ships.prev_fuel[0], "no tank movement at ingest");
        assert_eq!(world.ships.credits_micros[0], 0, "no credit movement at ingest");
        assert_eq!(world.log_mut().at(Tick(2)).len(), 1, "command logged at tick");
        let emitted = world.events_mut().since(Tick(0));
        assert_eq!(emitted.len(), 1);
        assert!(matches!(
            emitted[0].kind,
            EventKind::ActionIngested { target } if target == Target::Entity(EntityRef::Craft(id0))
        ));
    }
```

  (iii) The narration (economy.rs tests; `starved_two_body_contract_fixture` + the documented mid-flight-drain field-write pattern, economy.rs:2526-2533; add `CorporationId` to the test-mod imports):

```rust
    #[test]
    fn fuel_empty_failure_emits_contract_failed_with_actual_refund() {
        use crate::world::World;
        // Deadhead arm (Accepted): origin is the far station, so the hauler
        // launches empty-handed; we force the depletion edge with a mid-flight
        // drain (the documented field-write pattern) instead of waiting out
        // the burn.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        assert_eq!(world.contracts.status[0], ContractStatus::Accepted, "deadhead leg armed");
        let escrow_before = world.contracts.escrow_micros[0];
        assert!(escrow_before > 0, "escrow held");

        // Drain mid-deadhead: prev_fuel (stage-4 copy of last tick's tank) is
        // still > eps, so the next step's edge detector fires FuelEmpty.
        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());

        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed {
                    contract,
                    hauler,
                    cause: FailureCause::FuelEmpty,
                    escrow_refunded_micros,
                    cargo_lost: 0,
                } if contract == cid && hauler == craft && escrow_refunded_micros == escrow_before
            )),
            "FuelEmpty failure narrated with the ACTUAL refund (deadhead: no cargo lost)"
        );

        // Stale-corp degrade arm: the refund leg is skipped (escrow stays put,
        // the credit identity holds) and the event reports the ACTUAL 0 refund.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        world.contracts.corp[0] = CorporationId { slot: 99, generation: 0 }; // stale row
        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(world.contracts.escrow_micros[0] > 0, "escrow stays put on the degrade arm");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed { escrow_refunded_micros: 0, .. }
            )),
            "degrade arm reports the actual 0 refund"
        );
    }

    #[test]
    fn robbed_teardown_is_not_narrated_by_contract_failed() {
        use crate::world::World;
        // Single-emit law (world-gets-big §7): the settle body emits NOTHING
        // for FailureCause::Robbed — the 3b2 caller owns the Robbed narration.
        // Drive to the one-tick CargoLoaded window (the fuel_empty_mid_deadhead
        // arm-2 pattern), then call the settle body directly.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(0, 1)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds); // docked at origin: accept + load settle this tick
        assert_eq!(world.contracts.status[0], ContractStatus::CargoLoaded, "the load-tick window");

        let mut ev = crate::events::EventStream::default();
        settle_contract_failure(
            &mut world.contracts,
            &mut world.corporations,
            &mut world.ships,
            &mut world.econ,
            0,
            FailureCause::Robbed,
            Tick(99),
            &mut ev,
        );
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            !ev.since(Tick(0)).iter().any(|e| matches!(e.kind, EventKind::ContractFailed { .. })),
            "Robbed teardown emits NO ContractFailed (Robbed narrates itself at 3b2)"
        );
    }
```

- [ ] **Step 2: run → expected failures.** `cargo test -p jumpgate-core scripted_assign_filters_dry` → control arm passes but the dry arm fails: `dry tank: never claimed` (the contract IS claimed — no filter exists). `cargo test -p jumpgate-core refuel_writes_pending` → `E0599: no variant named `Refuel` found for enum `CommandKind``. `cargo test -p jumpgate-core fuel_empty_failure_emits` → `E0599: no variant named `ContractFailed``.
- [ ] **Step 3: implement the ASSIGN filter.** In `run_scripted_dispatch`'s per-hauler gate block (economy.rs:533-545), directly after the stagger gate, before the `capacity` derivation:

```rust
        // PLAY-C1 (world-gets-big §5): dispatch eligibility requires a live
        // tank — world-truth feasibility filter-at-choice (the capacity-filter
        // precedent at the per-contract loop below), never claim-and-strand.
        // A stranded craft stays Idle forever: the ADRIFT end state is
        // role Idle ∧ fuel <= eps, matched by detection, not by shaping.
        if ships.fuel_mass[crow] <= crate::events::FUEL_EMPTY_EPS {
            continue;
        }
```

- [ ] **Step 4: implement the verb.** `types.rs`, after `BuyUpgrade` (:76):

```rust
    /// Intent to top up propellant at the docked station (world-gets-big §5):
    /// ingestion writes the transient `pending_refuel` column only; the settle
    /// (dock check, integer quantization, four legs, Δv re-derivation) lives in
    /// `resolve_refuels` (stage 1d2), which consumes the intent the same tick.
    /// Top-to-full, threshold-free: the verb carries no quantity.
    Refuel,
```

  `ingest.rs`, after the `BuyUpgrade` arm (:204):

```rust
                CommandKind::Refuel => {
                    // Record INTENT only (the BuyUpgrade template): write the
                    // transient `pending_refuel` column. The settle is DEFERRED
                    // to `resolve_refuels` (stage 1d2), which consumes the
                    // intent the SAME tick (on a lot-0 world the stage consumes
                    // it as a deterministic no-op). A stale craft id is a
                    // deterministic skip; the command is still logged above and
                    // ActionIngested still fires below (the seam).
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_refuel[i] = Some(());
                    }
                }
```

  (`ingest_into`'s catch-all `_ =>` arm at :138-141 already covers the variant — no edit, matching the BuyUpgrade precedent.)
- [ ] **Step 5: implement the narration.** `contract.rs`, append after `Refueled`:

```rust
    /// A contract failed on propellant exhaustion (world-gets-big §7) —
    /// emitted in `settle_contract_failure` for `FailureCause::FuelEmpty`
    /// ONLY (the robbery path keeps its own `Robbed` narration at the 3b2
    /// emission site; single emit path preserved). `escrow_refunded_micros`
    /// is the ACTUAL refund — the stale-corp degrade arm reports 0;
    /// `cargo_lost` the qty accounted into `consumed` (0 on a deadhead leg).
    /// Today the failure path is silent; the tragedy becomes visible.
    ContractFailed {
        contract: ContractId,
        hauler: CraftId,
        cause: crate::economy::FailureCause,
        escrow_refunded_micros: i64,
        cargo_lost: u32,
    },
```

  `economy.rs`: give `FailureCause` the event-payload derives (it currently has none):

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FailureCause {
```

  Grow `settle_contract_failure` (economy.rs:1262): signature gains `tick: Tick, events: &mut EventStream` after `cause`; body changes — capture the ACTUAL refund before the leg, the lost qty at the sink leg, and emit cause-gated at the end (after `contracts.status[kidx] = ContractStatus::Failed;`):

```rust
    // Refund escrow -> owning corp treasury (credit TRANSFER; identity
    // invariant). Captured BEFORE the leg: a stale corp row skips the refund
    // (the escrow stays put so the identity holds) and the narration reports
    // the ACTUAL 0 — what happened, not what was owed.
    let corp = contracts.corp[kidx];
    let mut escrow_refunded_micros: i64 = 0;
    if let Some(corp_row) = corporations.ids.dense_index(corp.slot, corp.generation) {
        escrow_refunded_micros = contracts.escrow_micros[kidx];
        corporations.treasury_micros[corp_row] =
            corporations.treasury_micros[corp_row].saturating_add(contracts.escrow_micros[kidx]);
        contracts.escrow_micros[kidx] = 0;
    }
    // Cargo loss: account the lost cargo as a SINK leg, then release the hauler.
    let mut cargo_lost: u32 = 0;
    if let Some(hauler) = contracts.hauler[kidx]
        && let Some(crow) = ships.index_of(hauler)
    {
        if let Some((resource, qty)) = ships.cargo[crow] {
            counters.consumed[resource.index()] =
                counters.consumed[resource.index()].saturating_add(qty as i64);
            ships.cargo[crow] = None;
            cargo_lost = qty;
        }
        ships.contract[crow] = None;
        ships.role[crow] = CraftRole::Idle;
    }
    contracts.status[kidx] = ContractStatus::Failed;
    // Single-emit narration (world-gets-big §7): FuelEmpty-cause ONLY — the
    // robbery path emits `Robbed` at its own 3b2 site; emitting here too would
    // double-narrate one teardown. `contracts.hauler` survives the release
    // above (only the SHIP's columns were cleared), so the payload binds.
    if matches!(cause, FailureCause::FuelEmpty)
        && let Some(hauler) = contracts.hauler[kidx]
    {
        events.emit(Event {
            tick,
            kind: EventKind::ContractFailed {
                contract: contract_id(contracts, kidx),
                hauler,
                cause,
                escrow_refunded_micros,
                cargo_lost,
            },
        });
    }
```

  Grow `resolve_failures` (economy.rs:1194): signature gains `tick: Tick, events: &mut EventStream`; the settle call passes them through; DELETE the now-stale "No dedicated failure event" comment (economy.rs:1219-1221) and replace with `// ContractFailed (FuelEmpty-cause) is emitted inside the settle body (§7).`. Update the two callers: `world.rs` stage 3c (:1012-1018) appends `next, &mut self.events` (legal — the FuelEmpty ids were already lifted into `failed_craft`, the immutable borrow is dropped); `pirate.rs` ~:236 appends `tick, events` (both already in scope at the Robbed settle site).
- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core scripted_assign_filters_dry refuel_writes_pending fuel_empty_failure robbed_teardown` all green. `cargo test --workspace` green — in particular `starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss` (world.rs:2208) and `fuel_empty_mid_deadhead_refunds_escrow` (economy.rs:2465+) still pass: the legs are unchanged, only captured-and-narrated. `cargo clippy --all-targets -- -D warnings` clean. Zero goldens move (events are unhashed; the ASSIGN filter never binds on any existing fixture whose dispatching craft has fuel > eps — verify with the full suite, not by assumption).
- [ ] **Step 7: commit.**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/contract.rs \
  crates/jumpgate-core/src/types.rs crates/jumpgate-core/src/ingest.rs \
  crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/pirate.rs
git commit -F - <<'EOF'
feat(world-gets-big): PLAY-C1 dispatch fuel-eligibility + Refuel verb + ContractFailed narration

ASSIGN requires fuel_mass > FUEL_EMPTY_EPS (filter-at-choice, the
capacity precedent) — a stranded craft stays Idle, on the record.
CommandKind::Refuel = intent-only ingest (BuyUpgrade template).
settle_contract_failure grows (tick, events) and emits ContractFailed
for FailureCause::FuelEmpty ONLY, reporting the ACTUAL refund (stale-corp
arm: 0) and lost qty; Robbed keeps its own narration. Depends on the
eps re-bake (1e-11): band tanks sit at the OLD eps exactly.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.7: the trophic-inertness gate proven — lot-0 unit pin + the cross-branch 2000-tick digest (the phase-1 exit)

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (test `refuel_default_is_inert_and_consumes_stray_intents`)
- Modify: `crates/jumpgate-core/src/scenario.rs` (one assert added to `scenario_trophic_shape` ~:337-466)
- No new tracked files. Digest artifacts live under `runs/wgb_phase1_inert/` — **never staged** (runs/ is never committed).

- [ ] **Step 1: failing test first** (economy.rs tests):

```rust
    #[test]
    fn refuel_default_is_inert_and_consumes_stray_intents() {
        use crate::world::World;
        // RefuelCfg::default() (lot_mass == 0.0): BOTH stages no-op — the named
        // trophic-inertness gate. A manual intent on a lot-0 world is consumed
        // (the all-None hash invariant) but settles NOTHING.
        let (mut world, _h) = World::reset(vendor_world_fixture(false)).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_eq!(world.ships.pending_refuel[0], None, "stray intent consumed on the lot-0 world");
        assert_eq!(world.ships.fuel_mass[0], 1e-9, "tank untouched");
        assert_eq!(world.ships.credits_micros[0], 1_000_000, "wallet untouched");
        assert_eq!(world.corporations.treasury_micros[0], 0, "no treasury movement");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "no Refueled event on a lot-0 world"
        );
        let _ = crate::hash::state_hash(&world); // the all-None debug_assert holds
    }
```

  And in `scenario_trophic_shape` (scenario.rs, alongside the dispatch/policy value asserts ~:445-453):

```rust
        assert_eq!(
            cfg.refuel.lot_mass, 0.0,
            "the trophic-inertness gate: the refuel verb stays OFF on the band"
        );
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_default_is_inert scenario_trophic_shape` → expected pass immediately** (both behaviors were built in Tasks 1.2.3/1.2.4; this step PINS them — if either fails, STOP: the gate is broken, fix before the digest). This is the one task whose tests pin rather than drive; the failing-first evidence for the gate is the digest in Step 3, which fails loudly on any divergence.
- [ ] **Step 3: the cross-branch 2000-tick digest (the digest-tests-are-determinism-not-golden law; the media-rung Task 9.2 procedure).** Baseline build = the last commit BEFORE phase 1's first commit (i.e. before Task 1.1's eps change — by then phase 0a/0b instrumentation is in BOTH builds, so stdout/JSONL line sets match). Record that commit hash when phase 1 starts; here `<PRE>` stands for it.

```
git worktree add /tmp/wgb-pre-phase1 <PRE>
mkdir -p runs/wgb_phase1_inert
( cd /tmp/wgb-pre-phase1 && \
  for S in 7 23; do \
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --seed $S --ticks 2000 \
      --jsonl /home/john/jumpgate/runs/wgb_phase1_inert/base-s$S.jsonl \
      > /home/john/jumpgate/runs/wgb_phase1_inert/base-s$S.out; \
  done )
for S in 7 23; do \
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed $S --ticks 2000 \
    --jsonl runs/wgb_phase1_inert/head-s$S.jsonl \
    > runs/wgb_phase1_inert/head-s$S.out; \
done
for S in 7 23; do \
  diff runs/wgb_phase1_inert/base-s$S.jsonl runs/wgb_phase1_inert/head-s$S.jsonl && \
  diff runs/wgb_phase1_inert/base-s$S.out  runs/wgb_phase1_inert/head-s$S.out; \
done
git worktree remove /tmp/wgb-pre-phase1
```

  **Expected: every diff exits 0 with no output — byte-identical.** The whole phase (eps re-bake + RefuelCfg + both stages + dv-rederive + dispatch filter + verbs + events) must be bit-inert on the band: eps appears in no physics expression, band fuel never approaches 1e-11, the dispatch filter never binds above eps, lot 0 gates both stages, and events that never fire print nothing. **Any divergence is a determinism break: STOP and bisect commit-by-commit; never rationalize a diff.** This digest green is the phase-1 exit (spec §9; W12's trophic arm).
- [ ] **Step 4: replay determinism at HEAD.**

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 2000 --replay-check
```

  → expected: `replay-check OK` (exercises the pending_refuel all-None assert at every recorded hash point, in a debug-assertions-off release build AND under the recorded-run hashing).
- [ ] **Step 5: full phase gates.** `cargo test --workspace` green (record the count); `cargo clippy --all-targets -- -D warnings` clean; `PYTHONPATH=/home/john/jumpgate/python pytest python/tests` green (the gym crate gained only a defaulted config field). Grep-verify the golden inventory: exactly ONE changed `GOLDEN_CONFIG_HASH` (Task 1.2.1), `HASH_FORMAT_VERSION` still `5`, `GOLDEN_ZERO_STATE_HASH` and the hash.rs:1108 trajectory golden byte-identical to `<PRE>`:

```
git diff <PRE> -- crates/jumpgate-core/src/hash.rs | grep -E "GOLDEN|FORMAT_VERSION"   # expect: no hits
git diff <PRE> -- crates/jumpgate-core/src/config.rs | grep GOLDEN_CONFIG_HASH          # expect: exactly the one re-pin
```

- [ ] **Step 6: commit (tests only — the digest artifacts stay untracked).**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
test(world-gets-big): pin the trophic-inertness gate — lot-0 no-op + band factory OFF

RefuelCfg::default() consumes stray intents and settles nothing (hash
invariant exercised); scenario_trophic pins lot_mass == 0.0. Cross-branch
2000-tick digest vs pre-phase-1 (seeds 7/23, stdout+JSONL) byte-identical
and replay-check OK — the phase-1 exit (spec §9, W12 trophic arm);
digest artifacts in runs/ (untracked, never staged).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

## Cross-section handoffs (named, not built here)

- **Chronicle arms** for `Refueled`/`ContractFailed` in `trophic_run.rs` (`chronicle_subject`'s catch-all `_ => None` at :262 means the new variants compile but silently vanish from the chronicle) and the per-craft ADRIFT epilogue — owned by the lab/chronicle section (spec §7 printer-side, phase 0b/2 beats).
- **FUEL-line refuel fields** (`refuels, refuel_spend_micros, strandings, adrift_end`) and **TrophicSample** additive fields (`per_station_fuel_stock`, `per_station_fuel_price`, `refuels`, `refuel_units`, `refuel_spend_micros`) — spec §8 says they "append with the mechanic"; they ride the lab section's anchored-line + version-gated-regex work (TROPHIC-C2/LAB-C2), not this one.
- **W9 liveness window** (max non-terminal contract age per run) — recorded lab window, lab section.
- **Port corp creation** (`CorporationInit { treasury_micros: 0, .. }` + `refuel.corp_index` pointed at it) — the `scenario_frontier` factory task, phase 2; the binding and its stale-row degrade are built and tested here.
