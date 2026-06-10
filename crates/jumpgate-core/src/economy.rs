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

/// Linear demand-deflation pricing stage (Task 19 — PURE-INTEGER micro-price; no
/// float, no `to_micros`). For each station row (`0..len`, sorted by the dense
/// `slot == row` invariant) and each resource `r` with `cap[r] > 0`:
///
///   s = clamp(stock[row][r], 0, cap[r])
///   p = max(0, base_micros[r] * (2000 - s * slope_milli / cap[r]) / 1000)
///
/// At `s == 0` → `base*2`; at `s == cap` → `base*(2 - slope_milli/1000)`. The price
/// is monotone NON-INCREASING in stock. Resources with `cap[r] == 0` are SKIPPED
/// (div-by-zero guard; their price is left unchanged). On a CHANGE (`p !=` the
/// stored price) the new price is written AND a `PriceUpdate` event is emitted.
/// Deterministic, sorted-id, integer-only. Not yet wired into `World::step` (T20).
pub fn update_prices(
    stations: &mut StationStore,
    price_cfg: &crate::config::PriceCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    for row in 0..stations.ids.len() {
        for r in 0..N_RESOURCES {
            if price_cfg.cap[r] == 0 {
                continue;
            }
            let s = stations.stock[row][r].max(0).min(price_cfg.cap[r]);
            let p = (price_cfg.base_micros[r] * (2000 - s * price_cfg.slope_milli / price_cfg.cap[r])
                / 1000)
                .max(0);
            if p != stations.price_micros[row][r] {
                stations.price_micros[row][r] = p;
                if let Some(station) =
                    stations.ids.id_at(row).map(|(slot, generation)| StationId { slot, generation })
                {
                    events.emit(Event {
                        tick,
                        kind: EventKind::PriceUpdate {
                            station,
                            resource: Resource::ALL[r],
                            price_micros: p,
                        },
                    });
                }
            }
        }
    }
}

use crate::ephemeris::Ephemeris;
use crate::stores::{BodyStore, CraftRole, CraftStore, NavState, effective_params};
use crate::types::{EntityRef, NavDest};

/// Scripted dispatch + repost stage (Stage-1 — Task 17). Makes the loop SELF-RUN
/// with no external commands. DETERMINISTIC, sorted dense-id order, no RNG, no map
/// iteration. Runs in `World::step` as stage (7) — AFTER `run_producers`, BEFORE
/// `resolve_contracts` — so a same-tick repost is visible to ASSIGN and the same-tick
/// accept is escrowed/loaded by `resolve_contracts` next.
///
/// IDENTITY-NEUTRAL BY CONSTRUCTION: this stage touches NO flow counter, NO station
/// stock, NO craft cargo. REPOST only `ContractStore::push`es an `Offered` row; ASSIGN
/// only writes `craft.contract` + `role` (mirroring the single ingest ACCEPT path).
/// All escrow/load/dispatch motion stays in `resolve_contracts`.
///
///   (a) REPOST: for each route (corp+from+to+resource), find its LATEST contract
///       (highest dense row — the dedup is "skip a row if a later row shares its
///       route"). If that latest contract is TERMINAL (Completed/Failed) AND the
///       destination station's stock of the resource is below `DispatchCfg.demand_low`,
///       push a NEW Offered contract CLONING the latest contract's route fields
///       (corp/resource/qty/from/to/reward — never inferred from producer topology),
///       and emit `ContractOffered`. The seeded `ContractInit` is the first template.
///   (b) ASSIGN: each Idle hauler (sorted dense row) takes the lowest-`ContractId`
///       `Offered` contract not already claimed this stage. One hauler -> one contract.
///       `resolve_contracts` settles the accept next. `stagger_period == 0` disables
///       ASSIGN entirely (manual / RL `AcceptContract` only); REPOST is unaffected.
pub fn run_scripted_dispatch(
    contracts: &mut ContractStore,
    stations: &StationStore,
    ships: &mut CraftStore,
    dispatch: &crate::config::DispatchCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    // (a) REPOST — ORDER-UP-TO with a HYSTERESIS deadband (Task 21). The deadband is
    // store-derived (no new field, LAW 4): "in-flight pipeline" = the Σ qty of a route's
    // NON-terminal contracts, and `projected = destination stock + in_flight`. The
    // Schmitt trigger is expressed purely from contract liveness + stock:
    //
    //   * IDLE route (no non-terminal contract): START a burst only when destination
    //     stock < `demand_low` (the low edge).
    //   * BURST in progress (>=1 non-terminal contract): keep topping up while
    //     `projected < demand_high` (the order-up-to ceiling / high edge).
    //
    // So a route is NOT re-posted while its projected supply sits in the
    // `[demand_low, demand_high)` deadband — `demand_high == demand_low` collapses the
    // deadband (undamped). Posting fills the gap to the ceiling IN ONE TICK, so a fresh
    // burst yields several concurrent Offered rows for the staggered ASSIGN below to
    // spread across ticks. Bind `n` BEFORE the loop so freshly-pushed rows are not
    // reprocessed; the per-route representative is the LATEST row (skip if a later row
    // shares the route), and the in-flight sum spans ALL rows of the route.
    let n = contracts.ids.len();
    // Backstop bound on posts-per-tick-per-route (the `projected` guard already
    // terminates the loop; this caps it even on a degenerate config).
    const MAX_POSTS_PER_ROUTE: usize = 64;
    for i in 0..n {
        // Route key for row i (corp+from+to+resource). Skip if a LATER row shares it
        // (only the latest contract per route is the repost representative).
        let later_dup = (i + 1..n).any(|j| {
            contracts.corp[j] == contracts.corp[i]
                && contracts.from_station[j] == contracts.from_station[i]
                && contracts.to_station[j] == contracts.to_station[i]
                && contracts.resource[j] == contracts.resource[i]
        });
        if later_dup {
            continue;
        }
        let resource = contracts.resource[i];
        let to = contracts.to_station[i];
        let Some(to_row) = stations.ids.dense_index(to.slot, to.generation) else {
            continue;
        };
        // A degenerate qty would never raise `projected` -> guard the order-up-to loop.
        let qty = contracts.qty[i];
        if qty == 0 {
            continue;
        }
        // In-flight pipeline for this route: Σ qty of its NON-terminal contracts, and
        // the count of them (the regime selector). Spans EVERY row of the route (sorted
        // dense order, integer).
        let mut in_flight: i64 = 0;
        let mut in_flight_count: u32 = 0;
        for j in 0..n {
            let same_route = contracts.corp[j] == contracts.corp[i]
                && contracts.from_station[j] == contracts.from_station[i]
                && contracts.to_station[j] == contracts.to_station[i]
                && contracts.resource[j] == contracts.resource[i];
            if !same_route {
                continue;
            }
            let terminal = matches!(
                contracts.status[j],
                ContractStatus::Completed | ContractStatus::Failed
            );
            if !terminal {
                in_flight += contracts.qty[j] as i64;
                in_flight_count += 1;
            }
        }
        let stock = stations.stock[to_row][resource.index()];
        // Hysteresis regime: an IDLE route starts a burst only below the LOW edge; a
        // route with live contracts is already in a burst.
        let bursting = in_flight_count > 0 || stock < dispatch.demand_low;
        if !bursting {
            continue;
        }
        // Order-up-to: post (one clone per loop) while projected supply is below the
        // HIGH edge (the ceiling). `demand_high == demand_low` -> a single post brings
        // projected to/over the low edge and the loop stops (undamped one-shot).
        let mut projected = stock + in_flight;
        let mut posts = 0usize;
        while projected < dispatch.demand_high.max(dispatch.demand_low)
            && posts < MAX_POSTS_PER_ROUTE
        {
            let new_id = contracts.push(
                contracts.corp[i],
                resource,
                qty,
                contracts.from_station[i],
                to,
                contracts.reward_micros[i],
            );
            events.emit(Event {
                tick,
                kind: EventKind::ContractOffered { contract: new_id },
            });
            projected += qty as i64;
            posts += 1;
        }
    }

    // ASSIGN GATE (trader rung 1): `stagger_period == 0` turns scripted acceptance
    // OFF entirely — manual / RL-issued `AcceptContract` only. REPOST above is
    // unaffected: the board keeps flowing; nothing scripted claims it.
    if dispatch.stagger_period == 0 {
        return;
    }

    // (b) ASSIGN — STAGGERED (Task 21). Each Idle hauler (sorted dense row) claims the
    // lowest-ContractId Offered contract not already claimed (this stage or earlier).
    // STAGGER GATE: an Idle hauler in dense row `crow` may accept only on ticks where
    // `tick % stagger_period == crow % stagger_period`; `stagger_period == 1` => every
    // hauler passes every tick (no stagger). This spreads a fresh burst's acceptances
    // across `stagger_period` ticks (deterministic, integer, sorted-id, no RNG).
    let stagger = dispatch.stagger_period.max(1) as u64;
    for crow in 0..ships.ids.len() {
        if ships.role[crow] != CraftRole::Idle {
            continue;
        }
        if tick.0 % stagger != crow as u64 % stagger {
            continue;
        }
        for kidx in 0..contracts.ids.len() {
            if contracts.status[kidx] != ContractStatus::Offered {
                continue;
            }
            let cid = contract_id(contracts, kidx);
            if (0..ships.ids.len()).any(|r| ships.contract[r] == Some(cid)) {
                continue;
            }
            ships.contract[crow] = Some(cid);
            ships.role[crow] = CraftRole::Hauler;
            break;
        }
    }
}

/// Contract-resolution stage (deterministic, sorted-`ContractId` order — the dense
/// `slot == row` invariant makes `0..len` iteration already sorted). Runs after
/// command ingest (which records ACCEPT intent: `craft.contract` + `role = Hauler`)
/// and `run_producers`, before physics. Drives the accept/escrow/load lifecycle a
/// SINGLE transition-group per tick (a `match` on the current status, never a
/// fall-through chain):
///
///   * `Offered` (a hauler is bound): escrow the reward — debit the corp treasury
///     into `escrow_micros`, status `Offered->Accepted`, emit `ContractAccepted`;
///     then, if the hauler is co-located with `from_station`'s body AND the station
///     has the stock, LOAD in the same tick (status `Accepted->CargoLoaded`). If the
///     corp treasury cannot cover the reward, REVERT the assignment (clear the
///     craft's `contract`/`role`, leave the contract `Offered`, hauler `None`) —
///     deterministic, no escrow movement.
///   * `Accepted` (escrowed but not yet loaded — e.g. accepted off-station): load
///     when co-located + stocked (status `Accepted->CargoLoaded`).
///   * `CargoLoaded`: dispatch — status `CargoLoaded->InTransit` (the craft is
///     already Seeking the destination body, set at load).
///
/// LOADING is a TRANSFER (station stock -> craft cargo / in-transit): it touches NO
/// flow counter (the resource accounting identity tracks in-transit cargo). The
/// craft is dispatched by setting its nav to Seek `to_station`'s body.
#[allow(clippy::too_many_arguments)]
pub fn resolve_contracts(
    contracts: &mut ContractStore,
    corporations: &mut CorporationStore,
    stations: &mut StationStore,
    ships: &mut CraftStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    guidance: &crate::config::GuidanceParams,
    tick: Tick,
    events: &mut EventStream,
) {
    let _ = guidance; // reserved for a future dv-budget policy; v1 uses fuel-derived dv
    for kidx in 0..contracts.ids.len() {
        match contracts.status[kidx] {
            ContractStatus::Offered => {
                // The ingest ACCEPT path records intent on the CRAFT side only
                // (`craft.contract` + `role = Hauler`); the contract-side `hauler`
                // binding + escrow are deferred here. Find the accepting craft by its
                // `contract` column, lowest dense row first (sorted, no RNG). No such
                // craft -> the offer is unclaimed this tick.
                let contract = contract_id(contracts, kidx);
                let Some(crow) = (0..ships.ids.len())
                    .find(|&r| ships.contract[r] == Some(contract))
                else {
                    continue;
                };
                let hauler = ships.ids_at(crow);
                // Escrow: debit the corp treasury by the reward. Insufficient
                // treasury (or a stale corp row) -> REVERT the assignment.
                let corp = contracts.corp[kidx];
                let reward = contracts.reward_micros[kidx];
                let corp_row = corporations.ids.dense_index(corp.slot, corp.generation);
                let funded = matches!(corp_row, Some(r) if corporations.treasury_micros[r] >= reward);
                if !funded {
                    // Deterministic revert: release the craft, leave the offer open.
                    ships.contract[crow] = None;
                    ships.role[crow] = CraftRole::Idle;
                    contracts.hauler[kidx] = None;
                    continue;
                }
                let corp_row = corp_row.expect("funded implies a live corp row");
                corporations.treasury_micros[corp_row] -= reward;
                contracts.escrow_micros[kidx] += reward;
                contracts.hauler[kidx] = Some(hauler);
                contracts.status[kidx] = ContractStatus::Accepted;
                events.emit(Event {
                    tick,
                    kind: EventKind::ContractAccepted { contract, hauler },
                });
                // Same-tick load if co-located at the origin station with stock.
                try_load(contracts, stations, ships, bodies, eph, kidx, crow, tick);
            }
            ContractStatus::Accepted => {
                if let Some(hauler) = contracts.hauler[kidx]
                    && let Some(crow) = ships.index_of(hauler)
                {
                    try_load(contracts, stations, ships, bodies, eph, kidx, crow, tick);
                }
            }
            ContractStatus::CargoLoaded => {
                // Dispatch: the craft is already Seeking the destination (set at load).
                contracts.status[kidx] = ContractStatus::InTransit;
            }
            // InTransit/Delivered/Completed/Failed are resolved by later stages
            // (delivery on Arrival — Task 15; failure on FuelEmpty — Task 16).
            _ => {}
        }
    }
}

/// Construct the typed `ContractId` for dense row `kidx` (sole live-row helper).
fn contract_id(contracts: &ContractStore, kidx: usize) -> ContractId {
    let (slot, generation) = contracts.ids.id_at(kidx).expect("live contract row");
    ContractId { slot, generation }
}

/// LOAD an `Accepted` contract's cargo if the hauler (dense row `crow`) is
/// co-located with `from_station`'s body AND the station has the stock. On success:
/// move `qty` of `resource` from station stock into craft cargo (a TRANSFER — no
/// counter), set the craft Seeking `to_station`'s body, status `Accepted->CargoLoaded`.
/// A not-yet-arrived or under-stocked station is a deterministic no-op (retried next
/// tick); InTransit promotion happens the tick after CargoLoaded.
#[allow(clippy::too_many_arguments)]
fn try_load(
    contracts: &mut ContractStore,
    stations: &mut StationStore,
    ships: &mut CraftStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    kidx: usize,
    crow: usize,
    tick: Tick,
) {
    let from = contracts.from_station[kidx];
    let to = contracts.to_station[kidx];
    let resource = contracts.resource[kidx];
    let qty = contracts.qty[kidx];

    let Some(from_row) = stations.ids.dense_index(from.slot, from.generation) else {
        return;
    };
    let Some(to_row) = stations.ids.dense_index(to.slot, to.generation) else {
        return;
    };
    let from_body = stations.body[from_row];
    let Some(from_body_row) = bodies.ids.dense_index(from_body.slot, from_body.generation) else {
        return;
    };
    // Co-location: the hauler must be within ARRIVAL_RADIUS of the origin body,
    // compared IN THE CRAFT'S FRAME: resolve_contracts runs at stage (1c), BEFORE
    // physics, so `ships.pos` is still the tick-(`tick`-1) state — the same frame
    // the autopilot resolves destinations in (`resolve_dest_pos(dest, cur)`).
    // Comparing against body_pos(`tick`) mixed two time points: invisible for the
    // near-stationary fixture bodies (motion/tick ≪ ARRIVAL_RADIUS), but a body on
    // a 1-M_sun orbit moves ~73x ARRIVAL_RADIUS per tick (0.029 AU/day × 0.25 day),
    // so a hauler PERFECTLY tracking its pickup body could never pass the gate and
    // the load starved forever (found by the trader-rung scenario).
    let body_pos = eph.body_pos(bodies.eph_index[from_body_row], Tick(tick.0.saturating_sub(1)));
    if ships.pos[crow].sub(body_pos).length() > crate::autopilot::ARRIVAL_RADIUS {
        // Not at the pickup yet: WALK TO THE FOOD. Dispatch the hauler to Seek the
        // origin body so a hauler that just delivered elsewhere (now Idle but bound to
        // this contract) returns to load it — this deadhead leg is what CLOSES the
        // forage loop (deliver -> return -> reload -> deliver ...). Set the nav ONCE
        // (idempotent): if the craft is already Seeking this origin, leave dv_remaining
        // alone so the budget depletes over the trip instead of resetting every tick.
        let from_dest = NavDest::Entity(EntityRef::Body(from_body));
        let already_seeking_origin = matches!(
            ships.nav[crow],
            NavState::Seeking { dest, .. } if dest == from_dest
        );
        if !already_seeking_origin {
            let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
            let dv =
                crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ships.fuel_mass[crow]);
            ships.nav[crow] = NavState::Seeking { dest: from_dest, dv_remaining: dv };
        }
        return;
    }
    // Stock gate: the origin station must hold the cargo.
    if stations.stock[from_row][resource.index()] < qty as i64 {
        return;
    }
    // TRANSFER station stock -> craft cargo (in-transit). No counter touched.
    stations.stock[from_row][resource.index()] -= qty as i64;
    ships.cargo[crow] = Some((resource, qty));
    // Dispatch: Seek the destination body. dv budget is fuel-derived (mirrors the
    // ingest path's no-explicit-budget rule), never INFINITY into dv_remaining.
    let to_body = stations.body[to_row];
    let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
    let dv = crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ships.fuel_mass[crow]);
    ships.nav[crow] = NavState::Seeking {
        dest: NavDest::Entity(EntityRef::Body(to_body)),
        dv_remaining: dv,
    };
    contracts.status[kidx] = ContractStatus::CargoLoaded;
    let _ = tick; // load itself emits no event in v1 (ContractAccepted already fired)
}

/// Delivery-on-arrival settlement stage (deterministic, sorted-`ContractId` order).
/// Runs in `World::step` AFTER boundary-event detection: `arrivals` is the list of
/// `(craft, dest)` pairs lifted from this tick's just-detected `Arrival` events. For
/// each `InTransit` contract whose bound `hauler` arrived at its `to_station`'s body:
///
///   * unload the craft's cargo into `to_station` stock (a TRANSFER — touches NO
///     `mined`/`consumed` counter; the resource identity already accounts in-transit
///     cargo on the way out at load),
///   * pay `escrow_micros` -> the craft's `credits_micros`, zero the escrow (a credit
///     TRANSFER: Σtreasury+Σcredits+Σescrow is invariant — corp money escrowed at
///     accept lands in the hauler's account),
///   * clear the craft's `cargo`/`contract`/`role` (`Hauler->Idle`),
///   * status `InTransit -> Completed` (the `Delivered` waypoint collapses into the
///     same settlement), emit `ContractFulfilled`.
///
/// The destination match uses the `Arrival` event's `dest` (already the destination
/// `BodyId`) against `stations.body[to_station]`, so this stage needs no ephemeris.
pub fn resolve_deliveries(
    contracts: &mut ContractStore,
    stations: &mut StationStore,
    ships: &mut CraftStore,
    arrivals: &[(CraftId, NavDest)],
    tick: Tick,
    events: &mut EventStream,
) {
    for kidx in 0..contracts.ids.len() {
        if contracts.status[kidx] != ContractStatus::InTransit {
            continue;
        }
        let Some(hauler) = contracts.hauler[kidx] else {
            continue;
        };
        // Resolve the destination station's body; a stale row -> skip (deterministic).
        let to = contracts.to_station[kidx];
        let Some(to_row) = stations.ids.dense_index(to.slot, to.generation) else {
            continue;
        };
        let to_body = stations.body[to_row];
        let dest = NavDest::Entity(EntityRef::Body(to_body));
        // Did this contract's hauler arrive at the destination body THIS tick?
        if !arrivals.iter().any(|&(c, d)| c == hauler && d == dest) {
            continue;
        }
        let Some(crow) = ships.index_of(hauler) else {
            continue;
        };
        // Unload: craft cargo (in-transit) -> destination station stock. TRANSFER only.
        if let Some((resource, qty)) = ships.cargo[crow] {
            stations.stock[to_row][resource.index()] += qty as i64;
        }
        // Settle escrow -> craft credits (credit TRANSFER; identity invariant).
        let payout = contracts.escrow_micros[kidx];
        contracts.escrow_micros[kidx] = 0;
        ships.credits_micros[crow] += payout;
        // Release the hauler.
        ships.cargo[crow] = None;
        ships.contract[crow] = None;
        ships.role[crow] = CraftRole::Idle;
        // Terminal status + fulfilment event.
        contracts.status[kidx] = ContractStatus::Completed;
        let contract = contract_id(contracts, kidx);
        events.emit(Event {
            tick,
            kind: EventKind::ContractFulfilled { contract, hauler },
        });
    }
}

/// Contract-failure stage on propellant exhaustion (deterministic, sorted-`ContractId`
/// order). Runs in `World::step` AFTER the delivery stage (3b): `failed_craft` is the
/// list of craft-ids lifted from this tick's just-detected `FuelEmpty` events. For
/// each `InTransit` contract whose bound `hauler` ran out of propellant this tick:
///
///   * status `InTransit -> Failed`,
///   * refund `escrow_micros` -> the owning corp's `treasury_micros`, zero the escrow
///     (a credit TRANSFER: Σtreasury+Σcredits+Σescrow is invariant — the money the corp
///     escrowed at accept returns to the corp; the hauler is NOT paid),
///   * **v1: the loaded cargo is LOST.** Account it into `consumed[resource]` and clear
///     the craft's `cargo`. This keeps the resource identity
///     (`Σstock + in_transit == initial + mined − consumed`) exact: the cargo was
///     debited from origin stock at load and rode as in-transit; on loss the sink leg
///     (`consumed += qty`) balances the now-vanished in-transit cargo.
///   * clear the craft's `contract`/`role` (`Hauler -> Idle`).
///
/// Ordering after 3b is load-bearing: a same-tick Arrival+FuelEmpty resolves as
/// DELIVERED (3b already moved the contract off `InTransit`, so 3c skips it). Removes
/// nothing from any store (status-only lifecycle, LAW 6).
pub fn resolve_failures(
    contracts: &mut ContractStore,
    corporations: &mut CorporationStore,
    ships: &mut CraftStore,
    counters: &mut EconCounters,
    failed_craft: &[CraftId],
) {
    for kidx in 0..contracts.ids.len() {
        if contracts.status[kidx] != ContractStatus::InTransit {
            continue;
        }
        let Some(hauler) = contracts.hauler[kidx] else {
            continue;
        };
        // Did this contract's hauler run out of propellant THIS tick?
        if !failed_craft.contains(&hauler) {
            continue;
        }
        // Refund escrow -> owning corp treasury (credit TRANSFER; identity invariant).
        // A stale corp row -> skip the refund leg but still fail the contract; the
        // escrow stays put so Σtreasury+Σcredits+Σescrow remains invariant.
        let corp = contracts.corp[kidx];
        if let Some(corp_row) = corporations.ids.dense_index(corp.slot, corp.generation) {
            corporations.treasury_micros[corp_row] += contracts.escrow_micros[kidx];
            contracts.escrow_micros[kidx] = 0;
        }
        // Cargo loss: account the lost cargo as a SINK leg, then clear it.
        if let Some(crow) = ships.index_of(hauler) {
            if let Some((resource, qty)) = ships.cargo[crow] {
                counters.consumed[resource.index()] += qty as i64;
                ships.cargo[crow] = None;
            }
            ships.contract[crow] = None;
            ships.role[crow] = CraftRole::Idle;
        }
        // Terminal status. v1 emits no dedicated failure event (the FuelEmpty event
        // already fired this tick and carries the cause); adding an EventKind variant
        // is out of scope for Phase 1b (would force a jumpgate-py exhaustive-match
        // update).
        contracts.status[kidx] = ContractStatus::Failed;
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
    fn update_prices_linear_deflation_exact_integer() {
        use crate::config::PriceCfg;
        // base_micros[Fuel]=100_000, cap[Fuel]=10, slope_milli=1800.
        // Ore has cap==0 -> SKIPPED (div-by-zero guard, price left unchanged).
        let price_cfg = PriceCfg {
            slope_milli: 1800,
            base_micros: [0, 100_000],
            cap: [0, 10],
            ..PriceCfg::default()
        };

        // Five station rows with increasing Fuel stock: 0, 3, 5, 8, 10.
        // Ore gets a non-zero initial price (777) we expect to be LEFT UNCHANGED.
        let fuel_stocks = [0i64, 3, 5, 8, 10];
        let mut stations = StationStore::empty();
        for (i, &fstock) in fuel_stocks.iter().enumerate() {
            stations.push(
                BodyId { slot: i as u32, generation: 0 },
                [0, fstock],
                [777, 0],
            );
        }
        let mut events = EventStream::new();

        update_prices(&mut stations, &price_cfg, Tick(1), &mut events);

        let fi = Resource::Fuel.index();
        let oi = Resource::Ore.index();
        // EXACT integer prices: p = 100_000*(2000 - s*1800/10)/1000.
        //   s=0  -> 100_000*2000/1000        = 200_000  (= base*2)
        //   s=3  -> 100_000*(2000-540)/1000  = 146_000
        //   s=5  -> 100_000*(2000-900)/1000  = 110_000
        //   s=8  -> 100_000*(2000-1440)/1000 =  56_000
        //   s=10 -> 100_000*(2000-1800)/1000 =  20_000  (= base*(2-1.8))
        assert_eq!(stations.price_micros[0][fi], 200_000, "stock 0 -> base*2");
        assert_eq!(stations.price_micros[4][fi], 20_000, "stock cap -> base*0.2");
        assert_eq!(stations.price_micros[1][fi], 146_000);
        assert_eq!(stations.price_micros[2][fi], 110_000);
        assert_eq!(stations.price_micros[3][fi], 56_000);

        // Monotone NON-INCREASING across rows (sorted dense-id iteration).
        for row in 1..stations.ids.len() {
            assert!(
                stations.price_micros[row][fi] <= stations.price_micros[row - 1][fi],
                "fuel price non-increasing across rows"
            );
        }

        // cap==0 resource (Ore) is SKIPPED: its pushed price is untouched.
        for row in 0..stations.ids.len() {
            assert_eq!(stations.price_micros[row][oi], 777, "Ore (cap==0) price unchanged");
        }

        // Exactly one PriceUpdate per row (Fuel changed 0 -> p on every row),
        // and NONE for the skipped Ore resource.
        let updates: Vec<_> = events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::PriceUpdate { .. }))
            .collect();
        assert_eq!(updates.len(), fuel_stocks.len(), "one PriceUpdate per row (Fuel)");
        assert!(updates.iter().all(|e| matches!(
            e.kind,
            EventKind::PriceUpdate { resource: Resource::Fuel, .. }
        )));
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

    #[test]
    fn stagger_period_zero_disables_assign_but_not_repost() {
        // ASSIGN gate (trader rung 1): `stagger_period == 0` turns scripted
        // acceptance OFF entirely (manual / RL-issued `AcceptContract` only) while
        // REPOST keeps the board flowing. Fixture: 1 corp, 2 stations, 1 TERMINAL
        // route template whose destination stock sits below `demand_low` (so REPOST
        // fires), 1 Idle craft (which ASSIGN must NOT claim).
        use crate::config::{BaseSpec, DispatchCfg};
        use crate::math::Vec3;

        let mut stations = StationStore::empty();
        let from = stations.push(BodyId { slot: 0, generation: 0 }, [0, 100], [0; N_RESOURCES]);
        let to = stations.push(BodyId { slot: 1, generation: 0 }, [0, 0], [0; N_RESOURCES]);
        let mut corporations = CorporationStore::empty();
        let corp = corporations.push(1_000_000, from);
        let mut contracts = ContractStore::empty();
        let seed = contracts.push(corp, Resource::Fuel, 5, from, to, 1_000);
        let srow = contracts.ids.dense_index(seed.slot, seed.generation).unwrap();
        contracts.status[srow] = ContractStatus::Completed;
        let mut ships = CraftStore::empty();
        ships.push(
            BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
            },
            Vec3::ZERO,
            Vec3::ZERO,
            0.0,
        );
        // demand_low == demand_high == 5 (== qty): destination stock 0 < 5 starts a
        // burst, ONE post brings projected to the ceiling -> exactly one repost.
        let dispatch = DispatchCfg {
            demand_low: 5,
            demand_high: 5,
            stagger_period: 0,
            contract_reward_micros: 0,
            contract_qty: 0,
        };
        let mut events = EventStream::new();
        let n_before = contracts.ids.len();

        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &dispatch,
            Tick(1),
            &mut events,
        );

        // REPOST is UNAFFECTED: exactly one fresh Offered clone of the terminal route.
        assert_eq!(
            contracts.ids.len(),
            n_before + 1,
            "REPOST still posts with stagger_period == 0"
        );
        assert_eq!(contracts.status[n_before], ContractStatus::Offered);
        assert!(
            events
                .events
                .iter()
                .any(|e| matches!(e.kind, EventKind::ContractOffered { .. })),
            "ContractOffered emitted by the repost"
        );
        // ASSIGN is OFF: the Idle craft is untouched (no scripted claim).
        assert_eq!(ships.role[0], CraftRole::Idle, "craft stays Idle");
        assert_eq!(ships.contract[0], None, "no contract bound to the craft");
    }
}
