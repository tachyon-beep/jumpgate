# jumpgate — Person + Ship Foundational Design

**Date:** 2026-06-08
**Branch:** `jumpgate-v1-design`
**Status:** Design — awaiting user review before plan
**Build stance:** A — two first-class stores, one live seam (foreclose nothing structurally, implement trivially)

This spec extends the v1 Newtonian core
(`2026-06-08-jumpgate-newtonian-space-core-design.md`) with the two most
foundational game objects: **`Person`** and the crew/control half of **`Ship`**.
It was produced through a brainstorming dialogue plus a 5-lens expert panel
(emergence, simulation/LOD, determinism, architecture, solution-design
discipline) whose findings are reconciled here.

---

## 1. Goal & the central tension

Support the realism we will want later — crew on a flight deck, CK3-style
character drama, DRL-controlled captains, multi-domain skill — **without
threading any of it through the physics core.** The codebase already proves the
mechanism: capability changes enter through the `Effective = base × mods × wear`
seam (`effective_params`), computed in deterministic tick-stages, never through
the integrator. This design applies that discipline to people.

**Decisions taken in the brainstorm (locked):**

1. **Unified `Person`** — one class is both the *controller* (captain / pilot /
   DRL-embodied) and a *crew modifier* (the engineer who boosts efficiency).
2. **First-class entity** with its own generational id and a new
   `EntityRef::Person` arm; location is an enum (aboard / at-station / in-space).
3. **Person-LOD** — model only macro-influential roles; the rest of the crew is
   a single abstracted competence scalar.
4. **Pluggable brain** — a person's decision source is a slot
   (policy / state-engine / trait-driven); only the bare-autopilot arm is live.
5. **Skills → the `Effective` seam** — competence is a multiplier, never a
   physics-path change.
6. **CK3 personality** — trait flags + base archetypes; data shaped, behaviour
   inert ("free to build in").
7. **Off-ship agenda** — coarse activity when docked; reserved, inert.
8. **One authority + skill auras** — exactly one controller drives a ship; all
   other modeled persons contribute only through skill→`Effective` + trait
   flavour.
9. **Succession cascade (emergent)** — per-domain skill vector × role
   skill-demand × a dynamic, reassignable controller slot, with **leadership as
   a crew-wide meta-modifier**. A great captain dies, a graceless genius is
   promoted, the leadership aura collapses and the whole ship degrades — with no
   special-cased code.

**Panel verdict: sound-with-changes.** The architecture is right; three changes
are mandatory (§2.0), and the multi-domain half of the cascade is correctly
*shaped-inert* in v1 because `Effective` has only one functional output channel
today (§4.3).

---

## 2. Overview & the three mandatory changes

### 2.0 The only non-additive gate

`effective_params(spec: &BaseSpec) -> Effective` is read at 6+ sites — including
the Δv **budget** in `ingest.rs` and the **burn** in `world.rs`. Threading crew
mods later would be a multi-site refactor *and* would silently desync budget from
burn the instant a mod goes non-identity. Therefore, **now, with identity
values**:

```rust
pub fn effective_params(spec: &BaseSpec, mods: &CrewMods) -> Effective { /* base × mods */ }
```

With `CrewMods::IDENTITY`, `effective_params(spec, &IDENTITY)` is **bit-identical**
to today. Every other piece of this design is additive behind this one gate.

The two other mandatory changes: **hash the modifier inputs from this same
commit** (§5) and **use fixed enum-indexed arrays** for skills/roster (§3, §5).

---

## 3. The `Person` model

`PersonStore` is a third first-class SoA store beside `ShipStore` / `BodyStore`,
with the same generational-`SlotMap` discipline and the same `slot == row` v1
invariant.

```rust
// ids.rs — mirrors CraftId/BodyId exactly; Ord is non-negotiable (succession + hash sort).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PersonId { pub slot: u32, pub generation: u32 }

pub struct PersonStore {
    pub ids:         SlotMap<()>,        // [LIVE] identity; cursor is hashed
    pub location:    Vec<Location>,      // [LIVE] discriminant + container id; position derived
    pub skills:      Vec<SkillVector>,   // [LIVE-as-data] full vector stored+hashed; Engine+Leadership wired
    pub status:      Vec<PersonStatus>,  // [LIVE] gates succession; changes mid-run → hashed
    pub brain:       Vec<Brain>,         // [LIVE tag] only None/Autopilot dispatch
    pub personality: Vec<Personality>,   // [SHAPED-INERT] stored; passed to compute_crew_mods as ignored arg
    pub agenda:      Vec<Agenda>,        // [SHAPED-INERT] one reserved enum value; no state machine
}
```

### 3.1 Enums and the live/inert split

```rust
pub enum Location {
    Aboard(CraftId),    // [LIVE] position DERIVED from the ship; role lives in ship.roster, NOT here
    AtStation(BodyId),  // [SHAPED-INERT] constructible; resolver unimplemented in v1
    InSpace(Vec3),      // [SHAPED-INERT] set-once; NOT integrated, NOT woken (EVA deferred)
}
```

`Aboard` carries **no role** — the ship's roster is the single authority for
role↔person (§4.1). Only `InSpace` carries own coordinates that get hashed;
`Aboard`/`AtStation` hash discriminant + container id only.

```rust
pub const N_DOMAINS: usize = 5;
#[repr(usize)]
pub enum Domain { Engine = 0, Leadership = 1, Nav = 2, Gunnery = 3, Sensors = 4 }
pub struct SkillVector(pub [f64; N_DOMAINS]);  // fixed array — canonical iteration & sum order
```

The **full 5-vector is stored, hashed, and summed by the real reducer from day
one**, but only `Engine` and `Leadership` map to a real codomain in v1.
`Nav`/`Gunnery`/`Sensors` weights are dead config until those domains exist
(§4.3). Widening `N_DOMAINS` later is a `HASH_FORMAT_VERSION` bump.

```rust
pub enum PersonStatus { Alive, Incapacitated, Dead }  // [LIVE] Copy, Ord; folded in hash
pub enum Brain { None, Autopilot, Policy, Fsm, TraitDriven }  // [LIVE: only None/Autopilot dispatch]
pub struct Personality(pub u32);  // [SHAPED-INERT] Copy bitflags; pinned bit order; NOT String/Vec
pub enum Agenda { OnDuty }        // [SHAPED-INERT] one reserved value
```

`Brain`'s `Policy`/`Fsm`/`TraitDriven` arms are typed-but-uninhabited-of-logic
(exactly like `Lod::NpcInteraction` today). **No dispatch code for the dead
arms.** When `Policy` lands it is a **Command producer at the ingest boundary**,
never invoked inside `step()` (§5).

`status = Dead` rather than freeing the slot: `PersonStore` is the first store
with real mid-run "removal", so every person lookup returns `Option` via the
dense-index resolver and **never `expect`s**.

---

## 4. The `Ship` additions and the one live seam

### 4.1 New `ShipStore` columns + source-of-truth ownership

```rust
pub const N_ROLES: usize = 8;  // modeled macro-roles only
pub enum Role { Captain, ChiefEngineer, Pilot, FlightDeckCoordinator, /* … reserved */ }

// length-parallel ShipStore columns (added with all other columns; push()/reset honour slot==row):
pub roster:     Vec<[Option<PersonId>; N_ROLES]>,  // [LIVE] fixed array indexed by Role — canonical order
pub controller: Vec<Option<PersonId>>,             // [LIVE] default None == today's bare autopilot
pub crew_mods:  Vec<CrewMods>,                      // [LIVE] derived cache; identity (all 1.0) in v1
pub abstracted_crew_competence: Vec<f64>,          // [LIVE] one scalar for the unmodeled crew mass
```

**`ship.roster` is the single authority for role↔person binding.** A person's
role is *derived* by reverse-lookup from the roster, never stored on the Person.
`Location::Aboard(CraftId)` records *which ship*; the roster records *which
chair*. Crew transfer goes through **one lockstep mutator** that writes both ends
(`person.location = Aboard(ship)` + `ship.roster[role] = Some(person)`) and
recomputes `crew_mods` synchronously. The hash folds only the authoritative
direction.

The controller is exposed only behind a
`controller_for_domain(domain) -> Option<PersonId>` accessor (returning the one
controller for every domain in v1). When per-domain controllers arrive, the
accessor changes; the readers do not.

### 4.2 `compute_crew_mods` — placement, ordering, and feed

`compute_crew_mods` runs as a single canonical pass at the **top of `step()`,
before `ingest_commands`**, writing the per-craft `crew_mods` column. It reads
only already-committed state and mutates nothing else.

```
step():
  (0) compute_crew_mods    ← NEW: write ships.crew_mods[*] (reads roster, skills, controller,
                                  status, personality[ignored]); LOD-gated (Nothing => skip)
  (1) ingest_commands      ← Δv budget reads effective_params(spec, &crew_mods[ci])
  (2) integrate loop       ← burn reads effective_params(spec, &crew_mods[ci])
  (3) event detect
  (4) copy-forward (prev_fuel …)
  (5) tick++
```

**Top-of-step, not between ingest and integrate:** the Δv *budget* and the *burn*
both read `Effective`. Computing into a persisted column *before* ingest makes
both readers see the identical `Effective`; computing in between would silently
desync them the instant a mod goes non-identity. Enforced by a test with a
deliberately non-identity fixture asserting budget and burn read the same
`Effective`.

The column feeds `effective_params` as an already-reduced struct — so
`effective_params` stays Person-agnostic *and* carries the mods:

```rust
pub struct CrewMods {        // named fields, NOT a bare f64
    pub thrust_factor: f64,  // [LIVE] the one wired channel
    // reserved (default 1.0): gunnery_factor, sensor_factor, … — adding a field, never a signature change
}
impl CrewMods { pub const IDENTITY: CrewMods = CrewMods { thrust_factor: 1.0 }; }
```

### 4.3 The live v1 formula (single thrust channel) + the inert multi-domain half

`Effective` has one functional output channel (thrust/fuel) today. So:

- **LIVE:** the **leadership × engine cascade on the thrust channel.**
- **SHAPED-INERT:** multi-domain separability ("engines fine, gunnery degrades")
  — not *observable* until a second domain has a real codomain.

```text
compute_crew_mods, per ship:
  for each occupied roster slot:  contribution[role] = dot(person.skills, RoleDemand::for(role))
  crew_sum   = Σ contribution[role]            // iterate roster in fixed Role-index order (canonical)
  leadership = controller.map(|p| skills[Leadership]).unwrap_or(NEUTRAL)
  thrust_factor = clamp( f(leadership) · g(crew_sum, abstracted_crew_competence) )
  // Only Engine demand-weight + the Leadership meta-multiply map to a real codomain.
  // Nav/Gunnery/Sensors demand-weights are 0.0 (dead config).
```

`RoleDemand` is **const data** (`fn demand(role) -> [f64; N_DOMAINS]`), not
match-arm code, so succession is pure data substitution into a fixed formula. It
rides the **config hash** (immutable initial condition), not the per-tick state
hash.

**v1 formula precision (user decision):** *any monotone, four-ops, clamped
formula that passes the proof test.* No transcendentals (`exp`/`sqrt` can drift
even same-build). Calibration is deferred until there is gameplay to tune
against.

**The proof test (makes the seam non-vaporware):** controller =
high-leadership / mediocre-engineer → step → record `thrust_factor`. Reassign
controller = graceless genius (low-leadership / elite-engineer) → step →
**assert `thrust_factor` drops** despite the better engineer in the chair
(`low_leadership · high_engine < high_leadership · med_engine`). Minimum viable
succession cascade, live, on one channel, zero special-case code.

---

## 5. Determinism plan

Determinism stance is **relaxed** (seed-reproducible on the same build only; not
multiplayer/lockstep; no cross-platform bit-identity). Replay-on-same-build is
still required for RL debugging.

**Hash folding — `HASH_FORMAT_VERSION` 1 → 2, golden re-derived, the
`recompute_with_cursors` executable spec updated in the same commit.** Append at
reserved `HASH_FIELD_ORDER` words 16+ (words 14–15 stay reserved for
`prev_fuel`/`prev_inside_dest`), discriminant-first / self-delimiting:

- `person_store.ids.cursor()` (via the existing `write_store_cursor` pattern) —
  without it, two worlds with different Person spawn histories hash equal.
- Persons **sorted by `PersonId`**: `location` (discriminant + container id;
  `InSpace` `Vec3` folded only on that arm), `skills` (all N domains via
  `to_bits()` in enum order), `status`.
- Ship `roster` (fixed array, Role-index order) and `controller` (authoritative
  link direction only).

**Hash the inputs, never the derived `crew_mods` cache** — it is transitively
pinned by its inputs exactly as `prev_fuel[t] == fuel[t-1]` is. Hash inputs
**from the commit that makes `effective_params` two-argument**, even though v1
values are identity, or a run recorded before "crew on" and one after agree then
silently diverge.

**Other channels:**

- **Command sort order:** when `EntityRef::Person` is added, extend
  `command_sort_key` to `(scope_rank, kind_rank, slot, gen)` with
  `Sim=0, World=1, Craft=2, Body=3, Person=4`, and update the existing
  stable-sort tie test to assert distinct keys. (The current Craft/Body collision
  is documented + tested intentional; this makes the order total on kind before
  a third kind depends on the tie.)
- **Brain/policy nondeterminism:** the policy is a Command producer at the ingest
  boundary, invoked only during record, **never inside `step()`, never during
  replay** (which re-feeds the log and never calls the driver). A torch/CUDA
  forward pass inside the hashed loop would break replay outright.
- **Succession tie-break:** `argmax` over a total order
  `(matched_skill_bits DESC, PersonId ASC)`. Defined and unit-tested **now**,
  though the cascade is otherwise inert — an unspecified comparator is a silent
  divergence the first time it fires.
- **RNG:** reserve a named `Crew` variant in the `RngStream` enum now
  (salt-derived ChaCha8 sub-streams are draw-order-independent, so reservation
  perturbs nothing). No crew draw exists yet; any future one goes through
  `RngStreams`.
- **Float-sum order:** fixed enum-indexed arrays give canonical iteration and
  deterministic summation for free. Test: shuffle roster *insertion* order →
  assert identical `state_hash`. No Kahan/fixed-point (relaxed stance needs
  deterministic *order*, not portable arithmetic).

**Two invariants tested separately (do not conflate):**
1. **Golden state-hash** re-derived + version-bumped — *expected to change* (new
   fields).
2. **Trajectory-equivalence** — with `controller = None` + empty roster,
   `pos/vel/fuel` over N ticks are **bit-identical** to the pre-Person build.

---

## 6. Person-LOD ↔ ship-LOD composition

**Person has no `Lod` field.** A parallel Person-LOD enum is a foot-gun (a "live"
person on a dormant ship is an impossible state). Person simulation-LOD is
derived from the container:

- `Aboard(ship)` → live iff `ships.lod[row] != Nothing`.
- `compute_crew_mods` is gated behind the **same `Lod::Nothing => continue`**
  predicate the physics loop uses; a dormant ship pays zero. On wake
  (`Nothing → Player`), `crew_mods` is recomputed before the first stepped tick.
- `AtStation` persons are inert stored data in v1.
- `InSpace` is set-once/inert, so independent person-LOD does not arise.

**Recompute cadence (user decision: wear is per-tick continuous):** the per-tick,
LOD-gated top-of-step pass is **mandatory, not wasteful** — continuous wear means
crew-mods genuinely vary every tick. Event-driven invalidation is therefore *not*
built; the per-tick pass is the simplest correct architecture. (Should wear ever
become discrete, the persisted column is already the single read surface that
would make event-driven recompute safe to add.)

---

## 7. The YAGNI line

**BUILD NOW (live, tested):**

- `PersonStore` SoA + `EntityRef::Person` + `PersonId` (with `Ord`).
- `effective_params(spec, &CrewMods)` + `CrewMods` named-field struct (identity
  v1) + `crew_mods` column. *(The one non-additive gate.)*
- `compute_crew_mods` top-of-step stage, LOD-gated, full
  `skill × role-demand × leadership` formula on the thrust channel.
- Ship `roster` (fixed array), `controller: Option<PersonId>` (default None),
  `abstracted_crew_competence`, `controller_for_domain` accessor, the lockstep
  transfer mutator (**test-only API in v1** — no Command triggers it yet).
- `Location` (Aboard live), `Domain`/`SkillVector` (full vector stored+hashed;
  Engine+Leadership wired), `PersonStatus`, `Brain` (None/Autopilot dispatch),
  `Role` + const `RoleDemand` table.
- Hash fold of all live columns + person cursor (version 2); extended
  `command_sort_key`; succession comparator; `Crew` RNG reservation.
- Tests: controller-swap drops `thrust_factor`; trajectory-equivalence;
  roster-shuffle hash-invariance; budget/burn same-`Effective`.

**SHAPE-INERT (data/site present, no behaviour):**

- `personality` bitfield — stored **and** passed to `compute_crew_mods` as an
  ignored arg (the future `trait × skill` interaction site).
- `Nav`/`Gunnery`/`Sensors` domains + their zero demand-weights — stored/hashed,
  no codomain.
- `Brain::Policy`/`Fsm`/`TraitDriven` — typed tags, no dispatch.
- The succession comparator — defined + tested, not wired into auto-promotion.
- `Location::AtStation` — constructible, resolver unimplemented.

**CUT-UNTIL-PULLED (not even stored; append later behind a version bump):**

- Per-domain `Effective` *sets* (multiple `Effective` structs) — one domain
  exists; the `CrewMods` struct seam makes this additive.
- `agenda` beyond one reserved enum value (no state machine, no docking hooks).
- `Location::InSpace` integrated physics (EVA).
- A parallel Person-LOD enum.
- Per-crewman rows for the unmodeled mass (the scalar is the whole
  representation).
- Automatic multi-role promotion / vacancy-backfill / leadership-collapse
  *logic*.

---

## 8. Risk register

| # | Sev | Risk | Mitigation |
|---|-----|------|------------|
| 1 | High | `effective_params(&BaseSpec)` can't carry crew mods; deferring = non-additive 6-site refactor + hash bump. | Land `effective_params(spec, &CrewMods)` + `crew_mods` column now, identity-valued, bit-identical to today. (§2.0, §4.2) |
| 2 | High | 5-skill vector, 1 functional channel — multi-domain "engines fine, ship degrades" not demonstrable in v1. | Bifurcate: LIVE = leadership × engine on thrust (proof test); INERT = multi-domain. Store full vector, wire Engine+Leadership. (§4.3) |
| 3 | High | Inputs un-hashed in v1 → "crew on" later is a silent agree-then-diverge replay break. | Hash modifier inputs from the two-arg commit, version 2; never hash the derived cache. (§5) |
| 4 | High | Non-associative f64 sum over an unordered roster diverges same-build. | Fixed enum-indexed `[f64; N]` skills + roster; Role-index iteration; shuffle-invariance test. (§3, §5) |
| 5 | High | Roster↔location denormalized truth desyncs, hiding origin if both hashed. | `ship.roster` sole authority; role derived; one lockstep mutator; hash only authoritative direction. (§4.1) |
| 6 | High | CK3 trait half has no interaction surface → future non-additive retrofit. | Pass `personality` into `compute_crew_mods` as an ignored arg now — the multiply-site exists. (§3.1, §7) |
| 7 | Med | `command_sort_key` collapses Craft/Body (documented stable-sort tie). | Extend to `(scope_rank, kind_rank, slot, gen)`, Person=4; update tie test to assert distinct keys. (§5) |
| 8 | Med | `Brain::Policy` forward pass inside `step()` breaks replay. | Policy is a Command producer at the ingest boundary; never in `step()`/replay. (§5) |
| 9 | Med | Phase skew: Δv budget vs burn read different `Effective`. | `compute_crew_mods` at top of step into a persisted column both readers use; non-identity-fixture test. (§4.2) |
| 10 | Med | Succession tie-break unspecified → silent divergence. | `argmax` over `(skill_bits DESC, PersonId ASC)`, defined+tested now. (§5) |
| 11 | Med | `InSpace(pos)` adds a second integrated body, contradicting "skills never touch the integrator." | `InSpace` set-once/inert, never integrated/woken; only that arm's Vec3 hashed. (§3.1, §7) |
| 12 | Med | `PersonStore` is first store with real mid-run removal; `expect` on dead lookup breaks on cascade. | `status = Dead` over slot-freeing; Option-returning resolver, never `expect`; hash by sorted live id. (§3.1) |

---

## 9. Calibration decisions (resolved with the user)

1. **Domains/Roles:** accept proposed defaults — `Domain = {Engine, Leadership,
   Nav, Gunnery, Sensors}` (Engine+Leadership wired); `Role = {Captain,
   ChiefEngineer, Pilot, FlightDeckCoordinator, …}`. Widen on demand.
2. **Wear cadence:** per-tick continuous → the per-tick crew-mod pass is
   mandatory; no event-driven invalidation machinery in v1.
3. **Mod formula:** any monotone, four-ops, clamped formula that passes the
   drop-on-graceless-genius test; calibration deferred.
4. **Crew transfer:** test-only lockstep mutator in v1 (no Command triggers it
   yet; Person's command-sort rank is reserved, not yet a live consumer).

---

## 10. Testing strategy (summary)

- **Trajectory-equivalence:** `controller=None` + empty roster ⇒ `pos/vel/fuel`
  bit-identical to pre-Person build over N ticks.
- **Golden state-hash:** re-derived under `HASH_FORMAT_VERSION = 2`; independently
  recomputed (never trust a subagent's gate claim).
- **Proof-of-cascade:** controller swap (good captain → graceless genius) drops
  `thrust_factor`.
- **Phase:** non-identity fixture ⇒ Δv budget and burn read the same `Effective`.
- **Determinism:** roster insertion-order shuffle ⇒ identical `state_hash`;
  succession comparator tie-break unit test.
- **Store discipline:** `PersonStore` SoA length-parallel after `push`; stale-id
  lookups return `None`, never panic.

---

## 11. Explicitly out of scope (deferred rungs)

Combat/weapons and sensor models; station/planet NPC simulation; EVA physics;
the autonomous off-duty agenda state machine; trait-driven and FSM brains; the
live DRL `Policy` brain (the *seam* is shaped; the model is a later deliverable);
automatic succession/promotion logic; per-domain `Effective` sets; per-crewman
modeling of the abstracted mass.
