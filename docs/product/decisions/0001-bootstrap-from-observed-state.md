# PDR-0001 — Bootstrap the product workspace from observed state

Date: 2026-06-09   Status: accepted   Author: acting-PM (Claude)   Owner sign-off: pending
Supersedes: —   Related: vision.md, roadmap.md, metrics.md, current-state.md

## Context

The owner asked the agent to orient in the jumpgate repo and take over as product
manager. No `docs/product/` workspace existed, so there was no prior product state
to resume — `/own-product` ran in BOOTSTRAP mode and had to construct the five
artifacts from observed reality rather than a remembered history.

## Options considered

1. **Bootstrap from observed reality** (repo + specs/plans + filigree + project
   memory) — pro: honest, grounded in what is actually built and decided; con:
   purpose/audience are *inferred* from artifacts, not stated by the owner.
2. **Interrogate the owner first, write nothing yet** — pro: no inference; con:
   slower, ignores abundant written direction (a detailed design spec, 5 plans, a
   rich memory log), and the pack says read-before-ask.
3. **Do nothing durable, just chat** — rejected: loses the continuity property the
   whole pack exists for; ownership is stateful.

## The call

Bootstrap (option 1). Seed `vision.md`, `roadmap.md`, `metrics.md`,
`current-state.md`, and this `decisions/` record from the design spec, the
plan-of-record, the filigree tracker, and the project memory. Mark every inferred
claim _(inferred)_ and write the authority grant as **DRAFT — unconfirmed** pending
owner confirmation in this same session.

## Rationale

jumpgate carries unusually rich written direction (an approved core design spec
with an explicit thesis and anti-goals, a 5-part plan, a terminology glossary, and
a detailed memory log). Inferring from that is far more honest and faster than
interrogation, and the pack mandates read-before-ask. The risk — that an inference
is wrong — is contained by marking inferences and gating the authority grant on
explicit confirmation.

## Reversal trigger

Revisit when the owner confirms or corrects the vision and the authority grant. If
the owner's stated purpose/audience differs materially from the inferred text, supersede
the relevant sections (new PDR), do not edit this record. Also revisit if the
real plan-of-record diverges from `docs/superpowers/` (e.g. the filigree backlog
becomes the source of truth).
