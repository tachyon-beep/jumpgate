# PDR-0006 — Judge v1 as a game; retire the presolvability gate as a build prerequisite

Date: 2026-06-10   Status: accepted   Author: acting-PM (Claude)   Owner sign-off: yes (2026-06-10, directed)
Supersedes: PDR-0005's *gate-as-prerequisite* (the analytic-cut/fraction-of-ceiling discipline as a precondition for building); the rest of PDR-0005's situational analysis stands as history.
Refines: PDR-0002 (done-definition).   Related: charter.md, metrics.md, `vsl-cannot-host-judgment-principle`, `jumpgate-game-science-principle`, `jumpgate-hard-reset-3d-newtonian`, crates/jumpgate-commons-cut/RESULT.md

## Context

The DRL-room test as practiced — *"can a learner beat the exactly-computed optimum by ≥10% of ceiling?"* — is a **catch-22 for games**:

- Anything we can measure exactly is, by the project's own `vsl-cannot-host-judgment-principle` (tractable/replayable = presolvable), **"no room."**
- Anything with genuine room is, by that same principle, **not exactly measurable** that way.

So the frame can only ever return **NO-GO** or **"can't measure."** It is structurally incapable of green-lighting a buildable game — it **defines the game away**. Using it as a *prerequisite gate* guarantees no game ever gets built. This is the exact trap that killed the previous prototype (the deterministic-econ/DRL line, scrapped at the hard reset — `jumpgate-hard-reset-3d-newtonian`). The first arena increment, the commons-miner analytic cut (`crates/jumpgate-commons-cut/RESULT.md`), re-confirmed it: the cut could only produce a strawman-GO, a computation-trap NO-GO, or an unmeasurable result — never a game.

Meanwhile the owner has framed v1 as a **game** throughout (`tension-web-population-game`, `jumpgate-game-science-principle`, the foragers/pirates/police life-sim vision), and the project's one unambiguous success — `ecosystem-oscillation` — was **judged by play, with heuristic agents and zero RL.** That is the evidence the game frame works.

The error being corrected: treating a written ADR's gate as more authoritative than the owner's live, repeated framing, and inverting "game science" (rigorously *study* a fun game) into "gate the game on a science test."

## The call

1. **DROP** the presolvability gate (fraction-of-ceiling vs a computed optimum; the constant→best-closed-form→omniscient-DP analytic cut) **as a prerequisite for building.** No experiment must "pass a room gate" before the game is built. The `vsl` principle remains a true *observation about small replayable markets* — it is retired only as a **build gate**, never invoked again to forbid building a game.

2. **KEEP** the deterministic substrate + chronicle / diagnostics / sweeps / watch-the-system as the **reproducible lab for studying emergent dynamics.** *This* is "game science": rigorous, replayable study of the game's behaviour — not a turnstile in front of it. Determinism stays because it lets us study and reproduce a game, not because it gates one.

3. **REFRAME the done-definition (refines PDR-0002):** v1 is **judged as a game** — does it produce emergent, surprising, watchable, sustained play? — the way `ecosystem-oscillation` was judged. The measurable foundation is about the **game's own dynamics** (sustained cycles, pack formation/dispersal, predator-prey lag, the chronicle of individual lives), not about beating a lookup table.

4. **DRL is a *player*,** introduced where it makes agents interesting opponents/allies, and evaluated by **the quality of play it produces** — not by a fraction-of-ceiling differential against a presolvable optimum.

5. **Mechanics are game mechanics.** Information/Media (hidden richness, scouting, word-of-mouth, staleness), salvage/tugs, refuel/energy, pirates, police — these exist to make decisions rich and the world alive, **not** as "rooms" to be measured.

## Consequences

- The **charter done-definition** (a measurable strategic/operational DRL-vs-scripted differential) is superseded by the **game frame**; charter.md and metrics.md need updating to emergent-play / game-dynamics criteria. (Flagged for the owner; not auto-edited here.)
- The **scale/density arena issue** (`jumpgate-aec6e7bc14`) reframes from "presolvability-gated DRL arena (shaping + analytic cut)" to **"build the emergent game world (trophic life-sim) on the substrate."**
- The **commons-miner cut** (`jumpgate-commons-cut`) is retained as the artifact that *confirmed the gate frame is a dead end for game-building* — a useful negative result, not the thesis vehicle.
- The **information-room bet** is not pursued as a presolvability cut. Hidden-information/Media becomes a game mechanic added to make play richer; its value is judged by the play it produces.
- The historical `vsl-cannot-host-judgment-principle` memories stay as accurate observations about *small replayable markets*, re-tagged: they describe why a small market is boring, **not** a gate that forbids building the dense game.

## Reversal trigger

If "judged by play" proves too subjective to drive progress, re-introduce **lightweight emergent-dynamics metrics** that measure the *game's own properties* (sustained-cycle amplitude/period, pack autocorrelation, trophic balance, chronicle richness). **Never** re-introduce a presolvability / beat-the-computed-optimum gate as a prerequisite for building — that is the trap, by name.
