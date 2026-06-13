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

use crate::autopilot::ARRIVAL_RADIUS;
use crate::contract::{EventKind, StateView};
use crate::economy::Good;
use crate::ids::{BodyId, ContractId, StationId};
use crate::math::Vec3;
use crate::stores::NavState;
use crate::time::Tick;
use crate::types::{EntityRef, NavDest};
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
///
/// Fitted against the 2026-06-11 labeled real-run set (filigree
/// jumpgate-50c6a8a3bd; 50k ticks, band baseline vs the hungry-roamer
/// disease control `pirate_max_reach_au=999 stay_milli=0 upkeep_per_tick=200
/// grubstake_micros=2000000000`), measured mean normalized HHI (milli):
///   TRUE-clumped   (baseline s7/s23/s42/s99): 3070, 2962, 2918, 3498
///   TRUE-equalized (control  s7/s23):         1490, 1472
/// Threshold = margin midpoint (2918 + 1490) / 2 = 2204. (The previous 600
/// sat far below the real boundary: BOTH labels passed it, so this clause
/// never discriminated; the run-aggregate HHI was also measured and does NOT
/// separate — raw 130–153 vs 123–143, normalized 548–714 vs 551–629.)
///
/// RE-FIT ATTEMPT 2026-06-12 (world-gets-big phase 3.2, dual-map labeled-run
/// method after the haven-lurk-leak fix): NOT ADOPTED because the margin
/// closed. Same fit seeds 7/23/42/99, held-out 11/31/57/101, 50k ticks, with
/// baseline as the clumped label and the repaired hungry-roamer control
/// (`pirate_max_reach_au=999 stay_milli=0 upkeep_per_tick=200
/// grubstake_micros=2000000000 engage_radius_au=0.05`) as the equalized
/// label. Measured HHI (milli):
///   trophic  clumped 910..3715 vs equalized 1009..1206 -> CLOSED
///   frontier clumped 4512..7646 vs equalized 2732..3311 -> OPEN
///   pooled candidate threshold 2110 -> CLOSED (min clumped 910 <= max eq 3311)
/// Therefore the literal remains 2204 and the closed fit is the deferred
/// trigger for scenario-conditional thresholds or a revised instrument, not a
/// constant move.
pub const HHI_NORM_MIN_MILLI: u64 = 2204;

/// Slack (in argmax-change counts) granted to the hot-route persistence
/// clause: heterogeneous requires `hot_changes <= traffic_changes +
/// HOT_PERSISTENCE_SLACK_CHANGES`. At 1–3 robs/window the per-window rob
/// argmax is sampling noise between genuinely-hot routes, so zero slack
/// misreads sparse clumping (the seed-7 caught lie: 12 hot changes vs 11
/// traffic changes read equalized while robs sat on 9 of 36 routes).
/// Fitted against the same labeled set, measured hot-change excess over
/// traffic changes:
///   TRUE-clumped:   +1, -1, -3, -6  (max +1, the seed-7 boundary)
///   TRUE-equalized: +6, +5          (min +5)
/// Slack = margin midpoint (1 + 5) / 2 = 3.
///
/// RE-FIT ATTEMPT 2026-06-12 (same phase-3.2 run as above): NOT ADOPTED
/// because the slack margin closed even where frontier HHI separated:
///   trophic  clumped 1..6 vs equalized 3..4 -> CLOSED
///   frontier clumped -1..7 vs equalized 4..10 -> CLOSED
///   pooled candidate slack 5 -> CLOSED (max clumped 7 >= min eq 3)
/// Therefore the literal remains 3 and the overlap is recorded for the same
/// deferred scenario-conditional-threshold / revised-instrument trigger.
pub const HOT_PERSISTENCE_SLACK_CHANGES: u32 = 3;

/// Minimum final-window per-craft credit spread (milli of the largest
/// magnitude) for "outcomes disperse".
pub const OUTCOME_DISPERSION_MIN_MILLI: u64 = 200;

/// FLOOR-rounded fixed-point read `floor(num / den * 1000)`, clamped to `u32`.
/// This is the one f64-to-integer seam for fuel instruments and META radii in
/// world-gets-big phase 0b. Non-finite inputs, non-positive denominators, and
/// negative results read the 0 sentinel. Diagnostics-only: never behavior input,
/// never hashed.
pub fn permille_floor(num: f64, den: f64) -> u32 {
    if den <= 0.0 || !num.is_finite() {
        return 0;
    }
    let v = (num / den * 1000.0).floor();
    if v <= 0.0 {
        0
    } else if v >= u32::MAX as f64 {
        u32::MAX
    } else {
        v as u32
    }
}

/// UNHASHED per-craft fuel diagnostics (world-gets-big phase 0b). Written by
/// `World::step`, read only by `sample_window`, never a behavior input and
/// never hashed. Reset-sized to `n_craft`; craft rows never mint mid-run.
#[derive(Clone, Debug, Default)]
pub struct FuelDiag {
    /// Ticks with a live burn (`fuel_consumed > 0`), run-cumulative.
    pub thrust_ticks: Vec<u64>,
    /// Propellant mass burned, run-cumulative.
    pub burned_mass: Vec<f64>,
    /// Tank low-water mark over the run.
    pub min_fuel_mass: Vec<f64>,
    /// Tank at the open of the craft's current contract leg.
    pub leg_start_fuel: Vec<Option<f64>>,
    /// `(close_tick, burn as permille-of-capacity, FLOOR)` per completed leg.
    pub leg_burns: Vec<(Tick, u32)>,
}

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
    /// Exchange corp treasury at the sample point (OD-2 standing drain read,
    /// goods-as-goods rung A). Out-of-range index reads 0 (Exchange inactive or
    /// absent — also a valid read: "Exchange is not live this run").
    pub exchange_treasury_micros: i64,
    /// Per-craft wallet snapshot at the sample point (dense row order).
    pub per_craft_credits: Vec<i64>,
    /// Trip-phase (fraction-of-trip-elapsed × 1000) of each engagement in the
    /// window — the endpoint-ambush histogram input (Task 4 pushes these).
    pub engagement_phase_milli: Vec<u32>,
    // --- media lab fields (media rung cut 1, spec §9; windows, not gates).
    // Additive: every pre-media JSONL key is untouched. ---
    /// `AlertBorn` events in the window.
    pub gossip_born: u32,
    /// Craft-carrier `GossipHeard` in the window — the propagation signal.
    /// Pier deposits are draw-free, so Station hearings deliberately do NOT
    /// count here.
    pub gossip_first_heard: u32,
    /// Run-cumulative alerts born (pure read over `recent_events(Tick(0))`).
    pub gossip_born_cum: u32,
    /// Run-cumulative distinct `alert_seq`s with ≥ 1 Craft-carrier hearing
    /// (pure read over `recent_events(Tick(0))`) — the escape numerator.
    pub gossip_escaped_cum: u32,
    /// Σ occupied craft comms-log slots at the sample point.
    pub alerts_carried: u32,
    /// Station reservoirs holding ≥ 1 alert at the sample point.
    pub stations_with_news: u32,
    /// Occupied reservoir slots per station (the news-desert map; empty when
    /// media is off — the vectors size to the reservoirs).
    pub per_station_alerts: Vec<u32>,
    /// Dock EDGES per station in the window (`media_diag.contacts`) — the
    /// P(escape) denominator.
    pub per_station_contacts: Vec<u32>,
    /// Per Craft-carrier first-hearing in the window: `e.tick − rob_tick`
    /// (news age at hearing — the knowledge-front input).
    pub heard_lag_ticks: Vec<u32>,
    /// Per Craft-carrier first-hearing in the window: hops at hearing.
    pub heard_hops: Vec<u32>,
    /// The `media_diag.evictions` snapshot at the sample point (run-cumulative).
    pub alerts_evicted_cum: u64,
    /// `assign_diag.decisions` snapshot (run-cumulative): belief-scored picks.
    pub assign_decisions_cum: u64,
    /// `assign_diag.flips` snapshot (run-cumulative): picks where the
    /// gossip read and the legacy-ring read disagree on the argmax
    /// (media-live only) — the WHY-panel channel-liveness window.
    pub assign_flips_cum: u64,
    /// `assign_diag.candidate_counts` snapshot (run-cumulative): evidence
    /// count per scored candidate, buckets 0..=5 then >=6 (the clamp region).
    pub assign_counts_cum: Vec<u64>,
    // --- fuel lab fields (world-gets-big phase 0b, spec §8; windows, not
    // gates). Additive: every pre-fuel JSONL key above is untouched. ---
    /// `CraftRole::rank()` per craft at the sample point (dense row order).
    pub per_craft_role: Vec<u32>,
    /// `fuel_diag.thrust_ticks` snapshot, run-cumulative.
    pub per_craft_thrust_ticks: Vec<u64>,
    /// `fuel_diag.burned_mass` as milli of effective fuel capacity, FLOOR.
    pub per_craft_burn_milli: Vec<u32>,
    /// `fuel_diag.min_fuel_mass` as permille of effective fuel capacity, FLOOR.
    pub per_craft_min_tank_permille: Vec<u32>,
    /// Burn of each completed contract leg in the window, permille of capacity.
    pub leg_burn_permille: Vec<u32>,
    // -- world-gets-big lab fields (phase 2; TROPHIC-C2) -- ADDITIVE: every
    // pre-frontier JSONL key above is byte-untouched. All integers: samples
    // are hash-adjacent evidence, never float analytics.
    /// Settled lurkers per dense station row: active pirates whose nav-derived
    /// lurk is this station and whose position is inside the engagement
    /// envelope of the station body at the sample tick.
    pub per_station_lurking_pirates: Vec<u32>,
    /// Active pirates with no settled lurk, plus lying-low pirates still
    /// commuting to the haven.
    pub pirates_commuting: u32,
    /// Lying-low pirates arrived at the hideout body.
    pub pirates_at_haven: u32,
    /// Station fuel-side cargo book at the sample point: traded Fuel, not
    /// craft propellant.
    pub per_station_fuel_stock: Vec<i64>,
    pub per_station_fuel_price: Vec<i64>,
    // --- goods-as-goods lab fields (rung A, A0; additive — every pre-goods
    // JSONL key above is byte-untouched). per_station_fuel_stock/price remain
    // the scalar fuel columns; these flat matrices carry ALL resources. ---
    /// Per-station stock at the sample point: `[station_row][resource_index]`.
    /// Sized n_stations × N_RESOURCES. Fuel column equals per_station_fuel_stock.
    pub per_station_stock: Vec<Vec<i64>>,
    /// Per-station price_micros at the sample point: `[station_row][resource_index]`.
    /// Sized n_stations × N_RESOURCES. Fuel column equals per_station_fuel_price.
    pub per_station_price: Vec<Vec<i64>>,
    /// Windowed `Refueled` event reads.
    pub refuels: u32,
    pub refuel_units: u64,
    pub refuel_spend_micros: i64,
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
/// route must move slower than laden traffic does"), within
/// `HOT_PERSISTENCE_SLACK_CHANGES` (sparse windows make the rob argmax noisy
/// between genuinely-hot routes; see the constants' fitted-calibration notes).
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
    mean_norm_milli >= u128::from(HHI_NORM_MIN_MILLI)
        && hot_changes <= traffic_changes.saturating_add(HOT_PERSISTENCE_SLACK_CHANGES)
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

// ---- media classifier (media rung cut 1, spec §9) ----

/// DIAGNOSTIC WINDOWS, NOT GATES (PDR-0006): a named row of the media
/// propagation-reading matrix. A separate pure classifier beside `classify`
/// (`Verdict`/`classify()`/the RESULT line are byte-untouched).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaReading {
    /// No alerts born all run (media off, or a world with no robberies).
    NoMedia,
    /// Alerts born but ZERO Craft-carrier hearings all run — news never
    /// escapes the piers. Operationalized correction to spec §9's "zero
    /// hops≥1 hearings": pier deposits are draw-free hops-1 inserts, so the
    /// meaningful propagation zero is CRAFT hearings (M-DEAD, sig forced 0,
    /// must read NewsDesert).
    NewsDesert,
    /// Hearings present in the first half, ZERO across the second, while
    /// robs CONTINUE and stale copies still ride in buffers — the network
    /// went deaf over a live predation field (the PermanentPeace analogue).
    /// Operationalized correction to spec §9's "births first half, zero
    /// second": births ≡ robs by construction, so the dead coupling is
    /// HEARINGS dying under continuing robs.
    StaleEcho,
    /// Run-aggregate escape ≥ `COMMON_KNOWLEDGE_ESCAPE_MILLI` AND the final
    /// window's coverage is complete — the self-averaging alarm (locality
    /// is what the cut is buying; everyone-knows-everything destroys it).
    CommonKnowledge,
    /// The alive reading: news propagates, stays partial, stays local.
    Localized,
}

/// CommonKnowledge escape threshold (milli). DIAGNOSTIC WINDOW, NOT A GATE
/// (PDR-0006): a readout band for the console loop, free to move there.
pub const COMMON_KNOWLEDGE_ESCAPE_MILLI: u32 = 950;

/// Run-aggregate escape fraction in milli: `1000 × escaped_cum / born_cum`
/// read at the LAST sample (the fields are run-cumulative). 0 sentinel when
/// nothing was born (spec §9).
pub fn escaped_milli(samples: &[TrophicSample]) -> u32 {
    let Some(last) = samples.last() else {
        return 0;
    };
    if last.gossip_born_cum == 0 {
        return 0;
    }
    (u64::from(last.gossip_escaped_cum).saturating_mul(1000)
        / u64::from(last.gossip_born_cum)) as u32
}

/// Classify a windowed run's MEDIA propagation field (spec §9). Pure over the
/// samples, like `classify`; precedence is the listed reading order.
/// `endpoint_rows` is the CommonKnowledge coverage denominator: news must
/// reach every contract-endpoint station, not structurally-dark stations that
/// host no contract endpoint.
pub fn media_classify(samples: &[TrophicSample], endpoint_rows: &[bool]) -> MediaReading {
    let born: u64 = samples.iter().map(|s| u64::from(s.gossip_born)).sum();
    if born == 0 {
        return MediaReading::NoMedia;
    }
    let heard: u64 = samples.iter().map(|s| u64::from(s.gossip_first_heard)).sum();
    if heard == 0 {
        return MediaReading::NewsDesert;
    }
    // StaleEcho: hearings died in the second half (heard > 0 overall, so they
    // were in the first) while robs continued AND the mean held-alert count
    // over the second half stayed ≥ 1 (stale copies echo in buffers). The
    // quiet-but-alive trap (robs also stopped) falls through to Localized.
    let mid = samples.len() / 2;
    let late = &samples[mid..];
    if mid >= 1 {
        let late_heard: u64 = late.iter().map(|s| u64::from(s.gossip_first_heard)).sum();
        let late_robs: u64 = late.iter().map(|s| u64::from(s.robs)).sum();
        let late_carried: u64 = late.iter().map(|s| u64::from(s.alerts_carried)).sum();
        if late_heard == 0 && late_robs > 0 && late_carried >= late.len() as u64 {
            return MediaReading::StaleEcho;
        }
    }
    let last = samples.last().expect("non-empty: born > 0");
    let endpoints: Vec<usize> = endpoint_rows
        .iter()
        .enumerate()
        .filter_map(|(i, &e)| e.then_some(i))
        .collect();
    if escaped_milli(samples) >= COMMON_KNOWLEDGE_ESCAPE_MILLI
        && !last.per_station_alerts.is_empty()
        && !endpoints.is_empty()
        && endpoints
            .iter()
            .all(|&i| last.per_station_alerts.get(i).copied().unwrap_or(0) > 0)
    {
        return MediaReading::CommonKnowledge;
    }
    MediaReading::Localized
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
    let mut gossip_born: u32 = 0;
    let mut gossip_first_heard: u32 = 0;
    let mut refuels: u32 = 0;
    let mut refuel_units: u64 = 0;
    let mut refuel_spend_micros: i64 = 0;
    let mut heard_lag_ticks: Vec<u32> = Vec::new();
    let mut heard_hops: Vec<u32> = Vec::new();
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
            EventKind::Refueled { units, price_micros, .. } => {
                refuels = refuels.saturating_add(1);
                refuel_units = refuel_units.saturating_add(units.max(0) as u64);
                refuel_spend_micros =
                    refuel_spend_micros.saturating_add(units.saturating_mul(price_micros));
            }
            EventKind::AlertBorn { .. } => {
                gossip_born = gossip_born.saturating_add(1);
            }
            // Craft-carrier hearings ONLY (the propagation signal): pier
            // deposits are draw-free Station inserts and deliberately do not
            // count. Lag = news age at hearing (the knowledge front).
            EventKind::GossipHeard {
                carrier: crate::media::GossipNode::Craft(_),
                rob_tick,
                hops,
                ..
            } => {
                gossip_first_heard = gossip_first_heard.saturating_add(1);
                heard_lag_ticks.push(
                    u32::try_from(e.tick.0.saturating_sub(rob_tick.0)).unwrap_or(u32::MAX),
                );
                heard_hops.push(u32::from(hops));
            }
            _ => {}
        }
    }
    // Run-cumulative media reads (pure scan over the whole retained stream):
    // total alerts born + distinct alert_seqs with ≥ 1 Craft-carrier hearing.
    let mut gossip_born_cum: u32 = 0;
    let mut escaped_seqs = std::collections::BTreeSet::new();
    for e in world.recent_events(Tick(0)) {
        match e.kind {
            EventKind::AlertBorn { .. } => {
                gossip_born_cum = gossip_born_cum.saturating_add(1);
            }
            EventKind::GossipHeard {
                carrier: crate::media::GossipNode::Craft(_),
                alert_seq,
                ..
            } => {
                escaped_seqs.insert(alert_seq);
            }
            _ => {}
        }
    }
    // Buffer snapshots at the sample point + windowed dock-edge counts.
    let alerts_carried = world
        .ships
        .gossip
        .iter()
        .flatten()
        .map(crate::media::GossipBuffer::occupied)
        .fold(0u32, u32::saturating_add);
    let per_station_alerts: Vec<u32> =
        world.station_gossip.iter().map(crate::media::GossipBuffer::occupied).collect();
    let stations_with_news = per_station_alerts.iter().filter(|&&n| n > 0).count() as u32;
    let mut per_station_contacts = vec![0u32; world.station_gossip.len()];
    for &(t, srow) in &world.media_diag.contacts {
        if t.0 > window_start.0
            && t.0 <= tick.0
            && let Some(c) = per_station_contacts.get_mut(srow as usize)
        {
            *c = c.saturating_add(1);
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
    // World-gets-big pirate-location partition (TROPHIC-C2): nav-derived lurk
    // plus geometry at the sample tick. Pure read; stale config/id reads
    // degrade to "commuting" so the partition stays total.
    let trophic = world.trophic_cfg();
    let station_pos_now: Vec<Option<Vec3>> = (0..n_stations)
        .map(|srow| {
            world
                .stations
                .ids
                .id_at(srow)
                .map(|(slot, generation)| StationId { slot, generation })
                .and_then(|sid| world.station_pos(sid))
        })
        .collect();
    let hideout_pos: Option<Vec3> = world
        .bodies
        .ids
        .id_at(trophic.hideout_body_index as usize)
        .map(|(slot, generation)| BodyId { slot, generation })
        .and_then(|bid| world.body_pos(bid, tick));
    let mut per_station_lurking_pirates = vec![0u32; n_stations];
    let mut pirates_commuting: u32 = 0;
    let mut pirates_at_haven: u32 = 0;
    for r in 0..world.ships.ids.len() {
        let Some(p) = world.ships.pirate[r] else {
            continue;
        };
        if p.lie_low_until > tick {
            let arrived =
                hideout_pos.is_some_and(|hp| world.ships.pos[r].sub(hp).length() <= ARRIVAL_RADIUS);
            if arrived {
                pirates_at_haven = pirates_at_haven.saturating_add(1);
            } else {
                pirates_commuting = pirates_commuting.saturating_add(1);
            }
            continue;
        }

        let nav_lurk: Option<usize> = match world.ships.nav[r] {
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
                (0..n_stations).find(|&s| world.stations.body[s] == b)
            }
            _ => None,
        };
        let settled = nav_lurk.is_some_and(|s| {
            station_pos_now[s]
                .is_some_and(|sp| world.ships.pos[r].sub(sp).length() <= trophic.engage_radius_au)
        });
        match nav_lurk {
            Some(s) if settled => {
                per_station_lurking_pirates[s] =
                    per_station_lurking_pirates[s].saturating_add(1);
            }
            _ => {
                pirates_commuting = pirates_commuting.saturating_add(1);
            }
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
        // OD-2 standing drain read (rung A): the Exchange corp treasury.
        exchange_treasury_micros: world
            .corporations
            .treasury_micros
            .get(world.exchange_cfg().corp_index as usize)
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
        gossip_born,
        gossip_first_heard,
        gossip_born_cum,
        gossip_escaped_cum: escaped_seqs.len() as u32,
        alerts_carried,
        stations_with_news,
        per_station_alerts,
        per_station_contacts,
        heard_lag_ticks,
        heard_hops,
        alerts_evicted_cum: world.media_diag.evictions,
        assign_decisions_cum: world.assign_diag.decisions,
        assign_flips_cum: world.assign_diag.flips,
        assign_counts_cum: world.assign_diag.candidate_counts.to_vec(),
        // Fuel lab fields: pure snapshots of the UNHASHED fuel_diag,
        // integerized through the one permille_floor seam.
        per_craft_role: world.ships.role.iter().map(|r| u32::from(r.rank())).collect(),
        per_craft_thrust_ticks: world.fuel_diag.thrust_ticks.clone(),
        per_craft_burn_milli: (0..world.ships.ids.len())
            .map(|r| {
                let cap =
                    crate::stores::effective_params(&world.ships.spec[r], &world.ships.mods[r])
                        .fuel_capacity;
                permille_floor(world.fuel_diag.burned_mass[r], cap)
            })
            .collect(),
        per_craft_min_tank_permille: (0..world.ships.ids.len())
            .map(|r| {
                let cap =
                    crate::stores::effective_params(&world.ships.spec[r], &world.ships.mods[r])
                        .fuel_capacity;
                permille_floor(world.fuel_diag.min_fuel_mass[r], cap)
            })
            .collect(),
        leg_burn_permille: world
            .fuel_diag
            .leg_burns
            .iter()
            .filter(|(t, _)| t.0 > window_start.0 && t.0 <= tick.0)
            .map(|&(_, p)| p)
            .collect(),
        per_station_lurking_pirates,
        pirates_commuting,
        pirates_at_haven,
        per_station_fuel_stock: world
            .stations
            .stock
            .iter()
            .map(|st| st[Good::FUEL.index()])
            .collect(),
        per_station_fuel_price: world
            .stations
            .price_micros
            .iter()
            .map(|pr| pr[Good::FUEL.index()])
            .collect(),
        per_station_stock: world
            .stations
            .stock
            .iter()
            .map(|st| st.to_vec())
            .collect(),
        per_station_price: world
            .stations
            .price_micros
            .iter()
            .map(|pr| pr.to_vec())
            .collect(),
        refuels,
        refuel_units,
        refuel_spend_micros,
    }
}

/// Directed route index of a contract: `from_row * n_stations + to_row`
/// (dense `n_stations²` layout, matching the per-route vectors). Pub since
/// Task 8: the gossip-log writer (`trophic_run --gossip-log`) joins Robbed /
/// ContractAccepted events onto routes through this same read.
pub fn route_of(world: &World, contract: ContractId) -> Option<usize> {
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

/// Resource and reward for a contract, for the gossip-log `accept` / `deliver`
/// rows (A0, WA2/WA4 joins). Returns `None` when the contract slot/generation
/// is no longer live (stale id); the caller serialises `null` in that case.
/// Pub since A0.2: called from `trophic_run.rs` examples.
pub fn contract_resource_reward(
    world: &World,
    contract: ContractId,
) -> Option<(Good, i64)> {
    let k = world
        .contracts
        .ids
        .dense_index(contract.slot, contract.generation)?;
    Some((world.contracts.resource[k], world.contracts.reward_micros[k]))
}

/// Contract-endpoint station rows derived from the run's own config: row i is
/// `true` iff some seeded contract has it as `from_station_index` or
/// `to_station_index`. This is the scenario-conditional CommonKnowledge
/// coverage denominator.
pub fn endpoint_station_rows(cfg: &crate::config::RunConfig) -> Vec<bool> {
    let mut rows = vec![false; cfg.stations.len()];
    for k in &cfg.contracts {
        if let Some(r) = rows.get_mut(k.from_station_index) {
            *r = true;
        }
        if let Some(r) = rows.get_mut(k.to_station_index) {
            *r = true;
        }
    }
    rows
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
            exchange_treasury_micros: 0,
            per_craft_credits: credits.to_vec(),
            engagement_phase_milli: Vec::new(),
            // Media lab fields default-zero: `classify` never reads them.
            ..Default::default()
        }
    }

    const DISPERSED: [i64; 4] = [10_000_000, 2_000_000, 5_000_000, 1_000_000];
    const UNIFORM: [i64; 4] = [5_000_000, 5_000_000, 5_000_000, 5_000_000];

    /// Boom/bust + robs clumped on route 0 (persistent) while the traffic
    /// gradient's argmax hops every window + dispersed final outcomes.
    /// Actives 2/4 (mean normalized HHI 1000 × 3 = 3000) sit inside the
    /// labeled TRUE-clumped band (2918–3498) above the fitted
    /// `HHI_NORM_MIN_MILLI` = 2204.
    fn cycling_heterogeneous(credits: &[i64]) -> Vec<TrophicSample> {
        (0..12u64)
            .map(|w| {
                let boom = w % 2 == 0;
                let traffic: &[u32] = if boom { &[5, 7, 6, 5] } else { &[7, 5, 6, 5] };
                s(
                    (w + 1) * WINDOW_TICKS,
                    if boom { 2 } else { 4 },
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
        // window (HHI 250 milli; ≤ 2 active pirates ⇒ normalized ≤ 500, far
        // below the fitted HHI_NORM_MIN_MILLI = 2204).
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

    /// The seed-7 shape (2026-06-11 labeled-run recalibration, filigree
    /// jumpgate-50c6a8a3bd): robs clumped on 2 hot routes of 9 occupied at
    /// 1–3 robs/window sparsity. At that sparsity the per-window rob argmax
    /// hops between the hot routes (6 changes here) slightly faster than the
    /// traffic argmax moves (5 changes), so the zero-slack persistence clause
    /// misread eyeball-clumped as equalized — the instrument's second caught
    /// lie. Must read heterogeneous.
    fn sparse_two_hot_of_nine() -> Vec<TrophicSample> {
        (0..12u64)
            .map(|w| {
                // 9 occupied routes; traffic argmax hops between routes 2 and
                // 3 every two windows (5 changes over 12 windows).
                let mut traffic = [1u32; 9];
                traffic[if (w / 2) % 2 == 0 { 2 } else { 3 }] = 3;
                // Robs stay on routes {0, 1}: hot route 0 with periodic hops
                // to route 1 (hot sequence 0,1,0,0 repeating → 6 changes).
                let mut robs = [0u32; 9];
                match w % 4 {
                    0 | 2 => {
                        robs[0] = 2;
                        robs[1] = 1;
                    }
                    1 => robs[1] = 2,
                    _ => robs[0] = 1,
                }
                s((w + 1) * WINDOW_TICKS, 4, 4, &robs, &traffic, &DISPERSED)
            })
            .collect()
    }

    #[test]
    fn sparse_clumped_minority_routes_read_heterogeneous() {
        let d = classify(&sparse_two_hot_of_nine());
        assert!(
            d.risk_heterogeneous,
            "robs clumped on 2 of 9 occupied routes at 1-3 robs/window must \
             read heterogeneous (the seed-7 boundary shape)"
        );
    }

    fn one_craft_vendor_cfg() -> crate::config::RunConfig {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RunConfig, ShipyardCfg, StationInit, SubstepCfg,
            TrophicCfg,
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
                initial_stock: vec![0i64, 0i64],
                initial_price_micros: vec![0i64, 0i64],
                sells_upgrades: true,
            }],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 0, home_station_index: 0, arb_premium_micros: 0 }],
            contracts: vec![],
            price_cfg: PriceCfg::default(),
            dispatch_cfg: DispatchCfg::default(),
            trophic: TrophicCfg::default(),
            shipyard: ShipyardCfg::default(), // corp_index 0 == the only corp
            media: crate::config::MediaCfg::default(),
            refuel: crate::config::RefuelCfg::default(),
            goods: crate::config::GoodsCfg::default(),
            exchange: crate::config::ExchangeCfg::default(),
            arbitrage: crate::config::ArbitrageCfg::default(),
        }
    }

    /// The Task-6 sampler wires: `UpgradePurchased` events count into the
    /// per-window purchase fields, and the Yard corp treasury
    /// (`ShipyardCfg.corp_index`) is read at the sample point — the
    /// purchase-desync and Yard-circulation panels' inputs (spec §9).
    #[test]
    fn sample_window_counts_purchases_and_reads_yard_treasury() {
        use crate::contract::Command;
        use crate::stores::UpgradeKind;
        use crate::types::{CommandKind, EntityRef, Target};
        use crate::world::World;

        let (mut world, _h) = World::reset(one_craft_vendor_cfg()).expect("resolvable cfg");
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
        // Media lab fields (Task 8) on a media-OFF world: all zero/empty —
        // the vectors size to the reservoirs (none when media is off).
        assert_eq!(s.gossip_born, 0);
        assert_eq!(s.gossip_first_heard, 0);
        assert_eq!(s.gossip_born_cum, 0);
        assert_eq!(s.gossip_escaped_cum, 0);
        assert_eq!(s.alerts_carried, 0);
        assert_eq!(s.stations_with_news, 0);
        assert!(s.per_station_alerts.is_empty(), "no reservoirs when media is off");
        assert!(s.per_station_contacts.is_empty(), "no contacts when media is off");
        assert!(s.heard_lag_ticks.is_empty());
        assert!(s.heard_hops.is_empty());
        assert_eq!(s.alerts_evicted_cum, 0);
    }

    /// Phase-0b fuel lab fields: synthetic FuelDiag -> sample_window integer
    /// reads. Values choose FLOOR cases and a window filter boundary.
    #[test]
    fn sample_window_reads_fuel_diag_through_the_floor_seam() {
        use crate::world::World;

        let (mut world, _h) = World::reset(one_craft_vendor_cfg()).expect("resolvable cfg");
        let mut cmds = Vec::new();
        world.step(&mut cmds);
        world.step(&mut cmds);

        world.fuel_diag.thrust_ticks[0] = 41;
        world.fuel_diag.burned_mass[0] = 0.5999e-9;
        world.fuel_diag.min_fuel_mass[0] = 0.4001e-9;
        world.fuel_diag.leg_burns = vec![(Tick(1), 77), (Tick(2), 123), (Tick(5000), 200)];

        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_craft_role, vec![0], "role snapshot (Idle rank 0)");
        assert_eq!(s.per_craft_thrust_ticks, vec![41], "duty numerator snapshot");
        assert_eq!(s.per_craft_burn_milli, vec![599], "FLOOR: 599.9 -> 599");
        assert_eq!(s.per_craft_min_tank_permille, vec![400], "FLOOR: 400.1 -> 400");
        assert_eq!(
            s.leg_burn_permille,
            vec![77, 123],
            "window filter: window_start < close_tick <= sample tick only"
        );
    }

    #[test]
    fn sample_window_reads_fuel_book_and_pirate_partition() {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RunConfig, ShipyardCfg, StationInit, SubstepCfg,
            TrophicCfg,
        };
        use crate::math::Vec3;
        use crate::stores::{CraftRole, NavState};
        use crate::time::Dt;
        use crate::world::World;

        fn cfg(hideout: u32) -> RunConfig {
            RunConfig {
                master_seed: 7,
                dt: Dt::new(0.25),
                softening: 1e-3,
                substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
                ephemeris_window: 256,
                bodies: vec![BodyInit {
                    mass: 1e-9,
                    elements: OrbitalElements {
                        a: 0.0,
                        e: 0.0,
                        i: 0.0,
                        raan: 0.0,
                        argp: 0.0,
                        m0: 0.0,
                    },
                }],
                craft: vec![CraftInit {
                    spec: BaseSpec {
                        base_dry_mass: 1e-9,
                        base_max_thrust: 1e-12,
                        base_exhaust_velocity: 1e-2,
                        base_fuel_capacity: 1e-9,
                        base_cargo_capacity: 5,
                    },
                    pos: Vec3::ZERO,
                    vel: Vec3::ZERO,
                    fuel_mass: 1e-9,
                    role: CraftRole::Pirate,
                    scripted: true,
                }],
                guidance: GuidanceParams::default(),
                stations: vec![StationInit {
                    body_index: 0,
                    initial_stock: vec![3i64, 17i64],
                    initial_price_micros: vec![0i64, 5_000i64],
                    sells_upgrades: false,
                }],
                producers: vec![],
                corporations: vec![CorporationInit {
                    treasury_micros: 0,
                    home_station_index: 0,
                    arb_premium_micros: 0,
                }],
                contracts: vec![],
                price_cfg: PriceCfg::default(),
                dispatch_cfg: DispatchCfg::default(),
                trophic: TrophicCfg {
                    engage_radius_au: 5.0e-4,
                    hideout_body_index: hideout,
                    ..TrophicCfg::default()
                },
                shipyard: ShipyardCfg::default(),
                media: crate::config::MediaCfg::default(),
                refuel: crate::config::RefuelCfg::default(),
                goods: crate::config::GoodsCfg::default(),
                exchange: crate::config::ExchangeCfg::default(),
                arbitrage: crate::config::ArbitrageCfg::default(),
            }
        }

        let (world, _h) = World::reset(cfg(99)).expect("resolvable cfg");
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_station_fuel_stock, vec![17], "Fuel-side stock book");
        assert_eq!(s.per_station_fuel_price, vec![5_000], "Fuel-side price book");
        assert_eq!(s.per_station_lurking_pirates, vec![1], "settled lurker at its station");
        assert_eq!(s.pirates_commuting, 0);
        assert_eq!(s.pirates_at_haven, 0);
        assert_eq!(s.refuels, 0, "no Refueled events on an inert-refuel world");
        assert_eq!(s.refuel_units, 0);
        assert_eq!(s.refuel_spend_micros, 0);
        let lurking: u32 = s.per_station_lurking_pirates.iter().sum();
        assert_eq!(lurking + s.pirates_commuting + s.pirates_at_haven, 1, "partition is total");

        let (mut world, _h) = World::reset(cfg(99)).expect("resolvable cfg");
        world.ships.nav[0] = NavState::Idle;
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_station_lurking_pirates, vec![0]);
        assert_eq!(s.pirates_commuting, 1, "no settled lurk reads as commuting");

        let (mut world, _h) = World::reset(cfg(0)).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().lie_low_until = Tick(10_000);
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.pirates_at_haven, 1, "lying low on the hideout body reads at-haven");
        assert_eq!(s.pirates_commuting, 0);
        assert_eq!(s.per_station_lurking_pirates, vec![0], "a refugee is not a lurker");
    }

    #[test]
    fn sample_window_counts_refuels() {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RefuelCfg, RunConfig, ShipyardCfg, StationInit,
            SubstepCfg, TrophicCfg,
        };
        use crate::math::Vec3;
        use crate::stores::CraftRole;
        use crate::time::Dt;
        use crate::world::World;

        let cfg = RunConfig {
            master_seed: 7,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1e-9,
                elements: OrbitalElements {
                    a: 0.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-9,
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2,
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::ZERO,
                vel: Vec3::ZERO,
                fuel_mass: 2.5e-10,
                role: CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![StationInit {
                body_index: 0,
                initial_stock: vec![0i64, 10i64],
                initial_price_micros: vec![0i64, 5_000i64],
                sells_upgrades: false,
            }],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 0, home_station_index: 0, arb_premium_micros: 0 }],
            contracts: vec![],
            price_cfg: PriceCfg {
                base_micros: vec![0i64, 5_000i64],
                cap: vec![0i64, 40i64],
                slope_milli: 1800,
                reprice_interval: 1,
            },
            dispatch_cfg: DispatchCfg::default(),
            trophic: TrophicCfg::default(),
            shipyard: ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 },
            goods: crate::config::GoodsCfg::default(),
            exchange: crate::config::ExchangeCfg::default(),
            arbitrage: crate::config::ArbitrageCfg::default(),
        };
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.step(&mut Vec::new());
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.refuels, 1, "one Refueled event in the window");
        assert_eq!(s.refuel_units, 3, "units = min(need 3, stock 10, afford 200)");
        assert_eq!(s.refuel_spend_micros, 15_000, "3 units x seeded 5_000 micros");
        assert_eq!(s.per_station_fuel_stock, vec![7], "stock book debited by the purchase");
    }

    /// Phase-0b FLOOR pin (world-gets-big spec §7/§8): no f64-to-fixed-point
    /// precedent existed in-tree. This test pins the form for every fuel
    /// instrument and the phase-1 `tank_before_permille`. FLOOR, never round.
    #[test]
    fn permille_floor_is_floor_never_round() {
        assert_eq!(permille_floor(0.35, 1.0), 350, "milli-AU radius read");
        assert_eq!(permille_floor(1.999, 1000.0), 1, "FLOOR, never round-half-up");
        assert_eq!(permille_floor(0.9999999, 1.0), 999, "sub-unit stays below 1000");
        assert_eq!(permille_floor(1.0, 1.0), 1000, "exact full tank reads 1000");
        assert_eq!(permille_floor(1.0, 0.0), 0, "zero denominator reads the 0 sentinel");
        assert_eq!(permille_floor(1.0, -1.0), 0, "negative denominator reads 0");
        assert_eq!(permille_floor(-1.0, 1.0), 0, "negative numerator clamps to 0");
        assert_eq!(permille_floor(f64::NAN, 1.0), 0, "non-finite reads the 0 sentinel");
    }

    // ---- media lab bench (Task 8): labeled synthetics, one per
    // `MediaReading`, plus the quiet-but-alive StaleEcho trap (the seed-7
    // rule: every new metric ships with a synthetic that would catch it
    // lying) ----

    /// Media-field synthetic: everything non-media defaulted except `robs`
    /// (StaleEcho's coupling term).
    #[allow(clippy::too_many_arguments)]
    fn m(
        tick: u64,
        born: u32,
        heard: u32,
        robs: u32,
        carried: u32,
        born_cum: u32,
        escaped_cum: u32,
        station_alerts: &[u32],
    ) -> TrophicSample {
        TrophicSample {
            tick,
            robs,
            gossip_born: born,
            gossip_first_heard: heard,
            gossip_born_cum: born_cum,
            gossip_escaped_cum: escaped_cum,
            alerts_carried: carried,
            stations_with_news: station_alerts.iter().filter(|&&n| n > 0).count() as u32,
            per_station_alerts: station_alerts.to_vec(),
            ..Default::default()
        }
    }

    #[test]
    fn media_no_media_when_nothing_born() {
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| m((w + 1) * WINDOW_TICKS, 0, 0, 2, 0, 0, 0, &[0, 0, 0]))
            .collect();
        assert_eq!(media_classify(&samples, &[true, true, true]), MediaReading::NoMedia);
    }

    #[test]
    fn media_news_desert_when_no_craft_hearings() {
        // M-DEAD (sig forced 0) must read NewsDesert: alerts are born (pier
        // deposits are draw-free hops-1 inserts) but NO craft ever hears one
        // — the meaningful propagation zero is CRAFT hearings.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                m((w + 1) * WINDOW_TICKS, 1, 0, 1, 0, (w + 1) as u32, 0, &[1, 0, 0])
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::NewsDesert
        );
    }

    #[test]
    fn media_stale_echo_when_network_goes_deaf_under_continuing_robs() {
        // Hearings present in the first half, ZERO across the second, while
        // robs CONTINUE and stale copies still sit in buffers — the network
        // went deaf over a live predation field (the PermanentPeace analogue).
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let heard = if w < 6 { 2 } else { 0 };
                m((w + 1) * WINDOW_TICKS, 1, heard, 1, 3, (w + 1) as u32, 4, &[2, 1, 0])
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::StaleEcho
        );
    }

    #[test]
    fn media_stale_echo_trap_quiet_but_alive_reads_localized() {
        // The seed-7 rule's trap: hearings drop to zero in the second half
        // but robs ALSO stop — a peaceful world, not a deaf network. An
        // instrument that cries wolf over peace is lying: must read
        // Localized, NOT StaleEcho.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let (born, heard, robs) = if w < 6 { (1, 2, 1) } else { (0, 0, 0) };
                let cum = (w + 1).min(6) as u32;
                m((w + 1) * WINDOW_TICKS, born, heard, robs, 3, cum, 4, &[2, 1, 0])
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::Localized
        );
    }

    #[test]
    fn media_common_knowledge_when_escape_saturates_and_coverage_completes() {
        // Run-aggregate escape 23/24 = 958‰ ≥ 950 at the last sample AND the
        // final window's coverage is complete (every station holds news) —
        // the self-averaging alarm.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                let escaped_cum = born_cum.saturating_sub(1);
                m((w + 1) * WINDOW_TICKS, 2, 3, 1, 6, born_cum, escaped_cum, &[2, 1, 1])
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::CommonKnowledge
        );
    }

    #[test]
    fn media_localized_is_the_alive_reading() {
        // Propagation alive in both halves, escape below saturation, coverage
        // incomplete: the reading the cut is aiming for.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                m((w + 1) * WINDOW_TICKS, 1, 1, 1, 2, (w + 1) as u32, 6, &[2, 0, 0])
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::Localized
        );
    }

    #[test]
    fn media_common_knowledge_denominates_on_contract_endpoint_stations() {
        // Frontier shape: a dark station (no contract endpoint) never holds
        // news; coverage must be satisfiable over the endpoint set.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 0],
                )
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::Localized,
            "dark row counted -> coverage unsatisfiable"
        );
        assert_eq!(
            media_classify(&samples, &[true, true, false]),
            MediaReading::CommonKnowledge,
            "endpoint coverage complete -> CommonKnowledge"
        );
    }

    #[test]
    fn media_empty_endpoint_set_never_reads_common_knowledge() {
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 1],
                )
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[false, false, false]),
            MediaReading::Localized
        );
    }

    #[test]
    fn endpoint_station_rows_trophic_is_all_true() {
        let cfg = crate::scenario::scenario_trophic(7);
        assert_eq!(endpoint_station_rows(&cfg), vec![true; 6]);
    }

    #[test]
    fn endpoint_station_rows_marks_a_contractless_station_dark() {
        let mut cfg = crate::scenario::scenario_trophic(7);
        cfg.stations.push(crate::config::StationInit {
            body_index: 1,
            initial_stock: vec![0i64, 0i64],
            initial_price_micros: vec![0i64, 0i64],
            sells_upgrades: false,
        });
        let rows = endpoint_station_rows(&cfg);
        assert_eq!(rows.len(), 7);
        assert!(!rows[6], "no contract touches the new station -> dark");
    }

    #[test]
    fn sample_window_has_per_station_stock_and_price_matrices() {
        use crate::{scenario_trophic, World};
        let (world, _) = World::reset(scenario_trophic(7)).expect("reset");
        let s = sample_window(&world, crate::time::Tick(0));
        // After A0.1 these fields exist and have n_stations entries.
        let n = world.stations.ids.len();
        assert_eq!(s.per_station_stock.len(), n,
            "per_station_stock: one row per station");
        assert_eq!(s.per_station_price.len(), n,
            "per_station_price: one row per station");
        // Each row covers all resources (N_RESOURCES columns today).
        // Fuel column must equal the existing per_station_fuel_stock scalar.
        let fuel_r = crate::economy::Good::FUEL.index();
        for (row, stock) in s.per_station_stock.iter().enumerate() {
            assert_eq!(stock[fuel_r], s.per_station_fuel_stock[row],
                "per_station_stock fuel column matches existing fuel scalar at row {row}");
        }
        for (row, price) in s.per_station_price.iter().enumerate() {
            assert_eq!(price[fuel_r], s.per_station_fuel_price[row],
                "per_station_price fuel column matches existing fuel scalar at row {row}");
        }
    }
}
