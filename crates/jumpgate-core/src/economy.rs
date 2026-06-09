//! Economy stores + systems (the first demand-driven loop, deterministic harness).
//! All economy state is hashed (HASH_FIELD_ORDER words appended in this phase) and
//! all money is i64 microcredits. Stations are Bodies; haulers dock via the live
//! co-orbiting rendezvous arrival (events.rs).

/// The commodity set for the v1 thin loop. `index()` is the canonical dense order
/// used by every per-station resource array and by the state hash. APPEND-ONLY.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resource {
    Ore,
    Fuel,
}

pub const N_RESOURCES: usize = 2;

impl Resource {
    pub const ALL: [Resource; N_RESOURCES] = [Resource::Ore, Resource::Fuel];
    pub fn index(self) -> usize {
        match self {
            Resource::Ore => 0,
            Resource::Fuel => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_index_is_stable_and_dense() {
        assert_eq!(Resource::Ore.index(), 0);
        assert_eq!(Resource::Fuel.index(), 1);
        assert_eq!(Resource::ALL.len(), N_RESOURCES);
        for (i, r) in Resource::ALL.iter().enumerate() {
            assert_eq!(r.index(), i);
        }
    }
}
