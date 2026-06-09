# Trophic Cut 1 Implementation Plan

> **⚠ DEFERRED (2026-06-10, owner).** Superseded in sequence by the DRL pivot: agents must
> LEARN risk (PPO+LSTM), not carry a hardcoded `risk_appetite` scalar — that scalar was the
> computed-answer reflex again. Do NOT resume this build as written. The trophic world
> returns AFTER the tactical-flight rung proves the training pipeline
> (`2026-06-10-tactical-flight-rung1-design.md`). Phase-1 foundations are parked at WIP
> commit `2e1e1ad` (pirate columns/events/RngStream::Piracy — salvageable later).

> **For agentic workers:** Execution is **subagent-driven / ultracode**. Each task is
> speced as *interface + test-behaviours + acceptance + determinism notes* — the
> implementing subagent writes the code TDD-style (failing test → minimal impl → green).
> Steps use checkbox (`- [ ]`) syntax.

**Goal:** Build the machinery for a demonstrable multi-agent boom/bust cycle with
decision-driven peer dispersion, on the live `jumpgate-core` substrate.

**Architecture:** Additive, column-oriented, integer-deterministic. Spatial pirates
(2nd trophic level, food-driven) + heterogeneous hauler decisions on a lagged
belief-state seam + an aliveness-discriminator diagnostic. New dynamics in new
scenarios; existing goldens move only once, for one named cause (hash format v3).

**Tech Stack:** Rust 2024, `#![forbid(unsafe_code)]`, FNV-1a hash, ChaCha8 seeded RNG,
generational SlotMap stores, i64 microcredits.

**Spec:** `docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md`
(read it — the §3 discriminator and §11 reframe guards govern this build).

---

## Frame guard (PDR-0006)

Every diagnostic here is a **designer's aliveness window**, never a pass/fail gate. The
integration test (Task 8) asserts the classifier **runs** and that determinism +
conservation hold — it does **NOT** assert the cycle exists. Finding the cycle is the
console tuning loop *after* this plan, not a build gate. Do not add an assertion that the
verdict must be "alive."

## Determinism / golden protocol (read before Task 1)

- New randomness uses a **new appended** `RngStream::Piracy` — never reorder existing
  salts.
- All simulation state participates in the canonical hash. Adding trophic columns is a
  **hash-format change**: bump `HASH_FORMAT_VERSION` (2 → 3) and re-pin the moved goldens.
  This is **one** named cause (format v3 adds trophic state) — the canonical batch-
  justified golden move. No logic change to existing scenarios in the same step.
- **Never invent a hash literal.** Derive every golden from actual `cargo test` output
  (the failing assertion prints expected-vs-actual); paste the *actual*; re-run to green.
- Existing behaviour-bearing tests stay green; if the hauler-decision change (Task 7)
  alters a forage-loop test outcome, that is a **separate single cause** — update with an
  explicit note, do not fold it into the format bump.

---

## Phase 1 — Foundations + the keystone instrument

### Task 1: Trophic state in the canonical hash (format v3)

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`CraftRole`, `CraftStore`)
- Modify: `crates/jumpgate-core/src/rng.rs` (`RngStream`)
- Modify: `crates/jumpgate-core/src/events.rs` (`EventKind`)
- Modify: `crates/jumpgate-core/src/types.rs` (`RouteKey`)
- Modify: `crates/jumpgate-core/src/hash.rs` (fold new columns; bump `HASH_FORMAT_VERSION`)
- Modify: `crates/jumpgate-core/src/lib.rs`

**Interface to land:**
- `CraftRole::Pirate` with appended `rank() = 2` (existing `Idle=0`, `Hauler=1` unchanged).
- `CraftStore` new columns (pushed/defaulted in every constructor + push path):
  - `risk_appetite: Vec<i32>` (fixed-point 0..=1000; 0 default; cautious→greedy).
  - `pirate: Vec<Option<PirateState>>` where `PirateState { food_micros: i64,
    notoriety: u32, lie_low_until: Tick }`.
- `RngStream::Piracy` appended with a new fixed `SALT_PIRACY` constant.
- `EventKind` additive variants: `Robbed { pirate, hauler, contract, value_micros }`,
  `DrivenOff { pirate, hauler }`, `HaulerKilled { pirate, hauler }`,
  `PirateLieLow { pirate, until: Tick }`, `PirateLeft { pirate }`,
  `PirateSpawned { pirate }`.
- `RouteKey(pub StationId, pub StationId)` in `types.rs` (`Copy + Eq + Hash + Ord`).
- `hash.rs`: fold `risk_appetite`, `pirate` (self-delimiting: tag 0 for `None`, tag 1 +
  fields for `Some`) into the craft state hash; `HASH_FORMAT_VERSION = 3`.

**Test-behaviours (TDD):**
- [ ] New columns round-trip and default correctly on `CraftStore` push.
- [ ] `PirateState` folds self-delimitingly (None vs Some give different, stable hashes).
- [ ] Existing suite compiles and is green after re-pinning.

**Golden re-pin (single cause = format v3):**
- [ ] Bump `HASH_FORMAT_VERSION` to 3. Run `cargo test -p jumpgate-core`; the determinism
  goldens fail showing actual hashes. Re-pin zero-state, config, and any pinned state
  golden to the **actual** printed values. Confirm `record_then_replay_is_bit_identical`
  still passes (format v3 on both sides).
- [ ] `provenance` hash_fmt_v reflects 3 if it records the format version.

**Acceptance:** `cargo test -p jumpgate-core` green; `cargo clippy --all-targets` clean;
exactly the format-bump goldens moved; no behavioural change to existing scenarios.

### Task 2: Diagnostics + the aliveness discriminator

**Files:**
- Create: `crates/jumpgate-core/src/diagnostics.rs`
- Modify: `crates/jumpgate-core/src/lib.rs`

**Interface to land:**
- `TrophicSample { tick: Tick, active_pirates: u32, lying_low: u32,
  active_hauler_density: u32, robs_this_tick: u32, per_route_risk: Vec<(RouteKey, i64)>,
  avg_cargo_in_flight_micros: i64 }`.
- `HaulerLedger { craft: CraftId, risk_appetite: i32, wealth_micros: i64,
  deliveries: u32, robs_suffered: u32, alive: bool }`.
- `Diagnosis { cycled: bool, risk_heterogeneous: bool, outcomes_disperse: bool,
  verdict: Verdict }` where `Verdict` is the §3 matrix outcome
  (`BarMet | NoCycle | RiskEqualized | DecisionNotTranslating`).
- `fn classify(series: &[TrophicSample], ledger: &[HaulerLedger]) -> Diagnosis`:
  - `cycled` = anti-phase amplitude of `active_pirates` vs `active_hauler_density` above a
    noise floor, neither pinned at 0/saturated (integer/fixed-point autocorrelation).
  - `risk_heterogeneous` = cross-route variance of `per_route_risk` **and** its temporal
    autocorrelation both above thresholds.
  - `outcomes_disperse` = variance of `wealth_micros` above a floor **and** monotone-ish
    association with `risk_appetite` (e.g. rank correlation sign/strength).
  - `verdict` per the §3 matrix.
- Thresholds are named `const`s at the top of the file (the console tuning loop reads
  these; they are *diagnostic* thresholds, NOT acceptance gates).

**Test-behaviours (TDD) — the four corners, the keystone:**
- [ ] Synthetic anti-phase oscillating series + dispersed appetite-tracking ledger → `BarMet`.
- [ ] Flat/equilibrium series → `NoCycle`.
- [ ] Oscillating but uniform per-route risk (variance→0) → `RiskEqualized`.
- [ ] Oscillating, heterogeneous risk, but flat ledger → `DecisionNotTranslating`.
- [ ] Determinism: `classify` is pure (same input → same output).

**Acceptance:** all four corners classify correctly; pure; clippy clean.

---

## Phase 2 — The trophic machinery

### Task 3: Belief-state seam (observability as first-class data)

**Files:** Create `crates/jumpgate-core/src/belief.rs`; modify `lib.rs`; add config knobs.

**Interface:**
- `RiskBeliefs` holding a per-`RouteKey` integer risk register plus a lagged read buffer.
- `fn bump(&mut self, route: RouteKey, amount: i64)` — on an observed rob.
- `fn decay(&mut self)` — each tick, integer geometric decay by `config.belief_decay`.
- `fn snapshot_for_read(&mut self)` — rotate the lag buffer (`config.belief_lag` ticks).
- `fn believed_risk(&self, route: RouteKey) -> i64` — reads the **lagged** value.
- Config: `belief_decay: i32` (fixed-point), `belief_lag: u8`.

**Test-behaviours:** bump then decay follows the integer trajectory; `believed_risk`
returns the value from `lag` ticks ago, not the live one; determinism; empty route → 0.

**Acceptance:** lagged read provably trails live state; clippy clean.

### Task 4: Pirate target choice + intercept

**Files:** Create `crates/jumpgate-core/src/pirate.rs`; modify `lib.rs`.

**Interface:**
- `fn choose_target(pr: usize, ships: &CraftStore, contracts: &ContractStore,
  beliefs: &RiskBeliefs, eph: &Ephemeris, tick: Tick, rng: &mut ChaCha8Rng)
  -> Option<CraftId>`: among in-transit cargo-bearing haulers, score by
  `cargo_value / reachability` **de-weighted by local over-fishing** (recent rob density
  near the target's route, read from beliefs); deterministic tie-break; respects a bounded
  `pirate_max_reach` knob (the primary locality lever).
- `fn set_intercept(pr: usize, target: CraftId, ships: &mut CraftStore, ...)`: set nav to
  `Seek` the target's predicted position (reuse the rendezvous/velocity-match primitive).
- Lie-low pirates (`lie_low_until > tick`) never choose a target.

**Test-behaviours:** picks the fattest *reachable* hauler; skips over-fished routes; a
lie-low pirate returns `None`; unreachable target excluded; seeded determinism.

**Acceptance:** behaviours hold; clippy clean.

### Task 5: Encounter resolution

**Files:** Modify `crates/jumpgate-core/src/pirate.rs`.

**Interface:**
- `fn resolve_encounters(ships, contracts, corporations, stations, beliefs, eph, tick,
  rng: RngStream::Piracy, events, config)`: for each non-lie-low pirate **within
  `engage_range` AND velocity-matched** (`|Δv| < match_speed`) to a hauler, roll outcome
  on the Piracy stream:
  - **rob** (`p_rob`): transfer cargo value to pirate `food_micros`; fail the contract
    (refund escrow per the existing failure path); `+notoriety`; `bump` beliefs for the
    route; emit `Robbed`.
  - **driven-off** (`p_drive`): no transfer; emit `DrivenOff`.
  - **kill** (`p_kill`): remove the hauler (cut-1 salvage stub); fail+refund its contract;
    `+notoriety`; bump beliefs; emit `HaulerKilled`.

**Test-behaviours (conservation is a gate here):**
- [ ] Out-of-range or velocity-unmatched → no encounter (gating).
- [ ] Rob: `Σ stock + Σ in-transit + Σ pirate-held == initial + mined − consumed` holds;
  contract fails; credit invariant holds (escrow refunded).
- [ ] Kill: hauler removed; contract fails+refunds; conservation holds.
- [ ] Determinism on the Piracy stream.

**Acceptance:** conservation + credit invariants proven under rob and kill; clippy clean.

### Task 6: Food-driven pirate population

**Files:** Modify `crates/jumpgate-core/src/pirate.rs`.

**Interface:**
- `fn update_pirate_population(ships, eph, tick, rng, events, config)`:
  - Per-tick food upkeep (`food_micros -= upkeep`).
  - `food_micros >= spawn_threshold` → spawn a pirate (split food); emit `PirateSpawned`.
  - `food_micros <= 0` → go **lie-low** (`lie_low_until = tick + lie_low_ticks`, food reset
    to a floor) — the **refuge**, NOT removal; emit `PirateLieLow`. Repeated starvation
    past a tolerance → `PirateLeft` (remove).
  - `notoriety >= heat_threshold` → forced lie-low (`LIE_LOW < WANTED` refuge), notoriety
    decays while lying low.

**Test-behaviours:** starve → lie-low (still present, off-field); well-fed → spawn;
high-notoriety → forced lie-low + notoriety decay; lie-low pirate excluded from Task-4
targeting and Task-5 encounters; determinism.

**Acceptance:** the refuge keeps prey from extinction (a lie-low pirate stops predating);
clippy clean.

### Task 7: Heterogeneous hauler decision (belief-driven)

**Files:** Modify `crates/jumpgate-core/src/economy.rs` (the ASSIGN step in
`run_scripted_dispatch`).

**Interface:**
- Replace lowest-`ContractId` ASSIGN with a per-Idle-hauler **choice** over `Offered`
  contracts: `score = reward_micros − risk_weight(risk_appetite) *
  beliefs.believed_risk(route_of(contract))`. Greedy (high appetite) discounts risk;
  cautious (low appetite) avoids it. Deterministic tie-break by `ContractId`.
- The hauler reads **`believed_risk`** (Task 3), never ground-truth pirate positions.

**Test-behaviours:** given one safe-low and one risky-high offer, a cautious hauler takes
safe-low and a greedy hauler takes risky-high; the choice changes when *belief* changes
even if ground truth does not (proves the seam); determinism; the existing forage-loop
tests still pass OR move for the explicit single cause "decision now belief-weighted"
(note it).

**Acceptance:** appetite changes the choice; belief (not truth) drives it; clippy clean.

### Task 8: Wire the tick, the scenario, and integration

**Files:** Modify `crates/jumpgate-core/src/world.rs`; add a runnable example or a
`#[test]` that prints the trace.

**Interface / wiring (spec §5 order):**
- Insert into the step loop: hauler decision (T7) → pirate decision (T4) → physics →
  encounter (T5) → population (T6) → belief update (T3 bump+decay+snapshot) → diagnostics
  sample (T2).
- `fn scenario_trophic_cut1(seed: u64) -> World`: N haulers with a seeded `risk_appetite`
  spectrum, M pirates, ≥3 routes (distinct station pairs), **non-binding fuel**, producers
  feeding stock. Records a `Vec<TrophicSample>` + builds the `HaulerLedger` at end.
- A test/example `trophic_cut1_emits_a_readable_trace` that runs K ticks and prints the
  series + ledger + `classify(...)` verdict + a short chronicle for console reading.

**Test-behaviours (gates — NOT the verdict):**
- [ ] `record_then_replay_is_bit_identical` for `scenario_trophic_cut1` (new golden,
  single cause = new scenario).
- [ ] Conservation (resource + credit, incl. pirate-held cargo) holds over the run.
- [ ] `classify(...)` **runs** and returns a `Diagnosis` (no assertion on which verdict).

**Acceptance:** new scenario replays bit-identical; conservation holds; the trace prints;
`cargo test -p jumpgate-core` green; `cargo clippy --all-targets` clean.

---

## Phase 3 — Console tuning loop (after the build; me-driven)

Run `scenario_trophic_cut1`, read the §3 windows, and tune **sequentially** with the
diagnosis matrix: `NoCycle` → food/lie-low/regen; `RiskEqualized` → locality (reach ↓,
over-fish ↑, notoriety ↑, belief-lag ↑); `DecisionNotTranslating` → appetite spread /
belief perfection. Iterate until `BarMet` is demonstrable and seed-reproducible. Stats are
windows the whole way — never targets. This is where the owner's "until we have a
demonstrable cycle" is actually satisfied.

## Self-review notes

- Spec coverage: Components A→Tasks 4/5/6; B→Task 5; C→Task 7; D→Task 3; E→Task 2;
  wiring/scenario→Task 8; foundations/hash→Task 1. All covered.
- No placeholders; the only deferred decision (pirate storage shape) is fixed here
  (`Option<PirateState>` column).
- Type consistency: `RouteKey` defined in Task 1, consumed by Tasks 2/3/4/7; `PirateState`
  fields consistent across Tasks 1/5/6; `Diagnosis`/`Verdict` consistent Tasks 2/8.
