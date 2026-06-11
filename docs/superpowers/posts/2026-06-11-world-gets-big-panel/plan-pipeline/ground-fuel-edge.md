# Grounding extract — the fuel/energy edge (HEAD e7e490e)

All paths relative to `/home/john/jumpgate/crates/`. Verified by reading this session.

## 1. FUEL_EMPTY_EPS + the FuelEmpty edge predicate

`jumpgate-core/src/events.rs:16`
```rust
pub const FUEL_EMPTY_EPS: f64 = 1e-9;
```
Re-exported at `jumpgate-core/src/lib.rs:57` (`pub use events::{EventStream, FUEL_EMPTY_EPS, detect_boundary_events};`).

Edge predicate, `events.rs:47-51`:
```rust
fn fuel_just_emptied(fuel_now: f64, fuel_prev: f64) -> bool {
    fuel_now <= FUEL_EMPTY_EPS && fuel_prev > FUEL_EMPTY_EPS
}
```
NOTE: `fuel_prev > eps` is STRICT. A craft whose tank *starts at* exactly `FUEL_EMPTY_EPS` (e.g. scenario_trophic's 1.0e-9, scenario.rs:113/126) can NEVER arm the edge — this is the spec §1/§4 "FuelEmpty arithmetically unfireable" finding.

Emission site: `detect_boundary_events` (`events.rs:104-121`) — private fn `fuel_just_emptied` is only called at `events.rs:116`:
```rust
if fuel_just_emptied(ships.fuel_mass[idx], ships.prev_fuel[idx]) {
    out.emit(Event { tick, kind: EventKind::FuelEmpty { craft: id } });
}
```
Pure read (shared `&` refs), fires once per depletion edge. Unit test pinning the edge semantics: `fuel_just_emptied_fires_only_on_depletion_edge`, `events.rs:179-190` — asserts `(FUEL_EMPTY_EPS, 0.5)`→true, `(FUEL_EMPTY_EPS*0.5, 1.0)`→true, `(0.0, FUEL_EMPTY_EPS)`→FALSE (prev==eps does not fire), `(0.0,0.0)`→false. These literals reference the const, so changing eps does NOT break this test.

## 2. Fuel burn math — ship.rs

`jumpgate-core/src/ship.rs:27-49` `thrust_accel_and_burn(eff: &Effective, fuel_mass: f64, thrust_dir: Vec3, throttle: f64, dt: f64) -> (Vec3, f64)`:
```rust
if throttle <= 0.0 || fuel_mass <= 0.0 { return (Vec3::ZERO, 0.0); }
let thrust_force = throttle * eff.max_thrust;
let total_mass = eff.dry_mass + fuel_mass;
let accel = thrust_dir.scale(thrust_force / total_mass);
let consumed = (thrust_force / eff.exhaust_velocity * dt).min(fuel_mass);   // ship.rs:47
(accel, consumed)
```
Burn rate = `throttle*max_thrust/v_e` per day, clamped to tank. Doc note ship.rs:23-26: on the final clamped tick accel still reflects FULL thrust ("fuel-clamp over-impulse", accepted v1).

Per-tick burn application: `world.rs::step` craft loop. `world.rs:817` reads `let fuel = self.ships.fuel_mass[ci];` → `world.rs:826-829` `autopilot_command(...)` → `world.rs:831-832` `thrust_accel_and_burn(&eff, fuel, thrust_dir, throttle, dt)` → write-back `world.rs:854`:
```rust
self.ships.fuel_mass[ci] = (fuel - fuel_consumed).max(0.0);
```
Then `world.rs:856-869`: if `throttle > 0.0`, `dv = thrust_accel.length() * dt` is SUBTRACTED from `nav.dv_remaining` and a `ThrustApplied { craft, dv }` event is emitted at tick `next`.

prev_fuel copy-forward = stage (4), `world.rs:1041-1060`, AFTER detection (3) / delivery (3b) / failure (3c) / reprice (3d):
```rust
self.ships.prev_fuel[ci] = self.ships.fuel_mass[ci];   // world.rs:1049
```
Also set at reset: `world.rs:265` `ships.prev_fuel.push(c.fuel_mass);` and on `CraftStore::push` (`stores.rs:268`). `prev_fuel` declared `stores.rs:164`; hashed-state position RESERVED-but-NOT-folded ("transitively pinned"), `hash.rs:44` word 14. `fuel_mass` IS hashed: `hash.rs:36` word 11 (`fuel_mass.to_bits()`), folded via `world.craft_fuel(c)` at `hash.rs:234`.

## 3. dv_remaining derivation + coast-at-zero

Tsiolkovsky helper `jumpgate-core/src/math.rs:121-127`:
```rust
pub fn tsiolkovsky_dv(exhaust_velocity: f64, dry_mass: f64, propellant_mass: f64) -> f64 {
    if dry_mass <= 0.0 || propellant_mass <= 0.0 { 0.0 }
    else { exhaust_velocity * ((dry_mass + propellant_mass) / dry_mass).ln() }
}
```
Pinned numerics: left-to-right product, ratio inside ln, NO FMA (doc, math.rs:118-120).

Coast-at-zero condition `jumpgate-core/src/autopilot.rs:61` (inside `NavState::Seeking` arm of `autopilot_command`, autopilot.rs:35-45 signature):
```rust
if d <= ARRIVAL_RADIUS || dv_remaining <= 0.0 { return (Vec3::ZERO, 0.0); }
```
This is the escrow-lock trap the spec's FUEL-C1 closes: a Seeking craft with `dv_remaining <= 0` coasts forever even with a full tank, because nothing re-derives the budget after dispatch.

dv-derivation dispatch sites (BOTH derive via tsiolkovsky over CURRENT fuel):
1. **Ingest fallback** (no explicit `burn_budget`): `ingest.rs:218-221` `dv_from_fuel(ship, idx)` → `tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ship.fuel_mass[idx])`. Live path uses `World::dv_from_fuel_for` (`world.rs:483-491`, same formula).
2. **Economy dispatch** `economy.rs::try_load` — TWO writes: deadhead leg `economy.rs:807-811` (Seek origin body, idempotent — `already_seeking_origin` check at 802-805 leaves dv alone) and post-load leg `economy.rs:824-829` (Seek destination, `ContractStatus::CargoLoaded` set at 830). Both: `NavState::Seeking { dest, dv_remaining: dv }`.

Additional Seeking-write site a drafter must know: pirate lurk scatter at reset, `world.rs:421-430`, same tsiolkovsky form; also `pirate.rs:483`.

Full-tank Δv (cruise cap) inside autopilot: `autopilot.rs:86-88` uses `eff.fuel_capacity`, not current fuel.

## 4. Failure settlement — economy.rs

Stage (3c) in `World::step`, `world.rs:996-1018`: lifts this tick's `EventKind::FuelEmpty { craft }` ids out of `events.since(next)` then calls `crate::economy::resolve_failures(&mut contracts, &mut corporations, &mut ships, &mut econ, &failed_craft)`. Runs AFTER 3b delivery so same-tick Arrival+FuelEmpty resolves as DELIVERED.

`resolve_failures` `economy.rs:1194-1231`: fails contracts in `Accepted | CargoLoaded | InTransit` (ALL THREE escrow-holding non-terminal statuses — fix jumpgate-2c0c2d92bb, economy.rs:1202-1210) whose hauler is in `failed_craft`, via `settle_contract_failure(..., FailureCause::FuelEmpty)` (economy.rs:1222-1229).

`FailureCause` enum `economy.rs:1237-1245`: variants `FuelEmpty`, `Robbed`.

`settle_contract_failure` `economy.rs:1262-1309` (`pub(crate)`), legs: escrow→corp treasury refund (saturating, skipped on stale corp row, economy.rs:1290-1295); cargo loss as sink leg `counters.consumed[r] += qty` + `cargo=None` (1297-1304); hauler release `contract=None`, `role=Idle` (1305-1306); `status = Failed` (1308). Debug-assert pins legal source statuses per cause (1270-1288).

**FuelEmpty failure is currently SILENT on the failure side**: comment at economy.rs:1219-1221 — "No dedicated failure event: the FuelEmpty event already fired this tick and carries the cause (the robbery path emits `Robbed` at its own emission site in stage 3b2)." The spec (§ events) plans a narration event for `FailureCause::FuelEmpty` ONLY.

## 5. Test fixtures straddling the eps — ALL of them (the plan redesigns these)

The eps-straddle pattern is `fuel_mass: 1.06e-9` with `base_fuel_capacity: 1e-9`: "survives step 1, drains across the 1e-9 threshold a couple of ticks in." Spec §4 says fixture families are REDESIGNED (lower starting fuel), not literal-nudged, when eps → 1e-11.

1. `world.rs:2196-2205` `two_body_starved_contract_fixture()` — sets `cfg.craft[0].fuel_mass = 1.06e-9;` (world.rs:2203). Consumed by test `starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss` (`world.rs:2208-2286`): accepts the sole contract, steps ≤6000 ticks until `ContractStatus::Failed`, asserts escrow zeroed/refunded, cargo cleared + `consumed[Fuel] += qty`, role Idle, Σtreasury+Σcredits+Σescrow invariant.
2. `economy.rs:2364-2455` `starved_two_body_contract_fixture(from_station_index, to_station_index)` — `fuel_mass: 1.06e-9` (economy.rs:2417), `base_fuel_capacity: 1e-9` (2411), `scripted: false`. Consumed by test `fuel_empty_mid_deadhead_refunds_escrow` (`economy.rs:2465+`) — TWO arms in ONE test fn: Arm 1 deadhead (`Accepted`, stations swapped so the origin is the far body); Arm 2 `CargoLoaded` window via a DIRECT `resolve_failures` stage call after draining propellant with a field write (comment economy.rs:2526-2533: through `World::step` the next tick's stage 1c promotes CargoLoaded→InTransit before stage-3 detection, so in-window failure is only reachable in the load tick).
3. `replay_equivalence.rs:25-44` (`tests/replay_equivalence.rs`) `base_config()` craft: `base_fuel_capacity: 1.0e-9` (line 36), **`fuel_mass: 5.0e-10`** (line 41) — starts BELOW eps, so FuelEmpty can never fire there either; v_e 0.02, max_thrust 1e-13 (the §6 reset-guard recalibration comment, lines 28-34).

Fixtures AT (not straddling) eps — fuel==1e-9==eps exactly, edge unfireable, burn still happens:
- `world.rs:1282-1297` `one_body_one_thrusting_craft()` (`fuel_mass: 1e-9`, world.rs:1294) — drives `thrust_command_accelerates_craft_and_burns_fuel` (world.rs:1412), `thrust_command_persists_until_replaced` (world.rs:1468), `live_ingest_no_budget_uses_fuel_derived_dv_not_infinity` (world.rs:1390), reset-guard tests (world.rs:1320/1333/1339).
- `economy.rs:1564-1611` `vendor_world_fixture()` — `fuel_mass: 1e-9` (economy.rs:1591), cap 1e-9 (1586). Docked-vendor purchase tests; the refuel verb clones this shape.
- `tests/physics_sanity.rs:230-246` `thrusting_craft()` — `fuel_mass: 1.0e-9`, cap 1e-9, v_e 1e-2; sizing math in comments lines 225-229. Drives `fueled_autopilot_transfer_reaches_destination` (282), `transfer_arrival_tick_is_deterministic` (294), `transfer_to_moving_body_rendezvous` (318). Also `coasting_craft` (physics_sanity.rs:92-103): cap/fuel 0.0.
- Scenario factories: `scenario.rs:89-95` trophic `BaseSpec` (`base_exhaust_velocity: 20.0`, `base_fuel_capacity: 1.0e-9`), haulers+pirates `fuel_mass: 1.0e-9` (scenario.rs:113, 126). py gym: `env.rs:149` `config_template` (cap/fuel 1.0e-12, v_e 1e-3 — entirely below eps); `env.rs:295` `trader_config_template` (v_e 2.0, cap/fuel 1.0e-9 at env.rs:330-331, 352, 372). py env also has its own UNRELATED reward-side `prev_fuel` snapshot (env.rs:500/769/846) — do not confuse with the core hashed-adjacent column.

## 6. Craft spec / CraftInit fuel fields

`jumpgate-core/src/config.rs:15-24` `BaseSpec { base_dry_mass, base_max_thrust, base_exhaust_velocity, base_fuel_capacity: f64, base_cargo_capacity: u32 }`.
`config.rs:44-57` `CraftInit { spec: BaseSpec, pos, vel, fuel_mass: f64, role: CraftRole, scripted: bool }`. NO credits field (tests write the live store, economy.rs:1562-1563).
Effective seam: `stores.rs::effective_params(&spec, &mods)` → `Effective { dry_mass, max_thrust, exhaust_velocity, fuel_capacity, ... }`; v1 mods are `EffectiveMods::IDENTITY` (identity, stores.rs:56). Projection reads effective fuel_capacity at world.rs:1102. Accessors: `World::craft_fuel` (world.rs:1157-1158), `craft_fuel_capacity` (1160).

## GOTCHAS

1. **Edge predicate is strict-greater on prev**: tank that starts at exactly `FUEL_EMPTY_EPS` (every trophic-band craft: scenario.rs:113/126) never fires FuelEmpty; replay_equivalence starts BELOW eps (5e-10) and never fires either. Changing eps to 1e-11 makes those 1e-9 tanks live — and silently re-times the 1.06e-9 starved fixtures (the edge now fires ~later, after MORE fuel is burned), which is why the spec says redesign, not nudge.
2. **`fuel_mass` is hashed (hash.rs word 11) but `prev_fuel` is NOT folded** (hash.rs:44, RESERVED). Any change that alters burn arithmetic or fuel write-back moves `state_hash` goldens; changing only the eps const does not touch burn arithmetic (eps appears in zero physics expressions — only in events.rs detection + future ASSIGN eligibility).
3. **`dv_remaining` is set ONCE at dispatch** (ingest.rs:220, economy.rs:809/825, world.rs:422) and only ever decremented (world.rs:858-862). Nothing re-derives it; `autopilot.rs:61` then pins a `dv_remaining <= 0` Seeking craft into a permanent coast — the FUEL-C1 reason `resolve_refuels` must re-derive it for Seeking craft.
4. **Stage order is load-bearing**: 3c failure runs after 3b delivery (same-tick Arrival+FuelEmpty == delivered, world.rs:996-1002); detection (3) sees post-physics `fuel_mass` vs PRE-tick `prev_fuel` (copied forward at stage 4 AFTER everything, world.rs:1049). A refuel stage must run pre-physics (1d2 per spec) and leave `prev_fuel` alone so stage-4 pinning is undisturbed; note CargoLoaded→InTransit promotion happens in stage 1c the NEXT tick (economy.rs:2530-2533 comment).
5. **The borrow-split idiom**: `detect_boundary_events` requires `std::mem::take(&mut self.events)` then put-back (world.rs:877-879); event-lift before mutation (world.rs:1003-1011 collects FuelEmpty craft ids first). Clone these patterns, don't fight E0502. Also: FuelEmpty failure currently emits NO dedicated event (economy.rs:1219-1221) — the planned narration event lands in `settle_contract_failure` for `FailureCause::FuelEmpty` only.
