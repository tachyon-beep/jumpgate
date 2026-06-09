//! Machine-readable verdict summary (spec §6/§10) + the pre-registered run.
//!
//! A harness asserts on the `CutSummary` struct, not on `println!` output. The
//! `#[ignore]` `run_the_cut` is THE pre-registered experiment: it sweeps the
//! regen × field-correlation knobs, builds the N-ladder (exact DP at N=3, MC
//! above), computes the verdict, prints it, and returns the summary. The
//! verdict is a FINDING, not a target — `run_the_cut` asserts only apparatus
//! fairness (the identical-regions negative control must be NO-GO).

use crate::gate::Verdict;

/// Machine-readable verdict (spec §10) — a harness asserts on this, not on println output.
#[derive(Clone, Debug, PartialEq)]
pub struct CutSummary {
    pub verdict: Verdict,
    /// (N, frac_mean, ci_lo, ci_hi) per ladder rung, smallest N first.
    pub curve: Vec<(u32, f64, f64, f64)>,
    /// Apparatus fairness: the identical-regions negative control must be NO-GO.
    pub negative_control_nogo: bool,
    /// The labelled coordination-headroom upper bound (reported, NOT gated).
    pub planner_headroom_frac: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_summary_carries_verdict_and_curve() {
        let s = CutSummary {
            verdict: crate::gate::Verdict::NoGo,
            curve: vec![(3, 0.04, 0.02, 0.06)],
            negative_control_nogo: true,
            planner_headroom_frac: 0.40,
        };
        assert_eq!(s.verdict, crate::gate::Verdict::NoGo);
        assert!(s.negative_control_nogo, "apparatus fairness: identical regions must NO-GO");
        assert!(s.planner_headroom_frac > s.curve[0].1, "planner upper bound exceeds selfish frac");
    }
}

/// THE PRE-REGISTERED RUN (spec §2/§5). `#[ignore]` — invoked deliberately, not in CI.
/// Sweeps regen × field-correlation, builds the N-ladder (exact DP at N=3, MC above),
/// computes the verdict, prints it, and returns the CutSummary.
#[cfg(test)]
mod run {
    use super::*;
    use crate::dp::{best_response_value_closed_loop_checked, planner_value};
    use crate::gate::{fraction_of_ceiling, verdict};
    use crate::mc::mc_best_response;
    use crate::policies::{fit_closed_form, rollout, Constant};
    use crate::rng_bridge::build_scenario;

    /// Bounded lookahead depth for the MC best-response (the honest calibration knob,
    /// LAW 4). Matches the depth used to DP-calibrate the estimator in Task 13.
    const MC_DEPTH: u32 = 3;
    /// MC samples per eval seed at the truncated rungs.
    const MC_SAMPLES: u32 = 128;

    #[test]
    #[ignore = "the pre-registered experiment — run deliberately: cargo test -p jumpgate-commons-cut --ignored run_the_cut"]
    fn run_the_cut() {
        let train: Vec<u64> = (1000..1008).collect();
        let eval: Vec<u64> = (2000..2008).collect();
        let summary = execute(&train, &eval, 0 /*one-shot regen*/, 0 /*independent corr*/);
        println!("CUT VERDICT: {:?}", summary.verdict);
        println!("  curve (N, frac, lo, hi): {:?}", summary.curve);
        println!("  negative_control_nogo: {}", summary.negative_control_nogo);
        println!("  planner_headroom_frac (NOT learnable): {:.3}", summary.planner_headroom_frac);
        // No assertion on Go/NoGo — the verdict is the finding. Only apparatus fairness is asserted.
        assert!(summary.negative_control_nogo, "negative control must NO-GO or the apparatus is rigged");
    }

    /// The experiment body, separated so it is unit-testable on tiny inputs.
    pub fn execute(train: &[u64], eval: &[u64], regen: u32, corr: u32) -> CutSummary {
        // N=3 exact rung (M=3).
        let cf = fit_closed_form(3, 3, regen, corr, train);
        let others = crate::policies::ClosedForm {
            tau: cf.tau,
            move_prob_milli: cf.move_prob_milli,
            seed: cf.seed,
        };
        let (mut ceil_sum, mut bar_sum, mut floor_sum) = (0f64, 0f64, 0f64);
        for &s in eval {
            let cfg = build_scenario(s, 3, 3, regen, corr);
            let starts = [0u8, 1, 2];
            let (c, _) = best_response_value_closed_loop_checked(&cfg, &starts, 0, &others);
            // Single-deviator ceiling: ONE ship's best-response take (LAW 2 — leave undivided).
            ceil_sum += c as f64;
            // Population rungs are TOTALS — divide to a per-ship mean to match the single-ship ceiling.
            bar_sum += rollout(&cfg, &starts, &cf).iter().sum::<u64>() as f64 / 3.0;
            floor_sum += rollout(&cfg, &starts, &Constant).iter().sum::<u64>() as f64 / 3.0;
        }
        let n = eval.len() as f64;
        let frac3 = fraction_of_ceiling(ceil_sum / n, bar_sum / n, floor_sum / n);
        // (CI at N=3 is exact -> degenerate interval = the point.)
        let mut curve = vec![(3u32, frac3, frac3, frac3)];

        // MC-carried rungs (N=6,12,24 at fixed M=3): estimate the single-deviator ceiling via MC.
        for &nn in &[6u32, 12, 24] {
            let (mut cl, mut ch, mut bar, mut flo) = (0f64, 0f64, 0f64, 0f64);
            for &s in eval {
                let cfg = build_scenario(s, 3, 3, regen, corr); // M=3 fixed; N scales via starts
                let starts: Vec<u8> = (0..nn).map(|i| (i % 3) as u8).collect();
                let est = mc_best_response(&cfg, &starts, 0, &others, MC_SAMPLES, s, MC_DEPTH);
                cl += est.lo;
                ch += est.hi;
                // Population rollouts are TOTALS — divide by the ship count (LAW 2).
                bar += rollout(&cfg, &starts, &cf).iter().sum::<u64>() as f64 / nn as f64;
                flo += rollout(&cfg, &starts, &Constant).iter().sum::<u64>() as f64 / nn as f64;
            }
            let mean_ceiling = (cl + ch) / (2.0 * n);
            let frac_mean = fraction_of_ceiling(mean_ceiling, bar / n, flo / n);
            let frac_lo = fraction_of_ceiling(cl / n, bar / n, flo / n);
            let frac_hi = fraction_of_ceiling(ch / n, bar / n, flo / n);
            curve.push((nn, frac_mean, frac_lo.min(frac_hi), frac_lo.max(frac_hi)));
        }

        // Negative control: identical regions (corr=1000) must NO-GO.
        let neg = {
            let cfgn = build_scenario(eval[0], 3, 3, regen, 1000);
            let starts = [0u8, 1, 2];
            let (c, _) = best_response_value_closed_loop_checked(&cfgn, &starts, 0, &others);
            let bar = rollout(&cfgn, &starts, &cf).iter().sum::<u64>() as f64 / 3.0;
            let flo = rollout(&cfgn, &starts, &Constant).iter().sum::<u64>() as f64 / 3.0;
            fraction_of_ceiling(c as f64, bar, flo) < crate::gate::GAP_FRAC_MIN
        };

        let planner = {
            let cfg = build_scenario(eval[0], 3, 3, regen, corr);
            let p = planner_value(&cfg, &[0, 1, 2]) as f64;
            let flo = rollout(&cfg, &[0, 1, 2], &Constant).iter().sum::<u64>() as f64;
            fraction_of_ceiling(p, flo, 0.0) // headroom of the planner total over the floor total
        };

        CutSummary {
            verdict: verdict(&curve),
            curve,
            negative_control_nogo: neg,
            planner_headroom_frac: planner,
        }
    }

    #[test]
    fn execute_runs_and_negative_control_holds_on_tiny_inputs() {
        let s = execute(&[1u64, 2], &[3u64], 0, 0);
        assert!(!s.curve.is_empty());
        // On independent regions the negative control (corr=1000) should be NO-GO by construction.
        assert!(s.negative_control_nogo);
    }
}
