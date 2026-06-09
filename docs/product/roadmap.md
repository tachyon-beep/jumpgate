# Roadmap — jumpgate            Updated: 2026-06-10 (PDR-0006, game frame)

> Sequencing, WSJF / cost-of-delay, and dated forecasts are produced by
> /axiom-program-management. This file records bets as INTENT, not a delivery
> schedule. Do not compute WSJF here; hand the committed bet over for sequencing.
>
> Bootstrapped from observed direction; reconciled 2026-06-09 against the parallel
> **program** layer (`docs/superpowers/program/charter.md` + `raid.md`) and the now-
> real filigree backlog. The tactical backlog lives in **filigree** and is
> referenced by issue ID, never copied here. Sequencing/forecast is the program
> layer's job; this file is intent.

## Now  (committed, in-flight)

- **Person + Ship Plan A — the EffectiveMods seam** · tracker: `jumpgate-d30fcebaac`
  (P1, ready) · metric: determinism guardrail (prove trajectory-equivalence) —
  *enables* the north-star, doesn't move it directly.
  The behaviour-preserving widening `effective_params(&BaseSpec) →
  (&BaseSpec, &EffectiveMods)` + an identity-valued `mods` column. **The single
  non-additive gate in the entire forward plan** — it changes one signature once,
  ever; everything after (Person B/C, the gym, and the ecosystem actors' mining
  yield / hauler capacity / weapons) is additive on top of it. Owner priority is
  debt-avoidance, so getting this seam exactly right is the bet. Owner confirmed
  "stay the course: Plan A next" (2026-06-09). Spec:
  `specs/2026-06-08-jumpgate-person-ship-foundation-design.md`.

  _Background context (built, not a bet):_ engine v1 on `jumpgate-v1-design` — 17
  core modules, 144 tests (verified green), guidance landed, goldens pinned. Not
  merged to `main` (cutover gate open — see `current-state.md`).

## Next (shaped, decreasing certainty)

> Land order **CONFIRMED** by owner 2026-06-09 (charter §"Land order"); the earlier
> "post-A ordering" question is resolved. After the irreversible seam (A), the work
> that **builds the first living economic loop** (the demand-driven trophic life-sim)
> precedes crew fidelity. Everything after A is additive — no rip-and-replace.
>
> **Frame (PDR-0006):** v1 is a **game, judged by emergent play** — surprising,
> watchable, alive — the way `ecosystem-oscillation` was judged (heuristic agents,
> zero RL: the project's one unambiguous success). The deterministic substrate +
> chronicle / diagnostics / sweeps is the **reproducible lab for studying the game's
> emergent dynamics** ("game science"), **not** a gate. The retired
> presolvability/DRL-room frame — "can a learner beat the computed optimum by a
> fraction-of-ceiling" — is gone: it defined the game away (catch-22). Do not
> re-derive it.

- **Vertical-slice shaping pass (DESIGN)** · tracker: `jumpgate-818a04bb6b` (P1,
  ready; runs alongside the Plan-A build) — harvest the archived ecosystem design
  (`archive/solution-architecture/{19..25}`, ADRs, epic plans) onto the 3D/SoA
  substrate; define the **first closed economic loop**, the **demand-driven pricing**
  mechanism (the supply/demand tension that makes traders' decisions matter and the
  world feel alive), where the agents act, and **the game-dynamics signals that say
  it's alive** (sustained predator-prey cycles — amplitude/period — pack
  formation/dispersal + autocorrelation, trophic balance, chronicle richness). These
  are how we *study* the game in the lab, not a turnstile in front of it. Output = a
  decomposed Layer-1 backlog. *The highest-value next PM move.*
- **Plan-4 — gym surface + first trainable rung** · tracker: `jumpgate-5a3e01ab08`
  (P1, blocked_by A only) — `JumpgateEnv` + frame-relative obs + action/reward +
  reproducible `reset(seed)`/`step()`. **The PyO3 facade is a `scaffold_ok()` stub
  today.** The single-craft navigator A→B is the *first trainable rung* — it proves
  an agent can exist and be trained inside the world. DRL here is a **player** we can
  later drop into the life-sim to make agents interesting opponents/allies, judged by
  the quality of play it produces. Measure **env throughput (steps/sec)** here,
  early — too-slow forces a smaller venue.
- **Layer-1 vertical-slice ecosystem (NET-NEW — the first living world)** · tracker:
  `jumpgate-a494b1d700` (P2, blocked_by shaping; placeholder epic, decomposed at
  shaping) — mechanical scripted/fixed-price loop → demand-driven pricing →
  foragers/haulers reacting to it → the combat/piracy/law trophic level on top. The
  goal is a **trophic life-sim that produces emergent, watchable play** (the
  `ecosystem-oscillation` boom/bust cycle, re-grown on the 3D substrate). DRL agents
  are introduced as players where they make the play richer — not as a thesis to
  validate against a computed optimum.

## Later (directional bets, no order, no dates)

- **Crew fidelity — Person + Ship Plan B / C** · tracker: `jumpgate-12f37a8d74` (B,
  blocked_by A; owns HASH_FORMAT_VERSION 1→2 dual-golden re-derive),
  `jumpgate-205fd66b25` (C, blocked_by B). **Re-sequenced to additive enrichment
  AFTER the first economic loop closes** (P3) — crew is not loop-load-bearing.
- **Multi-domain fidelity** — economy, combat, exploration, industry as the
  long-term north stars.
- **Tactical ship combat** — drone-led, bigger ships lobbing rounds;
  **gravity-decoupled**, resolved at a local high-fidelity LOD / floating origin
  (rides the existing §3.2 LOD seam — additive, no new debt). Not v1; the first
  living world is the economic trophic life-sim, not combat (PDR-0003).
- **Crew-on-a-flight-deck depth** — positional command (chairs not classes),
  succession, recognition/mutiny — the fidelity yardstick.
- **Rendering + player-interactivity** — an external client draws the headless
  world; a human flies/acts in it.
- **LOD tiers 2 & 3** — `LodNpcInteraction` (fair fight/trade grid) and
  `LodNothing` (closed-form dormant entities woken by event).
- **Full trophic-economy drama** — the richer boom/bust cycle, navy deterrence,
  per-pirate notoriety, and event chronicle from the scrapped econ/DRL line, atop
  the Next vertical-slice once its thin loops close. Design-harvestable from
  `archive/`; code was hard-reset 2026-06-08.
