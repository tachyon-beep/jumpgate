export const meta = {
  name: 'commons-miner-cut-design-review',
  description: 'Multi-expert design + adversarial critique of the commons-miner analytic-cut probe (first increment of the Jumpgate scale/density DRL arena), run as game science',
  phases: [
    { title: 'Design lenses' },
    { title: 'Adversarial critique' },
    { title: 'Synthesis' },
  ],
}

// ============================ SHARED CONTEXT ============================
const CONTEXT = `
PROJECT: Jumpgate — a 3D-Newtonian deterministic space-economy sim (Rust crate jumpgate-core + a Python/Gym wrapper). The owner's keystone framing: this is "GAME SCIENCE masquerading as game design" — the fun game is the phenotype; the genotype is a controlled, measurable, REPRODUCIBLE dynamical-systems experiment. Determinism/replay is the scientific control. Every mechanic ships with a falsifiable measurement (a stated emergent-dynamic hypothesis + a cheap discriminator that can refute it), not vibes-tuning.

WHERE WE ARE: a deterministic "first economic loop" HARNESS just shipped (producers→stock→contracts→haulers→demand-deflation pricing; conservation + replay-determinism gates; a closed forage loop via return-to-origin routing). That harness is the LABORATORY substrate. The next thing is the SCALE/DENSITY DRL ARENA — the real thesis vehicle.

THE THESIS DISCIPLINE (PDR-0005, hard-won over SIX prior empirical probes that ALL NO-GO'd):
- DRL-room = can a learner beat the BEST FIXED SCRIPT by a measurable fraction-of-ceiling (>10%, telemetry-ablated, vs best-closed-form, NEVER vs uniform-random)?
- The spine of every NO-GO was SELF-CANCELLATION: LLN self-averaging, risk self-equalization, clamped/closed-form markets. "Replay-determinism IS presolvability" — a small replayable few-actor market hosts computation, not judgment.
- The ONE thing that ever made room: a COUPLED loop held off its attractor (food-driven predator-prey → sustained disequilibrium) — but that was judged by PLAY with HEURISTIC agents and zero RL.
- The only MEDIUM-ceiling arena found was "navy vs co-adapting pirates" (two-population non-stationarity) — maximally unreachable (needs the apex stack + two-population co-training never converged).
- DISCIPLINE: before training ANY learner, run a CHEAP ANALYTIC CUT — a policy ladder constant → best-closed-form → per-seed-myopic → omniscient-DP, on held-out seeds, telemetry-ablated. NO-GO if best-closed-form→omniscient gap < 10% of ceiling. One repositioning, not an infinite search (reversal trigger: if the real arena also NO-GOs, fall back to entertaining-emergence).

THE DESIGN UNDER REVIEW: the FIRST increment = the COMMONS-MINER ANALYTIC-CUT PROBE. Decisions locked so far:
- PURPOSE: measure whether the commons/density structure has DRL room on live code, BEFORE building the predator/police world. Game-science: the cut IS the measurement apparatus; running it first proves the apparatus and dissolves a fabrication caveat (the shaping pass's ceiling numbers were workflow-generated/unverified) by re-measuring on real code.
- HOME: a STANDALONE deterministic Rust analysis module, reusing jumpgate-core's RNG + determinism primitives, NOT folded into the hashed/golden World (modeled on the archived gym diagnostics oscillation_report/chronicle_report). Cheap, isolated, reproducible.
- ARENA (owner-enriched, this is the substantive model): M mining regions with HETEROGENEOUS reward levels. Per-tick yield = f(CURRENT stock) and DECLINES as a region is mined toward EXHAUSTION (mine a rich region → less each tick → eventually none). Regions regenerate at a rate (a sweep axis; one-shot-exhaustion vs slow-regen is an open question). MOVING to another region costs TIME/"spend" (travel) — so each ship decides WHEN to abandon a depleting region for a richer one.
- THE STRATEGIC CORE (the room candidate): an ANTI-COORDINATION / herd-timing game — "move EARLY to claim the rich site (pay the cost now, but risk the whole herd follows and crowds/depletes it) vs STAY and bet the others leave, so you inherit the RESIDUALS at your current site with less competition." This is El Farol / minority-game / war-of-attrition shaped. Anti-coordination is one of the few structures that genuinely RESISTS self-cancellation (you want to NOT be where everyone else goes), which is why it's a plausible room source. "Dynamic mining packs" (forming/dispersing over time) is the emergent signature.
- SWEEP AXES: regen-rate × field-correlation (how diverse/independent the regions are; identical regions → no room, diverse/independent → max possible room).
- FIDELITY (owner decision): BOTH — (a) a deliberately SMALL, EXACTLY-SOLVABLE instance (few ships, few regions, short horizon) where the omniscient ceiling is computed exactly = the TRUSTWORTHY room number; AND (b) a LARGE instance run purely to OBSERVE pack emergence qualitatively (reported alongside, NOT as the room measurement).

THE MAKE-OR-BREAK OPEN QUESTION #1 (CEILING HONESTY — stress hard): in an ANTI-COORDINATION game, does a COORDINATED social-planner omniscient ceiling OVERSTATE *learnable* room? A decentralized single learner cannot capture a pure COORDINATION gap (that needs a central planner / communication). So the social-planner ceiling might measure "coordination headroom" not "learnable headroom," producing a misleading GO. The more honest "learnable room" might be a SINGLE omniscient agent BEST-RESPONDING to fixed others (selfish room), or the gap between a population at decentralized-reactive equilibrium and a population where ONE member is omniscient. This choice determines whether the cut measures the right thing.

THE MAKE-OR-BREAK OPEN QUESTION #2 (PARTIAL OBSERVABILITY / INFORMATION AS THE ROOM SOURCE — owner-elevated, stress equally hard): in the real world the rich areas are NOT globally observable — miners must LEARN/SCOUT where the rich minerals are (and on extension, haulers where the fat demand is, pirates where the fat haulers are; they all work the same way). Owner's words: "the only problem is they need ways to learn where the rich areas are." This is partial observability / ESTIMATED-NOT-OBSERVED state (info locality, lag, staleness, scouting, reputation) — explicitly on the room-maker list and the project's "locality/lag/reputation/capacity is the first-class room constraint" directive. CRITICAL CONSEQUENCE FOR THE CUT: the design as drafted gives policies the CURRENT region stocks/crowds (full observability, telemetry-ablation only denies FUTURE/oracle info). Under FULL observability the anti-coordination timing game may have little learnable room (a reactive rule suffices) → a FALSE NO-GO that wrongly KILLS the arena, when the REAL hidden-richness game has large room from INFORMATION ADVANTAGE (a learner that scouts/remembers/predicts where richness and the herd are beats a rule that sees only locally). So OBSERVABILITY is very likely a required AXIS: run the cut under FULL-info AND under PARTIAL/scouted-local-info, reporting room separately, to isolate whether room comes from anti-coordination timing, from information, or both. Experts MUST decide: observability as a third axis now, or deferred — and if deferred, how to avoid the full-info false-NO-GO killing the arena prematurely.

OTHER OPEN QUESTIONS: (2) exact yield/depletion/regen laws + one-shot-vs-regen; (3) travel/"spend" cost model (time vs a fuel/energy resource — the world has a refuel/energy mechanic planned); (4) what is a FAIR "best-closed-form" reactive heuristic in a multi-agent anti-coordination setting + telemetry-ablation details; (5) exact-small instance sizing (N ships, M regions, horizon T, stock discretization) so the omniscient DP/search is EXACTLY and CHEAPLY solvable — complexity check; (6) observe-large diagnostics that evidence "dynamic packs" (pack count, regroup events, spatial-distribution entropy, boom/bust period); (7) has this organically become the REAL arena rather than a cheap sanity-cut, and is that OK?; (8) determinism plan (integer dynamics, seeded RNG reuse); (9) trivial-NO-GO / trivial-GO degeneracy risks — will the cut fairly discriminate?

CONSTRAINTS: keep the cut CHEAP and EXACTLY-measurable (its whole virtue). Determinism = the scientific control. Compare vs best-closed-form, never uniform-random. Honor a NO-GO. CAVEAT: do NOT assert unverified in-code facts (workflow subagents have fabricated code claims here before) — reason about design, flag where a code reality-check is needed.
`

// ============================ PHASE 1: DESIGN LENSES ============================
phase('Design lenses')
const LENSES = [
  {
    key: 'game-theory-ceiling',
    agentType: 'general-purpose',
    brief: `GAME-THEORY / MECHANISM-DESIGN / CEILING-HONESTY lens. This is the most important lens. Analyze the commons-miner cut as an anti-coordination / minority-game / war-of-attrition problem.
FOCUS:
- THE CEILING QUESTION (make-or-break): does a coordinated social-planner omniscient ceiling overstate LEARNABLE room? Work through what "room" a single decentralized RL agent could actually capture here vs what only a central planner could. Recommend the RIGHT ceiling(s) for the ladder (social-planner? single-best-response-to-fixed-others? a population-with-one-omniscient-member? report multiple rungs?). Be concrete and decisive.
- Is the equilibrium of this anti-coordination game pure or MIXED? If the rational solution is a mixed strategy, what does that imply for "learnable room" and for what the best-closed-form rung should be?
- Where exactly does the move-early-vs-stay-for-residuals tension create exploitable structure a learner could beat a fixed reactive script at — and where might it self-cancel anyway (the prior NO-GO failure mode)?
- Recommend the falsifiable hypothesis + the precise fraction-of-ceiling definition for THIS multi-agent setting.`,
  },
  {
    key: 'analytic-cut-methodology',
    agentType: 'general-purpose',
    brief: `ANALYTIC-CUT / DRL-ROOM METHODOLOGY lens. Pressure-test the measurement protocol itself.
FOCUS:
- The 4-rung ladder (constant → best-closed-form → per-seed-myopic → omniscient-DP): is each rung well-defined for THIS arena? What is "per-seed-myopic" when the non-stationarity is other agents? What does telemetry-ablation mean concretely (what observable info does the closed-form get; what oracle info must it be denied)?
- Is the 10%-of-ceiling gate the right discriminator here? Define fraction-of-ceiling precisely (numerator/denominator) and what GO vs NO-GO concretely means.
- Held-out seeds, sweep design (regen-rate × field-correlation): how to avoid overfitting the closed-form; how to ensure the cut FAIRLY discriminates (not rigged to NO-GO like the prior survival test, nor rigged to GO).
- What would make this cut produce a FALSE GO or FALSE NO-GO, and how to guard each.`,
  },
  {
    key: 'sim-architecture-determinism',
    agentType: 'bravos-simulation-tactics:simulation-architect',
    brief: `SIMULATION-ARCHITECTURE + DETERMINISM + TRACTABILITY lens. Design the standalone deterministic Rust arena and check the exact ceiling is computable.
FOCUS:
- The arena as integer, deterministic, seeded dynamics reusing jumpgate-core's RNG (NOT in the hashed World). Data model for regions (stock, richness, regen), ships (position/region, travel state), the move/spend cost.
- TRACTABILITY: for the SMALL exactly-solvable instance, what concrete (N ships, M regions, horizon T, stock-discretization) keeps the omniscient ceiling EXACTLY solvable by DP or search? Give the state-space size / complexity. Where does it blow up? Recommend the largest exactly-solvable size.
- The LARGE observe-instance: how to run it cheaply for pack-emergence observation.
- Determinism pitfalls (float vs integer, RNG draw order, sort stability) and how the gym-diagnostic pattern (oscillation_report/chronicle_report #[ignore] reports) maps here.`,
  },
  {
    key: 'emergence-dynamic-packs',
    agentType: 'bravos-systems-as-experience:emergence-designer',
    brief: `EMERGENCE / DYNAMIC-PACKS lens. Make the "dynamic mining packs, not fixed" the owner wants actually emerge, and design what we OBSERVE.
FOCUS:
- What conditions (yield curve shape, depletion vs regen rates, move cost, field-correlation) produce genuine dynamic packs that form, deplete a region, and disperse/regroup — vs degenerate outcomes (everyone clumps forever, or perfectly uniform spread, or chaotic noise)?
- The move-early-vs-stay-for-residuals tension: what parameter regime makes it a live decision (neither dominant)?
- Concrete observe-large diagnostics that EVIDENCE dynamic packs as a measurable dynamical signal (pack count over time, regroup/exodus events, spatial-distribution entropy, boom/bust period, residual-camper payoff vs early-mover payoff). What's the game-room signature distinct from the DRL-room number?`,
  },
  {
    key: 'archetype-pattern',
    agentType: 'yzmir-systems-thinking:pattern-recognizer',
    brief: `ARCHETYPE / PRIOR-ART lens. Match this arena to known system archetypes and harvest their lessons.
FOCUS:
- Which archetypes does this instantiate (minority game / El Farol, tragedy of the commons, war of attrition, congestion game, ideal-free-distribution from ecology)? For each match, what is KNOWN about its dynamics, its equilibria, and whether learning/anticipation helps beyond a simple rule?
- Ideal Free Distribution (ecology): foragers distributing over patches proportional to richness is a known EQUILIBRIUM — does that mean a simple proportional rule already captures most value (a NO-GO signal), or does depletion+travel-cost+timing break IFD enough to leave room?
- Known interventions/pitfalls from these archetypes that should shape the design or the gate.
- Prior-art verdict: based on these archetypes, is room more likely a GO or NO-GO, and what single design lever most changes that?`,
  },
  {
    key: 'partial-observability-information',
    agentType: 'general-purpose',
    brief: `PARTIAL-OBSERVABILITY / INFORMATION-AS-ROOM-SOURCE lens (owner-elevated, co-equal with the ceiling lens). The owner insists the rich areas are NOT globally observable: agents must LEARN/SCOUT where richness is ("the only problem is they need ways to learn where the rich areas are") — for minerals, hauler-demand, and pirate prey alike.
FOCUS:
- Make the case for/against observability being THE dominant room source here (estimated-not-observed state; info locality/lag/staleness/scouting/reputation). Prior probes NO-GO'd largely under full observability — is hidden richness the missing room-maker?
- FALSE-NO-GO RISK: if the cut runs under FULL observability, does the anti-coordination timing game collapse to a reactive rule (no room) and wrongly KILL the arena, even though the real hidden-richness game has large information room? How likely, and how do we guard it?
- DESIGN: how to model partial observability cheaply and deterministically in the standalone Rust cut — local sensing radius, stale shared signals/rumor, a scouting action that trades mining-time for information, memory/belief state. What is the MINIMAL faithful version?
- Should observability be a THIRD sweep axis (full-info vs scouted-local) reported separately, so the cut isolates timing-room vs information-room vs both? Or is that scope creep for a first cut — and if deferred, what's the cheapest guard against the full-info false-NO-GO (e.g. run BOTH the full-info gate and a partial-info gate; only a both-NO-GO kills the arena)?
- For the ladder under partial info: what does best-closed-form / omniscient mean (the omniscient sees true richness; the closed-form sees only scouted/local) — and is the full-info-omniscient vs partial-info-best-closed-form gap the most honest "information room" measure?`,
  },
]

const lensResults = await parallel(LENSES.map(L => () =>
  agent(
    `${CONTEXT}\n\n=== YOUR LENS ===\n${L.brief}\n\nProduce a focused design-development memo from your lens: concrete recommendations, the issues/risks you see, and your answers to the open questions in your focus. Be decisive and specific (this feeds a synthesis that becomes a design spec). You may read the repo docs for grounding (docs/product/decisions/0005-*.md, docs/superpowers/reviews/2026-06-09-vertical-slice-shaping-findings.md, the memory files) but do NOT assert unverified in-code facts. Return your memo as text.`,
    { label: `lens:${L.key}`, phase: 'Design lenses', agentType: L.agentType },
  ).then(text => ({ key: L.key, text }))
))

const lensBlob = lensResults.filter(Boolean).map(r => `\n===== LENS: ${r.key} =====\n${r.text}`).join('\n')
log(`Lenses done (${lensResults.filter(Boolean).length}/${LENSES.length}); running adversarial critique`)

// ============================ PHASE 2: ADVERSARIAL CRITIQUE ============================
phase('Adversarial critique')
const CRITICS = [
  {
    key: 'room-is-real',
    brief: `RED-TEAM: "the measured room is not real / not learnable." Argue as hard as you can that this cut will produce a MISLEADING result. Attack: the coordinated-ceiling-overstates-learnable-room problem; whether the "room" is a pure coordination gap a lone learner can't touch; whether the anti-coordination structure self-cancels at the population level (mixed equilibrium → reactive rule already optimal); whether the room vanishes once telemetry-ablated. State the SINGLE most likely way this cut declares GO when there is no real learnable room (and vice versa). Then state what design change would fix it.`,
  },
  {
    key: 'cheap-and-computable',
    brief: `RED-TEAM: "the cut is not actually cheap / the exact ceiling is not computable." Attack the tractability of the exact omniscient ceiling under travel + depletion + multi-agent timing; the discretization error; whether the small exactly-solvable instance is so small it can't exhibit the very tension being measured (a scale paradox: exact-solvable ⇒ too small for packs/anti-coordination). Attack the determinism claims. State the single biggest feasibility risk and the cheapest mitigation.`,
  },
  {
    key: 'fair-test-discipline',
    brief: `RED-TEAM: "the cut dodges the discipline / is rigged." Attack: is it a FAIR test or secretly rigged toward GO (so we get to build the fun world) or toward NO-GO (like the prior survival test that required zero refutations)? Has the owner's enrichment quietly turned a 'cheap sanity check' into 'the real arena' WITHOUT acknowledging the cost/scope change? Does the design honor the reversal trigger, or does it set up an infinite arena search? Is 'observe-large packs' being smuggled in as evidence of DRL room when it's only game-room? State the single biggest discipline breach and the fix.`,
  },
]

const critiqueResults = await parallel(CRITICS.map(C => () =>
  agent(
    `${CONTEXT}\n\n=== THE DESIGN MEMOS FROM THE EXPERT LENSES ===\n${lensBlob}\n\n=== YOUR JOB (ADVERSARIAL CRITIC) ===\n${C.brief}\n\nBe specific and ruthless but fair. Cite which lens claims you are attacking. End with: the SINGLE highest-severity finding, and the concrete design change that resolves it. Return text.`,
    { label: `critic:${C.key}`, phase: 'Adversarial critique', agentType: 'general-purpose' },
  ).then(text => ({ key: C.key, text }))
))

const critiqueBlob = critiqueResults.filter(Boolean).map(r => `\n===== CRITIC: ${r.key} =====\n${r.text}`).join('\n')
log(`Critique done (${critiqueResults.filter(Boolean).length}/${CRITICS.length}); synthesizing`)

// ============================ PHASE 3: SYNTHESIS ============================
phase('Synthesis')
const synthesis = await agent(
  `${CONTEXT}\n\n=== EXPERT LENS MEMOS ===\n${lensBlob}\n\n=== ADVERSARIAL CRITIQUES ===\n${critiqueBlob}\n\n=== YOUR JOB (SYNTHESIZER) ===\nSynthesize all of the above into a HARDENED DESIGN for the commons-miner analytic-cut probe that the project owner can approve. Resolve conflicts; where experts disagree, pick a position and justify it. Produce:\n1. RESOLVED DESIGN — the arena model (yield/depletion/regen laws, move/spend cost), the policy ladder + the CEILING DECISION (resolve the coordinated-vs-best-response ceiling question decisively — crux #1), the OBSERVABILITY DECISION (resolve crux #2 decisively: is partial-observability/scouting a third sweep axis in this first cut or deferred — and the concrete guard against a full-info FALSE-NO-GO killing the arena, e.g. require BOTH a full-info and a partial-info NO-GO before honoring it), the telemetry-ablation, the fraction-of-ceiling definition + the pre-registered GO/NO-GO gate, the exact-small instance sizing (concrete N/M/T/discretization) + the observe-large instance + its pack diagnostics, the determinism plan, the standalone-Rust-module home.\n2. THE TOP 3-5 OPEN DECISIONS that still need the OWNER's call (with your recommendation each), phrased crisply.\n3. THE BIGGEST RISK that survives the design, and how the cut itself will detect it.\n4. A one-paragraph HONEST FRAMING: is this still a cheap sanity-cut or now the real arena, and what that means for cost/expectations.\nBe concrete enough that the next step (writing a design spec) is mechanical. Return text.`,
  { label: 'synthesize', phase: 'Synthesis', agentType: 'general-purpose' },
)

return { lensResults, critiqueResults, synthesis }
