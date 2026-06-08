//! Core math: hand-rolled f64 `Vec3` and canonical-unit constants.
//!
//! Vec3 is hand-rolled (not glam) so the crate stays `#![forbid(unsafe_code)]`
//! and so `to_bits()` owns a FIXED field order (x,y,z) for the Tier-B state hash.
//! f64 throughout: no SIMD, no mantissa loss at solar-system scale. The only
//! precision boundary is the f32 OBSERVATION downcast, which lives in jumpgate-py.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// The zero vector. Associated const so `Vec3::ZERO` reads cleanly.
    pub const ZERO: Vec3 = Vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    // `add`/`sub` are intentional inherent methods (contract-mandated names for
    // determinism auditability); the verbatim tests call `a.add(b)` with only
    // `use super::*`, which requires inherent methods — trait impls do not resolve.
    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn add(self, o: Vec3) -> Vec3 {
        Vec3 {
            x: self.x + o.x,
            y: self.y + o.y,
            z: self.z + o.z,
        }
    }

    #[allow(clippy::should_implement_trait)]
    #[inline]
    pub fn sub(self, o: Vec3) -> Vec3 {
        Vec3 {
            x: self.x - o.x,
            y: self.y - o.y,
            z: self.z - o.z,
        }
    }

    #[inline]
    pub fn scale(self, s: f64) -> Vec3 {
        Vec3 {
            x: self.x * s,
            y: self.y * s,
            z: self.z * s,
        }
    }

    #[inline]
    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn length_sq(self) -> f64 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f64 {
        self.length_sq().sqrt()
    }

    /// Returns the unit vector, or `ZERO` if the length is below `NORMALIZE_EPS`
    /// (avoids dividing by ~0 and producing NaN/Inf).
    /// Uses per-component division (not reciprocal-multiply) so that vectors
    /// like (3,4,0) normalise to exactly (0.6, 0.8, 0.0) — IEEE 754 correctly
    /// rounds x/len for these rationals; reciprocal-multiply does not.
    #[inline]
    pub fn normalize_or_zero(self) -> Vec3 {
        let len = self.length();
        if len < NORMALIZE_EPS {
            Vec3::ZERO
        } else {
            Vec3::new(self.x / len, self.y / len, self.z / len)
        }
    }

    /// Fixed field order for hashing: x then y then z.
    #[inline]
    pub fn to_bits(self) -> [u64; 3] {
        [self.x.to_bits(), self.y.to_bits(), self.z.to_bits()]
    }
}

/// Length floor below which `normalize_or_zero` returns `ZERO` (NaN guard).
const NORMALIZE_EPS: f64 = 1e-12;

// ---- Canonical units (AU, M_sun, day). G folded so quantities sit near unity. ----

/// Gravitational parameter in canonical units: AU^3 / (M_sun * day^2).
/// Equals the square of the Gaussian gravitational constant k = 0.01720209895,
/// i.e. the heliocentric G*M_sun expressed in (AU, M_sun, day).
pub const G_CANONICAL: f64 = 0.01720209895_f64 * 0.01720209895_f64;

/// One astronomical unit in metres (SI), for facade-boundary conversion only.
pub const AU_IN_METERS: f64 = 1.495_978_707e11;
/// One solar mass in kilograms (SI), for facade-boundary conversion only.
pub const M_SUN_IN_KG: f64 = 1.988_47e30;
/// One day in seconds (SI), for facade-boundary conversion only.
pub const DAY_IN_SECONDS: f64 = 86_400.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_fields() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v.x, 1.0);
        assert_eq!(v.y, 2.0);
        assert_eq!(v.z, 3.0);
    }

    #[test]
    fn zero_const() {
        assert_eq!(Vec3::ZERO, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn add_sub_roundtrip() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(10.0, 20.0, 30.0);
        assert_eq!(a.add(b), Vec3::new(11.0, 22.0, 33.0));
        assert_eq!(a.add(b).sub(b), a);
    }

    #[test]
    fn scale_scales_each_component() {
        let a = Vec3::new(1.0, -2.0, 3.0);
        assert_eq!(a.scale(2.0), Vec3::new(2.0, -4.0, 6.0));
        assert_eq!(a.scale(0.0), Vec3::ZERO);
    }

    #[test]
    fn dot_known_value() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, -5.0, 6.0);
        // 1*4 + 2*-5 + 3*6 = 4 - 10 + 18 = 12
        assert_eq!(a.dot(b), 12.0);
    }

    #[test]
    fn length_three_four_zero_is_five() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        assert_eq!(v.length_sq(), 25.0);
        assert_eq!(v.length(), 5.0);
    }

    #[test]
    fn normalize_unit_length() {
        let v = Vec3::new(3.0, 4.0, 0.0).normalize_or_zero();
        assert!((v.length() - 1.0).abs() < 1e-12);
        assert_eq!(v, Vec3::new(0.6, 0.8, 0.0));
    }

    #[test]
    fn normalize_of_zero_is_zero() {
        assert_eq!(Vec3::ZERO.normalize_or_zero(), Vec3::ZERO);
        // a vector below the epsilon floor also returns ZERO (no NaN)
        let tiny = Vec3::new(1e-300, 0.0, 0.0);
        assert_eq!(tiny.normalize_or_zero(), Vec3::ZERO);
    }

    #[test]
    fn to_bits_field_order_is_x_then_y_then_z() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(
            v.to_bits(),
            [1.0f64.to_bits(), 2.0f64.to_bits(), 3.0f64.to_bits()]
        );
    }

    #[test]
    fn to_bits_distinguishes_signed_zero() {
        // f64 to_bits preserves the sign bit: -0.0 != +0.0 in the hash encoding.
        let pos = Vec3::new(0.0, 0.0, 0.0).to_bits();
        let neg = Vec3::new(-0.0, 0.0, 0.0).to_bits();
        assert_ne!(pos[0], neg[0]);
    }

    #[test]
    fn g_canonical_is_gaussian_constant_squared() {
        // k = 0.01720209895 (Gaussian grav const); G_CANONICAL = k^2.
        assert_eq!(G_CANONICAL, 0.01720209895_f64 * 0.01720209895_f64);
    }
}
