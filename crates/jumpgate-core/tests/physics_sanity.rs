//! Physics sanity + autopilot transfer integration tests.
//!
//! Bounded (not golden) checks over the full `World`:
//!   1. near-circular orbit stays bounded over many orbits,
//!   2. eccentric close-approach does NOT blow up (substepping + softening),
//!   3. pure-coast specific-orbital-energy drift is bounded,
//!   4. a fuel-budgeted autopilot transfer reaches its destination
//!      deterministically (same config -> same arrival tick).
//!
//! RESOLVED §11 TUNING (v1 defaults, measured here; promote into config.rs later):
//!   * dt              = 0.25 day
//!   * softening (eps) = 1.0e-3 AU
//!   * substep_cfg     = { accel_ref: 1.0e-3, max_substeps: 64 }
//!
//! Rationale: at 1 AU a near-circular orbit (period ~365 d) gets ~1460 ticks/orbit,
//! Verlet stays well-bounded at N == 1. The acceleration-keyed LOG2 substep schedule
//! (Task 8: N = clamp(1 + floor(log2(max(1.0, total_accel_mag / accel_ref))), 1, max_substeps))
//! supplies extra accuracy ONLY where the field is steep: with accel_ref = 1e-3,
//! the eccentric apoapsis (accel ~8.2e-5) and the circular orbit (accel ~3.0e-4)
//! stay at N == 1, while the eccentric periapsis (accel ~3.0e-2, ratio ~30) climbs to N ~= 5
//! -- so case (2) genuinely exercises substepping. CAVEAT: the log2 schedule is
//! gentle (periapsis tops out near N ~= 5 here, ~9 at accel_ref=1e-4, ~12 at 1e-5;
//! it CANNOT reach the ~30 substeps the old linear formula gave). accel_ref is a
//! weak lever; the PRIMARY lever for keeping the e=0.9 orbit bounded is a smaller
//! base DT_DAYS (and softening eps). Whether case (2) stays bounded under log2
//! substepping is an empirical tuning question to settle by lowering DT_DAYS first.
//!
//! AUTOPILOT TRANSFER (cases 4/5) -- measured tuning: the v1 autopilot has NO
//! braking law (autopilot.rs: thrust toward dest, cut only inside ARRIVAL_RADIUS
//! = 1e-4 AU). A fast craft TUNNELS through that 1e-4 AU sphere between tick
//! boundaries and never registers an Arrival. The pass/fail quantity is
//! `v_arrival * dt` vs ARRIVAL_RADIUS: a per-tick step larger than 1e-4 AU never
//! lands a tick boundary inside the sphere. With dt fixed at the v1 default
//! (0.25 day), a clean arrival therefore demands a SLOW approach: a very weak
//! thrust accel in a far-out (negligible-gravity) region over a short hop, so the
//! craft creeps in and a tick boundary falls inside 1e-4 AU. Measured operating
//! point: craft at 300 AU (central accel ~ G*M/300^2 ~ 3.3e-9), thrust accel
//! ~7.4e-8 AU/day^2 (>> local gravity, << old plan default), 0.5 AU hop, which
//! gives v_arrival ~ sqrt(2*7.4e-8*0.5) ~ 2.7e-4 AU/day -> per-tick step ~6.8e-5
//! AU < 1e-4: a boundary lands inside. Needs a large tick budget (~25k) because
//! the creep is slow.
//!
//! Upstream dependencies: Task 7's gravity_accel must honour the a == 0.0 star
//! guard (no NaN from the star's own slot) and Task 8's substep formula must
//! engage on the quantized total accel magnitude; the is_finite() asserts below
//! would otherwise report an upstream regression as a (false) physics blowup.

use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, CraftInit, Dt, EntityRef, EventKind,
    G_CANONICAL, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg, Target,
    Tick, Vec3, World,
};

// ---- resolved v1 tuning defaults ----
const DT_DAYS: f64 = 0.25;
const SOFTENING: f64 = 1.0e-3;
const SUBSTEP_CFG: SubstepCfg = SubstepCfg { accel_ref: 1.0e-3, max_substeps: 64 };

/// A massive star pinned at the origin (a == 0 => Kepler conic degenerates to a
/// fixed point at the focus), plus a caller-supplied set of craft. One body only,
/// so the gravity field a craft feels is the clean central term G*M/(r^2+eps^2)^1.5.
fn star_config(seed: u64, star_mass: f64, window: u64, craft: Vec<CraftInit>) -> RunConfig {
    RunConfig {
        master_seed: seed,
        dt: Dt::new(DT_DAYS),
        softening: SOFTENING,
        substep_cfg: SUBSTEP_CFG,
        ephemeris_window: window,
        bodies: vec![BodyInit {
            mass: star_mass,
            elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
        }],
        craft,
    }
}

/// A coasting craft: zero fuel so the autopilot/thrust path is inert and the
/// trajectory is pure gravity.
fn coasting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
    CraftInit {
        spec: BaseSpec {
            base_dry_mass: 1.0e-12,        // ~negligible vs M_sun; craft exerts no gravity anyway
            base_max_thrust: 0.0,
            base_exhaust_velocity: 1.0e-2,
            base_fuel_capacity: 0.0,
        },
        pos,
        vel,
        fuel_mass: 0.0,
    }
}

#[test]
fn circular_orbit_stays_bounded_over_many_orbits() {
    let m: f64 = 1.0; // M_sun
    let r0: f64 = 1.0; // AU
    let v_circ = (G_CANONICAL * m / r0).sqrt(); // AU/day
    // place at +x, velocity +y => prograde circular orbit in the z=0 plane
    let craft = vec![coasting_craft(
        Vec3::new(r0, 0.0, 0.0),
        Vec3::new(0.0, v_circ, 0.0),
    )];

    let period_days = std::f64::consts::TAU / (G_CANONICAL * m / (r0 * r0 * r0)).sqrt();
    let ticks_per_orbit = (period_days / DT_DAYS).ceil() as u64;
    let n_orbits: u64 = 10;
    let total_ticks = ticks_per_orbit * n_orbits;

    let (mut world, _cfg_hash) = World::reset(star_config(1, m, total_ticks + 8, craft));
    let cid = world.craft_ids()[0];

    let mut r_min = f64::INFINITY;
    let mut r_max = 0.0_f64;
    let mut cmds: Vec<Command> = Vec::new();
    for _ in 0..total_ticks {
        world.step(&mut cmds);
        let p = world.craft_pos(cid).expect("craft alive");
        assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite(), "position went non-finite");
        let r = p.length();
        if r < r_min { r_min = r; }
        if r > r_max { r_max = r; }
    }
    // bounded: radius never drifts more than 5% off the initial circular radius
    assert!(r_min > 0.95 * r0, "orbit decayed inward: r_min = {r_min}");
    assert!(r_max < 1.05 * r0, "orbit grew outward: r_max = {r_max}");
}

#[test]
fn eccentric_close_approach_does_not_blow_up() {
    let m: f64 = 1.0;
    let a: f64 = 1.0;   // semi-major axis (AU)
    let e: f64 = 0.9;   // high eccentricity => periapsis r_p = a(1-e) = 0.1 AU
    let r_apo = a * (1.0 + e);                 // 1.9 AU, start here
    // vis-viva: v^2 = G*M*(2/r - 1/a); at apoapsis velocity is purely tangential
    let v_apo = (G_CANONICAL * m * (2.0 / r_apo - 1.0 / a)).sqrt();
    let craft = vec![coasting_craft(
        Vec3::new(r_apo, 0.0, 0.0),
        Vec3::new(0.0, v_apo, 0.0),
    )];

    let period_days = std::f64::consts::TAU * (a * a * a / (G_CANONICAL * m)).sqrt();
    let total_ticks = (5.0 * period_days / DT_DAYS).ceil() as u64; // 5 orbits incl. 5 periapsis passes

    let (mut world, _h) = World::reset(star_config(2, m, total_ticks + 8, craft));
    let cid = world.craft_ids()[0];

    let mut r_min = f64::INFINITY;
    let mut r_max = 0.0_f64;
    let mut cmds: Vec<Command> = Vec::new();
    for _ in 0..total_ticks {
        world.step(&mut cmds);
        let p = world.craft_pos(cid).expect("craft alive");
        assert!(
            p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
            "close approach produced a non-finite position (blowup or upstream a==0 star NaN)"
        );
        let r = p.length();
        if r < r_min { r_min = r; }
        if r > r_max { r_max = r; }
    }
    // engaged substepping kept periapsis off the singularity ...
    assert!(r_min > 0.5 * a * (1.0 - e), "periapsis collapsed: r_min = {r_min}");
    // ... and did not get slingshot to escape (bound orbit stays near apoapsis scale)
    assert!(r_max < 3.0 * r_apo, "trajectory blew outward: r_max = {r_max}");
}

#[test]
fn coast_specific_energy_drift_is_bounded() {
    let m: f64 = 1.0;
    let r0: f64 = 1.0;
    let v_circ = (G_CANONICAL * m / r0).sqrt();
    let craft = vec![coasting_craft(
        Vec3::new(r0, 0.0, 0.0),
        Vec3::new(0.0, v_circ, 0.0),
    )];

    let period_days = std::f64::consts::TAU / (G_CANONICAL * m / (r0 * r0 * r0)).sqrt();
    let total_ticks = (period_days / DT_DAYS).ceil() as u64; // one orbit

    let (mut world, _h) = World::reset(star_config(3, m, total_ticks + 8, craft));
    let cid = world.craft_ids()[0];

    let energy = |p: Vec3, v: Vec3| -> f64 {
        0.5 * v.length_sq() - G_CANONICAL * m / p.length()
    };
    let e0 = energy(
        world.craft_pos(cid).unwrap(),
        world.craft_vel(cid).unwrap(),
    );

    let mut cmds: Vec<Command> = Vec::new();
    for _ in 0..total_ticks {
        world.step(&mut cmds);
    }
    let e1 = energy(
        world.craft_pos(cid).unwrap(),
        world.craft_vel(cid).unwrap(),
    );

    let rel_drift = ((e1 - e0) / e0).abs();
    assert!(rel_drift < 1.0e-2, "energy drift too large over one orbit: {rel_drift}");
}

/// A craft with real thrust + fuel, in a weak-gravity region so the autopilot's
/// guidance dominates AND the approach is slow enough to land a tick boundary
/// inside ARRIVAL_RADIUS (no braking law -> a fast craft tunnels through). The
/// thrust is deliberately tiny so v_arrival * dt < ARRIVAL_RADIUS.
fn thrusting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
    CraftInit {
        spec: BaseSpec {
            base_dry_mass: 1.0e-9,
            // F / (dry+fuel == 2e-9) ~ 7.4e-8 AU/day^2: >> local gravity at 300 AU
            // (~3.3e-9) but slow enough that v_arrival*dt < ARRIVAL_RADIUS (no braking).
            base_max_thrust: 1.48e-16,
            base_exhaust_velocity: 1.0e-2,
            base_fuel_capacity: 1.0e-9,
        },
        pos,
        vel,
        fuel_mass: 1.0e-9,               // full tank => ample dv budget for a short hop
    }
}

/// Run a transfer to `dest` and return Some(arrival_tick) if an Arrival event for
/// the (single) craft fired within `max_ticks`, else None.
fn run_transfer(seed: u64, start: Vec3, dest: Vec3, budget: Option<f64>, max_ticks: u64)
    -> Option<u64>
{
    let craft = vec![thrusting_craft(start, Vec3::ZERO)];
    let (mut world, _h) = World::reset(star_config(seed, 1.0, max_ticks + 8, craft));
    let cid = world.craft_ids()[0];

    // single ingestion path: command the destination once at tick 0
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(cid)),
        kind: CommandKind::Destination { dest: NavDest::Position(dest), burn_budget: budget },
    }];
    world.step(&mut cmds); // tick 0 ingests + integrates

    let mut last_seen = Tick(0);
    loop {
        for ev in world.recent_events(last_seen) {
            if let EventKind::Arrival { craft: ac, .. } = ev.kind
                && ac == cid
            {
                return Some(ev.tick.0);
            }
        }
        last_seen = world.tick();
        if world.tick().0 >= max_ticks {
            return None;
        }
        let mut none: Vec<Command> = Vec::new();
        world.step(&mut none);
    }
}

#[test]
fn fueled_autopilot_transfer_reaches_destination() {
    // far-out, negligible-gravity region (300 AU): central accel ~ G*M/300^2 ~ 3.3e-9,
    // thrust accel ~7.4e-8 dominates yet stays slow enough to avoid tunnelling.
    let start = Vec3::new(300.0, 0.0, 0.0);
    let dest = Vec3::new(300.5, 0.0, 0.0); // 0.5 AU hop
    let arrival = run_transfer(11, start, dest, Some(1.0), 25_000);
    assert!(arrival.is_some(), "craft never emitted Arrival within budget");
}

#[test]
fn transfer_arrival_tick_is_deterministic() {
    let start = Vec3::new(300.0, 0.0, 0.0);
    let dest = Vec3::new(300.5, 0.0, 0.0);
    let a = run_transfer(11, start, dest, Some(1.0), 25_000);
    let b = run_transfer(11, start, dest, Some(1.0), 25_000);
    assert!(a.is_some(), "first run did not arrive");
    assert_eq!(a, b, "same config produced different arrival ticks: {a:?} vs {b:?}");
}
