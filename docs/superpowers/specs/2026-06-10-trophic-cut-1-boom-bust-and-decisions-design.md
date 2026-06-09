# Trophic Cut 1 — A demonstrable boom/bust cycle with decision-driven peer dispersion

> **⚠ DEFERRED (2026-06-10, owner).** Superseded in sequence by the DRL pivot: agents must
> LEARN risk (PPO+LSTM), not carry a hardcoded `risk_appetite` scalar — that scalar was the
> computed-answer reflex again. Do NOT resume this build as written. The trophic world
> returns AFTER the tactical-flight rung proves the training pipeline
> (`2026-06-10-tactical-flight-rung1-design.md`). Phase-1 foundations are parked at WIP
> commit `2e1e1ad` (pirate columns/events/RngStream::Piracy — salvageable later).

> **FRAME (PDR-0006 — read first).** This is **GAME science**: we are building a game
> layer and judging it by **emergent play**. Every measurement below is a **designer's
> aliveness diagnostic** — an instrument for the observe→steer→re-observe loop — **NOT**
> a presolvability / fraction-of-ceiling / beat-the-optimum gate (that frame is retired
> and caused two hard resets). The word "self-averaging" appears here with the **opposite**
> meaning to the old probes: there it was "is there DRL room to prove"; here it is "are
> decisions consequential enough that the world is a *game*, so I know how to steer the
> scenario." If you ever find yourself asking whether a learner beats a computed optimum,
> stop — that is the trap.

Date: 2026-06-10   Status: DRAFT (awaiting owner review)   Author: acting-PM (Claude)
Supersedes/Refines: builds on the landed first-economic-loop harness; first build of the
arena issue `jumpgate-aec6e7bc14` ("emergent game world, judged by play").

---

## 1. The success bar (the owner's own definition)

> "A demonstrable multi-agent boom/bust cycle — the minimum point for a game — where you
> can make decisions and those decisions lead you to a position somewhere on the cycle
> compared to your peers who all ended up elsewhere based on their decisions."

Decomposed into **two co-equal, seed-reproducible properties**:

- **ALIVE** — a sustained boom/bust predator-prey cycle in the 3D Newtonian substrate
  (the `ecosystem-oscillation` signature: peace↔feast, scarcity→price spike), not a
  collapse to a fixed point and not extinction of either population.
- **A GAME** — heterogeneous hauler **decisions disperse peer outcomes**: where a hauler
  ends up (wealth / survival / deliveries) depends on its own choices versus peers who
  chose differently. A homogeneous cycle is *alive but not a game*; this clause is the
  load-bearing one.

We hit the bar when a single seeded run exhibits **both**, and the dispersion **tracks
the decisions** (not noise).

## 2. Why the GAME half is the hard half (the central risk)

A food-driven predator-prey system oscillates **even when predation is well-mixed** (any
pirate can reach any hauler). So we will likely *get a cycle* — and it can still fail the
bar, because a well-mixed cycle gives every hauler the *same* risk-over-time, risk-appetite
washes out, and peer outcomes collapse to noise.

The GAME half requires **persistent spatial risk-heterogeneity**: routes that are
genuinely, *durably* more dangerous than others at the same moment, so a route/contract
choice is a real bet. This is exactly where the project's two prior NO-GOs died —
`interdiction-rl-first-curve` (pirates-chase-traffic self-equalized risk → 0.4%) and
`contention-game-fifth-nogo` (LLN self-averaging → ~4%). **Both failed at risk
*equalization*, not at population dynamics.** So:

- The **cycle** (aliveness) is the robust half.
- The **dispersion** (gameyness) is the fragile half, and lives or dies on whether
  predation stays **local and persistent**.

The gym's antidotes are all designed in below: food-driven predators, **locality** (spatial
intercept gives this for free *if* pirate mobility is bounded and travel is committal),
**lag/memory** (notoriety + lie-low + stale beliefs give routes a *risk history*).

## 3. The keystone instrument — built FIRST (Component E)

Before any tuning, we build the **aliveness discriminator**, because "run and tune" across
a dozen coupled knobs without it is a flail. Two **independent** axes, computed from a run:

1. **Did the population cycle?** — anti-phase amplitude of `active-pirate-count` vs
   `active-hauler-density` (cross-correlation at a lag; amplitude above a noise floor;
   neither population pinned at 0 or saturated).
2. **Did risk stay heterogeneous (or equalize)?** — **cross-route variance** of per-route
   predation-risk **×** its **temporal autocorrelation**. Equalization shows up as either
   variance → 0 (every route equally dangerous) *or* low autocorrelation (risk flickers
   too fast to decide on).

Plus the **bar metric**: variance of per-hauler outcome (wealth/survival) **and** whether
it tracks risk-appetite.

**Diagnosis matrix** (different failures → different fixes — this is the whole point):

| Cycle? | Risk heterogeneous & persistent? | Outcomes disperse & track appetite? | Diagnosis → fix |
|---|---|---|---|
| ✅ | ✅ | ✅ | **Bar met.** |
| ❌ | — | — | Population didn't cycle → tune **food / lie-low / notoriety / regen** |
| ✅ | ❌ | — | **Risk equalized** → tighten **locality** (pirate mobility ↓, target uniformity ↓, over-fish-avoidance ↑, notoriety memory ↑, belief-staleness ↑) |
| ✅ | ✅ | ❌ | Decision layer not translating risk → widen **appetite spectrum**; check belief not too perfect |

## 4. Components

All additive on live seams. Column-oriented, integer/deterministic, in `jumpgate-core`.

### A. Pirate role — the boom/bust engine (2nd trophic level)
- New `CraftRole::Pirate` (append discriminant `rank() = 2`; existing ranks unchanged).
- Pirate per-craft state (new additive columns or an `Option<PirateState>` column on
  `CraftStore` — plan decides): `food` (i64 micros, accumulated from robs), `notoriety`
  (u32, heat), `lie_low_until` (Tick).
- **Decision** (seeded, deterministic): scan haulers; score candidates by **cargo value /
  reachability**, *de-weighted by local over-fishing* (recent rob density near the target's
  route); pick best; `Seek`-intercept (reuses the existing rendezvous/velocity-match
  primitive — you cannot rob what you cannot match). **Pirate mobility is a bounded knob**
  (the primary locality lever).
- **Food-driven population**: well-fed pirates persist / spawn; starving pirates go
  **LIE_LOW** (a state that is **off the predation field** — the structural refuge that
  stops hunters exterminating prey; `LIE_LOW < WANTED` from `navy-deterrence-and-chronicle`)
  or leave/die. Notoriety accrues per rob and *forces* lie-low past a threshold — this is
  the route **risk-memory** that resists equalization.

### B. Spatially-gated, statistically-resolved encounter
- Trigger: pirate within engage-range AND velocity-matched (a real 3D Newtonian condition;
  a hauler with Δv advantage can flee — preserves the spatial decision).
- Outcome (seeded roll on a new `RngStream::Piracy`, appended after `Scenario`): **rob**
  (cargo transferred to pirate / contract fails — cargo is conserved, not created),
  **driven-off** (no transfer), or **kill** (hauler removed — cut-1 stub for salvage).
- **Deferred to cut 2** (NOT built now): projectile potshots over time, cargo-DROP as
  ballistic salvage, grapple+board at ~0.1C, hauler-weapons drive-off model.

### C. Heterogeneous hauler decision — the "it's a game" layer
- New per-hauler **`risk_appetite`** column, spread across a cautious↔greedy spectrum at
  scenario build (seeded).
- Replace the uniform ASSIGN step (`run_scripted_dispatch` lowest-`ContractId`) with a
  per-hauler **choice** among offered contracts: score = `reward` weighted against
  **believed route-risk** through the lens of that hauler's `risk_appetite`. Cautious
  haulers pick safe/low-pay; greedy pick lucrative/risky.
- Pirate density on a route is **endogenous** to these choices (the canonical "safe B vs
  lucrative-but-pirate C" tension) — this is what disperses peer outcomes.

### D. Belief-state seam — observability as a first-class data attribute
- Haulers decide on a **belief** (estimated route-risk), **never on ground truth.** This
  makes observability a first-class attribute of the data from day one, so the full Media
  engine (broadcast + SIR word-of-mouth) is **additive later, not a retrofit**.
- **Cut-1 belief model is deliberately *lagged*, not perfect** — and this is *load-bearing
  for the cycle itself*, not just Media plumbing: a stale risk-belief is a **prey-response
  delay** (haulers keep running into danger after a spike, stay shy after it clears), and
  *delays are what sustain oscillation* instead of letting it settle to equilibrium. So
  belief-staleness is a deliberate **emergence knob**, not a placeholder.
- Cut-1 model: a per-route risk register, bumped by observed robs, **decaying** over time,
  **read with a lag**. Two knobs: decay rate, observation lag. (Media replaces the
  *propagation* model later; the *seam* — agents act on belief — is permanent.)

### E. Diagnostics + chronicle — the windows (Section 3)
- Per-run time series (the two discriminator axes + prices + robs/tick).
- Per-hauler final ledger {risk_appetite, wealth, deliveries, robs-suffered, survived?}.
- The discriminator classifier (Section 3 matrix) as a tested function over a run.
- Chronicle: a handful of distinguishable life-arcs, owner-readable as story.

## 5. Data flow (one tick)

```
ingest → run_producers → resolve_contracts (escrow/load)
       → HAULER DECISION (Component C: read belief D, choose contract by appetite)
       → PIRATE DECISION (Component A: pick target, Seek-intercept)
       → physics (integrator; existing)
       → ENCOUNTER RESOLUTION (Component B: gated rob/driven-off/kill, RngStream::Piracy)
       → PIRATE POPULATION UPDATE (Component A: food/notoriety/lie-low/spawn/leave)
       → BELIEF UPDATE (Component D: bump per-route risk from robs, decay)
       → resolve_failures / update_prices (existing)
       → DIAGNOSTICS sample (Component E)
```

## 6. Determinism constraints (non-negotiable)

- All new randomness via a **new appended** `RngStream::Piracy` (append-only; never reorder
  existing salts — that changes replay identity).
- Integer microcredits (i64), FNV-1a hashing, generational SlotMap ids; new `CraftRole`
  variant gets a stable appended `rank()`.
- **Single-cause golden discipline.** New dynamics live in **new scenarios/fixtures**; the
  existing goldens (zero-state `0x65d7_af3b…`, config `0x278c_5d91_b75a_9e5a`, recorded-run)
  must stay bit-stable. Any golden that moves moves for **one named reason**.
- Record-then-replay must be **bit-identical** for the new pirate scenario.

## 7. Conservation identities (extended — tested as gates)

- **Cargo conservation under robbery**: a rob *transfers* cargo (hauler→pirate) or *fails
  the contract* (cargo→consumed); it never creates/destroys cargo. `Σ stock + Σ in-transit
  + Σ pirate-held == initial + mined − consumed`.
- **Credit invariant** holds across robbery and contract failure (escrow refund path).
- Pirate `food` is an accounting column, not a credit source (no money minted by piracy).

## 8. Testing strategy (TDD, per component)

- Unit tests per component (pirate target-scoring; encounter gating; appetite-weighted
  choice; belief decay/lag; population transitions incl. lie-low refuge).
- The **discriminator classifier** tested against *synthetic* time series with known
  labels (cycle/no-cycle × equalized/heterogeneous) — it must classify the four corners
  correctly *before* we trust it on a real run.
- Determinism test (record-then-replay bit-identical) for the new pirate scenario.
- Conservation identities (Section 7) as run-level assertions.
- Existing goldens re-verified untouched (`cargo test -p jumpgate-core`,
  `clippy --all-targets`, grep the pinned hashes).

## 9. What we deliberately DON'T build now (deferred cuts, in order)

Full spatial combat (cut 2) → salvage/tugs (cut 3) → binding refuel/energy + pirate
hideouts (cut 4) → police/navy (cut 5) → full Media engine (cut 6). Fuel stays
**non-binding** here; a kill is a **stub-removal**. We stop when the **bar (Section 1) is
demonstrable** — that is the owner's "until we have a demonstrable cycle."

## 10. The build → observe → tune loop (where the bar is actually met)

Ultracode builds the **machinery** in parallel (Components A–E, TDD per task, verified).
A green build is **not** a met bar. The cycle is then **found at the console**: run the
seeded scenario, read the Section-3 windows, and **tune sequentially** — using the
diagnosis matrix to pick the *right* knob (population vs locality vs decision) instead of
flailing. This tuning loop is the `ecosystem-oscillation` discipline ("food-driven
predators was the key fix" came from looking, not from the first build). Expect a few
rounds. Stats are windows the whole way — never targets.

## 11. Reframe guards (anti-drift)

- Risk-heterogeneity is a **designer's aliveness diagnostic**, not the retired room gate.
- No metric in Section 3 is a pass/fail acceptance gate; they are instruments for steering.
- DRL is absent here; it enters a *later* cut as a **player**, judged by play.
- If a flat result appears, the response is **diagnose with the matrix and steer the
  scenario** — never "prove there's room" and never re-introduce a presolvability gate.
