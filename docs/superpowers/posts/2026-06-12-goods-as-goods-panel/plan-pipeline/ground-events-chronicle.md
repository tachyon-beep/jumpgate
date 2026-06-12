# Ground Extract — Events + Chronicle Beat
## Goods-as-Goods Rung A Plan (2026-06-13)

---

## 1. `EventKind` — complete variant list with payloads

**Source:** `crates/jumpgate-core/src/contract.rs:44–201`

```rust
// contract.rs:44
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    // --- Core nav/physics ---
    Arrival { craft: CraftId, dest: NavDest },          // :46
    FuelEmpty { craft: CraftId },                        // :50
    ThrustApplied { craft: CraftId, dv: f64 },          // :53
    ActionIngested { target: Target },                   // :57
    Reward { craft: CraftId, value: f64 },               // :60
    Wake { craft: CraftId },                             // :67

    // --- Economy ---
    Production { producer: ProducerId, resource: Resource, qty: u32 }, // :70
    Trade { station: StationId, resource: Resource,      // :75  ← DEAD (see §3)
            qty: u32, price_micros: i64 },
    PriceUpdate { station: StationId, resource: Resource,
                  price_micros: i64 },                   // :81
    ContractOffered { contract: ContractId },            // :86
    ContractAccepted { contract: ContractId, hauler: CraftId },   // :89
    ContractFulfilled { contract: ContractId, hauler: CraftId },  // :93

    // --- Trophic (NOT folded into state_hash) ---           // :97-98
    Robbed { pirate: CraftId, hauler: CraftId,           // :100
             contract: ContractId, value_micros: i64 },
    DrivenOff { pirate: CraftId, hauler: CraftId },      // :107
    HaulerKilled { pirate: CraftId, hauler: CraftId },   // :112
    PirateLieLow { pirate: CraftId, until: Tick },       // :117
    PirateLeft { pirate: CraftId },                      // :121
    PirateSpawned { pirate: CraftId },                   // :125
    UpgradePurchased { craft: CraftId, kind: UpgradeKind,// :133
                       level: u8, price_micros: i64 },

    // --- Media (hash-neutral like all events) ---          // :139
    AlertBorn { alert_seq: u32, route: u32, pirate: CraftId, hauler: CraftId,  // :143
                truth_value_micros: i64, claimed_value_micros: i64 },
    GossipHeard { carrier: GossipNode, alert_seq: u32, route: u32, // :160
                  pirate_slot: u32, claimed_value_micros: i64,
                  hops: u8, rob_tick: Tick },

    // --- World-gets-big (hash-neutral like all events) --- // :169
    Refueled { craft: CraftId, station: StationId, units: i64,   // :174
               price_micros: i64, tank_before_permille: u32,
               tank_after_permille: u32 },
    ContractFailed { contract: ContractId, hauler: CraftId,       // :186
                     cause: FailureCause, escrow_refunded_micros: i64,
                     cargo_lost: u32 },
    LurkMoved { pirate: CraftId, to_station: u32, breakout: bool }, // :200
}
```

**`Event` struct** (`contract.rs:203–207`):
```rust
pub struct Event {
    pub tick: Tick,
    pub kind: EventKind,
}
```

---

## 2. Single-emit discipline

One `EventStream` per `World` (`world.rs:93`). `EventStream::emit` (`events.rs:35`) is the only write path. Per-module emit sites:
- `events.rs` detect_boundary_events → `Arrival`, `FuelEmpty`
- `world.rs:968` → `ThrustApplied`
- `economy.rs:511,730,1350,1508,1076` → `ContractOffered`, `ContractAccepted`, `ContractFulfilled`, `ContractFailed`, `Refueled`
- `economy.rs:279,321,728,924` → `Production`, `PriceUpdate`, `UpgradePurchased`; `PirateLieLow` also emits from `pirate.rs:726,732`
- `pirate.rs:260,340,355` → `Robbed`, `DrivenOff`, `HaulerKilled`, `PirateLeft`, `PirateSpawned`
- `pirate.rs:618,661` → `LurkMoved` (two sites in `step_pirate_state`; same logical condition, different code branches — the only multi-site variant)
- `media.rs:230` → `GossipHeard`
- `ingest.rs:123,131,139` → `ActionIngested`

**Law:** every kind is emitted by one logical path. `AlertBorn` is emitted inside the `Robbed` settlement body (`pirate.rs`, same call).

---

## 3. The DEAD `EventKind::Trade` variant

**Location:** `contract.rs:75–80`

```rust
Trade {
    station: StationId,
    resource: Resource,
    qty: u32,
    price_micros: i64,
},
```

**Only constructor:** `contract.rs:417` — inside the test `economy_event_kinds_are_copy_and_partial_eq` (no production emit path whatsoever).

**Nothing emits it in production code.** Verified: `grep -rn "EventKind::Trade" crates/` returns only `contract.rs:75` (definition) and `contract.rs:417` (test). No arm in `chronicle_subject`, `gossip_log_event_json`, `diagnostics.rs`, or `world.rs` ever matches it.

**Panel directive:** DELETE `EventKind::Trade` and its test usages **in the same commit** that adds `TradeBought`/`TradeSold` (synthesis cut Part 1.2, paragraph starting "TradeBought/TradeSold land in the same commit that DELETES the dead EventKind::Trade").

**What the test must become:** `economy_event_kinds_are_copy_and_partial_eq` currently constructs `Trade`. When `Trade` is deleted, that test must drop the `trade` / `trade_copy` lines and replace the `assert_ne!(production, trade)` assertion with one comparing `production` to a different variant (e.g., `TradeBought`).

---

## 4. How recent variants were added — precedent code

### Refueled precedent (`economy.rs:1076–1086`)

Emit is the last thing done in the loop body, after all accounting legs (stock, credits, treasury, nav re-derive). Guarded by `if let Some(station) = stations.ids.id_at(srow).map(...)` to resolve `StationId`. `units` is the already-settled `i64` lot count.

### LurkMoved precedent (`pirate.rs:618–625`)

Two emit sites in `pirate.rs` (post-refuge: `:618`; hungry-relocation: `:661`) — both ONLY when the station row actually changed (`s != lurk`). `to_station` is a **dense row `u32`**, NOT a `StationId` — the JSONL encodes it as-is.

**Pattern for new rung-A events:** add variant to `contract.rs`, emit after all accounting legs settle, add `chronicle_subject` arm and `gossip_log_event_json` arm in the SAME commit.

---

## 5. Chronicle printer — `chronicle_subject` and `gossip_log_event_json`

### `chronicle_subject` (`trophic_run.rs:481–511`)

```rust
fn chronicle_subject(kind: &EventKind) -> Option<CraftId> {
    match *kind {
        EventKind::Arrival { craft, .. }
        | EventKind::FuelEmpty { craft }
        | EventKind::Wake { craft }
        | EventKind::Reward { craft, .. }
        | EventKind::UpgradePurchased { craft, .. } => Some(craft),
        EventKind::ContractAccepted { hauler, .. }
        | EventKind::ContractFulfilled { hauler, .. } => Some(hauler),
        EventKind::Refueled { craft, .. } => Some(craft),
        EventKind::ContractFailed { hauler, .. } => Some(hauler),
        EventKind::Robbed { pirate, .. }
        | EventKind::DrivenOff { pirate, .. }
        | EventKind::HaulerKilled { pirate, .. }
        | EventKind::PirateLieLow { pirate, .. }
        | EventKind::PirateLeft { pirate }
        | EventKind::PirateSpawned { pirate }
        | EventKind::LurkMoved { pirate, .. } => Some(pirate),
        EventKind::GossipHeard { carrier: GossipNode::Craft(c), .. } => Some(c),
        EventKind::GossipHeard { .. } | EventKind::AlertBorn { .. } => None,
        _ => None,   // ← THE WILDCARD SWALLOW (line 509)
    }
}
```

**The `_ => None` at line 509** is what the panel flagged. Currently it silently swallows `Trade` (dead, fine) and any future variant not yet in scope. The synthesis cut (Part 3, "Chronicle") mandates: **exhaustive match, no `_` arm** — that is a deliberate policy reversal. The doc comment at line 477–480 says "future variants default to skipped rather than breaking the printer" — this comment must be removed when the `_` arm is replaced.

**New variants that need `chronicle_subject` arms (synthesis cut Part 3):**
`TradeBought` → `craft` (buyer), `TradeSold` → `craft` (seller), `Jettisoned` → `craft`, `Scooped` → `pirate`, `CrateSeized` → `pirate`, `TraderRobbed` → `pirate`, `Fenced` → `pirate`, `RefuelDenied` → `craft`.

### `gossip_log_event_json` (`trophic_run.rs:384–450`)

Handles: `AlertBorn` ("born"), `GossipHeard` ("heard"), `Robbed` ("rob"), `ContractAccepted` ("accept"), `Refueled` ("refuel"), `LurkMoved` ("lurk_moved").

**Missing rows (panel-identified, synthesis cut A0):**
- `ContractFulfilled` → `"deliver"` row — **currently falls through to `_ => None`** (line 448); required by WA2/WA4 joins. Verified: no `ContractFulfilled` arm in `gossip_log_event_json`.
- `PirateLieLow` → `"lie_low"` row — also currently swallowed by `_ => None`; required by WB2.

**Accept row gains `reward` + `resource`** (synthesis cut A0) — currently only `route` + `hauler.slot`.

The `_ => None` at trophic_run.rs:448 swallows all unlisted variants. The synthesis cut mandates exhaustive match here too.

### Per-craft epilogue (`trophic_run.rs:556–611`)

```rust
// printer-side, reads world.craft_role / craft_fuel / craft_fuel_capacity / craft_credits
// + Arrival events for workplace_radius_milli_au
// + FuelEmpty event for ADRIFT since t= annotation
```

The per-station epilogue block (synthesis cut Part 3: "per-station epilogue block...threaded `&[TrophicSample]`") is **entirely absent today** — it is a rung-A addition.

---

## 6. JSONL serialization — `gossip_log_event_json` (`trophic_run.rs:384`)

Key encoding facts for new arms:
- Station carriers: `format!("s{}", s.slot)`. Craft carriers: `format!("c{}", c.slot)`.
- `LurkMoved.to_station` encodes as raw `u32` dense row (`:446`).
- `Robbed` encodes only `e`+`tick`+`route` via `diagnostics::route_of` join (`:417-420`).
- `ContractAccepted`: `route` + `hauler.slot` (`:421-425`).
- `Refueled`: `craft.slot`, `station.slot`, `units`, `price_micros`, permilles (`:426-439`).

**`PackagePosted` is dropped from the event list** (synthesis cut 1.2): the `"post"` gossip-log row is runner-enriched from the `ContractOffered` event + current prices at read time — no new event variant.

---

## 7. Events are UNHASHED

**Canonical statement:** `contract.rs:97–98`:
```
// --- Trophic events (additive; NOT folded into state_hash — the event stream
// is not hashed, and replay records (tick, state_hash) not events) ---
```
Same phrasing repeated for Media (`:139`) and World-gets-big (`:169`) groups: "hash-neutral like all events."

**Mechanically confirmed:** `hash.rs:195` — `state_hash(world: &World)` never reads `world.events`. The `EventStream` field at `world.rs:93` has no corresponding fold in `hash.rs`. Replay check in `trophic_run.rs:826-849` compares `(tick, state_hash)` pairs, not event streams.

**Consequence for rung A:** every new event variant (TradeBought, TradeSold, RefuelDenied, OfferWithdrawn, etc.) is a free addition to the enum with no GOLDEN_STATE_HASH re-pin and no HASH_FORMAT_VERSION bump. The bump in rung A (v6) is caused by the `hold` column in the state, not by any event.

---

## 8. `Robbed` event payload (current)

```rust
Robbed {
    pirate: CraftId,
    hauler: CraftId,
    contract: ContractId,
    value_micros: i64,   // = ransom (not cargo value; see pirate.rs:260-268)
}
```

`value_micros` is the **ransom** paid by the hauler wallet (pirate.rs:262-268). In rung B, `Robbed.value_micros` stays the ransom; the new `CrateSeized { pirate, prey, contract: Option, good, qty }` per-lot variant carries cargo identity. **Rung A must NOT add CrateSeized.**

---

## 9. `RefuelDenied` — rung B, NOT rung A

`RefuelDenied` does **not exist** today (verified: `grep -rn "RefuelDenied" crates/` returns nothing). The synthesis cut places it in group **A0** (scenario-blind, instruments-only) because:
- It fires at `resolve_refuels`'s silent `continue` sites (economy.rs:1028–1046): `stock <= 0` (`:1028`), `afford < 1` (`:1044`), `need < 1` (`:1040`).
- None of those sites emits any event today — all are bare `continue`.
- A `RefuelDenied` arm in `chronicle_subject` enables the WB4 "middle beat" (robbed → broke → `RefuelDenied` → stranded).

**Rung A adds `RefuelDenied` to `EventKind` and the chronicle arm.** Rung A does NOT add fencing, jetsam, or seizure events.

---

## 10. GOTCHAS

1. **Single-emit-path law is ironclad.** Every `EventKind` variant has exactly one emitting site in production code. `LurkMoved` is the only variant with two emit calls — both are in the same function (`step_pirate_state`, pirate.rs) guarding the same physical condition from two code branches. New events must follow the same discipline: find the settled accounting site, emit there, nowhere else.

2. **Chronicle arms MUST land in the SAME commit as the event variant.** The `_ => None` wildcard in both `chronicle_subject` (trophic_run.rs:509) and `gossip_log_event_json` (trophic_run.rs:448) currently swallows unhandled variants silently. The synthesis cut mandates replacing both wildcards with exhaustive matches. Until that replacement lands, forgetting a chronicle arm compiles cleanly and the event disappears from the console with no error — the bug is invisible.

3. **`EventKind::Trade` deletion requires removing the test constructor.** The variant is defined at contract.rs:75 and constructed ONLY at contract.rs:417 in a test. When the variant is deleted (in the TradeBought/TradeSold commit), the test `economy_event_kinds_are_copy_and_partial_eq` must be updated: drop the `trade`/`trade_copy` bindings and replace `assert_ne!(production, trade)` with a comparison against a new variant. The compile will catch it, but the test intent (proving Copy+PartialEq) must be preserved.

4. **`gossip_log_event_json` does NOT have a `ContractFulfilled` arm today.** The function falls through to `_ => None` for `ContractFulfilled`, `PirateLieLow`, and all trophic variants not explicitly listed. The "deliver" gossip-log row (required for WA2/WA4 joins) is entirely absent from the current codebase. This must land in the A0 instruments group.

5. **`to_station` in `LurkMoved` is a dense row `u32`, not a `StationId`.** The gossip-log encodes it as a bare integer (trophic_run.rs:446: `"to_station": to_station`). New rung-A events involving stations should check whether to use `StationId` (for event stream fidelity) or a dense row (for panel join simplicity) — `Refueled` uses `StationId` (economy.rs:1071-1074), `LurkMoved` uses the raw row. New events should use `StationId` and encode `.slot` in the JSONL arm, matching the `Refueled` precedent.
