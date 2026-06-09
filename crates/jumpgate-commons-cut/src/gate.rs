//! The gate (spec §5): fraction-of-ceiling + the pre-registered GO/NO-GO threshold.
//!
//! `f64` is permitted in this module — it is MEASUREMENT/reporting, strictly
//! downstream of the integer arena/DP. No `f64` value here is ever fed back into
//! a transition (LAW 2). `fraction_of_ceiling` is a pure `(ceiling, bar, floor)`
//! function; the caller must pass per-ship-comparable values (LAW 5).

/// Pre-registered gate threshold (spec §2). Do NOT move post-hoc.
pub const GAP_FRAC_MIN: f64 = 0.10;

/// Fraction-of-ceiling — the PRE-REGISTERED gate metric (PDR-0005 / spec §2:
/// "gap < 10% **of ceiling**"): the share of the ceiling the best closed-form rung
/// *fails* to capture — the room a single learner could still claim over the
/// deployable script.
///
/// `frac = (ceiling - bar) / ceiling`.
///
/// (An earlier implementation used `/(ceiling - floor)`, which collapsed to 0/0
/// whenever the do-nothing constant already equalled the ceiling — a metric
/// artifact, not a finding. This is the pre-registered `/ceiling` form: when the
/// closed-form is already single-agent optimal (`ceiling == bar`) it yields a
/// clean `0.0`.)
///
/// `f64` is MEASUREMENT only, downstream of the integer sim, never fed into a
/// transition (LAW 2). A genuinely dead instance (`ceiling <= 0`) returns `0.0`,
/// never `NaN`; the result is clamped `>= 0.0` so a bar above the ceiling
/// (estimator noise) reads as "no room", not negative room.
pub fn fraction_of_ceiling(ceiling: f64, bar: f64) -> f64 {
    if ceiling <= 0.0 {
        return 0.0;
    }
    ((ceiling - bar) / ceiling).max(0.0)
}

/// The pre-registered verdict (spec §5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Go,
    NoGo,
    Inconclusive,
}

/// The N-scaling gate (spec §5/§8). GO iff `frac(N) >= GAP_FRAC_MIN` at the
/// smallest (exact-DP) rung AND flat-or-rising in N (CI-aware: do not GO when
/// the CI straddles the gate at the smallest rung). NO-GO iff below the gate at
/// the smallest rung OR decaying toward the gate as N rises (the LLN
/// self-averaging signature). Inconclusive when the CI straddles the gate and
/// the mean is on the GO side (widen sampling).
///
/// Each rung: `(N, frac_mean, frac_ci_lo, frac_ci_hi)`. Smallest N first
/// (the exact-DP rung). `f64` here is MEASUREMENT only (LAW 2/5).
pub fn verdict(curve: &[(u32, f64, f64, f64)]) -> Verdict {
    if curve.is_empty() {
        return Verdict::Inconclusive;
    }
    let (_, m0, lo0, hi0) = curve[0];
    // smallest exact rung entirely below gate -> NO-GO
    if hi0 < GAP_FRAC_MIN {
        return Verdict::NoGo;
    }
    // CI straddles the gate but the mean is still below -> NO-GO
    if lo0 < GAP_FRAC_MIN && hi0 >= GAP_FRAC_MIN && m0 < GAP_FRAC_MIN {
        return Verdict::NoGo;
    }
    // CI straddles the gate with the mean on the GO side -> widen sampling
    if lo0 < GAP_FRAC_MIN {
        return Verdict::Inconclusive;
    }
    // above gate at the smallest rung; now require flat-or-rising (no decay toward gate)
    let mut prev = m0;
    for &(_, m, _, _) in &curve[1..] {
        if m < prev - 0.02 {
            return Verdict::NoGo; // decaying (LLN signature), 2pt tolerance
        }
        prev = m;
    }
    // final rung must still clear the gate
    let last = curve.last().unwrap();
    if last.1 >= GAP_FRAC_MIN {
        Verdict::Go
    } else {
        Verdict::NoGo
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_definition_is_ceiling_minus_bar_over_ceiling() {
        // PRE-REGISTERED metric: (ceiling - bar) / ceiling.
        // ceiling=100, bar=95 -> 5/100 = 0.05 -> below the 0.10 gate.
        let f = fraction_of_ceiling(100.0, 95.0);
        assert!((f - 0.05).abs() < 1e-9);
        assert!(f < GAP_FRAC_MIN);
        // ceiling=100, bar=80 -> 20/100 = 0.20 -> GO-eligible.
        assert!((fraction_of_ceiling(100.0, 80.0) - 0.20).abs() < 1e-9);
        assert!(fraction_of_ceiling(100.0, 80.0) >= GAP_FRAC_MIN);
        // closed-form already optimal (ceiling == bar) -> clean 0.0 (not 0/0).
        assert_eq!(fraction_of_ceiling(50.0, 50.0), 0.0);
    }

    #[test]
    fn degenerate_dead_instance_is_zero_frac_not_nan() {
        // genuinely dead instance (ceiling == 0) -> 0.0, never NaN.
        assert_eq!(fraction_of_ceiling(0.0, 0.0), 0.0);
        // bar above ceiling (estimator noise) -> clamped to 0.0, not negative.
        assert_eq!(fraction_of_ceiling(10.0, 12.0), 0.0);
    }

    #[test]
    fn verdict_go_requires_above_gate_and_non_decaying() {
        // frac flat-or-rising and all >= 0.10 -> GO
        let rising = vec![(3, 0.12, 0.10, 0.14), (6, 0.13, 0.11, 0.15), (12, 0.15, 0.13, 0.17)];
        assert_eq!(verdict(&rising), Verdict::Go);
        // decaying toward the gate as N rises -> NO-GO (LLN signature)
        let decaying = vec![(3, 0.30, 0.28, 0.32), (6, 0.18, 0.16, 0.20), (12, 0.09, 0.07, 0.11)];
        assert_eq!(verdict(&decaying), Verdict::NoGo);
        // below gate at smallest rung -> NO-GO
        let low = vec![(3, 0.04, 0.02, 0.06)];
        assert_eq!(verdict(&low), Verdict::NoGo);
        // CI straddles gate at smallest rung -> Inconclusive (widen sampling)
        let straddle = vec![(3, 0.11, 0.06, 0.16)];
        assert_eq!(verdict(&straddle), Verdict::Inconclusive);
    }
}
