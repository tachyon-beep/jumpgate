# jumpgate — 3D Newtonian Space Core (v1) Design

**Date:** 2026-06-08
**Status:** Approved design (pending user review of this document)
**Supersedes:** the archived deterministic economy/DRL line (now in `archive/`), scrapped on hard reset.

---

## 1. Purpose & thesis

jumpgate exists to **demonstrate that deep RL is usable in a gaming context and is much more entertaining than FSM / scripted AI.** DRL-controlled agents are the showcase; the 3D physics simulation is the substrate that shows them off. Therefore the ML/gym surface is a **first-class v1 deliverable**, not a deferred add-on.

The artifact is a Rust simulation of a 3D space region the size of a solar system that moves objects under Newtonian physics, wired into a PyTorch / Gymnasium training loop via PyO3 + maturin — the proven pattern from the author's `keisei` and `murk` projects.

**North stars (fidelity ambition).** v1 is rung 3, but the long-term target is a multi-domain high-fidelity simulator — **economy, combat, exploration, industry** — with **crew-on-a-flight-deck as the depth yardstick** (a finger-in-the-air for how deep the simulation should eventually go). "Build for but not with" (§2) is judged against *this* ambition, not just the rung-5 deferral list: the seams must not *structurally preclude* these directions, even though none are built in v1. The one ambition that reaches past the documented deferral list is crew-level fidelity — which is exactly why it is the useful stress test for the foundation. Concrete additive forward-paths (new `EntityRef` kind, new SoA column, new obs function, parent-relative state storage) are noted at their seams. The items needing a conscious *decision* now — because `Event`/`EventKind` and the hash field order are frozen in the plan-0 anchor — are the fixed-rate event-clock invariant (§3.1) and the reserved per-entity hash-composition upgrade path (§6); both are recorded as decisions, **neither is built in v1** (Tier B requires neither).

**Non-goal:** scientific accuracy. It is not multiplayer and the program is the single source of truth for its universe. There is no external ground truth to be "wrong" against.

---

## 2. Scope

### v1 slice (this spec): through "ship with thrusters + fuel + mass"

The incremental rung ladder is: **universe → ship moves A→B → ship with thrusters+fuel+mass → station refuel → craft interactions.** v1 stops at rung 3 — the first slice that contains a *controllable agent*, so the DRL showcase thesis can be validated end-to-end on a real fuel-constrained control task.

v1 delivers:

1. A solar-system-scale 3D space with massive bodies (star, planets, moons) on **on-rails** orbits.
2. Mobile **craft** that feel gravity from the bodies and move under a deterministic, replayable integrator.
3. Craft with real physical state: **mass, thrust, fuel** (variable-mass / Tsiolkovsky — fuel is a finite Δv budget, not an on-time).
4. The **navigator** ML facade: a policy emits a macro-action (destination + optional burn-budget); an in-engine **autopilot** flies it.
5. A **Gymnasium-compatible** Python binding with reproducible `reset(seed)` / `step()` and frame-relative observations.
6. A **replay/determinism** contract with a per-tick state hash test surface.

### Build-for-but-not-with (the governance gate)

"Build for but not with" is defined precisely as: **make no decision that structurally precludes a future capability; implement only the trivial version now.** To keep this falsifiable, v1 has two explicit lists.

**MUST-shape seams** (cheap now, contract-break later — get them right in v1):

- One **state-access read trait** (`StateView`) that all facades read through.
- One **typed `Command` DTO** + a single ingestion function shared by all writers.
- **Tick-indexed body positions** (derived from `tick`, never stored mutable body state).
- An **`Lod` enum + dispatch seam + wake-event hook** on entities.
- **Per-type stores** keyed by a **generational slot-map id** (stable across deletion).
- An **integrator trait** (so RK4 can swap in for validation).
- The **frame-relative observation transform** (the real ML work).
- Step return shaped as **per-craft** + **presence-masked variable-N** from day one.
- Ship physical parameters are **derived, not hardcoded**: the physics layer reads *effective* values (thrust, exhaust velocity, fuel capacity, dry mass, …) through an **accessor**, so a later component / upgrade / wear layer (`effective = base × modifiers × wear`) drops in without touching the integrator. New per-ship scalars (heat, efficiency, …) attach as **additional SoA arrays + a tick-stage**.
- Command address is a **`Target` sum** — `Entity(craft|body) | World | Sim` — not a bare `CraftId`, so spawn, world/sim-level interventions, and entity-targeted navigation are not foreclosed (and the navigator can target a body). See §4.4.
- A **typed `Event`/`EventKind` record stream** mirroring `Command` (tick-stamped, one path, **no reactivity**). See §4.4.
- **`StateView` exposes the command + event history**, not just physical state — the intent read-surface the watcher/chronicle needs (thesis-critical). See §4.4.
- **Projection takes an observer at one seam** (`project(observer)`); the presence mask comes from a single `visible(observer, ·)` predicate (all-true in v1) — pins viewer-filter ≡ POMDP-mask. See §4.4.
- **Future-intervention determinism:** the slot-map allocator cursor is hashed state and minted ids are a pure function of the logged command stream; initial conditions are **one hashed run-config struct**. See §4.4 / §6.

**DO-NOT-BUILD-IN-V1 watch-list** (the default gravity toward `murk`'s 13-crate machinery — adding any of these requires an ADR naming a concrete v1 requirement):

- No arena / generational *allocator* (a simple slot-map is fine; the bump/ping-pong arena is not).
- No `ObsSpec`/`ObsPlan` observation compiler.
- No flatbuffers / serde in the hashed path.
- No observation pool/cache, no egress worker pool / epoch rings.
- No spatial hash (until combat needs craft-craft queries).
- No LOD *scheduler* (ship the enum + seam only).
- No ECS, no command bus / validation framework.
- No component / upgrade system, no wear model, no heat (or other secondary-number) subsystem. v1 = a single fixed component profile where `effective == base`, no wear, no heat. (The *accessor seam* above is required; the *layer behind it* is not.)
- No lever/knob **registry**, no scenario **DSL** (a scenario is a data struct — §4.4 run-config — not a language), no **undo/redo** stack (replay + state-hash already give deterministic time-travel), no **faction/entitlement tables** (the observer *token* is the seam; the faction system behind it is deferred), and **no second mechanism for configurable constraints** (fuel/mass/heat overrides ride the §5.5 effective-param accessor's modifier slot). The lever *invariant* (§4.4) and the address/event/observer *seams* are required; the lever *variants* (spawn logic, fog-of-war filter, time control, parameter-override commands) are not.

---

## 3. Simulation model

### 3.1 Two layers

- **Event layer — deterministic & authoritative.** The discrete things that matter (arrival at destination, fuel-empty, thrust command applied, RL action, reward), recorded as **typed `Event` records** (§4.4). Events are evaluated **at tick boundaries** against quantized state, so they are crisp and reproducible and cannot jitter on sub-step floating-point noise. This is the source of truth for "what happened."
>
> **Clock invariant (shape-now).** Every clock in the system is a fixed-rate integer counter folded into the hash; there is no adaptive `dt`. `Event` is keyed by integer `tick` only. Finer (sub-tick) event resolution — the granularity deeper fidelity (e.g. crew-scale decisions) would eventually need — is a **deliberate future `HASH_FORMAT_VERSION` epoch**, not a silent widening of the event key, so the single global `dt` never hardens into "one global tick == all state." This is recorded now because `Event`/`EventKind` is frozen in the plan-0 contract anchor and widening the key later would invalidate prior-epoch replay hashes.
- **Macro-physics layer — approximate ("close enough for a given second").** Positions/velocities advance under a cheap integrator that must stay bounded and *look* Newtonian. It does **not** need to conserve energy to many digits. The base tick is a fixed `dt` set at init; "close enough for a given second" states the *fidelity* expectation, not necessarily the literal step size. Substepping (§5.3) supplies accuracy *within* a tick regardless of its size, so `dt` is a throughput/event-granularity tuning parameter (resolved in implementation — see §11).

"Close enough" tolerates *inaccuracy*, not *blowup*: ships must not tunnel through or spiral around bodies (see substepping, §5.3).

### 3.2 LOD as a simulation *mode* (forward-design seam; one tier implemented in v1)

LOD is not just a fidelity dial — it selects *how* an entity is simulated:

- **`LodPlayer`** — max fidelity: sub-stepped, fine collision, local origin. (The single tier v1 implements.)
- **`LodNpcInteraction`** — a "real physics grid" good enough that fights/trades resolve fairly. *(Deferred.)*
- **`LodNothing`** — **not ticked.** Entity = `(position, velocity, fuel_rate, depletion_fn)` propagated closed-form; the engine schedules the one future tick where its state changes (fuel dry, arrival, something enters its space) as an event-queue entry. Costs zero per tick until woken. *(Deferred.)*

High-g combat maneuvering (e.g. drones pulling hard turns) happens **only at the higher LOD tiers** (`LodPlayer` / `LodNpcInteraction`), where fine acceleration-aware substepping (§5.3) and the deferred continuous/tactical pilot mode (§5.6) apply — so it never stresses the coarse macro tick.

Validity rule for dormancy: `LodNothing` is valid only where the future is analytically predictable (deep-space straight line, or a Kepler conic). Thrust / interaction / entering a well is itself the **wake trigger**. The **wake↔sleep boundary must round-trip consistently** (a ship slept analytically then woken lands where the closed form predicted). v1 ships the enum + dispatch + wake hook; the analytic mode and multi-tier promotion are deferred without touching callers.

### 3.3 Units — canonical (AU, solar mass, day)

The authoritative core works in canonical units (lengths in AU, masses in M☉, time in days), with `G` folded into the constant so quantities sit near unity. This keeps reward components and observation magnitudes O(1) for a fixed learning rate, widens the determinism margin, and aids debug readability. Conversion to/from SI happens **only at facade boundaries** (e.g. display). Units are **not** the fix for f32 precision (scaling adds no mantissa bits — see §7.2).

### 3.4 Determinism tier — Tier B

**Tier B = same-binary / same-machine bit-reproducible.** This is exactly "seed-reproducible on the same build" — sufficient for RL training/replay on homogeneous workers. **Tier C (cross-platform bit-identical) is explicitly out of scope** (it would cost a permanent `no_std` libm shim). The replay header records build metadata (toolchain, target triple, seed, `dt` bit pattern) so a tier mismatch is *detectable* even though not prevented.

Tier B is achievable cheaply, but only if the cheap floor (§6) is respected — the archived line violated all of it (`thread_rng`, `dt` from Python per-step, JSON-into-HashMap actions) which is why its replay was dead.

---

## 4. Architecture

### 4.1 Crates

- **`jumpgate-core`** — pure-Rust authoritative engine. `#![forbid(unsafe_code)]`. Contains: world state, integrator, ephemeris, autopilot, event layer, command ingestion, RNG, state hashing, replay, and the `StateView` read trait. No graphics, no Python, no I/O on the hot path.
- **`jumpgate-py`** — PyO3 `cdylib` (`abi3-py312`, `numpy`). The **ML/navigator facade**: Gymnasium env wrapper, frame-relative observation extraction into caller buffers, action ingestion, reward. `maturin develop`.

The **viewer** and **game-control** facades are *not* separate v1 crates. Per the panel, v1 = "one read trait + one command DTO + the PyO3 binding." Viewer and game-control are future specializations of those same two surfaces.

### 4.2 The three facades (vocabulary kept; weights differ in v1)

A headless authoritative engine is the sole writer; facades are read-only projections (plus the one shared command-ingestion path for writers).

| Facade | v1 weight | What it is |
|---|---|---|
| **ML / Python** | **Real substance** | obs (frame-relative) + action + reward, Gymnasium-shaped. The real v1 work. |
| **Game-control ("levers")** | Minimal | a narrow command set routed through the *same* ingestion path; interventions logged to the action stream. |
| **Viewer** | Pass-through stub | read-only projection over `StateView`; v1 emits full state with no filter. The visibility / anti-cheat filter is a later spec. (The viewer entitlement filter and the policy's POMDP masking are the *same* operation — designed for, not built.) |

### 4.3 Data model

- **Per-type Struct-of-Arrays stores** (`ShipStore`, `BodyStore`, …; `StationStore`/`ProjectileStore` arrive with later rungs).
- Entities addressed by a **generational slot-map id** (`CraftId { slot, gen }`) — stable across deletion, so a destroyed ship can't be confused with its replacement. (This is the minimal future-proofing for combat's spatial queries; it is *not* the arena allocator on the watch-list.)
- Body positions are **derived from `tick`** via the ephemeris (§5.4), never stored as mutable state.

### 4.4 Lever, event & legibility seams (game-control + viewer + emergence)

These are **shapes, not systems** — each is the trivial version sized to land in v1 without building the layer behind it. They exist because retrofitting any of them touches *every caller of a shared contract* (the command/event/replay format or the `StateView` trait across all three facades), not just an added array. (Sourced from the `systems-as-experience` seam audit.)

- **Command address = a `Target` sum.** `Command { target: Target, kind: CommandKind }` where `Target = Entity(EntityRef) | World | Sim` and `EntityRef = Craft(CraftId) | Body(BodyId)`. v1 `CommandKind` stays at its single `Destination` variant; only the *address* widens. The single ingestion function and the canonical command ordering (§6) are defined over the full `Target` from day one (the sort order across `World`/`Sim`/entity scopes must be total and deterministic). This unlocks future `Spawn` (mints an id, addresses `World`), world/sim interventions, and time-scoped commands — and lets the navigator destination be `Position(Vec3) | Entity(EntityRef)` (fixing the §7.1 body-targeting gap).
- **Typed event records.** `Event { tick, kind: EventKind, … }`, emitted into one tick-stamped stream through one path, symmetric with `Command`. **No reactivity / no bus** — emergent chains already arise from the §3.1 model (an event mutates state; next tick another event's predicate reads it). The seam is *typing + a uniform record*, so collision/weapon-hit/sensor-contact/trade-settled are new `EventKind` variants, not new ad-hoc branches, and the chronicle/viewer/replay read one stream.
- **`StateView` carries intent, not just state.** The read trait exposes recent/queryable `Command` and `Event` history alongside physical state. This is the **legibility surface the thesis needs** — a watcher must read *why* an agent acted (its macro-action decisions are already discrete and recorded per §7.1/§6). Distinct from §7.3 `info` (legibility for the *trainer*); this is legibility for the *watcher*. Shaping `StateView` state-only now forces a trait-break across all three facades later.
- **Observer-parameterized projection.** State→view goes through a single `project(observer) -> View` seam; the presence mask (§7.2) is sourced from one `visible(observer, entity)` predicate returning all-true in v1. This pins the spec's "viewer entitlement filter ≡ policy POMDP masking" claim (§4.2) to *one* location, so fog-of-war / per-faction knowledge / viewer anti-cheat each become "implement `visible()` + pass a real observer," not a simultaneous three-facade signature break.
- **The lever invariant** (extends the §6 determinism contract — every present and future lever obeys it): (1) a lever writes state **only** by emitting a logged `Command` through the one ingestion path (no out-of-band store mutation); (2) the log stores **resolved values, not re-rolled intentions** ("perturb by noise" logs the resolved delta; replay re-feeds it and never re-rolls); (3) no lever mutates `dt` or the tick/ephemeris path; (4) intervention randomness draws from a **named, replayed sub-stream**, never `thread_rng`. Rules (2) and (3) are the two failure modes that killed the archived line.

---

## 5. Physics

### 5.1 Force model (D1 — endorsed)

Massive bodies move on on-rails orbits; mobile craft feel gravity **from** the bodies but exert **no** gravitational force on the bodies or on each other. This is physically justified, not merely convenient: craft-craft mutual gravity is ~8 orders of magnitude below the central term — below the f64 noise floor for any realistic craft mass. It also collapses an O(n²) order-sensitive reduction into *n* independent per-craft updates, making craft embarrassingly parallel at zero determinism cost.

> **Precise statement:** craft exert no *gravitational* force on each other or on bodies. Craft-craft **non-gravitational** queries (collision / weapons / sensors) are explicitly *in scope for later rungs* — the data model (slot-map ids, per-type stores) is shaped for them now.

### 5.2 Integrator (D2 — corrected rationale)

Default integrator is **velocity-Verlet**, behind an **`Integrator` trait** so RK4 can swap in for golden/validation runs and for ballistic bodies where high accuracy matters and no corrective control exists.

**Verlet is chosen for cost/determinism, NOT energy conservation.** The energy-conservation rationale does not hold here: the field is time-dependent (attractors on moving rails → non-autonomous) and thrust is non-conservative, so the symplectic bounded-energy guarantee does not apply. The justification that *does* hold: Verlet performs **1 force evaluation per step vs RK4's 4**, a 4× smaller transcendental / FP-divergence surface — and per the navigator decision (§7.1) the in-engine autopilot corrects trajectory each tick, so integrator fidelity is a *sim-quality* concern, not a trainability one.

> **Implementation note:** true velocity-Verlet in a moving field needs body positions at **both** `t_n` and `t_{n+1}`; a naive single-eval implementation silently degrades to O(dt). **The two-eval form is a tested correctness invariant, not just a code comment:** Plan 2 gates `VelocityVerlet` on a moving-attractor (time-dependent field) convergence-order test — halving `dt` must drop global error ~4× (second order), not ~2× (first order) — because a single-eval Verlet passes an *autonomous*-field coast test false-green (it lands within tolerance of RK4 when no body moves between `t_n` and `t_{n+1}`). Determinism is unaffected either way (one-eval is still bit-reproducible); this guards *accuracy*, which only the moving-field test can see.

### 5.3 Deterministic integer substepping (critical)

A single global timestep cannot span the ~7-orders-of-magnitude acceleration range (a close-in orbit can sweep tens of degrees in a single coarse step and tunnel/spiral). The fix preserves determinism **without** adaptive `dt`:

- The outer **observation/action cadence stays fixed** (`dt`), preserving tick-indexed replay.
- Inside each tick run **N fixed substeps**, where **N is a pure function of quantized local state** — keyed on **total local acceleration magnitude (gravity + thrust)**, not proximity alone. Because N is computed from quantized state, it is **identical on replay**. (Proximity-to-a-body is one driver, but **high-g thrust maneuvers** — e.g. combat drones — need fine substeps even far from any body, so acceleration is the right key.)
- The worst-case substep budget is documented for predictable RL throughput.
- This *is* the LOD substep mechanism (§3.2): higher scrutiny / higher acceleration ⇒ more substeps. High-g combat maneuvering lives only at the higher LOD tiers, where fine substepping is already active.

Gravity uses a **softening length** — `G·M / (r² + ε²)^1.5` — rather than a hard distance cutoff (a hard cutoff is a force discontinuity that itself causes artifacts).

### 5.4 On-rails bodies — precomputed ephemeris table (D-S4)

Body positions come from a **precomputed integer-tick ephemeris table**: transcendentals (Kepler) run **once at init**; the hot path is a pure array lookup. This gives zero per-step FP divergence and enables perfect replay bisection (and is Tier-C-capable if ever needed). Sub-tick body positions required by substepping/Verlet are obtained by **deterministic interpolation** between adjacent tick samples (linear, or cubic-Hermite using stored body velocity) — never by re-running transcendentals on the hot path. Memory: ~2.4 MB per 10k-tick rolling window.

### 5.5 Mass, thrust, fuel (rung 3)

A ship's state separates three things so that upgrades, wear, and secondary subsystems can attach later without restructuring (build-for-but-not-with):

- **Base spec** — the ship's nominal numbers (`base_dry_mass`, `base_max_thrust`, `base_exhaust_velocity`, `base_fuel_capacity`, …).
- **Effective parameters** — what physics actually uses, obtained through an **accessor**: `effective = base × component-modifiers × wear`. In v1 the modifier and wear factors are identity, so `effective == base`. The integrator (§5.2) and autopilot (§5.6) read **effective** values only — they never read `base_*` directly — which is the single seam that lets a component/upgrade/wear layer arrive later.
- **Dynamic state** — `fuel_mass` now; `wear`, `heat`, and similar arrive later as additional SoA arrays updated by their own deterministic tick-stage (pure functions of tick state, consistent with §6).

Acceleration is `F / (effective_dry_mass + fuel_mass)`; firing thrust consumes fuel at a rate set by effective exhaust velocity (variable-mass dynamics; fuel is a finite Δv budget). Fuel reaching zero is an **event** (§3.1). In v1 the navigator macro-action is fixed within a tick, so thrust is held **tick-constant** and **fuel is debited once per tick over `dt`** — which is what keeps the integrator's `accel_at` a pure `Fn(pos, t)` (§5.2). Finer substep-granular mass-bleed is a deferred refinement that does **not** change the `Integrator` seam.

### 5.6 Autopilot (navigator decision)

The policy emits a **macro-action** (destination + optional burn-budget). The in-engine **autopilot** translates it into per-tick thrust commands via a simple deterministic guidance law (v1: thrust toward the destination, throttle/cut on arrival or when the burn-budget/fuel is exhausted). The autopilot is the only thing that applies continuous thrust in v1; continuous/tactical pilot control is a deferred combat-mode facade behind a feature flag.

---

## 6. Determinism & replay contract (the cheap Tier-B floor)

All of the following are v1 requirements — they are what make "seed-reproducible on the same build" actually hold:

- **Time:** `tick: u64` is authoritative; `sim_time = (tick as f64) * dt` is derived only where needed (prefer table-indexing on `tick`). `dt` is **fixed at init**, stored as its `u64` bit pattern in the replay header and folded into the config hash. `dt` is **never** a `step()` argument.
- **RNG:** one master `u64` seed in run config (recorded + hashed). Named sub-streams are derived by seeding **separate `ChaCha8Rng` instances** from the master via a fixed derivation (never by drawing from a shared parent — that couples stream-creation order to draw order). `rand_chacha` is pinned (`StdRng` is not version-stable). `thread_rng` / `from_entropy` are banned in the core.
- **Actions:** a typed `Command { target: Target, kind: CommandKind }` (`Target` per §4.4; v1 `kind` = `Destination`), routed through **one** ingestion function shared by ML and game-control, applied in **canonical, deterministically-totally-ordered** order across all target scopes. Every control-facade intervention is logged into the **same tick-stamped action stream**, and every lever obeys the §4.4 lever invariant.
- **Initial conditions:** the body set, craft count, and per-ship **base-spec** values (§5.5) live in one **run-config struct**, recorded and folded into the config hash alongside seed and `dt` — never embedded as scattered `const`s (so scenario/loadout variants are data, and two distinct scenarios can't silently collide on replay identity).
- **Replay:** replay **re-feeds the recorded action log**; it never re-runs the policy/GPU. This makes the engine immune to PyTorch/GPU nondeterminism (which lives on the non-authoritative policy side).
- **State hash test surface:** a per-tick (or every-N-tick) **FNV-1a** hash over canonical state from `f64::to_bits()` in a fixed field-then-craft order. The hashed state **includes the slot-map allocator cursor** (constant after `reset` in v1). The cursor is hashed to **detect allocator desync**, *not* as a cross-version replay guarantee: replay validity is scoped to `(HASH_FORMAT_VERSION, entity-population schema)`. Appending any hashed field, or adding a new entity kind, bumps `HASH_FORMAT_VERSION` and opens a **new replay epoch** — recordings stay valid under the version they were made on (Tier B is same-binary; golden hashes are refilled per build). v1 hashes a **single flat FNV-1a stream** over the canonical field order (as the committed `hash.rs` anchor does). The *reserved* forward-path — should kind-local appends + entity-granular divergence localization ever be wanted — is to recompose the per-tick hash as a **sorted fold of per-entity sub-digests** (each entity hashed to a u64 via its own field order, then folded in sorted-id order). That is a deliberate `HASH_FORMAT_VERSION` epoch (a Merkle/cached-digest seam), **not built in v1** and not required by Tier B; it is noted here only so the flat encoding is not treated as load-bearing against that upgrade. Replay asserts hash equality and **reports the first differing tick**. Canonical encoding is explicit little-endian with a `MAGIC + FORMAT_VERSION` header. `serde_json`/`pickle` never appear in the hashed path.
- **FP codegen / build determinism:** the Tier-B FP profile (`codegen-units=1`, no fast-math) MUST reach **the binary that actually trains** — the `jumpgate-py` cdylib built by `maturin --release`, **not** the `maturin develop` debug default (`codegen-units=256`, `opt-level=0`) — otherwise golden hashes pinned from the core release build will not match the training cdylib, and Tier B is same-binary only. Build metadata (toolchain, target triple, profile, `dt` bits, seed) is recorded in the replay header so a profile mismatch is *detectable*. All hashed-path reductions (the gravity sum over bodies, any multi-entity fold) iterate in a **fixed documented order** (sorted id / stable index) because f64 add is non-associative; `f64::mul_add`/FMA is **either** banned in the hashed path via `clippy disallowed-methods` **or** mandated everywhere with rationale, so the canonical arithmetic form is not "whatever the first impl emits."
- **Substep quantization is a determinism invariant, not a tuning knob:** `substep_count` maps acceleration to N via an explicit integer/fixed-point quantization with a **stated rounding mode** (the bin *schedule* constants in `SubstepCfg` stay tunable; the quantization *operation* is fixed). The accel used to *bin* need not equal the accel used to *integrate*.
- **Lints:** `clippy` `disallowed-methods` bans `SystemTime`/`Instant::now`/`thread_rng`/env reads in the core.

---

## 7. ML / gym surface

### 7.1 Action

Primary action is the **navigator** macro-action: a destination — `Position(Vec3)` or `Entity(EntityRef)` (a craft *or* a body; §4.4) — plus an optional scalar burn-budget. This collapses the credit-assignment horizon from thousands of thrust microsteps to a handful of option-decisions — the reason the author's precedent works.

### 7.2 Observations — frame-relative (critical, highest-confidence finding)

Absolute solar-system f64 coordinates downcast to f32 collapse to ~10 km noise at 1 AU and training silently fails. **Hard invariant:** all positional/velocity observations are **frame-relative** (ego- or target-centric). The f64 subtraction (`p_craft − p_target`, `v_craft − v_target`) happens **in the Rust core**, and only the **small relative delta** is downcast to f32. A debug assertion guards that no raw absolute coordinate crosses the f32 boundary. (This is the same mechanism as the LOD local-origin / floating-origin trick. Canonical units do **not** substitute for it.)

Observation shape: a fixed **ego block** + a **variable-length entity-set with a presence mask** (v1 may emit zero neighbors, but the schema is versioned so combat's variable-N neighbors don't force a break).

### 7.3 Gym contract

- `step()` returns the **Gymnasium 5-tuple** `(obs, reward, terminated, truncated, info)`; `reset(seed=…)`. (Conflating `terminated`/`truncated` biases value targets on every fixed-length episode.)
- Constructor takes `num_envs` and `num_craft` (both may be 1 in v1); buffers are pre-allocated flat `(N_envs · N_craft · obs_dim)`.
- `step` takes a **per-craft action array** and returns **per-craft `done`** (degenerates to per-env at `num_craft = 1`) — avoids a later contract break. (Per-craft vs per-env reset is left to the multi-craft slice; the per-craft signal is present from v1.)
- Oracle channels (e.g. a future `true_risk` analog) and the reward-component breakdown go through `info`, **never** `obs`.
- **FFI buffers:** the engine **writes into caller-provided buffers** (one memcpy/step — murk's pattern). Raw zero-copy numpy views into simulation memory are **not** exposed: they foreclose the viewer filter's interposition point and create lifetime hazards.

### 7.4 Reward

Computed in f64 in the core, downcast at the boundary. v1 reward supports the fuel-constrained transfer/intercept task (reach destination; penalize fuel/time). Reward-shaping hooks are exposed but a shaping *framework* is not built.

---

## 8. Testing strategy

- **Replay-equivalence (the determinism contract):** run N ticks recording the action log + per-tick hash; replay from the log; assert per-tick hash equality; on failure report the first differing tick. This is the primary correctness test, not "existing tests pass."
- **Physics sanity (bounded, not golden):** a circular orbit stays bounded over many orbits; an eccentric / close-approach trajectory does **not** blow up (substepping); pure-coast energy drift is bounded (sanity check only).
- **Integrator convergence order (moving field):** in a *time-dependent* (moving-attractor) field, `VelocityVerlet` matches `Rk4` to coarse tolerance **and** halving `dt` drops global error ~4× (second order). This is the guard against a single-eval Verlet silently collapsing to O(dt) — which an autonomous-field coast test cannot catch because it passes false-green.
- **Frame-relative invariant:** assert no absolute coordinate crosses the f32 boundary; relative deltas retain expected precision.
- **Autopilot:** a test transfer reaches its destination within its fuel budget deterministically.
- **Gym smoke test (Python):** `reset`/`step` loop; 5-tuple shapes and dtypes; **same seed → identical obs sequence** (Tier-B reproducibility through the binding).

---

## 9. Decisions log

| # | Decision | Choice | Note |
|---|---|---|---|
| D1 | Force model | Central + on-rails; no craft-craft gravity | Endorsed; non-grav craft-craft queries in scope later |
| D2 | Integrator | velocity-Verlet behind a trait | Justified on **cost/determinism**, not energy (energy claim dropped) |
| — | Close-approach | Deterministic integer substepping + softening length | Integrator-independent; = the LOD substep mechanism |
| D3 | Determinism | **Tier B** (same-binary); Tier C out of scope | = "seed-reproducible on same build" |
| D4 | Architecture | Headless engine + `StateView` trait + `Command` DTO + PyO3 ML binding | Viewer = stub; game-control = minimal; vocabulary of 3 facades kept |
| D5 | Scope discipline | Foreclose-nothing / implement-trivially + seam contract + watch-list | Makes "build for but not with" falsifiable |
| — | Units | Canonical AU / M☉ / day | Hygiene + O(1) magnitudes; not an f32 fix |
| — | ML action | **Navigator** macro-action + in-engine autopilot | Continuous pilot deferred to combat-mode facade |
| — | Rails | Precomputed integer-tick ephemeris table | Deterministic sub-tick interpolation; zero hot-path transcendentals |
| — | v1 slice | Through thrusters + fuel + mass (rung 3) | First controllable agent → validates DRL thesis |

**Rejected / corrected:** Verlet-for-energy-conservation (invalid for a non-autonomous + thrust-controlled system); adaptive `dt` (breaks fixed-cadence replay — use substepping); single global `dt` (cannot span the dynamic range); zero-copy FFI views (forecloses viewer filter); plain Vec indices as ids (break on deletion — use slot-map); f64-core catastrophic-cancellation worry (over-rated; f64 is fine at solar-system scale — the real precision issue is the f32 *observation* boundary).

---

## 10. Deferred (later rungs / specs)

Station refuel (rung 4); craft interactions — collision/weapons/sensors, spatial hash (rung 5+); `LodNpcInteraction` and `LodNothing` analytic-dormancy modes + the LOD scheduler; the viewer visibility/anti-cheat filter; continuous/tactical pilot action mode; multi-craft per-craft-reset semantics; canonical-units SI display conversion polish; **ship component / upgrade system, component wear, and heat (and other secondary-number) subsystems** — all riding the effective-parameter accessor seam (§5.5); **game-control lever *variants*** — spawn/despawn logic, the real `visible()` visibility/fog-of-war filter, time control (pause/step/speed, as host-loop pacing only), and runtime parameter-override commands — all riding the §4.4 address/observer/invariant seams; faction/owner tagging (a cheap additive array when needed, per §5.5's rule).

---

## 11. Open implementation tuning items

These do not change the architecture and are resolved during implementation against measured behavior:

- **Base `dt`:** the fixed tick size (throughput vs event-granularity trade-off). "Close enough for a given second" sets the fidelity bar; substepping makes accuracy robust to the choice. Pick a default during the first physics-sanity tests.
- **Substep binning function:** the exact `N = f(total local acceleration magnitude)` schedule (gravity + thrust) and its worst-case budget.
- **Softening length ε** and the ephemeris sub-tick interpolation order (linear vs cubic-Hermite).
- **Reward shaping** weights for the v1 fuel-constrained transfer task.
