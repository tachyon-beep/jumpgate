# PDR-0002 — The thesis venue is strategic/operational, not tactical

Date: 2026-06-09   Status: accepted (done-definition superseded by PDR-0006)   Author: acting-PM (Claude)   Owner sign-off: yes (2026-06-09)
Supersedes: —   Superseded-in-part-by: PDR-0006 (done-definition only)   Related: metrics.md (north-star), vision.md (purpose), roadmap.md, docs/superpowers/program/charter.md (done-definition)

> ## ⚠ DONE-DEFINITION SUPERSEDED BY PDR-0006 (game frame)
>
> **What survives:** the *venue* call. v1's fun is **strategic/operational** —
> long-horizon contract/route/fuel decisions across a multi-agent ecosystem, not
> tactical fly-by-stick. Owner-confirmed, and PDR-0006 keeps it ("the game lives
> where this PDR said it does"). This is still right; read the body for it.
>
> **What is RETIRED:** this PDR's *done-definition* — judging v1 by a **measurable
> strategic/operational DRL-vs-scripted differential**. PDR-0006 retires that as
> the success criterion because the "can a learner beat the computed optimum by a
> fraction-of-ceiling" frame is a catch-22 for games: anything measurable that way
> is "presolvable → no room," anything with real room isn't measurable that way —
> it can only ever return NO-GO or can't-measure, and so **defines the game away.**
>
> **What replaces it:** v1 is **judged as a game, by emergent play** — surprising,
> watchable, alive, sustained — the way `ecosystem-oscillation` (heuristic agents,
> zero RL, the project's one unambiguous success) was judged. The measurable
> foundation is the **game's own dynamics**: predator-prey cycle amplitude/period,
> pack formation/dispersal + autocorrelation, trophic balance, chronicle richness
> of individual lives — **not** beating a lookup table. Determinism + chronicle +
> diagnostics + sweeps are the **reproducible lab for studying** those dynamics
> ("game science"), **not a gate** in front of building the game. **DRL is a
> player** — added where it makes agents interesting opponents/allies, judged by
> the quality of play it produces. The single-craft navigator rung stays as the
> first trainable rung, not a thesis test.
>
> Below this banner is the original 2026-06-09 record, kept verbatim as history.

## Context

Bootstrapping the workspace surfaced a construct gap: the thesis is that DRL is
*more entertaining* than scripted AI, but the obvious v1 metric — beat the scripted
autopilot on the single-craft fuel-constrained A→B transfer — measures *tactical
performance* on a near-convex control problem. The autopilot is already
near-optimal there, so DRL can at best tie, and "entertaining" has near-zero
variance. The project's own archived findings (`vsl-cannot-host-judgment-principle`,
the contention-game / population-games line) concluded repeatedly that small
replayable single-agent tasks cannot host interesting behavior. A clean gym + a
trained navigator policy could therefore prove DRL *works* without proving the
actual bet. This was raised to the owner as the success-criterion question.

## Options considered

1. **Lock a tactical performance metric** ("beat autopilot on the navigator task")
   as the north-star — pro: measurable as soon as the gym exists; con: measures the
   wrong construct; an autopilot-tie is a hollow win; the owner says the game is not
   tactical.
2. **Strategic/operational venue: the multi-agent ecosystem** — DRL agents make
   long-horizon contract/route/fuel-commitment decisions under endogenous pricing +
   predation; "more entertaining/better" judged there — pro: matches the owner's
   stated fun ("strategic/operational, weeks/months of transit, not fly-by-stick")
   and the charter's vertical-slice; venue actually has decision variance; con: the
   venue is net-new (ecosystem layer not built; design harvestable from `archive/`,
   code dead) so the north-star is not measurable until that lands.
3. **Hold the north-star open** and run a separate entertainment-metric discovery —
   rejected as premature: the owner already named the construct (strategic/
   operational); the open work is the concrete metric, deferrable to shaping.

## The call

> **[RETIRED — PDR-0006]** The done-definition stated below (a *measurable
> DRL-vs-scripted differential* as the north-star) is superseded by the game frame:
> v1 is judged by emergent play and the game's own dynamics, not by a learner
> beating a scripted/computed baseline. The **venue** choice (Option 2,
> strategic/operational ecosystem) survives unchanged; only the *differential as
> success metric* is retired. Original text preserved below.

Option 2. The north-star is the **strategic/operational DRL-vs-scripted
differential inside the demand-driven multi-agent ecosystem**, not tactical control.
Owner confirmed directly (2026-06-09): "it's not a fly-by-stick game — almost all
transit will be via navigation over several weeks or months — the fun is
strategic/operational not tactical." The single-craft navigator A→B is reclassified
as the *first trainable rung* (a Plan-4 milestone proving an agent can exist and be
trained), explicitly **not** the thesis test. The concrete ecosystem metric (the
`TARGET`) is to be defined at vertical-slice shaping, with the owner.

## Rationale

The bet must be falsified at the layer where the fun lives, or "did it work" is
answering the wrong question. The owner, my independent gym-stub analysis, and the
parallel program charter's vertical-slice section all converge on the ecosystem as
that layer. Keeping the tactical metric would have set the scoreboard to reward a
result the owner doesn't care about — the build trap in metric form.

## Reversal trigger

> **[CLAUSE (c) RETIRED — PDR-0006]** The original trigger (c) — "revisit if a
> defined ecosystem metric proves unfalsifiable / presolvable = computation not
> judgment" — is the **named trap** PDR-0006 forbids reintroducing: it is the
> presolvability gate, which can only return NO-GO or can't-measure. It is retired;
> never re-cite the `vsl-cannot-host-judgment-principle` as a build gate. (That
> principle stays a true *observation* about why small replayable markets are
> boring.) Triggers (a) and (b) still stand. If a forward trigger is wanted, it is
> the PDR-0006 kind: re-introduce lightweight **emergent-dynamics** metrics
> (cycle amplitude/period, pack autocorrelation, trophic balance, chronicle
> richness) — properties of the *game itself*, never a beat-the-optimum gate.
> Original text preserved below.

Revisit if (a) vertical-slice shaping shows the ecosystem cannot be made trainable
at acceptable throughput (the env-steps/sec bottleneck), forcing a smaller venue;
or (b) the owner redefines where the fun lives; or (c) a defined ecosystem metric,
once instrumented, proves unfalsifiable (learned and scripted indistinguishable by
construction — the "presolvable = computation not judgment" failure the archived
line hit). On (c), redesign the metric/venue, do not lower the bar to claim a win.
