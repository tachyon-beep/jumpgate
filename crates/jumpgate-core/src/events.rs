//! Typed event record stream (§4.4): one tick-stamped append-only stream, one
//! emit path, no reactivity. Task 10 lands the container; Task 11 adds the
//! boundary detectors (Arrival / FuelEmpty / Wake). The `action_ingested`
//! constructor deliberately lives in `ingest.rs`, not here, so the single
//! ingestion path does not create a Task-10 -> Task-11 module cycle.

use crate::autopilot::ARRIVAL_RADIUS;
use crate::contract::{Event, EventKind};
use crate::ephemeris::Ephemeris;
use crate::math::Vec3;
use crate::stores::{BodyStore, CraftStore, NavState};
use crate::time::Tick;
use crate::types::{EntityRef, NavDest};

/// Quantization epsilon for fuel comparisons (fuel at/below this == "empty").
pub const FUEL_EMPTY_EPS: f64 = 1e-9;

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

/// Pure fuel-empty predicate. True iff fuel is at/below epsilon now AND was
/// strictly above epsilon at the previous tick (the depletion edge, fires once).
fn fuel_just_emptied(fuel_now: f64, fuel_prev: f64) -> bool {
    fuel_now <= FUEL_EMPTY_EPS && fuel_prev > FUEL_EMPTY_EPS
}

/// Pure arrival predicate. True iff the craft is within ARRIVAL_RADIUS of
/// dest_pos now AND was outside it at the previous tick (a crossing, fires once).
fn arrival_crossed(pos: Vec3, dest_pos: Vec3, prev_inside: bool) -> bool {
    let inside = pos.sub(dest_pos).length() <= ARRIVAL_RADIUS;
    inside && !prev_inside
}

/// Detect Arrival/FuelEmpty at the tick boundary against QUANTIZED state and
/// record them into `out`. Pure read: never mutates any store (enforced by the
/// shared `&` refs). No reactivity — each predicate reads only state, and
/// emitting an event cannot trigger another same-tick event.
///
/// Reads the prior-tick snapshot from `CraftStore::prev_fuel` /
/// `CraftStore::prev_inside_dest` (both index-aligned, populated in Task 4 and
/// part of the canonical hashed state). Resolves an entity destination via
/// `BodyStore::eph_index` + `Ephemeris::body_pos`.
pub fn detect_boundary_events(
    ships: &CraftStore,
    bodies: &BodyStore,
    ephem: &Ephemeris,
    tick: Tick,
    out: &mut EventStream,
) {
    for idx in 0..ships.ids.len() {
        // Index -> stable CraftId via the Task-4 live-id accessor (see Depends-on).
        let id = ships.ids_at(idx);

        // FuelEmpty edge: fuel at/below eps now, above eps at prior tick.
        if fuel_just_emptied(ships.fuel_mass[idx], ships.prev_fuel[idx]) {
            out.emit(Event {
                tick,
                kind: EventKind::FuelEmpty { craft: id },
            });
        }

        // Arrival edge: only meaningful while Seeking toward a resolved dest.
        if let NavState::Seeking { dest, .. } = ships.nav[idx] {
            let dest_pos = match dest {
                NavDest::Position(p) => p,
                NavDest::Entity(EntityRef::Body(body_id)) => {
                    ephem.body_pos(bodies.eph_index[body_id.slot as usize], tick)
                }
                // Entity(Craft) destinations are not a v1 nav target; skip.
                NavDest::Entity(EntityRef::Craft(_)) => continue,
            };
            if arrival_crossed(ships.pos[idx], dest_pos, ships.prev_inside_dest[idx]) {
                out.emit(Event {
                    tick,
                    kind: EventKind::Arrival { craft: id, dest },
                });
            }
        }
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

    #[test]
    fn fuel_just_emptied_fires_only_on_depletion_edge() {
        // Was fuelled, now empty -> edge, fires.
        assert!(fuel_just_emptied(0.0, 0.5));
        // At/below eps now, above eps before -> still the edge.
        assert!(fuel_just_emptied(FUEL_EMPTY_EPS, 0.5));
        assert!(fuel_just_emptied(FUEL_EMPTY_EPS * 0.5, 1.0));
        // Already empty last tick -> does NOT fire again.
        assert!(!fuel_just_emptied(0.0, 0.0));
        assert!(!fuel_just_emptied(0.0, FUEL_EMPTY_EPS));
        // Still fuelled -> does not fire.
        assert!(!fuel_just_emptied(0.4, 0.5));
    }

    #[test]
    fn arrival_crossing_contract_documented() {
        use crate::math::Vec3;
        let dest = Vec3::new(10.0, 0.0, 0.0);
        let radius = 0.5_f64;

        // Helper mirrors arrival_crossed but with an explicit radius for the test.
        let crossed = |pos: Vec3, prev_inside: bool| {
            let inside = pos.sub(dest).length() <= radius;
            inside && !prev_inside
        };

        // Outside last tick, inside now -> crossing, fires.
        assert!(crossed(Vec3::new(10.2, 0.0, 0.0), false));
        // Inside last tick, inside now -> no new crossing.
        assert!(!crossed(Vec3::new(10.1, 0.0, 0.0), true));
        // Outside now -> never fires regardless of prior.
        assert!(!crossed(Vec3::new(11.0, 0.0, 0.0), false));
        assert!(!crossed(Vec3::new(11.0, 0.0, 0.0), true));
    }

    #[test]
    fn arrival_crossed_uses_arrival_radius_constant() {
        use crate::autopilot::ARRIVAL_RADIUS;
        use crate::math::Vec3;
        let dest = Vec3::new(0.0, 0.0, 0.0);
        // A point just inside ARRIVAL_RADIUS, coming from outside -> fires.
        let just_inside = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
        assert!(arrival_crossed(just_inside, dest, false));
        // Same point, already inside -> no repeat.
        assert!(!arrival_crossed(just_inside, dest, true));
        // A point well outside -> never.
        let outside = Vec3::new(ARRIVAL_RADIUS * 10.0 + 1.0, 0.0, 0.0);
        assert!(!arrival_crossed(outside, dest, false));
    }
}
