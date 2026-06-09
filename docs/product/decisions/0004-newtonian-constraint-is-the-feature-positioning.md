# PDR-0004 — Positioning: the Newtonian constraint is the feature ("Space Crusader Kings," not "TIE Fighter")

Date: 2026-06-09   Status: accepted   Author: acting-PM (Claude)   Owner sign-off: yes (owner-stated 2026-06-09)
Supersedes: —   Related: vision.md (purpose/positioning), PDR-0002 (thesis venue), PDR-0003 (combat/travel model)

## Context

The owner's founding tension was "I want it to be realistic, but realism isn't
exciting." A session of worldbuilding (the theory-of-crime thread; PDR-0003 +
ecosystem-epic comments #3/#4) resolved it. The resolution is a positioning
statement worth making explicit, because it governs which design instincts to
accept and which to reject.

## The insight (owner, 2026-06-09)

"The same Newtonian physics that make a *TIE Fighter*-style game un-fun are exactly
what create the strategic decision space for *Space Crusader Kings*." Realism is
only un-fun in the **tactical** frame (twitch dogfighting is impossible at these
speeds — light-lag, g-loads; PDR-0003). In the **strategic/operational** frame, the
physics constraint is the *generator* of the fun: Δv budgets, velocity-matching
gates, conflict regions, reachability commitments. Corollary: **cheap omnidirectional
travel "sounds cool" but homogenizes space into something small and samey;
Δv-constrained Newtonian travel is what makes space feel big and gives geography
meaning.**

## Options considered

1. **Adopt "constraint is the feature" as positioning** — realism serves the
   strategic game; lean into Δv/geography/time as the design's core pleasures.
   Pro: resolves the founding tension; gives a sharp keep/cut test; matches the
   confirmed strategic/operational thesis (PDR-0002). Con: commits against a
   broad audience that wants arcade-style flight.
2. **Hybrid: realistic strategy + arcade tactical layer** — keep Newtonian strategy
   but bolt on forgiving twitch combat. Pro: wider appeal. Con: the two frames
   fight each other; arcade tactical contradicts PDR-0003's physics and dilutes the
   thesis; "homogenous small space" creep.
3. **Lean arcade (cheap travel, twitch combat)** — rejected: it is the "sounds cool,
   feels small" trap the owner explicitly named, and it kills the DRL-judgment venue.

## The call

Option 1. Positioning: **jumpgate is "Space Crusader Kings," not "TIE Fighter" —
the Newtonian constraint is a load-bearing feature, not a realism tax.** Δv cost,
travel time, velocity-matching, and conflict-region geography are the *sources* of
the strategic decision space, and cheap/omnidirectional travel is rejected because
it homogenizes space. This is the *why* behind the strategic/operational thesis
venue (PDR-0002) and the design test for future features.

## Rationale

It resolves the founding tension by relocating "fun" from the tactical to the
strategic frame, where realism is generative rather than punishing. It gives a crisp
keep/cut heuristic: a feature that makes space feel smaller/more homogenous (cheap
travel, teleport, frictionless logistics) is suspect; a feature that makes the
constraint legible and consequential (geography, Δv commitment, time, conflict
regions) serves the core. And it is consistent with everything confirmed this
session (PDR-0002/0003) rather than a new direction.

## Reversal trigger

Revisit if (a) playtesting/owner judgment finds the strategic frame does not, in
fact, generate enough fun to carry the game without an arcade tactical layer (would
reopen option 2); or (b) the Δv/geography constraints prove to make the world feel
*tediously* large rather than *meaningfully* large (tune the scale, don't abandon
the principle); or (c) the owner re-scopes the target audience toward arcade
play. Positioning is a vision-level statement — any reversal escalates per the
authority grant.
