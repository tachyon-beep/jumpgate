# Systems Review: Goods-as-Goods Rung A (Bazaar)

**Plan:** `/tmp/gag-plan-a/assembled-plan.md`
**Spec:** `/home/john/jumpgate/docs/superpowers/specs/2026-06-12-goods-as-goods-design.md`
**Panel synthesis:** `/home/john/jumpgate/docs/superpowers/posts/2026-06-12-goods-as-goods-panel/synthesis-recommended-cut.md`
**Codebase HEAD:** 140a8f1
**Review date:** 2026-06-12
**Reviewer lens:** Systems / second-order effects

---

## Findings Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 1     |
| MAJOR    | 3     |
| MINOR    | 2     |

---

## CRITICAL Findings

### C-1: Own-Trade Sell Mechanism Entirely Missing from A5.1

**Plan location:** `/tmp/gag-plan-a/draft-a5.md`, Task A5.1, Step 6 (`run_trade_policies`)

**Evidence (plan code, Step 6):**

```rust
for crow in 0..ships.ids.len() {
    if ships.role[crow] == CraftRole::Pirate { continue; }
    if craft_cfg.get(crow).is_some_and(|c| !c.scripted) { continue; }
    if ships.role[crow] != CraftRole::Idle { continue; }   // skips all non-Idle
    if ships.pending_trade_buy[crow].is_some() { continue; }
    if ships.pending_refuel[crow].is_some() { continue; }
    // ... only writes pending_trade_buy[crow]
    // NO Branch 2: laden craft at destination -> pending_trade_sell
}
```

The plan defines `pending_trade_buy` and `pending_trade_sell` columns (A2 schema), and the settle stage (A5.1 Step 10, `settle_trades`) consumes both. But `run_trade_policies` only has a buy branch for Idle craft. There is no branch for the case: hold is non-empty AND craft is docked at its sell destination.

**System-level failure chain:**

```
run_trade_policies (own-trade branch)
    ↓ writes pending_trade_buy for empty-hold Idle craft
    ↓ settle_trades: craft flies to source, buys, transition to Transit/Enroute
    ↓ craft arrives at sell destination, transitions to Idle
    ↓ run_trade_policies again: hold != empty but no Sell branch
    ↓ craft has pending_trade_buy == None (old buy was settled)
    ↓ BUY branch fires again: craft is Idle, tries to buy — but hold is already full
    ↓ buy logic likely skipped (capacity check) or written over the hold
    ↓ craft permanently loops: Idle→Transit→Idle with no sell
```

The own-trade hauler never monetizes its cargo. `wallet` never grows. The monoculture risk (MAJOR-3) cannot even materialize — the more immediate consequence is that every own-trader is a dead-weight carrier cycling to the source station repeatedly, draining fuel and wage-equivalent treasury without generating revenue.

**Second-order effect:** Exchange treasury drains for fuel subsidies while zero profitable own-trade revenue flows back. MAJOR-2 (Exchange battery arithmetic) becomes a certainty rather than a worst-case estimate.

**Concrete fix:**

Add Branch 2 to `run_trade_policies`, after the buy branch:

```rust
// Branch 2: own-trade SELL — laden craft at profitable destination
if !ships.hold[crow].is_empty()
    && ships.pending_trade_sell[crow].is_none()
    && ships.pending_trade_buy[crow].is_none()
{
    let station = ships.pos[crow];  // must be docked
    let (good, qty) = /* read hold */ ;
    let sell_price = boards.ask_price(station, good);
    // write pending_trade_sell[crow] = Some(TradeSell { station, good, qty, price: sell_price })
    // also: set nav destination to station (already there — nav is a no-op)
}
```

Branch 2 fires when role == Idle (hold non-empty, at sell destination). Branch 1 fires when role == Idle (hold empty). The guard `ships.role[crow] != CraftRole::Idle { continue }` is shared — keep it; add the hold-empty vs hold-full split inside.

**Confidence:** High — the plan code is explicit; `pending_trade_sell` is defined in the schema but never populated by any policy stage shown.

---

## MAJOR Findings

### M-1: Corp Rotation Is Vacuous with One Exchange Corp

**Plan location:** `/tmp/gag-plan-a/draft-a5.md`, Task A5.2, `scenario_bazaar` factory; synthesis L1-C2

**Evidence:**

Synthesis L1-C2 claims three anti-self-averaging fixes: corp rotation, clumped topology, HHI concentration panel. The plan implements corp rotation as:

```rust
let corp_idx = (route_index + scan_index) % n_corps;
```

`scenario_bazaar` constructs exactly one corporation (the Exchange):

```rust
corporations: vec![CorporationInit {
    treasury_micros: BAZAAR_EXCHANGE_TREASURY_MICROS,
    // ...
}],
```

`n_corps = 1`. Therefore `(route_index + scan_index) % 1 = 0` for all values of `route_index` and `scan_index`. The rotation is the identity function. There is no variance in first-refusal ordering; all arbitrage decisions always go to corp 0 (Exchange). The synthesis commitment "corp rotation provides first-refusal variance" does not hold in the bazaar scenario.

**Second-order effect:** The three anti-averaging fixes reduce to two (clumped topology + HHI panel). Whether two is sufficient depends on the concentration panel catching monoculture — which requires the panel to fire before the divergence is locked in. The panel is a passive observer, not a blocker.

**Concrete fix (two options):**

Option A (minimal): Add a note to A5.2 and the WA3 instrument that corp rotation provides no variance in bazaar (1 corp); rely solely on clumped topology and HHI. Update the synthesis claim to reflect actual coverage.

Option B (structural): Split the Exchange into 3-5 virtual sub-corps with separate treasury buckets but unified settlement. Rotation then has non-zero variance. This is more work but delivers the claimed statistical property.

Option A is sufficient for Rung A; Option B is a Rung B candidate.

**Confidence:** High — `n % 1 = 0` is arithmetic; the `corporations: vec![...]` count is confirmed in the plan.

---

### M-2: Exchange Battery Is a Panel Estimate, Not a Calibrated Value

**Plan location:** `/tmp/gag-plan-a/draft-a5.md`, Task A5.2, `BAZAAR_EXCHANGE_TREASURY_MICROS = 5_400_000_000`

**Evidence:**

The 5.4B figure appears in the plan without derivation. The synthesis panel critique offered 5.4B as an estimate. The plan adopts it directly.

**Worst-case drain computation (from plan numbers):**

- `BAZAAR_WAGE_MICROS` (from A5.1): base wage per trip = 50_000; package wage at spread trigger = spread × 0.5
- Trade good: source price ~112_000, sink price ~400_000; spread = 288_000 per unit × qty=5 → package spread = 1_440_000; wage = 720_000 per trip
- Fuel good: source ~28_000, sink ~100_000; spread = 72_000 × qty=5 → 360_000; wage = 230_000 per trip
- Trip length at arc=1 (nearest hops): ~200 ticks (transit + dwell)
- 100_000 ticks / 200 ticks = 500 trips per hauler
- 20 haulers × 500 trips × 720_000 (trade wage) = 7_200_000_000

7.2B > 5.4B. The worst case (all haulers on high-spread trade routes, zero revenue recycling before spreads close) exhausts the battery before the 100k-tick run ends.

**Mitigating factor:** In equilibrium, the Exchange earns revenue on profitable packages (D3: Exchange buys at source price, sells at sink price; net = spread - 2 × transport_cost > 0 when trigger fires). Revenue recycling reduces net drain. The question is whether recycling begins fast enough to outpace early-game maximum wages.

**Feedback loop risk:** High wages attract haulers → Exchange pays → treasury shrinks → Exchange must post lower spreads to reduce wages → fewer haulers qualify → but REPOST is structurally off so spreads don't auto-adjust → Exchange drains to zero → Exchange inactive (named behavior) → all own-trade stops → scenario collapses to wage-only.

**Concrete fix:**

1. Before committing A5.2, run a calibration sweep: 10 seeds × [3M, 5.4B, 10B, 20B] treasury; plot Exchange balance vs tick; find the treasury value at which balance > 0 at tick 100k for ≥ 8/10 seeds.
2. Alternatively, add a `battery_refill_rate_micros_per_tick` parameter (e.g., 10_000/tick = 1B/100k) to give the Exchange a slow trickle income independent of arbitrage profits. This decouples solvency from early-game convergence speed.
3. Log Exchange treasury at WINDOW boundaries (WA3 instrument) so the drain curve is visible in the first science run.

**Confidence:** Moderate — the 7.2B worst-case is computed from plan constants, but actual drain depends on route mix and convergence speed, which are not statically determinable.

---

### M-3: Two-Mode Monoculture Risk with `trade_reserve_micros = 0`

**Plan location:** `/tmp/gag-plan-a/draft-a5.md`, Task A5.1, craft initialization; Task A5.2, `CraftInit`

**Evidence:**

The plan initializes all haulers with:

```rust
CraftInit {
    trade_reserve_micros: 0,
    // ...
}
```

The two-mode switch condition (D6) fires when:

```
best_trade_net > best_wage_net
```

With `trade_reserve_micros = 0`, the capitalization threshold for switching to own-trade is: wallet ≥ buy_cost (source price × qty). For trade goods at source price ~112_000 × qty=5 = 560_000 micros.

**Transition timeline (worst case):**

- Tick 0: all haulers start at wallet=0, all in wage mode
- First trip earns wage: 720_000 (trade good, arc=1) — exceeds 560_000 threshold
- After ONE trip, wallet > buy_cost → best_trade_net > best_wage_net for high-spread routes
- All 20 haulers could switch to own-trade simultaneously after their first trip

**Prey-shrink feedback loop:**

```
All haulers switch to own-trade
    ↓ Pirates target haulers (trophic food = hauler encounters)
    ↓ Own-trade haulers carry cargo (higher-value targets)
    ↓ More pirate attacks per hauler → hauler loss rate increases
    ↓ Fewer haulers → pirate food shrinks (prey-shrink)
    ↓ PermanentPeace or feast/famine oscillation destabilized
    ↓ WA5 trophic verdict confounded by mode-switch, not organic dynamics
```

The synthesis identifies WA3+WA5 joint read as the control: "high own-trade share + PermanentPeace = confound, not finding." This is correct IF the share is observable. But the confound manifests before the first WA3 window (tick 2000) has closed, before the experimenter can intervene.

**Concrete fix:**

1. Set `trade_reserve_micros` to a per-scenario tunable with a non-zero default in `scenario_bazaar`: e.g., `BAZAAR_CRAFT_TRADE_RESERVE_MICROS = 2_000_000` (requires ~3 trips to accumulate). This staggered capitalization prevents simultaneous switching.
2. Add WA3 instrument line: `own_trade_share` (fraction of hauler-trips that were own-trade vs wage) per WINDOW, so the confound is detectable before the run ends.
3. Add a soft gate: if `own_trade_share > 0.8` at tick 10_000, flag a WARNING in the run log (does not halt; allows the experimenter to decide whether to discard the seed).

**Confidence:** Moderate — the threshold arithmetic is from plan constants; the "all switch after one trip" scenario depends on the actual wage paid on the first trip, which requires a live run to confirm.

---

## MINOR Findings

### m-1: TradeBought/TradeSold Exhaustive Match Not Explicitly Instructed in A5.1

**Plan location:** `/tmp/gag-plan-a/draft-a5.md`, Task A5.1, Step 11

**Evidence:**

Step 11 instructs: "Delete `EventKind::Trade`; add `EventKind::TradeBought { ... }` and `EventKind::TradeSold { ... }`." The synthesis L2-C2 mandates replacing `_ => None` wildcards in `chronicle_subject` and `gossip_log_event_json` with exhaustive arms.

Step 11 does not include explicit instructions to update these two match sites. The synthesis mandate is dispositioned (CRITICAL in synthesis → instructed in plan) but the instruction is missing from the plan text.

**Mitigating factor:** Rust's exhaustive match enforcement means this will produce a compile error when the new variants are added. The implementer cannot ship without fixing it. The risk is not a silent defect but a build failure that delays A5.1 without guidance on the intended arm bodies.

**Concrete fix:**

Add to A5.1 Step 11: "Update `chronicle_subject` and `gossip_log_event_json` — replace any `_ => None` wildcard with explicit arms for `TradeBought` and `TradeSold`. For `TradeBought`: subject = the buying craft; for `TradeSold`: subject = the selling craft. For `gossip_log_event_json`: map both to the existing trade gossip schema with the new `bought`/`sold` discriminator field."

**Confidence:** High — the plan Step 11 text is explicit about the enum change but silent on the match sites; synthesis L2-C2 is explicit about the exhaustive match requirement.

---

### m-2: `endpoint_station_rows()` Returns All-False in Bazaar — CommonKnowledge Structurally Impossible

**Plan location:** `/tmp/gag-plan-a/draft-a6.md`, WA3+WA5 joint reader, `media_classify` call

**Evidence (`/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs`, lines 844-855):**

```rust
fn endpoint_station_rows(cfg: &ScenarioConfig) -> Vec<bool> {
    // reads cfg.contracts to build CommonKnowledge coverage denominator
    // returns all-false when cfg.contracts is empty
}
```

`scenario_bazaar` has `contracts: vec![]` (no contract routes). Therefore `endpoint_station_rows()` returns a `Vec<bool>` of all `false`. The `media_classify` function requires at least one `true` endpoint row to produce a `CommonKnowledge` reading; with all-false, it always returns `None` or the default non-CommonKnowledge class.

**Impact scope:** Limited to the MEDIA line of the WA3/WA5 joint output. The WA5 trophic verdict (pirate boom/bust, PermanentPeace) does not depend on CommonKnowledge classification; it reads the ecosystem oscillation signal directly. The WA3 exchange-flow analysis reads spreads and treasury, not media class.

**Second-order risk:** If the A6 science script interprets a missing MEDIA reading as a "control" observation rather than a structural artifact, it could produce a false CommonKnowledge = 0 datum in a cross-scenario comparison. This would be a measurement artifact, not a game-world signal.

**Concrete fix:**

1. Add a comment in the WA3 reader: "bazaar has no contracts; `endpoint_rows` will be all-false; MEDIA classification is N/A for this scenario — skip or log as 'no-contracts-baseline'."
2. Guard the `media_classify` call: `if endpoint_rows.iter().any(|&v| v) { media_classify(...) } else { MediaClass::NoContractsBaseline }`.
3. Document in the A6 science task that bazaar MEDIA readings are structurally absent and should not be compared against frontier/trophic MEDIA readings.

**Confidence:** High — `endpoint_station_rows()` code at diagnostics.rs:844-855 is confirmed; `contracts: vec![]` is confirmed in the plan's `scenario_bazaar` factory.

---

## Systems Analysis Sections

### Dependency Analysis

**Components changed:** Exchange corp schema, `run_trade_policies`, `settle_trades`, `pending_trade_buy/sell` columns, `EventKind` enum, `scenario_bazaar` factory, WA3/WA5 joint reader

**Dependency chain:**

```
run_trade_policies (A5.1)
    ↓ writes pending_trade_buy / pending_trade_sell
    ↓ settle_trades (A5.1) consumes both columns
    ↓ CraftRole transitions (Idle → Transit → Enroute → Idle)
    ↓ EventKind::TradeBought / TradeSold
    ↓ chronicle_subject (narrative)
    ↓ gossip_log_event_json (media layer)
    ↓ WA3 joint reader (A6)
    ↓ WA5 trophic verdict (A6)

scenario_bazaar (A5.2)
    ↓ 1 Exchange corp (n_corps = 1)
    ↓ corp rotation formula → vacuous
    ↓ clumped topology (L1-C2) → still active
    ↓ HHI panel (WA3) → still active
    ↓ Exchange treasury (5.4B) → drain risk

CraftInit.trade_reserve_micros = 0
    ↓ two-mode threshold
    ↓ all haulers → own-trade after first trip
    ↓ prey-shrink confound
    ↓ WA5 trophic verdict confounded
```

**Ripple risk:** High (C-1 makes own-trade non-functional; M-2 and M-3 are calibration risks that compound each other)

### Feedback Loops

| Potential Loop | Type | Risk | Mitigation in Plan |
|----------------|------|------|-------------------|
| Own-trade buy → no sell → Exchange pays fuel subsidies → treasury drain → Exchange inactive → all trade stops | Reinforcing (drain) | CRITICAL | None — sell branch absent |
| High wage → treasury drain → spreads cannot adjust (REPOST off) → drain continues | Reinforcing (drain) | High | Battery sizing (uncalibrated) |
| All haulers switch to own-trade → more pirate targets → prey-shrink → PermanentPeace | Reinforcing (destabilizing) | Medium | WA3+WA5 joint read (post-hoc) |
| Corp rotation → 0 variance → concentration not checked until WA3 window closes | Delayed | Medium | HHI panel (observer, not gate) |

### Historical Pattern Match

| Pattern | Match Level | Concern |
|---------|-------------|---------|
| "Missing symmetric operation" | Yes | Buy without sell is the classic half-implementation of a two-sided operation; seen in every marketplace implementation that ships before end-to-end testing |
| "Panel estimate as production constant" | Yes | 5.4B adopted directly from critique; calibration run was never specified |
| "Threshold with zero reserve" | Yes | `trade_reserve_micros = 0` means the switch fires at the first opportunity; staggered thresholds are the standard fix |
| "N=1 rotation" | Yes | Modulo-1 is always 0; this is a common off-by-one in parameterized code that was designed for N>1 but run with N=1 |

### Failure Mode Analysis

| Change | Silent Failure | Loud Failure | Idempotent? |
|--------|---------------|--------------|-------------|
| run_trade_policies (own-trade) | Haulers cycle source→source with cargo; treasury drains; no error | None — compiles clean, runs silently wrong | N/A — stateful transition |
| Exchange treasury 5.4B | Balance reaches 0; Exchange inactive; named behavior fires silently | None — no assertion on treasury floor | No — treasury is monotone decreasing until recycling begins |
| trade_reserve_micros = 0 | All haulers switch after tick ~200; prey-shrink begins | None — no WARNING until WA3 window closes | N/A |
| TradeBought/TradeSold exhaustive match | Compile error (not silent) | Build failure blocks A5.1 delivery | N/A |
| endpoint_rows all-false | MEDIA = None silently; confounds cross-scenario comparison | None | N/A |

### Integration Point Stress

| Integration Point | Failure Modes | Plan Coverage |
|-------------------|---------------|---------------|
| Exchange corp treasury | Drain to zero → named inactive behavior | Named but not gated; no calibration run specified |
| `pending_trade_sell` column | Never written → settle_trades never fires sell path | Not covered — sell branch absent |
| `chronicle_subject` / `gossip_log_event_json` | Compile error on new EventKind variants | Mitigated by Rust exhaustive match (loud failure) |
| `endpoint_station_rows` | All-false → MEDIA N/A | Not documented; silent measurement gap |

### Timing Assumptions

| Assumption | What Could Break It | Severity |
|------------|---------------------|----------|
| "Spreads close before treasury drains" | High-wage early game; slow convergence | High |
| "All haulers start in wage mode (wallet=0)" | True at tick 0; breaks after first trip | Medium |
| "Corp rotation provides variance" | n_corps = 1 → rotation is identity | Medium (statistical coverage only) |
| "WA3 window at tick 2000 catches monoculture" | Monoculture may be complete at tick 200 | Medium |

---

## Confidence Assessment

**Overall Confidence:** High for C-1 and m-1/m-2; Moderate for M-2 and M-3

| Finding | Confidence | Basis |
|---------|------------|-------|
| C-1: sell branch absent | High | Plan Step 6 code is explicit; `pending_trade_sell` defined in schema but never populated in any shown policy stage |
| M-1: corp rotation vacuous | High | `n_corps = 1` from plan factory; `n % 1 = 0` is arithmetic |
| M-2: battery uncalibrated | Moderate | Worst-case computation from plan constants; actual drain depends on route mix and convergence not statically determinable |
| M-3: monoculture risk | Moderate | Threshold arithmetic from plan constants; actual switch timing requires live run to confirm |
| m-1: exhaustive match gap | High | Plan Step 11 text confirmed; synthesis L2-C2 confirmed; gap between them is direct |
| m-2: endpoint_rows all-false | High | `endpoint_station_rows()` code at diagnostics.rs:844-855 confirmed; `contracts: vec![]` confirmed in plan |

---

## Risk Assessment

**Implementation Risk:** High (C-1 makes own-trade non-functional; M-2/M-3 are calibration risks)
**Reversibility:** Moderate (C-1 requires adding a sell branch — new code, not a config change; M-1/M-2/M-3 are config/tunable changes)

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| Own-trade deadlock (C-1) | Critical | Certain if unaddressed | Add sell branch to `run_trade_policies` before A5.1 ships |
| Battery exhaustion (M-2) | High | Possible (depends on convergence speed) | Calibration sweep before committing 5.4B; add trickle-income fallback |
| Two-mode monoculture (M-3) | High | Possible (depends on first-trip wage) | Non-zero `trade_reserve_micros`; WA3 own-trade-share instrument |
| Corp rotation vacuous (M-1) | Medium | Certain (arithmetic) | Document limitation; rely on clumped topology + HHI; defer Option B to Rung B |
| TradeBought/TradeSold compile failure (m-1) | Low | Certain (Rust enforces) | Add explicit match arms to Step 11 instructions |
| CommonKnowledge N/A in bazaar (m-2) | Low | Certain (no contracts) | Document; guard `media_classify` call |

---

## Information Gaps

The following would improve this analysis:

1. [ ] **Live calibration run for Exchange battery** — a 10-seed sweep at 3M/5.4B/10B/20B treasury would convert M-2 from "possible" to "measured"; current analysis is worst-case arithmetic only
2. [ ] **First-trip wage distribution** — the monoculture risk (M-3) depends on the actual wage paid on the first trip; this requires observing the initial spread state at tick 16 (first scan), which is not statically determinable without knowing which route the first hauler takes
3. [ ] **Settle_trades code for the sell path** — the plan shows the buy settlement but not the sell settlement; if sell settlement is also incomplete, C-1 is broader than the missing branch alone
4. [ ] **Behavior digest baseline construction** — the plan specifies "pin at A0 tip" but does not show which stdout lines are in scope for the trophic/frontier digest; a full line enumeration would confirm whether any A1-A5 instrument lines print unconditionally

---

## Caveats and Required Follow-ups

### Before Relying on This Analysis

- [ ] Verify that `settle_trades` (A5.1 Step 10) has a sell path — if the settle stage is also incomplete, C-1 requires two fixes, not one
- [ ] Confirm `BAZAAR_WAGE_MICROS` formula; the worst-case 7.2B computation assumes `wage = spread × 0.5`, which is the D6 spec formula but may have been adjusted in A5.1
- [ ] Re-check corp rotation after A5.2 is finalized — if the plan is updated to use multiple sub-corps, M-1 is resolved

### Assumptions Made

- `run_trade_policies` as shown in the plan is the complete policy stage code; no sell branch exists elsewhere in A5.1
- `n_corps = 1` in `scenario_bazaar` is intentional (Exchange is the sole corp); no additional corps are added in later A-phase steps
- The 5.4B figure has no derivation documentation beyond the panel critique estimate

### Limitations

- This analysis does NOT cover symbol existence, type correctness, or test coverage — other reviewers handle those
- This analysis does NOT verify quantitative claims (latency, throughput, exact convergence speed) — those require measurement
- The "all haulers switch after one trip" scenario is a worst case; actual behavior depends on route assignment order and initial spread distribution
