# Vision — jumpgate

> Bootstrapped 2026-06-09 from observed repo + tracker reality (see PDR-0001).
> Items marked _(inferred)_ are drawn from specs/commits, not an owner statement —
> confirm or correct them.

## Purpose

jumpgate exists to **demonstrate that deep reinforcement learning produces more
entertaining game AI than scripted / FSM agents** — DRL is the discriminator that
sets it apart from every other space game. The artifact is a headless,
authoritative, deterministic **3D Newtonian space-sim engine** (Rust) the size of
a solar system, wired to PyTorch/Gymnasium via PyO3 + maturin, that serves as the
substrate on which DRL-controlled agents are shown off. The ML/gym surface is a
**first-class deliverable, not a deferred add-on** — closing the
DRL-beats-scripted loop end-to-end is the whole point.

## Positioning — the constraint is the feature (PDR-0004)

jumpgate is **"Space Crusader Kings," not "TIE Fighter."** The same Newtonian
physics that make twitch tactical flight un-fun (impossible at these speeds —
light-lag, g-loads) are exactly what *generate* the strategic/operational decision
space that is the fun: Δv budgets, velocity-matching, travel time, and
conflict-region geography. Realism is therefore a **load-bearing feature, not a
realism tax.** Corollary keep/cut test: cheap omnidirectional travel "sounds cool"
but homogenizes space into something small and samey — features that make the
constraint **legible and consequential** (geography, Δv commitment, time, conflict
regions) serve the core; features that frictionlessly dissolve it are suspect. This
is the *why* behind the strategic/operational thesis venue (PDR-0002).

## Who it serves

- **Primary:** the owner as researcher/builder — the immediate job is to *validate
  the thesis* (a learned policy is measurably better and more interesting than a
  hand-tuned baseline on a real control task).
- **Secondary _(inferred)_:** a future player who experiences DRL-driven emergent
  behavior in a rendered, player-interactive world (rendering + interactivity are
  Later bets, not v1).
- **Explicitly not:** a multiplayer audience. The program is the single source of
  truth for its universe; there is no external client to stay in lockstep with.

## Anti-goals (what it refuses to be)

- **Not multiplayer.** Single authoritative source of truth; no cross-machine
  lockstep, no MP determinism obligation.
- **Not scientifically accurate.** "Close enough for a given second" — orbits a
  bit off is fine. There is no external ground truth to be wrong against. Bounded,
  non-exploding, *looks* Newtonian is the bar.
- **Not a `murk`-style 13-crate machine.** "Build for but not with" is a governance
  gate: implement only the trivial version now; foreclose nothing structurally.
  The DO-NOT-BUILD-IN-V1 watch-list (ECS, arena allocator, observation compiler,
  scenario DSL, lever registry, spatial hash, LOD scheduler, component/wear/heat
  subsystems) requires an ADR naming a concrete requirement before any item is
  built. See the core design spec §2.
- **Not a scripted-AI game.** Hand-coded brains (autopilot, future trophic agents)
  are explicitly *scaffolds* for what DRL will learn — never the destination.

## North stars (long-term fidelity ambition)

v1 is rung 3 of the ladder (universe → craft moves A→B → craft with
thrust+fuel+mass → station refuel → craft interactions). The long-term target is a
**multi-domain high-fidelity simulator — economy, combat, exploration, industry —
with crew-on-a-flight-deck as the depth yardstick** (how deep the sim should
eventually go, down to individual crew on a deck). "Build for but not with" is
judged against *this* ambition: the seams must not structurally preclude these
directions even though none are built in v1.

## Authority grant

Granted by: John (owner)     Last reviewed: 2026-06-09
Review cadence: on any vision/thesis/scope change, else monthly

**Status: CONFIRMED by owner 2026-06-09 (PDR-0001) — accepted as drafted.**

Autonomous within strategy — the agent (acting PM) MAY, without asking:
  prioritize/triage the filigree backlog, write PRDs with falsifiable acceptance
  criteria, dispatch delivery (to /axiom-planning, engineering packs, workflows),
  accept or reject delivered work against its criteria, reprioritize, and kill a
  failing bet per metrics.md. File and close issues; run the design→plan→build→
  review loop on a chosen bet.

Escalate BEFORE acting — the agent MUST get owner sign-off for:
  - changing this vision / the DRL thesis / the rung ladder / the north stars;
  - **merging `jumpgate-v1-design` → `main`** (the integration commit is the
    closest thing here to a "release");
  - another hard-reset or scrapping a line of work (as happened 2026-06-08);
  - any git history rewrite / force-push on a shared branch;
  - adding any DO-NOT-BUILD-IN-V1 watch-list item (requires an ADR + sign-off);
  - anything irreversible or touching an external party.
  (Taxonomy + rationale: product-ownership-operating-model.md.)

This product has no users, no pricing, and no external surface today, so the
pack's default pricing / deprecation / data-deletion clauses are largely n/a —
they are retained above only where a jumpgate-specific analogue exists (merge to
main, scrapping a line, history rewrite).
