# Grounding Extract — CraftStore + hash layout + reset sizing (v6 hold-column surface)

**Branch:** jumpgate-v1-design, HEAD b446095 · All file:line verified by Read this session.

---

## 1. CraftStore columns (stores.rs:156-215)

```rust
pub struct CraftStore {
    pub ids: SlotMap<()>,          // authority; slot==row v1 invariant
    pub pos: Vec<Vec3>,            // hashed word 9
    pub vel: Vec<Vec3>,            // hashed word 10
    pub fuel_mass: Vec<f64>,       // hashed word 11; propellant NOT traded cargo
    pub spec: Vec<BaseSpec>,       // via config_hash only
    pub nav: Vec<NavState>,        // hashed word 12, discriminant-first
    pub lod: Vec<Lod>,             // hashed word 13
    pub prev_fuel: Vec<f64>,       // RESERVED deferred word 14; transitively pinned
    pub prev_inside_dest: Vec<bool>, // RESERVED deferred word 15; transitively pinned
    pub prev_pos: Vec<Vec3>,       // NOT hashed; transitively pinned by pos at t-1
    pub mods: Vec<EffectiveMods>,  // DERIVED, NOT hashed; IDENTITY until crew-mod stage
    // --- Hauler economy columns (HASHED v2+) ---
    pub role: Vec<CraftRole>,               // word 16 via rank()
    pub cargo: Vec<Option<(Resource, u32)>>,// word 17 self-delimiting
    pub credits_micros: Vec<i64>,           // word 18 as u64
    pub contract: Vec<Option<ContractId>>,  // word 19 self-delimiting
    // --- Trophic columns (HASHED v3+) ---
    pub risk_appetite: Vec<i32>,            // word 25; 0..=1000 fixed-point
    pub pirate: Vec<Option<PirateState>>,   // word 26 self-delimiting
    // --- Pirates-rung columns (HASHED v4+) ---
    pub upgrades: Vec<UpgradeLevels>,       // words 27a/b: hulls u8, escorts u8
    pub info_tick: Vec<Tick>,               // word 28
    // --- TRANSIENT (NOT hashed; all-None debug_assert in state_hash) ---
    pub pending_upgrade: Vec<Option<UpgradeKind>>,
    pub pending_refuel: Vec<Option<()>>,
    // --- Media-rung column (HASHED v5+) ---
    pub gossip: Vec<Option<GossipBuffer>>,  // word 30; None for pirates + media-off
}
```

**v6 hold column (NOT YET PRESENT — this task):**
Spec synthesis §A2: `hold: Vec<Vec<(Good, u32)>>`, canonical ascending-Good no-zero-qty form, count-first fold after word 28 in `write_craft_economy`. Pirates get empty `Vec::new()` — NOT `None`, NOT a separate pirate column.

---

## 2. cargo column — the self-delimiting Option fold precedent

`stores.rs:177-179` — declaration:
```rust
/// Loaded cargo: `Some((resource, qty))` while carrying a delivery, else `None`.
/// Distinct from `fuel_mass` (propellant) — traded Fuel is cargo in v1.
pub cargo: Vec<Option<(crate::economy::Resource, u32)>>,
```

`stores.rs:279` — push() seeds None: `self.cargo.push(None);`

`hash.rs:328-336` — **the fold pattern to clone for hold:**
```rust
match world.ships.cargo[idx] {
    None => h.write_u64(0),
    Some((res, qty)) => {
        h.write_u64(1);
        h.write_u64(res.index() as u64);
        h.write_u64(qty as u64);
    }
}
```
For `hold`, the outer structure is count-first (`h.write_u64(hold.len() as u64)`) then per-entry `(good.0 as u64, qty as u64)` — no tag byte needed per entry because the count is the self-delimiting header.

---

## 3. credits_micros

`stores.rs:181` — `pub credits_micros: Vec<i64>`.  
`world.rs:311` — reset: `ships.credits_micros.push(0);` — day-0 wallets are 0.  
`hash.rs:337` — fold: `h.write_u64(world.ships.credits_micros[idx] as u64); // word 18` (i64 cast as u64, two's complement).

---

## 4. CraftRole + rank discipline

`stores.rs:71-90`:
```rust
pub enum CraftRole { Idle, Hauler, Pirate }
impl CraftRole {
    pub fn rank(self) -> u8 {
        match self { CraftRole::Idle => 0, CraftRole::Hauler => 1, CraftRole::Pirate => 2 }
    }
}
```
APPEND-ONLY. Used at state hash word 16 (`hash.rs:327`) and config hash word 21 (`config.rs:644`).

---

## 5. Hash fold — exact word order, v6 insertion point

`hash.rs:326-368` — `write_craft_economy` (shared by `state_hash` + `recompute_with_cursors`):

```
16. role.rank() as u64
17. cargo: 0(None) | 1 + resource.index() + qty
18. credits_micros as u64
19. contract: 0(None) | 1 + slot + generation
25. risk_appetite as u64
26. pirate: 0(None) | 1 + food_micros + notoriety + lie_low_until.0 + engage_cooldown_until.0
    (engage_cooldown_until appended INSIDE the tag-1 payload at v4 — tag-0 arm unchanged)
27a. upgrades.hulls as u64
27b. upgrades.escorts as u64
28. info_tick.0
>>> v6 HOLD INSERTS HERE (after hash.rs:367) <<<
[v6]: h.write_u64(hold.len()); for (g,q) in hold { h.write_u64(g.0 as u64); h.write_u64(*q as u64); }
```

World-level words (after the craft loop, `hash.rs:293-304`):
```
20-24. EconCounters + 4 economy stores (write_economy_stores)
29.    route_evidence (write_route_evidence)
30.    craft gossip (write_craft_gossip)
31.    station gossip reservoirs (write_station_gossip)
32.    next_alert_seq
```

**Current goldens to re-pin at v6:**
- `hash.rs:132`: `GOLDEN_ZERO_STATE_HASH = 0x0f20_843f_ccfd_8c70`
- `hash.rs:1118` (full zero-world golden): `0x274b_6874_3b8d_2700u64`
- `config.rs:783`: `GOLDEN_CONFIG_HASH = 0x128c_1299_5c48_4fdc` (re-pins only at A3's config commit)
- `HASH_FORMAT_VERSION` bumps 5 → 6 at `hash.rs:126`

**`manual_zero_fold` (`hash.rs:1172`)** is a hand-written canonical sequence. For v6, add after word 28: `h.write_u64(0); // hold count (empty vec on zero-init world)`.

---

## 6. reset() — per-craft Vec sizing

`world.rs:290-339` — craft mint loop pushes EVERY column in lockstep for each `CraftInit` in `cfg.craft`. Key initial values:

| Column | Reset value | Line |
|--------|-------------|------|
| `cargo` | `None` | 311 |
| `credits_micros` | `0` | 311 |
| `contract` | `None` | 311 |
| `risk_appetite` | `0` | 315 |
| `pirate` | `Some(PirateState{grubstake..})` iff role==Pirate, else `None` | 316-324 |
| `upgrades` | `UpgradeLevels::default()` (hulls=0,escorts=0) | 327 |
| `info_tick` | `Tick(0)` | 328 |
| `pending_upgrade` | `None` | 329 |
| `pending_refuel` | `None` | 330 |
| `gossip` | `Some(GossipBuffer::empty(...))` iff media_live && !Pirate, else `None` | 334-338 |

**v6 `hold` must add:** `ships.hold.push(Vec::new());` for ALL roles including Pirate.

**CraftStore::empty() (`stores.rs:229-254`)** and **CraftStore::push() (`stores.rs:261-292`)** must also grow the field — reset uses a manual parallel push loop, not `push()`, but both must stay consistent.

---

## 7. Pending-transient all-None asserts at hash points

`hash.rs:309-318` — both asserts fire inside `state_hash` before `h.finish()`:
```rust
debug_assert!(
    world.ships.pending_upgrade.iter().all(Option::is_none),
    "pending_upgrade must be fully consumed (all None) at every state-hash point"
);
debug_assert!(
    world.ships.pending_refuel.iter().all(Option::is_none),
    "pending_refuel must be fully consumed (all None) at every state-hash point"
);
```
The synthesis §"Two-mode policy" says `pending_trade_buy/sell` follow the same discipline. Add matching `debug_assert!` for each.

---

## 8. Ingest verb enum + dispatch — Refuel precedent for TradeBuy/TradeSell

Existing `CommandKind` arms in `ingest_commands` (`ingest.rs:151-222`):
- `Destination` → NavState::Seeking
- `AcceptContract` → ships.contract + role = Hauler (INTENT; settle deferred to resolve_contracts)
- `SetRole` → ships.role
- `Thrust` → NavState::DirectThrust
- `BuyUpgrade` → ships.pending_upgrade (INTENT; settle deferred to resolve_purchases stage 1d)
- `Refuel` → ships.pending_refuel (INTENT; settle deferred to resolve_refuels stage 1d2)

**Exact Refuel arm template (`ingest.rs:205-213`):**
```rust
CommandKind::Refuel => {
    if let Some(i) = world.ships.index_of(id) {
        world.ships.pending_refuel[i] = Some(());
    }
}
```
`TradeBuy` and `TradeSell` use the identical shape: write intent column, defer settle.

**`!scripted` + pirate skip rule:** scripted policy stages skip `!scripted` craft and pirates. The ingest arm does NOT skip (it just writes intent). The SETTLE stage (1dx) skips. See `world.rs:433-439` for the precedent.

---

## 9. step() stage ordering — substage names around 1c3b/1d2 and new trade slots

`world.rs:725-1197` — abbreviated step() stage sequence:

```
(1)      ingest_commands
(1b)     run_producers
(1b2)    run_scripted_dispatch      ← REPOST; retires in GAG; Exchange poster occupies this slot
(1c)     resolve_contracts
(1c2)    run_pirate_brains
(1c3)    run_purchase_policies
(1c3b)   run_refuel_policies        ← world.rs:833; gate: docked + !pirate + scripted + ≥1 lot headroom
[1c3x]   run_trade_policies         ← v6 new; after 1c3b, same gate discipline
(1d)     resolve_purchases          ← world.rs:851; AFTER resolve_contracts, PRE-physics
(1d2)    resolve_refuels            ← world.rs:867; AFTER purchases; prev_fuel untouched here
[1dx]    resolve_trade_buys/sells   ← v6 new; after 1d2, same "AFTER purchases, PRE-physics" rule
(physics loop per craft)
(3)      detect_boundary_events
(3b)     resolve_deliveries
(3b2)    resolve_encounters
(3b3)    update_pirate_population
(3c)     resolve_failures
(3d)     update_prices
(4)      copy-forward prev_*        ← world.rs:1174; prev_fuel/prev_pos/prev_inside_dest
(5)      tick++
```

`resolve_deliveries` (`world.rs:998-1005`) notes: "ships.pos is the tick-`cur` state" because stages 1c-1d2 are PRE-physics. The "prev_fuel is untouched by 1d2" note at `world.rs:867` matters: the trade settle stages must follow the same discipline — they may touch `fuel_mass` (they don't in v6 rung A) but must never touch `prev_fuel`.

---

## 10. Gym/scripted distinction

`config.rs:54-56`:
```rust
/// Scripted stages skip `!scripted` craft — the gym-exclusion flag.
pub scripted: bool,
```

Participates in config_hash at word 21 (`config.rs:644-645`): `h.write_u64(c.scripted as u64)`.

Stages that check `scripted`: `run_scripted_dispatch`, `run_pirate_brains`, `run_purchase_policies`, `run_refuel_policies`, reset lurk assignment (`world.rs:436`). The synthesis: "pirates and !scripted craft skipped (the refuel-policy precedent — without this rung-A pirates become own-traders and D7's split silently breaks)."

---

## GOTCHAS (top 5)

1. **Pirate rows hold zeros in the v6 hold column — `Vec::new()`, not `None`.** `hold` is `Vec<Vec<(Good,u32)>>` uniform across ALL rows. Pirates never become own-traders (policy gate), so their hold stays empty. The synthesis is explicit: no provenance fields in v6 state. The fold emits one word (`h.write_u64(0)`) for an empty pirate hold. Do not make it `Option` or add a separate pirate hold.

2. **`manual_zero_fold` at `hash.rs:1172` is a hand-written canonical sequence that must be updated in the same commit as the v6 bump.** It drives `golden_zero_state_hash`. For a zero-init world with one craft, the hold fold adds exactly one word: `h.write_u64(0); // hold count (empty vec)`. Miss this and the golden test diverges unexpectedly.

3. **The hold fold is INSIDE `write_craft_economy` (per-craft loop), NOT after it.** `write_craft_economy` is called at `hash.rs:289` inside the sorted-craft loop. `write_economy_stores` (words 20-24) and `write_route_evidence` (word 29) are world-level, called AFTER. Both `state_hash` and `recompute_with_cursors` must call `write_craft_economy` — the shared fold ensures they stay in sync.

4. **`pending_trade_buy/sell` must have matching `debug_assert!` in `state_hash` before `h.finish()`.** The existing pattern at `hash.rs:309-318` fires for `pending_upgrade` and `pending_refuel`. Without the assert, a stage-ordering bug (policy writing intent, settle not consuming) silently corrupts hash-point determinism instead of failing loudly. Add the asserts as part of the same commit that adds the columns.

5. **`run_scripted_dispatch` (stage 1b2) is being REPLACED by the Exchange poster, not removed.** The call site at `world.rs:744-768` stays; the function body changes. The signature may change (exchange poster needs `ArbitrageCfg`). Do not delete the 1b2 call site when retiring REPOST — replace the implementation. Default `scan_interval == 0` makes it inert (the structural off), preserving trophic/frontier bit-identity.
