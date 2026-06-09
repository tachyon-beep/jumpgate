//! Named RNG sub-streams (Task 5).
//!
//! One master `u64` seed → several SEPARATE `ChaCha8Rng` instances, each seeded
//! by a FIXED derivation `master ^ SALT[stream]`. Streams are never drawn from a
//! shared parent: that would couple stream-creation order to draw order and break
//! Tier-B replay (spec §6 / line 191). Distinct salts give independent sequences;
//! the same `(master, stream)` always reproduces the same sequence.
//!
//! Pinned to rand_chacha 0.10.0 / rand_core 0.10.1. In this family `seed_from_u64`
//! lives on `rand_core::SeedableRng`, and the infallible `next_u64` lives on
//! `rand_core::Rng` (NOT `RngCore`, which only exposes the fallible `try_next_u64`).

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

/// The named sub-streams. `Intervention` carries lever/perturbation randomness;
/// `Scenario` carries initial-condition / loadout randomness; `Piracy` (appended)
/// carries pirate encounter-resolution randomness. APPEND-ONLY: never reorder the
/// existing variants or change their salts — that changes replay identity.
#[derive(Clone, Copy)]
pub enum RngStream {
    Intervention,
    Scenario,
    Piracy,
}

/// Per-stream salt constants. Fixed forever (changing one changes replay identity
/// for that stream). Unrelated 64-bit constants so `master ^ SALT` never aliases
/// across streams for any single master (since the salts differ).
const SALT_INTERVENTION: u64 = 0x9E37_79B9_7F4A_7C15;
const SALT_SCENARIO: u64 = 0xC2B2_AE3D_27D4_EB4F;
/// Appended for `RngStream::Piracy` (pirate encounter rolls). A new fixed constant
/// — the existing salts are untouched, so existing streams keep their identity.
const SALT_PIRACY: u64 = 0x6A09_E667_F3BC_C908;

impl RngStream {
    /// Fixed salt for this stream. `const fn` so the derivation is unambiguous and
    /// has no runtime state.
    const fn salt(self) -> u64 {
        match self {
            RngStream::Intervention => SALT_INTERVENTION,
            RngStream::Scenario => SALT_SCENARIO,
            RngStream::Piracy => SALT_PIRACY,
        }
    }
}

/// Holds one independent `ChaCha8Rng` per named stream, all derived from a single
/// master seed. Construction order is irrelevant — each stream is a pure function
/// of `(master, stream)`.
pub struct RngStreams {
    intervention: ChaCha8Rng,
    scenario: ChaCha8Rng,
    piracy: ChaCha8Rng,
}

impl RngStreams {
    /// Seed every named stream from `master` via its fixed salt derivation.
    /// `seed_from_u64` (rand_core 0.10) deterministically expands the u64 into the
    /// 32-byte ChaCha seed; pinned versions make this reproducible across runs.
    pub fn from_master(master: u64) -> Self {
        RngStreams {
            intervention: ChaCha8Rng::seed_from_u64(master ^ RngStream::Intervention.salt()),
            scenario: ChaCha8Rng::seed_from_u64(master ^ RngStream::Scenario.salt()),
            piracy: ChaCha8Rng::seed_from_u64(master ^ RngStream::Piracy.salt()),
        }
    }

    /// Borrow the named stream's generator for drawing.
    pub fn stream(&mut self, which: RngStream) -> &mut ChaCha8Rng {
        match which {
            RngStream::Intervention => &mut self.intervention,
            RngStream::Scenario => &mut self.scenario,
            RngStream::Piracy => &mut self.piracy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // 0.10: `next_u64` is on `rand_core::Rng`, NOT `RngCore`.
    use rand_core::Rng;

    fn draw_n(rng: &mut ChaCha8Rng, n: usize) -> Vec<u64> {
        (0..n).map(|_| rng.next_u64()).collect()
    }

    #[test]
    fn same_master_reproduces_each_stream() {
        let mut a = RngStreams::from_master(42);
        let mut b = RngStreams::from_master(42);
        assert_eq!(
            draw_n(a.stream(RngStream::Intervention), 8),
            draw_n(b.stream(RngStream::Intervention), 8),
            "Intervention sequence must reproduce for the same master"
        );
        assert_eq!(
            draw_n(a.stream(RngStream::Scenario), 8),
            draw_n(b.stream(RngStream::Scenario), 8),
            "Scenario sequence must reproduce for the same master"
        );
    }

    #[test]
    fn distinct_streams_differ() {
        let mut s = RngStreams::from_master(42);
        let iv = draw_n(s.stream(RngStream::Intervention), 8);
        let sc = draw_n(s.stream(RngStream::Scenario), 8);
        let pi = draw_n(s.stream(RngStream::Piracy), 8);
        assert_ne!(
            iv, sc,
            "Intervention and Scenario must not produce the same sequence"
        );
        assert_ne!(iv, pi, "Intervention and Piracy must differ");
        assert_ne!(sc, pi, "Scenario and Piracy must differ");
    }

    #[test]
    fn piracy_stream_reproduces_and_is_draw_order_independent() {
        // The appended Piracy stream reproduces for the same master and is a pure
        // function of (master, stream) — draining other streams cannot perturb it.
        let mut a = RngStreams::from_master(42);
        let mut b = RngStreams::from_master(42);
        assert_eq!(
            draw_n(a.stream(RngStream::Piracy), 8),
            draw_n(b.stream(RngStream::Piracy), 8),
            "Piracy sequence must reproduce for the same master"
        );

        let mut drained = RngStreams::from_master(7);
        for _ in 0..1000 {
            drained.stream(RngStream::Scenario).next_u64();
        }
        let pi_after_drain = draw_n(drained.stream(RngStream::Piracy), 8);
        let mut fresh = RngStreams::from_master(7);
        let pi_fresh = draw_n(fresh.stream(RngStream::Piracy), 8);
        assert_eq!(
            pi_after_drain, pi_fresh,
            "Piracy must be unaffected by draws from other streams"
        );
    }

    #[test]
    fn streams_are_independent_of_draw_order() {
        // Draining Intervention must not perturb Scenario: the streams are separate
        // ChaCha8Rng instances, NOT siblings off a shared parent.
        let mut drained = RngStreams::from_master(7);
        for _ in 0..1000 {
            drained.stream(RngStream::Intervention).next_u64();
        }
        let sc_after_drain = draw_n(drained.stream(RngStream::Scenario), 8);

        let mut fresh = RngStreams::from_master(7);
        let sc_fresh = draw_n(fresh.stream(RngStream::Scenario), 8);

        assert_eq!(
            sc_after_drain, sc_fresh,
            "Scenario must be unaffected by draws from Intervention"
        );
    }

    #[test]
    fn different_masters_differ_on_every_stream() {
        // Strengthened: divergence must hold on BOTH named streams, not just one.
        // A weak single-stream check could pass while the other stream silently
        // collapsed to a master-independent sequence.
        let mut a = RngStreams::from_master(1);
        let mut b = RngStreams::from_master(2);
        assert_ne!(
            draw_n(a.stream(RngStream::Intervention), 8),
            draw_n(b.stream(RngStream::Intervention), 8),
            "Different masters must yield different Intervention sequences"
        );

        let mut a2 = RngStreams::from_master(1);
        let mut b2 = RngStreams::from_master(2);
        assert_ne!(
            draw_n(a2.stream(RngStream::Scenario), 8),
            draw_n(b2.stream(RngStream::Scenario), 8),
            "Different masters must yield different Scenario sequences"
        );
    }

    #[test]
    fn golden_first_draws_are_pinned() {
        // VERSION/API-DRIFT GUARD. "Same run reproduces" only proves a run agrees
        // with itself; it cannot catch a silent rand_chacha/rand_core bump that
        // changes the byte stream. These hardcoded constants were captured against
        // rand_chacha=0.10.0 / rand_core=0.10.1 and pin the actual sequence.
        // If this test fails, the RNG version family changed and EVERY recorded
        // replay's state hashes are invalidated — that is a deliberate, reviewed
        // event, not a number to silently re-baseline.
        let mut s = RngStreams::from_master(0);
        let iv0 = s.stream(RngStream::Intervention).next_u64();
        let sc0 = s.stream(RngStream::Scenario).next_u64();
        assert_eq!(
            iv0, 0xa6ab_1181_2ab1_c509,
            "Intervention[master=0] first draw drifted"
        );
        assert_eq!(
            sc0, 0x4f53_8dce_87ab_d2df,
            "Scenario[master=0] first draw drifted"
        );
    }
}
