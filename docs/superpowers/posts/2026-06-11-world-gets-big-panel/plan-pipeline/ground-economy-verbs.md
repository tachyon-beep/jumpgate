# Grounding extract — economy verbs, prices, corps (HEAD e7e490e)

All paths relative to `/home/john/jumpgate/crates/jumpgate-core/src/`. All money i64 microcredits. `N_RESOURCES = 2`, `Resource::{Ore=0, Fuel=1}` (`economy.rs:9-24`); per-station arrays + hash use `Resource::index()` dense order, APPEND-ONLY.

## 1. The BuyUpgrade verb end-to-end (the refuel-verb clone template)

### Intent write — ingest arm (`ingest.rs:193-204`)
```rust
CommandKind::BuyUpgrade { kind } => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_upgrade[i] = Some(kind);
    }
}
```
Stale craft id = deterministic skip; command still logged; `ActionIngested` still fires (`ingest.rs:207-210`). Intent column: `CraftStore.pending_upgrade: Vec<Option<UpgradeKind>>` (`stores.rs:203`), pushed `None` in `push` (`stores.rs:280`) and in `World::reset` (`world.rs:294`). NOT hashed — instead hash.rs ASSERTS it is all-`None` at every hash point (`hash.rs:303-308`: `world.ships.pending_upgrade.iter().all(Option::is_none)` debug-style assert with message "pending_upgrade must be fully consumed (all None) at every state-hash point"). A `pending_refuel` column must join this assert.

### Scripted intent writer — `run_purchase_policies`, stage 1c3 (`economy.rs:1012-1103`)
Signature (`economy.rs:1012-1022`): `(ships: &mut CraftStore, craft_cfg: &[CraftInit], stations: &StationStore, stations_cfg: &[StationInit], bodies: &BodyStore, eph: &Ephemeris, trophic: &TrophicCfg, shipyard: &ShipyardCfg, tick: Tick)` — no events param (intent only). Early return when both arms inert (`:1025-1029`). Loop `0..ships.ids.len()` dense order; skips `!scripted` craft via `craft_cfg.get(crow).is_some_and(|c| !c.scripted)` (`:1033`); **never clobbers an ingest-written intent**: `if ships.pending_upgrade[crow].is_some() { continue; }` (`:1037`). Hauler arm gates: `CraftRole::Idle`, no contract (`:1069`), `docked_at_vendor` (`:1072`), ladder rung, wallet headroom `price.saturating_mul(shipyard.buy_headroom_milli as i64) / 1000` (`:1094`). Pirate arm (`:1041-1067`): lying low (`tick >= p.lie_low_until` → skip), at hideout body within `ARRIVAL_RADIUS`, full price no headroom.

### Settle — `resolve_purchases`, stage 1d (`economy.rs:853-922`)
Signature (`:853-863`): `(ships: &mut CraftStore, stations: &StationStore, stations_cfg: &[StationInit], bodies: &BodyStore, eph: &Ephemeris, corporations: &mut CorporationStore, shipyard: &ShipyardCfg, tick: Tick, events: &mut EventStream)`.

**The always-consume-then-gate idiom** (`:866-875`):
```rust
let prev = Tick(tick.0.saturating_sub(1));
for crow in 0..ships.ids.len() {
    let Some(kind) = ships.pending_upgrade[crow] else { continue; };
    ships.pending_upgrade[crow] = None;   // ALWAYS consume FIRST
    if !docked_at_vendor(...) { continue; }  // then gate (skip = plain continue)
```
Deterministic no-op skip arms, in order: not docked (`:873`), at cap `level >= cap` (`:889`), ladder row missing `ladder.get(level as usize)` let-else (`:894` — "no unwraps on impossible states"), wallet short `credits_micros[crow] < price` (`:897`), stale corp row (`:903-906`):
```rust
let yard_row = shipyard.corp_index as usize;
if corporations.ids.id_at(yard_row).is_none() { continue; }
```
Settle = pure transfer (`:908-910`): `ships.credits_micros[crow].saturating_sub(price)` / `corporations.treasury_micros[yard_row].saturating_add(price)`; then count bump and `EventKind::UpgradePurchased { craft, kind, level, price_micros }` (`:917-920`).

### Dock predicate + the t−1 frame (`economy.rs:931-952`)
`docked_at_vendor(ships, crow, stations, stations_cfg, bodies, eph, prev)` — any station row with `stations_cfg.get(srow).is_some_and(|s| s.sells_upgrades)`, body pos via `eph.body_pos(bodies.eph_index[brow], prev)`, distance `<= crate::autopilot::ARRIVAL_RADIUS`. **`prev = tick−1` is the try_load frame**: stages 1c–1d run PRE-physics so `ships.pos` is the tick-`cur` state and body_pos must be sampled at `next − 1 == cur` (`world.rs:710-715`, `:768-775`). A refuel dock predicate cloning this but for "ANY station" simply drops the `sells_upgrades` filter.

### Stage ordering in `World::step` (`world.rs`)
(1b) `run_scripted_dispatch` (`:693-708`) → (1c) `resolve_contracts` (`:716-727`) → (1c2) `run_pirate_brains`, gated `engage_radius_au > 0.0` (`:736-747`) → **(1c3) `run_purchase_policies`** (`:756-766`) → **(1d) `resolve_purchases`** (`:776-786`) → physics (`:797+`) → (3d) `update_prices` on the reprice clock (`:1027-1036`). Spec amendment puts `run_refuel_policies` at 1c3b and `resolve_refuels` at 1d2, i.e. directly after their precedents. `resolve_failures` (escrow refund on FuelEmpty/Robbed) is at `world.rs:1012-1018`, AFTER physics, BEFORE 3d.

## 2. Corps / treasuries — the Yard precedent, what a Port corp touches

- Store (`economy.rs:125-150`): `CorporationStore { ids: SlotMap<()>, treasury_micros: Vec<i64>, home_station: Vec<StationId> }`; `push(treasury_micros, home_station) -> CorporationId`, dense `slot == row` debug_assert.
- Config (`config.rs:120-123`): `CorporationInit { treasury_micros: i64, home_station_index: usize }`; seeded at `World::reset` (`world.rs:330-341`) with `BadEconomyRef` validation of `home_station_index` BEFORE tick 0.
- The Yard binding is just `ShipyardCfg.corp_index: u32` (`config.rs:322`) — "Corporation (config index) credited with every upgrade payment"; dense slot == row, so the settle indexes `treasury_micros[corp_index as usize]` directly after the `id_at` liveness check (`economy.rs:903-910`).
- **Adding a Port corp** = append a `CorporationInit { treasury_micros: 0, home_station_index: … }` to the scenario factory + a `corp_index` field on the new `RefuelCfg`. config_hash folds corp COUNT at `config.rs:572` and per-corp `(treasury_micros, home_station_index)` at `:585-588` — so a new corp moves config_hash via existing folds; the new RefuelCfg fields must be appended at the TAIL with an exhaustive destructure (the ShipyardCfg model, `config.rs:672-689`; "a NEW field is a COMPILE ERROR until explicitly folded", `:620-621`). `GOLDEN_CONFIG_HASH = 0xee02_df67_1889_78dc` (`config.rs:745`) re-pins on any fold change.
- state_hash: per-corp `slot, gen, treasury_micros, home_station` folded at `hash.rs:432` (section 23, `hash.rs:111`); craft `credits_micros` word 18 (`hash.rs:328`), `upgrades` words 27a/27b (`:356-357`), contract `escrow_micros` (`:454`). `HASH_FORMAT_VERSION = 5` (`hash.rs:123`), `GOLDEN_ZERO_STATE_HASH = 0x0f20_843f_ccfd_8c70` (`:129`). Spec §9: NO version bump for refuel (transient column unhashed; fuel_mass already hashed).
- Credit identity: `Σtreasury + Σcredits + Σescrow` invariant — test `credit_identity_holds_across_purchases` (`economy.rs:2229-2273`) sums exactly those three (`:2238-2242`).

## 3. update_prices + PriceCfg (`economy.rs:300-332`, `config.rs:143-161`)

```rust
pub struct PriceCfg {
    pub base_micros: [i64; N_RESOURCES],
    pub cap: [i64; N_RESOURCES],
    pub slope_milli: i64,        // k*1000, default 1800
    pub reprice_interval: u32,   // tick % interval == 0; default 1
}
```
Default `base_micros: [0;2]`, `cap: [1;2]` (`config.rs:154-159`). Curve (`economy.rs:306-314`):
```rust
if price_cfg.cap[r] == 0 { continue; }            // :308-310 — the cap==0 SKIP (div-by-zero guard, price untouched)
let s = stations.stock[row][r].max(0).min(price_cfg.cap[r]);
let p = (price_cfg.base_micros[r] * (2000 - s * price_cfg.slope_milli / price_cfg.cap[r]) / 1000).max(0);
```
Writes + emits `PriceUpdate` only on change (`:315-328`). `s==0 → base*2`; `s==cap → base*(2 − slope/1000)`. Invoked at `world.rs:1027-1036` against fully-settled stock; `reprice_interval > 0` guard avoids modulo-by-zero. Config-hash folds: `slope_milli`, `reprice_interval`, per-resource `base_micros`/`cap` (`config.rs:597-602`). Seeding: `StationInit.initial_price_micros: [i64; N_RESOURCES]` (`config.rs:104`) → `stations.push(body, initial_stock, initial_price_micros)` at reset (`world.rs:319`); both folded per-station into config_hash (`config.rs:577-578`). Spec §5 wants `initial_price_micros[Fuel]` computed from the curve at factory build; Ore stays `cap[Ore]==0` (structurally dead, the verified skip).

Test precedent: `update_prices_linear_deflation_exact_integer` (`economy.rs:1446-1511`) — store-level (no World), exact integer prices per stock row, monotonicity loop, cap==0 resource asserted untouched (777), exactly-one-PriceUpdate-per-changed-row event count.

## 4. ASSIGN dispatch site — where the fuel filter slots (`economy.rs:407-643`)

`run_scripted_dispatch(contracts, stations, ships: &mut CraftStore, craft_cfg, route_evidence: &world::RouteEvidence, media_live: bool, staleness_from_rob_tick: bool, diag: &mut AssignDiag, dispatch: &DispatchCfg, shipyard: &ShipyardCfg, trophic: &TrophicCfg, tick, events)` (`:406-421`). `AssignDiag { decisions, flips, candidate_counts: [u64;7] }` (`:393-404`) — UNHASHED diagnostics, never a behavior input.

Per-hauler eligibility (the craft side, `:533-545`): role Idle (`:534`), scripted (`:540`), stagger gate `tick.0 % stagger != crow as u64 % stagger` (`:543`). **A `fuel_mass > FUEL_EMPTY_EPS` filter (spec PLAY-C1) slots here**, alongside these per-craft gates — note `ships.fuel_mass: Vec<f64>` (`stores.rs:160`).

Per-contract candidate filter — the capacity-filter precedent (`:556-568`):
```rust
if contracts.status[kidx] != ContractStatus::Offered { continue; }
if contracts.qty[kidx] > capacity { continue; }   // :563 — filter-at-choice, never claim-and-revert
let cid = contract_id(contracts, kidx);
if (0..ships.ids.len()).any(|r| ships.contract[r] == Some(cid)) { continue; }
```
`capacity` derived at `:546` via `cargo_capacity(&ships.spec[crow], ships.upgrades[crow], shipyard)` (`:342-349` — derived at read site, never stored). Scoring-off pick = first eligible (lowest ContractId, `:570-572`); scoring-on = evidence argmax with 900-clamp (`:613-620`). Assignment writes only `ships.contract[crow] = Some(cid); ships.role[crow] = CraftRole::Hauler` (`:632-634`); `resolve_contracts` settles next (capacity is ALSO backstopped there for manual/RL paths — accept REVERT, `:653-659` doc).

## 5. consumed[] sinks (resource identity: `Σstock + in_transit == initial + mined − consumed`, `economy.rs:207-217`)

Only two live sink sites today: producer input leg `counters.consumed[r_in.index()] += q` (`economy.rs:270`) and failed-contract cargo loss `saturating_add(qty)` in `settle_contract_failure` (`economy.rs:1301-1302`). Refuel adds the third: `consumed[Fuel] += units` (spec §5 quantization). `EconCounters` is hashed (words 20, `hash.rs:108,395`). Propellant mass itself lives OUTSIDE both identities (spec §5, documented design).

## 6. Test precedents to clone (all in `economy.rs` `#[cfg(test)]`)

- `vendor_world_fixture(sells_upgrades: bool) -> RunConfig` (`:1564-1611`): 1 near-massless body, 1 docked scripted Idle craft (pos ZERO), 1 station on body 0, 1 corp (treasury 0, the Yard at corp_index 0 default), all cfg defaults. Exhaustive RunConfig literal — a new RunConfig field breaks it (compile error = the discipline).
- `buy_cmd(craft, kind)` (`:1613-1619`): `Command { target: Target::Entity(EntityRef::Craft(craft)), kind: CommandKind::BuyUpgrade { kind } }`.
- `assert_purchase_skipped(world, credits_before, upgrades_before, arm)` (`:1623-1641`): no level change, zero wallet movement, treasury untouched, intent cleared, NO event — the skip-arm postcondition helper to clone for refuel skips.
- `purchase_settles_at_vendor` (`:1644-1700`): exact-debit ladder walk, exact event payload match.
- `purchase_skips_deterministically` (`:1703-1743`): four labeled arms — not-docked (pos bumped + `prev_pos` mirrored, `:1712-1713`), underfunded (one micro short), at-cap, non-vendor.
- `pirate_buys_escort_while_lying_low_at_hideout_vendor` (`:2203-2226`) incl. the **vendor-at-hideout settle path**: hideout body doubles as the vendor body (`cfg.trophic.hideout_body_index = 0` on a `sells_upgrades: true` station); a vendor-less hideout is a deterministic 1d no-op (`:1004-1006` doc).
- `credit_identity_holds_across_purchases` (`:2229-2273`): identity every tick + non-vacuity count of events.
- `scripted_assign_filters_oversized_contracts` (`:1826`) — the ASSIGN filter test to clone for the fuel filter; `capacity_world_fixture` (`:1749`).

## GOTCHAS

1. **Consume-then-gate, not gate-then-consume**: `pending_upgrade[crow] = None` happens BEFORE any gate (`economy.rs:872`); every skip is a plain `continue` after the clear. If you gate first, a skipped intent survives to the hash point and trips the all-None assert (`hash.rs:303-308`).
2. **The t−1 frame**: every pre-physics dock predicate samples `eph.body_pos(_, Tick(tick.0.saturating_sub(1)))` because stages run with `next` as the tick arg while `ships.pos` is still tick-`cur` (`world.rs:710-715`). Using `tick` directly desyncs policy-intent vs settle docking.
3. **Corp index is a CONFIG index relying on dense slot==row, guarded by `ids.id_at(row).is_none() → skip`** (`economy.rs:903-906`) — never a one-legged debit, never an unwrap. All money moves are `saturating_sub`/`saturating_add` pairs; a credit sink/source breaks the Σtreasury+Σcredits+Σescrow test.
4. **`cap[r] == 0` in PriceCfg means "this resource's price never updates"** (`economy.rs:308-310`) — it is the structural-off switch (Ore stays dead), NOT an error. But `PriceCfg::default().cap == [1, 1]` — live-ish! Scenario factories must set caps explicitly.
5. **`!scripted` skip + never-clobber-ingest-intent** are mandatory in any new scripted policy stage (`economy.rs:1033`, `:1037`); and config_hash uses exhaustive destructures (`config.rs:620-689`) — a new cfg struct must be destructured + tail-folded, then `GOLDEN_CONFIG_HASH` (`config.rs:745`) re-pinned. No `HASH_FORMAT_VERSION` bump for a transient (unhashed) intent column.
