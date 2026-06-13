//! Economy stores + systems (the first demand-driven loop, deterministic harness).
//! All economy state is hashed (HASH_FIELD_ORDER words appended in this phase) and
//! all money is i64 microcredits. Stations are Bodies; haulers dock via the live
//! co-orbiting rendezvous arrival (events.rs).

/// Runtime goods newtype (OD-1).  Dense index `0..n_goods` is the canonical
/// per-resource array key; the numeric value is the GoodsCfg order and is
/// NEVER folded as a count word — only the value is emitted to the state hash.
/// Named constants ORE/FUEL pin the v1 pair at indices 0 and 1 (tested by
/// `good_ore_and_fuel_pinned_indices`); appending new goods is config-only.
/// Custom `Debug` preserves the v1 canonical names "Ore"/"Fuel" for indices
/// 0 and 1 so the gossip-log format is unchanged (baseline-digest continuity).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Good(pub u16);

impl std::fmt::Debug for Good {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => write!(f, "Ore"),
            1 => write!(f, "Fuel"),
            n => write!(f, "Good({n})"),
        }
    }
}

impl Good {
    /// Canonical dense index (0-based); used by every per-resource array and
    /// by the state-hash fold.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// v1 pinned goods.  Indices are VERIFIED by `good_ore_and_fuel_pinned_indices`.
    pub const ORE:  Good = Good(0);
    pub const FUEL: Good = Good(1);

    /// All v1 base goods in canonical index order.  Used only by `update_prices`
    /// to build PriceUpdate events; code that needs a runtime count should read
    /// `n_goods` from GoodsCfg (A3).
    pub const ALL_V1: [Good; N_RESOURCES] = [Good::ORE, Good::FUEL];
}

/// Backward-compatible alias kept for migration in A1a; removed in A1b once all
/// call sites are updated.  Declared after `Good` so `Resource::Ore.index()`
/// still compiles, easing the mechanical conversion.
#[allow(non_camel_case_types, dead_code)]
#[deprecated(since = "0.0.0", note = "migrate to Good::ORE / Good::FUEL (A1)")]
pub type Resource = Good;

/// Backward-compat shim: re-export old names as associated consts.
#[allow(dead_code, non_upper_case_globals)]
impl Good {
    #[deprecated(since = "0.0.0", note = "use Good::ORE")]
    pub const Ore:  Good = Good::ORE;
    #[deprecated(since = "0.0.0", note = "use Good::FUEL")]
    pub const Fuel: Good = Good::FUEL;
}

/// Number of base goods in v1 (pinned; Experiment C raises this via config).
/// Used ONLY for fixed-size array literals that survive until A1b converts them
/// to Vecs; do NOT introduce new uses.
pub const N_RESOURCES: usize = 2;

use crate::diagnostics::permille_floor;
use crate::ids::{BodyId, ContractId, CorporationId, CraftId, ProducerId, SlotMap, StationId};
use crate::time::Tick;

/// A producer's recipe: optional input consumed, optional output produced, every
/// `interval` ticks (all-or-nothing). Mining = (None, Some(Ore)); refine =
/// (Some(Ore), Some(Fuel)); demand sink = (Some(Fuel), None).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Recipe {
    pub input: Option<(Good, u32)>,
    pub output: Option<(Good, u32)>,
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
    pub resource: Vec<Good>,
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
        resource: Good,
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

use crate::contract::{Event, EventKind, RefuelDeniedReason};
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
                            resource: Good::ALL_V1[r],
                            price_micros: p,
                        },
                    });
                }
            }
        }
    }
}

use crate::ephemeris::Ephemeris;
use crate::stores::{BodyStore, CraftRole, CraftStore, NavState, UpgradeLevels, effective_params};
use crate::types::{EntityRef, NavDest};

/// Effective cargo capacity (units) — DERIVED at the read site, never stored
/// (the fleet-ledger discipline, spec §6): `base + hulls * hull_step_units`.
/// Saturating throughout: absurd config values degrade deterministically
/// instead of wrapping (the spec §8 totality discipline).
pub fn cargo_capacity(
    spec: &crate::config::BaseSpec,
    upgrades: UpgradeLevels,
    shipyard: &crate::config::ShipyardCfg,
) -> u32 {
    spec.base_cargo_capacity
        .saturating_add((upgrades.hulls as u32).saturating_mul(shipyard.hull_step_units))
}

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
///   (b) ASSIGN: each Idle SCRIPTED hauler (sorted dense row; `!scripted` craft
///       are skipped — the spec-§5 gym-exclusion law) takes an `Offered` contract
///       not already claimed this stage that FITS its derived cargo capacity (the
///       pirates-rung §6 filter-at-choice — scripted roles never claim-and-revert).
///       With `trophic.hauler_belief_scoring` OFF (default) the pick is the
///       lowest-`ContractId`; ON, it is the spec-§7 evidence-scored argmax
///       `reward_micros * (1000 - min(route_evidence * evidence_penalty_milli, 900)) / 1000`
///       (ties -> lowest `ContractId`). The COUNT is the media rung's swapped
///       read (media spec §7, Task 7): `media_live` -> the hauler's OWN
///       comms-log on the contract's route, per-reader `first_heard` window
///       (`info_tick`'s evidence-read role ends; it stays the dock detector);
///       media off -> the legacy ring through the hauler's dock-gated
///       `info_tick`, byte-identical. The 900-clamp valence around the count
///       is untouched either way. One hauler -> one contract.
///       `resolve_contracts` settles the accept next. `stagger_period == 0`
///       disables ASSIGN entirely (manual / RL `AcceptContract` only); REPOST
///       is unaffected.
/// UNHASHED ASSIGN instrumentation (the `engagement_diag` pattern — read only
/// by the diagnostics sampler, NEVER a behavior input; PDR-0006: a window,
/// not a gate). The two WHY-panel windows (2026-06-11): how often the
/// evidence count at decision time sits on the flat (clamped) region of the
/// avoidance transfer function, and how often the gossip read and the legacy
/// ring read would pick a DIFFERENT contract (the argmax-flip share — the
/// direct measure of how much of the channel's realism reaches play).
#[derive(Default)]
pub struct AssignDiag {
    /// Belief-scored picks made.
    pub decisions: u64,
    /// Picks where the counterfactual source's argmax differed. Counted only
    /// when media is live (gossip = the active source, the legacy ring = the
    /// pure-read counterfactual); the ring arm has no defined counterfactual.
    pub flips: u64,
    /// Histogram of the ACTIVE evidence count per scored candidate:
    /// buckets 0,1,2,3,4,5,>=6 (>=6 == the 900-clamp flat region).
    pub candidate_counts: [u64; 7],
}

#[allow(clippy::too_many_arguments)]
pub fn run_scripted_dispatch(
    contracts: &mut ContractStore,
    stations: &StationStore,
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    route_evidence: &crate::world::RouteEvidence,
    media_live: bool,
    staleness_from_rob_tick: bool,
    diag: &mut AssignDiag,
    dispatch: &crate::config::DispatchCfg,
    shipyard: &crate::config::ShipyardCfg,
    trophic: &crate::config::TrophicCfg,
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
        // Scripted stages skip gym-controlled craft (spec §5; craft are
        // config-minted dense, `slot == row`, so `craft_cfg[crow]` is the row's
        // init — the resolve_purchases `stations_cfg` precedent).
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        if tick.0 % stagger != crow as u64 % stagger {
            continue;
        }
        // PLAY-C1 (world-gets-big §5): dispatch eligibility requires a live
        // tank. Filter-at-choice, never claim-and-strand; a stranded craft
        // remains Idle with fuel <= eps as the recorded adrift end state.
        if ships.fuel_mass[crow] <= crate::events::FUEL_EMPTY_EPS {
            continue;
        }
        let capacity = cargo_capacity(&ships.spec[crow], ships.upgrades[crow], shipyard);
        // One pass, ascending dense row == ascending ContractId: with scoring
        // OFF the FIRST eligible row wins (lowest ContractId, the original
        // behavior); with scoring ON the strictly-greatest score wins, so ties
        // keep the lowest ContractId.
        let mut pick: Option<(usize, i64)> = None;
        // Counterfactual pick under the legacy ring (media-live only) — the
        // argmax-flip window. PURE READ: never feeds the actual assignment.
        let mut ring_pick: Option<(usize, i64)> = None;
        let mut scored = false;
        for kidx in 0..contracts.ids.len() {
            if contracts.status[kidx] != ContractStatus::Offered {
                continue;
            }
            // Capacity filter (pirates rung §6): scripted choice skips a lot it
            // cannot haul — filter-at-choice, never claim-and-revert (the
            // accept-settle gate in `resolve_contracts` backstops manual/RL paths).
            if contracts.qty[kidx] > capacity {
                continue;
            }
            let cid = contract_id(contracts, kidx);
            if (0..ships.ids.len()).any(|r| ships.contract[r] == Some(cid)) {
                continue;
            }
            if !trophic.hauler_belief_scoring {
                pick = Some((kidx, 0));
                break;
            }
            scored = true;
            // Evidence-scored pick (spec §7): the route's recent-rob count,
            // penalty milli per rob clamped at 900 — the score never hits
            // zero, so a hot route is avoided, not erased. Saturating integer
            // arithmetic throughout (spec §8). The count is the media rung's
            // swapped read (media spec §7): media-live -> the hauler's OWN
            // comms-log (per-reader `first_heard` window at the dispatch
            // tick; a missing buffer — e.g. a pirate row — reads 0); media
            // off -> the legacy ring through the hauler's dock-gated
            // `info_tick`, byte-identical. The ring count is ALWAYS computed:
            // it is the active count when media is off, and the diagnostics
            // counterfactual when media is live.
            let from = contracts.from_station[kidx];
            let to = contracts.to_station[kidx];
            let (count, ring_count) = stations
                .ids
                .dense_index(from.slot, from.generation)
                .zip(stations.ids.dense_index(to.slot, to.generation))
                .map_or((0, 0), |(f, t)| {
                    let route = f.saturating_mul(stations.ids.len()).saturating_add(t);
                    let ring = route_evidence.count_recent(
                        route,
                        ships.info_tick[crow],
                        trophic.evidence_window,
                    );
                    let active = if media_live {
                        ships.gossip[crow].as_ref().map_or(0, |buf| {
                            buf.count_route_recent(
                                route,
                                tick,
                                trophic.evidence_window,
                                staleness_from_rob_tick,
                            )
                        })
                    } else {
                        ring
                    };
                    (active, ring)
                });
            diag.candidate_counts[(count as usize).min(6)] += 1;
            let penalty =
                (count.saturating_mul(trophic.evidence_penalty_milli)).min(900) as i64;
            let score =
                contracts.reward_micros[kidx].saturating_mul(1000 - penalty) / 1000;
            if pick.is_none_or(|(_, best)| score > best) {
                pick = Some((kidx, score));
            }
            if media_live {
                let ring_penalty = (ring_count
                    .saturating_mul(trophic.evidence_penalty_milli))
                .min(900) as i64;
                let ring_score =
                    contracts.reward_micros[kidx].saturating_mul(1000 - ring_penalty) / 1000;
                if ring_pick.is_none_or(|(_, best)| ring_score > best) {
                    ring_pick = Some((kidx, ring_score));
                }
            }
        }
        if let Some((kidx, _)) = pick {
            ships.contract[crow] = Some(contract_id(contracts, kidx));
            ships.role[crow] = CraftRole::Hauler;
            if scored {
                diag.decisions += 1;
                if media_live && ring_pick.map(|(k, _)| k) != Some(kidx) {
                    diag.flips += 1;
                }
            }
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
///     corp treasury cannot cover the reward, OR the lot exceeds the hauler's
///     derived cargo capacity (pirates rung §6 gate), REVERT the assignment (clear
///     the craft's `contract`/`role`, leave the contract `Offered`, hauler `None`) —
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
    shipyard: &crate::config::ShipyardCfg,
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
                // CAPACITY GATE (pirates rung §6): a lot bigger than the hauler's
                // DERIVED cargo capacity (base + hulls * step) reverts the accept —
                // the underfunded-escrow precedent below. Scripted ASSIGN filters
                // the same way at choice time; this gate backstops the manual/RL
                // `AcceptContract` path.
                let capacity = cargo_capacity(&ships.spec[crow], ships.upgrades[crow], shipyard);
                // Escrow: debit the corp treasury by the reward. Insufficient
                // treasury (or a stale corp row) -> REVERT the assignment.
                let corp = contracts.corp[kidx];
                let reward = contracts.reward_micros[kidx];
                let corp_row = corporations.ids.dense_index(corp.slot, corp.generation);
                let funded = matches!(corp_row, Some(r) if corporations.treasury_micros[r] >= reward);
                if !funded || contracts.qty[kidx] > capacity {
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

/// Upgrade-purchase settle stage — stage 1d, pirates rung §6 (deterministic,
/// dense craft-row order). Runs AFTER `resolve_contracts`, PRE-physics, so
/// `ships.pos` is still the tick-(t-1) state and the vendor dock predicate
/// samples `body_pos(t-1)` — the same frame (the `try_load` precedent).
///
/// Consumes EVERY `pending_upgrade` intent THIS tick (the transient-column
/// invariant `state_hash` debug_asserts): settle iff the craft is within
/// `ARRIVAL_RADIUS` of a `sells_upgrades` station AND `credits >= price` AND
/// `level < cap` (structural, settle no-op at cap) → debit the buyer EXACTLY
/// the per-level catalog price, credit the Yard corp the same (a pure
/// transfer — zero new identity legs), bump the fleet-ledger count, emit
/// `UpgradePurchased`. Any failed check clears the intent as a deterministic
/// no-op: no event, no credit movement. All arithmetic saturating (spec §8
/// totality discipline).
///
/// `stations_cfg` is the config row set (`RunConfig.stations`): stations are
/// config-minted dense (`slot == row`, no despawn), so row `srow`'s vendor bit
/// is `stations_cfg[srow].sells_upgrades`.
#[allow(clippy::too_many_arguments)]
pub fn resolve_purchases(
    ships: &mut CraftStore,
    stations: &StationStore,
    stations_cfg: &[crate::config::StationInit],
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    shipyard: &crate::config::ShipyardCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    use crate::stores::UpgradeKind;
    let prev = Tick(tick.0.saturating_sub(1));
    for crow in 0..ships.ids.len() {
        let Some(kind) = ships.pending_upgrade[crow] else {
            continue;
        };
        // ALWAYS consume the intent this stage, settle or skip (the transient-
        // column invariant: `pending_upgrade` is None at every hash point).
        ships.pending_upgrade[crow] = None;
        if !docked_at_vendor(ships, crow, stations, stations_cfg, bodies, eph, prev) {
            continue;
        }
        // Per-arm catalog row: current count, structural cap, price ladder.
        let (level, cap, ladder) = match kind {
            UpgradeKind::Hull => (
                ships.upgrades[crow].hulls,
                shipyard.max_hulls,
                &shipyard.hull_price_micros,
            ),
            UpgradeKind::Escort => (
                ships.upgrades[crow].escorts,
                shipyard.max_escorts,
                &shipyard.escort_price_micros,
            ),
        };
        if level >= cap {
            continue;
        }
        // A cap configured beyond the priced ladder degrades to a deterministic
        // skip (spec §8: no unwraps on "impossible" states).
        let Some(&price) = ladder.get(level as usize) else {
            continue;
        };
        if ships.credits_micros[crow] < price {
            continue;
        }
        // The Yard (spec §6): the config-index corp receives every upgrade
        // payment (dense slot == row). A stale/out-of-range index is a
        // deterministic skip — never a one-legged debit.
        let yard_row = shipyard.corp_index as usize;
        if corporations.ids.id_at(yard_row).is_none() {
            continue;
        }
        // Pure transfer: buyer wallet -> Yard treasury (zero new identity legs).
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(price);
        corporations.treasury_micros[yard_row] =
            corporations.treasury_micros[yard_row].saturating_add(price);
        let new_level = level.saturating_add(1);
        match kind {
            UpgradeKind::Hull => ships.upgrades[crow].hulls = new_level,
            UpgradeKind::Escort => ships.upgrades[crow].escorts = new_level,
        }
        let craft = ships.ids_at(crow);
        events.emit(Event {
            tick,
            kind: EventKind::UpgradePurchased { craft, kind, level: new_level, price_micros: price },
        });
    }
}

/// Vendor dock predicate (pirates rung §6): within `ARRIVAL_RADIUS` of ANY
/// `sells_upgrades` station's body, compared in the craft's frame (`body_pos`
/// at `prev == t-1`; the try_load precedent). Shared by `resolve_purchases`
/// (the settle gate) and `run_purchase_policies` (the scripted intent writer),
/// so policy intent and same-tick settle agree on what "docked" means.
/// `stations_cfg` is the config row set (stations are config-minted dense,
/// `slot == row`, no despawn).
fn docked_at_vendor(
    ships: &CraftStore,
    crow: usize,
    stations: &StationStore,
    stations_cfg: &[crate::config::StationInit],
    bodies: &BodyStore,
    eph: &Ephemeris,
    prev: Tick,
) -> bool {
    (0..stations.ids.len()).any(|srow| {
        stations_cfg.get(srow).is_some_and(|s| s.sells_upgrades) && {
            let body = stations.body[srow];
            bodies
                .ids
                .dense_index(body.slot, body.generation)
                .is_some_and(|brow| {
                    let bpos = eph.body_pos(bodies.eph_index[brow], prev);
                    ships.pos[crow].sub(bpos).length() <= crate::autopilot::ARRIVAL_RADIUS
                })
        }
    })
}

/// Any-station dock predicate (world-gets-big §5): the first, lowest dense row
/// station whose body is within `ARRIVAL_RADIUS` of the craft, compared in the
/// craft's frame at `prev == t - 1`. Unlike `docked_at_vendor`, every dock can
/// sell propellant when its live Fuel price row is valid.
fn docked_station_row(
    ships: &CraftStore,
    crow: usize,
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    prev: Tick,
) -> Option<usize> {
    (0..stations.ids.len()).find(|&srow| {
        let body = stations.body[srow];
        bodies
            .ids
            .dense_index(body.slot, body.generation)
            .is_some_and(|brow| {
                let bpos = eph.body_pos(bodies.eph_index[brow], prev);
                ships.pos[crow].sub(bpos).length() <= crate::autopilot::ARRIVAL_RADIUS
            })
    })
}

/// Refuel settle stage — stage 1d2, world-gets-big §5. Consumes every
/// `pending_refuel` intent this tick, then gates deterministically. Integer lots
/// decide before writes: `need = floor((cap_eff - fuel)/lot)`, `afford =
/// credits/price`, and `units = min(need, stock, afford)`. Settled lots perform
/// four legs: station stock decreases, `consumed[Fuel]` increases, wallet moves
/// to the Port corporation treasury, and the craft tank receives one clamped
/// write. Tank fill permilles go through the shared FLOOR seam.
#[allow(clippy::too_many_arguments)]
pub fn resolve_refuels(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    counters: &mut EconCounters,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    if refuel.lot_mass <= 0.0 {
        for intent in ships.pending_refuel.iter_mut() {
            *intent = None;
        }
        return;
    }

    let lot = refuel.lot_mass;
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Good::FUEL.index();
    for crow in 0..ships.ids.len() {
        if ships.pending_refuel[crow].is_none() {
            continue;
        }
        ships.pending_refuel[crow] = None;

        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };

        // Resolve craft and station identities early so RefuelDenied can be emitted
        // at each guard site (A0 instrument, WB4 middle beat).
        let craft = ships.ids_at(crow);
        let station_id = stations
            .ids
            .id_at(srow)
            .map(|(slot, generation)| StationId { slot, generation });

        let unit_price = stations.price_micros[srow][fuel_r];
        if unit_price < 1 {
            // price<1 guard: no deny event (the station is effectively not selling;
            // this is a config state, not a craft-level denial to narrate).
            continue;
        }
        let stock = stations.stock[srow][fuel_r];
        if stock <= 0 {
            if let Some(station) = station_id {
                events.emit(Event {
                    tick,
                    kind: EventKind::RefuelDenied {
                        craft,
                        station,
                        reason: RefuelDeniedReason::NoStock,
                    },
                });
            }
            continue;
        }
        let port_row = refuel.corp_index as usize;
        if corporations.ids.id_at(port_row).is_none() {
            continue;
        }

        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        let cap_eff = eff.fuel_capacity;
        let fuel = ships.fuel_mass[crow];
        let need = ((cap_eff - fuel) / lot).floor() as i64;
        if need < 1 {
            if let Some(station) = station_id {
                events.emit(Event {
                    tick,
                    kind: EventKind::RefuelDenied {
                        craft,
                        station,
                        reason: RefuelDeniedReason::TankFull,
                    },
                });
            }
            continue;
        }
        let afford = ships.credits_micros[crow].max(0) / unit_price;
        if afford < 1 {
            if let Some(station) = station_id {
                events.emit(Event {
                    tick,
                    kind: EventKind::RefuelDenied {
                        craft,
                        station,
                        reason: RefuelDeniedReason::CannotAfford,
                    },
                });
            }
            continue;
        }
        let units = need.min(stock).min(afford);
        let cost = units.saturating_mul(unit_price);
        let tank_before_permille = permille_floor(fuel, cap_eff);

        stations.stock[srow][fuel_r] -= units;
        counters.consumed[fuel_r] = counters.consumed[fuel_r].saturating_add(units);
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(cost);
        corporations.treasury_micros[port_row] =
            corporations.treasury_micros[port_row].saturating_add(cost);
        ships.fuel_mass[crow] = (fuel + units as f64 * lot).min(cap_eff);
        let tank_after_permille = permille_floor(ships.fuel_mass[crow], cap_eff);
        // FUEL-C1: a refueled craft already Seeking may have been dispatched
        // with an exhausted budget. Re-derive from the just-written tank using
        // the same Tsiolkovsky source as dispatch/ingest so the same tick's
        // physics stage can burn instead of coasting forever at dv_remaining 0.
        if let NavState::Seeking { dest, .. } = ships.nav[crow] {
            let dv = crate::math::tsiolkovsky_dv(
                eff.exhaust_velocity,
                eff.dry_mass,
                ships.fuel_mass[crow],
            );
            ships.nav[crow] = NavState::Seeking { dest, dv_remaining: dv };
        }
        if let Some(station) = station_id {
            events.emit(Event {
                tick,
                kind: EventKind::Refueled {
                    craft,
                    station,
                    units,
                    price_micros: unit_price,
                    tank_before_permille,
                    tank_after_permille,
                },
            });
        }
    }
}

/// The next rung of the scripted hauler purchase ladder (spec §6): the FIRST
/// un-met `(kind, target_level)` in policy order, skipping rungs at/above the
/// structural cap. `EscortFirst`: Escort L1 -> Hull L1 -> Escort L2 -> Hull L2;
/// `HullFirst` swaps the pairs. `None` when the ladder is complete (or
/// `BuyPolicy::Off`).
fn next_ladder_rung(
    upgrades: crate::stores::UpgradeLevels,
    policy: crate::config::BuyPolicy,
    shipyard: &crate::config::ShipyardCfg,
) -> Option<crate::stores::UpgradeKind> {
    use crate::config::BuyPolicy;
    use crate::stores::UpgradeKind;
    let ladder: [(UpgradeKind, u8); 4] = match policy {
        BuyPolicy::Off => return None,
        BuyPolicy::EscortFirst => [
            (UpgradeKind::Escort, 1),
            (UpgradeKind::Hull, 1),
            (UpgradeKind::Escort, 2),
            (UpgradeKind::Hull, 2),
        ],
        BuyPolicy::HullFirst => [
            (UpgradeKind::Hull, 1),
            (UpgradeKind::Escort, 1),
            (UpgradeKind::Hull, 2),
            (UpgradeKind::Escort, 2),
        ],
    };
    for (kind, level) in ladder {
        let (current, cap) = match kind {
            UpgradeKind::Hull => (upgrades.hulls, shipyard.max_hulls),
            UpgradeKind::Escort => (upgrades.escorts, shipyard.max_escorts),
        };
        if current < level && level <= cap {
            return Some(kind);
        }
    }
    None
}

/// Scripted purchase policies — stage 1c3 (pirates rung §6): write
/// `pending_upgrade` INTENT only (the ASSIGN precedent — the same transient
/// column the `BuyUpgrade` ingest arm writes), consumed by `resolve_purchases`
/// (stage 1d) the SAME tick, so the column stays `None` at every hash point.
/// Deterministic, dense craft-row order, desynchronized BY CONSTRUCTION (no
/// taste scalars — timing varies per craft through wealth and docking history):
///
/// * **Hauler** (role Idle, no contract, docked at a vendor): the next
///   `BuyPolicy` ladder rung, gated by the working-capital headroom
///   `credits >= price * buy_headroom_milli / 1000`.
/// * **Pirate** (lying low AT the hideout body): Escort to cap at full price —
///   pirates shop while hiding, which phase-lags the pirate ladder behind the
///   hauler ladder (the settle still requires a vendor at the dock; a
///   vendor-less hideout is a deterministic 1d no-op).
///
/// Scripted stage: skips `!scripted` craft; never clobbers an already-written
/// (ingest) intent. Inert by default: `BuyPolicy::Off` disables the hauler arm
/// and the spec-§8 lever (`engage_radius_au <= 0`) the pirate arm.
#[allow(clippy::too_many_arguments)]
pub fn run_purchase_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    stations_cfg: &[crate::config::StationInit],
    bodies: &BodyStore,
    eph: &Ephemeris,
    trophic: &crate::config::TrophicCfg,
    shipyard: &crate::config::ShipyardCfg,
    tick: Tick,
) {
    use crate::config::BuyPolicy;
    use crate::stores::UpgradeKind;
    let hauler_arm = trophic.hauler_buy_policy != BuyPolicy::Off;
    let pirate_arm = trophic.engage_radius_au > 0.0;
    if !hauler_arm && !pirate_arm {
        return;
    }
    let prev = Tick(tick.0.saturating_sub(1));
    for crow in 0..ships.ids.len() {
        // Scripted stages skip gym-controlled craft (spec §5).
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        // Never clobber an intent already written by this tick's ingest.
        if ships.pending_upgrade[crow].is_some() {
            continue;
        }
        match ships.role[crow] {
            CraftRole::Pirate if pirate_arm => {
                let Some(p) = ships.pirate[crow] else {
                    continue;
                };
                if tick >= p.lie_low_until {
                    continue; // shops only WHILE HIDING
                }
                let hrow = trophic.hideout_body_index as usize;
                if bodies.ids.id_at(hrow).is_none() {
                    continue;
                }
                let hpos = eph.body_pos(bodies.eph_index[hrow], prev);
                if ships.pos[crow].sub(hpos).length() > crate::autopilot::ARRIVAL_RADIUS {
                    continue; // not at the hideout yet
                }
                let level = ships.upgrades[crow].escorts;
                if level >= shipyard.max_escorts {
                    continue;
                }
                let Some(&price) = shipyard.escort_price_micros.get(level as usize) else {
                    continue;
                };
                if ships.credits_micros[crow] < price {
                    continue; // full price, NO headroom (ransom money is all working capital)
                }
                ships.pending_upgrade[crow] = Some(UpgradeKind::Escort);
            }
            CraftRole::Idle if hauler_arm => {
                if ships.contract[crow].is_some() {
                    continue; // intent-bound: not idle for a strategic decision
                }
                if !docked_at_vendor(ships, crow, stations, stations_cfg, bodies, eph, prev) {
                    continue;
                }
                let Some(kind) = next_ladder_rung(
                    ships.upgrades[crow],
                    trophic.hauler_buy_policy,
                    shipyard,
                ) else {
                    continue;
                };
                let (level, ladder) = match kind {
                    UpgradeKind::Hull => (ships.upgrades[crow].hulls, &shipyard.hull_price_micros),
                    UpgradeKind::Escort => {
                        (ships.upgrades[crow].escorts, &shipyard.escort_price_micros)
                    }
                };
                let Some(&price) = ladder.get(level as usize) else {
                    continue;
                };
                // Working-capital headroom (spec §6): buy only when the wallet
                // clears price * headroom — purchase timing then varies per
                // hauler through earned wealth, never a taste scalar.
                let need = price.saturating_mul(shipyard.buy_headroom_milli as i64) / 1000;
                if ships.credits_micros[crow] < need {
                    continue;
                }
                ships.pending_upgrade[crow] = Some(kind);
            }
            _ => {}
        }
    }
}

/// Scripted refuel-intent stage — stage 1c3b, world-gets-big §5. Writes
/// `pending_refuel` for scripted non-pirate craft docked at any station with
/// at least one lot of headroom and a wallet covering one unit at the dock's
/// live Fuel price. Top-to-full, threshold-free; the 1d2 settler buys
/// `min(need, stock, afford)` lots. Never clobbers an ingest intent.
#[allow(clippy::too_many_arguments)]
pub fn run_refuel_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
) {
    if refuel.lot_mass <= 0.0 {
        return;
    }
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Good::FUEL.index();
    for crow in 0..ships.ids.len() {
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        if ships.pending_refuel[crow].is_some() {
            continue;
        }
        if ships.role[crow] == CraftRole::Pirate {
            continue;
        }
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };
        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        let need = ((eff.fuel_capacity - ships.fuel_mass[crow]) / refuel.lot_mass).floor();
        if need < 1.0 {
            continue;
        }
        if ships.credits_micros[crow] < stations.price_micros[srow][fuel_r] {
            continue;
        }
        ships.pending_refuel[crow] = Some(());
    }
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
/// each escrow-holding non-terminal contract (`Accepted` — the deadhead leg,
/// `CargoLoaded` — the one-tick load window, `InTransit`; jumpgate-2c0c2d92bb) whose
/// bound `hauler` ran out of propellant this tick:
///
///   * status `-> Failed`,
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
    tick: Tick,
    events: &mut EventStream,
) {
    for kidx in 0..contracts.ids.len() {
        // jumpgate-2c0c2d92bb: ALL THREE escrow-holding non-terminal statuses fail
        // on FuelEmpty — `Accepted` (the deadhead leg to the pickup), `CargoLoaded`
        // (the one-tick load window), and `InTransit`. Filtering to `InTransit`
        // alone locked a deadhead-stranded hauler's escrow forever.
        if !matches!(
            contracts.status[kidx],
            ContractStatus::Accepted | ContractStatus::CargoLoaded | ContractStatus::InTransit
        ) {
            continue;
        }
        let Some(hauler) = contracts.hauler[kidx] else {
            continue;
        };
        // Did this contract's hauler run out of propellant THIS tick?
        if !failed_craft.contains(&hauler) {
            continue;
        }
        // ContractFailed (FuelEmpty-cause) is emitted inside the settle body (§7).
        settle_contract_failure(
            contracts,
            corporations,
            ships,
            counters,
            kidx,
            FailureCause::FuelEmpty,
            tick,
            events,
        );
    }
}

/// Why a contract-failure settlement fired (pirates rung Task 4: the
/// `resolve_failures` settle body generalized to take a cause). The legs are
/// cause-INDEPENDENT — the cause names the caller's stage and pins which
/// source statuses are legal there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FailureCause {
    /// Propellant exhaustion (stage 3c): every escrow-holding non-terminal status
    /// can fail — `Accepted` (the deadhead leg), `CargoLoaded` (the one-tick load
    /// window), and `InTransit` (jumpgate-2c0c2d92bb).
    FuelEmpty,
    /// Robbery (stage 3b2, spec §3): both escrow-holding laden statuses
    /// (`CargoLoaded` — the one-tick load window — and `InTransit`) are robbable.
    Robbed,
}

/// One contract's failure settlement — the SHARED settle body of stage 3c
/// (`resolve_failures`, FuelEmpty) and stage 3b2 (the robbery settle, spec §3):
///
///   * refund `escrow_micros` -> the owning corp treasury (a credit TRANSFER:
///     Σtreasury+Σcredits+Σescrow invariant; a stale corp row skips the refund
///     leg but still fails the contract — the escrow stays put so the identity
///     holds),
///   * account the loaded cargo as a SINK leg (`consumed[r] += qty`, the
///     resource-identity leg) and clear it,
///   * release the hauler (`contract`/`role` cleared, `Hauler -> Idle` — the
///     robbed-hauler exit the no-abandonment gap otherwise denies),
///   * status -> `Failed`.
///
/// The ransom leg of a robbery is NOT here — it is pirate-side state, settled
/// by the 3b2 caller. Saturating arithmetic per the spec §8 totality discipline.
#[allow(clippy::too_many_arguments)]
pub(crate) fn settle_contract_failure(
    contracts: &mut ContractStore,
    corporations: &mut CorporationStore,
    ships: &mut CraftStore,
    counters: &mut EconCounters,
    kidx: usize,
    cause: FailureCause,
    tick: Tick,
    events: &mut EventStream,
) {
    debug_assert!(
        match cause {
            // jumpgate-2c0c2d92bb: FuelEmpty can fire on the deadhead leg
            // (`Accepted`), in the one-tick load window (`CargoLoaded`), or
            // mid-haul (`InTransit`) — all three escrow-holding non-terminal
            // statuses are legal sources here.
            FailureCause::FuelEmpty => matches!(
                contracts.status[kidx],
                ContractStatus::Accepted
                    | ContractStatus::CargoLoaded
                    | ContractStatus::InTransit
            ),
            FailureCause::Robbed => matches!(
                contracts.status[kidx],
                ContractStatus::CargoLoaded | ContractStatus::InTransit
            ),
        },
        "settle_contract_failure: source status inconsistent with the cause"
    );
    // Refund escrow -> owning corp treasury (credit TRANSFER; identity
    // invariant). Capture the actual refunded amount: a stale corp row skips
    // the refund and leaves escrow in place, so the narrated refund is 0.
    let corp = contracts.corp[kidx];
    let mut escrow_refunded_micros: i64 = 0;
    if let Some(corp_row) = corporations.ids.dense_index(corp.slot, corp.generation) {
        escrow_refunded_micros = contracts.escrow_micros[kidx];
        corporations.treasury_micros[corp_row] =
            corporations.treasury_micros[corp_row].saturating_add(contracts.escrow_micros[kidx]);
        contracts.escrow_micros[kidx] = 0;
    }
    // Cargo loss: account the lost cargo as a SINK leg, then release the hauler.
    let mut cargo_lost: u32 = 0;
    if let Some(hauler) = contracts.hauler[kidx]
        && let Some(crow) = ships.index_of(hauler)
    {
        if let Some((resource, qty)) = ships.cargo[crow] {
            counters.consumed[resource.index()] =
                counters.consumed[resource.index()].saturating_add(qty as i64);
            ships.cargo[crow] = None;
            cargo_lost = qty;
        }
        ships.contract[crow] = None;
        ships.role[crow] = CraftRole::Idle;
    }
    contracts.status[kidx] = ContractStatus::Failed;
    if matches!(cause, FailureCause::FuelEmpty)
        && let Some(hauler) = contracts.hauler[kidx]
    {
        events.emit(Event {
            tick,
            kind: EventKind::ContractFailed {
                contract: contract_id(contracts, kidx),
                hauler,
                cause,
                escrow_refunded_micros,
                cargo_lost,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::EventKind;
    use crate::events::EventStream;
    use crate::ids::{BodyId, CorporationId};

    // NEW test — A1 pinned-index contract.  Enum exhaustiveness is gone;
    // this test is the load-bearing substitute that fires if ORE/FUEL are
    // declared at the wrong index or GoodsCfg boot order changes.
    #[test]
    fn good_ore_and_fuel_pinned_indices() {
        // ORE must be index 0, FUEL must be index 1.  These are the canonical
        // dense order used by every per-resource array and by the state hash.
        // If either constant moves, every existing trophic/frontier golden
        // diverges and the cross-branch digest proof fails.
        assert_eq!(Good::ORE.index(), 0, "ORE must be index 0");
        assert_eq!(Good::FUEL.index(), 1, "FUEL must be index 1");
        // Good(u16) must implement Copy, Clone, Debug, PartialEq, Eq — required
        // by Recipe/ContractStore/EventKind which derive these.
        fn needs_copy<T: Copy>() {}
        fn needs_eq<T: Eq>() {}
        needs_copy::<Good>();
        needs_eq::<Good>();
    }

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
            Recipe { input: None, output: Some((Good::ORE, 5)), interval: 1 },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);

        assert_eq!(stations.stock[0][Good::ORE.index()], 5, "ore stock rose by 5");
        assert_eq!(counters.mined[Good::ORE.index()], 5, "mined[Ore]==5");
        assert_eq!(counters.consumed[Good::ORE.index()], 0);
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
            EventKind::Production { resource: Good::ORE, qty: 5, .. }
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
                input: Some((Good::ORE, 3)),
                output: Some((Good::FUEL, 2)),
                interval: 1,
            },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);

        assert_eq!(stations.stock[0][Good::ORE.index()], 7, "ore -3");
        assert_eq!(stations.stock[0][Good::FUEL.index()], 2, "fuel +2");
        assert_eq!(counters.consumed[Good::ORE.index()], 3, "consumed[Ore]==3");
        assert_eq!(counters.mined[Good::FUEL.index()], 2, "mined[Fuel]==2");
        assert_eq!(counters.mined[Good::ORE.index()], 0);
        assert_eq!(counters.consumed[Good::FUEL.index()], 0);
        // Production event names the OUTPUT leg.
        let prod: Vec<_> = events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Production { .. }))
            .collect();
        assert_eq!(prod.len(), 1);
        assert!(matches!(
            prod[0].kind,
            EventKind::Production { resource: Good::FUEL, qty: 2, .. }
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
                input: Some((Good::ORE, 5)),
                output: Some((Good::FUEL, 2)),
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
            Recipe { input: None, output: Some((Good::ORE, 1)), interval: 3 },
        );
        let mut counters = EconCounters::zero();
        let mut events = EventStream::new();

        run_producers(&mut stations, &mut producers, &mut counters, Tick(1), &mut events);
        run_producers(&mut stations, &mut producers, &mut counters, Tick(2), &mut events);
        assert_eq!(stations.stock[0][Good::ORE.index()], 0, "not yet (interval gate)");

        run_producers(&mut stations, &mut producers, &mut counters, Tick(3), &mut events);
        assert_eq!(stations.stock[0][Good::ORE.index()], 1, "fires at tick 3");
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

        let fi = Good::FUEL.index();
        let oi = Good::ORE.index();
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
            EventKind::PriceUpdate { resource: Good::FUEL, .. }
        )));
    }

    #[test]
    fn resource_index_is_stable_and_dense() {
        assert_eq!(Good::ORE.index(), 0);
        assert_eq!(Good::FUEL.index(), 1);
        assert_eq!(Good::ALL_V1.len(), N_RESOURCES);
        for (i, r) in Good::ALL_V1.iter().enumerate() {
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

    // ---- Pirates rung Commit C: purchase verb + capacity gate ------------------
    //
    // Per-arm EXACT-INTEGER assertions are the point: the credit identity
    // (Σtreasury+Σcredits+Σescrow) stays green under a wrong-price bug, so every
    // settle arm pins its catalog literal (5/12/8/20 M micros) independently of
    // the ShipyardCfg code under test.

    /// Vendor fixture: one near-massless central body hosting a station (vendor
    /// bit per arg), one craft docked at it (pos == body pos == origin), one Yard
    /// corp at `ShipyardCfg::default().corp_index == 0` with an EMPTY treasury so
    /// every credited micro is purchase money. Wallet/upgrade columns are mutated
    /// per-test (CraftInit has no credits field; tests write the live store).
    fn vendor_world_fixture(sells_upgrades: bool) -> crate::config::RunConfig {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, GuidanceParams, OrbitalElements,
            RunConfig, StationInit, SubstepCfg,
        };
        use crate::math::Vec3;
        use crate::time::Dt;
        RunConfig {
            master_seed: 7,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1e-9, // near-massless: the docked craft is not gravity-trapped
                elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-9,
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2,
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::ZERO, // docked at the vendor body
                vel: Vec3::ZERO,
                fuel_mass: 1e-9,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![StationInit {
                body_index: 0,
                initial_stock: [0, 0],
                initial_price_micros: [0, 0],
                sells_upgrades,
            }],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 0, home_station_index: 0 }],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
            trophic: crate::config::TrophicCfg::default(),
            shipyard: crate::config::ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: crate::config::RefuelCfg::default(),
        }
    }

    fn buy_cmd(craft: crate::ids::CraftId, kind: crate::stores::UpgradeKind) -> crate::contract::Command {
        use crate::types::{EntityRef, Target};
        crate::contract::Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::BuyUpgrade { kind },
        }
    }

    /// Skip-arm postcondition: zero credit movement anywhere, no level change,
    /// intent consumed, NO UpgradePurchased event.
    fn assert_purchase_skipped(
        world: &mut crate::world::World,
        credits_before: i64,
        upgrades_before: crate::stores::UpgradeLevels,
        arm: &str,
    ) {
        assert_eq!(world.ships.upgrades[0], upgrades_before, "{arm}: no level change");
        assert_eq!(world.ships.credits_micros[0], credits_before, "{arm}: zero wallet movement");
        assert_eq!(world.corporations.treasury_micros[0], 0, "{arm}: Yard treasury untouched");
        assert_eq!(world.ships.pending_upgrade[0], None, "{arm}: intent cleared");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::UpgradePurchased { .. })),
            "{arm}: NO UpgradePurchased event"
        );
    }

    #[test]
    fn purchase_settles_at_vendor() {
        use crate::stores::UpgradeKind;
        use crate::world::World;
        let (mut world, _h) = World::reset(vendor_world_fixture(true)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;

        // Arm: Escort L1 — debit EXACTLY escort_price_micros[0] == 5_000_000.
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_eq!(world.ships.upgrades[0].escorts, 1, "escort count bumped to 1");
        assert_eq!(world.ships.upgrades[0].hulls, 0, "hull count untouched");
        assert_eq!(world.ships.credits_micros[0], 45_000_000, "debited EXACTLY 5_000_000");
        assert_eq!(world.corporations.treasury_micros[0], 5_000_000, "Yard credited the same");
        assert_eq!(world.ships.pending_upgrade[0], None, "intent consumed");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::UpgradePurchased {
                    craft: c,
                    kind: UpgradeKind::Escort,
                    level: 1,
                    price_micros: 5_000_000,
                } if c == craft
            )),
            "UpgradePurchased emitted with exact payload"
        );

        // Arm: Escort L2 — EXACTLY escort_price_micros[1] == 12_000_000.
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_eq!(world.ships.upgrades[0].escorts, 2);
        assert_eq!(world.ships.credits_micros[0], 33_000_000, "debited EXACTLY 12_000_000");
        assert_eq!(world.corporations.treasury_micros[0], 17_000_000);

        // Arm: Hull L1 — EXACTLY hull_price_micros[0] == 8_000_000.
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Hull)]);
        assert_eq!(world.ships.upgrades[0].hulls, 1);
        assert_eq!(world.ships.credits_micros[0], 25_000_000, "debited EXACTLY 8_000_000");
        assert_eq!(world.corporations.treasury_micros[0], 25_000_000);

        // Arm: Hull L2 — EXACTLY hull_price_micros[1] == 20_000_000.
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Hull)]);
        assert_eq!(world.ships.upgrades[0].hulls, 2);
        assert_eq!(world.ships.credits_micros[0], 5_000_000, "debited EXACTLY 20_000_000");
        assert_eq!(world.corporations.treasury_micros[0], 45_000_000);
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::UpgradePurchased {
                    kind: UpgradeKind::Hull,
                    level: 2,
                    price_micros: 20_000_000,
                    ..
                }
            )),
            "Hull L2 event carries the exact L2 price"
        );
    }

    #[test]
    fn purchase_skips_deterministically() {
        use crate::math::Vec3;
        use crate::stores::{UpgradeKind, UpgradeLevels};
        use crate::world::World;

        // (a) NOT DOCKED: ~10_000x ARRIVAL_RADIUS from the vendor body.
        let (mut world, _h) = World::reset(vendor_world_fixture(true)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;
        world.ships.pos[0] = Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_purchase_skipped(&mut world, 50_000_000, UpgradeLevels::default(), "not-docked");

        // (b) UNDERFUNDED: docked, but one micro short of the Escort L1 price.
        let (mut world, _h) = World::reset(vendor_world_fixture(true)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 4_999_999;
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_purchase_skipped(&mut world, 4_999_999, UpgradeLevels::default(), "underfunded");

        // (c) AT CAP: escorts already at max_escorts == 2 (structural, spec §6).
        let (mut world, _h) = World::reset(vendor_world_fixture(true)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;
        world.ships.upgrades[0].escorts = 2;
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_purchase_skipped(
            &mut world,
            50_000_000,
            UpgradeLevels { hulls: 0, escorts: 2 },
            "at-cap",
        );

        // (d) NON-VENDOR: docked + funded, but the station does not sell upgrades.
        let (mut world, _h) = World::reset(vendor_world_fixture(false)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;
        world.step(&mut vec![buy_cmd(craft, UpgradeKind::Escort)]);
        assert_purchase_skipped(&mut world, 50_000_000, UpgradeLevels::default(), "non-vendor");
    }

    /// Refuel fixture: the vendor fixture's one-body/one-station/one-craft dock
    /// with the Fuel price surface live, station Fuel stock 40, and
    /// `RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 }`. The reprice clock is
    /// off, so the seeded 5_000 micros is the exact settle price.
    fn refuel_world_fixture() -> crate::config::RunConfig {
        let mut cfg = vendor_world_fixture(false);
        cfg.craft[0].fuel_mass = 0.0;
        cfg.stations[0].initial_stock = [0, 40];
        cfg.stations[0].initial_price_micros = [0, 5_000];
        cfg.price_cfg = crate::config::PriceCfg {
            base_micros: [0, 5_000],
            cap: [0, 40],
            slope_milli: 1800,
            reprice_interval: 0,
        };
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 };
        cfg
    }

    #[test]
    fn refuel_settles_quantized_with_four_legs_and_exact_event() {
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        // need = floor((1e-9 - 0)/2.5e-10) = 4; afford = 12_000/5_000 = 2.
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());

        let f = Good::FUEL.index();
        assert_eq!(world.stations.stock[0][f], 38, "stock leg: -= units");
        assert_eq!(world.econ.consumed[f], 2, "sink leg: consumed[Fuel] += units");
        assert_eq!(world.ships.credits_micros[0], 2_000, "wallet leg: debited units*price");
        assert_eq!(world.corporations.treasury_micros[0], 10_000, "Port treasury credited");
        assert_eq!(world.ships.fuel_mass[0], 5.0e-10, "tank leg: fuel += units*lot");
        assert_eq!(world.ships.pending_refuel[0], None, "intent consumed");
        let stock_now: i64 = world.stations.stock.iter().map(|s| s[f]).sum();
        assert_eq!(
            stock_now,
            40 + world.econ.mined[f] - world.econ.consumed[f],
            "resource identity holds"
        );
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    craft: c,
                    units: 2,
                    price_micros: 5_000,
                    tank_before_permille: 0,
                    tank_after_permille: 500,
                    ..
                } if c == craft
            )),
            "Refueled emitted with the exact quantized payload"
        );
        let _ = crate::hash::state_hash(&world);
    }

    #[test]
    fn refuel_tank_permille_is_floor_rounded() {
        use crate::world::World;
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].fuel_mass = 5.555e-10;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 5_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    units: 1,
                    tank_before_permille: 555,
                    tank_after_permille: 805,
                    ..
                }
            )),
            "tank permilles are FLOOR-rounded against effective capacity"
        );
    }

    /// Skip-arm postcondition: zero movement on every leg, intent consumed,
    /// NO Refueled event.
    fn assert_refuel_skipped(
        world: &mut crate::world::World,
        credits_before: i64,
        fuel_before: f64,
        stock_before: i64,
        arm: &str,
    ) {
        let f = Good::FUEL.index();
        assert_eq!(world.ships.fuel_mass[0], fuel_before, "{arm}: tank untouched");
        assert_eq!(world.ships.credits_micros[0], credits_before, "{arm}: zero wallet movement");
        assert_eq!(world.stations.stock[0][f], stock_before, "{arm}: stock untouched");
        assert_eq!(world.econ.consumed[f], 0, "{arm}: no sink leg");
        assert_eq!(world.corporations.treasury_micros[0], 0, "{arm}: Port treasury untouched");
        assert_eq!(world.ships.pending_refuel[0], None, "{arm}: intent consumed");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "{arm}: NO Refueled event"
        );
    }

    #[test]
    fn refuel_skips_deterministically() {
        use crate::math::Vec3;
        use crate::world::World;
        let f = Good::FUEL.index();

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "undocked");

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.stock[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 0, "stock-0");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::RefuelDenied { reason: RefuelDeniedReason::NoStock, .. }
            )),
            "stock-0: RefuelDenied(NoStock) must be emitted"
        );

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 4_999, 0.0, 40, "wallet-short");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::RefuelDenied { reason: RefuelDeniedReason::CannotAfford, .. }
            )),
            "wallet-short: RefuelDenied(CannotAfford) must be emitted"
        );

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 1.0e-9, 40, "tank-full");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::RefuelDenied { reason: RefuelDeniedReason::TankFull, .. }
            )),
            "tank-full: RefuelDenied(TankFull) must be emitted"
        );

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.price_micros[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "price-0");

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        assert!(
            world.corporations.ids.remove(0, 0).is_some(),
            "test setup invalidates the live Port corp row after a valid reset"
        );
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "stale-corp");
    }

    #[test]
    fn credit_identity_holds_across_refuels_and_policy_is_self_running() {
        use crate::contract::Command;
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        let total = |w: &crate::world::World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        let t0 = total(&world);
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..50 {
            world.step(&mut empty);
            assert_eq!(total(&world), t0, "Σtreasury+Σcredits+Σescrow invariant every tick");
        }
        assert!(
            world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { units: 4, .. })),
            "policy-driven top-to-full refuel happened (4 lots, dry -> full)"
        );
        assert_eq!(world.ships.fuel_mass[0], 1.0e-9, "topped to capacity: 4 * 2.5e-10");
        assert_eq!(world.ships.credits_micros[0], 1_000_000 - 20_000, "4 units at 5_000");
    }

    #[test]
    fn refuel_policy_gates_deterministically() {
        use crate::world::World;
        let no_refuel = |world: &mut crate::world::World, arm: &str| {
            assert!(
                !world
                    .events_mut()
                    .since(Tick(0))
                    .iter()
                    .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
                "{arm}: the policy must not have produced a refuel"
            );
        };

        let mut cfg = refuel_world_fixture();
        cfg.craft[0].scripted = false;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "!scripted");

        let mut cfg = refuel_world_fixture();
        cfg.craft[0].role = crate::stores::CraftRole::Pirate;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "pirate");

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "full-tank");

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "wallet-short");

        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = crate::math::Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut Vec::new());
        no_refuel(&mut world, "undocked");
    }

    #[test]
    fn refuel_rederives_dv_for_seeking_craft_fuel_c1() {
        use crate::types::NavDest;
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.ships.nav[0] = NavState::Seeking {
            dest: NavDest::Position(crate::math::Vec3::new(0.5, 0.0, 0.0)),
            dv_remaining: 0.0,
        };
        world.step(&mut Vec::new());

        let refilled: f64 = (0.0 + 4.0 * 2.5e-10_f64).min(1.0e-9);
        let dv_applied: f64 = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .find_map(|e| match e.kind {
                EventKind::ThrustApplied { dv, .. } => Some(dv),
                _ => None,
            })
            .expect("the re-derived budget unlocked a same-tick burn");
        assert!(
            world.ships.fuel_mass[0] < refilled,
            "the same-tick burn drew from the refilled tank"
        );
        let dv_full = crate::math::tsiolkovsky_dv(1e-2, 1e-9, refilled);
        match world.ships.nav[0] {
            NavState::Seeking { dv_remaining, .. } => {
                assert_eq!(
                    dv_remaining,
                    dv_full - dv_applied,
                    "budget re-derived from the refilled tank, then burned once"
                );
                assert!(dv_remaining > 0.0, "no more coast-at-zero with a full tank");
            }
            other => panic!("expected Seeking, got {other:?}"),
        }
    }

    #[test]
    fn refuel_default_is_inert_and_consumes_stray_intents() {
        use crate::world::World;
        let (mut world, _h) = World::reset(vendor_world_fixture(false)).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_eq!(world.ships.pending_refuel[0], None, "stray intent consumed on lot-0 world");
        assert_eq!(world.ships.fuel_mass[0], 1e-9, "tank untouched");
        assert_eq!(world.ships.credits_micros[0], 1_000_000, "wallet untouched");
        assert_eq!(world.corporations.treasury_micros[0], 0, "no treasury movement");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "no Refueled event on a lot-0 world"
        );
        let _ = crate::hash::state_hash(&world);
    }

    /// Capacity-gate fixture: two bodies (origin star hosts station A, a 0.3 AU
    /// body hosts station B), one craft docked at A, one funded corp, ONE seeded
    /// qty-10 Fuel contract A->B. `stagger_period == 0` keeps scripted ASSIGN off
    /// (this test drives the manual AcceptContract path the gate backstops).
    fn capacity_world_fixture() -> crate::config::RunConfig {
        use crate::config::{BodyInit, ContractInit, CorporationInit, OrbitalElements, StationInit};
        let mut cfg = vendor_world_fixture(false);
        cfg.bodies.push(BodyInit {
            mass: 1e-12,
            elements: OrbitalElements { a: 0.3, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
        });
        cfg.stations = vec![
            StationInit {
                body_index: 0,
                initial_stock: [0, 20], // covers the qty-10 Fuel load
                initial_price_micros: [0, 0],
                sells_upgrades: false,
            },
            StationInit {
                body_index: 1,
                initial_stock: [0, 0],
                initial_price_micros: [0, 0],
                sells_upgrades: false,
            },
        ];
        cfg.corporations =
            vec![CorporationInit { treasury_micros: 5_000_000, home_station_index: 0 }];
        cfg.contracts = vec![ContractInit {
            corp_index: 0,
            resource: Good::FUEL,
            qty: 10,
            from_station_index: 0,
            to_station_index: 1,
            reward_micros: 1_000_000,
        }];
        cfg.dispatch_cfg.stagger_period = 0;
        cfg
    }

    #[test]
    fn capacity_gate_reverts_oversized_accept() {
        use crate::stores::CraftRole;
        use crate::types::{EntityRef, Target};
        use crate::world::World;
        let (mut world, _h) = World::reset(capacity_world_fixture()).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let contract = world
            .contracts
            .ids
            .id_at(0)
            .map(|(slot, generation)| ContractId { slot, generation })
            .unwrap();
        let accept = crate::contract::Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract },
        };

        // qty 10 > capacity 5 (base, hulls 0): the accept-settle REVERTS — the
        // underfunded-escrow precedent (craft released, offer left open, zero
        // credit movement).
        world.step(&mut vec![accept]);
        assert_eq!(world.contracts.status[0], ContractStatus::Offered, "contract stays Offered");
        assert_eq!(world.contracts.hauler[0], None, "no hauler bound");
        assert_eq!(world.contracts.escrow_micros[0], 0, "no escrow movement");
        assert_eq!(world.corporations.treasury_micros[0], 5_000_000, "treasury untouched");
        assert_eq!(world.ships.contract[0], None, "craft released");
        assert_eq!(world.ships.role[0], CraftRole::Idle, "craft Idle");
        assert_eq!(world.ships.cargo[0], None, "nothing loaded");

        // Hull L1 -> capacity 5 + 1*5 == 10: the SAME accept now settles
        // (escrow + same-tick load at the stocked, co-located origin).
        world.ships.upgrades[0].hulls = 1;
        world.step(&mut vec![accept]);
        assert_eq!(world.contracts.status[0], ContractStatus::CargoLoaded, "settles after hull L1");
        assert_eq!(world.contracts.hauler[0], Some(craft), "hauler bound");
        assert_eq!(world.contracts.escrow_micros[0], 1_000_000, "reward escrowed");
        assert_eq!(world.corporations.treasury_micros[0], 4_000_000, "treasury debited");
        assert_eq!(world.ships.cargo[0], Some((Good::FUEL, 10)), "qty-10 lot loaded");
    }

    #[test]
    fn scripted_assign_filters_oversized_contracts() {
        // Filter-at-choice, never claim-and-revert (spec §6): scripted ASSIGN
        // skips a lot bigger than the hauler's derived capacity.
        use crate::config::{BaseSpec, DispatchCfg, ShipyardCfg};
        use crate::math::Vec3;

        let mut stations = StationStore::empty();
        let from = stations.push(BodyId { slot: 0, generation: 0 }, [0, 100], [0; N_RESOURCES]);
        let to = stations.push(BodyId { slot: 1, generation: 0 }, [0, 0], [0; N_RESOURCES]);
        let mut corporations = CorporationStore::empty();
        let corp = corporations.push(1_000_000, from);
        let mut contracts = ContractStore::empty();
        let cid = contracts.push(corp, Good::FUEL, 10, from, to, 1_000);
        let mut ships = CraftStore::empty();
        ships.push(
            BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
                base_cargo_capacity: 5,
            },
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
        );
        // High demand edges so REPOST stays quiet (the route has a live Offered
        // row -> bursting, but projected 10 >= demand_high 5 posts nothing).
        let dispatch = DispatchCfg {
            demand_low: 5,
            demand_high: 5,
            stagger_period: 1,
            contract_reward_micros: 0,
            contract_qty: 0,
        };
        let shipyard = ShipyardCfg::default();
        let mut events = EventStream::new();

        let no_evidence =
            crate::world::RouteEvidence { robs: Vec::new(), cursor: Vec::new() };
        // hulls 0 -> capacity 5 < qty 10: ASSIGN must NOT claim.
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &no_evidence,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &crate::config::TrophicCfg::default(),
            Tick(1),
            &mut events,
        );
        assert_eq!(ships.contract[0], None, "oversized lot not claimed");
        assert_eq!(ships.role[0], CraftRole::Idle, "craft stays Idle");

        // hulls 1 -> capacity 10 == qty 10: ASSIGN claims it.
        ships.upgrades[0].hulls = 1;
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &no_evidence,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &crate::config::TrophicCfg::default(),
            Tick(2),
            &mut events,
        );
        assert_eq!(ships.contract[0], Some(cid), "fitting lot claimed");
        assert_eq!(ships.role[0], CraftRole::Hauler);
    }

    #[test]
    fn scripted_assign_filters_dry_tank_craft_play_c1() {
        use crate::contract::Command;
        use crate::world::World;
        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        cfg.craft[0].fuel_mass = 0.0;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..8 {
            world.step(&mut empty);
        }
        assert_eq!(world.contracts.status[0], ContractStatus::Offered, "dry tank: never claimed");
        assert_eq!(world.ships.role[0], CraftRole::Idle, "stays Idle forever");
        assert_eq!(world.ships.contract[0], None, "no binding written");

        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        for _ in 0..8 {
            world.step(&mut empty);
        }
        assert_ne!(world.contracts.status[0], ContractStatus::Offered, "live tank: claimed");
    }

    #[test]
    fn trophic_world_still_dispatches_under_fuel_eligibility() {
        use crate::world::World;
        let cfg = crate::scenario::scenario_trophic(7);
        let (mut world, _h) = World::reset(cfg).expect("trophic resolves");
        let mut cmds = Vec::new();
        for _ in 0..3_000u64 {
            world.step(&mut cmds);
        }
        let accepts = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ContractAccepted { .. }))
            .count();
        assert!(
            accepts > 0,
            "trophic world dispatched ZERO contracts — the fuel-eligibility filter is binding on full-tank band haulers"
        );
    }

    // ---- Pirates rung Commit E: evidence-scored ASSIGN + scripted purchases ----

    /// ASSIGN-scoring fixture: stations A/B/C (rows 0/1/2), two same-reward
    /// Offered contracts A->B (c0, the lower ContractId) and A->C (c1), one
    /// Idle craft, empty route-evidence rings (dense 3x3 = 9 directed routes).
    #[allow(clippy::type_complexity)]
    fn scoring_fix() -> (
        ContractStore,
        StationStore,
        CraftStore,
        crate::world::RouteEvidence,
        ContractId,
        ContractId,
    ) {
        use crate::config::BaseSpec;
        use crate::math::Vec3;
        let mut stations = StationStore::empty();
        let a = stations.push(BodyId { slot: 0, generation: 0 }, [0; N_RESOURCES], [0; N_RESOURCES]);
        let b = stations.push(BodyId { slot: 1, generation: 0 }, [0; N_RESOURCES], [0; N_RESOURCES]);
        let c = stations.push(BodyId { slot: 2, generation: 0 }, [0; N_RESOURCES], [0; N_RESOURCES]);
        let mut corporations = CorporationStore::empty();
        let corp = corporations.push(0, a);
        let mut contracts = ContractStore::empty();
        let c0 = contracts.push(corp, Good::FUEL, 5, a, b, 1_000_000);
        let c1 = contracts.push(corp, Good::FUEL, 5, a, c, 1_000_000);
        let mut ships = CraftStore::empty();
        ships.push(
            BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
                base_cargo_capacity: 5,
            },
            Vec3::ZERO,
            Vec3::ZERO,
            1.0,
        );
        let route_evidence = crate::world::RouteEvidence {
            robs: vec![[Tick(0); 8]; 9],
            cursor: vec![0; 9],
        };
        (contracts, stations, ships, route_evidence, c0, c1)
    }

    #[test]
    fn evidence_scored_assign_avoids_hot_routes() {
        // Spec §7: with `hauler_belief_scoring` on, scripted ASSIGN picks
        // argmax reward * (1000 - evidence * penalty) / 1000 (clamped), ties to
        // the lowest ContractId — and the avoidance DECAYS as the evidence ages
        // out of the reader's dock-gated window.
        use crate::config::{DispatchCfg, ShipyardCfg, TrophicCfg};
        let dispatch = DispatchCfg {
            demand_low: 0,
            demand_high: 0,
            stagger_period: 1,
            contract_reward_micros: 0,
            contract_qty: 0,
        };
        let shipyard = ShipyardCfg::default();
        let scoring = TrophicCfg { hauler_belief_scoring: true, ..TrophicCfg::default() };

        // CONTROL (scoring OFF): hot evidence is ignored; lowest ContractId wins.
        let (mut contracts, stations, mut ships, mut re, c0, _c1) = scoring_fix();
        re.robs[1][0] = Tick(50); // a rob on directed route A->B (0*3 + 1)
        ships.info_tick[0] = Tick(60);
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &re,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &TrophicCfg::default(),
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(ships.contract[0], Some(c0), "scoring off: lowest ContractId, evidence ignored");

        // SCORING ON: a fresh rob on A->B flips the scripted claim to A->C.
        let (mut contracts, stations, mut ships, mut re, _c0, c1) = scoring_fix();
        re.robs[1][0] = Tick(50);
        ships.info_tick[0] = Tick(60); // docked after the rob: evidence visible
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &re,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &scoring,
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(ships.contract[0], Some(c1), "scoring on: the hot route is avoided");
        assert_eq!(ships.role[0], CraftRole::Hauler);

        // DECAYS BACK: far past the evidence window the route reads cold again;
        // equal scores tie to the lowest ContractId.
        let (mut contracts, stations, mut ships, mut re, c0, _c1) = scoring_fix();
        re.robs[1][0] = Tick(50);
        ships.info_tick[0] = Tick(50 + 4000); // aged exactly out of the window
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &re,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &scoring,
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(ships.contract[0], Some(c0), "aged evidence decays the avoidance away");
    }

    #[test]
    fn evidence_scored_assign_reads_gossip_when_media_live() {
        // Task 7 (media rung spec §7/§13): media LIVE, the ASSIGN site's
        // evidence count is the hauler's OWN comms-log — `info_tick`'s
        // evidence-read role has ended — while the 900-clamp valence
        // arithmetic around the count is untouched.
        use crate::config::{DispatchCfg, ShipyardCfg, TrophicCfg};
        use crate::media::{GossipAlert, GossipBuffer};
        let dispatch = DispatchCfg {
            demand_low: 0,
            demand_high: 0,
            stagger_period: 1,
            contract_reward_micros: 0,
            contract_qty: 0,
        };
        let shipyard = ShipyardCfg::default();
        let scoring = TrophicCfg { hauler_belief_scoring: true, ..TrophicCfg::default() };
        let hot_route_1 = GossipAlert {
            alert_seq: 0,
            route: 1, // directed A->B (0*3 + 1) — c0's route
            pirate_slot: 0,
            rob_tick: Tick(0),
            claimed_value_micros: 2_000_000,
            first_heard: Tick(0),
            hops: 1,
        };

        // (1) Gossip-hot route 1, ring EMPTY, media live: the scripted claim
        // flips to A->C — the count came from the comms-log.
        let (mut contracts, stations, mut ships, re, _c0, c1) = scoring_fix();
        let mut buf = GossipBuffer::empty(8);
        buf.slots[0] = Some(hot_route_1);
        ships.gossip[0] = Some(buf);
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &re,
            true,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &scoring,
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(ships.contract[0], Some(c1), "live media: own gossip flags the hot route");

        // (2) Ring-hot route 1, comms-log EMPTY, media live: the ring no
        // longer reaches the score — lowest ContractId wins.
        let (mut contracts, stations, mut ships, mut re, c0, _c1) = scoring_fix();
        re.robs[1][0] = Tick(50);
        ships.info_tick[0] = Tick(60); // docked after the rob: legacy would see it
        ships.gossip[0] = Some(GossipBuffer::empty(8));
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &re,
            true,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &shipyard,
            &scoring,
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(
            ships.contract[0],
            Some(c0),
            "live media: the legacy ring is dead to the reader"
        );
    }

    #[test]
    fn assign_skips_unscripted_craft() {
        // Spec §5: scripted stages skip gym-controlled craft — ASSIGN never
        // claims for a `!scripted` row.
        use crate::config::{BaseSpec, CraftInit, DispatchCfg, ShipyardCfg, TrophicCfg};
        use crate::math::Vec3;
        let (mut contracts, stations, mut ships, re, _c0, _c1) = scoring_fix();
        let dispatch = DispatchCfg {
            demand_low: 0,
            demand_high: 0,
            stagger_period: 1,
            contract_reward_micros: 0,
            contract_qty: 0,
        };
        let unscripted = vec![CraftInit {
            spec: BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
                base_cargo_capacity: 5,
            },
            pos: Vec3::ZERO,
            vel: Vec3::ZERO,
            fuel_mass: 0.0,
            role: CraftRole::Idle,
            scripted: false,
        }];
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &unscripted,
            &re,
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &ShipyardCfg::default(),
            &TrophicCfg::default(),
            Tick(1),
            &mut EventStream::new(),
        );
        assert_eq!(ships.contract[0], None, "ASSIGN never claims for a !scripted craft");
        assert_eq!(ships.role[0], CraftRole::Idle);
    }

    #[test]
    fn purchases_desynchronize() {
        // Spec §6: purchase timing varies per hauler through wealth/docking
        // history — no taste scalars, no synchronized buy tick (the
        // synchronization-death guard). Four docked haulers on distinct income
        // rates cross the working-capital headroom at different ticks.
        use crate::config::BuyPolicy;
        use crate::stores::UpgradeKind;
        use crate::world::World;
        let mut cfg = vendor_world_fixture(true);
        cfg.craft = vec![
            cfg.craft[0].clone(),
            cfg.craft[0].clone(),
            cfg.craft[0].clone(),
            cfg.craft[0].clone(),
        ];
        cfg.trophic.hauler_buy_policy = BuyPolicy::EscortFirst;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        for _t in 1..=60u64 {
            for r in 0..4 {
                // Distinct income rates stand in for distinct contract histories.
                world.ships.credits_micros[r] += (r as i64 + 1) * 250_000;
            }
            world.step(&mut Vec::new());
        }
        let buys: Vec<(crate::ids::CraftId, Tick)> = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::UpgradePurchased { craft, kind: UpgradeKind::Escort, level: 1, .. } => {
                    Some((craft, e.tick))
                }
                _ => None,
            })
            .collect();
        assert_eq!(buys.len(), 4, "every hauler bought Escort L1: {buys:?}");
        let mut ticks: Vec<u64> = buys.iter().map(|(_, t)| t.0).collect();
        ticks.sort_unstable();
        ticks.dedup();
        assert!(ticks.len() > 1, "purchase ticks must DISPERSE, got a synchronized buy at {ticks:?}");
    }

    #[test]
    fn pirate_buys_escort_while_lying_low_at_hideout_vendor() {
        // Spec §6: pirates shop WHILE HIDING (at the hideout, lying low, full
        // price — no headroom), phase-lagging the pirate ladder behind the
        // hauler ladder. An ACTIVE pirate never writes the intent.
        use crate::world::World;
        let mut cfg = vendor_world_fixture(true);
        cfg.craft[0].role = CraftRole::Pirate;
        cfg.trophic.engage_radius_au = 5.0e-4; // trophic live
        cfg.trophic.hideout_body_index = 0; // the vendor body doubles as the refuge
        let (mut world, _h) = World::reset(cfg.clone()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 5_000_000;
        world.ships.pirate[0].as_mut().unwrap().lie_low_until = Tick(10_000);
        world.step(&mut Vec::new());
        assert_eq!(world.ships.upgrades[0].escorts, 1, "lying-low pirate bought the escort");
        assert_eq!(world.ships.credits_micros[0], 0, "debited the full L1 price (no headroom)");
        assert_eq!(world.corporations.treasury_micros[0], 5_000_000, "Yard credited");

        // Control: an ACTIVE pirate does not shop.
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 5_000_000;
        world.step(&mut Vec::new());
        assert_eq!(world.ships.upgrades[0].escorts, 0, "active pirate does not shop");
        assert_eq!(world.ships.credits_micros[0], 5_000_000, "wallet untouched");
    }

    #[test]
    fn credit_identity_holds_across_purchases() {
        // The existing Σtreasury+Σcredits+Σescrow identity, extended across the
        // new purchase leg: every settle is a pure wallet->Yard transfer.
        use crate::stores::UpgradeKind;
        use crate::world::World;
        let (mut world, _h) = World::reset(vendor_world_fixture(true)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;

        let credit_now = |w: &World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        let initial_credit = credit_now(&world);

        // Walk the full ladder (Escort L1/L2, Hull L1/L2); identity holds EVERY tick.
        for (i, kind) in [
            UpgradeKind::Escort,
            UpgradeKind::Escort,
            UpgradeKind::Hull,
            UpgradeKind::Hull,
        ]
        .into_iter()
        .enumerate()
        {
            world.step(&mut vec![buy_cmd(craft, kind)]);
            assert_eq!(
                credit_now(&world),
                initial_credit,
                "Σtreasury+Σcredits+Σescrow invariant after purchase {i}"
            );
        }

        // Non-vacuity: all four arms actually settled (45M total moved to the Yard).
        let purchases = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::UpgradePurchased { .. }))
            .count();
        assert_eq!(purchases, 4, "all four catalog arms fired");
        assert_eq!(world.corporations.treasury_micros[0], 45_000_000);
        assert_eq!(world.ships.credits_micros[0], 5_000_000);
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
        let seed = contracts.push(corp, Good::FUEL, 5, from, to, 1_000);
        let srow = contracts.ids.dense_index(seed.slot, seed.generation).unwrap();
        contracts.status[srow] = ContractStatus::Completed;
        let mut ships = CraftStore::empty();
        ships.push(
            BaseSpec {
                base_dry_mass: 1.0,
                base_max_thrust: 0.0,
                base_exhaust_velocity: 1.0,
                base_fuel_capacity: 1.0,
                base_cargo_capacity: 5,
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
            &[],
            &crate::world::RouteEvidence { robs: Vec::new(), cursor: Vec::new() },
            false,
            false,
            &mut AssignDiag::default(),
            &dispatch,
            &crate::config::ShipyardCfg::default(),
            &crate::config::TrophicCfg::default(),
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

    // ---- jumpgate-2c0c2d92bb: FuelEmpty on the escrow-holding pre-transit statuses ----

    /// Two-body starved-hauler shape (the world.rs `two_body_starved_contract_fixture`
    /// pattern, rebuilt here for the stage-3c tests): a near-massless central body 0
    /// and a negligible-mass marker body 1 on a 0.3 AU orbit, one station on each
    /// (BOTH stocked, so distance is the only load blocker), one corp, one Fuel(5)
    /// contract `from_station_index -> to_station_index`, and one manual (unscripted)
    /// hauler co-located with body 0 whose propellant starts just above
    /// `FUEL_EMPTY_EPS` (1e-11) — enough to survive step 1, but drained across the eps
    /// threshold a couple of ticks into any burn, long before it can cover 0.3 AU.
    fn starved_two_body_contract_fixture(
        from_station_index: usize,
        to_station_index: usize,
    ) -> crate::config::RunConfig {
        use crate::config::{
            BaseSpec, BodyInit, ContractInit, CorporationInit, CraftInit, GuidanceParams,
            OrbitalElements, RunConfig, StationInit, SubstepCfg,
        };
        use crate::math::Vec3;
        use crate::time::Dt;
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 4096,
            bodies: vec![
                BodyInit {
                    // Near-massless: the co-located craft is not gravity-trapped.
                    mass: 1e-9,
                    elements: OrbitalElements {
                        a: 0.0,
                        e: 0.0,
                        i: 0.0,
                        raan: 0.0,
                        argp: 0.0,
                        m0: 0.0,
                    },
                },
                BodyInit {
                    // Negligible-mass marker body; only its position matters.
                    mass: 1e-12,
                    elements: OrbitalElements {
                        a: 0.3,
                        e: 0.0,
                        i: 0.0,
                        raan: 0.0,
                        argp: 0.0,
                        m0: 0.0,
                    },
                },
            ],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-9,
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2,
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::ZERO, // co-located with body 0 (station 0's host)
                vel: Vec3::ZERO,
                // REDESIGNED (not nudged) for the eps re-bake 1e-9 -> 1e-11
                // (spec §4 item 1; was 1.06e-9 = old eps + 6e-11). Same 6e-11
                // headroom above the NEW eps = 2.4 full-throttle burn ticks
                // (1e-12/1e-2*0.25 = 2.5e-11/tick): survives step 1 at 4.5e-11,
                // runs dry across eps on tick 3 — both the Accepted deadhead arm
                // and the CargoLoaded-window arm keep their step-1 asserts.
                fuel_mass: 7.0e-11,
                role: CraftRole::Idle,
                scripted: false, // manual-accept only: scripted ASSIGN stays out
            }],
            guidance: GuidanceParams::default(),
            stations: vec![
                StationInit {
                    body_index: 0,
                    initial_stock: [0, 10],
                    initial_price_micros: [0, 0],
                    sells_upgrades: false,
                },
                StationInit {
                    body_index: 1,
                    initial_stock: [0, 10],
                    initial_price_micros: [0, 0],
                    sells_upgrades: false,
                },
            ],
            producers: vec![],
            corporations: vec![CorporationInit {
                treasury_micros: 5_000_000,
                home_station_index: 0,
            }],
            contracts: vec![ContractInit {
                corp_index: 0,
                resource: Good::FUEL,
                qty: 5,
                from_station_index,
                to_station_index,
                reward_micros: 1_000_000,
            }],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
            trophic: crate::config::TrophicCfg::default(),
            shipyard: crate::config::ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: crate::config::RefuelCfg::default(),
        }
    }

    /// The sum the Σtreasury+Σcredits+Σescrow identity asserts over.
    fn total_credit(world: &crate::world::World) -> i64 {
        world.corporations.treasury_micros.iter().sum::<i64>()
            + world.ships.credits_micros.iter().sum::<i64>()
            + world.contracts.escrow_micros.iter().sum::<i64>()
    }

    #[test]
    fn fuel_empty_mid_deadhead_refunds_escrow() {
        // jumpgate-2c0c2d92bb: `resolve_failures` only failed `InTransit` contracts
        // on FuelEmpty, so a hauler that ran dry on the DEADHEAD leg (`Accepted`,
        // escrow already debited) or in the one-tick `CargoLoaded` window locked its
        // escrow forever. Fix = option (a): fail+refund all three escrow-holding
        // non-terminal statuses.
        use crate::contract::Command;
        use crate::types::{CommandKind, EntityRef, Target};
        use crate::world::World;
        let fuel = Good::FUEL.index();

        // ---- Arm 1: status `Accepted` (the deadhead leg), end-to-end through
        // `World::step`. Origin = the FAR station (body 1), so the step-1 accept
        // escrows OFF-STATION, `try_load` dispatches the deadhead, and the load
        // never happens. The hauler runs dry mid-deadhead while still `Accepted`.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let initial_treasury = world.corporations.treasury_micros[0];
        let initial_credit = total_credit(&world);
        let consumed_before = world.econ.consumed[fuel];

        let contract = contract_id(&world.contracts, 0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert_eq!(
            world.contracts.status[0],
            ContractStatus::Accepted,
            "escrowed off-station on step 1; the load never fires (origin 0.3 AU away)"
        );
        assert_eq!(world.contracts.escrow_micros[0], 1_000_000, "escrow == reward");

        // Step until FuelEmpty fires mid-deadhead and stage 3c settles the failure.
        let mut empty: Vec<Command> = Vec::new();
        let mut failed = false;
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.contracts.status[0] == ContractStatus::Failed {
                failed = true;
                break;
            }
        }
        assert!(failed, "Accepted contract reached Failed within the step bound");
        assert_eq!(world.contracts.escrow_micros[0], 0, "escrow zeroed on fail");
        assert_eq!(
            world.corporations.treasury_micros[0], initial_treasury,
            "escrow refunded exactly the reward to the corp treasury"
        );
        let crow = world.ships.index_of(craft).unwrap();
        assert_eq!(world.ships.cargo[crow], None, "no cargo was ever loaded");
        assert_eq!(world.ships.contract[crow], None, "hauler released: handle cleared");
        assert_eq!(world.ships.role[crow], CraftRole::Idle, "hauler released: role Idle");
        // NO cargo sink leg: there was no cargo on the deadhead.
        assert_eq!(
            world.econ.consumed[fuel], consumed_before,
            "consumed[Fuel] unchanged (deadhead carried no cargo)"
        );
        assert_eq!(total_credit(&world), initial_credit, "Σtreasury+Σcredits+Σescrow invariant");

        // ---- Arm 2: status `CargoLoaded` (the one-tick load window). Standard
        // direction (origin = the co-located station 0): the step-1 accept loads
        // and dispatches. Drain the propellant PRE-DISPATCH via a direct field
        // write, then run stage 3c directly with this craft in the FuelEmpty list.
        // (Through `World::step` the NEXT tick's stage 1c promotes
        // CargoLoaded -> InTransit before stage-3 detection, so the in-window
        // failure is only reachable when FuelEmpty fires in the load tick itself;
        // the direct stage call reproduces exactly that stage-3c input.)
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(0, 1)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let initial_treasury = world.corporations.treasury_micros[0];
        let initial_credit = total_credit(&world);
        let consumed_before = world.econ.consumed[fuel];

        let contract = contract_id(&world.contracts, 0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert_eq!(
            world.contracts.status[0],
            ContractStatus::CargoLoaded,
            "loaded at the co-located origin on step 1"
        );
        let crow = world.ships.index_of(craft).unwrap();
        let (lost_res, lost_qty) = world.ships.cargo[crow].expect("cargo loaded on step 1");
        assert_eq!((lost_res, lost_qty), (Good::FUEL, 5));

        world.ships.fuel_mass[crow] = 0.0; // drained dry pre-dispatch
        let mut failure_events = EventStream::new();
        resolve_failures(
            &mut world.contracts,
            &mut world.corporations,
            &mut world.ships,
            &mut world.econ,
            &[craft],
            Tick(2),
            &mut failure_events,
        );

        assert_eq!(
            world.contracts.status[0],
            ContractStatus::Failed,
            "CargoLoaded contract failed in the one-tick load window"
        );
        assert_eq!(world.contracts.escrow_micros[0], 0, "escrow zeroed on fail");
        assert_eq!(
            world.corporations.treasury_micros[0], initial_treasury,
            "escrow refunded exactly the reward to the corp treasury"
        );
        assert_eq!(world.ships.cargo[crow], None, "cargo cleared (lost) on fail");
        assert_eq!(world.ships.contract[crow], None, "hauler released: handle cleared");
        assert_eq!(world.ships.role[crow], CraftRole::Idle, "hauler released: role Idle");
        // Cargo sink leg: the loaded cargo is LOST and accounted.
        assert_eq!(
            world.econ.consumed[fuel],
            consumed_before + lost_qty as i64,
            "lost cargo accounted into consumed[Fuel]"
        );
        assert_eq!(total_credit(&world), initial_credit, "Σtreasury+Σcredits+Σescrow invariant");
    }

    #[test]
    fn fuel_empty_failure_emits_contract_failed_with_actual_refund() {
        use crate::contract::Command;
        use crate::types::{CommandKind, EntityRef, Target};
        use crate::world::World;
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        assert_eq!(world.contracts.status[0], ContractStatus::Accepted, "deadhead leg armed");
        let escrow_before = world.contracts.escrow_micros[0];
        assert!(escrow_before > 0, "escrow held");

        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());

        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed {
                    contract,
                    hauler,
                    cause: FailureCause::FuelEmpty,
                    escrow_refunded_micros,
                    cargo_lost: 0,
                } if contract == cid && hauler == craft && escrow_refunded_micros == escrow_before
            )),
            "FuelEmpty failure narrated with the actual refund"
        );

        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        world.contracts.corp[0] = CorporationId { slot: 99, generation: 0 };
        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(world.contracts.escrow_micros[0] > 0, "escrow stays put on the degrade arm");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed { escrow_refunded_micros: 0, .. }
            )),
            "degrade arm reports the actual 0 refund"
        );
    }

    #[test]
    fn robbed_teardown_is_not_narrated_by_contract_failed() {
        use crate::contract::Command;
        use crate::types::{CommandKind, EntityRef, Target};
        use crate::world::World;
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(0, 1)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        assert_eq!(world.contracts.status[0], ContractStatus::CargoLoaded, "the load-tick window");

        let mut ev = EventStream::default();
        settle_contract_failure(
            &mut world.contracts,
            &mut world.corporations,
            &mut world.ships,
            &mut world.econ,
            0,
            FailureCause::Robbed,
            Tick(99),
            &mut ev,
        );
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            !ev.since(Tick(0)).iter().any(|e| matches!(e.kind, EventKind::ContractFailed { .. })),
            "Robbed teardown emits no ContractFailed"
        );
    }

    /// The WHY-panel windows (2026-06-11): the candidate-count histogram bumps
    /// per scored candidate, and a gossip-vs-ring argmax disagreement is
    /// counted as a flip (media-live only); agreeing sources never flip.
    #[test]
    fn assign_diag_counts_candidates_and_flags_argmax_flips() {
        use crate::config::{BaseSpec, DispatchCfg, ShipyardCfg, TrophicCfg};
        use crate::ids::BodyId;
        use crate::math::Vec3;
        use crate::media::{GossipAlert, GossipBuffer};
        use crate::world::RouteEvidence;
        let spec = || BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 0.0,
            base_exhaust_velocity: 1.0,
            base_fuel_capacity: 1.0,
            base_cargo_capacity: 5,
        };
        // Two stations, two equal-reward offers on opposite directed routes
        // (dense 2-station layout: s0->s1 = route 1, s1->s0 = route 2).
        let mut stations = StationStore::empty();
        let s0 = stations.push(BodyId { slot: 0, generation: 0 }, [0, 0], [0; N_RESOURCES]);
        let s1 = stations.push(BodyId { slot: 1, generation: 0 }, [0, 0], [0; N_RESOURCES]);
        let mut corporations = CorporationStore::empty();
        let corp = corporations.push(0, s0);
        let mut contracts = ContractStore::empty();
        contracts.push(corp, Good::FUEL, 5, s0, s1, 1_000_000); // k0, route 1
        contracts.push(corp, Good::FUEL, 5, s1, s0, 1_000_000); // k1, route 2
        let mut ships = CraftStore::empty();
        ships.push(spec(), Vec3::ZERO, Vec3::ZERO, 1.0);
        // The hauler's OWN gossip says route 1 (k0) is hot...
        let mut buf = GossipBuffer::empty(8);
        buf.slots[0] = Some(GossipAlert {
            alert_seq: 0,
            route: 1,
            pirate_slot: 9,
            rob_tick: Tick(90),
            claimed_value_micros: 3_000_000,
            first_heard: Tick(95),
            hops: 1,
        });
        ships.gossip[0] = Some(buf);
        // ...while the legacy ring says route 2 (k1) is hot.
        let mut ring = RouteEvidence { robs: vec![[Tick(0); 8]; 4], cursor: vec![0; 4] };
        ring.robs[2][0] = Tick(90);
        ships.info_tick[0] = Tick(100); // ring read is dock-fresh
        let trophic = TrophicCfg { hauler_belief_scoring: true, ..TrophicCfg::default() };
        let dispatch = DispatchCfg { stagger_period: 1, ..DispatchCfg::default() };
        let mut diag = AssignDiag::default();
        let mut events = EventStream::new();
        run_scripted_dispatch(
            &mut contracts,
            &stations,
            &mut ships,
            &[],
            &ring,
            true,
            false,
            &mut diag,
            &dispatch,
            &ShipyardCfg::default(),
            &trophic,
            Tick(100),
            &mut events,
        );
        // Gossip avoids route 1 -> picks k1 (slot 1); the ring would have
        // avoided route 2 and picked k0 -> an argmax flip, counted.
        assert_eq!(ships.contract[0].map(|c| c.slot), Some(1), "gossip pick is k1");
        assert_eq!(diag.decisions, 1, "one belief-scored pick");
        assert_eq!(diag.flips, 1, "gossip vs ring argmax disagreement is a flip");
        // Candidate histogram: k0's active (gossip) count 1, k1's count 0.
        assert_eq!(diag.candidate_counts[0], 1, "one zero-count candidate");
        assert_eq!(diag.candidate_counts[1], 1, "one count-1 candidate");

        // Control: agreeing sources (no evidence anywhere) -> no flip.
        let mut ships2 = CraftStore::empty();
        ships2.push(spec(), Vec3::ZERO, Vec3::ZERO, 1.0);
        ships2.gossip[0] = Some(GossipBuffer::empty(8));
        ships2.info_tick[0] = Tick(100);
        let mut contracts2 = ContractStore::empty();
        contracts2.push(corp, Good::FUEL, 5, s0, s1, 1_000_000);
        contracts2.push(corp, Good::FUEL, 5, s1, s0, 1_000_000);
        let empty_ring = RouteEvidence { robs: vec![[Tick(0); 8]; 4], cursor: vec![0; 4] };
        let mut diag2 = AssignDiag::default();
        run_scripted_dispatch(
            &mut contracts2,
            &stations,
            &mut ships2,
            &[],
            &empty_ring,
            true,
            false,
            &mut diag2,
            &dispatch,
            &ShipyardCfg::default(),
            &trophic,
            Tick(100),
            &mut events,
        );
        assert_eq!(diag2.decisions, 1);
        assert_eq!(diag2.flips, 0, "agreeing sources never flip");
    }
}
