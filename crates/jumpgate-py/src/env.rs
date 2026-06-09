//! `JumpgateEnv`: the Gymnasium PyO3 facade. Holds one `World` per env, writes
//! frame-relative obs / reward / terminated / truncated into caller buffers.
//! Pure `decode_action` / `compute_reward` are unit-tested without the GIL.

use crate::obs::{write_obs_frame_relative, write_obs_thrust_mode, OBS_DIM, THRUST_OBS_DIM};
use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, CraftId, CraftInit, Dt, EntityRef, Event, EventKind,
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
    let scale = cfg.target_dist_max.max(1e-12);
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
    };
    let craft = (0..num_craft)
        .map(|_| CraftInit {
            spec: spec.clone(),
            pos: Vec3::new(1.0, 0.0, 0.0),    // 1 AU from the star
            vel: Vec3::new(0.0, 0.0172, 0.0), // ~circular at 1 AU: sqrt(G_CANONICAL) AU/day
            fuel_mass: 1.0e-12,
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
    fn new(num_envs: usize, num_craft: usize) -> Self {
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
    ) -> PyResult<(usize, usize)> {
        if mode > 1 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mode must be 0 (waypoint) or 1 (thrust)",
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
        if let Some(v) = arrival_bonus {
            f.arrival_bonus = v;
        }

        self.control_mode = mode;
        if mode == 1 {
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
        let mut env = JumpgateEnv::new(1, 1);
        env.configure(
            1, None, None, Some(0.0), None, None, None, None, None, None, None, None, None,
        )
        .unwrap();
        assert_eq!(
            env.template.craft[0].vel.length(),
            0.0,
            "gravity off -> craft starts at rest"
        );
        env.configure(
            1, None, None, Some(1.0), None, None, None, None, None, None, None, None, None,
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
