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

use crate::contract::{Event, EventKind};
use crate::events::EventStream;

/// Producer firing stage (deterministic, sorted-`ProducerId` order — the dense
/// `slot == row` invariant makes `0..len` iteration already sorted). Fires a
/// producer when `tick - last_fired >= interval`, all-or-nothing on the input
/// leg. PER-LEG counter discipline (the T18 resource identity depends on it):
///
///   * input  (r_in,  q): `stock[r_in]  -= q`  AND  `consumed[r_in]  += q`
///   * output (r_out, q): `stock[r_out] += q`  AND  `mined[r_out]    += q`
///
/// So a refiner (Ore->Fuel) bumps BOTH `consumed[Ore]` AND `mined[Fuel]`. A
/// `Production` event is emitted only when there is an output leg. A producer
/// that skips for insufficient input does NOT advance `last_fired` (it retries
/// the next eligible tick).
pub fn run_producers(
    stations: &mut StationStore,
    producers: &mut ProducerStore,
    counters: &mut EconCounters,
    tick: Tick,
    events: &mut EventStream,
) {
    for idx in 0..producers.ids.len() {
        let recipe = producers.recipe[idx];
        // Interval gate: u64 arithmetic on the raw tick counters.
        if tick.0 - producers.last_fired[idx].0 < recipe.interval as u64 {
            continue;
        }
        // Resolve the producer's station to its dense row (slot == row in v1).
        let st = producers.station[idx];
        let srow = match stations.ids.dense_index(st.slot, st.generation) {
            Some(r) => r,
            None => continue,
        };
        // All-or-nothing: if there is an input leg and the station can't cover it,
        // skip WITHOUT advancing last_fired.
        if let Some((r_in, q)) = recipe.input
            && stations.stock[srow][r_in.index()] < q as i64
        {
            continue;
        }
        // Apply the input leg: debit stock, bump consumed.
        if let Some((r_in, q)) = recipe.input {
            stations.stock[srow][r_in.index()] -= q as i64;
            counters.consumed[r_in.index()] += q as i64;
        }
        // Apply the output leg: credit stock, bump mined, emit Production.
        if let Some((r_out, q)) = recipe.output {
            stations.stock[srow][r_out.index()] += q as i64;
            counters.mined[r_out.index()] += q as i64;
            let producer = producers.ids.id_at(idx).map(|(slot, generation)| ProducerId { slot, generation });
            if let Some(producer) = producer {
                events.emit(Event {
                    tick,
                    kind: EventKind::Production { producer, resource: r_out, qty: q },
                });
            }
        }
        producers.last_fired[idx] = tick;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::EventKind;
    use crate::events::EventStream;
    use crate::ids::BodyId;

    /// A station whose Body is a throwaway (BodyId not resolved here; producers
    /// read by StationId row, not body).
    fn one_station(stock: [i64; N_RESOURCES]) -> StationStore {
        let mut s = StationStore::empty();
        s.push(BodyId { slot: 0, generation: 0 }, stock, [0; N_RESOURCES]);
        s
    }

    #[test]
    fn run_producers_miner_mines_ore_and_counts() {
        // Miner: ∅ -> Ore(5), interval 1. Source leg only: mined[Ore]+=5.
        let mut stations = one_station([0, 0]);
        let st = StationId { slot: 0, generation: 0 };
        let mut producers = ProducerStore::empty();
        producers.push(
            st,
            Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 1 },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);

        assert_eq!(stations.stock[0][Resource::Ore.index()], 5, "ore stock rose by 5");
        assert_eq!(counters.mined[Resource::Ore.index()], 5, "mined[Ore]==5");
        assert_eq!(counters.consumed[Resource::Ore.index()], 0);
        assert_eq!(producers.last_fired[0], Tick(1), "last_fired advanced");
        // exactly one Production event for the miner output leg.
        let prod: Vec<_> = events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Production { .. }))
            .collect();
        assert_eq!(prod.len(), 1);
        assert!(matches!(
            prod[0].kind,
            EventKind::Production { resource: Resource::Ore, qty: 5, .. }
        ));
    }

    #[test]
    fn run_producers_refiner_bumps_both_legs() {
        // Refiner: Ore(3) -> Fuel(2), interval 1. PER-LEG: consumed[Ore]+=3 AND
        // mined[Fuel]+=2 (the dual-bump the T18 identity depends on).
        let mut stations = one_station([10, 0]);
        let st = StationId { slot: 0, generation: 0 };
        let mut producers = ProducerStore::empty();
        producers.push(
            st,
            Recipe {
                input: Some((Resource::Ore, 3)),
                output: Some((Resource::Fuel, 2)),
                interval: 1,
            },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);

        assert_eq!(stations.stock[0][Resource::Ore.index()], 7, "ore -3");
        assert_eq!(stations.stock[0][Resource::Fuel.index()], 2, "fuel +2");
        assert_eq!(counters.consumed[Resource::Ore.index()], 3, "consumed[Ore]==3");
        assert_eq!(counters.mined[Resource::Fuel.index()], 2, "mined[Fuel]==2");
        assert_eq!(counters.mined[Resource::Ore.index()], 0);
        assert_eq!(counters.consumed[Resource::Fuel.index()], 0);
        // Production event names the OUTPUT leg.
        let prod: Vec<_> = events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Production { .. }))
            .collect();
        assert_eq!(prod.len(), 1);
        assert!(matches!(
            prod[0].kind,
            EventKind::Production { resource: Resource::Fuel, qty: 2, .. }
        ));
    }

    #[test]
    fn run_producers_all_or_nothing_skip_on_insufficient_input() {
        // Refiner needs Ore(5) but station has 2 -> skip: no stock change, no
        // counter change, no event, last_fired NOT advanced (retries next tick).
        let mut stations = one_station([2, 0]);
        let st = StationId { slot: 0, generation: 0 };
        let mut producers = ProducerStore::empty();
        producers.push(
            st,
            Recipe {
                input: Some((Resource::Ore, 5)),
                output: Some((Resource::Fuel, 2)),
                interval: 1,
            },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);

        assert_eq!(stations.stock[0], [2, 0], "no stock change on skip");
        assert_eq!(counters, EconCounters::zero(), "no counter change on skip");
        assert_eq!(producers.last_fired[0], Tick(0), "last_fired NOT advanced on skip");
        assert!(events.events.is_empty(), "no Production on skip");
    }

    #[test]
    fn run_producers_respects_interval() {
        // Miner interval 3: fires at tick 3 (3-0>=3), not at tick 1 or 2.
        let mut stations = one_station([0, 0]);
        let st = StationId { slot: 0, generation: 0 };
        let mut producers = ProducerStore::empty();
        producers.push(
            st,
            Recipe { input: None, output: Some((Resource::Ore, 1)), interval: 3 },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);
        run_producers(&mut stations, &mut producers, &mut counters, Tick(2), &mut events);
        assert_eq!(stations.stock[0][Resource::Ore.index()], 0, "not yet (interval gate)");

        run_producers(&mut stations, &mut producers, &mut counters, Tick(3), &mut events);
        assert_eq!(stations.stock[0][Resource::Ore.index()], 1, "fires at tick 3");
        assert_eq!(producers.last_fired[0], Tick(3));
    }

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
