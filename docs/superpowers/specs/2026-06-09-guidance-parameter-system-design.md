# Jumpgate v1 — Guidance Parameter System (REVISED design spec)

> **Status: supersedes the prior brief** ("Guidance parameter system v1, pre-review").
> The panel endorsed the *direction* of the prior brief (a three-class determinism
> taxonomy with config-hashed `GuidanceParams`) but corrected it in detail. This
> revision incorporates all six panel must-fixes and the should-fix list, with every
> code claim verified against primary source (`crates/jumpgate-core/src`).
>
> **Must-fixes addressed:** M1 (anti-tunnel: per-ship cruise speed is unbounded → swept
> arrival + reset resolvability guard), M2 (pin `cruise_burn_fraction` default + derivation),
> M3 (precise determinism: cruise-axis goldens *re-derive*, not re-verify; no VERSION bump),
> M4 (`ARRIVAL_RADIUS` is shared world geometry coupling *directly* to hashed pos/vel, not via
> the deferred word 15), M5 (ingest dv-budget INFINITY→fuel-derived is load-bearing, in-scope),
> M6 (exhaustive-destructure `config_hash` so a new unhashed field is a compile error).
>
> **Verified-against-source anchors:** `state_hash` writes only `HASH_FIELD_ORDER` words 1–13
> and ends at `h.finish()` (hash.rs:117–193); words 14/15 are doc-only and never written
> (hash.rs:40–43); `HASH_FORMAT_VERSION = 1` (hash.rs:48); `GOLDEN_ZERO_STATE_HASH = 0xf0dd_a1ba_f433_3735`
> (hash.rs:54); the state_hash golden is `0x532d_07bf_95a2_abc5` (cited as `state_hash_golden_zero_world`,
> hash.rs ~:412); `config_hash` reads `self.field` with no destructure (config.rs:112–152);
> `RunConfig` has 7 fields, no `Default` derive (config.rs:58–70); ephemeris hardcodes
> `mu = G_CANONICAL` (ephemeris.rs:48); `dv_from_fuel` grouping at ingest.rs:171; the live-path
> INFINITY default at ingest.rs:151; the slice-path fuel-derived default at ingest.rs:114.

---

## 1. Scope

**In scope (this spec → implementation plan):**

1. Establish the determinism taxonomy (Class-1 module const / Class-2 config-hashed
   `GuidanceParams` / Class-3 Effective-derived) **and implement it for the guidance
   constants now**: `K_BRAKE`, `cruise_burn_fraction` (replacing absolute `V_CRUISE`),
   `V_ERR_EPS` move into a config-hashed `GuidanceParams` struct.
2. Anti-tunnel, **both halves** (binding user decision):
   (a) replace the point-in-sphere arrival predicate with a deterministic **swept**
   segment-vs-(moving-)sphere closest-approach test; and
   (b) add a `World::reset` **resolvability validation** that rejects ship/dt configs whose
   worst-case (empty-tank) braking cannot bring the craft to rest at the target.
3. The structural `config_hash` fix (M6: exhaustive destructure, no rest-pattern).
4. The shared `tsiolkovsky_dv` helper + the M5 ingest dv-budget reconciliation.
5. **Catalogue** the remaining danger-set as debt (do **not** migrate yet): `ARRIVAL_RADIUS`,
   the Kepler iteration budget, the octave/noise base, and the new `ARRIVAL_SPEED` gate.
6. Split observation (a) — body mass vs hardcoded ephemeris μ — out into a standalone
   **correctness** item (not taxonomy-deferrable).

**Out of scope** — see §13.

---

## 2. Determinism taxonomy (the rule)

Every numeric tuning quantity in the core belongs to exactly one class. The class
determines where it lives and whether it touches a hash.

- **Class-1 — module `const`** (physical law / fixed contract threshold that cannot
  vary per run). Not folded into any hash; covered by **binary identity** (the
  provenance stamp, `provenance.rs` / `replay.rs:88–96`). A Class-1 value reaches hashed
  state only through the trajectories it produces; two runs of the *same binary* agree
  because the constant is baked in. Examples: `G_CANONICAL`, `ARRIVAL_RADIUS`, the Kepler
  iteration budget, the octave base, `ARRIVAL_SPEED` (new).
- **Class-2 — config-hashed `RunConfig` field** (dimensionless run-level *policy* that a
  caller may legitimately vary per run). Folded into `config_hash`. Changing it produces a
  *different config* whose recordings are correctly rejected at the replay config-hash
  guard. Examples: the three `GuidanceParams` fields.
- **Class-3 — derived from already-hashed inputs.** **Rule (explicit clause): a quantity
  that is a pure deterministic function of inputs that are *already folded into a hash*
  needs no separate hash slot of its own.** Its determinism is inherited from its inputs.
  Two sub-cases:
  - *Transitively-pinned snapshot:* `prev_fuel[t] == fuel[t-1]` and (new) `prev_pos[t] ==
    pos[t-1]`, both already hashed at tick `t-1`; the snapshot column is sound to leave
    unhashed (stores.rs:40–45).
  - *Pure scalar derivation:* `tsiolkovsky_dv(...)` and the per-ship cruise speed cap are
    pure functions of `exhaust_velocity`/`dry_mass`/`fuel_capacity` (all in `config_hash`)
    and `fuel_mass` (in `state_hash` word 11). The helper adds **no** new hashed state; its
    *output* may still flow into a hashed field (e.g. `dv_remaining`, word 12) — that is the
    input being hashed, not the helper.

**Class-3 clause, stated crisply:** *If `X = f(inputs)` where `f` is deterministic and every
element of `inputs` is already hashed (in `config_hash` or `state_hash`), then `X` requires
no hash entry; folding it would be redundant, not additive.*

**Taxonomy placement is by ROLE, not by pattern-matching the name.** A value that *looks
like* a tuning constant but is **shared world geometry** (consumed by physics *and* arrival
detection *and* edge-state, not just the guidance policy) must **not** be swept into
`GuidanceParams` (see D3 / `ARRIVAL_RADIUS`).

---

## 3. Decisions (D1..D13)

### D1 — Guidance is run-level *policy*; derive the per-ship cap at the autopilot (REVISED)

**Decision.** The three guidance tunables are Class-2 run-level policy held in a new
`GuidanceParams` struct folded into `config_hash`. The *per-ship* cruise **speed** cap is
**derived inside `autopilot_command`** from `GuidanceParams` + `Effective`, **not** pushed
into `effective_params`.

**Correction to prior D1.** The prior brief claimed "no signature change; the pure seam is
preserved." That is **false**: `autopilot_command` *does* change signature. What the seam
actually protects is the **purity of `effective_params(&BaseSpec) -> Effective`**
(stores.rs:29–36): a `BaseSpec → Effective` accessor with no `dt` and no concept of
guidance/arrival. The dt-dependent, policy-dependent braking command is derived *at the
autopilot* (which already owns the braking law) — not in the param accessor. New signature:

```rust
pub fn autopilot_command(
    nav: NavState,
    pos: Vec3,
    vel: Vec3,
    dest_pos: Vec3,
    dest_vel: Vec3,
    fuel_mass: f64,
    eff: &Effective,
    guidance: &GuidanceParams,   // NEW (D1 / D4)
    dt: f64,                     // NEW (D6 backstop)
) -> (Vec3, f64)
```

`effective_params` is unchanged. `Effective` is unchanged (`dry_mass`, `max_thrust`,
`exhaust_velocity`, `fuel_capacity`; stores.rs:20–26).

### D2 — `K_BRAKE` and `V_ERR_EPS` are exact literal carryovers

`k_brake = 0.5` and `v_err_eps = 1.0e-4` are **bit-exact** copies of the current consts
(autopilot.rs:25, 39). Along these two axes the migration is value-preserving: any run whose
ship specs match produces an identical `state_hash`.

### D3 — `ARRIVAL_RADIUS` stays a Class-1 const; CATALOGUED, not migrated (M4 correction)

**Decision.** `ARRIVAL_RADIUS` (autopilot.rs:19, `1.0e-4` AU, re-exported lib.rs:43) stays a
module-local Class-1 const and is **catalogued** as debt (§12), not migrated into
`GuidanceParams`.

**Correction to prior rationale (M4).** The prior brief argued `ARRIVAL_RADIUS` "reaches
`state_hash` only indirectly via the deferred `prev_inside_dest`." That is wrong: word 15
(`prev_inside_dest`) is doc-only / **unhashed** in v1 (hash.rs:42–43; `state_hash` ends at
word 13, hash.rs:193). The correct statement: `ARRIVAL_RADIUS` couples **directly** to the
hashed `pos`/`vel` because it gates the autopilot's arrival cut (autopilot.rs:63) and the
brake-distance offset (autopilot.rs:75), which shape the trajectory that *is* hashed. It is
also **shared world geometry**, with (today) three live consumers:
autopilot.rs:63, autopilot.rs:75, and world.rs:282 (the `prev_inside_dest` edge recompute).
Under this spec the world.rs:282 consumer **relocates into the swept arrival predicate**
(§5), leaving the autopilot's two consumers; `ARRIVAL_RADIUS` remains the swept test's sphere
radius `R`. It is therefore *not* autopilot policy and must **not** be dropped into
`GuidanceParams` by pattern-matching. **Promotion trip-condition + cost:** see §12.1.

### D4 — `GuidanceParams` struct + placement

```rust
#[derive(Clone, Copy, Debug)]
pub struct GuidanceParams {
    /// Closing-speed cap as a FRACTION of full-tank Tsiolkovsky Δv
    /// (`exhaust_velocity * ln((dry + capacity)/dry)`). Replaces the absolute
    /// V_CRUISE = 2e-3. Default 0.25 (see D5 derivation note).
    pub cruise_burn_fraction: f64,
    /// Brake-early safety margin (< 1). Exact carryover of K_BRAKE.
    pub k_brake: f64,
    /// Velocity-matched deadband (canonical AU/day). Exact carryover of V_ERR_EPS.
    pub v_err_eps: f64,
}

impl Default for GuidanceParams {
    fn default() -> Self {
        GuidanceParams { cruise_burn_fraction: 0.25, k_brake: 0.5, v_err_eps: 1.0e-4 }
    }
}
```

`RunConfig` gains `pub guidance: GuidanceParams` as its **last** field (after `craft`,
config.rs:69) so the existing `config_hash` byte-stream prefix stays byte-identical and only
extends at the tail. `RunConfig` still has **no** `Default` derive, so every full struct
literal must name `guidance` (blast radius §11).

### D5 — `cruise_burn_fraction` default = **0.25** (M2)

**This is a NEW behavioural choice.** The old `V_CRUISE = 2e-3` was an *absolute* speed cap;
the new cap is `cruise_burn_fraction * full_tank_dv`, which is **ship-dependent**. No single
fraction reproduces the old absolute cap across ships, so an honest round-number default is
the right call rather than a false "value-preserving" claim.

Derivation note (verified against the `one_body_one_thrusting_craft` fixture, world.rs:494–500:
`dry = 1e-9`, `cap = 1e-9`, `v_e = 1e-2`):
`full_tank_dv = 1e-2 * ln((1e-9 + 1e-9)/1e-9) = 1e-2 * ln 2 = 6.9315e-3`.
Exact carryover of `2e-3` for *that* ship would need `fraction = 2e-3 / 6.9315e-3 = 0.28854`;
the `replay_equivalence` ship (`v_e = 0.02`, `full_tank_dv ≈ 1.386e-2`) would need a different
fraction. **Pinned default = 0.25**: clean, conservative (it *lowers* the cap relative to the
old reference-ship value → wider anti-tunnel margin), and honestly flagged as the M2
behavioural change. This default co-determines every default-config `config_hash` and every
cruise trajectory; it is a reviewed value, not an incidental one.

### D6 — Reset-time resolvability invariant + dt-aware autopilot backstop (M1, anti-tunnel half b)

See §6 for the full derivation and numerical validation. **Decision:** `World::reset` rejects
any craft whose worst-case (empty-tank) braking cannot resolve the arrival sphere at the
run's `dt`; `autopilot_command` gains `dt` and carries an in-tick `debug_assert` backstop at
the derivation site.

### D7 — Swept arrival detection (M1, anti-tunnel half a)

See §5. **Decision:** replace point-in-sphere `arrival_crossed` with a swept closest-approach
test in the target frame, gated by a relative-speed flyby/rendezvous check; add an unhashed
`prev_pos` ship column.

### D8 — Shared `tsiolkovsky_dv` helper in `math.rs`

See §7. **Decision:** one helper, two callers (the live `dv_from_fuel` and the autopilot's
cruise speed cap), so the law cannot drift; home is `math.rs` (the numeric floor that owns
`G_CANONICAL` and the `to_bits` discipline), keeping the module graph acyclic.

### D9 — Ingest dv-budget reconciliation: live path → fuel-derived (M5)

See §8. **Decision:** change `ingest_commands` (the live `World` path, ingest.rs:151) from
`burn_budget.unwrap_or(f64::INFINITY)` to the fuel-derived default via `tsiolkovsky_dv`,
matching the slice path (`ingest_into`, ingest.rs:114). This is **in-scope** correctness work,
not a 14-day observation.

### D10 — `config_hash` exhaustive destructure (M6)

See §9. **Decision:** rewrite `config_hash` to bind every `RunConfig` field by name with no
rest-pattern, so a future unhashed field is a **compile error**. Fold `guidance` at the tail.

### D11 — `ARRIVAL_SPEED` flyby/rendezvous gate is a Class-1 const, catalogued

The swept test needs a relative-speed threshold to distinguish a velocity-matched rendezvous
(fires Arrival) from a fast flyby that merely grazes the sphere (must **not** fire).
**Decision:** `ARRIVAL_SPEED` is a new Class-1 `pub const` in events.rs, **catalogued** as a
tunable (§12.3), not promoted to config in v1. Rationale: its value is TBD-by-measurement (it
must be tuned exactly as `K_BRAKE`/`V_CRUISE` were), it affects only **Arrival event timing**
— never `state_hash` — and keeping it a const holds the `config_hash` surface minimal. (See
§5 for the proposed starting value and the open item.)

### D12 — Body mass vs hardcoded ephemeris μ is a CORRECTNESS issue, not a deferrable tunable

See §12.4. **Decision:** observation (a) is split out as a standalone two-body-consistency
correctness item; it is not part of the taxonomy-deferred catalogue.

### D13 — Config-hash drift-lock anchor: mirror `state_hash` discipline

**Decision.** Add a `CONFIG_FIELD_ORDER` doc block (mirroring `HASH_FIELD_ORDER`) and a pinned
**config-hash golden** test, mirroring `GOLDEN_ZERO_STATE_HASH`. Rationale: the M6 destructure
catches a *new unhandled field* but not a *reorder or encoding change of existing folds*; the
replay guard cannot either, because it compares `rec.config_hash` against
`rec.config.config_hash()` — the *same function on both sides* (replay.rs ~:108), so uniform
encoding drift moves both identically and passes. Only a pinned golden detects fold-order /
encoding drift. `config_hash` is a trust-boundary stamp (provenance/replay), so it deserves the
same anchor `state_hash` has; "intentionally volatile" would leave it with a hole `state_hash`
does not have.

---

## 4. `GuidanceParams` wiring (Class-2)

- **Definition + Default:** config.rs (D4/D5).
- **Folded into `config_hash`:** three `to_bits` words at the tail (§9).
- **Read at the autopilot call site:** world.rs:222 passes `&self.config.guidance` and `dt`
  (the `World` already owns `config` and `dt = self.dt.get()`, world.rs:177). **No new `World`
  field.**
- **Consts deleted:** `K_BRAKE`, `V_CRUISE`, `V_ERR_EPS` are removed from autopilot.rs
  (autopilot.rs:25, 32, 39). `ARRIVAL_RADIUS` is **kept** (D3). Inside `autopilot_command`:
  `2.0 * K_BRAKE` → `2.0 * guidance.k_brake`; `V_CRUISE.min(v_brake)` →
  `(guidance.cruise_burn_fraction * full_tank_dv).min(v_brake)`; `V_ERR_EPS` →
  `guidance.v_err_eps`, where
  `full_tank_dv = tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, eff.fuel_capacity)`
  (FULL tank, so the cap is **trajectory-constant**, not shrinking as fuel burns).
- **Multiply grouping (FP non-associativity):** the cruise term is
  `(guidance.cruise_burn_fraction * full_tank_dv)` with plain `*`, and `full_tank_dv` uses
  `(eff.exhaust_velocity) * ((dry + cap)/dry).ln()` (left-to-right product, ratio inside
  `.ln()`, **no `mul_add`**). The `2.0 * guidance.k_brake * a_max * (d - ARRIVAL_RADIUS)`
  product stays left-to-right exactly as today (autopilot.rs:74–75).
- **Precondition:** `dry_mass > 0` and `fuel_capacity > 0` so the `ln` ratio `> 1` (handled in
  the helper, §7, with a guard + `debug_assert`).

---

## 5. Swept arrival detection (D7) — replacing point-in-sphere `arrival_crossed`

### 5.1 Motivation

`arrival_crossed` (events.rs:55–58) is a point sample: `pos.sub(dest_pos).length() <=
ARRIVAL_RADIUS`. With `R = 1e-4` AU and any non-trivial per-tick chord a craft can pass
entirely through the arrival sphere between two tick boundaries and never sample a point
inside. **The naive bound `V_CRUISE*dt < 2R` is already violated today** (`2e-3 * 0.25 = 5e-4
> 2e-4`); current tests pass only because the braking law shrinks the near-arrival step. The
swept test makes detection robust to step size on its own. This is exactly the documented
contingency in the braking-law spec
(`docs/superpowers/specs/2026-06-08-autopilot-braking-law-design.md`, "add a swept
segment-to-point closest-approach test in events.rs").

### 5.2 The closest-approach formula (target frame, plain f64)

Work in the **target frame** so a moving body falls out for free (mirrors the autopilot,
which brakes in `rel_vel = vel - dest_vel`). For detection tick `T` (`T >= 1`, since detection
runs with `next`, world.rs:271, so `Tick(T-1)` never underflows):

```
C_prev = target position at tick T-1   // Position(p): p ; Entity(Body): ephem.body_pos(eidx, Tick(T-1))
C_now  = target position at tick T      // Position(p): p ; Entity(Body): ephem.body_pos(eidx, Tick(T))
A = prev_pos.sub(C_prev)                // craft offset from target at chord start
B = pos.sub(C_now)                      // craft offset from target at chord end
d = B.sub(A)
dd = d.dot(d)
R  = ARRIVAL_RADIUS

if dd <= DD_EPS {                       // DEGENERATE / RENDEZVOUS branch (5.3)
    min_sq = B.dot(B)                   // endpoint point-in-sphere test
} else {
    let t = (-(A.dot(d)) / dd).max(0.0).min(1.0);   // clamp closest-approach param to [0,1]
    let closest = A.add(d.scale(t));
    min_sq = closest.dot(closest);
}
inside_now = (min_sq <= R * R) && (rel_speed <= ARRIVAL_SPEED);   // 5.4 speed gate
fire = inside_now && !prev_inside;       // once-only edge latch (existing prev_inside_dest)
```

All ops are `Vec3::sub/dot/add/scale` plus scalar `*`/`/`/`.max`/`.min` (math.rs:43–73) — **no
`f64::mul_add`**. `R * R` and `d.scale(t)` are left-to-right, matching the anti-FMA convention.

### 5.3 The `dd → 0` branch is the rendezvous, not an edge case

In a velocity-matched rendezvous (the *intended* success path) `rel_vel ≈ 0`, so the
target-frame chord is ~stationary: `A ≈ B`, `d ≈ 0`, `dd ≈ 0`. The unguarded
`t = -(A·d)/dd = 0/0 = NaN`, and `NaN <= R*R` is `false` — the true arrival would **never
fire**. The `dd <= DD_EPS` branch falls back to the endpoint test `B.dot(B) <= R*R`, the old
(correct, for slow approaches) behaviour. This is the common arrival path, not a corner.

### 5.4 Flyby vs rendezvous (the speed gate)

A fast flyby produces a long chord that can clip the sphere for one tick without the craft
stopping there. Gating `inside_now` on `rel_speed = vel.sub(dest_vel).length() <=
ARRIVAL_SPEED` suppresses Arrival on such a pass while still firing for a genuine
velocity-matched arrival. **There is no code construct literally named a "reset resolvability
check"** in the prior framing of the gate; the real reference is the autopilot's
velocity-matched deadband `V_ERR_EPS` (now `guidance.v_err_eps`) and the rendezvous-vs-flyby
framing of the braking-law spec. (The "resolvability check" of the user decision is the
*separate* `World::reset` guard of §6, not this gate.)

`ARRIVAL_SPEED` is a new Class-1 `pub const` in events.rs (D11). **Proposed starting value:**
the old `V_CRUISE` magnitude `2e-3` (the closing-speed envelope the law never exceeds while
braking) or a small multiple of `v_err_eps` (`1e-4`); **pin by measurement** during TDD (§10).

### 5.5 State, ordering, and the `prev_pos` column

A new `pub prev_pos: Vec<Vec3>` ship column snapshots the chord start. It rides the **exact**
deferred/unhashed tier as `prev_fuel`/`prev_inside_dest` (stores.rs:40–45):

- `ShipStore` (stores.rs:46–56): add `pub prev_pos: Vec<Vec3>` after `prev_inside_dest`; extend
  the doc block to list it as a copy-forward edge-detect snapshot that is **not** folded into
  `state_hash` and is transitively pinned (`prev_pos[t] == pos[t-1]`, hashed at word 9 at tick
  `t-1`).
- `ShipStore::empty()` (stores.rs:70–82): `prev_pos: Vec::new()`.
- `ShipStore::push()` (stores.rs:87–103): `self.prev_pos.push(pos);` so the SoA arrays stay
  length-parallel and `slot == row` holds; doc: `prev_pos` initialised to current `pos`.
- `World::reset` (world.rs:100–123): init `prev_pos: Vec::new()` in the `ShipStore` literal and
  `ships.prev_pos.push(c.pos);` in the per-craft loop, mirroring `prev_fuel`. At tick 0
  `prev_pos == pos` → the chord is zero-length → no spurious first-step fire.
- `World::step` copy-forward loop (world.rs:277–286): add
  `self.ships.prev_pos[ci] = self.ships.pos[ci];`. **Order matters:** detection
  (world.rs:271) runs *before* this loop, so at detection tick `T` the chord is
  `[prev_pos = pos(T-1), pos = pos(T)]`. `prev_inside_dest` is **kept** as the once-only latch
  and fed the swept verdict (lowest-risk; do not retire it for v1 — see open items).

### 5.6 events.rs changes

- Replace `arrival_crossed(pos, dest_pos, prev_inside)` (events.rs:55–58) with
  `arrival_swept(prev_pos, pos, c_prev, c_now, rel_speed, prev_inside) -> bool` implementing
  §5.2.
- In `detect_boundary_events` (events.rs:69–106), for a `Seeking` craft resolve the dest at
  **both** `Tick(T-1)` and `Tick(T)` (`c_prev`/`c_now`; for `NavDest::Position(p)` both `= p`),
  read `prev_pos = ships.prev_pos[idx]` and `pos = ships.pos[idx]`, compute
  `rel_speed = ships.vel[idx].sub(dest_vel).length()` (`dest_vel` via `ephem.body_vel` for a
  Body — ephemeris.rs:103 — or `Vec3::ZERO` for a fixed `Position`), then call `arrival_swept`.
- `ARRIVAL_RADIUS` is read here as `R` (relocating the world.rs:282 geometry consumer; D3).

### 5.7 Determinism (swept arrival)

**`prev_pos` is a pure copy-forward of `pos`** (never independently mutated), so
`prev_pos[t] == pos[t-1]`, already hashed as `HASH_FIELD_ORDER` word 9 at tick `t-1`
(hash.rs:145–147). It is therefore Class-3 (transitively pinned) and is **appended as a
doc-only word 16** in the same deferred tier as words 14/15 — **not** written by `state_hash`.
This is the *identical* argument stores.rs:40–45 already makes for `prev_fuel`, and those words
have never forced a re-derive at `HASH_FORMAT_VERSION = 1`.

**Net for swept arrival:** no `HASH_FORMAT_VERSION` bump, no `HASH_FIELD_ORDER` change, no
`state_hash`/`config_hash` change, tick-0 golden `0x532d_07bf_95a2_abc5` unchanged. The only
behavioural delta is the **timing/whether of `Arrival` *events***. The `Arrival` event is not
part of `state_hash` (only the per-tick state chain is). If any golden pins the **event
stream**, it rebaselines; the per-tick state-hash chain does not move. (No committed event-
stream golden exists — §10.5.)

---

## 6. Reset-time resolvability invariant (D6) — anti-tunnel half (b)

### 6.1 Statement

For every craft in a `RunConfig`, `World::reset` MUST reject the config unless the craft's
worst-case (empty-tank) braking can stop inside `ARRIVAL_RADIUS` without tunnelling:

```
a_max_empty * dt^2  <  ARRIVAL_RADIUS / (2 * K_BRAKE)
```

where `a_max_empty = base_max_thrust / base_dry_mass` (thrust acceleration at an empty tank,
the **largest** a craft ever sees because `a_max = max_thrust / (dry + fuel)` is maximised at
`fuel = 0` — autopilot.rs:72), `dt = cfg.dt.get()`, `R = ARRIVAL_RADIUS = 1e-4`, and
`K_BRAKE = 0.5`. With `K_BRAKE = 0.5` this collapses to the clean form:

```
(base_max_thrust / base_dry_mass) * dt^2  <  ARRIVAL_RADIUS
```

The guard additionally rejects `base_dry_mass <= 0` (division) and any non-finite
`a_max_empty`.

> **Note (RESOLVED, Q2):** `k_brake` is a *fleet-wide, per-run* policy in `GuidanceParams`
> (Class-2), shared by every craft in a run; there is **no per-ship `k_brake`** in v1 ("brake
> differently" = run a different fleet/config). The reset guard reads the run's actual
> `cfg.guidance.k_brake` (NOT a hardcoded `0.5`), so the bound is correct for any fleet,
> including one that sets a non-default brake margin. The guard still takes the worst case
> *over fuel* (empty tank), which is orthogonal to the policy value.

### 6.2 Derivation

The autopilot brakes with a sqrt deceleration profile, commanding closing speed
`v_des = min(cruise_cap, v_brake)` where `v_brake = sqrt(2*K_BRAKE*a_max*(d - R))`
(autopilot.rs:75). In continuous time the craft tracks this ramp — following `v_brake(d)`
requires deceleration exactly `K_BRAKE * a_max < a_max`, always feasible. The failure is
purely *discrete*: velocity changes by at most `a_max*dt` per tick and position advances by
`v*dt` per tick. Near the sphere, the smallest distance at which a discrete step can still
resolve the ramp is `u* = d - R = 2*K_BRAKE*a_max*dt^2`; at that distance the commanded
crossing speed is `v_cross = v_brake(u*) = 2*K_BRAKE*a_max*dt`. For a tick boundary to land
*inside* the sphere rather than skip across it, the per-tick step at crossing must fit the
band: `v_cross * dt < R`. Substituting gives `2*K_BRAKE*a_max*dt^2 < R`, i.e. the bound. The
cruise cap (`cruise_burn_fraction * full_tank_dv`) plays **no role**: `v_brake → 0` as
`d → R`, so the cap only bounds far-field cruise, never the crossing speed.

### 6.3 Numerical confirmation (conservative SUFFICIENT bound)

A 1-D arrival-latched velocity-Verlet sim of the exact braking law (dt = 0.25) confirms the
bound is a correct **sufficient** condition:

| a_max / bound | a_max·dt² / R | v_cross·dt | < R? |
|---|---|---|---|
| 0.10 | 0.10 | 3.0e-5 | yes |
| 0.50 | 0.50 | 5.0e-5 | yes |
| 0.90 | 0.90 | 5.0e-5 | yes |
| 1.00 (bound) | 1.00 | 1.0e-4 | = R |

Below the bound the per-tick crossing step stays under `R` (no tunnel); the crossing-speed
envelope peaks at exactly `R` *at* the bound. Catastrophic clean tunnel (craft escapes to the
far side) first appears at `a_max ≈ 1.74×` the bound, phase-dependent tunnels at `~2.5×`. The
~1.7× margin is **intentional**: the guard must reject on the worst-case discretisation phase,
not the lucky phase, so a conservative sufficient bound is the right shape.

### 6.4 Worked examples (dt = 0.25)

- **Test ship** `one_body_one_thrusting_craft` (world.rs:494–500: `dry = 1e-9`,
  `max_thrust = 1e-12`): `a_max_empty = 1e-3`, `a_max·dt² = 6.25e-5 < R = 1e-4` → **ACCEPT**
  (still reaches tick 0; this fixture must keep passing).
- **High-thrust ship** (`dry = 1e-9`, `max_thrust = 1e-11`, 10× thrust): `a_max_empty = 1e-2`,
  `a_max·dt² = 6.25e-4 ≥ R` → **REJECT** (`v_cross·dt = 5e-4 > R`; a tick can skip the band at
  some phases). This is the M1 regression case (§10.1).
- **Golden-zero config** `cfg_with_craft_x` (hash.rs:213–214: `dry = 1`, `max_thrust = 0.1`,
  `dt = 0.01`): `a_max·dt² = 0.1 * 1e-4 = 1e-5 < R` → **ACCEPT**, so the golden-zero tests
  still reach tick 0 unchanged.

### 6.5 Enforcement

`World::reset` (world.rs:81) changes signature:

```rust
pub fn reset(cfg: RunConfig) -> Result<(World, ConfigHash), ResetError>
```

Before any store is built, loop `for (i, c) in cfg.craft.iter().enumerate()`:
`let dry = c.spec.base_dry_mass; let a_max_empty = c.spec.base_max_thrust / dry;`
`let dt = cfg.dt.get(); let limit = ARRIVAL_RADIUS / (2.0 * cfg.guidance.k_brake);`  // per-run policy (Q2)
reject the first violating craft with:

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResetError {
    Unbrakable { craft_index: usize, a_max_empty: f64, dt: f64, limit: f64 },
}
```

rejection predicate: `!(dry > 0.0 && a_max_empty.is_finite() && a_max_empty * dt * dt < limit)`.
On success wrap the existing return in `Ok((world, hash))`. `Display` reports `craft_index`,
`a_max_empty * dt^2`, the `limit`, and the remediation (lower `max_thrust`, raise `dry_mass`,
or shrink `dt`). A silent tunnel becomes a loud config error.

**`ResetError` placement.** `World::reset` is part of the recorded contract surface
(`replay.rs:47,105` call it and assert its returned hash; replay crosses the trust boundary).
Place `ResetError` in `world.rs` next to `reset` and **re-export it from `lib.rs`** alongside
`World` (lib.rs:62) so the gym/FFI layer can match on it. (If a later contract-surface task
moves public error types into `contract.rs`, `ResetError` moves with them; not required for v1.)

### 6.6 dt-aware autopilot backstop

At the derivation site (autopilot.rs:75, after `a_max` is computed):

```rust
debug_assert!(
    a_max * dt * dt < ARRIVAL_RADIUS / (2.0 * guidance.k_brake),
    "unbrakable config reached autopilot: a_max*dt^2={} >= R/(2K)={}",
    a_max * dt * dt, ARRIVAL_RADIUS / (2.0 * guidance.k_brake)
);
```

This uses the **live** `a_max` (current fuel), which is `<=` the empty-tank value the reset
guard checked, so it can only fire if reset was bypassed or effective params drift above base
(a future LOD/component-mod hazard). It is compiled out of release and feeds no arithmetic, so
it cannot affect the hash.

### 6.7 Determinism (resolvability guard)

**Determinism-neutral.** The guard runs in `World::reset` *before* tick 0 and reads only
config inputs already in `config_hash`: `cfg.dt` (config.rs:116), `base_max_thrust`
(config.rs:138), `base_dry_mass` (config.rs:137), plus the default `k_brake`. It introduces no
persisted/evolving state. `state_hash`, `HASH_FIELD_ORDER`, `HASH_FORMAT_VERSION`,
`config_hash`, and both goldens are unchanged; no recorded-run rebaseline. The autopilot's new
`dt` parameter feeds only the `debug_assert`; the `v_brake`/`v_des` arithmetic does not consume
`dt`, so every config that passes the guard produces bit-identical trajectories. The only
behavioural change is that some previously-accepted-but-broken configs now return `Err` instead
of silently tunnelling; any config that previously produced a hash still produces the identical
hash.

---

## 7. Shared `tsiolkovsky_dv` helper (D8)

```rust
// crates/jumpgate-core/src/math.rs
/// Ideal-rocket (Tsiolkovsky) Δv: Δv = v_e · ln((dry + prop) / dry).
///
/// Precondition: `dry_mass > 0.0` (a massless dry hull has unbounded Δv and is
/// non-physical). Returns 0.0 for `dry_mass <= 0.0` or `propellant_mass <= 0.0`
/// (no tank, no budget) rather than NaN/Inf; a `debug_assert!` traps producer bugs.
///
/// Pinned numerics: the product is LEFT-TO-RIGHT (`v_e * ln(...)`), the mass ratio
/// is formed inside the `ln` argument, NO `mul_add`/FMA — the exact grouping the
/// recorded hashes were captured under (matches ingest.rs:171).
pub fn tsiolkovsky_dv(exhaust_velocity: f64, dry_mass: f64, propellant_mass: f64) -> f64 {
    debug_assert!(dry_mass > 0.0, "tsiolkovsky_dv requires dry_mass > 0");
    if dry_mass <= 0.0 || propellant_mass <= 0.0 {
        0.0
    } else {
        exhaust_velocity * ((dry_mass + propellant_mass) / dry_mass).ln()
    }
}
```

**Home: `math.rs`**, not `ship.rs`. The Δv form is a pure scalar with no `Effective`/store
dependency; `math.rs` already owns `G_CANONICAL` and the `to_bits` field-order discipline
(it is the lowest common ancestor of both callers and keeps the module graph acyclic).
`ship.rs` would force `ingest.rs` and `autopilot.rs` to depend on the dynamics module for a
scalar they do not otherwise need.

**Two callers, same grouping (cannot drift):**

| Caller | `propellant_mass` | Source |
|---|---|---|
| `dv_from_fuel` (live nav budget, ingest.rs:164) | `ship.fuel_mass[idx]` — LIVE tank | current fuel |
| Autopilot cruise **speed** cap (new, in `autopilot_command`) | `eff.fuel_capacity` — FULL tank | spec |

Re-pointing `dv_from_fuel` at the helper is **bit-for-bit hash-neutral**: same operands, same
grouping as the inline form at ingest.rs:171.

**Export:** add `tsiolkovsky_dv` to the `math` re-export (lib.rs:54:
`pub use math::{G_CANONICAL, Vec3, tsiolkovsky_dv};`). `V_CRUISE` is **deleted** (D4), so the
prior "add `V_CRUISE` re-export" question is **moot**; the cruise cap is computed inside
`autopilot_command`, not exposed.

---

## 8. Ingest dv-budget reconciliation (D9 / M5)

The two ingestion paths default a missing `burn_budget` differently: `ingest_into` uses
`dv_from_fuel` (fuel-derived, ingest.rs:114) while `ingest_commands` — the live `World` path —
uses `f64::INFINITY` (ingest.rs:151).

**Decision: the live path is changed to fuel-derived via the shared helper.** This is
load-bearing, not incidental: the Δv budget is the **sole** feasibility guard the autopilot
consults (autopilot.rs:63: `dv_remaining <= 0.0` is the only Δv stop), and it is hashed
verbatim into `dv_remaining` (`HASH_FIELD_ORDER` word 12, hash.rs:184) the instant a command
is ingested. An `INFINITY` budget therefore (a) lets a craft burn past its tank's true
achievable Δv with no physical stop but an empty tank, and (b) writes a non-finite word into
the per-tick state hash. Making both paths fuel-derived makes the recorded policy
path-independent.

**Implementation note.** `ingest_commands` does not currently resolve the craft's dense row
before `set_nav`, whereas `ingest_into` holds `idx` from `ship.index_of`. The change requires
resolving the craft's `Effective` + current `fuel_mass` for the `CraftId` before `set_nav`
(`World` already exposes `craft_fuel` and `effective_params` access). The default becomes
`burn_budget.unwrap_or_else(|| tsiolkovsky_dv(eff.exhaust_velocity, eff.dry_mass, fuel_mass))`.

**Determinism.** This **is** a behavioural `state_hash` change for the live no-budget path: a
different value lands in `dv_remaining` (word 12) for every no-budget `World`-path command, so
`state_hash` moves for any run exercising that path → those recordings rebaseline. But: **no**
`HASH_FORMAT_VERSION` bump (the field set and order are unchanged, only the stored *value*
differs) and **no** golden-zero change (the golden tests use `Idle` nav, never ingest a
no-budget command). See §10.5: no committed recording exercises this path, so the rebaseline is
forward-discipline only.

---

## 9. `config_hash` structural fix (D10 / M6)

Rewrite `config_hash` (config.rs:112–152) to bind every field by name via exhaustive
destructure with **no** rest-pattern, so a future added field is a **compile error** until
folded:

```rust
pub fn config_hash(&self) -> ConfigHash {
    // No rest-pattern: a NEW field added without folding it is a COMPILE ERROR.
    let RunConfig {
        master_seed, dt, softening, substep_cfg,
        ephemeris_window, bodies, craft, guidance,
    } = self;
    let mut h = ConfigFnv::new();
    h.write_u64(*master_seed);
    h.write_u64(dt.bits());
    h.write_u64(softening.to_bits());
    h.write_u64(substep_cfg.accel_ref.to_bits());
    h.write_u64(substep_cfg.max_substeps as u64);
    h.write_u64(*ephemeris_window);
    h.write_u64(bodies.len() as u64);
    h.write_u64(craft.len() as u64);
    for b in bodies { /* mass + 6 elements, byte-identical to config.rs:126-134 */ }
    for c in craft  { /* 4 spec + 3 pos + 3 vel + fuel, byte-identical to config.rs:136-150 */ }
    // GUIDANCE at the TAIL: existing byte stream stays byte-identical, only extends.
    h.write_u64(guidance.cruise_burn_fraction.to_bits());
    h.write_u64(guidance.k_brake.to_bits());
    h.write_u64(guidance.v_err_eps.to_bits());
    ConfigHash(h.finish())
}
```

Add the `CONFIG_FIELD_ORDER` doc + pinned `config_hash` golden (D13). Add three per-field
perturbation tests (§10.4).

---

## 10. TDD test plan

Tests first, then implementation; each test names the decision it pins.

### 10.1 High-`a_max` empty-tank reset regression (M1, the headline test)

`reset_rejects_unbrakable_high_thrust_craft`: build a craft `dry = 1e-9`, `max_thrust = 1e-11`
at `dt = 0.25` (10× the passing fixture; `a_max_empty = 1e-2`, `a_max·dt² = 6.25e-4 ≥ R`).
Assert `World::reset(cfg)` returns `Err(ResetError::Unbrakable { craft_index: 0, .. })`.
Companion `reset_accepts_resolvable_thrusting_craft`: the real `one_body_one_thrusting_craft`
fixture (`a_max·dt² = 6.25e-5 < R`) returns `Ok`.

### 10.2 Swept arrival behaviour

- `swept_fires_when_point_test_would_miss`: a chord that passes *through* the sphere between
  ticks (no endpoint inside) at low `rel_speed` → fires.
- `fast_flyby_does_not_fire`: same geometric clip but `rel_speed > ARRIVAL_SPEED` → does not
  fire (pins the speed gate / D11).
- `velocity_matched_rendezvous_fires`: `rel_vel ≈ 0` (the `dd → 0` endpoint branch) → fires
  (pins §5.3; without the branch this would NaN-out and fail).
- `moving_body_rendezvous_fires`: a craft co-moving with an orbiting `Body`, resolving
  `C_prev`/`C_now` via `ephem.body_pos` at `Tick(T-1)`/`Tick(T)` and `dest_vel` via
  `ephem.body_vel` → fires; confirms the quantized-tick-endpoint target approximation is
  adequate at `R`-scale geometry.
- `tick0_zero_length_chord_does_not_fire`: at reset `prev_pos == pos` → no spurious fire.
- Replace/repurpose the in-module tests `arrival_crossing_contract_documented` and
  `arrival_crossed_uses_arrival_radius_constant` (events.rs:140–174) for the new signature.

### 10.3 Guidance migration / value-preservation

- `k_brake_default_is_exact_carryover` and `v_err_eps_default_is_exact_carryover`: the autopilot
  output for a fixed scenario is bit-identical to the pre-migration const path (D2).
- `cruise_cap_is_fraction_of_full_tank_dv`: assert the cap equals
  `cruise_burn_fraction * tsiolkovsky_dv(v_e, dry, capacity)` for the thrusting fixture
  (`0.25 * 6.9315e-3 = 1.733e-3`), and that it is trajectory-constant as fuel burns (D5/D8).

### 10.4 `config_hash` belt-and-suspenders (behind the destructure)

`changing_cruise_burn_fraction_changes_hash`, `changing_k_brake_changes_hash`,
`changing_v_err_eps_changes_hash` (mutate `c.guidance.<field>`, assert
`assert_ne!(sample().config_hash(), c.config_hash())`). Plus `config_hash_golden` pinning the
`sample()` config-hash value (D13).

### 10.5 Determinism regression guards (must stay green / be re-derived)

- `golden_zero_state_hash` and `state_hash_golden_zero_world` (`0x532d_07bf_95a2_abc5`): **stay
  green unchanged** (Idle nav never invokes the cruise cap or the no-budget ingest path;
  golden-zero config passes the reset guard, §6.4).
- `record_then_replay_is_bit_identical` (replay_equivalence.rs): stays green (record + replay
  same build).
- **Re-derived (not re-verified):** the physics arrival/rendezvous property tests
  (`physics_sanity.rs`: `transfer_to_moving_body_rendezvous`, `transfer_arrival_tick_is_*`)
  exercise the cruise-axis change (absolute → per-ship cap, lower cap → more ticks to arrive
  within `max_ticks`); re-run and re-pin their arrival ticks / property thresholds after the
  default is fixed. Their asserts are *properties* (rel-speed-at-arrival ≪ v_circ), not pinned
  hashes, so they are expected to hold but must be re-confirmed empirically.
- **Recordings:** no committed/serialized recording or event-stream golden exists; every
  `Recording` is built and consumed in-process via `record_run` within the same test and build
  (replay_equivalence.rs:111,119,161,184). Therefore "old recordings rebaseline" is
  **forward-discipline only, zero current migration cost** (§12 / §11).

---

## 11. Migration & blast radius

Concrete call sites (workspace-wide grep, not "update callers"):

**`World::reset` → `Result<(World, ConfigHash), ResetError>`** (24 sites):
- Production: replay.rs:47, replay.rs:105 (use `?` / `.expect("config's own hash")`,
  preserving the existing reset-hash assertions).
- Tests (use `.expect("resolvable config")`): hash.rs:245,246,253,254,264,283,295,397,411,418;
  world.rs:513,524,560,609,635; physics_sanity.rs:112,147,183,237,336;
  replay_equivalence.rs:43,83.

**`autopilot_command` (+`guidance: &GuidanceParams`, +`dt: f64`)** (8 sites):
- Production: world.rs:222 (pass `&self.config.guidance`, `dt`).
- In-module tests: autopilot.rs:120,135,167,205,233,254,268,275 (pass
  `&GuidanceParams::default()` and the test's `dt`/`ENGINE_DT`).

**`RunConfig { .. }` full literals needing `guidance: GuidanceParams::default()`** (7):
config.rs:160 (`sample`), replay_equivalence.rs:9 (`base_config`), physics_sanity.rs:66
(`star_config`), physics_sanity.rs:315, world.rs:425 (`one_body_one_craft`), world.rs:473
(`one_body_one_thrusting_craft`), hash.rs:214 (`cfg_with_craft_x`). The 8th literal,
replay_equivalence.rs:162, uses `..rec.config.clone()` → **no edit**.

**Const deletions:** remove `K_BRAKE`, `V_CRUISE`, `V_ERR_EPS` (autopilot.rs:25,32,39); keep
`ARRIVAL_RADIUS`. Update the autopilot module doc (autopilot.rs:1–13) and the stale
`V_CRUISE`-referencing prose comments (world.rs:496–499 fixture comment; absolute→fractional).

**Exports (lib.rs):** add `GuidanceParams` (config re-export), `tsiolkovsky_dv` (math re-export
line 54), `ResetError` (world re-export line 62). `ARRIVAL_RADIUS` stays exported (line 43);
`V_CRUISE` is removed (was not re-exported, so no export edit).

**No `jumpgate-gym` impact:** no `RunConfig` literal or `World::reset`/`autopilot_command` call
exists in the gym crate (grep clean).

---

## 12. Catalogue — deferred danger-set debt

These are **not migrated** in v1 (binding scope decision). Each is recorded with its class,
live consumers, and the exact trip-condition + determinism cost of promotion.

### 12.1 `ARRIVAL_RADIUS` — shared world geometry (Class-1, DEFERRED)

`autopilot.rs:19`, `1.0e-4` AU, re-exported lib.rs:43. **Live consumers:** autopilot.rs:63
(arrival cut), autopilot.rs:75 (brake-distance offset), and the swept arrival predicate `R`
(events.rs, relocated from world.rs:282 under this spec). It couples **directly** to hashed
`pos`/`vel` (not via the deferred word 15 — M4). **Trip-condition:** promote the first time a
run needs a per-scenario arrival tolerance (mixed station-docking vs flyby radii, or a scale
change that makes `1e-4` AU wrong for some bodies). **Promotion cost:** fold into `RunConfig` +
`config_hash`; *if* the swept latch is then derived from stored prev-inside state, fold
`prev_inside_dest` into `state_hash` (words 14/15) → **`HASH_FORMAT_VERSION` bump + golden
re-derive + recorded-run rebaseline**. Because it is shared geometry, it goes into a
geometry/world config section, **not** `GuidanceParams`.

### 12.2 Kepler iteration budget (Class-1, DEFERRED)

The Newton/iteration count in the Kepler solve (ephemeris.rs) is a Class-1 const today.
Promotion would change every multi-body orbit and is a `config_hash` fold (Class-2) + full
recorded-run rebaseline; defer until a scenario needs per-run solver fidelity.

### 12.3 Octave / noise base + `ARRIVAL_SPEED` (Class-1, DEFERRED/NEW)

The procedural octave base (where applicable) and the new `ARRIVAL_SPEED` gate (D11) are
Class-1 consts. `ARRIVAL_SPEED` affects only Arrival **event** timing (never `state_hash`), so
even a future promotion is event-stream-only, not a state golden re-derive.

### 12.4 CORRECTNESS issue (split out, NOT taxonomy-deferrable) — body mass vs hardcoded μ

**Title:** Body mass is hashed and drives craft gravity, but a body's own orbit ignores it
(hardcoded μ).

Per-body `mass` is folded into `config_hash` (config.rs:127) and `state_hash` word 7
(hash.rs:136), and it **does** drive *craft* gravitational acceleration
(`integrator.rs:31`: `G_CANONICAL * mass * inv`, fed from world.rs:185–186). However the
on-rails ephemeris hardcodes `mu = G_CANONICAL` for *all* bodies (ephemeris.rs:48,66), so a
body's own Kepler orbit propagates as if it had the central μ regardless of its declared mass.
Net: a heavy planet pulls craft correctly but orbits as though massless (no
`μ = G(M_sun + m_body)` two-body correction, no barycentre offset). **Scope it as a
two-body-consistency defect, NOT "mass is inert"** — mass is live on the craft-gravity path and
inert only on the body-orbit path. The fix (use `μ = G·(M_central + m_body)`) is a
behavioural change to every multi-body orbit (`HASH_FORMAT`-neutral but recorded-run
rebaseline) and is a **product decision** (correctness target), filed separately and not
settleable from code alone (§14).

> **RESOLVED 2026-06-09:** varied star types are a design goal, so the fix direction is
> chosen — `μ = G·(M_central + m_body)`. Tracked as **jumpgate-fca8c9e0c0** (P2 bug),
> implemented as a *separate* task from the guidance-param work. Full barycentric wobble
> remains deferred.

---

## 13. Out of scope

- Migrating `ARRIVAL_RADIUS`, the Kepler iteration budget, or the octave base into config
  (catalogued as debt, §12; binding scope decision).
- Sweeping the *target body's* sub-tick path during a chord (the swept test uses the body's
  quantized tick-endpoint positions `C_prev`/`C_now`; adequate at `R`-scale geometry — flag
  for empirical confirmation in the moving-body test, §10.2).
- Retiring `prev_inside_dest` in favour of deriving inside-prev from the chord (kept as the
  once-only latch for v1; lowest-risk).
- A per-tick / re-validation hook for when *effective* params drift above the *base* values the
  reset guard checked (future LOD / component-mod work; the autopilot `debug_assert` is the v1
  backstop).
- The body-mass/μ fix itself (filed as a separate correctness issue; only the catalogue entry
  is in scope here).
- The deadband oscillation property `v_err_eps * dt < R` is a *distinct, already-satisfied*
  property (`1e-4 * 0.25 = 2.5e-5 < 1e-4`), deliberately **not** folded into the reset guard.
- **Fleet/taskforce structure** (see `docs/glossary.md`). `GuidanceParams` is *fleet-wide*
  policy; in v1 there is no fleet aggregate, so it lives **run-level** in `RunConfig` (one
  implicit fleet per run). When the **fleet** concept lands it migrates to a per-fleet
  attribute and the reset guard reads `craft.fleet.guidance.k_brake`; the **taskforce**
  layer (command delays at the `ingest.rs` seam) is a further-future echelon. None of this
  changes v1; the run-level placement is forward-compatible (the migration is RunConfig →
  Fleet, the same Class-2 config-hashed value moving down one level).

---

## 14. Unresolved questions (surfaced for the user)

1. **`ARRIVAL_SPEED` value** (D11): a new const with no pinned value. Proposed starting point
   `2e-3` (old `V_CRUISE` magnitude) or a small multiple of `v_err_eps`; must be fixed by
   measurement (fast-flyby-must-not-fire + real-arrival-must-fire tests). Implementation-plan
   item, not a blocker.
2. **`k_brake` override vs the reset guard** (§6.1 note): **RESOLVED 2026-06-09** — `k_brake`
   is fleet-wide, per-run policy in `GuidanceParams` (no per-ship override; "brake differently"
   = a different fleet/config). The reset guard reads `cfg.guidance.k_brake`, so it is correct
   for any fleet. No further decision needed.
3. **Body-mass/μ correctness fix direction** (§12.4): **RESOLVED 2026-06-09** — varied star
   types are a design goal, so the ephemeris will use `μ = G·(M_central + m_body)` (heavier
   star → faster/tighter orbits). Tracked as **jumpgate-fca8c9e0c0** (P2 bug), a *separate*
   correctness task — **not** folded into the guidance-param implementation. Full barycentric
   two-body (central-star wobble) stays deferred (imperceptible at planet:star ratios; would
   break the `a=0`-fixed-at-focus invariant). Re-derives the physics arrival/rendezvous
   goldens (a>0 orbits shift slightly), not the tick-0 state golden; no committed recordings.
4. **Moving-body target approximation** (§5.6 / §10.2): `C_prev`/`C_now` are the body's
   *quantized* tick-endpoint positions, not its sub-tick path. Confirmed adequate by reasoning
   at `R`-scale; to be confirmed empirically by the moving-body rendezvous test. If a body's
   per-tick travel ever approaches `R`, the target itself should be swept (out of scope for v1).