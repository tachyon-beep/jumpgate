//! Deterministic guidance law (§5.6). Reads the RESOLVED `NavState` field
//! (never the `Command`), returns `(thrust_dir, throttle)`. v1 law: thrust
//! toward the destination; cut throttle inside `ARRIVAL_RADIUS` or when the
//! remaining Δv budget is exhausted. Reads `Effective` (the §5.5 accessor
//! output), never `BaseSpec` directly.

use crate::math::Vec3;
use crate::stores::{Effective, NavState};

/// Distance (canonical AU) at which the autopilot declares "arrived" and cuts thrust.
pub const ARRIVAL_RADIUS: f64 = 1.0e-4;

/// Deterministic guidance. Returns `(thrust_dir, throttle)`.
/// `thrust_dir` is a unit vector (or `Vec3::ZERO` when not thrusting);
/// `throttle` is in `[0.0, 1.0]`.
pub fn autopilot_command(
    nav: NavState,
    pos: Vec3,
    _vel: Vec3,
    dest_pos: Vec3,
    _eff: &Effective,
) -> (Vec3, f64) {
    match nav {
        NavState::Idle => (Vec3::ZERO, 0.0),
        NavState::Seeking { dv_remaining, .. } => {
            let to_dest = dest_pos.sub(pos);
            let dist = to_dest.length();
            if dist <= ARRIVAL_RADIUS || dv_remaining <= 0.0 {
                (Vec3::ZERO, 0.0)
            } else {
                (to_dest.normalize_or_zero(), 1.0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BaseSpec;
    use crate::stores::effective_params;

    fn eff() -> Effective {
        effective_params(&BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 1.0,
            base_exhaust_velocity: 1.0,
            base_fuel_capacity: 1.0,
        })
    }

    #[test]
    fn points_toward_dest() {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let dest = Vec3::new(3.0, 0.0, 0.0);
        let nav = NavState::Seeking {
            dest: crate::types::NavDest::Position(dest),
            dv_remaining: 5.0,
        };
        let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
        assert_eq!(dir, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(throttle, 1.0);
    }

    #[test]
    fn cuts_inside_arrival_radius() {
        let dest = Vec3::new(0.0, 0.0, 0.0);
        // pos is closer than ARRIVAL_RADIUS to dest.
        let pos = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
        let nav = NavState::Seeking {
            dest: crate::types::NavDest::Position(dest),
            dv_remaining: 5.0,
        };
        let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn dv_exhaustion_stops_thrust() {
        let pos = Vec3::new(0.0, 0.0, 0.0);
        let dest = Vec3::new(3.0, 0.0, 0.0); // far away, would otherwise thrust
        let nav = NavState::Seeking {
            dest: crate::types::NavDest::Position(dest),
            dv_remaining: 0.0, // budget gone
        };
        let (dir, throttle) = autopilot_command(nav, pos, Vec3::ZERO, dest, &eff());
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }

    #[test]
    fn idle_never_thrusts() {
        let (dir, throttle) = autopilot_command(
            NavState::Idle,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::ZERO,
            Vec3::new(9.0, 9.0, 9.0),
            &eff(),
        );
        assert_eq!(dir, Vec3::ZERO);
        assert_eq!(throttle, 0.0);
    }
}
