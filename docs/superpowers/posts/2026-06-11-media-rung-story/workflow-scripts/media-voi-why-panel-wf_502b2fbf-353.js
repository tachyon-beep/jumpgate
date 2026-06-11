export const meta = {
  name: 'media-voi-why-panel',
  description: 'Systems-thinking + simulation expert panel: why does gossip neither beat nor lose to the oracle ring, and what drove the apparent negative VoI',
  phases: [{ title: 'Panel' }],
}

const DATA = `
CONTEXT — Jumpgate v1 (deterministic Newtonian space life-sim, PDR-0006: a GAME judged by emergent play; metrics are designer windows, NEVER gates; fixes must be world mechanics/prices, never reward shaping).

THE WORLD (scenario_trophic band, console-baked): 6 stations on orbits 0.35-1.4 AU, 12 scripted haulers (belief-scored contract choice), 6 pirates (hunger-driven lurkers; rob_cooldown 600 ticks, hunger-gated relocation every ~2500 ticks, lie-low refuges, escort/hull arms race both sides). Contract rewards 1.0M/2.3M/3.9M micros by tier; ransom on robbery = min(wallet, 6M); a robbed hauler also loses the trip. Belief-scored ASSIGN: score = reward x (1000 - min(rob_count x 150, 900))/1000 over evidence for the route; evidence retention window 4000 ticks.

THE NEW MEDIA LAYER (just landed, replay-bit-identical): robbery mints a gossip alert carried by the victim; stations are reservoirs, haulers are vectors; transfer only on dock-visit EDGES, one RNG draw per novel item, P = significance x 0.85^hops; retellings inflate claimed value x1.125/hop (net decay 0.956/hop); per-reader forgetting (4000 ticks from when THAT reader heard). Pirates are information-blind. When media is ON, the ASSIGN evidence read switches from the legacy RING (a global per-route rob log, instantly complete, read dock-gated) to each hauler's OWN gossip buffer (only what it personally heard).

MEASURED (50k-tick runs, seeds 7/23/42/99, all arms identical except the evidence source):
ARM A "blind" (belief scoring OFF — no avoidance): hauler median per seed 39.95/16.35/26.90/55.85M, POOLED MEDIAN 36.25M; robs 46/59/58/17.
ARM B "ring" (avoidance via the global ring): 53.40/49.70/67.20/63.35M, POOLED 56.55M; robs 39/66/41/35.
ARM C "gossip" (avoidance via personal gossip): 72.60/4.95/64.70/46.55M, POOLED 55.40M; robs 6/74/39/55.
Pirate median wallets, same order — blind: 5.0/26.95/19.75/2.5M; ring: 0/21/4/3M; gossip: 0/39.95/2.5/3.5M.
Verdict flips across arms on the SAME seed: s7 gossip -> PermanentPeace (war ends, peace dividend 72.6M); s99 blind -> PermanentPeace; all else Alive (boom-bust).
CONTROL: s7's gossip-arm peace persists under M-DEAD (gossip live but carries nothing -> zero avoidance signal) — so that flip is the read-SOURCE swap perturbing a seed near the escort-ladder peace basin, not gossip content.
PROPAGATION INSTRUMENTS (gossip arm, defaults): saturation = 6/6 alerts reach >800 permille of all craft (pre-registered expectation was single-digit permille) — 2/4 seeds read CommonKnowledge; median first-hearing lag 2669 ticks, p90 6450 (vs pirate relocation ~2500, rob cooldown 600, retention 4000); hub/backwater news ratio 2.5-3.0; avoidance lag (first hearing -> next accept on hot route) 170-632 ticks; escape probability ~1.0.
PRIOR FINDING NOW REFRAMED: an earlier 2-arm read (gossip vs ring only, pooled) was reported as "VoI negative" (50.0 vs 56.3M); the 3-arm grid shows information vs NO information is +56% median, and gossip ~= ring on the pooled read with violent per-seed variance.
HISTORY THAT MATTERS: a PPO ablation on the single-trader gym found contact-information worthless to a lone learner at these prices (NULL, banked); two prior NO-GOs died of self-averaging (LLN over a homogeneous field); the spec pre-registered "gossip-retention vs pirate lie-low/relocation timescale phase-lock" as the central tuning risk; the spec also carried "saturation at 4-6 stations" as risk #1 with 'the honest fix is map growth, not artifact changes'.

THE OWNER'S QUESTION: "the play story is great so I'm finding it hard to believe it's not hitting the world right. I think it might be because the incremental value of money vs danger isn't worth it — figure out the WHY."
Note the blind arm appears to falsify the owner's stated hypothesis at the population level — but engage with what the owner may really be sensing: the gossip CHANNEL itself adds ~zero earnings over the ring, i.e. the new realism (lag, locality, loss) is not yet a live gameplay variable.

YOUR DELIVERABLE: a diagnosis of WHY (mechanism-level, citing the numbers), ranked by confidence; what single measurement or world-price change would most decisively test your top hypothesis; explicit anti-recommendations (what NOT to touch). Diagnose only — no implementation. Respect PDR-0006 vocabulary (windows, prices, mechanics — no gates, no shaping).
`

const QUESTIONS = {
  pattern: `As the systems pattern-recognizer: name the system archetype(s) at work. Candidates to weigh against the data: (1) saturation makes gossip informationally equivalent to the ring (common knowledge ~= oracle), so the channel difference cannot express; (2) delayed signal vs mobile threat — median hearing lag 2669 ~ pirate relocation 2500, so avoidance often fires at a route the pirate has already left (the pre-registered phase-lock); (3) chaotic basin sensitivity — 4 seeds, two PermanentPeace flips under tiny perturbations, pooled medians dominated by basin membership not channel quality; (4) predator-prey with an information-coupled harvest rate. Which archetype dominates which observation? Where does the seed-23 gossip catastrophe (hauler 4.95M, pirate 39.95M, robs 74) fit?`,
  leverage: `As the leverage analyst (Meadows hierarchy): given the 3-arm grid, where are the high-leverage points for making the gossip channel's realism (lag, locality, loss) MATTER as play, and for letting the owner feel information as a survival variable? Assess specifically: slot caps / retention (saturation levers), the 6-station map size (the spec says the honest saturation fix is map growth), pirate relocation vs gossip timescales (delay structure), stakes (ransom cap, cargo value at risk), and the evidence-penalty curve (150/rob, 900 clamp) as the avoidance transfer function. Rank by leverage level and by PDR-0006 lawfulness (world prices/mechanics yes, shaping no). What is the LOWEST-leverage thing we might be tempted to tune that you advise against?`,
  simarch: `As the simulation architect: critique the MEASUREMENT before the mechanism. 4 seeds with two basin flips — what does the pooled-median VoI read actually estimate, and what ensemble/protocol would separate channel quality from basin chaos (paired seeds? matched-basin conditioning? more seeds? longer runs? per-window dose-response between hearing events and subsequent accepts)? Then the mechanism: with saturation at 1000 permille and hearing lag ~ relocation period, what does theory predict for gossip-vs-ring earnings, and is the observed ~zero gap exactly what a correctly-functioning system SHOULD show at this map scale? Design the single most decisive next experiment (cheap, deterministic, reported-never-gated).`,
  emergence: `As the emergence/experience designer: the owner says the play story is great but suspects it is not hitting the world right. With saturation (everyone eventually knows everything) the gossip layer currently changes WHO knows WHEN — visible in the chronicle — but barely changes WHO EARNS WHAT vs the old ring. What would make information a felt, decision-relevant resource in play: information asymmetry that persists (bigger maps, slot scarcity, route-local circulation), threats that move faster than news (so freshness matters), prices that make one robbery catastrophic rather than a 6M tax, or consumers beyond route-avoidance (detours, convoys, escorts-on-demand)? Which of these is the smallest cut that creates a visible information haves/have-nots arc in the chronicle, consistent with the deferred ladder (no new consumers without generators, pirates stay blind)?`,
}

const SCHEMA = {
  type: 'object',
  required: ['diagnosis', 'top_hypothesis', 'decisive_test', 'anti_recommendations', 'confidence'],
  properties: {
    diagnosis: { type: 'string', description: 'mechanism-level WHY, citing the supplied numbers' },
    top_hypothesis: { type: 'string', description: 'one-sentence top-ranked cause' },
    decisive_test: { type: 'string', description: 'the single measurement/world-price probe that best tests it' },
    anti_recommendations: { type: 'array', items: { type: 'string' }, description: 'what NOT to change and why' },
    confidence: { type: 'string', description: 'confidence + main information gap' },
  },
}

phase('Panel')
const [pattern, leverage, simarch, emergence] = await parallel([
  () => agent(DATA + QUESTIONS.pattern, { label: 'pattern-recognizer', schema: SCHEMA, agentType: 'yzmir-systems-thinking:pattern-recognizer' }),
  () => agent(DATA + QUESTIONS.leverage, { label: 'leverage-analyst', schema: SCHEMA, agentType: 'yzmir-systems-thinking:leverage-analyst' }),
  () => agent(DATA + QUESTIONS.simarch, { label: 'simulation-architect', schema: SCHEMA, agentType: 'bravos-simulation-tactics:simulation-architect' }),
  () => agent(DATA + QUESTIONS.emergence, { label: 'emergence-designer', schema: SCHEMA, agentType: 'bravos-systems-as-experience:emergence-designer' }),
])
return { pattern, leverage, simarch, emergence }