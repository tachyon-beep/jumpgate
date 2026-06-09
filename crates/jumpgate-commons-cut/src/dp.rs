use crate::dynamics::step;
use crate::policies::{ClosedForm, Observation, Policy};
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

/// Closed-loop BR: `me` best-responds while the OTHERS run `others` (reactive `ClosedForm`)
/// against the live state each tick — they re-crowd in response to the deviator rather than
/// staying frozen (spec §4.1). Returns `(computed V0, realized rollout total)`; the two MUST
/// be equal — the phantom-ceiling cross-check (LAW 3). A mismatch is a DP/realize bug.
pub fn best_response_value_closed_loop_checked(
    cfg: &ArenaConfig,
    starts: &[u8],
    me: usize,
    others: &ClosedForm,
) -> (u64, u64) {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    let v0 = br_value_cl(&st0, cfg, me, others, &mut memo);
    // Realized rollout: replay me's optimal choice forward through the reactive field.
    let realized = realize_cl(&st0, cfg, me, others, &mut memo);
    (v0, realized)
}

/// Build the simultaneous action vector for one tick: `me` stays (caller overwrites), every
/// other ship runs the reactive `others` rule against the SAME tick-start state.
fn others_actions(st: &ArenaState, me: usize, others: &ClosedForm) -> Vec<Action> {
    (0..st.ships.len())
        .map(|i| {
            if i == me {
                Action::Stay
            } else {
                others.decide(&Observation { state: st, ship_idx: i })
            }
        })
        .collect()
}

/// Candidate actions for `me` from this state: Stay, then MoveTo each other region (lowest
/// index first — the pinned tie-break, spec §7). Only generates moves when in-region.
fn me_candidates(st: &ArenaState, me: usize) -> Vec<Action> {
    let mut candidates = vec![Action::Stay];
    if let Some(here) = st.ships[me].region {
        for r in 0..st.regions.len() {
            if r != here as usize {
                candidates.push(Action::MoveTo(r as u8));
            }
        }
    }
    candidates
}

fn br_value_cl(
    st: &ArenaState,
    cfg: &ArenaConfig,
    me: usize,
    others: &ClosedForm,
    memo: &mut HashMap<(u64, u32), u64>,
) -> u64 {
    if st.tick >= cfg.horizon {
        return 0;
    }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) {
        return v;
    }
    let mut best = 0u64;
    for a in me_candidates(st, me) {
        let mut next = st.clone();
        let mut acts = others_actions(st, me, others);
        acts[me] = a;
        let before = next.ships[me].total_yield;
        step(&mut next, &acts, cfg);
        let reward = next.ships[me].total_yield - before;
        let v = reward + br_value_cl(&next, cfg, me, others, memo);
        if v > best {
            best = v;
        }
    }
    memo.insert(key, best);
    best
}

/// Roll `me`'s optimal policy forward through the reactive field, returning realized total.
/// At each tick picks the first candidate (Stay-first, lowest MoveTo index) whose
/// `reward + br_value_cl(next)` equals the memoized optimum — the DP telescopes so the
/// realized sum equals V0 exactly regardless of which optimum a tie picks.
fn realize_cl(
    st0: &ArenaState,
    cfg: &ArenaConfig,
    me: usize,
    others: &ClosedForm,
    memo: &mut HashMap<(u64, u32), u64>,
) -> u64 {
    let mut st = st0.clone();
    while st.tick < cfg.horizon {
        let target = br_value_cl(&st, cfg, me, others, memo);
        let mut chosen = Action::Stay;
        for a in me_candidates(&st, me) {
            let mut next = st.clone();
            let mut acts = others_actions(&st, me, others);
            acts[me] = a;
            let before = next.ships[me].total_yield;
            step(&mut next, &acts, cfg);
            let reward = next.ships[me].total_yield - before;
            let v = reward + br_value_cl(&next, cfg, me, others, memo);
            if v == target {
                chosen = a;
                break;
            }
        }
        let mut acts = others_actions(&st, me, others);
        acts[me] = chosen;
        step(&mut st, &acts, cfg);
    }
    st.ships[me].total_yield
}

/// LABELLED UPPER BOUND ONLY (NOT the gate, spec §4.1): the coordinated social-planner
/// optimum = max over JOINT action sequences of the SUMMED ship yield. Exact backward
/// induction over the joint action space (Stay + MoveTo(other regions) per ship). Reported
/// as "coordination headroom — not learnable"; it bounds the selfish closed-loop BR from
/// above (yields are non-negative, so the planner's best total dominates the selfish
/// trajectory's total, which itself dominates any single ship's share). The joint action
/// space is exponential in N — keep this for N=3 only (reporting-only, so its absence never
/// blocks the gate beyond N=3).
pub fn planner_value(cfg: &ArenaConfig, starts: &[u8]) -> u64 {
    let st0 = ArenaState::from_config(cfg, starts);
    let mut memo: HashMap<(u64, u32), u64> = HashMap::new();
    planner_rec(&st0, cfg, &mut memo)
}

fn planner_rec(st: &ArenaState, cfg: &ArenaConfig, memo: &mut HashMap<(u64, u32), u64>) -> u64 {
    if st.tick >= cfg.horizon {
        return 0;
    }
    let key = (encode(st, cfg), st.tick);
    if let Some(&v) = memo.get(&key) {
        return v;
    }
    // Joint action space = product over ships of {Stay, MoveTo(other regions)}.
    let per_ship: Vec<Vec<Action>> = (0..st.ships.len())
        .map(|i| {
            let mut c = vec![Action::Stay];
            if let Some(here) = st.ships[i].region {
                for r in 0..st.regions.len() {
                    if r != here as usize {
                        c.push(Action::MoveTo(r as u8));
                    }
                }
            }
            c
        })
        .collect();
    let mut best = 0u64;
    let mut idx = vec![0usize; st.ships.len()];
    loop {
        let acts: Vec<Action> = (0..st.ships.len()).map(|i| per_ship[i][idx[i]]).collect();
        let mut next = st.clone();
        let before: u64 = next.ships.iter().map(|s| s.total_yield).sum();
        step(&mut next, &acts, cfg);
        let after: u64 = next.ships.iter().map(|s| s.total_yield).sum();
        let v = (after - before) + planner_rec(&next, cfg, memo);
        if v > best {
            best = v;
        }
        // Odometer over the joint index; when it wraps fully, memoize and return.
        let mut k = 0;
        loop {
            if k == idx.len() {
                memo.insert(key, best);
                return best;
            }
            idx[k] += 1;
            if idx[k] < per_ship[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
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

    #[test]
    fn closed_loop_br_value_equals_realized_rollout_phantom_check() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 2], vec![2, 0]],
            horizon: 8,
        };
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let (v0, realized) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        assert_eq!(v0, realized, "phantom-ceiling: computed V0 must equal realized rollout");
    }

    /// Phantom-ceiling on a fixture whose OPTIMAL BR path requires a MOVE (not the trivial
    /// Stay-everywhere optimum). `me` starts on a near-empty region; a rich region pays only
    /// after a transit tick, so the realized rollout must telescope through depart(0)+arrive
    /// rewards. This makes the cross-check exercise `realize_cl`'s move branch — a realize
    /// bug that mis-follows the move path produces realized != V0 and is caught here (LAW 3).
    #[test]
    fn closed_loop_phantom_check_holds_when_optimal_path_moves() {
        // Both ships START crowded on the poor region 0; the rich region 1 is unoccupied and
        // reachable. `me` (ship 0) wins by abandoning the crowd for the rich, empty region.
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 2, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 6,
        };
        // tau high so the reactive other ALSO wants to leave the poor region — a live,
        // re-crowding field (its move target uses (occ+1), and `me`'s departure changes occ).
        let others = crate::policies::ClosedForm {
            tau: crate::STOCK_MAX as u64,
            move_prob_milli: 1000,
            seed: 1,
        };
        let (v0, realized) = best_response_value_closed_loop_checked(&cfg, &[0u8, 0u8], 0, &others);
        assert_eq!(v0, realized, "phantom-ceiling (move path): computed V0 must equal realized rollout");
        // Guard the guard: the optimal BR really must beat the Stay-only floor, i.e. moving
        // pays — otherwise this fixture would degenerate to the trivial Stay optimum above.
        // The reactive other has tau=STOCK_MAX, so at tick 0 it ABANDONS the poor region 0
        // for the rich region, leaving `me` alone on region 0 to take the full stock 2 then
        // 0 -> Stay-only floor is 2. Require v0 > 2 so a degenerate Stay-optimum can't slip
        // through (the real optimum here is 11: contest the rich region for 10 then 1).
        assert!(v0 > 2, "fixture must require a move to be optimal (Stay-only floor is 2); v0={v0}");
    }

    #[test]
    fn closed_loop_le_open_loop_recrowding_reduces_inherited_residual() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 8,
        };
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let open = best_response_value_open_loop(&cfg, &[0u8, 1u8], 0);
        let (closed, _) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        assert!(closed <= open, "closed-loop {closed} <= open-loop {open} (reacting field contests residuals)");
    }

    #[test]
    fn planner_is_an_upper_bound_on_selfish_br() {
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                crate::Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 6,
        };
        let planner = planner_value(&cfg, &[0u8, 1u8]);
        let others = crate::policies::ClosedForm { tau: 5, move_prob_milli: 1000, seed: 1 };
        let (selfish, _) = best_response_value_closed_loop_checked(&cfg, &[0u8, 1u8], 0, &others);
        // Planner maximizes TOTAL across ships; selfish is one ship's take -> planner total >= any single share.
        assert!(planner >= selfish, "planner total {planner} >= selfish single-ship {selfish}");
    }
}
