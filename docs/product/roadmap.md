# Roadmap — jumpgate            Updated: 2026-06-09 (PDR-0001, bootstrap)

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
> that *retires thesis risk* (the demand-driven economy where DRL has room) precedes
> crew fidelity. Everything after A is additive — no rip-and-replace.

- **Vertical-slice shaping pass (DESIGN)** · tracker: `jumpgate-818a04bb6b` (P1,
  ready; runs alongside the Plan-A build) — harvest the archived ecosystem design
  (`archive/solution-architecture/{19..25}`, ADRs, epic plans) onto the 3D/SoA
  substrate; define the **first closed economic loop**, the **demand-driven pricing**
  mechanism (the contention that gives DRL room), where the DRL decision lives, and
  **the concrete falsifiable ecosystem metric** (the `metrics.md` / charter `TARGET`).
  Output = a decomposed Layer-1 backlog. *The highest-value next PM move.*
- **Plan-4 — gym surface + first trainable rung** · tracker: `jumpgate-5a3e01ab08`
  (P1, blocked_by A only) — `JumpgateEnv` + frame-relative obs + action/reward +
  reproducible `reset(seed)`/`step()`. **The PyO3 facade is a `scaffold_ok()` stub
  today.** The single-craft navigator A→B is the *first trainable rung* (proves an
  agent can exist + be trained), **not** the thesis test (PDR-0002). Measure **env
  throughput (steps/sec)** here, early — too-slow forces a smaller venue.
- **Layer-1 vertical-slice ecosystem (NET-NEW — the thesis venue)** · tracker:
  `jumpgate-a494b1d700` (P2, blocked_by shaping; placeholder epic, decomposed at
  shaping) — mechanical scripted/fixed-price loop → demand-driven pricing →
  ecosystem obs/action → **swap scripted→DRL & measure** (the thesis test) → then
  the combat/piracy/law trophic level. Where "DRL > scripted, strategic/operational"
  is actually falsified.

## Later (directional bets, no order, no dates)

- **Crew fidelity — Person + Ship Plan B / C** · tracker: `jumpgate-12f37a8d74` (B,
  blocked_by A; owns HASH_FORMAT_VERSION 1→2 dual-golden re-derive),
  `jumpgate-205fd66b25` (C, blocked_by B). **Re-sequenced to additive enrichment
  AFTER the first economic loop closes** (P3) — crew is not loop-load-bearing.
- **Multi-domain fidelity** — economy, combat, exploration, industry as the
  long-term north stars.
- **Tactical ship combat** — drone-led, bigger ships lobbing rounds;
  **gravity-decoupled**, resolved at a local high-fidelity LOD / floating origin
  (rides the existing §3.2 LOD seam — additive, no new debt). Not v1; not the
  primary thesis venue (PDR-0003).
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
