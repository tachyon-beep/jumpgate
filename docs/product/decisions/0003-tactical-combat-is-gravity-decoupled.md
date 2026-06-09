# PDR-0003 — Tactical combat is a separable, gravity-decoupled domain; travel is EVE/Elite-style macro-warp

Date: 2026-06-09   Status: accepted   Author: acting-PM (Claude)   Owner sign-off: yes (2026-06-09)
Supersedes: —   Related: PDR-0002 (thesis venue), vision.md (scope), roadmap.md, core design spec §3.2 (LOD), docs/superpowers/program/charter.md (land order)

## Context

PDR-0002 set the thesis venue as strategic/operational. Two scope questions
remained: does the game have tactical ship combat at all, and does combat's physics
couple to the heavy solar-system gravity/orbital simulation (which would be a
correctness + throughput hazard)? The owner clarified the travel and combat model
directly (2026-06-09).

## The clarification (owner, recorded)

- **Travel is EVE/Elite-style macro-warp.** Select a target, "go," travel until you
  "stop" — but Newtonian, so "stop" means *start stopping*, decelerating over days
  or months of sim-time. (This is precisely the destination macro-action + the
  Newtonian autopilot already built; reinforces the strategic/operational framing —
  almost all transit is navigation over long horizons, not stick-and-rudder.)
- **There WILL be tactical ship combat** — mostly drones, with bigger ships lobbing
  rounds at each other.
- **Tactical combat does NOT need the complex gravity/orbital calculations.** "The
  bigger space simulation won't impact the tactical one that much" — the two are
  separable.

## The call

Record tactical combat as a **real, in-the-long-term-scope domain that is
structurally decoupled from the macro gravity simulation**, resolved at a local
high-fidelity LOD with a local origin — exactly the seam the core design spec
already reserves (§3.2: `LodPlayer` / `LodNpcInteraction` run fine local physics in
a floating origin; high-g drones substep on *total acceleration*, not body
proximity). Tactical combat is therefore **additive on an existing seam — no new
structural debt**. It is **not v1** (charter scope defers combat/sensors/economy
domains; v1's "combat/piracy/law" is the *strategic* predation loop, abstractable),
and it is **not the primary thesis venue** — the DRL-beats-scripted bet is judged at
the strategic/operational layer (PDR-0002), not in tactical dogfights (though combat
agents may themselves be DRL later).

## Rationale

The clarification removes a latent fear (combat coupling to orbital physics) by
confirming it rides the LOD/local-origin seam already designed for "build for but
not with." It keeps the thesis venue clean (strategic/operational, not tactical) and
confirms the travel model the whole foundation assumes. Nothing needs to be built or
changed now; this prevents future scope drift in two directions — combat being
treated as gravity-coupled, or tactical combat being mistaken for the thesis test.

## Reversal trigger

Revisit if (a) tactical combat is later found to *require* gravity/orbital coupling
(would break the separability assumption and the LOD/local-origin seam — escalate as
a structural change), or (b) the owner decides tactical combat *is* a DRL-vs-scripted
thesis venue (would add a second north-star venue), or (c) combat is pulled into v1
scope (a charter scope change → program-management + owner sign-off).
