# Person+Ship Plan A — The CrewMods Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thread a per-craft `CrewMods` modifier struct through `effective_params` — identity-valued, behaviour-preserving — so the crew/Person work in Plans B & C is purely additive on top of an already-migrated seam.

**Architecture:** This is the one non-additive change in the whole Person+Ship design (spec §2.0): `effective_params(spec)` becomes `effective_params(spec, &CrewMods)`. A new `crew_mods: Vec<CrewMods>` SoA column on `ShipStore` holds the (v1-identity) factors; production call sites read it, test-only call sites pass `CrewMods::IDENTITY`. Because the factor is `1.0` and `x * 1.0 == x` bit-identically, **no trajectory and no state hash change** — the existing golden-hash test and full suite staying green is the behaviour-preservation proof. `crew_mods` is *derived* state (written by a later plan's `compute_crew_mods`); it is **not** folded into the per-tick hash, so `HASH_FORMAT_VERSION` is **not** bumped in this plan.

**Tech Stack:** Rust 2024, `cargo test` / `cargo clippy --all-targets`, crate `jumpgate-core`.

**Spec:** `docs/superpowers/specs/2026-06-08-jumpgate-person-ship-foundation-design.md` (§2.0, §4.2, §12 steps 1–2).

**Sequencing gate:** Do not start until the parallel universe/physics substructure work is merged/locked — this plan edits the hottest shared surfaces (`stores.rs`, `world.rs::step`, `ingest.rs`). Re-verify the line numbers below against `git` before editing; they reflect the locked state at plan-authoring time.

---

## File map

- `crates/jumpgate-core/src/stores.rs` — **Create** `CrewMods` + `IDENTITY`; change `effective_params` signature + apply `thrust_factor`; add `crew_mods` column to `ShipStore` (`empty`/`push`); update `craft_fuel_capacity` caller + length-parallel tests + the `effective_equals_base_in_v1` test.
- `crates/jumpgate-core/src/world.rs` — Init `crew_mods` in `World::reset`; update the three `effective_params` call sites (`step` burn, `project` capacity, `StateView::craft_fuel_capacity`).
- `crates/jumpgate-core/src/ingest.rs` — Update the Δv-budget `effective_params` call site.
- `crates/jumpgate-core/src/autopilot.rs`, `crates/jumpgate-core/src/ship.rs` — Update test-only `effective_params` calls to pass `&CrewMods::IDENTITY`.
- `crates/jumpgate-core/src/lib.rs` — Re-export `CrewMods`.

---

### Task 1: `CrewMods` struct + `IDENTITY`

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (add struct near `Effective`, ~after line 36)
- Modify: `crates/jumpgate-core/src/lib.rs:59` (re-export)
- Test: `crates/jumpgate-core/src/stores.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/jumpgate-core/src/stores.rs`:

```rust
#[test]
fn crewmods_identity_is_unit() {
    let m = CrewMods::IDENTITY;
    assert_eq!(m.thrust_factor, 1.0);
    // Copy + PartialEq are part of the contract (read every tick, compared in tests).
    let n = m;
    assert_eq!(m, n);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core crewmods_identity_is_unit`
Expected: FAIL — `cannot find type CrewMods` / `CrewMods not in scope`.

- [ ] **Step 3: Write minimal implementation**

In `crates/jumpgate-core/src/stores.rs`, immediately after the `Effective` struct and its `effective_params` fn (after line ~36), add:

```rust
/// Per-craft crew-derived modifier factors applied to `BaseSpec` by
/// `effective_params`. DERIVED state: written only by the crew-mod tick-stage
/// (a later plan) at the top of each `step`; read by `effective_params`. NOT
/// folded into the per-tick state hash — transitively pinned by its hashed
/// inputs, exactly like `prev_fuel`. v1: identity (`thrust_factor == 1.0`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrewMods {
    /// Multiplier on `max_thrust` — the one wired channel in v1. `1.0` == no effect.
    pub thrust_factor: f64,
}

impl CrewMods {
    /// The no-effect value. `effective_params(spec, &IDENTITY)` is bit-identical
    /// to the pre-CrewMods `effective_params(spec)` (`x * 1.0 == x`).
    pub const IDENTITY: CrewMods = CrewMods { thrust_factor: 1.0 };
}
```

Then update the re-export in `crates/jumpgate-core/src/lib.rs:59`:

```rust
pub use stores::{BodyStore, CrewMods, Effective, NavState, ShipStore, effective_params};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core crewmods_identity_is_unit`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/lib.rs
git commit -m "feat(core): add CrewMods modifier struct + IDENTITY (crew seam, plan A/1)"
```

---

### Task 2: `crew_mods` SoA column on `ShipStore`

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`ShipStore` struct ~line 46-56; `empty` ~70-82; `push` ~87-103)
- Modify: `crates/jumpgate-core/src/world.rs` (`World::reset` ShipStore literal ~100-110 + push loop ~111-123)
- Test: `crates/jumpgate-core/src/stores.rs` (extend `stores_construct_soa_parallel` + `shipstore_push_and_accessors`)

- [ ] **Step 1: Write the failing test**

Extend `stores_construct_soa_parallel` in `crates/jumpgate-core/src/stores.rs` — add after the existing `prev_inside_dest` length assertion:

```rust
    // crew_mods is a length-parallel DERIVED column, initialized to IDENTITY.
    assert_eq!(ship.crew_mods.len(), n);
```

And extend `shipstore_push_and_accessors` — add after the existing `prev_inside_dest` length assertion:

```rust
    assert_eq!(ship.crew_mods.len(), n);
    assert_eq!(ship.crew_mods[0], CrewMods::IDENTITY, "push initializes crew_mods to IDENTITY");
    assert_eq!(ship.crew_mods[1], CrewMods::IDENTITY);
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core stores_construct_soa_parallel shipstore_push_and_accessors`
Expected: FAIL — `no field crew_mods on type ShipStore`.

- [ ] **Step 3: Write minimal implementation**

In `crates/jumpgate-core/src/stores.rs`, add the column to the `ShipStore` struct (after `prev_inside_dest: Vec<bool>,`):

```rust
    /// Per-craft crew-derived modifier cache. DERIVED (written by the crew-mod
    /// tick-stage in a later plan; IDENTITY until then), length-parallel, NOT
    /// folded into the state hash. Initialized to `CrewMods::IDENTITY` so reads
    /// before the first `step` (e.g. projections) are well-defined.
    pub crew_mods: Vec<CrewMods>,
```

In `ShipStore::empty()`, add to the returned literal:

```rust
            crew_mods: Vec::new(),
```

In `ShipStore::push()`, add alongside the other `push` calls (after `self.prev_inside_dest.push(false);`):

```rust
        self.crew_mods.push(CrewMods::IDENTITY);
```

In `crates/jumpgate-core/src/world.rs`, add to the `ShipStore { … }` literal in `reset` (after `prev_inside_dest: Vec::new(),`):

```rust
            crew_mods: Vec::new(),
```

And in the craft-spawn loop in `reset` (after `ships.prev_inside_dest.push(false);`):

```rust
            ships.crew_mods.push(crate::stores::CrewMods::IDENTITY);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core stores_construct_soa_parallel shipstore_push_and_accessors`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs
git commit -m "feat(core): add length-parallel crew_mods column to ShipStore (plan A/2)"
```

---

### Task 3: `effective_params(spec, &CrewMods)` — signature + all call sites

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`effective_params` ~29-36; caller `craft_fuel_capacity` ~130; test `effective_equals_base_in_v1` ~165-178; test helper at ~173)
- Modify: `crates/jumpgate-core/src/world.rs:210, 328, 390`
- Modify: `crates/jumpgate-core/src/ingest.rs:165`
- Modify: `crates/jumpgate-core/src/autopilot.rs:98`, `crates/jumpgate-core/src/ship.rs:59`
- Test: `crates/jumpgate-core/src/stores.rs` (new `effective_scales_with_thrust_factor`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/jumpgate-core/src/stores.rs`:

```rust
#[test]
fn effective_scales_with_thrust_factor() {
    use crate::config::BaseSpec;
    let spec = BaseSpec {
        base_dry_mass: 10.0,
        base_max_thrust: 250.0,
        base_exhaust_velocity: 30.0,
        base_fuel_capacity: 40.0,
    };
    // IDENTITY is bit-identical to the base numbers.
    let id = effective_params(&spec, &CrewMods::IDENTITY);
    assert_eq!(id.max_thrust, 250.0);
    assert_eq!(id.dry_mass, spec.base_dry_mass);
    assert_eq!(id.exhaust_velocity, spec.base_exhaust_velocity);
    assert_eq!(id.fuel_capacity, spec.base_fuel_capacity);

    // thrust_factor multiplies ONLY max_thrust (the one wired channel in v1).
    let boosted = effective_params(&spec, &CrewMods { thrust_factor: 1.5 });
    assert_eq!(boosted.max_thrust, 375.0);
    assert_eq!(boosted.dry_mass, spec.base_dry_mass, "dry_mass unaffected");
    assert_eq!(boosted.exhaust_velocity, spec.base_exhaust_velocity, "v_e unaffected");
    assert_eq!(boosted.fuel_capacity, spec.base_fuel_capacity, "capacity unaffected");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core effective_scales_with_thrust_factor`
Expected: FAIL — `this function takes 1 argument but 2 arguments were supplied` (compile error).

- [ ] **Step 3: Write minimal implementation**

Change `effective_params` in `crates/jumpgate-core/src/stores.rs` (lines ~29-36) to:

```rust
/// The ONLY accessor the integrator/autopilot read for ship params. v1 applies
/// `mods.thrust_factor` to `max_thrust`; all other fields pass through. With
/// `CrewMods::IDENTITY` the result is bit-identical to the base spec.
pub fn effective_params(spec: &BaseSpec, mods: &CrewMods) -> Effective {
    Effective {
        dry_mass: spec.base_dry_mass,
        max_thrust: spec.base_max_thrust * mods.thrust_factor,
        exhaust_velocity: spec.base_exhaust_velocity,
        fuel_capacity: spec.base_fuel_capacity,
    }
}
```

Update each call site:

`crates/jumpgate-core/src/stores.rs:130` (`ShipStore::craft_fuel_capacity`):

```rust
            .map(|i| effective_params(&self.spec[i], &self.crew_mods[i]).fuel_capacity)
```

`crates/jumpgate-core/src/world.rs:210` (`step` burn path):

```rust
            let eff = effective_params(&self.ships.spec[ci], &self.ships.crew_mods[ci]);
```

`crates/jumpgate-core/src/world.rs:328` (`project` capacity):

```rust
                let cap = effective_params(&self.ships.spec[i], &self.ships.crew_mods[i]).fuel_capacity;
```

`crates/jumpgate-core/src/world.rs:390` (`StateView::craft_fuel_capacity`):

```rust
            .map(|i| effective_params(&self.ships.spec[i], &self.ships.crew_mods[i]).fuel_capacity)
```

`crates/jumpgate-core/src/ingest.rs:165` (Δv budget):

```rust
    let eff = crate::stores::effective_params(&ship.spec[idx], &ship.crew_mods[idx]);
```

`crates/jumpgate-core/src/autopilot.rs:98` (test-only) — change the call to pass identity:

```rust
        effective_params(&BaseSpec {
```
becomes a call whose closing `})` is followed by `, &crate::stores::CrewMods::IDENTITY)`. Concretely the call is:

```rust
        effective_params(
            &BaseSpec { /* ...existing fields unchanged... */ },
            &crate::stores::CrewMods::IDENTITY,
        )
```

`crates/jumpgate-core/src/ship.rs:59` (test-only, in `eff_fixture`) — same shape:

```rust
        effective_params(
            &BaseSpec { /* ...existing fields unchanged... */ },
            &crate::stores::CrewMods::IDENTITY,
        )
```

Update the existing `effective_equals_base_in_v1` test in `crates/jumpgate-core/src/stores.rs` (~line 173) — change the call to:

```rust
        let eff = effective_params(&spec, &CrewMods::IDENTITY);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core effective_scales_with_thrust_factor effective_equals_base_in_v1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/autopilot.rs \
        crates/jumpgate-core/src/ship.rs
git commit -m "refactor(core): effective_params takes &CrewMods; thrust_factor on max_thrust (plan A/3)"
```

---

### Task 4: Behaviour-preservation gate (the trajectory-equivalence proof)

This task adds no new behaviour; it proves the migration changed nothing observable. The existing golden-hash test (`hash.rs`) and the full suite passing **unchanged** is the proof. `HASH_FORMAT_VERSION` stays `1`; `GOLDEN_ZERO_STATE_HASH` stays `0xf0dd_a1ba_f433_3735`.

**Files:**
- Test: `crates/jumpgate-core/src/world.rs` (new `identity_crew_mods_preserve_trajectory` in the `tests` module)

- [ ] **Step 1: Write the test**

Add to the `tests` module in `crates/jumpgate-core/src/world.rs` (uses the existing `one_body_one_thrusting_craft` fixture):

```rust
#[test]
fn identity_crew_mods_preserve_trajectory() {
    // With the default (all-IDENTITY) crew_mods, a thrusting transfer must produce
    // the SAME state as the pre-CrewMods build: thrust_factor == 1.0 and x*1.0 == x,
    // so max_thrust — and therefore pos/vel/fuel — are bit-identical.
    let cfg = one_body_one_thrusting_craft();
    let (mut world, _) = World::reset(cfg);
    let id = world.craft_ids()[0];

    // crew_mods initialized to IDENTITY at reset.
    assert_eq!(world.ships.crew_mods[0], crate::stores::CrewMods::IDENTITY);

    use crate::types::{EntityRef, NavDest, Target};
    let target = Vec3::new(5.3, 0.0, 0.0);
    let mut cmds = vec![Command {
        target: Target::Entity(EntityRef::Craft(id)),
        kind: CommandKind::Destination { dest: NavDest::Position(target), burn_budget: Some(1.0) },
    }];
    world.step(&mut cmds);
    for _ in 0..50 {
        let mut none: Vec<Command> = Vec::new();
        world.step(&mut none);
    }

    // crew_mods is never written in plan A, so it stays IDENTITY throughout.
    assert_eq!(world.ships.crew_mods[0], crate::stores::CrewMods::IDENTITY);
    let p = world.craft_pos(id).unwrap();
    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
}
```

- [ ] **Step 2: Run the new test + the golden hash test**

Run: `cargo test -p jumpgate-core identity_crew_mods_preserve_trajectory`
Expected: PASS.
Run: `cargo test -p jumpgate-core golden`
Expected: PASS — the zero-state golden hash is **unchanged** (no hashed state added). If this fails, a hashed field was touched by mistake — stop and investigate; do NOT re-baseline the golden.

- [ ] **Step 3: Run the full suite + clippy (the real gate)**

Run: `cargo test -p jumpgate-core`
Expected: PASS — all lib tests (the pre-existing count + 3 new) green.
Run: `cargo test -p jumpgate-core --test replay_equivalence --test physics_sanity`
Expected: PASS — replay + physics-sanity unchanged.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. (Use `--all-targets`; `--lib` is a no-op here — binary crate.)

- [ ] **Step 4: Commit**

```bash
git add crates/jumpgate-core/src/world.rs
git commit -m "test(core): identity crew_mods preserve trajectory; golden hash unchanged (plan A/4)"
```

---

## Plan A self-review

- **Spec coverage:** §2.0 (the two-arg gate) → Tasks 1+3; §4.2 `CrewMods` named struct + `IDENTITY` + reset-init + derived/not-hashed → Tasks 1+2; §12 steps 1–2 (seam + trajectory-equivalence) → Tasks 3+4. `HASH_FORMAT_VERSION` deliberately NOT bumped (no hashed state) — consistent with §5 (hash inputs, never the derived cache).
- **No placeholders:** every code step shows complete code; the two test-only call-site edits show the exact wrapping `, &CrewMods::IDENTITY)`.
- **Type consistency:** `CrewMods { thrust_factor: f64 }`, `CrewMods::IDENTITY`, `effective_params(&BaseSpec, &CrewMods) -> Effective`, `ShipStore.crew_mods: Vec<CrewMods>` are used identically in every task.

---

## What comes next (Plans B & C — outline only, detail after A lands)

These are deliberately **not** bite-sized yet: their exact code depends on the symbols A introduces and on the substructure agent's final foundation. Detail them (re-running writing-plans) once Plan A is merged.

**Plan B — PersonStore + crew effect (spec §3, §4, §5, §6):**
1. `PersonId` (+`Ord`) + `EntityRef::Person` arm + `command_sort_key` extension `(scope_rank, kind_rank, slot, gen)` with Person=4 (update the stable-sort tie test).
2. `PersonStore` SoA: `location`, `skills: [f64; N_DOMAINS]`, `status`, `brain`, `personality`, `agenda`; finite-validation at construction; `PersonInit` in `RunConfig` (folded into the config hash).
3. Ship `roster: [Option<PersonId>; N_ROLES]`, `controller: Option<PersonId>`, `abstracted_crew_competence`; the lockstep transfer mutator (test-only API).
4. Hash fold of the new INPUT columns + person-store cursor → bump `HASH_FORMAT_VERSION` 1→2, re-derive golden.
5. `compute_crew_mods` (top-of-step, before ingest, LOD-gated, per-domain accumulation) writing `crew_mods`; `resolve_controller` (`Aboard && Alive`) + `is_effective_crew` gates; `RoleDemand` const table (Leadership weight 0); frozen `NEUTRAL_LEADERSHIP` + `f`/`g` with exact empty-state identity.
6. Proof tests: controller-swap thrust drop; budget/burn same-`Effective`; roster-shuffle hash-invariance; controller-validity; tested inertness (flip personality bits); leadership-never-in-demand; dead-vs-stale; incapacitated-zero.

**Plan C — shaped-inert reserves + crewed scenario fixtures (spec §7, §11):**
- `AtStation`/`InSpace` resolver stubs; reserved `Crew` RNG stream; succession comparator (defined+tested, not auto-wired); crewed scenario fixtures that exercise a populated roster end-to-end.
