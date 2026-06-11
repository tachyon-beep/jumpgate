# Ground extract: world step stages + stores + hash points
HEAD e7e490e, branch jumpgate-v1-design. All paths under `/home/john/jumpgate/crates/jumpgate-core/src/`.

## 1. `World::step()` stage ordering (world.rs:665-1064)

`pub fn step(&mut self, cmds: &mut Vec<Command>)` — world.rs:665. `next = Tick(cur.0+1)` (668) is the event/settle tick everywhere.

| Stage | Call | Lines |
|---|---|---|
| 1 | `crate::ingest::ingest_commands(self, cur, cmds)` | 672 |
| 1b | `economy::run_producers(...)` | 676-682 |
| 1b2 | `economy::run_scripted_dispatch(...)` (ASSIGN; `media_live` hoisted at 693) | 694-708 |
| 1c | `economy::resolve_contracts(...)` | 716-727 |
| 1c2 | `pirate::run_pirate_brains(...)` inside `if self.config.trophic.engage_radius_au > 0.0` | 736-747 |
| 1c3 | `economy::run_purchase_policies(&mut self.ships, &self.config.craft, &self.stations, &self.config.stations, &self.bodies, &self.eph, &self.config.trophic, &self.config.shipyard, next)` | 756-766 |
| **→ 1c3b** | **`run_refuel_policies` slots HERE: after 1c3 (766), before 1d (776)** | spec §5, design.md:148-151 |
| 1d | `economy::resolve_purchases(&mut self.ships, &self.stations, &self.config.stations, &self.bodies, &self.eph, &mut self.corporations, &self.config.shipyard, next, &mut self.events)` | 776-786 |
| **→ 1d2** | **`resolve_refuels` slots HERE: after 1d (786), before the physics body-snapshot at 789** | spec §5, design.md:152 |
| 2 | physics loop `for ci in 0..n_craft` (Lod dispatch, autopilot, fuel burn `self.ships.fuel_mass[ci] = (fuel - fuel_consumed).max(0.0)` at 854) | 797-870 |
| 3 | `events::detect_boundary_events` via `std::mem::take(&mut self.events)` borrow split | 877-879 |
| 3b | arrivals lifted from events, `economy::resolve_deliveries(...)` | 886-902 |
| 3b2/3b3 | gossip exchange + dock-gated `info_tick` refresh + `pirate::resolve_encounters` + `update_pirate_population`, all inside `engage_radius_au > 0.0` guard | 917-994 |
| 3c | FuelEmpty lifted, `economy::resolve_failures(...)` | 1003-1018 |
| 3d | `economy::update_prices(...)` on `reprice_interval` clock | 1027-1036 |
| 4 | copy-forward: `prev_pos`/`prev_fuel`/`prev_inside_dest` | 1041-1060 |
| 5 | `self.tick = next` | 1063 |

Pre-physics frame rule (cited in comments at 713-715, 770-773): stages 1c/1c3/1d run while `ships.pos` is still tick-`cur` state, so dock predicates sample `body_pos` at `prev = next - 1 == cur`. `resolve_purchases` computes `let prev = Tick(tick.0.saturating_sub(1))` (economy.rs:865) and calls `docked_at_vendor(..., prev)` (economy.rs:873, predicate at economy.rs:926+, "the try_load precedent"). 1c3b/1d2 must do the same.

**Stage-4 prev_fuel copy-forward (Class-3 pinning):** `self.ships.prev_fuel[ci] = self.ships.fuel_mass[ci]` — world.rs:1049. Spec §5 (design.md:155-156): refuel leaves `prev_fuel` untouched; stage-4 copy-forward preserves Class-3 pinning. hash.rs:44-45: words 14-15 (`prev_fuel`, `prev_inside_dest`) are "RESERVED — deferred; transitively pinned, NOT folded". FuelEmpty edge: `FUEL_EMPTY_EPS: f64 = 1e-9` (events.rs:16); predicate `fuel_now <= FUEL_EMPTY_EPS && fuel_prev > FUEL_EMPTY_EPS` (events.rs:50); detection in `detect_boundary_events(ships, bodies, ephem, tick, out)` (events.rs:104).

## 2. Precedent bodies a drafter will clone

- `resolve_purchases` (economy.rs:853-924): loop `for crow in 0..ships.ids.len()`; `let Some(kind) = ships.pending_upgrade[crow] else { continue }` then **immediately** `ships.pending_upgrade[crow] = None;` (economy.rs:872, comment 870-871: "ALWAYS consume the intent this stage, settle or skip") — every later check is a bare `continue` (deterministic no-op). Wallet→corp = `saturating_sub`/`saturating_add` pure transfer (economy.rs:909-911); corp row guard: `if corporations.ids.id_at(yard_row).is_none() { continue }` (economy.rs:904-906). Emits event at `tick` (== `next`) at economy.rs:918-921.
- `run_purchase_policies` (economy.rs:1012-1022 signature): intent writer; skips rows where intent already set (`if ships.pending_upgrade[crow].is_some()` economy.rs:1037); writes `ships.pending_upgrade[crow] = Some(kind)` (1066, 1098). Scripted gate reads `craft_cfg` (`&self.config.craft`); inert arms via `trophic.hauler_buy_policy != BuyPolicy::Off` / `engage_radius_au > 0.0` (1025-1026).

## 3. CraftStore (stores.rs:156-210)

Fields: `ids: SlotMap<()>`, `pos/vel: Vec<Vec3>`, `fuel_mass: Vec<f64>`, `spec: Vec<BaseSpec>`, `nav: Vec<NavState>`, `lod: Vec<Lod>`, `prev_fuel: Vec<f64>`, `prev_inside_dest: Vec<bool>`, `prev_pos: Vec<Vec3>`, `mods: Vec<EffectiveMods>` (unhashed derived), `role: Vec<CraftRole>`, `cargo: Vec<Option<(Resource, u32)>>`, `credits_micros: Vec<i64>`, `contract: Vec<Option<ContractId>>`, `risk_appetite: Vec<i32>`, `pirate: Vec<Option<PirateState>>`, `upgrades: Vec<UpgradeLevels>` (word 27), `info_tick: Vec<Tick>` (word 28), **`pending_upgrade: Vec<Option<UpgradeKind>>`** (stores.rs:203, TRANSIENT, NOT in HASH_FIELD_ORDER — doc 198-202), `gossip: Vec<Option<media::GossipBuffer>>` (word 30).

Three places size a column (all must be updated for `pending_refuel: Vec<Option<()>>`):
1. `CraftStore::empty()` — stores.rs:224-248 (`pending_upgrade: Vec::new()` at 245).
2. `CraftStore::push()` — stores.rs:255+ (`self.pending_upgrade.push(None)` at 280).
3. **`World::reset` builds the store by hand, NOT via `push`** — world.rs:232-254 literal struct (`pending_upgrade: Vec::new()` at 252) then per-craft loop world.rs:255-303 (`ships.pending_upgrade.push(None)` at 294). Comment world.rs:272-273: keep all columns length-parallel or the state hash's dense-row unwrap panics. Store test asserting parallel lengths: stores.rs:482 `assert_eq!(ship.pending_upgrade.len(), ship.ids.len())`.

## 4. The all-None hash-point assert + hash points

hash.rs:306-309 (inside `state_hash`, after all folds):
```rust
debug_assert!(
    world.ships.pending_upgrade.iter().all(Option::is_none),
    "pending_upgrade must be fully consumed (all None) at every state-hash point"
);
```
`state_hash(world)` is THE hash point; callers = `replay::record_run` (replay.rs:65, after every `world.step`) and `replay_run` (replay.rs:116), plus tests. A `Some` surviving step ⇒ stage-ordering bug, fail loud in debug (hash.rs:303-305). `pending_refuel` joins this same assert (spec design.md:146-148: "joins the all-None-at-every-hash-point assert — the pending_upgrade precedent, NOT hashed").

## 5. State-hash word-layout tail (hash.rs:12-113)

`HASH_FORMAT_VERSION: u32 = 5` (hash.rs:123). Tail: word 29 route_evidence (`write_route_evidence`, hash.rs:471-485) → word 30 craft gossip (`write_craft_gossip`, 513-524) → word 31 station gossip (`write_station_gossip`, 528-533) → **word 32 `next_alert_seq` is the LAST fold** (hash.rs:301, doc :87). Doc block hash.rs:89-92 records the not-folded-transient precedent for `pending_upgrade`. **Refuel transient is NOT hashed; no new word, no HASH_FORMAT_VERSION bump, no state-golden re-pin** — credits/fuel_mass/stock/treasury/consumed it mutates are already words 11/18/20/21/23. Goldens: `GOLDEN_ZERO_STATE_HASH = 0x0f20_843f_ccfd_8c70` (hash.rs:129), `state_hash_golden_zero_world` pins `0x274b_6874_3b8d_2700` (hash.rs:1108); both re-derivable via ignored test `print_golden` (hash.rs:1112-1117). Parity recompute `recompute_with_cursors` (hash.rs:675-764) must mirror any state_hash change. Fold-completeness test style: `state_v4_columns_are_folded` / `state_v5_columns_are_folded` (hash.rs:943, 1017) — `moves!` macro, single-field mutation must move the hash.

**Distinct CONFIG hash:** `GOLDEN_CONFIG_HASH = 0xee02_df67_1889_78dc` (config.rs:745, asserted config.rs:804, printed by config.rs:1029). Adding `RefuelCfg` to `RunConfig` + its fold = exactly ONE config-golden re-pin (spec design.md:279).

## 6. Ingest verb surface (the Refuel-verb precedent)

`CommandKind` enum — types.rs:53-77: `Destination`, `AcceptContract`, `SetRole`, `Thrust`, **`BuyUpgrade { kind: UpgradeKind }`** (types.rs:76). YES, BuyUpgrade is an ingest verb. Doc types.rs:50-51: "`CommandKind` is NOT hashed (commands resolve into already-hashed state), so adding variants is hash-neutral."

Dispatch: `ingest_commands(world, tick, cmds)` — ingest.rs:151-213. Sorts by `command_sort_key` (contract.rs:33-40 — keyed on `Target` ONLY: `(scope_rank, slot, generation)`; verb-agnostic, so a new verb needs NO sort change). Logs every command first (`world.log_mut().record(tick, cmd)` ingest.rs:154), then matches `cmd.kind` only for `Target::Entity(EntityRef::Craft(id))` (157), emits `ActionIngested` unconditionally (207-210). The BuyUpgrade arm (ingest.rs:193-204) is the clone template — intent-only:
```rust
CommandKind::BuyUpgrade { kind } => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_upgrade[i] = Some(kind);
    }
}
```
NOTE: there is a second, legacy ingest path `ingest_into` (ingest.rs:95-143, CraftStore-only) whose `match` has a catch-all `_ =>` arm (138-141), so a new variant compiles there without edits (economy kinds fall through, logged + event only).

Replay/command-log surface: `Command` is `#[derive(Clone, Copy)]`; `ActionLog.record` (ingest.rs:40-43) is the single writer of `entries` + `commands_flat` in lockstep; in-memory only — **no serde/serialization to satisfy**. `record_run` logs driver commands BEFORE `step` sorts/consumes (replay.rs:56-64); `replay_run` re-feeds `rec.log.at(pre_tick).to_vec()` (replay.rs:114) and compares per-tick hashes. So a new verb = enum variant + ingest_commands arm + (no-op) ingest_into coverage; replay works automatically.

Ingest test style precedent: `buy_upgrade_writes_pending_intent_logs_and_emits_action_ingested` (ingest.rs:440-469) — asserts intent column set, NO settle side-effects at ingest (`upgrades` default, credits 0), log length at tick, `matches!` on `ActionIngested`. Settle-side tests: economy.rs:1632/1657 assert `pending_upgrade[0] == None` post-step ("intent cleared"/"intent consumed").

## 7. reset() n-generic sizing (world.rs:183-461)

- Per-craft columns: loop over `cfg.craft` (world.rs:255-303) — every column pushed once per craft.
- `RouteEvidence` (struct world.rs:25-31: `robs: Vec<[Tick; 8]>`, `cursor: Vec<u8>`): `let n_routes = stations.ids.len().saturating_mul(stations.ids.len()); robs: vec![[Tick(0); 8]; n_routes], cursor: vec![0u8; n_routes]` — world.rs:368-372. Sized ONCE at reset from station count ("no mid-run station spawn in v1", length transitively pinned by config_hash — world.rs:22-24).
- `station_gossip`: `vec![GossipBuffer::empty(cfg.media.station_gossip_slots); stations.ids.len()]` when media-live else `Vec::new()` — world.rs:377-384.
- Per-craft gossip: media-live AND non-pirate ⇒ `Some(GossipBuffer::empty(cfg.media.craft_gossip_slots))` — world.rs:298-302.
- Reset-time Piracy RNG scatter (world.rs:396-432) consumes draws ONLY when `engage_radius_au > 0.0 && !station_ids.is_empty()` — anything added to reset must not perturb draw order on inert worlds.
- `ResetError` variants (world.rs:142-155): `Unbrakable`, `BadEconomyRef { what, index }`, `BadMediaCfg { reason }` — the half-on-media validation pattern (world.rs:207-211) is the cited idiom for the spec's refuel half-on reset error.

## GOTCHAS

1. **`World::reset` does NOT use `CraftStore::push`** — it hand-builds the struct literal (world.rs:232-254) and hand-pushes every column (255-303). A new column added only to `empty()`/`push()` will desync reset's store and panic the hash's dense-row unwraps. Three sites: stores.rs:224, stores.rs:255, world.rs:232+294.
2. **Always-consume-then-gate**: 1d2 must take the intent out (`= None`) BEFORE any dock/stock/wallet check (economy.rs:870-872 precedent). A `continue` before consumption leaves a `Some` at the hash point ⇒ `debug_assert` panic in every recorded run, but ONLY in debug builds — release silently passes, so also extend the assert in hash.rs:306-309 to the new column.
3. **Pre-physics dock frame is t−1**: stages 1c3/1d (and the new 1c3b/1d2) run before physics, so the dock predicate samples `body_pos` at `Tick(tick.0.saturating_sub(1))` (economy.rs:865), NOT at `next`. Stage 3b2's dock scan, by contrast, uses post-physics `next` (world.rs:921-941). Mixing frames breaks bit-identity between policy-intent and settle.
4. **Two golden hashes, one re-pin**: adding the `Refuel` CommandKind variant is state-hash-neutral (types.rs:50-51) and needs NO `HASH_FORMAT_VERSION` bump; adding `RefuelCfg` to `RunConfig` DOES move `GOLDEN_CONFIG_HASH` (config.rs:745) — re-derive via config.rs:1029 print test, and via hash.rs `print_golden` only if state encoding actually changed (it shouldn't).
5. **`prev_fuel` is copied forward at stage 4 from post-physics `fuel_mass`** (world.rs:1049) — a 1d2 refill happens BEFORE the physics burn and before stage 4, so the FuelEmpty edge detector (events.rs:50, eps 1e-9) sees the refilled trajectory naturally; do not touch `prev_fuel` in resolve_refuels (spec design.md:155-156).
6. Inert-lever symmetry: existing stages early-return on `engage_radius_au == 0.0` (world.rs:736, 917) or `BuyPolicy::Off` (economy.rs:1025); refuel's named gate is `lot_mass == 0.0` ⇒ BOTH stages early-return (spec design.md:138-140), proving scenario_trophic bit-identity by digest test, not by hash-version juggling.
