export const meta = {
  name: 'pirates-rung-design',
  description: 'Design the pirates rung + upgrades economy: scouts, 3-angle design panel, adversarial critique',
  phases: [
    { title: 'Scout', detail: '5 parallel code/doc scouts map the substrate surfaces' },
    { title: 'Design', detail: '3 designers: ecology, economy, substrate/RL' },
    { title: 'Critique', detail: '2 adversarial critics per design (frame, feasibility)' },
  ],
}

const REPO = '/home/john/jumpgate'

const LAWS = `
PROJECT LAWS (violating any is a critical finding):
- PDR-0006 FRAME: Jumpgate v1 is a GAME judged by EMERGENT PLAY. The presolvability-gate / fraction-of-ceiling / beat-the-optimum frame is RETIRED (caused two hard resets). DRL is a PLAYER, not a discriminator-of-worth. Determinism+chronicle+sweeps = the lab for studying emergent dynamics, NOT a gate. Read ${REPO}/docs/product/decisions/0006-judge-v1-as-a-game-not-a-presolvability-gate.md if unsure.
- GAME-SCIENCE: every mechanic ships with a falsifiable hypothesis + a cheap discriminator baked in (designer's aliveness diagnostic), not bolted on.
- DETERMINISM: world state is integer/fixed-point, deterministic, seeded. Randomness ONLY via the append-only RngStream enum (RngStream::Piracy already exists, appended after Scenario). HASHED state must fold into state_hash; adding hashed columns CHANGES goldens — allowed, but each golden change must be a SINGLE-CAUSE commit with re-derived literals (never invented). Current goldens: HASH_FORMAT_VERSION=3, GOLDEN_ZERO_STATE_HASH=0x1d44_b373_5ccd_33f7, GOLDEN_CONFIG_HASH=0xf4bc_85c3_7cb6_8a6b.
- EVIDENCE-NOT-VALUATIONS: agent observations carry evidence (who/what/where/when, positions, magnitudes), never pre-computed valuations ("danger 0.7", threat tags, confidence scores). Valence is role-relative and assigned by each role's reward stream.
- RETIRED: the risk_appetite scalar premise (a hardcoded per-agent risk knob driving decisions) — agents must LEARN risk. The risk_appetite column exists in CraftStore as a vestige; do not build decision logic on it.
- OBS LAW: fixed global scales, stationary by construction, NO VecNormalize. Reward for the RL trader = Δcredits (robbery loss prices itself; no hand-tuned penalties).
- The autopilot flies; RL is strategic (semi-MDP gym mode 2: one step = one decision).
- Rust 2024 (gen is reserved). clippy --all-targets -D warnings must stay clean.
`

const CONTEXT = `
WHERE WE ARE (2026-06-10): tactical-flight rung LANDED (PPO flies Newtonian thrust-to-target). Trader rung 1 LANDED+PROVEN (PPO learns contract haulage via gym mode 2 over the live economy; beats random 1.32x AND greedy +0.74 = learned rate-maximization). The trophic-cut-1 spec (${REPO}/docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md) was DEFERRED pending exactly this point; its phase-1 foundations (CraftRole::Pirate, PirateState{food?,notoriety,lie_low_until}, RngStream::Piracy) are in HEAD. Its deferral reason was the risk_appetite scalar premise — that part is retired; the rest (encounter model, locality levers, aliveness discriminator, diagnosis matrix) is reviewed design capital to salvage.

PRIOR NO-GO LESSONS (the ghosts to design against): interdiction-rl (pirates-chase-traffic self-equalizes risk -> reading telemetry worth ~0.4%) and contention-game (24-agent self-averaging field -> ~4% room). Both died of RISK EQUALIZATION. The antidotes (from the trophic spec): locality (bounded pirate mobility, committal travel), persistence/memory (notoriety, lie-low, stale beliefs), food-driven population. The fragile half of the bar is risk staying heterogeneous and persistent.

THE OWNER'S NEW REQUEST (verbatim): "the plan for the pirates -- as part of this consider a side bet on haulers being able to spend their profits on upgrades: tankers (more fuel), additional hulls for their fleet (can carry more stuff/take bigger jobs) and escorts - can fight off weaker pirates. Pirates can also buy these same upgrades with their takings."

TWO RUN REGIMES to keep distinct: (a) GAME runs — long (tens of thousands of ticks), scripted/heuristic population, chronicle-narrated, judged by play + the aliveness discriminator; (b) GYM episodes — the RL trader's semi-MDP episodes (currently horizon 2000 ticks, ~3-4 decisions; a filed follow-up suggests ~5000). The pirates rung's PRIMARY deliverable is (a) alive+game; extending (b) so the trader faces the live risk field fully-observed is the second half. Upgrades likely enter (a) via scripted purchase policies first; whether the RL trader gets upgrade ACTIONS in this rung is a design decision to argue.
`

const SCOUT_SCHEMA = {
  type: 'object',
  required: ['map_md', 'key_symbols', 'constraints', 'gaps'],
  properties: {
    map_md: { type: 'string', description: 'Markdown map of the surface you scouted, max ~250 lines, every claim with file:line' },
    key_symbols: { type: 'array', items: { type: 'object', required: ['path', 'symbol', 'signature', 'notes'], properties: { path: { type: 'string' }, symbol: { type: 'string' }, signature: { type: 'string' }, notes: { type: 'string' } } } },
    constraints: { type: 'array', items: { type: 'string' }, description: 'Hard constraints designers must respect, each with evidence' },
    gaps: { type: 'array', items: { type: 'string' }, description: 'Things that do NOT exist yet that a pirates+upgrades rung would need' },
  },
}

const SCOUT_COMMON = `You are a read-only code scout for the Jumpgate project at ${REPO}. Your output feeds a design panel for the "pirates rung": heuristic pirate predators + a purchasable upgrades economy (tankers=fuel capacity, hulls=cargo/fleet capacity, escorts=combat strength) for both haulers and pirates, on a deterministic 3D Newtonian sim with a contract-haulage economy and an RL trader gym.
${LAWS}
RULES: Every factual claim MUST carry file:line evidence you actually read. NEVER state a symbol/behavior exists without reading it. If something is absent, list it under gaps. Do not propose designs — map reality. Keep map_md under 250 lines.`

const SCOUTS = [
  {
    key: 'economy',
    prompt: `${SCOUT_COMMON}
YOUR SURFACE: ${REPO}/crates/jumpgate-core/src/economy.rs and contract.rs (and ingest.rs where contracts are minted).
Map: the full contract lifecycle states + transitions; escrow/payment flow (who holds credits when, what happens on failure/abandonment — is there a contract-failure path AT ALL?); cargo representation (is cargo a typed good with quantity? capacity limits per craft?); run_scripted_dispatch (REPOST/ASSIGN, the stagger_period==0 ASSIGN gate); update_prices / station stock+price model; miners/sinks (production/consumption); where credits enter and leave the system (is there ANY credit sink today?); resolve_contracts ordering within the tick (pre-physics frame convention, the body_pos(t-1) fix); try_load/try_deliver mechanics. For the upgrades design: where would a station-purchase verb hook in, and what would robbery (cargo loss mid-flight + contract failure) need that doesn't exist?`,
  },
  {
    key: 'stores',
    prompt: `${SCOUT_COMMON}
YOUR SURFACE: ${REPO}/crates/jumpgate-core/src/stores.rs, world.rs, hash.rs, events.rs, ids.rs.
Map: every CraftStore column (name, type, hashed-or-not — check hash.rs folding to be sure which columns fold into state_hash); PirateState exactly as it exists (fields, hashing, where it is read/written — grep usages); CraftRole variants + rank; how craft are minted (config -> world init, can craft be spawned/removed mid-run? is there ANY mid-run spawn/despawn machinery?); the World read accessors added for the trader rung (offered_contracts/station_pos/craft_credits/craft_is_idle); the events.rs chronicle surface (what event kinds exist, how events are emitted/stored, are they hashed?); StationStore/BodyStore essentials; ids.rs ID scheme. For upgrades: where per-craft capability columns live (fuel capacity, cargo capacity, max_thrust, the EffectiveMods seam — Effective=base*mods*wear) and what adding strength/upgrade columns means for hash goldens.`,
  },
  {
    key: 'flight',
    prompt: `${SCOUT_COMMON}
YOUR SURFACE: ${REPO}/crates/jumpgate-core/src/autopilot.rs, ship.rs, integrator.rs, config.rs (and skim ephemeris.rs for body_pos).
Map: the navigation primitives (NavState variants — waypoint? DirectThrust? anything resembling rendezvous/velocity-match/intercept of a MOVING target?); how the autopilot plans+flies a trip (target acquisition, braking law, arrival radius/speed); the fuel/propellant model (capacity, consumption, refuel — does refuel exist anywhere?); craft physical params (mass, max_thrust, exhaust_velocity, where capacities live, the EffectiveMods seam in ship.rs); the integrator's frame conventions; config.rs scenario surface (what is configurable per-run: bodies, craft, dispatch, economy knobs; how TraderCfg/FlightCfg-style config flows in via jumpgate-py configure). CRITICAL QUESTION for the panel: can a pirate craft, with what exists today, (a) fly to and loiter at a fixed point, (b) chase/intercept a moving hauler? Answer with evidence, including what's missing for each.`,
  },
  {
    key: 'gym',
    prompt: `${SCOUT_COMMON}
YOUR SURFACE: ${REPO}/crates/jumpgate-py/src/ (env.rs, obs.rs, lib.rs) and ${REPO}/python/jumpgate/gym_env.py, python/train/*.py, python/tests/test_trader_*.py.
Map: gym mode 2 exactly (configure, the macro-step loop in step_trader — what advances the world, decision-point conditions, TRADER_WAIT_TICKS, horizon/truncation, auto-reset seeding); TRADER_OBS_DIM=20 layout in obs.rs (every dim, scale constants); the action marshalling path (Discrete(5) -> f32 buffer); trader_config_template (scenario: bodies, routes, miners/sinks, seed-derived anomalies); the baselines + training scripts surface; what extending obs with K pirate-contact blocks (bearing unit-vec + log-distance + strength + laden flags...) would touch; what extending the action space (upgrade-purchase actions) would touch; num_envs/num_craft>1 marshalling state (known filed debt). Also: how a long NON-gym "game run" would be driven today — is there a rust binary/python path for running the world N ticks without RL (look for harness/runner/examples, check python/ and crates for main.rs/bin)?`,
  },
  {
    key: 'docs',
    prompt: `${SCOUT_COMMON}
YOUR SURFACE: design capital in ${REPO}/docs. READ IN FULL: docs/superpowers/specs/2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md AND docs/superpowers/plans/2026-06-10-trophic-cut-1-implementation.md (the deferred pirate design — your main job is a faithful digest of EVERYTHING salvageable: the encounter model, locality levers, lie-low/notoriety, food-driven population, the aliveness discriminator + diagnosis matrix, the deferred-to-cut-2 list, and exactly WHICH parts were built at WIP 2e1e1ad vs not — cross-check against HEAD with git log/grep). Also read: docs/superpowers/specs/2026-06-10-trader-rung1-haulage-design.md (sections on env/scenario/changes-by-layer), docs/product/decisions/0006-*.md, docs/superpowers/concepts/media-observability-engine.md (skim — pirates rung precedes media; note only what pirates must NOT foreclose), docs/glossary.md if present. Output: the salvage list (design elements ready to reuse, each with its source section), the explicitly-deferred list, the retired list (risk_appetite-driven decisions), and any contradiction between trophic-cut-1 assumptions and what the trader rung actually built.`,
  },
]

const DESIGN_SCHEMA = {
  type: 'object',
  required: ['design_md', 'decisions', 'open_questions', 'scope_cuts'],
  properties: {
    design_md: { type: 'string', description: 'Full design from your angle, markdown. Concrete: mechanics, numbers/formulas where you can defend them, file-level integration points, the falsifiable discriminator per mechanic.' },
    decisions: { type: 'array', items: { type: 'object', required: ['id', 'decision', 'recommendation', 'alternatives', 'confidence', 'risk'], properties: { id: { type: 'string' }, decision: { type: 'string' }, recommendation: { type: 'string' }, alternatives: { type: 'string' }, confidence: { type: 'string', enum: ['high', 'medium', 'low'] }, risk: { type: 'string' } } } },
    open_questions: { type: 'array', items: { type: 'string' }, description: 'Questions only the owner can answer' },
    scope_cuts: { type: 'array', items: { type: 'string' }, description: 'What you deliberately EXCLUDED from this rung and why' },
  },
}

const DESIGNERS = [
  {
    key: 'ecology',
    mission: `YOU ARE THE ECOLOGY/GAME DESIGNER. Own: the pirate behavior loop (target selection, lurking/locality, engagement, retreat/lie-low), population dynamics (food-driven persistence; how pirates enter/leave the field given whatever spawn machinery exists or doesn't), the locality+persistence levers that keep risk HETEROGENEOUS (the fragile half — design explicitly against the two risk-equalization NO-GOs), notoriety/heat memory, and how the UPGRADES ARMS RACE feeds the ecology: escorts create a strength threshold so pirates pick soft targets -> risk displaces onto the poor/unescorted -> emergent class dynamics; pirate upgrades counter-escalate. Define the rung's success bar (the ALIVE + GAME co-equal properties from the trophic spec, restated for this rung), the aliveness discriminator to build FIRST, the diagnosis matrix, and chronicle legibility (what events make individual pirate/hauler lives narratable). Also: what the boom-bust cycle looks like in SPACE with upgrades in the loop, and the kill criterion for the rung.`,
  },
  {
    key: 'economy',
    mission: `YOU ARE THE ECONOMY DESIGNER. Own: the upgrades catalog exactly as the owner asked (tanker = more fuel; additional hull = carry more stuff / take bigger jobs; escort = fight off weaker pirates) purchasable by BOTH haulers (from profits) and pirates (from takings); pricing and affordability pacing (how many successful trips/robberies buys each tier; should prices be demand-bound like shipyard prices in the old navy design?); credits flow accounting for ROBBERY end-to-end (what exactly does the hauler lose: cargo? escrow? contract penalty? what does the pirate GAIN — how does stolen cargo become pirate credits: sell at any station? a fence discount? does that create a stolen-goods supply shock at the fencing station?); cargo conservation (stolen cargo must not be created/destroyed silently); whether upgrades are a CREDIT SINK (does the economy currently have any sink? wealth accumulation unbounded?); "bigger jobs" — what makes a job bigger (contract size tiers gated on hull capacity?); exploit analysis (upgrade-resale arbitrage, robbery farming, escort-stacking degenerate equilibria, pirate-robs-pirate?); and the falsifiable economic signatures (e.g. escort purchase rate regresses on local robbery rate; wealth distribution tails). Be concrete: propose actual integer price/value numbers anchored to the trader scenario's contract rewards (read the trader spec for the reward scale).`,
  },
  {
    key: 'substrate',
    mission: `YOU ARE THE SUBSTRATE/RL DESIGNER. Own: exact integration with the code as it exists. The encounter mechanic (trigger condition compatible with what the autopilot can actually fly TODAY — if true moving-target interception doesn't exist, design the rung-1 mechanic around loiter/choke-point proximity and say what cut-2 needs; deterministic resolution via RngStream::Piracy: rob/driven-off outcomes with the escort strength threshold "fight off WEAKER pirates"); new state columns (strength/upgrades/fuel-capacity deltas — on which store, hashed how, golden re-baseline plan, single-cause commit sequencing); the purchase verb (dock + deterministic price + atomic credit/capability swap, scripted purchase policy for the population); config surface (scenario knobs for pirate count/mobility/encounter radii/prices, all per-run config not constants where the sweep lab needs them); the GAME-RUN driver (how we actually run 50k-tick chronicle runs + sweeps); the GYM extension (obs: K pirate-contact evidence blocks with fixed global scales — bearing unit-vec, log-distance, strength class, laden flag, stationary by construction; whether route-risk needs anything beyond raw contacts; action space: argue whether the RL trader gets upgrade-purchase actions THIS rung or next; horizon implications); the fleet question (recommend abstract capacity-scalar "additional hull" vs true multi-craft fleets, given num_craft>1 marshalling is filed debt — argue it); test strategy (discriminating tests per mechanic, hash-neutrality vs single-cause golden changes); and a build-order sketch (what lands in what commit-sized chunk).`,
  },
]

const CRITIQUE_SCHEMA = {
  type: 'object',
  required: ['verdict', 'findings', 'strongest_elements'],
  properties: {
    verdict: { type: 'string', enum: ['sound', 'sound-with-fixes', 'needs-rework'] },
    findings: { type: 'array', items: { type: 'object', required: ['severity', 'title', 'evidence', 'fix'], properties: { severity: { type: 'string', enum: ['critical', 'major', 'minor'] }, title: { type: 'string' }, evidence: { type: 'string' }, fix: { type: 'string' } } } },
    strongest_elements: { type: 'array', items: { type: 'string' }, description: 'Elements the synthesis MUST keep' },
  },
}

const CRITICS = [
  {
    key: 'frame',
    prompt: (d, designMd) => `You are the FRAME + GAME-SCIENCE critic for the Jumpgate pirates-rung design (angle: ${d.key}). Repo: ${REPO}.
${LAWS}
${CONTEXT}
Attack this design for: (1) PDR-0006 violations — any reintroduction of the retired presolvability/fraction-of-ceiling/beat-the-optimum gate frame, any "DRL room" justification smuggled in as a build gate; (2) missing or bolted-on falsifiability — each mechanic needs a hypothesis + cheap discriminator BAKED IN; (3) the risk-equalization ghosts — will this design's pirate behavior self-equalize risk like the two NO-GOs did? attack the locality/persistence story specifically; (4) evidence-not-valuations violations in any proposed observation; (5) judged-by-play — does the rung actually produce watchable, narratable, surprising play, or just metrics; (6) does anything here FORECLOSE the media/info layer that comes next (channels must go live WITH their generators; pirates are the generators).
THE DESIGN:\n${designMd}`,
  },
  {
    key: 'feasibility',
    prompt: (d, designMd) => `You are the FEASIBILITY + SCOPE critic for the Jumpgate pirates-rung design (angle: ${d.key}). Repo: ${REPO}. You have tools — VERIFY claims against the actual code with grep/read before accepting them; designers and scouts can both be wrong.
${LAWS}
${CONTEXT}
Attack this design for: (1) substrate reality — does every integration point reference symbols/behaviors that actually exist at the cited locations? Does the encounter mechanic require flight capabilities (moving-target intercept, velocity-match) that autopilot.rs does not have? (2) determinism/hash discipline — integer state, RngStream usage, hashed-column plan, golden re-baseline sequencing as single-cause commits; (3) YAGNI/scope — what here is gold-plating for rung 1? Is the fleet modeled more heavily than "carry more stuff/take bigger jobs" requires? (4) numbers — do proposed prices/horizons/radii actually work against the trader scenario's scales (contract rewards ~1-3cr, episodes 2000 ticks, 4 stations at 0.35-1.1 AU, dt 0.25)? Do the math where the design didn't. (5) test strategy — are the proposed tests discriminating (would fail if the mechanic were broken)? (6) build order — can it land in reviewable, individually-green commits?
THE DESIGN:\n${designMd}`,
  },
]

phase('Scout')
log('5 scouts mapping economy, stores/hash, flight, gym, and the deferred trophic-cut-1 design capital')
const scoutResults = await parallel(SCOUTS.map(s => () =>
  agent(s.prompt, { label: `scout:${s.key}`, phase: 'Scout', schema: SCOUT_SCHEMA })))

const scouts = scoutResults.map((r, i) => ({ key: SCOUTS[i].key, r })).filter(x => x.r)
if (scouts.length < 3) throw new Error('too many scouts failed: ' + scouts.length + '/5')

// Barrier justified: every designer needs ALL surface maps.
const factSheet = scouts.map(({ key, r }) =>
  `## SCOUT ${key}\n${r.map_md}\n\n### Key symbols\n${r.key_symbols.map(k => `- ${k.path} :: ${k.symbol} :: ${k.signature} — ${k.notes}`).join('\n')}\n\n### Hard constraints\n${r.constraints.map(c => `- ${c}`).join('\n')}\n\n### Gaps (does not exist yet)\n${r.gaps.map(g => `- ${g}`).join('\n')}`
).join('\n\n---\n\n')

log(`fact sheet assembled from ${scouts.length}/5 scouts (${Math.round(factSheet.length / 1000)}k chars)`)

phase('Design')
const designed = await pipeline(
  DESIGNERS,
  d => agent(
    `You are a senior game-systems designer on the Jumpgate project (${REPO}).
${LAWS}
${CONTEXT}
${d.mission}

THE FACT SHEET (assembled by 5 code scouts; every claim carries file:line — trust it over your priors, and verify with the tools you have if something is load-bearing):
${factSheet}

OUTPUT DISCIPLINE: design_md is your full design. Be concrete enough that a spec-writer can lift sections verbatim: mechanics with exact trigger conditions and formulas, integer constants with units and a one-line defense of each number, file-level integration points, and per-mechanic falsifiable discriminators. Mark anything you could not ground in the fact sheet as ASSUMPTION. Do NOT fabricate symbols — if you need something that doesn't exist, name it as NEW and say where it goes. Stay inside rung-1 scope; push everything else to scope_cuts with a reason.`,
    { label: `design:${d.key}`, phase: 'Design', schema: DESIGN_SCHEMA }),
  (design, d) => {
    if (!design) return null
    return parallel(CRITICS.map(c => () =>
      agent(c.prompt(d, design.design_md), { label: `crit:${c.key}:${d.key}`, phase: 'Critique', schema: CRITIQUE_SCHEMA })))
      .then(crits => ({ designer: d.key, design, critiques: crits.filter(Boolean).map((cr, i) => ({ lens: CRITICS[i].key, ...cr })) }))
  }
)

const results = designed.filter(Boolean)
log(`${results.length}/3 designs complete with critiques`)

return {
  factSheet,
  results: results.map(x => ({
    designer: x.designer,
    design_md: x.design.design_md,
    decisions: x.design.decisions,
    open_questions: x.design.open_questions,
    scope_cuts: x.design.scope_cuts,
    critiques: x.critiques,
  })),
}