//! Monte-Carlo best-response estimator (spec §5). For population sizes where the exact
//! closed-loop backward-induction DP (`dp::best_response_value_closed_loop_checked`) is
//! infeasible, this estimates ship `me`'s closed-loop best-response value by sampled,
//! bounded-depth-lookahead rollouts against the live reactive field, and reports a
//! confidence interval across samples.
//!
//! CALIBRATION (LAW 4): at small N the exact DP is ground truth and the MC's CI MUST
//! bracket it. The honest knob is the lookahead DEPTH — a bounded-depth backward
//! induction over `me`'s actions (the field stays its deterministic reactive self).
//! The calibration test (`mc_br_brackets_exact_dp_at_small_n_within_ci`) is built on a
//! transit-delayed-payoff instance where this is DISCRIMINATING, not a tautology: too-
//! shallow depths (1 and 2) provably UNDERSHOOT the exact DP because they cannot see
//! across the transit to the post-arrival mine, while the production depth (3) reaches
//! it and brackets. So the guard fails if the production lookahead is ever shallowed —
//! it tests the estimator, not just the arithmetic. Width is NOT the knob: against a
//! deterministic field the single-deviator lookahead is itself deterministic, so the CI
//! is honestly zero-width; the LAW-4 fix for a miss is to DEEPEN depth, never widen the
//! CI. f64 appears only in the CI summary — never fed back into the integer sim (LAW 2).

use crate::dynamics::step;
use crate::policies::{ClosedForm, Observation, Policy};
use crate::{Action, ArenaConfig, ArenaState};
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::Rng;

/// A point estimate with a `mean ± 1.96·SE` (95%) confidence interval across MC samples.
/// f64 = measurement only (LAW 2), downstream of the integer sim.
#[derive(Clone, Copy, Debug)]
pub struct Estimate {
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
}

/// Candidate actions for `me` from a given state: `Stay`, plus `MoveTo` each *other*
/// region. Pinned order (Stay first, then ascending region index) — determinism.
fn candidates(st: &ArenaState, me: usize) -> Vec<Action> {
    let mut c = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() {
            if r != here as usize {
                c.push(Action::MoveTo(r as u8));
            }
        }
    }
    c
}

/// The reactive field's actions for the current `st`: every ship except `me` runs the
/// fixed reactive `others` rule against the SAME tick-start state (matching the DP's
/// `others_actions`, so MC and DP drive an identical deterministic field). `me`'s slot
/// is a placeholder the caller overwrites.
fn field_actions(st: &ArenaState, me: usize, others: &ClosedForm) -> Vec<Action> {
    (0..st.ships.len())
        .map(|i| {
            if i == me {
                Action::Stay
            } else {
                others.decide(&Observation { state: st, ship_idx: i })
            }
        })
        .collect()
}

/// Bounded-depth lookahead value of taking action `a` now from `st` for `me`: the
/// immediate reward plus the best continuation up to `depth` further ticks. The field
/// reacts live each tick; the continuation maximises over `me`'s candidates (a true
/// bounded-depth backward induction over `me`'s own actions, NOT a single greedy step).
/// At `depth >= remaining horizon` this equals `me`'s exact best-response value.
fn lookahead_value(
    st: &ArenaState,
    cfg: &ArenaConfig,
    me: usize,
    a: Action,
    others: &ClosedForm,
    depth: u32,
) -> u64 {
    let mut next = st.clone();
    let mut acts = field_actions(st, me, others);
    acts[me] = a;
    let before = next.ships[me].total_yield;
    step(&mut next, &acts, cfg);
    let reward = next.ships[me].total_yield - before;
    if depth <= 1 || next.tick >= cfg.horizon {
        return reward;
    }
    let mut best_cont = 0u64;
    for ca in candidates(&next, me) {
        let v = lookahead_value(&next, cfg, me, ca, others, depth - 1);
        if v > best_cont {
            best_cont = v;
        }
    }
    reward + best_cont
}

/// Pick `me`'s action by bounded-depth lookahead. Ties broken by a sampled coin (for
/// MC diversity across samples); with no ties the choice is deterministic.
fn choose_action(
    st: &ArenaState,
    cfg: &ArenaConfig,
    me: usize,
    others: &ClosedForm,
    depth: u32,
    coin: &mut impl Rng,
) -> Action {
    let cands = candidates(st, me);
    let mut best = cands[0];
    let mut best_v: i64 = -1;
    for a in cands {
        let v = lookahead_value(st, cfg, me, a, others, depth) as i64;
        if v > best_v || (v == best_v && (coin.next_u32() & 1) == 1) {
            best_v = v;
            best = a;
        }
    }
    best
}

/// MC estimate of ship `me`'s closed-loop best-response value: `samples` independent
/// bounded-depth-lookahead rollouts (depth `depth`) with the reactive `others` field
/// live each tick. Deterministic (seeded per sample from `base_seed`). Returns the
/// mean and a 95% CI across samples. `depth` is the honest calibration knob (LAW 4).
pub fn mc_best_response(
    cfg: &ArenaConfig,
    starts: &[u8],
    me: usize,
    others: &ClosedForm,
    samples: u32,
    base_seed: u64,
    depth: u32,
) -> Estimate {
    let mut vals = Vec::with_capacity(samples as usize);
    for s in 0..samples {
        let mut st = ArenaState::from_config(cfg, starts);
        let mut streams = RngStreams::from_master(base_seed ^ (s as u64));
        while st.tick < cfg.horizon {
            let action = {
                let coin = streams.stream(RngStream::Scenario);
                choose_action(&st, cfg, me, others, depth, coin)
            };
            let mut acts = field_actions(&st, me, others);
            acts[me] = action;
            step(&mut st, &acts, cfg);
        }
        vals.push(st.ships[me].total_yield as f64);
    }
    summarize(&vals)
}

/// Mean + 95% CI (`mean ± 1.96·SE`). Empty -> all-zero. Zero variance -> `lo == hi == mean`.
fn summarize(vals: &[f64]) -> Estimate {
    if vals.is_empty() {
        return Estimate { mean: 0.0, lo: 0.0, hi: 0.0 };
    }
    let n = vals.len() as f64;
    let mean = vals.iter().sum::<f64>() / n;
    let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let se = (var / n).sqrt();
    Estimate { mean, lo: mean - 1.96 * se, hi: mean + 1.96 * se }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArenaConfig;

    /// Depth-discriminating calibration instance (LAW 4). Transit-delayed payoff: `me`'s
    /// optimal best-response abandons a near-empty region (stock 2) for a full region
    /// (stock 20) whose payoff lands only AFTER a 2-tick transit. So a lookahead too
    /// shallow to see across the transit provably *undershoots* the exact DP, and only a
    /// depth that reaches the post-arrival mine recovers it. This is what makes the
    /// bracketing assertion test the estimator rather than pass by triviality (a single-
    /// tick-max instance brackets at any depth and discriminates nothing).
    fn calib_cfg() -> ArenaConfig {
        ArenaConfig {
            regions: vec![
                crate::Region { stock: 2, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 2], vec![2, 0]],
            horizon: 8,
        }
    }

    fn calib_starts() -> [u8; 2] {
        [0u8, 0u8]
    }

    fn calib_others() -> crate::policies::ClosedForm {
        // tau high so the reactive other ALSO contests the rich region — a live,
        // re-crowding field (matches the dp.rs move-path phantom fixture).
        crate::policies::ClosedForm {
            tau: crate::STOCK_MAX as u64,
            move_prob_milli: 1000,
            seed: 1,
        }
    }

    /// LAW 4 — the load-bearing calibration guard, made discriminating. On the transit-
    /// delayed `calib_cfg`, the exact closed-loop DP value is the ground truth. The MC CI
    /// must bracket it AT THE PRODUCTION DEPTH (3) — and this is NOT a tautology, because a
    /// too-shallow lookahead (depth 1 and depth 2 here) provably falls strictly below the
    /// exact value: those depths cannot see across the 2-tick transit to the post-arrival
    /// mine, so they pick the Stay-only floor. The paired assertion therefore proves depth
    /// is load-bearing: a regression that shallowed the production lookahead would fail,
    /// and a maximally-shallow MC cannot pass. (The CI is zero-width here, which is honest:
    /// the single-deviator lookahead against a deterministic field is itself deterministic,
    /// so every sample returns the identical optimal total. Width is NOT the calibration
    /// knob — DEPTH is; per LAW 4 the fix for a miss is to deepen, never to widen.)
    #[test]
    fn mc_br_brackets_exact_dp_at_small_n_within_ci() {
        let cfg = calib_cfg();
        let starts = calib_starts();
        let others = calib_others();
        let (exact, _) =
            crate::dp::best_response_value_closed_loop_checked(&cfg, &starts, 0, &others);

        // Too-shallow lookaheads MUST undershoot — proves the guard has teeth and that the
        // bracketing below is not won by triviality. depth 1 sees only the immediate tick;
        // depth 2 still cannot reach across the 2-tick transit to the post-arrival mine.
        for shallow_depth in [1u32, 2u32] {
            let shallow = mc_best_response(&cfg, &starts, 0, &others, 64, 7, shallow_depth);
            assert!(
                shallow.hi < exact as f64,
                "depth {shallow_depth} must STRICTLY undershoot exact {exact} \
                 (else the calibration is a tautology); got CI [{:.3}, {:.3}]",
                shallow.lo,
                shallow.hi
            );
        }

        // Production depth (3) reaches the post-arrival mine and recovers the exact value,
        // so the MC CI brackets the DP ground truth (LAW 4). If this ever failed on a live
        // instance, the honest fix is to DEEPEN the lookahead, never to widen the CI.
        let prod = mc_best_response(&cfg, &starts, 0, &others, 64, 7, 3);
        assert!(
            prod.lo <= exact as f64 && exact as f64 <= prod.hi,
            "exact {exact} must lie in production-depth MC CI [{:.3}, {:.3}] (mean {:.3})",
            prod.lo,
            prod.hi,
            prod.mean
        );
    }

    #[test]
    fn degenerate_empty_samples_is_zero_not_nan() {
        let cfg = calib_cfg();
        let others = calib_others();
        let est = mc_best_response(&cfg, &calib_starts(), 0, &others, 0, 7, 3);
        assert_eq!(est.mean, 0.0);
        assert_eq!(est.lo, 0.0);
        assert_eq!(est.hi, 0.0);
    }
}
