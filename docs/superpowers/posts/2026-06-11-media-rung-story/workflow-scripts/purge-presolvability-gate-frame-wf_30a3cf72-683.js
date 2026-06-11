export const meta = {
  name: 'purge-presolvability-gate-frame',
  description: 'Excise the DRL-room/presolvability-gate/fraction-of-ceiling frame from every authority artifact and replace with the GAME frame (PDR-0006). The frame caused TWO hard resets; purge so it cannot be re-inherited.',
  phases: [ { title: 'Purge' }, { title: 'Verify residue' } ],
}

const BRIEF = `
JUMPGATE HAS DONE A HARD RESET **TWICE** because the same poison keeps creeping back: framing v1 as a **DRL-ROOM thesis to be PROVEN by a PRESOLVABILITY GATE** — the "analytic cut" ("can a learner beat the exactly-computed optimum by >=10% of ceiling"; "fraction-of-ceiling"; "beat the best script"; "honor NO-GO if <10%"; "replay-determinism IS presolvability AS A GATE"). That frame is a **CATCH-22 FOR GAMES**: anything exactly measurable is "presolvable -> no room", anything with real room is "unmeasurable" — so it can ONLY return NO-GO or can't-measure. It **defines the game away**. PDR-0006 (docs/product/decisions/0006-judge-v1-as-a-game-not-a-presolvability-gate.md — READ IT) retired it.

YOUR JOB: **EXCISE this frame from your assigned artifact and replace it with the GAME FRAME below. Do NOT soften it — REMOVE it.** Reframe the file (do NOT delete it).

THE GAME FRAME (write this instead):
- v1 is a **GAME, judged by EMERGENT PLAY** — surprising, watchable, alive, fun — the way \`ecosystem-oscillation\` was judged (play-judged, heuristic agents, ZERO RL: the project's ONE unambiguous success).
- **Determinism + chronicle/diagnostics/sweeps = the reproducible LAB for STUDYING the game's emergent dynamics** ("game science"). KEEP this. It is NOT a gate; it is how you study a game rigorously.
- Success = the **game's OWN dynamics**: sustained predator-prey cycles (amplitude/period), pack formation/dispersal + autocorrelation, trophic balance, chronicle richness of individual lives. NOT "beat a computed optimum".
- **DRL is a PLAYER** that makes agents interesting opponents/allies, judged by the quality of play it produces — NOT a thesis validated by a fraction-of-ceiling differential.
- **Mechanics are game mechanics**: information/Media (hidden richness, scouting, word-of-mouth, staleness), salvage/tugs, refuel/energy, pirates, police — they make decisions rich and the world alive, NOT "rooms" to measure.
- The \`vsl-cannot-host-judgment-principle\` is an ACCURATE OBSERVATION about why a *small replayable market* is boring — KEEP it as that, but it is **RETIRED AS A BUILD GATE**: never again cite it to forbid building the game.

PRESERVE genuine history/facts (what was built, what was measured — e.g. the commons-miner cut is real and confirmed the gate is a dead end for games). Mark gate-DOCTRINE as RETIRED per PDR-0006; do not erase empirical records.

DO NOT re-derive or re-justify the gate. If you catch yourself writing "but we need to measure whether DRL beats the best script / the optimum / fraction-of-ceiling" — STOP, that IS the poison. Edit the file in place. Return a short summary of what you excised and what you replaced it with.
`

const ARTIFACTS = [
  { key:'charter',   path:'docs/superpowers/program/charter.md', note:'Reframe Outcome/thesis, the v1 done-definition (currently "measurable DRL-vs-scripted differential" + the PDR-0005 refinement paragraph), the land-order (currently "cheap analytic cut (expected-fail gate) -> IF clears train & measure"), and the backlog. Done = a good game judged by emergent play.' },
  { key:'metrics',   path:'docs/product/metrics.md', note:'Reframe the north-star (currently "DRL-vs-scripted strategic/operational differential ... beats best scripted heuristic by >=TARGET"). Replace with game-dynamics success metrics (sustained-cycle amplitude/period, pack autocorrelation, trophic balance, chronicle richness). KEEP the determinism guardrail (it is the lab, not a gate).' },
  { key:'vision',    path:'docs/product/vision.md', note:'Reframe any "prove DRL beats scripted / measurable differential / presolvability" framing to: build a living emergent game; DRL is a player; judged by play.' },
  { key:'pdr0002',   path:'docs/product/decisions/0002-thesis-venue-is-strategic-operational.md', note:'Add a prominent SUPERSEDED-BY-PDR-0006 banner at the top: the "measurable DRL-vs-scripted differential" done-definition is retired in favour of the game frame. Keep the historical content below the banner.' },
  { key:'roadmap',   path:'docs/product/roadmap.md', note:'Reframe any gate/analytic-cut/DRL-room sequencing to the game-build sequence (build the trophic life-sim, judged by play).' },
  { key:'current',   path:'docs/product/current-state.md', note:'Reframe any "DRL room / gate / thesis-to-prove" status framing to the game frame + record the frame change (PDR-0006) and the commons-cut as the gate-confirming dead-end artifact.' },
  { key:'raid',      path:'docs/superpowers/program/raid.md', note:'Reframe R3 (thesis-unproven) and R5 (site DRL where it has room) — the "DRL-room must be proven" risk is dissolved by the game frame; the real risk is now "is it a good, alive game". Keep determinism/fabrication risks.' },
  { key:'mem-pm',    path:'/home/john/.claude/projects/-home-john-jumpgate/memory/jumpgate-pm-takeover-and-structure.md', note:'Add a prominent FRAME banner at the very TOP: v1 is a GAME judged by emergent play (PDR-0006); the gate/PDR-0005/analytic-cut content below is RETIRED doctrine kept only as history. Reframe the forward "what next" lines to the game build.' },
  { key:'mem-vsl',   path:'/home/john/.claude/projects/-home-john-jumpgate/memory/vsl-cannot-host-judgment-principle.md', note:'Re-tag at the TOP: this is an ACCURATE OBSERVATION about small replayable markets, RETIRED AS A BUILD GATE per PDR-0006 — it caused two hard resets when used as a gate; never cite it to forbid building the game. Keep the empirical findings as history.' },
  { key:'mem-probes',path:'/home/john/.claude/projects/-home-john-jumpgate/memory/contention-game-fifth-nogo.md', note:'This + its sibling NO-GO probe memories record real small-market findings. Add a one-line top tag: historical small-market observation, NOT a build gate (PDR-0006 game frame governs). Reframe only the framing line; keep the facts. (Also do the same lightweight top-tag, if you have time, conceptually — but only edit THIS file.)' },
]

phase('Purge')
const purged = await parallel(ARTIFACTS.map(a => () =>
  agent(
    `${BRIEF}\n\n=== YOUR ARTIFACT ===\nFile: ${a.path}\nGuidance: ${a.note}\n\nRead it, EXCISE the presolvability-gate/DRL-room-as-thesis frame, write the GAME frame in its place (edit in place). Return a 3-5 line summary of exactly what you removed and what you wrote.`,
    { label:`purge:${a.key}`, phase:'Purge', agentType:'general-purpose' },
  ).then(text => ({ key:a.key, text })).catch(e => ({ key:a.key, text:'ERROR: '+String(e) }))
))

const purgeLog = purged.filter(Boolean).map(r => `[${r.key}] ${r.text}`).join('\n\n')
log(`Purge pass done (${purged.filter(Boolean).length}/${ARTIFACTS.length})`)

phase('Verify residue')
const residue = await agent(
  `Frame-purge verification. The presolvability-gate / DRL-room-as-thesis frame was just excised from the Jumpgate authority artifacts and replaced with the GAME frame (PDR-0006). Your job: GREP for RESIDUAL gate-frame language that still reads as LIVE DOCTRINE (not clearly marked as retired/history) across docs/ and the memory dir /home/john/.claude/projects/-home-john-jumpgate/memory/.\n\nSearch for: "fraction-of-ceiling", "analytic cut", "beat the (best )?script", "DRL room"/"DRL-room", "presolvab", "NO-GO if", "10% of ceiling", "thesis to prove", "measurable.*differential", "best.closed.form".\n\nFor EACH hit: report file:line, the line, and whether it is (OK) clearly marked retired/historical, or (RESIDUE) still reading as live forward doctrine. List only RESIDUE hits that still need fixing. Be precise; this frame has crept back twice and must not survive as live doctrine. Return the residue list (or "clean").`,
  { label:'verify-residue', phase:'Verify residue', agentType:'general-purpose' },
)

return { purged: purgeLog, residue }
