# Ground Extract — Economy Verbs (paid-verb idioms, prices, REPOST/ASSIGN)

Beat: `run_purchase_policies` + `resolve_purchases` end-to-end; `run_refuel_policies` +
`resolve_refuels` end-to-end; `update_prices` demand-deflation curve; REPOST/ASSIGN exact
mechanics; corporation registry idiom.

All file:line references verified against HEAD `b446095` / branch `jumpgate-v1-design`.

---

## 1. Stage-ordering map (World::step)

From `world.rs:725-878`:

```
ingest_commands          (1)
run_producers            (1b)   → next tick, counter-leg discipline
run_scripted_dispatch    (1b2)  → REPOST + ASSIGN, identity-neutral
resolve_contracts        (1c)   → accept/escrow/load/dispatch
run_pirate_brains        (1c2)
run_purchase_policies    (1c3)  → write pending_upgrade intent only
run_refuel_policies      (1c3b) → write pending_refuel intent only
resolve_purchases        (1d)   → consume pending_upgrade; always-consume-then-gate
resolve_refuels          (1d2)  → consume pending_refuel; always-consume-then-gate
physics                  (2)
...
resolve_deliveries       (3b)
...
resolve_failures         (3c)
update_prices            (3d)   → tick-gated clock
```

Trade-buy/trade-sell verbs (rung A) slot into the **1c3x / 1dx** blocks — after
`run_refuel_policies` (1c3b) and before/alongside `resolve_refuels` (1d2), following
the same write-intent-then-consume pattern.  The REPOST arbitrage poster also slots at
**1b2** (the `run_scripted_dispatch` position — spec §1.2 "stage 1b2 slot").

---

## 2. Intent / pending-column idiom (THE pattern all new verbs clone)

Every paid verb follows a two-stage split: ingest or scripted policy writes a
**transient intent column** (`pending_*`); the settle stage consumes it the **same
tick**. The column must be `None` at every hash point.

### 2a. BuyUpgrade — the canonical template

**Ingest write** (`ingest.rs:193-203`):
```rust
CommandKind::BuyUpgrade { kind } => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_upgrade[i] = Some(kind);
    }
}
```

**Scripted policy write** (`economy.rs:1235`):
```rust
ships.pending_upgrade[crow] = Some(kind);
```

**Settle — always consume first, then gate** (`economy.rs:873-879`):
```rust
let Some(kind) = ships.pending_upgrade[crow] else { continue; };
// ALWAYS consume the intent this stage, settle or skip
ships.pending_upgrade[crow] = None;
if !docked_at_vendor(ships, crow, stations, stations_cfg, bodies, eph, prev) {
    continue;
}
```

The `None`-clear happens unconditionally at `resolve_purchases`, so the column is
always `None` at every state-hash point (debug-asserted at `hash.rs:309-312`).

### 2b. Refuel — identical shape

**Ingest write** (`ingest.rs:205-212`):
```rust
CommandKind::Refuel => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_refuel[i] = Some(());
    }
}
```

**Settle — always consume first** (`economy.rs:1018`):
```rust
ships.pending_refuel[crow] = None;
```

`pending_refuel` is `Vec<Option<()>>` — the payload is a unit type because the
settle resolves quantity from world state (tank level, stock, wallet, price).

**Hash invariant** (`hash.rs:313-317`):
```rust
debug_assert!(
    world.ships.pending_refuel.iter().all(Option::is_none),
    "pending_refuel must be fully consumed (all None) at every state-hash point"
);
```

---

## 3. run_purchase_policies (economy.rs:1149-1240) — scripted intent writer

Signature:
```rust
pub fn run_purchase_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    stations_cfg: &[crate::config::StationInit],
    bodies: &BodyStore,
    eph: &Ephemeris,
    trophic: &crate::config::TrophicCfg,
    shipyard: &crate::config::ShipyardCfg,
    tick: Tick,
)
```

Key structural gates (economy.rs:1162-1176):
- Early return if both arms are disabled (`hauler_arm` = `BuyPolicy != Off`, `pirate_arm` = `engage_radius_au > 0.0`)
- `prev = Tick(tick.0.saturating_sub(1))` — t-1 frame dock predicate
- Skips `!scripted` craft (`craft_cfg.get(crow).is_some_and(|c| !c.scripted)`)
- Never clobbers an already-written ingest intent (`ships.pending_upgrade[crow].is_some()`)

**Hauler arm** (economy.rs:1205-1236): Idle craft with no contract, docked at vendor,
working-capital headroom gate:
```rust
let need = price.saturating_mul(shipyard.buy_headroom_milli as i64) / 1000;
if ships.credits_micros[crow] < need { continue; }
ships.pending_upgrade[crow] = Some(kind);
```

---

## 4. resolve_purchases (economy.rs:860-929) — settle stage 1d

Signature:
```rust
pub fn resolve_purchases(
    ships: &mut CraftStore,
    stations: &StationStore,
    stations_cfg: &[crate::config::StationInit],
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    shipyard: &crate::config::ShipyardCfg,
    tick: Tick,
    events: &mut EventStream,
)
```

**prev = t-1 frame** (`economy.rs:872`): `let prev = Tick(tick.0.saturating_sub(1));`

Full settle flow (economy.rs:873-929):
1. Clear intent unconditionally: `ships.pending_upgrade[crow] = None;`
2. `docked_at_vendor` check (uses prev tick frame — same as `try_load` precedent)
3. Per-arm: `(level, cap, ladder) = match kind { Hull | Escort }`
4. Structural cap check: `if level >= cap { continue; }`
5. Ladder bounds: `let Some(&price) = ladder.get(level as usize) else { continue; }`
6. Wallet check: `if ships.credits_micros[crow] < price { continue; }`
7. Corp row validity: `let yard_row = shipyard.corp_index as usize; if corporations.ids.id_at(yard_row).is_none() { continue; }`
8. Pure transfer (no identity legs): buyer debited, Yard credited, level bumped, event emitted

```rust
ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(price);
corporations.treasury_micros[yard_row] =
    corporations.treasury_micros[yard_row].saturating_add(price);
let new_level = level.saturating_add(1);
```

**Corp registry idiom** (`economy.rs:910-913`): `shipyard.corp_index` is a dense
row index — `slot == row` invariant means `id_at(yard_row)` validates the corp
without a SlotMap lookup. A stale/out-of-range index is a deterministic settle
skip, never a one-legged debit.

**ShipyardCfg fields** (config.rs:320-349):
```rust
pub struct ShipyardCfg {
    pub corp_index: u32,
    pub hull_price_micros: [i64; 2],    // default [8_000_000, 20_000_000]
    pub escort_price_micros: [i64; 2],  // default [5_000_000, 12_000_000]
    pub hull_step_units: u32,           // default 5
    pub max_hulls: u8,                  // default 2
    pub max_escorts: u8,                // default 2
    pub buy_headroom_milli: u32,        // default 1500
}
```

The Exchange (rung A) is the same corp-index idiom: `ExchangeCfg { corp_index, active:
false }` — a config-index dense corp that receives every trade payment, same
stale-row-is-deterministic-skip, same `id_at(yard_row).is_none()` guard.

**docked_at_vendor** (economy.rs:938-958): iterates all station rows, checks
`stations_cfg[srow].sells_upgrades` (the `StationInit` config-minted row), then
distance vs `ARRIVAL_RADIUS` at `body_pos(prev)`. The rung-A equivalent will use
`docked_station_row` (economy.rs:965-983) instead — any station's dock, not
`sells_upgrades`-gated.

---

## 5. run_refuel_policies (economy.rs:1248-1285) — stage 1c3b

Signature:
```rust
pub fn run_refuel_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
)
```

**Inert gate** (economy.rs:1257): `if refuel.lot_mass <= 0.0 { return; }` — the
named trophic-inertness gate; `lot_mass == 0.0` disables both refuel stages.

Key gates (economy.rs:1262-1283):
- Skips `!scripted` craft
- Never clobbers existing intent
- Skips pirates (`ships.role[crow] == CraftRole::Pirate`)
- `docked_station_row` finds first docked station (any station, not vendor-flagged)
- Need check: `((eff.fuel_capacity - ships.fuel_mass[crow]) / refuel.lot_mass).floor() >= 1.0`
- Wallet floor: `ships.credits_micros[crow] < stations.price_micros[srow][fuel_r]`
- Writes: `ships.pending_refuel[crow] = Some(());`

**Panel consensus #5 key distinction**: the trade goods leg is a TRANSFER (stock↔hold,
no counter touched — the `try_load` shape at economy.rs:826-827), NOT the refuel
`consumed[]` leg. Refuel adds fuel to the pirate/trophic sink (`consumed[fuel_r] += units`
at economy.rs:1052) because propellant is consumed from the economy. Trade goods
are NOT consumed — they move between stock and hold.

---

## 6. resolve_refuels (economy.rs:993-1089) — stage 1d2

Signature:
```rust
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
)
```

**Inert gate with full-column clear** (economy.rs:1004-1009):
```rust
if refuel.lot_mass <= 0.0 {
    for intent in ships.pending_refuel.iter_mut() { *intent = None; }
    return;
}
```
The lot-0 path still clears every pending intent (keeps the hash invariant).

**price < 1 guard** (economy.rs:1023-1025):
```rust
let unit_price = stations.price_micros[srow][fuel_r];
if unit_price < 1 { continue; }
```
Panel consensus L2-C3: this guard must be cloned into EVERY buy/sell/fence settle
to prevent div-by-zero and silent zero-price drains. The spec explicitly flags
economy.rs:1023-1025 as the precedent to clone.

**Integer quantization** (economy.rs:1039-1047):
```rust
let need = ((cap_eff - fuel) / lot).floor() as i64;
if need < 1 { continue; }
let afford = ships.credits_micros[crow].max(0) / unit_price;
if afford < 1 { continue; }
let units = need.min(stock).min(afford);
let cost = units.saturating_mul(unit_price);
```
All three legs computed as integers before ANY write. `min(need, stock, afford)` —
the three-way minimum produces the settled lot count.

**Four write legs** (economy.rs:1051-1056):
```rust
stations.stock[srow][fuel_r] -= units;                          // (1) station stock
counters.consumed[fuel_r] = counters.consumed[fuel_r].saturating_add(units); // (2) SINK counter
ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(cost); // (3) wallet
corporations.treasury_micros[port_row] =
    corporations.treasury_micros[port_row].saturating_add(cost); // (4) Port treasury
ships.fuel_mass[crow] = (fuel + units as f64 * lot).min(cap_eff); // (5) tank
```

Trade goods (rung A) use only two legs: stock decrements, hold increments. No
`consumed[]` increment (not a trophic sink), no treasury credit (the Exchange does
the money side). The four-leg vs two-leg distinction is load-bearing for the
resource identity.

**Post-settle nav re-derivation** (economy.rs:1062-1068): if the craft is already
`NavState::Seeking`, re-derive `dv_remaining` from the freshly-written tank. Same
Tsiolkovsky call as dispatch/ingest. Rung-A sell settle will likely need an analogous
re-nav (if delivering sold goods, craft may already be seeking a destination).

**Port corp idiom** (economy.rs:1031-1034):
```rust
let port_row = refuel.corp_index as usize;
if corporations.ids.id_at(port_row).is_none() { continue; }
```
Identical to the Yard idiom: dense row index, stale = deterministic skip.

**RefuelCfg fields** (config.rs:413-430):
```rust
pub struct RefuelCfg {
    pub lot_mass: f64,     // 0.0 = OFF (inert gate)
    pub corp_index: u32,   // Port corp, same idiom as ShipyardCfg.corp_index
}
```

---

## 7. update_prices (economy.rs:301-333) — demand-deflation curve, stage 3d

Signature:
```rust
pub fn update_prices(
    stations: &mut StationStore,
    price_cfg: &crate::config::PriceCfg,
    tick: Tick,
    events: &mut EventStream,
)
```

**cap == 0 skip** (economy.rs:309-311):
```rust
if price_cfg.cap[r] == 0 { continue; }
```
Resources with `cap[r] == 0` are silently skipped — their price stays unchanged. This
is a structural disable for any resource not yet live-priced. A new good with
`cap == 0` is invisible to the pricer.

**Integer-only formula** (economy.rs:312-315):
```rust
let s = stations.stock[row][r].max(0).min(price_cfg.cap[r]);
let p = (price_cfg.base_micros[r] * (2000 - s * price_cfg.slope_milli / price_cfg.cap[r])
    / 1000).max(0);
```
`s == 0` → `base * 2`; `s == cap` → `base * (2 - slope_milli/1000)`. No float.

**Change-only emit** (economy.rs:316): `if p != stations.price_micros[row][r]` — no
PriceUpdate event if the price is already correct.

**PriceCfg** (config.rs:143-160):
```rust
pub struct PriceCfg {
    pub base_micros: [i64; N_RESOURCES],  // per-resource base prices
    pub cap: [i64; N_RESOURCES],           // 0 = resource not live-priced
    pub slope_milli: i64,                  // e.g. 1800 (1.8)
    pub reprice_interval: u32,             // 1 = every tick
}
```
After rung A's `Good(u16)` refactor, these arrays become length-validated Vecs —
but the formula and the cap==0 skip remain identical.

The tick-gate in `World::step` (world.rs:1153): `update_prices` runs at stage 3d,
guarded by `reprice_interval`.

---

## 8. REPOST exact mechanics (economy.rs:440-518)

### Route key and latest-row representative (economy.rs:444-455)

```rust
let n = contracts.ids.len();  // bound BEFORE the loop: fresh-pushed rows not reprocessed
const MAX_POSTS_PER_ROUTE: usize = 64;
for i in 0..n {
    let later_dup = (i + 1..n).any(|j| {
        contracts.corp[j] == contracts.corp[i]
            && contracts.from_station[j] == contracts.from_station[i]
            && contracts.to_station[j] == contracts.to_station[i]
            && contracts.resource[j] == contracts.resource[i]
    });
    if later_dup { continue; }
```
Route key = `(corp, from_station, to_station, resource)` — four fields. Only the
**latest** row (highest dense index sharing the key) is the repost representative.

### In-flight sum (economy.rs:469-487)

Scans ALL rows of the route for non-terminal contracts:
```rust
if !terminal { in_flight += contracts.qty[j] as i64; in_flight_count += 1; }
```
`terminal = Completed | Failed`.

### Schmitt trigger (economy.rs:491-516)

```rust
let bursting = in_flight_count > 0 || stock < dispatch.demand_low;
if !bursting { continue; }
let mut projected = stock + in_flight;
while projected < dispatch.demand_high.max(dispatch.demand_low)
    && posts < MAX_POSTS_PER_ROUTE
{
    let new_id = contracts.push(...);
    projected += qty as i64;
    posts += 1;
}
```
- IDLE route: starts burst only when `stock < demand_low` (low edge)
- BURST route (in-flight > 0): keeps posting while `projected < demand_high.max(demand_low)`
- `demand_low == demand_high`: collapses to a single post (undamped one-shot)
- `demand_low == demand_high == 0`: `bursting = (in_flight_count > 0 || stock < 0)` — stock is never
  negative (it floors at 0 in the pricer), so an idle route never fires; this is the
  **structural off** for REPOST (verified by the panel: spec §1.2 "REPOST retirement")

### REPOST retirement (panel spec §1.2)
`demand_low = demand_high = 0` is the structural disable. Verified:
- `stock < demand_low` = `stock < 0` — never true (stock stays non-negative after producers)
- `projected < max(0, 0)` = `projected < 0` — never true for non-negative projected

The panel also specifies a **hash-neutral early-return prelude commit** (behavior-identical,
proven by within-build digest) so the dead O(rows²) scan stops running. Slot for the
arbitrage poster: same `1b2` position in `run_scripted_dispatch`, or a new parallel
function called at the same stage.

---

## 9. ASSIGN exact mechanics (economy.rs:520-650)

### Stagger gate (economy.rs:533-546)

```rust
let stagger = dispatch.stagger_period.max(1) as u64;
for crow in 0..ships.ids.len() {
    if ships.role[crow] != CraftRole::Idle { continue; }
    if craft_cfg.get(crow).is_some_and(|c| !c.scripted) { continue; }
    if tick.0 % stagger != crow as u64 % stagger { continue; }
    if ships.fuel_mass[crow] <= crate::events::FUEL_EMPTY_EPS { continue; } // PLAY-C1
```

`stagger_period == 0` → entire ASSIGN arm returns early at economy.rs:523-525.

### Two-mode policy decision slot (economy.rs:639-641)

```rust
if let Some((kidx, _)) = pick {
    ships.contract[crow] = Some(contract_id(contracts, kidx));
    ships.role[crow] = CraftRole::Hauler;
```
This is **the write site** for the two-mode policy decision in rung A. The rung-A
version replaces the single-mode `pick` with a scored comparison between
`best_wage_net` (package) and `best_trade_net` (own-trade), then either:
- Sets `ships.contract[crow]` + `CraftRole::Hauler` (package path — unchanged)
- Writes `pending_trade_buy` intent (own-trade path — new column)

The `scored` / `diag` block at economy.rs:641-648 is ASSIGN-internal diagnostics only
and is unaffected.

**Capacity filter** (economy.rs:569-572):
```rust
if contracts.qty[kidx] > capacity { continue; }
```
After rung A, this becomes the milli-mass gate: `used_milli + q * unit_mass_milli >
capacity * 1000` (capacity stays derived from `cargo_capacity()`).

**ASSIGN empty-hold gate (panel L3-M3)**: rung A requires that ASSIGN only assigns
package contracts to craft with an empty hold. This keeps the prey taxonomy exact
(own-traders are dark to pirates) and the no-double-rob invariant true. Add:
`if ships.hold[crow].is_empty() { ... }` at the candidate-consideration point.

---

## 10. Corporation registry pattern

**CorporationStore** (economy.rs:126-151):
```rust
pub struct CorporationStore {
    pub ids: SlotMap<()>,
    pub treasury_micros: Vec<i64>,
    pub home_station: Vec<StationId>,
}
```

**Dense row access idiom** (`economy.rs:910-913`, `economy.rs:1031-1034`):
```rust
let yard_row = shipyard.corp_index as usize;
if corporations.ids.id_at(yard_row).is_none() { continue; }
// then: corporations.treasury_micros[yard_row] += price;
```
`corp_index` is a `u32` in config (ShipyardCfg, RefuelCfg) → cast to `usize` at
the use site. The `id_at` call validates the row (stale/despawned corp = deterministic
skip). This is the **Exchange corp** precedent: `ExchangeCfg { corp_index, ... }` is
the same shape.

**Pure transfer discipline**: every credit move is a simultaneous debit + credit
(`Σtreasury + Σcredits + Σescrow` invariant). No leg fires alone. A stale corp row
skips the entire transfer, not just the credit leg.

---

## 11. try_load — TRANSFER shape (economy.rs:826-827)

```rust
// TRANSFER station stock -> craft cargo (in-transit). No counter touched.
stations.stock[from_row][resource.index()] -= qty as i64;
ships.cargo[crow] = Some((resource, qty));
```
This is the precedent for trade-buy/sell: stock↔hold moves with NO `mined`/`consumed`
counter increment. The identity tracks in-transit cargo on the way OUT (stock
decrements at load); the resource identity already accounts for it. Contrast with
refuel's `consumed[fuel_r] += units` — propellant is a trophic sink, goods are not.

---

## 12. Existing test names and assertion styles

**resolve_purchases tests** (`economy.rs:1852-1952`):
- `purchase_settles_at_vendor` — step-by-step exact-price literals (`5_000_000`, `12_000_000`, `8_000_000`, `20_000_000`); asserts `pending_upgrade == None` after step
- `purchase_skips_deterministically` — four skip arms: `not-docked`, `underfunded`, `at-cap`, `non-vendor`; uses `assert_purchase_skipped` helper (checks zero credit movement, no level change, intent consumed, no event)

**resolve_refuels tests** (`economy.rs:1973-2143`):
- `refuel_settles_quantized_with_four_legs_and_exact_event` — `need=4, afford=2` → `units=2`; checks all four legs + event payload + resource identity; calls `state_hash` at end (hash-point invariant)
- `refuel_tank_permille_is_floor_rounded` — `tank_before_permille: 555, tank_after_permille: 805`
- `refuel_skips_deterministically` — six skip arms: `undocked`, `stock-0`, `wallet-short`, `tank-full`, `price-0`, `stale-corp`; uses `assert_refuel_skipped` helper
- `credit_identity_holds_across_refuels_and_policy_is_self_running` — 50-tick loop; checks `Σtreasury+Σcredits+Σescrow == t0` every tick; checks policy fires automatically (4 lots at 5_000)

**REPOST/ASSIGN tests** (`world.rs:2591+, world.rs:3070+`):
- `scripted_dispatch_makes_stage1_loop_self_run` — full Stage-1 self-running fixture
- `stage2_hysteresis_and_stagger_each_have_a_measured_stabilising_effect` — measures undamped vs deadband vs damped oscillation amplitude

---

## 13. GOTCHAS

1. **The `n = contracts.ids.len()` bind is load-bearing** (`economy.rs:440`). Bind
   `n` BEFORE the REPOST loop so freshly-pushed rows are not reprocessed in the same
   tick. Missing this causes the poster to re-evaluate its own just-posted contracts.

2. **Frame discipline: t-1 vs t.** All dock predicates (`docked_at_vendor`,
   `docked_station_row`) use `prev = Tick(tick.0.saturating_sub(1))` and call
   `eph.body_pos(..., prev)`. The reason is that resolve_* stages run PRE-physics;
   `ships.pos` is still the tick-(t-1) state. Sampling `body_pos(tick)` (the current
   ephemeris tick) mixes two time frames and produces the orbit-track bug found in the
   trader rung (`world.rs:2299-2302`). Every new dock predicate must follow the same
   convention.

3. **Trade goods leg is a TRANSFER, not a consumed[] sink.** The refuel settle
   increments `counters.consumed[fuel_r]` because propellant is a trophic sink.
   Trade goods (stock→hold, hold→stock) touch NO counter — the resource identity
   carries in-flight units implicitly. Adding `consumed[]` to a goods transfer would
   corrupt the resource identity.

4. **The `price < 1` guard is mandatory on every buy/sell settle** (panel L2-C3;
   `economy.rs:1023-1025`). A zero or negative price produces integer division
   instability. Clone the guard into every new settle arm before any integer
   division: `if unit_price < 1 { continue; }`.

5. **Pending columns must be all-None at hash points** — `state_hash` debug_asserts
   this for both `pending_upgrade` and `pending_refuel` (`hash.rs:309-317`). New
   `pending_trade_buy`/`pending_trade_sell` intent columns must follow the same
   discipline: always-consume-then-gate (clear the field first, then check conditions),
   and the hash module's debug_assert must be extended to cover the new columns.

6. **REPOST structural off requires demand_low = demand_high = 0 AND the early-return
   prelude.** The default `DispatchCfg` already has both at 0 (`config.rs:188-190`),
   but the O(rows²) route-key scan still executes over the growing board. The panel
   requires a hash-neutral early-return prelude commit to prevent this (behavior-proven
   by within-build digest before and after).

7. **Quantity == 0 guard in REPOST** (`economy.rs:461-464`). A degenerate `qty == 0`
   on a contract would never raise `projected` and spin the order-up-to loop forever
   (capped by `MAX_POSTS_PER_ROUTE = 64`). Any new posting logic that introduces a
   derived quantity must carry this guard.

8. **ASSIGN only assigns packages to craft with empty holds (rung A, panel L3-M3).**
   Own-traders with a live hold must not be assigned package contracts — this keeps
   the dark/public prey taxonomy exact and the no-double-rob invariant true. The
   capacity filter already exists at economy.rs:569-572; the empty-hold gate adds a
   separate condition on `ships.hold[crow]`.
