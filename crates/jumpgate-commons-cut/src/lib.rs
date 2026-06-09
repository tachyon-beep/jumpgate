//! Commons-miner analytic cut — a standalone deterministic probe measuring
//! learnable DRL room in the full-information commons-miner game (spec
//! 2026-06-10-commons-miner-cut-design.md). NOT part of the hashed World.

/// Global stock discretization. Region stock and richness_cap are in `0..=STOCK_MAX`.
/// Start at 20; the gradient check (Task 4) raises it to 50 if depletion flattens.
pub const STOCK_MAX: u32 = 20;

/// A mining region. All integer (determinism).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub stock: u32,        // 0..=richness_cap
    pub richness_cap: u32, // 1..=STOCK_MAX
    pub regen_per_tick: u32,
}

/// A mining ship. `region == None` means in transit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ship {
    pub region: Option<u8>,
    pub dest: u8,
    pub travel_ticks_remaining: u8,
    pub total_yield: u64,
}

/// Static arena definition (seeded at construction; never mutated during a run).
#[derive(Clone, Debug)]
pub struct ArenaConfig {
    pub regions: Vec<Region>,
    pub travel: Vec<Vec<u8>>, // travel[i][j] = ticks to move from region i to j; [i][i] = 0
    pub horizon: u32,
}

/// Mutable arena state advanced by `dynamics::step`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArenaState {
    pub regions: Vec<Region>,
    pub ships: Vec<Ship>,
    pub tick: u32,
}

impl ArenaState {
    /// Build the initial state: each ship starts mining its assigned region.
    pub fn from_config(cfg: &ArenaConfig, ship_start_regions: &[u8]) -> Self {
        let ships = ship_start_regions
            .iter()
            .map(|&r| Ship { region: Some(r), dest: r, travel_ticks_remaining: 0, total_yield: 0 })
            .collect();
        ArenaState { regions: cfg.regions.clone(), ships, tick: 0 }
    }
}

/// A ship's per-decision action. Integer, Copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Stay,
    MoveTo(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_and_ship_are_plain_integer_value_types() {
        let r = Region { stock: 10, richness_cap: 20, regen_per_tick: 0 };
        let s = Ship { region: Some(0), dest: 0, travel_ticks_remaining: 0, total_yield: 0 };
        assert_eq!(r.stock, 10);
        assert_eq!(s.region, Some(0));
        // Copy semantics (value types, no heap in the hot loop).
        let _r2 = r;
        let _s2 = s;
        assert_eq!(r, _r2);
    }

    #[test]
    fn arena_state_holds_regions_and_ships_and_tick() {
        let cfg = ArenaConfig {
            regions: vec![
                Region { stock: 20, richness_cap: 20, regen_per_tick: 0 },
                Region { stock: 10, richness_cap: 10, regen_per_tick: 0 },
            ],
            travel: vec![vec![0, 3], vec![3, 0]],
            horizon: 30,
        };
        let st = ArenaState::from_config(&cfg, &[0u8, 1u8]);
        assert_eq!(st.tick, 0);
        assert_eq!(st.regions.len(), 2);
        assert_eq!(st.ships.len(), 2);
        assert_eq!(st.ships[0].region, Some(0));
        assert_eq!(st.ships[1].region, Some(1));
        assert!(matches!(Action::Stay, Action::Stay));
        assert!(matches!(Action::MoveTo(1), Action::MoveTo(1)));
    }
}
