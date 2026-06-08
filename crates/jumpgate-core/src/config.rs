//! The single hashed run-config struct (spec §6). Initial conditions — body set,
//! craft count, per-ship base spec, master seed, dt, softening, substep params —
//! live HERE, recorded and folded into the CONFIG hash. This config hash is
//! DISTINCT from the per-tick STATE hash (`hash.rs`): this one hashes immutable
//! initial conditions ONCE with its own `"CONFIG_1"` tag; that one hashes the
//! evolving world each tick via the shared `FnvHasher` seeded with `HASH_MAGIC`.
//! Different magic/purpose; never conflate or share state.

use crate::math::Vec3;
use crate::time::Dt;

/// Nominal ("base") ship numbers. Physics reads EFFECTIVE values via an accessor
/// (Task 4 `stores::effective_params`); v1 effective == base.
#[derive(Clone, Debug)]
pub struct BaseSpec {
    pub base_dry_mass: f64,
    pub base_max_thrust: f64,
    pub base_exhaust_velocity: f64,
    pub base_fuel_capacity: f64,
}

/// Classical Kepler conic elements (radians for angles), solved once at init.
#[derive(Clone, Debug)]
pub struct OrbitalElements {
    pub a: f64,
    pub e: f64,
    pub i: f64,
    pub raan: f64,
    pub argp: f64,
    pub m0: f64,
}

#[derive(Clone, Debug)]
pub struct BodyInit {
    pub mass: f64,
    pub elements: OrbitalElements,
}

#[derive(Clone, Debug)]
pub struct CraftInit {
    pub spec: BaseSpec,
    pub pos: Vec3,
    pub vel: Vec3,
    pub fuel_mass: f64,
}

/// N substeps = pure fn of QUANTIZED total local acceleration magnitude (Task 7).
#[derive(Clone, Copy, Debug)]
pub struct SubstepCfg {
    /// Reference acceleration (AU/day²) for the substep schedule, NOT a log base:
    /// `substep_count` (Task 8) uses `n = 1 + floor(log2(max(1, mag/accel_ref)))`,
    /// clamped to `[1, max_substeps]`. At/below `accel_ref` → 1 substep; every
    /// doubling of `mag` above it adds one substep.
    pub accel_ref: f64,
    pub max_substeps: u32,
}

/// Class-2 run-level guidance POLICY (config-hashed). Dimensionless tunables a
/// caller may legitimately vary per run; folded into `config_hash` so a changed
/// value yields a different config whose recordings are correctly rejected at the
/// replay config-hash guard. (In a future fleet layer this migrates to a per-fleet
/// attribute; v1 holds it run-level — see spec §13.)
#[derive(Clone, Copy, Debug)]
pub struct GuidanceParams {
    /// Closing-speed cap as a FRACTION of full-tank Tsiolkovsky Δv
    /// (`exhaust_velocity * ln((dry + capacity)/dry)`). Replaces the absolute
    /// `V_CRUISE = 2e-3`. Default 0.25 (D5 derivation note).
    pub cruise_burn_fraction: f64,
    /// Brake-early safety margin (< 1). Exact carryover of the old `K_BRAKE`.
    pub k_brake: f64,
    /// Velocity-matched deadband (canonical AU/day). Exact carryover of `V_ERR_EPS`.
    pub v_err_eps: f64,
}

impl Default for GuidanceParams {
    fn default() -> Self {
        GuidanceParams { cruise_burn_fraction: 0.25, k_brake: 0.5, v_err_eps: 1.0e-4 }
    }
}

#[derive(Clone, Debug)]
pub struct RunConfig {
    /// gym reset(seed) OVERWRITES this per episode.
    pub master_seed: u64,
    pub dt: Dt,
    /// epsilon in (r^2 + eps^2)^1.5 gravity softening.
    pub softening: f64,
    pub substep_cfg: SubstepCfg,
    /// ticks precomputed in the ephemeris window.
    pub ephemeris_window: u64,
    pub bodies: Vec<BodyInit>,
    pub craft: Vec<CraftInit>,
    /// Class-2 guidance policy (D4). Folded at the TAIL of config_hash.
    pub guidance: GuidanceParams,
}

/// The CONFIG hash (immutable initial conditions). NOT the per-tick state hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigHash(pub u64);

// FNV-1a 64-bit, folding one u64 at a time as 8 little-endian bytes. LOCAL to
// the CONFIG hash; the per-tick STATE hash (hash.rs) is a separate hasher with a
// different seed magic. The two hash spaces must never alias.
const CONFIG_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const CONFIG_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

struct ConfigFnv {
    state: u64,
}

impl ConfigFnv {
    fn new() -> Self {
        let mut h = ConfigFnv {
            state: CONFIG_FNV_OFFSET,
        };
        h.write_u64(0x434f_4e46_4947_5f31); // "CONFIG_1" tag, distinct space
        h
    }

    fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(CONFIG_FNV_PRIME);
        }
    }

    fn finish(self) -> u64 {
        self.state
    }
}

impl RunConfig {
    /// FNV-1a over master_seed, dt.bits(), softening.to_bits(), substep cfg, the
    /// ephemeris window, and every numeric field of every body/craft in a FIXED
    /// order (counts folded in first so two scenarios with different cardinality
    /// can never collide). DISTINCT from the per-tick state hash.
    ///
    /// CONFIG_FIELD_ORDER (config_hash fold order — append-only; re-pin the golden on change):
    ///   1. master_seed                       9.  per-body: mass + 6 elements
    ///   2. dt.bits()                         10. per-craft: 4 spec + pos[3] + vel[3] + fuel
    ///   3. softening.to_bits()               11. guidance.cruise_burn_fraction   (D4)
    ///   4. substep_cfg.accel_ref.to_bits()   12. guidance.k_brake                (D4)
    ///   5. substep_cfg.max_substeps          13. guidance.v_err_eps              (D4)
    ///   6. ephemeris_window
    ///   7. bodies.len()   8. craft.len()
    pub fn config_hash(&self) -> ConfigHash {
        // Exhaustive destructure: a NEW RunConfig field is a COMPILE ERROR here
        // until it is explicitly folded below (D10/M6 — closes the silent-omission
        // provenance hole). Field FOLD ORDER below is unchanged (value-preserving).
        let RunConfig {
            master_seed,
            dt,
            softening,
            substep_cfg,
            ephemeris_window,
            bodies,
            craft,
            guidance, // NEW (D4): destructure forces folding below
        } = self;
        let mut h = ConfigFnv::new();
        // Scalars in fixed order.
        h.write_u64(*master_seed);
        h.write_u64(dt.bits());
        h.write_u64(softening.to_bits());
        h.write_u64(substep_cfg.accel_ref.to_bits());
        h.write_u64(substep_cfg.max_substeps as u64);
        h.write_u64(*ephemeris_window);
        // Counts folded BEFORE field values so cardinality changes always move
        // the hash even if the new elements are all-zero.
        h.write_u64(bodies.len() as u64);
        h.write_u64(craft.len() as u64);
        // Bodies in declaration order; each field in fixed order.
        for b in bodies {
            h.write_u64(b.mass.to_bits());
            h.write_u64(b.elements.a.to_bits());
            h.write_u64(b.elements.e.to_bits());
            h.write_u64(b.elements.i.to_bits());
            h.write_u64(b.elements.raan.to_bits());
            h.write_u64(b.elements.argp.to_bits());
            h.write_u64(b.elements.m0.to_bits());
        }
        // Craft in declaration order; spec, pos, vel, fuel in fixed order.
        for c in craft {
            h.write_u64(c.spec.base_dry_mass.to_bits());
            h.write_u64(c.spec.base_max_thrust.to_bits());
            h.write_u64(c.spec.base_exhaust_velocity.to_bits());
            h.write_u64(c.spec.base_fuel_capacity.to_bits());
            let p = c.pos.to_bits();
            h.write_u64(p[0]);
            h.write_u64(p[1]);
            h.write_u64(p[2]);
            let v = c.vel.to_bits();
            h.write_u64(v[0]);
            h.write_u64(v[1]);
            h.write_u64(v[2]);
            h.write_u64(c.fuel_mass.to_bits());
        }
        // GUIDANCE (D4/D9) at the TAIL: the existing byte stream above stays
        // byte-identical; config_hash only EXTENDS. Order: cruise_burn_fraction,
        // k_brake, v_err_eps (CONFIG_FIELD_ORDER words below).
        h.write_u64(guidance.cruise_burn_fraction.to_bits());
        h.write_u64(guidance.k_brake.to_bits());
        h.write_u64(guidance.v_err_eps.to_bits());
        ConfigHash(h.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN_CONFIG_HASH: u64 = 0x278c_5d91_b75a_9e5a; // RE-PINNED: +guidance fold (D4). Was 0x9767_52c4_8d05_053c.

    fn sample() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.5),
            softening: 1e-4,
            substep_cfg: SubstepCfg {
                accel_ref: 2.0,
                max_substeps: 64,
            },
            ephemeris_window: 10_000,
            bodies: vec![BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 1.0,
                    e: 0.0167,
                    i: 0.0,
                    raan: 0.0,
                    argp: 1.0,
                    m0: 0.5,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1.0,
                    base_max_thrust: 0.01,
                    base_exhaust_velocity: 3.0,
                    base_fuel_capacity: 0.5,
                },
                pos: Vec3::new(1.0, 0.0, 0.0),
                vel: Vec3::new(0.0, 1.0, 0.0),
                fuel_mass: 0.5,
            }],
            guidance: GuidanceParams::default(),
        }
    }

    #[test]
    fn config_hash_golden_anchor_is_stable() {
        // Drift-lock: the sample config's hash must not move under a refactor that
        // is meant to be value-preserving (e.g. the exhaustive-destructure change).
        // If a NEW field is added and folded, this value SHOULD change and be re-pinned
        // deliberately (mirrors the state_hash golden discipline).
        let got = sample().config_hash();
        assert_eq!(
            got,
            ConfigHash(GOLDEN_CONFIG_HASH),
            "config_hash drifted: re-pin only if intentional"
        );
    }

    #[test]
    fn same_config_same_hash() {
        assert_eq!(sample().config_hash(), sample().config_hash());
    }

    #[test]
    fn changing_seed_changes_hash() {
        let mut c = sample();
        c.master_seed = 43;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_dt_changes_hash() {
        let mut c = sample();
        c.dt = Dt::new(0.25);
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_softening_changes_hash() {
        let mut c = sample();
        c.softening = 2e-4;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_a_body_field_changes_hash() {
        let mut c = sample();
        c.bodies[0].elements.e = 0.02;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_a_craft_field_changes_hash() {
        let mut c = sample();
        c.craft[0].spec.base_max_thrust = 0.02;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_craft_position_changes_hash() {
        let mut c = sample();
        c.craft[0].pos = Vec3::new(1.5, 0.0, 0.0);
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_cruise_burn_fraction_changes_hash() {
        let mut c = sample();
        c.guidance.cruise_burn_fraction = 0.30;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_k_brake_changes_hash() {
        let mut c = sample();
        c.guidance.k_brake = 0.6;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_v_err_eps_changes_hash() {
        let mut c = sample();
        c.guidance.v_err_eps = 2.0e-4;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_substep_cfg_changes_hash() {
        let mut c = sample();
        c.substep_cfg.max_substeps = 128;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_ephemeris_window_changes_hash() {
        let mut c = sample();
        c.ephemeris_window = 20_000;
        assert_ne!(sample().config_hash(), c.config_hash());
    }

    #[test]
    fn changing_cardinality_changes_hash() {
        // An extra all-zero craft must still change the hash, because counts are
        // folded in BEFORE field values.
        let mut c = sample();
        c.craft.push(CraftInit {
            spec: BaseSpec {
                base_dry_mass: 0.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 0.0,
                base_fuel_capacity: 0.0,
            },
            pos: Vec3::new(0.0, 0.0, 0.0),
            vel: Vec3::new(0.0, 0.0, 0.0),
            fuel_mass: 0.0,
        });
        assert_ne!(sample().config_hash(), c.config_hash());
    }
}
