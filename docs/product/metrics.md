# Metrics — jumpgate             Last read: 2026-06-09

> Bootstrapped 2026-06-09. Targets marked **TARGET** / **\<date\>** are placeholders
> the owner must set to real numbers before they can fire an acceptance or a PDR
> reversal trigger. A directional word is not a metric.

## North-star

v1 is a **game, judged by emergent play** — is it surprising, watchable, alive,
fun? — the way `ecosystem-oscillation` was judged (play-judged, heuristic agents,
**zero RL**: the project's one unambiguous success). The success criterion is the
**game's own dynamics**: a demand-driven multi-agent world — miners→fuel→haulers→
contracts→deflationary pricing→piracy/law — that produces sustained, surprising,
watchable life. We judge that life directly (cycles, packs, trophic balance, the
chronicle of individual lives), not against any computed optimum.

DRL is a **player** in this world: it is introduced where it makes agents
interesting opponents/allies, and judged by **the quality of play it produces** —
never by a fraction-of-ceiling differential against a presolvable optimum. The
mechanics (information/Media, salvage/tugs, refuel/energy, pirates, police) are
**game mechanics** that make decisions rich and the world alive, not "rooms" to be
measured.

> **Stats are windows, not judges (owner, 2026-06-10; PDR-0006).** The rows below are
> **signals we watch to see and shape how alive the ecosystem is** — instruments for the
> observe→steer→re-observe design loop, wanted because they're interesting and used to
> inform the holistic "is this alive and fun" read. They are **NOT** bars to clear,
> numbers to optimise toward, or proof of anything. Each "≥ TARGET" describes *what alive
> tends to look like* (vs the dead failure mode beside it) — a shape to recognise, not a
> gate. Making any of them load-bearing is the same compulsion that regenerated the gate.

| Signal (a window to watch + steer toward — NOT a bar to clear) | What "alive" looks like (vs dead) | Current | Read on | Trend |
|--------|----------------------|---------|---------|-------|
| **Sustained predator-prey cycle** in the demand-driven ecosystem (boom/bust amplitude + period — the `ecosystem-oscillation` signature: peace↔feast, scarcity→price spike) | a self-sustaining cycle with amplitude ≥ **TARGET** and period in a stated band, persisting over **TARGET** sim-time without collapse to a fixed point (owner+PM to set at vertical-slice shaping), seed-reproducible | **not yet measurable** — ecosystem layer is net-new (design harvestable from `archive/`, code dead) | 2026-06-09 | — |
| **Pack formation/dispersal** (spatial/temporal clustering of agents — autocorrelation of pack-membership over time; pirates massing then scattering) | measurable clustering with autocorrelation ≥ **TARGET** and observable form/disperse transitions | **not yet measurable** | 2026-06-09 | — |
| **Trophic balance** (the three levels — prey/predator/apex i.e. haulers/pirates/navy — coexist without one collapsing or runaway-dominating) | all levels persist over **TARGET** sim-time; no level extinct or saturating | **not yet measurable** | 2026-06-09 | — |
| **Chronicle richness** (the event chronicle narrates distinguishable individual lives — per-agent notoriety/history; lives that differ and are worth watching) | chronicle yields **TARGET** distinguishable life-arcs per run (e.g. distinct notoriety/career trajectories), owner-readable as story | **not yet measurable** | 2026-06-09 | — |

> These are the **game's own properties**, not a beat-the-script/optimum gate
> (retired as a build prerequisite by PDR-0006 — see Guardrails note). Promote
> concrete numbers here at vertical-slice shaping. The done-definition in
> `docs/superpowers/program/charter.md` is the same emergent-play criterion from
> the delivery side; keep them in sync (PDR-0006 refines PDR-0002).

## Input metrics (the levers that move the north-star)

| Metric | Target | Current | Read on |
|--------|--------|---------|---------|
| Demand-driven ecosystem runs end-to-end (miners→fuel→haulers→contracts→pricing→piracy/law as a living loop) | ecosystem sustains itself over a full run | absent (net-new) | 2026-06-09 |
| Chronicle + diagnostics + sweeps emit the dynamics signals above (cycle/pack/trophic/chronicle measurable from a run) | signals extractable per run, seed-reproducible | absent | 2026-06-09 |
| DRL player can join the world (gym surface `reset(seed)`/`step()` reproducible, frame-relative obs) so learned agents can enrich play | a policy can act as an agent in the ecosystem | absent (stub only) | 2026-06-09 |
| Env throughput (sim steps/sec — how fast we can run and study the game) | ≥ **TARGET** steps/sec | unmeasured | 2026-06-09 |

## Guardrails (must NOT degrade)

| Metric | Floor / ceiling | Current | Read on |
|--------|-----------------|---------|---------|
| Replay determinism (Tier B: same-binary bit-reproducible) | replay bit-identical; 3 pinned hash goldens stable (config `0x9767…`→re-pinned `0x278c_5d91_b75a_9e5a`; state `0xf0dd…` / `0x532d_07bf_95a2_abc5`) | **verified intact** — `record_then_replay_is_bit_identical` + `recorded_run_actually_thrusts` pass (`cargo test -p jumpgate-core`, exit 0) | 2026-06-09 |
| Core test suite green | 100% pass (clippy `--all-targets` clean) | **verified** — `cargo test -p jumpgate-core` exit 0 (144 test fns) | 2026-06-09 |
| "Build for but not with" discipline | no DO-NOT-BUILD-IN-V1 watch-list item added without an ADR | held | 2026-06-09 |

> Determinism is the **reproducible lab for studying the game's emergent
> dynamics** ("game science") — it lets us replay, chronicle, diagnose, and sweep
> a living game rigorously. It is **not** a gate the game must pass: PDR-0006
> retired the presolvability / beat-the-computed-optimum frame as a build
> prerequisite (the catch-22 that "anything measurable is presolvable → no room").
> `vsl-cannot-host-judgment-principle` remains a true observation about why a
> *small replayable market* is boring, **retired as a build gate** — never cite it
> to forbid building the game. Experiment design and kill/keep logic:
> `product-metrics-and-experimentation.md`.
