export const meta = {
  name: 'jumpgate-vertical-slice-shaping-v2',
  description: 'Adversarial shaping pass for the Jumpgate v1 demand-driven economy slice (v2: schemaless harvest + abort-if-empty guard): harvest archive on the DRL-room axis, generate candidate arenas, grade each by fraction-of-ceiling + N-agent trainability, red-team, synthesize a design SPACE (not a spec) with coupled owner-decisions',
  phases: [
    { title: 'Harvest', detail: '4 readers (text): DRL-room graveyard, economy primitives, live substrate seams, demand-pricing mechanism' },
    { title: 'Generate', detail: '4 lens generators propose candidate DRL arenas in the loop' },
    { title: 'Consolidate', detail: 'dedup + rank the union into top candidates' },
    { title: 'Judge', detail: 'per-candidate ceiling+R6 estimate, then adversarial refute' },
    { title: 'Synthesize', detail: 'design space + coupled arena/metric owner-decisions' },
  ],
}

// ---------- schemas (downstream only; harvest is schemaless text — the big nested
//            harvest schema was the v1 failure mode, all 4 readers returned null) ----------
const CANDIDATE_LIST = {
  type: 'object', properties: {
    lens: { type: 'string' },
    candidates: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' },
      decision: { type: 'string', description: 'what the DRL agent actually decides' },
      where_in_loop: { type: 'string' },
      first_pass_room: { type: 'string', description: 'why a learner might beat the best fixed script here' } },
      required: ['name','decision','where_in_loop'] } },
  }, required: ['lens','candidates'],
}

const CONSOLIDATED = {
  type: 'object', properties: {
    candidates: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' }, decision: { type: 'string' }, where_in_loop: { type: 'string' },
      merged_from: { type: 'string' } }, required: ['name','decision','where_in_loop'] } },
  }, required: ['candidates'],
}

const CEILING = {
  type: 'object', properties: {
    name: { type: 'string' },
    ceiling_estimate: { type: 'object', properties: {
      optimal_vs_best_fixed_script: { type: 'string' },
      is_presolvable: { type: 'boolean' },
      ceiling_height: { type: 'string', enum: ['HIGH','MEDIUM','LOW','~ZERO'] },
      reasoning: { type: 'string' } }, required: ['ceiling_height','is_presolvable','reasoning'] },
    room_source: { type: 'string', enum: ['population_contention','partial_observability','non_stationarity_other_learners','sequential_commitment_under_uncertainty','none'] },
    r6_trainability: { type: 'object', properties: {
      concurrent_awake_agents: { type: 'string' }, verdict: { type: 'string', enum: ['FEASIBLE','MARGINAL','INFEASIBLE'] }, reasoning: { type: 'string' } },
      required: ['verdict','reasoning'] },
    archive_rerun_risk: { type: 'string' },
    metric_that_reads_differential: { type: 'string' },
    verdict: { type: 'string', enum: ['PROMISING','MARGINAL','DEAD'] },
  }, required: ['name','ceiling_estimate','room_source','r6_trainability','verdict'],
}

const REFUTE = {
  type: 'object', properties: {
    strongest_attack: { type: 'string' },
    refuted: { type: 'boolean' },
    residual_room_if_survives: { type: 'string' },
    confidence: { type: 'string', enum: ['low','medium','high'] },
  }, required: ['strongest_attack','refuted','confidence'],
}

const DESIGN_SPACE = {
  type: 'object', properties: {
    first_loop_design: { type: 'object', properties: {
      summary: { type: 'string' },
      primitives: { type: 'array', items: { type: 'object', properties: {
        name: { type: 'string' }, role: { type: 'string' }, sits_on_seam: { type: 'string' } }, required: ['name','role'] } },
      scripted_fixed_price_stage: { type: 'string' },
      demand_pricing_stage: { type: 'string' } },
      required: ['summary','primitives'] },
    surviving_arenas: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' }, ceiling_height: { type: 'string' }, room_source: { type: 'string' },
      r6_verdict: { type: 'string' }, metric: { type: 'string' }, why_it_survives: { type: 'string' } },
      required: ['name','ceiling_height','metric'] } },
    dead_arenas: { type: 'array', items: { type: 'object', properties: {
      name: { type: 'string' }, killed_by: { type: 'string' } }, required: ['name','killed_by'] } },
    coupled_owner_decisions: { type: 'array', items: { type: 'object', properties: {
      decision: { type: 'string' }, options: { type: 'array', items: { type: 'string' } }, recommendation: { type: 'string' } },
      required: ['decision','options','recommendation'] } },
    foundational_integration: { type: 'string' },
    open_questions: { type: 'array', items: { type: 'string' } },
    top_risks: { type: 'array', items: { type: 'string' } },
  }, required: ['first_loop_design','surviving_arenas','coupled_owner_decisions','open_questions'],
}

// ---------- shared context handed to every agent ----------
const CTX = `
PROJECT: Jumpgate v1 — a deterministic 3D Newtonian space sim (Rust core jumpgate-core + PyO3/Gymnasium facade jumpgate-py). cwd = /home/john/jumpgate.
THE THESIS (product PDR-0002, owner-confirmed): DRL-controlled agents make measurably BETTER LONG-HORIZON STRATEGIC/OPERATIONAL decisions than scripted/FSM agents, and are more entertaining. Venue is STRATEGIC/OPERATIONAL, NOT tactical fly-by-stick. The thesis is tested inside a DEMAND-DRIVEN MULTI-AGENT ECONOMY vertical slice.
THE SLICE (owner definition): miners mine -> refine -> sell FUEL; HAULERS move goods station->station under DELIVERY CONTRACTS for a reward; PRICES DEFLATE when many agents are willing to do that route-work (endogenous market); then a combat/piracy/law trophic level. Built thin-loop-first: scripted fixed-price loop -> demand pricing -> swap scripted->DRL & MEASURE.

THE CENTRAL RISK (RAID R5) — the spine of this whole pass:
The ENTIRE prior archived line is a GRAVEYARD of "DRL has no room" findings. A small, tractable, presolvable market hosts COMPUTATION, not JUDGMENT — DRL only beats scripts where the problem has genuine ROOM (population-scale contention, partial observability, non-stationarity from other learners), NOT a closed-form clearing price. Building the DRL decision where it has no room = the thesis QUIETLY FAILS.
THE DISCIPLINE (learned the hard way): grade every candidate arena by FRACTION-OF-CEILING (can a learner beat the BEST FIXED SCRIPT at all? estimate perfect-info-optimal vs best-script differential), NOT by effect size. If the ceiling is ~0 (closed-form optimum), no DRL can win there.
SECOND FEASIBILITY GATE (RAID R6): even a high-ceiling arena is dead if it cannot be TRAINED at concurrent-awake N-agent throughput (single-env ~600k steps/sec measured; LOD dormancy is the lever). Check BOTH gates per candidate.
`

// ---------- Phase 1: Harvest (SCHEMALESS — return focused markdown) ----------
phase('Harvest')
const READ_DISCIPLINE = `
OUTPUT: return FOCUSED MARKDOWN prose (NO JSON, no tool to call at the end — your final message text IS the deliverable). Target ~700-1000 words, structured with headers. Read SELECTIVELY: skim for the relevant sections, quote the key lines with file:line, do NOT echo whole files. If a path is missing, note it and continue. Do NOT fabricate code/measurements — if unsure, say so.`

const harvestSpecs = [
  { label: 'harvest:drl-room-graveyard',
    p: `${CTX}
YOUR JOB (the most important reader): harvest the DRL-ROOM GRAVEYARD. For EVERY probe/arena, extract: the verdict (GO/NOGO/PARTIAL), the differential or fraction-of-ceiling reached, and the STRUCTURAL CAUSE (what gave it room or killed it — population contention / partial obs / non-stationarity, vs closed-form clearing / self-averaging LLN / presolvable). Read selectively:
- archive/solution-architecture/24-first-real-decision-playability-gate.md
- archive/solution-architecture/25-judgment-requires-intractability.md
- memory dir /home/john/.claude/projects/-home-john-jumpgate/memory/ : vsl-cannot-host-judgment-principle.md, contention-game-fifth-nogo.md, interdiction-rl-first-curve.md, dminus-skill-signal-finding.md, cplus-contested-sourcing-landed.md, vsl-contest-is-seed-invariant.md, ecosystem-oscillation-landed.md
SPECIAL ATTENTION: the ONE GO (ecosystem-oscillation — WHY did food-driven predators create room when everything else failed?) and the PRINCIPLE (judgment requires intractability). End with a crisp list "STRUCTURAL PROPERTIES THAT CREATE ROOM (vs kill it)" distilled from the graveyard.${READ_DISCIPLINE}`,
  },
  { label: 'harvest:economy-primitives',
    p: `${CTX}
YOUR JOB: harvest the REUSABLE ECONOMY DESIGN + concrete primitive shapes (recipe / producer / market / contract / corporation). Prioritize these 4, skim the rest:
- archive/solution-architecture/19-vertical-slice-high-level-design.md, 22-production-graph-economy-epic.md
- archive/solution-architecture/adrs/0006-corporations-as-funded-contract-originators.md, 0008-decision-authority-policy-harness.md
Then skim: 20-corporate-economy-incremental-proposal.md, 21-rival-hauler-npcs-proposal.md, 23-freight-granularity-and-the-freightstar.md, archive/docs/plans/2026-06-05-epicB-recipe-primitive.md, 2026-06-05-epicC-first-industry.md, 2026-06-05-epicE-physical-extraction.md, archive/VERTICAL_SLICE_STATUS_GAP_ANALYSIS.md
Extract primitive shapes REUSABLE AS DESIGN on the new 3D substrate (hecs-ECS code is dead, design is gold). For each primitive note how it would sit on a struct-of-arrays + event/contract substrate.${READ_DISCIPLINE}`,
  },
  { label: 'harvest:live-substrate-seams',
    p: `${CTX}
YOUR JOB: harvest the LIVE SUBSTRATE — what exists TODAY that the economy must attach to. Read the REAL code (cite file:line, quote — do NOT fabricate; if unsure say so):
- crates/jumpgate-core/src/stores.rs (SoA columns, BaseSpec, EffectiveMods / effective_params seam), events.rs (event + contract/arrival machinery), world.rs (step/reset), autopilot.rs
- crates/jumpgate-py/src/env.rs, obs.rs (gym obs/action surface)
- docs/superpowers/reviews/2026-06-09-maneuver-authority-panel.md (maneuver-authority / rendezvous / LOD foundational inputs)
Report the REAL seams the economy attaches to (additive SoA columns, the EffectiveMods bundle, the event/contract surface, the gym obs/action surface) AND the panel foundational inputs (dt=0.25 authority regime, co-orbiting rendezvous arrival, ARRIVAL_RADIUS-as-config, LOD tiering). Be concrete about what is additive vs what would force a seam change.${READ_DISCIPLINE}`,
  },
  { label: 'harvest:demand-pricing-mechanism',
    p: `${CTX}
YOUR JOB: harvest the DEMAND-DRIVEN PRICING mechanism design AND why the skill-signal/pricing probes FAILED (so we don't rebuild a known NO-GO). Read selectively:
- archive/solution-architecture/20-corporate-economy-incremental-proposal.md, 22-production-graph-economy-epic.md, 24-first-real-decision-playability-gate.md
- archive/docs/plans/2026-06-05-epicD-price-homeostasis.md, 2026-06-06-epicDa-skill-signal-probe.md
- memory: cplus-contested-sourcing-landed.md, dminus-skill-signal-finding.md, vsl-contest-is-seed-invariant.md
Extract: (a) the concrete demand/deflation pricing mechanism (how route reward deflates with labour supply), (b) the CRITICAL finding — why a tractable replayable price/VSL could NOT host a learnable skill signal (the trailing-drain predictor couldn't beat the level-reader out-of-sample without becoming a closed-form oracle). State plainly where pricing-as-arena is presolvable.${READ_DISCIPLINE}`,
  },
]
const harvestsRaw = await parallel(harvestSpecs.map(s => () =>
  agent(s.p, { label: s.label, phase: 'Harvest' })  // NO schema -> returns text
))
const harvests = harvestsRaw.map((h, i) => ({ focus: harvestSpecs[i].label, text: h })).filter(h => h.text)

// HARD GUARD: never limp forward on an empty harvest (the v1 bug).
if (harvests.length < 3) {
  throw new Error(`Harvest underran: only ${harvests.length}/4 readers returned non-null. Aborting rather than building on an empty harvest. Failed: ${harvestSpecs.filter((s,i)=>!harvestsRaw[i]).map(s=>s.label).join(', ')}`)
}

const harvestDigest = harvests.map(h => `\n===== ${h.focus} =====\n${h.text}`).join('\n')
log(`Harvest complete: ${harvests.length}/4 readers returned text (${harvestDigest.length} chars).`)

// ---------- Phase 2: Generate candidate arenas (perspective-diverse) ----------
phase('Generate')
const lenses = [
  { lens: 'hauler route/contract selection', hint: 'route choice, contract acceptance, multi-leg routing, timing under deflationary pricing' },
  { lens: 'miner/producer operation', hint: 'extraction siting, refine-vs-sell timing, inventory/stockpile commitment, which good to make' },
  { lens: 'pirate/navy predation & deterrence', hint: 'target selection, lie-low vs strike under notoriety/heat, navy patrol allocation' },
  { lens: 'fuel & capital commitment under uncertainty', hint: 'fuel-load vs cargo tradeoff, when to commit capital to a long voyage given price/predation risk' },
]
const genLists = (await parallel(lenses.map(L => () =>
  agent(`${CTX}

HARVEST (the 4 readers' reports — USE IT, especially the structural room/no-room properties and the principle):
${harvestDigest}

YOUR LENS: ${L.lens} (think about: ${L.hint}).
Propose 2-4 candidate DRL ARENAS within this lens — places in the demand-driven loop where a learned agent makes a recurring decision. For each: WHAT the agent decides, WHERE in the loop, and a FIRST-PASS argument for why a learner might beat the best fixed script there. CRITICAL: bias toward arenas with structural ROOM (population contention among many haulers/miners, partial observability, non-stationarity from OTHER learning agents) and AWAY from anything that reduces to a closed-form clearing price or a self-averaging LLN field (the documented NO-GOs). Fewer high-room arenas beats many presolvable ones.`,
    { label: `gen:${L.lens.split(' ')[0]}`, phase: 'Generate', schema: CANDIDATE_LIST })
))).filter(Boolean)

// ---------- Phase 3: Consolidate ----------
phase('Consolidate')
const allCandidates = genLists.flatMap(g => (g.candidates||[]).map(c => ({...c, lens: g.lens})))
const consolidated = await agent(`${CTX}

Here are ${allCandidates.length} candidate DRL arenas proposed across 4 lenses:
${JSON.stringify(allCandidates, null, 1)}

Deduplicate (merge arenas that are the same decision under different names), then RANK by likely fraction-of-ceiling room (population contention / partial obs / non-stationarity rank HIGH; closed-form / self-averaging rank LOW). Return the TOP 6 distinct candidates. Keep the highest-room ones even if harder to build.`,
  { label: 'consolidate', phase: 'Consolidate', schema: CONSOLIDATED })

const candidates = (consolidated?.candidates || []).slice(0, 6)
log(`Consolidated to ${candidates.length} distinct candidate arenas.`)

// ---------- Phase 4: Judge (ceiling+R6, then adversarial refute) ----------
phase('Judge')
const judged = await pipeline(
  candidates,
  (c) => agent(`${CTX}

HARVEST:
${harvestDigest}

CANDIDATE ARENA: ${JSON.stringify(c)}

Estimate rigorously: (1) the CEILING — differential between a perfect-information optimal policy and the BEST SINGLE FIXED SCRIPT; is the optimum presolvable/closed-form (=> DRL can only tie => DEAD)? Rate ceiling_height HIGH/MEDIUM/LOW/~ZERO. (2) room_source. (3) R6 trainability at concurrent-awake N-agent throughput. (4) archive_rerun_risk — which prior NO-GO does this resemble (contention-game ~4%, interdiction NO-GO, dminus skill-signal, vsl presolvable), or novel? (5) a falsifiable metric reading the DRL-vs-script differential IN THIS ARENA. Be HARSH — the default prior from this project's history is that arenas have no room.`,
    { label: `ceiling:${(c.name||'?').slice(0,24)}`, phase: 'Judge', schema: CEILING }),
  (ceil, c) => parallel([
    () => agent(`${CTX}\n\nArena: ${JSON.stringify(c)}\nCeiling assessment: ${JSON.stringify(ceil)}\n\nYou are a CEILING SKEPTIC. Try hard to REFUTE that this arena has room: argue the optimum is closed-form / the field self-averages / the best fixed script already captures ~all the value. Default to refuted=true unless there is a concrete, durable source of room. Cite the archive NO-GO it most resembles.`,
      { label: `refute-ceiling:${(c.name||'?').slice(0,18)}`, phase: 'Judge', schema: REFUTE }),
    () => agent(`${CTX}\n\nArena: ${JSON.stringify(c)}\nCeiling assessment: ${JSON.stringify(ceil)}\n\nYou are a TRAINABILITY/REALITY SKEPTIC. Try hard to REFUTE viability on R6 (throughput at concurrent-awake N agents) AND on whether it can be built on the current pure-physics substrate within the thin v1 slice (no economy exists yet). Default to refuted=true if it needs infeasible scale or machinery far beyond the thin slice.`,
      { label: `refute-build:${(c.name||'?').slice(0,18)}`, phase: 'Judge', schema: REFUTE }),
  ]).then(votes => {
    const v = (votes||[]).filter(Boolean)
    const refutes = v.filter(x => x.refuted).length
    return { candidate: c, ceiling: ceil, refutations: v, refute_count: refutes,
             survives: refutes < 1 && ceil?.ceiling_estimate?.ceiling_height !== '~ZERO' && ceil?.verdict !== 'DEAD' }
  })
)
const judgedClean = judged.filter(Boolean)
const survivors = judgedClean.filter(j => j.survives)
log(`Judged ${judgedClean.length} arenas; ${survivors.length} survive the refute panel.`)

// ---------- Phase 5: Synthesize the design SPACE (NOT a spec) ----------
phase('Synthesize')
const designSpace = await agent(`${CTX}

You are the shaping-pass SYNTHESIZER. Produce a design SPACE (NOT a finished spec, NOT a decomposed backlog — those come AFTER owner dialogue). Inputs:

HARVEST:
${harvestDigest}

JUDGED ARENAS (ceiling, R6, refutations, survive flag):
${JSON.stringify(judgedClean, null, 1)}

Produce:
1. first_loop_design — the thin closed economic loop (miners->refine->fuel->haulers->contracts) as PRIMITIVES on the REAL live seams (SoA columns / EffectiveMods / event+contract surface from the substrate harvest). Two stages: scripted fixed-price, then demand-deflation pricing. Charter-locked, lower-risk — keep crisp.
2. surviving_arenas — those that survived ceiling+R6+refute, each with ceiling_height, room_source, r6_verdict, and the falsifiable metric reading its differential.
3. dead_arenas — what was killed and by what (show the graveyard was respected).
4. coupled_owner_decisions — THE key output. The metric (TARGET) is COUPLED to the arena: present arena+ceiling+metric as ONE coupled choice, not a metric question alone. Give 2-3 concrete coupled options with a recommendation.
5. foundational_integration — how the panel's maneuver-authority (dt=0.25, thrust~3x gravity), co-orbiting rendezvous arrival, ARRIVAL_RADIUS-as-config, and LOD tiering fold into the loop.
6. open_questions for the owner, and top_risks.

Be honest: if NONE of the candidates has defensible HIGH ceiling room, SAY SO loudly — that is the single most important finding (it would mean the slice as conceived risks the RAID R5 quiet-thesis-failure).`,
  { label: 'synthesize-design-space', phase: 'Synthesize', schema: DESIGN_SPACE })

return {
  harvest: harvests,
  candidates_judged: judgedClean,
  survivors_count: survivors.length,
  design_space: designSpace,
}
