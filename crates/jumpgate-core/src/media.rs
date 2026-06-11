//! Media rung cut 1: gossip data structures (spec §2). EVIDENCE ONLY — raw
//! ticks and claimed integers; significance/staleness are computed at the
//! READ, never stored. No heat, threat, or confidence fields — ever (PDR-0006).

use crate::ids::{CraftId, StationId};
use crate::time::Tick;

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
