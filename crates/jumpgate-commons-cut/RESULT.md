# Commons-Miner Analytic Cut — RESULT

**Date:** 2026-06-10
**Spec:** `docs/superpowers/specs/2026-06-10-commons-miner-cut-design.md`
**Plan:** `docs/superpowers/plans/2026-06-10-commons-miner-cut.md`
**Issue:** `jumpgate-aec6e7bc14` (scale/density DRL arena — first increment)

## Verdict

**The cheap full-information commons-miner cut does NOT produce a trustworthy
learnable-room signal.** Read together with the six prior probes, the game-theory
analysis, and the strawman-bar collapse below, this is consistent with the
pre-registered, expected outcome: **no demonstrable learnable room in the
full-information regime.** The decision routes to the **information / Media bet**
(partial observability — spec §9), which is where every signal says the room is.

This is NOT a crisp GO/NO-GO integer, and deliberately so: the apparatus is
instrument-limited at the only tractable scale (see Evidence). It is **not**
evidence against the information game — this tested *only* the full-info game.

## What the run printed (pre-registered metric `frac = (ceiling − bar)/ceiling`, dense bar, regen=2)

```
curve (N, frac, lo, hi): [(3, 0.122, …), (6, 0.0, …), (12, 0.0, …), (24, 1.0, …)]
mechanical verdict():    NoGo   (triggered by the post-N=3 decay)
negative_control_nogo:   true   (apparatus fairness: identical regions NO-GO)
planner_headroom_frac:   0.000  (no coordination headroom in the regen regime)
```

The mechanical `verdict()` = NoGo, but it is processing an **uninterpretable
curve** — a real room signal cannot read `0.122 → 0 → 0 → 1.0`. So the honest
finding is "apparatus cannot resolve," not a clean NoGo.

## Evidence the signal is not trustworthy (three independent confounds)

1. **Scale paradox (spec §8), confines the only exact rung to N=3.** The exact DP
   is tractable only at N=3, where the herd/anti-coordination tension cannot
   structurally occur and a lone best-responder dodging two near-frozen others
   manufactures residual room. The spec pre-registered that an N=3 point estimate
   is untrustworthy; honor it.

2. **The N=3 apparent room is a strawman-bar artifact.** Pre-committed bar-strength
   check: refitting the closed-form on a finer grid (a stronger bar can only
   *shrink* apparent room → strictly guards against false-GO) collapsed the N=3
   frac from **0.118 → ~0.005** (best fine-fit at tau=1, mp=450). The coarse 4×4
   fit grid was under-fitting the deployable script. (A residual confound remains:
   `fit_closed_form` maximises *population-total* yield, not the *slot-0* value the
   bar measures — a fit-objective mismatch that still slightly inflates the bar's
   apparent room. Not fixed: it does not change the conclusion, since the N>3
   ladder is noise-dominated regardless.)

3. **The MC-estimated N-ladder is noise-dominated.** `0.122 → 0 → 0 → 1.0` is not a
   trend; the bounded-depth-greedy MC against a deterministic field gives erratic,
   non-monotone estimates (incl. an absurd frac=1.0 at N=24 where heavy crowding
   floors the bar toward 0). The N-scaling gate — the entire point of the ladder —
   cannot be evaluated. The exact DP only calibrated the MC at small N; it does not
   make the MC trustworthy at N≫M.

## What IS trustworthy (the apparatus works; it just can't resolve THIS at scale)

- **Determinism**: the golden trajectory hash (`0x5701_7d18_ffff_70c6`) holds across
  processes; all dynamics/DP are integer + replayable.
- **DP correctness**: the phantom-ceiling cross-check (`V₀ == realized`, load-bearing
  after a tautology was caught), DP==brute-force, `closed-loop ≤ open-loop`,
  `planner ≥ selfish` all genuinely hold.
- **Apparatus positive control**: the Task-9 `prefers_a_move_when_it_pays` test
  proves the BR detects room (20 vs 4) when room genuinely exists — so a near-zero
  frac under a strong bar is a real "closed-form ≈ optimal," not a BR-can't-move bug.
- **Comparability fixed**: floor/bar/ceiling are all slot-0 against the same field
  (mimic invariant `ceiling ≥ bar` verified), and the metric is the pre-registered
  `/ceiling` (an earlier `/(ceiling−floor)` caused a 0/0 artifact).

## Honoring PDR-0005

A NO-GO / no-trustworthy-signal here is **reported and honored**; it does **not**
kill the arena. It routes the DRL bet to its pre-registered next gate: the
**partial-observability / information-room** experiment (the Media engine —
`docs/superpowers/concepts/media-observability-engine.md`), authorized by the
owner *after* this result. That escalation is the owner's call, not automatic
(the reversal trigger stays with the human).

## Cost note

This was the cheap sanity probe. It ballooned (the measurement was subtle:
multi-agent comparability, the metric denominator, regime degeneracy, a strawman
bar, MC noise). Five real plan bugs and several hollow-green tests were caught
along the way — the apparatus is sound. But the instrument cannot crisply resolve
the full-info commons at tractable scale, and it does not need to: the decision it
gates (go to the information bet) is supported by every line of evidence.
