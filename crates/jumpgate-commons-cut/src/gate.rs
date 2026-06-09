//! The gate (spec §5): fraction-of-ceiling + the pre-registered GO/NO-GO threshold.
//!
//! `f64` is permitted in this module — it is MEASUREMENT/reporting, strictly
//! downstream of the integer arena/DP. No `f64` value here is ever fed back into
//! a transition (LAW 2). `fraction_of_ceiling` is a pure `(ceiling, bar, floor)`
//! function; the caller must pass per-ship-comparable values (LAW 5).

/// Pre-registered gate threshold (spec §2). Do NOT move post-hoc.
pub const GAP_FRAC_MIN: f64 = 0.10;

/// Fraction-of-ceiling (spec §5): how much of the head-room between the
/// constant floor and the closed-loop best-response ceiling the best closed-form
/// rung *fails* to close — i.e. the room a learner could still claim.
///
/// `frac = (ceiling - bar) / (ceiling - floor)`.
///
/// `f64` is permitted here — this is MEASUREMENT, downstream of the integer sim,
/// never fed back into a transition (LAW 2). Degenerate range (`ceiling <= floor`)
/// returns `0.0`, never `NaN` (LAW 5). The result is clamped to `>= 0.0` so a bar
/// above the ceiling (measurement noise) reads as "no room", not negative room.
pub fn fraction_of_ceiling(ceiling: f64, bar: f64, floor: f64) -> f64 {
    let range = ceiling - floor;
    if range <= 0.0 {
        return 0.0;
    }
    ((ceiling - bar) / range).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frac_definition_is_ceiling_minus_bar_over_ceiling_minus_floor() {
        // ceiling=100, bar=95, floor=20 -> (100-95)/(100-20) = 5/80 = 0.0625 -> below 0.10.
        let f = fraction_of_ceiling(100.0, 95.0, 20.0);
        assert!((f - 0.0625).abs() < 1e-9);
        assert!(f < GAP_FRAC_MIN);
        // ceiling=100, bar=80, floor=20 -> 20/80 = 0.25 -> GO-eligible.
        assert!(fraction_of_ceiling(100.0, 80.0, 20.0) >= GAP_FRAC_MIN);
    }

    #[test]
    fn degenerate_zero_range_is_zero_frac_not_nan() {
        assert_eq!(fraction_of_ceiling(20.0, 20.0, 20.0), 0.0);
    }
}
