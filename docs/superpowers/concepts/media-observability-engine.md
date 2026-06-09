# Concept note — Media (the observability engine)

**Status:** CONCEPT CAPTURE ONLY (2026-06-10) — deliberately NOT a full spec. A few
details so the idea isn't lost. Owner-sketched. Gets its own brainstorm → spec → plan
cycle later, when the information-room bet is authorized.

**Where it fits:** Media is the answer to "how does partial information work." It is the
substrate the deferred *information-room* bet needs — see the commons-miner cut spec §9
(`2026-06-10-commons-miner-cut-design.md`). It is NOT part of the first full-info cut.

## What it is

A **subsystem, not an in-game entity**: the single authority for *"here's the event
stream, and here's who gets to see what, when."* It layers **over the event journal
`jumpgate-core` already has** (`EventKind`: Production, Trade, ContractFulfilled,
Arrival, FuelEmpty, …) — additive on a real seam. It governs how far information on each
event cascades, so agents act on partial, stale, local knowledge rather than global truth.

## The two channels

1. **Broadcast** — facts that propagate by transmission: fast, wide, authoritative /
   low-noise, range-limited.
2. **Word-of-mouth = an SIR / epidemic per station** — once a story *reaches* a station it
   spreads there at a rate set by the event's **excitement / virality**: a viral story rips
   through the station in a tick or two; a boring one decays out over a few days. Each
   station is a "population" carrying a per-story infection/decay state.

## Why it matters (and why it's game-science-shaped)

- It makes "**learn where the rich areas are**" mechanically concrete for *every* forager
  role on one engine — miners (rich minerals), haulers (fat demand), pirates (fat haulers).
- It is what *defines the observables* the info-room bet measures over (the §9 belief-state
  ceiling vs best-memoryless bar). Information advantage becomes a measurable room source.
- It has its own falsifiable dynamics to validate later: does a viral story actually ripple
  fast, does a boring one die in days, does the asymmetry it creates become measurable room?
- Must be **deterministic** (seeded, integer epidemic) to stay in the lab.

## Open threads (for the future cycle, not now)

- What sets "excitement/virality" (event type? magnitude? recency?).
- Agent knowledge/belief representation + how staleness/decay is queried.
- **Reputation** (per the tension-web directive: locality/lag/reputation/capacity).
- Determinism + scale of an epidemic over many stations + agent beliefs.
- The **minimal** version the info-room bet actually needs (likely far less than the full
  engine — resist building the whole thing before the bet is authorized).
