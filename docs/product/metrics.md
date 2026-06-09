# Metrics — jumpgate             Last read: 2026-06-09

> Bootstrapped 2026-06-09. Targets marked **TARGET** / **\<date\>** are placeholders
> the owner must set to real numbers before they can fire an acceptance or a PDR
> reversal trigger. A directional word is not a metric.

## North-star

The thesis *is* the bet: DRL is **more entertaining** than scripted/FSM AI — and
(owner, 2026-06-09, PDR-0002) **the fun is strategic/operational, not tactical**.
jumpgate is "not a fly-by-stick game — almost all transit is navigation over
weeks/months of sim-time." So the thesis is NOT tested at the joystick (the
single-craft A→B transfer, where a near-optimal autopilot leaves DRL nothing to
win and "entertaining" has near-zero variance — confirmed by the project's own
`vsl-cannot-host-judgment-principle` / population-games findings). It is tested at
the **strategic/operational decision layer inside the demand-driven multi-agent
ecosystem** (charter §"v1 vertical slice"): which contract, which route, when to
commit weeks of fuel/time, risk vs reward over long horizons — miners→fuel→
haulers→contracts→deflationary pricing→piracy/law, with DRL agents AS the actors.

| Metric | Target (falsifiable) | Current | Read on | Trend |
|--------|----------------------|---------|---------|-------|
| DRL-vs-scripted **strategic/operational** differential in the multi-agent ecosystem — learned agents (miner/hauler/pirate/law) make richer, measurably better long-horizon decisions (contract/route/fuel-commitment under endogenous deflationary pricing + predation risk) than scripted-heuristic agents | learned beats best scripted heuristic by ≥ **TARGET** on a stated ecosystem metric (owner+PM to define at vertical-slice shaping), seed-reproducible | **not yet measurable** — gym facade is a `scaffold_ok()` stub; ecosystem layer is net-new (design harvestable from `archive/`, code dead) | 2026-06-09 | — |

> The *tactical* navigator task (single-craft A→B) is **not** the north-star — it
> is the first trainable rung that proves an agent can exist and be trained at all
> (a Plan-4 milestone, not the thesis test). Promote a concrete ecosystem metric
> here at vertical-slice shaping. The done-definition in
> `docs/superpowers/program/charter.md` is the same criterion from the delivery
> side; keep them in sync (PDR-0002).

## Input metrics (the levers that move the north-star)

| Metric | Target | Current | Read on |
|--------|--------|---------|---------|
| Gym surface exists end-to-end (`reset(seed)`/`step()` reproducible, frame-relative obs) | `JumpgateEnv` trains a policy | absent (stub only) | 2026-06-09 |
| Env throughput (the RL bottleneck is CPU env steps/sec, per strategy memory) | ≥ **TARGET** steps/sec | unmeasured | 2026-06-09 |

## Guardrails (must NOT degrade)

| Metric | Floor / ceiling | Current | Read on |
|--------|-----------------|---------|---------|
| Replay determinism (Tier B: same-binary bit-reproducible) | replay bit-identical; 3 pinned hash goldens stable (config `0x9767…`→re-pinned `0x278c_5d91_b75a_9e5a`; state `0xf0dd…` / `0x532d_07bf_95a2_abc5`) | **verified intact** — `record_then_replay_is_bit_identical` + `recorded_run_actually_thrusts` pass (`cargo test -p jumpgate-core`, exit 0) | 2026-06-09 |
| Core test suite green | 100% pass (clippy `--all-targets` clean) | **verified** — `cargo test -p jumpgate-core` exit 0 (144 test fns) | 2026-06-09 |
| "Build for but not with" discipline | no DO-NOT-BUILD-IN-V1 watch-list item added without an ADR | held | 2026-06-09 |

> Determinism is a guardrail, not the north-star: it is the foundation the
> RL-replay-debugging story rests on, so a thesis win that *broke* replay would be
> a false win. Experiment design and kill/keep logic:
> `product-metrics-and-experimentation.md`.
