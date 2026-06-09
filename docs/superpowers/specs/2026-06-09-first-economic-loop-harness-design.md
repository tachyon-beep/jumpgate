# First Economic Loop — Deterministic Harness (design spec)

Date: 2026-06-09   Owner: John   Author: acting-PM (Claude)   Issue: `jumpgate-fe825a65f3`
Status: draft for owner review
Related: PDR-0005 (DRL repositioned to scale/density), `docs/superpowers/reviews/2026-06-09-vertical-slice-shaping-findings.md`, charter land-order, RAID R2/R5.

## Purpose

Close the first demand-driven economic loop — miners → refine → fuel → haulers move goods under delivery contracts for a reward, with price that deflates as stock rises — as a **deterministic correctness/replay harness**. Per PDR-0005 this loop is **explicitly NOT where the DRL thesis is proven** (a small tractable market hosts computation, not judgment). It is the substrate the later scale/density arena (`jumpgate-aec6e7bc14`) and the cheap analytic cut run on. Success here = *the loop conserves mass and credits, reprices endogenously, and replays bit-identically on the same build* — not "DRL beats a script."

Everything is **additive** on the live seams verified in core (`stores.rs`, `events.rs`, `contract.rs`, `world.rs`, `hash.rs`, `config.rs`); the one non-additive cost is a single `HASH_FORMAT_VERSION` bump (1→2) for the new hashed economy state.

## Scope

**In (this spec):**
- One commodity chain end-to-end: **Ore → (refine) → Fuel → (consume)**. A miner produces Ore, a refiner turns Ore into Fuel, a demand-sink consumes Fuel. (One chain proves the loop; widen later by adding recipe rows, not new machinery.)
- Stations holding per-(station, resource) **stock** and **price**.
- A **corporation** that funds and posts **delivery contracts** (move Fuel from refinery-station to consumer-station).
- **Haulers**: a role + cargo on the existing `CraftStore`, that accept a contract, load, route to the destination body, dock via the **already-live co-orbiting rendezvous arrival**, deliver, and are paid from escrow.
- **Stage 1** (scripted, fixed price) then **Stage 2** (demand-deflation pricing): `price = f(stock)`, a deterministic reprice clock, hysteresis + staggered dispatch for stability.
- The `HASH_FORMAT_VERSION` 1→2 bump folding all new hashed economy state.

**Out / deferred (YAGNI — foreclose nothing, build trivially or not at all):**
- **`ARRIVAL_RADIUS` → config-field promotion.** The harness uses the existing `pub const ARRIVAL_RADIUS` (autopilot.rs; used by events.rs:7,78 + world.rs:113,354). Promotion belongs to the navigator/authority + scale-arena work (PDR-0005 foundational integration), not here — it is separable and would re-pin `0x278c` on its own schedule.
- **LOD dormancy / the Task-12 Wake hook.** The harness runs at small N with in-transit/idle haulers integrated normally (`Lod::Player`). Dormancy is an R6 throughput lever for the *dense arena*, not needed to close the loop. The seam already exists (`world.rs:260-271`, `Lod::Nothing` skips; `EventKind::Wake` pinned at contract.rs:66) — leave it.
- **The DRL arena, any learner, the cheap analytic cut** — separate issue (`jumpgate-aec6e7bc14`).
- **Combat/piracy/law, crews (Person B/C), multi-commodity graphs, multiple corporations, market-maker NPCs beyond the one demand sink** — all later additive content.
- **Money as float, partial deliveries, contract auctions, spot trading** — not in the thin loop.

## Architecture

Two additions, both additive:

1. **New SoA stores** (new module(s), e.g. `economy.rs`), each keyed by a generational `SlotMap` id with the same `slot == row` / no-mid-run-despawn invariant `CraftStore` uses (stores.rs:107-150). Length-parallel columns, minted at `World::reset`.
2. **New deterministic stages in `World::step`** (world.rs:238), inserted in a fixed order relative to the existing physics + boundary-event stages.

No existing seam signature changes. `effective_params(spec, &mods)` (stores.rs:32), `command_sort_key` (contract.rs:32), the `EventStream` (events.rs:18), and the rendezvous arrival path (events.rs:104-158) are consumed as-is.

### Primitives

Each primitive: its data shape, the seam it sits on, and its determinism obligation.

- **Producer** — a recipe column-table indexed by `ProducerId` (`rate`, `owner_corp`, `station`, parallel Vecs) plus a flat **edge table** `(producer_id, resource, qty, direction)` that *is* the commodity graph as relational data. `run_producers` is a new step stage: each all-or-nothing firing moves inputs→outputs in station stock and emits a `Production` event. Mining = a producer with no input (empty→Ore); refining = Ore→Fuel; the demand sink = a producer with an input and no resold output (Fuel→∅), whose draw is what makes stock fall and price move. *Determinism:* fire in sorted `ProducerId` order; integer or quantized quantities; firing predicate reads only hashed state.

- **Station market** — per-(station, resource) **`stock`** and **`price`** columns (a dense `station × resource` table; v1 has few of each). Buy/sell is a validate-then-commit-atomically operation; price recompute is a *separate* stage reading stock (Stage 2). Station spatial position is a Body (haulers rendezvous with it); its market is these economic columns (decoupled). *Determinism:* both columns are independent mutable per-tick state → **hashed** (see seam budget). No negative stock (validate before commit).

- **Corporation** — a non-spatial funded registry column indexed by `CorporationId`: `treasury_micros: i64` (**integer microcredits — no float money**, one named conversion boundary) + `home_station`. The contract originator. *Determinism:* integer arithmetic; treasury hashed.

- **Delivery Contract** — a table keyed by `ContractId` with a **status enum** column (`Offered → Accepted → CargoLoaded → InTransit → Delivered → Completed`, plus `Failed`) and an `escrow_micros` column. Reward is escrowed at accept (debit corp), paid from escrow at settle, so `Δhauler + Δcorp + Δescrow == 0`. Lifecycle transitions emit events. *Determinism:* status enum folded discriminant-first (self-delimiting); escrow integer; transitions resolved in sorted `ContractId` order.

- **Hauler role + cargo** — additive columns on `CraftStore` (stores.rs:79): a `role` tag, `cargo` (resource + qty), `credits_micros: i64`, and a `contract` handle. A hauler accepts a contract via a new `CommandKind` variant, routes to the destination Body, and docks via the **live rendezvous arrival** (`NavDest::Entity(EntityRef::Body)` → `arrival_swept` gated by `ARRIVAL_SPEED`, events.rs:127-155) — the moving-station docking model, no new machinery. *Determinism:* cargo/credits are mutable state → hashed; a future cargo-MASS→maneuver effect folds into `EffectiveMods` (stores.rs:54), **never** an `effective_params` signature change.

- **New event + command variants** (hash-neutral — `EventKind`/`CommandKind` are not in `HASH_FIELD_ORDER`; events are a stream, commands resolve into hashed state): add `EventKind::{Production, Trade, PriceUpdate, ContractOffered, ContractAccepted, ContractFulfilled}` (all `Copy`, ids + scalars only — no `Vec`/`String`, matching the existing enum, contract.rs:43-69) and `CommandKind::{AcceptContract, SetRole}` (additive; `command_sort_key` already total-orders by target, contract.rs:32).

### Data flow (the loop) and the step order

Within `World::step` (world.rs:238), the new stages run in this fixed order each tick (chosen so the loop is causal and replayable):

```
(1) ingest_commands            [existing] — incl. new AcceptContract/SetRole
(2) run_producers              [NEW]      — fire recipes: stock += outputs, stock -= inputs; emit Production
(3) physics / Lod-dispatch     [existing] — haulers integrate, autopilot, thrust
(4) detect_boundary_events     [existing] — Arrival/FuelEmpty (Arrival = a hauler reached its dest station-body)
(5) resolve_contracts          [NEW]      — on a hauler's Arrival at the contract dest: unload cargo→stock,
                                            mark Delivered→Completed, settle escrow (corp→hauler), emit ContractFulfilled
(6) update_prices              [NEW, Stage 2] — deterministic reprice clock: price = f(stock), with hysteresis
(7) repost/dispatch            [NEW]      — corp posts new contracts from demand; the scripted hauler policy
                                            emits AcceptContract through the SAME ingestion path (1) a future
                                            agent would use (resolved next tick) — one path, not a side channel
(8) copy-forward prev_*, tick++ [existing]
```

The **closed homeostatic cycle**: producers raise/lower stock → `update_prices` moves price off stock → corp posting/reward tracks margin → haulers move goods → delivery changes destination stock → price re-moves. Stage 1 freezes step (6) at a constant price (pure correctness baseline). Stage 2 turns it on.

### Stage 1 vs Stage 2

- **Stage 1 (scripted, fixed price):** steps (2)(5)(7) live; (6) is a constant. Scripted haulers accept by a fixed rule (e.g. nearest / lowest-`StationId`). Proves the loop conserves and replays. **Hosts no skill by design** — the archived C+ finding is that a blind constant ties any "strategic" rule here.
- **Stage 2 (demand-deflation):** turn on (6) `price = base · (2 − stock/cap · 1.8)` (linear deflation; tune the constants in config). The load-bearing work is *closing the loop without oscillation*: a **deterministic Wait/tick-gated reprice clock** invoked from the step path, a **hysteresis deadband**, and **staggered dispatch** so the scripted fleet does not herd into a limit cycle (the archive deliberately left this loop open at both ends to avoid exactly this).

## Determinism & the seam budget (RAID R2)

The whole point of the harness is replay. Rules:

1. **One `HASH_FORMAT_VERSION` bump, 1 → 2** (hash.rs:48), landing **all** new hashed economy columns at once (not one bump per stage). Append them to `HASH_FIELD_ORDER` (hash.rs:12-43) after word 13, in a fixed, documented order; fold each in **sorted-id order**, **self-delimiting** (enum discriminant before payload, as `NavState` does, hash.rs:167-194); update the executable parity spec `recompute_with_cursors` (hash.rs:334) in lockstep; **re-pin both state goldens** `GOLDEN_ZERO_STATE_HASH = 0xf0dd…` (hash.rs:54) and the zero-world golden `0x532d…` (hash.rs:425) from a real `print_golden` run. This is RAID R2's "the one change allowed to move both goldens" — single-cause, never batched with anything else.
2. **Hashed vs not:** stock, price, corp treasury, contract status + escrow, hauler cargo + credits are independent mutable per-tick state → **must be hashed**. New `EventKind`/`CommandKind` variants are **not** hashed (verified: `HASH_FIELD_ORDER` folds stores only) → adding them is hash-neutral.
3. **Integer money.** All credits are `i64` microcredits with exactly one named float↔int conversion boundary. No floating-point treasury/escrow/reward.
4. **Economy config in `RunConfig`** (recipes, stations, corp seed, the Stage-2 price constants) is folded into `config_hash` — and because `config_hash` exhaustively destructures (config.rs:150), the compiler *forces* each new field to be folded, then **re-pin `GOLDEN_CONFIG_HASH = 0x278c…`** (config.rs:212) once, deliberately.
5. **Reprice clock from the step path.** `update_prices` is invoked deterministically from `World::step`, not lazily on read (the archived open-loop bug). Wait/tick-gated so its cadence is part of the recorded schedule.

## Invariants & error handling

- **Mass conservation:** for every producer firing and every delivery, total resource units in = units out (the demand sink is the only true destruction, and it is explicit). A whole-system conservation test is the Stage-1 gate.
- **Credit conservation:** `Δhauler + Δcorp + Δescrow == 0` at every settlement (integer-exact).
- **No negative stock / no overdraft:** validate-then-commit; a producer firing or a buy that would underflow does not fire (deterministic skip, not a panic).
- **Contract lifecycle is a strict state machine:** illegal transitions are unrepresentable / rejected; a hauler that runs out of fuel mid-contract → `Failed`, escrow returns to corp.
- `World::reset` keeps returning `Result<_, ResetError>` (world.rs:106); economy misconfig (e.g. a recipe referencing an unknown resource) is a new `ResetError` arm, validated before tick 0.

## Testing strategy

- **Conservation tests** (the Stage-1 acceptance gate): mass-in==mass-out across a multi-tick run; credit `Δ==0` at each settlement.
- **Determinism / replay** (per the `digest-tests-are-determinism-not-golden` lesson): a cross-run digest test — same config + same scripted inputs → bit-identical `state_hash` sequence over N ticks; and same-seed reset determinism.
- **Golden discipline:** the `HASH_FORMAT_VERSION` bump re-pins both state goldens in one named commit; the economy-config fold re-pins `0x278c` in one named commit. Never batch a golden move with unrelated change.
- **Stage-2 stability test:** drive the closed loop and assert price/stock reach a hysteresis band rather than a growing limit cycle (the oscillation hazard is real — make it a regression test, not a hope).
- **Lifecycle tests:** every contract transition incl. `Failed`/escrow-return; no-negative-stock; producer all-or-nothing firing.

## Open questions for the owner (do not block drafting; resolve before/at planning)

1. **Commodity names/chain depth:** Ore→Fuel→consume as the one chain — fine, or a different first chain?
2. **Stage-2 deflation curve:** linear `price = base·(2 − stock/cap·1.8)` as the starting form, tuned in config — acceptable as a starting point (it is replaceable without reshaping the loop)?
3. **Stations as Bodies:** stations sit on (orbit with) Bodies so haulers rendezvous with a moving target via the live arrival path — confirm stations are body-attached, not fixed inertial points (this is also the physically-correct model and what the dense arena will need).
4. **One spec or split plan:** this is one coherent loop but a sizeable build; the natural plan phases are: **prelude** (hash-bump scaffolding + stores) → **Stage 1** (producers + contracts + scripted haul, fixed price, conservation gate) → **Stage 2** (reprice clock + stability). OK to keep as one spec with phased plan, or split Stage 2 into its own?

## Reality-check note (standing memory: workflow agents fabricate in-code claims)

The seams cited here were **read and verified in live code this session** (stores.rs, events.rs, contract.rs, world.rs, hash.rs, config.rs — file:line refs above). The synthesis's economy-design specifics (recipe/market/contract *shapes*) are reused as design intent from the archive, not as live code; the implementer must reality-check any archive line-cite at task time (one shaping-pass refutation cited a past-EOF archive symbol).
