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

/// Relative-speed gate (canonical AU/day) distinguishing a velocity-matched
/// rendezvous (fires Arrival) from a fast flyby that merely grazes the sphere
/// (must NOT fire). Class-1 const (D11); affects ONLY Arrival event timing, never
/// state_hash. Starting value = the old V_CRUISE magnitude; pin by measurement (§10).
pub const ARRIVAL_SPEED: f64 = 2.0e-3;

/// Degeneracy epsilon for the swept chord (target-frame chord length^2 below this
/// is treated as a stationary rendezvous; §5.3).
const DD_EPS: f64 = 1.0e-30;

/// Swept arrival predicate (§5.2): closest approach of the craft↔target chord in
/// the TARGET frame, gated by rel_speed. `c_prev`/`c_now` are the target position
/// at tick T-1 / T (equal, for a fixed Position). All ops are Vec3 + scalar; NO FMA.
fn arrival_swept(
    prev_pos: Vec3,
    pos: Vec3,
    c_prev: Vec3,
    c_now: Vec3,
    rel_speed: f64,
    prev_inside: bool,
) -> bool {
    let a = prev_pos.sub(c_prev); // craft offset from target at chord start
    let b = pos.sub(c_now); // craft offset from target at chord end
    let d = b.sub(a);
    let dd = d.dot(d);
    let r = ARRIVAL_RADIUS;
    let min_sq = if dd <= DD_EPS {
        b.dot(b) // degenerate / rendezvous: endpoint point-in-sphere (§5.3)
    } else {
        // Explicit max-then-min (NOT `.clamp`): clamp's NaN/sign-of-zero semantics
        // differ, and this is the reviewed determinism-path form. The verdict is
        // sign-of-zero-invariant anyway (t flows into closest.dot(closest), which
        // squares the sign away), but we keep the exact reviewed arithmetic.
        #[allow(clippy::manual_clamp)]
        let t = ((-(a.dot(d))) / dd).max(0.0).min(1.0); // clamp closest-approach param to [0,1]
        let closest = a.add(d.scale(t));
        closest.dot(closest)
    };
    let inside_now = (min_sq <= r * r) && (rel_speed <= ARRIVAL_SPEED);
    inside_now && !prev_inside
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
        // Swept test (§5): resolve the target at BOTH Tick(T-1) and Tick(T) and
        // gate by the craft's speed relative to the target. T >= 1 always
        // (detection runs with `next`), so Tick(T-1) never underflows.
        if let NavState::Seeking { dest, .. } = ships.nav[idx] {
            let (c_prev, c_now, dest_vel) = match dest {
                NavDest::Position(p) => (p, p, Vec3::ZERO),
                NavDest::Entity(EntityRef::Body(body_id)) => {
                    let eidx = bodies.eph_index[body_id.slot as usize];
                    let prev_tick = Tick(tick.0 - 1);
                    (
                        ephem.body_pos(eidx, prev_tick),
                        ephem.body_pos(eidx, tick),
                        ephem.body_vel(eidx, tick),
                    )
                }
                // Entity(Craft) destinations are not a v1 nav target; skip.
                NavDest::Entity(EntityRef::Craft(_)) => continue,
            };
            let rel_speed = ships.vel[idx].sub(dest_vel).length();
            if arrival_swept(
                ships.prev_pos[idx],
                ships.pos[idx],
                c_prev,
                c_now,
                rel_speed,
                ships.prev_inside_dest[idx],
            ) {
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

    use crate::math::Vec3;

    // Helper mirrors detect_boundary_events' call for a fixed Position target
    // (c_prev == c_now == dest, dest_vel == 0).
    fn swept_fixed(prev_pos: Vec3, pos: Vec3, dest: Vec3, vel: Vec3, prev_inside: bool) -> bool {
        let rel_speed = vel.sub(Vec3::ZERO).length();
        arrival_swept(prev_pos, pos, dest, dest, rel_speed, prev_inside)
    }

    #[test]
    fn swept_fires_when_point_test_would_miss() {
        // Chord passes THROUGH the sphere at the origin between ticks; neither endpoint
        // is inside R=1e-4, but the closest approach is. Low rel_speed -> fires.
        let dest = Vec3::ZERO;
        let prev_pos = Vec3::new(-1.0e-3, 0.0, 0.0);
        let pos = Vec3::new(1.0e-3, 0.0, 0.0);
        let slow = Vec3::new(1.0e-5, 0.0, 0.0); // |v| < ARRIVAL_SPEED
        assert!(swept_fixed(prev_pos, pos, dest, slow, false));
    }

    #[test]
    fn fast_flyby_does_not_fire() {
        // Same geometric clip, but rel_speed above the gate -> suppressed.
        let dest = Vec3::ZERO;
        let prev_pos = Vec3::new(-1.0e-3, 0.0, 0.0);
        let pos = Vec3::new(1.0e-3, 0.0, 0.0);
        let fast = Vec3::new(1.0, 0.0, 0.0); // |v| >> ARRIVAL_SPEED
        assert!(!swept_fixed(prev_pos, pos, dest, fast, false));
    }

    #[test]
    fn velocity_matched_rendezvous_fires() {
        // Two independent facts clear arrival here. (1) prev_pos == pos, so the
        // target-frame chord is zero-length -> dd ~ 0 -> the `dd <= DD_EPS` endpoint
        // branch is taken (this is what would NaN-out without the DD_EPS guard, §5.3;
        // dd is the squared chord length, NOT the rel_speed). (2) the independently
        // passed rel_speed (|v| = 1e-6) clears the `rel_speed <= ARRIVAL_SPEED` gate.
        let dest = Vec3::ZERO;
        let inside = Vec3::new(0.5e-4, 0.0, 0.0); // within R
        assert!(swept_fixed(inside, inside, dest, Vec3::new(1.0e-6, 0.0, 0.0), false));
    }

    #[test]
    fn tick0_zero_length_chord_does_not_fire_outside() {
        // prev_pos == pos and outside R -> no spurious fire.
        let dest = Vec3::ZERO;
        let outside = Vec3::new(1.0e-2, 0.0, 0.0);
        assert!(!swept_fixed(outside, outside, dest, Vec3::ZERO, false));
    }

    #[test]
    fn arrival_speed_gate_boundary_pins_comparison_direction() {
        // The gate is `rel_speed <= ARRIVAL_SPEED`. Pin the boundary so a future
        // flip to `<` / `>=` is caught. Geometry inside R, vary only rel_speed.
        let dest = Vec3::ZERO;
        let inside = Vec3::new(0.5e-4, 0.0, 0.0);
        // Just under the gate -> fires.
        assert!(swept_fixed(inside, inside, dest, Vec3::new(ARRIVAL_SPEED - 1e-9, 0.0, 0.0), false));
        // Strictly over the gate -> does not fire.
        assert!(!swept_fixed(inside, inside, dest, Vec3::new(ARRIVAL_SPEED + 1e-3, 0.0, 0.0), false));
    }

    #[test]
    fn swept_latch_suppresses_repeat_when_prev_inside() {
        // Already delivered last tick (prev_inside = true) -> the once-only latch
        // suppresses a second fire even though geometry+speed would otherwise qualify.
        let dest = Vec3::ZERO;
        let inside = Vec3::new(0.5e-4, 0.0, 0.0);
        assert!(!swept_fixed(inside, inside, dest, Vec3::new(1.0e-6, 0.0, 0.0), true));
    }

    #[test]
    fn arrival_crossing_contract_documented() {
        // The once-only crossing latch, routed through the live `arrival_swept`
        // predicate (via `swept_fixed`). Geometry drives the verdict: low rel_speed
        // clears the gate so only the inside/prev_inside transition decides. This
        // pins the latch contract (outside->inside fires; inside->inside does not;
        // outside never fires) against the REAL predicate, not a re-implemented
        // point-in-sphere closure.
        let dest = Vec3::new(10.0, 0.0, 0.0);
        // Zero-length chords at fixed offsets from `dest`; `slow` clears ARRIVAL_SPEED.
        let slow = Vec3::new(1.0e-6, 0.0, 0.0);
        let inside_pt = dest.add(Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0));
        let outside_pt = dest.add(Vec3::new(ARRIVAL_RADIUS * 10.0 + 1.0, 0.0, 0.0));

        // Inside now, was outside last tick -> crossing, fires.
        assert!(swept_fixed(inside_pt, inside_pt, dest, slow, false));
        // Inside now, was inside last tick -> latch suppresses a second fire.
        assert!(!swept_fixed(inside_pt, inside_pt, dest, slow, true));
        // Outside now -> never fires, regardless of prior inside state.
        assert!(!swept_fixed(outside_pt, outside_pt, dest, slow, false));
        assert!(!swept_fixed(outside_pt, outside_pt, dest, slow, true));
    }

    #[test]
    fn arrival_swept_uses_arrival_radius_constant() {
        use crate::autopilot::ARRIVAL_RADIUS;
        let dest = Vec3::new(0.0, 0.0, 0.0);
        // A point just inside ARRIVAL_RADIUS, zero-length chord, low rel_speed,
        // coming from outside -> fires.
        let just_inside = Vec3::new(ARRIVAL_RADIUS * 0.5, 0.0, 0.0);
        assert!(swept_fixed(just_inside, just_inside, dest, Vec3::ZERO, false));
        // Same point, already inside -> no repeat (once-only latch).
        assert!(!swept_fixed(just_inside, just_inside, dest, Vec3::ZERO, true));
        // A point well outside -> never (zero-length chord, closest approach is the
        // point itself, outside R).
        let outside = Vec3::new(ARRIVAL_RADIUS * 10.0 + 1.0, 0.0, 0.0);
        assert!(!swept_fixed(outside, outside, dest, Vec3::ZERO, false));
    }
}
