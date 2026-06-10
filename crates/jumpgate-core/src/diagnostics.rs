//! Trophic diagnostics — the instrument (pirates rung 1, P0).
//!
//! FRAME (PDR-0006): Jumpgate v1 is a GAME judged by emergent play. Everything
//! in this module is a designer's WINDOW for the observe→steer→re-observe loop
//! at the console — never an acceptance gate, never a build trigger. The
//! classifier is "the instrument beside the window, not the judge" (spec §10).
//!
//! Shape: one integer `TrophicSample` per W-tick window (sampled by the runner,
//! `examples/trophic_run.rs`), and a pure `classify(&[TrophicSample]) ->
//! Diagnosis` that names a row of the spec-§9 diagnosis matrix. Sampling reads
//! the world; classification reads only samples (synthetic series test it).

use crate::contract::{EventKind, StateView};
use crate::ids::ContractId;
use crate::time::Tick;
use crate::world::World;

// DIAGNOSTIC WINDOWS, NOT GATES (PDR-0006): the named thresholds below tune the
// instrument's readout bands. They are designer windows for the console tuning
// loop (plan Task 6.6), free to move there; no numeric value here is a kill
// trigger or an acceptance bar.

/// Window width in ticks (spec §1: 50k-tick runs = 25 windows of W = 2,000).
pub const WINDOW_TICKS: u64 = 2000;

/// Minimum direction alternations of the active-pirate series for "cycled"
/// (a flat or monotone series has none; boom/bust has many).
pub const CYCLE_MIN_ALTERNATIONS: u32 = 3;

/// Minimum mean active-pirate-normalized HHI (milli) of robberies over
/// OCCUPIED routes for "risk heterogeneous". 1000 milli ≈ "each active pirate
/// owns one route"; even spread over m ≫ k routes reads ≈ 1000·k/m.
pub const HHI_NORM_MIN_MILLI: u64 = 600;

/// Minimum final-window per-craft credit spread (milli of the largest
/// magnitude) for "outcomes disperse".
pub const OUTCOME_DISPERSION_MIN_MILLI: u64 = 200;

/// One per-window integer reading of the trophic field (spec §9). All fields
/// are integers: samples are hash-adjacent evidence, never float analytics.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrophicSample {
    /// World tick at the window's CLOSE (the sample point).
    pub tick: u64,
    /// Pirates on the predation field at the sample point (`lie_low_until <= tick`).
    pub active_pirates: u32,
    /// Pirates lying low at the sample point.
    pub lying_low: u32,
    /// Craft carrying cargo at the sample point.
    pub laden_in_transit: u32,
    /// `ContractFulfilled` events in the window — THE §4 calibration input
    /// (`laden_trips_per_window` parameterizes the food band).
    pub laden_trips: u32,
    /// `Robbed` events in the window.
    pub robs: u32,
    /// `DrivenOff` events in the window.
    pub drivenoffs: u32,
    /// Hull purchases in the window. Counted from Task 3's `UpgradePurchased`.
    pub purchases_hull: u32,
    /// Escort purchases in the window. Counted from Task 3's `UpgradePurchased`.
    pub purchases_escort: u32,
    /// Robberies per directed route (`from_row * n_stations + to_row`, dense).
    pub per_route_robs: Vec<u32>,
    /// Contract accepts settled per directed route in the window.
    pub per_route_accepts: Vec<u32>,
    /// Laden transits that flew the route in the window, however they ended
    /// (fulfilled + robbed) — the occupancy mask for the heterogeneity metric.
    pub per_route_traffic: Vec<u32>,
    /// Yard corp treasury at the sample point (the broken-flow diagnostic;
    /// 0 until Task 1 mints the Yard).
    pub yard_treasury_micros: i64,
    /// Per-craft wallet snapshot at the sample point (dense row order).
    pub per_craft_credits: Vec<i64>,
    /// Trip-phase (fraction-of-trip-elapsed × 1000) of each engagement in the
    /// window — the endpoint-ambush histogram input (Task 4 pushes these).
    pub engagement_phase_milli: Vec<u32>,
}

/// A named row of the spec-§9 diagnosis matrix. Diagnosis vocabulary only —
/// no gate vocabulary (PDR-0006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Boom/bust cycles, risk clumped & persistent, outcomes track decisions.
    Alive,
    /// Populations flat or pinned — no predator-prey alternation.
    NoCycle,
    /// Risk spread evenly over occupied routes, or the hot route just tracks
    /// the traffic gradient (the ghost: the two prior NO-GOs' death mode).
    RiskEqualized,
    /// Every station covered most ticks — coverage has swamped locality.
    /// Classification arm lands with the mechanics it diagnoses (Tasks 4-6).
    Saturated,
    /// Risk is heterogeneous but peer outcomes don't track choices.
    DecisionNotTranslating,
    /// Engagements → 0 with all haulers at/above pirate strength (the
    /// absorbing-peace ratchet). Arm lands with the upgrade mechanics.
    PermanentPeace,
    /// No purchases / no regime changes on the upgrades axis. Arm lands with
    /// the upgrade mechanics.
    ArmsRaceFlat,
}

/// The instrument's reading: three orthogonal booleans plus the matrix row
/// they select. Pure function of the samples; the owner's chronicle read is
/// the judge (PDR-0006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Diagnosis {
    /// Active-pirate population alternates (boom/bust) rather than damping flat.
    pub cycled: bool,
    /// Robbery risk is clumped over occupied routes and the hot route is more
    /// persistent than the traffic gradient.
    pub risk_heterogeneous: bool,
    /// Identically-specced peers end the run with dispersed outcomes.
    pub outcomes_disperse: bool,
    /// The selected diagnosis-matrix row.
    pub verdict: Verdict,
}

/// Classify a windowed run into a diagnosis-matrix row (spec §9). Pure over
/// the samples; deliberately minimal — the console loop (Task 6.6) is the
/// place this gets refined against real chronicles.
pub fn classify(samples: &[TrophicSample]) -> Diagnosis {
    let cycled = population_cycles(samples);
    let risk_heterogeneous = risk_is_heterogeneous(samples);
    let outcomes_disperse = outcome_dispersion(samples);
    // PermanentPeace takes precedence over every other row: a starvation
    // lie-low duty cycle over a dead predation field oscillates the active
    // count without any predator-prey coupling, so `cycled` alone is a false
    // witness once the war has ended (the seed-7 lesson, 2026-06-11).
    let verdict = if predation_collapsed(samples) {
        Verdict::PermanentPeace
    } else if !cycled {
        Verdict::NoCycle
    } else if !risk_heterogeneous {
        Verdict::RiskEqualized
    } else if !outcomes_disperse {
        Verdict::DecisionNotTranslating
    } else {
        Verdict::Alive
    };
    Diagnosis { cycled, risk_heterogeneous, outcomes_disperse, verdict }
}

/// Minimum first-half engagements for PermanentPeace to be meaningful — a run
/// that never had a war (e.g. zero pirates) is NoCycle, not peace.
/// DIAGNOSTIC WINDOW, NOT A GATE (PDR-0006).
pub const PEACE_MIN_EARLY_ENGAGEMENTS: u64 = 4;

/// PermanentPeace = the war ended: engagements (robs + driven-offs) present in
/// the first half of the run but ZERO across the entire second half — the
/// upgrade ratchet's absorbing state (spec §9: pirate ladder failed to
/// out-reach the hauler ladder).
fn predation_collapsed(samples: &[TrophicSample]) -> bool {
    if samples.len() < 4 {
        return false;
    }
    let mid = samples.len() / 2;
    let eng =
        |s: &TrophicSample| u64::from(s.robs).saturating_add(u64::from(s.drivenoffs));
    let early: u64 = samples[..mid].iter().map(&eng).fold(0, u64::saturating_add);
    let late: u64 = samples[mid..].iter().map(&eng).fold(0, u64::saturating_add);
    early >= PEACE_MIN_EARLY_ENGAGEMENTS && late == 0
}

/// "Cycled" = the active-pirate series changes direction at least
/// `CYCLE_MIN_ALTERNATIONS` times (boom/bust alternation; flat/monotone = 0).
/// Anti-phase against the laden series is read at the console (spec §1); this
/// window only separates oscillation from a fixed point.
fn population_cycles(samples: &[TrophicSample]) -> bool {
    let mut alternations: u32 = 0;
    let mut prev_sign: i8 = 0;
    for w in samples.windows(2) {
        let d = i64::from(w[1].active_pirates) - i64::from(w[0].active_pirates);
        let sign: i8 = match d {
            d if d > 0 => 1,
            d if d < 0 => -1,
            _ => 0,
        };
        if sign != 0 {
            if prev_sign != 0 && sign != prev_sign {
                alternations = alternations.saturating_add(1);
            }
            prev_sign = sign;
        }
    }
    alternations >= CYCLE_MIN_ALTERNATIONS
}

/// Heterogeneity metric (spec §1/§9 — NOT top/median, which sparsity games):
/// HHI of robberies over OCCUPIED routes (`per_route_traffic > 0`), normalized
/// by the active-pirate count (mean over robbing windows), PLUS rank-persistence
/// of the hot route relative to the traffic gradient: the argmax-rob route must
/// change across windows no faster than the argmax-traffic route does ("the hot
/// route must move slower than laden traffic does").
fn risk_is_heterogeneous(samples: &[TrophicSample]) -> bool {
    let mut norm_sum: u128 = 0;
    let mut robbing_windows: u128 = 0;
    let mut hot_changes: u32 = 0;
    let mut traffic_changes: u32 = 0;
    let mut prev_hot: Option<usize> = None;
    let mut prev_traffic_max: Option<usize> = None;
    for s in samples {
        // Robs masked to occupied routes only (an un-trafficked route cannot
        // carry risk evidence; the sparsity guard).
        let occupied_robs: Vec<u64> = s
            .per_route_robs
            .iter()
            .zip(&s.per_route_traffic)
            .map(|(&r, &t)| if t > 0 { u64::from(r) } else { 0 })
            .collect();
        let total: u64 = occupied_robs.iter().fold(0u64, |a, &r| a.saturating_add(r));
        // Traffic gradient persistence is tracked over the SAME windows as the
        // hot route so the two change-rates are comparable.
        if total == 0 {
            continue;
        }
        robbing_windows += 1;
        let sum_sq: u128 = occupied_robs
            .iter()
            .map(|&r| u128::from(r) * u128::from(r))
            .sum();
        let hhi_milli = sum_sq.saturating_mul(1000) / (u128::from(total) * u128::from(total));
        norm_sum = norm_sum
            .saturating_add(hhi_milli.saturating_mul(u128::from(s.active_pirates.max(1))));
        let hot = argmax_lowest(&occupied_robs);
        if let (Some(h), Some(p)) = (hot, prev_hot)
            && h != p
        {
            hot_changes = hot_changes.saturating_add(1);
        }
        if hot.is_some() {
            prev_hot = hot;
        }
        let traffic: Vec<u64> = s.per_route_traffic.iter().map(|&t| u64::from(t)).collect();
        let tmax = argmax_lowest(&traffic);
        if let (Some(m), Some(p)) = (tmax, prev_traffic_max)
            && m != p
        {
            traffic_changes = traffic_changes.saturating_add(1);
        }
        if tmax.is_some() {
            prev_traffic_max = tmax;
        }
    }
    if robbing_windows == 0 {
        return false;
    }
    let mean_norm_milli = norm_sum / robbing_windows;
    mean_norm_milli >= u128::from(HHI_NORM_MIN_MILLI) && hot_changes <= traffic_changes
}

/// Index of the strictly greatest positive value, ties to the LOWEST index
/// (the deterministic tie convention); `None` when everything is zero/empty.
fn argmax_lowest(xs: &[u64]) -> Option<usize> {
    let mut best: Option<(usize, u64)> = None;
    for (i, &x) in xs.iter().enumerate() {
        if x > 0 && best.is_none_or(|(_, b)| x > b) {
            best = Some((i, x));
        }
    }
    best.map(|(i, _)| i)
}

/// "Outcomes disperse" = the final window's per-craft credit range is at least
/// `OUTCOME_DISPERSION_MIN_MILLI` milli of the largest wallet magnitude.
fn outcome_dispersion(samples: &[TrophicSample]) -> bool {
    let Some(last) = samples.last() else {
        return false;
    };
    let credits = &last.per_craft_credits;
    if credits.len() < 2 {
        return false;
    }
    let max = *credits.iter().max().expect("len >= 2");
    let min = *credits.iter().min().expect("len >= 2");
    let scale = u128::from(max.unsigned_abs().max(min.unsigned_abs()).max(1));
    let spread = (i128::from(max) - i128::from(min)).unsigned_abs();
    let spread_milli = spread.saturating_mul(1000) / scale;
    spread_milli >= u128::from(OUTCOME_DISPERSION_MIN_MILLI)
}

/// Read one window's `TrophicSample` off the live world: events with
/// `tick > window_start` (and ≤ the current tick) plus an instantaneous
/// population/wallet snapshot at the sample point. The runner calls this every
/// `WINDOW_TICKS`; it is a pure read (no world mutation).
pub fn sample_window(world: &World, window_start: Tick) -> TrophicSample {
    let tick = world.tick();
    let n_stations = world.stations.ids.len();
    let n_routes = n_stations.saturating_mul(n_stations);
    let mut per_route_robs = vec![0u32; n_routes];
    let mut per_route_accepts = vec![0u32; n_routes];
    let mut per_route_traffic = vec![0u32; n_routes];
    let mut laden_trips: u32 = 0;
    let mut robs: u32 = 0;
    let mut drivenoffs: u32 = 0;
    let mut purchases_hull: u32 = 0;
    let mut purchases_escort: u32 = 0;
    for e in world.recent_events(Tick(window_start.0.saturating_add(1))) {
        match e.kind {
            EventKind::ContractAccepted { contract, .. } => {
                if let Some(route) = route_of(world, contract) {
                    per_route_accepts[route] = per_route_accepts[route].saturating_add(1);
                }
            }
            EventKind::ContractFulfilled { contract, .. } => {
                laden_trips = laden_trips.saturating_add(1);
                if let Some(route) = route_of(world, contract) {
                    per_route_traffic[route] = per_route_traffic[route].saturating_add(1);
                }
            }
            EventKind::Robbed { contract, .. } => {
                robs = robs.saturating_add(1);
                if let Some(route) = route_of(world, contract) {
                    per_route_robs[route] = per_route_robs[route].saturating_add(1);
                    // A robbed trip still flew the route: it is traffic.
                    per_route_traffic[route] = per_route_traffic[route].saturating_add(1);
                }
            }
            EventKind::DrivenOff { .. } => {
                drivenoffs = drivenoffs.saturating_add(1);
            }
            EventKind::UpgradePurchased { kind, .. } => match kind {
                crate::stores::UpgradeKind::Hull => {
                    purchases_hull = purchases_hull.saturating_add(1);
                }
                crate::stores::UpgradeKind::Escort => {
                    purchases_escort = purchases_escort.saturating_add(1);
                }
            },
            _ => {}
        }
    }
    let mut active_pirates: u32 = 0;
    let mut lying_low: u32 = 0;
    let mut laden_in_transit: u32 = 0;
    for r in 0..world.ships.ids.len() {
        if let Some(p) = world.ships.pirate[r] {
            if p.lie_low_until > tick {
                lying_low = lying_low.saturating_add(1);
            } else {
                active_pirates = active_pirates.saturating_add(1);
            }
        }
        if world.ships.cargo[r].is_some() {
            laden_in_transit = laden_in_transit.saturating_add(1);
        }
    }
    TrophicSample {
        tick: tick.0,
        active_pirates,
        lying_low,
        laden_in_transit,
        laden_trips,
        robs,
        drivenoffs,
        purchases_hull,
        purchases_escort,
        per_route_robs,
        per_route_accepts,
        per_route_traffic,
        // The Yard treasury (ShipyardCfg.corp_index) at the sample point —
        // the §9 broken-flow / circulation panel. Out-of-range index reads 0.
        yard_treasury_micros: world
            .corporations
            .treasury_micros
            .get(world.shipyard_cfg().corp_index as usize)
            .copied()
            .unwrap_or(0),
        per_craft_credits: world.ships.credits_micros.clone(),
        // The stage-3b2 emission sites push one kinematic snapshot per
        // engagement (pirate.rs); this window reads the trip-phase channel.
        engagement_phase_milli: world
            .engagement_diag
            .iter()
            .filter(|s| s.tick.0 > window_start.0 && s.tick.0 <= tick.0)
            .map(|s| s.phase_milli)
            .collect(),
    }
}

/// Directed route index of a contract: `from_row * n_stations + to_row`
/// (dense `n_stations²` layout, matching the per-route vectors).
fn route_of(world: &World, contract: ContractId) -> Option<usize> {
    let k = world
        .contracts
        .ids
        .dense_index(contract.slot, contract.generation)?;
    let n = world.stations.ids.len();
    let from = world.contracts.from_station[k];
    let to = world.contracts.to_station[k];
    let fr = world.stations.ids.dense_index(from.slot, from.generation)?;
    let tr = world.stations.ids.dense_index(to.slot, to.generation)?;
    Some(fr.saturating_mul(n).saturating_add(tr))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-built window: route vectors are 4 routes wide; traffic doubles as
    /// the accepts series (irrelevant to classify).
    fn s(
        tick: u64,
        active: u32,
        laden: u32,
        robs_by_route: &[u32],
        traffic: &[u32],
        credits: &[i64],
    ) -> TrophicSample {
        TrophicSample {
            tick,
            active_pirates: active,
            lying_low: 0,
            laden_in_transit: laden,
            laden_trips: traffic.iter().sum(),
            robs: robs_by_route.iter().sum(),
            drivenoffs: 0,
            purchases_hull: 0,
            purchases_escort: 0,
            per_route_robs: robs_by_route.to_vec(),
            per_route_accepts: traffic.to_vec(),
            per_route_traffic: traffic.to_vec(),
            yard_treasury_micros: 0,
            per_craft_credits: credits.to_vec(),
            engagement_phase_milli: Vec::new(),
        }
    }

    const DISPERSED: [i64; 4] = [10_000_000, 2_000_000, 5_000_000, 1_000_000];
    const UNIFORM: [i64; 4] = [5_000_000, 5_000_000, 5_000_000, 5_000_000];

    /// Boom/bust + robs clumped on route 0 (persistent) while the traffic
    /// gradient's argmax hops every window + dispersed final outcomes.
    fn cycling_heterogeneous(credits: &[i64]) -> Vec<TrophicSample> {
        (0..12u64)
            .map(|w| {
                let boom = w % 2 == 0;
                let traffic: &[u32] = if boom { &[5, 7, 6, 5] } else { &[7, 5, 6, 5] };
                s(
                    (w + 1) * WINDOW_TICKS,
                    if boom { 1 } else { 3 },
                    if boom { 6 } else { 2 },
                    &[3, 0, 0, 0],
                    traffic,
                    credits,
                )
            })
            .collect()
    }

    /// Seed-7's real shape (2026-06-11 console session, the instrument's first
    /// caught lie): a violent opening, then the upgrade ratchet ends the war —
    /// actives keep duty-cycling (starvation lie-low) but engagements are ZERO
    /// for the whole late half. The flagship run read Alive on this; it is
    /// PermanentPeace.
    fn early_burst_then_silence() -> Vec<TrophicSample> {
        (0..12u64)
            .map(|w| {
                let robs: [u32; 4] = if w < 3 { [4, 1, 0, 0] } else { [0, 0, 0, 0] };
                s(
                    (w + 1) * WINDOW_TICKS,
                    if w % 2 == 0 { 1 } else { 3 },
                    if w % 2 == 0 { 6 } else { 2 },
                    &robs,
                    &[5, 7, 6, 5],
                    &DISPERSED,
                )
            })
            .collect()
    }

    #[test]
    fn early_burst_then_silence_reads_permanent_peace() {
        let d = classify(&early_burst_then_silence());
        assert_eq!(
            d.verdict,
            Verdict::PermanentPeace,
            "a lie-low duty cycle over a dead predation field is not Alive"
        );
    }

    #[test]
    fn cycling_heterogeneous_reads_alive() {
        let d = classify(&cycling_heterogeneous(&DISPERSED));
        assert!(d.cycled, "boom/bust series must read cycled");
        assert!(d.risk_heterogeneous, "clumped persistent robs must read heterogeneous");
        assert!(d.outcomes_disperse, "spread final wallets must read dispersed");
        assert_eq!(d.verdict, Verdict::Alive);
    }

    #[test]
    fn flat_reads_no_cycle() {
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| s((w + 1) * WINDOW_TICKS, 2, 4, &[2, 0, 0, 0], &[5, 7, 6, 5], &DISPERSED))
            .collect();
        let d = classify(&samples);
        assert!(!d.cycled, "a flat active-pirate series must not read cycled");
        assert_eq!(d.verdict, Verdict::NoCycle);
    }

    #[test]
    fn cycling_equalized_reads_risk_equalized() {
        // Boom/bust, but robs spread evenly over every occupied route each
        // window (HHI 250 milli; ≤ 2 active pirates ⇒ normalized ≤ 500 < 600).
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let boom = w % 2 == 0;
                s(
                    (w + 1) * WINDOW_TICKS,
                    if boom { 1 } else { 2 },
                    if boom { 6 } else { 2 },
                    &[1, 1, 1, 1],
                    &[5, 5, 5, 5],
                    &DISPERSED,
                )
            })
            .collect();
        let d = classify(&samples);
        assert!(d.cycled);
        assert!(!d.risk_heterogeneous, "even spread over occupied routes must not read heterogeneous");
        assert_eq!(d.verdict, Verdict::RiskEqualized);
    }

    #[test]
    fn cycling_hetero_no_dispersion_reads_decision_not_translating() {
        let d = classify(&cycling_heterogeneous(&UNIFORM));
        assert!(d.cycled);
        assert!(d.risk_heterogeneous);
        assert!(!d.outcomes_disperse, "uniform final wallets must not read dispersed");
        assert_eq!(d.verdict, Verdict::DecisionNotTranslating);
    }

    /// The Task-6 sampler wires: `UpgradePurchased` events count into the
    /// per-window purchase fields, and the Yard corp treasury
    /// (`ShipyardCfg.corp_index`) is read at the sample point — the
    /// purchase-desync and Yard-circulation panels' inputs (spec §9).
    #[test]
    fn sample_window_counts_purchases_and_reads_yard_treasury() {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RunConfig, ShipyardCfg, StationInit, SubstepCfg,
            TrophicCfg,
        };
        use crate::contract::Command;
        use crate::math::Vec3;
        use crate::stores::UpgradeKind;
        use crate::time::Dt;
        use crate::types::{CommandKind, EntityRef, Target};
        use crate::world::World;
        let cfg = RunConfig {
            master_seed: 7,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1e-9,
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
                sells_upgrades: true,
            }],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 0, home_station_index: 0 }],
            contracts: vec![],
            price_cfg: PriceCfg::default(),
            dispatch_cfg: DispatchCfg::default(),
            trophic: TrophicCfg::default(),
            shipyard: ShipyardCfg::default(), // corp_index 0 == the only corp
        };
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        world.ships.credits_micros[0] = 50_000_000;
        let buy = |kind| Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::BuyUpgrade { kind },
        };
        world.step(&mut vec![buy(UpgradeKind::Escort)]);
        world.step(&mut vec![buy(UpgradeKind::Hull)]);
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.purchases_escort, 1, "escort purchase counted in the window");
        assert_eq!(s.purchases_hull, 1, "hull purchase counted in the window");
        assert_eq!(
            s.yard_treasury_micros,
            5_000_000 + 8_000_000,
            "Yard treasury read at the sample point (escort L1 + hull L1)"
        );
    }
}
