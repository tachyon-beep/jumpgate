# jumpgate v1 — Plan 0: Contract Surface & Conventions

> **READ THIS FIRST.** Every task in Plans 1–4 is authored against the canonical type/trait signatures and conventions below. This is the drift-lock anchor: if a task body and this document disagree, **this document wins** and the task is the bug.

**Goal:** Build the jumpgate v1 rung-3 deterministic (Tier B) 3D Newtonian space core — on-rails bodies, gravity-feeling thrust/fuel/mass craft flown by an in-engine autopilot under a navigator macro-action — exposed as a reproducible Gymnasium env, with a per-tick state-hash replay-equivalence contract.

**Architecture:** Two crates: a pure-Rust `jumpgate-core` (`#![forbid(unsafe_code)]`) that is the sole authoritative writer (SoA stores, tick-indexed ephemeris, velocity-Verlet behind an Integrator trait with accel-keyed integer substepping, Tsiolkovsky variable-mass craft, autopilot guidance, one typed Command/Target ingestion path, a typed Event stream, FNV-1a state hashing, and log-replay), and a `jumpgate-py` PyO3 cdylib facade that writes frame-relative f32 observations into caller-provided buffers and presents the Gymnasium 5-tuple. All facades read through one `StateView` trait that exposes command+event history, not just physics; the engine is shaped (Target sum, Event typing, observer-parameterized projection, effective-param accessor, slot-map ids, Lod seam) so combat/upgrades/fog-of-war drop in without a contract break.

**Tech Stack:** Rust 2021 edition (rustc/cargo 1.95; edition 2021 is deliberate — `gen` is a reserved keyword in edition 2024 but is used as a struct field in `CraftId`/`BodyId`/slot-map generations); jumpgate-core deps: rand_chacha (pinned, ChaCha8Rng) + rand_core only; no serde/glam/rayon in the hashed path; hand-rolled f64 Vec3. jumpgate-py: pyo3 0.23 + numpy 0.23 (abi3-py312, extension-module). Build via /home/john/jumpgate/archive/.venv/bin/python -m maturin develop. Python test deps already present: gymnasium 1.2.3, numpy 2.4.6, torch 2.9.1. Workspace-root clippy.toml with disallowed-methods. FNV-1a hashing hand-rolled over f64::to_bits little-endian.

---

## Plan-level conventions (apply to every task)

Cross-cutting decisions consolidated from the adversarial plan review (architecture/quality/reality/systems panel). They override any task-local drift.

1. **Pinned dependency versions (workspace-wide, exact `=`):** `rand_chacha = "=0.10.0"`, `rand_core = "=0.10.1"`. No other RNG crate. `pyo3 = "0.23"`, `numpy = "0.23"` in `jumpgate-py` only. No `serde`/`glam`/`rayon` anywhere in the hashed path.
2. **Acyclic module order** (dependency edges, not just declaration order): `math -> time -> ids -> types -> config -> contract -> stores -> ephemeris -> integrator -> ship -> autopilot -> ingest -> events -> hash -> world -> replay`. `ids` precedes `types` because `EntityRef = Craft(CraftId) | Body(BodyId)` and `NavDest = Position(Vec3) | Entity(EntityRef)` reference the id types. Primitive seam enums (`Lod`, `NavDest`) live in `types.rs` (Task 3) so `stores` (Task 4) never forward-imports `contract` (Task 6).
3. **Single-definition rule.** The `Integrator` trait and the `StateView` trait are each defined **exactly once**, in `contract.rs`. All other modules import them from `crate::contract` and write impls only. (Task 8 includes a `grep` guard asserting exactly one `pub trait Integrator`.)
4. **One canonical FNV-1a field order.** The shared `FnvHasher` + `HASH_MAGIC`/`HASH_FORMAT_VERSION` + numbered `HASH_FIELD_ORDER` anchor lands in `hash.rs` in **Task 3**; **Task 13 modifies that file** to add `state_hash`/`write_store_cursor` and append store fields to the numbered order (it does NOT re-create `hash.rs`). `hash.rs` is the single authority for the per-tick state-hash field ordering over `f64::to_bits()` (little-endian), including the slot-map allocator cursor, behind the `MAGIC + FORMAT_VERSION` header. `config_hash` (Task 3) uses a **deliberately separate local FNV** (same FNV-1a constants/discipline, distinct seed tag) — run-identity hash and per-tick state hash are intentionally never the same hasher. Golden hash constants are filled from the first green run.
5. **`Lod`/`Wake` is a built v1 must-shape seam** (not a do-not-build item): the `Lod` enum, a `Wake` `EventKind` variant, and a Lod-dispatch branch in `World::step` ship in v1 (single tier implemented; scheduler deferred).
6. **Determinism floor (every task upholds it):** integer `tick: u64` (`dt` fixed at init, never a `step()` arg); master seed to named separate `ChaCha8Rng` sub-streams; canonical sorted-by-target command application via one ingestion path; replay re-feeds the action log; `clippy` `disallowed-methods` bans `SystemTime`/`Instant::now`/`thread_rng`/`from_entropy`/`env::var`.

---

## Workspace layout

```
/home/john/jumpgate/
├── Cargo.toml                      # [workspace] resolver=3, members = core + py; pinned deps in [workspace.dependencies]
├── clippy.toml                     # disallowed-methods: SystemTime, Instant::now, thread_rng, from_entropy, env::var/vars; lint test modules via clippy --all-targets
├── rust-toolchain.toml             # pin channel for Tier-B same-binary reproducibility
├── crates/
│   ├── jumpgate-core/
│   │   ├── Cargo.toml              # #![forbid(unsafe_code)] crate; deps rand_chacha (pinned), rand_core
│   │   └── src/
│   │       ├── lib.rs              # forbid(unsafe_code); pub re-exports; module wiring; crate-level docs of the contract
│   │       ├── math.rs             # hand-rolled f64 Vec3 (+ scalar ops, dot, length, norm); units constants (AU, M_sun, day, G_canonical, softening default)
│   │       ├── ids.rs              # CraftId{slot,gen}, BodyId{slot,gen}; generational slot-map SlotMap<T> with hashable allocator cursor
│   │       ├── time.rs             # Tick(u64), dt fixed-at-init wrapper, sim_time = tick*dt derivation helper
│   │       ├── config.rs           # RunConfig (bodies, craft count, per-ship BaseSpec, master seed, dt bits, softening, substep params); ConfigHash compute (FNV-1a over to_bits)
│   │       ├── rng.rs              # RngStreams: master u64 -> named separate ChaCha8Rng sub-streams via fixed derivation
│   │       ├── contract.rs         # SHARED CONTRACT: Target/EntityRef, Command/CommandKind/Destination, Event/EventKind, Lod, Integrator trait, StateView trait (signatures; landed early)
│   │       ├── stores.rs           # ShipStore (SoA: pos,vel,fuel_mass,nav_state,lod), BodyStore (SoA: orbital elements, ephemeris handles), BaseSpec, effective-param accessor
│   │       ├── ephemeris.rs        # Kepler-solve-once at init -> integer-tick body position+velocity table; deterministic sub-tick interpolation (linear; cubic-Hermite seam)
│   │       ├── integrator.rs       # Integrator trait impls: VelocityVerlet (two-eval moving-field, tagged) + RK4 (golden); substep_count(total_accel_mag) pure fn; softening in accel kernel
│   │       ├── ship.rs             # Tsiolkovsky variable-mass: accel = F/(eff_dry_mass+fuel_mass); fuel consumption by eff exhaust velocity; substep-granular mass update
│   │       ├── autopilot.rs        # deterministic guidance law: reads resolved nav_state field -> per-substep thrust vector; arrival/budget/fuel cutoff
│   │       ├── ingest.rs           # single ing_command path; canonical total ordering across World/Sim/Entity targets; ActionLog (tick-stamped); lever invariant enforcement
│   │       ├── events.rs           # EventStream record buffer; emit at tick boundary against quantized state; Arrival + FuelEmpty detectors (no reactivity/bus)
│   │       ├── world.rs            # World: owns stores/ephemeris/rng/log/events/tick; reset(RunConfig)->ConfigHash; step() assembly; impl StateView; project(observer)+visible()
│   │       ├── hash.rs             # FnvHasher; per-tick state hash over canonical sorted to_bits (incl slot-map cursor); MAGIC+FORMAT_VERSION header
│   │       └── replay.rs           # record (action log + per-tick hash), replay re-feeds log, assert hash equality, report first differing tick
│   └── jumpgate-py/
│       ├── Cargo.toml              # cdylib+rlib; pyo3 0.23, numpy 0.23; feature extension-module; NO forbid(unsafe) (FFI needs unsafe)
│       ├── pyproject.toml          # maturin backend, module-name jumpgate._native, abi3-py312, python-source ../../python
│       └── src/
│           ├── lib.rs             # #[pymodule] _native; registers JumpgateEnv
│           ├── env.rs             # JumpgateEnv: new(num_envs,num_craft,...), reset(seed)->writes obs buffer, step(action_buf)->writes obs/reward/done buffers; 5-tuple assembly
│           └── obs.rs             # frame-relative f64->f32 extraction into caller buffers; ego block + presence-masked entity set; debug-assert no absolute coord crosses f32 boundary
├── python/
│   └── jumpgate/
│       ├── __init__.py            # re-export native env
│       └── gym_env.py             # gymnasium.Env wrapper around _native.JumpgateEnv (spaces, 5-tuple, seeded reset)
└── tests/  (per-crate src tests + integration)
    ├── crates/jumpgate-core/tests/replay_equivalence.rs
    ├── crates/jumpgate-core/tests/physics_sanity.rs
    └── python/tests/test_gym_smoke.py   # reset/step shapes+dtypes; same seed -> identical obs sequence
```

---

## Canonical type contract (verbatim signatures — use these names exactly)

```rust
// ============================================================================
// VEC3 STRATEGY (chosen): hand-rolled f64 Vec3. Rationale:
// (1) Core is #![forbid(unsafe_code)]; a hand-rolled struct needs no unsafe.
// (2) Tier-B determinism hashes f64::to_bits() in a FIXED field order; owning
//     the layout (x,y,z, repr default) makes the hash encoding unambiguous and
//     removes any dependency on a third-party crate's FP codegen / version.
// (3) f64 (NOT f32): no SIMD, no mantissa loss at solar-system scale; the only
//     precision boundary is the f32 OBSERVATION downcast (frame-relative), which
//     lives in jumpgate-py, never in core math.
// glam DVec3+scalar-math was considered (DVec3 is already non-SIMD) but rejected
// to avoid justifying around someone else's internals for a ~60-line type.
// ============================================================================

// ---- math.rs ----
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 { pub x: f64, pub y: f64, pub z: f64 }
impl Vec3 {
    pub const ZERO: Vec3;
    pub fn new(x: f64, y: f64, z: f64) -> Vec3;
    pub fn add(self, o: Vec3) -> Vec3;
    pub fn sub(self, o: Vec3) -> Vec3;
    pub fn scale(self, s: f64) -> Vec3;
    pub fn dot(self, o: Vec3) -> f64;
    pub fn length(self) -> f64;
    pub fn length_sq(self) -> f64;
    pub fn normalize_or_zero(self) -> Vec3;
    /// fixed field order for hashing: x then y then z
    pub fn to_bits(self) -> [u64; 3];
}
// Canonical units (AU, M_sun, day); G folded so quantities ~O(1).
pub const G_CANONICAL: f64; // AU^3 / (M_sun * day^2)

// ---- ids.rs ----
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CraftId { pub slot: u32, pub gen: u32 }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId  { pub slot: u32, pub gen: u32 }

/// Generational slot-map. `cursor()` (next free slot / high-water) is HASHED state.
pub struct SlotMap<T> { /* dense values + gen array + free list + cursor */ }
impl<T> SlotMap<T> {
    pub fn new() -> Self;
    pub fn len(&self) -> usize;
    pub fn cursor(&self) -> u64;            // included in per-tick hash
}

// ---- time.rs ----
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);
/// dt is fixed at init, stored as its u64 bit pattern; NEVER a step() argument.
#[derive(Clone, Copy, Debug)]
pub struct Dt(f64);
impl Dt { pub fn new(dt: f64) -> Dt; pub fn get(self) -> f64; pub fn bits(self) -> u64; }
pub fn sim_time(tick: Tick, dt: Dt) -> f64; // (tick as f64) * dt, derived only where needed

// ---- config.rs ----
#[derive(Clone, Debug)]
pub struct BaseSpec {
    pub base_dry_mass: f64,
    pub base_max_thrust: f64,
    pub base_exhaust_velocity: f64,
    pub base_fuel_capacity: f64,
}
#[derive(Clone, Debug)]
pub struct BodyInit { pub mass: f64, pub elements: OrbitalElements } // Kepler conic
#[derive(Clone, Debug)]
pub struct OrbitalElements { pub a: f64, pub e: f64, pub i: f64, pub raan: f64, pub argp: f64, pub m0: f64 }
#[derive(Clone, Debug)]
pub struct CraftInit { pub spec: BaseSpec, pub pos: Vec3, pub vel: Vec3, pub fuel_mass: f64 }
#[derive(Clone, Debug)]
pub struct RunConfig {
    pub master_seed: u64,            // gym reset(seed) OVERWRITES this per episode
    pub dt: Dt,
    pub softening: f64,              // epsilon in (r^2+eps^2)^1.5
    pub substep_cfg: SubstepCfg,
    pub ephemeris_window: u64,       // ticks precomputed
    pub bodies: Vec<BodyInit>,
    pub craft: Vec<CraftInit>,
}
#[derive(Clone, Copy, Debug)]
pub struct SubstepCfg { pub accel_bin_base: f64, pub max_substeps: u32 } // N = f(total accel mag)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigHash(pub u64);
impl RunConfig { pub fn config_hash(&self) -> ConfigHash; } // FNV-1a over seed,dt.bits,fields via to_bits

// ---- rng.rs ----
/// Named sub-streams, each a SEPARATE ChaCha8Rng seeded by a fixed derivation
/// from master (NOT drawn from a shared parent). Pinned rand_chacha.
pub struct RngStreams { /* one ChaCha8Rng per named stream */ }
#[derive(Clone, Copy)]
pub enum RngStream { Intervention, Scenario }
impl RngStreams {
    pub fn from_master(master: u64) -> Self;
    pub fn stream(&mut self, which: RngStream) -> &mut rand_chacha::ChaCha8Rng;
}

// ---- contract.rs  (SHARED; landed early, bodies stubbed) ----
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityRef { Craft(CraftId), Body(BodyId) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target { Entity(EntityRef), World, Sim }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NavDest { Position(Vec3), Entity(EntityRef) }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CommandKind {
    /// v1's ONLY variant. burn_budget: optional scalar Δv cap.
    Destination { dest: NavDest, burn_budget: Option<f64> },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command { pub target: Target, pub kind: CommandKind }
/// Total, deterministic ordering across World/Sim/Entity scopes for canonical apply.
pub fn command_sort_key(c: &Command) -> (u8, u32, u32); // (scope_rank, slot, gen)

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    Arrival   { craft: CraftId, dest: NavDest },
    FuelEmpty { craft: CraftId },
    ThrustApplied { craft: CraftId, dv: f64 },
    ActionIngested { target: Target },
    Reward    { craft: CraftId, value: f64 },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event { pub tick: Tick, pub kind: EventKind }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lod { Player, NpcInteraction, Nothing } // v1 implements Player only

/// Verlet needs body pos at BOTH t_n and t_{n+1}; impls take an ephemeris sampler.
pub trait Integrator {
    fn step_craft(
        &self,
        pos: Vec3, vel: Vec3,
        accel_at: &dyn Fn(Vec3, f64 /*sub_t in days*/) -> Vec3, // gravity(soft)+thrust
        dt: f64, n_substeps: u32,
    ) -> (Vec3, Vec3);
    fn name(&self) -> &'static str;
}

/// Read trait ALL facades read through. Carries intent (cmd+event history), not just physics.
pub trait StateView {
    fn tick(&self) -> Tick;
    fn dt(&self) -> Dt;
    fn craft_ids(&self) -> Vec<CraftId>;
    fn craft_pos(&self, id: CraftId) -> Option<Vec3>;
    fn craft_vel(&self, id: CraftId) -> Option<Vec3>;
    fn craft_fuel(&self, id: CraftId) -> Option<f64>;
    fn body_ids(&self) -> Vec<BodyId>;
    fn body_pos(&self, id: BodyId, tick: Tick) -> Option<Vec3>;   // derived from tick via ephemeris
    fn recent_commands(&self, since: Tick) -> &[Command];
    fn recent_events(&self, since: Tick) -> &[Event];
    fn lod(&self, id: CraftId) -> Option<Lod>;
}

// ---- stores.rs ----
#[derive(Clone, Copy, Debug)]
pub enum NavState { Idle, Seeking { dest: NavDest, dv_remaining: f64 } } // resolved field autopilot reads
pub struct ShipStore {
    pub ids: SlotMap<()>,        // slot/gen authority
    pub pos: Vec<Vec3>, pub vel: Vec<Vec3>,
    pub fuel_mass: Vec<f64>,
    pub spec: Vec<BaseSpec>,
    pub nav: Vec<NavState>,
    pub lod: Vec<Lod>,
}
pub struct BodyStore { pub ids: SlotMap<()>, pub mass: Vec<f64>, pub eph_index: Vec<usize> }
/// effective = base × component-mods × wear; v1 mods/wear = identity (effective==base).
pub struct Effective { pub dry_mass: f64, pub max_thrust: f64, pub exhaust_velocity: f64, pub fuel_capacity: f64 }
pub fn effective_params(spec: &BaseSpec) -> Effective; // ONLY accessor integrator/autopilot read

// ---- ephemeris.rs ----
pub struct Ephemeris { /* per-body Vec<Vec3> pos + Vec<Vec3> vel over window, base_tick */ }
impl Ephemeris {
    pub fn precompute(bodies: &[BodyInit], dt: Dt, window: u64) -> Ephemeris; // Kepler once at init
    pub fn body_pos(&self, body_idx: usize, tick: Tick) -> Vec3;
    pub fn body_pos_subtick(&self, body_idx: usize, tick: Tick, frac: f64) -> Vec3; // deterministic interp
}

// ---- integrator.rs ----
pub struct VelocityVerlet;
pub struct Rk4;
impl Integrator for VelocityVerlet { /* two body-pos evals per step, tagged moving-field */ }
impl Integrator for Rk4 { /* golden/validation */ }
/// N = pure fn of QUANTIZED total local acceleration magnitude (gravity+thrust); identical on replay.
pub fn substep_count(total_accel_mag: f64, cfg: SubstepCfg) -> u32;
/// softened gravity sum from bodies at a sub-tick instant.
pub fn gravity_accel(p: Vec3, body_positions: &[(Vec3, f64)], softening: f64) -> Vec3;

// ---- ship.rs ----
/// variable-mass: accel = thrust_force/(eff_dry_mass + fuel_mass); consume fuel by eff exhaust velocity.
pub fn thrust_accel_and_burn(eff: &Effective, fuel_mass: f64, thrust_dir: Vec3, throttle: f64, dt: f64)
    -> (Vec3 /*accel*/, f64 /*fuel_consumed*/);

// ---- autopilot.rs ----
/// deterministic guidance: reads resolved NavState (NOT Command), returns (thrust_dir, throttle).
pub fn autopilot_command(nav: NavState, pos: Vec3, vel: Vec3, dest_pos: Vec3, eff: &Effective) -> (Vec3, f64);
pub const ARRIVAL_RADIUS: f64;

// ---- ingest.rs ----
pub struct ActionLog { pub entries: Vec<(Tick, Command)> }
impl ActionLog { pub fn record(&mut self, tick: Tick, cmd: Command); pub fn at(&self, tick: Tick) -> &[Command]; }
/// THE single ingestion path: validates, resolves NavDest, sets NavState, logs, emits ActionIngested.
/// Applies in canonical command_sort_key order. Lever invariant enforced here.
pub fn ingest_commands(world: &mut World, tick: Tick, cmds: &mut Vec<Command>);

// ---- events.rs ----
pub struct EventStream { pub events: Vec<Event> }
impl EventStream { pub fn emit(&mut self, e: Event); pub fn since(&self, t: Tick) -> &[Event]; }
/// detect Arrival/FuelEmpty at tick boundary against QUANTIZED state; records only, no reactivity.
pub fn detect_boundary_events(world: &World, tick: Tick, out: &mut EventStream);

// ---- world.rs ----
pub struct World { /* ShipStore, BodyStore, Ephemeris, RngStreams, ActionLog, EventStream, tick, dt, config */ }
impl World {
    pub fn reset(cfg: RunConfig) -> (World, ConfigHash); // seed/dt come from cfg; recompute config hash
    pub fn step(&mut self, cmds: &mut Vec<Command>);      // dt is NOT an arg; ingest->substep integrate->events->tick++
    pub fn project<O: Observer>(&self, observer: &O) -> View;
}
impl StateView for World { /* ... */ }
/// observer-parameterized projection; visible() all-true in v1 (one location for future fog-of-war).
pub trait Observer { fn visible(&self, target: EntityRef) -> bool; }
pub struct FullObserver; // v1 default, visible()==true
pub struct View { /* projected, presence-masked snapshot the obs layer reads */ }

// ---- hash.rs ----
pub struct FnvHasher { state: u64 }
impl FnvHasher {
    pub fn new() -> Self;
    pub fn write_u64(&mut self, v: u64);     // little-endian to_bits values
    pub fn finish(self) -> u64;
}
pub const HASH_MAGIC: u64;
pub const HASH_FORMAT_VERSION: u32;
/// canonical order: header, tick, then bodies-then-craft by sorted id, incl SlotMap cursor.
pub fn state_hash(world: &World) -> u64;

// ---- replay.rs ----
pub struct Recording { pub config: RunConfig, pub log: ActionLog, pub hashes: Vec<(Tick, u64)> }
pub fn record_run(cfg: RunConfig, ticks: u64, driver: impl FnMut(Tick) -> Vec<Command>) -> Recording;
/// re-feeds log (NEVER the policy); returns Ok(()) or Err(first_differing_tick).
pub fn replay_run(rec: &Recording) -> Result<(), Tick>;

// ============================================================================
// jumpgate-py (PyO3 0.23 + numpy 0.23). unsafe ALLOWED here (FFI), NOT in core.
// ============================================================================
// env.rs
#[pyclass]
pub struct JumpgateEnv { /* Vec<World>, num_envs, num_craft, obs_dim, config template */ }
#[pymethods]
impl JumpgateEnv {
    #[new]
    fn new(num_envs: usize, num_craft: usize /*, scenario params */) -> Self;
    /// seed becomes RunConfig.master_seed per env; writes frame-relative obs into out buffer.
    fn reset(&mut self, seed: u64, out_obs: PyReadwriteArray1<f32>) -> ();
    /// action_buf: (num_envs*num_craft*action_dim); writes obs/reward/terminated/truncated/done buffers.
    /// returns nothing meaningful via buffers; gym 5-tuple assembled in python wrapper OR returned here.
    fn step(&mut self,
            action: PyReadonlyArray1<f32>,
            out_obs: PyReadwriteArray1<f32>,
            out_reward: PyReadwriteArray1<f32>,
            out_terminated: PyReadwriteArray1<bool>,
            out_truncated: PyReadwriteArray1<bool>) -> ();
    #[getter] fn obs_dim(&self) -> usize;
    #[getter] fn action_dim(&self) -> usize;
}
// obs.rs : frame-relative extraction (f64 sub in core units -> f32 delta only).
pub fn write_obs_frame_relative(view: &View, ego: CraftId, out: &mut [f32]); // debug_assert no abs coord -> f32
```

---

## Execution map

- **Plan 1 — Foundations** (Tasks 1–6): workspace/lint/toolchain, f64 `Vec3` + units, time/config/`ConfigHash`/`types`, generational slot-map + stores, named RNG sub-streams, the shared `contract.rs`.
- **Plan 2 — Physics** (Tasks 7–9): ephemeris (Kepler-once to tick table), `Integrator` impls + accel-keyed substepping + softened gravity, Tsiolkovsky variable-mass ship dynamics.
- **Plan 3 — Engine & replay** (Tasks 10–15): command ingestion + action log, event layer, `World::step` + `StateView` + projection, FNV state hash, replay-equivalence, physics-sanity + autopilot-transfer tests.
- **Plan 4 — Observation & gym** (Tasks 16–18): frame-relative obs extraction into caller buffers, PyO3 Gymnasium env, Python wrapper + smoke/determinism test + maturin build.

Execute in order. Each plan's tasks are TDD (failing test -> run-it-fails -> minimal impl -> run-it-passes -> commit).
