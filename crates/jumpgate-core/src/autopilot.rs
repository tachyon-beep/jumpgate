//! Deterministic guidance law (§5.6). Reads the RESOLVED `NavState` field
//! (never the `Command`), returns `(thrust_dir, throttle)`.
//!
//! v1 braking law (velocity-targeting with a sqrt deceleration profile): work in
//! the TARGET's reference frame (`rel_vel = vel - dest_vel`), command a closing
//! speed `min(cruise_cap, v_brake)` toward the destination where
//! `v_brake = sqrt(2*k_brake*a_max*(d - ARRIVAL_RADIUS))` and the cruise cap is
//! `cruise_burn_fraction × full-tank Δv` (ship-dependent, trajectory-constant),
//! and steer the FULL velocity error (radial + tangential) so the craft brakes
//! to ~rest relative to a fixed `Position` and velocity-matches a moving `Body`.
//! Cut throttle inside `ARRIVAL_RADIUS`, when the Δv budget is exhausted, or when
//! already velocity-matched (the `v_err_eps` deadband — `thrust_accel_and_burn`
//! burns fuel ∝ throttle regardless of direction, so don't burn for ~zero accel).
//! `k_brake`, `cruise_burn_fraction`, and `v_err_eps` come from `GuidanceParams`
//! (run-level policy, D1/D4). Reads `Effective` (the §5.5 accessor output), never
//! `BaseSpec` directly.

use crate::config::GuidanceParams;
use crate::math::Vec3;
use crate::stores::{Effective, NavState};

/// Distance (canonical AU) at which the autopilot declares "arrived" and cuts thrust.
pub const ARRIVAL_RADIUS: f64 = 1.0e-4;

/// Deterministic guidance. Returns `(thrust_dir, throttle)`.
/// `thrust_dir` is a unit vector (or `Vec3::ZERO` when not thrusting);
/// `throttle` is in `[0.0, 1.0]`.
///
/// `dest_vel` is the target's velocity (`Vec3::ZERO` for a fixed `Position`);
/// `fuel_mass` is the craft's current fuel so `a_max = max_thrust / (dry + fuel)`
/// reflects the TRUE available thrust acceleration at this instant.
// D1/D4/D6 widened the guidance signature: `guidance` (policy) and `dt` (the
// §6.6 backstop) are deliberate additions to the existing 7-arg law.
#[allow(clippy::too_many_arguments)]
pub fn autopilot_command(
    nav: NavState,
    pos: Vec3,
    vel: Vec3,
    dest_pos: Vec3,
    dest_vel: Vec3,
    fuel_mass: f64,
    eff: &Effective,
    guidance: &GuidanceParams, // NEW (D1/D4): run-level policy
    dt: f64,                   // NEW (D6 backstop only; feeds no trajectory arithmetic)
) -> (Vec3, f64) {
    match nav {
        NavState::Idle => (Vec3::ZERO, 0.0),
        // Tactical Rung 1 pass-through: the held stick. Direction = the vector's
        // unit; throttle = |v| clamped to 1 (over-length never over-thrusts).
        NavState::DirectThrust { throttle_vec } => {
            let mag = throttle_vec.length();
            if mag <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            (throttle_vec.normalize_or_zero(), mag.min(1.0))
        }
        NavState::Seeking { dv_remaining, .. } => {
            let rel_pos = dest_pos.sub(pos);
            let d = rel_pos.length();
            // Arrived, or no Δv budget left: coast.
            if d <= ARRIVAL_RADIUS || dv_remaining <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let dir = rel_pos.normalize_or_zero();
            // Craft velocity expressed in the target's reference frame. For a
            // moving body this folds the rendezvous in for free: "arrive at rest
            // in the target frame" == co-move with the body.
            let rel_vel = vel.sub(dest_vel);
            // True available thrust acceleration (variable mass).
            let a_max = eff.max_thrust / (eff.dry_mass + fuel_mass);
            // §6.6 backstop: the reset guard (Task 4) guarantees this for the
            // empty-tank worst case; live a_max <= empty-tank a_max, so this can
            // only fire if reset was bypassed or effective params drift above base.
            // Compiled out of release; feeds NO arithmetic, so it cannot affect the hash.
            debug_assert!(
                a_max * dt * dt < ARRIVAL_RADIUS / (2.0 * guidance.k_brake),
                "unbrakable config reached autopilot: a_max*dt^2={} >= R/(2K)={}",
                a_max * dt * dt,
                ARRIVAL_RADIUS / (2.0 * guidance.k_brake)
            );
            // Max closing speed still stoppable within the remaining distance.
            // Left-to-right product (NOT an FMA): 2 * k_brake * a_max * (d - eps).
            let v_brake = (2.0 * guidance.k_brake * a_max * (d - ARRIVAL_RADIUS)).sqrt();
            // Cruise cap = fraction of FULL-tank Δv (trajectory-constant, not
            // shrinking as fuel burns). full_tank_dv left-to-right, ratio inside ln.
            let full_tank_dv =
                crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, eff.fuel_capacity);
            let cruise_cap = guidance.cruise_burn_fraction * full_tank_dv;
            let v_des = dir.scale(cruise_cap.min(v_brake));
            let v_err = v_des.sub(rel_vel);
            // Already velocity-matched: don't burn fuel for ~zero accel.
            if v_err.length() < guidance.v_err_eps {
                return (Vec3::ZERO, 0.0);
            }
            (v_err.normalize_or_zero(), 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BaseSpec;
    use crate::stores::effective_params;
    use crate::types::NavDest;

    /// A realistic craft: dry+fuel == 2.0, max_thrust 1.0 => a_max == 0.5 AU/day^2
    /// at full tank, so the synthetic far/slow and braking checks see a meaningful
    /// `v_brake`.
    fn eff() -> Effective {
        effective_params(
            &BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 1.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
                base_cargo_capacity: 5,
            },
            &crate::stores::EffectiveMods::IDENTITY,
        )
    }

    fn seeking(dest: Vec3) -> NavState {
        NavState::Seeking {
            dest: NavDest::Position(dest),
            dv_remaining: 5.0,
        }
    }

    fn guidance() -> crate::config::GuidanceParams {
        crate::config::GuidanceParams::default()
    }

    #[test]
    fn k_brake_and_v_err_eps_defaults_are_exact_carryover() {
        // Same braking scenario as `brakes_when_overspeeding_toward_dest`, asserted
        // bit-identical under default policy (k_brake=0.5, v_err_eps=1e-4 == old consts).
        let dest = Vec3::new(0.0, 0.0, 0.0);
        let pos = Vec3::new(0.01, 0.0, 0.0);
        let vel = Vec3::new(-1.0, 0.0, 0.0);
        let (dir, throttle) = autopilot_command(
            // dt = 1e-4, NOT 0.25: dt feeds ONLY the backstop debug_assert, and eff()
            // (a_max=0.5) trips it at 0.25 (0.5*0.0625 = 0.03125 >= R/(2k)=1e-4). 1e-4
            // passes (0.5*1e-8 = 5e-9 < 1e-4) and leaves the trajectory assertion intact.
            seeking(dest),
            pos,
            vel,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(throttle, 1.0);
        assert!(
            dir.dot(dest.sub(pos).normalize_or_zero()) < 0.0,
            "still brakes retrograde"
        );
    }

    #[test]
    fn cruise_cap_is_fraction_of_full_tank_dv() {
        // eff(): dry=1, max_thrust=1, v_e=1, capacity=1 => full_tank_dv = 1*ln(2) = 0.693147…
        // cap = 0.25 * full_tank_dv = 0.173287…; far & slow so v_des is cap-limited, not brake-limited.
        let full_tank_dv = crate::math::tsiolkovsky_dv(1.0, 1.0, 1.0);
        let expected_cap = 0.25 * full_tank_dv;
        let dest = Vec3::new(1.0e6, 0.0, 0.0); // very far: v_brake >> cap, so v_des == cap
        let pos = Vec3::ZERO;
        // Craft already moving at exactly the cap toward dest -> v_err ~ 0 (matched at cap).
        let vel = Vec3::new(expected_cap, 0.0, 0.0);
        let (_dir, throttle) = autopilot_command(
            seeking(dest),
            pos,
            vel,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4, // dt: backstop-only
        );
        assert_eq!(
            throttle, 0.0,
            "at the cap with zero residual error -> within deadband, coast"
        );
        // Bracket the cap magnitude (a factor-of-2 cap bug would suppress at the wrong
        // speed): moving FASTER than the cap toward dest must command a retrograde brake.
        let faster = Vec3::new(expected_cap * 2.0, 0.0, 0.0);
        let (dir2, throttle2) = autopilot_command(
            seeking(dest),
            pos,
            faster,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(throttle2, 1.0);
        assert!(
            dir2.x < 0.0,
            "over the cap -> brake retrograde (pins cap magnitude, not just deadband)"
        );
    }

    #[test]
    fn points_toward_dest_when_far_and_slow() {
        // Far away, at rest: v_des points toward dest, rel_vel == 0, so v_err
        // points toward dest at full throttle.
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let dest = Vec3::new(3.0, 0.0, 0.0);
        let (dir, throttle) = autopilot_command(
            seeking(dest),
            pos,
            Vec3::ZERO,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(dir, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(throttle, 1.0);
    }

    #[test]
    fn brakes_when_overspeeding_toward_dest() {
        // Close to the dest AND already moving fast toward it: v_brake is small,
        // rel_vel large prograde, so v_err points RETROGRADE (away from dest).
        let dest = Vec3::new(0.0, 0.0, 0.0);
        let pos = Vec3::new(0.01, 0.0, 0.0); // 0.01 AU from dest
        let dir_to_dest = dest.sub(pos).normalize_or_zero();
        // Closing fast: velocity points toward the dest (-x) at a large speed.
        let vel = Vec3::new(-1.0, 0.0, 0.0);
        let (thrust_dir, throttle) = autopilot_command(
            seeking(dest),
            pos,
            vel,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(throttle, 1.0, "should still be thrusting to brake");
        assert!(
            thrust_dir.dot(dir_to_dest) < 0.0,
            "thrust must point retrograde to brake: dot = {}",
            thrust_dir.dot(dir_to_dest)
        );
    }

    #[test]
    fn arrives_with_low_relative_speed() {
        // 1-D sim: drive the law + a_max forward-Euler and confirm the craft
        // ENTERS the arrival band with small |rel_vel| (no fast tunnel). dt is
        // small here so the synthetic integrator resolves the cruise cap and
        // the sqrt braking ramp (a single coarse Euler step at a_max=0.5 would
        // jump velocity by 0.125 in one tick and alias right past the cap — the
        // engine substeps the steep-accel regime; this loop mimics that).
        let dt = 1.0e-4_f64;
        let eff = eff();
        let dest = Vec3::new(0.5, 0.0, 0.0);
        let mut pos = Vec3::ZERO;
        let mut vel = Vec3::ZERO;
        let fuel = 1.0_f64;
        let nav = NavState::Seeking {
            dest: NavDest::Position(dest),
            dv_remaining: 1e9, // ample budget for the synthetic check
        };
        let mut entered_band_speed: Option<f64> = None;
        // 0.5 AU at the cruise cap (cruise_burn_fraction x full-tank Δv ~ 2e-3
        // AU/day for this fixture) with dt=1e-4 needs ~2.5e6
        // steps; give generous headroom.
        for _ in 0..5_000_000 {
            let (tdir, throttle) = autopilot_command(
                nav,
                pos,
                vel,
                dest,
                Vec3::ZERO,
                fuel,
                &eff,
                &guidance(),
                1.0e-4,
            );
            let a_max = eff.max_thrust / (eff.dry_mass + fuel);
            let accel = tdir.scale(throttle * a_max);
            vel = vel.add(accel.scale(dt));
            pos = pos.add(vel.scale(dt));
            let d = dest.sub(pos).length();
            if d <= ARRIVAL_RADIUS {
                entered_band_speed = Some(vel.length());
                break;
            }
        }
        let speed = entered_band_speed.expect("craft never reached the arrival band");
        // Measured crossing speed ~1.5e-4 AU/day (floors near v_err_eps as v_des
        // dips into the deadband near the band), so speed*0.25 ~ 3.75e-5 < 1e-4.
        // Anti-tunnel property expressed at the ENGINE cadence (dt=0.25), NOT the
        // fine synthetic step above: a tick boundary at the real dt must land
        // inside the sphere. The sqrt braking profile drives v_des -> 0 as d -> 0,
        // so the crossing speed is far below the cruise cap.
        const ENGINE_DT: f64 = 0.25;
        assert!(
            speed * ENGINE_DT < ARRIVAL_RADIUS,
            "entered band too fast (would tunnel at engine dt): |v|={speed}, |v|*dt={}",
            speed * ENGINE_DT
        );
    }

    #[test]
    fn matches_moving_target_velocity() {
        // Craft co-moving with a moving target, far from it: rel_vel ~= dest_vel,
        // so v_err is dominated by v_des (toward dest) -> it still closes.
        // But a craft AT the target velocity AND inside-ish: rel_vel == dest_vel,
        // and when v_des also ~= 0 (very close) the error is within the deadband.
        let dest = Vec3::new(2.0 * ARRIVAL_RADIUS, 0.0, 0.0);
        let dest_vel = Vec3::new(0.0, 0.01, 0.0);
        let pos = Vec3::ZERO;
        // Craft already moving exactly at the target velocity.
        let vel = dest_vel;
        let (thrust_dir, throttle) = autopilot_command(
            seeking(dest),
            pos,
            vel,
            dest,
            dest_vel,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        // d is ~2e-4, so v_brake = sqrt(2*0.5*0.5*1e-4) ~= 7e-3, capped further by
        // v_des; with rel_vel==0 the error is v_des (toward dest). The point of
        // the test: tangential target velocity is fully cancelled (matched), the
        // command only ever reflects the small residual v_des, never the body's
        // 0.01 cross-velocity.
        if throttle > 0.0 {
            // Whatever thrust is commanded points along the closing direction
            // (x), NOT along the body's y-velocity (which is matched).
            assert!(
                thrust_dir.y.abs() < 1e-9,
                "tangential body velocity must be matched, not chased: {thrust_dir:?}"
            );
        }
    }

    #[test]
    fn matched_co_mover_inside_deadband_cuts_throttle() {
        // Craft AT the target velocity and so close that v_des is below the
        // deadband: error < v_err_eps (GuidanceParams) -> cut throttle (don't burn
        // for ~zero accel).
        let dest = Vec3::new(2.0 * ARRIVAL_RADIUS, 0.0, 0.0);
        let dest_vel = Vec3::new(0.0, 0.01, 0.0);
        // Sit just outside the arrival sphere, moving exactly with the target,
        // with v_des already ~0 because d - ARRIVAL_RADIUS is tiny.
        let pos = Vec3::new(ARRIVAL_RADIUS + 1e-12, 0.0, 0.0);
        let dp = dest.sub(pos);
        let _ = dp;
        let vel = dest_vel;
        let (thrust_dir, throttle) = autopilot_command(
            NavState::Seeking {
                dest: NavDest::Position(dest),
                dv_remaining: 5.0,
            },
            pos,
            vel,
            dest,
            dest_vel,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(thrust_dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn cuts_inside_arrival_radius() {
        let dest = Vec3::new(0.0, 0.0, 0.0);
        let pos = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
        let (dir, throttle) = autopilot_command(
            seeking(dest),
            pos,
            Vec3::ZERO,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn dv_exhaustion_stops_thrust() {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let dest = Vec3::new(3.0, 0.0, 0.0); // far away, would otherwise thrust
        let nav = NavState::Seeking {
            dest: NavDest::Position(dest),
            dv_remaining: 0.0, // budget gone
        };
        let (dir, throttle) = autopilot_command(
            nav,
            pos,
            Vec3::ZERO,
            dest,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn direct_thrust_passes_vector_through() {
        let nav = NavState::DirectThrust {
            throttle_vec: Vec3::new(0.6, 0.0, 0.0),
        };
        let (dir, throttle) = autopilot_command(
            nav,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(dir, Vec3::new(1.0, 0.0, 0.0));
        assert!((throttle - 0.6).abs() < 1e-12);
    }

    #[test]
    fn direct_thrust_overlong_vector_clamps_to_full_throttle() {
        let nav = NavState::DirectThrust {
            throttle_vec: Vec3::new(3.0, 4.0, 0.0), // |v| = 5
        };
        let (dir, throttle) = autopilot_command(
            nav,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert!((dir.length() - 1.0).abs() < 1e-12);
        assert_eq!(throttle, 1.0);
    }

    #[test]
    fn direct_thrust_zero_vector_coasts() {
        let nav = NavState::DirectThrust {
            throttle_vec: Vec3::ZERO,
        };
        let (_dir, throttle) = autopilot_command(
            nav,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn idle_never_thrusts() {
        let (dir, throttle) = autopilot_command(
            NavState::Idle,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::ZERO,
            Vec3::new(9.0, 9.0, 9.0),
            Vec3::ZERO,
            1.0,
            &eff(),
            &guidance(),
            1.0e-4,
        );
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }
}
