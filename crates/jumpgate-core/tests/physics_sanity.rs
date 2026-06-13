//! Physics sanity + autopilot transfer integration tests.
//!
//! Bounded (not golden) checks over the full `World`:
//!   1. near-circular orbit stays bounded over many orbits,
//!   2. eccentric close-approach does NOT blow up (substepping + softening),
//!   3. pure-coast specific-orbital-energy drift is bounded,
//!   4. a fuel-budgeted autopilot transfer reaches its destination
//!      deterministically (same config -> same arrival tick),
//!   5. a velocity-matched rendezvous at a MOVING body settles inside the
//!      arrival sphere (braking law co-moves with the target).
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
//! AUTOPILOT TRANSFER (cases 4/5/6) -- the v1 autopilot now has a velocity-targeting
//! BRAKING law (autopilot.rs, K_BRAKE/V_CRUISE/V_ERR_EPS): it brakes on a sqrt
//! deceleration profile so the craft arrives at REST at a fixed Position and
//! velocity-matched at a moving Body, settling inside ARRIVAL_RADIUS instead of
//! tunnelling through it. The old contrived 300-AU/1.48e-16-thrust/25k-tick
//! slow-creep workaround (which only worked because v_arrival*dt happened to land
//! a boundary inside 1e-4 AU) is gone. The INTENDED regime, measured here:
//!   * dt = 0.25 day, K_BRAKE=0.5, V_CRUISE=2e-3 AU/day, V_ERR_EPS=1e-4 AU/day
//!   * craft: dry=fuel=1e-9, thrust=1e-12 (a_max(full)=5e-4 >> local gravity;
//!     a_max(empty)*dt=2.5e-4 << V_CRUISE; Δv_max=v_e*ln2~6.9e-3 > 2*V_CRUISE)
//!   * case 4/5: ~5 AU start, 0.5 AU hop, ~4000-tick budget (clean arrival ~t1000)
//!   * case 6: rendezvous with a planet on a circular a=5 AU orbit; craft starts
//!     co-moving (its small Δv budget cannot acquire the planet's orbital speed
//!     from rest) offset 0.3 AU; the law holds the velocity-match while closing
//!     (measured: arrives ~t621, d~4.8e-5 AU, craft speed ~= body v_circ).
//!
//! Upstream dependencies: Task 7's gravity_accel must honour the a == 0.0 star
//! guard (no NaN from the star's own slot) and Task 8's substep formula must
//! engage on the quantized total accel magnitude; the is_finite() asserts below
//! would otherwise report an upstream regression as a (false) physics blowup.

use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, CraftInit, Dt, EntityRef, EventKind, G_CANONICAL,
    GuidanceParams, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg, Target, Tick, Vec3,
    World,
};

// ---- resolved v1 tuning defaults ----
const DT_DAYS: f64 = 0.25;
const SOFTENING: f64 = 1.0e-3;
const SUBSTEP_CFG: SubstepCfg = SubstepCfg {
    accel_ref: 1.0e-3,
    max_substeps: 64,
};

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
            elements: OrbitalElements {
                a: 0.0,
                e: 0.0,
                i: 0.0,
                raan: 0.0,
                argp: 0.0,
                m0: 0.0,
            },
        }],
        craft,
        guidance: GuidanceParams::default(),
        stations: vec![],
        producers: vec![],
        corporations: vec![],
        contracts: vec![],
        price_cfg: jumpgate_core::config::PriceCfg::default(),
        dispatch_cfg: jumpgate_core::config::DispatchCfg::default(),
        trophic: jumpgate_core::config::TrophicCfg::default(),
        shipyard: jumpgate_core::config::ShipyardCfg::default(),
        media: jumpgate_core::config::MediaCfg::default(),
        refuel: jumpgate_core::config::RefuelCfg::default(),
        goods: jumpgate_core::config::GoodsCfg::default(),
        exchange: jumpgate_core::config::ExchangeCfg::default(),
        arbitrage: jumpgate_core::config::ArbitrageCfg::default(),
    }
}

/// A coasting craft: zero fuel so the autopilot/thrust path is inert and the
/// trajectory is pure gravity.
fn coasting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
    CraftInit {
        spec: BaseSpec {
            base_dry_mass: 1.0e-12, // ~negligible vs M_sun; craft exerts no gravity anyway
            base_max_thrust: 0.0,
            base_exhaust_velocity: 1.0e-2,
            base_fuel_capacity: 0.0,
            base_cargo_capacity: 5,
        },
        pos,
        vel,
        fuel_mass: 0.0,
        role: jumpgate_core::stores::CraftRole::Idle,
        scripted: true,
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

    let (mut world, _cfg_hash) =
        World::reset(star_config(1, m, total_ticks + 8, craft)).expect("resolvable config");
    let cid = world.craft_ids()[0];

    let mut r_min = f64::INFINITY;
    let mut r_max = 0.0_f64;
    let mut cmds: Vec<Command> = Vec::new();
    for _ in 0..total_ticks {
        world.step(&mut cmds);
        let p = world.craft_pos(cid).expect("craft alive");
        assert!(
            p.x.is_finite() && p.y.is_finite() && p.z.is_finite(),
            "position went non-finite"
        );
        let r = p.length();
        if r < r_min {
            r_min = r;
        }
        if r > r_max {
            r_max = r;
        }
    }
    // bounded: radius never drifts more than 5% off the initial circular radius
    assert!(r_min > 0.95 * r0, "orbit decayed inward: r_min = {r_min}");
    assert!(r_max < 1.05 * r0, "orbit grew outward: r_max = {r_max}");
}

#[test]
fn eccentric_close_approach_does_not_blow_up() {
    let m: f64 = 1.0;
    let a: f64 = 1.0; // semi-major axis (AU)
    let e: f64 = 0.9; // high eccentricity => periapsis r_p = a(1-e) = 0.1 AU
    let r_apo = a * (1.0 + e); // 1.9 AU, start here
    // vis-viva: v^2 = G*M*(2/r - 1/a); at apoapsis velocity is purely tangential
    let v_apo = (G_CANONICAL * m * (2.0 / r_apo - 1.0 / a)).sqrt();
    let craft = vec![coasting_craft(
        Vec3::new(r_apo, 0.0, 0.0),
        Vec3::new(0.0, v_apo, 0.0),
    )];

    let period_days = std::f64::consts::TAU * (a * a * a / (G_CANONICAL * m)).sqrt();
    let total_ticks = (5.0 * period_days / DT_DAYS).ceil() as u64; // 5 orbits incl. 5 periapsis passes

    let (mut world, _h) =
        World::reset(star_config(2, m, total_ticks + 8, craft)).expect("resolvable config");
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
        if r < r_min {
            r_min = r;
        }
        if r > r_max {
            r_max = r;
        }
    }
    // engaged substepping kept periapsis off the singularity ...
    assert!(
        r_min > 0.5 * a * (1.0 - e),
        "periapsis collapsed: r_min = {r_min}"
    );
    // ... and did not get slingshot to escape (bound orbit stays near apoapsis scale)
    assert!(
        r_max < 3.0 * r_apo,
        "trajectory blew outward: r_max = {r_max}"
    );
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

    let (mut world, _h) =
        World::reset(star_config(3, m, total_ticks + 8, craft)).expect("resolvable config");
    let cid = world.craft_ids()[0];

    let energy = |p: Vec3, v: Vec3| -> f64 { 0.5 * v.length_sq() - G_CANONICAL * m / p.length() };
    let e0 = energy(world.craft_pos(cid).unwrap(), world.craft_vel(cid).unwrap());

    let mut cmds: Vec<Command> = Vec::new();
    for _ in 0..total_ticks {
        world.step(&mut cmds);
    }
    let e1 = energy(world.craft_pos(cid).unwrap(), world.craft_vel(cid).unwrap());

    let rel_drift = ((e1 - e0) / e0).abs();
    assert!(
        rel_drift < 1.0e-2,
        "energy drift too large over one orbit: {rel_drift}"
    );
}

/// A craft with real thrust + fuel, in a weak-gravity region so the autopilot's
/// velocity-targeting BRAKING law (autopilot.rs) dominates the local gravity. The
/// braking law brings the craft to rest at the destination instead of tunnelling,
/// so this no longer needs the contrived ultra-weak-thrust slow-creep regime.
///
/// Sizing (verified against the law's three constraints, K_BRAKE=0.5,
/// V_CRUISE=2e-3, V_ERR_EPS=1e-4, dt=0.25):
///   * a_max(full) = 1e-12 / 2e-9 = 5e-4 AU/day^2  >> local gravity (~1.2e-5 at 5 AU)
///   * a_max(empty) = 1e-12 / 1e-9 = 1e-3, so a_max*dt = 2.5e-4 << V_CRUISE (no aliasing)
///   * Δv_max = v_e*ln((dry+fuel)/dry) = 1e-2*ln(2) ~ 6.9e-3 > 2*V_CRUISE = 4e-3 (round trip)
fn thrusting_craft(pos: Vec3, vel: Vec3) -> CraftInit {
    CraftInit {
        spec: BaseSpec {
            base_dry_mass: 1.0e-9,
            base_max_thrust: 1.0e-12,
            base_exhaust_velocity: 1.0e-2,
            base_fuel_capacity: 1.0e-9,
            base_cargo_capacity: 5,
        },
        pos,
        vel,
        fuel_mass: 1.0e-9, // full tank => Δv budget for the accelerate+brake round trip
        role: jumpgate_core::stores::CraftRole::Idle,
        scripted: true,
    }
}

/// Run a transfer to `dest` and return Some(arrival_tick) if an Arrival event for
/// the (single) craft fired within `max_ticks`, else None.
fn run_transfer(
    seed: u64,
    start: Vec3,
    dest: Vec3,
    budget: Option<f64>,
    max_ticks: u64,
) -> Option<u64> {
    let craft = vec![thrusting_craft(start, Vec3::ZERO)];
    let (mut world, _h) =
        World::reset(star_config(seed, 1.0, max_ticks + 8, craft)).expect("resolvable config");
    let cid = world.craft_ids()[0];

    // single ingestion path: command the destination once at tick 0
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(cid)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(dest),
            burn_budget: budget,
        },
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
    // INTENDED regime (the v1 braking law brings the craft to rest at the dest, so
    // the contrived 300-AU/1.48e-16-thrust/25k-tick slow-creep workaround is gone):
    // a ~5 AU start in a weak-gravity region (central accel ~ G*M/5^2 ~ 1.2e-5),
    // a 0.5 AU hop, realistic thrust ~1e-12 (a_max ~5e-4), ~4000-tick budget.
    let start = Vec3::new(5.0, 0.0, 0.0);
    let dest = Vec3::new(5.5, 0.0, 0.0); // 0.5 AU hop
    let arrival = run_transfer(11, start, dest, Some(1.0), 4_000);
    assert!(
        arrival.is_some(),
        "craft never emitted Arrival within budget"
    );
}

#[test]
fn transfer_arrival_tick_is_deterministic() {
    let start = Vec3::new(5.0, 0.0, 0.0);
    let dest = Vec3::new(5.5, 0.0, 0.0);
    let a = run_transfer(11, start, dest, Some(1.0), 4_000);
    let b = run_transfer(11, start, dest, Some(1.0), 4_000);
    assert!(a.is_some(), "first run did not arrive");
    assert_eq!(
        a, b,
        "same config produced different arrival ticks: {a:?} vs {b:?}"
    );
}

/// Velocity-matched RENDEZVOUS at a MOVING body. The autopilot works in the
/// target's reference frame (rel_vel = vel - body_vel), so "arrive at rest in the
/// target frame" == co-move with the body: the craft must null the relative
/// velocity AND close the gap, then settle inside ARRIVAL_RADIUS and stay (rather
/// than flying through). Without the braking law a craft would tunnel the moving
/// 1e-4 AU sphere just as for a fixed Position.
///
/// Setup: a central star (gravity source) at the origin plus a planet on a
/// circular a=5 AU orbit (v_circ = sqrt(G_CANONICAL/5) ~ 7.69e-3 AU/day). The
/// craft starts CO-MOVING with the planet (the natural rendezvous initial
/// condition — otherwise the craft's small Δv budget ~6.9e-3 could never acquire
/// the planet's ~7.69e-3 AU/day orbital speed from rest) but offset 0.3 AU
/// radially inward, so the law must hold the velocity
/// match while closing the gap against the body's continuously-curving velocity.
#[test]
fn transfer_to_moving_body_rendezvous() {
    let max_ticks: u64 = 6_000;
    // Planet on a circular 5 AU orbit; m0=0 => at tick 0 it sits at (5,0,0) moving
    // +y at v_circ. Small mass so its own gravity barely perturbs the craft.
    let planet_a = 5.0_f64;
    let v_circ = (G_CANONICAL * 1.0 / planet_a).sqrt();
    let planet_pos0 = Vec3::new(planet_a, 0.0, 0.0);
    let planet_vel0 = Vec3::new(0.0, v_circ, 0.0);

    // Craft: 0.3 AU radially inward of the planet, co-moving with it at tick 0.
    let craft_pos0 = Vec3::new(planet_a - 0.3, 0.0, 0.0);
    let craft = vec![thrusting_craft(craft_pos0, planet_vel0)];

    let cfg = RunConfig {
        master_seed: 7,
        dt: Dt::new(DT_DAYS),
        softening: SOFTENING,
        substep_cfg: SUBSTEP_CFG,
        ephemeris_window: max_ticks + 8, // cover every stepped tick (body_vel clamps past window)
        bodies: vec![
            // Central star (a==0 => fixed at the focus, the gravity source).
            BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 0.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            },
            // Planet: real a>0 orbit, tiny mass (negligible self-gravity on the craft).
            BodyInit {
                mass: 1.0e-9,
                elements: OrbitalElements {
                    a: planet_a,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            },
        ],
        craft,
        guidance: GuidanceParams::default(),
        stations: vec![],
        producers: vec![],
        corporations: vec![],
        contracts: vec![],
        price_cfg: jumpgate_core::config::PriceCfg::default(),
        dispatch_cfg: jumpgate_core::config::DispatchCfg::default(),
        trophic: jumpgate_core::config::TrophicCfg::default(),
        shipyard: jumpgate_core::config::ShipyardCfg::default(),
        media: jumpgate_core::config::MediaCfg::default(),
        refuel: jumpgate_core::config::RefuelCfg::default(),
        goods: jumpgate_core::config::GoodsCfg::default(),
        exchange: jumpgate_core::config::ExchangeCfg::default(),
        arbitrage: jumpgate_core::config::ArbitrageCfg::default(),
    };

    let (mut world, _h) = World::reset(cfg).expect("resolvable config");
    let cid = world.craft_ids()[0];
    let planet = world.body_ids()[1];
    // Sanity: the planet really is the moving body we think it is at tick 0.
    assert!(
        world
            .body_pos(planet, Tick(0))
            .unwrap()
            .sub(planet_pos0)
            .length()
            < 1e-9,
        "planet tick-0 position mismatch"
    );

    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(cid)),
        kind: CommandKind::Destination {
            dest: NavDest::Entity(EntityRef::Body(planet)),
            burn_budget: Some(1.0),
        },
    }];
    world.step(&mut cmds);

    let mut last_seen = Tick(0);
    let arrival = loop {
        let mut found = None;
        for ev in world.recent_events(last_seen) {
            if let EventKind::Arrival { craft: ac, .. } = ev.kind
                && ac == cid
            {
                found = Some(ev.tick.0);
            }
        }
        if found.is_some() {
            break found;
        }
        last_seen = world.tick();
        if world.tick().0 >= max_ticks {
            break None;
        }
        let mut none: Vec<Command> = Vec::new();
        world.step(&mut none);
    };

    assert!(
        arrival.is_some(),
        "craft never rendezvoused with the moving body within {max_ticks} ticks"
    );

    // Arrival fired is necessary but NOT sufficient for a MOVING target: the
    // point-in-(moving-)sphere edge test would also fire for a high-speed flyby
    // that happens to have a tick land inside. Pin the success-bar property
    // directly — at arrival the craft is inside the sphere AND velocity-matched
    // to the body (relative speed far below the body's orbital speed, NOT ~v_circ
    // as a flyby would be). Body velocity via a central finite difference of the
    // ephemeris position (World exposes no body-velocity accessor).
    let t = world.tick();
    let p = world.craft_pos(cid).unwrap();
    let v = world.craft_vel(cid).unwrap();
    let bp = world.body_pos(planet, t).unwrap();
    let bp_next = world.body_pos(planet, Tick(t.0 + 1)).unwrap();
    let bp_prev = world.body_pos(planet, Tick(t.0 - 1)).unwrap();
    let body_vel = bp_next.sub(bp_prev).scale(1.0 / (2.0 * DT_DAYS));
    let d = p.sub(bp).length();
    let rel_speed = v.sub(body_vel).length();
    // measured: d=4.77e-5, rel_speed=1.25e-4 (1.6% of v_circ) at tick 621.
    // d within a few ARRIVAL_RADIUS (1e-4) of the moving body; loosened slightly
    // from exactly 1e-4 to absorb the tick between event-emit and state-read.
    assert!(
        d < 5.0e-4,
        "craft not near the body at rendezvous: d={d:.4e}"
    );
    // Velocity-matched: relative speed is a small fraction of the body's orbital
    // speed. A flyby would show rel_speed ~ v_circ; a rendezvous, ~0.
    assert!(
        rel_speed < 0.25 * v_circ,
        "not velocity-matched (flyby, not rendezvous): rel_speed={rel_speed:.4e} vs v_circ={v_circ:.4e}"
    );
}

/// Swept-arrival moving-body wiring (Task 6 / §5): exercise `detect_boundary_events`'
/// resolution of the target at BOTH `Tick(T-1)` and `Tick(T)` plus the rel_speed
/// gate, through the REAL `step` path, for a MOVING body — and prove the rel_speed
/// gate cleanly separates a velocity-matched rendezvous (fires) from a fast head-on
/// flyby of identical geometry (does NOT fire).
///
/// Construction (the honesty guard against a vacuous "no Arrival because it geometrically
/// missed"): a coasting craft (zero thrust => pure ballistic, the autopilot never
/// brakes it) is placed ON the planet's tick-0 position with a relative velocity
/// aimed straight back through the planet center. Under near-identical local gravity
/// at 5 AU the RELATIVE path is ~straight and provably transits the planet center
/// (closest approach ~0 << ARRIVAL_RADIUS). The ONLY variable between the two halves
/// is `|rel_vel|`, so a difference in whether Arrival fires is attributable to the
/// rel_speed gate, not to geometry.
fn coasting_flyby_arrival_fires(rel_speed_mag: f64) -> bool {
    let max_ticks: u64 = 200;
    let planet_a = 5.0_f64;
    let v_circ = (G_CANONICAL * 1.0 / planet_a).sqrt();
    let planet_pos0 = Vec3::new(planet_a, 0.0, 0.0);
    let planet_vel0 = Vec3::new(0.0, v_circ, 0.0);

    // Craft co-located with the planet at tick 0; relative velocity is +y (along the
    // planet's instantaneous motion) of magnitude rel_speed_mag, so the craft's
    // RELATIVE displacement sweeps a chord straight through the moving planet center.
    // Co-located start => the very next tick already straddles the center: the chord
    // [prev_pos=center, pos=center+rel] has closest approach 0 (the dd>DD_EPS branch
    // clamps t to 0 and reads `a` = offset-at-start = 0 in the target frame).
    let craft_vel0 = planet_vel0.add(Vec3::new(0.0, rel_speed_mag, 0.0));
    let craft = vec![coasting_craft(planet_pos0, craft_vel0)];

    let cfg = RunConfig {
        master_seed: 7,
        dt: Dt::new(DT_DAYS),
        softening: SOFTENING,
        substep_cfg: SUBSTEP_CFG,
        ephemeris_window: max_ticks + 8,
        bodies: vec![
            BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 0.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            },
            BodyInit {
                mass: 1.0e-9,
                elements: OrbitalElements {
                    a: planet_a,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            },
        ],
        craft,
        guidance: GuidanceParams::default(),
        stations: vec![],
        producers: vec![],
        corporations: vec![],
        contracts: vec![],
        price_cfg: jumpgate_core::config::PriceCfg::default(),
        dispatch_cfg: jumpgate_core::config::DispatchCfg::default(),
        trophic: jumpgate_core::config::TrophicCfg::default(),
        shipyard: jumpgate_core::config::ShipyardCfg::default(),
        media: jumpgate_core::config::MediaCfg::default(),
        refuel: jumpgate_core::config::RefuelCfg::default(),
        goods: jumpgate_core::config::GoodsCfg::default(),
        exchange: jumpgate_core::config::ExchangeCfg::default(),
        arbitrage: jumpgate_core::config::ArbitrageCfg::default(),
    };

    let (mut world, _h) = World::reset(cfg).expect("resolvable config");
    let cid = world.craft_ids()[0];
    let planet = world.body_ids()[1];

    // Seek the MOVING planet so detection resolves c_prev/c_now/dest_vel via ephemeris.
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(cid)),
        kind: CommandKind::Destination {
            dest: NavDest::Entity(EntityRef::Body(planet)),
            burn_budget: Some(1.0),
        },
    }];
    world.step(&mut cmds);

    let mut last_seen = Tick(0);
    loop {
        for ev in world.recent_events(last_seen) {
            if let EventKind::Arrival { craft: ac, .. } = ev.kind
                && ac == cid
            {
                return true;
            }
        }
        last_seen = world.tick();
        if world.tick().0 >= max_ticks {
            return false;
        }
        let mut none: Vec<Command> = Vec::new();
        world.step(&mut none);
    }
}

#[test]
fn moving_body_swept_gate_separates_rendezvous_from_fast_flyby() {
    // Velocity-matched (rel_speed well under ARRIVAL_SPEED=2e-3): a tick lands inside
    // the moving sphere with a low relative speed -> Arrival FIRES (moving-body wiring
    // proven: c_prev != c_now, dest_vel != 0, resolved through step).
    assert!(
        coasting_flyby_arrival_fires(1.0e-4),
        "velocity-matched rendezvous at a moving body did not fire Arrival"
    );

    // Identical head-on geometry but rel_speed = the body's full orbital speed
    // (v_circ ~ 7.7e-3 AU/day >> ARRIVAL_SPEED=2e-3): a fast flyby that still transits
    // the center -> the rel_speed gate SUPPRESSES Arrival. This is the "cleanly
    // separates" measurement (§10 / Step 9): only |rel_vel| changed.
    let v_circ = (G_CANONICAL * 1.0 / 5.0_f64).sqrt();
    assert!(
        !coasting_flyby_arrival_fires(v_circ),
        "fast flyby of a moving body spuriously fired Arrival (rel_speed gate failed)"
    );
}
