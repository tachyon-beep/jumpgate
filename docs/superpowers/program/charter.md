# Jumpgate v1 — Delivery Charter

*One-page program charter. Owner: John. PM: Claude (program-management). Created 2026-06-09. Living doc — revise on decision, not on cadence.*

## Outcome (the thesis)
Demonstrate that **DRL-controlled agents are usable in a game context and more entertaining than scripted/FSM AI.** The DRL/gym surface is the **first-class deliverable**; the 3D Newtonian physics core is the **substrate that shows it off**. (Hard reset 2026-06-08; prior econ/DRL line archived.)

## The v1 vertical slice (the concrete thesis vehicle — owner, 2026-06-09)
The thesis is proven inside a **demand-driven multi-agent ecosystem**, built thin-loop-first (close each loop once → then "more stuff" is additive content):
1. **Economic loop:** miners mine → refine → sell **fuel**; **haulers** move goods station→station under **delivery contracts** and earn a reward for the job.
2. **Demand-driven pricing:** job/route prices **deflate when many agents are willing to do that work** (supply of labour on a route drives the reward down) — an endogenous market, not fixed payouts.
3. **Combat / piracy / law-and-order loop:** predation on haulers, and a law/deterrence response, closing the third trophic level.

The DRL agents ARE the actors (miner / hauler / pirate / law). "DRL > scripted, and entertaining" is demonstrated by these agents producing richer behaviour than scripts inside this ecosystem.

**Layered model.** *Layer 0 — foundation* (in progress): deterministic 3D core ✅, guidance ✅, Person/Ship, the gym surface (Plan-4). Makes agents exist + be trainable. *Layer 1 — vertical-slice ecosystem* (this section, NET-NEW on the substrate): stations + goods + recipes + market + contracts + combat/piracy/law.

**Archive provenance — design reusable, code not.** This ecosystem was largely *designed* on the scrapped line: `archive/solution-architecture/{19,20,21,22,23,24,25}.md`, ADR-0006/0008, and epic plans (recipe primitive, first industry, rival haulers). The hecs-ECS code is dead, but the design + the **dynamics lessons** are gold — see RAID R5. **Harvest before re-deriving.**

## v1 done-definition — *VENUE CONFIRMED (owner, 2026-06-09); concrete metric TBD at shaping*
> **Direction CONFIRMED** (owner 2026-06-09; product PDR-0002, `docs/product/decisions/0002-thesis-venue-is-strategic-operational.md`): the thesis is **strategic/operational, not tactical** — "not a fly-by-stick game; almost all transit is navigation over weeks/months." So v1's done-definition is **NOT** "beat the autopilot on the single-craft A→B transfer" (that tactical task is near-convex — DRL can only tie, and "entertaining" has near-zero variance there). It is: **learned DRL agents (miner/hauler/pirate/law) make measurably better long-horizon decisions — contract/route/fuel-commitment under endogenous deflationary pricing + predation risk — than scripted-heuristic agents, inside the demand-driven multi-agent ecosystem, seed-reproducibly on the same build.**

The single-craft navigator A→B is reclassified as the **first trainable rung** (a Plan-4 milestone proving an agent can exist + be trained), not the thesis test. Product north-star: `docs/product/metrics.md`. This ties "done" to the thesis, not to features shipped.

> **REFINED by shaping pass + PDR-0005 (owner, 2026-06-09).** The done-definition *construct* stands (measurable strategic/operational DRL-vs-scripted differential), but its **arena moves off the thin market**. The shaping pass (`docs/superpowers/reviews/2026-06-09-vertical-slice-shaping-findings.md`) found no DRL room is demonstrable inside the small tractable economy — 3 of 6 candidate arenas provably presolvable, 3 LOW/MEDIUM + unmeasured; *buildability anti-correlated with room* — reconfirming the six prior probes (`vsl-cannot-host-judgment-principle`: replay-determinism IS presolvability). So the DRL thesis is **repositioned to the scale/density/population path** (PDR-0005). The first economic loop is built as a **deterministic harness**, explicitly NOT the DRL win; the dense/population arena (the real thesis vehicle) is a *later* shaping pass on top of that substrate, gated by a cheap analytic cut. The concrete falsifiable `TARGET` is set when that arena is designed — NOT at this pass.

## Scope
- **In (v1):** deterministic core (✅ Plans 0–3), guidance params (✅), Person+Ship foundation (Plan A → B → C), the gym surface (Plan-4 / Tasks 16–18), single stepped LOD tier with the enum+dispatch seam shaped.
- **Out / deferred (foreclose-nothing, implement-trivially):** craft–craft force perturbation; multi-tier LOD bodies (only one tier implemented); wear/component/heat modifier layers (the `EffectiveMods` bundle reserves them); non-autopilot Person brains; combat/sensors/economy domains; barycentric two-body wobble.
- **North star (NOT v1):** multi-domain high-fidelity sim (economy/combat/exploration/industry); crew-on-a-flight-deck is the depth yardstick.

## Land order (sequencing) — UPDATED post-shaping (owner, 2026-06-09; PDR-0005)
```
Plan A (the one irreversible seam — FIRST regardless)                      ✅ LANDED
  ├─ Plan-4 gym + navigator "first trainable rung"   (A-gated)            ✅ LANDED
  └─ vertical-slice shaping pass (harvest + adversarial arena grading)    ✅ DONE → PDR-0005
→ FIRST LOOP AS A DETERMINISTIC HARNESS (charter-locked, additive, NOT the DRL win):
     Stage 1 mechanical loop (producers→stock→contracts→haulers→arrival, FIXED price)
     → Stage 2 demand-deflation pricing (close the reprice loop + hysteresis/staggered dispatch)
     [seam budget: ARRIVAL_RADIUS→config re-pin 0x278c; ONE HASH_FORMAT_VERSION bump for all economy columns]
→ SCALE/DENSITY DRL ARENA (the real thesis vehicle — a LATER shaping pass on top of the harness):
     design the dense/population arena → cheap analytic cut (expected-fail gate) → IF clears, train & MEASURE
→ then THICKEN (all additive): crews (Person B/C) → engines/wear → combat/piracy/law trophic level
```

**Rationale (post-shaping).** Plan A (the only non-additive seam) and the gym rung both landed. The shaping pass (PDR-0005) established that the DRL thesis cannot be proven inside the thin market, so the sequence now **separates the harness from the thesis vehicle**: build the economic loop as a deterministic correctness/replay substrate (it is needed regardless, sits entirely on additive seams + the already-live co-orbiting rendezvous arrival, and is where the cheap analytic cut runs), then design the dense/population arena where DRL actually has room as a *separate* pass. **Person B/C remain additive enrichment after the loop.** No rip-and-replace debt: every primitive is additive SoA / `EffectiveMods` / event+contract. The Layer-1 epic decomposes into (a) the harness loop, (b) the scale/density arena shaping.

## Cutover gate — *OPEN QUESTION, CONFIRM WITH OWNER*
Everything lives on `jumpgate-v1-design`; `main` = last-stable. Is the long-lived WIP branch deliberate, and **what gate cuts v1 over to `main`?** (e.g. "Plan-4 green + a trained agent beats baseline" = the done-definition above.) Currently undefined.

## Cadence & quality discipline (already strong — keep)
Per-task: spec-review → quality-review → **independent main-loop re-verification of every gate** (subagents fabricate gate claims — always re-run `cargo test` / `clippy --all-targets` / grep the goldens yourself). Single-cause golden discipline: one moved golden = exactly one named reason; never batch re-baselines.

## Tracked backlog (filigree — the live source of "what's next")
| Issue | Item | Status |
|---|---|---|
| `jumpgate-d30fcebaac` (P1) | Person Plan A — EffectiveMods seam (the one irreversible seam) | **ready — first build** |
| `jumpgate-818a04bb6b` (P1) | Vertical-slice shaping pass (harvest archive → loop + metric) | **ready — next design** |
| `jumpgate-5a3e01ab08` (P1) | Plan-4 — gym + navigator first-trainable-rung | blocked by A |
| `jumpgate-a494b1d700` (P2) | Layer-1 vertical-slice ecosystem (decomposed at shaping) | blocked by shaping |
| `jumpgate-12f37a8d74` (P3) | Person Plan B — crew effect (additive enrichment) | blocked by A |
| `jumpgate-205fd66b25` (P3) | Person Plan C — reserves + fixtures | blocked by B |
| `123b9f4856` / `1ec57e1002` / `c3c85a5da0` (P4) | deferred Class-1 tunables | ready, backlog |

RAID: `docs/superpowers/program/raid.md`.
