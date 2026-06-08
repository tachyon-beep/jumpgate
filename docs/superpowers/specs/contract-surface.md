# Jumpgate cross-task contract surface

Authoritative list of every type, method, and const that flows across task
boundaries. Derived verbatim from the SHARED TYPE CONTRACT. This document is
the parent fix for the per-call-site gaps (SlotMap, ShipStore, View-accessor,
commands_flat): it makes the dependency surface explicit so each task can be
authored against the whole, not task-locally.

## RULE (binding on every task)

The task that PROVIDES a symbol MUST define **every method any downstream task
calls on that symbol**, and that providing task's own `#[cfg(test)]` suite MUST
cover those methods. A method that a later task needs but the providing task
did not implement+test is a contract gap and blocks the providing task's commit.

## Module dependency order (acyclic; enforced by lib.rs decl order)

math -> time -> ids -> types -> config -> hash -> rng -> contract -> stores
     -> ephemeris -> integrator -> ship -> autopilot -> ingest -> events
     -> world -> replay -> (jumpgate-py)

Reconciliation note: `EntityRef`/`NavDest` (in `types`) depend on
`CraftId`/`BodyId` (in `ids`), so `ids` precedes `types`. `config` imports only
`math` + `time`, so it follows `types` without a cycle. `contract` (Integrator,
StateView, Command, Event, Lod re-export point) is defined ONCE and built on
top of `types`/`ids`/`stores`; concrete impls (`integrator`, `world`) import
from `contract` and never redeclare a same-shaped trait.

Note on deviation from convention #2 literal order: convention #2 lists
`math -> time -> types -> ids -> ...`, but `EntityRef`/`NavDest` in `types.rs`
reference `CraftId`/`BodyId` from `ids.rs`, making `types-before-ids` uncompilable.
The only acyclic resolution is `ids` BEFORE `types`. This task records the
reconciliation explicitly here and in the module declaration order in `lib.rs`.

## Symbol -> providing task -> consumers

| Symbol | Provided by | Key methods/fields downstream relies on | Consumers |
|---|---|---|---|
| `Vec3` | Task 2 (`math`) | `new`,`ZERO`,`add`,`sub`,`scale`,`dot`,`length`,`length_sq`,`normalize_or_zero`,`to_bits` | every physics/config/hash task |
| `G_CANONICAL` | Task 2 (`math`) | const | integrator, ephemeris |
| `Tick`,`Dt`,`sim_time` | Task 3 (`time`) | `Tick(u64)`; `Dt::new/get/bits`; `sim_time(Tick,Dt)` | config, hash, world, ephemeris, events, replay |
| `CraftId`,`BodyId` | Task 3 (`ids`) | tuple `{slot,generation}`; `Ord`/`Hash` derives | types, stores, contract, world, hash, py |
| `SlotMap<T>` | Task 3 (`ids`) | `new`,`len`,`is_empty`,`cursor`,`insert`,`get`,`remove`,`gen_of`,`dense_index`,`id_at` | stores, world; `cursor()` is HASHED state |
| `Lod` | Task 3 (`types`) | `Player`/`NpcInteraction`/`Nothing` | stores (`lod:Vec<Lod>`), contract, world dispatch |
| `EntityRef`,`Target`,`NavDest`,`CommandKind` | Task 3 (`types`) | enum variants | contract (`Command`,`Event`), stores (`NavState`), ingest |
| `BaseSpec`,`OrbitalElements`,`BodyInit`,`CraftInit`,`SubstepCfg`,`RunConfig`,`ConfigHash` | Task 3 (`config`) | `RunConfig::config_hash`; field access | world::reset, ephemeris, replay, py |
| `FnvHasher`,`HASH_MAGIC`,`HASH_FORMAT_VERSION`,`HASH_FIELD_ORDER` | Task 3 (`hash`) | `new`,`write_u64`,`finish`; consts; canonical order | Task 13 `state_hash`, replay, world |
| `RngStreams`,`RngStream` | Task 5 (`rng`) | `from_master`,`stream` | world |
| `Command`,`Event`,`EventKind`,`command_sort_key`,`Integrator`,`StateView` | Task 6 (`contract`) | one definition each | integrator, ingest, events, world, py |
| `NavState`,`ShipStore`,`BodyStore`,`Effective`,`effective_params` | Task 4 (`stores`) | field access; `effective_params(&BaseSpec)` | world, integrator, autopilot, ship |
| `World`,`Observer`,`FullObserver`,`View`,`project` | Task 12 (`world`) | `World::{reset,step,project}`; `impl StateView for World`; `View` accessors; `Observer::visible` | ingest, events, replay, py |

## Hash-ownership invariant

There are TWO distinct FNV-1a hashes, never sharing state or magic:
- **config hash** (`RunConfig::config_hash`, Task 3 `config.rs`): hashes
  immutable initial conditions once. Uses a LOCAL fold with a `"CONFIG_1"` tag.
- **per-tick STATE hash** (`state_hash`, Task 13 `hash.rs`): hashes evolving
  world state each tick via the shared `FnvHasher` seeded with `HASH_MAGIC`.
  Its canonical field order is `HASH_FIELD_ORDER` (Task 3, this task), and ANY
  task that adds a hashed field MUST append to `HASH_FIELD_ORDER`, bump
  `HASH_FORMAT_VERSION`, and update the golden-hash test.

## Canonical-arithmetic invariant (spec §6 FMA decision)

`f64::mul_add` / `f32::mul_add` are **banned in the hashed path** via
`clippy.toml` `disallowed-methods` (the project resolved spec §6's FMA choice to
the BAN side, not the mandate side). All hashed-path reductions (the gravity sum
over bodies, any multi-entity fold) are written as explicit `a * b + c` and
iterate in a **fixed documented order** (sorted id / stable index) because f64
add is non-associative — so the canonical arithmetic form is "whatever is
written", never implementation-dependent fused codegen.
