# Commons-Miner Analytic Cut — design spec

**Status:** draft for owner review (2026-06-10)
**Issue:** `jumpgate-aec6e7bc14` (scale/density DRL arena — first increment)
**Frame:** game science (`jumpgate-game-science-principle`) — the cut IS the measurement apparatus; pre-register the hypothesis + gate, then build.
**Provenance:** shaped via brainstorming; hardened by a 10-agent expert review (6 lenses + 3 adversarial critics + synthesis, run `wf_ce2216f5-5f2`). Owner decisions: arena = commons miner; home = standalone Rust; fidelity = exact-small + observe-large; **observability = full-info cut FIRST, partial-observability deferred to a separate owner-gated bet**.

---

## 1. Goal & what this is NOT

**Goal.** On *live code*, measure whether the commons/density structure has **learnable DRL room** — can a learner beat the best deployable script by ≥10% of ceiling — BEFORE any world-building or any learner is trained. Re-measures the commons miner's presolvability on real code (dissolving the shaping pass's workflow-generated/unverified ceiling numbers, `vertical-slice-shaping-findings.md` L50/52), and builds + calibrates the measurement apparatus (the N-scaling MC estimator, the determinism/golden harness) that the later information-room bet will reuse.

**This is NOT** the world, a learner, or the partial-observability/information arena. It runs **full-information only**. Per PDR-0005 it is expected to NO-GO by precedent; a fast, substrate-honest NO-GO that is *reported and honored* is the correct outcome. Crossing into the information arena is a separate decision the owner makes *after* seeing this result (§9).

## 2. Pre-registered hypothesis & gate (fixed BEFORE building)

- **H₀ (null, expected):** the full-info commons-miner game is presolvable — a best fixed reactive script captures ≥90% of the constant→omniscient dynamic range; `frac < 0.10`; and/or `frac` decays toward the gate as N rises (LLN self-averaging).
- **H₁ (room):** `frac(N) ≥ 0.10` **and** flat-or-rising in N across the calibrated ladder.
- **Gate constant:** `GAP_FRAC_MIN = 0.10` (pre-registered; do not move post-hoc).
- **Grade by fraction-of-ceiling, never by Cohen's d** (probe-5 scar: d=3.54 certified a ~4% gap as *reliably positive*, not *large* — effect size ≠ magnitude).
- **NO-GO is dispositive and honored** for the full-info commons game. The honest scope of a NO-GO is explicit: *"the full-info game is presolvable; this did not test the information game"* (§9), which is then surfaced as the owner's next decision — **not** an automatic escalation.

## 3. Arena model (integer, deterministic)

All state and transitions are **integer** (`u32`/`u64`); **no `f64` anywhere** (float → platform rounding → kills cross-run replay). The crate is outside core's RNG-lint perimeter, so this is by-convention and enforced by a pinned golden trajectory hash (§7).

**Regions** (`M`):
- `stock: u32` — current minable units, `0..=richness_cap`.
- `richness_cap: u32` — per-region max; the **heterogeneity / field-correlation axis**.
- `regen_per_tick: u32` — `0` = one-shot exhaustion; `>0` = slow regen (**regen-rate axis**).

**Yield law** (saturating-in-stock, crowd-split, integer; `occupants ≥ 1`):
```
per_ship_yield = (stock as u64 * richness_cap as u64) / (STOCK_MAX as u64 * occupants as u64)   // floored
```
Stock decrements each tick by the summed yield of all occupants. The `/occupants` crowd-split is the engine of anti-coordination: arriving with the herd dilutes your take, so "everyone follows" is self-punishing.

**SPEC-TIME GRADIENT CHECK (gate before coding the DP):** the double-floor (yield floor + stock-decrement floor) can flatten the depletion gradient into a trivial NO-GO at coarse discretization. Compute the per-tick yield curve across the full stock range for the chosen `(STOCK_MAX, richness_cap, occupants)`; confirm yield takes **≥4–5 distinct values** as a region drains (a live abandon-decision needs a gradient). Start `STOCK_MAX = 20`; raise to `50` and re-check DP tractability (§5) if it flattens.

**Ships** (`N`): `region: Option<u8>` (None = transit), `dest: u8`, `travel_ticks_remaining: u8`, `total_yield: u64`.

**Move cost = TIME ONLY.** A move costs `TRAVEL[i][j]: u8` ticks of zero yield (in transit cannot mine). **No fuel/energy in this cut** (a fuel dimension blows the exact DP and the world's fuel mechanic is unbuilt — deferred, §9). Tune `TRAVEL` so a move amortizes over ~2–4 ticks of foregone yield (the live-decision band: residence is a commitment, packs persist long enough to be packs).

## 4. Policy ladder (the four rungs)

All evaluated on **held-out eval seeds**; closed-form params fit on a disjoint train split, then **one fixed setting across all eval seeds**.

1. **Constant** — mine your start region until empty, never move. The do-nothing floor (denominator only; never the gate bar). Job: prove the substrate is responsive.
2. **Best-closed-form reactive (THE BAR)** — best-of-a-small-family anti-herding rule: "abandon when my realized per-ship yield drops below τ; move to the highest observed stock-per-occupant region." **Must be allowed to RANDOMIZE** (move-with-probability-`p(state)`): the symmetric equilibrium of this anti-coordination game is *mixed*, so a deterministic threshold self-herds and would be a strawman that manufactures fake room. Sees only **current observables** (current stocks + current crowd); denied all future/oracle info. **The gate compares against this — never uniform-random** (commission L26/L58).
3. **Per-seed-myopic** — greedy one-step-optimal on current observed state, no anticipation. Diagnostics: `(myopic − closed-form)` = value of richer reaction; `(ceiling − myopic)` = value of *anticipating* the herd/depletion trajectory.
4. **Omniscient ceiling** — §4.1.

### 4.1 Ceiling decision (Crux #1) — RESOLVED: single-agent closed-loop selfish best-response

The gate ceiling is **a single omniscient agent best-responding to a fixed population of best-closed-form (rung-2) others — NOT a coordinated social planner.** In an anti-coordination/commons game, planner ceiling = selfish room **+** coordination headroom (price-of-anarchy); the coordination term is the *large* term and is **un-capturable by a decentralized learner** (needs central command/communication) — gating on it = guaranteed **false GO**. The planner DP is computed and reported **only as a labelled upper bound** ("coordination headroom — NOT learnable"); `(planner − selfish)` is reported as an honesty guard.

Two mandatory corrections:
- **Closed-loop, not frozen-field.** The N−1 reactive others must **re-crowd in response** to the deviator (re-simulate the reactive field per candidate deviation). A frozen-field best-response is valid only at large N where the deviator is marginal; this cut is *forced small* (§5), so a frozen field lets the omniscient agent inherit residuals a reacting field would contest — that inherited value is coordination headroom in disguise and inflates the ceiling worse as N shrinks.
- **It is NOT "dramatically cheaper" than the planner DP.** The reactive others condition on the full joint state (all stocks + crowd), and stocks couple to everyone's mining, so the BR DP still carries all N positions + all M stocks. You drop the others' *action branching*, not the *state*.

**Phantom-ceiling cross-check (mandatory, probe-5 scar):** roll the computed closed-loop BR policy through a fresh simulation and assert realized total ≈ computed `V₀`. Without this, capture numbers are theater. (This cross-check IS the MC-estimator calibration, §5.)

## 5. The gate: an N-scaling curve (not a point) + fraction definition

**Fraction-of-ceiling (pre-registered single definition):**
```
frac(N) = (ceiling_closed_loop_BR(N) − best_closed_form(N)) / (ceiling_closed_loop_BR(N) − constant(N))
```
- Numerator = headroom a perfectly-(present+future)-informed unilateral deviator has over the deployable script.
- Denominator = total dynamic range above the do-nothing floor (offset/scale invariant).

**The gate is the SCALING CURVE** (the central hardening — defeats the "scale paradox", §8). A point estimate at N=3 is trustworthy about the *wrong* game (no herd at N=3). So:
- Run `frac(N)` at an **N-ladder at fixed N/M** (e.g. N = 3 → 6 → 12 → 24).
- **Exact closed-loop DP only at the smallest rungs**, used to **calibrate a Monte-Carlo best-response estimator** (the phantom-ceiling cross-check, §4.1, IS the calibration); carry **only the validated MC estimator** up the ladder where the DP is infeasible.
- **GO** iff `frac(N) ≥ 0.10` **AND** flat-or-rising in N.
- **NO-GO** iff `frac < 0.10` at the smallest exact rung, **OR** `frac` decays toward the gate as N rises (the LLN self-averaging signature — pre-registered as NO-GO).
- **MC confidence:** at the MC-carried rungs, `frac(N)` is an estimate — report a **confidence interval** (across eval seeds / rollouts). The gate is read against the CI, not a bare point: do not GO when the CI straddles 0.10; widen the rollout/seed count until the CI resolves the gate. (The exact-DP rungs are point-exact and need no CI.)

**Controls (apparatus fairness, run alongside):**
- **Negative control** — identical regions (correlation → 1): must NO-GO *by construction*. If it GOs, the apparatus is rigged-to-GO. Calibration only; excluded from the GO grid.
- **Positive control** — maximally diverse/independent regions × one-shot exhaustion: the cell most likely to show room. Read as "room exists at small N", then **checked for survival up the N-ladder** (at N=3 a "must-GO" positive control risks rigging-to-GO — the N-curve is the guard).

## 6. Instances

**Exact-small (trustworthy calibration rung):** `N=3, M=2` (purest binary here-vs-there) and `N=3, M=3`. `STOCK_MAX=20` pending the §3 gradient check, `TRAVEL ≤ 5`, horizon `H` sized to express ≥1–2 full deplete-and-relocate cycles (start ~`H=30`; too short understates the ceiling → false NO-GO). **Honest cost:** closed-loop BR joint state ≈ **54M** at N=3/M=3 — tractable in seconds-to-minutes via backward induction + a `u64` value per live state. `N=4/M=4` (~9.8B) is infeasible; beyond the small rungs the ladder runs on the MC estimator.

**Observe-large (qualitative pack emergence — NOT the gate):** `N=100–200, M=10–20, H≈5000`, under best-closed-form only. Diagnostics: (1) pack count `K(t)` + its autocorrelation time (the best real-packs-vs-flicker test); (2) spatial-entropy `H(t)` *oscillation* (concentrate↔disperse); (3) exodus/regroup events + lag between yield-crossing and exodus (tight positive lag = depletion-causal); (4) boom/bust period per region (regen regime); (5) residual-camper vs early-mover realized payoff (live-decision fingerprint). **Pre-registered: packs existing is NOT a GO** (`ecosystem-oscillation` had a screaming dynamical GO and zero learnable room). The verdict is the §5 number only.

## 7. Determinism plan

- **Integer-only state + transitions; no in-tick RNG.** One `RngStreams::from_master` draw at construction (`RngStream::Scenario`; both `pub` in `crates/jumpgate-core/src/rng.rs`, verified) sets richness_caps, initial ship assignments, travel matrix. The tick loop is then a pure function of (initial state, policy) — required for an exact deterministic DP transition and exact closed-loop re-sim.
- **Ships processed in index order** (0..N), never HashMap iteration. **Simultaneous update:** collect all ships' actions from tick-start state, then apply, then compute yields / decrement stock — never mutate stock mid-tick.
- **Three determinism cracks baked in as requirements:**
  1. New crate is **outside core's RNG-lint perimeter** → enforce the float/entropy ban with a **pinned golden trajectory-hash `#[test]`** (hash the `(tick, per-ship total_yield, per-region stock)` sequence for a fixed `(seed, policy, N, M, H)`), modeled on `golden_first_draws_are_pinned` (rng.rs:148).
  2. **Integer ties are frequent** (discretized stock → equal estimated yields) and the closed-loop BR value is **tie-break-sensitive** → pin a deterministic tie-break (lowest region index); document as replay-identity-bearing.
  3. The §3 gradient check is a spec-time gate, not a runtime assertion.

## 8. Biggest surviving risk: the scale paradox

The exact instance is forced to N=3 (DP-computable) *and* to defeat LLN — but at N=3 the herd-timing tension **cannot structurally occur**, and a single BR dodging two fixed agents manufactures residual room that evaporates at the N≫M scale where the arena lives (LLN self-averaging, killer of all six prior probes). A point estimate at N=3 can GO while the real arena NO-GOs. **Detection is built into the gate (§5): the N-scaling curve.** Room must be flat-or-rising in N; decay toward the gate is the pre-registered NO-GO. The exact DP's only role is calibrating the MC estimator (realized ≈ V₀), never gating off a single small point.

## 9. Deferred: the partial-observability / information-room bet (owner-gated, pre-registered)

Full observability is this cut's deliberate scope. If it NO-GOs, that is honestly scoped as "the *full-info* game is presolvable; this did not test the information game." The information-room bet — where the project's most plausible room lives (`tension-web` directive; 7a showed observability gating room 0% ↔ 34–49%) — is a **separate experiment the owner authorizes after seeing this result** (not an auto-trigger; that would usurp the PDR-0005 reversal trigger). Pre-registered so its traps are pre-disarmed: ceiling = **belief-state Bellman optimum (POMDP-optimal-on-observables)**; bar = **best memoryless/simple-reactive rule**; **oracle/true-state is a diagnostic upper bound only, never the denominator** (a crippled-estimator bar manufactures a GO proportional to how badly it's crippled). Hard precondition: size so the belief-DP is itself exactly solvable, else the estimator stand-in is an upper bound capable only of *refuting* room, never establishing it.

## 10. Module structure

New workspace crate **`crates/jumpgate-commons-cut`** (third member; clean add). Depends on `jumpgate-core` **only** for `RngStreams`/`RngStream`; imports no `World`/`CraftStore`/physics/economy. Modeled on the archived `oscillation_report`/`chronicle_report` diagnostic pattern.

- `lib.rs` — types (Region, Ship, ArenaConfig, ArenaState).
- `dynamics.rs` — integer tick (simultaneous update).
- `policies.rs` — the ladder (constant, randomizing closed-form family, myopic).
- `dp.rs` — backward-induction closed-loop single-agent BR ceiling + the labelled planner upper bound.
- `mc.rs` — the validated Monte-Carlo BR estimator for the N-ladder.
- `report.rs` — `#[ignore]` diagnostic entry points printing ladder + sweep + controls + pack diagnostics, **plus a machine-readable summary struct** so a harness can assert the verdict (not just `println!`).
- `rng_bridge.rs` — the `Scenario`-stream seeding.
- One **non-ignored** golden trajectory-hash test (§7).

## 11. Adopted defaults (owner may override at review)

- **Regen:** one-shot exhaustion for the exact-small gate (smaller DP state, cleanest forced migration); slow-regen for observe-large boom/bust. Sweep regen ∈ {0, slow, fast}.
- **N-ladder:** exact DP at N=3 (M=2 and M=3); MC-carried to 6 → 12 → 24 at fixed N/M. Owner sets the top.
- **Travel/spend:** time-only this cut; fuel-cost re-run only if the arena proceeds.

## 12. Testing strategy

- **Golden trajectory-hash test** (non-ignored) — the determinism control.
- **Phantom-ceiling cross-check** — realized rollout total ≈ DP `V₀` (also the MC calibration).
- **Negative control** must NO-GO by construction; **positive control** room must survive up the N-ladder.
- **Gradient check** (§3) at spec/impl time.
- **Ladder monotonicity assertion** — constant ≤ closed-form ≤ myopic ≤ closed-loop BR ≤ planner upper bound (sanity; a violation = a bug in a rung).
