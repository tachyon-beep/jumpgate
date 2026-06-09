//! `JumpgateEnv`: the Gymnasium PyO3 facade. Holds one `World` per env, writes
//! frame-relative obs / reward / terminated / truncated into caller buffers.
//! Pure `decode_action` / `compute_reward` are unit-tested without the GIL.

use crate::obs::{write_obs_frame_relative, OBS_DIM};
use jumpgate_core::{
    BaseSpec, BodyInit, Command, CommandKind, CraftId, CraftInit, Dt, EntityRef, Event, EventKind,
    FullObserver, GuidanceParams, NavDest, OrbitalElements, RunConfig, StateView, SubstepCfg,
    Target, Vec3, World,
};
use numpy::{PyReadonlyArray1, PyReadwriteArray1};
use pyo3::prelude::*;

pub const ACTION_DIM: usize = 4;

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
    time_limit: u64, // truncation horizon in ticks
    template: RunConfig,
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

    /// Rebuild every world with `seed` (the gym seed BECOMES the master seed,
    /// distinct per env), then write the initial obs into `out_obs`.
    fn reset(&mut self, seed: u64, mut out_obs: PyReadwriteArray1<f32>) {
        let out = out_obs.as_slice_mut().expect("out_obs must be contiguous");
        for env in 0..self.num_envs {
            let mut cfg = self.template.clone();
            cfg.master_seed = seed.wrapping_add(env as u64);
            let (world, _hash) = World::reset(cfg).expect("resolvable cfg");
            self.worlds[env] = world;

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
