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

The single-craft navigator A→B is reclassified as the **first trainable rung** (a Plan-4 milestone proving an agent can exist + be trained), not the thesis test. **Still open:** the concrete falsifiable ecosystem metric (the `TARGET`) — define it with the owner at vertical-slice shaping. Product north-star: `docs/product/metrics.md`. This ties "done" to the thesis, not to features shipped.

## Scope
- **In (v1):** deterministic core (✅ Plans 0–3), guidance params (✅), Person+Ship foundation (Plan A → B → C), the gym surface (Plan-4 / Tasks 16–18), single stepped LOD tier with the enum+dispatch seam shaped.
- **Out / deferred (foreclose-nothing, implement-trivially):** craft–craft force perturbation; multi-tier LOD bodies (only one tier implemented); wear/component/heat modifier layers (the `EffectiveMods` bundle reserves them); non-autopilot Person brains; combat/sensors/economy domains; barycentric two-body wobble.
- **North star (NOT v1):** multi-domain high-fidelity sim (economy/combat/exploration/industry); crew-on-a-flight-deck is the depth yardstick.

## Land order (sequencing) — CONFIRMED (owner, 2026-06-09)
```
Plan A (the one irreversible seam — FIRST regardless)
  ├─ Plan-4 gym + navigator "first trainable rung"   (A-gated; proves the gym works; can run early)
  └─ vertical-slice shaping pass (harvest archive)    (defines the Layer-1 backlog + the falsifiable metric)
→ Layer-1 economy build:
     mechanical loop (scripted, FIXED price) → demand-driven pricing
     → ecosystem obs/action surface → swap scripted→DRL & MEASURE   ← the thesis test
→ then THICKEN (all additive): crews (Person B/C) → engines/wear → combat/piracy/law trophic level
```

**Rationale (debt-avoidance + thesis-risk, reconciled).** Plan A is the only non-additive seam — first regardless; the economy actors (mining yield, hauler capacity, weapons) are exactly what `EffectiveMods` will carry. After it, the work that *retires the thesis risk* — the demand-driven economy where DRL actually has room (PDR-0002 / RAID R5) — precedes crew fidelity. **Person B/C are additive enrichment that land AFTER the first loop closes, not before hauling** (owner's bottom-up order, with crews reordered after the loop). Every thin primitive sits on the real seams (additive SoA / `EffectiveMods` / event+contract surface), so thin-first incurs **no rip-and-replace debt**. The shaping pass decomposes the Layer-1 epic into concrete issues and sets the metric `TARGET`.

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
