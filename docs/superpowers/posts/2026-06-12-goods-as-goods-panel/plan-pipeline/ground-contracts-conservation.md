# Grounding Extract — Contract Lifecycle + Conservation Identities

**Branch:** `jumpgate-v1-design` (HEAD b446095)  
**Files verified this session:** `crates/jumpgate-core/src/economy.rs` (3348 lines), `crates/jumpgate-core/src/world.rs` (3409 lines), `crates/jumpgate-core/src/ingest.rs` (531 lines), `crates/jumpgate-core/src/pirate.rs` (2154 lines), `crates/jumpgate-core/src/stores.rs`, `crates/jumpgate-core/src/config.rs`

---

## 1. ContractStatus + APPEND-ONLY rank table

`economy.rs:100-124`

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractStatus {
    Offered,       // rank 0
    Accepted,      // rank 1
    CargoLoaded,   // rank 2
    InTransit,     // rank 3
    Delivered,     // rank 4  ← EXISTS but is NEVER SET (see §2 below)
    Completed,     // rank 5
    Failed,        // rank 6
}

impl ContractStatus {
    /// Stable discriminant for self-delimiting state-hash folding. APPEND-ONLY.
    pub fn rank(self) -> u8 { ... }
}
```

**`Delivered` (rank 4) is a dead waypoint.** `resolve_deliveries` doc comment says "the `Delivered` waypoint collapses into the same settlement" — `economy.rs:1299` — and transitions `InTransit -> Completed` directly (economy.rs:1346). No code path ever writes `ContractStatus::Delivered`. The rank is reserved so APPEND-ONLY fold encoding stays stable; new statuses must append ranks ≥ 7, never renumber.

---

## 2. ContractStore — fields and `push` (posting is free and unfunded)

`economy.rs:155-205`

```rust
pub struct ContractStore {
    pub ids: SlotMap<()>,
    pub status: Vec<ContractStatus>,
    pub corp: Vec<CorporationId>,
    pub resource: Vec<Resource>,
    pub qty: Vec<u32>,
    pub from_station: Vec<StationId>,
    pub to_station: Vec<StationId>,
    pub reward_micros: Vec<i64>,
    pub escrow_micros: Vec<i64>,      // always 0 at push; funded at accept
    pub hauler: Vec<Option<CraftId>>, // None until escrow succeeds
}
```

`ContractStore::push` (`economy.rs:185-205`) always pushes `escrow_micros: 0` and `hauler: None`. **Posting costs the corp zero credits** — there is NO treasury debit at push time. The escrow debit happens only inside `resolve_contracts` at the `Offered` match arm, and only if the corp can cover it (economy.rs:723-725). A corp-posted arbitrage row must carry exactly the fields above; no additional fields exist today (the `resource` + `qty` + `from_station` + `to_station` + `reward_micros` are the full payload of a `ContractInit`, config.rs:128-135).

---

## 3. Escrow at accept — the unfunded/over-capacity REVERT

`economy.rs:689-734`

The `resolve_contracts` loop iterates ALL rows and switches on `status[kidx]`. At `Offered`:

1. Find the accepting craft by `ships.contract[r] == Some(contract)` — lowest dense row first (no RNG).
2. **CAPACITY GATE:** `contracts.qty[kidx] > capacity` → REVERT.
3. **FUNDED GATE:** `corporations.treasury_micros[corp_row] >= reward` checked; if not → REVERT.
4. REVERT clears `ships.contract[crow] = None`, `ships.role[crow] = CraftRole::Idle`, `contracts.hauler[kidx] = None`; the offer stays `Offered`, zero credit motion.
5. If funded AND fits: debit `corporations.treasury_micros[corp_row] -= reward` (line 724), credit `contracts.escrow_micros[kidx] += reward` (line 725), bind hauler, transition `Offered -> Accepted`.
6. Immediately call `try_load(...)` for possible same-tick load.

The REVERT is described in the doc comment as "the underfunded-escrow precedent" — the capacity gate was added later (pirates rung §6) using the same pattern.

---

## 4. Two-phase intent/settle accept (ingest.rs:165-184)

```rust
CommandKind::AcceptContract { contract } => {
    // Record INTENT only: set craft's contract column + role Hauler
    // iff the contract is Offered and unassigned. The actual contract
    // state transition (status/escrow) is DEFERRED to resolve_contracts.
    if let (Some(i), Some(ci)) = (craft_row, contract_row)
        && world.contracts.status[ci] == ContractStatus::Offered
        && world.contracts.hauler[ci].is_none()
    {
        world.ships.contract[i] = Some(contract);
        world.ships.role[i] = CraftRole::Hauler;
    }
}
```

**Ingest writes craft-side intent only.** `resolve_contracts` reads that intent (finds `ships.contract[r] == Some(contract_id)`) and owns the corp-side escrow + status transition. This is the same split used by `BuyUpgrade` / `resolve_purchases`.

---

## 5. `try_load` — t-1 frame, co-location, stock gate, pure TRANSFER

`economy.rs:759-839`

Key invariants the drafter must not break:

- **Frame:** `resolve_contracts` runs at stage 1c, BEFORE physics. `ships.pos` is the tick-(t-1) state. `try_load` computes `body_pos` at `Tick(tick.0.saturating_sub(1))` (line 800) to avoid a two-epoch mismatch. Do NOT compare against `body_pos(tick)`.
- **Co-location check:** `ships.pos[crow].sub(body_pos).length() > ARRIVAL_RADIUS` → no load; dispatches the hauler toward the origin (the "walk to the food" deadhead leg, lines 802-819), idempotent if already seeking.
- **Stock gate:** `stations.stock[from_row][resource.index()] < qty as i64` → no load, retry next tick (line 822-823).
- **TRANSFER on success:** `stations.stock[from_row][resource.index()] -= qty as i64` (line 826), `ships.cargo[crow] = Some((resource, qty))` (line 827). **No counter is touched** — the resource identity already accounts in-transit cargo. Status → `CargoLoaded` (line 837).
- `try_load` emits **no event** (line 838 comment).

---

## 6. `resolve_deliveries` — Arrival-matched settle

`economy.rs:1304-1352`

Called after the physics stage; takes `arrivals: &[(CraftId, NavDest)]` collected from this tick's `Arrival` events.

Settle on match:
1. Unload: `stations.stock[to_row][resource.index()] += qty as i64` — TRANSFER, no counter. (line 1335)
2. Payout: `let payout = contracts.escrow_micros[kidx]` → zero the escrow → `ships.credits_micros[crow] += payout`. (lines 1338-1340) Credit identity: escrow→craft wallet.
3. Release hauler: `cargo = None`, `contract = None`, `role = Idle`.
4. `status = Completed`. Emit `ContractFulfilled`. The `Delivered` waypoint is never visited.

---

## 7. `settle_contract_failure` — three legs exactly

`economy.rs:1448-1516`

Shared body for both callers (`resolve_failures` via `FuelEmpty` and the pirate rob path via `Robbed`). **FailureCause is purely a legal-source-status discriminant**, not a conditional on the legs:

| Leg | Code | Identity role |
|-----|------|---------------|
| L1: escrow refund → corp treasury | lines 1477-1487 | credit TRANSFER; stale corp row skips refund, escrow stays (identity still holds) |
| L2: cargo → `consumed[r] += qty`; `cargo = None` | lines 1488-1497 | SINK leg; cargo was debited from stock at load (in-transit); this closes the resource identity |
| L3: release hauler: `contract=None`, `role=Idle` | lines 1499-1501 | no identity leg |
| status → `Failed` | line 1502 | |

**Which statuses each FailureCause may start from** (the `debug_assert` at lines 1458-1475):

- `FuelEmpty`: `Accepted | CargoLoaded | InTransit` (all three escrow-holding non-terminal statuses — bug jumpgate-2c0c2d92bb: filtering to InTransit alone locked deadhead-stranded escrow forever)
- `Robbed`: `CargoLoaded | InTransit` only (must be laden; Offered/Accepted have no cargo)

**`ContractFailed` event is emitted only for `FuelEmpty`** (lines 1503-1516); the `Robbed` path suppresses the event (the caller emits `Robbed` instead).

---

## 8. The credit identity assertion

`world.rs:3326-3350` (the T17/T18 test; also inline in world.rs:2604-2622 and the `assert_resource_identity` helper at world.rs:2870-2884):

```rust
let initial_credit = world.corporations.treasury_micros.iter().sum::<i64>()
    + world.ships.credits_micros.iter().sum::<i64>()
    + world.contracts.escrow_micros.iter().sum::<i64>();
// ... every tick:
assert_eq!(
    credit_now(&world),
    initial_credit,
    "Σtreasury+Σcredits+Σescrow invariant at tick {t}"
);
```

**The complete list of lawful credit legs (all verified this session):**

| Leg | Source | Credit motion |
|-----|--------|---------------|
| Escrow debit at accept | economy.rs:723-725 | treasury → escrow |
| Payout at delivery | economy.rs:1338-1340 | escrow → craft wallet |
| Escrow refund on failure | economy.rs:1480-1487 | escrow → treasury |
| Upgrade purchase | economy.rs:915-917 | craft wallet → Yard treasury |
| Refuel purchase | economy.rs:1053-1055 | craft wallet → Port treasury |
| Ransom (rob) | pirate.rs:248-252 | hauler wallet → pirate wallet |

All six are **pure transfers** between `Σtreasury + Σcredits + Σescrow`. No leg creates or destroys credits. The identity is asserted every tick in `phase2_credit_identity_holds_every_tick` (world.rs:3315) and `credit_identity_holds_across_refuels_and_policy_is_self_running` (economy.rs:2116) and `credit_identity_holds_across_purchases` (economy.rs:2786).

**`food_micros` is OUTSIDE the credit identity.** It is an in-pirate metabolic counter minted from robbed cargo (pirate.rs:254-258: `food_micros += qty * food_per_unit_micros`) and re-minted on starvation grubstake reset (pirate.rs:724-725: `food_micros = grubstake_micros`). Neither write touches any treasury/wallet/escrow column. The rob food-mint will be **REMOVED** in rung B (`settle_contract_failure` L2 already sends cargo to `consumed`, so rung B's seizure/jetsam path replaces the food-mint with a hold transfer + separate fence stage).

---

## 9. Resource conservation identity

`world.rs:2870-2884`

```rust
fn assert_resource_identity(world: &World, initial: &[i64; N_RESOURCES]) {
    for r in 0..N_RESOURCES {
        let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
        let in_transit: i64 = world
            .ships
            .cargo
            .iter()
            .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
            .sum();
        let lhs = stock + in_transit;
        let rhs = initial[r] + world.econ.mined[r] - world.econ.consumed[r];
        assert_eq!(lhs, rhs, "resource identity for r={r}: ...");
    }
}
```

**The in-transit term today iterates `ships.cargo` only** (the single-slot `Option<(Resource, u32)>` per craft). After A2's `hold: Vec<Vec<(Good, u32)>>` column lands, this assertion **must be updated to sum over the hold Vec as well** — the synthesis note says "the `assert_resource_identity`'s in-transit term gains the hold sum (today it iterates `ships.cargo` only, world.rs:2870-2884) in this commit [A2]."

Current `ships.cargo` definition (stores.rs:179):
```rust
pub cargo: Vec<Option<(crate::economy::Resource, u32)>>,
```
This is the column A2 replaces with `hold: Vec<Vec<(Good, u32)>>` in canonical form (ascending Good, no zero qty).

---

## 10. `offered_contracts` board read

`world.rs:549-578`

```rust
pub fn offered_contracts(&self) -> Vec<(ContractId, i64, StationId, StationId)> {
    (0..self.contracts.ids.len())
        .filter_map(|k| {
            if self.contracts.status[k] != ContractStatus::Offered
                || self.contracts.hauler[k].is_some()
            {
                return None;
            }
            let cid = ContractId { slot, generation };
            let intent_claimed =
                (0..self.ships.ids.len()).any(|r| self.ships.contract[r] == Some(cid));
            if intent_claimed { return None; }
            Some((cid, self.contracts.reward_micros[k],
                  self.contracts.from_station[k], self.contracts.to_station[k]))
        })
        .collect()
}
```

**The return tuple is `(ContractId, reward_micros, from_station, to_station)` — it does NOT expose `resource` or `qty`.** The arbitrage poster needs both (to compute `spread × qty > transport + premium`). The new arbitrage scanner will read `contracts.resource[k]` and `contracts.qty[k]` directly from the store, not through `offered_contracts`. The public board read will need a new accessor or an extended tuple.

The three-way "off the board" filter is:
1. status ≠ Offered
2. `hauler.is_some()` (contract-side binding)
3. any craft holds accept-INTENT (`ships.contract[r] == Some(cid)`) — ingest writes this one stage before `resolve_contracts` binds the contract side

---

## 11. L1-C1 fix: where the withdrawal sweep lives

The synthesis (Part 1.2, Withdrawal) extends the arbitrage poster to fail two classes of zombie rows:

**(a) Unclaimed Offered rows** whose price check no longer clears: `spread × qty − transport − premium ≤ 0` at current prices → withdraw. This runs in the same stage-1b2 scan.

**(b) Accepted-but-never-loaded rows whose corp cannot fund the buy** — `status == Accepted && corp.treasury < buy_price_at_load`. The fix: call `settle_contract_failure(... FailureCause::Robbed ...)` minus the cargo leg (no cargo exists; only the escrow refund + hauler release legs fire). Emit unhashed `OfferWithdrawn`. Without this, every corp insolvency attrits the fleet permanently: the hauler holds Accepted-role forever, the escrow refund never fires, and only `FuelEmpty` (economy.rs:1376-1401) eventually releases it.

**Where to add it:** the withdrawal sweep belongs in the same `run_scripted_dispatch` function (economy.rs:408), run before or after the REPOST loop at stage 1b2. The REPOST loop currently iterates `0..n` (the pre-scan row count) and skips non-latest route rows. The withdrawal sweep needs a parallel pass over rows whose status is `Offered` (for price recheck) or `Accepted` (for corp-funding recheck).

---

## 12. Corp-posted package row — what it must carry

The new arbitrage poster calls `ContractStore::push` with the same signature as today's REPOST (`corp, resource, qty, from_station, to_station, reward_micros`). No new store columns are needed for rung A. What must be derived before the push:

- `corp`: the posting corporation's `CorporationId`
- `resource`: the good being arbitraged
- `qty`: from the qty ladder (smallest-first; plan-time ladder, not runtime ephemeris)
- `from_station`: the source station (lowest price)
- `to_station`: the destination station (highest price)
- `reward_micros`: `transport[route] + surplus × wage_share_milli / 1000` (OD-4 formula)

The corp treasury is NOT debited at push. Escrow fires only at accept (resolve_contracts Offered arm, lines 711-727). The poster must track a `committed[corp]` scratch during the scan to avoid over-posting against a single treasury (synthesis Part 1.2: "funding headroom `treasury ≥ wage + price[a]·qty` tracked through a per-scan `committed[c]` scratch").

---

## 13. Key tests to extend / reference

| Test name | File:line | What it asserts |
|-----------|-----------|-----------------|
| `capacity_gate_reverts_oversized_accept` | economy.rs:2293 | REVERT path: zero escrow/treasury movement, offer stays Offered |
| `phase2_credit_identity_holds_every_tick` | world.rs:3315 | Σtreas+Σcredits+Σescrow invariant every tick across full lifecycle |
| `phase1_gate_resource_accounting_identity_holds_every_tick` | world.rs:2888 | Σstock+in_transit = initial+mined-consumed every tick |
| `credit_identity_holds_across_refuels_and_policy_is_self_running` | economy.rs:2116 | credit identity through refuel legs |
| `credit_identity_holds_across_purchases` | economy.rs:2786 | credit identity through upgrade purchase legs |
| `scripted_assign_filters_dry_tank_craft_play_c1` | economy.rs:2415 | PLAY-C1: dry-tank craft stays Idle |
| `scripted_assign_filters_oversized_contracts` | economy.rs:2334 | filter-at-choice capacity gate |

**Test assertion style** (copy this pattern for new identity tests):
```rust
assert_eq!(credit_now(&world), initial_credit, "Σ... invariant at tick {t}");
```
Non-vacuity guards are mandatory: assert that each leg actually fired (mined > 0, consumed > 0, etc.) or the identity holds trivially on a stalled world.

---

## GOTCHAS

1. **`Delivered` rank 4 is dead but allocated — never reuse it.** The status exists in the enum (economy.rs:106) and its rank is folded into the state hash. The lifecycle skips it: `resolve_deliveries` transitions `InTransit → Completed` directly. Adding a reader for `Delivered` would be wrong; any new intermediate status must take rank ≥ 7.

2. **Posting is free; escrow is deferred to accept.** `ContractStore::push` sets `escrow_micros = 0` unconditionally. A corp with treasury 0 can post unlimited rows. The unfunded check is in `resolve_contracts`'s Offered arm — it REVERTs the accept (zero credits move) rather than preventing the post. The withdrawal sweep (L1-C1) is the only planned mechanism for cleaning up zombie Offered rows that can never clear.

3. **`try_load` uses tick-(t-1) body positions.** `resolve_contracts` runs before physics (stage 1c). The co-location check calls `eph.body_pos(..., Tick(tick.0.saturating_sub(1)))` (economy.rs:800). Using `tick` instead causes the "hauler tracking its pickup body could never pass the gate" starvation bug (already fixed and documented in the `try_load` comment). Any new load-style settle (own-trade buy) must use the same t-1 frame.

4. **`food_micros` mint from robbed cargo is OUTSIDE the credit identity and will be removed in rung B.** Currently, pirate.rs:254-258 adds `qty × food_per_unit_micros` to `food_micros` on every successful rob. In rung B this mint is removed (synthesis Part 2, seizure legs: "L6 rob food mint REMOVED"). The re-grubstake mint on starvation (pirate.rs:724-725) is explicitly KEPT. Do not conflate the two.

5. **`offered_contracts()` does not expose `resource` or `qty`.** The arbitrage poster needs both fields to compute whether a spread clears. It must read `contracts.resource[k]` and `contracts.qty[k]` directly. The existing `offered_contracts()` return type `(ContractId, i64, StationId, StationId)` is the strategic-layer board read for scripted ASSIGN — extending it or writing a separate scanner accessor is required.
