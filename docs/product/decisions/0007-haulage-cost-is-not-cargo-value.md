# PDR-0007 — Haulage cost is not cargo value

**Date:** 2026-06-12
**Status:** ACCEPTED — owner-directed
**Decider:** John (owner)
**Context:** Pre-execution shaping constraint for the World Gets Big / Layer-1 economy build.

---

## Decision

Maintain **two orthogonal price systems** throughout the economy build. Never conflate them.

1. **Commodity price** — local supply/demand for the good itself. Food is food; ore is ore. A unit of food can be scarce and expensive at a remote station (local shortage), but that scarcity does not make it intrinsically high-value cargo. Commodity price is set by the production/consumption balance at each station.

2. **Transport price** — what a hauler must be paid to accept a run: Δv cost, fuel availability en route, deadhead time, danger premium, opportunity cost. A remote or sparse route can carry an enormous transport premium regardless of what is being carried.

**The governing rule:**

> A route can be expensive without being lucrative.

---

## Rationale

Without this separation the economy collapses to "remote route pays more → remote cargo is valuable → pirates camp distant stations" — which is the current (pre–World Gets Big) behavior. That is a false economy. It prevents the correct emergent behavior from developing:

- Remote, sparse routes are **expensive to serve but poor piracy targets** — low cargo value, infrequent traffic, bad opportunity cost for pirates.
- High-throughput industrial corridors (fuel tankers, refined metals, weapons, payroll, ship components) are **pirate targets because of value × frequency density**, not geographic remoteness.
- Cops follow **crime concentration history** — they are reactive, positioned where past value flowed. Normally crime concentration and value density track together, so cops and pirates are co-located on industrial corridors.

---

## The "big score" mechanic

The interesting rare event is when the two price systems decouple:

1. High-value cargo takes an **unusual route** — the normal corridor is too hot, a regional shortage pulls cargo sideways, or a one-off contract diverts it.
2. That route has **no crime history** → no cops.
3. Pirates watching value signals (not camping a fixed location) see high-value cargo on a sparse, undefended line.
4. They score. Cops follow the new crime concentration with a lag. The window closes.

The big score is structurally rare because it requires two conditions that are normally anti-correlated: **high-value cargo AND absent law**. They decouple only through routing anomalies and information lag. This is an emergent property of the lag, not a scripted event.

---

## Implementation constraints (must hold at each layer)

| Mechanic | Constraint |
|---|---|
| Contract reward | Must equal **transport cost + margin**, not a proxy for cargo value. Reward scaling with distance alone is the conflation bug. |
| Cargo value field | A separate field on the cargo/shipment, read by pirates when targeting, independent of contract reward. |
| Pirate target selection | Must read **cargo value × expected frequency** on a corridor, not contract reward or route distance. |
| Police positioning | Must be driven by **crime concentration history** with a real temporal lag — not omnipresent, not clairvoyant. |
| Law/crime equilibrium | Cops and high-value corridors normally co-locate. Decoupling (the big-score window) happens through routing anomalies + lag, not by design fiat. |

---

## Reversal trigger

Revisit if playtesting shows the big-score window never opens in practice (law lag too short, routing too rigid) or opens so frequently it trivializes danger on all routes (lag too long, cargo value too homogeneous). Tuning is in the scenario config; the two-price-system architecture is not the knob.

---

## Cross-references

- PDR-0006 — judge v1 as a game; the big-score mechanic is the kind of emergent play the game is built to produce.
- PDR-0002 — fun lives at strategic/operational scale; the routing decision "take the cheaper safer route vs. the faster dangerous corridor" is exactly the strategic decision space this creates.
- `jumpgate-a494b1d700` — Layer-1 vertical-slice ecosystem epic; these constraints apply to its decomposed tasks.
