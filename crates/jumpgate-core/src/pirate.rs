//! Pirate predation systems (pirates rung 1, Commits D + E — spec §§2-5, §7).
//!
//! One pre-physics brain stage and two deterministic post-physics tick stages
//! over hashed integer state:
//!
//! * **Stage 1c2 `run_pirate_brains`** — the bounded DUMB lurker (spec §5):
//!   lie-low routing to the hideout, staggered reach-bounded relocation
//!   (uniform-in-reach, NEVER traffic-weighted — `relocate_lurk_target`'s
//!   signature admits geometry only), and a loiter re-seek strictly inside the
//!   engagement envelope. Scripted stage: skips `!scripted` craft.
//! * **Stage 3b2 `resolve_encounters`** — choke-point engagements (NOT chase):
//!   each active pirate engages the NEAREST eligible laden hauler inside the
//!   `(engage_radius_au, engage_speed)` envelope, resolved by ONE seeded roll on
//!   `RngStream::Piracy` (its FIRST runtime consumer; draws at exactly this
//!   stage, dense pirate-row order). Runs AFTER `resolve_deliveries` (3b) —
//!   a same-tick Arrival settles the delivery first, so the DESTINATION dock is
//!   sanctuary by ordering — and BEFORE `resolve_failures` (3c), extending the
//!   proven 3b-before-3c ordering precedent. Engagement at the ORIGIN dock
//!   (rob-on-load / departure ambush) is legal and is the headline behavior.
//! * **Stage 3b3 `update_pirate_population`** — food/heat lifecycle (population
//!   without spawn/despawn): upkeep while active, starvation and heat both
//!   force a lie-low refuge off the predation field, notoriety decays
//!   geometrically on a tick-gated interval. Lie-low IS the rung-1 population
//!   dynamic; the active count is the boom/bust variable.
//!
//! Conservation: the robbery settlement adds ZERO new identity legs — cargo is
//! an accounted SINK (`consumed += qty`, the FuelEmpty precedent), the escrow
//! refund and the ransom are pure TRANSFERS (`Σtreasury+Σcredits+Σescrow`
//! invariant). All new strength/credit arithmetic is saturating (spec §8
//! totality discipline).

use crate::autopilot::ARRIVAL_RADIUS;
use crate::config::{CraftInit, MediaCfg, TrophicCfg};
use crate::contract::{Event, EventKind};
use crate::media::{GossipAlert, GossipBuffer, GossipNode, MediaDiag, insert_alert};
use crate::economy::{
    ContractStatus, ContractStore, CorporationStore, EconCounters, FailureCause, StationStore,
    settle_contract_failure,
};
use crate::ephemeris::Ephemeris;
use crate::events::EventStream;
use crate::ids::BodyId;
use crate::math::Vec3;
use crate::rng::{RngStream, RngStreams};
use crate::stores::{BodyStore, CraftRole, CraftStore, NavState, UpgradeLevels, effective_params};
use crate::time::Tick;
use crate::types::{EntityRef, NavDest};
use crate::world::RouteEvidence;
use rand_core::Rng;

/// Per-engagement kinematic snapshot, pushed by BOTH outcome emission sites in
/// stage 3b2 (spec §2: log fraction-of-trip-elapsed and speed per engagement —
/// the owner's pre-registered endpoint-ambush discriminator). UNHASHED
/// diagnostics-only state: read ONLY by the diagnostics sampler
/// (`diagnostics::sample_window`), never by any behavior stage, so it cannot
/// perturb determinism; every field is derived from hashed inputs at the
/// emission site. The relative-bearing + speed pair is also the hit-location
/// data seam for the future part-graph damage model (spec §14.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngagementSnapshot {
    /// Tick of the engagement (the 3b2 stage tick).
    pub tick: Tick,
    /// Elapsed-trip-fraction × 1000 at engagement:
    /// `|h − origin| / (|h − origin| + |h − dest|)` over the contract's station
    /// body positions at the engagement tick. 0 = origin dock (departure
    /// ambush), 1000 = destination dock (deceleration-window ambush).
    pub phase_milli: u32,
    /// Relative bearing in milliradians (0..=3142): the angle between the
    /// hauler→pirate line and the hauler's velocity. 0 = pirate dead ahead
    /// (head-on), π = pirate dead astern (stern chase). 0 when the hauler is
    /// at rest (docked) or co-located.
    pub rel_bearing_millirad: u32,
    /// Relative speed `|vel_p − vel_h|` (AU/day) inside the envelope.
    pub rel_speed: f64,
}

/// Coercion strength (spec §2/§6): `escorts` (the wing SIZE — a count of real
/// un-simulated ships, never a stat) plus the role's base
/// (`pirate_base_strength` for pirates, 0 otherwise). DERIVED at the read
/// site, never stored (the fleet-ledger discipline); saturating per the §8
/// totality discipline. Range [0, 3] under the structural caps.
pub fn strength(role: CraftRole, upgrades: UpgradeLevels, trophic: &TrophicCfg) -> u8 {
    let base = if role == CraftRole::Pirate { trophic.pirate_base_strength } else { 0 };
    upgrades.escorts.saturating_add(base)
}

/// Stage 3b2 — choke-point encounter resolution (spec §2) + ransom robbery
/// settlement (spec §3). See the module doc for ordering and conservation.
///
/// `station_pos` is the station body position at `tick` per dense station row
/// (computed post-physics by the caller; used ONLY for the trip-phase
/// diagnostic, never for the engagement predicate — craft-craft distance needs
/// no body frame).
///
/// `route_evidence` is the spec-§7 evidence store: each Robbed settlement
/// bumps the contract's directed-route ring with the rob tick (the WRITE half
/// of the media seam; the dock-gated READ is `World::route_evidence`).
///
/// Media mint (media rung cut 1, spec §4): a Robbed settlement additionally
/// seeds a `GossipAlert` in the VICTIM's comms-log (hops 0, claimed = contract
/// reward + ransom — the seed-honesty law) and, when the victim is on a pier,
/// deposits a firsthand copy (hops 1) in that station's reservoir. ALL of it
/// gated on `media.caps_live()` (the trophic lever already holds here);
/// media-off worlds are bit-identical. `station_gossip` / `next_alert_seq` /
/// `media_diag` are the world's media state (hashed, hashed, unhashed).
///
/// Inert lever (spec §8): `engage_radius_au <= 0.0` (the default) returns
/// immediately — no scan, no Piracy draw, existing scenarios bit-identical.
#[allow(clippy::too_many_arguments)]
pub fn resolve_encounters(
    ships: &mut CraftStore,
    contracts: &mut ContractStore,
    corporations: &mut CorporationStore,
    counters: &mut EconCounters,
    stations: &StationStore,
    station_pos: &[Vec3],
    route_evidence: &mut RouteEvidence,
    station_gossip: &mut [GossipBuffer],
    next_alert_seq: &mut u32,
    trophic: &TrophicCfg,
    media: &MediaCfg,
    rng: &mut RngStreams,
    tick: Tick,
    events: &mut EventStream,
    diag: &mut Vec<EngagementSnapshot>,
    media_diag: &mut MediaDiag,
) {
    if trophic.engage_radius_au <= 0.0 {
        return;
    }
    for prow in 0..ships.ids.len() {
        // Pirate eligibility (spec §2): role, off lie-low, off engage cooldown.
        if ships.role[prow] != CraftRole::Pirate {
            continue;
        }
        let Some(pstate) = ships.pirate[prow] else {
            continue;
        };
        if tick < pstate.lie_low_until || tick < pstate.engage_cooldown_until {
            continue;
        }
        let s_p = strength(ships.role[prow], ships.upgrades[prow], trophic);

        // NEAREST eligible hauler inside the envelope. Eligibility is re-checked
        // SEQUENTIALLY (state mutated by earlier pirates this stage is visible:
        // a just-robbed hauler is no longer laden, so it cannot be robbed twice
        // in a tick). Distance ties go to the LOWEST dense row (strict `<`).
        let mut best: Option<(usize, f64)> = None;
        for hrow in 0..ships.ids.len() {
            if !hauler_is_eligible(ships, contracts, hrow, s_p, trophic) {
                continue;
            }
            let d = ships.pos[hrow].sub(ships.pos[prow]).length();
            if d > trophic.engage_radius_au {
                continue;
            }
            // A Δv-advantaged hauler under way is out of the envelope —
            // flee-by-physics is preserved.
            if ships.vel[hrow].sub(ships.vel[prow]).length() > trophic.engage_speed {
                continue;
            }
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((hrow, d));
            }
        }
        let Some((hrow, _)) = best else {
            continue;
        };
        // Totality: eligibility guarantees these, but degrade deterministically
        // around an absurd state instead of unwrapping (spec §8).
        let Some(cid) = ships.contract[hrow] else {
            continue;
        };
        let Some(kidx) = contracts.ids.dense_index(cid.slot, cid.generation) else {
            continue;
        };
        let Some((_res, qty)) = ships.cargo[hrow] else {
            continue;
        };
        let pirate_id = ships.ids_at(prow);
        let hauler_id = ships.ids_at(hrow);

        // Kinematic snapshot at the emission site (spec §2/§14.7) — captured
        // BEFORE settlement clears the contract/cargo. Pushed for BOTH outcomes.
        diag.push(EngagementSnapshot {
            tick,
            phase_milli: trip_phase_milli(contracts, stations, station_pos, kidx, ships.pos[hrow]),
            rel_bearing_millirad: rel_bearing_millirad(
                ships.pos[hrow],
                ships.vel[hrow],
                ships.pos[prow],
            ),
            rel_speed: ships.vel[prow].sub(ships.vel[hrow]).length(),
        });

        // ONE engagement per pirate per tick, resolved by ONE Piracy draw:
        // u ∈ [0,1000), Robbed iff u < p_rob_milli (spec §2 outcome resolution).
        let u = (rng.stream(RngStream::Piracy).next_u64() % 1000) as u32;
        let robbed = u < trophic.p_rob_milli;
        if robbed {
            // Evidence-ring bump (spec §7, the WRITE half of the media seam):
            // record the rob tick on the contract's directed-route ring
            // (dense row-major `from_row * n_stations + to_row`). Bounded +
            // saturating — an unresolvable station row degrades to a no-write
            // (spec §8 totality), never a panic. The resolved route is reused
            // by the gossip mint below (spec §8 degrade: unresolvable rows
            // skip the mint too).
            let n_stations = stations.ids.len();
            let from = contracts.from_station[kidx];
            let to = contracts.to_station[kidx];
            let route: Option<usize> = match (
                stations.ids.dense_index(from.slot, from.generation),
                stations.ids.dense_index(to.slot, to.generation),
            ) {
                (Some(f), Some(t)) => Some(f.saturating_mul(n_stations).saturating_add(t)),
                _ => None,
            };
            if let Some(route) = route
                && let (Some(ring), Some(cur)) = (
                    route_evidence.robs.get_mut(route),
                    route_evidence.cursor.get_mut(route),
                )
            {
                let slot = (*cur as usize) % ring.len();
                ring[slot] = tick;
                *cur = ((slot + 1) % ring.len()) as u8;
            }
            // Seed-honesty law (spec §3): the robbed contract's reward is read
            // BEFORE settle_contract_failure tears the contract down — the
            // settlement precedes the ransom computation, and Robbed's
            // value_micros (the wallet-clamped ransom) must NOT seed
            // significance.
            let reward = contracts.reward_micros[kidx];
            // Contract teardown: the generalized resolve_failures settle body
            // (escrow refund TRANSFER, cargo→consumed SINK, hauler released).
            settle_contract_failure(
                contracts,
                corporations,
                ships,
                counters,
                kidx,
                FailureCause::Robbed,
            );
            // Takings: ransom = min(wallet, cap), hauler → pirate wallet — a
            // pure TRANSFER, no new identity leg (spec §3).
            let ransom = ships.credits_micros[hrow]
                .max(0)
                .min(trophic.ransom_cap_micros.max(0));
            ships.credits_micros[hrow] = ships.credits_micros[hrow].saturating_sub(ransom);
            ships.credits_micros[prow] = ships.credits_micros[prow].saturating_add(ransom);
            // Metabolism: robbed cargo feeds the hunger clock; heat accrues.
            if let Some(p) = ships.pirate[prow].as_mut() {
                p.food_micros = p
                    .food_micros
                    .saturating_add((qty as i64).saturating_mul(trophic.food_per_unit_micros));
                p.notoriety = p.notoriety.saturating_add(trophic.notoriety_per_rob);
            }
            events.emit(Event {
                tick,
                kind: EventKind::Robbed {
                    pirate: pirate_id,
                    hauler: hauler_id,
                    contract: cid,
                    value_micros: ransom,
                },
            });
            // Media mint (spec §4): the victim is the index case. ALL gated on
            // `caps_live()` (the trophic lever already holds here); an
            // unresolvable route or a buffer-less victim skips the mint
            // entirely (spec §8 degrade). No draws anywhere on this path.
            if media.caps_live()
                && let Some(route) = route
                && ships.gossip[hrow].is_some()
            {
                let seq = *next_alert_seq;
                *next_alert_seq = next_alert_seq.wrapping_add(1);
                // Seed honesty (spec §3): claimed = the victim's true loss in
                // hand — robbed contract reward + ransom actually paid.
                let claimed = reward.saturating_add(ransom);
                let seed = GossipAlert {
                    alert_seq: seq,
                    route: route as u32,
                    pirate_slot: pirate_id.slot,
                    rob_tick: tick,
                    claimed_value_micros: claimed,
                    first_heard: tick,
                    hops: 0,
                };
                // Victim's own copy: cover == truth at hop 0. The pirate gets
                // no copy; NO GossipHeard for the seed (Robbed tells it).
                if let Some(buf) = ships.gossip[hrow].as_mut() {
                    insert_alert(
                        buf,
                        seed,
                        tick,
                        trophic.evidence_window,
                        media,
                        &mut media_diag.evictions,
                    );
                }
                // Origin-pier deposit (spec §4.3): a victim robbed on a pier
                // (rob-on-load, the dominant robbery class) deposits a
                // firsthand copy — hops 1, NO inflation — into that station's
                // reservoir the same tick. Lowest station row in radius wins.
                if let Some(srow) = (0..station_pos.len())
                    .find(|&s| ships.pos[hrow].sub(station_pos[s]).length() <= ARRIVAL_RADIUS)
                    && let Some(sbuf) = station_gossip.get_mut(srow)
                {
                    let copy = GossipAlert { hops: 1, ..seed };
                    if insert_alert(
                        sbuf,
                        copy,
                        tick,
                        trophic.evidence_window,
                        media,
                        &mut media_diag.evictions,
                    ) && let Some((slot, generation)) = stations.ids.id_at(srow)
                    {
                        events.emit(Event {
                            tick,
                            kind: EventKind::GossipHeard {
                                carrier: GossipNode::Station(crate::ids::StationId {
                                    slot,
                                    generation,
                                }),
                                alert_seq: copy.alert_seq,
                                route: copy.route,
                                pirate_slot: copy.pirate_slot,
                                claimed_value_micros: copy.claimed_value_micros,
                                hops: copy.hops,
                                rob_tick: copy.rob_tick,
                            },
                        });
                    }
                }
                // The truth join (spec §8): captured at the only moment
                // event↔route↔subjects are simultaneously resolvable.
                events.emit(Event {
                    tick,
                    kind: EventKind::AlertBorn {
                        alert_seq: seq,
                        route: route as u32,
                        pirate: pirate_id,
                        hauler: hauler_id,
                        truth_value_micros: claimed,
                        claimed_value_micros: claimed,
                    },
                });
            }
        } else {
            // The hauler slips away / the bluff fails: no settlement, no
            // transfer, no heat — just the cooldown below.
            events.emit(Event {
                tick,
                kind: EventKind::DrivenOff { pirate: pirate_id, hauler: hauler_id },
            });
        }
        // Either outcome digests: no per-tick re-rolls (spec §2 step 3).
        let cooldown = if robbed { trophic.rob_cooldown } else { trophic.driveoff_cooldown };
        if let Some(p) = ships.pirate[prow].as_mut() {
            p.engage_cooldown_until = Tick(tick.0.saturating_add(cooldown));
        }
    }
}

/// Hauler-side engagement eligibility (spec §2): a laden hauler on a live
/// (CargoLoaded | InTransit — both hold escrow, the one-tick CargoLoaded
/// window is robbable) contract, STRICTLY weaker than the pirate. Ties go to
/// the DEFENDER, deterministically, no draw — pirates do not start fights they
/// have already lost (the tether-coercion logic: the threat must be credible).
/// Escort level is visible evidence (a wing is an observable physical
/// configuration, not a valuation).
fn hauler_is_eligible(
    ships: &CraftStore,
    contracts: &ContractStore,
    hrow: usize,
    pirate_strength: u8,
    trophic: &TrophicCfg,
) -> bool {
    if ships.role[hrow] != CraftRole::Hauler || ships.cargo[hrow].is_none() {
        return false;
    }
    let Some(cid) = ships.contract[hrow] else {
        return false;
    };
    let Some(kidx) = contracts.ids.dense_index(cid.slot, cid.generation) else {
        return false;
    };
    if !matches!(
        contracts.status[kidx],
        ContractStatus::CargoLoaded | ContractStatus::InTransit
    ) {
        return false;
    }
    strength(ships.role[hrow], ships.upgrades[hrow], trophic) < pirate_strength
}

/// Elapsed-trip-fraction × 1000 of the engaged hauler over its contract's
/// directed route: `|h − origin| / (|h − origin| + |h − dest|)`, clamped to
/// [0, 1000]. 0 on any unresolvable/degenerate geometry (deterministic skip).
fn trip_phase_milli(
    contracts: &ContractStore,
    stations: &StationStore,
    station_pos: &[Vec3],
    kidx: usize,
    hauler_pos: Vec3,
) -> u32 {
    let pos_of = |sid: crate::ids::StationId| {
        stations
            .ids
            .dense_index(sid.slot, sid.generation)
            .and_then(|row| station_pos.get(row).copied())
    };
    let (Some(origin), Some(dest)) = (
        pos_of(contracts.from_station[kidx]),
        pos_of(contracts.to_station[kidx]),
    ) else {
        return 0;
    };
    let d_from = hauler_pos.sub(origin).length();
    let d_to = hauler_pos.sub(dest).length();
    let total = d_from + d_to;
    if total > 0.0 { (((d_from / total) * 1000.0) as u32).min(1000) } else { 0 }
}

/// Relative bearing (milliradians) of the pirate as seen from the hauler,
/// measured against the hauler's velocity: 0 = dead ahead, ~3142 (π) = dead
/// astern. 0 when the hauler is at rest or the pair is co-located (no
/// direction defined). Diagnostics-only (never folded into state).
fn rel_bearing_millirad(hauler_pos: Vec3, hauler_vel: Vec3, pirate_pos: Vec3) -> u32 {
    let to_pirate = pirate_pos.sub(hauler_pos);
    let d = to_pirate.length();
    let v = hauler_vel.length();
    if d <= 0.0 || v <= 0.0 {
        return 0;
    }
    let cos = (to_pirate.dot(hauler_vel) / (d * v)).clamp(-1.0, 1.0);
    (cos.acos() * 1000.0) as u32
}

/// Relocation target draw (spec §5): uniform among stations within
/// `max_reach_au` of `anchor` (the PRIMARY locality lever — 1-2 neighbors,
/// never the whole map); none in reach -> the NEAREST station (ties to the
/// lowest dense row); `None` only when there are no stations at all (spec §8
/// totality).
///
/// **DUMB BY CONSTRUCTION** (the interdiction-equalizer lesson): the signature
/// admits GEOMETRY ONLY — no contracts, no stock, no traffic, no evidence — so
/// a traffic-weighted relocation attractor cannot be introduced without
/// changing this fn's type. `u` is the caller's pre-drawn Piracy word; the
/// in-reach pick is `u % candidates.len()` (uniform).
pub fn relocate_lurk_target(
    anchor: Vec3,
    station_pos: &[Vec3],
    max_reach_au: f64,
    exclude: Option<usize>,
    u: u64,
) -> Option<usize> {
    let huntable = |s: &usize| Some(*s) != exclude;
    let in_reach: Vec<usize> = (0..station_pos.len())
        .filter(|&s| huntable(&s) && station_pos[s].sub(anchor).length() <= max_reach_au)
        .collect();
    if !in_reach.is_empty() {
        return Some(in_reach[(u % in_reach.len() as u64) as usize]);
    }
    // None huntable in reach: a MAROONED pirate (the hideout-ghetto lesson,
    // seed-23 console session 2026-06-11) breaks out with ONE committal flight
    // to a uniform draw over ALL huntable stations — the lair round-trip is
    // already long-range travel, so bounding the return leg by hop-reach is
    // what created the ghetto. Hunting hops stay reach-bounded; only the
    // breakout is unbounded.
    let all: Vec<usize> = (0..station_pos.len()).filter(huntable).collect();
    if all.is_empty() {
        return None;
    }
    Some(all[(u % all.len() as u64) as usize])
}

/// Issue a fuel-derived-budget Seek of `body` (the ingest/try_load dv rule:
/// never INFINITY into `dv_remaining`).
fn seek_body(ships: &mut CraftStore, row: usize, body: BodyId) {
    let eff = effective_params(&ships.spec[row], &ships.mods[row]);
    let dv = crate::math::tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, ships.fuel_mass[row]);
    ships.nav[row] =
        NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(body)), dv_remaining: dv };
}

/// Stage 1c2 — the pirate brain: a bounded DUMB lurker (spec §5). Pre-physics
/// (`body_pos(t-1)` frame, the try_load precedent), dense pirate-row order:
///
/// * **Lying low** and not at the hideout body: `Seeking{Body(hideout)}` — the
///   refuge OFF the predation field.
/// * **Lurk identity**: the pirate's lurk IS its nav destination (a station
///   body) — no extra hashed column. A pirate whose nav holds no station body
///   (post-refuge re-emergence) draws a fresh lurk via `relocate_lurk_target`
///   from its current position.
/// * **Relocation** — staggered (`tick % relocate_period == row %
///   relocate_period`, sticky on the prey timescale): one Piracy draw keeps
///   the station with `stay_milli`; otherwise one more draw picks
///   uniform-in-reach around the CURRENT lurk (never traffic-weighted; the
///   relocation attractor is deliberately decorrelated from traffic).
/// * **Loiter**: re-issue the lurk seek when drift > `engage_radius / 2` —
///   strictly inside the engagement envelope, so a settled lurker is
///   geometrically guaranteed to cover a body-docked hauler.
///
/// Deliberately NO value-seeking target scoring: dumbness + locality +
/// persistence is the antidote, not a placeholder (spec §5). Scripted stage:
/// skips `!scripted` craft (`craft_cfg` is the config row set; craft are
/// config-minted dense, `slot == row`). Shares the spec-§8 inert lever.
#[allow(clippy::too_many_arguments)]
pub fn run_pirate_brains(
    ships: &mut CraftStore,
    craft_cfg: &[CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    trophic: &TrophicCfg,
    rng: &mut RngStreams,
    tick: Tick,
) {
    if trophic.engage_radius_au <= 0.0 || stations.ids.is_empty() {
        return;
    }
    let prev = Tick(tick.0.saturating_sub(1));
    // Station body positions at `prev` (the pre-physics craft frame), dense
    // station-row order.
    let station_pos: Vec<Vec3> = (0..stations.ids.len())
        .map(|srow| {
            let body = stations.body[srow];
            bodies
                .ids
                .dense_index(body.slot, body.generation)
                .map(|brow| eph.body_pos(bodies.eph_index[brow], prev))
                .unwrap_or(Vec3::ZERO)
        })
        .collect();
    // The haven station (on the hideout body) is excluded from every lurk
    // draw: a pirate does not rob where it fences (and a haven lurk is the
    // seed-23 ghetto's other half — a starving pirate camped at its own lair).
    let haven_station: Option<usize> = bodies
        .ids
        .id_at(trophic.hideout_body_index as usize)
        .map(|(slot, generation)| BodyId { slot, generation })
        .and_then(|hb| (0..stations.ids.len()).find(|&s| stations.body[s] == hb));
    for row in 0..ships.ids.len() {
        if ships.role[row] != CraftRole::Pirate {
            continue;
        }
        let Some(p) = ships.pirate[row] else {
            continue;
        };
        // Scripted stages skip gym-controlled craft (spec §5).
        if craft_cfg.get(row).is_some_and(|c| !c.scripted) {
            continue;
        }
        if tick < p.lie_low_until {
            // Off the predation field: route to the hideout body (a stale
            // hideout index degrades to a deterministic skip, spec §8).
            let hrow = trophic.hideout_body_index as usize;
            let Some((slot, generation)) = bodies.ids.id_at(hrow) else {
                continue;
            };
            let hideout = BodyId { slot, generation };
            let hpos = eph.body_pos(bodies.eph_index[hrow], prev);
            let at_hideout = ships.pos[row].sub(hpos).length() <= ARRIVAL_RADIUS;
            let seeking_hideout = matches!(
                ships.nav[row],
                NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } if b == hideout
            );
            if !at_hideout && !seeking_hideout {
                seek_body(ships, row, hideout);
            }
            continue;
        }
        // Current lurk = the station whose body this pirate is Seeking. None
        // (Idle / seeking the hideout / a non-station body) -> draw a fresh
        // lurk from the pirate's current position.
        let nav_lurk: Option<usize> = match ships.nav[row] {
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
                (0..stations.ids.len()).find(|&s| stations.body[s] == b)
            }
            _ => None,
        };
        // TROPHIC-C3 (spec §6, phase 0a): the haven is NEVER a lurk — not
        // even by nav inheritance. A post-refuge pirate still
        // Seeking{Body(hideout)} would otherwise ADOPT the haven station
        // here, bypassing the exclusion that guards only the fresh and
        // relocation draws ("a pirate does not rob where it fences").
        // Treat a haven nav_lurk as None: the arm below performs the fresh
        // reach-bounded draw from the pirate's current position (marooned
        // breakout when nothing else is in reach).
        let nav_lurk = nav_lurk.filter(|&s| Some(s) != haven_station);
        let mut lurk = match nav_lurk {
            Some(s) => s,
            None => {
                let u = rng.stream(RngStream::Piracy).next_u64();
                match relocate_lurk_target(
                    ships.pos[row],
                    &station_pos,
                    trophic.pirate_max_reach_au,
                    haven_station,
                    u,
                ) {
                    Some(s) => s,
                    None => continue,
                }
            }
        };
        // Starvation-triggered relocation (owner GO 2026-06-11, console
        // session 1): a FED pirate (food >= grubstake) camps — locality is
        // preserved exactly where predation works; a HUNGRY one roams on its
        // stagger slot — coverage exactly where it is failing. The fresh
        // grubstake is the restlessness reference (deliberately not a new
        // config knob: no taste scalar, no config-golden churn). Within the
        // hungry branch relocation stays sticky (stay_milli), reach-bounded,
        // uniform, traffic-blind.
        let hungry = ships.pirate[row]
            .as_ref()
            .is_some_and(|p| p.food_micros < trophic.grubstake_micros);
        if hungry
            && trophic.relocate_period > 0
            && tick.0 % trophic.relocate_period == (row as u64) % trophic.relocate_period
        {
            let stay = (rng.stream(RngStream::Piracy).next_u64() % 1000) as u32;
            if stay >= trophic.stay_milli {
                let u = rng.stream(RngStream::Piracy).next_u64();
                if let Some(s) = relocate_lurk_target(
                    station_pos[lurk],
                    &station_pos,
                    trophic.pirate_max_reach_au,
                    haven_station,
                    u,
                ) {
                    lurk = s;
                }
            }
        }
        // Loiter / (re-)seek: issue the lurk seek when the destination changed
        // or the pirate drifted past engage_radius/2 (the dv-refresh nudge —
        // the threshold is strictly inside the envelope, so a SETTLED lurker
        // covers a docked hauler and is never churned).
        let lurk_body = stations.body[lurk];
        let seeking_lurk = matches!(
            ships.nav[row],
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } if b == lurk_body
        );
        let drifted =
            ships.pos[row].sub(station_pos[lurk]).length() > trophic.engage_radius_au / 2.0;
        if !seeking_lurk || drifted {
            seek_body(ships, row, lurk_body);
        }
    }
}

/// Stage 3b3 — pirate lifecycle (spec §4): population without spawn/despawn.
/// Per pirate, dense order:
///
/// * **Upkeep** while active: `food_micros -= upkeep_per_tick`.
/// * **Starvation** (`food <= 0`): lie low for `starve_lie_low_ticks`, food
///   reset to the grubstake (re-emerges hungry). Emits `PirateLieLow`.
/// * **Heat** (`notoriety >= heat_threshold`): forced lie-low for
///   `heat_lie_low_ticks`. No notoriety reset — heat cools through decay.
/// * **Decay**: notoriety decays geometrically (integer milli arithmetic)
///   every `decay_interval` ticks, active or hiding.
///
/// Desynchronized duty cycles give each route a risk HISTORY — the persistence
/// the heterogeneity axis measures. Shares the spec-§8 inert lever with stage
/// 3b2 ("engage_radius = 0 ⇒ the whole trophic machinery inert").
pub fn update_pirate_population(
    ships: &mut CraftStore,
    trophic: &TrophicCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    if trophic.engage_radius_au <= 0.0 {
        return;
    }
    for row in 0..ships.ids.len() {
        if ships.role[row] != CraftRole::Pirate {
            continue;
        }
        let Some(mut p) = ships.pirate[row] else {
            continue;
        };
        if tick >= p.lie_low_until {
            // Active: pay upkeep, then check the two lie-low triggers.
            p.food_micros = p.food_micros.saturating_sub(trophic.upkeep_per_tick);
            if p.food_micros <= 0 {
                p.lie_low_until = Tick(tick.0.saturating_add(trophic.starve_lie_low_ticks));
                p.food_micros = trophic.grubstake_micros;
                events.emit(Event {
                    tick,
                    kind: EventKind::PirateLieLow { pirate: ships.ids_at(row), until: p.lie_low_until },
                });
            } else if p.notoriety >= trophic.heat_threshold {
                p.lie_low_until = Tick(tick.0.saturating_add(trophic.heat_lie_low_ticks));
                events.emit(Event {
                    tick,
                    kind: EventKind::PirateLieLow { pirate: ships.ids_at(row), until: p.lie_low_until },
                });
            }
        }
        // Geometric notoriety decay on the interval (runs while hiding too —
        // lying low is how heat cools off the predation field).
        if trophic.decay_interval > 0 && tick.0.is_multiple_of(trophic.decay_interval) {
            p.notoriety =
                ((p.notoriety as u64).saturating_mul(trophic.notoriety_decay_milli as u64) / 1000)
                    as u32;
        }
        ships.pirate[row] = Some(p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BaseSpec, BodyInit, ContractInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
        OrbitalElements, PriceCfg, ProducerInit, RunConfig, ShipyardCfg, StationInit, SubstepCfg,
    };
    use crate::contract::{Command, StateView};
    use crate::economy::{Recipe, Resource};
    use crate::ids::{BodyId, ContractId};
    use crate::stores::{NavState, PirateState};
    use crate::time::Dt;
    use crate::types::{CommandKind, EntityRef, NavDest, Target};
    use crate::world::World;

    fn unit_spec() -> BaseSpec {
        BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 0.0,
            base_exhaust_velocity: 1.0,
            base_fuel_capacity: 1.0,
            base_cargo_capacity: 5,
        }
    }

    /// A LIVE trophic config for unit tests: envelope on, deterministic Robbed
    /// (p_rob 1000 — every u ∈ [0,1000) is < 1000), food band set so the
    /// metabolism legs are observable.
    fn live_trophic() -> TrophicCfg {
        TrophicCfg {
            engage_radius_au: 5.0e-4,
            p_rob_milli: 1000,
            food_per_unit_micros: 100_000,
            upkeep_per_tick: 10,
            grubstake_micros: 1_000,
            ..TrophicCfg::default()
        }
    }

    /// Stage-3b2 unit fixture: origin station (row 0) at the origin, destination
    /// station (row 1) at 0.3 AU; one escrowed InTransit contract bound to a
    /// laden hauler (ships row 1) sitting at the origin beside a pirate (row 0).
    /// Corp treasury starts 0 so every refunded micro is robbery money.
    struct Fix {
        ships: CraftStore,
        contracts: ContractStore,
        corporations: CorporationStore,
        counters: EconCounters,
        stations: StationStore,
        station_pos: Vec<Vec3>,
        route_evidence: RouteEvidence,
        station_gossip: Vec<GossipBuffer>,
        next_alert_seq: u32,
        media: crate::config::MediaCfg,
        media_diag: MediaDiag,
        events: EventStream,
        diag: Vec<EngagementSnapshot>,
        rng: RngStreams,
        cid: ContractId,
    }

    fn fix() -> Fix {
        let mut stations = StationStore::empty();
        let from = stations.push(BodyId { slot: 0, generation: 0 }, [0, 0], [0, 0]);
        let to = stations.push(BodyId { slot: 1, generation: 0 }, [0, 0], [0, 0]);
        let station_pos = vec![Vec3::ZERO, Vec3::new(0.3, 0.0, 0.0)];
        let mut corporations = CorporationStore::empty();
        let corp = corporations.push(0, from);
        let mut contracts = ContractStore::empty();
        let cid = contracts.push(corp, Resource::Fuel, 5, from, to, 1_000_000);
        contracts.status[0] = ContractStatus::InTransit;
        contracts.escrow_micros[0] = 1_000_000;
        let mut ships = CraftStore::empty();
        ships.push(unit_spec(), Vec3::ZERO, Vec3::ZERO, 1.0); // row 0: pirate
        let hauler = ships.push(unit_spec(), Vec3::ZERO, Vec3::ZERO, 1.0); // row 1: hauler
        contracts.hauler[0] = Some(hauler);
        ships.role[0] = CraftRole::Pirate;
        ships.pirate[0] = Some(PirateState {
            food_micros: 1_000_000,
            notoriety: 0,
            lie_low_until: Tick(0),
            engage_cooldown_until: Tick(0),
        });
        ships.role[1] = CraftRole::Hauler;
        ships.cargo[1] = Some((Resource::Fuel, 5));
        ships.contract[1] = Some(cid);
        Fix {
            ships,
            contracts,
            corporations,
            counters: EconCounters::zero(),
            stations,
            station_pos,
            route_evidence: RouteEvidence { robs: vec![[Tick(0); 8]; 4], cursor: vec![0; 4] },
            // Media defaults OFF (caps 0/0): existing tests are media-blind.
            // Media tests flip `f.media` to live caps and mint the buffers.
            station_gossip: vec![GossipBuffer::empty(16), GossipBuffer::empty(16)],
            next_alert_seq: 0,
            media: crate::config::MediaCfg::default(),
            media_diag: MediaDiag::default(),
            events: EventStream::new(),
            diag: Vec::new(),
            rng: RngStreams::from_master(42),
            cid,
        }
    }

    fn run(f: &mut Fix, cfg: &TrophicCfg, tick: Tick) {
        resolve_encounters(
            &mut f.ships,
            &mut f.contracts,
            &mut f.corporations,
            &mut f.counters,
            &f.stations,
            &f.station_pos,
            &mut f.route_evidence,
            &mut f.station_gossip,
            &mut f.next_alert_seq,
            cfg,
            &f.media,
            &mut f.rng,
            tick,
            &mut f.events,
            &mut f.diag,
            &mut f.media_diag,
        );
    }

    /// A LIVE media config for unit tests (both caps > 0 opens the config half
    /// of the dual gate; the trophic lever is live via `live_trophic`).
    fn live_media() -> crate::config::MediaCfg {
        crate::config::MediaCfg {
            station_gossip_slots: 16,
            craft_gossip_slots: 8,
            ..crate::config::MediaCfg::default()
        }
    }

    #[test]
    fn witness_seed_is_truth_at_hops_zero() {
        // The mint (spec §4): a Robbed settlement seeds the VICTIM's comms-log
        // with ONE alert — claimed == contract reward + ransom actually paid
        // (the seed-honesty law: NOT Robbed.value_micros, which is the
        // wallet-clamped ransom), hops 0, first_heard == rob tick. The seed
        // emits NO GossipHeard (Robbed tells that story); AlertBorn carries the
        // truth join. Mid-route (off any pier) so no origin-pier deposit fires.
        let cfg = live_trophic(); // p_rob 1000 -> deterministic Robbed
        let mut f = fix();
        f.media = live_media();
        f.ships.gossip[1] = Some(GossipBuffer::empty(8));
        f.ships.pos[0] = Vec3::new(0.15, 0.0, 0.0); // mid-route, off both piers
        f.ships.pos[1] = Vec3::new(0.15, 0.0, 0.0);
        f.contracts.status[0] = ContractStatus::InTransit;
        f.ships.credits_micros[1] = 5_000_000; // ransom cap 2M binds
        run(&mut f, &cfg, Tick(10));
        assert_eq!(robbed_count(&f), 1, "the engagement robbed");
        // Victim buffer: exactly one alert, truth at hops 0.
        let buf = f.ships.gossip[1].as_ref().expect("victim comms-log");
        assert_eq!(buf.occupied(), 1, "exactly one seed alert");
        let seed = buf.slots[0].expect("seed in slot 0");
        assert_eq!(
            seed.claimed_value_micros, 3_000_000,
            "claimed == reward 1M + ransom 2M (NOT the ransom alone)"
        );
        assert_eq!(seed.hops, 0, "the victim's own copy");
        assert_eq!(seed.first_heard, Tick(10), "acquired at the rob tick");
        assert_eq!(seed.rob_tick, Tick(10));
        assert_eq!(seed.pirate_slot, f.ships.ids_at(0).slot, "true perpetrator");
        assert_eq!(seed.route, 1, "route 0->1 over 2 stations = 0*2+1");
        // The pirate gets no copy (its gossip row is None in world resets; the
        // fixture's push default is None too).
        assert!(f.ships.gossip[0].is_none(), "pirate stays information-blind");
        // Events: NO GossipHeard for the hops-0 seed; AlertBorn with the join.
        assert!(
            !f.events.events.iter().any(|e| matches!(e.kind, EventKind::GossipHeard { .. })),
            "the seed does not emit GossipHeard"
        );
        assert!(
            f.events.events.iter().any(|e| matches!(
                e.kind,
                EventKind::AlertBorn {
                    alert_seq: 0,
                    route: 1,
                    truth_value_micros: 3_000_000,
                    claimed_value_micros: 3_000_000,
                    ..
                }
            )),
            "AlertBorn carries the truth join"
        );
        assert_eq!(f.next_alert_seq, 1, "mint counter advanced once");
        // Station reservoirs untouched (no pier in radius).
        assert!(f.station_gossip.iter().all(|b| b.occupied() == 0), "no pier deposit mid-route");
    }

    #[test]
    fn media_off_mints_nothing() {
        // Caps 0/0 (the default fixture): the same robbery leaves ALL media
        // state untouched — the mint is behind `caps_live()`.
        let cfg = live_trophic();
        let mut f = fix();
        f.ships.gossip[1] = Some(GossipBuffer::empty(8));
        run(&mut f, &cfg, Tick(10));
        assert_eq!(robbed_count(&f), 1);
        assert_eq!(f.next_alert_seq, 0, "no mint when media is off");
        assert_eq!(f.ships.gossip[1].as_ref().unwrap().occupied(), 0);
        assert!(f.station_gossip.iter().all(|b| b.occupied() == 0));
        assert!(!f.events.events.iter().any(|e| matches!(
            e.kind,
            EventKind::AlertBorn { .. } | EventKind::GossipHeard { .. }
        )));
    }

    fn engagement_count(f: &Fix) -> usize {
        f.events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Robbed { .. } | EventKind::DrivenOff { .. }))
            .count()
    }

    fn robbed_count(f: &Fix) -> usize {
        f.events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Robbed { .. }))
            .count()
    }

    #[test]
    fn escort_threshold_is_a_step() {
        // strength(pirate) = base 1 + escorts; strength(hauler) = escorts.
        // Engagement iff S_h < S_p — ties go to the DEFENDER (protection is
        // reliable, which is what makes buying it a real decision).
        let cfg = live_trophic();
        for &(p_escorts, h_escorts, expect) in &[
            (0u8, 0u8, true),  // S_p 1 vs S_h 0 -> engage
            (0, 1, false),     // 1 vs 1 -> tie to defender, NO engagement
            (0, 2, false),     // 1 vs 2 -> stronger defender, NO engagement
            (1, 1, true),      // 2 vs 1 -> engage again
        ] {
            let mut f = fix();
            f.ships.upgrades[0].escorts = p_escorts;
            f.ships.upgrades[1].escorts = h_escorts;
            run(&mut f, &cfg, Tick(1));
            assert_eq!(
                engagement_count(&f) > 0,
                expect,
                "S_p={} vs S_h={}: engagement expectation",
                1 + p_escorts,
                h_escorts
            );
        }
    }

    #[test]
    fn robbery_settlement_exact() {
        // Exact-integer per-arm assertions (the identities alone cannot catch
        // wrong-price bugs). Both robbable statuses settle identically.
        for (status, wallet, expected_ransom) in [
            (ContractStatus::InTransit, 5_000_000i64, 2_000_000i64), // cap binds
            (ContractStatus::CargoLoaded, 500_000, 500_000),         // wallet binds
        ] {
            let cfg = live_trophic(); // p_rob 1000 -> deterministic Robbed
            let mut f = fix();
            f.contracts.status[0] = status;
            f.ships.credits_micros[1] = wallet;
            run(&mut f, &cfg, Tick(10));
            // Cargo: the accounted SINK leg (the FuelEmpty precedent).
            assert_eq!(f.counters.consumed[Resource::Fuel.index()], 5, "{status:?}: consumed += qty");
            assert_eq!(f.ships.cargo[1], None, "{status:?}: cargo cleared");
            // Contract: Failed; escrow refunded EXACTLY reward to the right corp.
            assert_eq!(f.contracts.status[0], ContractStatus::Failed, "{status:?}");
            assert_eq!(f.contracts.escrow_micros[0], 0, "{status:?}: escrow zeroed");
            assert_eq!(
                f.corporations.treasury_micros[0], 1_000_000,
                "{status:?}: refund EXACTLY reward_micros"
            );
            // Ransom: EXACTLY min(wallet, cap), hauler -> pirate (pure transfer).
            assert_eq!(
                f.ships.credits_micros[1],
                wallet - expected_ransom,
                "{status:?}: hauler debited exactly the ransom"
            );
            assert_eq!(
                f.ships.credits_micros[0], expected_ransom,
                "{status:?}: pirate credited exactly the ransom"
            );
            // Hauler released to Idle (the robbed-hauler exit).
            assert_eq!(f.ships.contract[1], None, "{status:?}");
            assert_eq!(f.ships.role[1], CraftRole::Idle, "{status:?}");
            // Metabolism: food += qty * per-unit; heat accrues; cooldown set.
            let p = f.ships.pirate[0].unwrap();
            assert_eq!(p.food_micros, 1_000_000 + 5 * 100_000, "{status:?}: food fed");
            assert_eq!(p.notoriety, cfg.notoriety_per_rob, "{status:?}: heat accrued");
            assert_eq!(
                p.engage_cooldown_until,
                Tick(10 + cfg.rob_cooldown),
                "{status:?}: rob digests for rob_cooldown"
            );
            // Event payload carries the exact ransom.
            let cid = f.cid;
            assert!(
                f.events.events.iter().any(|e| matches!(
                    e.kind,
                    EventKind::Robbed { contract, value_micros, .. }
                        if contract == cid && value_micros == expected_ransom
                )),
                "{status:?}: Robbed event with exact value_micros"
            );
            // Kinematic snapshot logged at the emission site: origin dock -> phase ~0.
            assert_eq!(f.diag.len(), 1, "{status:?}: one snapshot per engagement");
            assert!(
                f.diag[0].phase_milli < 50,
                "{status:?}: origin-dock engagement reads near phase 0, got {}",
                f.diag[0].phase_milli
            );
        }
    }

    #[test]
    fn driveoff_emits_and_cools_down_without_settlement() {
        let mut cfg = live_trophic();
        cfg.p_rob_milli = 0; // every roll fails the bluff -> DrivenOff
        let mut f = fix();
        run(&mut f, &cfg, Tick(10));
        assert!(
            f.events.events.iter().any(|e| matches!(e.kind, EventKind::DrivenOff { .. })),
            "DrivenOff emitted"
        );
        assert_eq!(f.contracts.status[0], ContractStatus::InTransit, "no settlement");
        assert_eq!(f.ships.cargo[1], Some((Resource::Fuel, 5)), "cargo kept");
        assert_eq!(f.counters.consumed[Resource::Fuel.index()], 0, "no sink leg");
        assert_eq!(f.ships.credits_micros[0], 0, "no ransom");
        let p = f.ships.pirate[0].unwrap();
        assert_eq!(p.notoriety, 0, "no heat on a failed bluff");
        assert_eq!(p.engage_cooldown_until, Tick(10 + cfg.driveoff_cooldown));
        assert_eq!(f.diag.len(), 1, "DrivenOff emission site also logs the snapshot");
    }

    #[test]
    fn cooldown_prevents_rerolls() {
        let cfg = live_trophic(); // rob_cooldown 600 (default)
        let mut f = fix();
        run(&mut f, &cfg, Tick(10));
        assert_eq!(robbed_count(&f), 1, "first engagement robs");
        // Re-arm the same hauler (fresh escrowed laden run) and probe the window.
        let rearm = |f: &mut Fix| {
            f.contracts.status[0] = ContractStatus::InTransit;
            f.contracts.escrow_micros[0] = 1_000_000;
            f.contracts.hauler[0] = Some(f.ships.ids_at(1));
            f.ships.role[1] = CraftRole::Hauler;
            f.ships.cargo[1] = Some((Resource::Fuel, 5));
            f.ships.contract[1] = Some(f.cid);
        };
        rearm(&mut f);
        run(&mut f, &cfg, Tick(11));
        assert_eq!(robbed_count(&f), 1, "no re-roll at t+1 (digesting)");
        run(&mut f, &cfg, Tick(609));
        assert_eq!(robbed_count(&f), 1, "still digesting at 609 < 10+600");
        run(&mut f, &cfg, Tick(610));
        assert_eq!(robbed_count(&f), 2, "re-engages exactly at cooldown expiry");
    }

    #[test]
    fn default_engage_radius_is_inert() {
        // engage_radius_au == 0.0 (the default) turns the WHOLE trophic
        // machinery off (spec §8): no engagement, no Piracy draw, no
        // population dynamics — existing scenarios bit-identical.
        let cfg = TrophicCfg::default();
        let mut f = fix();
        run(&mut f, &cfg, Tick(10));
        assert!(f.events.events.is_empty(), "no engagement events when inert");
        assert!(f.diag.is_empty(), "no snapshots when inert");
        assert_eq!(f.contracts.status[0], ContractStatus::InTransit, "contract untouched");
        // Population stage shares the lever: a zero-food, white-hot pirate
        // neither starves nor lies low while the machinery is inert.
        f.ships.pirate[0] = Some(PirateState {
            food_micros: 0,
            notoriety: 1_000,
            lie_low_until: Tick(0),
            engage_cooldown_until: Tick(0),
        });
        update_pirate_population(&mut f.ships, &cfg, Tick(10), &mut f.events);
        assert!(f.events.events.is_empty(), "no PirateLieLow when inert");
        assert_eq!(f.ships.pirate[0].unwrap().lie_low_until, Tick(0));
    }

    #[test]
    fn lie_low_and_heat() {
        let cfg = live_trophic(); // upkeep 10, grubstake 1_000, starve 2000,
        // heat 250 / 1500, decay 950 milli every 200 ticks (defaults).

        // (a) STARVATION: food drains to <= 0 -> lie low + grubstake reset + event.
        let mut f = fix();
        f.ships.pirate[0].as_mut().unwrap().food_micros = 5; // 5 - 10 <= 0
        update_pirate_population(&mut f.ships, &cfg, Tick(100), &mut f.events);
        let p = f.ships.pirate[0].unwrap();
        assert_eq!(p.lie_low_until, Tick(100 + cfg.starve_lie_low_ticks), "starve lie-low");
        assert_eq!(p.food_micros, cfg.grubstake_micros, "re-emerges hungry on the grubstake");
        assert!(
            f.events.events.iter().any(|e| matches!(
                e.kind,
                EventKind::PirateLieLow { until, .. } if until == Tick(100 + cfg.starve_lie_low_ticks)
            )),
            "PirateLieLow emitted with the refuge deadline"
        );
        // (b) Lying low: NO upkeep drain (upkeep only while active).
        let food_before = f.ships.pirate[0].unwrap().food_micros;
        update_pirate_population(&mut f.ships, &cfg, Tick(101), &mut f.events);
        assert_eq!(f.ships.pirate[0].unwrap().food_micros, food_before, "no upkeep while hiding");

        // (c) HEAT: notoriety >= threshold forces lie-low; NO notoriety reset
        // (heat cools through decay, not a reset).
        let mut f = fix();
        f.ships.pirate[0].as_mut().unwrap().notoriety = cfg.heat_threshold;
        update_pirate_population(&mut f.ships, &cfg, Tick(50), &mut f.events);
        let p = f.ships.pirate[0].unwrap();
        assert_eq!(p.lie_low_until, Tick(50 + cfg.heat_lie_low_ticks), "heat lie-low");
        assert_eq!(p.notoriety, cfg.heat_threshold, "notoriety NOT reset by the refuge");
        assert!(
            f.events.events.iter().any(|e| matches!(
                e.kind,
                EventKind::PirateLieLow { until, .. } if until == Tick(50 + cfg.heat_lie_low_ticks)
            )),
            "PirateLieLow emitted on heat"
        );

        // (d) DECAY: geometric integer-milli decay on the interval ONLY, and it
        // runs while hiding too (lying low is how heat cools).
        let mut f = fix();
        f.ships.pirate[0] = Some(PirateState {
            food_micros: 1_000_000,
            notoriety: 100,
            lie_low_until: Tick(10_000), // hiding
            engage_cooldown_until: Tick(0),
        });
        update_pirate_population(&mut f.ships, &cfg, Tick(199), &mut f.events);
        assert_eq!(f.ships.pirate[0].unwrap().notoriety, 100, "no decay off the interval");
        update_pirate_population(&mut f.ships, &cfg, Tick(200), &mut f.events);
        assert_eq!(f.ships.pirate[0].unwrap().notoriety, 95, "100 * 950 / 1000 on the interval");
    }

    // ---- World-level tests: stage ordering + first runtime Piracy draws ------

    fn hauler_init(pos: Vec3) -> CraftInit {
        CraftInit {
            spec: BaseSpec {
                base_dry_mass: 1e-9,
                base_max_thrust: 1e-12,
                base_exhaust_velocity: 1e-2,
                base_fuel_capacity: 1e-9,
                base_cargo_capacity: 5,
            },
            pos,
            vel: Vec3::ZERO,
            fuel_mass: 1e-9,
            role: CraftRole::Idle,
            scripted: true,
        }
    }

    fn pirate_init(pos: Vec3) -> CraftInit {
        CraftInit { role: CraftRole::Pirate, ..hauler_init(pos) }
    }

    /// Two-body pirate world: near-massless central star at the origin hosting
    /// station A (origin, 10 Fuel stocked), a 0.3 AU body hosting station B;
    /// one hauler (row 0) and one lurking pirate (row 1) both at the origin;
    /// one funded Offered contract A->B. Scripted ASSIGN off (manual accept).
    /// Trophic LIVE with p_rob 1000 (deterministic Robbed for ordering tests).
    fn pirate_world_cfg() -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 6000,
            bodies: vec![
                BodyInit {
                    mass: 1e-9,
                    elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
                },
                BodyInit {
                    mass: 1e-12,
                    elements: OrbitalElements { a: 0.3, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
                },
            ],
            craft: vec![hauler_init(Vec3::ZERO), pirate_init(Vec3::ZERO)],
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
                    initial_stock: [0, 0],
                    initial_price_micros: [0, 0],
                    sells_upgrades: false,
                },
            ],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 5_000_000, home_station_index: 0 }],
            contracts: vec![ContractInit {
                corp_index: 0,
                resource: Resource::Fuel,
                qty: 5,
                from_station_index: 0,
                to_station_index: 1,
                reward_micros: 1_000_000,
            }],
            price_cfg: PriceCfg::default(),
            dispatch_cfg: DispatchCfg { stagger_period: 0, ..Default::default() },
            trophic: TrophicCfg {
                engage_radius_au: 5.0e-4,
                p_rob_milli: 1000,
                food_per_unit_micros: 100_000,
                upkeep_per_tick: 10,
                grubstake_micros: 1_000_000,
                ..TrophicCfg::default()
            },
            shipyard: ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
        }
    }

    /// `pirate_world_cfg` with the media caps live (the spec-§11 dual gate:
    /// caps > 0 AND the trophic lever, which `pirate_world_cfg` already opens).
    fn media_live_cfg() -> RunConfig {
        let mut cfg = pirate_world_cfg();
        cfg.media.station_gossip_slots = 16;
        cfg.media.craft_gossip_slots = 8;
        cfg
    }

    #[test]
    fn origin_pier_deposit_lands_same_tick() {
        // The rob-on-load world test with media live (spec §4.3): the victim is
        // robbed ON the origin pier, so the station's reservoir hears the
        // firsthand report (hops 1, NO inflation) the SAME tick — without this
        // the edge trigger never re-fires for the dominant robbery class.
        let (mut world, _h) = World::reset(media_live_cfg()).expect("resolvable cfg");
        assert!(world.media_live());
        let hauler = world.ships.ids_at(0);
        let contract = contract_id_row0(&world);
        world.step(&mut vec![Command {
            target: Target::Entity(EntityRef::Craft(hauler)),
            kind: CommandKind::AcceptContract { contract },
        }]);
        assert!(
            world
                .recent_events(Tick(1))
                .iter()
                .any(|e| e.tick == Tick(1) && matches!(e.kind, EventKind::Robbed { .. })),
            "the rob-on-load robbery fired"
        );
        // Origin station (row 0) reservoir holds the firsthand copy at hops 1.
        let deposit = world.station_gossip[0]
            .slots
            .iter()
            .flatten()
            .next()
            .expect("origin pier heard the robbery the same tick");
        assert_eq!(deposit.alert_seq, 0);
        assert_eq!(deposit.hops, 1, "a firsthand report");
        assert_eq!(deposit.rob_tick, Tick(1));
        assert_eq!(deposit.first_heard, Tick(1), "landed the SAME tick");
        // reward 1M + ransom min(grubstake 1M wallet, cap 2M) = 2M: no
        // inflation on the deposit (claimed == the victim's seed).
        let victim_seed =
            world.ships.gossip[0].as_ref().unwrap().slots[0].expect("victim seed");
        assert_eq!(
            deposit.claimed_value_micros, victim_seed.claimed_value_micros,
            "NO inflation on the pier deposit"
        );
        // Exactly one GossipHeard, station carrier, at the rob tick.
        let heard: Vec<_> = world
            .recent_events(Tick(1))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::GossipHeard { .. }))
            .cloned()
            .collect();
        assert_eq!(heard.len(), 1, "one GossipHeard for the deposit");
        assert_eq!(heard[0].tick, Tick(1));
        assert!(
            matches!(
                heard[0].kind,
                EventKind::GossipHeard {
                    carrier: crate::media::GossipNode::Station(_),
                    alert_seq: 0,
                    hops: 1,
                    ..
                }
            ),
            "the pier is the carrier"
        );
    }

    #[test]
    fn pirates_are_information_blind() {
        // OD-6: pirate rows carry NO comms-log (None after reset) and never
        // appear as a GossipHeard Craft carrier, even parked on a pier with a
        // stocked reservoir for a whole edge window. (The read-side fence is
        // compile-level: `relocate_lurk_target`'s geometry-only signature.)
        let (mut world, _h) = World::reset(media_live_cfg()).expect("resolvable cfg");
        let pirate_id = world.ships.ids_at(1);
        assert!(world.ships.gossip[1].is_none(), "pirate row mints None at reset");
        let hauler = world.ships.ids_at(0);
        let contract = contract_id_row0(&world);
        // The rob-on-load robbery stocks the origin reservoir at tick 1; the
        // pirate lurks docked at the origin through the window that follows.
        world.step(&mut vec![Command {
            target: Target::Entity(EntityRef::Craft(hauler)),
            kind: CommandKind::AcceptContract { contract },
        }]);
        assert!(world.station_gossip[0].occupied() > 0, "reservoir stocked (non-vacuous)");
        let mut cmds = Vec::new();
        for _ in 0..30 {
            world.step(&mut cmds);
        }
        assert!(world.ships.gossip[1].is_none(), "still no comms-log");
        assert!(
            !world.recent_events(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::GossipHeard {
                    carrier: crate::media::GossipNode::Craft(c),
                    ..
                } if c == pirate_id
            )),
            "a pirate never hears gossip (neither uploads nor downloads)"
        );
    }

    fn contract_id_row0(world: &World) -> ContractId {
        world
            .contracts
            .ids
            .id_at(0)
            .map(|(slot, generation)| ContractId { slot, generation })
            .expect("contract row 0 live")
    }

    #[test]
    fn rob_on_load_is_legal_at_origin() {
        // The owned headline behavior (spec §2): a pirate lurking at the ORIGIN
        // dock engages the hauler on its LOAD tick — stage 3b2 runs after the
        // same-tick load (1c) and physics, so departure ambush is legal.
        let (mut world, _h) = World::reset(pirate_world_cfg()).expect("resolvable cfg");
        let hauler = world.ships.ids_at(0);
        let contract = contract_id_row0(&world);
        world.step(&mut vec![Command {
            target: Target::Entity(EntityRef::Craft(hauler)),
            kind: CommandKind::AcceptContract { contract },
        }]);
        // Engagement fired THE SAME tick the cargo was loaded.
        assert!(
            world
                .recent_events(Tick(1))
                .iter()
                .any(|e| e.tick == Tick(1) && matches!(e.kind, EventKind::Robbed { .. })),
            "rob-on-load engagement fires on the load tick"
        );
        assert_eq!(
            world.contracts.status[0],
            crate::economy::ContractStatus::Failed,
            "contract failed by robbery"
        );
        assert_eq!(world.econ.consumed[Resource::Fuel.index()], 5, "cargo sink leg accounted");
        assert_eq!(
            world.corporations.treasury_micros[0], 5_000_000,
            "escrow refunded same tick (escrowed 1M at 1c, refunded at 3b2)"
        );
        assert_eq!(world.ships.role[0], CraftRole::Idle, "hauler released");
        assert_eq!(world.ships.cargo[0], None, "cargo gone");
        // The §2 kinematic snapshot reaches the diagnostics sampler: one
        // engagement at trip-phase ~0 (the departure-ambush endpoint).
        let sample = crate::diagnostics::sample_window(&world, Tick(0));
        assert_eq!(sample.engagement_phase_milli.len(), 1, "sampler sees the snapshot");
        assert!(
            sample.engagement_phase_milli[0] < 50,
            "origin-dock ambush reads near phase 0, got {}",
            sample.engagement_phase_milli[0]
        );
    }

    /// Hand-build an escrowed InTransit leg for craft row 0 on contract row 0
    /// (laden, bound, treasury debited so the credit identity stays whole).
    fn make_in_transit(world: &mut World) {
        let hauler = world.ships.ids_at(0);
        let cid = contract_id_row0(world);
        world.contracts.status[0] = crate::economy::ContractStatus::InTransit;
        world.contracts.hauler[0] = Some(hauler);
        world.contracts.escrow_micros[0] = 1_000_000;
        world.corporations.treasury_micros[0] -= 1_000_000;
        world.ships.role[0] = CraftRole::Hauler;
        world.ships.cargo[0] = Some((Resource::Fuel, 5));
        world.ships.contract[0] = Some(cid);
    }

    #[test]
    fn dock_is_sanctuary_at_destination() {
        // Same-tick Arrival + in-envelope pirate -> the delivery settles and NO
        // engagement fires: 3b (deliveries) runs BEFORE 3b2 (encounters).
        // "Made port with the corsair an engine-length behind."
        let dest_pos = Vec3::new(0.3, 0.0, 0.0); // body 1 at m0=0: (a, 0, 0)
        let mut cfg = pirate_world_cfg();
        cfg.craft[0].pos = dest_pos;
        cfg.craft[1].pos = dest_pos; // the corsair, an engine-length behind
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        make_in_transit(&mut world);
        let to_body = world
            .bodies
            .ids
            .id_at(1)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        // Seeking the destination body from inside ARRIVAL_RADIUS at ~rest:
        // the Arrival edge fires on step 1.
        world.ships.nav[0] =
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(to_body)), dv_remaining: 1e-3 };
        world.step(&mut Vec::new());
        assert!(
            world
                .recent_events(Tick(1))
                .iter()
                .any(|e| matches!(e.kind, EventKind::ContractFulfilled { .. })),
            "delivery settled on the arrival tick"
        );
        assert_eq!(world.contracts.status[0], crate::economy::ContractStatus::Completed);
        assert_eq!(world.ships.credits_micros[0], 1_000_000, "payout landed");
        assert!(
            !world.recent_events(Tick(1)).iter().any(|e| matches!(
                e.kind,
                EventKind::Robbed { .. } | EventKind::DrivenOff { .. }
            )),
            "NO engagement: the destination dock is sanctuary by ordering (3b before 3b2)"
        );

        // CONTROL ARM (non-vacuity): identical geometry but NO same-tick Arrival
        // (nav Idle) -> the contract is still InTransit at 3b2 and the pirate
        // engages. Sanctuary above is the ORDERING, not a dead envelope.
        let mut cfg = pirate_world_cfg();
        cfg.craft[0].pos = dest_pos;
        cfg.craft[1].pos = dest_pos;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        make_in_transit(&mut world);
        world.step(&mut Vec::new());
        assert!(
            world
                .recent_events(Tick(1))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Robbed { .. })),
            "control: same geometry without the same-tick delivery IS engaged"
        );
        // The inbound-deceleration ambush reads near trip-phase 1000.
        let sample = crate::diagnostics::sample_window(&world, Tick(0));
        assert_eq!(sample.engagement_phase_milli.len(), 1);
        assert!(
            sample.engagement_phase_milli[0] > 950,
            "destination-side ambush reads near phase 1000, got {}",
            sample.engagement_phase_milli[0]
        );
    }

    /// Self-running 5k-tick pirate scenario: the stage-1 economy loop (miner +
    /// refiner at A, sink at B, scripted dispatch ON) with two haulers and one
    /// lurking pirate at the origin choke point. p_rob 700 — real odds, both
    /// outcomes occur. Hauler endurance is scenario-sized (the spec-§6
    /// precedent): v_e x20 and a 9:1 fuel:dry ratio give a Δv budget of
    /// ~v_e·ln(10) ≈ 0.46 AU/day — ~20 round trips, so reload-cycle
    /// engagements recur across the whole window instead of stranding after
    /// one trip. Fixture cooldowns are shortened below the ~450-tick reload
    /// cadence so successive loads actually re-engage.
    fn self_running_pirate_cfg() -> RunConfig {
        let mut cfg = pirate_world_cfg();
        cfg.craft = vec![hauler_init(Vec3::ZERO), hauler_init(Vec3::ZERO), pirate_init(Vec3::ZERO)];
        for h in &mut cfg.craft[0..2] {
            h.spec.base_exhaust_velocity = 2e-1;
            h.spec.base_fuel_capacity = 9e-9;
            h.fuel_mass = 9e-9;
        }
        cfg.producers = vec![
            ProducerInit {
                station_index: 0,
                recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 1 },
            },
            ProducerInit {
                station_index: 0,
                recipe: Recipe {
                    input: Some((Resource::Ore, 2)),
                    output: Some((Resource::Fuel, 2)),
                    interval: 1,
                },
            },
            ProducerInit {
                station_index: 1,
                recipe: Recipe { input: Some((Resource::Fuel, 1)), output: None, interval: 1 },
            },
        ];
        cfg.dispatch_cfg = DispatchCfg {
            demand_low: 10,
            demand_high: 20,
            stagger_period: 1,
            contract_reward_micros: 1_000_000,
            contract_qty: 5,
        };
        cfg.corporations =
            vec![CorporationInit { treasury_micros: 100_000_000, home_station_index: 0 }];
        cfg.trophic.p_rob_milli = 700;
        cfg.trophic.rob_cooldown = 200;
        cfg.trophic.driveoff_cooldown = 50;
        cfg
    }

    #[test]
    fn replay_bit_identical_with_piracy_draws() {
        // The FIRST runtime RngStream::Piracy draws are the new replay surface:
        // two instances from the same config must produce bit-identical
        // (tick, state_hash) streams across 5k ticks of live predation.
        let (mut wa, _) = World::reset(self_running_pirate_cfg()).expect("resolvable cfg");
        let (mut wb, _) = World::reset(self_running_pirate_cfg()).expect("resolvable cfg");
        let h0 = crate::hash::state_hash(&wa);
        assert_eq!(h0, crate::hash::state_hash(&wb), "tick 0 hashes agree");
        let mut ea: Vec<Command> = Vec::new();
        let mut eb: Vec<Command> = Vec::new();
        let mut last = h0;
        for t in 1..=5000u64 {
            wa.step(&mut ea);
            wb.step(&mut eb);
            let ha = crate::hash::state_hash(&wa);
            let hb = crate::hash::state_hash(&wb);
            assert_eq!(ha, hb, "state_hash diverged at tick {t}");
            last = ha;
        }
        assert_ne!(last, h0, "state evolved (not a constant sequence)");
        // Non-vacuity: the Piracy stream actually drew — engagements occurred
        // and at least one robbery settled.
        let robs = wa
            .recent_events(Tick(1))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Robbed { .. }))
            .count();
        let driveoffs = wa
            .recent_events(Tick(1))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::DrivenOff { .. }))
            .count();
        assert!(robs >= 1, "no robbery in 5k ticks — the replay claim is vacuous");
        assert!(robs + driveoffs >= 2, "expected repeated engagements, got {robs}+{driveoffs}");
    }

    // ---- Task 5 (Commit E): brains, evidence, scripted policies --------------

    #[test]
    fn initial_lurks_are_seed_drawn() {
        // Spec §5: initial lurk stations are drawn from the Piracy stream AT
        // RESET — never config-fixed (a fixed pirate→station map would let a
        // gym policy memorize geography instead of reading contacts).
        fn cfg(seed: u64) -> RunConfig {
            let mut c = pirate_world_cfg();
            c.master_seed = seed;
            // 4 station bodies, 3 pirates -> 64 possible lurk maps.
            c.bodies = (0..4)
                .map(|i| BodyInit {
                    mass: 1e-12,
                    elements: OrbitalElements {
                        a: 0.1 + 0.2 * i as f64,
                        e: 0.0,
                        i: 0.0,
                        raan: 0.0,
                        argp: 0.0,
                        m0: 0.0,
                    },
                })
                .collect();
            c.stations = (0..4)
                .map(|i| StationInit {
                    body_index: i,
                    initial_stock: [0, 0],
                    initial_price_micros: [0, 0],
                    sells_upgrades: false,
                })
                .collect();
            c.corporations =
                vec![CorporationInit { treasury_micros: 0, home_station_index: 0 }];
            c.contracts = vec![];
            c.craft =
                vec![pirate_init(Vec3::ZERO), pirate_init(Vec3::ZERO), pirate_init(Vec3::ZERO)];
            c
        }
        fn lurk_map(world: &World) -> Vec<u32> {
            (0..world.ships.ids.len())
                .map(|row| match world.ships.nav[row] {
                    NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => b.slot,
                    ref other => panic!("pirate row {row} not Seeking a body at reset: {other:?}"),
                })
                .collect()
        }
        let (wa, _) = World::reset(cfg(11)).expect("resolvable cfg");
        let (wb, _) = World::reset(cfg(12)).expect("resolvable cfg");
        let (ma, mb) = (lurk_map(&wa), lurk_map(&wb));
        for m in ma.iter().chain(mb.iter()) {
            assert!(*m < 4, "lurk body slot {m} must be a station body");
        }
        assert_ne!(ma, mb, "different master seeds must draw different lurk maps");
    }

    #[test]
    fn relocation_respects_reach() {
        // Spec §5: relocation is uniform-in-reach and NEVER traffic-weighted —
        // enforced BY CONSTRUCTION: `relocate_lurk_target`'s signature admits
        // GEOMETRY ONLY (anchor, station positions, reach, a pre-drawn word).
        // No contract / stock / traffic input exists to weight by.
        let station_pos = vec![
            Vec3::new(0.1, 0.0, 0.0),  // 0: in reach
            Vec3::new(0.0, 0.3, 0.0),  // 1: in reach
            Vec3::new(0.9, 0.0, 0.0),  // 2: out of reach
            Vec3::new(0.5, 0.0, 0.0),  // 3: in reach
            Vec3::new(0.0, 0.0, 2.0),  // 4: out of reach
            Vec3::new(0.0, 0.59, 0.0), // 5: in reach (just inside 0.6)
        ];
        let anchor = Vec3::ZERO;
        let reach = 0.6;
        let in_reach = [0usize, 1, 3, 5];
        // 64 staggered draws through a REAL Piracy stream: never beyond reach,
        // and every in-reach station gets drawn.
        let mut rng = RngStreams::from_master(99);
        let mut hits = [0u32; 6];
        for _ in 0..64 {
            let u = rng.stream(RngStream::Piracy).next_u64();
            let s = relocate_lurk_target(anchor, &station_pos, reach, None, u)
                .expect("stations exist");
            assert!(in_reach.contains(&s), "target {s} is beyond reach");
            hits[s] += 1;
        }
        for &s in &in_reach {
            assert!(hits[s] > 0, "uniform-in-reach: station {s} never drawn in 64");
        }
        // Exact uniformity of the draw map: u = 0..64 cycles the 4 candidates
        // evenly (16 each) — no weighting of any kind.
        let mut exact = [0u32; 6];
        for u in 0..64u64 {
            exact[relocate_lurk_target(anchor, &station_pos, reach, None, u).unwrap()] += 1;
        }
        for &s in &in_reach {
            assert_eq!(exact[s], 16, "u %% n_candidates must map uniformly");
        }
        // None in reach -> the marooned BREAKOUT: one committal flight to a
        // uniform draw over ALL huntable stations (u % 6 = 1 here), never the
        // nearest-only ghetto (seed-23 lesson).
        let far = Vec3::new(50.0, 0.0, 0.0);
        assert_eq!(
            relocate_lurk_target(far, &station_pos, reach, None, 7).unwrap(),
            1,
            "marooned breakout draws uniformly over all huntable stations"
        );
        // The haven is never huntable: in-reach (anchor covers 0,1,3,5 minus
        // excluded 1 -> [0,3,5], u=4 -> 4 % 3 = 1 -> station 3) and breakout
        // (far, exclude 2 -> [0,1,3,4,5], 7 % 5 = 2 -> station 3) both skip it.
        assert_eq!(relocate_lurk_target(anchor, &station_pos, reach, Some(1), 4), Some(3));
        assert_eq!(relocate_lurk_target(far, &station_pos, reach, Some(2), 7), Some(3));
        // No stations at all -> None (totality, spec §8).
        assert_eq!(relocate_lurk_target(anchor, &[], reach, None, 7), None);
    }

    #[test]
    fn reseek_threshold_covers_dock() {
        // Spec §5 loiter: re-seek strictly INSIDE the engagement envelope
        // (engage_radius / 2) so a settled lurker geometrically covers a
        // body-docked hauler: engage_radius/2 + ARRIVAL_RADIUS <= engage_radius.
        let radius = pirate_world_cfg().trophic.engage_radius_au;
        assert!(
            crate::autopilot::ARRIVAL_RADIUS <= radius / 2.0,
            "a settled lurker (within engage_radius/2) plus a docked hauler \
             (within ARRIVAL_RADIUS) must stay inside the engagement envelope"
        );
        let mk = || {
            let mut c = pirate_world_cfg();
            c.craft = vec![pirate_init(Vec3::ZERO)]; // lone pirate
            c.contracts = vec![];
            c.trophic.hideout_body_index = 99; // isolate loiter geometry from haven exclusion
            World::reset(c).expect("resolvable cfg").0
        };
        let body0 = |w: &World| {
            w.bodies
                .ids
                .id_at(0)
                .map(|(slot, generation)| BodyId { slot, generation })
                .unwrap()
        };

        // (a) DRIFTED past engage_radius/2: the brain re-issues the lurk seek
        // with a fresh fuel-derived dv budget.
        let mut world = mk();
        let b0 = body0(&world);
        world.ships.nav[0] =
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b0)), dv_remaining: 0.0 };
        world.ships.pos[0] = Vec3::new(radius * 0.6, 0.0, 0.0); // > radius/2 from the lurk body
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut Vec::new());
        match world.ships.nav[0] {
            NavState::Seeking { dest, dv_remaining } => {
                assert_eq!(dest, NavDest::Entity(EntityRef::Body(b0)), "still seeking the lurk");
                assert!(
                    dv_remaining > 1.0e-3,
                    "drifted lurker re-seeks with a refreshed dv, got {dv_remaining}"
                );
            }
            ref other => panic!("expected Seeking, got {other:?}"),
        }

        // (b) SETTLED inside engage_radius/2: NO re-issue (budget not refreshed).
        let mut world = mk();
        let b0 = body0(&world);
        world.ships.nav[0] =
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b0)), dv_remaining: 0.0 };
        world.ships.pos[0] = Vec3::ZERO; // exactly at the lurk body
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut Vec::new());
        match world.ships.nav[0] {
            NavState::Seeking { dv_remaining, .. } => {
                assert!(
                    dv_remaining < 1.0e-3,
                    "settled lurker keeps its depleted budget, got {dv_remaining}"
                );
            }
            ref other => panic!("expected Seeking, got {other:?}"),
        }
    }

    #[test]
    fn fed_pirate_camps_hungry_pirate_roams() {
        // Owner GO 2026-06-11 (console session 1): starvation-triggered
        // relocation — a FED pirate (food >= grubstake) never redraws its
        // lurk (locality preserved where predation works); a HUNGRY one
        // roams on its stagger slot (coverage exactly where it is failing).
        fn lurk_of(world: &World, row: usize) -> Option<BodyId> {
            match world.ships.nav[row] {
                NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
                    Some(b)
                }
                _ => None,
            }
        }
        fn cfg() -> RunConfig {
            let mut cfg = pirate_world_cfg();
            cfg.contracts = vec![];
            cfg.craft = vec![pirate_init(Vec3::ZERO)];
            cfg.trophic.relocate_period = 1; // eligible every tick
            cfg.trophic.stay_milli = 0; // never sticky: every slot redraws
            cfg.trophic.upkeep_per_tick = 0; // hold hunger constant
            cfg.trophic.pirate_max_reach_au = 10.0; // both stations in reach
            // Out-of-range hideout: no haven exclusion (degrades to skip per
            // spec §8 totality) — this test isolates the HUNGER gate.
            cfg.trophic.hideout_body_index = 99;
            cfg
        }
        // FED: the lurk must never change.
        let c = cfg();
        let grubstake = c.trophic.grubstake_micros;
        let (mut world, _) = World::reset(c).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().food_micros = grubstake;
        world.step(&mut Vec::new());
        let home = lurk_of(&world, 0).expect("settled on a lurk");
        for _ in 0..64 {
            world.step(&mut Vec::new());
            assert_eq!(lurk_of(&world, 0), Some(home), "a fed pirate camps");
        }
        // HUNGRY: the seeded draw changes the lurk within the probe window
        // (P(no change) = 2^-64 under the uniform 2-station draw;
        // deterministic for this seed).
        let (mut world, _) = World::reset(cfg()).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().food_micros = 1;
        world.step(&mut Vec::new());
        let start = lurk_of(&world, 0).expect("settled on a lurk");
        let mut moved = false;
        for _ in 0..64 {
            world.step(&mut Vec::new());
            if lurk_of(&world, 0) != Some(start) {
                moved = true;
                break;
            }
        }
        assert!(moved, "a hungry pirate roams (relocation re-enabled by hunger)");
    }

    #[test]
    fn post_refuge_pirate_never_adopts_the_haven_lurk() {
        // Spec §6 (TROPHIC-C3, phase 0a): a post-refuge pirate whose nav
        // still resolves the hideout BODY must not inherit the HAVEN station
        // as its hunting lurk — the nav-derived lurk path must respect the
        // same exclusion that guards fresh draws ("a pirate does not rob
        // where it fences"). Pre-fix this is the rob-where-you-fence
        // attractor inside every banked baseline.
        fn cfg() -> RunConfig {
            let mut cfg = pirate_world_cfg();
            cfg.contracts = vec![];
            cfg.craft = vec![pirate_init(Vec3::ZERO)]; // lone pirate, row 0
            // Body 0 (origin) hosts station 0 -> the haven is station 0.
            cfg.trophic.hideout_body_index = 0;
            cfg.trophic.upkeep_per_tick = 0; // hold the FED state constant
            cfg
        }
        let c = cfg();
        let grubstake = c.trophic.grubstake_micros;
        let (mut world, _) = World::reset(c).expect("resolvable cfg");
        let hideout = world
            .bodies
            .ids
            .id_at(0)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        // Construct the post-refuge state: refuge EXPIRED, nav still routed
        // at the hideout body (exactly what the lie-low arm leaves behind),
        // FED (food >= grubstake) so the hungry-relocation arm never runs —
        // the nav-adoption path is the ONLY draw under test.
        {
            let p = world.ships.pirate[0].as_mut().unwrap();
            p.lie_low_until = Tick(0);
            p.food_micros = grubstake;
        }
        world.ships.nav[0] = NavState::Seeking {
            dest: NavDest::Entity(EntityRef::Body(hideout)),
            dv_remaining: 1.0,
        };
        let lurk_body = |w: &World| match w.ships.nav[0] {
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => Some(b),
            _ => None,
        };
        for _ in 0..8 {
            world.step(&mut Vec::new());
            assert_ne!(
                lurk_body(&world),
                Some(hideout),
                "post-refuge pirate adopted the HAVEN as its lurk \
                 (the rob-where-you-fence leak)"
            );
        }
    }

    #[test]
    fn post_refuge_redraw_excludes_haven_even_when_marooned() {
        // Spec §6: the post-refuge redraw goes through relocate_lurk_target
        // with the haven EXCLUDED — when the haven is the only station in
        // reach, the draw falls through to the marooned BREAKOUT (uniform
        // over all huntable stations) rather than back onto the haven. This
        // is the spec's stated cost, owned: on today's band most post-refuge
        // draws become map-wide breakouts (console re-judgment scheduled).
        let mut cfg = pirate_world_cfg();
        cfg.contracts = vec![];
        cfg.craft = vec![pirate_init(Vec3::ZERO)]; // lone pirate, row 0
        cfg.trophic.hideout_body_index = 0; // haven = station 0 at the origin
        cfg.trophic.upkeep_per_tick = 0; // hold the FED state constant
        cfg.bodies[1].elements.a = 5.0; // station 1 beyond reach (0.6 AU)
        let grubstake = cfg.trophic.grubstake_micros;
        let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
        let hideout = world
            .bodies
            .ids
            .id_at(0)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        let far_body = world
            .bodies
            .ids
            .id_at(1)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        {
            let p = world.ships.pirate[0].as_mut().unwrap();
            p.lie_low_until = Tick(0);
            p.food_micros = grubstake;
        }
        world.ships.nav[0] = NavState::Seeking {
            dest: NavDest::Entity(EntityRef::Body(hideout)),
            dv_remaining: 1.0,
        };
        world.step(&mut Vec::new());
        assert!(
            matches!(
                world.ships.nav[0],
                NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. }
                    if b == far_body
            ),
            "marooned post-refuge pirate must break out to the non-haven \
             station, got {:?}",
            world.ships.nav[0]
        );
    }

    #[test]
    fn lying_low_pirate_seeks_hideout() {
        // Spec §5: lying low and not at the hideout -> Seeking{Body(hideout)}
        // (the refuge OFF the predation field).
        let mut cfg = pirate_world_cfg();
        cfg.contracts = vec![];
        cfg.craft = vec![pirate_init(Vec3::ZERO)];
        cfg.trophic.hideout_body_index = 1; // the outer body
        let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().lie_low_until = Tick(10_000);
        world.step(&mut Vec::new());
        let hideout = world
            .bodies
            .ids
            .id_at(1)
            .map(|(slot, generation)| BodyId { slot, generation })
            .unwrap();
        assert!(
            matches!(
                world.ships.nav[0],
                NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } if b == hideout
            ),
            "lying-low pirate routes to the hideout body"
        );
    }

    #[test]
    fn unscripted_pirate_is_skipped_by_brain_stages() {
        // Spec §5: scripted stages skip gym-controlled craft. An unscripted
        // pirate gets NO reset lurk draw and NO brain nav writes.
        let mut cfg = pirate_world_cfg();
        cfg.contracts = vec![];
        cfg.craft = vec![CraftInit { scripted: false, ..pirate_init(Vec3::new(0.01, 0.0, 0.0)) }];
        let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
        assert!(
            matches!(world.ships.nav[0], NavState::Idle),
            "no reset lurk draw for a !scripted pirate"
        );
        world.step(&mut Vec::new());
        assert!(
            matches!(world.ships.nav[0], NavState::Idle),
            "brains never steer a !scripted pirate"
        );
    }

    #[test]
    fn info_tick_refreshes_only_docked() {
        // Spec §7: information refreshes by DOCKING (within ARRIVAL_RADIUS of
        // any station body); an in-flight craft keeps its stale info_tick.
        let mut cfg = pirate_world_cfg();
        cfg.contracts = vec![];
        cfg.craft = vec![hauler_init(Vec3::ZERO), hauler_init(Vec3::new(0.15, 0.0, 0.0))];
        let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
        world.step(&mut Vec::new());
        assert_eq!(world.ships.info_tick[0], Tick(1), "docked craft refreshes to current tick");
        assert_eq!(world.ships.info_tick[1], Tick(0), "in-flight craft keeps stale info_tick");
        world.step(&mut Vec::new());
        assert_eq!(world.ships.info_tick[0], Tick(2), "refresh repeats every docked tick");
        assert_eq!(world.ships.info_tick[1], Tick(0));
    }

    #[test]
    fn route_evidence_read_is_dock_gated() {
        // Spec §7: the store holds EVIDENCE ONLY (rob-tick rings); staleness is
        // a property of the READ — a reader sees the world as of its OWN last
        // dock, and entries age out past evidence_window.
        let (mut world, _h) = World::reset(pirate_world_cfg()).expect("resolvable cfg");
        let hauler = world.ships.ids_at(0);
        let contract = contract_id_row0(&world);
        world.step(&mut vec![Command {
            target: Target::Entity(EntityRef::Craft(hauler)),
            kind: CommandKind::AcceptContract { contract },
        }]);
        // The rob landed at tick 1 on directed route A->B (rows 0 -> 1, n=2).
        assert_eq!(
            world.contracts.status[0],
            crate::economy::ContractStatus::Failed,
            "precondition: the load-tick robbery settled"
        );
        let route_ab = 1usize; // 0 * n_stations + 1
        // Reader last docked BEFORE the rob -> evidence invisible.
        world.ships.info_tick[0] = Tick(0);
        assert_eq!(world.route_evidence(hauler, route_ab), 0, "docked at T-1 sees count 0");
        // Reader docked AFTER the rob -> evidence visible.
        world.ships.info_tick[0] = Tick(2);
        assert_eq!(world.route_evidence(hauler, route_ab), 1, "docked at T+1 sees the rob");
        // Ageing (default evidence_window 4000): the window is
        // (info_tick - window, info_tick] — at rob_tick + window the entry
        // sits exactly on the excluded lower edge.
        world.ships.info_tick[0] = Tick(1 + 4000 - 1);
        assert_eq!(world.route_evidence(hauler, route_ab), 1, "still inside the window");
        world.ships.info_tick[0] = Tick(1 + 4000);
        assert_eq!(world.route_evidence(hauler, route_ab), 0, "aged out past evidence_window");
        // Other routes / stale readers read 0 (totality, spec §8).
        world.ships.info_tick[0] = Tick(2);
        assert_eq!(world.route_evidence(hauler, 0), 0, "untouched route reads 0");
        assert_eq!(world.route_evidence(hauler, 99), 0, "out-of-range route reads 0");
        let stale = crate::ids::CraftId { slot: 0, generation: 99 };
        assert_eq!(world.route_evidence(stale, route_ab), 0, "stale reader reads 0");
    }

    #[test]
    fn conservation_identities_hold_across_robbed_run() {
        // Both identities, EVERY tick, over a 5k-tick run with live robbery:
        // credits (Σtreasury+Σcredits+Σescrow constant — ransom/refund are pure
        // transfers, ZERO new legs) and resources (Σstock + Σin_transit ==
        // initial + mined − consumed — robbed cargo uses the consumed leg).
        let (mut world, _) = World::reset(self_running_pirate_cfg()).expect("resolvable cfg");
        let credit_now = |w: &World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        let initial_credit = credit_now(&world);
        let mut initial_stock = [0i64; crate::economy::N_RESOURCES];
        for (r, slot) in initial_stock.iter_mut().enumerate() {
            *slot = world.stations.stock.iter().map(|s| s[r]).sum();
        }
        let mut empty: Vec<Command> = Vec::new();
        for t in 1..=5000u64 {
            world.step(&mut empty);
            assert_eq!(credit_now(&world), initial_credit, "credit identity broke at tick {t}");
            for r in 0..crate::economy::N_RESOURCES {
                let stock: i64 = world.stations.stock.iter().map(|s| s[r]).sum();
                let in_transit: i64 = world
                    .ships
                    .cargo
                    .iter()
                    .filter_map(|c| c.and_then(|(res, q)| (res.index() == r).then_some(q as i64)))
                    .sum();
                assert_eq!(
                    stock + in_transit,
                    initial_stock[r] + world.econ.mined[r] - world.econ.consumed[r],
                    "resource identity broke for r={r} at tick {t}"
                );
            }
        }
        // Non-vacuity: at least one robbery actually settled (a peaceful run
        // satisfies both identities trivially).
        let robs = world
            .recent_events(Tick(1))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::Robbed { .. }))
            .count();
        assert!(robs >= 1, "no robbery settled — the identity claim is vacuous");
    }
}
