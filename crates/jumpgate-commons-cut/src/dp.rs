use crate::dynamics::step;
use crate::{Action, ArenaConfig, ArenaState, Ship};
use std::collections::HashMap;

/// Encode the dynamic state (region stocks + ship positions) into a u64 key.
/// Layout: [stock_0..stock_{M-1}] each in 0..=STOCK_MAX (needs ceil(log2(STOCK_MAX+1)) bits),
/// then per ship [pos, dest, travel_ticks_remaining], where `pos` is 0..=M (M = "in
/// transit"/None sentinel). Only the dynamic fields are encoded; richness_cap/regen are
/// static (carried by `cfg`), and `total_yield` is the DP's reward, never state.
pub fn encode(st: &ArenaState, cfg: &ArenaConfig) -> u64 {
    let m = cfg.regions.len() as u64;
    let stock_bits = 64 - (crate::STOCK_MAX as u64).leading_zeros() as u64; // bits per stock
    let mut key = 0u64;
    let mut shift = 0u64;
    for r in &st.regions {
        key |= (r.stock as u64) << shift;
        shift += stock_bits;
    }
    // pos in 0..=M (M = transit); dest in 0..M; travel_ticks_remaining (small, <= 5).
    let pos_bits = 64 - (m + 1).leading_zeros() as u64;
    for s in &st.ships {
        let pos = s.region.map(|r| r as u64).unwrap_or(m);
        key |= pos << shift;
        shift += pos_bits;
        key |= (s.dest as u64) << shift;
        shift += pos_bits;
        key |= (s.travel_ticks_remaining as u64) << shift;
        shift += 4; // travel <= 5 fits in 4 bits
    }
    debug_assert!(shift <= 64, "state does not fit in u64 ({shift} bits) — shrink N/M/STOCK_MAX");
    key
}

/// Inverse of `encode`. `starts` supplies N (one entry per ship); its values are unused
/// because positions are recovered from the key. `total_yield` is reset to 0 (it is the
/// DP's reward, not state). Used for tests/debugging — `br_value` clones live states.
pub fn decode(key: u64, cfg: &ArenaConfig, starts: &[u8]) -> ArenaState {
    let m = cfg.regions.len() as u64;
    let stock_bits = 64 - (crate::STOCK_MAX as u64).leading_zeros() as u64;
    let mut shift = 0u64;
    let mut regions = cfg.regions.clone();
    for r in regions.iter_mut() {
        let mask = (1u64 << stock_bits) - 1;
        r.stock = ((key >> shift) & mask) as u32;
        shift += stock_bits;
    }
    let pos_bits = 64 - (m + 1).leading_zeros() as u64;
    let pos_mask = (1u64 << pos_bits) - 1;
    let n_ships = starts.len();
    let mut ships = Vec::with_capacity(n_ships);
    for _ in 0..n_ships {
        let pos = (key >> shift) & pos_mask;
        shift += pos_bits;
        let dest = ((key >> shift) & pos_mask) as u8;
        shift += pos_bits;
        let ttr = ((key >> shift) & 0xF) as u8;
        shift += 4;
        ships.push(Ship {
            region: if pos == m { None } else { Some(pos as u8) },
            dest,
            travel_ticks_remaining: ttr,
            total_yield: 0,
        });
    }
    ArenaState { regions, ships, tick: 0 }
}

/// Exact OPEN-LOOP best-response value for ship `me`: `me` chooses actions to maximize
/// its own total_yield over the horizon; the other ships are FROZEN at Stay. Backward
/// induction with memoization. (Closed-loop re-crowding is Task 10.)
pub fn best_response_value_open_loop(cfg: &ArenaConfig, starts: &[u8], me: usize) -> u64 {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    br_value(&st0, cfg, me, &mut memo)
}

fn br_value(
    st: &ArenaState,
    cfg: &ArenaConfig,
    me: usize,
    memo: &mut HashMap<(u64, u32), u64>,
) -> u64 {
    if st.tick >= cfg.horizon {
        return 0;
    }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) {
        return v;
    }
    // Candidate actions for `me`: Stay, or MoveTo each other region (only when in-region).
    let mut candidates = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() {
            if r != here as usize {
                candidates.push(Action::MoveTo(r as u8));
            }
        }
    }
    let mut best = 0u64;
    for a in candidates {
        let mut next = st.clone();
        let mut acts = vec![Action::Stay; st.ships.len()]; // others frozen
        acts[me] = a;
        let before = next.ships[me].total_yield;
        step(&mut next, &acts, cfg);
        let reward = next.ships[me].total_yield - before;
        let v = reward + br_value(&next, cfg, me, memo);
        if v > best {
            best = v;
        }
    }
    memo.insert(key, best);
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Region;

    #[test]
    fn encode_decode_roundtrips() {
        let cfg = ArenaConfig {
            regions: vec![
                Region { stock: 5, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 12, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 2], vec![2, 0]],
            horizon: 10,
        };
        let st = ArenaState::from_config(&cfg, &[0u8, 1u8]);
        let code = encode(&st, &cfg);
        let st2 = decode(code, &cfg, &[0u8, 1u8]);
        assert_eq!(
            st.regions.iter().map(|r| r.stock).collect::<Vec<_>>(),
            st2.regions.iter().map(|r| r.stock).collect::<Vec<_>>()
        );
        assert_eq!(
            st.ships.iter().map(|s| s.region).collect::<Vec<_>>(),
            st2.ships.iter().map(|s| s.region).collect::<Vec<_>>()
        );
    }

    #[test]
    fn single_ship_dp_value_matches_hand_rollout_on_tiny_instance() {
        // 1 ship, 1 region, full stock 20, cap 20, horizon 3, no regen, no moves possible.
        // Optimal = greedy stay: 20 + 0 + 0 = 20 (mined out tick 1).
        let cfg = ArenaConfig {
            regions: vec![Region { stock: 20, richness_cap: 20, regen_per_tick: 0 }],
            travel: vec![vec![0]],
            horizon: 3,
        };
        let v = best_response_value_open_loop(&cfg, &[0u8], 0);
        assert_eq!(v, 20);
    }

    /// Move-optimal discriminator (LAW 3 correctness guard): the tiny instance above
    /// never generates a MoveTo candidate (M=1), so it cannot exercise the
    /// depart/transit/arrival branch of `br_value`. This one forces a move to be the
    /// unique optimum so a future-blind ("moving never helps") DP bug is caught here,
    /// in the trivial open-loop setting, instead of surfacing as a Task-10 phantom
    /// mismatch.
    ///
    /// Hand-trace (occ=1 -> per_ship = stock*cap/(STOCK_MAX*1) = stock; STOCK_MAX=20, caps=20):
    ///   horizon=2, regions [stock:4][stock:20], travel cost 1, start region 0.
    ///   Stay-only:  tick0 mine 4 (stock->0); tick1 mine 0  -> total 4.
    ///   Move tick0: tick0 depart (yield 0, in transit); tick1 arrive region 1, mine 20
    ///               -> total 20.
    /// Optimal = 20.
    #[test]
    fn single_ship_dp_value_prefers_a_move_when_it_pays() {
        let cfg = ArenaConfig {
            regions: vec![
                Region { stock: 4, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 2,
        };
        let v = best_response_value_open_loop(&cfg, &[0u8], 0);
        assert_eq!(v, 20, "DP must move to the rich region (future-blind bug returns 4)");
    }
}
