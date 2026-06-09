use jumpgate_commons_cut::{dynamics::step, rng_bridge::build_scenario, Action, ArenaState};

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic "always stay" trajectory hash. Pins the integer dynamics + scenario
/// seeding so any accidental float/entropy/order change is caught loudly (spec §7).
#[test]
fn golden_trajectory_is_pinned() {
    let cfg = build_scenario(12345, 3, 3, 0, 0);
    let mut st = ArenaState::from_config(&cfg, &[0u8, 1u8, 2u8]);
    let actions = vec![Action::Stay; st.ships.len()];
    let mut buf = Vec::new();
    for _ in 0..cfg.horizon {
        step(&mut st, &actions, &cfg);
        buf.extend_from_slice(&st.tick.to_le_bytes());
        for s in &st.ships {
            buf.extend_from_slice(&s.total_yield.to_le_bytes());
        }
        for r in &st.regions {
            buf.extend_from_slice(&r.stock.to_le_bytes());
        }
    }
    let h = fnv1a(&buf);
    // GOLDEN: pinned on first green (see plan Task 5). Re-pin only on a reviewed change.
    assert_eq!(
        h, 0x5701_7d18_ffff_70c6u64,
        "trajectory hash drifted to {h:#018x} — a determinism break OR a deliberate, reviewed re-pin"
    );
}
