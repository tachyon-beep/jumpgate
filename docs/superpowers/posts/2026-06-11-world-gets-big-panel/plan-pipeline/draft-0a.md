# Phase 0a — the haven-lurk leak fix

Spec: `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` §6 (first
bullet, TROPHIC-C3) + §9 ("Phase 0a: haven-lurk fix (single-cause behavior
commit; console re-judgment scheduled; no literals move)"). Repo HEAD e7e490e.

**The leak (main-loop verified):** in `run_pirate_brains`
(`crates/jumpgate-core/src/pirate.rs:578-599`), a post-refuge pirate still
`Seeking { Body(hideout) }` resolves `nav_lurk = Some(haven_row)` at :578-583
and adopts it unchecked at :585 — the `haven_station` exclusion is passed only
into the fresh-draw arm (:592) and the hungry-relocation draw (:622). The fix
is one upstream filter: `nav_lurk == haven_station` → treated as `None` → the
existing `None` arm performs the fresh reach-bounded draw anchored at
`ships.pos[row]`.

**Scope guards for this phase (the plan executor must respect all four):**

- Single-cause behavior commit. Nothing else rides along.
- Do NOT touch `relocate_lurk_target` (pirate.rs:452-477) — its geometry-only
  signature is pinned by `pirates_are_information_blind` (pirate.rs:1307).
- Do NOT fix the stale marooned doc comment at pirate.rs:443-445 ("none in
  reach -> the NEAREST station" — stale; the body does a uniform breakout).
  Spec §6 attaches that doc fix to the phase-2 explicit-reach factory commit
  ("Reach 0.6 set EXPLICITLY in both factories …; the stale 'nearest station'
  marooned doc fixed in the same commit"). Phase 0a stays pure.
- No golden literals move. This fix changes Piracy-stream draw COUNT on
  post-refuge ticks, so *state-hash trajectories* diverge from banked
  baselines (`runs/` artifacts — never staged, never "fixed"), but the pinned
  goldens (`GOLDEN_CONFIG_HASH = 0xee02_df67_1889_78dc` at config.rs:745, the
  zero-world state-hash constants in hash.rs) fold no stepped pirate world and
  MUST NOT change. `HASH_FORMAT_VERSION` stays 5. If any golden test fails
  after this fix, STOP and debug — do not re-pin.

---

### Task 0a.1: Haven-lurk leak fix — the nav-derived lurk respects the haven exclusion

**Files**

- Modify: `crates/jumpgate-core/src/pirate.rs`
  - fix site: the `nav_lurk` adoption in `run_pirate_brains` (:578-585 — insert
    one filter between the `nav_lurk` binding at :578-583 and the
    `let mut lurk = match nav_lurk` at :584)
  - tests: `mod tests`, inserted immediately after
    `fed_pirate_camps_hungry_pirate_roams` (ends :1791) and before
    `lying_low_pirate_seeks_hideout` (:1793)

No other file changes. `git add` this one path only.

- [ ] **Step 1: Write the failing post-refuge adoption test.**

  In `crates/jumpgate-core/src/pirate.rs` `mod tests`, immediately after
  `fed_pirate_camps_hungry_pirate_roams` (after line 1791), add. All names used
  (`RunConfig`, `World`, `NavState`, `NavDest`, `EntityRef`, `BodyId`, `Tick`,
  `Vec3`) are already in scope via the existing `mod tests` imports — add no
  new `use` lines.

  ```rust
      #[test]
      fn post_refuge_pirate_never_adopts_the_haven_lurk() {
          // Spec §6 (TROPHIC-C3, phase 0a): a post-refuge pirate whose nav
          // still resolves the hideout BODY must not inherit the HAVEN station
          // as its hunting lurk — the nav-derived lurk path must respect the
          // same exclusion that guards fresh draws ("a pirate does not rob
          // where it fences"). Pre-fix this is the rob-where-you-fence
          // attractor inside every banked baseline.
          fn cfg() -> RunConfig {
              let mut cfg = pirate_world_cfg();
              cfg.contracts = vec![];
              cfg.craft = vec![pirate_init(Vec3::ZERO)]; // lone pirate, row 0
              // Body 0 (origin) hosts station 0 -> the haven is station 0.
              cfg.trophic.hideout_body_index = 0;
              cfg.trophic.upkeep_per_tick = 0; // hold the FED state constant
              cfg
          }
          let c = cfg();
          let grubstake = c.trophic.grubstake_micros;
          let (mut world, _) = World::reset(c).expect("resolvable cfg");
          let hideout = world
              .bodies
              .ids
              .id_at(0)
              .map(|(slot, generation)| BodyId { slot, generation })
              .unwrap();
          // Construct the post-refuge state: refuge EXPIRED, nav still routed
          // at the hideout body (exactly what the lie-low arm leaves behind),
          // FED (food >= grubstake) so the hungry-relocation arm never runs —
          // the nav-adoption path is the ONLY draw under test.
          {
              let p = world.ships.pirate[0].as_mut().unwrap();
              p.lie_low_until = Tick(0);
              p.food_micros = grubstake;
          }
          world.ships.nav[0] = NavState::Seeking {
              dest: NavDest::Entity(EntityRef::Body(hideout)),
              dv_remaining: 1.0,
          };
          let lurk_body = |w: &World| match w.ships.nav[0] {
              NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => Some(b),
              _ => None,
          };
          for _ in 0..8 {
              world.step(&mut Vec::new());
              assert_ne!(
                  lurk_body(&world),
                  Some(hideout),
                  "post-refuge pirate adopted the HAVEN as its lurk \
                   (the rob-where-you-fence leak)"
              );
          }
      }
  ```

  Why this construction is the leak: `pirate_world_cfg()` has station 0 on
  body 0 (origin) and station 1 on body 1 at 0.3 AU; default
  `pirate_max_reach_au` 0.6 means the post-fix fresh draw (anchor =
  `ships.pos[row]` = origin) has exactly one huntable in-reach candidate —
  station 1 — so the post-fix expectation is deterministic regardless of the
  Piracy word (`u % 1 == 0`).

- [ ] **Step 2: Write the failing marooned-exclusion test (fresh-draw
  exclusion still holds through the new path, including the breakout arm).**

  Directly below the Step-1 test, add:

  ```rust
      #[test]
      fn post_refuge_redraw_excludes_haven_even_when_marooned() {
          // Spec §6: the post-refuge redraw goes through relocate_lurk_target
          // with the haven EXCLUDED — when the haven is the only station in
          // reach, the draw falls through to the marooned BREAKOUT (uniform
          // over all huntable stations) rather than back onto the haven. This
          // is the spec's stated cost, owned: on today's band most post-refuge
          // draws become map-wide breakouts (console re-judgment scheduled).
          let mut cfg = pirate_world_cfg();
          cfg.contracts = vec![];
          cfg.craft = vec![pirate_init(Vec3::ZERO)]; // lone pirate, row 0
          cfg.trophic.hideout_body_index = 0; // haven = station 0 at the origin
          cfg.trophic.upkeep_per_tick = 0; // hold the FED state constant
          cfg.bodies[1].elements.a = 5.0; // station 1 beyond reach (0.6 AU)
          let grubstake = cfg.trophic.grubstake_micros;
          let (mut world, _) = World::reset(cfg).expect("resolvable cfg");
          let hideout = world
              .bodies
              .ids
              .id_at(0)
              .map(|(slot, generation)| BodyId { slot, generation })
              .unwrap();
          let far_body = world
              .bodies
              .ids
              .id_at(1)
              .map(|(slot, generation)| BodyId { slot, generation })
              .unwrap();
          {
              let p = world.ships.pirate[0].as_mut().unwrap();
              p.lie_low_until = Tick(0);
              p.food_micros = grubstake;
          }
          world.ships.nav[0] = NavState::Seeking {
              dest: NavDest::Entity(EntityRef::Body(hideout)),
              dv_remaining: 1.0,
          };
          world.step(&mut Vec::new());
          assert!(
              matches!(
                  world.ships.nav[0],
                  NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. }
                      if b == far_body
              ),
              "marooned post-refuge pirate must break out to the non-haven \
               station, got {:?}",
              world.ships.nav[0]
          );
      }
  ```

- [ ] **Step 3: Run the new tests and confirm BOTH fail for the leak reason.**

  ```
  cargo test -p jumpgate-core post_refuge
  ```

  Expected: `test result: FAILED. 0 passed; 2 failed`. The first panics with

  ```
  assertion `left != right` failed: post-refuge pirate adopted the HAVEN as its lurk (the rob-where-you-fence leak)
    left: Some(BodyId { slot: 0, generation: 0 })
   right: Some(BodyId { slot: 0, generation: 0 })
  ```

  and the second with `marooned post-refuge pirate must break out to the
  non-haven station, got Seeking { dest: Entity(Body(BodyId { slot: 0,
  generation: 0 })), .. }`. If either test PASSES here, stop — the
  construction missed the adoption path (check that `lie_low_until` is
  expired and the nav was overwritten to the hideout body), do not proceed.

- [ ] **Step 4: Minimal fix — filter the haven out of the nav-derived lurk.**

  In `run_pirate_brains`, between the `nav_lurk` binding (ends pirate.rs:583
  with `};`) and `let mut lurk = match nav_lurk {` (:584), insert the filter
  so the block reads:

  ```rust
          let nav_lurk: Option<usize> = match ships.nav[row] {
              NavState::Seeking { dest: NavDest::Entity(EntityRef::Body(b)), .. } => {
                  (0..stations.ids.len()).find(|&s| stations.body[s] == b)
              }
              _ => None,
          };
          // TROPHIC-C3 (spec §6, phase 0a): the haven is NEVER a lurk — not
          // even by nav inheritance. A post-refuge pirate still
          // Seeking{Body(hideout)} would otherwise ADOPT the haven station
          // here, bypassing the exclusion that guards only the fresh and
          // relocation draws ("a pirate does not rob where it fences").
          // Treat a haven nav_lurk as None: the arm below performs the fresh
          // reach-bounded draw from the pirate's current position (marooned
          // breakout when nothing else is in reach).
          let nav_lurk = nav_lurk.filter(|&s| Some(s) != haven_station);
          let mut lurk = match nav_lurk {
              Some(s) => s,
  ```

  That is the entire behavior change. No signature changes, no event emits
  (LurkMoved is phase 2), no changes inside `relocate_lurk_target`, no config
  fields.

- [ ] **Step 5: Run the new tests and confirm both pass.**

  ```
  cargo test -p jumpgate-core post_refuge
  ```

  Expected: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 6: Confirm the pinned exclusion/blindness/hunger behavior is
  untouched (the existing unit pins still hold).**

  ```
  cargo test -p jumpgate-core relocation_respects_reach
  cargo test -p jumpgate-core pirates_are_information_blind
  cargo test -p jumpgate-core fed_pirate_camps_hungry_pirate_roams
  cargo test -p jumpgate-core replay_bit_identical_with_piracy_draws
  ```

  Expected: each reports `test result: ok. 1 passed`.
  `relocation_respects_reach` pins both exclusion arms of
  `relocate_lurk_target` with exact indices (pirate.rs:1664-1668) — it must
  pass WITHOUT edits, proving the fix lives upstream of the draw fn.
  `fed_pirate_camps_hungry_pirate_roams` uses `hideout_body_index = 99`
  (out-of-range hideout ⇒ `haven_station = None` ⇒ the new filter is a no-op)
  — it must pass without edits; do NOT "fix" the out-of-range hideout into an
  error, it is legal spec-§8 degrade behavior the test exploits.

- [ ] **Step 7: Assert no golden literals moved.**

  ```
  cargo test -p jumpgate-core golden
  ```

  Expected: `config_hash_golden_anchor_is_stable`,
  `state_hash_golden_zero_world`, and `golden_zero_state_hash` all pass
  (`print_golden` / `print_golden_config` show as ignored). The diff of this
  commit must contain NO edits to `GOLDEN_CONFIG_HASH`
  (config.rs:745, currently `0xee02_df67_1889_78dc`), no edits to the hash.rs
  golden constants, and no `HASH_FORMAT_VERSION` change (stays 5). If any
  golden test fails, STOP and debug the fix — re-pinning is forbidden in this
  phase (spec §9: "no literals move").

- [ ] **Step 8: Full verification.**

  ```
  cargo test --workspace
  cargo clippy --all-targets -- -D warnings
  ```

  Expected: all green, no warnings. If a pre-existing world-level test fails,
  that is a real behavioral coupling this fix exposed — investigate it as a
  finding (systematic debugging), never nudge a fixture to silence it, and do
  not widen this commit; surface it before committing.

- [ ] **Step 9: Commit (single-cause behavior commit; explicit path; never
  `runs/`).**

  ```bash
  cd /home/john/jumpgate
  git add crates/jumpgate-core/src/pirate.rs
  git status --short   # verify: exactly one staged file, nothing from runs/
  git commit -F - <<'EOF'
  fix(pirate): post-refuge nav_lurk never adopts the haven (TROPHIC-C3)

  Phase 0a of the world-gets-big rung (spec §6 first bullet, §9). A
  post-refuge pirate still Seeking{Body(hideout)} resolved the haven
  station through the nav-derived lurk path, bypassing the haven
  exclusion that guards only fresh draws — contradicting the code's own
  doc ("a pirate does not rob where it fences") and seeding a
  self-reinforcing rob-where-you-fence attractor inside every banked
  baseline. nav_lurk == haven_station is now treated as None, so the
  existing None arm performs the fresh reach-bounded draw from the
  pirate's current position (marooned breakout when nothing else is in
  reach). The fix is upstream of relocate_lurk_target, whose
  geometry-only signature is unchanged.

  BEHAVIOR COMMIT — the judged band changes: on today's band ~86% of
  post-refuge draws become map-wide breakouts, and the extra Piracy
  draws shift state-hash trajectories away from banked baselines. A
  console re-judgment session is scheduled, and the 6-station HHI/slack
  calibrations (contaminated by the leak) will be re-fitted on both maps
  post-fix. No golden literals move: GOLDEN_CONFIG_HASH and the
  zero-world state-hash goldens are untouched, HASH_FORMAT_VERSION
  stays 5.

  Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
  EOF
  ```
