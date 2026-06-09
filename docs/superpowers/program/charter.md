# Jumpgate v1 — Delivery Charter

*One-page program charter. Owner: John. PM: Claude (program-management). Created 2026-06-09. Living doc — revise on decision, not on cadence.*

## Outcome (the game)
Build a **demand-driven multi-agent game world that is alive — surprising, watchable, fun — judged by its emergent play**, the way `ecosystem-oscillation` (the project's one unambiguous success) was judged: by watching it, with heuristic agents and zero RL. The 3D Newtonian physics core is the substrate the world lives on. **DRL is a *player*** introduced where it makes agents more interesting opponents/allies, evaluated by the quality of play it produces — not a thesis to be proven. (Hard reset 2026-06-08; prior econ/DRL line archived. Game frame restored per PDR-0006, retiring the presolvability gate that twice poisoned this project.)

## The v1 vertical slice (the game world — owner, 2026-06-09)
The game is a **demand-driven multi-agent ecosystem**, built thin-loop-first (close each loop once → then "more stuff" is additive content):
1. **Economic loop:** miners mine → refine → sell **fuel**; **haulers** move goods station→station under **delivery contracts** and earn a reward for the job.
2. **Demand-driven pricing:** job/route prices **deflate when many agents are willing to do that work** (supply of labour on a route drives the reward down) — an endogenous market, not fixed payouts.
3. **Combat / piracy / law-and-order loop:** predation on haulers, and a law/deterrence response, closing the third trophic level.

Agents (miner / hauler / pirate / law) are the actors — heuristic first, with DRL added as a player where it makes them more interesting. The world is alive when these agents produce surprising, watchable behaviour: predators and prey, scarcity and feast, individual lives worth narrating.

**Layered model.** *Layer 0 — foundation* (in progress): deterministic 3D core ✅, guidance ✅, Person/Ship, the gym surface (Plan-4). Makes agents exist + be trainable. *Layer 1 — vertical-slice ecosystem* (this section, NET-NEW on the substrate): stations + goods + recipes + market + contracts + combat/piracy/law.

**Archive provenance — design reusable, code not.** This ecosystem was largely *designed* on the scrapped line: `archive/solution-architecture/{19,20,21,22,23,24,25}.md`, ADR-0006/0008, and epic plans (recipe primitive, first industry, rival haulers). The hecs-ECS code is dead, but the design + the **dynamics lessons** are gold — see RAID R5. **Harvest before re-deriving.**

## v1 done-definition — *a good game, judged by emergent play (owner; PDR-0006, 2026-06-10)*
> **v1 is DONE when the game world is alive** — when watching it produces emergent, surprising, sustained play, the way `ecosystem-oscillation` was judged a success (by watching it, heuristic agents, zero RL). "Done" is tied to the **game's own dynamics**, not to beating any computed optimum:
> - **Sustained predator-prey cycles** (measurable amplitude/period) — boom/bust, peace↔feast, scarcity→price spike.
> - **Pack formation and dispersal** (autocorrelation / spatial clustering of pirates around traffic, of haulers around safe routes).
> - **Trophic balance** across miners → haulers → pirates → law that holds without collapsing or flat-lining.
> - **Chronicle richness** — individual lives worth narrating (a notorious pirate lying low, a hauler running a risky lane for a fat contract).
>
> The autopilot/navigator A→B task is the **first trainable rung** — a Plan-4 milestone proving an agent can exist and be trained — not a test the game must pass.

**Game science, not a gate.** Determinism + chronicle + diagnostics + sweeps is the **reproducible lab for studying these emergent dynamics rigorously** — it is how we *study* a fun game (replay a surprising run, sweep a parameter, measure a cycle), NOT a turnstile a build must clear first. Product north-star: `docs/product/metrics.md`.

> **RETIRED per PDR-0006 (owner, 2026-06-10):** the prior done-definition — *"learned DRL agents make a measurable strategic/operational differential vs scripted agents," graded by a fraction-of-ceiling / beat-the-computed-optimum analytic cut* — is **gone, not softened.** That frame is a catch-22 for games: anything exactly measurable is "presolvable → no room," anything with real room is "unmeasurable," so it can only return NO-GO or can't-measure — it defines the game away, and it twice poisoned this project. The empirical records stand as history: the shaping pass (`docs/superpowers/reviews/2026-06-09-vertical-slice-shaping-findings.md`) and the commons-miner cut (`crates/jumpgate-commons-cut/RESULT.md`) are **real, confirmed negative results** showing the gate is a dead end for game-building. The `vsl-cannot-host-judgment-principle` remains a **true observation about why a small replayable market is boring** — KEPT as that, RETIRED as a build gate; it is never again cited to forbid building the game.

## Scope
- **In (v1):** deterministic core (✅ Plans 0–3), guidance params (✅), Person+Ship foundation (Plan A → B → C), the gym surface (Plan-4 / Tasks 16–18), single stepped LOD tier with the enum+dispatch seam shaped.
- **Out / deferred (foreclose-nothing, implement-trivially):** craft–craft force perturbation; multi-tier LOD bodies (only one tier implemented); wear/component/heat modifier layers (the `EffectiveMods` bundle reserves them); non-autopilot Person brains; combat/sensors/economy domains; barycentric two-body wobble.
- **North star (NOT v1):** multi-domain high-fidelity sim (economy/combat/exploration/industry); crew-on-a-flight-deck is the depth yardstick.

## Land order (sequencing) — UPDATED per PDR-0006 (owner, 2026-06-10)
```
Plan A (the one irreversible seam — FIRST regardless)                      ✅ LANDED
  ├─ Plan-4 gym + navigator "first trainable rung"   (A-gated)            ✅ LANDED
  └─ vertical-slice shaping pass (harvest + arena exploration)            ✅ DONE
→ FIRST LOOP, on the deterministic substrate (charter-locked, additive):
     Stage 1 mechanical loop (producers→stock→contracts→haulers→arrival, FIXED price)
     → Stage 2 demand-deflation pricing (close the reprice loop + hysteresis/staggered dispatch)
     [seam budget: ARRIVAL_RADIUS→config re-pin 0x278c; ONE HASH_FORMAT_VERSION bump for all economy columns]
→ BUILD THE EMERGENT GAME WORLD (the trophic life-sim — where the game comes alive):
     add the scale/density/population, predators (pirates) + prey (haulers) + law,
     then WATCH it → tune for emergent play (sustained cycles, packs, living chronicle).
     Add DRL as a player here, where it makes agents more interesting; judge it by the play it produces.
     Determinism + diagnostics + sweeps = the lab for studying these dynamics (NOT a gate).
→ then THICKEN (all additive): crews (Person B/C) → engines/wear → combat/piracy/law trophic level
```

**Rationale.** Plan A (the only non-additive seam) and the gym rung both landed. Build the economic loop on the deterministic substrate (it is needed regardless and sits entirely on additive seams + the already-live co-orbiting rendezvous arrival), then **build the dense/population game world on top of it and tune it until it is alive** — judged by watching the emergent play, the way `ecosystem-oscillation` was. There is **no analytic-cut / room gate** in front of building: PDR-0006 retired that frame (it is a catch-22 that only ever blocks the game). **Person B/C remain additive enrichment after the loop.** No rip-and-replace debt: every primitive is additive SoA / `EffectiveMods` / event+contract. The Layer-1 epic decomposes into (a) the harness loop, (b) building the emergent game world.

## Cutover gate — *OPEN QUESTION, CONFIRM WITH OWNER*
Everything lives on `jumpgate-v1-design`; `main` = last-stable. Is the long-lived WIP branch deliberate, and **what gate cuts v1 over to `main`?** (e.g. "the game world is alive — sustained cycles + living chronicle on watching it" = the done-definition above.) Currently undefined.

## Cadence & quality discipline (already strong — keep)
Per-task: spec-review → quality-review → **independent main-loop re-verification of every gate** (subagents fabricate gate claims — always re-run `cargo test` / `clippy --all-targets` / grep the goldens yourself). Single-cause golden discipline: one moved golden = exactly one named reason; never batch re-baselines.

## Tracked backlog (filigree — the live source of "what's next")
| Issue | Item | Status |
|---|---|---|
| `jumpgate-d30fcebaac` (P1) | Person Plan A — EffectiveMods seam (the one irreversible seam) | **ready — first build** |
| `jumpgate-818a04bb6b` (P1) | Vertical-slice shaping pass (harvest archive → loop + metric) | **ready — next design** |
| `jumpgate-5a3e01ab08` (P1) | Plan-4 — gym + navigator first-trainable-rung | blocked by A |
| `jumpgate-a494b1d700` (P2) | Layer-1 vertical-slice ecosystem → build the emergent game world (trophic life-sim) | blocked by loop |
| `jumpgate-12f37a8d74` (P3) | Person Plan B — crew effect (additive enrichment) | blocked by A |
| `jumpgate-205fd66b25` (P3) | Person Plan C — reserves + fixtures | blocked by B |
| `123b9f4856` / `1ec57e1002` / `c3c85a5da0` (P4) | deferred Class-1 tunables | ready, backlog |

RAID: `docs/superpowers/program/raid.md`.
