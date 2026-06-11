export const meta = {
  name: 'media-rung-design',
  description: 'Design the media/gossip rung (cut 1) — grounded panel, six design lenses, adversarial critics, synthesis',
  phases: [
    { title: 'Ground', detail: '5 parallel code/doc readers produce citation-grade briefs' },
    { title: 'Design', detail: '6 independent designers, one lens each' },
    { title: 'Critique', detail: 'adversarial critic per design (pipelined)' },
    { title: 'Synthesize', detail: 'consensus map + divergences + owner decision points' },
  ],
}

// ============ THE AUTHORITY TEXT (owner design sessions, banked 2026-06-10) ============
const AUTHORITY = `
MEDIA / INFO-LAYER DESIGN AUTHORITY (owner sessions 2026-06-10 — this is the design intent; deviate only with argument):

KEYSTONE SEAM: alerts carry EVIDENCE, never VALUATIONS. Obs = who/what seen, claimed magnitude, where, when. Never "region heat 0.73", never threat/opportunity tags, never confidence scores. Valence is role-relative (pirate gossip repels haulers, attracts police, mixed for rival pirates) and is assigned by each role's REWARD stream (delta-credits/delta-food does the conditioning).

ENCODINGS: anchor rumors to NAMED entities (stations/lanes/coarse sectors — chronicle-legible). Per-alert geometry = ego-relative bearing unit-vec + log-distance, fixed global scales (stationary-obs law). Age = raw log-scaled feature, NEVER pre-decayed — staleness curves are goal-dependent and must be LEARNED.

(TRUTH, COVER) PAIRS: every media artifact keeps TWO snapshots — the ground-truth event and the alert's CLAIMED version; any identifier can be wrong or missing. Agents see only the cover; truth stays in the run record (the lab join makes every media discriminator measurable). Cover = seeded transform of truth (RngStream::Media — misinformation replays bit-identically); magnitude inflation is one corruption channel. Unlocks (mostly LATER cuts): heat lands on the CLAIMED identity (anonymity, false flags), mistaken-identity arcs, per-source trust learned (tokens carry channel id), info quality as a priced good.

BIG-SCORE WRINKLE = a MEDIA property: propagation probability proportional to excitement (claimed magnitude), not just recency — agents receive a heavy-tailed stale-biased rumor stream and must learn calibrated skepticism from experienced disappointment (the big score must occasionally be REAL).

TWO VECTORS — AIR and LAND. AIR = radio/quantum, instant within COVERAGE radius, reserved for the most MUNDANE (price feeds, contract boards — the trader gym's board obs was always air traffic) and the most CRITICAL (distress maydays with TRUE location, navy bulletins). The bimodality is the anti-ghost FIREWALL: the interesting middle band (sightings/rumors/reputations) is structurally exiled to LAND, stable in-fiction because airtime is ATTRIBUTABLE (criminals' information walks). Coverage radius gives core/frontier an INFORMATION mechanism: core maydays summon response, frontier screams go unheard. Quantum point-to-point = later. Air ~ verified/truthful; land carries the (truth,cover) corruption.

GOSSIP SYSTEM = EPIDEMIOLOGY: stations are reservoirs, ships are vectors, significance is virulence. Every node (ship/station; a gossip_buffer capability column) holds a BOUNDED store of (cover) alerts + {first_heard, hops, source}; eviction = significance-weighted age (the buffer IS the forgetting). On contact (dock / comms range): per-item transfer P = clamped significance fn (major ~ 1.0, minor small), seeded on RngStream::Media; per-hop identifier degradation (telephone game) + per-hop inflation of exciting items. Hashed, integer, deterministic eviction. Pre-registered: P(escape station) = 1-(1-p)^(visits within retention) so reach = significance x traffic. Emergents/discriminators: (1) news fronts travel at ship speed along trade lanes; (2) two regimes from one mechanism — minor stays local, major becomes common knowledge; (3) DEAD LANES CARRY NO NEWS: effective blockades silence their own reporting; (4) hubs are rumor mills, backwaters are news deserts — docking buys a database refresh. The pirates-rung dock-gated RouteEvidence is the degenerate single-reservoir version; route_evidence(reader, ...) is the swap seam.

AVERSIVE SLICE: successful avoidance generates NO signal. Mitigations (diegetic): (1) media is the in-world sample-efficiency mechanism (one agent's robbery becomes everyone's obs); (2) costs arrive through currency (robbery = cargo+contract loss; death = horizon truncation under gamma~1 — never a hand-tuned death penalty); (3) state-dependent risk attitude free via hold-value/fuel in obs. Falsifiable signature: DETOUR LENGTH REGRESSES ON CARGO VALUE.

MEDIA HEAD: every captain gets the SAME architectural module — attention/deep-set encoder over its personal comms-log feeding the policy trunk; role divergence ONLY via reward ("set the head and see what comes out"). Alert tokens carry NAMED ENTITIES as first-class evidence (per-episode-stable entity-slot embeddings so the-same-pirate is recognizably same across alerts); propagation must preserve attribution (who), not just place. Pre-registered discriminator: detour P regresses on per-entity mention frequency x proximity with zero shaped reward; identity-SHUFFLE ablation must collapse it.

COMMITMENT-CONDITIONING: the agent's CURRENT CONTRACT is a goal-conditioning token sharing the per-episode entity-embedding space with rumor tokens, so "rumor about MY target" is an attention similarity match, never an is_my_target flag. Laden-hauler-reads-route-risk = the same pattern. The moving-target rumor SEQUENCE (order implies direction) is the first honest LSTM-rung candidate — LSTM earns entry only on a demonstrated expressiveness gap.

ARCHITECTURE LADDER: Rung A = bounded ship comms-log of alert tokens + attention/deep-set pooling + MLP (log retention rule MIRRORS network propagation rule: one mechanism, two scales). Rung B (LSTM) gated on demonstrated gap.

FAILURE MODES TO DESIGN AGAINST: (1) dense/accurate alerts => fully-observed again => self-averaging field => NO-GO; media must be SPARSE, LOCAL, LOSSY (hear near you; refresh by docking — information is a priced resource). (2) Channels that go live BEFORE their generators exist teach sticky learned blindness — channels go live WITH their ground truth. (3) Kill criterion baked in: ablate media for one agent, measure income drop = the value of information; ~0 => ecology not load-bearing, fix world before architecture.

SPATIAL BOOM-BUST CLOSES THROUGH THIS LOOP (the owner's minimum-game bar): police/risk gossip displaces pirates -> hauler risk-gossip ages out -> haulers return -> prey gossip lights up -> pirates return; lag set by gossip staleness + travel time. NOTE: police do NOT exist yet — cut 1 closes whatever subset of this loop its live generators support.

FRESH EMPIRICAL INPUT (2026-06-11): the rung-1 PPO contact-ablation returned a NULL — contact-aware == zero-masked within noise at the current band (robbery costs ~5-20cr/ep both arms; avoidance not worth learning at these prices). This is the aversive-slice prediction confirmed. The media rung is the designed fix, and the fix is WORLD-PRICE + INFORMATION, never shaping. Cut-1 design should say what (if anything) about prices/density it changes so information can matter, and must keep that change honest (world mechanic, not reward).
`;

// ============ PROJECT LAWS (every agent gets these) ============
const LAWS = `
PROJECT LAWS (violations are critique-severity CRITICAL):
- PDR-0006 (docs/product/decisions/0006-*): Jumpgate is a GAME judged by EMERGENT PLAY. Metrics/diagnostics are designer's WINDOWS, never gates; ablation results are REPORTED never gated; DRL is a PLAYER. Never reintroduce presolvability/fraction-of-ceiling gating.
- NO-SHAPING KEYSTONE: reward = currency only (delta-credits / delta-food). A shaping urge is a diagnostic; the answer is a world mechanic (price it), a new head/module (make it learnable), or nothing (watch for it). New capacity (e.g. LSTM) requires a demonstrated expressiveness gap.
- EVIDENCE-NOT-VALUATIONS: observations carry claims (who/what/where/when/claimed-magnitude), never heat scores, threat tags, or confidence scalars. No per-craft taste scalars (the risk_appetite ghost is retired).
- DETERMINISM: integer state, hashed (HASH_FORMAT_VERSION=4, GOLDEN_ZERO_STATE_HASH=0xafdc_5c35_6266_0ff0; GOLDEN_CONFIG_HASH=0x1798_b108_edae_5bb6). Any state/config-shape change = its own single-cause golden commit; literals re-derived via print_golden, never invented. Randomness only via RngStream (fixed salt per stream; new stream = new fixed salt, existing salts untouched). Replay must be bit-identical.
- STATIONARY-OBS LAW: fixed global scales, ego-relative geometry, raw log-scaled age (never pre-decayed), no VecNormalize.
- NO LINGERING ABSTRACTIONS: any simplification (cf. the fleet-ledger UpgradeLevels) must name its sunset — what real system replaces it and at which rung. Fleets are "collections of ships with a single policy as a strategic head", never a level stat.
- Channels go live WITH their generators (no police gossip before police exist).
- Sparse / local / lossy or the game dies (anti self-averaging; the contention-game NO-GO lesson).
- Rust 2024 ('gen' is reserved), clippy --all-targets. TDD. Frequent small commits.
- Narrative-event/state-surgery interfaces (spec §14.3/§14.4) are FUTURE commitments — design must not block them but must not build them.
`;

const GROUND_SCHEMA = {
  type: 'object',
  required: ['brief', 'key_symbols', 'constraints'],
  properties: {
    brief: { type: 'string', description: 'Dense factual brief of what exists, how it works, exact mechanics. 400-900 words.' },
    key_symbols: { type: 'array', items: { type: 'object', required: ['file', 'symbol', 'note'], properties: { file: { type: 'string' }, symbol: { type: 'string' }, note: { type: 'string' } } } },
    constraints: { type: 'array', items: { type: 'string' }, description: 'Hard constraints a media design must respect, each with file:line evidence.' },
  },
};

const DESIGN_SCHEMA = {
  type: 'object',
  required: ['lens', 'cut1_scope', 'proposal_md', 'decision_points', 'deferred', 'risks'],
  properties: {
    lens: { type: 'string' },
    cut1_scope: { type: 'string', description: 'One paragraph: exactly what cut 1 builds.' },
    proposal_md: { type: 'string', description: 'The full design proposal, markdown. Concrete: data structures, algorithms, stage order, knobs with starting values, events, windows. Cite real symbols from the grounding pack.' },
    decision_points: { type: 'array', items: { type: 'object', required: ['question', 'options', 'recommendation'], properties: { question: { type: 'string' }, options: { type: 'array', items: { type: 'string' } }, recommendation: { type: 'string' } } } },
    deferred: { type: 'array', items: { type: 'string' }, description: 'What this lens explicitly pushes to cut 2/3, with the trigger that pulls it in.' },
    risks: { type: 'array', items: { type: 'string' } },
  },
};

const CRITIQUE_SCHEMA = {
  type: 'object',
  required: ['findings', 'strongest_elements', 'verdict'],
  properties: {
    findings: { type: 'array', items: { type: 'object', required: ['severity', 'issue', 'evidence', 'fix'], properties: { severity: { type: 'string', enum: ['CRITICAL', 'MAJOR', 'MINOR'] }, issue: { type: 'string' }, evidence: { type: 'string' }, fix: { type: 'string' } } } },
    strongest_elements: { type: 'array', items: { type: 'string' }, description: 'Elements the synthesis should keep even if the overall design loses.' },
    verdict: { type: 'string' },
  },
};

const SYNTH_SCHEMA = {
  type: 'object',
  required: ['consensus_md', 'divergences', 'recommended_cut1_md', 'owner_decision_points'],
  properties: {
    consensus_md: { type: 'string', description: 'What all/most designs agree on, post-critique.' },
    divergences: { type: 'array', items: { type: 'object', required: ['topic', 'positions', 'recommendation'], properties: { topic: { type: 'string' }, positions: { type: 'string' }, recommendation: { type: 'string' } } } },
    recommended_cut1_md: { type: 'string', description: 'The synthesized cut-1 design, markdown, concrete enough to spec from.' },
    owner_decision_points: { type: 'array', items: { type: 'object', required: ['id', 'question', 'options', 'recommendation'], properties: { id: { type: 'string' }, question: { type: 'string' }, options: { type: 'array', items: { type: 'string' } }, recommendation: { type: 'string' } } } },
  },
};

// ============ PHASE 1: GROUND ============
phase('Ground')
log('Grounding: 5 readers over code seams + authority docs')

const COMMON = `You are grounding a design panel for the MEDIA/GOSSIP rung of Jumpgate (repo /home/john/jumpgate, branch jumpgate-v1-design). Report ONLY what you verify by reading files — cite file:line. Never invent symbols, never claim tests ran. Your brief feeds designers who will NOT read the code themselves, so be precise and complete on your assigned surface.\n${LAWS}`;

const grounds = await parallel([
  () => agent(`${COMMON}
SURFACE 1 — the evidence seam and the heuristic brains. Read crates/jumpgate-core/src/pirate.rs (1625 lines) fully. Document: RouteEvidence (rob-tick rings, cursor), how Robbed settlements write it, World::route_evidence(reader, route) dock-gated read (info_tick refresh semantics), how hauler and pirate brains currently consume (or ignore) evidence, the hunger-gated relocation / haven-exclusion / marooned-breakout logic, lurk/engage mechanics (engage_radius, velocity-match, strength filter, p_rob_milli, ransom flow), lie-low/notoriety-equivalents if any, and the brain decision cadence. Also read the hauler brain (wherever contract accept/detour decisions live — grep for AcceptContract / run_hauler or similar in crates/jumpgate-core/src/). Constraints: what would a gossip layer have to plug into so route_evidence is the degenerate single-reservoir version it is documented to be?`, { label: 'ground:evidence-seam', phase: 'Ground', schema: GROUND_SCHEMA }),
  () => agent(`${COMMON}
SURFACE 2 — the deterministic substrate. Read crates/jumpgate-core/src/stores.rs (588 lines), crates/jumpgate-core/src/hash.rs, crates/jumpgate-core/src/rng.rs, crates/jumpgate-core/src/config.rs (955 lines), and the step/stage order in crates/jumpgate-core/src/world.rs (read the step() function and reset() fully; the file may be large — read what matters). Document: store/column layout conventions (SoA, Option columns, capability columns), how state hashing covers stores (what is hashed, in what order, HASH_FORMAT_VERSION discipline), RngStream salt pattern and how draws derive (master ^ salt, tick/row mixing?), TrophicCfg + ShipyardCfg knob conventions + apply_knob surface, the full stage order of World::step (numbered stages — physics, deliveries, encounters 3b2, failures, purchases 1d, brains...), and how a new store (per-node bounded gossip buffer) would enter hashing + reset + (de)serialization. Constraints: exactly what a new gossip_buffer column + RngStream::Media + new TrophicCfg knobs must do to keep the two-golden single-cause discipline.`, { label: 'ground:substrate', phase: 'Ground', schema: GROUND_SCHEMA }),
  () => agent(`${COMMON}
SURFACE 3 — events, chronicle, diagnostics lab. Read crates/jumpgate-core/src/events.rs (301 lines), crates/jumpgate-core/src/diagnostics.rs, crates/jumpgate-core/examples/trophic_run.rs, python/analysis/sweep_trophic.py. Document: every EventKind variant (these are the candidate gossip GENERATORS — list each with payload fields), recent_events retention semantics, TrophicSample fields + WINDOW_TICKS + classify()/Verdict (incl. PermanentPeace precedence and the positive-control discipline), the chronicle printer (per-craft grouping, repeat-collapse), the RESULT line format the sweep aggregator parses, and the jsonl window schema. Constraints: how a media rung should EXTEND the sample/verdict/chronicle surfaces (e.g. knowledge-front measurement needs the truth join — say what data the run record already has vs needs).`, { label: 'ground:lab', phase: 'Ground', schema: GROUND_SCHEMA }),
  () => agent(`${COMMON}
SURFACE 4 — the gym and training stack. Read crates/jumpgate-py/src/env.rs and obs.rs (or wherever the 34-dim obs is built — find TRADER_OBS_DIM), python/jumpgate/*.py env wrappers, python/tests/test_trader_pirates_mode.py, python/train/eval_pirates_ablation.py and any train scripts. Document: the exact obs layout (20 base + 2x7 contact blocks — every feature, scale, and the stationary-obs conventions), action space (Discrete(5) semantics), reward (delta-credits semi-MDP mode), episode/horizon mechanics, num_pirates kwarg, seed handling (held-out eval convention), and how the ablation script masks contacts. Constraints: how a variable-length comms-log obs could attach (Dict obs? fixed K token slots with presence flags? where SB3 PPO constrains us), and what per-episode-stable entity-slot embeddings require from the env side (entity ids stable within episode, what ids exist today).`, { label: 'ground:gym', phase: 'Ground', schema: GROUND_SCHEMA }),
  () => agent(`${COMMON}
SURFACE 5 — authority documents. Read docs/product/decisions/0006-*.md, docs/superpowers/specs/2026-06-10-pirates-rung1-predation-and-upgrades-design.md (ALL of §14 forward commitments and §15 decisions — list each commitment), docs/superpowers/concepts/crimes.md (skim for the information/Suppress/jamming-relevant parts and the crime-information interactions), docs/product/vision.md + metrics.md if present, docs/glossary.md. Document: the PDR-0006 frame in its own words, every §14 forward commitment that constrains or feeds media (esp. first-class goods §14.6, effects-not-shadows, narrative quarantine, state surgery, capability mixins, crimes adoption §14.9), the §15 fleet-ledger caveat verbatim, and what crimes.md says about information/evidence/heat that the media design should adopt or explicitly defer. Constraints list = the binding commitments.`, { label: 'ground:docs', phase: 'Ground', schema: GROUND_SCHEMA }),
])

const g = grounds.filter(Boolean)
if (g.length < 4) { throw new Error(`grounding too thin: ${g.length}/5 briefs`) }
log(`Grounding complete: ${g.length}/5 briefs`)

const PACK = g.map((x, i) => `=== GROUNDING BRIEF ${i + 1} ===\n${x.brief}\nKEY SYMBOLS:\n${x.key_symbols.map(s => `- ${s.file} :: ${s.symbol} — ${s.note}`).join('\n')}\nCONSTRAINTS:\n${x.constraints.map(c => `- ${c}`).join('\n')}`).join('\n\n')

// ============ PHASE 2+3: DESIGN -> CRITIQUE (pipelined) ============
const LENSES = [
  { key: 'substrate', brief: `LENS: SUBSTRATE DETERMINIST. Design the deterministic gossip core: the gossip_buffer store (bounded, integer, hashed — exact struct/column layout and capacities), alert artifact representation ((truth, cover) snapshots — exact fields incl. entity refs, claimed magnitude, place anchor, tick, hops, source/channel), generation (which existing EventKind variants spawn alerts, at which stage), propagation on contact (dock and/or comms-range — exact transfer-P integer math on RngStream::Media), per-hop degradation/inflation transforms (seeded, integer), significance-weighted eviction (deterministic tie-breaks), the stage-order insertion point(s) in World::step, the migration path that makes RouteEvidence the degenerate version it is documented to be (does route_evidence(reader, route) get reimplemented over gossip buffers? does the old store retire? name the sunset), new TrophicCfg knobs with starting values + apply_knob entries, hashing/golden impact (single-cause commits enumerated), and replay/determinism tests. Integer math only; no floats in state.` },
  { key: 'play', brief: `LENS: GAME DESIGNER (play-first, PDR-0006). Design what the WORLD DOES with gossip in cut 1 so the rung is judgeable by play at the console: do heuristic hauler brains act on rumor (detour/route choice/refuse contract)? do pirates use prey-gossip to pick lurks (replacing or augmenting their current draw)? Which arcs become watchable (the rumor that emptied a lane; the backwater that never heard; the big-score rumor stampede)? How does the boom-bust-through-information lag manifest with NO police (pirates displace on hauler-drought gossip? haulers return when risk-gossip ages out?)? What chronicle lines must exist for the owner to READ these arcs? What does the scenario band need (traffic levels, station count, retention) for news fronts to be visible at 50k ticks? Anchor every consumption rule in evidence-not-valuations (brains read claims and decide with their OWN thresholds — where do those thresholds live without becoming taste scalars? They are role policy constants in TrophicCfg, like existing brain constants — argue it). State explicitly which half of the boom-bust bar cut 1 can close with live generators only.` },
  { key: 'artifact', brief: `LENS: MEDIA-ARTIFACT DESIGNER (truth/cover, vectors, channels). Design the alert artifact and channel taxonomy for cut 1: which generators are live TODAY (Robbed, DrivenOff, HaulerKilled, UpgradePurchased, contract events, prices...) and which alerts they spawn; the AIR/LAND split in cut 1 (argue: contract board obs = already-live air-mundane; is there ANY new air in cut 1 — e.g. a mayday on HaulerKilled with true location — or does air-critical wait for responders that can act on it, per channels-live-WITH-consumers-too?); the (truth, cover) pair in cut 1 — recommend exactly which corruption channels go live at cut 1 (identity error? magnitude inflation? place blur? or truth==cover with the SCHEMA in place and corruption knobs at zero?) and defend against both ghost-channel risk (corruption before trust-learning exists) and schema-rot risk (retrofitting truth/cover later = state surgery); excitement/significance assignment (integer scale, from event payloads — cargo value robbed, ransom size, kill); attribution preservation (who-refs through hops); how heat-on-claimed-identity defers cleanly. Be concrete: field tables, integer scales, per-channel base transfer probabilities.` },
  { key: 'gym', brief: `LENS: RL/GYM ARCHITECT. Design the gym surface for cut 1: the comms-log obs (K alert-token slots — exact per-token features honoring stationary-obs law: presence, channel/vector tag, entity-slot id, place bearing+log-dist, claimed-magnitude log-scaled, raw log age, hops), per-episode-stable entity-slot embedding scheme (what the env must guarantee about ids), the media head Rung A (deep-set/attention pooling -> trunk; same module every captain), commitment-conditioning (contract token sharing the entity-embedding space), what stays OUT of cut 1 (LSTM gated on demonstrated gap; which gap test), and the discriminator suite: value-of-information ablation (mask comms-log; the kill criterion — but REPORTED never gated per PDR-0006), identity-shuffle ablation, detour-vs-cargo-value regression, act-on-rumor-P vs age per role. CRITICAL fresh input: the contact-ablation NULL means at current prices information is worthless to a PPO trader — state what world conditions (pirate density per scenario, cargo value at risk, route asymmetry) cut 1 must create for the value-of-information to be plausibly nonzero, as WORLD config not shaping, and how the eval would detect it either way. Also: does the gym ride cut 1 at all, or does cut 1 land substrate+console first and gym follows as cut 1.5 once the band is tuned (sequencing argument either way)?` },
  { key: 'lab', brief: `LENS: LAB SCIENTIST (windows, discriminators, controls — never gates). Design the measurement surface: TrophicSample extensions (alerts alive per node class, mean age, hop distribution, per-route knowledge counts...), the truth-join (run-record table of (truth, cover, first_heard per node) enabling knowledge-front maps — time-to-knowledge vs traffic-weighted graph distance, pre-registered P(escape)=1-(1-p)^visits check), new Verdict considerations (does classify() need media-aware verdicts — e.g. CommonKnowledge/NewsDesert as DIAGNOSES not gates — or do we window-only at cut 1 and keep verdicts untouched? recommend), chronicle extensions (gossip events as chronicle lines — which are watchable vs spam; repeat-collapse lessons), positive controls for the gossip instrument (a disease injection that MUST read a known verdict/window shape — e.g. transfer-P=0 must read news-desert everywhere; transfer-P=1000 retention=huge must read instant common knowledge — design 2), the instrument-kill rule applied to media metrics, and sweep_trophic.py extensions. Remember the seed-7 lesson: instruments lie; every new metric ships with a labeled synthetic.` },
  { key: 'scope', brief: `LENS: SCOPE-CUTTER / SEQUENCER. Produce the minimal cut-1 that is honestly play-judgeable, and the explicit cut ladder. Pressure-test everything the other lenses would add: does cut 1 need comms-range transfer or is dock-only enough (ships exchange at stations only)? Does it need cover-corruption live or schema-only? Does it need the gym at all? Does it need new air channels? Apply YAGNI ruthlessly BUT respect channels-live-WITH-generators and the schema-rot/state-surgery cost of deferring truth/cover schema. Sequence against the existing backlog: the cut-2 refuel package (tanker, priced fuel, escrow-lock bug jumpgate-2c0c2d92bb), fence/value-seeking pirates, police/navy/FOB/bounties, the heterogeneity-metric calibration (jumpgate-50c6a8a3bd), chase/tether. State what media cut 1 DEPENDS on (anything? or pure additive on pirates rung 1?), what it UNBLOCKS, and the recommended build order with reasons. Also pressure-test the opposite failure: a cut so minimal it cannot show a news front or change any decision — name the minimum bar (the play-judgeable claim) and check the cut clears it.` },
]

const DESIGN_PROMPT = (lens) => `You are ONE designer on an independent panel designing MEDIA RUNG CUT 1 for Jumpgate. Other designers have other lenses; you own yours — go deep, be opinionated, make concrete calls (exact structs, integer scales, starting knob values, stage numbers). You may read repo files to verify details, but the grounding pack below is citation-grade.

${LAWS}
${AUTHORITY}

${lens.brief}

CONTEXT — what exists (verified grounding):
${PACK}

Produce your design per the schema. decision_points = ONLY genuinely owner-level calls (design-direction forks, in-fiction commitments, scope bets) — not engineering choices you should make yourself.`

const CRITIQUE_PROMPT = (design) => `You are an adversarial design critic for Jumpgate's media rung. CRITIQUE, do not redesign. Hunt these failure modes, each a finding with evidence from the proposal text (quote it) and a concrete fix:
1. SELF-AVERAGING: alerts too dense/accurate/global -> fully-observed -> dead game (the contention-game NO-GO).
2. GHOST CHANNELS: a channel live before its generator OR its consumer exists -> sticky learned blindness.
3. SHAPING LEAK: any valuation/tag/confidence in obs; any reward term beyond currency; any taste scalar; any behavior shaped in rather than emerging.
4. DETERMINISM/HASH violations: floats in state, un-seeded choice, unhashed new state, golden discipline broken, eviction with nondeterministic ties, replay divergence risk.
5. LINGERING ABSTRACTION: a simplification with no named sunset (the fleet-as-level trap).
6. STATIONARY-OBS violations: pre-decayed age, normalized-by-running-stats features, non-ego geometry.
7. SCOPE: bloat (YAGNI violations; building for absent generators/consumers) AND anorexia (cut too thin to be play-judgeable — cannot show a news front or change any decision).
8. FABRICATION: claims about existing code that contradict the grounding pack (quote the pack).
9. PLAY BLINDNESS: no watchable arc, no chronicle surface, owner cannot judge it at the console.
10. MEASUREMENT THEATRE: metrics with no labeled synthetic / positive control; or metrics that smell like GATES (PDR-0006 violation).
Also list strongest_elements worth keeping regardless of verdict.

${LAWS}

GROUNDING PACK (the code truth):
${PACK}

THE PROPOSAL (lens: ${design.lens}):
CUT-1 SCOPE: ${design.cut1_scope}
${design.proposal_md}
DECISION POINTS: ${JSON.stringify(design.decision_points)}
DEFERRED: ${JSON.stringify(design.deferred)}`

log('Design panel: 6 lenses, critics pipelined behind each')
const designed = await pipeline(
  LENSES,
  (lens) => agent(DESIGN_PROMPT(lens), { label: `design:${lens.key}`, phase: 'Design', schema: DESIGN_SCHEMA }),
  (design, lens) => design
    ? agent(CRITIQUE_PROMPT(design), { label: `critique:${lens.key}`, phase: 'Critique', schema: CRITIQUE_SCHEMA }).then(c => ({ lens: lens.key, design, critique: c }))
    : null,
)

const panel = designed.filter(Boolean).filter(p => p.design)
log(`Panel complete: ${panel.length}/6 designs (critiques: ${panel.filter(p => p.critique).length})`)
if (panel.length < 4) { throw new Error(`panel too thin: ${panel.length}/6 designs survived`) }

// ============ PHASE 4: SYNTHESIZE ============
phase('Synthesize')
const panelText = panel.map(p => `
########## LENS: ${p.lens} ##########
CUT-1 SCOPE: ${p.design.cut1_scope}
${p.design.proposal_md}
DECISION POINTS: ${JSON.stringify(p.design.decision_points, null, 1)}
DEFERRED: ${JSON.stringify(p.design.deferred, null, 1)}
RISKS: ${JSON.stringify(p.design.risks, null, 1)}
--- CRITIQUE ---
${p.critique ? `VERDICT: ${p.critique.verdict}\nFINDINGS:\n${p.critique.findings.map(f => `[${f.severity}] ${f.issue} | evidence: ${f.evidence} | fix: ${f.fix}`).join('\n')}\nSTRONGEST: ${p.critique.strongest_elements.join(' | ')}` : '(critic unavailable)'}
`).join('\n')

const synth = await agent(`You are the synthesis lead for Jumpgate's media-rung design panel. Six designers (lenses: substrate determinism, play, media artifact, RL/gym, lab, scope) each proposed cut-1 designs; each was adversarially critiqued. Your job: produce the consensus map, the genuine divergences (with the critique evidence on each side), a single recommended cut-1 design merged from the strongest elements (resolve engineering-level conflicts yourself, with reasons), and the OWNER decision points — deduplicated across designers, only genuinely owner-level calls, each with options and a recommendation. Respect every CRITICAL critique finding: either the synthesis fixes it or you list it as an open risk with reasoning. Do NOT invent code facts beyond the grounding; do not water divergences into mush — where designers genuinely disagree (e.g. heuristic-brains-consume-gossip-now vs substrate-first; corruption-live vs schema-only; gym-now vs cut-1.5), present the fork sharply.

${LAWS}
${AUTHORITY}

GROUNDING PACK:
${PACK}

THE PANEL (designs + critiques):
${panelText}`, { label: 'synthesize', phase: 'Synthesize', schema: SYNTH_SCHEMA })

return {
  grounding_constraints: g.flatMap(x => x.constraints),
  panel_summary: panel.map(p => ({ lens: p.lens, scope: p.design.cut1_scope, critical_findings: (p.critique?.findings || []).filter(f => f.severity === 'CRITICAL').map(f => f.issue) })),
  synthesis: synth,
}