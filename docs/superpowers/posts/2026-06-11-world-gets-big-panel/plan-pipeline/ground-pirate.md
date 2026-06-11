# Grounding extract — pirate.rs (haven-lurk leak, relocation, marooned doc)

Repo: /home/john/jumpgate @ e7e490e (branch jumpgate-v1-design). File under study:
`crates/jumpgate-core/src/pirate.rs` (1939 lines). All line numbers verified this session.

## 1. The nav_lurk adoption path (THE LEAK SITE)

`run_pirate_brains` (stage 1c2), `crates/jumpgate-core/src/pirate.rs:511-644`. Signature
(pirate.rs:511-520) — note it takes **no `&mut EventStream`**:

```rust
pub fn run_pirate_brains(
    ships: &mut CraftStore,
    craft_cfg: &[CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    trophic: &TrophicCfg,
    rng: &mut RngStreams,
    tick: Tick,
)
```

Per-pirate loop: a pirate's lurk IS its nav destination (doc at pirate.rs:493-496 — "no
extra hashed column"). pirate.rs:578-599:

```rust
let nav_lurk: Option<usize> = match ships.nav[row] {
    NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
        (0..stations.ids.len()).find(|&s| stations.body[s] == b)
    }
    _ => None,
};
let mut lurk = match nav_lurk {
    Some(s) => s,                       // <-- LEAK: haven adopted unchecked
    None => {
        let u = rng.stream(RngStream::Piracy).next_u64();
        match relocate_lurk_target(
            ships.pos[row], &station_pos, trophic.pirate_max_reach_au,
            haven_station, u,            // <-- exclusion guards ONLY this fresh draw
        ) { Some(s) => s, None => continue }
    }
};
```

The leak: a post-refuge pirate still `Seeking{Body(hideout)}` whose hideout body hosts a
station resolves `nav_lurk = Some(haven_row)` at :578-583 and adopts it at :585 — the
`haven_station` exclusion is only passed into the fresh-draw arm (:592) and the hungry
relocation draw (:622).

**Fix insertion point (spec §6, TROPHIC-C3):** in `run_pirate_brains`, between :583 and
:584 — `nav_lurk == haven_station` → treat as `None`, e.g.
`let nav_lurk = nav_lurk.filter(|&s| Some(s) != haven_station);` — the existing `None` arm
then performs the fresh reach-bounded draw (anchor = `ships.pos[row]`, which at the haven
is the hideout body, so ~86% of post-refuge draws become marooned breakouts on today's
band — spec :206-209, console re-judgment scheduled).

## 2. Haven exclusion + the doc comment

Haven resolution `hideout_body_index → station row`, pirate.rs:537-544 (the doc the leak
contradicts is at :537-539):

```rust
// The haven station (on the hideout body) is excluded from every lurk
// draw: a pirate does not rob where it fences (and a haven lurk is the
// seed-23 ghetto's other half — a starving pirate camped at its own lair).
let haven_station: Option<usize> = bodies
    .ids
    .id_at(trophic.hideout_body_index as usize)
    .map(|(slot, generation)| BodyId { slot, generation })
    .and_then(|hb| (0..stations.ids.len()).find(|&s| stations.body[s] == hb));
```

Same resolution pattern is duplicated at reset scatter, `world.rs:409-415` (initial lurk
draw, world.rs:387-432: uniform over ALL stations minus haven, NOT reach-bounded; comment
"a pirate does not lurk where it fences" at world.rs:407).

## 3. Reach-bounded draw machinery

`relocate_lurk_target`, pirate.rs:452-477. Public, GEOMETRY-ONLY by construction (doc
:447-451: "the signature admits GEOMETRY ONLY — no contracts, no stock, no traffic"):

```rust
pub fn relocate_lurk_target(
    anchor: Vec3,
    station_pos: &[Vec3],
    max_reach_au: f64,
    exclude: Option<usize>,
    u: u64,
) -> Option<usize>
```

In-reach pick = `u % in_reach.len()` (:463-465). None in reach → marooned BREAKOUT:
uniform over ALL huntable stations (:466-476, comment cites "the hideout-ghetto lesson,
seed-23 console session 2026-06-11 ... only the breakout is unbounded").

**STALE DOC TO FIX (spec :222-223):** the fn header at pirate.rs:443-445 still says
"none in reach -> the NEAREST station (ties to the lowest dense row)" — that was the
pre-breakout behavior; the body now does the uniform breakout. Fix this doc in the same
commit as the leak fix.

`pirate_max_reach_au`: field `config.rs:266`, default `0.6` at `config.rs:303`.
`scenario_trophic` (scenario.rs:216-227) does NOT set it — inherited silently via
`..TrophicCfg::default()`; spec :222 wants 0.6 set EXPLICITLY in both scenario factories.
Sweep knob: `scenario.rs:288` (`apply_knob "pirate_max_reach_au"`). Consumed only at
pirate.rs:591 and :621.

## 4. Relocation write site (where LurkMoved would emit) + hunger gate

Hungry-only staggered relocation, pirate.rs:600-628 (doc comment :600-607 records the
owner GO 2026-06-11 — "a FED pirate (food >= grubstake) camps"):

```rust
let hungry = ships.pirate[row].as_ref()
    .is_some_and(|p| p.food_micros < trophic.grubstake_micros);
if hungry
    && trophic.relocate_period > 0
    && tick.0 % trophic.relocate_period == (row as u64) % trophic.relocate_period
{
    let stay = (rng.stream(RngStream::Piracy).next_u64() % 1000) as u32;
    if stay >= trophic.stay_milli {
        let u = rng.stream(RngStream::Piracy).next_u64();
        if let Some(s) = relocate_lurk_target(
            station_pos[lurk], &station_pos, trophic.pirate_max_reach_au,
            haven_station, u,
        ) { lurk = s; }      // <-- pirate.rs:625 — the relocation write site
    }
}
```

A `LurkMoved` event emits where `lurk` changes: `lurk = s` at :625 (hungry relocation) and
the fresh-draw arm at :595 (post-refuge re-entry). **Breakout vs local is distinguishable
at the call site by geometry**: anchor is in hand (`station_pos[lurk]` at :619, or
`ships.pos[row]` at :589), so `breakout = station_pos[s].sub(anchor).length() >
trophic.pirate_max_reach_au` (`relocate_lurk_target` returns no flag; do not change its
geometry-only signature — test `pirates_are_information_blind` pins it, see §7).
Emitting requires threading `&mut EventStream` into `run_pirate_brains` (today absent;
caller at world.rs:736-747, the `events` field is `self.events` as in
`update_pirate_population`, world.rs:988).

The actual nav write (loiter/re-seek) is :629-643: re-issue `seek_body(ships, row,
lurk_body)` when destination changed OR drift > `engage_radius_au / 2`. `seek_body`
(:481-486) issues a fuel-derived dv budget (`tsiolkovsky_dv`), never INFINITY.

## 5. Refuge / lie-low state machine

`PirateState`, stores.rs:97-110 — HASHED economy state (self-delimiting fold, tag 0/1,
stores.rs:92-95; `engage_cooldown_until` appended inside the word-26 `Some` fold, format
v4 note at stores.rs:107-109):

```rust
pub struct PirateState {
    pub food_micros: i64,
    pub notoriety: u32,
    pub lie_low_until: Tick,
    pub engage_cooldown_until: Tick,
}
```

Lie-low routing in the brain: pirate.rs:556-574 — while `tick < p.lie_low_until`, seek the
hideout BODY (stale hideout index degrades to a deterministic skip, :558). Lie-low entry =
`update_pirate_population` (stage 3b3), pirate.rs:660-703: starvation (`food <= 0` →
`starve_lie_low_ticks`, food reset to grubstake, :679-685) and heat (`notoriety >=
heat_threshold` → `heat_lie_low_ticks`, no notoriety reset, :686-692); both emit
`EventKind::PirateLieLow { pirate, until }` (variant at contract.rs:117-120 — EventKind
lives in **contract.rs**, not an events.rs enum). Trophic events are NOT hashed
(contract.rs:97-98: "replay records (tick, state_hash) not events").

Engagement skip while hiding/cooling: pirate.rs:139 (`tick < lie_low_until || tick <
engage_cooldown_until`).

## 6. Stage ordering + hash/golden touchpoints

- Stage 1c2 `run_pirate_brains`: world.rs:729-747, PRE-physics, `body_pos` sampled at
  `next - 1 == cur` (the try_load frame precedent); gated on
  `trophic.engage_radius_au > 0.0` (the spec-§8 inert lever, also checked inside at
  pirate.rs:521). Stage 3b3 `update_pirate_population`: world.rs:988.
- Config hash: exhaustive `TrophicCfg` destructure config.rs:622-647, fields folded
  :648-671 (`pirate_max_reach_au.to_bits()` at :664, `hideout_body_index` at :667). A NEW
  TrophicCfg field is a compile error until folded (D10/M6 discipline, :620-621), and it
  MOVES `GOLDEN_CONFIG_HASH` (pinned config.rs:804; reprint helper config.rs:1029). The
  leak fix itself adds no field — spec :209 "No golden literals move".
- The fix changes Piracy-stream draw COUNT on post-refuge ticks → state_hash trajectories
  diverge from banked baselines (behavior commit, phase 0a, console re-judgment owed —
  spec :282, :372-380 OD-3). RNG cursor is Class-3 transitively-pinned (world.rs:394-395).
- `HASH_FORMAT_VERSION = 5` (hash.rs:123) — untouched by this work.

## 7. Existing tests (pirate.rs `mod tests`) covering lurk/refuge

- `relocation_respects_reach` (:1616): pins uniform-in-reach (exact 16/16/16/16 over
  u=0..64), never-beyond-reach over a real Piracy stream, marooned breakout = uniform over
  ALL huntable (:1655-1663), haven exclusion in BOTH in-reach and breakout arms with exact
  expected indices (:1664-1668), empty-stations → None (:1670). Style: direct calls to
  `relocate_lurk_target` with hand-built `station_pos`, exact `assert_eq!` on indices.
- `fed_pirate_camps_hungry_pirate_roams` (:1738): hunger gate. Fed (food = grubstake) →
  lurk identical across 64 `world.step`s; hungry (food = 1) → lurk changes within 64
  steps. Sets `relocate_period=1, stay_milli=0, upkeep_per_tick=0, reach=10.0,
  hideout_body_index=99` (out-of-range hideout to DISABLE the haven exclusion and isolate
  the hunger gate, :1759-1761). Reads lurk via `lurk_of` = pattern-match on
  `ships.nav[row]` Seeking body (:1743-1750).
- `lying_low_pirate_seeks_hideout` (:1794): sets `lie_low_until = Tick(10_000)`, one step,
  asserts `Seeking{Body(hideout)}` via `matches!`.
- `initial_lurks_are_seed_drawn` (:1562): reset scatter is Piracy-stream drawn, lurk maps
  differ across master seeds.
- `reseek_threshold_covers_dock` (:1674): drifted > radius/2 → re-seek with refreshed dv;
  settled → budget untouched.
- `lie_low_and_heat` (:1094): starve/heat entry + `PirateLieLow` payload deadlines;
  notoriety NOT reset by heat refuge (:1124).
- `pirates_are_information_blind` (:1307): "compile-level: `relocate_lurk_target`'s
  geometry-only signature" (:1311) — blocks adding traffic inputs to the fn.
- `unscripted_pirate_is_skipped_by_brain_stages` (:1820); `default_engage_radius_is_inert`
  (:1070); `replay_bit_identical_with_piracy_draws` (:1523).
- Test fixtures: `pirate_world_cfg()` / `pirate_init()` helpers + `live_trophic()`
  (:733-742, deterministic p_rob 1000).

## GOTCHAS

1. `run_pirate_brains` has NO `events` parameter — a LurkMoved emit needs a signature
   change (pirate.rs:511-520) plus the world.rs:736-747 call site. Events are hash-neutral;
   the RNG draw-count change from the leak fix is NOT (baselines shift, goldens don't).
2. Do NOT fix the leak inside `relocate_lurk_target` or by changing its signature — the
   leak is upstream (the `Some(s) => s` adoption at pirate.rs:585), and
   `pirates_are_information_blind` pins the fn's geometry-only shape.
3. Two different anchors: fresh draw anchors at `ships.pos[row]` (:589); hungry relocation
   anchors at `station_pos[lurk]` (:619). Breakout detection must use the matching anchor.
4. `lurk` is nav-derived — there is no lurk column. "Relocation" = mutating the local
   `lurk` then `seek_body`; LurkMoved must compare old vs new station row before :629-643
   reissues the seek (a drift re-seek to the SAME station is not a move).
5. The marooned doc at pirate.rs:443-445 ("NEAREST station") is stale — body does a
   uniform breakout (:466-476). Fix doc only; behavior + test (:1655-1663) are correct.
6. `hideout_body_index` out of range is legal (spec §8 totality): `id_at` → None →
   `haven_station = None` / lie-low skip. Tests exploit this (hideout_body_index = 99) to
   switch the exclusion off — don't "fix" it into an error.
7. `stations.body[s] == hb` matching means MULTIPLE stations on the hideout body would
   only exclude the first found (`.find`); today's scenarios have at most one.
