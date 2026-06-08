# Person+Ship Plan A — The EffectiveMods Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thread a per-craft `EffectiveMods` modifier bundle through `effective_params` — identity-valued, behaviour-preserving — so the crew/Person work in Plans B & C (and a future `wear` system) is purely additive on top of an already-migrated seam.

**Architecture:** This is the one non-additive change in the whole Person+Ship design (spec §2.0): `effective_params(spec)` becomes `effective_params(spec, &EffectiveMods)`. The argument is a **general modifier bundle**, not a crew-only struct — the founding intent is `Effective = base × component-mods × wear` (three factor sources), so `EffectiveMods` reserves room for wear/component factors and `effective_params` never changes signature again (cross-spec review, architecture-critic: a `CrewMods`-named arg would force a *second* signature break when wear lands — the exact refactor this gate exists to avoid). A new `mods: Vec<EffectiveMods>` SoA column on `CraftStore` holds the (v1-identity) factors; production call sites read it, test-only call sites pass `EffectiveMods::IDENTITY`. Because the factor is `1.0` and `x * 1.0 == x` bit-identically (verified across subnormals/signed-zero; NaN excluded by finite-validation), **no trajectory and no state hash change** — the existing goldens + full suite staying green is the behaviour-preservation proof. `mods` is *derived* state (later written by `compute_crew_mods`); it is **not** folded into the per-tick hash, so `HASH_FORMAT_VERSION` is **not** bumped in this plan.

**Tech Stack:** Rust 2024, `cargo test` / `cargo clippy --all-targets`, crate `jumpgate-core`.

**Spec:** `docs/superpowers/specs/2026-06-08-jumpgate-person-ship-foundation-design.md` (§2.0, §4.2, §12 steps 1–2).

**SEQUENCING (revised after cross-spec review):** This plan lands **third**, after:
1. **The prelude** (`2026-06-09-jumpgate-prelude-craftstore-confighash.md`) — `ShipStore → CraftStore` rename + `config_hash` exhaustive destructure. This plan therefore targets `CraftStore` (NOT `ShipStore`).
2. **The Guidance-Parameter spec** (`2026-06-09-guidance-parameter-system-design.md`) — it changes `autopilot_command` (adds `guidance: &GuidanceParams, dt`), re-derives the cruise-axis *trajectory* goldens, and adds the `World::reset` resolvability guard. Landing it first means this plan's trajectory-equivalence proof is measured against a *settled* baseline. **Re-read the post-guidance signatures of `effective_params` (still `&BaseSpec`), `autopilot_command`, and `World::reset` (guidance makes it return `Result`) before editing — line numbers below reflect the pre-guidance locked state.**

**Capability vs policy (the seam split, confirmed by review):** `EffectiveMods` carries **capability** (what the craft *can* do — `max_thrust` after crew/wear scaling); it goes through `effective_params` because the integrator's burn reads `Effective`. `GuidanceParams` carries **policy** (how/when to burn — dt- and arrival-dependent) and stays in `autopilot_command`. Orthogonal; both correct.

---

## File map

- `crates/jumpgate-core/src/stores.rs` — **Create** `EffectiveMods` + `IDENTITY`; change `effective_params` signature + apply `thrust_factor`; add `mods` column to `CraftStore` (`empty`/`push`); update `craft_fuel_capacity` caller + length-parallel tests + the `effective_equals_base_in_v1` test.
- `crates/jumpgate-core/src/world.rs` — Init `mods` in `World::reset`; update the three `effective_params` call sites (`step` burn, `project` capacity, `StateView::craft_fuel_capacity`).
- `crates/jumpgate-core/src/ingest.rs` — Update the Δv-budget `effective_params` call site.
- `crates/jumpgate-core/src/autopilot.rs`, `crates/jumpgate-core/src/ship.rs` — Update test-only `effective_params` calls to pass `&EffectiveMods::IDENTITY`.
- `crates/jumpgate-core/src/lib.rs` — Re-export `EffectiveMods`.

---

### Task 1: `EffectiveMods` bundle + `IDENTITY`

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (add struct near `Effective`, ~after line 36)
- Modify: `crates/jumpgate-core/src/lib.rs` (re-export)
- Test: `crates/jumpgate-core/src/stores.rs` (inline `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `crates/jumpgate-core/src/stores.rs`:

```rust
#[test]
fn effective_mods_identity_is_unit() {
    let m = EffectiveMods::IDENTITY;
    assert_eq!(m.thrust_factor, 1.0);
    let n = m; // Copy + PartialEq are part of the contract (read every tick).
    assert_eq!(m, n);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core effective_mods_identity_is_unit`
Expected: FAIL — `cannot find type EffectiveMods`.

- [ ] **Step 3: Write minimal implementation**

In `crates/jumpgate-core/src/stores.rs`, immediately after `Effective` + `effective_params` (~after line 36), add:

```rust
/// Per-craft EFFECTIVE-parameter modifier bundle — the single combined multiply
/// applied to `BaseSpec` by `effective_params`. This is the `× component-mods ×
/// wear` half of the founding `Effective = base × component-mods × wear` intent,
/// PRE-REDUCED into one struct so `effective_params` never changes signature
/// again as new factor sources land.
///
/// v1 carries only `thrust_factor` (the crew-contributed engine multiplier,
/// written later by `compute_crew_mods`). Future wear/component factors fold into
/// the SAME bundle (e.g. `compute_wear`), never a new `effective_params` arg.
///
/// DERIVED state: written by the crew-mod / wear tick-stages; read by
/// `effective_params`. NOT folded into the per-tick state hash — transitively
/// pinned by its hashed inputs, exactly like `prev_fuel`. v1: identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectiveMods {
    /// Multiplier on `max_thrust` — the one wired channel in v1. `1.0` == no effect.
    pub thrust_factor: f64,
    // Reserved (default 1.0 in IDENTITY) for wear/component factors — adding a
    // field here is additive; it is NEVER a change to `effective_params`'s signature.
}

impl EffectiveMods {
    /// The no-effect value. `effective_params(spec, &IDENTITY)` is bit-identical
    /// to the pre-bundle `effective_params(spec)` (`x * 1.0 == x` for finite f64).
    pub const IDENTITY: EffectiveMods = EffectiveMods { thrust_factor: 1.0 };
}
```

Update the re-export in `crates/jumpgate-core/src/lib.rs` (the `pub use stores::{…}` line):

```rust
pub use stores::{BodyStore, CraftStore, EffectiveMods, Effective, NavState, effective_params};
```

(Adjust to the exact current `pub use` set — `CraftStore` exists post-prelude.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core effective_mods_identity_is_unit`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/lib.rs
git commit -m "feat(core): add EffectiveMods modifier bundle + IDENTITY (capability seam, plan A/1)"
```

---

### Task 2: `mods` SoA column on `CraftStore`

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`CraftStore` struct; `empty`; `push`)
- Modify: `crates/jumpgate-core/src/world.rs` (`World::reset` `CraftStore` literal + push loop)
- Test: `crates/jumpgate-core/src/stores.rs` (extend `stores_construct_soa_parallel` + `shipstore_push_and_accessors`)

- [ ] **Step 1: Write the failing test**

Extend `stores_construct_soa_parallel` in `crates/jumpgate-core/src/stores.rs` — after the existing `prev_inside_dest` length assertion:

```rust
    // mods is a length-parallel DERIVED column, initialized to IDENTITY.
    assert_eq!(ship.mods.len(), n);
```

And extend `shipstore_push_and_accessors` — after the existing `prev_inside_dest` length assertion:

```rust
    assert_eq!(ship.mods.len(), n);
    assert_eq!(ship.mods[0], EffectiveMods::IDENTITY, "push initializes mods to IDENTITY");
    assert_eq!(ship.mods[1], EffectiveMods::IDENTITY);
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core stores_construct_soa_parallel shipstore_push_and_accessors`
Expected: FAIL — `no field mods on type CraftStore`.

- [ ] **Step 3: Write minimal implementation**

In `crates/jumpgate-core/src/stores.rs`, add the column to the `CraftStore` struct (after `prev_inside_dest: Vec<bool>,`):

```rust
    /// Per-craft EFFECTIVE-modifier cache. DERIVED (written by the crew-mod / wear
    /// tick-stages; IDENTITY until then), length-parallel, NOT hashed. Initialized
    /// to `EffectiveMods::IDENTITY` so reads before the first `step` (projections)
    /// are well-defined. INVARIANT: `mods` is a pure function of state that is
    /// either constant (v1) or folded into HASH_FIELD_ORDER (Plan B); it must NEVER
    /// depend on an unhashed runtime-mutable input.
    pub mods: Vec<EffectiveMods>,
```

In `CraftStore::empty()`, add to the returned literal:

```rust
            mods: Vec::new(),
```

In `CraftStore::push()`, after `self.prev_inside_dest.push(false);`:

```rust
        self.mods.push(EffectiveMods::IDENTITY);
```

In `crates/jumpgate-core/src/world.rs`, add to the `CraftStore { … }` literal in `reset` (after `prev_inside_dest: Vec::new(),`):

```rust
            mods: Vec::new(),
```

And in the craft-spawn loop in `reset` (after `ships.prev_inside_dest.push(false);`):

```rust
            ships.mods.push(crate::stores::EffectiveMods::IDENTITY);
```

> **Forward-debt note (record, do not implement here):** Plan B's `compute_crew_mods` must populate `mods` **at reset, before the Guidance resolvability guard runs** (the guard reads effective `max_thrust`; once `mods` is non-identity it must read the modified value, and reset runs before any step). In Plan A `mods` is identity, so reset-population is trivially `IDENTITY` and the guard is unaffected — but the ordering decision is fixed now.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core stores_construct_soa_parallel shipstore_push_and_accessors`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs
git commit -m "feat(core): add length-parallel mods column to CraftStore (plan A/2)"
```

---

### Task 3: `effective_params(spec, &EffectiveMods)` — signature + all call sites

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (`effective_params`; caller `craft_fuel_capacity`; test `effective_equals_base_in_v1`)
- Modify: `crates/jumpgate-core/src/world.rs` (burn ~210, project capacity ~328, StateView capacity ~390)
- Modify: `crates/jumpgate-core/src/ingest.rs` (Δv budget ~165)
- Modify: `crates/jumpgate-core/src/autopilot.rs`, `crates/jumpgate-core/src/ship.rs` (test fixtures)
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
    let id = effective_params(&spec, &EffectiveMods::IDENTITY);
    assert_eq!(id.max_thrust, 250.0);
    assert_eq!(id.dry_mass, spec.base_dry_mass);
    assert_eq!(id.exhaust_velocity, spec.base_exhaust_velocity);
    assert_eq!(id.fuel_capacity, spec.base_fuel_capacity);

    // thrust_factor multiplies ONLY max_thrust (the one wired channel in v1).
    let boosted = effective_params(&spec, &EffectiveMods { thrust_factor: 1.5 });
    assert_eq!(boosted.max_thrust, 375.0);
    assert_eq!(boosted.dry_mass, spec.base_dry_mass, "dry_mass unaffected");
    assert_eq!(boosted.exhaust_velocity, spec.base_exhaust_velocity, "v_e unaffected");
    assert_eq!(boosted.fuel_capacity, spec.base_fuel_capacity, "capacity unaffected");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p jumpgate-core effective_scales_with_thrust_factor`
Expected: FAIL — `this function takes 1 argument but 2 arguments were supplied`.

- [ ] **Step 3: Write minimal implementation**

Change `effective_params` in `crates/jumpgate-core/src/stores.rs` to:

```rust
/// The ONLY accessor the integrator/autopilot read for craft params. v1 applies
/// `mods.thrust_factor` to `max_thrust`; all other fields pass through. With
/// `EffectiveMods::IDENTITY` the result is bit-identical to the base spec.
/// Use a plain `*` (no FMA contraction) so the multiply is a single rounding.
pub fn effective_params(spec: &BaseSpec, mods: &EffectiveMods) -> Effective {
    Effective {
        dry_mass: spec.base_dry_mass,
        max_thrust: spec.base_max_thrust * mods.thrust_factor,
        exhaust_velocity: spec.base_exhaust_velocity,
        fuel_capacity: spec.base_fuel_capacity,
    }
}
```

Update each call site:

`crates/jumpgate-core/src/stores.rs` (`CraftStore::craft_fuel_capacity`):

```rust
            .map(|i| effective_params(&self.spec[i], &self.mods[i]).fuel_capacity)
```

`crates/jumpgate-core/src/world.rs` (`step` burn path, ~210):

```rust
            let eff = effective_params(&self.ships.spec[ci], &self.ships.mods[ci]);
```

`crates/jumpgate-core/src/world.rs` (`project` capacity, ~328):

```rust
                let cap = effective_params(&self.ships.spec[i], &self.ships.mods[i]).fuel_capacity;
```

`crates/jumpgate-core/src/world.rs` (`StateView::craft_fuel_capacity`, ~390):

```rust
            .map(|i| effective_params(&self.ships.spec[i], &self.ships.mods[i]).fuel_capacity)
```

`crates/jumpgate-core/src/ingest.rs` (Δv budget, ~165):

```rust
    let eff = crate::stores::effective_params(&ship.spec[idx], &ship.mods[idx]);
```

`crates/jumpgate-core/src/autopilot.rs` (test-only) and `crates/jumpgate-core/src/ship.rs` (test fixture `eff_fixture`) — wrap each existing call so the `BaseSpec { … }` literal is followed by the identity bundle:

```rust
        effective_params(
            &BaseSpec { /* ...existing fields unchanged... */ },
            &crate::stores::EffectiveMods::IDENTITY,
        )
```

Update the existing `effective_equals_base_in_v1` test in `crates/jumpgate-core/src/stores.rs` — change the call to:

```rust
        let eff = effective_params(&spec, &EffectiveMods::IDENTITY);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p jumpgate-core effective_scales_with_thrust_factor effective_equals_base_in_v1`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs \
        crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/autopilot.rs \
        crates/jumpgate-core/src/ship.rs
git commit -m "refactor(core): effective_params takes &EffectiveMods; thrust_factor on max_thrust (plan A/3)"
```

---

### Task 4: Behaviour-preservation gate (the trajectory-equivalence proof)

This task adds no new behaviour; it proves the migration changed nothing observable. The existing goldens + full suite passing **unchanged** is the proof. `HASH_FORMAT_VERSION` stays `1`; **both** state goldens stay: `GOLDEN_ZERO_STATE_HASH = 0xf0dd_a1ba_f433_3735` and the cfg-with-craft golden `0x532d_07bf_95a2_abc5`.

**Files:**
- Test: `crates/jumpgate-core/src/world.rs` (new `identity_mods_preserve_trajectory`)

- [ ] **Step 1: Write the test**

Add to the `tests` module in `crates/jumpgate-core/src/world.rs` (uses the existing `one_body_one_thrusting_craft` fixture):

```rust
#[test]
fn identity_mods_preserve_trajectory() {
    // With the default (all-IDENTITY) mods, a thrusting transfer must produce the
    // SAME state as the pre-bundle build: thrust_factor == 1.0 and x*1.0 == x, so
    // max_thrust — and therefore pos/vel/fuel — are bit-identical.
    let cfg = one_body_one_thrusting_craft();
    let (mut world, _) = World::reset(cfg);
    let id = world.craft_ids()[0];

    assert_eq!(world.ships.mods[0], crate::stores::EffectiveMods::IDENTITY);

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

    // mods is never written in plan A, so it stays IDENTITY throughout.
    assert_eq!(world.ships.mods[0], crate::stores::EffectiveMods::IDENTITY);
    let p = world.craft_pos(id).unwrap();
    assert!(p.x.is_finite() && p.y.is_finite() && p.z.is_finite());
}
```

> If guidance landed first and made `World::reset` return `Result`, this test must `let (mut world, _) = World::reset(cfg).expect("resolvable cfg");`.

- [ ] **Step 2: Run the new test + the goldens**

Run: `cargo test -p jumpgate-core identity_mods_preserve_trajectory`
Expected: PASS.
Run: `cargo test -p jumpgate-core golden`
Expected: PASS — **both** state goldens unchanged (no hashed state added). If either fails, a hashed field was touched by mistake — STOP and investigate; do NOT re-baseline.

- [ ] **Step 3: Run the full suite + clippy (the real gate)**

Run: `cargo test -p jumpgate-core`
Expected: PASS — all lib tests (pre-existing count + 3 new) green.
Run: `cargo test -p jumpgate-core --test replay_equivalence --test physics_sanity`
Expected: PASS — replay + physics-sanity unchanged. (Post-guidance, the cruise-axis physics goldens were already re-derived by guidance; this plan must NOT move them again.)
Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. (`--lib` is a no-op here — binary crate.)

- [ ] **Step 4: Commit**

```bash
git add crates/jumpgate-core/src/world.rs
git commit -m "test(core): identity mods preserve trajectory; both goldens unchanged (plan A/4)"
```

---

## Plan A self-review

- **Spec coverage:** §2.0 (the two-arg gate) → Tasks 1+3; §4.2 named-field bundle + `IDENTITY` + reset-init + derived/not-hashed → Tasks 1+2; §12 steps 1–2 (seam + trajectory-equivalence) → Tasks 3+4. `HASH_FORMAT_VERSION` NOT bumped (no hashed state) — consistent with §5.
- **Cross-spec:** targets `CraftStore` (prelude), lands after guidance (settled trajectory baseline), records the reset-ordering forward-debt, names both goldens. Arg is the general `EffectiveMods` bundle (no future signature break when wear lands).
- **No placeholders:** every code step shows complete code; the two test-only call-site edits show the exact wrapping `, &EffectiveMods::IDENTITY)`.
- **Type consistency:** `EffectiveMods { thrust_factor: f64 }`, `EffectiveMods::IDENTITY`, `effective_params(&BaseSpec, &EffectiveMods) -> Effective`, `CraftStore.mods: Vec<EffectiveMods>` used identically across all tasks.

---

## What comes next (Plans B & C — outline only, detail after A lands)

Not bite-sized yet: their code depends on A's symbols + the post-guidance foundation. Detail them (re-run writing-plans) once Plan A is merged.

**Plan B — PersonStore + crew effect (spec §3, §4, §5, §6):**
1. `PersonId` (+`Ord`) + `EntityRef::Person` arm + `command_sort_key` extension `(scope_rank, kind_rank, slot, gen)` with Person=4 (update the stable-sort tie test).
2. `PersonStore` SoA: `location`, `skills: [f64; N_DOMAINS]`, `status`, `brain`, `personality`, `agenda`; finite-validation at construction; `PersonInit` in `RunConfig` — folded into the now-exhaustive `config_hash` destructure (the prelude makes omission a compile error).
3. Craft `roster: [Option<PersonId>; N_ROLES]`, `controller: Option<PersonId>`, `abstracted_crew_competence`; the lockstep transfer mutator (test-only API).
4. Hash fold of the new INPUT columns + person-store cursor → bump `HASH_FORMAT_VERSION` 1→2 and **re-derive BOTH** state goldens (`GOLDEN_ZERO_STATE_HASH` and `0x532d…`; the version word seeds the hasher, so both move). Keep this single-cause and separate from guidance's trajectory re-derivation.
5. `compute_crew_mods`: writes the crew contribution into the `mods` bundle. Runs at **reset** (before the Guidance resolvability guard) AND top-of-step (before ingest, LOD-gated, per-domain accumulation); `resolve_controller` (`Aboard && Alive`) + `is_effective_crew` gates; `RoleDemand` const table (Leadership weight 0); frozen `NEUTRAL_LEADERSHIP` + `f`/`g` with exact empty-state identity. Hashes INPUTS, never the derived `mods` cache.
6. Proof tests: controller-swap thrust drop; budget/burn same-`Effective`; roster-shuffle hash-invariance; controller-validity; tested inertness (flip personality bits); leadership-never-in-demand; dead-vs-stale; incapacitated-zero.

**Plan C — shaped-inert reserves + crewed scenario fixtures (spec §7, §11):**
- `AtStation`/`InSpace` resolver stubs; reserved `Crew` RNG stream; succession comparator (defined+tested, not auto-wired); crewed scenario fixtures exercising a populated roster end-to-end.
