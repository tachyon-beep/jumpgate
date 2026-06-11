# Lab note: PPO contact-ablation NULL (pirates rung 1)

**Date:** 2026-06-11
**Source:** `runs/pirates_ablation.log` (uncommitted bench state, promoted here
to committed provenance — the log itself stays out of git per repo law).
**Status:** REPORTED, NEVER GATED (PDR-0006). A null result is a PLAYER finding
at these prices — it triggers NOTHING.

## Run recipe

- Two PPO arms on the pirate-contact observation:
  - **contact-aware** — the agent sees the nearest-pirate-contact features.
  - **zero-masked** — the same features hard-zeroed (information ablated).
- 20,000 training steps per arm.
- `num_pirates = 2`.
- Evaluation on held-out seeds 10000–10019.

## Exact numbers

| reading | contact-aware | zero-masked |
|---|---|---|
| held-out Δcredits/episode | 1.620 ± 1.745 | 1.800 ± 1.898 |
| action share [wait, slot1..4] | [0, .681, 0, 0, .319] | [0, 0, 0, 0, 1] |
| nearest-contact log-dist at accept | 0.360 ± 0.391 (n=185) | 0.354 ± 0.398 (n=174) |

- Δcredits delta (aware − masked): **−0.180**.
- Per-decision robbery costs run **~2–20 cr in both arms** (aware: −16.0, −5.0,
  −11.2, −13.2, −12.6, −15.0, −9.0, −7.0, −2.4 by decision index 1–9; masked:
  −20.0, −5.0, −10.0, −18.0, −14.4, −7.4, −6.2, −1.6 by decision index
  1–7, 9).

## Interpretation (PLAYER finding)

Avoidance is not worth learning at these prices. The masked arm's degenerate
always-slot-4 policy loses nothing by being blind; the aware arm consumed the
contact observation but found no profit in it. With robbery costing only
~2–20 cr per incident against contract income, the price of getting robbed is
too small for risk-reading to pay its training cost — this is a statement about
the world's prices, not about the learner or the observation channel.

## Frame: REPORTED, NEVER GATED (PDR-0006)

This NULL gates nothing and triggers no shaping. The lawful response to "the
agent ignores a signal" is the no-shaping/add-capacity principle
(`jumpgate-no-shaping-add-capacity-principle`): change the WORLD so the signal
is worth reading — world prices (stakes) plus information (this rung's gossip
layer) — never reward shaping, never per-craft taste scalars. Reward stays
Δcredits. See the media rung-1 spec §10
(`docs/superpowers/specs/2026-06-11-media-rung1-gossip-design.md`) for the
world-price answer: the `media_stakes` sweep knobset over existing world knobs,
targeting "one robbery ≈ 3–5 trips' net profit" as a shape to recognize, never
a number to hit.
