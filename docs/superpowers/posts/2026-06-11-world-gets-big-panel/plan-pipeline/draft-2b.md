# World-gets-big plan — section 2b (Phase 2, second half)

Spec: `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` §6 (LurkMoved
context), §7, §8 (TrophicSample), §9 (phase-2 tail). HEAD at drafting: `e7e490e`.
All line refs below are against that HEAD; phase-0/1 and phase-2-first-half tasks
land before these and may shift lines — symbols are the anchor, lines the hint.

**Ordering within this section:** 2.6 → 2.7 → 2.8 → 2.9 → 2.10 → 2.11 → 2.12.
2.8 must precede 2.9 (the FUEL tokens read the new TrophicSample fields).
2.10 and 2.11 must precede 2.12 (the calibration re-pins the golden 2.10 creates
and drives the knob 2.11 creates).

**Standing house rules every commit step below obeys:**
- `git add` EXPLICIT paths only; never `-A`, never `.`; never stage `runs/`.
- Commit messages end with the exact trailer line
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`
  via `git commit -F -` heredoc.
- Golden literals are NEVER typed from this plan — they are pasted from the
  `#[ignore]` printer test output, single-cause commit, provenance comment
  citing cause + old literal.
- Reward surfaces untouched. Every metric below is a recorded window, never a
  pass/fail gate (determinism/unit tests excepted).

---

### Task 2.6: `LurkMoved` event — emit at both lurk-write sites (hash-neutral), chronicle arm

The relocation write sites are `crates/jumpgate-core/src/pirate.rs` — the
post-refuge fresh-draw arm (`run_pirate_brains`, the `None => relocate_lurk_target`
match at pirate.rs:586-598) and the hungry-relocation assignment `lurk = s;`
(pirate.rs:625). A drift re-seek to the SAME station (pirate.rs:629-643) is NOT a
move and must not emit. `run_pirate_brains` takes no `&mut EventStream` today
(pirate.rs:511-520) — the signature grows, plus the world.rs:736-747 call site.
Events are hash-neutral by design (contract.rs:97-98): zero goldens move, no
RNG-draw-count change (emits only).

**Files**
- Modify: `crates/jumpgate-core/src/contract.rs` (EventKind tail — append after
  the current last variant; phase 1 appended `Refueled`/`ContractFailed` there)
- Modify: `crates/jumpgate-core/src/pirate.rs` (`run_pirate_brains` signature
  :511-520, fresh-draw arm :584-599, relocation arm :611-628; tests mod)
- Modify: `crates/jumpgate-core/src/world.rs` (call site :736-747)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (`chronicle_subject`
  :242-264 — the `_ => None` catch-all would otherwise silently swallow it)

- [ ] **Step 1: Write the failing test** in `crates/jumpgate-core/src/pirate.rs`
  tests mod (clone the `fed_pirate_camps_hungry_pirate_roams` fixture style,
  pirate.rs:1737-1791):

```rust
    #[test]
    fn lurk_moves_emit_lurk_moved_with_breakout_flag() {
        // World-gets-big spec §7 / W6: LurkMoved emits ONLY when the lurk's
        // station row actually changes (a drift re-seek to the SAME station
        // is not a move); breakout is judged against the draw's own anchor
        // (fresh post-refuge draw: the pirate's position; hungry relocation:
        // the OLD lurk station).
        fn lurk_moved_events(world: &World) -> Vec<(u32, bool)> {
            world
                .recent_events(Tick(0))
                .iter()
                .filter_map(|e| match e.kind {
                    EventKind::LurkMoved { to_station, breakout, .. } => {
                        Some((to_station, breakout))
                    }
                    _ => None,
                })
                .collect()
        }
        fn cfg() -> RunConfig {
            let mut cfg = pirate_world_cfg();
            cfg.contracts = vec![];
            cfg.craft = vec![pirate_init(Vec3::ZERO)];
            cfg.trophic.relocate_period = 1; // eligible every tick
            cfg.trophic.stay_milli = 0; // never sticky
            cfg.trophic.upkeep_per_tick = 0; // hold hunger constant
            cfg.trophic.pirate_max_reach_au = 10.0; // both stations in reach
            // Out-of-range hideout: no haven exclusion (spec §8 totality).
            cfg.trophic.hideout_body_index = 99;
            cfg
        }
        // FED pirate: camps — zero LurkMoved over the probe window.
        let c = cfg();
        let grubstake = c.trophic.grubstake_micros;
        let (mut world, _) = World::reset(c).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().food_micros = grubstake;
        for _ in 0..64 {
            world.step(&mut Vec::new());
        }
        assert!(lurk_moved_events(&world).is_empty(), "a fed pirate's camp is not a move");
        // HUNGRY pirate, both stations in reach: relocations emit, and every
        // landing is in reach of the OLD lurk (0.3 AU < 10) — breakout=false.
        let (mut world, _) = World::reset(cfg()).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().food_micros = 1;
        for _ in 0..64 {
            world.step(&mut Vec::new());
        }
        let moves = lurk_moved_events(&world);
        assert!(!moves.is_empty(), "a hungry pirate's redraws emit LurkMoved");
        assert!(moves.iter().all(|&(_, b)| !b), "in-reach hops are not breakouts");
        // POST-REFUGE fresh draw with NOTHING in reach: one marooned breakout
        // (anchor = the pirate's own position, ~5 AU from both stations).
        let mut c = cfg();
        c.trophic.pirate_max_reach_au = 1.0e-6;
        c.craft = vec![pirate_init(Vec3::new(5.0, 0.0, 0.0))];
        let grubstake = c.trophic.grubstake_micros;
        let (mut world, _) = World::reset(c).expect("resolvable cfg");
        // Fed: suppresses the hungry-relocation arm, isolating the fresh draw.
        world.ships.pirate[0].as_mut().unwrap().food_micros = grubstake;
        // Post-refuge shape: nav holds no station body.
        world.ships.nav[0] = NavState::Idle;
        world.step(&mut Vec::new());
        let moves = lurk_moved_events(&world);
        assert_eq!(moves.len(), 1, "one fresh post-refuge draw -> one LurkMoved");
        assert!(moves[0].1, "nothing in reach -> the landing is a breakout");
    }
```

- [ ] **Step 2: Run it and watch it fail to compile** (the variant does not exist):

```
cargo test -p jumpgate-core lurk_moves_emit_lurk_moved_with_breakout_flag
```

Expected: `error[E0599]: no variant or associated item named `LurkMoved` found
for enum `EventKind``.

- [ ] **Step 3: Add the variant** at the tail of `EventKind` in
  `crates/jumpgate-core/src/contract.rs` (append-only — after the phase-1
  `Refueled`/`ContractFailed` variants; the media-block precedent at :139-168
  documents the emission latch + chronicle policy inline):

```rust
    /// A pirate's lurk moved to a new station (world-gets-big spec §7; backs
    /// W6 breakout share + landing distribution). Emitted in stage 1c2 at the
    /// two lurk-write sites ONLY when the station row changes (a drift
    /// re-seek to the SAME station is not a move). `to_station` is the dense
    /// station row (stations mint once at reset and never despawn — the
    /// gossip-log `s<row>` encoding precedent). `breakout` = the landing lies
    /// beyond `pirate_max_reach_au` of the draw's own anchor (fresh
    /// post-refuge draw anchors at the pirate's position; hungry relocation
    /// anchors at the old lurk station). Chronicle arm: the pirate's life arc.
    LurkMoved { pirate: CraftId, to_station: u32, breakout: bool },
```

- [ ] **Step 4: Grow the `run_pirate_brains` signature and emit at both write
  sites** in `crates/jumpgate-core/src/pirate.rs`. Signature (events last, the
  world.rs:725-727 `next, &mut self.events` stage convention):

```rust
#[allow(clippy::too_many_arguments)]
pub fn run_pirate_brains(
    ships: &mut CraftStore,
    craft_cfg: &[CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    trophic: &TrophicCfg,
    rng: &mut RngStreams,
    tick: Tick,
    events: &mut EventStream,
) {
```

Also append one bullet to the fn doc comment:

```rust
/// * **LurkMoved** (spec §7, W6): emitted wherever the lurk's station row
///   changes — the fresh post-refuge draw and the hungry relocation — never
///   on a drift re-seek to the same station. Hash-neutral (events are not
///   folded; no extra RNG draws on any path).
```

The fresh-draw arm (pirate.rs:584-599; the phase-0a haven filter line sits just
above it — leave it untouched):

```rust
        let mut lurk = match nav_lurk {
            Some(s) => s,
            None => {
                let u = rng.stream(RngStream::Piracy).next_u64();
                match relocate_lurk_target(
                    ships.pos[row],
                    &station_pos,
                    trophic.pirate_max_reach_au,
                    haven_station,
                    u,
                ) {
                    Some(s) => {
                        // Post-refuge re-entry IS a move (there was no lurk).
                        // Breakout judged against THIS draw's anchor: the
                        // pirate's own position (spec §6 re-entry honesty).
                        let breakout = station_pos[s].sub(ships.pos[row]).length()
                            > trophic.pirate_max_reach_au;
                        events.emit(Event {
                            tick,
                            kind: EventKind::LurkMoved {
                                pirate: ships.ids_at(row),
                                to_station: s as u32,
                                breakout,
                            },
                        });
                        s
                    }
                    None => continue,
                }
            }
        };
```

The hungry-relocation arm (pirate.rs:616-627) — gate the emit on `s != lurk`
(`relocate_lurk_target` can legally redraw the current station; an unchanged
row is not a move):

```rust
            if stay >= trophic.stay_milli {
                let u = rng.stream(RngStream::Piracy).next_u64();
                if let Some(s) = relocate_lurk_target(
                    station_pos[lurk],
                    &station_pos,
                    trophic.pirate_max_reach_au,
                    haven_station,
                    u,
                ) && s != lurk
                {
                    // Breakout judged against THIS draw's anchor: the OLD
                    // lurk station (the matching-anchor rule, spec §7).
                    let breakout = station_pos[s].sub(station_pos[lurk]).length()
                        > trophic.pirate_max_reach_au;
                    events.emit(Event {
                        tick,
                        kind: EventKind::LurkMoved {
                            pirate: ships.ids_at(row),
                            to_station: s as u32,
                            breakout,
                        },
                    });
                    lurk = s;
                }
            }
```

Do NOT touch `relocate_lurk_target` itself — `pirates_are_information_blind`
(pirate.rs:1307-1311) pins its geometry-only signature by construction.

- [ ] **Step 5: Update the world.rs call site** (world.rs:736-747) — append the
  events argument:

```rust
        if self.config.trophic.engage_radius_au > 0.0 {
            crate::pirate::run_pirate_brains(
                &mut self.ships,
                &self.config.craft,
                &self.stations,
                &self.bodies,
                &self.eph,
                &self.config.trophic,
                &mut self.rng,
                next,
                &mut self.events,
            );
        }
```

Fix any other direct `run_pirate_brains` callers the compiler names (tests pass
a fresh `&mut EventStream::new()` if any call it directly).

- [ ] **Step 6: Add the chronicle arm** in
  `crates/jumpgate-core/examples/trophic_run.rs` `chronicle_subject` — into the
  existing pirate block (:251-256), because the `_ => None` catch-all (:262)
  silently swallows new variants:

```rust
        EventKind::Robbed { pirate, .. }
        | EventKind::DrivenOff { pirate, .. }
        | EventKind::HaulerKilled { pirate, .. }
        | EventKind::PirateLieLow { pirate, .. }
        | EventKind::PirateLeft { pirate }
        | EventKind::PirateSpawned { pirate }
        | EventKind::LurkMoved { pirate, .. } => Some(pirate),
```

- [ ] **Step 7: Run the test and the determinism suite:**

```
cargo test -p jumpgate-core lurk_moves_emit_lurk_moved_with_breakout_flag
cargo test -p jumpgate-core replay_bit_identical_with_piracy_draws
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all green — `test lurk_moves_emit_lurk_moved_with_breakout_flag ... ok`;
replay bit-identity unchanged (emits add no RNG draws, no hashed state); zero
golden tests move.

- [ ] **Step 8: Commit:**

```
git add crates/jumpgate-core/src/contract.rs crates/jumpgate-core/src/pirate.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(events): LurkMoved{pirate,to_station,breakout} at both lurk-write sites (W6)

Hash-neutral single-emit per actual row change; matching-anchor breakout flag
(fresh draw: pirate pos; hungry relocation: old lurk). run_pirate_brains gains
&mut EventStream; chronicle arm added (the _=>None swallow). No goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.7: `World::craft_role` accessor + chronicle per-craft epilogue

Spec §7: "Chronicle epilogue per craft (printer-side): role, workplace radius,
tank permille, credits, `ADRIFT since t=…` — adrift computed from final world
state." Example binaries see only pub API (TROPHIC-C2 lesson); no pub role
accessor exists today — add one (the trader-accessor pattern, world.rs:546
`craft_credits` precedent). Everything else rides existing pub surface:
`craft_fuel`/`craft_fuel_capacity`/`body_pos`/`recent_events` (StateView,
contract.rs:198-214), `craft_credits` (world.rs:546), `craft_is_idle`
(world.rs:608-613), `FUEL_EMPTY_EPS` (lib.rs:57).

**Files**
- Modify: `crates/jumpgate-core/src/world.rs` (new accessor next to
  `craft_credits` at :546; test in the world.rs tests mod)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (`print_chronicle`
  :273-307 — epilogue after the final `flush(&pending)` at :305, still inside
  the per-craft loop; imports at :29-33)

- [ ] **Step 1: Write the failing accessor test** in
  `crates/jumpgate-core/src/world.rs` tests mod (reuse the existing
  `one_body_one_craft()` fixture, world.rs:1198):

```rust
    #[test]
    fn craft_role_reads_role_and_none_for_stale() {
        // World-gets-big spec §7: the chronicle epilogue's role read — a
        // plain pub accessor over already-hashed state (trader-accessor
        // pattern; no layout, fold-order, or stepping change).
        let (world, _) = World::reset(one_body_one_craft()).expect("resolvable cfg");
        let id = world.ships.ids_at(0);
        assert_eq!(world.craft_role(id), Some(crate::stores::CraftRole::Idle), "live read");
        let stale = CraftId { slot: id.slot, generation: id.generation + 1 };
        assert_eq!(world.craft_role(stale), None, "stale id reads None");
    }
```

- [ ] **Step 2: Run and watch it fail:**

```
cargo test -p jumpgate-core craft_role_reads_role_and_none_for_stale
```

Expected: `error[E0599]: no method named `craft_role` found`.

- [ ] **Step 3: Add the accessor** in `impl World`, directly below
  `craft_credits` (world.rs:546-548):

```rust
    /// Role of a live craft (the chronicle epilogue's read — world-gets-big
    /// spec §7), or `None` for a stale id. Plain read over already-hashed
    /// state (the trader-accessor pattern): no layout, fold-order, or
    /// stepping change.
    pub fn craft_role(&self, id: CraftId) -> Option<crate::stores::CraftRole> {
        self.ship_index(id).map(|i| self.ships.role[i])
    }
```

- [ ] **Step 4: Run — pass:**

```
cargo test -p jumpgate-core craft_role_reads_role_and_none_for_stale
```

Expected: `test ... ok`.

- [ ] **Step 5: Add the epilogue to `print_chronicle`.** Extend the
  trophic_run import (:30-33) with `EntityRef`, `NavDest`, `FUEL_EMPTY_EPS`:

```rust
use jumpgate_core::{
    Command, CraftId, EntityRef, EventKind, FUEL_EMPTY_EPS, GossipNode, NavDest, RunConfig,
    StateView, Tick, World, apply_knob, scenario_trophic, state_hash,
};
```

Then insert after the final `flush(&pending);` (trophic_run.rs:305), still
inside the `for id in world.craft_ids()` loop:

```rust
        // ---- per-craft epilogue (world-gets-big spec §7): final-state
        // summary — printer-side only (PDR-0006: a window, never a gate) ----
        let role = world
            .craft_role(id)
            .map_or_else(|| "stale".to_string(), |r| format!("{r:?}"));
        let fuel = world.craft_fuel(id).unwrap_or(0.0);
        let cap = world.craft_fuel_capacity(id).unwrap_or(0.0);
        // FLOOR permille — the same rounding form the Refueled
        // tank_before_permille pins (spec §7).
        let tank_permille = if cap > 0.0 { ((fuel / cap) * 1000.0).floor() as u32 } else { 0 };
        let credits = world.craft_credits(id).unwrap_or(0);
        // Workplace radius: mean radial distance (milli-AU, FLOOR) of the
        // bodies this craft ARRIVED at over the whole run. All factory orbits
        // are circular (e = 0), so the current-tick body_pos read is
        // radius-time-invariant. 0 = never arrived anywhere.
        let (mut r_sum, mut r_n) = (0.0f64, 0u64);
        for e in world.recent_events(Tick(0)) {
            if let EventKind::Arrival { craft, dest: NavDest::Entity(EntityRef::Body(b)) } =
                e.kind
                && craft == id
                && let Some(p) = world.body_pos(b, world.tick())
            {
                r_sum += p.length();
                r_n += 1;
            }
        }
        let workplace_radius_milli_au =
            if r_n == 0 { 0 } else { ((r_sum / r_n as f64) * 1000.0).floor() as u64 };
        // ADRIFT detector (spec §5 PLAY-C1's true end state): role-Idle with
        // an empty tank; `since` = the craft's LAST FuelEmpty edge.
        let adrift = world.craft_is_idle(id) == Some(true) && fuel <= FUEL_EMPTY_EPS;
        let line = format!(
            "  == epilogue: role={role} workplace_radius_milli_au={workplace_radius_milli_au} \
             tank_permille={tank_permille} credits_micros={credits}"
        );
        if adrift {
            let since = world
                .recent_events(Tick(0))
                .iter()
                .rev()
                .find_map(|e| match e.kind {
                    EventKind::FuelEmpty { craft } if craft == id => Some(e.tick.0),
                    _ => None,
                });
            match since {
                Some(t) => println!("{line} ADRIFT since t={t}"),
                None => println!("{line} ADRIFT since t=reset"),
            }
        } else {
            println!("{line}");
        }
```

- [ ] **Step 6: Verify on a real run** (the printer has no cargo-test surface;
  anchored-output verification is the house pattern for example binaries):

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 10000 --chronicle | grep -c "== epilogue:"
```

Expected output: `18` (12 haulers + 6 pirates — one epilogue line per craft).
Spot-check shape:

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 10000 --chronicle | grep "== epilogue:" | head -3
```

Expected: lines like
`  == epilogue: role=Hauler workplace_radius_milli_au=<n> tank_permille=<n> credits_micros=<n>`
with `role=Pirate` rows tailing; NO `ADRIFT` token on trophic (FuelEmpty is
unfireable there — W12's control stays a control).

- [ ] **Step 7: Full suite + lint, then commit:**

```
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/world.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(chronicle): per-craft epilogue (role/workplace radius/tank/credits/ADRIFT)

New pub World::craft_role (trader-accessor pattern); workplace radius = mean
Arrival-body radial distance in FLOOR milli-AU (circular orbits make the
current-tick read radius-invariant); ADRIFT = Idle + tank <= eps, since = last
FuelEmpty edge. Printer-side only; hash-neutral.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.8: TrophicSample additive frontier fields through `sample_window` + JSONL tail

TROPHIC-C2: the lab cannot read `pub(crate)` state from the example binary —
the eight new reads flow through `sample_window`
(`crates/jumpgate-core/src/diagnostics.rs:437-605`). TrophicSample is
all-integer by law with `Default`+`Eq` (diagnostics.rs:63-65); new fields
APPEND at the struct end and at the END of `sample_json`
(trophic_run.rs:142-177) so every pre-existing JSONL key is byte-untouched
(the media/assign additive precedent). Two different fuels: craft propellant is
`ships.fuel_mass` (stores.rs:160); `per_station_fuel_stock/price` read the
traded `Resource::Fuel` (index 1) in `stations.stock`/`price_micros`
(economy.rs:9-45) — cargo-side, never conflate.

**Files**
- Modify: `crates/jumpgate-core/src/diagnostics.rs` (struct tail after
  `assign_counts_cum` :139; `sample_window` :437-605; imports :13-16; tests)
- Modify: `crates/jumpgate-core/src/world.rs` (new `pub(crate) fn trophic_cfg`
  next to `shipyard_cfg` :595-597 — `config` is a private field, the
  `shipyard_cfg` accessor is the named precedent)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (`sample_json`
  :142-177, append 8 keys at the tail)

- [ ] **Step 1: Write the failing tests** in `diagnostics.rs` tests mod (clone
  the real-world style of `sample_window_counts_purchases_and_reads_yard_treasury`
  :812-894). NOTE the `refuel: RefuelCfg::default()` field is the phase-1
  RunConfig tail addition; both tests build full literals:

```rust
    #[test]
    fn sample_window_reads_fuel_book_and_pirate_partition() {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RunConfig, ShipyardCfg, StationInit, SubstepCfg,
            TrophicCfg,
        };
        use crate::math::Vec3;
        use crate::stores::{CraftRole, NavState};
        use crate::time::Dt;
        use crate::world::World;
        fn cfg(hideout: u32) -> RunConfig {
            RunConfig {
                master_seed: 7,
                dt: Dt::new(0.25),
                softening: 1e-3,
                substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
                ephemeris_window: 256,
                bodies: vec![BodyInit {
                    mass: 1e-9,
                    elements: OrbitalElements {
                        a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0,
                    },
                }],
                craft: vec![CraftInit {
                    spec: BaseSpec {
                        base_dry_mass: 1e-9,
                        base_max_thrust: 1e-12,
                        base_exhaust_velocity: 1e-2,
                        base_fuel_capacity: 1e-9,
                        base_cargo_capacity: 5,
                    },
                    pos: Vec3::ZERO, // co-located with the only body
                    vel: Vec3::ZERO,
                    fuel_mass: 1e-9,
                    role: CraftRole::Pirate,
                    scripted: true,
                }],
                guidance: GuidanceParams::default(),
                stations: vec![StationInit {
                    body_index: 0,
                    initial_stock: [3, 17],           // [Ore, Fuel]
                    initial_price_micros: [0, 5_000], // Fuel priced, Ore dead
                    sells_upgrades: false,
                }],
                producers: vec![],
                corporations: vec![CorporationInit {
                    treasury_micros: 0,
                    home_station_index: 0,
                }],
                contracts: vec![],
                price_cfg: PriceCfg::default(),
                dispatch_cfg: DispatchCfg::default(),
                trophic: TrophicCfg {
                    engage_radius_au: 5.0e-4,
                    hideout_body_index: hideout,
                    ..TrophicCfg::default()
                },
                shipyard: ShipyardCfg::default(),
                media: crate::config::MediaCfg::default(),
                refuel: crate::config::RefuelCfg::default(), // phase-1 tail field, inert
            }
        }
        // SETTLED LURKER: hideout 99 -> no haven exclusion; reset scatter
        // lurks the only station; the pirate sits ON its body (distance 0).
        let (world, _h) = World::reset(cfg(99)).expect("resolvable cfg");
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_station_fuel_stock, vec![17], "Fuel-side stock book");
        assert_eq!(s.per_station_fuel_price, vec![5_000], "Fuel-side price book");
        assert_eq!(s.per_station_lurking_pirates, vec![1], "settled lurker at its station");
        assert_eq!(s.pirates_commuting, 0);
        assert_eq!(s.pirates_at_haven, 0);
        assert_eq!(s.refuels, 0, "no Refueled events on an inert-refuel world");
        assert_eq!(s.refuel_units, 0);
        assert_eq!(s.refuel_spend_micros, 0);
        // Partition invariant (the lying-instrument check, seed-7 rule).
        let lurking: u32 = s.per_station_lurking_pirates.iter().sum();
        assert_eq!(lurking + s.pirates_commuting + s.pirates_at_haven, 1, "partition is total");
        // COMMUTING: an active pirate whose nav holds no station body.
        let (mut world, _h) = World::reset(cfg(99)).expect("resolvable cfg");
        world.ships.nav[0] = NavState::Idle;
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_station_lurking_pirates, vec![0]);
        assert_eq!(s.pirates_commuting, 1, "no settled lurk reads as commuting");
        // AT HAVEN: lying low AND arrived at the hideout body.
        let (mut world, _h) = World::reset(cfg(0)).expect("resolvable cfg");
        world.ships.pirate[0].as_mut().unwrap().lie_low_until = Tick(10_000);
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.pirates_at_haven, 1, "lying low on the hideout body reads at-haven");
        assert_eq!(s.pirates_commuting, 0);
        assert_eq!(s.per_station_lurking_pirates, vec![0], "a refugee is not a lurker");
    }

    #[test]
    fn sample_window_counts_refuels() {
        use crate::config::{
            BaseSpec, BodyInit, CorporationInit, CraftInit, DispatchCfg, GuidanceParams,
            OrbitalElements, PriceCfg, RefuelCfg, RunConfig, ShipyardCfg, StationInit,
            SubstepCfg, TrophicCfg,
        };
        use crate::math::Vec3;
        use crate::stores::CraftRole;
        use crate::time::Dt;
        use crate::world::World;
        // One docked Idle scripted craft, tank at cap/4, lot == cap/4 (exact
        // binary fractions: need = floor((cap - cap/4)/(cap/4)) = 3, no f64
        // rounding hazard). Stage 1c3b writes the intent, 1d2 resolves the
        // SAME tick (the pending_upgrade precedent) -> one Refueled event.
        let cfg = RunConfig {
            master_seed: 7,
            dt: Dt::new(0.25),
            softening: 1e-3,
            substep_cfg: SubstepCfg { accel_ref: 1e-3, max_substeps: 64 },
            ephemeris_window: 256,
            bodies: vec![BodyInit {
                mass: 1e-9,
                elements: OrbitalElements {
                    a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0,
                },
            }],
            craft: vec![CraftInit {
                spec: BaseSpec {
                    base_dry_mass: 1e-9,
                    base_max_thrust: 1e-12,
                    base_exhaust_velocity: 1e-2,
                    base_fuel_capacity: 1e-9,
                    base_cargo_capacity: 5,
                },
                pos: Vec3::ZERO, // docked at the only station's body
                vel: Vec3::ZERO,
                fuel_mass: 2.5e-10, // cap/4
                role: CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![StationInit {
                body_index: 0,
                initial_stock: [0, 10],
                initial_price_micros: [0, 5_000],
                sells_upgrades: false,
            }],
            producers: vec![],
            corporations: vec![CorporationInit { treasury_micros: 0, home_station_index: 0 }],
            contracts: vec![],
            // Fuel live, Ore structurally dead (cap 0 — the update_prices skip).
            price_cfg: PriceCfg {
                base_micros: [0, 5_000],
                cap: [0, 40],
                slope_milli: 1800,
                reprice_interval: 1,
            },
            dispatch_cfg: DispatchCfg::default(),
            trophic: TrophicCfg::default(),
            shipyard: ShipyardCfg::default(),
            media: crate::config::MediaCfg::default(),
            refuel: RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 },
        };
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.step(&mut Vec::new());
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.refuels, 1, "one Refueled event in the window");
        assert_eq!(s.refuel_units, 3, "units = min(need 3, stock 10, afford 200)");
        assert_eq!(s.refuel_spend_micros, 15_000, "3 units x seeded 5_000 micros");
        assert_eq!(s.per_station_fuel_stock, vec![7], "stock book debited by the purchase");
    }
```

- [ ] **Step 2: Run and watch them fail:**

```
cargo test -p jumpgate-core sample_window_reads_fuel_book_and_pirate_partition
cargo test -p jumpgate-core sample_window_counts_refuels
```

Expected: `error[E0609]: no field `per_station_fuel_stock` on type
`TrophicSample`` (and siblings).

- [ ] **Step 3: Add the `trophic_cfg` accessor** in `impl World`, directly
  below `shipyard_cfg` (world.rs:595-597):

```rust
    /// Trophic config for the diagnostics sampler (the `shipyard_cfg`
    /// precedent — `config` is private): the hideout index + engage radius
    /// feed the pirate-location partition read (world-gets-big spec §8,
    /// TROPHIC-C2). Plain read over already-hashed config — never a behavior
    /// input.
    pub(crate) fn trophic_cfg(&self) -> &crate::config::TrophicCfg {
        &self.config.trophic
    }
```

- [ ] **Step 4: Append the struct fields** at the end of `TrophicSample`
  (diagnostics.rs, after `assign_counts_cum` :139):

```rust
    // -- world-gets-big lab fields (spec §8; TROPHIC-C2) -- ADDITIVE: every
    // pre-frontier JSONL key above is byte-untouched. All integers (house
    // law: samples are hash-adjacent evidence, never float analytics).
    /// Settled lurkers per dense station row: active (not lying-low) pirates
    /// whose nav-derived lurk is this station AND whose position is inside
    /// the engagement envelope (`engage_radius_au`) of the station body at
    /// the sample tick.
    pub per_station_lurking_pirates: Vec<u32>,
    /// Pirates in transit: active with no settled lurk, plus lying-low
    /// pirates still commuting to the haven. With the settled lurkers and
    /// `pirates_at_haven` this PARTITIONS the pirate population (pinned by
    /// test): sum(lurking) + commuting + at_haven == pirates.
    pub pirates_commuting: u32,
    /// Lying-low pirates ARRIVED at the hideout body (within ARRIVAL_RADIUS).
    pub pirates_at_haven: u32,
    /// Station fuel-side cargo book at the sample point — the traded
    /// `Resource::Fuel` in stock/price (economy.rs), NOT craft propellant.
    pub per_station_fuel_stock: Vec<i64>,
    pub per_station_fuel_price: Vec<i64>,
    /// Windowed `Refueled`-event reads (0-sentinels when RefuelCfg is off).
    pub refuels: u32,
    pub refuel_units: u64,
    pub refuel_spend_micros: i64,
```

- [ ] **Step 5: Gather in `sample_window`.** Extend the imports
  (diagnostics.rs:13-16):

```rust
use crate::autopilot::ARRIVAL_RADIUS;
use crate::contract::{EventKind, StateView};
use crate::economy::Resource;
use crate::ids::{BodyId, ContractId, StationId};
use crate::math::Vec3;
use crate::stores::NavState;
use crate::time::Tick;
use crate::types::{EntityRef, NavDest};
use crate::world::World;
```

Declare with the other windowed counters (before the :453 event loop):

```rust
    let mut refuels: u32 = 0;
    let mut refuel_units: u64 = 0;
    let mut refuel_spend_micros: i64 = 0;
```

Add the windowed-event arm inside the :453-505 match (next to
`UpgradePurchased`):

```rust
            EventKind::Refueled { units, price_micros, .. } => {
                refuels = refuels.saturating_add(1);
                refuel_units = refuel_units.saturating_add(units.max(0) as u64);
                refuel_spend_micros =
                    refuel_spend_micros.saturating_add(units.saturating_mul(price_micros));
            }
```

(`units`/`price_micros` are the phase-1 `Refueled` payload integers — if phase
1 landed `units` narrower than `i64`, widen with `i64::from` here, never
truncate.)

Add the pirate-location partition after the per-craft snapshot loop
(:545-559):

```rust
    // World-gets-big pirate-location partition (spec §8; TROPHIC-C2: the lab
    // reads pub(crate) state ONLY through this sampler). Nav-derived lurk
    // (the stage-1c2 read) + geometry at the sample tick. Pure read.
    let trophic = world.trophic_cfg();
    let station_pos_now: Vec<Option<Vec3>> = (0..n_stations)
        .map(|srow| {
            world
                .stations
                .ids
                .id_at(srow)
                .map(|(slot, generation)| StationId { slot, generation })
                .and_then(|sid| world.station_pos(sid))
        })
        .collect();
    let hideout_pos: Option<Vec3> = world
        .bodies
        .ids
        .id_at(trophic.hideout_body_index as usize)
        .map(|(slot, generation)| BodyId { slot, generation })
        .and_then(|bid| world.body_pos(bid, tick));
    let mut per_station_lurking_pirates = vec![0u32; n_stations];
    let mut pirates_commuting: u32 = 0;
    let mut pirates_at_haven: u32 = 0;
    for r in 0..world.ships.ids.len() {
        let Some(p) = world.ships.pirate[r] else {
            continue;
        };
        if p.lie_low_until > tick {
            // Refuge population: ARRIVED at the hideout body vs still
            // commuting to it (a stale hideout index degrades to commuting —
            // spec §8 totality).
            let arrived = hideout_pos
                .is_some_and(|hp| world.ships.pos[r].sub(hp).length() <= ARRIVAL_RADIUS);
            if arrived {
                pirates_at_haven = pirates_at_haven.saturating_add(1);
            } else {
                pirates_commuting = pirates_commuting.saturating_add(1);
            }
            continue;
        }
        // The stage-1c2 nav-lurk read: the lurk IS the nav destination.
        let nav_lurk: Option<usize> = match world.ships.nav[r] {
            NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
                (0..n_stations).find(|&s| world.stations.body[s] == b)
            }
            _ => None,
        };
        let settled = nav_lurk.is_some_and(|s| {
            station_pos_now[s].is_some_and(|sp| {
                world.ships.pos[r].sub(sp).length() <= trophic.engage_radius_au
            })
        });
        match nav_lurk {
            Some(s) if settled => {
                per_station_lurking_pirates[s] = per_station_lurking_pirates[s].saturating_add(1);
            }
            _ => pirates_commuting = pirates_commuting.saturating_add(1),
        }
    }
```

Append to the `TrophicSample` literal tail (:560-604, after
`assign_counts_cum`):

```rust
        per_station_lurking_pirates,
        pirates_commuting,
        pirates_at_haven,
        per_station_fuel_stock: world
            .stations
            .stock
            .iter()
            .map(|st| st[Resource::Fuel.index()])
            .collect(),
        per_station_fuel_price: world
            .stations
            .price_micros
            .iter()
            .map(|pr| pr[Resource::Fuel.index()])
            .collect(),
        refuels,
        refuel_units,
        refuel_spend_micros,
```

- [ ] **Step 6: Append the JSONL keys** at the END of `sample_json`
  (trophic_run.rs:142-177, after `assign_counts_cum`):

```rust
        // world-gets-big lab keys (Task 2.8) — ADDITIVE: every pre-frontier
        // key above is byte-untouched.
        "per_station_lurking_pirates": s.per_station_lurking_pirates,
        "pirates_commuting": s.pirates_commuting,
        "pirates_at_haven": s.pirates_at_haven,
        "per_station_fuel_stock": s.per_station_fuel_stock,
        "per_station_fuel_price": s.per_station_fuel_price,
        "refuels": s.refuels,
        "refuel_units": s.refuel_units,
        "refuel_spend_micros": s.refuel_spend_micros,
```

- [ ] **Step 7: Run — pass — then the full suite:**

```
cargo test -p jumpgate-core sample_window_reads_fuel_book_and_pirate_partition
cargo test -p jumpgate-core sample_window_counts_refuels
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: both new tests `ok`; the existing
`sample_window_counts_purchases_and_reads_yard_treasury` still green (additive
fields only — any synthetic-sample builders the compiler flags get the new
fields via their existing `..Default::default()` tails); zero goldens move
(samples are unhashed).

- [ ] **Step 8: Commit:**

```
git add crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(lab): TrophicSample frontier fields through sample_window (TROPHIC-C2)

per_station_lurking_pirates / pirates_commuting / pirates_at_haven partition
(pinned total by test), per-station Fuel stock/price book, windowed
refuels/refuel_units/refuel_spend_micros. Appended at struct + JSONL tails
(media/assign additive precedent); pure reads, zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.9: FUEL line gains the refuel fields deferred from 0b (+ FUEL_RE optional tail, lockstep)

Phase 0b landed the anchored role-split `FUEL` line (measured fields only) and
its version-gated `FUEL_RE` in `python/analysis/sweep_trophic.py`. This task
appends the two mechanic-dependent tokens the spec defers to the mechanic
(`refuels=`, `refuel_spend_micros=`) at the END of the line, and extends
`FUEL_RE` with an OPTIONAL tail group in the SAME commit (the lockstep rule,
trophic_run.rs:398-401 / sweep_trophic.py:58-59) so banked pre-refuel FUEL
lines still parse — never add tokens mid-line (the RESULT/MEDIA `^...$` lesson).

**Files**
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (the 0b FUEL println —
  locate with `grep -n '"FUEL ' crates/jumpgate-core/examples/trophic_run.rs`)
- Modify: `python/analysis/sweep_trophic.py` (`FUEL_RE`)

- [ ] **Step 1: Demonstrate the gap** (printer surfaces verify by anchored
  output, not cargo test):

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 6000 | grep '^FUEL ' | tee /tmp/fuel_line_before.txt
```

Expected: one FUEL line containing the 0b hauler fields and NO `refuels=`
token (`grep -c 'refuels=' /tmp/fuel_line_before.txt` prints `0`).

- [ ] **Step 2: Append the tokens.** In `main`, immediately before the 0b FUEL
  `println!`, compute the run totals off the per-window samples (Task 2.8
  fields):

```rust
    // Refuel run totals (world-gets-big spec §8 — the FUEL fields deferred
    // from phase 0b; they exist only once the mechanic does). The 0 sentinel
    // stays honest: refuels=0 on a RefuelCfg-off arm means "mechanic dark",
    // on frontier "nobody bought" (texture, not failure).
    let refuels_total: u64 = samples.iter().map(|s| u64::from(s.refuels)).sum();
    let refuel_spend_total: i64 = samples.iter().map(|s| s.refuel_spend_micros).sum();
```

Then extend the FUEL format string by appending, at the very END (before the
closing quote), exactly:

```text
 refuels={} refuel_spend_micros={}
```

and append `refuels_total, refuel_spend_total,` at the end of the println's
argument list. Do not reorder or rename any existing token.

- [ ] **Step 3: Extend `FUEL_RE` in the same commit.** In
  `python/analysis/sweep_trophic.py`, insert immediately before the regex's
  closing `$` anchor (keeping the 0b body byte-identical):

```python
    r"(?: refuels=(?P<refuels>\d+) refuel_spend_micros=(?P<refuel_spend_micros>-?\d+))?"
```

This is the version gate: pre-refuel banked stdout (no tail) and post-refuel
stdout (tail present) both match; consumers read the two named groups as
`None`-able.

- [ ] **Step 4: Verify both line generations parse:**

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 6000 | grep '^FUEL ' | tee /tmp/fuel_line_after.txt
sed 's/ refuels=.*$//' /tmp/fuel_line_after.txt > /tmp/fuel_line_legacy.txt
python3 - <<'EOF'
import sys
sys.path.insert(0, "python/analysis")
from sweep_trophic import FUEL_RE
new = open("/tmp/fuel_line_after.txt").read().strip()
old = open("/tmp/fuel_line_legacy.txt").read().strip()
m_new = FUEL_RE.match(new)
m_old = FUEL_RE.match(old)
assert m_new and m_new.group("refuels") is not None, f"new line must carry refuels: {new}"
assert m_old and m_old.group("refuels") is None, f"legacy line must still parse: {old}"
print("FUEL_RE: new line OK, legacy (pre-refuel) line OK")
EOF
```

Expected output: `FUEL_RE: new line OK, legacy (pre-refuel) line OK`. On
trophic the new tokens read `refuels=0 refuel_spend_micros=0` (RefuelCfg off —
the named inertness gate).

- [ ] **Step 5: Full suite + lint, then commit (line + regex together — the
  lockstep rule):**

```
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/examples/trophic_run.rs python/analysis/sweep_trophic.py
git commit -F - <<'EOF'
feat(lab): FUEL line gains refuels/refuel_spend_micros; FUEL_RE optional tail

The spec-§8 fields deferred from phase 0b land with the mechanic; the regex
tail is optional so banked pre-refuel stdout still parses (version-gated
parsing, lockstep commit). Recorded windows, never gates.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.10: NEW frontier trajectory golden (printer + pinned 2000-tick state-hash test)

Spec §9: exactly one NEW frontier trajectory golden this rung. No stepped
golden exists today — existing goldens pin tick-0 hashes only (hash.rs:1101,
1224); `tests/physics_sanity.rs` is bounded-not-golden and
`tests/replay_equivalence.rs` compares runs to each other. Form: build
`scenario_frontier(7)`, step 2_000 ticks (one window — the phase-1 digest-test
duration precedent), pin `state_hash`. The literal comes from an `#[ignore]`
printer (the `print_golden` pattern, hash.rs:1111-1117) — NEVER from this plan.

**Files**
- Modify: `crates/jumpgate-core/src/scenario.rs` (tests mod, next to
  `scenario_frontier_shape`; `use crate::world::World` is already in scope at
  the tests-mod head)

- [ ] **Step 1: Add the printer and run it** (the derivation step — printer
  first, by the golden discipline):

```rust
    #[test]
    #[ignore = "prints the golden constant for frontier_trajectory_golden"]
    fn print_golden_frontier() {
        let (mut w, _) =
            World::reset(scenario_frontier(7)).expect("scenario_frontier must resolve");
        let mut cmds = Vec::new();
        for _ in 0..2_000 {
            w.step(&mut cmds);
        }
        println!("FRONTIER_TRAJECTORY_GOLDEN=0x{:016x}", crate::hash::state_hash(&w));
    }
```

```
cargo test -p jumpgate-core print_golden_frontier -- --ignored --nocapture
```

Expected output: one line `FRONTIER_TRAJECTORY_GOLDEN=0x<16 hex digits>`.
Record it — the next step pastes it verbatim.

- [ ] **Step 2: Add the pinned test, pasting the Step-1 output** (the literal
  below is written by the BUILDER from the printer output, never typed from
  this plan):

```rust
    /// The NEW frontier trajectory golden (world-gets-big spec §9): seed-7
    /// `scenario_frontier` stepped 2_000 ticks (one window), state_hash
    /// pinned. Existing goldens pin tick-0 worlds only; this pins a STEPPED
    /// big-map trajectory so physics/stage/config drift on the frontier is
    /// loud. Re-derive ONLY via `print_golden_frontier` (single-cause re-pin
    /// commits; the calibration v_e bake is the one scheduled re-pin).
    // PINNED from print_golden_frontier output, pre-calibration hauler v_e
    // prior (1.0).
    const FRONTIER_TRAJECTORY_GOLDEN: u64 = /* paste the Step-1 printer hex here */;

    #[test]
    fn frontier_trajectory_golden() {
        let (mut w, _) =
            World::reset(scenario_frontier(7)).expect("scenario_frontier must resolve");
        let mut cmds = Vec::new();
        for _ in 0..2_000 {
            w.step(&mut cmds);
        }
        assert_eq!(
            crate::hash::state_hash(&w),
            FRONTIER_TRAJECTORY_GOLDEN,
            "frontier trajectory drifted: re-pin only if intentional (single-cause commit, \
             re-derive via print_golden_frontier)"
        );
    }
```

- [ ] **Step 3: Run — pass — and confirm zero existing goldens moved:**

```
cargo test -p jumpgate-core frontier_trajectory_golden
cargo test -p jumpgate-core golden
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: `frontier_trajectory_golden ... ok`; `state_hash_golden_zero_world`,
`golden_zero_state_hash`, `config_hash_golden_anchor_is_stable` all unchanged
and green; `HASH_FORMAT_VERSION` stays 5.

- [ ] **Step 4: Commit:**

```
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
test(frontier): NEW frontier trajectory golden (2000-tick state_hash pin)

The spec-§9 budgeted new golden: seed-7 scenario_frontier stepped one window,
literal derived via the print_golden_frontier ignored printer (never
hand-computed). Zero existing goldens move; HASH_FORMAT_VERSION stays 5.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.11: `craft.fuel_capacity_scale` apply_knob arm

The calibration lever (spec §4 step 3): scale every craft's tank AND starting
fuel so endurance provably exceeds run length (the burn tail uncorrupted).
`apply_knob` lives at `crates/jumpgate-core/src/scenario.rs:260-330`; unknown/
malformed values are loud errors by design (:258-259). No craft-spec knob
exists yet — the dispatch arms (`cfg.dispatch_cfg.demand_low`, :313-315) are
the direct-`cfg`-access shape to clone. Knobs mutate config pre-reset, so
config_hash changes per arm exactly like every other knob; `GOLDEN_CONFIG_HASH`
pins `sample()`, not knobbed configs — it does not move.

**Files**
- Modify: `crates/jumpgate-core/src/scenario.rs` (the `apply_knob` match;
  `apply_knob_overrides_and_rejects_unknown`-style test :489-514)

- [ ] **Step 1: Write the failing test** in the scenario.rs tests mod:

```rust
    #[test]
    fn fuel_capacity_scale_knob_scales_every_tank() {
        // World-gets-big spec §4 step 3: the calibration ensemble's lever —
        // scales capacity AND starting fuel (full-tank starts preserved) so
        // endurance exceeds run length and the burn tail is uncorrupted.
        let mut cfg = scenario_trophic(7);
        let base: Vec<(f64, f64)> =
            cfg.craft.iter().map(|c| (c.spec.base_fuel_capacity, c.fuel_mass)).collect();
        apply_knob(&mut cfg, "fuel_capacity_scale", "100").expect("knob applies");
        for (c, (cap0, fuel0)) in cfg.craft.iter().zip(&base) {
            assert_eq!(c.spec.base_fuel_capacity, cap0 * 100.0, "capacity scaled");
            assert_eq!(c.fuel_mass, fuel0 * 100.0, "starting fuel scaled");
        }
        // Loud on nonsense (the sweep-grid-poison rule).
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "0").is_err(), "zero is loud");
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "-1").is_err(), "negative is loud");
        assert!(apply_knob(&mut cfg, "fuel_capacity_scale", "nan").is_err(), "NaN is loud");
    }
```

- [ ] **Step 2: Run and watch it fail** (the unknown-knob error is the loud
  default):

```
cargo test -p jumpgate-core fuel_capacity_scale_knob_scales_every_tank
```

Expected: panic `knob applies: "--set fuel_capacity_scale: unknown knob"`.

- [ ] **Step 3: Add the arm** to the `apply_knob` match, after the MediaCfg
  arms and before the `other =>` catch-all:

```rust
        // Craft-spec knobs (world-gets-big spec §4 — calibration levers).
        // Scales EVERY craft's tank and starting fuel together (full-tank
        // starts preserved; pirates' x10 endurance ratio preserved). Zero /
        // negative / non-finite would silently kill the FuelEmpty edge
        // across a whole grid — loud instead.
        "fuel_capacity_scale" => {
            let scale: f64 = p(name, value)?;
            if !(scale.is_finite() && scale > 0.0) {
                return Err(format!("--set {name}={value}: scale must be finite and > 0"));
            }
            for c in &mut cfg.craft {
                c.spec.base_fuel_capacity *= scale;
                c.fuel_mass *= scale;
            }
        }
```

- [ ] **Step 4: Run — pass — then full suite:**

```
cargo test -p jumpgate-core fuel_capacity_scale_knob_scales_every_tank
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: all green; `config_hash_golden_anchor_is_stable` untouched (the knob
adds no config field — it mutates existing ones).

- [ ] **Step 5: Commit:**

```
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(scenario): fuel_capacity_scale apply_knob arm (calibration lever)

Scales every craft's base_fuel_capacity AND fuel_mass together (full-tank
starts preserved); loud on zero/negative/non-finite (sweep-poison rule). No
config fields added; goldens untouched.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.12: Calibration ensemble → bake hauler v_e (OD-5b, k=2.5) → re-pin the frontier golden → scale-1 sanity

Spec §4 step 3 + §9 phase-2 tail. The chain: 20-seed `scenario_frontier`
ensemble at `fuel_capacity_scale=100` (endurance provably exceeds run length —
arithmetic below — so no leg is ever truncated by an empty tank), read the W8
worst HAULER-leg burn, derive `v_e = k × B_worst / tank` with the owner's
k = 2.5 applied to the MEASURED burn (never spec arithmetic), bake into the
factory WITH the derivation in the doc comment, re-derive the frontier
trajectory golden (the bake moves it — the one scheduled second pin), then
re-run at scale=1 and RECORD the W9/W10 readings (windows, never gates).

Endurance arithmetic (the "provably exceeds" claim, recorded in the bank):
at the v_e prior 1.0, burn = thrust/v_e × dt = 1e-12/1.0 × 0.25 = 2.5e-13 per
full-throttle tick; scaled tank = 100 × 1e-9 = 1e-7 → 400,000 full-throttle
ticks ≫ the 100,000-tick run, even before duty < 100%.

**Files**
- Modify: `crates/jumpgate-core/src/scenario.rs` (`scenario_frontier` hauler
  `base_exhaust_velocity`; `FRONTIER_TRAJECTORY_GOLDEN` re-pin)
- Create: `docs/superpowers/posts/2026-06-XX-world-gets-big-calibration/calibration.md`
  (the capture practice — `/tmp` is volatile; substitute the actual date)

- [ ] **Step 1: Pre-flight.** Confirm the phase-2-first-half surfaces this task
  drives are landed:

```
cargo test -p jumpgate-core scenario_frontier
grep -n 'scenario' crates/jumpgate-core/examples/trophic_run.rs | grep -i 'frontier\|--scenario'
grep -n '"FUEL ' crates/jumpgate-core/examples/trophic_run.rs
```

Expected: the frontier factory tests green; the runner's `--scenario` flag
present (frontier arm); the FUEL println present with the Task-2.9 tokens.
Identify the per-leg burn surface: the 0b FUEL median is computed from a
pooled per-leg burn collection (the MEDIA lag-pool pattern,
trophic_run.rs:403-409) —

```
grep -n 'leg_burn' crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/examples/trophic_run.rs
```

Record the TrophicSample per-leg-burn field name and its JSONL key (it is
dumped field-for-field by `sample_json`); call it `<LEG_BURN_KEY>` below.

- [ ] **Step 2: Instrument-resolution sanity (one seed).** Burn permille is
  measured against the SCALED tank at scale=100, so the expected per-leg read
  is small (analytic: worst leg ~1010 ticks × 2.5e-13 ≈ 2.5e-10 ≈ 2–3 permille
  of 1e-7). Confirm it is nonzero before trusting the ensemble:

```
mkdir -p runs/wgb-calibration
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 100000 \
  --set fuel_capacity_scale=100 --jsonl runs/wgb-calibration/cal-s7.jsonl \
  | grep -E '^(META|RESULT|FUEL) ' | tee runs/wgb-calibration/cal-s7.txt
```

Expected: the FUEL line's per-leg burn fields read ≥ 1 (permille of the scaled
tank) and `RESULT ... fuel_empty=0` (no leg truncated — instrument validity,
not a play gate). If the per-leg read is 0, the burn unit quantized the signal
away: STOP and surface to the orchestrator (the unit choice is 0b's; do not
bake from a dead instrument).

- [ ] **Step 3: Run the 20-seed ensemble:**

```
for seed in $(seq 1 20); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed "$seed" --ticks 100000 \
    --set fuel_capacity_scale=100 \
    --jsonl "runs/wgb-calibration/cal-s$seed.jsonl" \
    | grep -E '^(META|RESULT|FUEL) ' >> runs/wgb-calibration/scale100.txt
done
grep -c '^FUEL ' runs/wgb-calibration/scale100.txt
```

Expected: `20` FUEL lines; every RESULT line shows `fuel_empty=0` (400k-tick
endurance vs 100k run — the burn tail is uncorrupted). 100,000 ≤ the frontier
`ephemeris_window` 120,000, so the runner guard stays quiet.

- [ ] **Step 4: Extract the measured worst HAULER-leg burn** (max over all
  seeds and legs of the pooled per-leg burns — substitute `<LEG_BURN_KEY>`
  from Step 1):

```
python3 - <<'EOF'
import json, glob
KEY = "<LEG_BURN_KEY>"  # from Step 1's grep — the 0b per-leg burn JSONL key
worst, where = 0, None
for path in sorted(glob.glob("runs/wgb-calibration/cal-s*.jsonl")):
    for line in open(path):
        row = json.loads(line)
        for v in row.get(KEY, []):
            if v > worst:
                worst, where = v, (path, row["tick"])
print(f"worst hauler-leg burn = {worst} permille of the SCALED tank, at {where}")
EOF
```

Expected: a small integer `P` (analytic prior says 2–4) with its provenance.
Record `P` and the seed/window.

- [ ] **Step 5: Derive and bake v_e.** Arithmetic (write it into the bank AND
  the doc comment):

```text
B_worst (mass)  = P × scaled_tank / 1000 = P × 1e-7 / 1000 = P × 1e-10
v_e (baked)     = k × B_worst / tank × v_e_prior
                = 2.5 × (P × 1e-10) / 1.0e-9 × 1.0
                = 0.25 × P
```

Edit `scenario_frontier`'s HAULER spec in
`crates/jumpgate-core/src/scenario.rs`: replace the prior
`base_exhaust_velocity: 1.0` with the derived value, carrying the derivation
(values filled from Steps 3–4, like a golden paste — never invented):

```rust
            // CALIBRATED, not designed (world-gets-big spec §4 step 3, OD-5b:
            // k = 2.5 applied to the MEASURED worst hauler-leg burn, never
            // spec arithmetic). Instrument: 20-seed scenario_frontier
            // ensemble, --set fuel_capacity_scale=100 (endurance 400k
            // full-throttle ticks >> the 100k-tick run: burn tail
            // uncorrupted), banked at docs/superpowers/posts/
            // 2026-06-XX-world-gets-big-calibration/. Measured worst
            // hauler-leg burn: <P> permille of the scaled tank (seed <S>)
            // = <P>e-10 fuel mass. Bake: v_e = 2.5 * <P>e-10 / 1.0e-9 * 1.0.
            // Was the analytic prior 1.0. Pirates keep v_e 20.0 per-craft
            // (OD-6 — the x10 endurance spec, no taste scalar).
            base_exhaust_velocity: <0.25 * P, written as the literal>,
```

- [ ] **Step 6: Re-derive the frontier trajectory golden** (the bake moves it —
  the ONE scheduled second pin; single cause):

```
cargo test -p jumpgate-core print_golden_frontier -- --ignored --nocapture
```

Paste the printed hex over `FRONTIER_TRAJECTORY_GOLDEN` and update its
provenance comment to the re-pin format:

```rust
    // RE-PINNED: hauler v_e calibration bake (OD-5b, k=2.5 x measured worst
    // leg burn — see the scenario_frontier doc comment). Was 0x<old literal>.
    const FRONTIER_TRAJECTORY_GOLDEN: u64 = /* paste the printer hex */;
```

- [ ] **Step 7: Verify the budget held:**

```
cargo test -p jumpgate-core frontier_trajectory_golden
cargo test -p jumpgate-core golden
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Expected: frontier golden green on the new literal; `GOLDEN_CONFIG_HASH`,
`GOLDEN_ZERO_STATE_HASH`, `state_hash_golden_zero_world` ALL unchanged (the
bake touches only `scenario_frontier`, not `sample()` or the zero-world
fixtures); trophic digest/replay tests green (W12: the control stays a
control).

- [ ] **Step 8: Commit the bake + re-pin (one single-cause commit):**

```
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(frontier): bake calibrated hauler v_e (OD-5b k=2.5 x measured worst leg)

Derived from the 20-seed fuel_capacity_scale=100 ensemble (derivation in the
factory doc comment); FRONTIER_TRAJECTORY_GOLDEN re-pinned via
print_golden_frontier — single cause, old literal in the provenance comment.
Zero other goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 9: Scale-1 sanity ensemble (RECORDED, never gated).** Re-run the
  20 seeds with no knob — the world as players meet it:

```
for seed in $(seq 1 20); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed "$seed" --ticks 100000 \
    | grep -E '^(META|RESULT|FUEL) ' >> runs/wgb-calibration/scale1.txt
done
grep '^FUEL ' runs/wgb-calibration/scale1.txt
grep '^RESULT ' runs/wgb-calibration/scale1.txt
```

Record per seed: strandings, adrift_end, refuels, refuel_spend_micros,
fuel_empty. The pre-registered W9 window is 0–2 strandings/run and `fuel_empty`
on frontier means texture ("no stranding this seed") — ANY observed value is a
finding for the owner console session, not a pass/fail. If the median
strandings reads ≥ ~2/20 haulers lost per run, note the spec-§5 revisit
trigger (rescue/salvage is the named deferral) in the bank — still a recording.

- [ ] **Step 10: Bank the calibration artifact (the capture practice — /tmp
  and runs/ are volatile/unstaged).** Write
  `docs/superpowers/posts/2026-06-XX-world-gets-big-calibration/calibration.md`
  containing: the exact commands run, the endurance arithmetic, the per-seed
  FUEL/RESULT lines from BOTH ensembles (paste the two .txt banks), the
  measured `P` + provenance, the derivation line, the baked v_e, both golden
  literals (old → new), and the W9/W10 readings table. Then commit:

```
git add docs/superpowers/posts/2026-06-XX-world-gets-big-calibration/calibration.md
git commit -F - <<'EOF'
docs(calibration): bank the world-gets-big v_e calibration ensemble + readings

20-seed scale=100 burn measurement, OD-5b derivation, scale=1 W9/W10 readings
(recorded windows, never gates). Same-day capture practice; runs/ never staged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```
