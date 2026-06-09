use crate::ArenaConfig;
use crate::{Action, ArenaState, STOCK_MAX};

/// Advance the arena one tick. SIMULTANEOUS update: actions are read against the
/// tick-start state, applied, then yields computed and stock decremented, then regen.
/// Ships iterate in index order (determinism). No RNG, no float.
pub fn step(state: &mut ArenaState, actions: &[Action], cfg: &ArenaConfig) {
    debug_assert_eq!(actions.len(), state.ships.len());

    // --- Phase 1: apply movement decisions (read tick-start positions) ---
    for (i, ship) in state.ships.iter_mut().enumerate() {
        match (ship.region, actions[i]) {
            (Some(here), Action::MoveTo(dest)) if dest != here => {
                let cost = cfg.travel[here as usize][dest as usize];
                ship.dest = dest;
                if cost == 0 {
                    ship.region = Some(dest); // defensive; real moves cost >= 1
                } else {
                    // Depart: spend `cost` whole ticks of zero yield in transit.
                    ship.region = None;
                    ship.travel_ticks_remaining = cost;
                }
            }
            (None, _) => {
                // In transit: count down one tick; arrive on the tick it hits zero.
                ship.travel_ticks_remaining -= 1;
                if ship.travel_ticks_remaining == 0 {
                    ship.region = Some(ship.dest);
                }
            }
            _ => {} // Stay, or MoveTo current region: no change
        }
    }

    // --- Phase 2: count occupants per region (post-movement) ---
    let n_regions = state.regions.len();
    let mut occupants = vec![0u64; n_regions];
    for ship in &state.ships {
        if let Some(r) = ship.region {
            occupants[r as usize] += 1;
        }
    }

    // --- Phase 3: compute per-ship yields + total extraction per region ---
    let mut extraction = vec![0u64; n_regions];
    for ship in state.ships.iter_mut() {
        if let Some(r) = ship.region {
            let region = &state.regions[r as usize];
            let occ = occupants[r as usize].max(1);
            let per_ship = (region.stock as u64 * region.richness_cap as u64)
                / (STOCK_MAX as u64 * occ);
            ship.total_yield += per_ship;
            extraction[r as usize] += per_ship; // summed over occupants
        }
    }

    // --- Phase 4: decrement stock by total extraction, then regen ---
    for (r, region) in state.regions.iter_mut().enumerate() {
        let new_stock = (region.stock as u64).saturating_sub(extraction[r]) as u32;
        region.stock = (new_stock + region.regen_per_tick).min(region.richness_cap);
    }

    state.tick += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Region, Ship};

    fn one_region_state(stock: u32, cap: u32, occupants: usize) -> ArenaState {
        ArenaState {
            regions: vec![Region { stock, richness_cap: cap, regen_per_tick: 0 }],
            ships: (0..occupants)
                .map(|_| Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 })
                .collect(),
            tick: 0,
        }
    }

    #[test]
    fn yield_is_stock_times_cap_over_stockmax_times_occupants_floored() {
        // STOCK_MAX=20, full stock=20, cap=20, 1 occupant -> 20*20/(20*1)=20.
        let mut st = one_region_state(20, 20, 1);
        let actions = vec![Action::Stay];
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 };
        step(&mut st, &actions, &cfg);
        assert_eq!(st.ships[0].total_yield, 20);
        assert_eq!(st.regions[0].stock, 0, "20 mined out of 20 -> exhausted");
    }

    #[test]
    fn crowd_split_dilutes_per_ship_yield() {
        // full=20, cap=20, 2 occupants -> per_ship = 20*20/(20*2)=10 each; total extract 20.
        let mut st = one_region_state(20, 20, 2);
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 };
        step(&mut st, &[Action::Stay, Action::Stay], &cfg);
        assert_eq!(st.ships[0].total_yield, 10);
        assert_eq!(st.ships[1].total_yield, 10);
        assert_eq!(st.regions[0].stock, 0, "2x10 extracted from 20");
    }

    #[test]
    fn depletion_to_zero_then_zero_yield() {
        let mut st = one_region_state(2, 20, 1); // low stock -> 2*20/20 = 2
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 30 };
        step(&mut st, &[Action::Stay], &cfg);
        assert_eq!(st.ships[0].total_yield, 2);
        assert_eq!(st.regions[0].stock, 0);
        step(&mut st, &[Action::Stay], &cfg); // empty region -> 0
        assert_eq!(st.ships[0].total_yield, 2, "no further yield from empty region");
    }

    #[test]
    fn move_costs_transit_ticks_of_zero_yield() {
        let mut st = ArenaState {
            regions: vec![
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
            ],
            ships: vec![Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 }],
            tick: 0,
        };
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0, 2], vec![2, 0]], horizon: 30 };
        step(&mut st, &[Action::MoveTo(1)], &cfg); // depart: travel=2, this tick in transit
        assert_eq!(st.ships[0].region, None, "in transit");
        assert_eq!(st.ships[0].total_yield, 0, "no mining while moving");
        step(&mut st, &[Action::Stay], &cfg); // 1 tick left
        assert_eq!(st.ships[0].region, None);
        step(&mut st, &[Action::Stay], &cfg); // arrived, mines region 1
        assert_eq!(st.ships[0].region, Some(1));
        assert_eq!(st.ships[0].total_yield, 20);
    }

    /// SPEC §3 GRADIENT CHECK — gate, not a runtime assertion. Yield must take >=4
    /// distinct values as a single region drains, else the depletion gradient is too
    /// flat for a live abandon-decision.
    ///
    /// DEVIATION FROM PLAN (mathematically forced): the plan's fixture used
    /// `cap == STOCK_MAX`, which under the spec yield law `stock*cap/(STOCK_MAX*occ)`
    /// reduces to `yield == stock` — a one-shot exhaustion (a single distinct delta),
    /// NOT the "20,19,..." the plan comment claimed. Raising STOCK_MAX (the plan's
    /// prescribed remedy) is inert here: the drain rate is proportional to the
    /// `cap : STOCK_MAX` ratio, not STOCK_MAX's magnitude, so `cap == STOCK_MAX`
    /// one-shots for any STOCK_MAX. The real arena draws caps in `1..=STOCK_MAX`, so
    /// live abandon-decisions live on the SUB-MAXIMAL regions. We therefore exercise
    /// the gate on a sub-maximal cap (STOCK_MAX/2 = 10), which drains over several
    /// ticks (deltas 5,2,1,1,0 -> {5,2,1,0} = 4 distinct) and keeps STOCK_MAX=20 so
    /// the Task-9 DP state budget (54M @ STOCK_MAX=20) and Task-5 golden stay pinned.
    #[test]
    fn depletion_gradient_has_enough_distinct_yield_values() {
        let cap = STOCK_MAX / 2; // sub-maximal: the regime where a depletion gradient exists
        let mut st = one_region_state(cap, cap, 1);
        let cfg = ArenaConfig { regions: st.regions.clone(), travel: vec![vec![0]], horizon: 100 };
        let mut seen = std::collections::BTreeSet::new();
        let mut prev = 0u64;
        for _ in 0..STOCK_MAX {
            step(&mut st, &[Action::Stay], &cfg);
            seen.insert(st.ships[0].total_yield - prev);
            prev = st.ships[0].total_yield;
            if st.regions[0].stock == 0 { break; }
        }
        assert!(seen.len() >= 4, "depletion gradient too flat ({} distinct yields); raise STOCK_MAX", seen.len());
    }
}
