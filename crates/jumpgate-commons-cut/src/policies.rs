use crate::{Action, ArenaConfig, ArenaState};

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
}
