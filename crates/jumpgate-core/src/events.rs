//! Typed event record stream (§4.4): one tick-stamped append-only stream, one
//! emit path, no reactivity. Task 10 lands the container; Task 11 adds the
//! boundary detectors (Arrival / FuelEmpty / Wake). The `action_ingested`
//! constructor deliberately lives in `ingest.rs`, not here, so the single
//! ingestion path does not create a Task-10 -> Task-11 module cycle.

use crate::contract::Event;
use crate::time::Tick;

/// Append-only, tick-ordered event stream. Emitters push in tick order.
pub struct EventStream {
    pub events: Vec<Event>,
}

impl EventStream {
    pub fn new() -> Self {
        EventStream { events: Vec::new() }
    }

    pub fn emit(&mut self, e: Event) {
        self.events.push(e);
    }

    /// All events with `tick >= t`, in emission order.
    pub fn since(&self, t: Tick) -> &[Event] {
        // events is tick-monotonic by construction, so the first index with
        // tick >= t starts the contiguous tail.
        let start = self.events.partition_point(|e| e.tick < t);
        &self.events[start..]
    }
}

impl Default for EventStream {
    fn default() -> Self {
        EventStream::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::EventKind;
    use crate::ids::CraftId;

    #[test]
    fn since_returns_tick_tail() {
        let mut s = EventStream::new();
        let cid = CraftId { slot: 0, generation: 1 };
        s.emit(Event { tick: Tick(1), kind: EventKind::FuelEmpty { craft: cid } });
        s.emit(Event { tick: Tick(3), kind: EventKind::FuelEmpty { craft: cid } });
        s.emit(Event { tick: Tick(3), kind: EventKind::FuelEmpty { craft: cid } });
        assert_eq!(s.since(Tick(0)).len(), 3);
        assert_eq!(s.since(Tick(3)).len(), 2);
        assert_eq!(s.since(Tick(4)).len(), 0);
    }
}
