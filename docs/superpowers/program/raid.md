# Jumpgate v1 — RAID Log

*Risks · Assumptions · Issues · Dependencies. Living artifact — review when a line lands or a decision changes, not as a kickoff relic. Owner: John. PM: Claude. Created 2026-06-09.*

Exposure = Likelihood × Impact (L/M/H each). Re-score on review.

## Risks
| # | Risk | Exp. | Mitigation / Trigger |
|---|---|---|---|
| R1 | **The `EffectiveMods` seam shape.** ~~If the bundle shape is wrong, downstream needs rework.~~ **SIGNATURE LANDED CLEAN 2026-06-09** (Plan A, commits 8d207a9..c37b12f): `effective_params(&BaseSpec, &EffectiveMods)` is in, bit-identical (hash.rs byte-unchanged + both goldens green + `x*1.0==x` over every path), as the *general* bundle (thrust_factor + reserved room), and `effective_scales_with_thrust_factor` proves it's non-tautological. **Residual (keep open until Plan B lands):** the *shape* is only vindicated when Plan B builds on it **purely additively** — a new effective channel must be a new `EffectiveMods` field, NEVER a second `effective_params` signature break. Also: B must audit for direct `base_*` reads that bypass the seam (the reset guard is one, deliberately deferred). | ~~M×H~~ → **L×M** | Confirm at Plan-B review: zero `effective_params` signature change; all crew-modified params flow through the seam. |
| R2 | **Plan B's `HASH_FORMAT_VERSION` 1→2 + dual state-golden re-derive** is the one change allowed to move *both* `0xf0dd` and `0x532d`. Batching or mis-attribution corrupts determinism provenance. | M×M | Single-cause discipline: B owns the bump alone; re-derive from a real `print_golden` run; independently re-verify. Guidance already kept its goldens single-cause (no version bump) to leave this clean. |
| R3 | **Thesis unproven — gym surface (Plan-4) deferred behind the Person line.** `jumpgate-py` is a 21-line scaffold; no agent has been trained. Risk: substrate fidelity compounds while "DRL > scripted" stays undemonstrated. | M×M | **ACCEPTED** (owner, 2026-06-09) — debt-avoidance rationale: build the env against the settled Plan-A seam to avoid reshaping it. **Trigger to revisit:** if the Person line slips materially, OR doubt grows that the substrate can host a trainable agent → pull Plan-4 forward (it gates on Plan A only, not B/C). |
| R4 | Subagent-driven execution **fabricates gate claims** (test runs, measurements, modules). | L×H | MITIGATED & standard: main loop independently re-runs every gate (`cargo test`, `clippy --all-targets`, grep goldens) before trusting any "green" claim. Keep this non-negotiable. |
| R6 | **The ecosystem may not be trainable at acceptable throughput.** The RL bottleneck is CPU env steps/sec (strategy memory; metrics.md input metric). A multi-agent ecosystem with many actors stepped at sim-second granularity over week/month horizons could be too slow to train a policy in feasible wall-clock. This is PDR-0002 reversal trigger (a) — it could force a smaller venue. **First reading (2026-06-09, Plan-4 navigator rung): ~600k steps/sec single-env** through the Python binding (release) — a healthy floor at single-agent scale. | ~~M×H~~ → **M×M** | Single-env floor is comfortable; the open question is now N-agent ecosystem scale, not single-step cost. The LOD design (LOD_NOTHING closed-form dormancy, schedule-the-next-event — dormant agents ~0/tick) is the lever; shaping must budget steps/sec against the *concurrent-awake* agent count and re-measure once the ecosystem loop exists. |
| R5 | **Putting DRL where it has no room (the thesis quietly fails).** A small, tractable, presolvable market hosts **computation, not judgment** (`vsl-cannot-host-judgment-principle`: replay-determinism IS presolvability) — DRL only beats scripts where the problem has genuine *room* (population-scale contention / intractable density / a coupled loop held off equilibrium, NOT a closed-form clearing price). **MATERIALIZED & CHARACTERIZED 2026-06-09** by the shaping pass (`docs/superpowers/reviews/2026-06-09-vertical-slice-shaping-findings.md`): no DRL room is demonstrable inside the thin economy — 3 of 6 arenas provably presolvable, 3 LOW/MEDIUM + unmeasured; buildability anti-correlated with room. | ~~M×H~~ → **M×M (mitigation accepted)** | **MITIGATION ACCEPTED (owner, PDR-0005): reposition the DRL thesis off the thin market onto the scale/density/population path.** Build the first loop as a *deterministic harness only* (not the DRL win); design the dense/population arena as a separate later shaping pass; gate the first learner behind the cheap analytic cut (fraction-of-ceiling, telemetry-ablated, vs best-closed-form). **Residual / reversal trigger (PDR-0005):** if the dense arena, once designed + measured, ALSO fails the 10% gate at feasible N-agent throughput (R6), the measurable-decisions thesis is falsified for v1 → fall back to the entertaining-emergence frame or re-scope — one repositioning, not an infinite arena search. |

## Assumptions
| # | Assumption | If false |
|---|---|---|
| A1 | The substrate foundation (`world.rs` step/reset, `stores.rs`, `config.rs`/`hash.rs`) is stable — verified clean + green at the guidance handover (2026-06-09). | Person A & Plan-4 would be building on a moving foundation; re-establish stability first. |
| A2 | v1's relaxed determinism (seed-reproducible on same build; no cross-platform bit-identity) is sufficient for RL replay/debugging. | If cross-platform replay is needed, the FP profile / golden strategy changes materially. |

## Issues (active problems, not risks)
*(none open)* — the autopilot braking-law gap, μ inconsistency, and state_hash hardening that were live are all closed (see git `dfa1e77`, `bf97147`, `2a4d556`).

## Dependencies
| # | Dependency | Provider → Consumer | Status / Owner |
|---|---|---|---|
| D1 | **Reset-ordering cross-line contract:** when `EffectiveMods` multiplies `max_thrust` (non-identity crew effect), guidance's D6/§6.5 `World::reset` resolvability guard must validate the *crew-modified* `max_thrust`. The Person line resolves the `mods` column **at reset, before the guard runs**. | Person Plan B → guidance reset guard | **OWNED in Plan B** (`jumpgate-12f37a8d74`) as an acceptance criterion. Identity in v1 → inert today; honor ordering when B lands. |
| D2 | Plan-4 gym surface should be built against the **settled `effective_params`/`EffectiveMods` seam**. | Plan A → Plan-4 | Wired: `jumpgate-5a3e01ab08` blocked_by `jumpgate-d30fcebaac`. |
| D3 | Python toolchain: `archive/.venv` (maturin 1.12, torch 2.9.1, gymnasium 1.2.3, abi3-py312). | external → Plan-4 | Available; confirm still intact before Plan-4. |

## Open decisions for the owner
1. **v1 done-definition** — confirm/replace the falsifiable DRL-beats-baseline criterion (charter).
2. **Cutover gate** — is `jumpgate-v1-design` a deliberate long-lived branch, and what cuts v1 over to `main`?
