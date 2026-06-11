# Story archive: the media rung — "News Travels at Ship Speed"

Source material for future posts. One day's arc (2026-06-11), captured end to end:
a multi-agent design panel, a verified 10-task build, a wrong headline number, an
expert panel that diagnosed it, two instruments that adjudicated the experts, and
an owner intuition that turned out to be the measured truth.

Everything here is a **window, never a gate** (PDR-0006): Jumpgate is a game
judged by emergent play; these artifacts are the lab notebook beside the window.

## The arc

1. **Design panel** (`panels/01-design-panel-18-agents.json`, script
   `workflow-scripts/media-rung-design-wf_9ff25d02-56f.js`): 18 agents — 5
   citation-grade grounding readers, 6 independent design lenses, 6 adversarial
   critics, 1 synthesis. The critics caught 9+ CRITICAL findings before a line of
   code existed (per-tick transfer self-averaging, a significance seed built on a
   near-constant ransom, an event-stream/hashed-state coupling, a global
   forgetting clock...). Spec: `docs/superpowers/specs/2026-06-11-media-rung1-gossip-design.md`.
2. **Build** (`panels/02-…json`, `03-…json`, script
   `media-rung1-build-wf_14f650e2-005.js`): 10 plan tasks, each a builder plus an
   independent adversarial verifier instructed to trust nothing — verifiers
   re-ran every test, re-derived every golden hash, and reverted fixes to watch
   tests bite. Survived a session-limit kill mid-task-4 and resumed from the
   journal with cached results. 9 commits, 2 single-cause golden re-pins,
   replay-bit-identical, media-off worlds byte-identical to pre-media HEAD.
3. **The wrong headline**: the first bench read "VoI negative" (gossip haulers
   earn less than ring haulers). The owner's instinct: *"the play story is great,
   I find it hard to believe it's not hitting the world right."*
4. **The missing arm**: belief-scoring OFF entirely. Information vs ignorance =
   **+56% median lifetime earnings**. The headline had compared two information
   sources and called it the value of information.
5. **WHY panel** (`panels/04-why-panel-4-experts.json`, script
   `media-voi-why-panel-wf_502b2fbf-353.js`): four experts — systems
   pattern-recognizer, leverage analyst, simulation architect, emergence
   designer — given the 3-arm grid. Four mechanisms proposed: saturation
   (topology), comparator flatness (the 900-clamp), retention bleed (timing),
   basin contamination (measurement).
6. **The instruments decide** (`bench/basin_clean/`, commit 2f79be9): two new
   unhashed windows — a per-decision evidence-count histogram and the
   gossip-vs-ring **argmax-flip share** (does the channel ever change a
   decision?). 60 runs, seeds 0–19 × 3 arms × 50k ticks.
   - Flip share median **36%** → "channel is rank-converged dead weight" FALSIFIED.
   - Clamp region ~1% of candidates → "flat avoidance curve" FALSIFIED.
   - Ring beats gossip **4/4 on basin-clean seeds, +9.4M median**, while gossip
     haulers carry MORE evidence and get robbed MORE → **retention bleed
     CONFIRMED**: the read window anchored on *when you heard*, so a rumor heard
     2,669 ticks late stayed actionable until the robbery was ~6,669 ticks old —
     2.7 pirate relocations. Haulers dodged yesterday's danger into today's.
7. **The owner's theory wins** (`bench/theory_arms/`, knob commit 88a5d85):
   *"people would immediately ask 'when did that occur'… maybe you can still get
   useful data as long as you know when it happened."* Three arms tested it.

## The verdict tables

3-arm grid (seeds 7/23/42/99 × 50k, pooled median hauler credits):

| arm | evidence source | pooled median |
|---|---|---|
| blind | none | 36.25M |
| ring | global rob log (oracle) | 56.55M |
| gossip | personal hearsay, hear-time anchor | 55.40M |

6-arm theory grid (seeds 0–19 × 50k):

| arm | pooled median | basin-clean Δ vs ring | ring wins | flip share |
|---|---|---|---|---|
| blind | 32.2M | +20.4M | 9/9 | — |
| ring (oracle) | 52.0M | — | — | — |
| gossip (hear-time) | 49.6M | **+9.4M** | 4/4 | 36% |
| **A — age anchor only** | **56.9M** | **−0.6M** | 3/7 | 8% |
| F — faster + quicker fade | 60.9M | +1.4M | 4/7 | 40% |
| AF — both | 61.0M | −0.7M | 3/7 | 6% |

One rule change — readers ask *when did it happen* — closes the entire gap to
the omniscient oracle. The surviving 8% of decisions where hearsay still
disagrees (genuine locality and lag) cost nothing. **Hearsay plus a timestamp
equals an oracle, minus only what you honestly haven't heard yet.** And the
moment readers trust the timestamp, *when* becomes a claim worth lying about —
the next corruption arc, armed but unbuilt.

Bonus regime finding: media worlds end their wars far more often (5–6
PermanentPeace verdicts per media arm vs the ring's 0) — gossip-fed avoidance
starves pirates into the escort-ladder endgame in basins the oracle never tips.

## Pull-quote numbers

- 18-agent design panel; 9+ CRITICALs caught pre-code; ~1.8M design tokens.
- 10-task build, builder + adversarial verifier each; ~1.7M build tokens;
  every verifier independently re-derived the golden hashes.
- 120 fifty-thousand-tick deterministic runs for the science; every one
  replay-bit-identical.
- The decisive instrument is two integers on an existing line: `flips` and a
  7-bucket histogram.

## Contents

- `panels/` — raw multi-agent workflow outputs (design panel, build reports with
  verifier verdicts, WHY-panel expert reports), JSON.
- `workflow-scripts/` — the orchestration scripts, **the whole project's history**
  (24 scripts: econ harness → commons cut → tactical flight → trader → pirates →
  media → the world-gets-big design panel now running).
- `bench/basin_clean/`, `bench/theory_arms/` — per-run stdout (RESULT / MEDIA /
  ASSIGN lines) + the exact job lists; JSONL window streams reproducible from
  the job lists (deterministic, seeds included).
- Specs/plans referenced: `docs/superpowers/specs/2026-06-11-media-rung1-gossip-design.md`,
  `docs/superpowers/plans/2026-06-11-media-rung1-gossip-implementation.md`,
  `docs/superpowers/lab-notes/2026-06-11-ppo-contact-ablation-null.md`.

Capture practice from here on: every panel output and bench summary gets banked
here (or a sibling story dir) the day it is produced — /tmp is volatile.
