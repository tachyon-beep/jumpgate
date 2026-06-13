//! `JumpgateEnv`: the Gymnasium PyO3 facade. Holds one `World` per env, writes
//! frame-relative obs / reward / terminated / truncated into caller buffers.
//! Pure `decode_action` / `compute_reward` are unit-tested without the GIL.

use crate::obs::{write_obs_frame_relative, write_obs_thrust_mode, OBS_DIM, THRUST_OBS_DIM};
use crate::obs::{write_obs_pirate_contacts, write_obs_trader, TRADER_OBS_DIM, TRADER_PIRATES_OBS_DIM};
use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, ContractId, CraftId, CraftInit, Dt, EntityRef,
    Event, EventKind,
    FullObserver, G_CANONICAL, GuidanceParams, NavDest, OrbitalElements, RngStream, RngStreams,
    RunConfig,
    StateView, SubstepCfg, Target, Vec3, World,
};
use numpy::{PyReadonlyArray1, PyReadwriteArray1};
use pyo3::prelude::*;

pub const ACTION_DIM: usize = 4;

/// Thrust-mode action width: `[tx, ty, tz]`, each clamped to [-1, 1].
pub const THRUST_ACTION_DIM: usize = 3;

/// Decode one craft's flat action slice into the navigator macro-command.
/// `slice` is exactly `ACTION_DIM` long: `[dx, dy, dz, burn_budget]`.
/// Destination is EGO-RELATIVE (§7.2): `dest_abs = ego_pos + (dx,dy,dz)`.
/// `burn_budget < 0.0` decodes to `None`.
fn decode_action(slice: &[f32], ego_pos: Vec3, ego: CraftId) -> Command {
    let offset = Vec3::new(slice[0] as f64, slice[1] as f64, slice[2] as f64);
    let dest_abs = ego_pos.add(offset);
    let raw_budget = slice[3] as f64;
    let burn_budget = if raw_budget < 0.0 { None } else { Some(raw_budget) };
    Command {
        target: Target::Entity(EntityRef::Craft(ego)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(dest_abs),
            burn_budget,
        },
    }
}

/// v1 fuel-constrained transfer reward (§7.4), computed in f64.
/// Penalize fuel spent and elapsed time; bonus on task success (Arrival).
fn compute_reward(prev_fuel: f64, cur_fuel: f64, arrived: bool, dt: f64) -> f64 {
    let fuel_spent = prev_fuel - cur_fuel;
    let arrival_bonus = if arrived { 1.0 } else { 0.0 };
    -fuel_spent - 0.001 * dt + arrival_bonus
}

/// Tactical-flight curriculum + reward config (all settable from Python).
#[derive(Clone, Copy, Debug)]
pub struct FlightCfg {
    pub target_dist_min: f64,  // AU
    pub target_dist_max: f64,  // AU
    pub star_mass: f64,        // M_sun; 0.0 = gravity off (stage 0)
    pub exhaust_velocity: f64, // curriculum Δv knob (reset-guard-safe: thrust/mass unchanged)
    pub fuel_capacity: f64,    // scales with sprint length
    pub time_limit: u64,       // ticks
    pub arrival_radius: f64,   // AU
    pub arrival_speed: f64,    // AU/day (rendezvous gate)
    pub gamma: f64,            // MUST equal PPO gamma (potential-shaping invariant)
    pub fuel_weight: f64,      // reward cost per unit fuel
    pub time_penalty: f64,     // per-tick cost
    pub arrival_bonus: f64,
    /// Fixed potential-shaping scale (AU). Deliberately DECOUPLED from
    /// target_dist_max: tying Φ to the per-stage obs scale re-based the reward
    /// at every curriculum promotion (a value-function shock measured as the
    /// sprint-stage decay). One constant across all stages.
    pub phi_scale: f64,
}

impl Default for FlightCfg {
    fn default() -> Self {
        FlightCfg {
            target_dist_min: 0.001,
            target_dist_max: 0.005, // stage-0 short hops
            star_mass: 0.0,
            exhaust_velocity: 0.1,
            fuel_capacity: 1.0e-12,
            time_limit: 400,
            arrival_radius: 1.0e-4,
            arrival_speed: 5.0e-4,
            gamma: 0.99,
            fuel_weight: 1.0e9,
            time_penalty: 0.001,
            arrival_bonus: 10.0,
            phi_scale: 0.05,
        }
    }
}

/// Potential-based shaping (Φ = −d/dist_scale, normalized) + arrival bonus −
/// fuel − time. Potential-based SPECIFICALLY (Ng et al. form γΦ(s')−Φ(s)) so
/// shaping cannot be farmed by dithering.
pub fn flight_reward(
    cfg: &FlightCfg,
    prev_dist: f64,
    cur_dist: f64,
    fuel_spent: f64,
    arrived: bool,
    _dt: f64,
) -> f64 {
    let scale = cfg.phi_scale.max(1e-12);
    let phi_prev = -(prev_dist / scale);
    let phi_cur = -(cur_dist / scale);
    let shaping = cfg.gamma * phi_cur - phi_prev;
    let bonus = if arrived { cfg.arrival_bonus } else { 0.0 };
    shaping + bonus - cfg.fuel_weight * fuel_spent - cfg.time_penalty
}

/// Rendezvous gate: inside the arrival sphere AND slow relative to the target
/// (flybys do not terminate).
pub fn is_arrival(cfg: &FlightCfg, dist: f64, rel_speed: f64) -> bool {
    dist <= cfg.arrival_radius && rel_speed <= cfg.arrival_speed
}

/// Map a raw u64 to a uniform f64 in [0, 1): top 53 bits → mantissa. The
/// standard bit-exact construction, so the draw is reproducible under the
/// pinned rand_core without pulling the `rand` front-end into this crate.
#[inline]
fn u64_to_unit_f64(x: u64) -> f64 {
    ((x >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}

/// Seeded target draw: uniform direction (Marsaglia rejection via the core
/// ChaCha8 Scenario stream) at a uniform distance in [min, max].
/// `env_idx` decorrelates vectorized envs.
pub fn draw_target(cfg: &FlightCfg, seed: u64, env_idx: u64) -> Vec3 {
    // rand_core 0.10: the infallible `next_u64` lives on `rand_core::Rng`
    // (NOT `RngCore`, which only exposes the fallible form) — see core rng.rs.
    use rand_core::Rng as _;
    let mut streams = RngStreams::from_master(seed.wrapping_add(env_idx));
    let rng = streams.stream(RngStream::Scenario);
    let dist = cfg.target_dist_min
        + u64_to_unit_f64(rng.next_u64()) * (cfg.target_dist_max - cfg.target_dist_min);
    loop {
        let v = Vec3::new(
            2.0 * u64_to_unit_f64(rng.next_u64()) - 1.0,
            2.0 * u64_to_unit_f64(rng.next_u64()) - 1.0,
            2.0 * u64_to_unit_f64(rng.next_u64()) - 1.0,
        );
        let l = v.length();
        if l > 1e-9 && l <= 1.0 {
            return v.scale(dist / l);
        }
    }
}

/// Build the v1 scenario config: one central star + `num_craft` identical craft.
/// `master_seed` is overwritten per-env in `reset`.
fn config_template(num_craft: usize) -> RunConfig {
    let star = BodyInit {
        mass: 1.0, // 1 M_sun in canonical units
        elements: OrbitalElements {
            a: 0.0,
            e: 0.0,
            i: 0.0,
            raan: 0.0,
            argp: 0.0,
            m0: 0.0,
        },
    };
    // Plan drift: the §6 reset guard (anti-tunnel, landed after this plan was
    // written) rejects configs where empty-tank braking can overshoot the
    // arrival sphere in one tick: it requires
    //   a_max_empty * dt^2 < R/(2*k_brake),
    // i.e. base_max_thrust/base_dry_mass < 1e-4/(2*0.5) = 1e-4 at dt=1.0.
    // The plan's 1e-7/1e-9 = 100 fails hard. Adopt core's own resolvable test
    // fixture spec (a_max_empty = 1e-17/1e-12 = 1e-5 < 1e-4, a 10x margin).
    // Only the mass/thrust ratio matters for the guard; pos/vel are unchanged
    // (vel 0.0172 = sqrt(G_CANONICAL), well under the obs MAX_REL_AU guard).
    let spec = BaseSpec {
        base_dry_mass: 1.0e-12,
        base_max_thrust: 1.0e-17,
        base_exhaust_velocity: 1.0e-3,
        base_fuel_capacity: 1.0e-12,
        base_cargo_capacity: 5,
    };
    let craft = (0..num_craft)
        .map(|_| CraftInit {
            spec: spec.clone(),
            pos: Vec3::new(1.0, 0.0, 0.0),    // 1 AU from the star
            vel: Vec3::new(0.0, 0.0172, 0.0), // ~circular at 1 AU: sqrt(G_CANONICAL) AU/day
            fuel_mass: 1.0e-12,
            role: jumpgate_core::stores::CraftRole::Idle,
            scripted: true,
        })
        .collect();
    RunConfig {
        master_seed: 0,
        dt: Dt::new(1.0),
        softening: 1.0e-4,
        // accel_ref calibrated to Task-8 reference accel: gravity at 1 AU
        // from 1 M_sun is ~2.96e-4 AU/day^2, so 3.0e-4 keeps substeps near 1 at
        // cruise and escalates only on close approach (not the saturating 1e-6).
        substep_cfg: SubstepCfg {
            accel_ref: 3.0e-4,
            max_substeps: 64,
        },
        ephemeris_window: 100_000,
        bodies: vec![star],
        craft,
        // Plan drift (guidance landed after this plan was written): RunConfig
        // grew a `guidance: GuidanceParams` field. Use the canonical default
        // policy (cruise_burn_fraction / k_brake / v_err_eps).
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
    }
}

// === Trader mode (strategic Rung 1): constants + scenario template ===

/// Board slots exposed to the agent (M). Action space is `Discrete(M+1)` in
/// Python (0 = wait, j = accept slot j-1); the native buffer stays f32.
pub const TRADER_BOARD_SLOTS: usize = 4;

/// Trader-mode action width: a single f32 index decoded with `round()`.
pub const TRADER_ACTION_DIM: usize = 1;

/// Ticks advanced by the wait action (spec §5.1).
pub const TRADER_WAIT_TICKS: u64 = 8;

/// Trader-mode episode config (the macro-step horizon, in world ticks).
/// Reuses `configure`'s `time_limit` kwarg; no new configure args in v1.
#[derive(Clone, Copy, Debug)]
pub struct TraderCfg {
    pub horizon: u64,
}

impl Default for TraderCfg {
    fn default() -> Self {
        TraderCfg { horizon: 2000 }
    }
}

/// Pirates-variant horizon (spec §11): ≈ 6-10 decisions, long enough that a
/// robbery's Δcredits lands inside the episode that chose the route.
/// Baselines are re-rolled at this horizon by the report script.
pub const TRADER_PIRATES_HORIZON: u64 = 5000;

/// SplitMix64-style finalizer over `(seed, k)`: deterministic, dependency-free
/// pre-reset config derivation (NOT world state — RngStreams owns that; this
/// only seeds the scenario template's initial mean anomalies).
fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Build the trader-rung scenario (spec §6): one star (1 M_sun), four marker
/// bodies on circular orbits (`a = 0.35/0.55/0.8/1.1` AU) whose initial mean
/// anomalies derive from `seed` (anti-memorization: geometry differs per
/// episode), one station per body, Ore miners at stations 0/2 + Ore sinks at
/// 1/3, one funded corp, four seeded rate-mispriced routes, scripted ASSIGN
/// OFF (`stagger_period: 0` — the agent owns acceptance; REPOST keeps the
/// board flowing).
///
/// Craft spec is LIFTED from the proven world.rs forage-loop fixture
/// (`one_body_one_thrusting_craft`): dry 1e-9 / thrust 1e-12 / capacity 1e-9
/// at the fixtures' dt=0.25 passes the §6 anti-tunnel reset guard
/// (a_max_empty·dt² = 6.25e-5 < R/(2·k_brake) = 1e-4; the plan's dt=1.0
/// would reject it, 1e-3 ≥ 1e-4).
///
/// STAR MASS 1e-3, not the spec's 1 M_sun — a measured calibration, same
/// class as the fixtures' near-massless centrals: seeded phases regularly
/// put a route's endpoints near-opposite, so the transfer chord grazes the
/// star, where 1-M_sun gravity (g ~ 0.12 AU/day² at 0.05 AU) exceeds ANY
/// guard-compliant thrust (the guard caps a_max_empty·dt² < 1e-4) — measured
/// as a star-dive trap (thrust 1e-12: slingshot to 17 AU, 5610 ticks for one
/// delivery; thrust 8e-12 @ dt=0.1: captured circling the star indefinitely).
/// At M=1e-3 the well is controllable EVERYWHERE (g(0.05 AU) ~ 1.2e-4 ≪
/// a_max_full = 5e-4) while orbits still move: the inner body sweeps ~75°
/// per 2000-tick episode (T ≈ 2390 days at a=0.35), so within-episode
/// geometry shifts AND cross-episode phases vary (anti-memorization both
/// ways). Body motion per tick (~2.3e-4 AU ≈ 2.3× ARRIVAL_RADIUS at a=0.35)
/// is what surfaced the try_load frame fix.
/// `exhaust_velocity = 2.0` (vs the forage fixture's 1e-2): full-burn tank
/// life = capacity·v_e/thrust = 2000 days ≥ 4× a 500-day episode; tank Δv
/// 2·ln2 ≈ 1.39 ≫ the ~0.03/trip a transfer spends (≥ 3× greedy-burn margin,
/// spec §4.3). Raising v_e (not capacity) keeps wet mass — and therefore
/// trip times (~150–400 ticks/leg at a_full 5e-4) — unchanged.
/// `num_pirates` (pirates rung spec §11): 0 (the trader rung, byte-identical
/// config — no pirate rows, inert `TrophicCfg::default()`, zero Piracy draws)
/// or N pirate-role craft spawned co-orbiting the OUTERMOST body (the
/// hideout) with a LIVE trophic surface; their initial lurk stations are
/// seed-drawn from the Piracy stream at `World::reset` (§5 — the
/// gym-memorization guard).
pub fn trader_config_template(seed: u64, num_craft: usize, num_pirates: usize) -> RunConfig {
    use jumpgate_core::config::{
        ContractInit, CorporationInit, DispatchCfg, PriceCfg, ProducerInit, StationInit,
    };
    use jumpgate_core::economy::{Good, Recipe};

    /// Circular-orbit radii (AU) for the four station-host bodies.
    const ORBIT_AU: [f64; 4] = [0.35, 0.55, 0.8, 1.1];
    /// Negligible-mass marker bodies (fixture convention): only their
    /// positions matter; no local gravity wells around stations.
    const BODY_MASS: f64 = 1.0e-12;

    /// Central mass (M_sun): 1e-3, the controllable-everywhere calibration
    /// (doc above) — orbits move on episode timescales, gravity never beats
    /// guard-compliant thrust.
    const STAR_MASS: f64 = 1.0e-3;

    let star = BodyInit {
        mass: STAR_MASS,
        elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
    };
    let mut bodies = vec![star];
    for (k, &a) in ORBIT_AU.iter().enumerate() {
        // Seed-derived initial phase, uniform on [0, TAU): body k+1 uses
        // mix(seed, k+1) so phases decorrelate across bodies AND seeds.
        let m0 = u64_to_unit_f64(mix(seed, (k + 1) as u64)) * std::f64::consts::TAU;
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    let spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 2.0,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };

    // Spawn co-located with body 1 (station 0's host) at its seeded phase,
    // co-orbiting: for e=0/i=0 conics the ephemeris state is exactly
    // pos = a(cos m0, sin m0, 0), vel = v_circ(-sin m0, cos m0, 0) with
    // v_circ = sqrt(mu/a), mu = G·(M_star + m_body). Co-location means the
    // first 0→x accept loads same-tick (no deadhead) — the proven
    // two_body_contract_fixture opening.
    let m0_home = bodies[1].elements.m0;
    let a_home = ORBIT_AU[0];
    let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
    let v_circ = (mu / a_home).sqrt();
    let pos = Vec3::new(a_home * m0_home.cos(), a_home * m0_home.sin(), 0.0);
    let vel = Vec3::new(-v_circ * m0_home.sin(), v_circ * m0_home.cos(), 0.0);
    let mut craft: Vec<CraftInit> = (0..num_craft)
        .map(|_| CraftInit {
            spec: spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: jumpgate_core::stores::CraftRole::Idle,
            scripted: true,
        })
        .collect();
    // Pirates APPENDED after the agent craft (craft_ids()[0] stays the
    // trader). Spawn co-orbiting the outermost body (a = 1.1 AU, the gym
    // hideout); the reset Piracy draw sends each toward a seed-drawn lurk
    // station (scenario_trophic's spawn math).
    if num_pirates > 0 {
        let m0_h = bodies[4].elements.m0;
        let a_h = ORBIT_AU[3];
        let v_h = (mu / a_h).sqrt();
        let ppos = Vec3::new(a_h * m0_h.cos(), a_h * m0_h.sin(), 0.0);
        let pvel = Vec3::new(-v_h * m0_h.sin(), v_h * m0_h.cos(), 0.0);
        for _ in 0..num_pirates {
            craft.push(CraftInit {
                spec: spec.clone(),
                pos: ppos,
                vel: pvel,
                fuel_mass: 1.0e-9,
                role: jumpgate_core::stores::CraftRole::Pirate,
                scripted: true,
            });
        }
    }

    // Station k rides body k+1 (body 0 is the star). Miners' homes (0, 2)
    // open with deep Ore stock; sink stations (1, 3) open empty.
    let stations = vec![
        StationInit {
            body_index: 1,
            initial_stock: [40, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        },
        StationInit {
            body_index: 2,
            initial_stock: [0, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        },
        StationInit {
            body_index: 3,
            initial_stock: [40, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        },
        StationInit {
            body_index: 4,
            initial_stock: [0, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        },
    ];
    let producers = vec![
        // Ore miners at stations 0 and 2 (keep pickup stock flowing).
        ProducerInit {
            station_index: 0,
            recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 },
        },
        ProducerInit {
            station_index: 2,
            recipe: Recipe { input: None, output: Some((Good::ORE, 5)), interval: 40 },
        },
        // Ore demand-sinks at stations 1 and 3 (drain deliveries so REPOST
        // re-fires the routes).
        ProducerInit {
            station_index: 1,
            recipe: Recipe { input: Some((Good::ORE, 5)), output: None, interval: 60 },
        },
        ProducerInit {
            station_index: 3,
            recipe: Recipe { input: Some((Good::ORE, 5)), output: None, interval: 60 },
        },
    ];
    // Treasury large enough that escrow never reverts an accept (max 4
    // concurrent escrows × 3 cr ≪ 1000 cr).
    let corporations =
        vec![CorporationInit { treasury_micros: 1_000_000_000, home_station_index: 0 }];

    // LIVE trophic surface for the pirates variant (num_pirates == 0 keeps
    // the inert default — the spec-§8 lever — so existing trader scenarios
    // stay bit-identical). Values are the scenario_trophic §4 calibration,
    // EXCEPT the grubstake: 150_000 / upkeep 25 = a 6000-tick active runway
    // ≥ the 5000-tick horizon, so the predation field is live across the
    // whole episode (an early starvation lie-low would blank the second half
    // of every episode); lie-low visibility still reaches the obs via heat
    // (3 quick robs cross the 250 notoriety threshold).
    let trophic = if num_pirates > 0 {
        jumpgate_core::config::TrophicCfg {
            engage_radius_au: 5.0e-4, // 5× ARRIVAL_RADIUS (spec §2)
            upkeep_per_tick: 25,
            food_per_unit_micros: 10_000,
            grubstake_micros: 150_000,
            starve_lie_low_ticks: 4_000,
            hideout_body_index: 4, // outermost trader body (1.1 AU)
            ..jumpgate_core::config::TrophicCfg::default()
        }
    } else {
        jumpgate_core::config::TrophicCfg::default()
    };
    // Four rate-MISPRICED routes (reward NOT ∝ trip time — spec §6): which
    // route is best shifts with orbital phase, so rate-maximization is a
    // judgment, not a lookup.
    let contracts = vec![
        ContractInit { corp_index: 0, resource: Good::ORE, qty: 5, from_station_index: 0, to_station_index: 1, reward_micros: 1_000_000 },
        ContractInit { corp_index: 0, resource: Good::ORE, qty: 5, from_station_index: 2, to_station_index: 3, reward_micros: 1_200_000 },
        ContractInit { corp_index: 0, resource: Good::ORE, qty: 5, from_station_index: 0, to_station_index: 3, reward_micros: 1_600_000 },
        ContractInit { corp_index: 0, resource: Good::ORE, qty: 5, from_station_index: 2, to_station_index: 1, reward_micros: 3_000_000 },
    ];

    RunConfig {
        master_seed: seed,
        // dt 0.25: the proven economy-fixture timestep; see the doc above for
        // why the plan's dt=1.0 is rejected by the anti-tunnel guard with
        // this craft spec.
        dt: Dt::new(0.25),
        softening: 1.0e-4,
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        ephemeris_window: 100_000,
        bodies,
        craft,
        guidance: GuidanceParams::default(),
        stations,
        producers,
        corporations,
        contracts,
        price_cfg: PriceCfg::default(),
        dispatch_cfg: DispatchCfg {
            demand_low: 10,
            demand_high: 20,
            stagger_period: 0, // scripted ASSIGN OFF: the agent owns acceptance
            contract_reward_micros: 0,
            contract_qty: 0,
        },
        trophic,
        shipyard: jumpgate_core::config::ShipyardCfg::default(),
        media: jumpgate_core::config::MediaCfg::default(),
        refuel: jumpgate_core::config::RefuelCfg::default(),
    }
}

/// Vectorized Gymnasium env: `num_envs` independent `World`s, each with
/// `num_craft` craft. Writes frame-relative obs / per-craft reward / terminated /
/// truncated into caller-provided numpy buffers (one memcpy per step, §7.3).
#[pyclass]
pub struct JumpgateEnv {
    worlds: Vec<World>,
    prev_fuel: Vec<f64>, // (num_envs * num_craft) snapshot for reward
    num_envs: usize,
    num_craft: usize,
    obs_dim: usize,
    action_dim: usize,
    time_limit: u64, // truncation horizon in ticks (waypoint mode)
    template: RunConfig,
    // --- thrust control-mode state (tactical Rung 1) ---
    control_mode: u8,       // 0 = waypoint, 1 = thrust
    flight: FlightCfg,      // curriculum + reward knobs
    target_abs: Vec<Vec3>,  // (num_envs * num_craft) absolute target positions
    prev_dist: Vec<f64>,    // (num_envs * num_craft) for potential shaping
    ticks_in_episode: Vec<u64>, // per env
    master_seed: u64,       // seed of the last reset (auto-reset derivation base)
    episode_counter: u64,   // total auto-resets since the last reset()
    // --- trader control-mode state (strategic Rung 1) ---
    trader: TraderCfg, // macro-step horizon
    /// Pirates variant (pirates rung spec §11): pirate-role craft per trader
    /// world. 0 = the untouched trader rung (obs 20, horizon 2000); > 0
    /// appends K=2 contact blocks (obs 34) and defaults the horizon to 5000.
    num_pirates: usize,
    /// Slot → contract-id mapping captured at obs-write time
    /// (`num_envs × TRADER_BOARD_SLOTS`; `None` = empty slot). The action
    /// decode targets the SAME board snapshot the agent observed.
    board_ids: Vec<Option<ContractId>>,
    /// Per-env credit snapshot at the last decision point (reward = Δ).
    prev_credits: Vec<i64>,
}

/// Write the 10-dim thrust-mode obs for one craft (static target v1:
/// target rel-vel = 0 − craft vel). Free fn so the borrow of `world` stays
/// local while the caller holds `&mut self` buffers.
fn write_thrust_obs_for(world: &World, flight: &FlightCfg, target: Vec3, id: CraftId, out: &mut [f32]) {
    let pos = world.craft_pos(id).unwrap_or(Vec3::ZERO);
    let vel = world.craft_vel(id).unwrap_or(Vec3::ZERO);
    let fuel = world.craft_fuel(id).unwrap_or(0.0);
    let cap = world.craft_fuel_capacity(id).unwrap_or(1.0);
    let fuel_frac = if cap > 0.0 { (fuel / cap) as f32 } else { 0.0 };
    write_obs_thrust_mode(
        vel,
        fuel_frac,
        target.sub(pos),
        Vec3::ZERO.sub(vel),
        flight.target_dist_max,
        out,
    );
}

#[pymethods]
impl JumpgateEnv {
    #[new]
    #[pyo3(signature = (num_envs, num_craft, num_pirates = 0))]
    fn new(num_envs: usize, num_craft: usize, num_pirates: usize) -> Self {
        let template = config_template(num_craft);
        let worlds = (0..num_envs)
            .map(|i| {
                let mut cfg = template.clone();
                cfg.master_seed = i as u64; // overwritten in reset
                let (w, _hash) = World::reset(cfg).expect("resolvable cfg");
                w
            })
            .collect();
        JumpgateEnv {
            worlds,
            prev_fuel: vec![0.0; num_envs * num_craft],
            num_envs,
            num_craft,
            obs_dim: OBS_DIM,
            action_dim: ACTION_DIM,
            time_limit: 1000,
            template,
            control_mode: 0,
            flight: FlightCfg::default(),
            target_abs: vec![Vec3::ZERO; num_envs * num_craft],
            prev_dist: vec![0.0; num_envs * num_craft],
            ticks_in_episode: vec![0; num_envs],
            master_seed: 0,
            episode_counter: 0,
            trader: TraderCfg {
                horizon: if num_pirates > 0 {
                    TRADER_PIRATES_HORIZON
                } else {
                    TraderCfg::default().horizon
                },
            },
            num_pirates,
            board_ids: vec![None; num_envs * TRADER_BOARD_SLOTS],
            prev_credits: vec![0; num_envs],
        }
    }

    #[getter]
    fn obs_dim(&self) -> usize {
        self.obs_dim
    }

    #[getter]
    fn action_dim(&self) -> usize {
        self.action_dim
    }

    #[getter]
    fn episode_counter(&self) -> u64 {
        self.episode_counter
    }

    /// Set the control mode (0 = waypoint, 1 = thrust) and any subset of
    /// `FlightCfg` fields (omitted kwargs keep their current values). Rebuilds
    /// the scenario template (star mass, exhaust velocity, fuel capacity) so
    /// the next `reset` uses the new difficulty. Returns `(obs_dim, action_dim)`
    /// for the requested mode so the Python wrapper can re-derive its spaces.
    #[allow(clippy::too_many_arguments)] // flat scalar kwargs by design (no dict parsing)
    #[pyo3(signature = (
        mode,
        target_dist_min = None, target_dist_max = None, star_mass = None,
        exhaust_velocity = None, fuel_capacity = None, time_limit = None,
        arrival_radius = None, arrival_speed = None, gamma = None,
        fuel_weight = None, time_penalty = None, arrival_bonus = None,
        phi_scale = None,
    ))]
    fn configure(
        &mut self,
        mode: u8,
        target_dist_min: Option<f64>,
        target_dist_max: Option<f64>,
        star_mass: Option<f64>,
        exhaust_velocity: Option<f64>,
        fuel_capacity: Option<f64>,
        time_limit: Option<u64>,
        arrival_radius: Option<f64>,
        arrival_speed: Option<f64>,
        gamma: Option<f64>,
        fuel_weight: Option<f64>,
        time_penalty: Option<f64>,
        arrival_bonus: Option<f64>,
        phi_scale: Option<f64>,
    ) -> PyResult<(usize, usize)> {
        if mode > 2 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mode must be 0 (waypoint), 1 (thrust), or 2 (trader)",
            ));
        }
        let f = &mut self.flight;
        if let Some(v) = target_dist_min {
            f.target_dist_min = v;
        }
        if let Some(v) = target_dist_max {
            f.target_dist_max = v;
        }
        if let Some(v) = star_mass {
            f.star_mass = v;
        }
        if let Some(v) = exhaust_velocity {
            f.exhaust_velocity = v;
        }
        if let Some(v) = fuel_capacity {
            f.fuel_capacity = v;
        }
        if let Some(v) = time_limit {
            f.time_limit = v;
        }
        if let Some(v) = arrival_radius {
            f.arrival_radius = v;
        }
        if let Some(v) = arrival_speed {
            f.arrival_speed = v;
        }
        if let Some(v) = gamma {
            f.gamma = v;
        }
        if let Some(v) = fuel_weight {
            f.fuel_weight = v;
        }
        if let Some(v) = time_penalty {
            f.time_penalty = v;
        }
        if let Some(v) = phi_scale {
            f.phi_scale = v;
        }
        if let Some(v) = arrival_bonus {
            f.arrival_bonus = v;
        }

        self.control_mode = mode;
        if mode == 2 {
            // Trader mode: the scenario template is built FRESH at each reset
            // from the episode seed (`trader_config_template`); `self.template`
            // belongs to modes 0/1 and is deliberately untouched. `time_limit`
            // doubles as the macro-step horizon (no new configure args in v1).
            if let Some(v) = time_limit {
                self.trader.horizon = v;
            }
            // Pirates variant: append K=2 contact blocks (20 -> 34). The
            // action space is UNCHANGED (no purchase actions this rung).
            self.obs_dim = if self.num_pirates > 0 {
                TRADER_PIRATES_OBS_DIM
            } else {
                TRADER_OBS_DIM
            };
            self.action_dim = TRADER_ACTION_DIM;
        } else if mode == 1 {
            // Curriculum knobs ride the template; thrust/dry-mass ratio is
            // untouched so the core reset anti-tunnel guard still passes.
            self.template.bodies[0].mass = self.flight.star_mass;
            for craft in &mut self.template.craft {
                craft.spec.base_exhaust_velocity = self.flight.exhaust_velocity;
                craft.spec.base_fuel_capacity = self.flight.fuel_capacity;
                craft.fuel_mass = self.flight.fuel_capacity;
                // Initial velocity must be CONSISTENT with the configured star
                // mass: circular-orbit speed sqrt(G*M/r) at the spawn radius
                // (zero when gravity is off). The waypoint template's 1-M_sun
                // orbital vel (0.0172 AU/day) would otherwise dwarf a_max
                // (1e-5 AU/day^2) and carry the craft ballistically past every
                // stage-0 target — thrust could never matter.
                let r = craft.pos.length().max(1e-12);
                let v_circ = (G_CANONICAL * self.flight.star_mass / r).sqrt();
                let tangent = Vec3::new(0.0, 1.0, 0.0); // pos is +x; +y is prograde
                craft.vel = tangent.scale(v_circ);
            }
            self.obs_dim = THRUST_OBS_DIM;
            self.action_dim = THRUST_ACTION_DIM;
        } else {
            self.obs_dim = OBS_DIM;
            self.action_dim = ACTION_DIM;
        }
        Ok((self.obs_dim, self.action_dim))
    }

    /// Rebuild every world with `seed` (the gym seed BECOMES the master seed,
    /// distinct per env), then write the initial obs into `out_obs`.
    /// Thrust mode additionally draws a fresh per-env target from the seed.
    fn reset(&mut self, seed: u64, mut out_obs: PyReadwriteArray1<f32>) {
        let out = out_obs.as_slice_mut().expect("out_obs must be contiguous");
        self.master_seed = seed;
        self.episode_counter = 0;
        if self.control_mode == 2 {
            // Trader mode: the world is rebuilt from the SEED-derived template
            // (geometry varies per seed), not from self.template.
            for env in 0..self.num_envs {
                self.reset_trader_episode(env, seed, out);
            }
            return;
        }
        for env in 0..self.num_envs {
            let mut cfg = self.template.clone();
            cfg.master_seed = seed.wrapping_add(env as u64);
            let (world, _hash) = World::reset(cfg).expect("resolvable cfg");
            self.worlds[env] = world;
            self.ticks_in_episode[env] = 0;

            if self.control_mode == 1 {
                self.reset_thrust_episode(env, seed, out);
                continue;
            }

            let view = self.worlds[env].project(&FullObserver);
            let ids = self.worlds[env].craft_ids();
            // `craft` does double duty: the `ids` index AND the flat-buffer
            // offset (`env * num_craft + craft`), so the range form is clearer
            // than enumerate here.
            #[allow(clippy::needless_range_loop)]
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let obs_base = flat * self.obs_dim;
                write_obs_frame_relative(
                    &view,
                    ids[craft],
                    &mut out[obs_base..obs_base + self.obs_dim],
                );
                self.prev_fuel[flat] = self.worlds[env].craft_fuel(ids[craft]).unwrap_or(0.0);
            }
        }
    }

    /// Decode per-craft actions, advance each world one tick, then write
    /// obs / reward / terminated / truncated. `terminated` = Arrival (success);
    /// `truncated` = time-limit; kept DISTINCT (§7.3). No 5-tuple in Rust.
    /// Thrust mode: on terminated||truncated the env AUTO-RESETS (fresh world
    /// + fresh seeded target) so SB3's per-sub-env VecEnv contract holds; the
    /// written obs is then the NEW episode's initial obs while the flags still
    /// report the old episode's end.
    fn step(
        &mut self,
        action: PyReadonlyArray1<f32>,
        mut out_obs: PyReadwriteArray1<f32>,
        mut out_reward: PyReadwriteArray1<f32>,
        mut out_terminated: PyReadwriteArray1<bool>,
        mut out_truncated: PyReadwriteArray1<bool>,
    ) {
        let act = action.as_slice().expect("action must be contiguous");
        let obs = out_obs.as_slice_mut().expect("out_obs contiguous");
        let rew = out_reward.as_slice_mut().expect("out_reward contiguous");
        let term = out_terminated.as_slice_mut().expect("out_terminated contiguous");
        let trunc = out_truncated.as_slice_mut().expect("out_truncated contiguous");

        if self.control_mode == 1 {
            self.step_thrust(act, obs, rew, term, trunc);
            return;
        }
        if self.control_mode == 2 {
            self.step_trader(act, obs, rew, term, trunc);
            return;
        }

        for env in 0..self.num_envs {
            let ids = self.worlds[env].craft_ids();

            // Decode one command per craft from the flat action buffer.
            let mut cmds: Vec<Command> = Vec::with_capacity(self.num_craft);
            // `craft` indexes `ids` AND offsets the flat action buffer.
            #[allow(clippy::needless_range_loop)]
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let a_base = flat * self.action_dim;
                let ego_pos = self.worlds[env].craft_pos(ids[craft]).unwrap_or(Vec3::ZERO);
                cmds.push(decode_action(
                    &act[a_base..a_base + self.action_dim],
                    ego_pos,
                    ids[craft],
                ));
            }

            let before_tick = self.worlds[env].tick();
            self.worlds[env].step(&mut cmds);
            let dt = self.worlds[env].dt().get();
            let now_tick = self.worlds[env].tick();

            // Copy arrival-window events into an OWNED Vec BEFORE the per-craft
            // loop. `recent_events` returns `&[Event]` borrowed from
            // `self.worlds[env]`; collecting to owned (Event is Copy) releases
            // that immutable borrow so the loop below can re-borrow the world
            // for `craft_fuel` / `project` without a borrow-checker conflict.
            let arrivals: Vec<Event> = self.worlds[env].recent_events(before_tick).to_owned();

            let view = self.worlds[env].project(&FullObserver);
            let ids = self.worlds[env].craft_ids();
            // `craft` indexes `ids` AND offsets the flat obs/reward buffers.
            #[allow(clippy::needless_range_loop)]
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let cur_fuel = self.worlds[env].craft_fuel(ids[craft]).unwrap_or(0.0);

                let arrived = arrivals.iter().any(|e| {
                    matches!(e.kind, EventKind::Arrival { craft: c, .. } if c == ids[craft])
                });

                let r = compute_reward(self.prev_fuel[flat], cur_fuel, arrived, dt);
                self.prev_fuel[flat] = cur_fuel;

                let obs_base = flat * self.obs_dim;
                write_obs_frame_relative(
                    &view,
                    ids[craft],
                    &mut obs[obs_base..obs_base + self.obs_dim],
                );
                rew[flat] = r as f32;
                term[flat] = arrived;
                trunc[flat] = now_tick.0 >= self.time_limit;
            }
        }
    }
}

impl JumpgateEnv {
    /// (Re)start a thrust-mode episode for `env`. Assumes `self.worlds[env]`
    /// was just rebuilt from the template. Draws the seeded target, snapshots
    /// the shaping/fuel state, zeroes the episode clock, writes initial obs.
    fn reset_thrust_episode(&mut self, env: usize, seed: u64, out: &mut [f32]) {
        let ids = self.worlds[env].craft_ids();
        self.ticks_in_episode[env] = 0;
        // `craft` indexes `ids` AND offsets the flat obs/state buffers.
        #[allow(clippy::needless_range_loop)]
        for craft in 0..self.num_craft {
            let flat = env * self.num_craft + craft;
            let world = &self.worlds[env];
            let pos = world.craft_pos(ids[craft]).unwrap_or(Vec3::ZERO);
            let target = pos.add(draw_target(&self.flight, seed, flat as u64));
            self.target_abs[flat] = target;
            self.prev_dist[flat] = target.sub(pos).length();
            self.prev_fuel[flat] = world.craft_fuel(ids[craft]).unwrap_or(0.0);
            let obs_base = flat * self.obs_dim;
            write_thrust_obs_for(
                world,
                &self.flight,
                target,
                ids[craft],
                &mut out[obs_base..obs_base + self.obs_dim],
            );
        }
    }

    /// Thrust-mode step body: decode `[tx,ty,tz]` (clamped) into
    /// `CommandKind::Thrust`, advance, reward via potential shaping, terminate
    /// on rendezvous arrival, truncate on the episode clock, auto-reset.
    fn step_thrust(
        &mut self,
        act: &[f32],
        obs: &mut [f32],
        rew: &mut [f32],
        term: &mut [bool],
        trunc: &mut [bool],
    ) {
        for env in 0..self.num_envs {
            let ids = self.worlds[env].craft_ids();

            let mut cmds: Vec<Command> = Vec::with_capacity(self.num_craft);
            // `craft` indexes `ids` AND offsets the flat action buffer.
            #[allow(clippy::needless_range_loop)]
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let a_base = flat * self.action_dim;
                let throttle_vec = Vec3::new(
                    (act[a_base] as f64).clamp(-1.0, 1.0),
                    (act[a_base + 1] as f64).clamp(-1.0, 1.0),
                    (act[a_base + 2] as f64).clamp(-1.0, 1.0),
                );
                cmds.push(Command {
                    target: Target::Entity(EntityRef::Craft(ids[craft])),
                    kind: CommandKind::Thrust { throttle_vec },
                });
            }

            self.worlds[env].step(&mut cmds);
            let dt = self.worlds[env].dt().get();

            self.ticks_in_episode[env] += 1;
            let truncated = self.ticks_in_episode[env] >= self.flight.time_limit;
            let mut episode_over = truncated;

            // `craft` indexes `ids` AND offsets the flat obs/reward buffers.
            #[allow(clippy::needless_range_loop)]
            for craft in 0..self.num_craft {
                let flat = env * self.num_craft + craft;
                let world = &self.worlds[env];
                let pos = world.craft_pos(ids[craft]).unwrap_or(Vec3::ZERO);
                let vel = world.craft_vel(ids[craft]).unwrap_or(Vec3::ZERO);
                let cur_dist = self.target_abs[flat].sub(pos).length();
                let rel_speed = vel.length(); // static target v1
                let arrived = is_arrival(&self.flight, cur_dist, rel_speed);

                let cur_fuel = world.craft_fuel(ids[craft]).unwrap_or(0.0);
                let fuel_spent = (self.prev_fuel[flat] - cur_fuel).max(0.0);
                let r = flight_reward(
                    &self.flight,
                    self.prev_dist[flat],
                    cur_dist,
                    fuel_spent,
                    arrived,
                    dt,
                );

                rew[flat] = r as f32;
                term[flat] = arrived;
                trunc[flat] = truncated;
                self.prev_dist[flat] = cur_dist;
                self.prev_fuel[flat] = cur_fuel;
                episode_over = episode_over || arrived;

                let obs_base = flat * self.obs_dim;
                write_thrust_obs_for(
                    world,
                    &self.flight,
                    self.target_abs[flat],
                    ids[craft],
                    &mut obs[obs_base..obs_base + self.obs_dim],
                );
            }

            if episode_over {
                // Auto-reset this env (SB3 VecEnv contract): fresh world +
                // fresh derived seed; the obs written above is overwritten
                // with the NEW episode's initial obs.
                self.episode_counter += 1;
                let fresh_seed = self.master_seed ^ self.episode_counter;
                let mut cfg = self.template.clone();
                cfg.master_seed = fresh_seed.wrapping_add(env as u64);
                let (world, _hash) = World::reset(cfg).expect("resolvable cfg");
                self.worlds[env] = world;
                self.reset_thrust_episode(env, fresh_seed, obs);
            }
        }
    }

    // === Trader mode (strategic Rung 1): semi-MDP macro-step ===
    //
    // One step() = one strategic DECISION, not one tick. Trader mode drives
    // craft 0 of each env (one agent craft, spec §6); buffers are indexed per
    // ENV (obs `env*TRADER_OBS_DIM`, act/rew/flags `env`).

    /// (Re)start a trader episode for `env`: rebuild the world from the
    /// seed-derived scenario template (geometry varies per seed), zero the
    /// episode clock and credit snapshot, then write the first decision
    /// point's obs (the craft starts idle with a seeded board, so the first
    /// decision is immediate at tick 0).
    fn reset_trader_episode(&mut self, env: usize, seed: u64, out: &mut [f32]) {
        let cfg =
            trader_config_template(seed.wrapping_add(env as u64), self.num_craft, self.num_pirates);
        let (world, _hash) = World::reset(cfg).expect("resolvable trader cfg");
        self.worlds[env] = world;
        self.ticks_in_episode[env] = 0;
        let craft = self.worlds[env].craft_ids()[0];
        self.prev_credits[env] = self.worlds[env].craft_credits(craft).unwrap_or(0);
        self.write_trader_obs(env, out);
    }

    /// Rebuild the board snapshot (first `TRADER_BOARD_SLOTS` rows of
    /// `offered_contracts`, dense row order) into `board_ids`, then write the
    /// 20-dim obs for `env` into `out[env*obs_dim ..]`. Station positions are
    /// sampled at the CURRENT tick (orbits move).
    fn write_trader_obs(&mut self, env: usize, out: &mut [f32]) {
        let world = &self.worlds[env];
        let craft = world.craft_ids()[0];
        let craft_pos = world.craft_pos(craft).unwrap_or(Vec3::ZERO);

        let offered = world.offered_contracts();
        let mut rows: Vec<(f64, f64, f64)> = Vec::with_capacity(TRADER_BOARD_SLOTS);
        for slot in 0..TRADER_BOARD_SLOTS {
            let idx = env * TRADER_BOARD_SLOTS + slot;
            match offered.get(slot) {
                Some(&(cid, reward_micros, from, to)) => {
                    self.board_ids[idx] = Some(cid);
                    let from_pos = world.station_pos(from).unwrap_or(craft_pos);
                    let to_pos = world.station_pos(to).unwrap_or(from_pos);
                    rows.push((
                        reward_micros as f64,
                        from_pos.sub(craft_pos).length(),
                        to_pos.sub(from_pos).length(),
                    ));
                }
                None => self.board_ids[idx] = None,
            }
        }

        let fuel = world.craft_fuel(craft).unwrap_or(0.0);
        let cap = world.craft_fuel_capacity(craft).unwrap_or(1.0);
        let fuel_frac = if cap > 0.0 { (fuel / cap) as f32 } else { 0.0 };
        let credits = world.craft_credits(craft).unwrap_or(0) as f64;
        let busy = !world.craft_is_idle(craft).unwrap_or(false);
        let horizon = self.trader.horizon.max(1);
        let elapsed = self.ticks_in_episode[env].min(horizon);
        let time_remaining_frac = ((horizon - elapsed) as f64 / horizon as f64) as f32;

        let obs_base = env * self.obs_dim;
        write_obs_trader(
            &rows,
            fuel_frac,
            credits,
            busy,
            time_remaining_frac,
            &mut out[obs_base..obs_base + TRADER_OBS_DIM],
        );
        // Pirates variant (spec §11): append the K=2 contact blocks (dims
        // 20-33). `pirate_contacts` returns distance-sorted RAW evidence —
        // rel-pos/strength/lying-low, never a route score.
        if self.obs_dim == TRADER_PIRATES_OBS_DIM {
            let contacts = world.pirate_contacts(craft);
            write_obs_pirate_contacts(
                &contacts,
                &mut out[obs_base + TRADER_OBS_DIM..obs_base + self.obs_dim],
            );
        }
    }

    /// Trader-mode macro-step. Decode the f32 action index (0 = wait,
    /// j = accept board slot j-1), then advance world ticks until the next
    /// decision point:
    ///   - accept path: until the craft is idle again (delivered / failed /
    ///     no-op accept of a stale-empty slot ≡ a 1-tick wait) or horizon;
    ///   - wait path: exactly `TRADER_WAIT_TICKS` ticks (or horizon).
    ///
    /// Reward = Δ craft credits over the macro-step, in credits (micros/1e6).
    /// `terminated` is always false (continuing task); `truncated` at the
    /// horizon, then AUTO-RESET with the thrust-mode derived-seed scheme.
    fn step_trader(
        &mut self,
        act: &[f32],
        obs: &mut [f32],
        rew: &mut [f32],
        term: &mut [bool],
        trunc: &mut [bool],
    ) {
        for env in 0..self.num_envs {
            let craft = self.worlds[env].craft_ids()[0];
            let a_base = env * self.action_dim;
            let choice = (act[a_base] as f64)
                .round()
                .clamp(0.0, TRADER_BOARD_SLOTS as f64) as usize;
            let accept_path = choice >= 1;

            let mut cmds: Vec<Command> = Vec::new();
            if accept_path
                && let Some(cid) = self.board_ids[env * TRADER_BOARD_SLOTS + (choice - 1)]
            {
                cmds.push(Command {
                    target: Target::Entity(EntityRef::Craft(craft)),
                    kind: CommandKind::AcceptContract { contract: cid },
                });
            }

            let credits_before = self.prev_credits[env];
            let mut truncated = false;
            let mut ticks_advanced: u64 = 0;
            loop {
                // Commands only on the FIRST tick of the macro-step.
                self.worlds[env].step(&mut cmds);
                cmds.clear();
                self.ticks_in_episode[env] += 1;
                ticks_advanced += 1;
                if self.ticks_in_episode[env] >= self.trader.horizon {
                    truncated = true;
                    break;
                }
                let idle = self.worlds[env].craft_is_idle(craft).unwrap_or(true);
                if accept_path {
                    // Accept ran until the trip resolved (idle again) — or the
                    // accept was a no-op (empty/stale slot, unfunded revert)
                    // and the craft never left idle: a 1-tick wait.
                    if idle {
                        break;
                    }
                } else if ticks_advanced >= TRADER_WAIT_TICKS {
                    break;
                }
            }

            let credits_now = self.worlds[env].craft_credits(craft).unwrap_or(0);
            rew[env] = ((credits_now - credits_before) as f64 / 1.0e6) as f32;
            term[env] = false; // continuing task: never terminated
            trunc[env] = truncated;
            self.prev_credits[env] = credits_now;
            self.write_trader_obs(env, obs);

            if truncated {
                // Auto-reset (SB3 VecEnv contract), same derived-seed scheme
                // as thrust mode; the obs written above is overwritten with
                // the NEW episode's initial obs while the flags still report
                // the old episode's end.
                self.episode_counter += 1;
                let fresh_seed = self.master_seed ^ self.episode_counter;
                self.reset_trader_episode(env, fresh_seed, obs);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_action_builds_ego_relative_destination_with_budget() {
        let ego = Vec3::new(1.0, 2.0, 3.0);
        let craft = CraftId { slot: 0, generation: 0 };
        let a = [10.0f32, 0.0, 0.0, 5.0];
        let cmd = decode_action(&a, ego, craft);
        assert_eq!(cmd.target, Target::Entity(EntityRef::Craft(craft)));
        match cmd.kind {
            CommandKind::Destination { dest, burn_budget } => {
                match dest {
                    NavDest::Position(p) => {
                        assert!((p.x - 11.0).abs() < 1e-6);
                        assert!((p.y - 2.0).abs() < 1e-6);
                        assert!((p.z - 3.0).abs() < 1e-6);
                    }
                    NavDest::Entity(_) => panic!("expected Position dest"),
                }
                assert_eq!(burn_budget, Some(5.0));
            }
            other => panic!("expected Destination, got {other:?}"),
        }
    }

    #[test]
    fn decode_action_negative_budget_is_none() {
        let ego = Vec3::ZERO;
        let craft = CraftId { slot: 0, generation: 0 };
        let a = [0.0f32, 0.0, 0.0, -1.0];
        let cmd = decode_action(&a, ego, craft);
        match cmd.kind {
            CommandKind::Destination { burn_budget, .. } => assert_eq!(burn_budget, None),
            other => panic!("expected Destination, got {other:?}"),
        }
    }

    #[test]
    fn flight_reward_potential_shaping_rewards_approach() {
        let cfg = FlightCfg::default(); // gamma 0.99
        // moved from 0.5 to 0.4 distance: shaping term must be net-positive
        let r = flight_reward(&cfg, 0.5, 0.4, 0.0, false, 0.0);
        assert!(r > 0.0, "approach must be net-positive: {r}");
        let r_away = flight_reward(&cfg, 0.4, 0.5, 0.0, false, 0.0);
        assert!(r_away < 0.0, "retreat must be net-negative: {r_away}");
    }

    #[test]
    fn flight_reward_arrival_bonus_dominates() {
        let cfg = FlightCfg::default();
        let r = flight_reward(&cfg, 0.01, 0.0, 0.0, true, 0.0);
        assert!(r > 1.0, "arrival must dwarf per-tick terms: {r}");
    }

    #[test]
    fn arrival_requires_low_relative_speed() {
        let cfg = FlightCfg::default();
        assert!(is_arrival(&cfg, cfg.arrival_radius * 0.5, cfg.arrival_speed * 0.5));
        assert!(!is_arrival(&cfg, cfg.arrival_radius * 0.5, cfg.arrival_speed * 2.0)); // flyby
        assert!(!is_arrival(&cfg, cfg.arrival_radius * 2.0, 0.0));
    }

    #[test]
    fn target_draw_is_seed_deterministic_and_distance_bounded() {
        let cfg = FlightCfg {
            target_dist_min: 0.001,
            target_dist_max: 0.01,
            ..FlightCfg::default()
        };
        let a = draw_target(&cfg, 42, 0);
        let b = draw_target(&cfg, 42, 0);
        let c = draw_target(&cfg, 43, 0);
        assert_eq!(a, b, "same seed -> same target");
        assert_ne!(a, c, "different seed -> different target");
        let d = a.length();
        assert!((0.001..=0.01).contains(&d), "distance {d} out of band");
    }

    #[test]
    fn configure_thrust_mode_sets_velocity_consistent_with_star_mass() {
        // Stage 0 (gravity off): a stale 1-M_sun orbital velocity (0.0172
        // AU/day) would carry the craft ballistically past every target
        // (a_max = 1e-5 AU/day^2 cannot counter it). With star_mass = 0 the
        // craft must start at rest; with star_mass = 1 it must start at the
        // circular-orbit speed for its spawn radius.
        let mut env = JumpgateEnv::new(1, 1, 0);
        env.configure(
            1, None, None, Some(0.0), None, None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        assert_eq!(
            env.template.craft[0].vel.length(),
            0.0,
            "gravity off -> craft starts at rest"
        );
        env.configure(
            1, None, None, Some(1.0), None, None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        let r = env.template.craft[0].pos.length();
        let v_circ = (G_CANONICAL * 1.0 / r).sqrt();
        let v = env.template.craft[0].vel.length();
        assert!(
            (v - v_circ).abs() < 1e-15,
            "gravity on -> circular-orbit speed: got {v}, want {v_circ}"
        );
    }

    #[test]
    fn trader_template_resolves_and_seed_varies_geometry() {
        let a = trader_config_template(1, 1, 0);
        let b = trader_config_template(1, 1, 0);
        let c = trader_config_template(2, 1, 0);
        // Deterministic per seed:
        assert_eq!(a.bodies[1].elements.m0, b.bodies[1].elements.m0);
        // Varied across seeds (anti-memorization):
        assert_ne!(a.bodies[1].elements.m0, c.bodies[1].elements.m0);
        // Resolvable (anti-tunnel guard passes, economy refs in range) AND the
        // craft spawns co-located with station 0's body (same-tick first load).
        let (world, _hash) = World::reset(a).expect("resolvable trader cfg");
        let craft = world.craft_ids()[0];
        let board = world.offered_contracts();
        assert_eq!(board.len(), 4, "four seeded routes on the board");
        let from_station = board[0].2; // route 0 -> 1 pickup (station 0)
        let d = world
            .station_pos(from_station)
            .expect("live station")
            .sub(world.craft_pos(craft).expect("live craft"))
            .length();
        assert!(d < 1e-9, "craft co-located with station 0's body at t=0: d={d}");
        // Scripted ASSIGN is OFF: stepping a few ticks must NOT bind the craft.
        let (mut world, _hash) = World::reset(trader_config_template(1, 1, 0)).expect("cfg");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..10 {
            world.step(&mut empty);
        }
        assert_eq!(world.craft_is_idle(craft), Some(true), "no scripted acceptance");
    }

    // --- trader macro-step tests (no Python needed) ---

    /// Build a trader-mode env the way `configure(mode=2)` + `reset(seed)`
    /// would, but driveable from Rust (the pymethods need numpy buffers).
    fn trader_env(num_envs: usize, horizon: u64) -> JumpgateEnv {
        let mut env = JumpgateEnv::new(num_envs, 1, 0);
        env.control_mode = 2;
        env.trader = TraderCfg { horizon };
        env.obs_dim = TRADER_OBS_DIM;
        env.action_dim = TRADER_ACTION_DIM;
        env
    }

    fn trader_reset(env: &mut JumpgateEnv, seed: u64, out: &mut [f32]) {
        env.master_seed = seed;
        env.episode_counter = 0;
        for e in 0..env.num_envs {
            env.reset_trader_episode(e, seed, out);
        }
    }

    #[test]
    fn trader_macro_step_accept_pays_delta_credits() {
        // The end-to-end proof IN RUST: accept the co-located route's slot,
        // macro-step until the trip resolves, and the escrow settlement shows
        // up as a positive Δ-credits reward.
        let mut env = trader_env(1, 2000);
        let mut obs = vec![0.0f32; TRADER_OBS_DIM];
        trader_reset(&mut env, 7, &mut obs);

        // Craft spawns co-located with station 0's body; board row 0 is the
        // seeded route 0->1 (reward 1.0 cr), so slot 0's pickup distance ~ 0.
        assert_eq!(obs[0], 1.0, "slot 0 present");
        assert!(obs[2] < 1e-6, "slot-0 pickup is co-located: {}", obs[2]);
        assert_eq!(obs[18], 0.0, "craft idle at the decision point");

        let act = [1.0f32]; // accept slot 0
        let mut rew = [0.0f32];
        let mut term = [false];
        let mut trunc = [false];
        env.step_trader(&act, &mut obs, &mut rew, &mut term, &mut trunc);

        assert!(
            rew[0] > 0.0,
            "accept macro-step must pay the delivery (rew={}, ticks={})",
            rew[0],
            env.ticks_in_episode[0]
        );
        assert!((rew[0] - 1.0).abs() < 1e-6, "route 0->1 pays 1.0 cr: {}", rew[0]);
        assert!(!term[0], "terminated is always false");
        assert!(!trunc[0], "trip resolved inside the horizon");
        let craft = env.worlds[0].craft_ids()[0];
        assert_eq!(env.worlds[0].craft_is_idle(craft), Some(true), "idle again");
        assert_eq!(obs[18], 0.0, "next decision point: not busy");
        // Credits now visible in the own block (1 cr / 30 cr scale).
        assert!((obs[17] - (1.0e6 / crate::obs::TRADER_CREDITS_SCALE) as f32).abs() < 1e-9);
    }

    #[test]
    fn trader_wait_advances_eight_ticks() {
        let mut env = trader_env(1, 2000);
        let mut obs = vec![0.0f32; TRADER_OBS_DIM];
        trader_reset(&mut env, 7, &mut obs);

        let act = [0.0f32]; // wait
        let mut rew = [0.0f32];
        let mut term = [false];
        let mut trunc = [false];
        env.step_trader(&act, &mut obs, &mut rew, &mut term, &mut trunc);

        assert_eq!(env.ticks_in_episode[0], TRADER_WAIT_TICKS);
        assert_eq!(env.worlds[0].tick().0, TRADER_WAIT_TICKS);
        assert_eq!(rew[0], 0.0, "waiting earns nothing");
        assert!(!term[0] && !trunc[0]);
        // time_remaining_frac dropped by exactly 8/2000.
        let expect = ((2000 - TRADER_WAIT_TICKS) as f64 / 2000.0) as f32;
        assert_eq!(obs[19], expect);
    }

    #[test]
    fn trader_accept_of_empty_slot_is_a_one_tick_wait() {
        let mut env = trader_env(1, 2000);
        let mut obs = vec![0.0f32; TRADER_OBS_DIM];
        trader_reset(&mut env, 7, &mut obs);
        // Force slot 3 empty (only 4 seeded offers exist, so make one vanish
        // from the snapshot): clear the captured id directly.
        env.board_ids[3] = None;

        let act = [4.0f32]; // accept the (now empty) slot 3
        let mut rew = [0.0f32];
        let mut term = [false];
        let mut trunc = [false];
        env.step_trader(&act, &mut obs, &mut rew, &mut term, &mut trunc);

        assert_eq!(env.ticks_in_episode[0], 1, "no-op accept ≡ 1-tick wait");
        assert_eq!(rew[0], 0.0);
        let craft = env.worlds[0].craft_ids()[0];
        assert_eq!(env.worlds[0].craft_is_idle(craft), Some(true));
    }

    #[test]
    fn trader_truncates_at_horizon_and_autoresets() {
        let mut env = trader_env(1, 20);
        let mut obs = vec![0.0f32; TRADER_OBS_DIM];
        trader_reset(&mut env, 7, &mut obs);

        let act = [0.0f32];
        let mut rew = [0.0f32];
        let mut term = [false];
        let mut trunc = [false];
        // 8 + 8 + 4(clipped at horizon 20) ticks -> truncation on step 3.
        for step in 0..3 {
            env.step_trader(&act, &mut obs, &mut rew, &mut term, &mut trunc);
            assert!(!term[0], "never terminated (step {step})");
            if step < 2 {
                assert!(!trunc[0], "not yet truncated (step {step})");
            }
        }
        assert!(trunc[0], "horizon hit truncates");
        // Auto-reset happened: fresh episode counter, fresh clock, fresh obs
        // (the new episode's first decision point: full time remaining).
        assert_eq!(env.episode_counter, 1);
        assert_eq!(env.ticks_in_episode[0], 0);
        assert_eq!(obs[19], 1.0, "new episode obs: time_remaining_frac == 1");
        assert_eq!(obs[18], 0.0, "new episode obs: idle");
        assert_eq!(obs[0], 1.0, "new episode obs: a fresh seeded board");
    }

    #[test]
    fn trader_reset_is_seed_deterministic_and_seed_varied() {
        let mut env = trader_env(1, 2000);
        let mut a = vec![0.0f32; TRADER_OBS_DIM];
        let mut b = vec![0.0f32; TRADER_OBS_DIM];
        let mut c = vec![0.0f32; TRADER_OBS_DIM];
        trader_reset(&mut env, 11, &mut a);
        trader_reset(&mut env, 11, &mut b);
        trader_reset(&mut env, 12, &mut c);
        assert_eq!(a, b, "same seed -> identical first obs");
        assert_ne!(a, c, "different seed -> different geometry/obs");
    }

    #[test]
    fn compute_reward_penalizes_fuel_and_time() {
        // spent 2.0 fuel, dt = 1.0 day, not arrived
        let r = compute_reward(10.0, 8.0, false, 1.0);
        assert!((r - (-2.0 - 0.001)).abs() < 1e-9, "got {r}");
    }

    #[test]
    fn compute_reward_adds_arrival_bonus() {
        // no fuel spent, dt = 1.0, arrived
        let r = compute_reward(10.0, 10.0, true, 1.0);
        assert!((r - (1.0 - 0.001)).abs() < 1e-9, "got {r}");
    }
}
