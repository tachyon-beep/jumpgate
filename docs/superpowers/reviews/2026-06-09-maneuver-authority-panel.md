# Maneuver-Authority SME Panel — synthesis (2026-06-09)

Four independent SME reviews of "the navigator scenario is degenerate (thrust below gravity)": `yzmir-simulation-foundations:stability-analyst`, `yzmir-deep-rl:rl-training-diagnostician`, `axiom-determinism-and-replay:determinism-reviewer`, `bravos-simulation-tactics:simulation-architect`. Convened by the PM; this is the synthesis + decisions. (Advisor was rate-limited, so the sim-foundations report lacked a 2nd-model pass — weight its info-gaps accordingly.)

## The headline (sharper than the original diagnosis)
The RL diagnostician proved the navigator env is **degenerate in THREE independent, stacked ways** — every policy (do-nothing, random, hand-coded, oracle) gets **exactly −1.0 return**; there is no gradient, so no algorithm can learn it. Fixing any one alone is insufficient:

1. **Authority** — `a_thrust/a_grav ≈ 0.017–0.034` (thrust is ~2–3% of gravity). Action is noise on a gravity-owned trajectory.
2. **Δv reachability (decisive)** — the destination is a *static inertial Position*, so "arrival" requires shedding orbital speed (Δv ≈ 0.015 AU/day), but the total fuel Δv budget is ~6.9e-4 (~4% of required). **Arrival is physically impossible by any trajectory.** Δv = `v_e·ln(m_full/m_empty)` is **independent of dt and scale** → the physics/dt fix CANNOT fix this.
3. **Reward f32-annihilation** — at this mass scale per-tick fuel spent (~1e-14) is below f32 epsilon vs the time penalty (0.001), so `−1e-14 − 0.001 == −0.001` exactly. The only action-dependent reward term carries zero information.

Root cause: `config_template` numbers were crushed to satisfy the §6 anti-tunnel reset guard — a determinism constraint satisfied by destroying physical authority + Δv.

## The fix set for a LEARNABLE navigator (all required — independent gates)
1. **Authority** (sim-foundations): `dt=0.25` + per-craft thrust ≈ 3× gravity at 1 AU → `a_thrust/a_grav` ∈ [0.75…75] across 0.5–5 AU. (Or loosen `ARRIVAL_RADIUS`; see below.)
2. **Δv reachability** (RL): change destination semantics from inertial `Position` to a **co-orbiting rendezvous target** (`NavDest::Entity(Body)`, `dest_vel ≠ 0`) — needs only *transfer* Δv, not kill-all-orbital-velocity. The machinery already exists (`events.rs:130-137`). **This is also exactly what the economy needs** (haulers rendezvous with stations on moving bodies).
3. **Reward** (RL): rescale so the fuel term is f32-visible (per-tick magnitude ≳ 1e-3), and add **potential-based dense progress shaping** `F = γΦ(s')−Φ(s)`, `Φ = −w·dist` (and/or on closing-speed) so there's gradient before first arrival. Keep it potential-based to avoid reward-hacking.
4. **Re-verify the §6 guard passes** at the new numbers (don't trade non-learnability for a reset panic).
5. **Action-space ↔ decode mismatch** (RL, latent): Gym `Box(-1,1,4)` vs `decode_action` reading AU offsets + a `burn_budget<0 ⇒ None` cliff. Normalize/rescale inside the decoder; drop or gate the cliff.

**Pre-GPU gate (make it a CI regression test):** a 3-policy return-separation check — {do-nothing, random, burn-straight} returns must measurably DIFFER, and a scripted oracle must hit `terminated=True` sometimes. Today all return exactly −1.0; that degeneracy signature is the falsifiable "broken" test.

## Foundational decisions (feed the vertical-slice shaping pass — the economy needs the same answers)
- **Maneuver-authority regime**: `dt=0.25`, thrust ~3× local gravity, traffic at 0.5–5 AU. Authority is radius-dependent (sub-unity inside ~0.7 AU).
- **Arrival = co-orbiting rendezvous**, not inertial point (stations sit on moving bodies).
- **Promote `ARRIVAL_RADIUS`** from a `pub const` (autopilot.rs:23) to a config field (it's already read by the guard via `cfg.guidance.k_brake`); consider default 1e-3 AU (~150,000 km — still invisible at interplanetary scale; arrival = "strategic rendezvous," not docking).
- **LOD tiering** (sim-tactics) for the economy: substep ≠ authority — authority is set by **control cadence**, not integration cadence. Coarse tier must be **coast-only** (so the guard only binds thrust-bearing tiers); finer burn cadence (dt=0.1/0.05) for active/combat; `LOD_NOTHING` dormant via closed-form arc + scheduled wake. **Tier predicate must be quantized** (integer-doubling distance ladder — same ULP discipline as `substep_count` avoiding `log2`).
- Decision-cadence separation: agent decides routes at a coarse strategic cadence; the autopilot executes maneuvers at a finer cadence underneath (the autopilot law is already dt-independent, autopilot.rs:44).

## Determinism guardrails (determinism-reviewer)
- **All 3 committed goldens are tick-0/reset-only.** So trajectory-moving changes (guard, integrator, dt in the *gym config_template*) move **no committed golden** — they move runtime recordings, which `config_hash` + provenance already reject. **Tuning in `env.rs::config_template` is the golden-clean lever.**
- **Substep-aware guard reform is hash-INERT** (the verdict is never folded; goldens are tick-0) — BUT do not do it unilaterally: it desyncs from the runtime §6.6 backstop `debug_assert`, and the only *provably static* reset-time substep floor is **n=1** (substep count is per-tick dynamic). **Reform guard + backstop together, against the same `substep_count` schedule, with a consistency test** (a reset-accepted config must never trip the backstop). One golden per cause; config-field add → re-pin `0x278c`; hashed-state-field → version bump + both state goldens.

## The one genuine expert disagreement (flag, don't paper over)
Sim-foundations proposes a **substep-aware guard** (`a_max·(dt/n_guard)² < R/(2k)`, `n_guard = substep_count(a_max_empty)`), self-funding because substep_count is monotone in accel. Determinism-reviewer counters that the **provable static n-floor at reset is n=1**, so substep-awareness may be unsound in general. Resolution: it holds for the *worst-case braking* condition (empty tank, thrust active → n ≥ n_guard), but it's subtle and needs the consistency test. **Recommended: sidestep it for the navigator** — use the config-template levers (dt/scale/rendezvous), which need NO guard reform and are golden-clean. Treat substep-aware guard reform as a separate, carefully-reviewed change only if the economy later needs high thrust at coarse dt.

## Net way-forward
The navigator follow-on (`jumpgate-3497ef9e6e`) gets the 5-item fix set + the pre-GPU gate. The foundational items (authority regime, rendezvous arrival, ARRIVAL_RADIUS-as-config, LOD tiering) become explicit inputs to the vertical-slice shaping pass (`jumpgate-818a04bb6b`) — because the economy hits the identical wall. Determinism cost is near-zero if tuning stays in `config_template`.
