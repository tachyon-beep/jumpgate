//! On-rails body ephemeris: Kepler-solve ONCE at init into integer-tick
//! position+velocity tables; O(1) tick lookup; deterministic sub-tick interp.
//!
//! No transcendentals on the lookup/interp path. The stored per-tick velocity
//! table exists so cubic-Hermite interpolation is a drop-in replacement for the
//! v1 LINEAR interp at `body_pos_subtick` (see the SEAM comment there).
//!
//! CENTRAL-BODY GUARD: the star sits at the common focus and is configured with
//! `OrbitalElements { a: 0.0, .. }`. `a == 0.0` would make `n = sqrt(mu/a^3)`
//! infinite and propagate NaN through every position; `KEPLER_A_EPSILON` pins
//! any such body to the origin with zero velocity instead. See `precompute`
//! and `kepler_state`.

use crate::config::BodyInit;
use crate::math::{Vec3, G_CANONICAL};
use crate::time::{Dt, Tick};

/// Semi-major axes below this (canonical AU) are treated as a body fixed at the
/// focus (the central star), NOT propagated by Kepler. Guards `a == 0.0` from
/// producing `n = sqrt(mu/a^3) = inf -> M = inf -> inf % TAU = NaN`.
pub const KEPLER_A_EPSILON: f64 = 1e-12;

/// Precomputed per-body position+velocity tables over a fixed tick window.
pub struct Ephemeris {
    /// `pos[body_idx][tick]` — sampled at integer ticks `0..=window`.
    pos: Vec<Vec<Vec3>>,
    /// `vel[body_idx][tick]` — same indexing; read via `body_vel` (and the
    /// cubic-Hermite drop-in seam in `body_pos_subtick`).
    vel: Vec<Vec<Vec3>>,
    /// Number of integer-tick samples per body (== window + 1).
    n_samples: usize,
    /// Sampling timestep (days), folded into Kepler propagation at precompute.
    #[allow(dead_code)] // retained for the cubic-Hermite seam (basis on dt days).
    dt: Dt,
}

impl Ephemeris {
    /// Solve Kepler once per tick sample per body; store pos+vel tables.
    /// Bodies with `a < KEPLER_A_EPSILON` are FIXED at the focus (the central
    /// star): they skip the transcendental path entirely and are written as
    /// `Vec3::ZERO`. This both models the star correctly AND guards the
    /// `n = sqrt(mu/a^3) = inf -> M = NaN` blow-up that would otherwise be
    /// laundered into the deterministic state hash.
    pub fn precompute(bodies: &[BodyInit], dt: Dt, window: u64) -> Ephemeris {
        let n_samples = (window as usize) + 1;
        let dt_days = dt.get();
        // Central gravitational parameter in canonical units (M_sun = 1).
        let mu = G_CANONICAL;

        let mut pos = Vec::with_capacity(bodies.len());
        let mut vel = Vec::with_capacity(bodies.len());

        for body in bodies {
            let e = &body.elements;
            let mut pcol = Vec::with_capacity(n_samples);
            let mut vcol = Vec::with_capacity(n_samples);

            if e.a < KEPLER_A_EPSILON {
                // FIXED body (central star): pinned to the focus, no propagation.
                for _ in 0..n_samples {
                    pcol.push(Vec3::ZERO);
                    vcol.push(Vec3::ZERO);
                }
            } else {
                // Mean motion n = sqrt(mu / a^3). Computed ONCE per body.
                let n_motion = (mu / (e.a * e.a * e.a)).sqrt();
                for k in 0..n_samples {
                    let t_days = (k as f64) * dt_days;
                    let m = e.m0 + n_motion * t_days;
                    let (p, v) = kepler_state(e.a, e.e, e.i, e.raan, e.argp, m, mu);
                    pcol.push(p);
                    vcol.push(v);
                }
            }

            pos.push(pcol);
            vel.push(vcol);
        }

        Ephemeris { pos, vel, n_samples, dt }
    }

    /// O(1) array lookup of a body position at an integer tick (clamped to window).
    /// `try_from`+`unwrap_or(usize::MAX)` (rather than `as usize`) means a tick
    /// that overflows `usize` on a 32-bit target still CLAMPS to the last sample
    /// instead of silently truncating to a wrong-but-in-range index.
    pub fn body_pos(&self, body_idx: usize, tick: Tick) -> Vec3 {
        let i = usize::try_from(tick.0)
            .unwrap_or(usize::MAX)
            .min(self.n_samples.saturating_sub(1));
        self.pos[body_idx][i]
    }

    /// O(1) array lookup of a body velocity at an integer tick (clamped to window).
    /// Same clamp discipline as `body_pos`. Exposed so callers (and this task's
    /// vis-viva test) can read the stored velocity table on the public surface;
    /// also the input to the cubic-Hermite seam in `body_pos_subtick`.
    pub fn body_vel(&self, body_idx: usize, tick: Tick) -> Vec3 {
        let i = usize::try_from(tick.0)
            .unwrap_or(usize::MAX)
            .min(self.n_samples.saturating_sub(1));
        self.vel[body_idx][i]
    }

    /// Deterministic sub-tick position between sample `tick` and `tick+1`.
    /// `frac` MUST be in `[0, 1]` — this is a caller-guaranteed contract (the
    /// Task-8 integrator advances `frac` strictly within a single tick). Values
    /// outside the range would silently EXTRAPOLATE and launder a wrong position
    /// into the state hash, so a `debug_assert` traps a caller bug in dev/test
    /// builds (deliberately not a silent clamp, which would mask the bug).
    /// SEAM: cubic-Hermite drops in here using `self.vel[body_idx][i]` and
    /// `self.vel[body_idx][i+1]` (Hermite basis on `dt.get()` days) without
    /// changing this signature or any caller.
    pub fn body_pos_subtick(&self, body_idx: usize, tick: Tick, frac: f64) -> Vec3 {
        debug_assert!((0.0..=1.0).contains(&frac), "frac must be in [0,1]");
        let i = usize::try_from(tick.0)
            .unwrap_or(usize::MAX)
            .min(self.n_samples.saturating_sub(1));
        let j = (i + 1).min(self.n_samples - 1);
        let a = self.pos[body_idx][i];
        let b = self.pos[body_idx][j];
        // LINEAR: a + (b - a) * frac.
        a.add(b.sub(a).scale(frac))
    }
}

/// Solve Kepler's equation M = E - e*sin(E) for eccentric anomaly E.
/// Fixed iteration budget => identical FP path on replay (no convergence-count
/// branch that could differ between runs).
fn solve_eccentric_anomaly(m: f64, e: f64) -> f64 {
    // Wrap mean anomaly into [-pi, pi] for a stable Newton seed.
    let two_pi = std::f64::consts::TAU;
    let mut mw = m % two_pi;
    if mw > std::f64::consts::PI {
        mw -= two_pi;
    } else if mw < -std::f64::consts::PI {
        mw += two_pi;
    }
    let mut ecc = mw; // seed
    for _ in 0..16 {
        let f = ecc - e * ecc.sin() - mw;
        let fp = 1.0 - e * ecc.cos();
        ecc -= f / fp;
    }
    ecc
}

/// Perifocal -> inertial state for classical elements at mean anomaly `m`.
/// Returns (position, velocity) in canonical units.
///
/// DEFENSIVE GUARD: `a < KEPLER_A_EPSILON` returns `(Vec3::ZERO, Vec3::ZERO)`
/// so this function is NaN-safe for any caller (mirrors the `precompute` fast
/// path; the two guards must agree).
#[allow(clippy::too_many_arguments)] // arg list mirrors OrbitalElements verbatim.
fn kepler_state(a: f64, e: f64, i: f64, raan: f64, argp: f64, m: f64, mu: f64) -> (Vec3, Vec3) {
    if a < KEPLER_A_EPSILON {
        // Fixed body at the focus; no transcendentals, no inf/NaN.
        return (Vec3::ZERO, Vec3::ZERO);
    }

    let ecc = solve_eccentric_anomaly(m, e);
    let cos_e = ecc.cos();
    let sin_e = ecc.sin();
    let sqrt_1me2 = (1.0 - e * e).sqrt();

    // Perifocal-frame position (PQW): x along periapsis, y 90deg ahead.
    let xp = a * (cos_e - e);
    let yp = a * sqrt_1me2 * sin_e;

    // Perifocal-frame velocity. r = a(1 - e cosE); Edot = n / (1 - e cosE),
    // with n = sqrt(mu/a^3). Folded: vx = -(sqrt(mu*a)/r)*sinE,
    //                               vy =  (sqrt(mu*a)/r)*sqrt(1-e^2)*cosE.
    let r = a * (1.0 - e * cos_e);
    let sqrt_mu_a = (mu * a).sqrt();
    let vxp = -sqrt_mu_a / r * sin_e;
    let vyp = sqrt_mu_a / r * sqrt_1me2 * cos_e;

    // Rotation PQW -> inertial: Rz(raan) * Rx(i) * Rz(argp).
    let cos_o = raan.cos();
    let sin_o = raan.sin();
    let cos_i = i.cos();
    let sin_i = i.sin();
    let cos_w = argp.cos();
    let sin_w = argp.sin();

    // Combined rotation matrix rows (standard orbital-elements transform).
    let r11 = cos_o * cos_w - sin_o * sin_w * cos_i;
    let r12 = -cos_o * sin_w - sin_o * cos_w * cos_i;
    let r21 = sin_o * cos_w + cos_o * sin_w * cos_i;
    let r22 = -sin_o * sin_w + cos_o * cos_w * cos_i;
    let r31 = sin_w * sin_i;
    let r32 = cos_w * sin_i;

    let pos = Vec3::new(
        r11 * xp + r12 * yp,
        r21 * xp + r22 * yp,
        r31 * xp + r32 * yp,
    );
    let vel = Vec3::new(
        r11 * vxp + r12 * vyp,
        r21 * vxp + r22 * vyp,
        r31 * vxp + r32 * vyp,
    );
    (pos, vel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OrbitalElements;
    use crate::math::Vec3;
    use crate::time::{Dt, Tick};

    /// A circular (e=0) orbit at a=1 AU keeps a constant radius across the window.
    fn circular_body() -> BodyInit {
        BodyInit {
            mass: 0.0, // test bodies are massless probes; central mass is M_sun=1
            elements: OrbitalElements {
                a: 1.0,
                e: 0.0,
                i: 0.0,
                raan: 0.0,
                argp: 0.0,
                m0: 0.0,
            },
        }
    }

    /// The central star: sits at the focus, does NOT orbit -> a = 0.0.
    fn central_star() -> BodyInit {
        BodyInit {
            mass: 1.0, // M_sun in canonical units
            elements: OrbitalElements {
                a: 0.0,
                e: 0.0,
                i: 0.0,
                raan: 0.0,
                argp: 0.0,
                m0: 0.0,
            },
        }
    }

    #[test]
    fn circular_orbit_constant_radius() {
        let dt = Dt::new(1.0);
        let window = 400u64; // ~ a bit more than one 365-day orbit
        let eph = Ephemeris::precompute(&[circular_body()], dt, window);
        let r0 = eph.body_pos(0, Tick(0)).length();
        assert!((r0 - 1.0).abs() < 1e-9, "initial radius {r0} != 1 AU");
        for t in 0..=window {
            let r = eph.body_pos(0, Tick(t)).length();
            assert!(
                (r - 1.0).abs() < 1e-9,
                "radius drifted to {r} at tick {t} (e=0 must stay at 1 AU)"
            );
        }
    }

    #[test]
    fn subtick_endpoints_match_samples() {
        let dt = Dt::new(1.0);
        let eph = Ephemeris::precompute(&[circular_body()], dt, 10);
        let p_t = eph.body_pos(0, Tick(3));
        let p_t1 = eph.body_pos(0, Tick(4));
        let at0 = eph.body_pos_subtick(0, Tick(3), 0.0);
        let at1 = eph.body_pos_subtick(0, Tick(3), 1.0);
        assert_eq!(at0, p_t, "frac=0 must equal body_pos(tick)");
        assert_eq!(at1, p_t1, "frac=1 must equal body_pos(tick+1)");
    }

    #[test]
    fn precompute_is_bit_identical() {
        let dt = Dt::new(0.5);
        let window = 200u64;
        let bodies = [circular_body()];
        let a = Ephemeris::precompute(&bodies, dt, window);
        let b = Ephemeris::precompute(&bodies, dt, window);
        for t in 0..=window {
            let pa = a.body_pos(0, Tick(t));
            let pb = b.body_pos(0, Tick(t));
            assert_eq!(
                pa.to_bits(),
                pb.to_bits(),
                "tick {t}: two precomputes from same config must be bit-identical"
            );
        }
    }

    /// HEADLINE GUARD TEST. The central star is configured with a = 0.0. Without
    /// the KEPLER_A_EPSILON guard, `n = sqrt(mu/a^3) = inf -> M = inf -> NaN`,
    /// which laundered into the FNV hash gives a deterministic-but-WRONG state.
    /// Assert the output is (a) finite everywhere (no NaN/inf) and (b) pinned to
    /// the origin, at integer ticks AND at a sub-tick fraction.
    #[test]
    fn central_body_a_zero_is_fixed_at_origin_and_finite() {
        let dt = Dt::new(1.0);
        let window = 50u64;
        // Mixed config: index 0 is the fixed star, index 1 is a real orbiter.
        let eph = Ephemeris::precompute(&[central_star(), circular_body()], dt, window);

        for t in 0..=window {
            let p = eph.body_pos(0, Tick(t));
            assert!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "central body produced non-finite pos {p:?} at tick {t} \
                 (a=0 NaN leak — KEPLER_A_EPSILON guard missing)"
            );
            assert_eq!(
                p,
                Vec3::ZERO,
                "central body (a=0) must be pinned to origin, got {p:?} at tick {t}"
            );
        }

        // Sub-tick interpolation of a fixed body must also be the origin, finite.
        let mid = eph.body_pos_subtick(0, Tick(10), 0.5);
        assert!(
            mid.x.is_finite() && mid.y.is_finite() && mid.z.is_finite(),
            "central body sub-tick non-finite {mid:?}"
        );
        assert_eq!(mid, Vec3::ZERO, "central body sub-tick must stay at origin");

        // The co-resident real orbiter must be unaffected: r stays ~1 AU, finite.
        for t in 0..=window {
            let p = eph.body_pos(1, Tick(t));
            assert!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "orbiter contaminated by NaN at tick {t}: {p:?}"
            );
            assert!(
                (p.length() - 1.0).abs() < 1e-9,
                "orbiter radius drifted to {} at tick {t}",
                p.length()
            );
        }
    }

    /// Contract-surface coverage: lookups past the window must clamp, not panic
    /// or index OOB (downstream Verlet reads t_{n+1} at the last tick).
    #[test]
    fn lookup_past_window_clamps_to_last_sample() {
        let dt = Dt::new(1.0);
        let window = 5u64;
        let eph = Ephemeris::precompute(&[circular_body()], dt, window);
        let last = eph.body_pos(0, Tick(window));
        let past = eph.body_pos(0, Tick(window + 1000));
        assert_eq!(past, last, "past-window lookup must clamp to the final sample");
        let sub_past = eph.body_pos_subtick(0, Tick(window + 1000), 1.0);
        assert_eq!(sub_past, last, "past-window sub-tick must clamp to final sample");
    }

    /// An INCLINED, ECCENTRIC orbit (e=0.3, i=0.5, raan=0.7, argp=0.4). This is
    /// the test that actually exercises the physics: e != 0 forces the Newton
    /// solve to do real work (E != M) and makes `sqrt(1 - e^2) != 1`, while the
    /// non-zero (i, raan, argp) make the PQW->inertial rotation non-identity, so
    /// a sign/scale error in any of r11..r32 (which the circular zero-inclination
    /// tests cannot see) shows up here. It also reads the VELOCITY table on the
    /// public surface via `body_vel` and couples it to position through vis-viva.
    fn inclined_eccentric_body() -> BodyInit {
        BodyInit {
            mass: 1.0,
            elements: OrbitalElements {
                a: 1.0,
                e: 0.3,
                i: 0.5,
                raan: 0.7,
                argp: 0.4,
                m0: 0.0,
            },
        }
    }

    #[test]
    fn inclined_eccentric_obeys_vis_viva_and_actually_moves() {
        let dt = Dt::new(1.0);
        let window = 100u64; // a ~ 1 AU => period ~365 d; 100 d sweeps ~100 deg
        let a = 1.0_f64;
        let e = 0.3_f64;
        // mu convention matches kepler_state: n = sqrt(mu/a^3) with mu = G_CANONICAL
        // (central mass M_sun = 1 folded in). vis-viva: v^2 = mu*(2/r - 1/a).
        let mu = G_CANONICAL;
        let r_peri = a * (1.0 - e); // 0.7 AU
        let r_apo = a * (1.0 + e); // 1.3 AU

        let eph = Ephemeris::precompute(&[inclined_eccentric_body()], dt, window);

        let sample_ticks = [0u64, 25, 50, 75, 100];
        for &t in &sample_ticks {
            let p = eph.body_pos(0, Tick(t));
            let v = eph.body_vel(0, Tick(t));
            assert!(
                p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
                "non-finite pos {p:?} at tick {t}"
            );
            assert!(
                v.x.is_finite() && v.y.is_finite() && v.z.is_finite(),
                "non-finite vel {v:?} at tick {t}"
            );

            let r = p.length();
            // r must stay within the radial band [a(1-e), a(1+e)] (small tol).
            assert!(
                r >= r_peri - 1e-9 && r <= r_apo + 1e-9,
                "radius {r} at tick {t} outside [{r_peri}, {r_apo}] (e=0.3 band)"
            );

            // vis-viva couples the position and velocity tables. Exact-sample
            // read => holds to ~machine precision (relative tol 1e-9).
            let v2 = v.length_sq();
            let vis_viva = mu * (2.0 / r - 1.0 / a);
            let rel_err = (v2 - vis_viva).abs() / vis_viva;
            assert!(
                rel_err < 1e-9,
                "vis-viva broken at tick {t}: v^2={v2}, mu*(2/r-1/a)={vis_viva}, \
                 rel_err={rel_err} (catches rotation sign/scale AND velocity errors)"
            );
        }

        // The body must actually MOVE: distinct ticks => distinct directions.
        // (kills the 'frozen body still passes |r| in band' false-green.)
        let p_a = eph.body_pos(0, Tick(0));
        let p_b = eph.body_pos(0, Tick(25));
        let dir_a = p_a.normalize_or_zero();
        let dir_b = p_b.normalize_or_zero();
        // cos of the angle between the two direction vectors; must be < 1 if moved.
        let cos_ang = dir_a.dot(dir_b);
        assert!(
            cos_ang < 1.0 - 1e-6,
            "body did not move: dir(t=0).dir(t=25) cos = {cos_ang} (~1 means frozen)"
        );

        // The orbit must leave the ecliptic plane (i = 0.5 != 0): some z != 0.
        let any_z = sample_ticks
            .iter()
            .any(|&t| eph.body_pos(0, Tick(t)).z.abs() > 1e-6);
        assert!(any_z, "inclined orbit (i=0.5) never left z=0 plane (rotation lost)");
    }
}
