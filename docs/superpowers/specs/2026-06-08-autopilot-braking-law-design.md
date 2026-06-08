# Autopilot braking law — design

> Resolves filigree **jumpgate-ab98a67080**. Surfaced by Plan-3 Task 15: the v1
> autopilot has no deceleration law, so a craft at realistic speed overshoots and
> tunnels through the `ARRIVAL_RADIUS = 1e-4 AU` sphere and never arrives. The
> Task-15 arrival tests pass only in a contrived weak-thrust slow-approach regime.

## Problem

`autopilot_command(nav, pos, _vel, dest_pos, _eff)` (autopilot.rs) currently
ignores velocity and ship params: it points at the destination at full throttle
and cuts only once `pos` is inside `ARRIVAL_RADIUS`. With no braking:

- A craft accelerates the whole way, arrives at high speed, overshoots, and
  oscillates through the destination indefinitely (driven, undamped).
- Per-tick travel (~`v·dt`) far exceeds the `1e-4 AU` arrival sphere, so the
  point-in-sphere `arrival_crossed` predicate (events.rs) never fires —
  the craft tunnels through between tick boundaries.

## Goal

A deterministic guidance law that brings a craft to rest **at** a fixed
`Position`, and to a velocity-matched **rendezvous** at a moving **Body**, at
realistic thrust/distance, so `Arrival` fires reliably. Gravity-aware guidance
(thrust-vectoring against the local field) is explicitly **out of scope** for v1.
Craft→craft nav targets remain a best-effort stub (consistent with current v1,
which does not fully support craft destinations).

## The law (velocity-targeting with a sqrt braking profile)

Work in the **target's reference frame** so a moving body falls out for free.
Inputs (see signature change below): `pos`, `vel`, `dest_pos`, `dest_vel`,
`fuel_mass`, `eff`.

```
rel_pos = dest_pos - pos
d       = |rel_pos|
if d <= ARRIVAL_RADIUS or dv_remaining <= 0 → (ZERO, 0.0)   // arrived / budget gone
dir     = rel_pos / d                                        // unit, via normalize_or_zero
rel_vel = vel - dest_vel                                     // craft velocity in target frame
a_max   = eff.max_thrust / (eff.dry_mass + fuel_mass)        // true available thrust accel
v_brake = sqrt(2 * K_BRAKE * a_max * (d - ARRIVAL_RADIUS))   // max closing speed still stoppable
v_des   = min(V_CRUISE, v_brake) * dir                       // desired velocity (toward target)
v_err   = v_des - rel_vel
if |v_err| < V_ERR_EPS → (ZERO, 0.0)   // already velocity-matched: don't burn fuel for ~zero accel
thrust_dir = normalize_or_zero(v_err)
throttle   = 1.0
return (thrust_dir, throttle)
```

> **Why the `V_ERR_EPS` cut:** `thrust_accel_and_burn` (ship.rs) consumes fuel
> proportional to `throttle`, *independent of direction* — so commanding
> `throttle = 1.0` with a near-zero `v_err` would burn fuel for ~zero accel. When
> the craft already matches `v_des` (e.g. co-moving with a body), cut throttle.

Behaviour:
- **Far / slow:** `v_des` exceeds `rel_vel` → error points prograde → accelerate
  (capped at `V_CRUISE`).
- **Near / fast:** `v_brake` shrinks as `d→0`, so `v_des < rel_vel` → error points
  retrograde → **brake**. The craft reaches `d = ARRIVAL_RADIUS` with `rel_vel ≈ 0`.
- **Moving body:** because everything is in the target frame, "arrive at rest in
  the target frame" = match the body's velocity → the craft co-moves and stays
  inside the sphere rather than flying through.
- **Tangential velocity** is nulled too (the full `v_err` vector, not just the
  radial component, is commanded).

`K_BRAKE` (safety margin, `<1`, brake slightly early to absorb discretization
overshoot), `V_CRUISE` (closing-speed cap), and `V_ERR_EPS` (velocity-matched
deadband) are module constants tuned empirically during TDD; starting points
`K_BRAKE = 0.5`, `V_CRUISE = f64::INFINITY` (no cap unless the eccentric/transfer
tests want one), `V_ERR_EPS` small relative to expected closing speeds. They are
documented `pub const` so Task-15 / future tuning can reference them.

## Signature change (engine-internal)

`autopilot_command` is called only by `World::step` and its own unit tests; it is
**not** part of the public `StateView` contract, so widening it is low-cost.

```rust
pub fn autopilot_command(
    nav: NavState,
    pos: Vec3,
    vel: Vec3,            // now USED
    dest_pos: Vec3,
    dest_vel: Vec3,      // NEW: target velocity (0 for a fixed Position)
    fuel_mass: f64,      // NEW: so a_max = max_thrust / (dry_mass + fuel_mass)
    eff: &Effective,     // now USED (max_thrust, dry_mass)
) -> (Vec3, f64)
```

`World::step` resolves `dest_vel` alongside the existing `resolve_dest_pos`:
- `NavDest::Position(_)` → `Vec3::ZERO`
- `NavDest::Entity(Body(b))` → `ephemeris.body_vel(eph_index[b.slot], cur)` (the
  `body_vel(body_idx, tick)` accessor already exists, ephemeris.rs)
- `NavDest::Entity(Craft(c))` → that craft's stored `vel` (best-effort stub)

and passes the craft's current `fuel_mass[ci]` (already in hand at the call site).

## Determinism

- All ops (`sub`, `length`, `scale`, `normalize_or_zero`, `sqrt`, `min`) are
  already used in the hashed path. No `f64::mul_add` (banned); `2 * K_BRAKE *
  a_max * (d - ε)` stays a left-to-right product, not an FMA.
- The law is a pure function of its args → deterministic; replay re-feeds the
  same commands and reproduces identical thrust.
- The pinned `state_hash_golden_zero_world` is for an **Idle** craft (no command
  issued), so `autopilot_command` returns `(ZERO, 0)` on that path and the golden
  is **unaffected**. No `HASH_FORMAT_VERSION` change.
- `dv_remaining` accounting in `World::step` is unchanged (decrement by
  `thrust_accel.length() * dt` while throttling; cut at `≤ 0`).

## Arrival detection

With rendezvous braking the craft settles inside `ARRIVAL_RADIUS` at ~zero
relative velocity, so the existing `arrival_crossed` point-in-sphere edge test
fires cleanly — **no change needed**. CONTINGENCY: if re-tuning still shows
residual tunneling on a fast pass, add a swept segment-to-point closest-approach
test in `events.rs` (deterministic, reads the same prev/now state). Decide by
measurement, not up front.

## Testing (TDD order)

**autopilot.rs unit tests** (pure fn, synthetic inputs):
1. `points_toward_dest_when_far_and_slow` — far + zero vel → thrust ≈ toward dest, throttle 1 (adapt the existing test).
2. `brakes_when_overspeeding_toward_dest` — close + high closing speed → thrust points retrograde (dot(thrust_dir, dir_to_dest) < 0).
3. `arrives_with_low_relative_speed` — drive a 1-D sim loop with the law + a_max → final `|rel_vel|` small and `d ≤ ARRIVAL_RADIUS`, no overshoot past the band.
4. `matches_moving_target_velocity` — dest_vel ≠ 0, craft already co-moving at dest → near-zero thrust (already matched).
5. Keep `cuts_inside_arrival_radius`, `dv_exhaustion_stops_thrust`, `idle_never_thrusts` (update call sites for the new args).

**tests/physics_sanity.rs**:
6. Re-tune `fueled_autopilot_transfer_reaches_destination` and
   `transfer_arrival_tick_is_deterministic` to the **intended** regime (≈5 AU
   start, ≈0.5 AU hop, thrust ~1e-12, ~4000-tick budget) — drop the contrived
   300-AU/25k-tick workaround. The determinism `assert_eq!(a, b)` stays intact.
7. Add `transfer_to_moving_body_rendezvous` — command a craft to a `Body` target
   (a planet on an `a>0` orbit) and assert `Arrival` fires within budget.

**Regression:** full `cargo test -p jumpgate-core` (108 lib + replay + physics)
stays green; `cargo clippy --all-targets -- -D warnings` clean; full workspace
`cargo build` green.

## Implementation order

1. (red) Rewrite the autopilot unit tests for the new signature + braking
   behaviour; confirm they fail to compile / fail.
2. (green) Rewrite `autopilot_command` with the law + `pub const K_BRAKE`,
   `V_CRUISE`; pass autopilot tests.
3. Update `World::step`: resolve `dest_vel`, pass `fuel_mass`; full suite green.
4. Re-tune the two Task-15 arrival tests to the intended regime; add the
   moving-body rendezvous test; tune `K_BRAKE`/`V_CRUISE`/regime by measurement.
5. If residual tunneling appears, add the swept arrival test (contingency).
6. clippy + workspace build; update the `physics_sanity.rs` header + commit;
   close/annotate jumpgate-ab98a67080.

## Out of scope (deliberate)

Gravity-compensating guidance; craft→craft rendezvous; promoting tuned constants
into `config.rs`; any change to `state_hash` / `HASH_FIELD_ORDER`.
