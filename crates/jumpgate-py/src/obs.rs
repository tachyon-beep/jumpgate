//! Frame-relative observation extraction (§7.2 hard invariant).
//!
//! The f64 subtraction (p_craft - p_target, v_craft - v_target) happens HERE
//! over core f64 values; ONLY the small relative delta is downcast to f32.
//! A debug assertion guards that no raw absolute coordinate (~AU scale) is
//! ever cast to f32. Layout = fixed ego block + presence-masked entity set.
//!
//! This module is PyO3-free and unit-tested via `cargo test` without Python.

use jumpgate_core::math::Vec3;

/// Bumped whenever the obs layout changes. The variable-N entity set is
/// reserved (zeroed-absent) in v1 so combat's neighbors never force a break.
/// Reserved contract surface (the Python wrapper/training side reads it via
/// the versioned schema); not yet consumed by Rust code in v1.
#[allow(dead_code)]
pub const SCHEMA_VERSION: u32 = 1;

/// Ego block: own velocity in the target frame (3) + fuel fraction (1).
/// Own position relative to the frame target is trivially ~zero and is NOT
/// emitted (emitting it would risk a raw ego coord crossing the boundary).
pub const EGO_LEN: usize = 4;

/// Reserved neighbor slots. v1 emits zero LIVE neighbors but writes every
/// slot as masked-absent, so "presence mask zeros for absent slots" is real.
pub const MAX_NEIGHBORS: usize = 4;

/// Per-neighbor: presence flag (1) + relative pos (3) + relative vel (3).
pub const ENTITY_STRIDE: usize = 7;

/// Total observation width.
pub const OBS_DIM: usize = EGO_LEN + MAX_NEIGHBORS * ENTITY_STRIDE;

/// Tripwire bound (AU). Any frame-relative delta passed to the f32 boundary
/// must be smaller than this in magnitude; an absolute solar-system coord
/// (~tens of AU) trips it, catching the §7.2 "forgot to subtract" bug.
/// Recalibrated when live neighbors arrive (the schema is versioned).
pub const MAX_REL_AU: f64 = 1.0;

/// Downcast a single frame-relative scalar to f32, guarding the boundary.
#[inline]
fn rel_to_f32(v: f64) -> f32 {
    debug_assert!(
        v.abs() < MAX_REL_AU,
        "absolute coordinate {v} crossed the f32 observation boundary \
         (>= MAX_REL_AU {MAX_REL_AU} AU); obs must be frame-relative (§7.2)"
    );
    v as f32
}

/// Pure obs writer over plain values (no `View`). Does the delta downcast,
/// the boundary guard, the fixed layout, and presence-mask zeroing.
///
/// `ego_vel_in_frame` is the craft velocity already expressed relative to the
/// frame target (an f64 delta). `ego_fuel_frac` is dimensionless (O(1)).
/// `neighbors` are (rel_pos, rel_vel) f64 deltas for LIVE neighbors only;
/// any beyond `MAX_NEIGHBORS` are ignored, remaining slots are masked-absent.
pub fn write_obs_parts(
    ego_vel_in_frame: Vec3,
    ego_fuel_frac: f32,
    neighbors: &[(Vec3, Vec3)],
    out: &mut [f32],
) {
    debug_assert_eq!(out.len(), OBS_DIM, "obs buffer must be OBS_DIM wide");

    // --- Ego block: velocity-in-frame (guarded delta) + fuel fraction. ---
    out[0] = rel_to_f32(ego_vel_in_frame.x);
    out[1] = rel_to_f32(ego_vel_in_frame.y);
    out[2] = rel_to_f32(ego_vel_in_frame.z);
    out[3] = ego_fuel_frac; // dimensionless O(1); no boundary guard needed.

    // --- Presence-masked entity set: fill all reserved slots. ---
    for n in 0..MAX_NEIGHBORS {
        let base = EGO_LEN + n * ENTITY_STRIDE;
        match neighbors.get(n) {
            Some(&(rel_pos, rel_vel)) => {
                out[base] = 1.0; // present
                out[base + 1] = rel_to_f32(rel_pos.x);
                out[base + 2] = rel_to_f32(rel_pos.y);
                out[base + 3] = rel_to_f32(rel_pos.z);
                out[base + 4] = rel_to_f32(rel_vel.x);
                out[base + 5] = rel_to_f32(rel_vel.y);
                out[base + 6] = rel_to_f32(rel_vel.z);
            }
            None => {
                // Masked-absent: flag 0 + zeroed payload.
                for k in 0..ENTITY_STRIDE {
                    out[base + k] = 0.0;
                }
            }
        }
    }
}

/// Thrust-mode obs: own vel (3, /VEL_SCALE) + fuel frac (1) + target rel-pos
/// (3, /dist_scale) + target rel-vel (3, /VEL_SCALE). All O(1) post-scaling;
/// the rel_to_f32 guard still applies to every scaled component.
/// Consumed by the thrust control-mode env wiring (plan Task 3); until that
/// phase lands, only the unit tests below exercise it (same pattern as
/// `SCHEMA_VERSION` above).
#[allow(dead_code)]
pub const THRUST_OBS_DIM: usize = 11;
/// Velocity normalizer (AU/day): ~circular-orbit speed at 1 AU, so orbital
/// velocities land O(1).
#[allow(dead_code)]
pub const VEL_SCALE: f64 = 0.02;

#[allow(dead_code)]
pub fn write_obs_thrust_mode(
    own_vel: Vec3,
    fuel_frac: f32,
    target_rel_pos: Vec3,
    target_rel_vel: Vec3,
    dist_scale: f64,
    out: &mut [f32],
) {
    // Scale feature (obs[10]): WITHOUT it, per-stage rel-pos normalization
    // makes curriculum stages ALIASED — a hop and a sprint produce identical
    // observations at episode start while demanding different control
    // regimes, so one policy cannot serve mixed stages (measured: replay-mix
    // floored sprint acquisition in two runs). ln(scale/1e-3)/ln(1e3) maps
    // the curriculum band 0.001..1.0 AU onto ~0..1.
    debug_assert_eq!(out.len(), THRUST_OBS_DIM);
    debug_assert!(dist_scale > 0.0);
    out[0] = rel_to_f32(own_vel.x / VEL_SCALE);
    out[1] = rel_to_f32(own_vel.y / VEL_SCALE);
    out[2] = rel_to_f32(own_vel.z / VEL_SCALE);
    out[3] = fuel_frac;
    out[4] = rel_to_f32(target_rel_pos.x / dist_scale);
    out[5] = rel_to_f32(target_rel_pos.y / dist_scale);
    out[6] = rel_to_f32(target_rel_pos.z / dist_scale);
    out[7] = rel_to_f32(target_rel_vel.x / VEL_SCALE);
    out[8] = rel_to_f32(target_rel_vel.y / VEL_SCALE);
    out[9] = rel_to_f32(target_rel_vel.z / VEL_SCALE);
    let scale_feat = ((dist_scale / 1.0e-3).ln() / 1.0e3_f64.ln()).clamp(0.0, 1.0);
    out[10] = scale_feat as f32;
}

/// Trader-mode obs width: 4 board slots × `[present, reward, d_pickup,
/// d_haul]` + own `[fuel_frac, credits, busy, time_remaining_frac]`.
/// Layout + writer land with the macro-step (`write_obs_trader`); the width
/// is needed by `configure(mode=2)` first.
pub const TRADER_OBS_DIM: usize = 20;

use jumpgate_core::ids::CraftId;
use jumpgate_core::world::View;

/// Frame-relative obs extraction from a projected `View` into `out`.
///
/// Reads the ego craft's pos/vel/fuel from the view, computes the
/// frame-relative velocity (v_craft - v_target; v1 frame target is the
/// ego craft itself, so the ego delta is its own velocity in core units
/// kept small by canonical units), the fuel fraction, and writes a v1
/// zero-neighbor (presence-masked) observation. End-to-end correctness is
/// covered by the Python gym smoke test, not by a unit test here.
///
/// ACCESSOR NAMES below (`craft_vel`, `craft_fuel`, fuel-capacity) are owned
/// by Task 12's `View`; if they differ, rebind ONLY this fn — `write_obs_parts`
/// is unaffected. (Here they are inherent `View` methods, so no `StateView`
/// trait import is needed — adding one would be an unused-import clippy fail.)
pub fn write_obs_frame_relative(view: &View, ego: CraftId, out: &mut [f32]) {
    // Velocity in the ego frame. v1 uses an ego-centric frame; the velocity
    // delta is already small in canonical units.
    let ego_vel = view.craft_vel(ego).unwrap_or(Vec3::ZERO);

    // Fuel fraction = fuel_mass / fuel_capacity (dimensionless, O(1)).
    let fuel_mass = view.craft_fuel(ego).unwrap_or(0.0);
    let capacity = view.craft_fuel_capacity(ego).unwrap_or(1.0);
    let fuel_frac = if capacity > 0.0 {
        (fuel_mass / capacity) as f32
    } else {
        0.0
    };

    // v1: zero live neighbors; all reserved slots written masked-absent.
    let neighbors: &[(Vec3, Vec3)] = &[];
    write_obs_parts(ego_vel, fuel_frac, neighbors, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use jumpgate_core::math::Vec3;

    // Test A: a small relative delta between two tens-of-AU positions retains
    // sub-meter precision in f32, whereas the absolute coord would lose >km.
    #[test]
    fn relative_delta_retains_sub_meter_precision() {
        // Two craft ~10 AU out, separated by 0.0001 AU (~14_960 km).
        let p_craft = 10.000_1_f64; // AU, one axis
        let p_target = 10.000_0_f64; // AU
        let delta_au = p_craft - p_target; // 1e-4 AU

        // Frame-relative path: subtract in f64, downcast the small delta.
        let rel_f32 = delta_au as f32;
        // 1 AU = 1.495_978_707e11 m.
        const AU_M: f64 = 1.495_978_707e11;
        let err_m = ((rel_f32 as f64) - delta_au).abs() * AU_M;
        assert!(err_m < 1.0, "frame-relative err {err_m} m must be sub-meter");

        // Absolute path (what we must NOT do): downcast the ~10 AU coord.
        let abs_f32 = p_craft as f32;
        let abs_err_m = ((abs_f32 as f64) - p_craft).abs() * AU_M;
        assert!(
            abs_err_m > 1_000.0,
            "absolute coord err {abs_err_m} m should exceed a km (proves the \
             frame-relative transform is load-bearing)"
        );
    }

    // Test B: the debug-assert guard fires if a raw absolute (~tens of AU)
    // coordinate is fed to the boundary. Only meaningful in debug builds.
    #[test]
    #[should_panic(expected = "crossed the f32 observation boundary")]
    #[cfg(debug_assertions)]
    fn guard_fires_on_absolute_coordinate() {
        let mut out = [0.0f32; OBS_DIM];
        // ego velocity-in-frame holds a raw absolute coord (~10 AU) — the bug.
        let abs_vel = Vec3::new(10.0, 0.0, 0.0);
        write_obs_parts(abs_vel, 0.5, &[], &mut out);
    }

    #[test]
    fn thrust_obs_layout_and_scaling() {
        let mut out = [9.0f32; THRUST_OBS_DIM]; // 10; sentinel proves overwrite
        write_obs_thrust_mode(
            Vec3::new(0.01, 0.0, 0.0),  // own vel (AU/day)
            0.5,                        // fuel frac
            Vec3::new(0.2, -0.1, 0.0),  // target rel pos (AU)
            Vec3::new(-0.01, 0.0, 0.0), // target rel vel
            0.4,                        // dist_scale
            &mut out,
        );
        assert_eq!(out[0], (0.01f64 / VEL_SCALE) as f32); // vel block scaled
        assert_eq!(out[3], 0.5); // fuel frac raw
        assert_eq!(out[4], (0.2f64 / 0.4) as f32); // rel pos / dist_scale
        assert_eq!(out[7], (-0.01f64 / VEL_SCALE) as f32); // rel vel scaled
        let expect = ((0.4f64 / 1.0e-3).ln() / 1.0e3_f64.ln()).clamp(0.0, 1.0) as f32;
        assert_eq!(out[10], expect); // scale feature
    }

    #[test]
    #[should_panic(expected = "crossed the f32 observation boundary")]
    #[cfg(debug_assertions)]
    fn thrust_obs_guard_fires_on_unscaled_absolute() {
        let mut out = [0.0f32; THRUST_OBS_DIM];
        // rel pos of 10 AU with dist_scale 1.0 -> scaled value 10 > MAX_REL_AU: guard trips.
        write_obs_thrust_mode(Vec3::ZERO, 0.5, Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO, 1.0, &mut out);
    }

    // Test C: with zero live neighbors, every reserved entity slot is written
    // as masked-absent (presence flag 0 + zeroed payload).
    #[test]
    fn presence_mask_zeros_for_absent_slots() {
        let mut out = [9.0f32; OBS_DIM]; // sentinel: prove we overwrite.
        let ego_vel = Vec3::new(0.001, -0.002, 0.0003); // small frame-rel deltas
        write_obs_parts(ego_vel, 0.75, &[], &mut out);

        // Ego block written.
        assert_eq!(out[0], 0.001_f32);
        assert_eq!(out[1], -0.002_f32);
        assert_eq!(out[2], 0.000_3_f32);
        assert_eq!(out[3], 0.75_f32); // fuel fraction

        // Every neighbor slot is masked-absent: flag 0 + zeroed payload.
        for n in 0..MAX_NEIGHBORS {
            let base = EGO_LEN + n * ENTITY_STRIDE;
            assert_eq!(out[base], 0.0_f32, "slot {n} presence flag must be 0");
            for k in 1..ENTITY_STRIDE {
                assert_eq!(out[base + k], 0.0_f32, "slot {n} payload[{k}] must be 0");
            }
        }
    }
}
