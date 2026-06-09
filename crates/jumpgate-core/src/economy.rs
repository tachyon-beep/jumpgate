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

use crate::ids::{BodyId, ContractId, CorporationId, CraftId, ProducerId, SlotMap, StationId};
use crate::time::Tick;

/// A producer's recipe: optional input consumed, optional output produced, every
/// `interval` ticks (all-or-nothing). Mining = (None, Some(Ore)); refine =
/// (Some(Ore), Some(Fuel)); demand sink = (Some(Fuel), None).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub input: Option<(Resource, u32)>,
    pub output: Option<(Resource, u32)>,
    pub interval: u32,
}

/// Stations: a market (per-resource integer stock + integer micro-price) attached
/// to a Body (its orbital position). slot == row, no mid-run despawn (v1 invariant).
pub struct StationStore {
    pub ids: SlotMap<()>,
    pub body: Vec<BodyId>,
    pub stock: Vec<[i64; N_RESOURCES]>,
    pub price_micros: Vec<[i64; N_RESOURCES]>,
}

impl StationStore {
    pub fn empty() -> Self {
        StationStore {
            ids: SlotMap::new(),
            body: Vec::new(),
            stock: Vec::new(),
            price_micros: Vec::new(),
        }
    }
    /// Append a station; returns its StationId. Enforces slot == row.
    pub fn push(
        &mut self,
        body: BodyId,
        stock: [i64; N_RESOURCES],
        price_micros: [i64; N_RESOURCES],
    ) -> StationId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(slot as usize, self.body.len(), "station slot == row");
        self.body.push(body);
        self.stock.push(stock);
        self.price_micros.push(price_micros);
        StationId { slot, generation }
    }
}

pub struct ProducerStore {
    pub ids: SlotMap<()>,
    pub station: Vec<StationId>,
    pub recipe: Vec<Recipe>,
    pub last_fired: Vec<Tick>,
}

impl ProducerStore {
    pub fn empty() -> Self {
        ProducerStore {
            ids: SlotMap::new(),
            station: Vec::new(),
            recipe: Vec::new(),
            last_fired: Vec::new(),
        }
    }
    pub fn push(&mut self, station: StationId, recipe: Recipe) -> ProducerId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(slot as usize, self.station.len(), "producer slot == row");
        self.station.push(station);
        self.recipe.push(recipe);
        self.last_fired.push(Tick(0));
        ProducerId { slot, generation }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContractStatus {
    Offered,
    Accepted,
    CargoLoaded,
    InTransit,
    Delivered,
    Completed,
    Failed,
}

impl ContractStatus {
    /// Stable discriminant for self-delimiting state-hash folding. APPEND-ONLY.
    pub fn rank(self) -> u8 {
        match self {
            ContractStatus::Offered => 0,
            ContractStatus::Accepted => 1,
            ContractStatus::CargoLoaded => 2,
            ContractStatus::InTransit => 3,
            ContractStatus::Delivered => 4,
            ContractStatus::Completed => 5,
            ContractStatus::Failed => 6,
        }
    }
}

pub struct CorporationStore {
    pub ids: SlotMap<()>,
    pub treasury_micros: Vec<i64>,
    pub home_station: Vec<StationId>,
}

impl CorporationStore {
    pub fn empty() -> Self {
        CorporationStore {
            ids: SlotMap::new(),
            treasury_micros: Vec::new(),
            home_station: Vec::new(),
        }
    }
    pub fn push(&mut self, treasury_micros: i64, home_station: StationId) -> CorporationId {
        let (slot, generation) = self.ids.insert(());
        debug_assert_eq!(
            slot as usize,
            self.treasury_micros.len(),
            "corp slot == row"
        );
        self.treasury_micros.push(treasury_micros);
        self.home_station.push(home_station);
        CorporationId { slot, generation }
    }
}

/// Delivery contracts: move `qty` of `resource` from `from_station` to `to_station`
/// for `reward_micros` (escrowed at accept). status enum + escrow are the hashed
/// lifecycle. `hauler` is set on accept.
pub struct ContractStore {
    pub ids: SlotMap<()>,
    pub status: Vec<ContractStatus>,
    pub corp: Vec<CorporationId>,
    pub resource: Vec<Resource>,
    pub qty: Vec<u32>,
    pub from_station: Vec<StationId>,
    pub to_station: Vec<StationId>,
    pub reward_micros: Vec<i64>,
    pub escrow_micros: Vec<i64>,
    pub hauler: Vec<Option<CraftId>>,
}

impl ContractStore {
    pub fn empty() -> Self {
        ContractStore {
            ids: SlotMap::new(),
            status: Vec::new(),
            corp: Vec::new(),
            resource: Vec::new(),
            qty: Vec::new(),
            from_station: Vec::new(),
            to_station: Vec::new(),
            reward_micros: Vec::new(),
            escrow_micros: Vec::new(),
            hauler: Vec::new(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn push(
        &mut self,
        corp: CorporationId,
        resource: Resource,
        qty: u32,
        from_station: StationId,
        to_station: StationId,
        reward_micros: i64,
    ) -> ContractId {
        let (slot, generation) = self.ids.insert(());
        self.status.push(ContractStatus::Offered);
        self.corp.push(corp);
        self.resource.push(resource);
        self.qty.push(qty);
        self.from_station.push(from_station);
        self.to_station.push(to_station);
        self.reward_micros.push(reward_micros);
        self.escrow_micros.push(0);
        self.hauler.push(None);
        ContractId { slot, generation }
    }
}

/// Audited per-resource flow counters (i64 units). `mined` accumulates SOURCE legs
/// (a producer output with no input); `consumed` accumulates SINK legs (a producer
/// input with no resold output, plus accounted cargo loss on a Failed contract).
/// They make the resource accounting identity exact:
/// `Σstock(r) + in_transit(r) == initial(r) + mined(r) − consumed(r)`.
/// Mutable per-tick state → HASHED (folded in state_hash at the version-2 bump).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EconCounters {
    pub mined: [i64; N_RESOURCES],
    pub consumed: [i64; N_RESOURCES],
}

impl EconCounters {
    /// All-zero counters (the reset state).
    pub fn zero() -> Self {
        EconCounters { mined: [0; N_RESOURCES], consumed: [0; N_RESOURCES] }
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

    #[test]
    fn corp_and_contract_stores_start_empty() {
        let c = CorporationStore::empty();
        assert_eq!(c.ids.len(), 0);
        assert_eq!(c.treasury_micros.len(), 0);

        let k = ContractStore::empty();
        assert_eq!(k.ids.len(), 0);
        assert_eq!(k.status.len(), 0);
        // status rank is total + distinct (used by the state hash).
        assert_eq!(ContractStatus::Offered.rank(), 0);
        assert_ne!(ContractStatus::Failed.rank(), ContractStatus::Completed.rank());
    }

    #[test]
    fn station_and_producer_stores_start_empty_and_parallel() {
        let s = StationStore::empty();
        assert_eq!(s.ids.len(), 0);
        assert_eq!(s.body.len(), 0);
        assert_eq!(s.stock.len(), 0);
        assert_eq!(s.price_micros.len(), 0);

        let p = ProducerStore::empty();
        assert_eq!(p.ids.len(), 0);
        assert_eq!(p.station.len(), 0);
        assert_eq!(p.recipe.len(), 0);
        assert_eq!(p.last_fired.len(), 0);
    }
}
