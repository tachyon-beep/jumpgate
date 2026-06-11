//! Media rung cut 1: gossip data structures (spec §2). EVIDENCE ONLY — raw
//! ticks and claimed integers; significance/staleness are computed at the
//! READ, never stored. No heat, threat, or confidence fields — ever (PDR-0006).

use crate::config::MediaCfg;
use crate::contract::{Event, EventKind};
use crate::economy::StationStore;
use crate::events::EventStream;
use crate::ids::{CraftId, StationId};
use crate::rng::{RngStream, RngStreams};
use crate::stores::CraftStore;
use crate::time::Tick;
use rand_core::Rng;

/// One rumor as held by one node — the COVER. Truth lives in the run record
/// (Robbed + AlertBorn events, joined on `alert_seq`). EVIDENCE ONLY: raw
/// ticks and claimed integers; significance/staleness computed at the read,
/// never stored. No heat, threat, or confidence fields — ever.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GossipAlert {
    /// Hashed world mint counter — identity, dedup key, eviction tie-break,
    /// lab join key.
    pub alert_seq: u32,
    /// Claimed directed route, `from_row * n_stations + to_row` (TRUE in cut 1).
    pub route: u32,
    /// Claimed perpetrator `CraftId.slot` (TRUE in cut 1; corruption = a later
    /// cut WITH its consumer).
    pub pirate_slot: u32,
    /// Claimed when (TRUE in cut 1).
    pub rob_tick: Tick,
    /// The ONLY mutating field — seeded from true loss, inflates on
    /// retellings (spec §3).
    pub claimed_value_micros: i64,
    /// When THIS node acquired this copy (raw; the per-reader staleness anchor).
    pub first_heard: Tick,
    /// Saturating; 0 = the victim's own copy.
    pub hops: u8,
}

/// A node's rumor buffer: cap from config, fixed at reset (the RouteEvidence
/// sizing law — no mid-run resize).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GossipBuffer {
    pub slots: Vec<Option<GossipAlert>>,
}

impl GossipBuffer {
    /// An all-empty buffer with `cap` slots.
    pub fn empty(cap: u32) -> Self {
        GossipBuffer { slots: vec![None; cap as usize] }
    }
    /// Whether any held alert carries this `alert_seq` (the dedup check).
    pub fn holds(&self, alert_seq: u32) -> bool {
        self.slots.iter().any(|s| s.is_some_and(|a| a.alert_seq == alert_seq))
    }
    /// Count of occupied slots.
    pub fn occupied(&self) -> u32 {
        self.slots.iter().filter(|s| s.is_some()).count() as u32
    }
    /// RAW count of held alerts on `route` still inside THIS node's read
    /// window (spec §7, the media-live `route_evidence` body — Task 7's ONE
    /// shared read so the accessor and the ASSIGN site cannot drift):
    /// staleness anchors on `first_heard` at this node — the per-reader
    /// forgetting clock, never one synchronized world clock — and the count
    /// is unweighted (valence stays in the consumer, PDR-0006).
    pub fn count_route_recent(&self, route: usize, now: Tick, evidence_window: u64) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|a| {
                a.route as usize == route
                    && now.0.saturating_sub(a.first_heard.0) <= evidence_window
            })
            .count() as u32
    }
}

/// Which node heard (the GossipHeard carrier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GossipNode {
    Station(StationId),
    Craft(CraftId),
}

/// UNHASHED diagnostics (the `engagement_diag` pattern — never a behavior
/// input): eviction count + one `(tick, station_row)` record per dock EDGE
/// while media-live.
#[derive(Default)]
pub struct MediaDiag {
    pub evictions: u64,
    pub contacts: Vec<(Tick, u32)>,
}

// ---- pure mechanics (spec §3, §6): integer arithmetic, no draws ----

/// Significance in milli (0..=1000), computed at the READ — never stored
/// (spec §3). Linear in the claimed value with a floor, clamped to 1000.
pub fn sig_milli(claimed_value_micros: i64, m: &MediaCfg) -> u32 {
    ((claimed_value_micros / m.sig_divisor_micros.max(1)) + m.sig_floor_milli as i64)
        .clamp(m.sig_floor_milli as i64, 1000) as u32
}

/// Hop-attenuated transfer P (milli): `sig × (1000 − hop_loss)^hops / 1000^hops`
/// — integer pow by repeated floor-division (the telephone game's distance tax).
pub fn transfer_p_milli(claimed_value_micros: i64, hops: u8, m: &MediaCfg) -> u32 {
    let keep = 1000u64.saturating_sub(m.hop_loss_milli as u64);
    let mut p = sig_milli(claimed_value_micros, m) as u64;
    for _ in 0..hops {
        p = p * keep / 1000;
    }
    p as u32
}

/// Deterministic insert (spec §6), no draws: (a) lowest-index empty slot;
/// (b) lowest-index slot whose item has `(now − first_heard) > evidence_window`
/// — reclaimable space, NOT an eviction (forgetting has ONE owner, the read
/// window; eviction is purely overflow); (c) overflow: `priority = rob_tick +
/// claimed × value_ticks_milli / 1e6` (i64); evict the argmin (ties → lowest
/// `alert_seq`) iff the incoming priority exceeds it, else DROP the incoming.
/// Counts one eviction either way in (c). Returns whether `alert` was inserted.
pub fn insert_alert(
    buf: &mut GossipBuffer,
    alert: GossipAlert,
    now: Tick,
    evidence_window: u64,
    m: &MediaCfg,
    evictions: &mut u64,
) -> bool {
    let priority = |a: &GossipAlert| -> i64 {
        (a.rob_tick.0 as i64).saturating_add(
            a.claimed_value_micros.saturating_mul(m.value_ticks_milli as i64) / 1_000_000,
        )
    };
    // (a) lowest empty slot.
    if let Some(slot) = buf.slots.iter_mut().find(|s| s.is_none()) {
        *slot = Some(alert);
        return true;
    }
    // (b) lowest-index stale slot (strictly older than the read window).
    if let Some(slot) = buf
        .slots
        .iter_mut()
        .find(|s| s.is_some_and(|a| now.0.saturating_sub(a.first_heard.0) > evidence_window))
    {
        *slot = Some(alert);
        return true;
    }
    // (c) overflow: evict the argmin priority (ties -> lowest alert_seq) iff
    // the incoming strictly beats it, else drop the incoming. One eviction
    // either way.
    *evictions = evictions.saturating_add(1);
    let argmin = buf
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|a| (i, priority(&a), a.alert_seq)))
        .min_by_key(|&(_, p, seq)| (p, seq));
    if let Some((i, min_p, _)) = argmin
        && priority(&alert) > min_p
    {
        buf.slots[i] = Some(alert);
        return true;
    }
    false
}

// ---- propagation: edge-triggered dock exchange (spec §5, stage 3b2) ----

/// The receiver's copy on a successful transfer (spec §5): `hops + 1`
/// (saturating), `first_heard = now`, and — iff the RESULTING hops ≥ 2 — the
/// deterministic retelling inflation, capped. The sender is untouched;
/// first-heard sticks per node; re-hearing never re-inflates (dedupe kills
/// the evict-replant ratchet within retention).
fn receiver_copy(alert: GossipAlert, now: Tick, m: &MediaCfg) -> GossipAlert {
    let hops = alert.hops.saturating_add(1);
    let claimed = if hops >= 2 {
        (alert
            .claimed_value_micros
            .saturating_mul(1000 + m.inflation_milli as i64)
            / 1000)
            .min(m.claimed_value_cap_micros)
    } else {
        alert.claimed_value_micros
    };
    GossipAlert { hops, first_heard: now, claimed_value_micros: claimed, ..alert }
}

/// One candidate item crossing one direction of a dock edge: (1) dedupe FIRST
/// — a receiver that holds `alert_seq` consumes NO draw (the draw count stays
/// a pure function of hashed membership, so the Class-3 Media cursor is
/// transitively pinned); (2) ONE Media draw against the hop-attenuated
/// transfer P; (3) on transfer, insert the receiver copy; if inserted, emit
/// `GossipHeard` with the receiver's carrier node.
#[allow(clippy::too_many_arguments)]
fn try_transfer(
    alert: GossipAlert,
    receiver: &mut GossipBuffer,
    carrier: Option<GossipNode>,
    media: &MediaCfg,
    evidence_window: u64,
    rng: &mut RngStreams,
    now: Tick,
    events: &mut EventStream,
    evictions: &mut u64,
) {
    if receiver.holds(alert.alert_seq) {
        return; // dedupe first: NO draw
    }
    let u = (rng.stream(RngStream::Media).next_u64() % 1000) as u32;
    if u >= transfer_p_milli(alert.claimed_value_micros, alert.hops, media) {
        return;
    }
    let copy = receiver_copy(alert, now, media);
    if insert_alert(receiver, copy, now, evidence_window, media, evictions)
        && let Some(carrier) = carrier
    {
        events.emit(Event {
            tick: now,
            kind: EventKind::GossipHeard {
                carrier,
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

/// Edge-triggered dock gossip exchange (spec §5) — called from world stage
/// 3b2 BEFORE the `info_tick` refresh, dense craft-row order.
///
/// Per craft: skip unless docked (`dock_station[crow]`, the LOWEST station
/// row in radius) AND carrying a comms-log (`gossip[crow]` — the role filter:
/// pirates carry `None`, OD-6) AND on a dock EDGE — the load-bearing
/// predicate reads the PRE-refresh `info_tick`: within radius now AND not
/// docked last tick (`info_tick != now - 1`). Zero new state. (A craft docked
/// since reset first edges only after its first departure — vacuous, buffers
/// are empty at reset; radius oscillation re-fires — harmless, dedupe is
/// idempotent.)
///
/// On an edge: one `media_diag` contact record, then the PINNED direction
/// order — ship→station uploads (sender slot index order), THEN station→ship
/// downloads (sender slot index order, over the post-upload station content;
/// dedupe makes self-download a no-draw).
#[allow(clippy::too_many_arguments)]
pub fn run_gossip_exchange(
    ships: &mut CraftStore,
    station_gossip: &mut [GossipBuffer],
    stations: &StationStore,
    dock_station: &[Option<usize>],
    media: &MediaCfg,
    evidence_window: u64,
    rng: &mut RngStreams,
    now: Tick,
    events: &mut EventStream,
    diag: &mut MediaDiag,
) {
    for crow in 0..ships.ids.len() {
        let Some(srow) = dock_station.get(crow).copied().flatten() else {
            continue;
        };
        if ships.gossip[crow].is_none() {
            continue; // role filter: pirates are information-blind (OD-6)
        }
        // EDGE predicate (spec §5): pre-refresh info_tick.
        if ships.info_tick[crow] == Tick(now.0.saturating_sub(1)) {
            continue; // level, not edge: docked last tick too
        }
        if srow >= station_gossip.len() {
            continue; // totality degrade (spec §8): absurd partner row
        }
        diag.contacts.push((now, srow as u32));
        let station_node = stations
            .ids
            .id_at(srow)
            .map(|(slot, generation)| GossipNode::Station(StationId { slot, generation }));
        let craft_node = Some(GossipNode::Craft(ships.ids_at(crow)));
        // Ship→station uploads, sender slot index order (sender untouched, so
        // the snapshot is exact; the receiver mutates live for dedupe).
        let uploads: Vec<GossipAlert> = ships.gossip[crow]
            .as_ref()
            .map(|b| b.slots.iter().flatten().copied().collect())
            .unwrap_or_default();
        for alert in uploads {
            try_transfer(
                alert,
                &mut station_gossip[srow],
                station_node,
                media,
                evidence_window,
                rng,
                now,
                events,
                &mut diag.evictions,
            );
        }
        // Station→ship downloads over the POST-upload station content.
        let downloads: Vec<GossipAlert> =
            station_gossip[srow].slots.iter().flatten().copied().collect();
        let Some(cbuf) = ships.gossip[crow].as_mut() else {
            continue;
        };
        for alert in downloads {
            try_transfer(
                alert,
                cbuf,
                craft_node,
                media,
                evidence_window,
                rng,
                now,
                events,
                &mut diag.evictions,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{BaseSpec, MediaCfg};
    use crate::contract::EventKind;
    use crate::economy::StationStore;
    use crate::events::EventStream;
    use crate::ids::BodyId;
    use crate::math::Vec3;
    use crate::rng::{RngStream, RngStreams};
    use crate::stores::{CraftRole, CraftStore};
    use rand_core::Rng;

    fn alert(seq: u32, claimed: i64, rob: u64, heard: u64, hops: u8) -> GossipAlert {
        GossipAlert {
            alert_seq: seq,
            route: 1,
            pirate_slot: 7,
            rob_tick: Tick(rob),
            claimed_value_micros: claimed,
            first_heard: Tick(heard),
            hops,
        }
    }

    #[test]
    fn sig_clamps_to_floor_and_1000() {
        let m = MediaCfg::default(); // floor 50, divisor 10_000
        // Below the floor: claimed 0 (and even negative) reads the floor.
        assert_eq!(sig_milli(0, &m), 50);
        assert_eq!(sig_milli(-5_000_000, &m), 50);
        // Linear in the middle: 5.5M / 10k + 50 = 600.
        assert_eq!(sig_milli(5_500_000, &m), 600);
        // Above the cap: clamps to 1000.
        assert_eq!(sig_milli(100_000_000, &m), 1000);
    }

    #[test]
    fn transfer_p_hand_computed() {
        // sig 600 (claimed 5.5M: 550 + floor 50) at hop_loss 150 (keep 850):
        //   hops 0: 600
        //   hops 1: 600 * 850 / 1000 = 510
        //   hops 2: 510 * 850 / 1000 = 433500/1000 = 433 (integer floor)
        // — the P(escape) analytic anchor: P = 1 - (1-p)^k over dock EDGES.
        let m = MediaCfg::default();
        assert_eq!(transfer_p_milli(5_500_000, 0, &m), 600);
        assert_eq!(transfer_p_milli(5_500_000, 1, &m), 510);
        assert_eq!(transfer_p_milli(5_500_000, 2, &m), 433);
    }

    #[test]
    fn insert_prefers_empty_then_stale_then_priority() {
        let m = MediaCfg::default(); // value_ticks_milli 1000
        let mut ev = 0u64;
        let window = 100u64;
        let mut buf = GossipBuffer::empty(2);
        // (a) lowest empty slot first.
        assert!(insert_alert(&mut buf, alert(0, 1_000_000, 10, 10, 0), Tick(10), window, &m, &mut ev));
        assert_eq!(buf.slots[0].unwrap().alert_seq, 0);
        assert!(insert_alert(&mut buf, alert(1, 1_000_000, 10, 10, 0), Tick(10), window, &m, &mut ev));
        assert_eq!(buf.slots[1].unwrap().alert_seq, 1);
        assert_eq!(ev, 0, "filling empty slots is not eviction");
        // (b) a stale item (now - first_heard > window) is reclaimable space —
        // NOT an eviction (forgetting has one owner, the read window). Slot 0
        // goes stale at now 111; slot 1 stays fresh via a later first_heard.
        buf.slots[1].as_mut().unwrap().first_heard = Tick(60);
        assert!(insert_alert(&mut buf, alert(2, 1, 111, 111, 0), Tick(111), window, &m, &mut ev));
        assert_eq!(buf.slots[0].unwrap().alert_seq, 2, "lowest-index stale slot reclaimed");
        assert_eq!(buf.slots[1].unwrap().alert_seq, 1, "fresh item untouched");
        assert_eq!(ev, 0, "stale reclaim is not eviction");
        // (c) overflow, both fresh: priority = rob_tick + claimed*value_ticks/1e6.
        //   slot 0: rob 111 + 1*1000/1e6 = 111; slot 1: rob 10 + 1M*1000/1e6 = 1010.
        //   Incoming priority 111 + 0 = 111: NOT > argmin 111 -> DROP, 1 eviction.
        assert!(!insert_alert(&mut buf, alert(3, 0, 111, 111, 0), Tick(111), window, &m, &mut ev));
        assert_eq!(ev, 1, "the drop counts one eviction");
        assert_eq!(buf.slots[0].unwrap().alert_seq, 2, "incoming dropped, holder kept");
        //   Incoming priority 111 + 2*1 = wait — claimed 2M -> 111 + 2000? No:
        //   incoming rob 111, claimed 2_000_000 -> 111 + 2000 = 2111 > 111 ->
        //   evict the argmin (slot 0), 1 more eviction.
        assert!(insert_alert(&mut buf, alert(4, 2_000_000, 111, 111, 0), Tick(111), window, &m, &mut ev));
        assert_eq!(ev, 2, "the replace counts one eviction");
        assert_eq!(buf.slots[0].unwrap().alert_seq, 4, "argmin evicted, incoming landed");
    }

    #[test]
    fn insert_priority_tie_evicts_lowest_seq() {
        let m = MediaCfg::default();
        let mut ev = 0u64;
        let mut buf = GossipBuffer::empty(2);
        // Two holders with IDENTICAL priority (rob 5, claimed 1M -> 5 + 1000 =
        // 1005) but different seq; ties evict the LOWEST alert_seq.
        assert!(insert_alert(&mut buf, alert(9, 1_000_000, 5, 5, 0), Tick(5), 1000, &m, &mut ev));
        assert!(insert_alert(&mut buf, alert(3, 1_000_000, 5, 5, 0), Tick(5), 1000, &m, &mut ev));
        // Incoming strictly higher priority: rob 5, claimed 2M -> 2005.
        assert!(insert_alert(&mut buf, alert(11, 2_000_000, 5, 5, 0), Tick(5), 1000, &m, &mut ev));
        assert!(buf.holds(9), "higher seq kept on tie");
        assert!(!buf.holds(3), "lowest seq evicted on tie");
        assert!(buf.holds(11));
        assert_eq!(ev, 1);
    }

    // ---- exchange fixtures (the pirate.rs `fix()` pattern: hand-built
    // stores, one station, hauler-role craft with live comms-logs) ----

    struct XFix {
        ships: CraftStore,
        stations: StationStore,
        station_gossip: Vec<GossipBuffer>,
        media: MediaCfg,
        rng: RngStreams,
        events: EventStream,
        diag: MediaDiag,
    }

    fn xfix(n_craft: usize, master: u64) -> XFix {
        let mut stations = StationStore::empty();
        stations.push(BodyId { slot: 0, generation: 0 }, [0, 0], [0, 0]);
        let spec = || BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 0.0,
            base_exhaust_velocity: 1.0,
            base_fuel_capacity: 1.0,
            base_cargo_capacity: 5,
        };
        let mut ships = CraftStore::empty();
        for r in 0..n_craft {
            ships.push(spec(), Vec3::ZERO, Vec3::ZERO, 1.0);
            ships.role[r] = CraftRole::Hauler;
            ships.gossip[r] = Some(GossipBuffer::empty(8));
        }
        XFix {
            ships,
            stations,
            station_gossip: vec![GossipBuffer::empty(16)],
            media: MediaCfg {
                station_gossip_slots: 16,
                craft_gossip_slots: 8,
                ..MediaCfg::default()
            },
            rng: RngStreams::from_master(master),
            events: EventStream::new(),
            diag: MediaDiag::default(),
        }
    }

    /// One exchange pass at `now` with the given per-craft dock partners.
    fn exchange(f: &mut XFix, dock_station: &[Option<usize>], now: Tick) {
        run_gossip_exchange(
            &mut f.ships,
            &mut f.station_gossip,
            &f.stations,
            dock_station,
            &f.media,
            4000,
            &mut f.rng,
            now,
            &mut f.events,
            &mut f.diag,
        );
    }

    fn gossip_heard_count(f: &XFix) -> usize {
        f.events
            .events
            .iter()
            .filter(|e| matches!(e.kind, EventKind::GossipHeard { .. }))
            .count()
    }

    #[test]
    fn parked_n_ticks_produces_exactly_one_exchange() {
        // THE pinned unit test (spec §5): the edge predicate reads the
        // PRE-refresh info_tick, so a craft parked N ticks (info_tick refreshed
        // each tick by the world loop) exchanges exactly ONCE — per-tick
        // transfer would be the self-averaging bug rebuilt one layer up.
        let mut f = xfix(1, 7);
        // Station holds one big rumor: sig clamps to 1000 -> deterministic
        // transfer on the one draw the single edge is allowed.
        f.station_gossip[0].slots[0] = Some(alert(0, 30_000_000, 1, 1, 1));
        // The craft arrives docked at now=2 (info_tick 0 != 1: an edge), then
        // stays parked through now=6 with the world refreshing info_tick.
        for now in 2u64..=6 {
            exchange(&mut f, &[Some(0)], Tick(now));
            f.ships.info_tick[0] = Tick(now); // the world's post-exchange refresh
        }
        assert_eq!(f.diag.contacts.len(), 1, "exactly ONE contact for a parked craft");
        assert_eq!(f.diag.contacts[0], (Tick(2), 0), "the contact is the arrival edge");
        assert!(
            f.ships.gossip[0].as_ref().unwrap().holds(0),
            "the rumor crossed on the edge"
        );
        assert_eq!(gossip_heard_count(&f), 1, "at most one GossipHeard per alert");
    }

    #[test]
    fn dedupe_consumes_no_draw() {
        // Dedupe-first is load-bearing for determinism: the draw count must be
        // a pure function of hashed membership (the Class-3 cursor stays
        // transitively pinned). Receiver already holds the alert -> NO draw.
        const MASTER: u64 = 99;
        let mut f = xfix(1, MASTER);
        let a = alert(5, 30_000_000, 1, 1, 1);
        f.station_gossip[0].slots[0] = Some(a);
        f.ships.gossip[0].as_mut().unwrap().slots[0] = Some(a); // both hold seq 5
        exchange(&mut f, &[Some(0)], Tick(2));
        assert_eq!(f.diag.contacts.len(), 1, "the edge itself fired");
        assert_eq!(gossip_heard_count(&f), 0, "nothing new heard");
        // The stream-cursor equivalence trick: zero Media draws consumed.
        assert_eq!(
            f.rng.stream(RngStream::Media).next_u64(),
            RngStreams::from_master(MASTER).stream(RngStream::Media).next_u64(),
            "dedupe consumed ZERO Media draws"
        );
    }

    #[test]
    fn direction_order_is_ship_then_station() {
        // Station holds X (seq 1), craft holds Y (seq 2); both at p=500
        // (claimed 4.5M: 450 + floor 50, hops 0). Brute-force a master whose
        // FIRST Media draw FAILS a p=500 gate and SECOND PASSES: if uploads
        // draw first, the upload (Y->station) fails and the download
        // (X->craft) succeeds — observable only under the pinned order.
        let master = (0u64..)
            .find(|&m| {
                let mut r = RngStreams::from_master(m);
                let first = (r.stream(RngStream::Media).next_u64() % 1000) as u32;
                let second = (r.stream(RngStream::Media).next_u64() % 1000) as u32;
                first >= 500 && second < 500
            })
            .expect("such a master exists");
        let mut f = xfix(1, master);
        let x = alert(1, 4_500_000, 1, 1, 0);
        let y = GossipAlert { alert_seq: 2, ..x };
        f.station_gossip[0].slots[0] = Some(x);
        f.ships.gossip[0].as_mut().unwrap().slots[0] = Some(y);
        exchange(&mut f, &[Some(0)], Tick(2));
        assert!(
            !f.station_gossip[0].holds(2),
            "upload Y->station drew FIRST and failed"
        );
        assert!(
            f.ships.gossip[0].as_ref().unwrap().holds(1),
            "download X->craft drew SECOND and succeeded"
        );
    }

    #[test]
    fn inflation_only_on_retellings_and_capped() {
        // hop_loss 0 + claimed >= 9.5M -> sig 1000 -> every draw transfers:
        // the inflation arithmetic is isolated from the roll.
        let mut f = xfix(2, 11);
        f.media.hop_loss_milli = 0;
        const C: i64 = 9_999_999;
        f.ships.gossip[0].as_mut().unwrap().slots[0] = Some(alert(1, C, 1, 1, 0));
        // Craft 0 edge at now=2: upload hops 0 -> 1, NO inflation (firsthand).
        exchange(&mut f, &[Some(0), None], Tick(2));
        f.ships.info_tick[0] = Tick(2);
        let station_copy = f.station_gossip[0].slots.iter().flatten().next().expect("uploaded");
        assert_eq!(station_copy.hops, 1);
        assert_eq!(station_copy.claimed_value_micros, C, "hops 0->1 does NOT inflate");
        assert_eq!(station_copy.first_heard, Tick(2), "first_heard re-anchors per node");
        // Craft 1 edge at now=3: download hops 1 -> 2, x1.125 floor-division.
        exchange(&mut f, &[None, Some(0)], Tick(3));
        let retold = f.ships.gossip[1].as_ref().unwrap().slots[0].expect("downloaded");
        assert_eq!(retold.hops, 2);
        assert_eq!(
            retold.claimed_value_micros,
            C * 1125 / 1000, // 11_249_998 (floor)
            "the retelling inflates by inflation_milli"
        );
        // At the cap: clamped (fresh fixture, cap 10M).
        let mut g = xfix(2, 11);
        g.media.hop_loss_milli = 0;
        g.media.claimed_value_cap_micros = 10_000_000;
        g.ships.gossip[0].as_mut().unwrap().slots[0] = Some(alert(1, C, 1, 1, 0));
        exchange(&mut g, &[Some(0), None], Tick(2));
        g.ships.info_tick[0] = Tick(2);
        exchange(&mut g, &[None, Some(0)], Tick(3));
        let capped = g.ships.gossip[1].as_ref().unwrap().slots[0].expect("downloaded");
        assert_eq!(capped.claimed_value_micros, 10_000_000, "claims saturate at the cap");
    }

    #[test]
    fn evict_replant_cycle_terminates() {
        // A loop of inserts at the cap converges: every insert is total (insert
        // or drop, no panic), drops are deterministic, and replaying the same
        // sequence lands the same buffer.
        let m = MediaCfg::default();
        let run = || {
            let mut ev = 0u64;
            let mut buf = GossipBuffer::empty(2);
            for i in 0..50u32 {
                let claimed = ((i as i64) % 7) * 500_000;
                insert_alert(&mut buf, alert(i, claimed, 20, 20, 0), Tick(20), 1000, &m, &mut ev);
            }
            (buf, ev)
        };
        let (a, ev_a) = run();
        let (b, ev_b) = run();
        assert_eq!(a, b, "deterministic drops");
        assert_eq!(ev_a, ev_b);
        assert_eq!(a.occupied(), 2, "buffer stays at cap");
        assert!(ev_a > 0, "the cycle actually exercised overflow");
    }
}
