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
}
