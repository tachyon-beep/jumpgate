# Current State — jumpgate        Checkpoint: 2026-06-10 · commit 95f9209 · branch jumpgate-v1-design

## The bet right now

**Person + Ship Plan A — the EffectiveMods seam** (`jumpgate-d30fcebaac`, P1,
ready). The single non-additive signature change in the whole forward plan; owner
priority is debt-avoidance, so getting this one irreversible seam exactly right is
the bet. Owner confirmed "stay the course: Plan A next" (2026-06-09). Metric it
serves: the determinism guardrail (prove trajectory-equivalence) — it *enables* the
north-star, doesn't move it.

## Workspace layers (reconciled 2026-06-09)

Two complementary layers, no overlap:
- **Product** (`docs/product/`, this workspace) — what/why/for-whom/did-it-work:
  vision, north-star, the falsifiable bet, PDRs. Owned here.
- **Program** (`docs/superpowers/program/charter.md` + `raid.md`) — delivery
  structure: land order, RAID, cutover, backlog mechanics. Installed by a parallel
  session 2026-06-09; I reference it, don't duplicate it. (Routes to
  `/axiom-program-management`.)
- Plan-of-record (the engineering detail): `docs/superpowers/specs|plans/`.

## In flight (filigree — real backlog; land order CONFIRMED)

- `jumpgate-d30fcebaac` (P1) **Person Plan A — EffectiveMods seam** — ready, FIRST
  build (the one irreversible seam).
- `jumpgate-818a04bb6b` (P1) **Vertical-slice shaping pass** (DESIGN) — ready; runs
  alongside the Plan-A build. Defines the first economic loop, demand pricing, and
  the ecosystem `TARGET` metric. *Highest-value next PM move.*
- `jumpgate-5a3e01ab08` (P1) **Plan-4 gym + first trainable rung** — blocked_by A.
  PyO3 facade is a `scaffold_ok()` stub; gym plumbing + the A→B trainable rung — the
  seam where DRL enters as a *player* (PDR-0006), making agents interesting. Measure
  env steps/sec here, early.
- `jumpgate-a494b1d700` (P2) **Layer-1 ecosystem epic** (the game world) —
  blocked_by shaping; placeholder, decomposed at shaping. This is the emergent
  trophic life-sim — judged by play, the way `ecosystem-oscillation` was.
- `jumpgate-12f37a8d74` (P3) Person Plan B / `jumpgate-205fd66b25` (P3) Plan C —
  **re-sequenced to additive crew enrichment AFTER the first economic loop**
  (B blocked_by A, owns HASH_FORMAT_VERSION 1→2; C blocked_by B).
- `jumpgate-123b9f4856` / `1ec57e1002` / `c3c85a5da0` (P4) — deferred Class-1
  tunables, backlog.

## Resolved this session

- **FRAME CHANGE — v1 is judged as a GAME, not a presolvability gate (PDR-0006).**
  Headline decision. The "can a learner beat the computed optimum by ≥X% of ceiling"
  / fraction-of-ceiling discipline is a catch-22 for games (it can only return NO-GO
  or can't-measure) and is **RETIRED as a build prerequisite.** v1 is now judged by
  **emergent play** — surprising, watchable, alive — the way `ecosystem-oscillation`
  was (heuristic agents, zero RL: the project's one unambiguous success). DRL is a
  *player* that makes agents interesting, judged by the quality of play it produces.
  Determinism + chronicle/diagnostics/sweeps stays as the **reproducible lab for
  studying the game's emergent dynamics** ("game science") — never again as a gate.
  The `vsl-cannot-host-judgment-principle` stays as an accurate observation about why
  a *small replayable market* is boring — RETIRED as a build gate. Supersedes
  PDR-0005's gate-as-prerequisite; charter/metrics done-definition flagged for owner.
- **Commons-miner cut = the gate-confirming dead-end artifact** (`crates/jumpgate-commons-cut/RESULT.md`).
  Retained as a real negative result: the analytic cut could only ever produce a
  strawman-GO, a computation-trap NO-GO, or an unmeasurable result — empirically
  confirming the gate frame is a dead end for game-building. Kept as history, not
  the thesis vehicle.
- **Q1 Authority grant — CONFIRMED** as drafted (PDR-0001; `vision.md` grant now
  authoritative, not DRAFT).
- **Q2 Now bet — Plan A** (`jumpgate-d30fcebaac`). Owner chose stay-the-course over
  pulling the gym forward; debt-avoidance rationale (R3 in `raid.md`, ACCEPTED).
- **Q3 Where the fun lives — strategic/operational, NOT tactical** (PDR-0002). The
  fun is long-horizon contract/route/fuel-commitment decisions in the demand-driven
  multi-agent ecosystem, not the single-craft A→B joystick task. Navigator A→B is the
  first trainable rung; the game itself is the multi-agent world.
- **Combat/travel model clarified — PDR-0003.** Travel is EVE/Elite macro-warp
  ("go" → travel → "stop" = start decelerating over days/months, Newtonian = the
  built autopilot). Tactical drone combat WILL exist but is **gravity-decoupled**
  (rides the §3.2 LOD/local-origin seam — additive, no new debt); not v1, not the
  primary game focus.
- **Post-Plan-A ordering — RESOLVED** (charter land order CONFIRMED by owner):
  Plan A → (gym rung + shaping) → Layer-1 economy build (scripted loop → demand
  pricing → introduce DRL players where they make the world more alive) → then
  thicken (crew B/C → combat/law).
- **Positioning crystallized — PDR-0004.** "Space Crusader Kings, not TIE Fighter":
  the Newtonian constraint is the *feature* (it generates the strategic decision
  space); cheap omnidirectional travel is rejected (homogenizes space). Resolves the
  founding "realistic but not exciting" tension. Folded into `vision.md` Positioning.
- **Theory-of-crime worldbuilding captured** as design seeds on the ecosystem epic
  (`jumpgate-a494b1d700` comments #3/#4) for the loop-4 combat/piracy shaping — incl.
  the load-bearing design insight: the hull-as-capital-prize escalation ladder is the
  multi-agent layer that makes the world alive and rich — high-stakes, opposing-goal
  decisions (the tension-web population game) that emergent play feeds on.

## Open questions / blocked-on-owner

- **Vision positioning (PDR-0004)** — I added a "Positioning" section to `vision.md`
  from your own TIE-Fighter/Crusader-Kings framing. Touches vision, so confirm it
  captures your intent (or correct it).
- **Game-dynamics success criteria** — the old `TARGET` (a DRL-vs-scripted
  differential) is superseded by the game frame (PDR-0006). Define instead the
  game's *own* emergent-dynamics criteria — sustained predator-prey cycles
  (amplitude/period), pack formation/dispersal + autocorrelation, trophic balance,
  chronicle richness of individual lives — at **vertical-slice shaping**
  (`jumpgate-818a04bb6b`). The concrete charter/metrics redefinition is flagged for
  the owner (not auto-edited); left open.
- **Cutover gate** (`raid.md` open decision #2): is `jumpgate-v1-design` a
  deliberate long-lived branch, and what gate cuts v1 → `main`? (Merge-to-main is
  an escalation point in the grant.)

## Last checkpoint did

- Bootstrapped `docs/product/` (vision/roadmap/metrics/current-state + PDR-0001/0002).
- Verified reality directly: 17 modules + 144 tests (`cargo test -p jumpgate-core`
  exit 0); **gym facade confirmed a stub**; branch unmerged.
- Discovered + reconciled the parallel program layer (charter/raid + real backlog).
- Recorded the owner's answers + combat/travel clarification; reframed the
  north-star (PDR-0002, PDR-0003); reconciled to the CONFIRMED land order + new
  shaping/ecosystem issues (`818a04bb6b`, `a494b1d700`).
- Crystallized positioning (PDR-0004) + captured the theory-of-crime seeds
  (`a494b1d700` #3/#4). **Committed the workspace — this is the first checkpoint.**

## Next session, start here

1. **Vertical-slice shaping** (`jumpgate-818a04bb6b`, ready, A-independent) — the
   highest-value PM move: harvest `archive/` design → first closed economic loop +
   demand-pricing mechanism + **the game-dynamics success criteria** (emergent-play
   measures per PDR-0006: cycle amplitude/period, pack autocorrelation, trophic
   balance, chronicle richness) + a decomposed Layer-1 backlog. Recommended:
   superpowers brainstorming → design.
2. In parallel, Plan A is ready (`jumpgate-d30fcebaac`); dispatch via the
   engineering loop (spec-review → quality-review → independent gate re-verify).
3. Resolve the cutover gate with the owner, and confirm the vision Positioning
   (PDR-0004).
