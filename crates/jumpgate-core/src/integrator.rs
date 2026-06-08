//! Integrators (velocity-Verlet + RK4) as impls of the `Integrator` trait
//! (defined in `contract.rs`), the softened gravity kernel, and
//! reference-acceleration deterministic substepping.
//!
//! This module OWNS: `gravity_accel`, `substep_count`, `VelocityVerlet`, `Rk4`.
//! It does NOT own the `Integrator` trait — that lives in `contract.rs` and is
//! imported below. There is exactly one `Integrator` trait in the workspace.

use crate::config::SubstepCfg;
use crate::contract::Integrator;
use crate::math::{G_CANONICAL, Vec3};

/// Softened gravitational acceleration at point `p` summed over `body_positions`
/// (each `(body_pos, body_mass)`), using the kernel `G·M·d / (|d|² + ε²)^1.5`
/// with `d = body_pos − p`. A hard distance cutoff is FORBIDDEN.
///
/// `softening > 0` is REQUIRED: a body coincident with `p` at `softening == 0`
/// yields `0 · ∞ = NaN`. Production always passes a positive softening.
///
/// Bodies are summed in the GIVEN slice order. f64 add is non-associative, so
/// the iteration order IS the canonical (replay-deterministic) reduction form —
/// callers must not sort or reorder the slice between record and replay.
pub fn gravity_accel(p: Vec3, body_positions: &[(Vec3, f64)], softening: f64) -> Vec3 {
    let eps_sq = softening * softening;
    let mut acc = Vec3::ZERO;
    for &(body_pos, mass) in body_positions {
        let d = body_pos.sub(p); // vector from craft to body
        let r_sq = d.length_sq() + eps_sq;
        // Softened kernel: G·M·d / (r² + ε²)^1.5. No hard cutoff.
        let inv = 1.0 / (r_sq * r_sq.sqrt());
        acc = acc.add(d.scale(G_CANONICAL * mass * inv));
    }
    acc
}

/// `N` = pure fn of the QUANTIZED total local acceleration magnitude
/// (gravity + thrust). Identical on replay. Monotonic non-decreasing in
/// `total_accel_mag`, result always in `[1, max(1, cfg.max_substeps)]`.
///
/// Reference-acceleration schedule (fixed log base 2):
///   n = 1 + floor(log2(max(1, mag / cfg.accel_ref)))
/// `cfg.accel_ref` is a physical reference acceleration in AU/day².
///
/// DETERMINISM: the `floor(log2(...))` octave count is computed with an EXACT
/// integer-doubling loop (`threshold *= 2.0`, exact in f64) and `>=` compares —
/// NOT `f64::log2().floor()`. `log2` is an unpinned transcendental: a 1-ULP libm
/// difference near an octave boundary, after `floor()`, would flip N and produce
/// a replay-divergent trajectory. This is the same "pin one canonical arithmetic
/// form" principle that bans `mul_add` (spec §6). The doubling loop is
/// bit-identical across platforms and equals `floor(log2)` mathematically.
///
/// The loop's `octaves + 1 < cfg.max_substeps` bound subsumes the upper clamp,
/// and the unconditional `1 +` keeps the result `>= 1` even when
/// `cfg.max_substeps == 0` (never returns 0; never panics).
///
/// ASSUMPTION: config validation guarantees `cfg.accel_ref > 0` and finite. A
/// non-finite-or-non-positive `ratio` is nonetheless guarded here (→ n = 1) so a
/// bad config degrades safely rather than looping or producing garbage.
pub fn substep_count(total_accel_mag: f64, cfg: SubstepCfg) -> u32 {
    // Non-finite or non-positive accel => the floor of 1 substep.
    // (Phrased positively to avoid clippy's neg_cmp_op_on_partial_ord; NaN is
    // not finite, so NaN falls through to the early return — same semantics as
    // `!(mag > 0.0) || !mag.is_finite()`.)
    if !(total_accel_mag.is_finite() && total_accel_mag > 0.0) {
        return 1;
    }
    // Reference-acceleration ratio. accel_ref is AU/day^2 (a physical scale),
    // NOT a log base. Production configs use accel_ref < 1, so ratios >= 1 are
    // the norm; ratio < 2 => 0 octaves => n = 1. Guard a bad-config ratio
    // (non-finite or non-positive) by treating it as n = 1.
    let ratio = total_accel_mag / cfg.accel_ref;
    if !(ratio.is_finite() && ratio > 0.0) {
        return 1;
    }
    // floor(log2(max(1, ratio))) via exact doubling. `threshold` starts at 2
    // (the first octave boundary) and only ever doubles, which is exact in f64.
    // `octaves + 1 < max_substeps` caps octaves so the final n never exceeds
    // max_substeps; the unconditional `1 +` keeps n >= 1 (max_substeps == 0 → 1).
    let mut octaves: u32 = 0;
    let mut threshold = 2.0_f64;
    while octaves + 1 < cfg.max_substeps && ratio >= threshold {
        octaves += 1;
        threshold *= 2.0;
    }
    1 + octaves
}

/// Default integrator: 1 force eval per substep, two field samples (t_n, t_{n+1}).
pub struct VelocityVerlet;
/// Golden/validation integrator: 4 force evals per substep.
pub struct Rk4;

impl Integrator for VelocityVerlet {
    fn step_craft(
        &self,
        pos: Vec3,
        vel: Vec3,
        accel_at: &dyn Fn(Vec3, f64) -> Vec3,
        dt: f64,
        n_substeps: u32,
    ) -> (Vec3, Vec3) {
        let n = n_substeps.max(1);
        let h = dt / (n as f64);
        let mut p = pos;
        let mut v = vel;
        let mut t = 0.0_f64; // sub-tick time offset in days
        for _ in 0..n {
            // a_n: acceleration at the START of the substep (t_n).
            let a_n = accel_at(p, t);
            // Drift to the new position using a_n.
            let p_new = p.add(v.scale(h)).add(a_n.scale(0.5 * h * h));
            // a_{n+1}: acceleration at the END of the substep (t_{n+1}).
            // MOVING-FIELD CRITICAL: this SECOND eval (at p_new, t+h) is what
            // keeps Verlet O(dt^2). A single-eval form silently degrades to O(dt).
            let a_np1 = accel_at(p_new, t + h);
            // Kick: average the two accelerations.
            let v_new = v.add(a_n.add(a_np1).scale(0.5 * h));
            p = p_new;
            v = v_new;
            t += h;
        }
        (p, v)
    }
    fn name(&self) -> &'static str {
        "velocity_verlet"
    }
}

impl Integrator for Rk4 {
    fn step_craft(
        &self,
        pos: Vec3,
        vel: Vec3,
        accel_at: &dyn Fn(Vec3, f64) -> Vec3,
        dt: f64,
        n_substeps: u32,
    ) -> (Vec3, Vec3) {
        let n = n_substeps.max(1);
        let h = dt / (n as f64);
        let mut p = pos;
        let mut v = vel;
        let mut t = 0.0_f64;
        for _ in 0..n {
            // k1
            let k1_p = v;
            let k1_v = accel_at(p, t);
            // k2 at t + h/2
            let p2 = p.add(k1_p.scale(0.5 * h));
            let v2 = v.add(k1_v.scale(0.5 * h));
            let k2_p = v2;
            let k2_v = accel_at(p2, t + 0.5 * h);
            // k3 at t + h/2
            let p3 = p.add(k2_p.scale(0.5 * h));
            let v3 = v.add(k2_v.scale(0.5 * h));
            let k3_p = v3;
            let k3_v = accel_at(p3, t + 0.5 * h);
            // k4 at t + h
            let p4 = p.add(k3_p.scale(h));
            let v4 = v.add(k3_v.scale(h));
            let k4_p = v4;
            let k4_v = accel_at(p4, t + h);
            // weighted sum: (k1 + 2k2 + 2k3 + k4) / 6
            let sixth = h / 6.0;
            p = p
                .add(k1_p.scale(sixth))
                .add(k2_p.scale(2.0 * sixth))
                .add(k3_p.scale(2.0 * sixth))
                .add(k4_p.scale(sixth));
            v = v
                .add(k1_v.scale(sixth))
                .add(k2_v.scale(2.0 * sixth))
                .add(k3_v.scale(2.0 * sixth))
                .add(k4_v.scale(sixth));
            t += h;
        }
        (p, v)
    }
    fn name(&self) -> &'static str {
        "rk4"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;

    #[test]
    fn gravity_softened_matches_closed_form() {
        // One body of mass M at distance r along +x; craft at origin.
        let m = 3.0_f64;
        let r = 2.0_f64;
        let eps = 0.0_f64; // no softening => pure Newtonian
        let a = gravity_accel(Vec3::ZERO, &[(Vec3::new(r, 0.0, 0.0), m)], eps);
        let expected_mag = G_CANONICAL * m / (r * r); // along +x
        assert!(
            (a.x - expected_mag).abs() < 1e-12,
            "ax={} expected={}",
            a.x,
            expected_mag
        );
        assert!(a.y.abs() < 1e-15 && a.z.abs() < 1e-15);

        // Softening strictly reduces the magnitude vs the unsoftened case.
        let eps2 = 1.0_f64;
        let a_soft = gravity_accel(Vec3::ZERO, &[(Vec3::new(r, 0.0, 0.0), m)], eps2);
        assert!(
            a_soft.x < a.x && a_soft.x > 0.0,
            "softened {} should be in (0, {})",
            a_soft.x,
            a.x
        );
    }

    #[test]
    fn gravity_two_body_superposition_in_slice_order() {
        // Sum over two bodies must equal the component-wise sum of the two
        // single-body results, computed in the SAME slice order. Guards the
        // fixed-order reduction (a regression that summed only the first body,
        // or reordered, would be caught).
        let p = Vec3::new(0.4, -0.7, 0.25);
        let b1 = (Vec3::new(1.5, 0.0, 0.0), 2.0_f64);
        let b2 = (Vec3::new(-0.3, 2.1, -0.9), 0.7_f64);
        let softening = 1.0e-3_f64;

        let a_both = gravity_accel(p, &[b1, b2], softening);
        let a1 = gravity_accel(p, &[b1], softening);
        let a2 = gravity_accel(p, &[b2], softening);
        let a_sum = a1.add(a2); // same order as the two-body slice: b1 then b2

        assert!(
            (a_both.x - a_sum.x).abs() < 1e-15,
            "x: {} vs {}",
            a_both.x,
            a_sum.x
        );
        assert!(
            (a_both.y - a_sum.y).abs() < 1e-15,
            "y: {} vs {}",
            a_both.y,
            a_sum.y
        );
        assert!(
            (a_both.z - a_sum.z).abs() < 1e-15,
            "z: {} vs {}",
            a_both.z,
            a_sum.z
        );
        // Empty slice => zero acceleration (no bodies, no NaN).
        assert_eq!(gravity_accel(p, &[], softening), Vec3::ZERO);
    }

    #[test]
    fn substep_count_boundary_cases_never_zero() {
        // max_substeps == 0 must clamp UP to 1, never return 0 (the old
        // max(1).min(0)=0 footgun) and never panic.
        let cfg0 = SubstepCfg {
            accel_ref: 1.0e-4,
            max_substeps: 0,
        };
        assert_eq!(substep_count(1.0e-4, cfg0), 1);
        assert_eq!(
            substep_count(1.0e300, cfg0),
            1,
            "huge accel, cap=0 => still 1"
        );
        assert_eq!(substep_count(0.0, cfg0), 1);

        // max_substeps == 1 => always exactly 1, regardless of acceleration.
        let cfg1 = SubstepCfg {
            accel_ref: 1.0e-4,
            max_substeps: 1,
        };
        assert_eq!(substep_count(1.0e-4, cfg1), 1);
        assert_eq!(substep_count(1.0, cfg1), 1);
        assert_eq!(substep_count(1.0e300, cfg1), 1);

        // Cap saturation: a huge ratio returns exactly max_substeps.
        let cfg_cap = SubstepCfg {
            accel_ref: 1.0e-4,
            max_substeps: 7,
        };
        assert_eq!(substep_count(1.0e300, cfg_cap), 7);
        // One past the last representable octave still saturates at the cap.
        assert_eq!(substep_count(cfg_cap.accel_ref * 64.0, cfg_cap), 7);

        // Bad-config guards: non-positive / non-finite accel_ref degrade to 1.
        let cfg_bad = SubstepCfg {
            accel_ref: 0.0,
            max_substeps: 64,
        };
        assert_eq!(
            substep_count(1.0, cfg_bad),
            1,
            "ratio = x/0 = inf guarded => 1"
        );
        let cfg_neg = SubstepCfg {
            accel_ref: -1.0e-4,
            max_substeps: 64,
        };
        assert_eq!(
            substep_count(1.0, cfg_neg),
            1,
            "negative ratio guarded => 1"
        );
        let cfg_nan = SubstepCfg {
            accel_ref: f64::NAN,
            max_substeps: 64,
        };
        assert_eq!(substep_count(1.0, cfg_nan), 1, "NaN ratio guarded => 1");
    }

    #[test]
    fn substep_count_reference_accel_grounded() {
        // PRODUCTION-REGIME reference acceleration (AU/day^2), NOT a log base.
        let cfg = SubstepCfg {
            accel_ref: 1.0e-4,
            max_substeps: 64,
        };

        // Grounded schedule sweep: each entry is (mag, expected N) on the exact
        // octave ladder. (A same-process `f(x)==f(x)` check proves nothing about
        // cross-build replay; the cross-build invariant is that this fixed ladder
        // holds, which the integer-doubling impl guarantees.)
        for &(mult, want) in &[
            (1.0_f64, 1u32), // == accel_ref
            (1.999, 1),      // just under first octave
            (2.0, 2),
            (4.0, 3),
            (8.0, 4),
            (16.0, 5),
            (1024.0, 11), // 2^10 => 1 + 10
        ] {
            let n = substep_count(cfg.accel_ref * mult, cfg);
            assert_eq!(
                n, want,
                "mag = accel_ref*{} expected N={} got {}",
                mult, want, n
            );
        }

        // At/below the reference accel, exactly 1 substep.
        assert_eq!(substep_count(0.0, cfg), 1);
        assert_eq!(substep_count(cfg.accel_ref * 0.5, cfg), 1);
        assert_eq!(substep_count(cfg.accel_ref, cfg), 1);

        // Each DOUBLING above accel_ref adds exactly one substep:
        // n = 1 + floor(log2(mag/accel_ref)).
        assert_eq!(substep_count(cfg.accel_ref * 2.0, cfg), 2);
        assert_eq!(substep_count(cfg.accel_ref * 4.0, cfg), 3);
        assert_eq!(substep_count(cfg.accel_ref * 8.0, cfg), 4);
        // Just under the next octave stays in the lower bin (floor behaviour).
        assert_eq!(substep_count(cfg.accel_ref * 3.999, cfg), 2);

        // Physically-grounded: gravity from a 1 M_sun body at 1 AU and 0.1 AU.
        let m = 1.0_f64;
        let g_1au = gravity_accel(Vec3::ZERO, &[(Vec3::new(1.0, 0.0, 0.0), m)], 0.0).length();
        let g_01au = gravity_accel(Vec3::ZERO, &[(Vec3::new(0.1, 0.0, 0.0), m)], 0.0).length();
        assert_eq!(
            substep_count(g_1au, cfg),
            2,
            "1 AU should escalate past 1 substep"
        );
        assert_eq!(substep_count(g_01au, cfg), 9, "0.1 AU close approach");

        // Monotonic non-decreasing across increasing acceleration; in range.
        let mut prev = 0u32;
        let mut mag = cfg.accel_ref * 0.25_f64;
        for _ in 0..40 {
            let n = substep_count(mag, cfg);
            assert!(n >= prev, "non-monotonic at mag={}: {} < {}", mag, n, prev);
            assert!(n >= 1 && n <= cfg.max_substeps, "out of range n={}", n);
            prev = n;
            mag *= 2.0;
        }

        // Huge acceleration saturates exactly at the cap.
        assert_eq!(substep_count(1.0e300, cfg), cfg.max_substeps);
    }

    #[test]
    fn verlet_coast_is_exact_straight_line() {
        let v = VelocityVerlet;
        let pos = Vec3::new(1.0, -2.0, 0.5);
        let vel = Vec3::new(0.3, 0.1, -0.2);
        let dt = 0.5_f64;
        let zero_field = |_p: Vec3, _t: f64| Vec3::ZERO;
        for &n in &[1u32, 4, 16] {
            let (p1, v1) = v.step_craft(pos, vel, &zero_field, dt, n);
            let expected = pos.add(vel.scale(dt));
            assert!(
                (p1.sub(expected)).length() < 1e-12,
                "n={} pos drift {:?}",
                n,
                p1
            );
            assert!((v1.sub(vel)).length() < 1e-12, "n={} vel drift {:?}", n, v1);
        }
        assert_eq!(v.name(), "velocity_verlet");
    }

    #[test]
    fn rk4_coast_is_exact_straight_line() {
        let r = Rk4;
        let pos = Vec3::new(0.2, 4.0, -1.0);
        let vel = Vec3::new(-0.5, 0.0, 0.25);
        let dt = 0.5_f64;
        let zero_field = |_p: Vec3, _t: f64| Vec3::ZERO;
        for &n in &[1u32, 4, 16] {
            let (p1, v1) = r.step_craft(pos, vel, &zero_field, dt, n);
            let expected = pos.add(vel.scale(dt));
            assert!(
                (p1.sub(expected)).length() < 1e-12,
                "n={} pos drift {:?}",
                n,
                p1
            );
            assert!((v1.sub(vel)).length() < 1e-12, "n={} vel drift {:?}", n, v1);
        }
        assert_eq!(r.name(), "rk4");
    }

    #[test]
    fn near_circular_orbit_stays_bounded() {
        let v = VelocityVerlet;
        // Production-regime reference accel (AU/day^2): a 1 AU orbit gets n=2 here.
        let cfg = SubstepCfg {
            accel_ref: 1.0e-4,
            max_substeps: 64,
        };
        let m = 1.0_f64; // central mass (M_sun)
        let radius = 1.0_f64; // 1 AU
        let softening = 1.0e-6_f64;
        let body_pos = Vec3::ZERO;
        let mu = G_CANONICAL * m;
        let speed = (mu / radius).sqrt();
        let mut pos = Vec3::new(radius, 0.0, 0.0);
        let mut vel = Vec3::new(0.0, speed, 0.0);
        let dt = 1.0_f64; // 1 day per tick

        // Confirm the schedule actually escalates at this orbit (guards the redesign).
        let g0 = gravity_accel(pos, &[(body_pos, m)], softening).length();
        assert!(
            substep_count(g0, cfg) >= 2,
            "reference-accel schedule did not engage at 1 AU"
        );

        let mut r_min = f64::INFINITY;
        let mut r_max = 0.0_f64;
        for _ in 0..2000 {
            // accel closure: softened gravity from the central body (no thrust here).
            let field = |p: Vec3, _t: f64| gravity_accel(p, &[(body_pos, m)], softening);
            // accel-keyed substeps from the QUANTIZED gravity magnitude at current pos.
            let g_mag = gravity_accel(pos, &[(body_pos, m)], softening).length();
            let n = substep_count(g_mag, cfg);
            let (p1, v1) = v.step_craft(pos, vel, &field, dt, n);
            pos = p1;
            vel = v1;
            let r = pos.length();
            r_min = r_min.min(r);
            r_max = r_max.max(r);
        }
        // Bounded, not golden: radius stays within ±5% of 1 AU over 2000 days.
        assert!(
            r_min > 0.95 && r_max < 1.05,
            "orbit unbounded: r in [{}, {}]",
            r_min,
            r_max
        );
    }

    #[test]
    fn verlet_and_rk4_agree_on_coast_arc() {
        let m = 1.0_f64;
        let softening = 1.0e-6_f64;
        let body_pos = Vec3::ZERO;
        let mu = G_CANONICAL * m;
        let radius = 1.0_f64;
        let speed = (mu / radius).sqrt();
        let pos0 = Vec3::new(radius, 0.0, 0.0);
        let vel0 = Vec3::new(0.0, speed, 0.0);
        let dt = 1.0_f64;
        let n = 32u32; // fine, fixed substeps for both => fair comparison
        let field = |p: Vec3, _t: f64| gravity_accel(p, &[(body_pos, m)], softening);

        let mut pv = (pos0, vel0);
        let mut pr = (pos0, vel0);
        let verlet = VelocityVerlet;
        let rk4 = Rk4;
        for _ in 0..50 {
            pv = verlet.step_craft(pv.0, pv.1, &field, dt, n);
            pr = rk4.step_craft(pr.0, pr.1, &field, dt, n);
        }
        let pos_gap = pv.0.sub(pr.0).length();
        let vel_gap = pv.1.sub(pr.1).length();
        // Coarse agreement: well under 1% of an AU / orbital speed after 50 days.
        assert!(pos_gap < 1.0e-3, "verlet vs rk4 pos gap {}", pos_gap);
        assert!(vel_gap < 1.0e-3, "verlet vs rk4 vel gap {}", vel_gap);
    }

    #[test]
    fn verlet_is_second_order_in_a_moving_field() {
        let m = 1.0_f64;
        let softening = 1.0e-6_f64;
        // Attractor drifts during the tick => `accel_at` truly depends on sub_t (days).
        let bvel = Vec3::new(0.2, 0.0, 0.0);
        let field = |p: Vec3, t: f64| {
            let body = Vec3::new(-0.5, 0.3, 0.0).add(bvel.scale(t));
            gravity_accel(p, &[(body, m)], softening)
        };
        let pos0 = Vec3::new(1.0, 0.0, 0.0);
        let vel0 = Vec3::new(0.0, 0.9, 0.0);
        let verlet = VelocityVerlet;
        let rk4 = Rk4;
        let total = 4.0_f64;

        // High-resolution RK4 reference (fine fixed step, n=1), carrying a tick clock.
        let mut rref = (pos0, vel0);
        let ref_steps = 4096u32;
        let h = total / ref_steps as f64;
        let mut t0 = 0.0_f64;
        for _ in 0..ref_steps {
            let f = |p: Vec3, st: f64| field(p, t0 + st);
            rref = rk4.step_craft(rref.0, rref.1, &f, h, 1);
            t0 += h;
        }

        // Verlet global error at dt and dt/2; the ratio is the order signal.
        let err = |steps: u32| {
            let dt = total / steps as f64;
            let mut s = (pos0, vel0);
            let mut tt = 0.0_f64;
            for _ in 0..steps {
                let f = |p: Vec3, st: f64| field(p, tt + st);
                s = verlet.step_craft(s.0, s.1, &f, dt, 1);
                tt += dt;
            }
            s.0.sub(rref.0).length()
        };
        let ratio = err(64) / err(128);
        // 2nd order => ~4x; a single-eval (O(dt)) Verlet gives ~2x and trips this.
        assert!(
            ratio > 3.0,
            "verlet convergence ratio {} (want ~4; <3 = collapsed to first order)",
            ratio
        );
    }
}
