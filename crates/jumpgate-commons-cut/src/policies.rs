use crate::{Action, ArenaConfig, ArenaState};
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::Rng;

/// What a policy may observe at decision time: current state only (spec §4 ablation).
pub struct Observation<'a> {
    pub state: &'a ArenaState,
    pub ship_idx: usize,
}

/// A decision rule. Deterministic given (observation, rng if any) — but the ladder's
/// rungs 1/3/ceiling are deterministic; the closed-form (Task 7) carries its own
/// seeded coin for randomization.
pub trait Policy {
    fn decide(&self, obs: &Observation) -> Action;
}

/// Rung 1: never move.
pub struct Constant;
impl Policy for Constant {
    fn decide(&self, _obs: &Observation) -> Action {
        Action::Stay
    }
}

/// Decide one action per ship against the SAME tick-start state (simultaneous).
pub fn decide_all<P: Policy>(p: &P, st: &ArenaState) -> Vec<Action> {
    (0..st.ships.len())
        .map(|i| p.decide(&Observation { state: st, ship_idx: i }))
        .collect()
}

/// Run a homogeneous population under one policy for `horizon` ticks; return per-ship totals.
pub fn rollout<P: Policy>(cfg: &ArenaConfig, ship_starts: &[u8], p: &P) -> Vec<u64> {
    let mut st = ArenaState::from_config(cfg, ship_starts);
    for _ in 0..cfg.horizon {
        let acts = decide_all(p, &st);
        crate::dynamics::step(&mut st, &acts, cfg);
    }
    st.ships.iter().map(|s| s.total_yield).collect()
}

/// Per-region occupant counts of the CURRENT state (a shared observable helper).
pub fn occupant_counts(st: &ArenaState) -> Vec<u32> {
    let mut occ = vec![0u32; st.regions.len()];
    for s in &st.ships {
        if let Some(r) = s.region {
            occ[r as usize] += 1;
        }
    }
    occ
}

/// Rung 2: randomizing anti-herding reactive rule. Abandon when the region's
/// stock-per-occupant projected yield is below `tau`; move (with prob move_prob_milli
/// per-mille) to the region with the highest current stock-per-occupant. Seeded coin
/// keyed by (seed, tick, ship_idx) -> deterministic + replayable.
pub struct ClosedForm {
    pub tau: u64,
    pub move_prob_milli: u32,
    pub seed: u64,
}

impl ClosedForm {
    fn stock_per_occ(st: &ArenaState, r: usize, occ: &[u32]) -> u64 {
        let region = &st.regions[r];
        let o = occ[r].max(1) as u64;
        (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o)
    }
}

impl Policy for ClosedForm {
    fn decide(&self, obs: &Observation) -> Action {
        let st = obs.state;
        let i = obs.ship_idx;
        let Some(here) = st.ships[i].region else {
            return Action::Stay; // in transit: ride it out
        };
        let occ = occupant_counts(st);
        let here_yield = Self::stock_per_occ(st, here as usize, &occ);
        if here_yield >= self.tau {
            return Action::Stay; // still good enough
        }
        // Find the best alternative by current stock-per-occupant (counting self as +1 there).
        let mut best = here as usize;
        let mut best_val = here_yield;
        for (r, &occ_r) in occ.iter().enumerate() {
            if r == here as usize {
                continue;
            }
            let region = &st.regions[r];
            let o = (occ_r + 1).max(1) as u64; // if I joined
            let v = (region.stock as u64 * region.richness_cap as u64)
                / (crate::STOCK_MAX as u64 * o);
            // Pinned tie-break: strict > keeps the lowest index on ties (spec §7).
            if v > best_val {
                best_val = v;
                best = r;
            }
        }
        if best == here as usize {
            return Action::Stay;
        }
        // Seeded coin: hash (seed, tick, ship) into the Scenario stream space.
        let mut streams = RngStreams::from_master(self.seed ^ ((st.tick as u64) << 8) ^ (i as u64));
        let roll = streams.stream(RngStream::Scenario).next_u32() % 1000;
        if roll < self.move_prob_milli {
            Action::MoveTo(best as u8)
        } else {
            Action::Stay
        }
    }
}

/// Fit (tau, move_prob_milli) by grid search on TRAIN seeds (mean total yield).
/// Deterministic. Returns the best `ClosedForm` (seed pinned to 0xC0FFEE for eval coins).
pub fn fit_closed_form(
    n_regions: u8,
    travel: u8,
    regen: u32,
    field_corr: u32,
    n_ships: u8,
    train_seeds: &[u64],
) -> ClosedForm {
    let taus = [
        1u64,
        crate::STOCK_MAX as u64 / 4,
        crate::STOCK_MAX as u64 / 2,
        crate::STOCK_MAX as u64,
    ];
    let probs = [200u32, 500, 800, 1000];
    let mut best = ClosedForm { tau: taus[0], move_prob_milli: probs[0], seed: 0xC0FFEE };
    let mut best_mean = 0u64;
    for &tau in &taus {
        for &mp in &probs {
            let pol = ClosedForm { tau, move_prob_milli: mp, seed: 0xC0FFEE };
            let mut sum = 0u64;
            for &s in train_seeds {
                let cfg = crate::rng_bridge::build_scenario(s, n_regions, travel, regen, field_corr);
                // Fit on the ACTUAL deployment regime (n_ships, crowded by the same
                // (i % n_regions) pattern eval uses) — fitting on a no-crowd one-per-region
                // regime would yield a strawman bar and overstate room.
                let starts: Vec<u8> = (0..n_ships).map(|i| i % n_regions).collect();
                sum += rollout(&cfg, &starts, &pol).iter().sum::<u64>();
            }
            let mean = sum / train_seeds.len().max(1) as u64;
            if mean > best_mean {
                best_mean = mean;
                best = ClosedForm { tau, move_prob_milli: mp, seed: 0xC0FFEE };
            }
        }
    }
    best
}

/// Rung 3: greedy one-step optimal. Evaluate each action's immediate-tick yield
/// against the current state (others frozen), pick the max. Pinned tie-break: Stay
/// wins ties, then lowest MoveTo index.
pub struct Myopic;
impl Policy for Myopic {
    fn decide(&self, obs: &Observation) -> Action {
        let st = obs.state;
        let i = obs.ship_idx;
        let Some(here) = st.ships[i].region else {
            return Action::Stay;
        };
        let occ = occupant_counts(st);
        // Staying: my share of `here` this tick.
        let stay_val = {
            let region = &st.regions[here as usize];
            let o = occ[here as usize].max(1) as u64;
            (region.stock as u64 * region.richness_cap as u64) / (crate::STOCK_MAX as u64 * o)
        };
        // Moving: 0 this tick (in transit). So 1-step greedy only ever beats Stay if Stay==0.
        if stay_val > 0 {
            return Action::Stay;
        }
        // here is empty: move to the region with the best immediate post-arrival share
        // (approximated as current stock-per-(occ+1); the move still costs transit, but a
        // 1-step myopic that's stuck at 0 prefers heading somewhere with future value).
        let mut best = here as usize;
        let mut best_val = 0u64;
        for (r, &occ_r) in occ.iter().enumerate() {
            if r == here as usize {
                continue;
            }
            let region = &st.regions[r];
            let o = (occ_r + 1) as u64;
            let v = (region.stock as u64 * region.richness_cap as u64)
                / (crate::STOCK_MAX as u64 * o);
            if v > best_val {
                best_val = v;
                best = r;
            }
        }
        if best == here as usize {
            Action::Stay
        } else {
            Action::MoveTo(best as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng_bridge::build_scenario;

    #[test]
    fn constant_policy_never_moves() {
        let cfg = build_scenario(7, 3, 3, 0, 0);
        let starts = vec![0u8, 1u8, 2u8];
        let p = Constant;
        let totals = rollout(&cfg, &starts, &p);
        assert_eq!(totals.len(), 3);
        // Constant never emits MoveTo, so every ship stays on its start region the whole run.
        let mut st = ArenaState::from_config(&cfg, &starts);
        for _ in 0..cfg.horizon {
            let acts = decide_all(&p, &st);
            assert!(acts.iter().all(|a| matches!(a, Action::Stay)), "constant only Stays");
            crate::dynamics::step(&mut st, &acts, &cfg);
        }
    }

    #[test]
    fn closed_form_abandons_below_threshold_and_targets_best_region() {
        // A ship on a near-empty region with a rich alternative should (sometimes) move.
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 1, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
                crate::Region {
                    stock: crate::STOCK_MAX,
                    richness_cap: crate::STOCK_MAX,
                    regen_per_tick: 0,
                },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8]);
        // tau high (always-want-to-move), p=1000 (always move when triggered) -> deterministic move to region 1.
        let pol = ClosedForm { tau: crate::STOCK_MAX as u64, move_prob_milli: 1000, seed: 1 };
        let a = pol.decide(&Observation { state: &st, ship_idx: 0 });
        assert_eq!(a, Action::MoveTo(1), "abandons poor region for the richest");
    }

    #[test]
    fn closed_form_fit_returns_best_on_train_and_is_reused_on_eval() {
        let train: Vec<u64> = (100..104).collect();
        let eval: Vec<u64> = (200..204).collect();
        let fitted = fit_closed_form(3, 3, 0, 0, 3, &train);
        // Determinism: fitting twice on the same train seeds gives the same params.
        let again = fit_closed_form(3, 3, 0, 0, 3, &train);
        assert_eq!((fitted.tau, fitted.move_prob_milli), (again.tau, again.move_prob_milli));
        // The fitted policy is then evaluated on disjoint eval seeds (smoke: it runs).
        let cfg = build_scenario(eval[0], 3, 3, 0, 0);
        let totals = rollout(&cfg, &[0, 1, 2], &fitted);
        assert_eq!(totals.len(), 3);
    }

    #[test]
    fn myopic_picks_the_one_step_best_action() {
        // One ship; region 0 nearly empty, region 1 full + adjacent (travel 0 -> arrives same... use travel 1).
        let cfg = ArenaConfig {
            regions: vec![
                crate::Region { stock: 1, richness_cap: crate::STOCK_MAX, regen_per_tick: 0 },
                crate::Region {
                    stock: crate::STOCK_MAX,
                    richness_cap: crate::STOCK_MAX,
                    regen_per_tick: 0,
                },
            ],
            travel: vec![vec![0, 1], vec![1, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8]);
        // Staying yields ~1 this tick; moving yields 0 this tick (transit) but the myopic
        // 1-step horizon values only THIS tick -> myopic stays (greedy is myopic by design).
        let a = Myopic.decide(&Observation { state: &st, ship_idx: 0 });
        assert_eq!(a, Action::Stay, "1-step greedy values only the immediate tick");
    }

    #[test]
    fn mobile_closed_form_beats_crowded_constant() {
        // DISCRIMINATING sanity (replaces a vacuous all-equal fixture). When MORE ships
        // crowd ONE region, Constant strands them all there and only ever drains region 0,
        // leaving the other regions untouched; a mobile closed-form spreads out and drains
        // ALL regions -> strictly more total. Validates the core move mechanic the whole
        // cut rests on, and would catch a closed-form whose move logic is broken.
        //
        // NOTE (sound-invariant discipline): `constant <= closed-form <= myopic` is NOT a
        // theorem — closed-form and myopic are different heuristics with no guaranteed
        // order. Only "mobility beats crowded do-nothing" is a sound population invariant.
        // True ceiling dominance (BR >= any feasible policy) + the per-ship-vs-population
        // comparability are pinned in the DP/gate phases.
        let cfg = build_scenario(999, 3, 3, 0, 0);
        let crowded = [0u8, 0, 0, 0]; // 4 ships ALL on region 0, with 3 regions available
        let mean = |t: Vec<u64>| t.iter().sum::<u64>();
        let mobile = ClosedForm { tau: crate::STOCK_MAX as u64, move_prob_milli: 1000, seed: 0xC0FFEE };
        let c = mean(rollout(&cfg, &crowded, &Constant));
        let f = mean(rollout(&cfg, &crowded, &mobile));
        assert!(c < f, "mobility must beat crowded constant: constant {c} < mobile {f}");
    }
}
