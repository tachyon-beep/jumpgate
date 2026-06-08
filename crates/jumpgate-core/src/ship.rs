//! Ship variable-mass dynamics (Tsiolkovsky). Reads `Effective` only.

use crate::math::Vec3;
use crate::stores::Effective;

/// Variable-mass thrust step.
///
/// `dir` is the (already-unit) thrust direction supplied by the autopilot.
/// Returns `(accel, fuel_consumed)`:
/// - `accel`   = `throttle * eff.max_thrust * dir / (eff.dry_mass + fuel_mass)`
/// - `fuel_consumed` = `throttle * eff.max_thrust / eff.exhaust_velocity * dt`,
///   clamped to the available `fuel_mass`.
///
/// When `throttle <= 0` or `fuel_mass <= 0`, thrust contributes nothing:
/// returns `(Vec3::ZERO, 0.0)`.
pub fn thrust_accel_and_burn(
    eff: &Effective,
    fuel_mass: f64,
    thrust_dir: Vec3,
    throttle: f64,
    dt: f64,
) -> (Vec3, f64) {
    // No thrust if commanded off or tank is dry.
    if throttle <= 0.0 || fuel_mass <= 0.0 {
        return (Vec3::ZERO, 0.0);
    }
    let thrust_force = throttle * eff.max_thrust;
    let total_mass = eff.dry_mass + fuel_mass;
    let accel = thrust_dir.scale(thrust_force / total_mass);
    // Variable-mass consumption: mdot = F / v_e; clamp to what's in the tank.
    let consumed = (thrust_force / eff.exhaust_velocity * dt).min(fuel_mass);
    (accel, consumed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::BaseSpec;
    use crate::stores::effective_params;

    fn eff_fixture() -> Effective {
        // dry_mass 1, max_thrust 1, exhaust_velocity 10, fuel_capacity 2
        effective_params(&BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 1.0,
            base_exhaust_velocity: 10.0,
            base_fuel_capacity: 2.0,
        })
    }

    #[test]
    fn zero_throttle_yields_zero_accel_and_zero_burn() {
        let eff = eff_fixture();
        let (a, consumed) =
            thrust_accel_and_burn(&eff, 2.0, Vec3::new(1.0, 0.0, 0.0), 0.0, 0.1);
        assert_eq!(a, Vec3::ZERO);
        assert_eq!(consumed, 0.0);
    }

    #[test]
    fn fuel_consumed_clamped_to_available() {
        let eff = eff_fixture();
        // Tiny tank, huge dt: raw mdot*dt = 1.0/10.0*100.0 = 10.0 >> 0.001.
        let fuel = 0.001_f64;
        let (_a, consumed) =
            thrust_accel_and_burn(&eff, fuel, Vec3::new(1.0, 0.0, 0.0), 1.0, 100.0);
        assert!(consumed <= fuel, "consumed {consumed} must not exceed fuel {fuel}");
        assert!((consumed - fuel).abs() < 1e-12, "should consume exactly the tank");
    }

    #[test]
    fn accel_rises_as_fuel_drops_at_constant_throttle() {
        let eff = eff_fixture();
        let dir = Vec3::new(1.0, 0.0, 0.0);
        // Same throttle, less fuel => smaller total mass => larger accel.
        let (a_full, _) = thrust_accel_and_burn(&eff, 2.0, dir, 1.0, 0.1);
        let (a_low, _) = thrust_accel_and_burn(&eff, 0.5, dir, 1.0, 0.1);
        // dry=1,max_thrust=1: full -> 1/(1+2)=0.333..., low -> 1/(1+0.5)=0.666...
        assert!((a_full.x - (1.0 / 3.0)).abs() < 1e-12);
        assert!((a_low.x - (1.0 / 1.5)).abs() < 1e-12);
        assert!(a_low.length() > a_full.length(), "accel must rise as fuel drops");
    }

    #[test]
    fn known_burn_consumes_tsiolkovsky_fuel() {
        let eff = eff_fixture(); // dry=1, max_thrust=1, v_e=10, cap=2
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let dt = 0.001_f64;
        let fuel0 = 2.0_f64;

        let mut fuel = fuel0;
        let mut dv = 0.0_f64;
        for _ in 0..2000 {
            let (a, consumed) = thrust_accel_and_burn(&eff, fuel, dir, 1.0, dt);
            if consumed <= 0.0 {
                break; // tank dry
            }
            fuel -= consumed;
            dv += a.length() * dt;
        }
        let consumed_total = fuel0 - fuel;

        // Tsiolkovsky: m1 = m0 * exp(-dv / v_e); predicted consumed = m0 - m1.
        let m0 = eff.dry_mass + fuel0;
        let m1 = m0 * (-dv / eff.exhaust_velocity).exp();
        let pred_consumed = m0 - m1;

        let rel_err = (consumed_total - pred_consumed).abs() / pred_consumed;
        assert!(
            rel_err < 1e-3,
            "consumed {consumed_total} vs Tsiolkovsky {pred_consumed} (dv={dv}, rel_err={rel_err})"
        );
    }
}
