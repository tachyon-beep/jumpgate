# Jumpgate v1 — RAID Log

*Risks · Assumptions · Issues · Dependencies. Living artifact — review when a line lands or a decision changes, not as a kickoff relic. Owner: John. PM: Claude. Created 2026-06-09.*

Exposure = Likelihood × Impact (L/M/H each). Re-score on review.

## Risks
| # | Risk | Exp. | Mitigation / Trigger |
|---|---|---|---|
| R1 | **The `EffectiveMods` seam (Plan A) is the one irreversible signature change.** If the bundle shape is wrong, downstream (Person B/C, gym obs) needs rework — the exact tech debt the owner most wants to avoid. | **M×H** | Already designed as a *general* modifier bundle reserving wear/component/heat factors, so `effective_params` changes signature once ever. **Mitigation:** prove trajectory-equivalence (mods=identity → bit-identical) before any Person type exists; review the bundle shape against the deferred north-star list, not just crew. This is the highest-value review gate in the line. |
| R2 | **Plan B's `HASH_FORMAT_VERSION` 1→2 + dual state-golden re-derive** is the one change allowed to move *both* `0xf0dd` and `0x532d`. Batching or mis-attribution corrupts determinism provenance. | M×M | Single-cause discipline: B owns the bump alone; re-derive from a real `print_golden` run; independently re-verify. Guidance already kept its goldens single-cause (no version bump) to leave this clean. |
| R3 | **Thesis unproven — gym surface (Plan-4) deferred behind the Person line.** `jumpgate-py` is a 21-line scaffold; no agent has been trained. Risk: substrate fidelity compounds while "DRL > scripted" stays undemonstrated. | M×M | **ACCEPTED** (owner, 2026-06-09) — debt-avoidance rationale: build the env against the settled Plan-A seam to avoid reshaping it. **Trigger to revisit:** if the Person line slips materially, OR doubt grows that the substrate can host a trainable agent → pull Plan-4 forward (it gates on Plan A only, not B/C). |
| R4 | Subagent-driven execution **fabricates gate claims** (test runs, measurements, modules). | L×H | MITIGATED & standard: main loop independently re-runs every gate (`cargo test`, `clippy --all-targets`, grep goldens) before trusting any "green" claim. Keep this non-negotiable. |
| R6 | **The ecosystem may not be trainable at acceptable throughput.** The RL bottleneck is CPU env steps/sec (strategy memory; metrics.md input metric). A multi-agent ecosystem with many actors stepped at sim-second granularity over week/month horizons could be too slow to train a policy in feasible wall-clock. This is PDR-0002 reversal trigger (a) — it could force a smaller venue. | M×H | Measure env throughput at the Plan-4 first-rung milestone (before economy build) so the number is known early. The LOD design (LOD_NOTHING closed-form dormancy, schedule-the-next-event) is the intended lever — dormant agents cost ~0/tick. Shaping must budget steps/sec against the agent count. |
| R5 | **Re-deriving the economy/combat ecosystem from scratch, and putting DRL where it has no room.** The archived line spent heavy effort learning where a demand-driven multi-agent economy produces *interesting* dynamics — and the deepest lesson (`archive/solution-architecture/25-judgment-requires-intractability.md`; memory `vsl-cannot-host-judgment-principle`) is that a small, tractable, presolvable market hosts **computation, not judgment** — DRL only beats scripts where the problem has genuine *room* (population-scale contention, not a closed-form clearing price). Building the slice without this risks an economy where the "DRL > scripted" differential is ~0 and the thesis quietly fails. | M×H | **Harvest the archive design before building** (solution-architecture 19–25, ADR-0006/0008, epic plans; the `ecosystem-oscillation` lesson — food-driven predators drive the boom/bust cycle; the navy/notoriety third trophic level). The shaping pass must explicitly site the DRL decision where it has arena-room, graded by fraction-of-ceiling, on the new 3D substrate. |

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
