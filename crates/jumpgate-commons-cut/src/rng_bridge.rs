use crate::{ArenaConfig, Region, STOCK_MAX};
use jumpgate_core::rng::{RngStream, RngStreams};
use rand_core::Rng;

/// Build a deterministic `ArenaConfig` from a master seed. Integer-only.
/// `field_correlation` is per-mille in `0..=1000`: 0 = independent diverse caps,
/// 1000 = all caps identical (the negative control). `regen` sets every region's
/// regen_per_tick (0 = one-shot exhaustion).
pub fn build_scenario(
    master: u64,
    n_regions: u8,
    travel_ticks: u8,
    regen: u32,
    field_correlation: u32,
) -> ArenaConfig {
    let mut streams = RngStreams::from_master(master);
    let rng = streams.stream(RngStream::Scenario);

    // One shared "base richness" draw; per-region caps blend toward it by correlation.
    let base = 1 + (rng.next_u32() % STOCK_MAX); // 1..=STOCK_MAX
    let corr = field_correlation.min(1000);
    let regions = (0..n_regions)
        .map(|_| {
            let indep = 1 + (rng.next_u32() % STOCK_MAX); // 1..=STOCK_MAX
            // Integer blend: cap = (corr*base + (1000-corr)*indep) / 1000, clamped to >=1.
            let cap = ((corr as u64 * base as u64 + (1000 - corr) as u64 * indep as u64) / 1000)
                .max(1) as u32;
            Region { stock: cap, richness_cap: cap, regen_per_tick: regen } // start full
        })
        .collect::<Vec<_>>();

    // Fully-connected uniform travel matrix; [i][i] = 0.
    let travel = (0..n_regions as usize)
        .map(|i| (0..n_regions as usize).map(|j| if i == j { 0 } else { travel_ticks }).collect())
        .collect();

    ArenaConfig { regions, travel, horizon: 30 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_config_diff_seed_diff() {
        let a = build_scenario(42, 3, 3, 0, 0);
        let b = build_scenario(42, 3, 3, 0, 0);
        let c = build_scenario(43, 3, 3, 0, 0);
        assert_eq!(a.regions, b.regions, "same seed -> identical caps (determinism)");
        assert_ne!(a.regions, c.regions, "different seed -> different caps");
        assert_eq!(a.regions.len(), 3);
    }

    #[test]
    fn full_correlation_makes_all_caps_equal() {
        let cfg = build_scenario(42, 3, 4, 0, 1000); // corr = 1000 per-mille = identical
        let cap0 = cfg.regions[0].richness_cap;
        assert!(cfg.regions.iter().all(|r| r.richness_cap == cap0),
            "field_correlation=1000 -> identical regions (negative control)");
    }
}
