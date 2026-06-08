//! Per-tick STATE-hash specification + the shared FNV-1a hasher (spec §6).
//! Landed early as the DRIFT-LOCK ANCHOR: the canonical field order
//! (`HASH_FIELD_ORDER`) is authoritative here, so a later task that adds a
//! hashed field (e.g. Task 11 adds prev_fuel/prev_inside_dest) MUST append to
//! `HASH_FIELD_ORDER`, bump `HASH_FORMAT_VERSION`, and update the golden test —
//! the change cannot silently alter the hash uncaught.
//!
//! DISTINCT from the CONFIG hash (`config::RunConfig::config_hash`): that one
//! folds immutable initial conditions ONCE with a "CONFIG_1" tag. This one
//! hashes evolving world state each tick seeded with `HASH_MAGIC`.
//!
//! ## HASH_FIELD_ORDER — canonical per-tick state-hash field order
//!
//! `state_hash` (Task 13) writes EXACTLY these u64 words in EXACTLY this order.
//! Every f64 is encoded via `f64::to_bits()`; every word is folded
//! little-endian by `FnvHasher::write_u64`. Numbering is stable; APPEND only.
//!
//!  1. HASH_MAGIC                              (Task 3, header)
//!  2. HASH_FORMAT_VERSION as u64              (Task 3, header)
//!  3. tick.0                                  (Task 3, time)
//!  4. body_store.ids.cursor()                 (Task 4/13, slot-map high-water)
//!  5. ship_store.ids.cursor()                 (Task 4/13, slot-map high-water)
//!
//! Bodies, sorted by BodyId (slot, generation):
//!
//!  6. body.slot as u64, body.generation as u64       (Task 13)
//!  7. body.mass.to_bits()                     (Task 13)
//!     (body POSITION is derived from tick via ephemeris, NOT stored, so it is
//!     NOT hashed independently — it is a pure function of tick already hashed)
//!
//! Craft, sorted by CraftId (slot, generation):
//!
//!  8. craft.slot as u64, craft.generation as u64     (Task 13)
//!  9. pos.x,pos.y,pos.z to_bits()             (Task 13)
//! 10. vel.x,vel.y,vel.z to_bits()             (Task 13)
//! 11. fuel_mass.to_bits()                     (Task 13)
//! 12. nav discriminant as u64 (+ resolved dest/dv_remaining bits)  (Task 13)
//! 13. lod discriminant as u64                 (Task 13)
//!
//! APPEND BELOW THIS LINE (bump HASH_FORMAT_VERSION + golden test on change):
//!
//! 14. prev_fuel[i].to_bits()                  (Task 11, event edge-detect state)
//! 15. prev_inside_dest[i] as u64              (Task 11, event edge-detect state)

/// Header magic for the per-tick STATE hash (little-endian, spec §6).
pub const HASH_MAGIC: u64 = 0x4a55_4d50_4741_5445; // "JUMPGATE"
/// Bump whenever HASH_FIELD_ORDER changes (e.g. Task 11 appends fields).
pub const HASH_FORMAT_VERSION: u32 = 1;

/// Golden per-tick hash of the minimal zero-init slice under HASH_FIELD_ORDER
/// words 1..=13. Pinned so any change to the canonical encoding is caught.
/// Captured from the first run of `golden_zero_state_hash`; if HASH_FIELD_ORDER
/// or HASH_FORMAT_VERSION changes, recapture AND bump the version.
pub const GOLDEN_ZERO_STATE_HASH: u64 = 0xf0dd_a1ba_f433_3735;

/// Shared FNV-1a 64-bit hasher for the per-tick state hash. Folds each u64 as 8
/// little-endian bytes. `new()` seeds with `HASH_MAGIC` then the version word.
pub struct FnvHasher {
    state: u64,
}

const STATE_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const STATE_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl FnvHasher {
    pub fn new() -> Self {
        let mut h = FnvHasher {
            state: STATE_FNV_OFFSET,
        };
        h.write_u64(HASH_MAGIC); // HASH_FIELD_ORDER word 1
        h.write_u64(HASH_FORMAT_VERSION as u64); // HASH_FIELD_ORDER word 2
        h
    }
    /// Folds one u64 as 8 little-endian bytes (HASH_FIELD_ORDER words).
    pub fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(STATE_FNV_PRIME);
        }
    }
    pub fn finish(self) -> u64 {
        self.state
    }
}

impl Default for FnvHasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_hasher_is_deterministic() {
        assert_eq!(FnvHasher::new().finish(), FnvHasher::new().finish());
    }

    #[test]
    fn write_order_matters() {
        let mut a = FnvHasher::new();
        a.write_u64(1);
        a.write_u64(2);
        let mut b = FnvHasher::new();
        b.write_u64(2);
        b.write_u64(1);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn writing_changes_the_hash() {
        let base = FnvHasher::new().finish();
        let mut h = FnvHasher::new();
        h.write_u64(0);
        assert_ne!(base, h.finish(), "even writing 0 must move the hash");
    }

    /// GOLDEN HASH. This pins the canonical encoding of the HASH_FIELD_ORDER
    /// header + a zero-initialized single-body single-craft state slice (the
    /// same words Task 13's zero-init `state_hash` will reproduce). If this value
    /// changes, the canonical hash encoding changed — that is ONLY allowed
    /// alongside a HASH_FORMAT_VERSION bump and a HASH_FIELD_ORDER edit.
    #[test]
    fn golden_zero_state_hash() {
        let mut h = FnvHasher::new();
        // header (words 1-2) are already folded by new(); now the rest of a
        // minimal zero-init slice per HASH_FIELD_ORDER words 3..=13:
        h.write_u64(0); // 3. tick
        h.write_u64(0); // 4. body cursor
        h.write_u64(0); // 5. ship cursor
        // one body (slot 0, generation 0, mass 0.0):
        h.write_u64(0); // body slot
        h.write_u64(0); // body generation
        h.write_u64(0.0f64.to_bits()); // body mass
        // one craft (slot 0, generation 0; zero pos/vel/fuel; nav Idle=0; lod Player=0):
        h.write_u64(0); // craft slot
        h.write_u64(0); // craft generation
        h.write_u64(0.0f64.to_bits()); // pos.x
        h.write_u64(0.0f64.to_bits()); // pos.y
        h.write_u64(0.0f64.to_bits()); // pos.z
        h.write_u64(0.0f64.to_bits()); // vel.x
        h.write_u64(0.0f64.to_bits()); // vel.y
        h.write_u64(0.0f64.to_bits()); // vel.z
        h.write_u64(0.0f64.to_bits()); // fuel_mass
        h.write_u64(0); // nav discriminant (Idle)
        h.write_u64(0); // lod discriminant (Player)
        assert_eq!(h.finish(), GOLDEN_ZERO_STATE_HASH);
    }
}
