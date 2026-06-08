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

## 0. Terminology & cross-spec reconciliation (2026-06-09 addendum)

This addendum supersedes wording below it; the body is otherwise unchanged.

**Terminology (per `docs/glossary.md`).** The canonical unit is **craft** (generic,
drone→titan); "ship" is rejected as a class word. Throughout this doc read:
- "ship" (the unit) → **craft**; the internal store `ShipStore` → **`CraftStore`**
  (renamed in the prelude, below).
- **`CrewMods` → `EffectiveMods`** — and it is now a **general modifier bundle**,
  not a crew-only struct. Founding intent is `Effective = base × component-mods ×
  wear` (three factor sources); `EffectiveMods` pre-reduces all of them into one
  struct so `effective_params` never changes signature again when wear/component
  mods land. v1 carries only `thrust_factor` (the crew contribution); `compute_crew_mods`
  writes the crew part, a future `compute_wear` folds into the SAME bundle. The
  derived craft-store column is named **`mods`** (not `crew_mods`).
- **Captain** = the per-craft command authority (glossary), which is exactly the
  **controller** slot here; `controller = None` is the drone-chip/autopilot captain.

**Capability vs policy seam split (cross-spec, with the Guidance-Parameter spec).**
`EffectiveMods` carries **capability** (what a craft *can* do — `max_thrust` after
crew/wear scaling) and goes **into `effective_params`** because the integrator's
burn reads `Effective`. `GuidanceParams` (cruise/brake tuning) carries **policy**
(dt- and arrival-dependent) and stays in **`autopilot_command`**. Orthogonal; both
correct. (Guidance D1's "effective_params is unchanged" is time-scoped to *its* diff;
this spec is the sanctioned channel that changes it for capability mods.)

**Sequencing (cross-spec review — architecture-critic + determinism-reviewer).**
Land order is **prelude → Guidance-Parameter spec → this spec's Plan A → B → C**:
- **Prelude** (`plans/2026-06-09-jumpgate-prelude-craftstore-confighash.md`):
  `ShipStore→CraftStore` rename + `config_hash` exhaustive destructure. Owned by
  neither spec; both depend on it. The destructure makes a forgotten config field
  (this spec's `PersonInit`) a **compile error**, not a silent provenance hole.
- **Guidance first** so Plan A's trajectory-equivalence proof lands on a *settled*
  cruise baseline (guidance re-derives the cruise-axis *trajectory* goldens).

**Two determinism deltas folded into §5 below:**
1. **Resolve `mods` at reset**, before the Guidance `World::reset` resolvability
   guard runs (the guard reads effective `max_thrust`; once `mods` is non-identity
   it must read the modified value, and reset precedes any step). Identity in Plan A,
   so trivially satisfied; the ordering is fixed now.
2. **Plan B re-derives BOTH state goldens** (`GOLDEN_ZERO_STATE_HASH` *and* the
   cfg-with-craft `0x532d…`) on the `HASH_FORMAT_VERSION` 1→2 bump — the version word
   seeds the hasher, so both move. Keep this single-cause and separate from guidance's
   trajectory re-derivation.

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
    pub brain:       Vec<Brain>,         // [SHAPED] stored tag; no command dispatch in v1
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
pub enum Brain { None, Autopilot, Policy, Fsm, TraitDriven }  // [SHAPED] no v1 command dispatch
pub struct Personality(pub u32);  // [SHAPED-INERT] Copy bitflags; pinned bit order; NOT String/Vec
pub enum Agenda { OnDuty }        // [SHAPED-INERT] one reserved value
```

**`Brain` is shaped, not live, as a command source in v1.** *No* `Brain` variant
produces commands in v1 — not even `Autopilot`/`None`. The ship's existing
nav-executor (`autopilot.rs`, `NavState → thrust`) is **unchanged and is NOT a
Person brain**; it always runs, orthogonal to crew. The `Brain` field is a
reserved tag for the future embodied-control work; when `Policy`/`Fsm`/`TraitDriven`
land they are **Command producers at the ingest boundary** (recorded, replayable),
never invoked inside `step()` (§5). The controller slot's *only* LIVE effect in v1
is the leadership meta-modifier via `resolve_controller` (§4.1).

**v1 command authority (explicit, to kill the `controller=None` vs `Brain::None`
ambiguity):**

| controller | resolves valid? | brain | v1 behaviour |
|---|---|---|---|
| `None` | — | — | existing nav-executor + external/player/test command path; leadership = `NEUTRAL` |
| `Some(p)` | yes | any | **identical command path**; the *only* difference is `crew_mods` (leadership = `p.skills[Leadership]`) |
| `Some(p)` | no (stale/dead/other-ship) | any | as `None`: leadership falls back to `NEUTRAL` |

No `Brain` arm has dispatch code in v1; `Policy`/`Fsm`/`TraitDriven` are stored
tags only.

**Status contribution (binary in v1):** only `Alive` persons contribute — to the
controller meta-modifier **and** to `domain_sums`. `Incapacitated` and `Dead`
remain present and hashable but contribute **zero**. (Partial contribution,
recovery, command penalties are deferred.)

**Finite-value invariant (determinism-critical).** Every `f64` that is hashed or
enters the formula — all `skills`, `abstracted_crew_competence`, and any
`Location::InSpace(Vec3)` component — **must be finite (non-NaN, non-±∞) and is
validated/clamped at creation/load**. A NaN hashed via `to_bits()` poisons sort
keys, equality, and the determinism story; non-finite skill inputs are **rejected
at construction**, never silently folded. (Risk 18.)

**Dead vs stale (4 distinct states, never conflated).** A `PersonId` is either:
(1) present+`Alive`, (2) present+`Incapacitated`, (3) present+`Dead`, or
(4) absent/stale (generation mismatch / invalid slot). `status = Dead` rather
than freeing the slot — so a dead person is still **hashable state**, while a
stale reference is **not present at all**. Every person lookup returns `Option`
via the dense-index resolver and **never `expect`s**; `resolve_person` (presence),
`is_effective_crew` (present+`Alive`), and `resolve_controller` (present+`Alive`+
`Aboard(this_ship)`) are the three distinct gates. Both a stale and a present-but-
`Dead` roster entry are skipped for crew effect, with well-defined hash treatment
either way.

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
(`person.location = Aboard(ship)` + `ship.roster[role] = Some(person)`). The
mutator does **not** recompute `crew_mods` — that is the exclusive job of the
top-of-step pass (§4.2), which runs every tick (per-tick continuous wear, §6).
A synchronous recompute in the mutator would be a redundant second derivation
that could disagree with `step()`'s; the single derivation point is the
invariant. (`crew_mods` is initialized to `CrewMods::IDENTITY` at reset so any
projection read before the first `step()` is well-defined.) The hash folds only
the authoritative direction.

The controller is exposed only behind a
`controller_for_domain(domain) -> Option<PersonId>` accessor (returning the one
controller for every domain in v1). When per-domain controllers arrive, the
accessor changes; the readers do not.

**Controller validity is resolved, never assumed.** A `controller: Some(p)` is
only *effective* if `p` is `Aboard(this_ship)` **and** `status == Alive`. A
`resolve_controller(ship) -> Option<PersonId>` accessor enforces this and is the
only path `compute_crew_mods` uses; a controller who is stale, dead/incapacitated,
or aboard another ship resolves to `None` and the leadership term falls back to
`NEUTRAL_LEADERSHIP` (§4.3). This makes a leaderless or dead-captain ship degrade
to the no-leader baseline — a passive foretaste of the cascade before
auto-promotion exists. (Using a live person's leadership from *another* ship is
the dangerous case generational-id staleness alone would NOT catch; the
location+status check is what closes it.)

**Controller ↔ role relationship (stated to avoid later confusion):** the
controller *may* also occupy a roster slot (e.g. the Captain is both
`roster[Captain]` and `controller`). When they do, their **non-leadership**
skills contribute through that role's demand vector exactly like any crew member;
their leadership contributes **only** through the controller meta-modifier (it has
0 role-demand weight, §4.3) — so there is no double-count. Crucially,
**`controller` need not equal `roster[Captain]`**: the controller can be a pilot,
an acting commander, or (later) an AI. This is precisely what enables the
"graceless genius promoted into the chair" scenario.

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

**Tick-timing (frozen):** `crew_mods[t]` is computed from **committed world
state at the start of tick t** — after all commands/events from tick t-1 have
been applied, and *before* any tick-t command is ingested. Consequence: a
personnel change (assignment, status, injury, future crew-transfer command)
ingested *during* tick t takes effect on **tick t+1**, never the same tick. This
"changes affect next tick" rule is the deterministic default; a pre-step
personnel-command phase would be the only thing that could change it, and it is
deliberately not introduced in v1.

**Top-of-step, not between ingest and integrate:** the Δv *budget* and the *burn*
both read `Effective`. Computing into a persisted column *before* ingest makes
both readers see the identical `Effective`; computing in between would silently
desync them the instant a mod goes non-identity. Enforced by a test with a
deliberately non-identity fixture asserting budget and burn read the same
`Effective`.

The column feeds `effective_params` as an already-reduced struct — so
`effective_params` stays Person-agnostic *and* carries the mods:

```rust
pub struct EffectiveMods {   // general modifier BUNDLE (not crew-only); named fields, NOT a bare f64
    pub thrust_factor: f64,  // [LIVE] crew contribution to the thrust channel
    // reserved (default 1.0): wear/component factors, future per-domain factors —
    // adding a field is additive, NEVER a change to effective_params's signature.
}
impl EffectiveMods { pub const IDENTITY: EffectiveMods = EffectiveMods { thrust_factor: 1.0 }; }
```

`compute_crew_mods` writes the *crew* contribution into this bundle; a future
`compute_wear` folds wear into the **same** bundle (pre-reduced, one multiply in
`effective_params`). The craft-store column is `mods: Vec<EffectiveMods>`.

`crew_mods` is set to `CrewMods::IDENTITY` at `reset()` (so pre-first-step
projection reads are well-defined) and overwritten by `compute_crew_mods` at the
top of every `step()`. It is **derived** state: never mutated anywhere else,
never folded into the hash (§5).

### 4.3 The live v1 formula (single thrust channel) + the inert multi-domain half

`Effective` has one functional output channel (thrust/fuel) today. So:

- **LIVE:** the **leadership × engine cascade on the thrust channel.**
- **SHAPED-INERT:** multi-domain separability ("engines fine, gunnery degrades")
  — not *observable* until a second domain has a real codomain.

```text
compute_crew_mods, per ship:
  // PER-DOMAIN accumulation — keep domains separate, never collapse to one scalar.
  domain_sums = [0.0; N_DOMAINS]
  for each occupied roster slot (iterate in fixed Role-index order — canonical):
      person  = is_effective_crew(roster[role])    // Option-aware; skip empty/stale/non-Alive
      demand  = RoleDemand::demand(role)           // [f64; N_DOMAINS], const
      for d in 0..N_DOMAINS:  domain_sums[d] += person.skills[d] * demand[d]

  leadership   = resolve_controller(ship)          // §4.1: Aboard(this_ship) && Alive
                   .map(|p| p.skills[Leadership])
                   .unwrap_or(NEUTRAL_LEADERSHIP)
  thrust_factor = clamp( f(leadership) * g(domain_sums[Engine], abstracted_crew_competence) )
  // v1 reads ONLY domain_sums[Engine] and the leadership meta-multiply.
  // Nav/Gunnery/Sensors sums are accumulated but UNREAD (dead config) until those
  // domains gain a codomain — then it's `gunnery_factor = h(leadership, domain_sums[Gunnery])`,
  // a NEW CrewMods field, NEVER a change to this summation. (Additivity preserved.)
```

**Per-domain, not scalar.** Accumulating into `domain_sums[d]` rather than a
single `crew_sum` is what keeps multi-domain additive: adding a live domain later
reads its own sum into a new `CrewMods` field without restructuring the loop. Same
cost, derived (unhashed), proof test unchanged.

**`NEUTRAL_LEADERSHIP` is frozen, and the empty state is identity.** Define
`const NEUTRAL_LEADERSHIP: f64` and choose `f`, `g` so that with **no effective
controller, an empty roster, and default `abstracted_crew_competence`**,
`thrust_factor == 1.0` *exactly*. This is forced by the trajectory-equivalence
invariant (§5): the `controller=None`/empty-roster default must be bit-identical
to the pre-Person build. Concretely, `f(NEUTRAL_LEADERSHIP) == 1.0` and
`g(0.0, default_competence) == 1.0`. Per the user's calibration choice, any
monotone four-ops clamped `f`/`g` satisfying this and passing the drop test is
acceptable; the constants are frozen once chosen so the golden hash is stable.
**Tuning is deferred; numerical exactness is not** — at implementation time the
constants, operation order, and clamp order are pinned exactly (with explicit
min/max bounds), and skill inputs are finite-validated (§3.1). A monotone shape
is the only gameplay commitment; the arithmetic is otherwise fully specified
before code lands. Leadership's 0 role-demand weight is enforced by a test
(`assert RoleDemand::demand(role)[Leadership] == 0.0` for every role), so a future
contributor cannot reintroduce the double-count by "helpfully" weighting it.

**Leadership enters only as the meta-modifier.** `RoleDemand::demand(role)` has a
**0.0 weight on the `Leadership` domain for every role** — leadership influences
the ship solely through the controller meta-multiply `f(leadership)`, never via
`domain_sums`. This removes the double-count (controller's leadership counted both
as meta-modifier and as a crew-sum contributor) by construction. `domain_sums`
[Leadership] is therefore always 0 in v1.

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
- Persons **sorted by `PersonId`** (all *present* persons regardless of status —
  a `Dead` person is still hashed state in id order; stale slots are simply
  absent): `location` (discriminant + container id; `InSpace` `Vec3` folded only
  on that arm), `skills` (all N domains via `to_bits()` in enum order), `status`.
  All hashed `f64`s are finite by the §3.1 construction invariant, so `to_bits()`
  never folds a NaN payload.
- Ship `roster` (fixed array, Role-index order), `controller` (authoritative link
  direction only), and **`abstracted_crew_competence`** — the last is a mutable
  *input* to `crew_mods` (not transitively pinned by roster/controller), so
  omitting it would silently diverge replay the first time it changes (e.g. an
  unmodeled-crew casualty). Constant in v1; the column is folded regardless.

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
- `compute_crew_mods` top-of-step stage, LOD-gated; **per-domain** accumulation
  (`domain_sums[N_DOMAINS]`), v1 reading only `[Engine]` + the leadership meta;
  `crew_mods` initialized to `IDENTITY` at reset.
- Ship `roster` (fixed array), `controller: Option<PersonId>` (default None),
  `abstracted_crew_competence`, `controller_for_domain` + `resolve_controller`
  (`Aboard && Alive`) accessors, the lockstep transfer mutator (**test-only API
  in v1** — no Command triggers it yet; writes authoritative columns only).
- `NEUTRAL_LEADERSHIP` constant + `f`/`g` chosen so the empty state is exactly
  identity.
- `Location` (Aboard live), `Domain`/`SkillVector` (full vector stored+hashed;
  Engine+Leadership wired), `PersonStatus` (only `Alive` contributes),
  `Brain` (enum stored; **no command dispatch in v1**), `Role` + const
  `RoleDemand` table (Leadership weight 0, test-guarded).
- Finite-value validation at person construction/load (reject non-finite
  skills/competence/coords).
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
- `agenda` states beyond the single reserved `OnDuty` value (no state machine,
  no docking hooks).
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
| 13 | High | Scalar `crew_sum` collapses domains → making a 2nd domain live forces a non-additive formula refactor (breaks the additivity promise). | Accumulate per-domain `domain_sums[d]`; v1 reads only `[Engine]`; new domain = new `CrewMods` field, never a loop change. (§4.3) |
| 14 | High | Controller assumed valid → uses leadership of a stale/dead/other-ship person; silent wrong physics. | `resolve_controller` = `Aboard(this_ship) && Alive`, else `NEUTRAL_LEADERSHIP`; sole path `compute_crew_mods` uses. (§4.1, §4.3) |
| 15 | Med | `NEUTRAL_LEADERSHIP` unspecified + empty-state ≠ identity → proof-test assertions and golden hash become moving targets. | Freeze `NEUTRAL_LEADERSHIP`; choose `f`,`g` so no-controller/empty-roster/default-competence ⇒ `thrust_factor == 1.0` exactly. (§4.3) |
| 16 | High | `abstracted_crew_competence` is an unhashed `crew_mods` input → silent replay divergence when it changes. | Fold it into the ship hash alongside roster/controller. (§5) |
| 17 | Low | Lockstep mutator recomputing `crew_mods` is a redundant 2nd derivation that can disagree with `step()`. | Mutator writes only authoritative columns; `crew_mods` derived solely at top-of-step; `IDENTITY` at reset. (§4.1, §4.2) |
| 18 | High | NaN/non-finite `f64` in skills/competence/`InSpace` poisons `to_bits()` hash, sort keys, equality. | Finite-validate (reject) at construction/load; nothing non-finite ever folded. (§3.1, §5) |
| 19 | Med | `controller=None` vs `Brain::None` vs `Brain::Autopilot` semantics muddled → divergent control behaviour. | Explicit v1 command-authority table; no `Brain` produces commands in v1; ship nav-executor is not a Person brain. (§3.1) |
| 20 | Med | Ambiguous which tick a personnel change affects → non-reproducible crew effects. | Frozen rule: `crew_mods[t]` from start-of-tick-t state; personnel changes take effect t+1. (§4.2) |
| 21 | Low | `Dead` (hashable) conflated with stale (absent) → inconsistent skip/hash treatment. | Four explicit states + three gates (`resolve_person`/`is_effective_crew`/`resolve_controller`); both skipped, hash well-defined. (§3.1) |

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
- **Empty-state identity:** no controller + empty roster ⇒ `thrust_factor == 1.0`
  exactly (and `NEUTRAL_LEADERSHIP` path yields identity).
- **Controller validity:** a controller that is dead, or aboard another ship,
  resolves to `None` ⇒ leadership falls back to `NEUTRAL_LEADERSHIP`.
- **Tested inertness:** flipping *every* `Personality` bit on *every* crew member
  changes neither `crew_mods` nor trajectory in v1 (proves the seam is genuinely
  inert, not fake-future architecture).
- **Leadership-never-in-demand:** `RoleDemand::demand(role)[Leadership] == 0.0`
  for every `Role`.
- **Dead vs stale:** a roster entry pointing to a present-but-`Dead` person and
  one pointing to a stale `PersonId` are both skipped for crew effect, with a
  well-defined (and stable) state hash.
- **Incapacitated contributes zero:** an `Incapacitated` crew member yields the
  same `crew_mods` as an empty slot.
- **Finite-validation:** constructing a person with a non-finite skill (or
  non-finite competence / `InSpace` coord) is rejected, never folded into the
  hash.
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

**Forward-constraint — ship destruction.** v1 has no mid-run ship despawn
(`ShipStore` keeps `slot == row`), so persons cannot be orphaned in v1. When
destruction lands, its handler **must** update aboard-persons (e.g. set
`status = Dead` or relocate them) — a dangling `Location::Aboard(dead_craft)` is
caught by generational-id staleness (lookup returns `None`, no unsafety) but
leaves unreachable rows still folded into the hash. This is a destruction-feature
responsibility, deliberately not built now.

**Persistence note.** There is no save/load system; the world is materialized by
`reset(RunConfig)` and reproduced by replay-from-log. Persons are therefore
spawned at `reset` from config (a new `PersonInit` / roster section in
`RunConfig`, folded into the **config hash** like `CraftInit`), exactly as craft
are today. "Migration" here means the schema/seam change + the
`HASH_FORMAT_VERSION` bump, not a data-file migration.

---

## 12. Recommended implementation order

The seam-isolating sequence (each step independently testable; riskiest gate
first):

1. `CrewMods` + `CrewMods::IDENTITY` + `crew_mods` column; change
   `effective_params(spec, &CrewMods)` at all call sites.
2. **Prove trajectory-equivalence** with identity mods (bit-identical to today)
   *before* anything Person-shaped exists.
3. `PersonId` + `PersonStore` + `EntityRef::Person` + `command_sort_key`
   extension + `HASH_FORMAT_VERSION` 1→2 (golden re-derived).
4. Ship `roster` / `controller` / `abstracted_crew_competence` columns — all
   still identity in effect.
5. `compute_crew_mods` (top-of-step, LOD-gated, per-domain) with **exact
   empty-state identity**; `resolve_controller` + `is_effective_crew` gates;
   finite-validation at construction.
6. Proof tests: controller-swap thrust drop, budget/burn same-`Effective`,
   roster-shuffle hash-invariance, controller-validity, tested inertness,
   leadership-never-in-demand, dead-vs-stale, incapacitated-zero.
7. Only then: scenario fixtures with actual crewed ships.
