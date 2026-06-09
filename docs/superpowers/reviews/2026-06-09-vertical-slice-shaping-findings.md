# Vertical-Slice Shaping Pass — findings (2026-06-09)

Issue `jumpgate-818a04bb6b`. Adversarial shaping pass run as a 28-agent workflow (`wf_9a8eecdb-ccd`; raw harvest + judged candidates at `/tmp/.../tasks/wzdoxueio.output`, ~141k chars). Built around RAID R5 (site DRL where it has no room → thesis quietly fails). Harvest schemaless after the v1 run's nested-schema harvest agents all returned null. PM: Claude. Owner: John.

## Headline — no DRL room *demonstrated* in the thin slice; 3 arenas provably dead, 3 unmeasured-and-doubtful

**Honest scoring (corrected): 3 of 6 candidates are *provably presolvable* (closed-form → genuinely DEAD); the other 3 are "not structurally disproven, predicted sub-gate, but UNMEASURED."** This *reconfirms* the strong prior (the six real probes in `archive/solution-architecture/25`, all NO-GO by the same mechanism) — it is **not an independent seventh probe**: the workflow read those NO-GO conclusions as input and was primed "the default prior is no room," so it summarized the prior, it did not re-derive it.

> **Methodology caveat (do not over-read the run).** The workflow's `survivors_count` came back `0`, but that figure is a near-rigged artifact: the survival test required *zero* refutations against two skeptics each *instructed to default-refute* — survival was almost impossible by construction. The synthesizer's own `surviving_arenas` (3 carried forward as "not-disproven") is the more honest cut, and is what this doc uses. The genuine signal lives in the **ceiling phase, not the refute phase**: the 3 DEAD verdicts are closed-form-provable and trustworthy; the 3 "LOW/MEDIUM, unmeasured" are doubtful-by-precedent but not empirically settled here. The only thing that would be *independent* evidence is the cheap analytic cut on the real substrate (decision B) — which is cheap, and is the right next experiment.

| Arena | Ceiling | Presolvable | Room source | R6 | Verdict |
|---|---|---|---|---|---|
| Commons miner (depletion-aware siting) | LOW (~single-digit %) | no | non-stationarity (herd starves deposit) | MARGINAL | MARGINAL — buildable near loop, predicted sub-gate |
| Deflation-in-the-mail (dest choice vs hidden in-transit freight) | LOW (~4–6%) | no | sequential commitment under hidden info | INFEASIBLE (needs unbuilt LOD Wake) | predicted sub-gate |
| Navy vs co-adapting pirates | MEDIUM-conditional | no* | two-population non-stationarity | MARGINAL | highest ceiling, maximally unreachable |
| Pirate lie-low-vs-strike (heat) | LOW | **yes** | — | FEASIBLE | DEAD (closed-form bang-bang threshold) |
| Commit-timing vs boom/bust cycle | ~ZERO | **yes** | none | FEASIBLE | DEAD (haul 3–5 ticks vs ~1000-tick cycle) |
| Fuel-reserve sizing vs predation oscillation | LOW | **yes** | — | INFEASIBLE | DEAD (closed-form newsvendor/Kelly quantile) |

\* presolvable only *if* you don't build the co-learning + fat-tail machinery; absent that it flips to the interdiction NO-GO (~0.4%).

**Buildability is anti-correlated with room.** The arenas closest to the first loop are the most presolvable (contention-game probe-5 re-skins); the only non-LOW ceiling needs the apex trophic stack (deferred out of the slice) *and* two-population co-training the project has never converged.

## The reframe — two senses of "room" the project had conflated

- **DRL-room** — can a learner beat the *best fixed script* by a measurable fraction-of-ceiling (>10%, telemetry-ablated, vs best-closed-form, never vs uniform-random)? **Every probe NO-GO'd.** The team already *retired* this tribunal at the interdiction pivot (2026-06-07): in a small replayable toy even a Kalman filter is computation, so "judgment ≠ computation" is unsatisfiable by construction. **Replay-determinism IS presolvability.**
- **Dynamical/game-room** — does the system produce sustained, non-degenerate emergence worth watching? The *one* GO (`ecosystem-oscillation`) lives here — judged **by play, with heuristic agents and no RL**, never a measured fraction-of-ceiling differential.

The spine of the whole graveyard: **self-cancellation kills (LLN self-averaging, risk self-equalization, clamped/closed-form markets); a coupled loop held off its attractor makes (food-driven predator-prey → sustained disequilibrium the optimum can't track).**

**Live tension this exposes (owner-level):** the charter done-definition is written in the *measurable-better-decisions* frame (DRL-room — structurally dead in a small market). The product north-star is *more entertaining than scripted* (a game-room / play-judged question — which the one GO actually supports). These are different theses with different success criteria. Picking the frame is the real fork.

## What IS clean — the first loop as a deterministic harness (charter-locked, additive)

Verified additive onto the live physics core (zero economy code exists in `jumpgate-core` today). Two stages on one set of primitives, landing as data-defined column tables + systems, not new entity types:

- **Stage 1 (scripted, fixed price):** producers fire recipes → stock columns rise → corporations post contracts at a constant price → scripted haulers accept (lowest-StationId/nearest) → route via the **live co-orbiting rendezvous Arrival** → escrow settles, conservation holds. *Hosts no skill by design* (the C+ finding: a blind constant ties the "strategic" policy) — it proves the loop conserves and replays.
- **Stage 2 (demand-deflation):** close the homeostatic loop production→stock→**price=f(stock)**→margin→posting→haul→delivery→stock. Load-bearing work = closing the loop the archive left open at both ends: a deterministic Wait/tick-gated `update_prices` clock invoked from the slice step path (the archived open-loop bug: `step_slice` never called it), NPC demand that moves price not just stock, plus a **hysteresis deadband + staggered dispatch** to prevent limit-cycle oscillation. Still a tractable few-actor presolvable surface — *do not expect DRL room here either.*

Primitives (each on a real seam): **Producer** (recipe column-table + flat edge table = the commodity graph as relational data), **Station market** (per-(station,resource) stock+price columns), **Corporation** (non-spatial funded registry, `treasury_micros:i64`), **Delivery Contract** (status-enum lifecycle + escrow, `delta==0`), **Hauler role + cargo manifest** (cargo MASS folds into `EffectiveMods`, never an `effective_params` signature change), **Demand sink** (a consumer recipe — what makes stock fall and price move).

## Foundational integration (panel inputs fold in on live seams)

- **Authority regime** (dt=0.25, thrust ~3× gravity): autopilot law is dt-independent → decision-cadence (coarse routes) separates from integration-cadence (fine autopilot) for free; all tuning in `env.rs config_template` is golden-clean (committed goldens are tick-0).
- **Co-orbiting rendezvous arrival**: **already live + verified** — the same fix that makes the navigator learnable IS the economy's moving-station docking model. No new machinery.
- **`ARRIVAL_RADIUS` → config field** (currently `pub const 1e-4` in autopilot.rs): re-pins tick-0 config golden `0x278c`. **Seam-change #1 — budget up front.**
- **LOD Task-12 Wake/dormancy hook**: UNBUILT (`world.rs:261-264` skip-only stub; Wake `EventKind` pinned, emitter deferred). It's the R6 throughput lever — but **dormancy fights its own room source**: every arena's room is concurrent contention, which you can't LOD-sleep. So dormancy buys throughput for the first loop (idle/in-transit craft), NOT for the contention that would create DRL room.
- Economy mutable state (credits/cargo/stock/treasury/prices) forces a `HASH_FORMAT_VERSION` bump → **one** bump landing all economy columns together (not three), one golden re-derivation.

## Caveat — fabrication discount (standing memory: workflow agents fabricate in-code claims)

The qualitative **no-DRL-room conclusion is robust** (convergent with 6 real prior probes + the doc-25 principle). But the specific **ceiling numbers and some archive line-cites are workflow-generated and partly unverified** — one refutation cited `economy.rs:1491` in a 286-line file (past EOF). Before any GO/NO-GO is *trusted*, the cheap analytic cut must **re-measure on the real substrate** (oscillation period, the 0.18 price slope, realized stock-differentials), not lean on archived recall. The first-loop substrate claims (rendezvous live, Lod skip-stub, ARRIVAL_RADIUS const, Wake unbuilt) are consistent with known code but warrant a reality-check at planning time.

## Owner decisions (the output of this pass — NOT yet a spec)

**[1] PRIMARY — does v1 build a DRL arena at all inside the small economy?** Synthesizer recommendation: **A + B**.
- **A:** Ship the scripted + demand-pricing loop as the deterministic correctness/replay **harness only**; treat DRL-room as a separate bet on the scale/co-learning path (where doc 25 says room lives). Do NOT claim a DRL win inside the small tractable economy.
- **B:** Gate the first DRL bet behind a **cheap analytic pre-cut** on the commons miner, on the real substrate — constant → best-closed-form anti-herding → per-seed-myopic → omniscient-DP, held-out seeds, sweeping regen-rate × field-correlation. If best-closed-form→omniscient < 10%, **NO-GO before a single learner is built.** Honor it.
- **C (rejected for v1):** skip to the apex navy arena — maximally unreachable.

**PM weighting — A is the strategy, B is a cheap expected-fail sanity check.** The substantive direction the prior work already reached (`vsl-cannot-host-judgment-principle`, `drl-3d-geography-direction`: judgment lives on the **scale / density / population** path, not in a small replayable market) is what this pass reconfirms. So: ship the loop as a deterministic harness (A), and **reposition the DRL bet to the density/scale path** as the real thesis vehicle. Run B (the cheap analytic cut on the commons miner) because it is cheap, independent, and dissolves the fabrication caveat by re-measuring on live code — but expect NO-GO; it is a sanity check, not the plan.

**[2] IF a DRL arena is pursued, which arena+metric first?** → **Commons miner** (the only survivor buildable adjacent to the loop; its cheap cut needs no learner and no unbuilt Wake hook). Pre-register the gate; expect NO-GO by precedent, but a fast substrate-honest NO-GO is the correct next experiment.

**[3] Hash/golden seam budget** → ARRIVAL_RADIUS→config re-pin (`0x278c`) up front; **one** `HASH_FORMAT_VERSION` bump for all economy columns together.

**Open thesis-frame question (PM-added, above the synthesizer's [1]):** is v1's thesis the *measurable-better-decisions* frame (DRL-room — the graveyard says dead in a small market) or the *more-entertaining-emergent-play* frame (game-room)? This may reframe the done-definition itself (charter / PDR-0002 / metrics.md), and is the owner's call. **But the game-room frame is not a safe rescue:** the *only* game-room exemplar (`ecosystem-oscillation`) used **heuristic agents and zero RL**. So "does DRL beat *good heuristics* at being entertaining in an oscillating system" is itself unmeasured and may be as hard to demonstrate as the decision differential — switching frames trades one hard, unproven problem for another, not for an easy win. The owner should choose the frame knowing both carry an unproven burden.
