# The World Gets Big (scenario_frontier) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the frontier rung — a 10-station geometric map (0.35→3.0 AU) with partitioned tier loops, a real propellant economy (eps re-bake + refuel verb + first live Fuel price), the haven-lurk leak fix, and the lab instruments/panels to read it — per the APPROVED spec docs/superpowers/specs/2026-06-11-world-gets-big-design.md (OD-1..7 resolved).

**Architecture:** Five landing phases (0a leak fix → 0b instruments-before-mechanics → 1 eps+refuel gated off at lot 0 → 2 scenario_frontier factory + calibration-derived v_e bake → 3 dual-map re-fit, panels, headline grid, console). Hash discipline: no HASH_FORMAT_VERSION bump; exactly one GOLDEN_CONFIG_HASH re-pin (RefuelCfg) and one new frontier trajectory golden (re-pinned once more by the v_e bake, cause documented); trophic stays bit-identical (cross-branch digest is the phase-1 exit).

**Tech Stack:** Rust 2024 (jumpgate-core), examples/trophic_run.rs runner, Python panels (python/, pytest with PYTHONPATH), deterministic seeded ensembles.

---

## Phase 0a — the haven-lurk leak fix

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

> **Implementation status 2026-06-12 (Codex):** complete through verification.
> Steps 1-8 are done and evidenced below. Step 9 remains intentionally open
> because this run did not create a commit.

**Files**

- Modify: `crates/jumpgate-core/src/pirate.rs`
  - fix site: the `nav_lurk` adoption in `run_pirate_brains` (:578-585 — insert
    one filter between the `nav_lurk` binding at :578-583 and the
    `let mut lurk = match nav_lurk` at :584)
  - tests: `mod tests`, inserted immediately after
    `fed_pirate_camps_hungry_pirate_roams` (ends :1791) and before
    `lying_low_pirate_seeks_hideout` (:1793)

No other file changes. `git add` this one path only.

- [x] **Step 1: Write the failing post-refuge adoption test.**

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

- [x] **Step 2: Write the failing marooned-exclusion test (fresh-draw
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

- [x] **Step 3: Run the new tests and confirm BOTH fail for the leak reason.**

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

  **Completed 2026-06-12 (Codex):** `cargo test -p jumpgate-core post_refuge`
  failed RED as expected: 0 passed, 2 failed, both for the haven-adoption leak.

- [x] **Step 4: Minimal fix — filter the haven out of the nav-derived lurk.**

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

  **Completed 2026-06-12 (Codex):** inserted the upstream
  `nav_lurk.filter(|&s| Some(s) != haven_station)` guard in
  `run_pirate_brains`; no draw-function, config, event, or golden edits.

- [x] **Step 5: Run the new tests and confirm both pass.**

  ```
  cargo test -p jumpgate-core post_refuge
  ```

  Expected: `test result: ok. 2 passed; 0 failed`.

  **Completed 2026-06-12 (Codex):** `cargo test -p jumpgate-core post_refuge`
  passed GREEN: 2 passed, 0 failed.

- [x] **Step 6: Confirm the pinned exclusion/blindness/hunger behavior is
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

  **Completed 2026-06-12 (Codex):** all four targeted pins passed:
  `relocation_respects_reach`, `pirates_are_information_blind`,
  `fed_pirate_camps_hungry_pirate_roams`, and
  `replay_bit_identical_with_piracy_draws`.

- [x] **Step 7: Assert no golden literals moved.**

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

  **Completed 2026-06-12 (Codex):** `cargo test -p jumpgate-core golden`
  passed: 4 passed, 0 failed, 2 ignored; no golden literals were edited.

- [x] **Step 8: Full verification.**

  ```
  cargo test --workspace
  cargo clippy --all-targets -- -D warnings
  ```

  Expected: all green, no warnings. If a pre-existing world-level test fails,
  that is a real behavioral coupling this fix exposed — investigate it as a
  finding (systematic debugging), never nudge a fixture to silence it, and do
  not widen this commit; surface it before committing.

  **Completed 2026-06-12 (Codex):** initial full workspace run exposed
  `reseek_threshold_covers_dock` as a same-file fixture coupling to the default
  haven. The fixture now sets `hideout_body_index = 99` to keep that test scoped
  to loiter geometry. Clean verification after that: `cargo test --workspace`
  passed; `cargo clippy --all-targets -- -D warnings` passed.

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


---

## Phase 0b — lab instruments before mechanics (spec §8, §9 phase 0b)

> Frame (PDR-0006): every number these tasks emit is a designer's WINDOW, never a
> gate. No step below makes any metric pass/fail (determinism/unit tests and the
> pre-existing trophic-only `--assert-no-fuel-empty` endurance window excepted).
> Phase 0b touches **no hashed surface**: zero golden literals move, no
> `HASH_FORMAT_VERSION` bump, no config change, no reward surface. Everything is
> unhashed diagnostics + printer + parser. Sequencing: 0b.1 → 0b.2 (independent of
> fuel) and 0b.1 → 0b.3 → 0b.4 → 0b.5; 0b.6 last, and only after the phase-0a
> haven-lurk fix has landed (the baseline is POST-fix by definition).

---

### Task 0b.1: `permille_floor` — the pinned f64→fixed-point FLOOR seam

The fuel instruments and the META radii all need one f64→integer conversion. No
in-tree precedent exists (grounding: only integer milli division, e.g.
`trophic_run.rs:359`); the gotcha list requires the rounding form be pinned by a
new test. FLOOR is the spec-§7 pinned form (`tank_before_permille`, phase 1,
rides the same seam later).

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs` (new pub fn after the constants block ending at `:61`; test in the `tests` mod, after `sample_window_counts_purchases_and_reads_yard_treasury` ending `:894`)

- [ ] **Step 1: failing test first.** Add to `diagnostics.rs` `tests` mod:

```rust
    /// Phase-0b FLOOR pin (world-gets-big spec §7/§8): no f64→fixed-point
    /// precedent existed in-tree — this test pins the form for every fuel
    /// instrument (and the phase-1 `tank_before_permille`). FLOOR, never round.
    #[test]
    fn permille_floor_is_floor_never_round() {
        assert_eq!(permille_floor(0.35, 1.0), 350, "milli-AU radius read");
        assert_eq!(permille_floor(1.999, 1000.0), 1, "FLOOR, never round-half-up");
        assert_eq!(permille_floor(0.9999999, 1.0), 999, "sub-unit stays below 1000");
        assert_eq!(permille_floor(1.0, 1.0), 1000, "exact full tank reads 1000");
        assert_eq!(permille_floor(1.0, 0.0), 0, "zero denominator reads the 0 sentinel");
        assert_eq!(permille_floor(1.0, -1.0), 0, "negative denominator reads 0");
        assert_eq!(permille_floor(-1.0, 1.0), 0, "negative numerator clamps to 0");
        assert_eq!(permille_floor(f64::NAN, 1.0), 0, "non-finite reads the 0 sentinel");
    }
```

- [ ] **Step 2: run it, watch it fail.**
  `cargo test -p jumpgate-core permille_floor`
  Expected failure: compile error `error[E0425]: cannot find function `permille_floor` in this scope`.

- [ ] **Step 3: minimal implementation.** In `diagnostics.rs`, after the
  `OUTCOME_DISPERSION_MIN_MILLI` constant (`:61`), before `TrophicSample`:

```rust
/// FLOOR-rounded fixed-point read `⌊num/den × 1000⌋`, clamped to `u32` — the
/// ONE f64→integer seam for the fuel instruments (world-gets-big phase 0b;
/// spec §7 pins FLOOR). Milli-AU radii ride the same form
/// (`permille_floor(a_au, 1.0)`). Non-finite inputs, non-positive
/// denominators, and negative results read the 0 sentinel. Diagnostics-only:
/// never a behavior input, never hashed.
pub fn permille_floor(num: f64, den: f64) -> u32 {
    if !(den > 0.0) || !num.is_finite() {
        return 0;
    }
    let v = (num / den * 1000.0).floor();
    if v <= 0.0 {
        0
    } else if v >= u32::MAX as f64 {
        u32::MAX
    } else {
        v as u32
    }
}
```

- [ ] **Step 4: run it, watch it pass.**
  `cargo test -p jumpgate-core permille_floor`
  Expected: `test diagnostics::tests::permille_floor_is_floor_never_round ... ok` … `test result: ok.`
  Then `cargo clippy --all-targets -- -D warnings` — clean.

- [ ] **Step 5: commit (explicit paths only).**

```bash
git add crates/jumpgate-core/src/diagnostics.rs
git commit -F - <<'EOF'
feat(lab): permille_floor — the pinned FLOOR f64->fixed-point seam (phase 0b)

One conversion seam for every fuel instrument and the META radii; FLOOR
form pinned by test (no in-tree precedent existed). Diagnostics-only,
unhashed; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 0b.2: META anchored line + meta JSONL row + version-gated sweep parsing (the N_HAULERS mirror dies)

The runner starts printing config-derived population facts; the sweep stops
mirroring `scenario::NUM_HAULERS` in a Python constant (`sweep_trophic.py:70-72`,
used only in `voi_line`'s wallet slice at `:288`). Parsing becomes
presence-gated so banked pre-FUEL stdout/JSONL still parses — **this creates the
version-gating precedent** (none exists; grounding confirms `run_one` hard-exits
on any missing anchored line today, `sweep_trophic.py:111-116`). Lockstep rule:
the `META` println and `META_RE` land in this one commit.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs` (`RunProduct` `:105`, `simulate` `:109-139`, `main` `:321`, `:332-339` block, replay arm `:453`)
- Modify: `/home/john/jumpgate/python/analysis/sweep_trophic.py` (regexes `:51-72`, `run_one` `:89-118`, `panel` `:142-218`, `media_panel` `:221-265`, `voi_line` `:273-298`, `main` `:330-361`)
- Modify: `/home/john/jumpgate/python/analysis/media_log.py` (`:213` windows load)
- Create: `/home/john/jumpgate/python/tests/test_sweep_parsing.py`

- [ ] **Step 1: failing test first.** Create `python/tests/test_sweep_parsing.py`:

```python
"""Version-gated anchored-line parsing (world-gets-big phase 0b, spec §8).

Banked pre-FUEL stdout (RESULT+MEDIA only) must still parse: META (and later
FUEL) read None, never SystemExit. PRESENCE is the gate — this file creates
the precedent (none existed in python/analysis). Windows, not gates
(PDR-0006)."""

import importlib.util
import pathlib

_SPEC = importlib.util.spec_from_file_location(
    "sweep_trophic",
    pathlib.Path(__file__).resolve().parents[1] / "analysis" / "sweep_trophic.py",
)
sweep = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(sweep)

# v1 instrument format: the anchored set banked by the pirates+media rungs.
V1_STDOUT = """\
trophic_run: seed=7 ticks=50000 windows=25 (W=2000) sets=[]
RESULT seed=7 ticks=50000 verdict=Alive cycled=true risk_heterogeneous=true \
outcomes_disperse=true fuel_empty=0 robs=63 laden_trips=410 purchases=9
MEDIA seed=7 born=12 escaped_milli=833 median_lag=410 p90_lag=1290 reading=Localized
"""

# v2 (phase 0b): + META. (FUEL appends in its own lockstep commit.)
V2_STDOUT = V1_STDOUT + (
    "META seed=7 scenario=trophic stations=6 haulers=12 pirates_initial=6 "
    "station_radii_milli_au=[350, 560, 770, 980, 1190, 1400]\n"
)


def test_v1_banked_output_still_parses():
    parsed = sweep.parse_stdout(V1_STDOUT)
    assert parsed["result"]["verdict"] == "Alive"
    assert parsed["media"]["reading"] == "Localized"
    # The version gate: ABSENCE means v1 format, never an error.
    assert parsed["meta"] is None


def test_v2_meta_line_parses():
    parsed = sweep.parse_stdout(V2_STDOUT)
    assert parsed["meta"] is not None
    assert parsed["meta"]["scenario"] == "trophic"
    assert parsed["meta"]["stations"] == "6"
    assert parsed["meta"]["haulers"] == "12"
    assert parsed["meta"]["pirates_initial"] == "6"
    assert parsed["meta"]["radii"] == "350, 560, 770, 980, 1190, 1400"
```

- [ ] **Step 2: run it, watch it fail.**
  `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py`
  Expected failure: `AttributeError: module 'sweep_trophic' has no attribute 'parse_stdout'`.

- [ ] **Step 3: runner side — `MetaFacts` + the META line + the meta JSONL row.**
  In `trophic_run.rs`:

  (a) Replace the `RunProduct` alias (`:103-105`) and add the struct above it:

```rust
/// Config-derived facts for the META anchored line (world-gets-big phase 0b,
/// spec §8): computed in `simulate` from the resolved RunConfig BEFORE
/// `World::reset` consumes it; printed in `main` beside the other anchored
/// lines. `scenario` is hardcoded until the phase-2 `--scenario` flag exists —
/// that flag's value replaces this literal verbatim (same lockstep commit as
/// its sweep-side read).
struct MetaFacts {
    scenario: &'static str,
    stations: usize,
    haulers: usize,
    pirates_initial: usize,
    station_radii_milli_au: Vec<u32>,
}

/// `simulate`'s product: per-window samples, the sampled `(tick, state_hash)`
/// stream, the final world (chronicle + event counts), and the META facts.
type RunProduct = (Vec<TrophicSample>, Vec<(u64, u64)>, World, MetaFacts);
```

  (b) Add `CraftRole` to the existing `use jumpgate_core::{...}` list (`:30-33`).

  (c) In `simulate` (`:113-118`), after the `apply_knob` loop and before
  `World::reset` consumes `cfg`:

```rust
    let meta = MetaFacts {
        scenario: "trophic",
        stations: cfg.stations.len(),
        haulers: cfg.craft.iter().filter(|c| c.role != CraftRole::Pirate).count(),
        pirates_initial: cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count(),
        station_radii_milli_au: cfg
            .stations
            .iter()
            .map(|s| diagnostics::permille_floor(cfg.bodies[s.body_index].elements.a, 1.0))
            .collect(),
    };
```

  (d) Still in `simulate`, immediately after the `let mut samples`/`hashes`
  declarations and before the tick loop, write the meta JSONL row FIRST:

```rust
    if let Some(w) = jsonl.as_mut() {
        // META row FIRST (phase 0b): meta_-prefixed keys and NO "tick" key —
        // window consumers gate on `"tick" in row` (the version gate's JSONL
        // half; banked pre-META files simply have no such row).
        writeln!(
            w,
            "{}",
            serde_json::json!({
                "meta_seed": args.seed,
                "meta_scenario": meta.scenario,
                "meta_stations": meta.stations,
                "meta_haulers": meta.haulers,
                "meta_pirates_initial": meta.pirates_initial,
                "meta_station_radii_milli_au": meta.station_radii_milli_au,
            })
        )
        .expect("jsonl write");
    }
```

  (e) Change `simulate`'s tail to `Ok((samples, hashes, world, meta))`; in `main`
  destructure `let (samples, hashes, world, meta) = ...` (`:321`) and the replay
  arm `let (_, hashes2, _, _) = ...` (`:453`).

  (f) In `main`, directly before the `RESULT` println (`:383`):

```rust
    // The META anchored line (world-gets-big phase 0b, spec §8 — the lockstep
    // rule: this line and sweep_trophic.py's META_RE land in the SAME commit).
    // Kills the sweep's N_HAULERS mirror class: the lab reads population facts
    // from the run, never from a Python constant that can drift from the factory.
    println!(
        "META seed={} scenario={} stations={} haulers={} pirates_initial={} \
         station_radii_milli_au={:?}",
        args.seed,
        meta.scenario,
        meta.stations,
        meta.haulers,
        meta.pirates_initial,
        meta.station_radii_milli_au,
    );
```

  (`{:?}` on `Vec<u32>` prints `[350, 560, ...]` — exactly what `META_RE` anchors.)

- [ ] **Step 4: sweep side — `META_RE`, `parse_stdout`, dict-shaped runs, the
  N_HAULERS kill.** In `sweep_trophic.py`:

  (a) After `MEDIA_RE` (`:60-64`) add:

```python
# ---- instrument-format versioning (world-gets-big phase 0b) ----
# The anchored-line set GROWS over time; banked stdout/JSONL from older
# binaries must still aggregate. PRESENCE is the version gate:
#   v1 (pirates+media rungs):   RESULT + MEDIA only.
#   v2 (world-gets-big 0b):     + META (and FUEL, its own lockstep commit).
# RESULT and MEDIA stay hard-required (every banked format has them);
# META parses to None on v1 output and every consumer below must print
# "n/a (pre-FUEL instrument format)" for it — never SystemExit.
META_RE = re.compile(
    r"^META seed=(?P<seed>\d+) scenario=(?P<scenario>\w+) "
    r"stations=(?P<stations>\d+) haulers=(?P<haulers>\d+) "
    r"pirates_initial=(?P<pirates_initial>\d+) "
    r"station_radii_milli_au=\[(?P<radii>[0-9, ]*)\]$"
)

# Anchored lines by key: (required-in-every-format?, regex). The lockstep
# rule still holds per line: a regex lands in the SAME commit as its println.
ANCHORED = {
    "result": (True, RESULT_RE),
    "media": (True, MEDIA_RE),
    "meta": (False, META_RE),
}


def parse_stdout(text):
    """Scan one run's stdout for the anchored lines. Returns a dict keyed by
    ANCHORED; optional lines absent from older-format output read None (the
    version gate — banked pre-FUEL stdout must keep parsing)."""
    found = {key: None for key in ANCHORED}
    for line in text.splitlines():
        for key, (_required, rx) in ANCHORED.items():
            m = rx.match(line.strip())
            if m:
                found[key] = m.groupdict()
    return found
```

  (b) Delete the `N_HAULERS = 12` block (`:70-72`) including its comment.

  (c) Replace `run_one`'s scan-and-return tail (`:102-118`) so a run is a dict:

```python
    parsed = parse_stdout(proc.stdout)
    for key, (required, _rx) in ANCHORED.items():
        if required and parsed[key] is None:
            sys.stderr.write(proc.stdout)
            raise SystemExit(f"no {key.upper()} line: {name} seed={seed}")
    rows = [json.loads(l) for l in jsonl.read_text().splitlines() if l.strip()]
    # Version gate, JSONL half: window rows carry "tick"; the v2 meta row
    # (meta_* keys) and per-role tail rows do not. Banked v1 files are all
    # window rows, so the filter is a no-op there.
    parsed["windows"] = [r for r in rows if "tick" in r]
    return parsed
```

  (d) Re-point every tuple unpack at the dict (mechanical; the full list):
  - `:145` → `verdicts = Counter(run["result"]["verdict"] for run in runs)`
  - `:151` → `fuel = [int(run["result"]["fuel_empty"]) for run in runs]`
  - `:156` → `phases = [p for run in runs for w in run["windows"] for p in w["engagement_phase_milli"]]`
  - `:172-175` → `for run in runs:` / `ws = run["windows"]` (then the body unchanged)
  - `:179-185` → `for run in runs[:1]:` / `r, ws = run["result"], run["windows"]`
  - `:195-216` → `for run in runs:` / `r, ws = run["result"], run["windows"]` (body unchanged; the `r['trips']`/`r['robs']` reads keep working)
  - `media_panel` `:225` → `readings = Counter(run["media"]["reading"] for run in runs)`
  - `:230-232` → `f"{[int(run['media']['escaped_milli']) for run in runs]}"`
  - `:235` → `lags = [l for run in runs for w in run["windows"] for l in w["heard_lag_ticks"]]`
  - `:250-265` → `for run in runs:` / `r, ws = run["result"], run["windows"]` (body unchanged)
  - `main` `:336-342` →

```python
            run = run_one(args, name, knobs, seed, out_dir)
            runs.append(run)
            print(
                f"  ran {name} seed={seed}: verdict={run['result']['verdict']} "
                f"robs={run['result']['robs']} fuel_empty={run['result']['fuel_empty']} "
                f"media={run['media']['reading']}"
            )
```

  - `:355-361` (positive-control restatement) → `if r["verdict"] ...` becomes
    `sum(1 for run in all_runs["control"] if run["result"]["verdict"] == "RiskEqualized")`.

  (e) `voi_line` — the mirror's replacement. Above it add:

```python
def hauler_rows(run):
    """Hauler-row count for the wallet slice (haulers are dense rows
    0..haulers in every factory). v2: read the META line's haulers= — this
    KILLED the module-level N_HAULERS mirror. v1 banked output: fall back to
    scenario_trophic's 12, the ONLY place the retired constant survives,
    behind the version gate."""
    if run["meta"] is not None:
        return int(run["meta"]["haulers"])
    return 12  # v1 fallback: scenario::NUM_HAULERS at the pre-META format
```

  and in `median_final_hauler_credits` (`:283-290`):

```python
    def median_final_hauler_credits(names):
        pool = []
        for n in names:
            for run in all_runs[n]:
                ws = run["windows"]
                if ws:
                    pool.extend(ws[-1]["per_craft_credits"][: hauler_rows(run)])
        pool.sort()
        return pool[(len(pool) - 1) // 2] if pool else None
```

  (f) `panel` opens with a map echo, version-gated (insert after the
  `print(f"\n=== knob set: ...")` line at `:144`):

```python
    m0 = runs[0]["meta"] if runs else None
    if m0 is not None:
        print(
            f"map: scenario={m0['scenario']} stations={m0['stations']} "
            f"haulers={m0['haulers']} pirates_initial={m0['pirates_initial']} "
            f"radii_milli_au=[{m0['radii']}]"
        )
    else:
        print("map: n/a (pre-FUEL instrument format)")
```

- [ ] **Step 5: media_log.py windows loader gets the same JSONL gate** (`:213`):

```python
    # Version gate (phase 0b): the runner's --jsonl now opens with a meta row
    # and may close with per-role fuel rows; window rows are the ones with
    # "tick". Banked pre-META files pass through unchanged.
    windows = (
        [w for w in load(args.windows) if "tick" in w] if args.windows else None
    )
```

- [ ] **Step 6: run the tests, watch them pass.**
  `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py`
  Expected: `2 passed`.
  Then the live lockstep check (line and regex agree):

```bash
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --seed 7 --ticks 2000 --jsonl /tmp/meta_check.jsonl | grep '^META '
head -1 /tmp/meta_check.jsonl
```

  Expected stdout: `META seed=7 scenario=trophic stations=6 haulers=12 pirates_initial=6 station_radii_milli_au=[350, 560, 770, 980, 1190, 1400]`
  Expected JSONL head: one row of `meta_*` keys, no `"tick"` key.
  Then a one-seed sweep smoke (exercises run_one + panels end-to-end):
  `python3 python/analysis/sweep_trophic.py --seeds 7 --ticks 6000 --knobset baseline --out /tmp/sweep_meta_smoke`
  Expected: panel prints the `map: scenario=trophic ...` echo and exits 0.
  Finally `grep -n "N_HAULERS" python/analysis/sweep_trophic.py` — expected: no matches.

- [ ] **Step 7: commit (lockstep: line + regex together; explicit paths).**

```bash
git add crates/jumpgate-core/examples/trophic_run.rs \
        python/analysis/sweep_trophic.py \
        python/analysis/media_log.py \
        python/tests/test_sweep_parsing.py
git commit -F - <<'EOF'
feat(lab): META anchored line + version-gated sweep parsing (phase 0b)

META seed/scenario/stations/haulers/pirates_initial/station_radii_milli_au
+ meta JSONL row; sweep parsing presence-gated (creates the precedent:
banked pre-FUEL outputs still parse, optional lines read None); the
N_HAULERS module mirror is dead — the wallet slice reads META haulers=
(v1 fallback kept ONLY behind the gate). Lockstep: println and META_RE
in this one commit. Windows, not gates (PDR-0006).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 0b.3: `FuelDiag` — the unhashed per-craft fuel instrument on World

Duty, cumulative burn, tank low-water, and contract-leg burn brackets. The
`media_diag`/`assign_diag` precedent exactly (world.rs:74-85): written by
`World::step`, read ONLY by `sample_window`, never a behavior input, never
hashed. Craft rows never mint mid-run (the only `ships.ids.insert` is reset's,
world.rs:256), so reset-sized vectors are safe. `prev_fuel` (stage 4,
world.rs:1049) is untouched.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs` (struct after `permille_floor`)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/world.rs` (World field after `engagement_diag` `:85`; reset literal `:447`; physics loop after `:854`; new stage 3c2 after `resolve_failures` `:1018`; test cloning the `two_body_contract_fixture` (`:2048`) usage in `deliver_on_arrival_settles_escrow_and_holds_credit_identity` (`:2471`))

- [ ] **Step 1: failing test first.** In `world.rs` tests, after
  `deliver_on_arrival_settles_escrow_and_holds_credit_identity` (same imports —
  `Command`, `CommandKind`, `Target`, `EntityRef`, `ContractStatus`, `StateView`
  are already in scope in that mod):

```rust
    /// Phase-0b fuel instrument (world-gets-big spec §8): one delivery run on
    /// the two_body_contract_fixture brackets exactly one contract leg and
    /// moves every per-craft diag channel. UNHASHED — no golden involved.
    #[test]
    fn fuel_diag_brackets_the_delivery_leg_and_tracks_burn() {
        let cfg = two_body_contract_fixture();
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let craft = world.craft_ids()[0];
        let cidx = 0;
        let contract = world
            .contracts
            .ids
            .id_at(cidx)
            .map(|(slot, generation)| crate::ids::ContractId { slot, generation })
            .unwrap();
        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::AcceptContract { contract },
        }];
        world.step(&mut cmds);
        assert!(
            world.fuel_diag.leg_start_fuel[0].is_some(),
            "accept opens the leg bracket at the current tank"
        );
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..6000 {
            world.step(&mut empty);
            if world.contracts.status[cidx] == ContractStatus::Completed {
                break;
            }
        }
        assert_eq!(
            world.contracts.status[cidx],
            ContractStatus::Completed,
            "delivery completed within the step bound (the 0b fixture rides \
             deliver_on_arrival_settles_escrow_and_holds_credit_identity)"
        );
        assert_eq!(world.fuel_diag.leg_burns.len(), 1, "exactly one leg closed");
        let (close_tick, burn_permille) = world.fuel_diag.leg_burns[0];
        assert!(close_tick.0 > 0, "leg closed at a real tick");
        assert!(burn_permille <= 1000, "leg burn is a permille of capacity");
        assert_eq!(
            world.fuel_diag.leg_start_fuel[0], None,
            "bracket consumed at close"
        );
        assert!(world.fuel_diag.thrust_ticks[0] > 0, "duty counted thrusting ticks");
        assert!(world.fuel_diag.burned_mass[0] > 0.0, "cumulative burn accumulated");
        assert!(
            world.fuel_diag.min_fuel_mass[0] <= world.ships.fuel_mass[0],
            "low-water mark never exceeds the live tank"
        );
    }
```

- [ ] **Step 2: run it, watch it fail.**
  `cargo test -p jumpgate-core fuel_diag_brackets`
  Expected failure: compile error `error[E0609]: no field `fuel_diag` on type ...World`.

- [ ] **Step 3: the struct.** In `diagnostics.rs` after `permille_floor`:

```rust
/// UNHASHED per-craft fuel diagnostics (world-gets-big phase 0b, spec §8 —
/// the `media_diag`/`assign_diag` pattern): written by `World::step`
/// (stage-2 physics: duty/burn/low-water; stage 3c2: contract-leg brackets),
/// read ONLY by `sample_window`. Never a behavior input, never hashed.
/// Reset-sized to `n_craft`; craft rows never mint mid-run (the only
/// `ships.ids.insert` is reset's). f64 is legal here — this struct never
/// reaches a `TrophicSample` except through the `permille_floor` seam.
#[derive(Clone, Debug, Default)]
pub struct FuelDiag {
    /// Ticks with a live burn (`fuel_consumed > 0`), run-cumulative.
    pub thrust_ticks: Vec<u64>,
    /// Propellant mass burned, run-cumulative.
    pub burned_mass: Vec<f64>,
    /// Tank low-water mark over the run (starts AT the starting tank — the
    /// prev_fuel "prev == current at tick 0" idiom).
    pub min_fuel_mass: Vec<f64>,
    /// Tank at the open of the craft's current contract leg (`Some` while a
    /// leg is open). A Robbed leg never closes — the hauler's next accept
    /// overwrites the bracket; only accept→fulfil legs are recorded.
    pub leg_start_fuel: Vec<Option<f64>>,
    /// `(close_tick, burn as permille-of-capacity, FLOOR)` per COMPLETED
    /// contract leg — the W8 median-leg-burn input. Both bracket ends read
    /// the post-physics tank of their tick, so the burn is the inter-tick
    /// propellant spent flying the leg.
    pub leg_burns: Vec<(Tick, u32)>,
}
```

- [ ] **Step 4: World field + reset init.** In `world.rs`, after
  `engagement_diag` (`:85`):

```rust
    /// UNHASHED fuel diagnostics (world-gets-big phase 0b): per-craft duty /
    /// burn / low-water written by the stage-2 physics loop; contract-leg
    /// burn brackets written by stage 3c2. Read ONLY by `sample_window`.
    pub(crate) fuel_diag: crate::diagnostics::FuelDiag,
```

  and in the reset literal, after `engagement_diag: Vec::new(),` (`:447`):

```rust
            fuel_diag: crate::diagnostics::FuelDiag {
                thrust_ticks: vec![0; ships.fuel_mass.len()],
                burned_mass: vec![0.0; ships.fuel_mass.len()],
                min_fuel_mass: ships.fuel_mass.clone(),
                leg_start_fuel: vec![None; ships.fuel_mass.len()],
                leg_burns: Vec::new(),
            },
```

  (This is a World field, not a CraftStore column — the three-place CraftStore
  rule does not apply; no hash fold, no `HASH_FORMAT_VERSION` movement.)

- [ ] **Step 5: physics-loop channels.** In `world.rs`, directly after
  `self.ships.fuel_mass[ci] = (fuel - fuel_consumed).max(0.0);` (`:854`):

```rust
            // UNHASHED fuel diagnostics (phase 0b): duty + cumulative burn +
            // tank low-water. Diag-only — no behavior stage reads fuel_diag.
            if fuel_consumed > 0.0 {
                self.fuel_diag.thrust_ticks[ci] =
                    self.fuel_diag.thrust_ticks[ci].saturating_add(1);
                self.fuel_diag.burned_mass[ci] += fuel_consumed;
            }
            if self.ships.fuel_mass[ci] < self.fuel_diag.min_fuel_mass[ci] {
                self.fuel_diag.min_fuel_mass[ci] = self.ships.fuel_mass[ci];
            }
```

- [ ] **Step 6: stage 3c2 — leg brackets.** In `world.rs`, after the
  `resolve_failures` call (`:1012-1018`) and before stage 3d:

```rust
        // (3c2) UNHASHED fuel-leg diagnostics (phase 0b): bracket contract
        //       legs on this tick's event stream (the stage-3c event-lift
        //       borrow idiom — collect first, then mutate). Accept opens a
        //       leg at the CURRENT post-physics tank; fulfil closes it and
        //       records the burn as permille of effective capacity (FLOOR —
        //       the permille_floor seam). A Robbed leg never closes (the next
        //       accept overwrites the bracket). Same-tick accept-then-fulfil
        //       cannot collide: CargoLoaded→InTransit promotes NEXT tick
        //       (stage 1c), and a delivering hauler is not Idle at dispatch.
        //       Diag-only: no behavior stage reads fuel_diag.
        let leg_edges: Vec<(CraftId, bool)> = self
            .events
            .since(next)
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::ContractAccepted { hauler, .. } => Some((hauler, true)),
                EventKind::ContractFulfilled { hauler, .. } => Some((hauler, false)),
                _ => None,
            })
            .collect();
        for (craft, opened) in leg_edges {
            let Some(row) = self.ship_index(craft) else { continue };
            if opened {
                self.fuel_diag.leg_start_fuel[row] = Some(self.ships.fuel_mass[row]);
            } else if let Some(start) = self.fuel_diag.leg_start_fuel[row].take() {
                let cap = effective_params(&self.ships.spec[row], &self.ships.mods[row])
                    .fuel_capacity;
                let burned = (start - self.ships.fuel_mass[row]).max(0.0);
                self.fuel_diag
                    .leg_burns
                    .push((next, crate::diagnostics::permille_floor(burned, cap)));
            }
        }
```

- [ ] **Step 7: run it, watch it pass — and prove neutrality.**
  `cargo test -p jumpgate-core fuel_diag_brackets`
  Expected: `test world::tests::fuel_diag_brackets_the_delivery_leg_and_tracks_burn ... ok`.
  Then `cargo test --workspace` — expected: all green, **zero golden movement**
  (`config_hash_golden_anchor_is_stable` and every state-hash/replay test
  untouched — `fuel_diag` is unhashed and reads nothing back into behavior).
  Then `cargo clippy --all-targets -- -D warnings` — clean.

- [ ] **Step 8: commit.**

```bash
git add crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(lab): FuelDiag — unhashed per-craft duty/burn/low-water + leg brackets (phase 0b)

The media_diag pattern: stage-2 physics writes duty/burn/low-water, new
stage 3c2 brackets contract legs accept->fulfil (Robbed legs never close).
Read only by sample_window; prev_fuel untouched; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 0b.4: TrophicSample additive fuel fields gathered in `sample_window`

The lab cannot read `pub(crate)` world state from the example binary
(TROPHIC-C2) — data flows through `sample_window`. All-integer law holds
(diagnostics.rs:63-65, `Eq` derive): everything crosses the `permille_floor`
seam. Fields APPEND at the struct end and (next task) at the `sample_json` end —
the media/assign additive precedent; every pre-fuel JSONL key stays
byte-untouched.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs` (`TrophicSample` tail `:139-140`, `sample_window` literal tail `:600-604`, tests mod — extract the `:824-863` config into a helper)

- [ ] **Step 1: failing test first.** In the tests mod: extract the RunConfig
  literal of `sample_window_counts_purchases_and_reads_yard_treasury`
  (`:824-863`) into `fn one_craft_vendor_cfg() -> crate::config::RunConfig`
  (verbatim move; the existing test calls the helper — behavior unchanged), then
  add the synthetic (the seed-7 rule — a synthetic that would catch the
  instrument lying on FLOOR form, window filtering, and the role snapshot):

```rust
    /// Phase-0b fuel lab fields: synthetic FuelDiag → sample_window integer
    /// reads. Values chosen so FLOOR and round-half-up DISAGREE; leg ticks
    /// chosen so the window filter must drop the out-of-window entry.
    #[test]
    fn sample_window_reads_fuel_diag_through_the_floor_seam() {
        use crate::world::World;
        let (mut world, _h) =
            World::reset(one_craft_vendor_cfg()).expect("resolvable cfg");
        let mut cmds = Vec::new();
        world.step(&mut cmds);
        world.step(&mut cmds); // tick 2: sample point
        // Synthetic diag write (UNHASHED; the tests own crate internals —
        // capacity is 1e-9, so milli/permille read against that).
        world.fuel_diag.thrust_ticks[0] = 41;
        world.fuel_diag.burned_mass[0] = 0.5999e-9; // 599.9 milli-tank
        world.fuel_diag.min_fuel_mass[0] = 0.4001e-9; // 400.1 permille
        world.fuel_diag.leg_burns =
            vec![(Tick(1), 77), (Tick(2), 123), (Tick(5000), 200)];
        let s = sample_window(&world, Tick(0));
        assert_eq!(s.per_craft_role, vec![0], "role snapshot (Idle rank 0)");
        assert_eq!(s.per_craft_thrust_ticks, vec![41], "duty numerator snapshot");
        assert_eq!(s.per_craft_burn_milli, vec![599], "FLOOR: 599.9 -> 599, never 600");
        assert_eq!(s.per_craft_min_tank_permille, vec![400], "FLOOR: 400.1 -> 400");
        assert_eq!(
            s.leg_burn_permille,
            vec![77, 123],
            "window filter: window_start < close_tick <= sample tick only"
        );
    }
```

- [ ] **Step 2: run it, watch it fail.**
  `cargo test -p jumpgate-core sample_window_reads_fuel_diag`
  Expected failure: compile error `error[E0609]: no field `per_craft_role` on type ...TrophicSample`.

- [ ] **Step 3: struct fields.** Append at the END of `TrophicSample`
  (after `assign_counts_cum`, `:139`):

```rust
    // --- fuel lab fields (world-gets-big phase 0b, spec §8; windows, not
    // gates). Additive: every pre-fuel JSONL key above is untouched. ---
    /// `CraftRole::rank()` per craft at the sample point (dense row order) —
    /// the FUEL role-split key (0 Idle / 1 Hauler / 2 Pirate; the hauler side
    /// of the split is "rank != 2": Idle↔Hauler flips per leg, Pirate never).
    pub per_craft_role: Vec<u32>,
    /// `fuel_diag.thrust_ticks` snapshot (run-cumulative ticks with a live
    /// burn) — the duty numerator.
    pub per_craft_thrust_ticks: Vec<u64>,
    /// `fuel_diag.burned_mass` as MILLI of effective fuel capacity, FLOOR
    /// (run-cumulative; may exceed 1000 once the phase-1 refuel verb lands).
    pub per_craft_burn_milli: Vec<u32>,
    /// `fuel_diag.min_fuel_mass` as permille of effective fuel capacity,
    /// FLOOR — the run low-water mark.
    pub per_craft_min_tank_permille: Vec<u32>,
    /// Burn (permille of capacity, FLOOR) of each contract leg COMPLETED in
    /// the window (`fuel_diag.leg_burns`, tick-filtered) — the W8 median
    /// input. Contract legs are hauler-only by construction.
    pub leg_burn_permille: Vec<u32>,
```

- [ ] **Step 4: gathering.** Append at the END of the `sample_window` literal
  (after `assign_counts_cum: ...` `:603`):

```rust
        // Fuel lab fields (phase 0b): pure snapshots of the UNHASHED
        // fuel_diag, integerized through the one permille_floor seam.
        per_craft_role: world.ships.role.iter().map(|r| u32::from(r.rank())).collect(),
        per_craft_thrust_ticks: world.fuel_diag.thrust_ticks.clone(),
        per_craft_burn_milli: (0..world.ships.ids.len())
            .map(|r| {
                let cap =
                    crate::stores::effective_params(&world.ships.spec[r], &world.ships.mods[r])
                        .fuel_capacity;
                permille_floor(world.fuel_diag.burned_mass[r], cap)
            })
            .collect(),
        per_craft_min_tank_permille: (0..world.ships.ids.len())
            .map(|r| {
                let cap =
                    crate::stores::effective_params(&world.ships.spec[r], &world.ships.mods[r])
                        .fuel_capacity;
                permille_floor(world.fuel_diag.min_fuel_mass[r], cap)
            })
            .collect(),
        leg_burn_permille: world
            .fuel_diag
            .leg_burns
            .iter()
            .filter(|(t, _)| t.0 > window_start.0 && t.0 <= tick.0)
            .map(|&(_, p)| p)
            .collect(),
```

  (Every synthetic-series test helper builds with `..Default::default()` —
  the new fields default to empty vectors, so no existing test changes.)

- [ ] **Step 5: run it, watch it pass.**
  `cargo test -p jumpgate-core sample_window_reads_fuel_diag` — expected `ok`.
  `cargo test -p jumpgate-core --lib` — expected: all diagnostics/classifier
  tests still green (additive fields, `Default` covers the synthetics).
  `cargo clippy --all-targets -- -D warnings` — clean.

- [ ] **Step 6: commit.**

```bash
git add crates/jumpgate-core/src/diagnostics.rs
git commit -F - <<'EOF'
feat(lab): TrophicSample fuel fields — role/duty/burn/low-water/leg burns (phase 0b)

Additive append-only fields (the media/assign precedent), gathered in
sample_window from the unhashed FuelDiag through the pinned FLOOR seam
(TROPHIC-C2: the lab reads pub(crate) state only via sample_window).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 0b.5: role-split FUEL line (measured fields only) + per-role JSONL rows + lockstep sweep parse

Clone of the MEDIA pattern (`trophic_run.rs:398-418`): anchored line,
same-commit regex, 0 sentinel when nothing measured. The anchored line carries
HAULER numbers only (LAB-C2); pirates ride per-role JSONL tail rows. Refuel
fields are **explicitly deferred to phase 1** — they append to the line WITH
the mechanic, never as zeros for a verb that does not exist.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs` (`sample_json` tail `:174`, new `FuelAgg`/`fuel_agg` after `sample_json`, `main` flush block `:328-330` and the anchored-line block after MEDIA `:418`)
- Modify: `/home/john/jumpgate/python/analysis/sweep_trophic.py` (`ANCHORED` map, `FUEL_RE`, `panel` fuel block)
- Modify: `/home/john/jumpgate/python/tests/test_sweep_parsing.py` (extend V2 + assertions)

- [ ] **Step 1: failing test first.** In `test_sweep_parsing.py`, extend
  `V2_STDOUT` and add a test:

```python
V2_STDOUT = V1_STDOUT + (
    "META seed=7 scenario=trophic stations=6 haulers=12 pirates_initial=6 "
    "station_radii_milli_au=[350, 560, 770, 980, 1190, 1400]\n"
    "FUEL seed=7 hauler_duty_milli=412 hauler_burn_total_milli=3180 "
    "hauler_median_leg_burn_permille=24 hauler_min_tank_permille=507\n"
)


def test_v2_fuel_line_parses_and_v1_reads_none():
    parsed = sweep.parse_stdout(V2_STDOUT)
    assert parsed["fuel"] is not None
    assert parsed["fuel"]["duty"] == "412"
    assert parsed["fuel"]["burn"] == "3180"
    assert parsed["fuel"]["leg"] == "24"
    assert parsed["fuel"]["min_tank"] == "507"
    # v1 banked output: FUEL absent is the version gate, never an error.
    assert sweep.parse_stdout(V1_STDOUT)["fuel"] is None
```

- [ ] **Step 2: run it, watch it fail.**
  `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py`
  Expected failure: `KeyError: 'fuel'` (parse_stdout's dict has no such key yet).

- [ ] **Step 3: sweep side.** In `sweep_trophic.py`, after `META_RE`:

```python
# The FUEL line (world-gets-big phase 0b) — lands in the SAME commit as the
# runner's println! (the lockstep rule). HAULER numbers only on the anchored
# line; pirates ride the per-role JSONL tail rows. Phase-1 refuel fields
# (refuels= refuel_spend_micros= strandings= adrift_end=) will APPEND here
# in the refuel mechanic's own lockstep commit.
FUEL_RE = re.compile(
    r"^FUEL seed=(?P<seed>\d+) hauler_duty_milli=(?P<duty>\d+) "
    r"hauler_burn_total_milli=(?P<burn>\d+) "
    r"hauler_median_leg_burn_permille=(?P<leg>\d+) "
    r"hauler_min_tank_permille=(?P<min_tank>\d+)$"
)
```

  add to `ANCHORED`: `"fuel": (False, FUEL_RE),` — and in `panel`, after the
  endurance line (`:151-152`):

```python
    # FUEL window (phase 0b, spec §8): hauler duty/burn/low-water per run.
    # Version-gated: pre-FUEL banked output prints n/a, never dies. On
    # trophic arms fuel_empty=0 above stays the endurance window; on frontier
    # the SAME read will mean "no stranding this seed" (texture) — recorded,
    # never gated (PDR-0006).
    if all(run["fuel"] is not None for run in runs):
        print(
            "fuel (hauler): duty_milli="
            f"{[int(run['fuel']['duty']) for run in runs]} burn_total_milli="
            f"{[int(run['fuel']['burn']) for run in runs]} median_leg_burn_permille="
            f"{[int(run['fuel']['leg']) for run in runs]} min_tank_permille="
            f"{[int(run['fuel']['min_tank']) for run in runs]}"
        )
    else:
        print("fuel: n/a (pre-FUEL instrument format)")
```

- [ ] **Step 4: runner side — aggregates, JSONL keys, role rows, the line.**
  In `trophic_run.rs`:

  (a) Append to `sample_json` (after `"assign_counts_cum"`, `:174`):

```rust
        // Fuel lab keys (world-gets-big phase 0b) — ADDITIVE: every pre-fuel
        // key above is byte-untouched.
        "per_craft_role": s.per_craft_role,
        "per_craft_thrust_ticks": s.per_craft_thrust_ticks,
        "per_craft_burn_milli": s.per_craft_burn_milli,
        "per_craft_min_tank_permille": s.per_craft_min_tank_permille,
        "leg_burn_permille": s.leg_burn_permille,
```

  (b) After `sample_json` add the aggregate (run-cumulative fields read off the
  LAST sample; leg burns pool over windows — the MEDIA lag-pooling clone):

```rust
/// Role-split fuel aggregates (phase 0b, spec §8 — windows, never gates).
/// The pirate side exists for the per-role JSONL rows only; the anchored
/// FUEL line carries HAULER numbers (LAB-C2). 0 sentinels throughout (the
/// MEDIA precedent): median_leg_burn 0 == no leg ever completed; duty/burn
/// 0 == nothing thrusted; an empty run reads all-0.
///
/// DEFERRED to phase 1 (with the refuel mechanic, never before): refuels,
/// refuel_spend_micros, strandings, adrift_end — they append to the FUEL
/// line and FUEL_RE in that mechanic's own lockstep commit.
struct FuelAgg {
    duty_milli: u64,
    burn_total_milli: u64,
    median_leg_burn_permille: u32,
    min_tank_permille: u32,
}

fn fuel_agg(samples: &[TrophicSample], pirate_side: bool) -> FuelAgg {
    let zero = FuelAgg {
        duty_milli: 0,
        burn_total_milli: 0,
        median_leg_burn_permille: 0,
        min_tank_permille: 0,
    };
    let Some(last) = samples.last() else { return zero };
    let rows: Vec<usize> = (0..last.per_craft_role.len())
        .filter(|&r| (last.per_craft_role[r] == 2) == pirate_side)
        .collect();
    if rows.is_empty() {
        return zero;
    }
    // Duty: pooled thrusting ticks over pooled craft-ticks, milli, FLOOR
    // (integer division). Idle-the-whole-run craft dilute honestly.
    let thrust: u64 = rows.iter().map(|&r| last.per_craft_thrust_ticks[r]).sum();
    let craft_ticks = (rows.len() as u64).saturating_mul(last.tick);
    let duty_milli =
        if craft_ticks == 0 { 0 } else { thrust.saturating_mul(1000) / craft_ticks };
    let burn_total_milli: u64 =
        rows.iter().map(|&r| u64::from(last.per_craft_burn_milli[r])).sum();
    // Contract legs are hauler-only by construction; the pirate row reads
    // the 0 sentinel (pirates fly no contract legs).
    let median_leg_burn_permille = if pirate_side {
        0
    } else {
        let mut legs: Vec<u32> =
            samples.iter().flat_map(|s| s.leg_burn_permille.iter().copied()).collect();
        legs.sort_unstable();
        if legs.is_empty() { 0 } else { legs[(legs.len() - 1) / 2] }
    };
    let min_tank_permille = rows
        .iter()
        .map(|&r| last.per_craft_min_tank_permille[r])
        .min()
        .unwrap_or(0);
    FuelAgg { duty_milli, burn_total_milli, median_leg_burn_permille, min_tank_permille }
}
```

  (c) In `main`, replace the flush block (`:328-330`) — aggregates are computed
  first, the per-role rows go out before the flush:

```rust
    let hauler_fuel = fuel_agg(&samples, false);
    let pirate_fuel = fuel_agg(&samples, true);
    if let Some(mut w) = jsonl_writer {
        // Per-role FUEL rows (phase 0b, spec §8): the anchored stdout line
        // carries HAULER numbers only; pirates ride these JSONL tail rows.
        // No "tick" key — window consumers gate on `"tick" in row`.
        for (role, a) in [("hauler", &hauler_fuel), ("pirate", &pirate_fuel)] {
            writeln!(
                w,
                "{}",
                serde_json::json!({
                    "fuel_role": role,
                    "duty_milli": a.duty_milli,
                    "burn_total_milli": a.burn_total_milli,
                    "median_leg_burn_permille": a.median_leg_burn_permille,
                    "min_tank_permille": a.min_tank_permille,
                })
            )
            .expect("jsonl write");
        }
        w.flush().expect("jsonl flush");
    }
```

  (d) After the MEDIA println (`:418`), before the ASSIGN block:

```rust
    // The FUEL line (world-gets-big phase 0b, spec §8 — a window, not a gate;
    // the lockstep rule: this line and FUEL_RE land in the SAME commit).
    // MEASURED fields only. DEFERRED to phase 1 WITH the refuel mechanic:
    // refuels= refuel_spend_micros= strandings= adrift_end= (they append
    // here + FUEL_RE in that commit — never zeros for a verb that doesn't
    // exist). fuel_empty (RESULT line) stays the trophic endurance window;
    // --assert-no-fuel-empty stays a TROPHIC-ARM flag only — on frontier
    // fuel_empty=0 will read as texture ("no stranding this seed"), recorded,
    // never asserted.
    println!(
        "FUEL seed={} hauler_duty_milli={} hauler_burn_total_milli={} \
         hauler_median_leg_burn_permille={} hauler_min_tank_permille={}",
        args.seed,
        hauler_fuel.duty_milli,
        hauler_fuel.burn_total_milli,
        hauler_fuel.median_leg_burn_permille,
        hauler_fuel.min_tank_permille,
    );
```

- [ ] **Step 5: run everything, watch it pass.**
  `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py`
  Expected: `3 passed`.
  Live lockstep check:

```bash
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --seed 7 --ticks 6000 --jsonl /tmp/fuel_check.jsonl | grep '^FUEL '
tail -2 /tmp/fuel_check.jsonl
```

  Expected stdout: one `FUEL seed=7 hauler_duty_milli=... hauler_burn_total_milli=... hauler_median_leg_burn_permille=... hauler_min_tank_permille=...` line with real (non-placeholder) integers — trophic craft genuinely burn (v_e 20, thrust 1e-12: ~1.25e-14/thrusting tick against the 1e-9 tank), so duty/burn/min-tank move even though `FuelEmpty` stays unfireable (tank == eps, the spec-§4 re-bake is phase 1's problem, NOT this task's).
  Expected JSONL tail: `{"fuel_role":"hauler",...}` then `{"fuel_role":"pirate",...}` (pirate row has `median_leg_burn_permille: 0` — the sentinel).
  Then the one-seed sweep smoke:
  `python3 python/analysis/sweep_trophic.py --seeds 7 --ticks 6000 --knobset baseline --out /tmp/sweep_fuel_smoke`
  Expected: the panel prints the `fuel (hauler): ...` block; exit 0.
  Then `cargo test --workspace` and `cargo clippy --all-targets -- -D warnings` — green/clean, zero goldens moved.

- [ ] **Step 6: commit (lockstep: line + regex together).**

```bash
git add crates/jumpgate-core/examples/trophic_run.rs \
        python/analysis/sweep_trophic.py \
        python/tests/test_sweep_parsing.py
git commit -F - <<'EOF'
feat(lab): role-split FUEL anchored line + per-role JSONL rows (phase 0b)

MEASURED hauler fields only (duty/burn_total/median_leg_burn/min_tank);
pirates ride JSONL tail rows; 0 sentinels per the MEDIA precedent; refuel
fields explicitly deferred to phase 1 with the mechanic. Lockstep: println
and FUEL_RE in this one commit. --assert-no-fuel-empty stays trophic-only.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 0b.6: bank the 20-seed trophic baseline POST-0a-fix with the new instruments

**Precondition (hard ordering):** the phase-0a haven-lurk fix commit is on the
branch (it changes state-hash trajectories vs the old banked baselines — that is
the point of re-banking). Verify before running:
`git log --oneline -5` must show the 0a fix commit; if it does not, STOP and do
phase 0a first.

This is a recording task: every number is a window. Nothing here passes or
fails on a metric — the only nonzero exits are mechanical (`--replay-check`
bit-identity and the pre-existing trophic endurance window).

**Files**
- Create: `runs/wgb_baseline/` (NEVER staged — `runs/` is gitignored at `.gitignore:25`; do not "fix" that)
- Create: `docs/superpowers/posts/<today's date>-wgb-trophic-baseline/README.md` (+ copied summaries) — the capture practice: bank same-day, /tmp is volatile

- [ ] **Step 1: run the 20-seed baseline (seeds 0–19), full instruments.**

```bash
cd /home/john/jumpgate
cargo build --release -p jumpgate-core --examples
mkdir -p runs/wgb_baseline
for seed in $(seq 0 19); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed "$seed" --ticks 50000 \
    --jsonl "runs/wgb_baseline/trophic_s${seed}.jsonl" \
    --replay-check --assert-no-fuel-empty \
    > "runs/wgb_baseline/trophic_s${seed}.stdout"
done
grep -h '^META \|^RESULT \|^MEDIA \|^FUEL \|^ASSIGN ' \
  runs/wgb_baseline/trophic_s*.stdout > runs/wgb_baseline/anchored_lines.txt
wc -l runs/wgb_baseline/anchored_lines.txt
```

  Expected: 20 runs, each exit 0 (replay-check bit-identical; the trophic
  endurance window holds — `--assert-no-fuel-empty` is correct HERE and only
  here: it never goes into a frontier recipe). `anchored_lines.txt` has 100
  lines (5 anchored lines × 20 seeds). Each JSONL: 1 meta row + 25 window rows
  + 2 fuel-role rows = 28 lines.

- [ ] **Step 2: the standard grid through the sweep (baseline + the
  hungry-roamer positive control — the 20-seed × arms discipline, spec §8).**

```bash
python3 python/analysis/sweep_trophic.py \
  --seeds 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 \
  --ticks 50000 \
  --knobset baseline \
  --knobset "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000" \
  --out runs/wgb_baseline/sweep \
  | tee runs/wgb_baseline/sweep_summary.txt
```

  Expected: exits 0; the panel for each arm shows the `map:` echo, the
  `fuel (hauler): ...` block, and the positive-control restatement line. The
  control reading and every verdict count are RECORDED in the summary, not
  judged here — the post-leak-fix re-judgment is the owner's console session
  and the phase-3 dual-map re-fit, not this task.

- [ ] **Step 3: bank the capture artifacts (same-day; runs/ is volatile-class,
  /tmp doubly so).**

```bash
DEST="docs/superpowers/posts/$(date +%F)-wgb-trophic-baseline"
mkdir -p "$DEST"
cp runs/wgb_baseline/sweep_summary.txt "$DEST/sweep_summary.txt"
cp runs/wgb_baseline/anchored_lines.txt "$DEST/anchored_lines.txt"
```

  Then write `$DEST/README.md` recording, concretely (no placeholders — fill
  every field from the actual session):
  - HEAD sha at run time (`git rev-parse --short HEAD`) and the phase-0a
    haven-lurk fix commit sha this baseline is POST- of;
  - the exact step-1 and step-2 command lines (paste them verbatim);
  - shape facts: 20 seeds × 50k ticks × 25 windows; instrument format v2
    (META+FUEL; version-gated parsing per `python/tests/test_sweep_parsing.py`);
  - one line per anchored-line family quoting a sample (seed 7's META, RESULT,
    MEDIA, FUEL, ASSIGN lines verbatim from `anchored_lines.txt`);
  - the framing sentence: "Windows recorded, never gated (PDR-0006). This
    baseline replaces every pre-haven-lurk-fix banked trophic baseline for
    cross-arm comparison; old banked state-hash trajectories are NOT
    comparable (the fix shifts Piracy draws; goldens unchanged).";
  - the note that `runs/wgb_baseline/` holds the full per-seed stdout+JSONL and
    is deliberately uncommitted (`runs/` is gitignored — never staged).

- [ ] **Step 4: commit the banked summary ONLY (explicit paths; never `runs/`,
  never `-A`, never `.`).**

```bash
git add "docs/superpowers/posts/$(date +%F)-wgb-trophic-baseline/README.md" \
        "docs/superpowers/posts/$(date +%F)-wgb-trophic-baseline/sweep_summary.txt" \
        "docs/superpowers/posts/$(date +%F)-wgb-trophic-baseline/anchored_lines.txt"
git commit -F - <<'EOF'
bench(wgb): 20-seed trophic baseline POST-haven-lurk-fix, v2 instruments (phase 0b)

Seeds 0-19 x 50k ticks, baseline + hungry-roamer control, banked with the
new META/FUEL anchored lines and per-role JSONL rows. Windows recorded,
never gated (PDR-0006). Raw runs live in runs/wgb_baseline/ (gitignored).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

## Phase-0b invariants (the builder's checklist, restated)

- Zero golden literals move; no `HASH_FORMAT_VERSION` bump; no config-surface
  change; reward surfaces untouched. If any golden test fails in 0b, that is a
  BUG in the change, never a re-pin.
- All new world state is UNHASHED diagnostics (`FuelDiag` — the `media_diag`
  precedent); `prev_fuel`/stage-4 copy-forward untouched.
- Every f64→integer crossing goes through `permille_floor` (FLOOR, pinned).
- Lockstep rule: META/FUEL printlns land in the same commits as
  `META_RE`/`FUEL_RE`.
- Version gate = presence: banked pre-FUEL stdout/JSONL must keep parsing
  (pinned by `python/tests/test_sweep_parsing.py`).
- `--assert-no-fuel-empty` stays trophic-only; `fuel_empty=0`'s frontier
  meaning-flip is encoded in comments now, in behavior never.
- `runs/` is never staged; banked summaries go to `docs/superpowers/posts/`.


---

## Phase 1 — eps re-bake (spec §4 item 1; §9 phase 1 first clause)

HEAD at plan time: `e7e490e`. Beat: make the FuelEmpty edge armable. At
`FUEL_EMPTY_EPS = 1e-9` every band tank (1.0e-9, `scenario.rs:113/126`) sits
exactly AT the eps and the strict `fuel_prev > FUEL_EMPTY_EPS` predicate
(`events.rs:50`) can never fire — the gauge's whole travel is inside the dead
zone. The re-bake drops the eps to `1e-11` (its own single-cause commit) and
REDESIGNS — does not nudge — the two fixture families that straddled the old
eps, then proves band hash-neutrality (goldens unmoved + cross-branch trophic
digest). eps appears in NO physics expression (only `events.rs` detection), so
burn arithmetic, `state_hash`, and `config_hash` are untouched by construction;
Task 1.2 measures that claim instead of trusting it.

**Complete affected-test inventory** (from the fuel-edge grounding, verified at
HEAD):

| Test | Fixture | Old fuel | Action |
|---|---|---|---|
| `starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss` (world.rs:2208) | `two_body_starved_contract_fixture` (world.rs:2196-2206) | `1.06e-9` | REDESIGN → `7.0e-11` |
| `fuel_empty_mid_deadhead_refunds_escrow`, BOTH arms (economy.rs:2465+) | `starved_two_body_contract_fixture` (economy.rs:2364-2455, fuel at :2417) | `1.06e-9` | REDESIGN → `7.0e-11` |
| `fuel_just_emptied_fires_only_on_depletion_edge` (events.rs:179-190) | const-relative literals | — | NO edit; stays green at the new eps by construction |
| replay_equivalence.rs all 6 tests (`recorded_run_actually_thrusts`, `record_then_replay_is_bit_identical`, `thrust_mode_record_then_replay_is_bit_identical`, `corrupting_one_logged_command_reports_first_differing_tick`, `config_hash_mismatch_is_rejected`, `provenance_mismatch_is_rejected`) | `base_config()` fuel `5.0e-10` (replay_equivalence.rs:41) | `5.0e-10` | KEEP value; document the decision in a comment (see Task 1.1 step 5 rationale) |
| `thrust_command_accelerates_craft_and_burns_fuel`, `thrust_command_persists_until_replaced`, `live_ingest_no_budget_uses_fuel_derived_dv_not_infinity`, reset-guard tests (world.rs:1320/1333/1339) | `one_body_one_thrusting_craft` fuel `1e-9` (world.rs:1294) | `1e-9` | NO change: contract-free worlds — a FuelEmpty event is state-inert (stage 3c `resolve_failures` is the only state-coupled consumer) and these horizons are far under the ~40 full-throttle ticks needed to cross `1e-11` |
| docked-vendor tests on `vendor_world_fixture` (economy.rs:1564-1611, fuel :1591) | fuel `1e-9` | `1e-9` | NO change: docked, zero burn, edge unreachable |
| physics_sanity `fueled_autopilot_transfer_reaches_destination` (:282), `transfer_arrival_tick_is_deterministic` (:294), `transfer_to_moving_body_rendezvous` (:318) | `thrusting_craft` fuel `1.0e-9` (physics_sanity.rs:230-246) | `1e-9` | NO change: contract-free; eps changes event emission only, never the fuel/position trajectory those tests assert |
| `scenario_trophic` band (haulers + pirates, `1.0e-9` tanks, v_e 20.0) | scenario.rs:89-95/113/126 | `1e-9` | NO change: burn/tick = `1e-12/20·0.25 = 1.25e-14` ⇒ crossing `1e-11` needs ~79,200 full-throttle ticks > any 50k-tick run even at 100% duty. Verified by measurement in Task 1.2 |
| py gym templates (env.rs:149 cap/fuel `1.0e-12`; trader template env.rs:330-331 fuel `1.0e-9`, v_e 2.0) | — | — | NO change: `1e-12` starts BELOW the new eps (edge still unarmed there); the trader tank goes live but needs ~7,900 thrusting ticks to cross — outside python/tests horizons. Verified by pytest in Task 1.1 step 6; any re-timed py test = STOP and surface, do not nudge |

**Redesign arithmetic (both starved families share one craft spec)** — dry
`1e-9`, max_thrust `1e-12`, v_e `1e-2`, dt `0.25` ⇒ burn/tick at full throttle
= `1e-12/1e-2·0.25 = 2.5e-11`. The old `1.06e-9` is the old eps `1e-9` plus a
`6e-11` headroom = 2.4 full-throttle ticks. The redesign keeps the SAME
headroom above the NEW eps: `7.0e-11 = 1e-11 + 6e-11`. Tick-by-tick: tick 1
(load/dispatch tick) `7e-11 → 4.5e-11` (> eps, so the step-1 `CargoLoaded` /
`Accepted` asserts hold); tick 2 `→ 2e-11`; tick 3 burn clamps the tank to
`0 ≤ eps` with `prev = 2e-11 > eps` → FuelEmpty fires on tick 3, exactly the
old "couple of ticks in" timing, after the tick-2 stage-1c CargoLoaded→InTransit
promotion (so the world.rs test still observes the failure from `InTransit`).
dv budget check: tsiolkovsky at dispatch = `1e-2·ln(1.07) ≈ 6.77e-4`; per-tick
decrement ≈ `2.34e-4`; remaining ≈ `2.04e-4 > 0` entering tick 3 — the tank
loses the race, as designed. (Left at `1.06e-9` under the new eps, the fixtures
would instead race dv-exhaustion out at ~tick 42 — intent broken either way it
resolves, which is why the redesign rides in the same single-cause commit.)

---

### Task 1.1: FUEL_EMPTY_EPS 1e-9 → 1e-11 + edge-arming pin + starved-fixture redesign (one single-cause commit)

Files:
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/events.rs` (const at :16; new test in `mod tests` after `fuel_just_emptied_fires_only_on_depletion_edge`, :179-190)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/world.rs` (:2188-2206 — fixture doc + `fuel_mass` literal at :2203)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/economy.rs` (:2360-2364 fn doc; :2416-2417 `fuel_mass` literal + comment)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/tests/replay_equivalence.rs` (:36-41 — comment-only documentation of the keep decision)

- [ ] **Step 1: Failing test first — pin that the edge can arm for a band-scale tank.** In `/home/john/jumpgate/crates/jumpgate-core/src/events.rs`, inside `#[cfg(test)] mod tests`, directly after `fuel_just_emptied_fires_only_on_depletion_edge` (events.rs:190), add:

```rust
    #[test]
    fn fuel_edge_arms_for_band_scale_tank_draining_through_eps() {
        // The world-gets-big eps re-bake (spec §4 item 1): a band-scale tank
        // (1.0e-9 — every scenario_trophic craft, scenario.rs) must be able to
        // ARM the edge. At the old eps (1e-9) prev == eps exactly and the
        // strict `>` in fuel_just_emptied made FuelEmpty arithmetically
        // unfireable for the whole band.
        assert!(fuel_just_emptied(0.0, 1.0e-9), "band-scale tank fires on its dry tick");
        // A tank draining THROUGH eps (at/below now, strictly above before)
        // arms without ever touching exact zero.
        assert!(fuel_just_emptied(FUEL_EMPTY_EPS * 0.5, FUEL_EMPTY_EPS * 2.0));
        // The strict-greater pin is unchanged: a tank parked AT eps never fires.
        assert!(!fuel_just_emptied(0.0, FUEL_EMPTY_EPS));
    }
```

  Run: `cargo test -p jumpgate-core --lib fuel_edge_arms_for_band_scale_tank_draining_through_eps`
  Expected FAILURE (at the current eps `1e-9`, `1.0e-9 > 1e-9` is false):

```
thread 'events::tests::fuel_edge_arms_for_band_scale_tank_draining_through_eps' panicked at crates/jumpgate-core/src/events.rs:...:
band-scale tank fires on its dry tick
...
test result: FAILED. 0 passed; 1 failed
```

- [ ] **Step 2: Minimal implementation — flip the const, with provenance doc.** In `/home/john/jumpgate/crates/jumpgate-core/src/events.rs:15-16` replace:

```rust
/// Quantization epsilon for fuel comparisons (fuel at/below this == "empty").
pub const FUEL_EMPTY_EPS: f64 = 1e-9;
```

  with:

```rust
/// Quantization epsilon for fuel comparisons (fuel at/below this == "empty").
/// 1e-11 since the world-gets-big eps re-bake (spec §4 item 1; was 1e-9):
/// every band tank is 1.0e-9 (scenario_trophic, scenario.rs) — exactly AT the
/// old eps — and the strict `fuel_prev > FUEL_EMPTY_EPS` edge below was
/// arithmetically unfireable for the whole band. At 1e-11 a 1e-9 tank sits
/// 100x above eps, so the gauge lives outside the dead zone. eps appears in
/// NO physics expression: burn arithmetic, state_hash, and config_hash are
/// untouched by this change (HASH_FORMAT_VERSION stays 5; zero goldens move).
pub const FUEL_EMPTY_EPS: f64 = 1e-11;
```

  Run: `cargo test -p jumpgate-core --lib events::tests::fuel`
  Expected PASS — both edge tests (the old `fuel_just_emptied_fires_only_on_depletion_edge` is const-relative and survives untouched):

```
test events::tests::fuel_edge_arms_for_band_scale_tank_draining_through_eps ... ok
test events::tests::fuel_just_emptied_fires_only_on_depletion_edge ... ok
test result: ok. 2 passed
```

  Do NOT run the wider suite yet: the two starved-contract fixtures still encode the OLD eps's headroom and are redesigned in steps 3–4.

- [ ] **Step 3: Redesign Family A — `two_body_starved_contract_fixture` (world.rs).** In `/home/john/jumpgate/crates/jumpgate-core/src/world.rs:2188-2206`, update the fixture's `///` doc (the "(1e-9)" reference) and replace the body comment + literal:

```rust
    /// A STARVED variant of `two_body_contract_fixture`: the craft can still accept
    /// and load at station A (loading pulls economy Fuel cargo from A's stock, which
    /// is independent of propellant `fuel_mass`), but its propellant is exhausted
    /// mid-transit before it can rendezvous with station B, so a `FuelEmpty` event
    /// fires while the contract is `InTransit`. The lever is `fuel_mass`: it starts
    /// just above `FUEL_EMPTY_EPS` (1e-11), enough to survive step 1 (still
    /// `CargoLoaded`, so the once-only FuelEmpty edge must NOT fire there) but drained
    /// across the eps threshold a couple of ticks into the burn, long before the craft
    /// can cover the 0.3 AU to station B.
    fn two_body_starved_contract_fixture() -> RunConfig {
        let mut cfg = two_body_contract_fixture();
        // REDESIGNED (not nudged) for the eps re-bake 1e-9 -> 1e-11 (spec §4
        // item 1; was 1.06e-9 = old eps + 6e-11). Same 6e-11 headroom above the
        // NEW eps = 2.4 full-throttle burn ticks (burn/tick = max_thrust/v_e*dt
        // = 1e-12/1e-2*0.25 = 2.5e-11). Tick 1 (load+dispatch): 7e-11 -> 4.5e-11
        // (survives step 1; the once-only edge must NOT fire while CargoLoaded);
        // tick 2: -> 2e-11; tick 3: clamped to 0 <= eps with prev 2e-11 > eps ->
        // FuelEmpty fires while InTransit (the stage-1c promotion ran on tick 2),
        // long before the craft covers the 0.3 AU to station B.
        cfg.craft[0].fuel_mass = 7.0e-11;
        cfg
    }
```

  Run: `cargo test -p jumpgate-core --lib starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss`
  Expected PASS: `test result: ok. 1 passed`.

- [ ] **Step 4: Redesign Family B — `starved_two_body_contract_fixture` (economy.rs).** In `/home/john/jumpgate/crates/jumpgate-core/src/economy.rs`, update the fn doc at :2360-2364 (the "(1e-9)" reference → "(1e-11)") and the craft literal at :2416-2417. The fn doc tail becomes:

```rust
    /// contract `from_station_index -> to_station_index`, and one manual (unscripted)
    /// hauler co-located with body 0 whose propellant starts just above
    /// `FUEL_EMPTY_EPS` (1e-11) — enough to survive step 1, but drained across the eps
    /// threshold a couple of ticks into any burn, long before it can cover 0.3 AU.
```

  and the craft field becomes:

```rust
                // REDESIGNED (not nudged) for the eps re-bake 1e-9 -> 1e-11
                // (spec §4 item 1; was 1.06e-9 = old eps + 6e-11). Same 6e-11
                // headroom above the NEW eps = 2.4 full-throttle burn ticks
                // (1e-12/1e-2*0.25 = 2.5e-11/tick): survives step 1 at 4.5e-11,
                // runs dry across eps on tick 3 — both the Accepted deadhead arm
                // and the CargoLoaded-window arm keep their step-1 asserts.
                fuel_mass: 7.0e-11,
```

  Run: `cargo test -p jumpgate-core --lib fuel_empty_mid_deadhead_refunds_escrow`
  Expected PASS: `test result: ok. 1 passed` (both arms live inside the one test fn).

- [ ] **Step 5: Document the replay_equivalence keep decision (comment only — no value change).** In `/home/john/jumpgate/crates/jumpgate-core/tests/replay_equivalence.rs:41`, replace the bare `fuel_mass: 5.0e-10,` line with:

```rust
            // Half a tank: a real multi-hundred-tick burn for the replay/corruption
            // tests. NOTE (eps re-bake, spec §4 item 1): 5.0e-10 sat BELOW the old
            // FUEL_EMPTY_EPS (1e-9, edge unarmed) and is ABOVE the new 1e-11.
            // Deliberately NOT lowered: this config has no contracts, so a
            // FuelEmpty event (reachable only near tick ~196 of a full-throttle
            // 200-tick run; burn/tick = 1e-13/0.02*0.5 = 2.5e-12) is state-inert
            // and fires identically in the record and replay arms. Lowering the
            // tank below 1e-11 would gut the burn these tests exist to record.
            fuel_mass: 5.0e-10,
```

  Run: `cargo test -p jumpgate-core --test replay_equivalence`
  Expected PASS: `test result: ok. 6 passed`.

- [ ] **Step 6: Full verification before the commit.** Run, in order:
  - `cargo test -p jumpgate-core --lib` — expected: all pass, 0 failed.
  - `cargo test --workspace` — expected: all pass (replay_equivalence, physics_sanity, determinism suites; the at-eps `1e-9` fixtures are contract-free or non-burning, so the eps flip cannot move their state trajectories).
  - `cargo clippy --all-targets -- -D warnings` — expected: clean.
  - `PYTHONPATH=/home/john/jumpgate/python pytest python/tests` — expected: all pass (gym template `1e-12` starts below the new eps; the trader template's `1e-9` tank needs ~7,900 thrusting ticks to cross, outside test horizons). If ANY python test re-times or fails here, STOP and surface it — that falsifies the horizon analysis; do NOT nudge the py templates to get green.

- [ ] **Step 7: Single-cause commit.** Stage EXPLICIT paths only (never `-A`, never `.`; nothing under `runs/`):

```bash
git add crates/jumpgate-core/src/events.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/economy.rs crates/jumpgate-core/tests/replay_equivalence.rs
git commit -F - <<'EOF'
fix(events): re-bake FUEL_EMPTY_EPS 1e-9 -> 1e-11 so the FuelEmpty edge can arm (spec §4 item 1)

At eps 1e-9 every band tank (1.0e-9, scenario_trophic) sat exactly AT the
eps and the strict `prev > eps` depletion edge was arithmetically
unfireable. The two starved-contract fixture families straddling the old
eps are REDESIGNED (1.06e-9 -> 7.0e-11: the same 6e-11 = 2.4-burn-tick
headroom above the NEW eps), preserving each fixture's intent — survive
step 1, die a couple of ticks in — not nudged. replay_equivalence's
5.0e-10 half-tank is deliberately unchanged (contract-free config; a
FuelEmpty there is state-inert and record/replay-symmetric).

eps appears in no physics expression: burn arithmetic, state_hash and
config_hash are untouched (zero goldens move, HASH_FORMAT_VERSION stays
5); band hash-neutrality is proven by the cross-branch digest that
follows this commit.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2: Band hash-neutrality audit — golden anchors + cross-branch trophic digest (verification-only; no source change, no commit)

Files:
- Create: none
- Modify: none (measurement task; evidence goes in the completion report, never into a `.md` artifact in-repo and never under `runs/`)

The eps commit CLAIMS band neutrality; this task measures it. Digest equality
here is a determinism measurement (the cross-branch digest discipline), not a
golden — nothing in this task writes or re-pins any literal. The
`fuel_empty=0` / `--assert-no-fuel-empty` check is the existing trophic-only
control flag (W12: "the control stays a control"), not a new gate, and no new
metric gate is introduced.

- [ ] **Step 1: Golden anchors unmoved.** Run:

```bash
cargo test -p jumpgate-core --lib golden
```

  Expected PASS for all three anchor tests, 0 failed:
  - `hash::tests::state_hash_golden_zero_world`
  - `hash::tests::golden_zero_state_hash` (manual-fold encoding pin)
  - `config::tests::config_hash_golden_anchor_is_stable`

  Then confirm the eps commit touched no golden-bearing file:

```bash
git diff HEAD^ -- crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/config.rs
```

  Expected: EMPTY output. `GOLDEN_ZERO_STATE_HASH` stays `0x0f20_843f_ccfd_8c70` (hash.rs:129), `GOLDEN_CONFIG_HASH` stays `0xee02_df67_1889_78dc` (config.rs:745), `HASH_FORMAT_VERSION` stays 5. If this diff is non-empty, the eps commit was not single-cause — STOP and surface; never re-pin a golden in this phase (the phase-1 golden budget is the RefuelCfg re-pin, which belongs to the refuel task, not this one).

- [ ] **Step 2: Cross-branch trophic digest (pre-eps vs post-eps, bit-identical).** Baseline = the parent of the Task 1.1 commit. Build both arms in the SAME profile (`--release` for both):

```bash
git worktree add /tmp/wgb-eps-base HEAD^
for seed in 1 7; do
  cargo run --manifest-path /tmp/wgb-eps-base/Cargo.toml --release -p jumpgate-core --example trophic_run -- \
    --seed $seed --ticks 2000 --jsonl /tmp/eps-base-s$seed.jsonl --replay-check --assert-no-fuel-empty \
    > /tmp/eps-base-s$seed.txt
  cargo run --manifest-path /home/john/jumpgate/Cargo.toml --release -p jumpgate-core --example trophic_run -- \
    --seed $seed --ticks 2000 --jsonl /tmp/eps-after-s$seed.jsonl --replay-check --assert-no-fuel-empty \
    > /tmp/eps-after-s$seed.txt
  diff /tmp/eps-base-s$seed.txt /tmp/eps-after-s$seed.txt
  sha256sum /tmp/eps-base-s$seed.jsonl /tmp/eps-after-s$seed.jsonl
done
```

  Expected, per seed:
  - `diff` prints NOTHING (stdout — `RESULT ... fuel_empty=0 ...`, every `window@` line, and `replay-check OK` — is byte-identical across branches).
  - `sha256sum` prints the SAME digest for the base and after `.jsonl` files.
  - Both arms exit 0 with `--assert-no-fuel-empty` (zero FuelEmpty on the band at BOTH eps values: the band's `1e-9` tanks at v_e 20 burn `1.25e-14`/tick and cannot reach `1e-11` inside the run).

  If any byte differs: STOP. The eps commit is single-cause, so a divergence falsifies the spec-§4 "band runs unaffected" assumption itself — surface to the owner with the first differing line; do not rationalize, do not re-bake fixtures, do not touch goldens.

- [ ] **Step 3: Clean up and record evidence.**

```bash
git worktree remove /tmp/wgb-eps-base
```

  Paste into your completion report: the three golden-anchor test names with their `ok` lines, the empty-diff confirmation from step 1, and both seeds' matched `sha256sum` pairs + `replay-check OK` lines from step 2. No commit in this task (nothing changed); the next phase-1 task (RefuelCfg) starts from the Task 1.1 commit.


---

## Phase 1 — the refuel verb (spec §5, §7 Refueled/ContractFailed, §9 phase-1 rest)

> Drafted against HEAD `e7e490e`. Prerequisite within phase 1: **Task 1.1 (the
> eps 1e-11 re-bake + fixture redesign, own commit — owned by the fuel-edge
> section)** lands BEFORE Task 1.2.6 (the PLAY-C1 dispatch filter compares
> `fuel_mass > FUEL_EMPTY_EPS`, and every trophic tank is exactly the OLD eps —
> with eps still 1e-9 the filter would blacklist every full-tank band hauler).
> Tasks 1.2.1–1.2.5 and 1.2.7 do not read the eps and may land before or after
> 1.1; the stated order below is the safe one.
>
> House rules every commit step obeys: `git add` EXPLICIT paths only (never
> `-A`, never `.`); never stage `runs/`; commit messages via `git commit -F -`
> heredoc ending with the exact trailer
> `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`;
> golden literals are NEVER typed from this plan — re-pins paste the output of
> the `print_golden_config` printer test, single-cause commits. Reward
> surfaces untouched. Windows are recorded, never gated.

---

### Task 1.2.1: `RefuelCfg { lot_mass, corp_index }` — config surface, exhaustive fold, half-on reset error, ONE golden re-pin

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` (struct after `MediaCfg` ~config.rs:398; `RunConfig` tail config.rs:407-436; `config_hash` destructure config.rs:503-521 + tail fold after the MediaCfg block config.rs:690-713; `CONFIG_FIELD_ORDER` doc config.rs:480-498; `sample()` config.rs:747-793; `GOLDEN_CONFIG_HASH` config.rs:745; new `changing_refuel_cfg_changes_config_hash` test cloning config.rs:989-999)
- Modify (RunConfig-literal compile fixes, `refuel: RefuelCfg::default()` appended after `media:` in each): `crates/jumpgate-core/src/scenario.rs` (~:229-253), `crates/jumpgate-core/src/economy.rs` (`vendor_world_fixture` ~:1564-1611, `starved_two_body_contract_fixture` ~:2364-2455), `crates/jumpgate-core/src/world.rs` (test fixtures incl. `one_body_one_thrusting_craft` ~:1282-1297, `two_body_starved_contract_fixture` ~:2196-2205), `crates/jumpgate-core/src/ingest.rs`, `crates/jumpgate-core/src/pirate.rs`, `crates/jumpgate-core/src/hash.rs`, `crates/jumpgate-core/src/diagnostics.rs`, `crates/jumpgate-core/tests/replay_equivalence.rs` (~:25-44), `crates/jumpgate-core/tests/physics_sanity.rs`, `crates/jumpgate-py/src/env.rs` (~:187-213, ~:464-490)
- Modify (commit 2): `crates/jumpgate-core/src/world.rs` (`ResetError` ~:142-155, `Display` ~:157-176, `World::reset` validation after the media half-on check ~:207-211; new reset-error test cloning the `BadMediaCfg` test at world.rs:1700-1712)

- [ ] **Step 1: failing test first.** In `crates/jumpgate-core/src/config.rs` tests, clone `changing_media_cfg_changes_config_hash` (config.rs:989-999):

```rust
    #[test]
    fn changing_refuel_cfg_changes_config_hash() {
        let base = sample().config_hash();
        let mut cfg = sample();
        cfg.refuel.lot_mass = 5e-11;
        assert_ne!(cfg.config_hash(), base, "lot_mass must be folded");
        let mut cfg = sample();
        cfg.refuel.corp_index = 4;
        assert_ne!(cfg.config_hash(), base, "corp_index must be folded");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core changing_refuel_cfg` → expected failure:** compile error `E0609: no field `refuel` on type `RunConfig`` (the exhaustive-destructure discipline working as designed).
- [ ] **Step 3: implement the struct + field + fold.** In `config.rs`, after the `MediaCfg` impl block (~:398):

```rust
/// The propellant-purchase verb (world-gets-big rung §5). Inert by default:
/// `lot_mass == 0.0` makes BOTH refuel stages (1c3b `run_refuel_policies`,
/// 1d2 `resolve_refuels`) deterministic no-ops — the named trophic-inertness
/// gate (scenario_trophic leaves this default-off; proven by the phase-exit
/// cross-branch digest, Task 1.2.7).
#[derive(Clone, Copy, Debug)]
pub struct RefuelCfg {
    /// Propellant mass per integer lot (same f64 unit as `fuel_mass`).
    /// `0.0` = the refuel verb is OFF. The settle decision is integer lots:
    /// `units = min(floor((cap_eff - fuel)/lot), stock[Fuel], credits/price)`.
    pub lot_mass: f64,
    /// Corporation (config index) credited with every refuel payment — the
    /// Port corp (the Yard precedent, `ShipyardCfg.corp_index`: dense
    /// slot == row; a stale/out-of-range row is a deterministic settle skip,
    /// never a one-legged debit). The frontier factory (phase 2) appends a
    /// `CorporationInit { treasury_micros: 0, .. }` Port corp and points this
    /// at it; on a lot-0 world this index is never read.
    pub corp_index: u32,
}

impl Default for RefuelCfg {
    fn default() -> Self {
        RefuelCfg { lot_mass: 0.0, corp_index: 0 }
    }
}
```

  On `RunConfig` (after `media`, config.rs:436):

```rust
    // World-gets-big rung (folded AFTER media, append-only). Default leaves the
    // refuel machinery inert (lot_mass == 0.0 => both refuel stages no-op).
    pub refuel: RefuelCfg,
```

  In `config_hash`: add `refuel, // NEW (world-gets-big): destructure forces folding below` to the top-level destructure (config.rs:521), extend the `CONFIG_FIELD_ORDER` doc list with `///  26. refuel: lot_mass.to_bits(), corp_index`, and append at the VERY tail, after the MediaCfg field writes (config.rs:713), before `ConfigHash(h.finish())`:

```rust
        // WORLD-GETS-BIG RUNG (TAIL, append-only — CONFIG_FIELD_ORDER 26). The
        // byte stream above stays byte-identical; this only EXTENDS it.
        // Exhaustive destructure: a NEW RefuelCfg field is a COMPILE ERROR here
        // until explicitly folded (the D10/M6 discipline).
        let RefuelCfg { lot_mass, corp_index } = refuel;
        h.write_u64(lot_mass.to_bits());
        h.write_u64(*corp_index as u64);
```

  Add `refuel: RefuelCfg::default(),` to `sample()` (config.rs:792).
- [ ] **Step 4: fix every RunConfig literal in the workspace.** Run `cargo build --workspace 2>&1 | grep -c E0063` — every error site is a struct literal missing the new field. Append `refuel: RefuelCfg::default(),` (or `refuel: jumpgate_core::config::RefuelCfg::default(),` in `crates/jumpgate-py/src/env.rs` :213/:490 — match the `media:` line's path style at each site) to EVERY listed literal: scenario.rs factory, economy.rs fixtures, world.rs fixtures, ingest.rs, pirate.rs, hash.rs, diagnostics.rs fixtures, tests/replay_equivalence.rs, tests/physics_sanity.rs, jumpgate-py env.rs. Re-run `cargo build --workspace` → clean.
- [ ] **Step 5: re-pin the config golden (the ONLY golden that moves this rung).** Run `cargo test -p jumpgate-core config_hash_golden_anchor` → expected failure: `config_hash drifted: re-pin only if intentional`. Then run

```
cargo test -p jumpgate-core --lib print_golden_config -- --ignored --nocapture
```

  and paste ITS printed hex (never hand-computed, never taken from this plan) into config.rs:745, keeping the provenance comment discipline on the literal's line:

```rust
    const GOLDEN_CONFIG_HASH: u64 = 0x<PASTE_PRINTED_VALUE>; // RE-PINNED: +RefuelCfg{lot_mass,corp_index} folded at config tail (world-gets-big §5). Was 0xee02_df67_1889_78dc.
```

- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core config` → all green including `changing_refuel_cfg_changes_config_hash` and the re-pinned anchor. `cargo test --workspace` green (state goldens untouched — the new field is config-side only; `HASH_FORMAT_VERSION` stays 5).
- [ ] **Step 7: commit (single cause: the RefuelCfg fold).**

```
git add crates/jumpgate-core/src/config.rs crates/jumpgate-core/src/scenario.rs \
  crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs \
  crates/jumpgate-core/src/ingest.rs crates/jumpgate-core/src/pirate.rs \
  crates/jumpgate-core/src/hash.rs crates/jumpgate-core/src/diagnostics.rs \
  crates/jumpgate-core/tests/replay_equivalence.rs crates/jumpgate-core/tests/physics_sanity.rs \
  crates/jumpgate-py/src/env.rs
git commit -F - <<'EOF'
feat(world-gets-big): RefuelCfg folded at config tail (GOLDEN_CONFIG_HASH re-pinned, single cause)

lot_mass (0.0 = the named trophic-inertness gate) + corp_index (the Port
corp binding, Yard precedent). CONFIG_FIELD_ORDER 26; every RunConfig
literal gains refuel: RefuelCfg::default(). No HASH_FORMAT_VERSION bump;
zero state goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 8: failing test for the half-on reset error.** In `crates/jumpgate-core/src/world.rs` tests, next to the `BadMediaCfg` half-on test (world.rs:1700-1712):

```rust
    #[test]
    fn refuel_half_on_price_surface_is_a_reset_error() {
        // lot_mass > 0 with a dead Fuel price surface would make every refuel a
        // silent `unit_price < 1` no-op — a misconfiguration, rejected before
        // tick 0 (the BadMediaCfg half-on idiom).
        let mut cfg = one_body_two_stations_one_miner();
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 5e-11, corp_index: 0 };
        // Arm 1: price_cfg.base_micros[Fuel] == 0 (the PriceCfg default).
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with base_micros[Fuel] == 0 must be rejected"
        );
        // Arm 2: base live, but a station's seeded initial_price_micros[Fuel] == 0.
        cfg.price_cfg.base_micros[crate::economy::Resource::Fuel.index()] = 5_000;
        assert!(
            matches!(World::reset(cfg.clone()), Err(ResetError::BadRefuelCfg { .. })),
            "lot_mass > 0 with a zero seeded station Fuel price must be rejected"
        );
        // Control: both live -> resolves.
        for s in cfg.stations.iter_mut() {
            s.initial_price_micros[crate::economy::Resource::Fuel.index()] = 5_000;
        }
        assert!(World::reset(cfg).is_ok(), "fully-on refuel config resolves");
    }
```

  (`one_body_two_stations_one_miner()` is the exact fixture the neighbouring `BadMediaCfg` half-on test `half_on_media_config_is_rejected` uses — defined at `world.rs:1648`. It carries two distinct station entities, which the reset-guard arms above need.)
- [ ] **Step 9: run `cargo test -p jumpgate-core refuel_half_on` → expected failure:** `E0599: no variant named `BadRefuelCfg``.
- [ ] **Step 10: implement.** New `ResetError` variant (after `BadMediaCfg`, world.rs:154):

```rust
    /// A half-on `RefuelCfg`: `lot_mass > 0` while the Fuel price surface is
    /// structurally dead (`price_cfg.base_micros[Fuel] == 0`, the cap-0/base-0
    /// update_prices skip) or any station's seeded
    /// `initial_price_micros[Fuel] == 0`. Every refuel would be a silent
    /// `unit_price < 1` settle skip — a misconfiguration, rejected before
    /// tick 0 (the media half-on idiom).
    BadRefuelCfg { reason: &'static str },
```

  Display arm (after the BadMediaCfg arm, world.rs:170-172):

```rust
            ResetError::BadRefuelCfg { reason } => {
                write!(f, "bad refuel config: {reason}")
            }
```

  Validation in `World::reset`, directly after the media half-on check (world.rs:211):

```rust
        // Refuel half-on validation (world-gets-big §5, the BadMediaCfg idiom):
        // a live lot size demands a live Fuel price surface, config-wide.
        if cfg.refuel.lot_mass > 0.0 {
            let fuel = crate::economy::Resource::Fuel.index();
            if cfg.price_cfg.base_micros[fuel] == 0 {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but price_cfg.base_micros[Fuel] == 0 (price surface dead)",
                });
            }
            if cfg.stations.iter().any(|s| s.initial_price_micros[fuel] == 0) {
                return Err(ResetError::BadRefuelCfg {
                    reason: "lot_mass > 0 but a station's seeded initial_price_micros[Fuel] == 0",
                });
            }
        }
```

- [ ] **Step 11: run + expected pass.** `cargo test -p jumpgate-core refuel_half_on` green; `cargo test -p jumpgate-core` green (no behavior change for lot-0 configs — every existing fixture).
- [ ] **Step 12: commit.**

```
git add crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): BadRefuelCfg reset error — the refuel half-on idiom

lot_mass > 0 with base_micros[Fuel] == 0 or any zero seeded station Fuel
price is rejected before tick 0 (the BadMediaCfg precedent). Hash-neutral.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.2: `CraftStore.pending_refuel: Vec<Option<()>>` — transient column at all THREE sizing sites + the all-None hash-point assert

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs` (field after `pending_upgrade` ~:203; `empty()` ~:224-248; `push()` ~:255-285; length-parallel test asserts ~:412-482)
- Modify: `crates/jumpgate-core/src/world.rs` (reset's hand-built literal ~:232-254 and per-craft loop ~:255-303 — reset does NOT use `push`)
- Modify: `crates/jumpgate-core/src/hash.rs` (extend the all-None debug_assert block hash.rs:301-309)

- [ ] **Step 1: failing test first.** Extend the store length-parallel tests in `stores.rs` (`stores_construct_soa_parallel` at :412 and the push test asserting `pending_upgrade.len()` at ~:482) with, in each, immediately after the `pending_upgrade` assert:

```rust
        assert_eq!(ship.pending_refuel.len(), ship.ids.len());
```

- [ ] **Step 2: run `cargo test -p jumpgate-core stores` → expected failure:** `E0609: no field `pending_refuel` on type `CraftStore``.
- [ ] **Step 3: implement the column at all three sites.** `stores.rs` field, directly after `pending_upgrade` (:203):

```rust
    /// TRANSIENT refuel intent (world-gets-big §5 — the `pending_upgrade`
    /// precedent, same strictness): written by ingest (`CommandKind::Refuel`)
    /// or the scripted stage 1c3b, consumed by `resolve_refuels` (stage 1d2)
    /// the SAME tick, so it is always `None` at every hash point —
    /// `state_hash` debug_asserts exactly that. NOT folded into
    /// HASH_FIELD_ORDER; no HASH_FORMAT_VERSION bump.
    pub pending_refuel: Vec<Option<()>>,
```

  `empty()` (:245, after `pending_upgrade: Vec::new(),`): `pending_refuel: Vec::new(),` — `push()` (:280, after `self.pending_upgrade.push(None);`): `self.pending_refuel.push(None);`. In `world.rs` reset: the hand-built struct literal gains `pending_refuel: Vec::new(),` (after :252's `pending_upgrade: Vec::new(),`) and the per-craft loop gains `ships.pending_refuel.push(None);` (after :294's `ships.pending_upgrade.push(None);` — reset bypasses `push`, all columns must stay length-parallel or the hash's dense-row unwraps panic).
- [ ] **Step 4: extend the hash-point assert.** In `hash.rs`, directly after the existing block at :306-309:

```rust
    // `pending_refuel` is TRANSIENT intent (world-gets-big §5): written and
    // consumed within one tick (stage 1d2), so it must be empty at EVERY hash
    // point. A `Some` here is a stage-ordering bug — fail loud in debug.
    debug_assert!(
        world.ships.pending_refuel.iter().all(Option::is_none),
        "pending_refuel must be fully consumed (all None) at every state-hash point"
    );
```

- [ ] **Step 5: run + expected pass.** `cargo test -p jumpgate-core stores` green; `cargo test -p jumpgate-core` green (goldens untouched: the column is never folded; `recompute_with_cursors` needs no mirror — nothing was added to the fold stream).
- [ ] **Step 6: commit.**

```
git add crates/jumpgate-core/src/stores.rs crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/hash.rs
git commit -F - <<'EOF'
feat(world-gets-big): pending_refuel transient column (3 sizing sites + all-None hash assert)

The pending_upgrade precedent: stores empty()/push() + World::reset's
hand-built literal and per-craft loop; joins the all-None-at-every-hash-
point debug_assert. NOT hashed; no format bump; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.3: `resolve_refuels` at stage 1d2 — always-consume-then-gate settle, integer quantization, four legs, `Refueled` event, FLOOR-permille pinned

**Files:**
- Modify: `crates/jumpgate-core/src/contract.rs` (`EventKind` — append `Refueled` at the enum tail, after `GossipHeard` ~:168)
- Modify: `crates/jumpgate-core/src/economy.rs` (new `docked_station_row` next to `docked_at_vendor` ~:926-952; new `resolve_refuels` after `resolve_purchases` ~:924; tests: `refuel_world_fixture`, `refuel_settles_quantized_with_four_legs_and_exact_event`, `refuel_tank_permille_is_floor_rounded`, `assert_refuel_skipped`, `refuel_skips_deterministically`)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 1d2 call after `resolve_purchases` :776-786, before the physics body snapshot at :789)

- [ ] **Step 1: failing tests first.** In `economy.rs` `#[cfg(test)]`, the fixture (clone of `vendor_world_fixture`'s shape with the Fuel price surface live) plus the exact-settle test:

```rust
    /// Refuel fixture: the vendor fixture's one-body/one-station/one-craft dock
    /// with the Fuel price surface LIVE (base 5_000, cap 40 — the §5 frontier
    /// shape; cap[Ore] == 0 keeps Ore structurally dead), the reprice clock OFF
    /// (interval 0, world.rs guard) so the seeded price 5_000 is the settle
    /// price for exact-integer assertions, station Fuel stock 40, and
    /// `RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 }` — 4 lots fill the 1e-9
    /// tank; corp 0 doubles as the Port corp with treasury 0 so every credited
    /// micro is refuel money. The craft starts DRY (fuel_mass 0.0; prev == fuel
    /// at reset, so no spurious FuelEmpty edge).
    fn refuel_world_fixture() -> crate::config::RunConfig {
        let mut cfg = vendor_world_fixture(false);
        cfg.craft[0].fuel_mass = 0.0;
        cfg.stations[0].initial_stock = [0, 40];
        cfg.stations[0].initial_price_micros = [0, 5_000];
        cfg.price_cfg = crate::config::PriceCfg {
            base_micros: [0, 5_000],
            cap: [0, 40],
            slope_milli: 1800,
            reprice_interval: 0, // clock OFF: the seeded 5_000 is the settle price
        };
        cfg.refuel = crate::config::RefuelCfg { lot_mass: 2.5e-10, corp_index: 0 };
        cfg
    }

    #[test]
    fn refuel_settles_quantized_with_four_legs_and_exact_event() {
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        // The integer decision: need = floor((1e-9 - 0)/2.5e-10) = 4 lots;
        // afford = 12_000 / 5_000 = 2; stock = 40 => units = min(4, 40, 2) = 2.
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());

        let f = Resource::Fuel.index();
        assert_eq!(world.stations.stock[0][f], 38, "stock leg: -= units");
        assert_eq!(world.econ.consumed[f], 2, "sink leg: consumed[Fuel] += units");
        assert_eq!(world.ships.credits_micros[0], 2_000, "wallet leg: debited EXACTLY units*price");
        assert_eq!(world.corporations.treasury_micros[0], 10_000, "Port treasury credited the same (pure transfer)");
        assert_eq!(world.ships.fuel_mass[0], 5.0e-10, "tank leg: fuel += units*lot (one clamped write)");
        assert_eq!(world.ships.pending_refuel[0], None, "intent consumed");
        // Resource identity: the stock leg exits through `consumed`, exactly
        // like a producer input leg (Σstock + in_transit == initial + mined − consumed).
        let stock_now: i64 = world.stations.stock.iter().map(|s| s[f]).sum();
        assert_eq!(stock_now, 40 + world.econ.mined[f] - world.econ.consumed[f], "resource identity holds");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    craft: c,
                    units: 2,
                    price_micros: 5_000,
                    tank_before_permille: 0,
                    tank_after_permille: 500,
                    ..
                } if c == craft
            )),
            "Refueled emitted with the exact quantized payload"
        );
        // The transient-column invariant survives the hash point.
        let _ = crate::hash::state_hash(&world);
    }
```

  The FLOOR-rounding pin (the event fields go through `permille_floor`, the Task 0b.1 seam — this test pins that the EVENT carries the seam's FLOOR semantics):

```rust
    #[test]
    fn refuel_tank_permille_is_floor_rounded() {
        // Pins FLOOR through the permille_floor seam (Task 0b.1): 555.5
        // and 805.5 both FLOOR — never half-up, never round-to-nearest.
        use crate::world::World;
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].fuel_mass = 5.555e-10; // 555.5 permille of the 1e-9 tank
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 5_000; // afford exactly 1 unit
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        // need = floor((1e-9 - 5.555e-10)/2.5e-10) = floor(1.778) = 1; units = 1.
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::Refueled {
                    units: 1,
                    tank_before_permille: 555,
                    tank_after_permille: 805,
                    ..
                }
            )),
            "tank permilles are FLOOR-rounded against EFFECTIVE capacity"
        );
    }
```

  The skip catalogue (clone of `purchase_skips_deterministically` + `assert_purchase_skipped`, economy.rs:1623-1743):

```rust
    /// Skip-arm postcondition: zero movement on every leg, intent consumed,
    /// NO Refueled event (the assert_purchase_skipped pattern).
    fn assert_refuel_skipped(
        world: &mut crate::world::World,
        credits_before: i64,
        fuel_before: f64,
        stock_before: i64,
        arm: &str,
    ) {
        let f = Resource::Fuel.index();
        assert_eq!(world.ships.fuel_mass[0], fuel_before, "{arm}: tank untouched");
        assert_eq!(world.ships.credits_micros[0], credits_before, "{arm}: zero wallet movement");
        assert_eq!(world.stations.stock[0][f], stock_before, "{arm}: stock untouched");
        assert_eq!(world.econ.consumed[f], 0, "{arm}: no sink leg");
        assert_eq!(world.corporations.treasury_micros[0], 0, "{arm}: Port treasury untouched");
        assert_eq!(world.ships.pending_refuel[0], None, "{arm}: intent consumed");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "{arm}: NO Refueled event"
        );
    }

    #[test]
    fn refuel_skips_deterministically() {
        use crate::math::Vec3;
        use crate::world::World;
        let f = Resource::Fuel.index();

        // (a) UNDOCKED: ~10_000x ARRIVAL_RADIUS from the station body.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "undocked");

        // (b) STOCK 0: the dry dock (the stranding arc's substrate).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.stock[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 0, "stock-0");

        // (c) WALLET SHORT: one micro short of one unit.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 4_999, 0.0, 40, "wallet-short");

        // (d) TANK FULL: headroom < 1 lot (need == 0).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 1.0e-9, 40, "tank-full");

        // (e) UNIT PRICE < 1: live store row zeroed in-test (the curve cannot
        //     reach 0 at slope 1800; the settle never divides by a dead price).
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.stations.price_micros[0][f] = 0;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "price-0");

        // (f) STALE PORT-CORP ROW: corp_index out of range — never a
        //     one-legged debit (the Yard id_at liveness precedent).
        let mut cfg = refuel_world_fixture();
        cfg.refuel.corp_index = 7;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_refuel_skipped(&mut world, 12_000, 0.0, 40, "stale-corp");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_settles` → expected failure:** `E0599: no variant named `Refueled` found for enum `EventKind`` (then, once the variant exists, the settle assertions fail — the stage doesn't exist).
- [ ] **Step 3: implement the event variant.** Append at the TAIL of `EventKind` in `contract.rs` (after `GossipHeard`, ~:168 — events are NOT hashed; append-only by convention):

```rust
    // --- World-gets-big events (refuel rung §5/§7; hash-neutral like all events) ---
    /// A craft bought propellant at a station dock (stage 1d2). `units` is the
    /// integer lot count actually settled (`min(need, stock, afford)`),
    /// `price_micros` the per-unit price read from the dock's live price row at
    /// settle. Tank permilles are FLOOR-rounded against EFFECTIVE capacity;
    /// `tank_after_permille` derives from the decided integer purchase (the
    /// same clamped write the tank leg performs).
    Refueled {
        craft: CraftId,
        station: StationId,
        units: i64,
        price_micros: i64,
        tank_before_permille: u32,
        tank_after_permille: u32,
    },
```

- [ ] **Step 4: implement the dock predicate + the settle stage** in `economy.rs`, after `docked_at_vendor` (~:952). The tank permilles go through `permille_floor` — the ONE f64→integer seam Task 0b.1 created in `diagnostics.rs`; `economy.rs` does not import diagnostics today, so add `use crate::diagnostics::permille_floor;` to its imports (never re-spell the FLOOR inline — a second rounding implementation can drift from the seam):

```rust
/// Any-station dock predicate (world-gets-big §5): the FIRST (lowest dense
/// row — deterministic tie-break for overlapping fixture bodies) station whose
/// body is within `ARRIVAL_RADIUS` of the craft, compared in the craft's frame
/// (`body_pos` at `prev == t-1`; the try_load precedent). Shared by
/// `run_refuel_policies` (stage 1c3b) and `resolve_refuels` (stage 1d2) so
/// policy intent and same-tick settle agree on what "docked" means. Unlike
/// `docked_at_vendor` there is NO `sells_upgrades` filter: every dock sells
/// propellant when the price surface is live.
fn docked_station_row(
    ships: &CraftStore,
    crow: usize,
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    prev: Tick,
) -> Option<usize> {
    (0..stations.ids.len()).find(|&srow| {
        let body = stations.body[srow];
        bodies
            .ids
            .dense_index(body.slot, body.generation)
            .is_some_and(|brow| {
                let bpos = eph.body_pos(bodies.eph_index[brow], prev);
                ships.pos[crow].sub(bpos).length() <= crate::autopilot::ARRIVAL_RADIUS
            })
    })
}

/// Refuel settle stage — stage 1d2, world-gets-big §5 (after
/// `resolve_purchases`, PRE-physics: the same-tick burn draws from the
/// refilled tank, and the stage-4 `prev_fuel` copy-forward is untouched —
/// Class-3 pinning and the FuelEmpty edge are undisturbed).
///
/// Consumes EVERY `pending_refuel` intent THIS tick (the transient-column
/// invariant `state_hash` debug_asserts). The integer decision precedes every
/// write: `need = floor((cap_eff − fuel)/lot)`; `afford = credits / price`
/// (price >= 1 by the skip); `units = min(need, stock[Fuel], afford)`; then
/// four legs — `stock -= units` · `consumed[Fuel] += units` (the sink leg the
/// resource identity demands) · wallet -> Port corp treasury (a pure transfer,
/// zero new identity legs) · `fuel_mass += units*lot` clamped to cap in ONE
/// write. Propellant mass lives OUTSIDE both identities by design: the traded
/// Fuel stock exits through `consumed` exactly like a producer input leg, and
/// the tank is not a resource store.
///
/// Deterministic no-op skips (every one a bare `continue` AFTER the intent is
/// consumed): undocked / `unit_price < 1` / stock 0 / stale Port-corp row /
/// tank full / wallet short. Top-to-full, threshold-free — no taste scalar.
#[allow(clippy::too_many_arguments)]
pub fn resolve_refuels(
    ships: &mut CraftStore,
    stations: &mut StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    corporations: &mut CorporationStore,
    counters: &mut EconCounters,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
    events: &mut EventStream,
) {
    if refuel.lot_mass <= 0.0 {
        // The trophic-inertness gate (world-gets-big §5). Still consume any
        // manual-ingest intents so the all-None hash invariant holds: a Refuel
        // command against a lot-0 world is a deterministic no-op, never a
        // debug_assert panic at the next hash point.
        for slot in ships.pending_refuel.iter_mut() {
            *slot = None;
        }
        return;
    }
    let lot = refuel.lot_mass;
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Resource::Fuel.index();
    for crow in 0..ships.ids.len() {
        if ships.pending_refuel[crow].is_none() {
            continue;
        }
        // ALWAYS consume the intent this stage, settle or skip (the transient-
        // column invariant: `pending_refuel` is None at every hash point).
        ships.pending_refuel[crow] = None;
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue; // undocked
        };
        let unit_price = stations.price_micros[srow][fuel_r];
        if unit_price < 1 {
            continue; // dead/degenerate price row (also the afford div-by-zero guard)
        }
        let stock = stations.stock[srow][fuel_r];
        if stock <= 0 {
            continue; // dry dock
        }
        // The Port corp (the Yard precedent): a stale/out-of-range config index
        // is a deterministic skip — never a one-legged debit.
        let port_row = refuel.corp_index as usize;
        if corporations.ids.id_at(port_row).is_none() {
            continue;
        }
        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        let cap_eff = eff.fuel_capacity;
        let fuel = ships.fuel_mass[crow];
        // The integer decision precedes every write (world-gets-big §5).
        // (Rust float->int casts saturate, so a degenerate cap/lot ratio cannot UB.)
        let need = ((cap_eff - fuel) / lot).floor() as i64;
        if need < 1 {
            continue; // tank full (headroom < 1 lot)
        }
        let afford = ships.credits_micros[crow].max(0) / unit_price;
        if afford < 1 {
            continue; // wallet short
        }
        let units = need.min(stock).min(afford);
        let cost = units.saturating_mul(unit_price);
        // FLOOR-rounded tank permilles against EFFECTIVE capacity through the
        // ONE pinned f64→integer seam (Task 0b.1); `after` derives from the
        // decided integer purchase below.
        let tank_before_permille = permille_floor(fuel, cap_eff);
        // Four legs.
        stations.stock[srow][fuel_r] -= units;
        counters.consumed[fuel_r] = counters.consumed[fuel_r].saturating_add(units);
        ships.credits_micros[crow] = ships.credits_micros[crow].saturating_sub(cost);
        corporations.treasury_micros[port_row] =
            corporations.treasury_micros[port_row].saturating_add(cost);
        // ONE clamped write — never an iterative per-lot accumulation.
        ships.fuel_mass[crow] = (fuel + units as f64 * lot).min(cap_eff);
        let tank_after_permille = permille_floor(ships.fuel_mass[crow], cap_eff);
        let craft = ships.ids_at(crow);
        if let Some(station) = stations
            .ids
            .id_at(srow)
            .map(|(slot, generation)| StationId { slot, generation })
        {
            events.emit(Event {
                tick,
                kind: EventKind::Refueled {
                    craft,
                    station,
                    units,
                    price_micros: unit_price,
                    tank_before_permille,
                    tank_after_permille,
                },
            });
        }
    }
}
```

- [ ] **Step 5: wire stage 1d2 in `World::step`**, after the `resolve_purchases` call (world.rs:786), BEFORE the physics body snapshot (:789):

```rust
        // (1d2) refuel settle stage (world-gets-big §5): consume every Refuel
        //       intent written by this tick's ingest and stage 1c3b. AFTER
        //       resolve_purchases, PRE-physics: the same-tick burn draws from
        //       the refilled tank, and the dock predicate samples body_pos at
        //       `next - 1 == cur` (the try_load frame). `prev_fuel` is NOT
        //       touched here — the stage-4 copy-forward keeps Class-3 pinning
        //       and the FuelEmpty edge undisturbed. Inert at lot_mass == 0.0
        //       (the trophic-inertness gate).
        crate::economy::resolve_refuels(
            &mut self.ships,
            &mut self.stations,
            &self.bodies,
            &self.eph,
            &mut self.corporations,
            &mut self.econ,
            &self.config.refuel,
            next,
            &mut self.events,
        );
```

- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core refuel` → `refuel_settles_quantized_with_four_legs_and_exact_event`, `refuel_tank_permille_is_floor_rounded`, `refuel_skips_deterministically` all green. `cargo test -p jumpgate-core` green (every existing world is lot-0 → the stage early-returns; zero goldens move). `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 7: commit.**

```
git add crates/jumpgate-core/src/contract.rs crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): resolve_refuels at stage 1d2 — quantized four-leg settle + Refueled event

Always-consume-then-gate (pending_upgrade precedent); units =
min(floor((cap_eff-fuel)/lot), stock[Fuel], credits/price); legs: stock,
consumed[Fuel] sink, wallet->Port corp pure transfer, one clamped tank
write. FLOOR tank permilles pinned by test. prev_fuel untouched
(Class-3). Skips: undocked/price<1/stock-0/stale-corp/tank-full/wallet.
Inert at lot_mass == 0 (consumes stray intents, settles nothing).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.4: `run_refuel_policies` at stage 1c3b — scripted top-to-full intent for docked non-pirate craft

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (new `run_refuel_policies` after `run_purchase_policies` ~:1103; tests `refuel_policy_gates_deterministically`, `credit_identity_holds_across_refuels_and_policy_is_self_running`)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 1c3b call after `run_purchase_policies` :756-766, before `resolve_purchases` :776)

- [ ] **Step 1: failing tests first** (in `economy.rs` tests — note these write NO manual intent; the scripted stage must produce the whole arc):

```rust
    #[test]
    fn credit_identity_holds_across_refuels_and_policy_is_self_running() {
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000; // covers the full 4-lot fill
        let total = |w: &crate::world::World| -> i64 {
            w.corporations.treasury_micros.iter().sum::<i64>()
                + w.ships.credits_micros.iter().sum::<i64>()
                + w.contracts.escrow_micros.iter().sum::<i64>()
        };
        let t0 = total(&world);
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..50 {
            world.step(&mut empty);
            assert_eq!(total(&world), t0, "Σtreasury+Σcredits+Σescrow invariant every tick");
        }
        // Non-vacuity: the SCRIPTED policy wrote the intent (no command, no
        // manual store write) and the settle topped the dry tank to full.
        assert!(
            world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { units: 4, .. })),
            "policy-driven top-to-full refuel happened (4 lots, dry -> full)"
        );
        assert_eq!(world.ships.fuel_mass[0], 1.0e-9, "topped to capacity: 4 * 2.5e-10");
        assert_eq!(world.ships.credits_micros[0], 1_000_000 - 20_000, "4 units at 5_000");
    }

    #[test]
    fn refuel_policy_gates_deterministically() {
        use crate::world::World;
        let no_refuel = |world: &mut crate::world::World, arm: &str| {
            assert!(
                !world
                    .events_mut()
                    .since(Tick(0))
                    .iter()
                    .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
                "{arm}: the policy must not have produced a refuel"
            );
        };

        // (a) !scripted craft: gym-controlled rows are the ingest verb's job.
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].scripted = false;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "!scripted");

        // (b) pirate rows: per-class endurance spec this rung (spec §6, OD-6) —
        //     the policy is non-pirate by construction.
        let mut cfg = refuel_world_fixture();
        cfg.craft[0].role = crate::stores::CraftRole::Pirate;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "pirate");

        // (c) headroom < 1 lot: a full tank writes no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.fuel_mass[0] = 1.0e-9;
        world.ships.prev_fuel[0] = 1.0e-9;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "full-tank");

        // (d) wallet below one unit at the dock's live price: no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 4_999;
        world.step(&mut Vec::new());
        no_refuel(&mut world, "wallet-short");

        // (e) undocked: no intent.
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 12_000;
        world.ships.pos[0] = crate::math::Vec3::new(1.0, 0.0, 0.0);
        world.ships.prev_pos[0] = world.ships.pos[0];
        world.step(&mut Vec::new());
        no_refuel(&mut world, "undocked");
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_policy` → expected failure:** `credit_identity_holds_across_refuels_and_policy_is_self_running` fails at the non-vacuity assert — `policy-driven top-to-full refuel happened (4 lots, dry -> full)` (no stage writes the intent yet; the gates test passes vacuously and is armed by the implementation).
- [ ] **Step 3: implement** in `economy.rs`, after `run_purchase_policies` (~:1103):

```rust
/// Scripted refuel-intent stage — stage 1c3b, world-gets-big §5 (after
/// `run_purchase_policies`, before `resolve_purchases`). Writes the transient
/// `pending_refuel` intent for every scripted NON-PIRATE craft (pirates keep
/// the per-class endurance spec this rung — spec §6/OD-6) that is docked at
/// ANY station (`body_pos` at `prev == t-1`, the try_load frame), has tank
/// headroom for at least one lot, and holds a wallet covering ONE unit at the
/// dock's live Fuel price. Top-to-full, threshold-free: no taste scalar, no
/// target level — the 1d2 settle buys `min(need, stock, afford)` lots.
///
/// Inert by default: `lot_mass == 0.0` early-returns (the trophic-inertness
/// gate). Scripted stage: skips `!scripted` craft; never clobbers an intent
/// already written by this tick's ingest (the run_purchase_policies
/// discipline).
#[allow(clippy::too_many_arguments)]
pub fn run_refuel_policies(
    ships: &mut CraftStore,
    craft_cfg: &[crate::config::CraftInit],
    stations: &StationStore,
    bodies: &BodyStore,
    eph: &Ephemeris,
    refuel: &crate::config::RefuelCfg,
    tick: Tick,
) {
    if refuel.lot_mass <= 0.0 {
        return; // the trophic-inertness gate (world-gets-big §5)
    }
    let prev = Tick(tick.0.saturating_sub(1));
    let fuel_r = Resource::Fuel.index();
    for crow in 0..ships.ids.len() {
        // Scripted stages skip gym-controlled craft (spec §5).
        if craft_cfg.get(crow).is_some_and(|c| !c.scripted) {
            continue;
        }
        // Never clobber an intent already written by this tick's ingest.
        if ships.pending_refuel[crow].is_some() {
            continue;
        }
        if ships.role[crow] == CraftRole::Pirate {
            continue; // per-class endurance spec this rung (OD-6)
        }
        let Some(srow) = docked_station_row(ships, crow, stations, bodies, eph, prev) else {
            continue;
        };
        let eff = effective_params(&ships.spec[crow], &ships.mods[crow]);
        // The SAME quantization expression as the 1d2 settle, so policy intent
        // and same-tick settle agree on "headroom >= 1 lot".
        let need = ((eff.fuel_capacity - ships.fuel_mass[crow]) / refuel.lot_mass).floor();
        if need < 1.0 {
            continue;
        }
        // Wallet covers ONE unit at the dock's live price (the settle re-gates
        // everything, including unit_price < 1).
        if ships.credits_micros[crow] < stations.price_micros[srow][fuel_r] {
            continue;
        }
        ships.pending_refuel[crow] = Some(());
    }
}
```

- [ ] **Step 4: wire stage 1c3b in `World::step`**, after the `run_purchase_policies` call (world.rs:766), before `resolve_purchases` (:776):

```rust
        // (1c3b) scripted refuel policies (world-gets-big §5): write the
        //        `pending_refuel` INTENT for docked, scripted, non-pirate craft
        //        with >= 1 lot of headroom and a wallet covering one unit at
        //        the dock's live price; consumed by stage 1d2 below the SAME
        //        tick — the column stays None at every hash point. Top-to-full,
        //        threshold-free. Inert at lot_mass == 0.0 (the trophic-
        //        inertness gate). PRE-physics: body_pos sampled at
        //        `next - 1 == cur` (the try_load frame).
        crate::economy::run_refuel_policies(
            &mut self.ships,
            &self.config.craft,
            &self.stations,
            &self.bodies,
            &self.eph,
            &self.config.refuel,
            next,
        );
```

- [ ] **Step 5: run + expected pass.** `cargo test -p jumpgate-core refuel` all green (including the Task-1.2.3 tests — they step exactly once with the intent pre-written, so the policy's no-clobber guard keeps their outcomes bit-identical). `cargo test -p jumpgate-core` green; `cargo clippy --all-targets -- -D warnings` clean.

  Note (untestable-by-construction, documented in code instead): the no-clobber guard cannot be black-box-distinguished — the intent payload is the unit type, so an overwrite of `Some(())` with `Some(())` is unobservable. The guard is kept because it is the `run_purchase_policies` discipline and protects any future payload.
- [ ] **Step 6: commit.**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/world.rs
git commit -F - <<'EOF'
feat(world-gets-big): run_refuel_policies at stage 1c3b — scripted top-to-full refuel intent

Docked-at-ANY-station (t-1 frame), headroom >= 1 lot (the settle's exact
quantization expression), wallet covers one unit. Non-pirate by
construction (OD-6); skips !scripted; never clobbers ingest intent;
inert at lot_mass == 0. Credit identity pinned across self-running
refuels.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.5: FUEL-C1 — `resolve_refuels` re-derives `dv_remaining` for refueled Seeking craft

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (inside `resolve_refuels`, after the tank-leg write, before the `Refueled` emit; test `refuel_rederives_dv_for_seeking_craft_fuel_c1`)

- [ ] **Step 1: failing test first** (in `economy.rs` tests):

```rust
    #[test]
    fn refuel_rederives_dv_for_seeking_craft_fuel_c1() {
        use crate::types::NavDest;
        use crate::world::World;
        let (mut world, _h) = World::reset(refuel_world_fixture()).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        // The escrow-lock trap (autopilot coast-at-zero, autopilot.rs:61):
        // Seeking with an exhausted budget and a dry tank. Without FUEL-C1 the
        // craft coasts FOREVER after the refill — nothing re-derives
        // dv_remaining after dispatch; it is only ever decremented.
        world.ships.nav[0] = NavState::Seeking {
            dest: NavDest::Position(crate::math::Vec3::new(0.5, 0.0, 0.0)),
            dv_remaining: 0.0,
        };
        world.step(&mut Vec::new());

        // The 1d2 refill: 4 lots, dry -> full — the same clamped-write expression.
        let refilled: f64 = (0.0 + 4.0 * 2.5e-10_f64).min(1.0e-9);
        // 1d2 is PRE-physics: the re-derived budget unlocked a SAME-TICK burn
        // drawn from the refilled tank (prev_fuel untouched until stage 4).
        let dv_applied: f64 = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .find_map(|e| match e.kind {
                EventKind::ThrustApplied { dv, .. } => Some(dv),
                _ => None,
            })
            .expect("the re-derived budget unlocked a same-tick burn");
        assert!(
            world.ships.fuel_mass[0] < refilled,
            "the same-tick burn drew from the REFILLED tank"
        );
        // Exact pin: dv_remaining == tsiolkovsky(refilled tank) − this tick's
        // burn — the SAME derivation both dispatch sites use (try_load /
        // ingest dv_from_fuel), then the step loop's single subtraction.
        let dv_full = crate::math::tsiolkovsky_dv(1e-2, 1e-9, refilled);
        match world.ships.nav[0] {
            NavState::Seeking { dv_remaining, .. } => {
                assert_eq!(
                    dv_remaining,
                    dv_full - dv_applied,
                    "budget re-derived from the refilled tank, then burned once"
                );
                assert!(dv_remaining > 0.0, "no more coast-at-zero with a full tank");
            }
            other => panic!("expected Seeking, got {other:?}"),
        }
    }
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_rederives` → expected failure:** panics at `expect("the re-derived budget unlocked a same-tick burn")` — with `dv_remaining <= 0` the autopilot returns `(Vec3::ZERO, 0.0)` (autopilot.rs:61), so no `ThrustApplied` ever fires.
- [ ] **Step 3: implement.** In `resolve_refuels`, immediately after the tank-leg write (`ships.fuel_mass[crow] = …`) and the `tank_after_permille` line, BEFORE the `Refueled` emit:

```rust
        // FUEL-C1 (world-gets-big §5): re-derive the Δv budget for a refueled
        // craft that is currently Seeking — a pure function of hashed state
        // (effective spec + the just-written fuel_mass), the SAME tsiolkovsky
        // derivation both dispatch sites already use (economy::try_load,
        // ingest's dv_from_fuel). Closes the same-tick dispatch-then-refuel
        // race: the autopilot treats `dv_remaining <= 0` as a permanent coast
        // even with a full tank, locking the contract's escrow forever.
        if let NavState::Seeking { dest, .. } = ships.nav[crow] {
            let dv = crate::math::tsiolkovsky_dv(
                eff.exhaust_velocity,
                eff.dry_mass,
                ships.fuel_mass[crow],
            );
            ships.nav[crow] = NavState::Seeking { dest, dv_remaining: dv };
        }
```

  (This runs only on the settle path — `units >= 1` — so a skipped refuel never touches `nav`. Idle / DirectThrust craft are untouched by the `if let`.)
- [ ] **Step 4: run + expected pass.** `cargo test -p jumpgate-core refuel` all green (the earlier exact-settle tests use Idle craft — nav untouched, payloads unchanged). `cargo test -p jumpgate-core` green; zero goldens move (the write is to already-hashed `nav` state on a path no existing fixture reaches).
- [ ] **Step 5: commit.**

```
git add crates/jumpgate-core/src/economy.rs
git commit -F - <<'EOF'
feat(world-gets-big): FUEL-C1 — resolve_refuels re-derives dv_remaining for Seeking craft

A refueled craft mid-Seek gets its Δv budget re-derived from the refilled
tank (the shared tsiolkovsky derivation of both dispatch sites), closing
the dispatch-then-refuel coast-at-zero escrow lock (autopilot.rs:61).
Settle-path only; Idle/DirectThrust nav untouched.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.6: PLAY-C1 dispatch fuel-eligibility + `Refuel` ingest verb + `ContractFailed` narration (FuelEmpty-cause ONLY)

**Depends on Task 1.1 (eps 1e-9 → 1e-11, the fuel-edge section's commit):** the ASSIGN filter compares `fuel_mass > FUEL_EMPTY_EPS`; every band tank is exactly the OLD eps (scenario.rs:113/126), so with eps still 1e-9 the filter would blacklist every full-tank trophic hauler and break the phase-exit digest. Do not start this task until 1.1 is merged.

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (ASSIGN per-craft gates ~:533-545; `FailureCause` derives ~:1237; `settle_contract_failure` ~:1262-1309 signature + capture + emit; `resolve_failures` ~:1194-1231 signature; tests)
- Modify: `crates/jumpgate-core/src/contract.rs` (`EventKind::ContractFailed` appended after `Refueled`)
- Modify: `crates/jumpgate-core/src/types.rs` (`CommandKind::Refuel` after `BuyUpgrade` ~:76)
- Modify: `crates/jumpgate-core/src/ingest.rs` (`ingest_commands` arm ~:193-204; test) — the legacy `ingest_into` path's catch-all `_ =>` arm (:138-141) covers the new variant with NO edit (economy kinds fall through, logged + event only)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 3c `resolve_failures` call :1012-1018 gains `next, &mut self.events`)
- Modify: `crates/jumpgate-core/src/pirate.rs` (the Robbed `settle_contract_failure` call ~:236 gains `tick, events`)

- [ ] **Step 1: failing tests first.**

  (i) The ASSIGN filter (economy.rs tests, the `scripted_assign_filters_oversized_contracts` / `capacity_world_fixture` pattern):

```rust
    #[test]
    fn scripted_assign_filters_dry_tank_craft_play_c1() {
        use crate::world::World;
        // The capacity-fixture board with scripted ASSIGN ON (stagger 1) and a
        // claimable lot (qty 5 <= base capacity): the only hauler's TANK is the
        // sole eligibility variable under test.
        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        cfg.craft[0].fuel_mass = 0.0; // DRY tank (<= FUEL_EMPTY_EPS)
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        let mut empty: Vec<Command> = Vec::new();
        for _ in 0..8 {
            world.step(&mut empty);
        }
        // World-truth feasibility, never claim-and-strand: the board keeps the
        // offer; the craft is the ADRIFT end state (role Idle ∧ dry tank).
        assert_eq!(world.contracts.status[0], ContractStatus::Offered, "dry tank: never claimed");
        assert_eq!(world.ships.role[0], CraftRole::Idle, "stays Idle forever");
        assert_eq!(world.ships.contract[0], None, "no binding written");

        // Control arm: the stock tank (1e-9 > the re-baked eps 1e-11, Task 1.1)
        // claims it — the filter, not the fixture, was the gate.
        let mut cfg = capacity_world_fixture();
        cfg.dispatch_cfg.stagger_period = 1;
        cfg.contracts[0].qty = 5;
        let (mut world, _h) = World::reset(cfg).expect("resolvable cfg");
        for _ in 0..8 {
            world.step(&mut empty);
        }
        assert_ne!(world.contracts.status[0], ContractStatus::Offered, "live tank: claimed");
    }
```

  (ii) The ingest verb (ingest.rs tests, the `buy_upgrade_writes_pending_intent…` clone at :440-469):

```rust
    #[test]
    fn refuel_writes_pending_intent_logs_and_emits_action_ingested() {
        // The Refuel ingest arm is INTENT-ONLY (the BuyUpgrade template): it
        // writes the transient `pending_refuel` column and nothing else — the
        // settle (dock check, quantization, four legs) is deferred to
        // `resolve_refuels` (stage 1d2), which consumes the intent the same
        // tick. Top-to-full: the verb carries no quantity.
        let (mut world, _h) = World::reset(one_body_one_craft_cfg()).expect("resolvable cfg");
        let id0 = world.ships.ids_at(0);

        let mut cmds = vec![Command {
            target: Target::Entity(EntityRef::Craft(id0)),
            kind: CommandKind::Refuel,
        }];
        ingest_commands(&mut world, Tick(2), &mut cmds);

        assert_eq!(world.ships.pending_refuel[0], Some(()), "intent column set");
        assert_eq!(world.ships.fuel_mass[0], world.ships.prev_fuel[0], "no tank movement at ingest");
        assert_eq!(world.ships.credits_micros[0], 0, "no credit movement at ingest");
        assert_eq!(world.log_mut().at(Tick(2)).len(), 1, "command logged at tick");
        let emitted = world.events_mut().since(Tick(0));
        assert_eq!(emitted.len(), 1);
        assert!(matches!(
            emitted[0].kind,
            EventKind::ActionIngested { target } if target == Target::Entity(EntityRef::Craft(id0))
        ));
    }
```

  (iii) The narration (economy.rs tests; `starved_two_body_contract_fixture` + the documented mid-flight-drain field-write pattern, economy.rs:2526-2533; add `CorporationId` to the test-mod imports):

```rust
    #[test]
    fn fuel_empty_failure_emits_contract_failed_with_actual_refund() {
        use crate::world::World;
        // Deadhead arm (Accepted): origin is the far station, so the hauler
        // launches empty-handed; we force the depletion edge with a mid-flight
        // drain (the documented field-write pattern) instead of waiting out
        // the burn.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        assert_eq!(world.contracts.status[0], ContractStatus::Accepted, "deadhead leg armed");
        let escrow_before = world.contracts.escrow_micros[0];
        assert!(escrow_before > 0, "escrow held");

        // Drain mid-deadhead: prev_fuel (stage-4 copy of last tick's tank) is
        // still > eps, so the next step's edge detector fires FuelEmpty.
        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());

        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed {
                    contract,
                    hauler,
                    cause: FailureCause::FuelEmpty,
                    escrow_refunded_micros,
                    cargo_lost: 0,
                } if contract == cid && hauler == craft && escrow_refunded_micros == escrow_before
            )),
            "FuelEmpty failure narrated with the ACTUAL refund (deadhead: no cargo lost)"
        );

        // Stale-corp degrade arm: the refund leg is skipped (escrow stays put,
        // the credit identity holds) and the event reports the ACTUAL 0 refund.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(1, 0)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds);
        world.contracts.corp[0] = CorporationId { slot: 99, generation: 0 }; // stale row
        world.ships.fuel_mass[0] = 0.0;
        world.step(&mut Vec::new());
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(world.contracts.escrow_micros[0] > 0, "escrow stays put on the degrade arm");
        assert!(
            world.events_mut().since(Tick(0)).iter().any(|e| matches!(
                e.kind,
                EventKind::ContractFailed { escrow_refunded_micros: 0, .. }
            )),
            "degrade arm reports the actual 0 refund"
        );
    }

    #[test]
    fn robbed_teardown_is_not_narrated_by_contract_failed() {
        use crate::world::World;
        // Single-emit law (world-gets-big §7): the settle body emits NOTHING
        // for FailureCause::Robbed — the 3b2 caller owns the Robbed narration.
        // Drive to the one-tick CargoLoaded window (the fuel_empty_mid_deadhead
        // arm-2 pattern), then call the settle body directly.
        let (mut world, _h) =
            World::reset(starved_two_body_contract_fixture(0, 1)).expect("resolvable cfg");
        let craft = world.ships.ids_at(0);
        let cid = contract_id(&world.contracts, 0);
        let mut cmds = vec![crate::contract::Command {
            target: crate::types::Target::Entity(crate::types::EntityRef::Craft(craft)),
            kind: crate::types::CommandKind::AcceptContract { contract: cid },
        }];
        world.step(&mut cmds); // docked at origin: accept + load settle this tick
        assert_eq!(world.contracts.status[0], ContractStatus::CargoLoaded, "the load-tick window");

        let mut ev = crate::events::EventStream::default();
        settle_contract_failure(
            &mut world.contracts,
            &mut world.corporations,
            &mut world.ships,
            &mut world.econ,
            0,
            FailureCause::Robbed,
            Tick(99),
            &mut ev,
        );
        assert_eq!(world.contracts.status[0], ContractStatus::Failed);
        assert!(
            !ev.since(Tick(0)).iter().any(|e| matches!(e.kind, EventKind::ContractFailed { .. })),
            "Robbed teardown emits NO ContractFailed (Robbed narrates itself at 3b2)"
        );
    }
```

- [ ] **Step 2: run → expected failures.** `cargo test -p jumpgate-core scripted_assign_filters_dry` → control arm passes but the dry arm fails: `dry tank: never claimed` (the contract IS claimed — no filter exists). `cargo test -p jumpgate-core refuel_writes_pending` → `E0599: no variant named `Refuel` found for enum `CommandKind``. `cargo test -p jumpgate-core fuel_empty_failure_emits` → `E0599: no variant named `ContractFailed``.
- [ ] **Step 3: implement the ASSIGN filter.** In `run_scripted_dispatch`'s per-hauler gate block (economy.rs:533-545), directly after the stagger gate, before the `capacity` derivation:

```rust
        // PLAY-C1 (world-gets-big §5): dispatch eligibility requires a live
        // tank — world-truth feasibility filter-at-choice (the capacity-filter
        // precedent at the per-contract loop below), never claim-and-strand.
        // A stranded craft stays Idle forever: the ADRIFT end state is
        // role Idle ∧ fuel <= eps, matched by detection, not by shaping.
        if ships.fuel_mass[crow] <= crate::events::FUEL_EMPTY_EPS {
            continue;
        }
```

- [ ] **Step 4: implement the verb.** `types.rs`, after `BuyUpgrade` (:76):

```rust
    /// Intent to top up propellant at the docked station (world-gets-big §5):
    /// ingestion writes the transient `pending_refuel` column only; the settle
    /// (dock check, integer quantization, four legs, Δv re-derivation) lives in
    /// `resolve_refuels` (stage 1d2), which consumes the intent the same tick.
    /// Top-to-full, threshold-free: the verb carries no quantity.
    Refuel,
```

  `ingest.rs`, after the `BuyUpgrade` arm (:204):

```rust
                CommandKind::Refuel => {
                    // Record INTENT only (the BuyUpgrade template): write the
                    // transient `pending_refuel` column. The settle is DEFERRED
                    // to `resolve_refuels` (stage 1d2), which consumes the
                    // intent the SAME tick (on a lot-0 world the stage consumes
                    // it as a deterministic no-op). A stale craft id is a
                    // deterministic skip; the command is still logged above and
                    // ActionIngested still fires below (the seam).
                    if let Some(i) = world.ships.index_of(id) {
                        world.ships.pending_refuel[i] = Some(());
                    }
                }
```

  (`ingest_into`'s catch-all `_ =>` arm at :138-141 already covers the variant — no edit, matching the BuyUpgrade precedent.)
- [ ] **Step 5: implement the narration.** `contract.rs`, append after `Refueled`:

```rust
    /// A contract failed on propellant exhaustion (world-gets-big §7) —
    /// emitted in `settle_contract_failure` for `FailureCause::FuelEmpty`
    /// ONLY (the robbery path keeps its own `Robbed` narration at the 3b2
    /// emission site; single emit path preserved). `escrow_refunded_micros`
    /// is the ACTUAL refund — the stale-corp degrade arm reports 0;
    /// `cargo_lost` the qty accounted into `consumed` (0 on a deadhead leg).
    /// Today the failure path is silent; the tragedy becomes visible.
    ContractFailed {
        contract: ContractId,
        hauler: CraftId,
        cause: crate::economy::FailureCause,
        escrow_refunded_micros: i64,
        cargo_lost: u32,
    },
```

  `economy.rs`: give `FailureCause` the event-payload derives (it currently has none):

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FailureCause {
```

  Grow `settle_contract_failure` (economy.rs:1262): signature gains `tick: Tick, events: &mut EventStream` after `cause`; body changes — capture the ACTUAL refund before the leg, the lost qty at the sink leg, and emit cause-gated at the end (after `contracts.status[kidx] = ContractStatus::Failed;`):

```rust
    // Refund escrow -> owning corp treasury (credit TRANSFER; identity
    // invariant). Captured BEFORE the leg: a stale corp row skips the refund
    // (the escrow stays put so the identity holds) and the narration reports
    // the ACTUAL 0 — what happened, not what was owed.
    let corp = contracts.corp[kidx];
    let mut escrow_refunded_micros: i64 = 0;
    if let Some(corp_row) = corporations.ids.dense_index(corp.slot, corp.generation) {
        escrow_refunded_micros = contracts.escrow_micros[kidx];
        corporations.treasury_micros[corp_row] =
            corporations.treasury_micros[corp_row].saturating_add(contracts.escrow_micros[kidx]);
        contracts.escrow_micros[kidx] = 0;
    }
    // Cargo loss: account the lost cargo as a SINK leg, then release the hauler.
    let mut cargo_lost: u32 = 0;
    if let Some(hauler) = contracts.hauler[kidx]
        && let Some(crow) = ships.index_of(hauler)
    {
        if let Some((resource, qty)) = ships.cargo[crow] {
            counters.consumed[resource.index()] =
                counters.consumed[resource.index()].saturating_add(qty as i64);
            ships.cargo[crow] = None;
            cargo_lost = qty;
        }
        ships.contract[crow] = None;
        ships.role[crow] = CraftRole::Idle;
    }
    contracts.status[kidx] = ContractStatus::Failed;
    // Single-emit narration (world-gets-big §7): FuelEmpty-cause ONLY — the
    // robbery path emits `Robbed` at its own 3b2 site; emitting here too would
    // double-narrate one teardown. `contracts.hauler` survives the release
    // above (only the SHIP's columns were cleared), so the payload binds.
    if matches!(cause, FailureCause::FuelEmpty)
        && let Some(hauler) = contracts.hauler[kidx]
    {
        events.emit(Event {
            tick,
            kind: EventKind::ContractFailed {
                contract: contract_id(contracts, kidx),
                hauler,
                cause,
                escrow_refunded_micros,
                cargo_lost,
            },
        });
    }
```

  Grow `resolve_failures` (economy.rs:1194): signature gains `tick: Tick, events: &mut EventStream`; the settle call passes them through; DELETE the now-stale "No dedicated failure event" comment (economy.rs:1219-1221) and replace with `// ContractFailed (FuelEmpty-cause) is emitted inside the settle body (§7).`. Update the two callers: `world.rs` stage 3c (:1012-1018) appends `next, &mut self.events` (legal — the FuelEmpty ids were already lifted into `failed_craft`, the immutable borrow is dropped); `pirate.rs` ~:236 appends `tick, events` (both already in scope at the Robbed settle site).
- [ ] **Step 5b: the eps-ordering liveness guard (system-level, fails LOUD on misordering).** The prose dependency on Task 1.1 is not enough: every trophic hauler spawns `fuel_mass: 1.0e-9` exactly, so if `FUEL_EMPTY_EPS` were still `1e-9` (Task 1.1 unlanded or reverted), the strict `fuel_mass > FUEL_EMPTY_EPS` filter is false for EVERY band hauler — zero dispatches, a silently dead trophic world, green unit suite, and the divergence would surface only at the phase-exit digest where the symptom does not name the cause. This test names it. Add next to the filter test in `economy.rs`:

```rust
    #[test]
    fn trophic_world_still_dispatches_under_fuel_eligibility() {
        // ORDERING GUARD (world-gets-big C3): trophic haulers spawn at
        // fuel_mass == 1.0e-9. The dispatch filter `fuel_mass > FUEL_EMPTY_EPS`
        // must not bind on a full band tank — i.e. the eps re-bake (1e-11,
        // Task 1.1) must already be in. If this assert fires, the filter
        // landed before the re-bake: revert or land 1.1 first.
        use crate::world::World;
        let cfg = crate::scenario::scenario_trophic(7);
        let (mut world, _h) = World::reset(cfg).expect("trophic resolves");
        let mut cmds = Vec::new();
        for _ in 0..3_000u64 {
            world.step(&mut cmds);
        }
        let accepts = world
            .events_mut()
            .since(Tick(0))
            .iter()
            .filter(|e| matches!(e.kind, EventKind::ContractAccepted { .. }))
            .count();
        assert!(
            accepts > 0,
            "trophic world dispatched ZERO contracts — the fuel-eligibility \
             filter is binding on full-tank band haulers (eps re-bake landed?)"
        );
    }
```

  (Adjust the event-read idiom to the file's existing whole-world tests if `since(Tick(0))` is not the local pattern — the assertion, `accepts > 0` over a 3,000-tick `scenario_trophic(7)` run, is the contract.) Run it: green NOW (Task 1.1 landed eps 1e-11 earlier in this phase); it is the permanent tripwire for any future eps/filter reordering, rebase, or revert.

- [ ] **Step 6: run + expected pass.** `cargo test -p jumpgate-core scripted_assign_filters_dry refuel_writes_pending fuel_empty_failure robbed_teardown trophic_world_still_dispatches_under_fuel_eligibility` all green. `cargo test --workspace` green — in particular `starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss` (world.rs:2208) and `fuel_empty_mid_deadhead_refunds_escrow` (economy.rs:2465+) still pass: the legs are unchanged, only captured-and-narrated. `cargo clippy --all-targets -- -D warnings` clean. Zero goldens move (events are unhashed; the ASSIGN filter never binds on any existing fixture whose dispatching craft has fuel > eps — verify with the full suite, not by assumption).
- [ ] **Step 7: commit.**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/contract.rs \
  crates/jumpgate-core/src/types.rs crates/jumpgate-core/src/ingest.rs \
  crates/jumpgate-core/src/world.rs crates/jumpgate-core/src/pirate.rs
git commit -F - <<'EOF'
feat(world-gets-big): PLAY-C1 dispatch fuel-eligibility + Refuel verb + ContractFailed narration

ASSIGN requires fuel_mass > FUEL_EMPTY_EPS (filter-at-choice, the
capacity precedent) — a stranded craft stays Idle, on the record.
CommandKind::Refuel = intent-only ingest (BuyUpgrade template).
settle_contract_failure grows (tick, events) and emits ContractFailed
for FailureCause::FuelEmpty ONLY, reporting the ACTUAL refund (stale-corp
arm: 0) and lost qty; Robbed keeps its own narration. Depends on the
eps re-bake (1e-11): band tanks sit at the OLD eps exactly (the
trophic_world_still_dispatches_under_fuel_eligibility tripwire pins it).

spec §9 bundle: PLAY-C1 fuel filter + Refuel verb + ContractFailed
narration — three concerns co-dependent at the lot_mass==0 gate.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.6b: chronicle arms for `Refueled` / `ContractFailed` — the tragedy becomes visible

Spec §7 requires both events printer-side ("the tragedy becomes visible").
`chronicle_subject` in `trophic_run.rs` ends in a `_ => None` catch-all
(:262 at HEAD), so without explicit arms both new variants compile, emit into
the event stream, and silently vanish from `--chronicle` — with no test to
catch it. The arms land HERE, in the same phase as the events, so there is no
silent window.

**Files:**
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (`chronicle_subject`, ~:240-262)

- [ ] **Step 1: add the arms.** In `chronicle_subject`, above the `_ => None` catch-all, threading each event into the affected craft's life arc (the existing per-variant idiom — fields bind by value, no deref):

```rust
        // World-gets-big §7: the refuel and the failure thread into the
        // craft's life arc — a stranded run must read end-to-end.
        EventKind::Refueled { craft, .. } => Some(craft),
        EventKind::ContractFailed { hauler, .. } => Some(hauler),
```

- [ ] **Step 2: build + lint green.** `cargo build --example trophic_run -p jumpgate-core && cargo clippy --all-targets -- -D warnings`. (No runner-visible output exists yet on trophic: RefuelCfg is lot-0 there and band FuelEmpty is unfireable post-re-bake — by design. The runner-level visibility check is pinned in the phase-2 section verification: the first frontier smoke run greps the chronicle for refuel lines.)
- [ ] **Step 3: commit.**

```
git add crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(chronicle): Refueled/ContractFailed thread into the craft life arc

chronicle_subject arms for the two world-gets-big §7 events (the _=>None
catch-all otherwise swallows them silently). Printer-side only; no hashed
state, no goldens, trophic output unchanged (lot-0 gate, no FuelEmpty).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 1.2.7: the trophic-inertness gate proven — lot-0 unit pin + the cross-branch 2000-tick digest (the phase-1 exit)

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (test `refuel_default_is_inert_and_consumes_stray_intents`)
- Modify: `crates/jumpgate-core/src/scenario.rs` (one assert added to `scenario_trophic_shape` ~:337-466)
- No new tracked files. Digest artifacts live under `runs/wgb_phase1_inert/` — **never staged** (runs/ is never committed).

- [ ] **Step 1: failing test first** (economy.rs tests):

```rust
    #[test]
    fn refuel_default_is_inert_and_consumes_stray_intents() {
        use crate::world::World;
        // RefuelCfg::default() (lot_mass == 0.0): BOTH stages no-op — the named
        // trophic-inertness gate. A manual intent on a lot-0 world is consumed
        // (the all-None hash invariant) but settles NOTHING.
        let (mut world, _h) = World::reset(vendor_world_fixture(false)).expect("resolvable cfg");
        world.ships.credits_micros[0] = 1_000_000;
        world.ships.pending_refuel[0] = Some(());
        world.step(&mut Vec::new());
        assert_eq!(world.ships.pending_refuel[0], None, "stray intent consumed on the lot-0 world");
        assert_eq!(world.ships.fuel_mass[0], 1e-9, "tank untouched");
        assert_eq!(world.ships.credits_micros[0], 1_000_000, "wallet untouched");
        assert_eq!(world.corporations.treasury_micros[0], 0, "no treasury movement");
        assert!(
            !world
                .events_mut()
                .since(Tick(0))
                .iter()
                .any(|e| matches!(e.kind, EventKind::Refueled { .. })),
            "no Refueled event on a lot-0 world"
        );
        let _ = crate::hash::state_hash(&world); // the all-None debug_assert holds
    }
```

  And in `scenario_trophic_shape` (scenario.rs, alongside the dispatch/policy value asserts ~:445-453):

```rust
        assert_eq!(
            cfg.refuel.lot_mass, 0.0,
            "the trophic-inertness gate: the refuel verb stays OFF on the band"
        );
```

- [ ] **Step 2: run `cargo test -p jumpgate-core refuel_default_is_inert scenario_trophic_shape` → expected pass immediately** (both behaviors were built in Tasks 1.2.3/1.2.4; this step PINS them — if either fails, STOP: the gate is broken, fix before the digest). This is the one task whose tests pin rather than drive; the failing-first evidence for the gate is the digest in Step 3, which fails loudly on any divergence.
- [ ] **Step 3: the cross-branch 2000-tick digest (the digest-tests-are-determinism-not-golden law; the media-rung Task 9.2 procedure).** Baseline build = the last commit BEFORE phase 1's first commit (i.e. before Task 1.1's eps change — by then phase 0a/0b instrumentation is in BOTH builds, so stdout/JSONL line sets match). Record that commit hash when phase 1 starts; here `<PRE>` stands for it.

```
git worktree add /tmp/wgb-pre-phase1 <PRE>
mkdir -p runs/wgb_phase1_inert
( cd /tmp/wgb-pre-phase1 && \
  for S in 7 23; do \
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --seed $S --ticks 2000 \
      --jsonl /home/john/jumpgate/runs/wgb_phase1_inert/base-s$S.jsonl \
      > /home/john/jumpgate/runs/wgb_phase1_inert/base-s$S.out; \
  done )
for S in 7 23; do \
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --seed $S --ticks 2000 \
    --jsonl runs/wgb_phase1_inert/head-s$S.jsonl \
    > runs/wgb_phase1_inert/head-s$S.out; \
done
for S in 7 23; do \
  diff runs/wgb_phase1_inert/base-s$S.jsonl runs/wgb_phase1_inert/head-s$S.jsonl && \
  diff runs/wgb_phase1_inert/base-s$S.out  runs/wgb_phase1_inert/head-s$S.out; \
done
git worktree remove /tmp/wgb-pre-phase1
```

  **Expected: every diff exits 0 with no output — byte-identical.** The whole phase (eps re-bake + RefuelCfg + both stages + dv-rederive + dispatch filter + verbs + events) must be bit-inert on the band: eps appears in no physics expression, band fuel never approaches 1e-11, the dispatch filter never binds above eps, lot 0 gates both stages, and events that never fire print nothing. **Any divergence is a determinism break: STOP and bisect commit-by-commit; never rationalize a diff.** This digest green is the phase-1 exit (spec §9; W12's trophic arm).
- [ ] **Step 4: replay determinism at HEAD.**

```
cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks 2000 --replay-check
```

  → expected: `replay-check OK` (exercises the pending_refuel all-None assert at every recorded hash point, in a debug-assertions-off release build AND under the recorded-run hashing).
- [ ] **Step 5: full phase gates.** `cargo test --workspace` green (record the count); `cargo clippy --all-targets -- -D warnings` clean; `PYTHONPATH=/home/john/jumpgate/python pytest python/tests` green (the gym crate gained only a defaulted config field). Grep-verify the golden inventory: exactly ONE changed `GOLDEN_CONFIG_HASH` (Task 1.2.1), `HASH_FORMAT_VERSION` still `5`, `GOLDEN_ZERO_STATE_HASH` and the hash.rs:1108 trajectory golden byte-identical to `<PRE>`:

```
git diff <PRE> -- crates/jumpgate-core/src/hash.rs | grep -E "GOLDEN|FORMAT_VERSION"   # expect: no hits
git diff <PRE> -- crates/jumpgate-core/src/config.rs | grep GOLDEN_CONFIG_HASH          # expect: exactly the one re-pin
```

- [ ] **Step 6: commit (tests only — the digest artifacts stay untracked).**

```
git add crates/jumpgate-core/src/economy.rs crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
test(world-gets-big): pin the trophic-inertness gate — lot-0 no-op + band factory OFF

RefuelCfg::default() consumes stray intents and settles nothing (hash
invariant exercised); scenario_trophic pins lot_mass == 0.0. Cross-branch
2000-tick digest vs pre-phase-1 (seeds 7/23, stdout+JSONL) byte-identical
and replay-check OK — the phase-1 exit (spec §9, W12 trophic arm);
digest artifacts in runs/ (untracked, never staged).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

## Cross-section handoffs (named, not built here)

- **Chronicle arms** for `Refueled`/`ContractFailed` are **built in this phase — Task 1.2.6b** (the `_ => None` catch-all at trophic_run.rs:262 would otherwise silently swallow them). Only the per-craft ADRIFT epilogue remains with the lab/chronicle section (Task 2.7, spec §7 printer-side).
- **FUEL-line refuel fields** (`refuels, refuel_spend_micros, strandings, adrift_end`) and **TrophicSample** additive fields (`per_station_fuel_stock`, `per_station_fuel_price`, `refuels`, `refuel_units`, `refuel_spend_micros`) — spec §8 says they "append with the mechanic"; they ride the lab section's anchored-line + version-gated-regex work (TROPHIC-C2/LAB-C2), not this one.
- **W9 liveness window** (max non-terminal contract age per run) — built in Task 2.7 Step 6b (runner-side `LIVENESS` line from event-stream bookkeeping; `ContractStore` carries no accept tick).
- **Port corp creation** (`CorporationInit { treasury_micros: 0, .. }` + `refuel.corp_index` pointed at it) — the `scenario_frontier` factory task, phase 2; the binding and its stale-row degrade are built and tested here.


---

## Phase 2 (first half) — the frontier factory (spec §2, §3, §5 Pricing, §6 reach, §9 phase-2 first half)

> Plan section for the world-gets-big rung (spec
> `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` §2, §3, §5
> Pricing, §6 reach bullet, §9 phase-2 first half). Grounded at HEAD
> `e7e490e`; line refs are to that commit — phases 0/1 land first and may
> shift them (re-grep before editing, symbols are authoritative).
>
> **Preconditions from phase 1** (this section consumes, never re-implements):
> `RefuelCfg { lot_mass, corp_index }` exists as the tail field
> `RunConfig.refuel` (the MediaCfg fold precedent), `lot_mass == 0.0` is the
> named inertness gate, and the reset half-on guard ("`lot_mass > 0` while
> `price_cfg.base_micros[Fuel] == 0` or any seeded
> `initial_price_micros[Fuel] == 0`" → reset error) is live. The ONE
> GOLDEN_CONFIG_HASH re-pin of this rung happened there. **Nothing in this
> section touches any golden, any reward surface, or HASH_FORMAT_VERSION** —
> a new scenario factory moves no existing hash (the frontier trajectory
> golden + LurkMoved + TrophicSample fields are the phase-2 second half,
> not here).
>
> All pre-registered numbers below (gaps, prices, lot counts) live in
> **tests and doc comments as recorded design law** — no plan step makes a
> run-metric a pass/fail gate (determinism/unit tests excepted, per the
> windows-not-gates rule).

---

### Task 2.1: `FRONTIER_ORBIT_AU` — the pinned geometric band law

Files
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/scenario.rs` (const next to `STATION_ORBIT_AU` at scenario.rs:37; test in the existing `#[cfg(test)] mod tests` at scenario.rs:331)

- [ ] **Step 1: Write the failing pinned-law test.** Append to `mod tests` in `crates/jumpgate-core/src/scenario.rs` (after `apply_knob_overrides_and_rejects_unknown`, scenario.rs:489-514):

```rust
    #[test]
    fn frontier_orbit_band_is_the_pinned_geometric_law() {
        // Spec §2: a_k = 0.35·r^k, r = (3.0/0.35)^(1/9) — endpoints EXACT,
        // interior pinned to the recomputed law (never to rounded prose).
        let r = (3.0f64 / 0.35).powf(1.0 / 9.0);
        assert_eq!(FRONTIER_ORBIT_AU.len(), 10);
        assert_eq!(FRONTIER_ORBIT_AU[0], 0.35, "inner endpoint exact");
        assert_eq!(FRONTIER_ORBIT_AU[9], 3.0, "outer endpoint exact");
        for (k, &a) in FRONTIER_ORBIT_AU.iter().enumerate() {
            let law = 0.35 * r.powi(k as i32);
            assert!(
                (a - law).abs() <= 1.0e-12,
                "a_{k} = {a} deviates from the geometric law {law}"
            );
        }
        for w in FRONTIER_ORBIT_AU.windows(2) {
            assert!(w[0] < w[1], "ascending band: {FRONTIER_ORBIT_AU:?}");
        }
        // The designed seam (spec §2/§6): the 8-9 radial gap (0.637) exceeds
        // pirate_max_reach_au 0.6 — the one hop haulers can fly and pirates
        // can never walk. Recorded design law, not a run gate.
        let outer_gap = FRONTIER_ORBIT_AU[9] - FRONTIER_ORBIT_AU[8];
        assert!(
            outer_gap > 0.6,
            "outer gap {outer_gap} must exceed pirate reach 0.6 (never-opens seam)"
        );
    }
```

- [ ] **Step 2: Run and watch it fail to compile.**

```bash
cargo test -p jumpgate-core frontier_orbit
```

Expected failure: `error[E0425]: cannot find value 'FRONTIER_ORBIT_AU' in this scope`.

- [ ] **Step 3: Add the const.** In `crates/jumpgate-core/src/scenario.rs`, directly below `STATION_ORBIT_AU` (scenario.rs:37). Literals are full-precision values of the law (Python-recomputed this session: `0.35 * ((3.0/0.35)**(1/9))**k`); the outer endpoint is the exact literal `3.0` (the f64 product lands one ulp low, so the endpoint is pinned, the law check absorbs the ulp):

```rust
/// Frontier station-body semi-major axes (AU) — the geometric band
/// `a_k = 0.35·r^k`, `r = (3.0/0.35)^(1/9)` (spec §2; endpoints exact, law
/// pinned by `frontier_orbit_band_is_the_pinned_geometric_law`). Body index
/// k+1 hosts station row k. Radial gaps run 0.094 → 0.637 AU; the 8-9 gap
/// (0.637) exceeds `pirate_max_reach_au` 0.6 BY DESIGN — the one hop
/// haulers can fly and pirates can never walk (the never-opens seam).
pub const FRONTIER_ORBIT_AU: [f64; 10] = [
    0.35,
    0.444_365_796_521_264_1,
    0.564_174_174_622_793,
    0.716_284_875_665_669_2,
    0.909_407_140_889_456_3,
    1.154_598_367_209_910_7,
    1.465_897_208_878_237_4,
    1.861_127_373_832_788_5,
    2.362_918_136_859_245,
    3.0,
];
```

- [ ] **Step 4: Run and watch it pass.**

```bash
cargo test -p jumpgate-core frontier_orbit
```

Expected: `test scenario::tests::frontier_orbit_band_is_the_pinned_geometric_law ... ok`. The const is `pub` but unused outside tests until Task 2.3 — if clippy's `-D warnings` flags nothing here (pub items are not dead code), proceed; do NOT add `#[allow(dead_code)]`.

- [ ] **Step 5: Lint and commit.**

```bash
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(frontier): FRONTIER_ORBIT_AU geometric band const + pinned-law test (WGB §2)

a_k = 0.35·r^k, r = (3.0/0.35)^(1/9); endpoints exact, interior pinned to
the recomputed law at 1e-12; the 8-9 gap 0.637 > pirate reach 0.6 is the
designed never-opens seam, asserted as design law.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.2: explicit pirate reach in `scenario_trophic` + the stale marooned-doc fix

Spec §6: "Reach 0.6 set EXPLICITLY in both factories (today inherited silently); the stale 'nearest station' marooned doc fixed in the same commit." The frontier factory sets reach at birth (Task 2.3); this task fixes the existing factory and the doc, in ONE commit. Both edits are behavior-neutral (value identical to the default; doc-only comment), so the TDD red step is replaced by a behavior-preservation digest (the digest-tests-are-determinism discipline).

Files
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/scenario.rs` (TrophicCfg literal at scenario.rs:216-227; assert in `scenario_trophic_shape`)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/pirate.rs` (doc comment at pirate.rs:441-445; the BODY at :466-476 and test at :1655-1663 already implement the uniform breakout — do NOT touch them)

- [ ] **Step 1: Capture the behavior baseline BEFORE editing.**

```bash
cargo run -p jumpgate-core --example trophic_run -- --seed 7 --ticks 2000 --replay-check > /tmp/wgb-2_2-before.txt 2>&1
tail -5 /tmp/wgb-2_2-before.txt
```

Expected: the run completes with its RESULT line and a passing replay check. Keep the file.

- [ ] **Step 2: Make reach explicit in the trophic factory.** In `crates/jumpgate-core/src/scenario.rs`, inside the `TrophicCfg` literal (scenario.rs:216-227), add one field above `..TrophicCfg::default()`:

```rust
        hideout_body_index: 6, // outermost body (1.4 AU)
        pirate_max_reach_au: 0.6, // EXPLICIT (WGB §6) — was a silent ..default()
                                  // inheritance; value unchanged ⇒ hash-neutral
        hauler_belief_scoring: true,
```

(i.e. insert the `pirate_max_reach_au` line between the existing `hideout_body_index` and `hauler_belief_scoring` lines; everything else in the literal stays byte-identical.)

- [ ] **Step 3: Pin the value in the shape test.** In `scenario_trophic_shape` (scenario.rs:337-466), after the `engage_radius_au > 0.0` assert (scenario.rs:452), add:

```rust
        // Reach is EXPLICIT in the factory (WGB §6) — the 0.6 the band was
        // judged at, no longer a silent ..TrophicCfg::default() inheritance.
        assert_eq!(cfg.trophic.pirate_max_reach_au, 0.6);
```

- [ ] **Step 4: Fix the stale marooned doc.** In `crates/jumpgate-core/src/pirate.rs`, replace the doc comment lines (pirate.rs:441-445):

```rust
/// Relocation target draw (spec §5): uniform among stations within
/// `max_reach_au` of `anchor` (the PRIMARY locality lever — 1-2 neighbors,
/// never the whole map); none in reach -> the NEAREST station (ties to the
/// lowest dense row); `None` only when there are no stations at all (spec §8
/// totality).
```

with the doc the body actually implements (the marooned uniform breakout, pirate.rs:466-476):

```rust
/// Relocation target draw (spec §5): uniform among stations within
/// `max_reach_au` of `anchor` (the PRIMARY locality lever — 1-2 neighbors,
/// never the whole map); none in reach -> a MAROONED breakout: ONE committal
/// flight to a uniform draw over ALL huntable stations (the hideout-ghetto
/// lesson — see the body comment below); `None` only when there are no
/// stations at all (spec §8 totality).
```

Leave the `**DUMB BY CONSTRUCTION**` paragraph (pirate.rs:447-451), the function body, and `pirates_are_information_blind` untouched.

- [ ] **Step 5: Prove behavior preservation.**

```bash
cargo test -p jumpgate-core scenario_trophic_shape
cargo run -p jumpgate-core --example trophic_run -- --seed 7 --ticks 2000 --replay-check > /tmp/wgb-2_2-after.txt 2>&1
diff /tmp/wgb-2_2-before.txt /tmp/wgb-2_2-after.txt && echo HASH-NEUTRAL-OK
```

Expected: test ok; `diff` silent; `HASH-NEUTRAL-OK` printed (value-identical config ⇒ identical config_hash ⇒ identical trajectory; no golden moves).

- [ ] **Step 6: Full suite, lint, commit (one commit per spec §6).**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/scenario.rs crates/jumpgate-core/src/pirate.rs
git commit -F - <<'EOF'
fix(trophic): pin pirate reach 0.6 explicitly + correct the stale marooned doc (WGB §6)

pirate_max_reach_au was inherited silently via ..TrophicCfg::default();
value unchanged, verified hash-neutral by a before/after replay-check
digest diff. The relocate_lurk_target doc claimed "nearest station" while
the body (and its test) implement the marooned uniform breakout — doc
brought to truth, zero code change.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.3: `scenario_frontier` — map, populations, per-class specs, partitioned tier loops, dark haven, Port corp

The whole factory lands here with the §3 invariant battery written FIRST. Pricing and the refuel verb stay OFF in this task (dead `PriceCfg` caps, `RefuelCfg::default()`) so Task 2.4 has a clean red; everything structural — including the Port corp row — is final here.

Files
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/scenario.rs` (new consts + `scenario_frontier` after `scenario_trophic` ends at scenario.rs:254; tests in `mod tests`; `use` list at scenario.rs:24-32 gains `RefuelCfg`)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/lib.rs` (export at lib.rs:71)

- [ ] **Step 1: Write the failing test battery — shape, §3 wiring invariants, seed determinism.** Append to `mod tests` in `scenario.rs`:

```rust
    #[test]
    fn scenario_frontier_shape() {
        let cfg = scenario_frontier(7);

        // 1 star + 10 station bodies riding the pinned band in order.
        assert_eq!(cfg.bodies.len(), 11, "star + 10 station bodies");
        assert_eq!(cfg.bodies[0].elements.a, 0.0, "central star");
        let axes: Vec<f64> = cfg.bodies[1..].iter().map(|b| b.elements.a).collect();
        assert_eq!(axes, FRONTIER_ORBIT_AU.to_vec(), "bodies ride FRONTIER_ORBIT_AU");

        // 10 stations; body k+1 hosts station row k (the trophic law, n=10).
        assert_eq!(cfg.stations.len(), 10);
        let body_idx: Vec<usize> = cfg.stations.iter().map(|s| s.body_index).collect();
        assert_eq!(body_idx, (1..=10).collect::<Vec<_>>());

        // Populations (spec §2): 20 haulers (2/station), 10 pirates — a 2:1
        // predator:prey DESIGN CHOICE, all scripted (no gym craft).
        assert_eq!(cfg.craft.len(), FRONTIER_NUM_HAULERS + FRONTIER_NUM_PIRATES);
        let pirates = cfg.craft.iter().filter(|c| c.role == CraftRole::Pirate).count();
        let haulers = cfg.craft.iter().filter(|c| c.role == CraftRole::Idle).count();
        assert_eq!(haulers, 20, "20 haulers");
        assert_eq!(pirates, 10, "10-pirate pool");
        assert!(cfg.craft.iter().all(|c| c.scripted), "all scripted (no gym craft)");
        assert_eq!(haulers % cfg.stations.len(), 0, "haulers ≡ 0 mod n (2/station)");

        // Per-CLASS craft specs (spec §4/§6, OD-6): haulers ride the NAMED
        // calibration-pending const; pirates keep the band's ×10 endurance.
        for c in &cfg.craft {
            match c.role {
                CraftRole::Pirate => assert_eq!(
                    c.spec.base_exhaust_velocity, 20.0,
                    "pirate v_e 20 per-craft (OD-6; cannot strand this rung)"
                ),
                _ => assert_eq!(
                    c.spec.base_exhaust_velocity, FRONTIER_HAULER_EXHAUST_VELOCITY,
                    "hauler v_e = the named analytic prior (calibration bakes it)"
                ),
            }
            assert_eq!(c.spec.base_fuel_capacity, 1.0e-9, "tank = 100× re-baked eps");
            assert_eq!(c.fuel_mass, 1.0e-9, "spawn with a full tank");
        }

        // The Saturated guard kept as CEILING DOCUMENTATION (spec §2): 10
        // pirates is a predator:prey choice, not the guard's integer floor.
        let runway = cfg.trophic.grubstake_micros / cfg.trophic.upkeep_per_tick;
        let cycle = runway as u64 + cfg.trophic.starve_lie_low_ticks;
        let expected_active = pirates as u64 * runway as u64 / cycle;
        assert!(
            expected_active <= cfg.stations.len() as u64 - 2,
            "expected-active {expected_active} <= stations - 2"
        );

        // Food band re-walk STARTS at 15k (spec §3, OD-2: dock-exposure
        // dilution); the band identities still pass at the new value.
        assert_eq!(cfg.trophic.food_per_unit_micros, 15_000);
        assert!(
            5 * cfg.trophic.food_per_unit_micros
                >= 2 * cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "one qty-5 rob sustains >= 2 windows"
        );
        assert!(
            cfg.trophic.grubstake_micros > cfg.trophic.upkeep_per_tick * WINDOW_TICKS as i64,
            "grubstake outlasts one window"
        );
        assert!(
            cfg.trophic.ransom_cap_micros >= cfg.shipyard.escort_price_micros[0],
            "one capped ransom funds the pirate counter-rung"
        );

        // Physics block VERBATIM from the band (spec §2) + the 120k window.
        assert_eq!(cfg.dt.get(), 0.25);
        assert_eq!(cfg.softening, 1.0e-4);
        assert_eq!(cfg.substep_cfg.accel_ref, 3.0e-4);
        assert_eq!(cfg.substep_cfg.max_substeps, 64);
        assert_eq!(cfg.ephemeris_window, 120_000, "frontier window (runner guard 2.5)");

        // Seam-haven law REPLACES hideout-outermost (spec §3, OD-3): haven =
        // station 6 hosted by body 7 (1.4660 AU), a vendor (the pirate escort
        // settle path), NOT the outermost body.
        assert_eq!(cfg.trophic.hideout_body_index, 7);
        assert_eq!(cfg.stations[FRONTIER_HAVEN_STATION].body_index, 7);
        assert!(
            cfg.stations[FRONTIER_HAVEN_STATION].sells_upgrades,
            "haven is a vendor (resolve_purchases settle path)"
        );
        assert!(
            (cfg.trophic.hideout_body_index as usize) < cfg.bodies.len() - 1,
            "haven sits at the SEAM, not the outermost body"
        );

        // Reach EXPLICIT in this factory too (spec §6) — the 8-9 gap is the
        // never-opens seam against exactly this value.
        assert_eq!(cfg.trophic.pirate_max_reach_au, 0.6);

        // ASSIGN/belief/buy machinery carried from the band.
        assert_eq!(cfg.dispatch_cfg.stagger_period, 16);
        assert_eq!(cfg.dispatch_cfg.demand_low, 10);
        assert_eq!(cfg.dispatch_cfg.demand_high, 20);
        assert!(cfg.trophic.hauler_belief_scoring, "belief scoring ON");
        assert_eq!(cfg.trophic.hauler_buy_policy, BuyPolicy::EscortFirst);
        assert!(cfg.trophic.engage_radius_au > 0.0, "trophic machinery LIVE");
    }

    #[test]
    fn scenario_frontier_wiring_invariants() {
        let cfg = scenario_frontier(7);
        let n = cfg.stations.len();

        // Partitioned tier loops EXACT (spec §3): per tier, 2 Ore legs
        // src→dest + 1 Fuel return dest→sink; rewards 1.0M / 2.3M / 3.9M.
        assert_eq!(cfg.contracts.len(), 9, "3 tiers × (2 Ore legs + 1 Fuel return)");
        for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
            let (src_a, src_b, dest, sink) = FRONTIER_TIER_WIRING[tier];
            let legs: Vec<&ContractInit> =
                cfg.contracts.iter().filter(|k| k.corp_index == tier).collect();
            assert_eq!(legs.len(), 3, "tier {tier} has 3 legs");
            let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
            for k in &legs {
                assert_eq!(k.qty, qty, "tier {tier} lot size");
                assert_eq!(k.reward_micros, reward, "tier {tier} reward ladder");
            }
            let ore_froms: std::collections::BTreeSet<usize> = legs
                .iter()
                .filter(|k| k.resource == Resource::Ore)
                .map(|k| k.from_station_index)
                .collect();
            assert_eq!(
                ore_froms,
                [src_a, src_b].into_iter().collect::<std::collections::BTreeSet<_>>(),
                "tier {tier} sources"
            );
            assert!(
                legs.iter()
                    .filter(|k| k.resource == Resource::Ore)
                    .all(|k| k.to_station_index == dest),
                "tier {tier} Ore legs land at dest {dest}"
            );
            let ret: Vec<_> = legs.iter().filter(|k| k.resource == Resource::Fuel).collect();
            assert_eq!(ret.len(), 1, "tier {tier} has exactly one Fuel return");
            assert_eq!(ret[0].from_station_index, dest, "return departs the dest");
            assert_eq!(ret[0].to_station_index, sink, "return lands at the sink");
        }
        // Spec §3 headline rewards, recomputed from the tier table.
        let rewards: Vec<i64> = TIERS
            .iter()
            .map(|&(q, m)| q as i64 * PER_UNIT_BASE_MICROS * m / 1000)
            .collect();
        assert_eq!(rewards, vec![1_000_000, 2_300_000, 3_900_000]);

        // Per-tier dests and sinks pairwise DISJOINT (independent Schmitt
        // triggers — the trophic decoupling law carried to the big map).
        for i in 0..3 {
            for j in 0..3 {
                if i == j {
                    continue;
                }
                assert_ne!(
                    FRONTIER_TIER_WIRING[i].2, FRONTIER_TIER_WIRING[j].2,
                    "tier dests disjoint"
                );
                assert_ne!(
                    FRONTIER_TIER_WIRING[i].3, FRONTIER_TIER_WIRING[j].3,
                    "tier sinks disjoint"
                );
            }
        }

        // Every station ∈ sources ∪ dests ∪ sinks ∪ {haven} — no orphans.
        let mut covered = std::collections::BTreeSet::new();
        for &(a, b, d, s) in &FRONTIER_TIER_WIRING {
            covered.extend([a, b, d, s]);
        }
        covered.insert(FRONTIER_HAVEN_STATION);
        assert_eq!(
            covered,
            (0..n).collect::<std::collections::BTreeSet<_>>(),
            "every station is in sources ∪ dests ∪ sinks ∪ {{haven}}"
        );

        // The haven is DARK (spec §3): vendor, NO producer, NO contract
        // endpoint — a dark port at the seam.
        assert!(
            cfg.contracts.iter().all(|k| {
                k.from_station_index != FRONTIER_HAVEN_STATION
                    && k.to_station_index != FRONTIER_HAVEN_STATION
            }),
            "haven hosts no contract endpoint"
        );
        assert!(
            cfg.producers.iter().all(|p| p.station_index != FRONTIER_HAVEN_STATION),
            "haven hosts no producer"
        );

        // Every tier loop touches a vendor (heavy haulers shop where they
        // deliver — the restored mechanism): the vendor sits at each dest.
        for &(_, _, dest, _) in &FRONTIER_TIER_WIRING {
            assert!(cfg.stations[dest].sells_upgrades, "tier dest {dest} is a vendor");
        }

        // Per-tier Schmitt-stagger initial stocks carried (18/14/10 against
        // the ONE global 10/20 band): dest Ore + sink Fuel, descending.
        let dest_ore: Vec<i64> = FRONTIER_TIER_WIRING
            .iter()
            .map(|w| cfg.stations[w.2].initial_stock[Resource::Ore.index()])
            .collect();
        let sink_fuel: Vec<i64> = FRONTIER_TIER_WIRING
            .iter()
            .map(|w| cfg.stations[w.3].initial_stock[Resource::Fuel.index()])
            .collect();
        assert_eq!(dest_ore, vec![18, 14, 10], "dest Ore Schmitt stagger");
        assert_eq!(sink_fuel, vec![18, 14, 10], "sink Fuel Schmitt stagger");

        // Producers: miners at all 6 sources, refiners at the 3 dests, fuel
        // sinks at the 3 sink rows.
        assert_eq!(cfg.producers.len(), 12, "6 miners + 3 refiners + 3 fuel sinks");

        // Corps: 3 tier corps + the Yard + the Port (Port armed in 2.4).
        assert_eq!(cfg.corporations.len(), 5, "3 tier corps + Yard + Port");
        assert_eq!(cfg.shipyard.corp_index, 3, "the Yard receives upgrade payments");
        assert_eq!(cfg.corporations[4].treasury_micros, 0, "the Port starts empty");
        assert!(cfg.contracts.iter().all(|k| k.corp_index < 3), "Yard/Port post no routes");

        // Resolvable + brakable; reset mints the 10-pirate pool.
        let (w, _h) = World::reset(cfg).expect("scenario_frontier must resolve");
        assert_eq!(w.ships.pirate.iter().filter(|p| p.is_some()).count(), 10);
    }

    #[test]
    fn scenario_frontier_is_seed_derived_and_deterministic() {
        assert_eq!(
            scenario_frontier(7).config_hash(),
            scenario_frontier(7).config_hash()
        );
        let a = scenario_frontier(7);
        let b = scenario_frontier(8);
        assert_ne!(a.config_hash(), b.config_hash());
        assert!(
            a.bodies[1..]
                .iter()
                .zip(&b.bodies[1..])
                .any(|(x, y)| x.elements.m0 != y.elements.m0),
            "mean anomalies are seed-derived"
        );
        // A NEW world, not a re-skin: frontier ≠ trophic at the same seed.
        assert_ne!(
            scenario_frontier(7).config_hash(),
            scenario_trophic(7).config_hash()
        );
    }
```

- [ ] **Step 2: Run and watch it fail to compile.**

```bash
cargo test -p jumpgate-core scenario_frontier
```

Expected failure: `error[E0425]: cannot find function 'scenario_frontier' in this scope` (plus E0425 for `FRONTIER_NUM_HAULERS`, `FRONTIER_HAVEN_STATION`, `FRONTIER_TIER_WIRING`, `FRONTIER_HAULER_EXHAUST_VELOCITY`).

- [ ] **Step 3: Add the frontier consts.** In `scenario.rs`, below `FRONTIER_ORBIT_AU` (Task 2.1):

```rust
/// Frontier populations (spec §2): 2 haulers per station; 10 pirates is a
/// 2:1 predator:prey DESIGN CHOICE carried from the band, NOT a guard-derived
/// cap — the Saturated guard's integer floor admits up to 13 at n=10 (the
/// guard stays as ceiling documentation in `scenario_frontier_shape`).
pub const FRONTIER_NUM_HAULERS: usize = 20;
pub const FRONTIER_NUM_PIRATES: usize = 10;

/// Frontier HAULER exhaust velocity — the ANALYTIC PRIOR, **pending
/// calibration** (spec §4, OD-5): the phase-2 calibration ensemble
/// (`craft.fuel_capacity_scale = 100`) measures the worst HAULER-leg burn and
/// the baked value is derived as k ≈ 2.5 × that MEASUREMENT — never spec
/// arithmetic. The bake task replaces this value and writes the derivation
/// into this doc comment. At 1.0: burn 2.5e-13/tick, endurance ≈ 4,000
/// thrusting ticks ≈ 2.5× the worst round trip; tank (1e-9) = 100× the
/// re-baked FUEL_EMPTY_EPS, so the FuelEmpty edge is LIVE. Pirates do NOT
/// use this const — they keep the band's 20.0 per-craft (OD-6).
pub const FRONTIER_HAULER_EXHAUST_VELOCITY: f64 = 1.0;

/// Haven station row (spec §3, OD-3): the dark port at the SEAM — hosted by
/// body 7 (1.4660 AU), a vendor (the pirate escort settle path requires a
/// vendor at the hideout dock), hosting NO producer and NO contract endpoint.
pub const FRONTIER_HAVEN_STATION: usize = 6;

/// Partitioned tier loops (spec §3, OD-2 — the self-averaging fix):
/// `(source_a, source_b, dest, fuel_sink)` station rows per tier. Dests and
/// sinks are per-tier disjoint (independent Schmitt triggers); every loop
/// touches a vendor (the vendor sits at the dest); the tier-2 return (9→8)
/// rides the never-walkable 8-9 gap.
pub const FRONTIER_TIER_WIRING: [(usize, usize, usize, usize); 3] =
    [(0, 1, 2, 3), (3, 4, 5, 4), (7, 8, 9, 8)];
```

- [ ] **Step 4: Add `RefuelCfg` to the scenario imports.** Extend the `use crate::config::{...}` list (scenario.rs:24-28) — it currently ends `..., ProducerInit, RunConfig, ShipyardCfg, StationInit, SubstepCfg, TrophicCfg,`; insert `RefuelCfg` in alphabetical position:

```rust
use crate::config::{
    BaseSpec, BodyInit, BuyPolicy, ContractInit, CorporationInit, CraftInit, DispatchCfg,
    GuidanceParams, MediaCfg, OrbitalElements, PriceCfg, ProducerInit, RefuelCfg, RunConfig,
    ShipyardCfg, StationInit, SubstepCfg, TrophicCfg,
};
```

- [ ] **Step 5: Write the factory.** Append after `scenario_trophic` ends (scenario.rs:254), before `apply_knob`:

```rust
/// Build the world-gets-big frontier scenario for one master seed (WGB spec
/// §2-§3): 10 stations on the geometric 0.35→3.0 AU band, partitioned tier
/// loops (core/mid/frontier), the dark seam haven, per-class craft specs.
/// Pure config: same seed ⇒ identical RunConfig (and config_hash); body mean
/// anomalies and all spawn geometry are seed-derived (the same `mix`).
///
/// A NEW world sharing the band's economic constants (GEO-C3): all cross-map
/// reads are rate-normalized distribution-vs-distribution, never same-seed
/// paired deltas.
pub fn scenario_frontier(seed: u64) -> RunConfig {
    const STAR_MASS: f64 = 1.0e-3;
    const BODY_MASS: f64 = 1.0e-12;

    // --- bodies: star + 10 station bodies on the pinned band, seed-derived
    // phases via the existing mix (anti-memorization unchanged) -------------
    let mut bodies = vec![BodyInit {
        mass: STAR_MASS,
        elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
    }];
    for (k, &a) in FRONTIER_ORBIT_AU.iter().enumerate() {
        let m0 = u64_to_unit_f64(mix(seed, (k + 1) as u64)) * std::f64::consts::TAU;
        bodies.push(BodyInit {
            mass: BODY_MASS,
            elements: OrbitalElements { a, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0 },
        });
    }

    // --- craft: per-CLASS specs (spec §4/§6, OD-6) --------------------------
    // Haulers: v_e = the named analytic prior (the calibration bakes it);
    // tank 1e-9 = 100× the re-baked eps — the FuelEmpty edge is LIVE.
    let hauler_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: FRONTIER_HAULER_EXHAUST_VELOCITY,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    // Pirates: the band's ×10 endurance spec (~80k thrusting ticks — pirates
    // cannot strand this rung; the unification trigger is W11).
    let pirate_spec = BaseSpec {
        base_dry_mass: 1.0e-9,
        base_max_thrust: 1.0e-12,
        base_exhaust_velocity: 20.0,
        base_fuel_capacity: 1.0e-9,
        base_cargo_capacity: 5,
    };
    let co_orbit = |body_index: usize| -> (Vec3, Vec3) {
        let el = &bodies[body_index].elements;
        let mu = G_CANONICAL * (STAR_MASS + BODY_MASS);
        let v_circ = (mu / el.a).sqrt();
        let pos = Vec3::new(el.a * el.m0.cos(), el.a * el.m0.sin(), 0.0);
        let vel = Vec3::new(-v_circ * el.m0.sin(), v_circ * el.m0.cos(), 0.0);
        (pos, vel)
    };
    let mut craft = Vec::with_capacity(FRONTIER_NUM_HAULERS + FRONTIER_NUM_PIRATES);
    for k in 0..FRONTIER_NUM_HAULERS {
        let (pos, vel) = co_orbit(1 + (k % FRONTIER_ORBIT_AU.len()));
        craft.push(CraftInit {
            spec: hauler_spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Idle,
            scripted: true,
        });
    }
    for _ in 0..FRONTIER_NUM_PIRATES {
        // Pirates start co-orbiting the haven body (the seam); the reset
        // Piracy draw scatters their initial lurks.
        let (pos, vel) = co_orbit(1 + FRONTIER_HAVEN_STATION);
        craft.push(CraftInit {
            spec: pirate_spec.clone(),
            pos,
            vel,
            fuel_mass: 1.0e-9,
            role: CraftRole::Pirate,
            scripted: true,
        });
    }

    // --- stations: partitioned tier loops (spec §3, FRONTIER_TIER_WIRING) --
    // Vendors at the three tier dests (2/5/9: every loop touches a vendor)
    // and the haven (6). Schmitt stagger carried as per-tier INITIAL stocks
    // (18/14/10 dest Ore + 18/14/10 sink Fuel) against the ONE global 10/20
    // band — the trophic DEVIATION comment applies unchanged.
    let stock = |ore: i64, fuel: i64| -> [i64; crate::economy::N_RESOURCES] {
        let mut s = [0i64; crate::economy::N_RESOURCES];
        s[Resource::Ore.index()] = ore;
        s[Resource::Fuel.index()] = fuel;
        s
    };
    let station = |body_index: usize, ore: i64, fuel: i64, vendor: bool| StationInit {
        body_index,
        initial_stock: stock(ore, fuel),
        initial_price_micros: [0, 0], // task 2.4 seeds Fuel from the live curve
        sells_upgrades: vendor,
    };
    let stations = vec![
        // Tier-0 core: sources 0-1 → dest 2 (vendor); Fuel sink at 3.
        station(1, 40, 0, false),
        station(2, 40, 0, false),
        station(3, 18, 0, true),
        // Tier-1 mid: sources 3-4 → dest 5 (vendor); Fuel sink at 4. Row 3
        // doubles as the tier-0 Fuel sink (18), row 4 as tier-1's own (14).
        station(4, 40, 18, false),
        station(5, 40, 14, false),
        station(6, 14, 0, true),
        // The haven (row 6, body 7): the dark port at the seam — vendor,
        // NO producer, NO contract endpoint (spec §3).
        station(7, 0, 0, true),
        // Tier-2 frontier: sources 7-8 → dest 9 (vendor); Fuel sink at 8
        // (10). The 9→8 return rides the never-walkable 8-9 gap.
        station(8, 40, 0, false),
        station(9, 40, 10, false),
        station(10, 10, 0, true),
    ];
    let producers = vec![
        // Ore miners at the six tier sources.
        ProducerInit { station_index: 0, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 1, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 3, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 7, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        ProducerInit { station_index: 8, recipe: Recipe { input: None, output: Some((Resource::Ore, 5)), interval: 40 } },
        // Refiners (Ore -> Fuel) at the three tier dests: the Ore demand
        // sinks AND the propellant supply geography (miners→refiners→tanks).
        ProducerInit { station_index: 2, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        ProducerInit { station_index: 5, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        ProducerInit { station_index: 9, recipe: Recipe { input: Some((Resource::Ore, 5)), output: Some((Resource::Fuel, 5)), interval: 60 } },
        // Fuel sinks at the per-tier return-leg destinations.
        ProducerInit { station_index: 3, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 4, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
        ProducerInit { station_index: 8, recipe: Recipe { input: Some((Resource::Fuel, 5)), output: None, interval: 80 } },
    ];

    // --- corps: 3 tier corps + the Yard (3, upgrade payments) + the Port
    // (4, propellant revenue — armed by RefuelCfg.corp_index in task 2.4;
    // treasury 0 = the Yard precedent, keeps the circulation panel clean).
    let corporations = vec![
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 2 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 5 },
        CorporationInit { treasury_micros: 2_000_000_000, home_station_index: 9 },
        CorporationInit { treasury_micros: 0, home_station_index: 2 }, // the Yard
        CorporationInit { treasury_micros: 0, home_station_index: 2 }, // the Port
    ];

    // --- 9 directed route templates: per tier, 2 Ore legs src→dest + 1 Fuel
    // return dest→sink (rewards 1.0M / 2.3M / 3.9M via the tier table).
    let mut contracts = Vec::with_capacity(9);
    for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
        let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
        let (src_a, src_b, dest, sink) = FRONTIER_TIER_WIRING[tier];
        for from in [src_a, src_b] {
            contracts.push(ContractInit {
                corp_index: tier,
                resource: Resource::Ore,
                qty,
                from_station_index: from,
                to_station_index: dest,
                reward_micros: reward,
            });
        }
        contracts.push(ContractInit {
            corp_index: tier,
            resource: Resource::Fuel,
            qty,
            from_station_index: dest,
            to_station_index: sink,
            reward_micros: reward,
        });
    }

    // --- the band's trophic constants as the STARTING WALK (spec §3): food
    // 10k→15k (dock-exposure dilution; identities still pass), everything
    // else carried and re-walked at the console — never "same band".
    let trophic = TrophicCfg {
        engage_radius_au: 5.0e-4,
        upkeep_per_tick: 12,
        food_per_unit_micros: 15_000,
        grubstake_micros: 100_000,
        ransom_cap_micros: 6_000_000,
        starve_lie_low_ticks: 4_000,
        hideout_body_index: 7, // the SEAM haven (station 6), NOT the outermost (OD-3)
        pirate_max_reach_au: 0.6, // EXPLICIT (spec §6): the 8-9 gap 0.637 never opens
        hauler_belief_scoring: true,
        hauler_buy_policy: BuyPolicy::EscortFirst,
        ..TrophicCfg::default()
    };

    RunConfig {
        master_seed: seed,
        dt: Dt::new(0.25),
        softening: 1.0e-4,
        substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
        // 120k (spec §2): worst leg ~1010 ticks, calibration runs are long;
        // the runner guard (task 2.5) aborts ticks > window — the ephemeris
        // CLAMPS silently past it (orbits would freeze).
        ephemeris_window: 120_000,
        bodies,
        craft,
        guidance: GuidanceParams::default(),
        stations,
        producers,
        corporations,
        contracts,
        price_cfg: PriceCfg {
            // DEAD until task 2.4 flips Fuel live. cap 0 = the structural-off
            // switch; never inherit PriceCfg::default()'s live-ish cap [1,1]
            // in a factory.
            base_micros: [0, 0],
            cap: [0, 0],
            slope_milli: 1800,
            reprice_interval: 1,
        },
        dispatch_cfg: DispatchCfg {
            demand_low: 10,
            demand_high: 20,
            stagger_period: 16,
            contract_reward_micros: 0,
            contract_qty: 0,
        },
        trophic,
        shipyard: ShipyardCfg { corp_index: 3, ..ShipyardCfg::default() },
        media: MediaCfg::default(),
        refuel: RefuelCfg::default(), // OFF (lot_mass 0.0) until task 2.4
    }
}
```

(The `RunConfig` literal's field set must match phase 1's tail exactly — if phase 1 named or ordered fields differently, the compiler's missing-field error on this exhaustive literal is the guide; do not drop fields to silence it.)

- [ ] **Step 6: Export the factory.** In `crates/jumpgate-core/src/lib.rs` (lib.rs:71):

```rust
pub use scenario::{apply_knob, scenario_frontier, scenario_trophic};
```

- [ ] **Step 7: Run and watch all three pass.**

```bash
cargo test -p jumpgate-core scenario_frontier
```

Expected: `scenario_frontier_shape ... ok`, `scenario_frontier_wiring_invariants ... ok`, `scenario_frontier_is_seed_derived_and_deterministic ... ok`. (The wiring test's `World::reset` exercises the 120k-window ephemeris precompute — a few seconds is normal.)

- [ ] **Step 8: Full suite, lint, commit.**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/scenario.rs crates/jumpgate-core/src/lib.rs
git commit -F - <<'EOF'
feat(frontier): scenario_frontier factory — 10-station band, partitioned tier loops, dark seam haven (WGB §2-§3)

Star + 10 bodies on FRONTIER_ORBIT_AU (seed phases via mix); 20 haulers
(2/station) + 10 pirates; per-class CraftInit specs (hauler v_e = named
analytic prior pending calibration, pirate v_e 20 per OD-6); physics block
verbatim from the band; ephemeris_window 120k. Tier loops per spec §3 with
the §3 invariant battery (disjoint dests/sinks, full coverage, dark haven,
vendor-touch, mod-n, seam-haven replaces hideout-outermost, Schmitt
stocks). food 15k start; reach 0.6 explicit. Pricing + refuel deliberately
OFF here (task 2.4 arms them). No goldens move (new factory, no
sample()/format change).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.4: the first live price — Fuel-only PriceCfg, curve-seeded initial prices, `RefuelCfg` lot 5e-11 → the Port corp

Spec §5 Pricing / OD-4: `base_micros [0, 5_000]`, `cap [0, 40]`, slope 1800 — full stock (≥ cap) 1,000 → dry 10,000 micros/unit; `cap[Ore] == 0` keeps Ore structurally dead (update_prices skips cap-0 rows, economy.rs:308-310); `initial_price_micros[Fuel]` seeded FROM THE CURVE at factory build; revenue → the Port corp (`RefuelCfg { lot_mass: 5e-11, corp_index: 4 }`, 20 lots/tank). The pre-registered "fuel spend ≈ 1–3% of revenue" null is a WINDOW — nothing here gates on it.

Files
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/scenario.rs` (the `station` closure, `price_cfg`, and `refuel` fields written in Task 2.3; new tests in `mod tests`)

- [ ] **Step 1: Write the failing tests.** Append to `mod tests` in `scenario.rs`. First add the test-module imports these need (next to the existing `use crate::world::World;` inside `mod tests`):

```rust
    use crate::contract::{Command, EventKind, StateView};
    use crate::time::Tick;
```

then the tests:

```rust
    #[test]
    fn scenario_frontier_fuel_pricing_and_port() {
        let cfg = scenario_frontier(7);

        // PriceCfg: Fuel-only live (spec §5, OD-4). cap[Ore]==0 is the
        // structural-off switch — Ore stays dead by construction.
        assert_eq!(cfg.price_cfg.base_micros, [0, 5_000], "Fuel-only base");
        assert_eq!(cfg.price_cfg.cap, [0, 40], "cap[Ore]==0 keeps Ore structurally dead");
        assert_eq!(cfg.price_cfg.slope_milli, 1800);
        assert_eq!(cfg.price_cfg.reprice_interval, 1);

        // Curve endpoints (the update_prices integer curve, recomputed):
        // dry (s=0) 10_000; full (s=cap) 1_000 micros/unit.
        assert_eq!((5_000i64 * (2000 - 0 * 1800 / 40) / 1000).max(0), 10_000);
        assert_eq!((5_000i64 * (2000 - 40 * 1800 / 40) / 1000).max(0), 1_000);

        // initial_price_micros[Fuel] is seeded FROM THE CURVE at the
        // station's initial stock (spec §5); Ore price 0 everywhere; every
        // seeded fuel price nonzero (the phase-1 half-on guard's input).
        for (row, s) in cfg.stations.iter().enumerate() {
            assert_eq!(
                s.initial_price_micros[Resource::Ore.index()],
                0,
                "station {row}: Ore price dead"
            );
            let st = s.initial_stock[Resource::Fuel.index()].clamp(0, 40);
            let want = (5_000 * (2000 - st * 1800 / 40) / 1000).max(0);
            assert_eq!(
                s.initial_price_micros[Resource::Fuel.index()],
                want,
                "station {row}: fuel price seeded from the curve"
            );
            assert!(
                s.initial_price_micros[Resource::Fuel.index()] > 0,
                "station {row}: half-on guard input must be nonzero"
            );
        }

        // RefuelCfg LIVE (spec §5): lot 5e-11 ⇒ 20 lots per 1e-9 tank
        // (~1 lot core leg, ~3-4 frontier leg); revenue → the Port corp.
        assert_eq!(cfg.refuel.lot_mass, 5.0e-11, "lot_mass");
        assert_eq!(cfg.refuel.corp_index, 4, "the Port corp index");
        let lots = (1.0e-9 / cfg.refuel.lot_mass).round() as u32;
        assert_eq!(lots, 20, "20 lots per tank");

        // The half-on guard accepts the armed factory: reset resolves.
        World::reset(scenario_frontier(7)).expect("frontier resolves with refuel live");
    }

    #[test]
    fn frontier_ore_price_never_updates_and_fuel_rides_the_curve() {
        // cap[Ore]==0 ⇒ update_prices skips the row forever; Fuel prices
        // stay inside the curve band [1_000, 10_000].
        let (mut world, _h) = World::reset(scenario_frontier(7)).expect("resolve");
        let mut cmds: Vec<Command> = Vec::new();
        for _ in 0..500 {
            world.step(&mut cmds);
        }
        let mut fuel_updates = 0u32;
        for e in world.recent_events(Tick(0)) {
            if let EventKind::PriceUpdate { resource, price_micros, .. } = e.kind {
                match resource {
                    Resource::Ore => {
                        panic!("Ore price updated — cap[Ore]==0 must keep it dead")
                    }
                    Resource::Fuel => {
                        fuel_updates += 1;
                        assert!(
                            (1_000..=10_000).contains(&price_micros),
                            "fuel price {price_micros} outside the curve band"
                        );
                    }
                }
            }
        }
        // Non-vacuity: the dest refiners land Fuel within 500 ticks
        // (interval 60) — stock moves ⇒ at least one Fuel PriceUpdate.
        assert!(fuel_updates > 0, "no Fuel PriceUpdate in 500 ticks — vacuous test");
    }
```

- [ ] **Step 2: Run and watch them fail.**

```bash
cargo test -p jumpgate-core frontier_ore_price scenario_frontier_fuel
```

Expected failure (the 2.3 factory has pricing dead): `assertion 'left == right' failed: Fuel-only base` — `left: [0, 0]`, `right: [0, 5000]`; and `no Fuel PriceUpdate in 500 ticks — vacuous test`.

- [ ] **Step 3: Arm the factory.** Three edits inside `scenario_frontier` from Task 2.3.

(a) Replace the `station` closure's price line with the curve seed — insert the `fuel_price` helper directly above it:

```rust
    // Demand-deflation curve seed (spec §5): the SAME integer curve
    // update_prices walks — price = base·(2000 − min(stock,cap)·slope/cap)/1000
    // at base 5_000 / cap 40 / slope 1800 ⇒ dry 10_000, full 1_000.
    let fuel_price = |fuel_stock: i64| -> i64 {
        let s = fuel_stock.clamp(0, 40);
        (5_000 * (2000 - s * 1800 / 40) / 1000).max(0)
    };
    let station = |body_index: usize, ore: i64, fuel: i64, vendor: bool| StationInit {
        body_index,
        initial_stock: stock(ore, fuel),
        initial_price_micros: [0, fuel_price(fuel)], // [Ore dead, Fuel from the curve]
        sells_upgrades: vendor,
    };
```

(b) Replace the `price_cfg` field:

```rust
        price_cfg: PriceCfg {
            // The first live price (OD-4): Fuel only — full (stock ≥ 40)
            // 1_000 → dry 10_000 micros/unit; a full fill ≈ the grubstake ≈
            // 10% of a tier-1 reward. cap[Ore] == 0 = the structural-off
            // switch (update_prices skips the row).
            base_micros: [0, 5_000],
            cap: [0, 40],
            slope_milli: 1800,
            reprice_interval: 1,
        },
```

(c) Replace the `refuel` field:

```rust
        // Refuel LIVE (spec §5): 20 lots/tank (~1 lot core leg, ~3-4
        // frontier leg); revenue → the Port corp (index 4, treasury 0) —
        // generator AND consumer land in one rung (the OD-5b two-sided law).
        refuel: RefuelCfg { lot_mass: 5.0e-11, corp_index: 4 },
```

- [ ] **Step 4: Run and watch them pass (and the 2.3 battery stay green).**

```bash
cargo test -p jumpgate-core scenario_frontier
cargo test -p jumpgate-core frontier_ore_price
```

Expected: all frontier tests ok, including the unchanged 2.3 battery (the wiring test asserts stocks/vendors, not prices).

- [ ] **Step 5: Full suite, lint, commit.** No golden touches: scenario factories never feed `sample()`/`GOLDEN_CONFIG_HASH`, and the trophic factory still carries `RefuelCfg::default()` (W12's control stays a control).

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/scenario.rs
git commit -F - <<'EOF'
feat(frontier): Fuel-only live pricing + curve-seeded initial prices + refuel lot 5e-11 -> Port corp (WGB §5, OD-4)

PriceCfg base [0, 5_000] / cap [0, 40] / slope 1800: full 1_000 -> dry
10_000 micros/unit; cap[Ore]==0 keeps Ore structurally dead (tested over a
stepped world). initial_price_micros[Fuel] seeded from the same integer
curve at factory build (the phase-1 half-on guard's input). RefuelCfg
{ lot_mass: 5e-11, corp_index: 4 }: 20 lots/tank, revenue to the empty
Port corp (the Yard precedent). Trophic stays refuel-off; no goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 2.5: `--scenario` flag + the ephemeris-window runner guard

Today `examples/trophic_run.rs` hardcodes `scenario_trophic` (trophic_run.rs:113) and errors on unknown args (trophic_run.rs:97); the ephemeris silently CLAMPS past-window lookups (ephemeris.rs:106-111 — pinned correct by `lookup_past_window_clamps_to_last_sample`, do NOT change `body_pos`). The guard lives in the runner: after the `apply_knob` loop, before `World::reset`, against `cfg.ephemeris_window`. The runner is an example binary, so red/green here are run commands with expected stdout/stderr, not unit tests.

Files
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs` (Args struct :38-53, `parse_args` :55-101, `simulate` :109-118, the `use jumpgate_core::{...}` list :30-33; plus the phase-0b META line's `scenario=` value if landed)

- [ ] **Step 1: Demonstrate the red state.**

```bash
cargo run -p jumpgate-core --example trophic_run -- --scenario frontier --ticks 2000
```

Expected failure: `unknown arg: --scenario` on stderr, nonzero exit.

- [ ] **Step 2: Add the flag and the guard.**

(a) Import the frontier factory (trophic_run.rs:30-33):

```rust
use jumpgate_core::{
    Command, CraftId, EventKind, GossipNode, RunConfig, StateView, Tick, World, apply_knob,
    scenario_frontier, scenario_trophic, state_hash,
};
```

(b) `Args` gains the scenario name (after `ticks`, trophic_run.rs:40):

```rust
    ticks: u64,
    /// Scenario factory: "trophic" (default, the banked control world) or
    /// "frontier" (WGB §2). Unknown names are loud errors.
    scenario: String,
```

and the default in `parse_args` (trophic_run.rs:56-66):

```rust
        ticks: 50_000,
        scenario: "trophic".to_string(),
```

(c) The parse arm (next to `--ticks`, trophic_run.rs:74-77):

```rust
            "--scenario" => {
                args.scenario = it.next().ok_or("--scenario needs a value")?;
            }
```

(d) In `simulate` (trophic_run.rs:109-118), replace the hardcoded factory call and add the guard AFTER the knob loop, BEFORE `World::reset`:

```rust
    let mut cfg: RunConfig = match args.scenario.as_str() {
        "trophic" => scenario_trophic(args.seed),
        "frontier" => scenario_frontier(args.seed),
        other => return Err(format!("--scenario {other}: unknown scenario (trophic|frontier)")),
    };
    for (k, v) in &args.sets {
        apply_knob(&mut cfg, k, v)?;
    }
    // NEW runner guard (WGB §2): past-window ephemeris lookups silently
    // CLAMP to the last sample (ephemeris.rs) — a longer run would freeze
    // every orbit and lie quietly. Checked after the knob loop, against the
    // window the run will actually precompute.
    if args.ticks > cfg.ephemeris_window {
        return Err(format!(
            "--ticks {} > ephemeris_window {}: past-window orbits silently freeze; lower --ticks or raise the window",
            args.ticks, cfg.ephemeris_window
        ));
    }
    let (mut world, _config_hash) = World::reset(cfg)
        .map_err(|e| format!("scenario_{} must resolve: {e}", args.scenario))?;
```

(`simulate` is called twice under `--replay-check`; both calls go through this match, so the second run rebuilds the same scenario from `(seed, scenario, sets)` — the existing recipe property, preserved.)

- [ ] **Step 3: Thread the scenario name into the phase-0b META line (if landed).** Phase 0b owns the META format (`META seed= scenario= stations= haulers= pirates_initial= station_radii_milli_au=[…]`). Locate it:

```bash
grep -n '"META' /home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs
```

If present with a hardcoded `scenario=trophic` token, change ONLY the value source so the same format string prints `args.scenario` (e.g. the `scenario={}` placeholder fed by `args.scenario` in the existing `println!` argument list). Do not add/move/reorder tokens — the sweep regexes are line-anchored and phase-0b-owned. If phase 0b has not landed yet, do NOT skip this step and leave a placeholder comment — instead coordinate the rebase so phase 0b lands first (it precedes phase 2 in the spec's landing order).

- [ ] **Step 4: Green runs — all four behaviors.**

```bash
# 1. frontier runs end-to-end under the flag
cargo run -p jumpgate-core --example trophic_run -- --scenario frontier --seed 7 --ticks 2000 --replay-check
# expect: normal run output ending in the RESULT line; replay check OK; exit 0

# 2. the guard catches a frontier run past the 120k window
cargo run -p jumpgate-core --example trophic_run -- --scenario frontier --seed 7 --ticks 130000
# expect stderr: "--ticks 130000 > ephemeris_window 120000: past-window orbits silently freeze; lower --ticks or raise the window"; nonzero exit

# 3. the guard protects trophic too (window 100k)
cargo run -p jumpgate-core --example trophic_run -- --seed 7 --ticks 150000
# expect stderr: "--ticks 150000 > ephemeris_window 100000: ..."; nonzero exit

# 4. unknown scenarios are loud
cargo run -p jumpgate-core --example trophic_run -- --scenario nope --ticks 100
# expect stderr: "--scenario nope: unknown scenario (trophic|frontier)"; nonzero exit
```

- [ ] **Step 5: Prove the default path is untouched.**

```bash
cargo run -p jumpgate-core --example trophic_run -- --seed 7 --ticks 2000 --replay-check > /tmp/wgb-2_5-trophic.txt 2>&1
diff /tmp/wgb-2_2-after.txt /tmp/wgb-2_5-trophic.txt && echo TROPHIC-PATH-UNCHANGED
```

Expected: `diff` silent (byte-identical output vs the Task 2.2 baseline — flag default + guard are no-ops on the control world), `TROPHIC-PATH-UNCHANGED` printed. (If phase 0b/1 landed between the captures, re-capture the pre-change baseline at this task's start instead of reusing 2.2's file — the diff must bracket ONLY this task's edit.)

- [ ] **Step 6: Full suite, lint, sweep-parser sanity, commit.**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
PYTHONPATH=/home/john/jumpgate/python pytest python/tests
git add crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(runner): --scenario {trophic|frontier} flag + ephemeris-window abort guard (WGB §2, §9 phase 2)

scenario_trophic was hardcoded; the factory is now selected by name with
loud errors for unknown names, and the runner aborts when ticks exceed
cfg.ephemeris_window (checked after the knob loop) instead of letting
past-window orbits freeze silently via the ephemeris clamp. body_pos and
its pinned clamp test are untouched; the default trophic path is verified
byte-identical by output diff. META's scenario= value now reads the flag.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

## Section-level verification (after 2.5)

- [ ] `cargo test --workspace` green; `cargo clippy --all-targets -- -D warnings` clean.
- [ ] `git log --oneline -8` shows five single-purpose commits; `git diff <pre-section>..HEAD -- crates/jumpgate-core/src/hash.rs` is EMPTY and the only `GOLDEN_CONFIG_HASH`/`GOLDEN_ZERO_STATE_HASH` literals in the diff range are phase 1's (this section moves zero goldens; the frontier trajectory golden belongs to the phase-2 second half).
- [ ] `runs/` untracked and unstaged (`git status --short runs/` empty).
- [ ] **Chronicle visibility (closes Task 1.2.6b's deferred check):** the first frontier smoke run shows refuel narration — `cargo run --release -p jumpgate-core --example trophic_run -- --scenario frontier --seed 7 --ticks 10000 --chronicle 2>&1 | grep -ci refuel` is non-zero (frontier has live RefuelCfg; a zero count means the chronicle arms or the verb wiring regressed — investigate, it is a defect not a reading).


---

## Phase 2 (second half) — instruments + calibration (spec §6 LurkMoved, §7, §8 TrophicSample, §9 phase-2 tail)

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

- [ ] **Step 6b: the W9 contract-age liveness read (runner-side bookkeeping — closes the spec §5 "liveness window" clause).** `ContractStore` carries NO accept tick (economy.rs:155-167 — status/escrow/hauler only), so contract age is an EVENT-stream read, and the runner already walks `recent_events` every tick. In `trophic_run.rs`'s `simulate` loop, in the same per-tick event pass the chronicle/gossip-log collection uses (extend the existing pass; do not add a second walk):

```rust
    // W9 liveness bookkeeping (recorded window, never a gate): accept tick per
    // live contract; terminal events retire the entry. A stuck non-terminal
    // contract (e.g. a hauler stranded by the dispatch fuel filter mid-leg)
    // shows up as a large max age at end of run.
    let mut open_contracts: std::collections::HashMap<ContractId, u64> = std::collections::HashMap::new();
    // ...in the per-tick event pass:
    match e.kind {
        EventKind::ContractAccepted { contract, .. } => {
            open_contracts.insert(contract, e.tick.0);
        }
        EventKind::ContractFulfilled { contract, .. }
        | EventKind::ContractFailed { contract, .. } => {
            open_contracts.remove(&contract);
        }
        _ => {}
    }
```

  and after the final-tick FUEL line, ONE anchored line (deterministic — take the max, not iteration order):

```rust
    let max_age = open_contracts.values().map(|&t0| final_tick.0 - t0).max().unwrap_or(0);
    println!("LIVENESS max_open_contract_age={} open_contracts={}", max_age, open_contracts.len());
```

  (Align the variant payloads with the landed phase-1 shapes — `ContractFailed` carries `contract` per spec §7; if `Robbed` retires a contract through a different terminal event on the stream, retire on that too — grep the teardown path and match what actually emits.) Verify: the seed-7 10k-tick run prints `LIVENESS max_open_contract_age=<small n> open_contracts=0` or near-0 on trophic (contracts cycle); the value is a recorded W9 read, never asserted.

- [ ] **Step 7: Full suite + lint, then commit:**

```
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/world.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
feat(chronicle): per-craft epilogue + W9 LIVENESS line

New pub World::craft_role (trader-accessor pattern); workplace radius = mean
Arrival-body radial distance in FLOOR milli-AU (circular orbits make the
current-tick read radius-invariant); ADRIFT = Idle + tank <= eps, since = last
FuelEmpty edge. LIVENESS max_open_contract_age from event-stream bookkeeping
(W9, recorded never gated). Printer-side only; hash-neutral.

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


---

## Phase 3 — science + console (spec §8 / §9 phase 3 / §11)

Plan section for `/home/john/jumpgate` @ `e7e490e` (writing-plans discipline).
Spec: `docs/superpowers/specs/2026-06-11-world-gets-big-design.md` §8, §9
(phase 3), §11. Frame: PDR-0006 — every number below is a WINDOW, recorded
and never gated; the only pass/fail surfaces in this phase are determinism
and unit tests.

**Cross-phase dependencies (must have landed before Phase 3 starts):**
phase 0a haven-lurk fix; phase 0b `META` line + role-split `FUEL` line +
version-gated `META_RE`/`FUEL_RE` in `sweep_trophic.py` + the banked
post-fix 20-seed trophic baseline; phase 1 refuel verb
(`RefuelCfg{lot_mass, corp_index}`, `pending_refuel`, `run_refuel_policies`
at 1c3b, `resolve_refuels` at 1d2, `Refueled`/`ContractFailed`); phase 2
`scenario_frontier`, the runner `--scenario` flag, `LurkMoved`,
`fuel_capacity_scale` calibration arm, and the TrophicSample additive fields
`per_station_lurking_pirates, pirates_commuting, pirates_at_haven,
per_station_fuel_stock, per_station_fuel_price, refuels, refuel_units,
refuel_spend_micros`. Where this section imports phase-0b symbols
(`META_RE`, its group names) the builder aligns spellings with what phase 0b
actually landed before running the failing test.

**The `simulate()` return tuple is a PINNED contract, not "whatever it
returns".** After phases 0b–2 the expected shape is exactly:

```python
samples, hashes, world, meta = simulate(cfg, ticks)
```

(`samples`: list of TrophicSample dicts; `hashes`: per-window state hashes;
`world`: the final world handle; `meta`: the META-line facts dict added by
phase 0b). Task 3.4 appends `endpoint_rows` as the FIFTH element —
`samples, hashes, world, meta, endpoint_rows = simulate(cfg, ticks)` — and
every destructure in this section uses EXACT arity (never `*rest`, which
binds silently wrong on a shape change; exact arity raises a loud
`ValueError`). Before writing any phase-3 destructure, the builder verifies
this statement against the landed `simulate()` signature; ANY task that
changes the tuple must update this paragraph and every destructure in the
same commit.

House rules encoded in every task: commits via `git commit -F -` heredoc
ending with the exact trailer line
`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`;
`git add` explicit paths only (never `-A`, never `.`); never stage `runs/`
or anything under `/tmp`; reward surfaces untouched; measured constants and
golden literals are PASTED from instrument output, never invented by the
planner or the builder's head.

---

### Task 3.1: sweep `--scenario` passthrough + per-cell stdout banking (the grid/fit substrate)

The fit (3.2), the I1/I2 control runs (3.3/3.5), and the headline grid (3.6)
all drive `trophic_run` through `sweep_trophic.py`, which today hardcodes
the runner invocation without `--scenario` (sweep_trophic.py:91-96) and
throws the stdout away after parsing (`run_one`, :89-118). The standalone
panels (3.6, the packet) need the banked stdout for `META`/`RESULT`.

**Files**
- Create: `/home/john/jumpgate/python/tests/test_sweep_cli.py`
- Modify: `/home/john/jumpgate/python/analysis/sweep_trophic.py`
  (`run_one` :89-118, argparse in `main` :300-312)

NOTE: spec §9 phase 2 gives the *runner* the `--scenario` flag. If the
phase-2 section also added a sweep passthrough, keep its semantics and land
only the `runner_cmd` extraction + stdout banking from this task; the test
below must pass either way.

- [ ] **Step 1: failing test — `runner_cmd` carries scenario, seed, knobs**

```python
"""CLI-seam tests for sweep_trophic (world-gets-big phase 3.1)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import sweep_trophic


def test_runner_cmd_carries_scenario_seed_and_knobs():
    cmd = sweep_trophic.runner_cmd(
        "frontier", 7, 50_000, "/tmp/x.jsonl", [("pirate_max_reach_au", "999")]
    )
    assert cmd[cmd.index("--scenario") + 1] == "frontier"
    assert cmd[cmd.index("--seed") + 1] == "7"
    assert cmd[cmd.index("--ticks") + 1] == "50000"
    assert cmd[cmd.index("--set") + 1] == "pirate_max_reach_au=999"


def test_runner_cmd_trophic_is_still_explicit():
    # The flag is passed UNCONDITIONALLY: the runner owns the
    # unknown-scenario error (a silent default would hide a typo'd arm).
    cmd = sweep_trophic.runner_cmd("trophic", 11, 1_000, "/tmp/y.jsonl", [])
    assert cmd[cmd.index("--scenario") + 1] == "trophic"
    assert "--set" not in cmd
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_cli.py`
Expected failure: `AttributeError: module 'sweep_trophic' has no attribute 'runner_cmd'`.

- [ ] **Step 2: implement — extract `runner_cmd`, add `--scenario`, bank stdout**

In `sweep_trophic.py`, above `run_one` (after `parse_knobset`), add:

```python
def runner_cmd(scenario, seed, ticks, jsonl, knobs):
    """The trophic_run invocation for one (arm, seed) cell. --scenario is
    passed UNCONDITIONALLY — the runner owns the unknown-scenario error."""
    cmd = [
        "cargo", "run", "-q", "-p", "jumpgate-core", "--release",
        "--example", "trophic_run", "--",
        "--scenario", scenario,
        "--seed", str(seed), "--ticks", str(ticks), "--jsonl", str(jsonl),
    ]
    for k, v in knobs:
        cmd += ["--set", f"{k}={v}"]
    return cmd
```

Rewrite the head of `run_one` (keep everything from `proc = subprocess.run`
parsing downward byte-identical except the inserted banking line):

```python
def run_one(args, name, knobs, seed, out_dir):
    jsonl = out_dir / f"{name}_s{seed}.jsonl"
    cmd = runner_cmd(args.scenario, seed, args.ticks, jsonl, knobs)
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit(f"run failed: {name} seed={seed}")
    # Bank the full stdout beside the JSONL: the standalone grid/packet
    # panels (w4_grid.py, the console packet) parse META/RESULT from it,
    # and /tmp sweep dirs get banked same-day (the capture practice).
    (out_dir / f"{name}_s{seed}.stdout").write_text(proc.stdout)
```

In `main()`'s argparse block (beside `--ticks`):

```python
    ap.add_argument(
        "--scenario",
        default="trophic",
        help="runner scenario factory (phase-2 flag): trophic | frontier",
    )
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_cli.py`
→ `2 passed`.

- [ ] **Step 4: commit**

```bash
git add python/analysis/sweep_trophic.py python/tests/test_sweep_cli.py
git commit -F - <<'EOF'
lab(sweep): --scenario passthrough + per-cell stdout banking

Phase-3 substrate for the dual-map fit and the 20-seed x 6-arm grid:
runner_cmd extracted (testable seam), --scenario forwarded
unconditionally, and each cell's stdout banked beside its JSONL so the
standalone panels and the console packet can parse META/RESULT later.
Windows, not gates (PDR-0006).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.2: dual-map HHI/slack re-fit post-leak-fix (labeled-run method, held-out seeds)

The heterogeneity instrument's two fitted constants —
`HHI_NORM_MIN_MILLI = 2204` (diagnostics.rs:44, doc :30-43) and
`HOT_PERSISTENCE_SLACK_CHANGES = 3` (diagnostics.rs:57, doc :46-56) — were
fitted on the 2026-06-11 labeled set, which the haven-lurk leak (spec §6,
fixed phase 0a) contaminates. Spec §8 requires a re-fit on BOTH maps with
the same labeled-run method. Constants are RE-DERIVED from measured runs and
pasted from the fit instrument's output (the golden discipline applied to
calibration constants) — never nudged. Neither constant is config-hashed:
zero goldens move.

**Pre-registered seed split (named now, before any frontier run):**
FIT seeds `7 23 42 99` (the historical labeled four); HELD-OUT seeds
`11 31 57 101`. Labels: `baseline` knobset = TRUE-clumped;
hungry-roamer `control` knobset = TRUE-equalized (recipe verbatim from
sweep_trophic.py:31-40).

**Files**
- Create: `/home/john/jumpgate/python/analysis/fit_heterogeneity.py`
- Create: `/home/john/jumpgate/python/tests/test_fit_heterogeneity.py`
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs`
  (constants + docs :30-57; classifier synthetics :667-810 only if the
  re-fitted boundary moves across them)

- [ ] **Step 1: failing test — the fit math mirrors diagnostics.rs and reproduces the documented 2026-06-11 fit**

`python/tests/test_fit_heterogeneity.py`:

```python
"""Pins fit_heterogeneity's integer math against diagnostics.rs (:270-316)
and against the DOCUMENTED 2026-06-11 fit (2204 / 3) — the mirror is the
method's instrument, so it ships with synthetics that would catch it lying
(the diagnostics.rs:896-899 house rule, applied lab-side)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import fit_heterogeneity as fh


def w(robs, traffic, active):
    return {
        "per_route_robs": robs,
        "per_route_traffic": traffic,
        "active_pirates": active,
    }


def test_mean_norm_hhi_masks_unoccupied_routes_and_normalizes_by_active():
    # Window 1: route 1 has robs but NO traffic -> masked to 0; occupied
    # robs [4, 0], HHI = 16*1000 // 16 = 1000 milli; x active.max(1)=3
    # -> 3000. Window 2: zero robs -> not a robbing window.
    ws = [w([4, 5], [1, 0], 3), w([0, 0], [1, 1], 9)]
    assert fh.mean_norm_hhi_milli(ws) == 3000


def test_mean_norm_hhi_floor_division_over_robbing_windows():
    # ([3,1] occupied: HHI=(9+1)*1000//16=625, x2=1250) and
    # ([1,1]: HHI=2*1000//4=500, x1=500) -> (1250+500)//2 = 875.
    ws = [w([3, 1], [1, 2], 2), w([1, 1], [3, 1], 1)]
    assert fh.mean_norm_hhi_milli(ws) == 875


def test_mean_norm_hhi_none_when_no_robbing_window():
    assert fh.mean_norm_hhi_milli([w([0, 0], [1, 1], 5)]) is None


def test_hot_change_excess_argmax_ties_to_lowest_index():
    # hot argmax: [2,1]->0, [1,2]->1, [2,2]->0 (tie -> LOWEST) = 2 changes;
    # traffic argmax constant at 0 -> 0 changes; excess = +2.
    ws = [w([2, 1], [1, 1], 1), w([1, 2], [1, 1], 1), w([2, 2], [1, 1], 1)]
    assert fh.hot_change_excess(ws) == 2


def test_fit_reproduces_the_documented_2026_06_11_constants():
    # The diagnostics.rs:30-56 doc tables ARE the regression fixture: the
    # method must reproduce threshold 2204 and slack 3 from them.
    t = fh.fit_threshold([3070, 2962, 2918, 3498], [1490, 1472])
    assert t == {
        "threshold": 2204,
        "clumped_min": 2918,
        "equalized_max": 1490,
        "margin_open": True,
    }
    s = fh.fit_slack([1, -1, -3, -6], [6, 5])
    assert s == {
        "slack": 3,
        "clumped_max": 1,
        "equalized_min": 5,
        "margin_open": True,
    }


def test_fit_reports_a_closed_margin_instead_of_inventing_a_boundary():
    t = fh.fit_threshold([1500, 1600], [1700])
    assert t["margin_open"] is False
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_fit_heterogeneity.py`
Expected failure: `ModuleNotFoundError: No module named 'fit_heterogeneity'`.

- [ ] **Step 2: implement `fit_heterogeneity.py`**

```python
"""fit_heterogeneity — labeled-run re-fit of the risk-heterogeneity
instrument's two constants (world-gets-big spec §8; post haven-lurk-fix).

FRAME (PDR-0006): the fit OUTPUT is pasted into diagnostics.rs doc + const
(provenance commit); the instrument separates TRUE-clumped from
TRUE-equalized LABELED runs — it is a measurement of the lab's own ruler,
never a gate on the game.

METHOD (the 2026-06-11 labeled-run method, diagnostics.rs:30-56):
  * per labeled run, compute mean per-window active-pirate-NORMALIZED HHI
    (milli) over OCCUPIED routes, and the hot-change excess
    (hot-route argmax changes - traffic argmax changes) — integer math
    mirroring diagnostics.rs:270-316 exactly (pinned by
    python/tests/test_fit_heterogeneity.py),
  * threshold = FLOOR midpoint of (min over clumped, max over equalized);
    slack = FLOOR midpoint of (max clumped excess, min equalized excess),
  * HELD-OUT runs are never in the fit: they are printed with their side
    of the fitted boundary, RECORDED,
  * a CLOSED margin (labels overlap) is reported as a finding — the script
    never invents a boundary.

Usage (per map, then pooled):
    python3 python/analysis/fit_heterogeneity.py \
        --clumped DIR/baseline_s*.jsonl --equalized DIR/control_s*.jsonl \
        --heldout-clumped HDIR/baseline_s*.jsonl \
        --heldout-equalized HDIR/control_s*.jsonl
"""
import argparse
import json


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def argmax_lowest(xs):
    """Strictly greatest POSITIVE value, ties to the LOWEST index; None when
    everything is zero/empty (mirror of diagnostics.rs argmax_lowest)."""
    best = None
    for i, x in enumerate(xs):
        if x > 0 and (best is None or x > xs[best]):
            best = i
    return best


def mean_norm_hhi_milli(windows):
    """Mean per-window active-pirate-normalized HHI (milli) over OCCUPIED
    routes; None if no window robbed (mirror of diagnostics.rs:270-316)."""
    norm_sum, robbing = 0, 0
    for w in windows:
        robs = [
            r if t > 0 else 0
            for r, t in zip(w["per_route_robs"], w["per_route_traffic"])
        ]
        total = sum(robs)
        if total == 0:
            continue
        robbing += 1
        hhi = sum(r * r for r in robs) * 1000 // (total * total)
        norm_sum += hhi * max(w["active_pirates"], 1)
    return None if robbing == 0 else norm_sum // robbing


def hot_change_excess(windows):
    """hot-route argmax changes minus traffic argmax changes, counted over
    robbing windows only (the diagnostics.rs persistence clause inputs)."""
    hot_changes = traffic_changes = 0
    prev_hot = prev_traffic = None
    for w in windows:
        robs = [
            r if t > 0 else 0
            for r, t in zip(w["per_route_robs"], w["per_route_traffic"])
        ]
        if sum(robs) == 0:
            continue
        hot = argmax_lowest(robs)
        if hot is not None and prev_hot is not None and hot != prev_hot:
            hot_changes += 1
        if hot is not None:
            prev_hot = hot
        tmax = argmax_lowest(w["per_route_traffic"])
        if tmax is not None and prev_traffic is not None and tmax != prev_traffic:
            traffic_changes += 1
        if tmax is not None:
            prev_traffic = tmax
    return hot_changes - traffic_changes


def fit_threshold(clumped, equalized):
    lo, hi = max(equalized), min(clumped)
    return {
        "threshold": (hi + lo) // 2,
        "clumped_min": hi,
        "equalized_max": lo,
        "margin_open": hi > lo,
    }


def fit_slack(clumped_excess, equalized_excess):
    hi, lo = max(clumped_excess), min(equalized_excess)
    return {
        "slack": (hi + lo) // 2,
        "clumped_max": hi,
        "equalized_min": lo,
        "margin_open": lo > hi,
    }


def measure(paths):
    rows = []
    for p in paths:
        ws = load(p)
        rows.append((p, mean_norm_hhi_milli(ws), hot_change_excess(ws)))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--clumped", nargs="+", required=True)
    ap.add_argument("--equalized", nargs="+", required=True)
    ap.add_argument("--heldout-clumped", nargs="*", default=[])
    ap.add_argument("--heldout-equalized", nargs="*", default=[])
    args = ap.parse_args()

    print("fit_heterogeneity (labeled-run method; PDR-0006: a ruler check, not a gate)")
    blocks = {}
    for label, paths in (
        ("clumped", args.clumped),
        ("equalized", args.equalized),
        ("heldout-clumped", args.heldout_clumped),
        ("heldout-equalized", args.heldout_equalized),
    ):
        rows = measure(paths)
        blocks[label] = rows
        for p, hhi, excess in rows:
            print(f"  {label:<18} {p}: mean_norm_hhi_milli={hhi} hot_change_excess={excess}")

    c = [h for _, h, _ in blocks["clumped"] if h is not None]
    e = [h for _, h, _ in blocks["equalized"] if h is not None]
    ce = [x for _, _, x in blocks["clumped"]]
    ee = [x for _, _, x in blocks["equalized"]]
    if not c or not e:
        raise SystemExit("a label produced no robbing windows: fit impossible — record it")
    t, s = fit_threshold(c, e), fit_slack(ce, ee)
    print(f"\nFIT threshold: {t}")
    print(f"FIT slack:     {s}")
    if not (t["margin_open"] and s["margin_open"]):
        print(
            "MARGIN CLOSED on this set — the labels overlap; do NOT move the "
            "constants from this fit. Record the overlap (a finding about the "
            "instrument on this map), keep the current literals, and register "
            "scenario-conditional thresholds as the named deferred trigger."
        )
    for label in ("heldout-clumped", "heldout-equalized"):
        for p, hhi, excess in blocks[label]:
            side = None if hhi is None else ("clumped" if hhi >= t["threshold"] else "equalized")
            print(f"HELD-OUT {label} {p}: hhi={hhi} -> boundary side={side} "
                  f"excess={excess} (RECORDED, never gated)")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_fit_heterogeneity.py`
→ `6 passed`.

- [ ] **Step 4: commit the instrument (before any field run)**

```bash
git add python/analysis/fit_heterogeneity.py python/tests/test_fit_heterogeneity.py
git commit -F - <<'EOF'
lab(fit): labeled-run re-fit instrument for HHI/slack (dual-map, spec s8)

Mirrors diagnostics.rs:270-316 integer math (pinned by synthetics that
reproduce the documented 2026-06-11 fit: 2204 / 3), fits FLOOR midpoints
over labeled runs, validates held-out seeds as recorded readings, and
refuses to invent a boundary when the margin is closed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

- [ ] **Step 5: PROCEDURE — frontier positive control FIRST (ordering step, not a ship gate)**

Run the hungry-roamer disease injection on `scenario_frontier` and read its
restatement BEFORE any other frontier reading is taken or recorded:

```bash
mkdir -p /tmp/wgb-fit
python3 python/analysis/sweep_trophic.py --scenario frontier \
  --seeds 7 23 42 99 --ticks 50000 \
  --knobset "control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000" \
  --out /tmp/wgb-fit/frontier-control-first \
  | tee /tmp/wgb-fit/frontier-control-first/sweep_stdout.txt
```

Read the trailing `positive control (hungry roamers, ...)` line. Expected:
`4/4 runs read RiskEqualized`. If NOT: the instrument is broken on the new
map — stop the fit, diagnose the instrument (this is the one place the
procedure ORDER is mandatory), and record what was seen. This step gates the
*procedure order*, never the shipping of any mechanic.

- [ ] **Step 6: run the labeled ensembles on both maps (50k ticks, default knobsets = baseline + control)**

```bash
python3 python/analysis/sweep_trophic.py --scenario trophic  --seeds 7 23 42 99   --ticks 50000 --out /tmp/wgb-fit/trophic-fit   | tee /tmp/wgb-fit/trophic-fit/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario trophic  --seeds 11 31 57 101 --ticks 50000 --out /tmp/wgb-fit/trophic-held  | tee /tmp/wgb-fit/trophic-held/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario frontier --seeds 7 23 42 99   --ticks 50000 --out /tmp/wgb-fit/frontier-fit  | tee /tmp/wgb-fit/frontier-fit/sweep_stdout.txt
python3 python/analysis/sweep_trophic.py --scenario frontier --seeds 11 31 57 101 --ticks 50000 --out /tmp/wgb-fit/frontier-held | tee /tmp/wgb-fit/frontier-held/sweep_stdout.txt
```

- [ ] **Step 7: fit per map, then pooled (the adopted constants come from the POOLED fit)**

```bash
python3 python/analysis/fit_heterogeneity.py \
  --clumped /tmp/wgb-fit/trophic-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/trophic-fit/control_s*.jsonl \
  --heldout-clumped /tmp/wgb-fit/trophic-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/trophic-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_trophic.txt

python3 python/analysis/fit_heterogeneity.py \
  --clumped /tmp/wgb-fit/frontier-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/frontier-fit/control_s*.jsonl \
  --heldout-clumped /tmp/wgb-fit/frontier-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/frontier-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_frontier.txt

python3 python/analysis/fit_heterogeneity.py \
  --clumped  /tmp/wgb-fit/trophic-fit/baseline_s*.jsonl /tmp/wgb-fit/frontier-fit/baseline_s*.jsonl \
  --equalized /tmp/wgb-fit/trophic-fit/control_s*.jsonl /tmp/wgb-fit/frontier-fit/control_s*.jsonl \
  --heldout-clumped  /tmp/wgb-fit/trophic-held/baseline_s*.jsonl /tmp/wgb-fit/frontier-held/baseline_s*.jsonl \
  --heldout-equalized /tmp/wgb-fit/trophic-held/control_s*.jsonl /tmp/wgb-fit/frontier-held/control_s*.jsonl \
  | tee /tmp/wgb-fit/fit_pooled.txt
```

The pooled fit's midpoint of the INTERSECTED margin is the adopted pair. If
the pooled (or either per-map) margin is CLOSED, the script says so: do NOT
move the constants; record the overlap in the console packet (Task 3.7) and
register scenario-conditional thresholds as a named deferred trigger — the
existing literals stay with a doc note. The build ships either way.

- [ ] **Step 8: re-derive the constants in `diagnostics.rs` (paste-only, single-cause commit)**

Edit `crates/jumpgate-core/src/diagnostics.rs:30-57`. The two literals and
the doc tables are PASTED from `/tmp/wgb-fit/fit_pooled.txt` (with the
per-map tables from `fit_trophic.txt` / `fit_frontier.txt`) — never typed
from memory, never nudged. Shape (the `«…»` slots are paste targets, the
golden discipline applied to calibration constants):

```rust
/// Minimum mean active-pirate-normalized HHI (milli) of robberies over
/// OCCUPIED routes for "risk heterogeneous". 1000 milli ≈ "each active pirate
/// owns one route"; even spread over m ≫ k routes reads ≈ 1000·k/m.
///
/// RE-FITTED «date» (dual-map labeled-run method, world-gets-big spec §8;
/// CAUSE: the phase-0a haven-lurk-leak fix changed the band the 2026-06-11
/// fit was measured on, and scenario_frontier joins the instrument's
/// domain; previous literal 2204). Fit seeds 7/23/42/99, held-out
/// 11/31/57/101, 50k ticks, baseline = TRUE-clumped vs hungry-roamer
/// control = TRUE-equalized; constants = FLOOR midpoint of the POOLED
/// intersected margin (python/analysis/fit_heterogeneity.py):
///   trophic  clumped «paste» vs equalized «paste»
///   frontier clumped «paste» vs equalized «paste»
///   pooled threshold = «paste»; held-out sides: «paste summary»
pub const HHI_NORM_MIN_MILLI: u64 = «paste pooled threshold»;
```

and the same treatment for `HOT_PERSISTENCE_SLACK_CHANGES` (previous
literal 3, excess tables pasted, slack = pasted pooled midpoint).

- [ ] **Step 9: re-check the classifier synthetics against the moved boundary**

```bash
cargo test -p jumpgate-core --lib diagnostics
```

Expected: all pass. If `cycling_heterogeneous_reads_alive` (:716),
`cycling_equalized_reads_risk_equalized` (:735), or
`sparse_clumped_minority_routes_read_heterogeneous` (:798) fail because the
re-fitted boundary crossed a synthetic's values: REDESIGN the synthetic to
sit inside the NEW measured band (e.g. concentrate `cycling_heterogeneous`'s
`robs_by_route` onto fewer routes — recompute its mean normalized HHI by the
Step-1 formula and place it above the new threshold with the same margin
the old builder had over 2204; the builder doc comment quotes the new band)
— never nudge the fitted constant back toward a synthetic. The synthetic is
the liar's regression fixture (the diagnostics.rs:896-899 house rule), so
its values must trace to the new fit table.

- [ ] **Step 10: full verification + single-cause commit**

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
git add crates/jumpgate-core/src/diagnostics.rs
git commit -F - <<'EOF'
lab(diagnostics): re-fit HHI/slack on the dual-map post-leak-fix labeled set

Single cause: phase-0a haven-lurk-leak fix invalidated the 2026-06-11
labeled fit (old literals 2204 / 3); re-derived by the same labeled-run
method over trophic+frontier with held-out seeds 11/31/57/101 (fit tables
pasted in the doc comments from fit_heterogeneity.py output). Frontier
positive control read RiskEqualized 4/4 BEFORE any frontier reading was
recorded. No config/state hash touched; zero goldens move.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

(If Step 7 reported a closed margin: the commit instead carries ONLY the doc
note recording the overlap + the deferred-trigger registration, with the
literals unchanged — adjust the message to say so.)

---

### Task 3.3: I1 — `haves_havenots.py` (workplace radius × hearing lag × end credits)

Spec §8 panels, W5. Per-hauler join of: workplace radius (mean endpoint
station radius over the hauler's accepted contracts — gossip-log `accept`
events, route decoded as `fr*n + tr` per `diagnostics::route_of`), hearing
lag (first hearing tick minus the alert's **BORN tick** — never the carried
`rob_tick`, which the inflation/lying channel corrupts), and end credits
(final window `per_craft_credits`). The PLAY-C3 confound is pre-registered
IN THE PANEL'S EMITTED TEXT, and the 6-station control is actually run.
Follows the `media_log.py` standalone-panel idioms (positional JSONL,
`load()` helper, "REPORTED, never gated" framing).

**Files**
- Create: `/home/john/jumpgate/python/analysis/haves_havenots.py`
- Create: `/home/john/jumpgate/python/tests/test_haves_havenots.py`

- [ ] **Step 1: failing test — born-tick join, radius math, rank correlation**

`python/tests/test_haves_havenots.py`:

```python
"""I1 panel math pins (world-gets-big spec §8 / W5)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import haves_havenots as hh


def test_hearing_lag_joins_on_born_tick_never_rob_tick():
    # The heard event carries a CORRUPTED rob_tick (inflation/lying stays
    # armed-dormant, spec §12): the lag must be 150-100=50, never touch 999.
    borns = [{"e": "born", "tick": 100, "alert": 1, "route": 0, "claimed": 5}]
    heards = [{
        "e": "heard", "tick": 150, "alert": 1, "carrier": "c3",
        "rob_tick": 999, "hops": 2, "claimed": 9,
    }]
    assert hh.hearing_lags(borns, heards) == {3: [50]}


def test_hearing_lag_takes_first_hearing_and_ignores_station_carriers():
    borns = [{"e": "born", "tick": 10, "alert": 7, "route": 0, "claimed": 5}]
    heards = [
        {"e": "heard", "tick": 90, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 40, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 20, "alert": 7, "carrier": "s0", "rob_tick": 10, "hops": 1, "claimed": 5},
    ]
    assert hh.hearing_lags(borns, heards) == {1: [30]}


def test_workplace_radius_is_floor_mean_of_accept_endpoints():
    # n=10 stations; route 29 = fr 2 -> to 9. radii in milli-AU.
    radii = [350, 444, 564, 716, 909, 1154, 1466, 1861, 2363, 3000]
    accepts = [
        {"e": "accept", "tick": 5, "route": 29, "hauler": 4},
        {"e": "accept", "tick": 9, "route": 29, "hauler": 4},
    ]
    # mean(564, 3000, 564, 3000) = 1782 exactly (FLOOR-safe case).
    assert hh.workplace_radius_milli(accepts, radii, 10) == {4: 1782}


def test_workplace_radius_skips_null_routes():
    assert hh.workplace_radius_milli(
        [{"e": "accept", "tick": 5, "route": None, "hauler": 0}], [1, 2], 2
    ) == {}


def test_spearman_perfect_monotone_and_tie_handling():
    assert hh.spearman([(1, 10), (2, 20), (3, 30)]) == 1.0
    assert hh.spearman([(1, 30), (2, 20), (3, 10)]) == -1.0
    assert hh.spearman([(1, 1), (1, 1), (1, 1)]) is None  # constant margin
    r = hh.spearman([(1, 10), (2, 10), (3, 30), (4, 40)])  # tied ys
    assert r is not None and 0.9 < r <= 1.0
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_haves_havenots.py`
Expected failure: `ModuleNotFoundError: No module named 'haves_havenots'`.

- [ ] **Step 2: implement `haves_havenots.py`**

```python
"""haves_havenots — I1 per-hauler knowledge-horizon panel (world-gets-big
spec §8 / W5).

FRAME (PDR-0006): every number printed here is a designer's WINDOW for the
console observe->steer->re-observe loop — never an acceptance gate.

Joins, PER HAULER:
  * workplace radius — FLOOR mean station radius (milli-AU) over the
    endpoint stations of the hauler's ACCEPTED contracts (gossip-log
    "accept" events; route = fr*n + tr per diagnostics::route_of),
  * hearing lag — first-hearing tick minus the alert's BORN tick. The join
    is on the BORN event tick, NEVER the carried rob_tick: the
    inflation/lying channel corrupts rob_tick and its consumers stay
    armed-dormant (spec §12),
  * end credits — final window per_craft_credits[row].

PRE-REGISTERED CONFOUND (PLAY-C3 / W5) — printed with every reading:
ASSIGN is position-blind, so a hauler's "workplace" is an emergent artifact
of dispatch order and the per-tier capacity ladder (qty 5/10/15 vs hull
capacity), never a chosen home. A radius x outcome correlation may be a
capacity-ladder read, not a locality read. The 6-station control run
(same panel on scenario_trophic, near-uniform radii) is the null map.

Usage:
    python3 python/analysis/haves_havenots.py GOSSIP_LOG \
        --windows RUN_JSONL --stdout RUN_STDOUT
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE  # phase-0b lockstep regex for the META line

CONFOUND_TEXT = (
    "PRE-REGISTERED CONFOUND (PLAY-C3 / W5): ASSIGN is position-blind — "
    "workplace radius is an artifact of dispatch order and the per-tier "
    "capacity ladder (qty 5/10/15 vs hull capacity), not a chosen home; a "
    "radius x outcome correlation may be a capacity-ladder read, not a "
    "locality read. Compare against the 6-station control (the null map). "
    "REPORTED, never gated — PDR-0006."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_meta(stdout_text):
    """META line -> dict (radii decoded to a list of ints); SystemExit when
    absent — a frontier run without META is a pre-phase-0b binary."""
    for line in stdout_text.splitlines():
        m = META_RE.match(line.strip())
        if m:
            d = m.groupdict()
            d["radii"] = [int(x) for x in d["radii"].strip("[]").split(",") if x]
            return d
    raise SystemExit("no META line in --stdout (re-run with the phase-0b+ runner)")


def workplace_radius_milli(accepts, radii, n_stations):
    """Per-hauler FLOOR-mean endpoint-station radius (milli-AU)."""
    per = {}
    for a in accepts:
        if a["route"] is None:
            continue
        fr, to = divmod(a["route"], n_stations)
        per.setdefault(a["hauler"], []).extend((radii[fr], radii[to]))
    return {c: sum(v) // len(v) for c, v in per.items()}


def hearing_lags(borns, heards):
    """Per-hauler first-hearing lags, joined on the alert's BORN tick (never
    the carried rob_tick — corruptible)."""
    born_tick = {b["alert"]: b["tick"] for b in borns}
    first = {}
    for h in heards:
        if not h["carrier"].startswith("c"):
            continue
        key = (h["carrier"], h["alert"])
        if key not in first or h["tick"] < first[key]:
            first[key] = h["tick"]
    lags = {}
    for (carrier, alert), t in sorted(first.items()):
        if alert in born_tick:
            lags.setdefault(int(carrier[1:]), []).append(t - born_tick[alert])
    return lags


def median(xs):
    s = sorted(xs)
    return s[(len(s) - 1) // 2]


def spearman(pairs):
    """Spearman rank correlation (mean ranks for ties); None when < 3 pairs
    or either margin is constant."""
    if len(pairs) < 3:
        return None

    def ranks(vals):
        order = sorted(range(len(vals)), key=lambda i: vals[i])
        r = [0.0] * len(vals)
        i = 0
        while i < len(order):
            j = i
            while j + 1 < len(order) and vals[order[j + 1]] == vals[order[i]]:
                j += 1
            mean_rank = (i + j) / 2 + 1
            for k in range(i, j + 1):
                r[order[k]] = mean_rank
            i = j + 1
        return r

    xs, ys = zip(*pairs)
    if len(set(xs)) < 2 or len(set(ys)) < 2:
        return None
    rx, ry = ranks(list(xs)), ranks(list(ys))
    n = len(pairs)
    mx, my = sum(rx) / n, sum(ry) / n
    cov = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    vx = sum((a - mx) ** 2 for a in rx)
    vy = sum((b - my) ** 2 for b in ry)
    if vx == 0 or vy == 0:
        return None
    return cov / (vx * vy) ** 0.5


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gossip_log", help="trophic_run --gossip-log output (JSONL)")
    ap.add_argument("--windows", required=True, help="the same run's --jsonl per-window file")
    ap.add_argument("--stdout", required=True, help="the same run's banked stdout (META source)")
    args = ap.parse_args()

    events = load(args.gossip_log)
    borns = [e for e in events if e["e"] == "born"]
    heards = [e for e in events if e["e"] == "heard"]
    accepts = [e for e in events if e["e"] == "accept"]
    windows = load(args.windows)
    meta = parse_meta(pathlib.Path(args.stdout).read_text())
    n_stations = int(meta["stations"])
    haulers = int(meta["haulers"])
    radii = meta["radii"]

    radius = workplace_radius_milli(accepts, radii, n_stations)
    lags = hearing_lags(borns, heards)
    final_credits = windows[-1]["per_craft_credits"] if windows else []

    print(
        f"haves_havenots (PDR-0006: windows, not gates) — scenario={meta['scenario']} "
        f"seed={meta['seed']} stations={n_stations} haulers={haulers}"
    )
    print("\n-- per-hauler knowledge horizon --")
    print("  row  accepts  workplace_milli_au  heard_n  median_lag  end_credits")
    rl_pairs, rc_pairs = [], []
    for row in range(haulers):
        r = radius.get(row)
        ls = lags.get(row, [])
        cred = final_credits[row] if row < len(final_credits) else None
        med = median(ls) if ls else None
        print(
            f"  {row:>3}  {sum(1 for a in accepts if a['hauler'] == row):>7}  "
            f"{r if r is not None else '-':>18}  {len(ls):>7}  "
            f"{med if med is not None else 'never-heard':>10}  {cred}"
        )
        if r is not None and med is not None:
            rl_pairs.append((r, med))
        if r is not None and cred is not None:
            rc_pairs.append((r, cred))
    never = [row for row in range(haulers) if not lags.get(row)]
    print(f"  never-heard haulers: {never}")
    sl = spearman(rl_pairs)
    sc = spearman(rc_pairs)
    print(
        f"\nspearman(workplace radius, median hearing lag)  = "
        f"{'n/a' if sl is None else f'{sl:.3f}'} over {len(rl_pairs)} haulers"
    )
    print(
        f"spearman(workplace radius, end credits)        = "
        f"{'n/a' if sc is None else f'{sc:.3f}'} over {len(rc_pairs)} haulers"
    )
    print(f"\n{CONFOUND_TEXT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_haves_havenots.py`
→ `5 passed`.

- [ ] **Step 4: PROCEDURE — run the 6-station control (the null map), then the frontier read**

The control runs FIRST and is recorded beside every frontier I1 reading
(media on via knobs — `MediaCfg` live-start caps 16/8):

```bash
mkdir -p /tmp/wgb-i1
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario trophic --seed 7 --ticks 50000 \
  --set station_gossip_slots=16 --set craft_gossip_slots=8 \
  --jsonl /tmp/wgb-i1/trophic_s7.jsonl --gossip-log /tmp/wgb-i1/trophic_s7.gossip.jsonl \
  > /tmp/wgb-i1/trophic_s7.stdout
python3 python/analysis/haves_havenots.py /tmp/wgb-i1/trophic_s7.gossip.jsonl \
  --windows /tmp/wgb-i1/trophic_s7.jsonl --stdout /tmp/wgb-i1/trophic_s7.stdout \
  | tee /tmp/wgb-i1/i1_trophic_control.txt

cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 50000 \
  --set station_gossip_slots=16 --set craft_gossip_slots=8 \
  --jsonl /tmp/wgb-i1/frontier_s7.jsonl --gossip-log /tmp/wgb-i1/frontier_s7.gossip.jsonl \
  > /tmp/wgb-i1/frontier_s7.stdout
python3 python/analysis/haves_havenots.py /tmp/wgb-i1/frontier_s7.gossip.jsonl \
  --windows /tmp/wgb-i1/frontier_s7.jsonl --stdout /tmp/wgb-i1/frontier_s7.stdout \
  | tee /tmp/wgb-i1/i1_frontier_s7.txt
```

Expected control shape (recorded, not gated): trophic radii near-uniform →
tiny radius spread, correlations ≈ n/a or ≈ 0. Both outputs are banked by
Task 3.7.

- [ ] **Step 5: commit**

```bash
git add python/analysis/haves_havenots.py python/tests/test_haves_havenots.py
git commit -F - <<'EOF'
lab(panel): I1 haves_havenots — workplace radius x hearing lag x credits

Per-hauler knowledge-horizon join (spec s8/W5): radius from accept-event
endpoints, lag joined on the alert's BORN tick (never the corruptible
rob_tick), end credits from the final window. PLAY-C3 position-blind-
dispatch confound pre-registered in the panel's emitted text; the
6-station control is the recorded null map. Windows, never gates.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.4: coverage re-denominated to contract-endpoint stations (scenario-conditional)

Spec §8: the dark haven (station 6, no contract endpoint) makes the
CommonKnowledge all-stations coverage check
(`stations_with_news as usize == per_station_alerts.len()`,
diagnostics.rs:426) structurally unsatisfiable on frontier. Re-denominate to
contract-endpoint stations, derived from the run's own config
(`RunConfig.contracts: Vec<ContractInit>` with
`from_station_index`/`to_station_index`, config.rs:126-135, 426). On
`scenario_trophic` every station is an endpoint (sources {0,1,2}, dests
{3,4,5}, fuel sinks {0,1,2} — scenario.rs:186-211), so banked trophic MEDIA
readings are unchanged; that invariance is pinned by test.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/src/diagnostics.rs`
  (`media_classify` :400-431; new `endpoint_station_rows`; media tests
  :929-1000)
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs`
  (`simulate` cfg block :113-136, MEDIA println :410-418, replay arm
  destructure :456-462)

- [ ] **Step 1: failing test — endpoint denomination + trophic invariance**

Add to the diagnostics test module (beside the media tests, after
`media_localized_is_the_alive_reading`):

```rust
    #[test]
    fn media_common_knowledge_denominates_on_contract_endpoint_stations() {
        // Frontier shape: a DARK station (no contract endpoint — the haven)
        // never holds news; coverage must be satisfiable over the ENDPOINT
        // set (spec §8). Escape 23/24 = 958‰ ≥ 950; alerts on rows 0,1 only.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 0],
                )
            })
            .collect();
        // All-stations denominator (the old read): the dark row blocks it.
        assert_eq!(
            media_classify(&samples, &[true, true, true]),
            MediaReading::Localized,
            "dark row counted -> coverage unsatisfiable"
        );
        // Endpoint denominator: row 2 is no contract endpoint.
        assert_eq!(
            media_classify(&samples, &[true, true, false]),
            MediaReading::CommonKnowledge,
            "endpoint coverage complete -> CommonKnowledge"
        );
    }

    #[test]
    fn media_empty_endpoint_set_never_reads_common_knowledge() {
        // A degenerate all-dark mask must not make coverage vacuously true.
        let samples: Vec<TrophicSample> = (0..12u64)
            .map(|w| {
                let born_cum = ((w + 1) * 2) as u32;
                m(
                    (w + 1) * WINDOW_TICKS,
                    2,
                    3,
                    1,
                    6,
                    born_cum,
                    born_cum.saturating_sub(1),
                    &[2, 1, 1],
                )
            })
            .collect();
        assert_eq!(
            media_classify(&samples, &[false, false, false]),
            MediaReading::Localized
        );
    }

    #[test]
    fn endpoint_station_rows_trophic_is_all_true() {
        // scenario_trophic: sources {0,1,2}, dests {3,4,5}, fuel sinks
        // {0,1,2} — every station is a contract endpoint, so the banked
        // trophic MEDIA readings are unchanged by the re-denomination.
        let cfg = crate::scenario::scenario_trophic(7);
        assert_eq!(endpoint_station_rows(&cfg), vec![true; 6]);
    }

    #[test]
    fn endpoint_station_rows_marks_a_contractless_station_dark() {
        let mut cfg = crate::scenario::scenario_trophic(7);
        cfg.stations.push(crate::config::StationInit {
            body_index: 1,
            initial_stock: [0, 0],
            initial_price_micros: [0, 0],
            sells_upgrades: false,
        });
        let rows = endpoint_station_rows(&cfg);
        assert_eq!(rows.len(), 7);
        assert!(!rows[6], "no contract touches the new station -> dark");
    }
```

Update the six existing `media_*` tests (:929-1000) mechanically: every
`media_classify(&samples)` becomes
`media_classify(&samples, &[true, true, true])` (their synthetic
`per_station_alerts` are 3-wide; the all-true mask is the old behavior).

Run: `cargo test -p jumpgate-core --lib media_`
Expected failure: compile error
`error[E0061]: this function takes 1 argument but 2 arguments were supplied`
(at the first updated call site) and
`cannot find function 'endpoint_station_rows' in this scope`.

- [ ] **Step 2: implement in `diagnostics.rs`**

New pub fn (beside `route_of`):

```rust
/// Contract-endpoint station rows derived from the run's own config: row i
/// is `true` iff some seeded contract has it as `from_station_index` or
/// `to_station_index`. The coverage denominator for `media_classify` —
/// scenario-conditional by construction (spec §8: the dark haven makes
/// all-stations coverage structurally unsatisfiable on frontier; on
/// scenario_trophic every station is an endpoint, so readings are
/// unchanged — pinned by test).
pub fn endpoint_station_rows(cfg: &crate::config::RunConfig) -> Vec<bool> {
    let mut rows = vec![false; cfg.stations.len()];
    for k in &cfg.contracts {
        if let Some(r) = rows.get_mut(k.from_station_index) {
            *r = true;
        }
        if let Some(r) = rows.get_mut(k.to_station_index) {
            *r = true;
        }
    }
    rows
}
```

`media_classify` signature + CommonKnowledge clause (NoMedia / NewsDesert /
StaleEcho arms untouched; `stations_with_news` STAYS sampled and in JSONL —
the additive law — the classifier just stops consuming it):

```rust
/// Classify a windowed run's MEDIA propagation field (spec §9). Pure over
/// the samples, like `classify`; precedence is the listed reading order.
/// `endpoint_rows` is the coverage denominator (world-gets-big spec §8):
/// CommonKnowledge requires news at every CONTRACT-ENDPOINT station, not
/// every station — the dark haven is structurally newsless.
pub fn media_classify(samples: &[TrophicSample], endpoint_rows: &[bool]) -> MediaReading {
```

and replace the CommonKnowledge `if` (:423-429) with:

```rust
    let last = samples.last().expect("non-empty: born > 0");
    let endpoints: Vec<usize> = endpoint_rows
        .iter()
        .enumerate()
        .filter_map(|(i, &e)| e.then_some(i))
        .collect();
    if escaped_milli(samples) >= COMMON_KNOWLEDGE_ESCAPE_MILLI
        && !last.per_station_alerts.is_empty()
        && !endpoints.is_empty()
        && endpoints
            .iter()
            .all(|&i| last.per_station_alerts.get(i).copied().unwrap_or(0) > 0)
    {
        return MediaReading::CommonKnowledge;
    }
    MediaReading::Localized
```

- [ ] **Step 3: thread the mask through `trophic_run.rs`**

`simulate` already owns the cfg; return the mask rather than rebuilding cfg
in `main` (self-contained regardless of the phase-2 `--scenario` dispatch
shape). After the `apply_knob` loop and before `World::reset`:

```rust
    let endpoint_rows = diagnostics::endpoint_station_rows(&cfg);
```

extend `simulate`'s return tuple with `endpoint_rows` (last position), and
update both call sites:

```rust
    let (samples, hashes, world, endpoint_rows) = match simulate(&args, Some(&mut jsonl_sink)) {
```

(replay arm: `let (_, hashes2, _, _) = match simulate(&args, None)`), then
the MEDIA println (:417) passes it:

```rust
        diagnostics::media_classify(&samples, &endpoint_rows),
```

(Exact tuple shape: match whatever `simulate` returns after phases 0b–2;
the mask is appended, nothing else moves.)

- [ ] **Step 4: run + expected pass, full suite, clippy**

```bash
cargo test -p jumpgate-core --lib media_     # all media_* + 2 endpoint tests pass
cargo test -p jumpgate-core --lib endpoint_  # 2 passed
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Also verify the trophic invariance live (recorded): re-run one banked
baseline seed and diff its MEDIA line against the phase-0b banked stdout —
expected byte-identical.

- [ ] **Step 5: commit**

```bash
git add crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/examples/trophic_run.rs
git commit -F - <<'EOF'
lab(media): coverage denominated over contract-endpoint stations

CommonKnowledge's coverage check counted ALL stations; the frontier dark
haven (no contract endpoint, spec s3) made it structurally unsatisfiable.
media_classify now takes an endpoint mask derived from the run's own
config (endpoint_station_rows over RunConfig.contracts). scenario_trophic
is all-endpoints, so banked trophic readings are unchanged (pinned by
test). stations_with_news stays sampled — JSONL keys untouched.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.5: I2 — radial-zone panel, `fuel_starve` discriminator, per-row refuel fill share

Spec §8 / W10. Two parts: (a) a tiny runner addition — `Refueled` events
written to the `--gossip-log` JSONL (the run's only machine-readable
per-event surface; the lockstep rule: emit + reader land in this one
commit); (b) `radial_zones.py`, the standalone zone panel.

**Files**
- Modify: `/home/john/jumpgate/crates/jumpgate-core/examples/trophic_run.rs`
  (`write_gossip_log` match :190-236)
- Create: `/home/john/jumpgate/python/analysis/radial_zones.py`
- Create: `/home/john/jumpgate/python/tests/test_radial_zones.py`

- [ ] **Step 1: failing test — discriminator readings, fill-share FLOOR math, zone series**

`python/tests/test_radial_zones.py`:

```python
"""I2 radial-zone panel pins (world-gets-big spec §8 / W10)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import radial_zones as rz


def test_fuel_starve_readings_cover_the_pre_registered_table():
    # NoStockout: never dry.
    assert rz.fuel_starve([5, 4, 3, 2], [9, 9, 9, 9]) == "NoStockout"
    # BoomBust: dry then RECOVERED and ends wet.
    assert rz.fuel_starve([5, 0, 3, 4], [9, 5, 7, 8]) == "BoomBust"
    # DeathSpiral: ends dry AND final-quarter traffic collapsed
    # (quarters [9,9,1,0]; 0*4 < 9).
    assert rz.fuel_starve([5, 2, 0, 0], [9, 9, 1, 0]) == "DeathSpiral"
    # Stockout (residual): ends dry but the lane is still alive.
    assert rz.fuel_starve([5, 0, 0, 0], [5, 5, 5, 5]) == "Stockout"
    # ShortRun: quarters undefined under 4 windows.
    assert rz.fuel_starve([5, 0, 1], [1, 1, 1]) == "ShortRun"


def test_fill_share_is_floor_permille_of_remaining_headroom():
    # before 250 -> after 850: (850-250)*1000 // (1000-250) = 800.
    ev = {"e": "refuel", "tick": 9, "craft": 4, "station": 2,
          "units": 12, "price_micros": 7, "before_permille": 250,
          "after_permille": 850}
    assert rz.fill_permille(ev) == 800
    # Full tank before (defensive: resolve_refuels skips these) -> None.
    full = dict(ev, before_permille=1000, after_permille=1000)
    assert rz.fill_permille(full) is None


def test_zone_series_sums_stock_and_routes_touching_the_zone():
    w = {
        "tick": 2000,
        "per_route_robs": [0, 1, 0, 2],          # n=2 stations
        "per_route_traffic": [3, 1, 0, 5],
        "per_station_fuel_stock": [7, 11],
        "per_station_fuel_price": [5000, 9000],
        "per_station_alerts": [1, 0],
    }
    z = rz.zone_series([w], [1])  # zone = station 1 only
    # routes touching station 1: 0->1 (idx 1), 1->0 (idx 2), 1->1 (idx 3).
    assert z == [{
        "tick": 2000, "traffic": 6, "robs": 3,
        "stock": 11, "price_max": 9000, "alerts": 0,
    }]


def test_parse_zones():
    assert rz.parse_zones("0,1,2|3,4,5|6|7,8,9") == [
        [0, 1, 2], [3, 4, 5], [6], [7, 8, 9]
    ]
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_radial_zones.py`
Expected failure: `ModuleNotFoundError: No module named 'radial_zones'`.

- [ ] **Step 2: the `Refueled` gossip-log arm (runner)**

In `write_gossip_log`'s match (trophic_run.rs, after the `ContractAccepted`
arm, before the `_ => continue` catch-all — the catch-all is exactly why
this arm must be added explicitly, the chronicle_subject lesson):

```rust
            EventKind::Refueled {
                craft,
                station,
                units,
                price_micros,
                tank_before_permille,
                tank_after_permille,
            } => serde_json::json!({
                "e": "refuel", "tick": e.tick.0, "craft": craft.slot,
                "station": station.slot, "units": units,
                "price_micros": price_micros,
                "before_permille": tank_before_permille,
                "after_permille": tank_after_permille,
            }),
```

Update the fn doc comment's event list to name "Refueled (`refuel`)". This
is additive: trophic logs (RefuelCfg default-off) simply contain no
`refuel` lines, and `media_log.py` filters by `e` so it is untouched.

- [ ] **Step 3: implement `radial_zones.py`**

```python
"""radial_zones — I2 per-zone boom-bust / fuel-scarcity panel
(world-gets-big spec §8 / W10).

FRAME (PDR-0006): windows, never gates. The fuel_starve discriminator is
the OD-4 measurement — EITHER answer is a finding:
  NoStockout  — zone fuel stock never hit 0,
  BoomBust    — stock hit 0, RECOVERED (>0 later), and ends >0 — the
                scarcity arc cycles like the predation arc,
  DeathSpiral — ends at 0 AND final-quarter zone traffic*4 < the peak
                quarter — the lane died with the fuel,
  Stockout    — residual: ends dry but the lane is still alive (scarcity
                without collapse, incl. recovered-then-dry tails),
  ShortRun    — fewer than 4 windows (quarters undefined).

Fill share (per craft row): resolve_refuels fills in DENSE CRAFT-ROW ORDER —
under scarce stock low rows drink first, so a fill-share gradient by row is
a RATIONING ARTIFACT (row-order rationing, spec §8), not strategy. Named in
the emitted text.

Zone defaults are the frontier tiers (spec §3): core 0,1,2 | mid 3,4,5 |
haven 6 | frontier 7,8,9. Scenario-conditional: pass --zones for other maps
(e.g. --zones "0,1,2|3,4,5" on the 6-station control).

Usage:
    python3 python/analysis/radial_zones.py RUN_JSONL \
        --gossip-log GOSSIP_JSONL [--zones "0,1,2|3,4,5|6|7,8,9"]
"""
import argparse
import json

ZONE_NAMES = ["core", "mid", "haven", "frontier"]

RATIONING_TEXT = (
    "row-order rationing (RECORDED, never gated — PDR-0006): resolve_refuels "
    "fills in dense craft-row order; under scarce stock low rows drink "
    "first, so a fill-share gradient by row is a rationing artifact, not "
    "strategy."
)


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_zones(spec):
    return [[int(x) for x in part.split(",") if x] for part in spec.split("|")]


def zone_series(windows, zone):
    """Per-window zone aggregates: traffic/robs over routes TOUCHING the
    zone (either endpoint), summed fuel stock, max fuel price, alerts."""
    out = []
    zset = set(zone)
    for w in windows:
        n = len(w["per_station_fuel_stock"])
        traffic = robs = 0
        for fr in range(n):
            for to in range(n):
                if fr in zset or to in zset:
                    traffic += w["per_route_traffic"][fr * n + to]
                    robs += w["per_route_robs"][fr * n + to]
        out.append({
            "tick": w["tick"],
            "traffic": traffic,
            "robs": robs,
            "stock": sum(w["per_station_fuel_stock"][s] for s in zone),
            "price_max": max((w["per_station_fuel_price"][s] for s in zone), default=0),
            "alerts": sum(w["per_station_alerts"][s] for s in zone)
            if w["per_station_alerts"] else 0,
        })
    return out


def fuel_starve(stock, traffic):
    """The OD-4 death-spiral-vs-boom-bust discriminator (docstring table)."""
    if len(stock) < 4:
        return "ShortRun"
    if 0 not in stock:
        return "NoStockout"
    q = len(traffic) // 4
    quarters = [sum(traffic[i * q:(i + 1) * q]) for i in range(4)]
    if stock[-1] == 0 and quarters[3] * 4 < max(quarters):
        return "DeathSpiral"
    first_zero = stock.index(0)
    if stock[-1] > 0 and any(s > 0 for s in stock[first_zero:]):
        return "BoomBust"
    return "Stockout"


def fill_permille(ev):
    """FLOOR permille of remaining headroom this refuel filled; None when
    the tank was already full (resolve_refuels skips those — defensive)."""
    before, after = ev["before_permille"], ev["after_permille"]
    if before >= 1000:
        return None
    return (after - before) * 1000 // (1000 - before)


def fill_share_rows(refuels):
    """Per craft row: (n events, total units, median fill permille)."""
    per = {}
    for ev in refuels:
        f = fill_permille(ev)
        if f is None:
            continue
        per.setdefault(ev["craft"], {"n": 0, "units": 0, "fills": []})
        per[ev["craft"]]["n"] += 1
        per[ev["craft"]]["units"] += ev["units"]
        per[ev["craft"]]["fills"].append(f)
    rows = []
    for craft in sorted(per):
        fills = sorted(per[craft]["fills"])
        rows.append((craft, per[craft]["n"], per[craft]["units"],
                     fills[(len(fills) - 1) // 2]))
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("windows", help="trophic_run --jsonl per-window file")
    ap.add_argument("--gossip-log", help="same run's --gossip-log (refuel events)")
    ap.add_argument("--zones", default="0,1,2|3,4,5|6|7,8,9")
    args = ap.parse_args()

    windows = load(args.windows)
    zones = parse_zones(args.zones)
    refuels = (
        [e for e in load(args.gossip_log) if e["e"] == "refuel"]
        if args.gossip_log else []
    )
    print(
        f"radial_zones (PDR-0006: windows, not gates) — {len(windows)} windows, "
        f"{len(refuels)} refuel events, zones={args.zones}"
    )
    for zi, zone in enumerate(zones):
        name = ZONE_NAMES[zi] if zi < len(ZONE_NAMES) else f"zone{zi}"
        series = zone_series(windows, zone)
        stock = [s["stock"] for s in series]
        traffic = [s["traffic"] for s in series]
        reading = fuel_starve(stock, traffic)
        print(f"\n-- zone {name} (stations {zone}) — fuel_starve={reading} "
              "(RECORDED; either answer is a finding — OD-4) --")
        print("  window_close  traffic  robs  fuel_stock  price_max  alerts")
        for s in series:
            print(
                f"  {s['tick']:>12}  {s['traffic']:>7}  {s['robs']:>4}  "
                f"{s['stock']:>10}  {s['price_max']:>9}  {s['alerts']:>6}"
            )
    print("\n-- per-row refuel fill share --")
    rows = fill_share_rows(refuels)
    if not rows:
        print("  no refuel events (zero-refuel sentinel — the MEDIA precedent)")
    else:
        print("  row  refuels  units  median_fill_permille")
        for craft, n, units, med in rows:
            print(f"  {craft:>3}  {n:>7}  {units:>5}  {med:>20}")
    print(f"  {RATIONING_TEXT}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: run + expected pass; live verification of the refuel lines**

```bash
PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_radial_zones.py   # 4 passed
cargo test --workspace                       # runner still compiles, nothing regressed
cargo clippy --all-targets -- -D warnings
# Lockstep verification (recorded): a frontier run emits refuel lines, a
# trophic run emits none (RefuelCfg default-off inertness).
mkdir -p /tmp/wgb-i2
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 50000 \
  --jsonl /tmp/wgb-i2/frontier_s7.jsonl --gossip-log /tmp/wgb-i2/frontier_s7.gossip.jsonl \
  > /tmp/wgb-i2/frontier_s7.stdout
grep -c '"e":"refuel"' /tmp/wgb-i2/frontier_s7.gossip.jsonl     # expected: > 0
python3 python/analysis/radial_zones.py /tmp/wgb-i2/frontier_s7.jsonl \
  --gossip-log /tmp/wgb-i2/frontier_s7.gossip.jsonl | tee /tmp/wgb-i2/i2_frontier_s7.txt
```

- [ ] **Step 5: commit (emit + reader, one lockstep commit)**

```bash
git add crates/jumpgate-core/examples/trophic_run.rs \
        python/analysis/radial_zones.py python/tests/test_radial_zones.py
git commit -F - <<'EOF'
lab(panel): I2 radial zones — fuel_starve discriminator + refuel fill share

Refueled events now flow to --gossip-log (explicit arm past the
catch-all; additive — trophic logs stay byte-identical, RefuelCfg off).
radial_zones.py reads per-zone traffic/robs/fuel stock/price/alerts and
prints the OD-4 death-spiral-vs-boom-bust discriminator (pre-registered
reading table pinned by pytest) plus per-row fill share with row-order
rationing named in the emitted text. Windows, never gates.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.6: the headline 20-seed × 6-arm grid (W4 flip-share VALUE, rate-normalized cross-map reads)

§11 W4: "flip-share VALUE: gossip-vs-ring over 20 clean seeds × both anchor
arms". The grid runs on `scenario_frontier`. **The six arms (pre-registered;
every knob set EXPLICITLY so arm identity never rides a factory default):**

| arm | `hauler_belief_scoring` | `station_gossip_slots` | `craft_gossip_slots` | `staleness_from_rob_tick` | active ASSIGN read |
|---|---|---|---|---|---|
| `blind-born`  | false | 0  | 0 | false | none (reward argmax) |
| `blind-rob`   | false | 0  | 0 | true  | none — A/A twin |
| `ring-born`   | true  | 0  | 0 | false | RouteEvidence ring |
| `ring-rob`    | true  | 0  | 0 | true  | ring — A/A twin |
| `gossip-born` | true  | 16 | 8 | false | gossip, born/heard-anchored staleness |
| `gossip-rob`  | true  | 16 | 8 | true  | gossip, rob-tick-anchored staleness |

The anchor knob is consumed ONLY on the gossip read (economy.rs:585-612
swapped read: `buf.count_route_recent(..., staleness_from_rob_tick)`;
media-off falls back to the ring, blind skips scoring entirely). The
blind/ring born-vs-rob twins are therefore **A/A instrument controls**:
identical dynamics expected; any RESULT divergence = wiring bug — fix the
instrument before reading W4 (recorded, not a ship gate). Gossip caps 16/8
are the `MediaCfg` documented live-start values (config.rs:359-362).

**Pre-registered:** 20 seeds = `7 11 13 23 29 31 37 41 42 43 47 53 57 59 61
67 71 73 99 101`. CLEAN seed = `blind-born` verdict != `PermanentPeace`
(the no-information baseline defines the peace basin — the media rung's
basin-chaos lesson). W4 value read = gossip−ring delta in median final
hauler credits per anchor over clean seeds; registered alternative: "mixing
persists → the deferred dispatch-locality lever" (spec §12). Cross-map reads
vs the banked post-fix trophic baseline are rate-normalized
distribution-vs-distribution, NEVER same-seed paired (GEO-C3).

**Files**
- Create: `/home/john/jumpgate/python/analysis/w4_grid.py`
- Create: `/home/john/jumpgate/python/tests/test_w4_grid.py`

- [ ] **Step 1: failing test — clean rule, A/A comparator, value/rate math**

`python/tests/test_w4_grid.py`:

```python
"""W4 grid reader pins (world-gets-big spec §8/§11 W4)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import w4_grid


def cell(verdict="Alive", robs="6", trips="40", credits=None, haulers=3):
    return {
        "result": {
            "seed": "7", "ticks": "50000", "verdict": verdict, "cycled": "true",
            "hetero": "true", "disperse": "true", "fuel_empty": "0",
            "robs": robs, "trips": trips, "purchases": "2",
        },
        "meta": {"haulers": str(haulers)},
        "windows": [{"per_craft_credits": credits or [30, 10, 20, 999]}],
    }


def test_clean_seeds_rule_is_blind_born_not_permanent_peace():
    cells = {
        ("blind-born", 7): cell(verdict="Alive"),
        ("blind-born", 11): cell(verdict="PermanentPeace"),
        # A peace reading on ANOTHER arm must not dirty the seed.
        ("gossip-born", 7): cell(verdict="PermanentPeace"),
    }
    assert w4_grid.clean_seeds(cells, [7, 11]) == [7]


def test_run_value_takes_hauler_slice_median_and_per_trip_rate():
    v = w4_grid.run_value(cell())
    # hauler rows 0..3 -> [30, 10, 20]; median 20; 20*1000 // 40 trips = 500.
    assert v == {"median_credits": 20, "laden_trips": 40, "credits_per_trip_milli": 500}


def test_aa_twin_divergence_names_the_differing_fields():
    cells = {
        ("blind-born", 7): cell(robs="6"),
        ("blind-rob", 7): cell(robs="7"),
        ("ring-born", 7): cell(),
        ("ring-rob", 7): cell(),
    }
    bad = w4_grid.aa_twin_divergences(cells, [7])
    assert len(bad) == 1
    fam, seed, fields = bad[0]
    assert (fam, seed) == ("blind", 7)
    assert fields == {"robs": ("6", "7")}


def test_quartiles_are_lower_index_integers():
    assert w4_grid.quartiles([5, 1, 9, 3, 7]) == (3, 5, 7)
```

Run: `PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_w4_grid.py`
Expected failure: `ModuleNotFoundError: No module named 'w4_grid'`.

- [ ] **Step 2: implement `w4_grid.py`**

```python
"""w4_grid — headline 20-seed x 6-arm frontier grid reader
(world-gets-big spec §8 ensembles / §11 W4).

FRAME (PDR-0006): REPORTED, NEVER GATED. The W4 question: does the
gossip-vs-ring flip share finally carry VALUE now the world is bigger than
the news? Registered alternative: mixing persists -> the deferred
dispatch-locality lever (spec §12).

THE SIX ARMS (pre-registered; anchor = staleness_from_rob_tick, consumed
ONLY on the gossip read — blind/ring born-vs-rob twins are A/A instrument
controls; any RESULT divergence is a wiring bug to fix BEFORE reading W4):
  blind-born blind-rob ring-born ring-rob gossip-born gossip-rob

CLEAN SEED (pre-registered): blind-born verdict != PermanentPeace — the
no-information baseline defines the peace basin. Value reads pool CLEAN
seeds; the all-seeds pool is printed beside as context.

CROSS-MAP (GEO-C3): vs the banked post-fix trophic baseline, rate-normalized
distribution-vs-distribution (credits per laden trip, robs per 1000 laden
trips) — NEVER same-seed paired deltas; scenario_frontier is a new world.

Usage:
    python3 python/analysis/w4_grid.py /tmp/wgb-grid \
        [--seeds 7 11 ...] [--compare /tmp/wgb-trophic-baseline]
"""
import argparse
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from sweep_trophic import META_RE, RESULT_RE

ARMS = ["blind-born", "blind-rob", "ring-born", "ring-rob", "gossip-born", "gossip-rob"]

# The exact sweep --knobset strings (kept here so the grid is re-runnable
# from this file's docstring alone; every knob explicit).
ARM_KNOBSETS = [
    "blind-born:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "blind-rob:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "ring-born:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false",
    "ring-rob:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true",
    "gossip-born:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=false",
    "gossip-rob:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=true",
]

SEEDS = [7, 11, 13, 23, 29, 31, 37, 41, 42, 43, 47, 53, 57, 59, 61, 67, 71, 73, 99, 101]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def load_cell(out_dir, arm, seed):
    """One (arm, seed) cell from the sweep's banked stdout + windows."""
    stdout = (out_dir / f"{arm}_s{seed}.stdout").read_text()
    result = meta = None
    for line in stdout.splitlines():
        m = RESULT_RE.match(line.strip())
        if m:
            result = m.groupdict()
        m = META_RE.match(line.strip())
        if m:
            meta = m.groupdict()
    if result is None:
        raise SystemExit(f"no RESULT in {arm}_s{seed}.stdout")
    return {
        "result": result,
        "meta": meta,
        "windows": load(out_dir / f"{arm}_s{seed}.jsonl"),
    }


def clean_seeds(cells, seeds):
    """CLEAN = blind-born verdict != PermanentPeace (pre-registered)."""
    return [
        s for s in seeds
        if cells[("blind-born", s)]["result"]["verdict"] != "PermanentPeace"
    ]


def run_value(cell):
    """Median final hauler credits + rate per laden trip (milli) for one run.
    The hauler count comes from META — never a mirrored constant."""
    haulers = int(cell["meta"]["haulers"])
    creds = sorted(cell["windows"][-1]["per_craft_credits"][:haulers])
    med = creds[(len(creds) - 1) // 2]
    trips = int(cell["result"]["trips"])
    return {
        "median_credits": med,
        "laden_trips": trips,
        "credits_per_trip_milli": med * 1000 // max(trips, 1),
    }


def aa_twin_divergences(cells, seeds):
    """blind/ring born-vs-rob twins must have IDENTICAL RESULT dicts (the
    anchor is unread off the gossip path). Returns the divergences."""
    bad = []
    for fam in ("blind", "ring"):
        for s in seeds:
            a = cells.get((f"{fam}-born", s))
            b = cells.get((f"{fam}-rob", s))
            if a is None or b is None:
                continue
            ar, br = a["result"], b["result"]
            if ar != br:
                bad.append((fam, s, {k: (ar[k], br[k]) for k in ar if ar[k] != br[k]}))
    return bad


def quartiles(xs):
    s = sorted(xs)
    n = len(s)
    return s[(n - 1) // 4], s[(n - 1) // 2], s[(3 * (n - 1)) // 4]


def arm_table(cells, seeds, label):
    print(f"\n-- per-arm value table ({label}: {len(seeds)} seeds) --")
    print("  arm          med(median_credits)  med(cr/trip_milli)  flips/decisions(pooled)  readings")
    out = {}
    for arm in ARMS:
        vals = [run_value(cells[(arm, s)]) for s in seeds]
        med_c = quartiles([v["median_credits"] for v in vals])[1] if vals else None
        med_r = quartiles([v["credits_per_trip_milli"] for v in vals])[1] if vals else None
        flips = decisions = 0
        readings = {}
        for s in seeds:
            ws = cells[(arm, s)]["windows"]
            if ws:
                flips += ws[-1]["assign_flips_cum"]
                decisions += ws[-1]["assign_decisions_cum"]
            v = cells[(arm, s)]["result"]["verdict"]
            readings[v] = readings.get(v, 0) + 1
        out[arm] = {"med_credits": med_c, "med_rate": med_r}
        print(f"  {arm:<12} {med_c!s:>19}  {med_r!s:>18}  {flips}/{decisions:<22} {readings}")
    for anchor in ("born", "rob"):
        g, r = out[f"gossip-{anchor}"], out[f"ring-{anchor}"]
        if g["med_credits"] is not None and r["med_credits"] is not None:
            print(
                f"  W4 VALUE delta ({anchor}-anchor): gossip-ring = "
                f"{g['med_credits'] - r['med_credits']} micros median final hauler "
                f"credits ({g['med_rate'] - r['med_rate']} milli per laden trip) — "
                "REPORTED, NEVER GATED; registered alternative: mixing persists "
                "-> the deferred dispatch-locality lever (spec §12)"
            )
    return out


def map_distributions(stdout_dir, pattern):
    """Per-run rate reads for one map dir of banked stdouts (cross-map)."""
    rates, rob_rates = [], []
    for p in sorted(pathlib.Path(stdout_dir).glob(pattern)):
        result = meta = None
        windows_path = p.with_suffix(".jsonl")
        for line in p.read_text().splitlines():
            m = RESULT_RE.match(line.strip())
            if m:
                result = m.groupdict()
            m = META_RE.match(line.strip())
            if m:
                meta = m.groupdict()
        if result is None or meta is None or not windows_path.exists():
            continue
        ws = load(windows_path)
        if not ws:
            continue
        haulers = int(meta["haulers"])
        creds = sorted(ws[-1]["per_craft_credits"][:haulers])
        med = creds[(len(creds) - 1) // 2]
        trips = max(int(result["trips"]), 1)
        rates.append(med * 1000 // trips)
        rob_rates.append(int(result["robs"]) * 1000 // trips)
    return rates, rob_rates


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("out_dir", help="the grid sweep's --out directory")
    ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
    ap.add_argument(
        "--compare",
        help="banked post-fix trophic baseline sweep dir (cross-map read; "
        "rate-normalized distribution-vs-distribution, NEVER paired — GEO-C3)",
    )
    args = ap.parse_args()
    out_dir = pathlib.Path(args.out_dir)

    cells = {
        (arm, s): load_cell(out_dir, arm, s) for arm in ARMS for s in args.seeds
    }
    print(
        "w4_grid (PDR-0006: REPORTED, NEVER GATED) — "
        f"{len(ARMS)} arms x {len(args.seeds)} seeds"
    )
    bad = aa_twin_divergences(cells, args.seeds)
    if bad:
        print("\nA/A TWIN DIVERGENCE (wiring bug — fix the instrument BEFORE reading W4):")
        for fam, s, fields in bad:
            print(f"  {fam} seed={s}: {fields}")
    else:
        print("A/A twins (blind, ring): born-vs-rob RESULT identical on every seed (instrument sound)")

    clean = clean_seeds(cells, args.seeds)
    dirty = [s for s in args.seeds if s not in clean]
    print(f"\nclean seeds (blind-born != PermanentPeace): {len(clean)}/{len(args.seeds)}; dirty={dirty}")
    arm_table(cells, clean, "CLEAN")
    arm_table(cells, args.seeds, "ALL (context)")

    if args.compare:
        fr, fr_robs = map_distributions(out_dir, "gossip-born_s*.stdout")
        tr, tr_robs = map_distributions(args.compare, "baseline_s*.stdout")
        print(
            "\n-- cross-map (rate-normalized, distribution-vs-distribution; "
            "NEVER same-seed paired — GEO-C3) --"
        )
        print(f"  frontier gossip-born: credits/trip milli quartiles={quartiles(fr) if fr else None} "
              f"robs/1000trips quartiles={quartiles(fr_robs) if fr_robs else None}")
        print(f"  trophic  baseline:    credits/trip milli quartiles={quartiles(tr) if tr else None} "
              f"robs/1000trips quartiles={quartiles(tr_robs) if tr_robs else None}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: run + expected pass**

`PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_w4_grid.py`
→ `4 passed`.

- [ ] **Step 4: PROCEDURE — ordering check, then run the grid (120 runs, 50k ticks)**

First confirm the Task 3.2 Step-5 frontier positive-control reading is
already recorded in `/tmp/wgb-fit/frontier-control-first/sweep_stdout.txt`
(procedure order: the control reading precedes the headline frontier
ensemble; an order step, not a ship gate). Then:

```bash
mkdir -p /tmp/wgb-grid
python3 python/analysis/sweep_trophic.py --scenario frontier \
  --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101 \
  --ticks 50000 \
  --knobset "blind-born:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false" \
  --knobset "blind-rob:hauler_belief_scoring=false,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true" \
  --knobset "ring-born:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=false" \
  --knobset "ring-rob:hauler_belief_scoring=true,station_gossip_slots=0,craft_gossip_slots=0,staleness_from_rob_tick=true" \
  --knobset "gossip-born:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=false" \
  --knobset "gossip-rob:hauler_belief_scoring=true,station_gossip_slots=16,craft_gossip_slots=8,staleness_from_rob_tick=true" \
  --out /tmp/wgb-grid | tee /tmp/wgb-grid/sweep_stdout.txt

python3 python/analysis/w4_grid.py /tmp/wgb-grid \
  --compare <PATH TO THE PHASE-0B BANKED POST-FIX TROPHIC BASELINE SWEEP DIR> \
  | tee /tmp/wgb-grid/w4_readout.txt
```

(The `--compare` path is the phase-0b 20-seed trophic bank; if that bank
predates Task 3.1's stdout banking, regenerate it first with
`python3 python/analysis/sweep_trophic.py --scenario trophic --seeds <same 20> --ticks 50000 --out /tmp/wgb-trophic-baseline`
— a deterministic re-run of the same seeds.) All readings RECORDED; banked
by Task 3.7.

- [ ] **Step 5: commit**

```bash
git add python/analysis/w4_grid.py python/tests/test_w4_grid.py
git commit -F - <<'EOF'
lab(grid): W4 headline reader — 6 arms x 20 seeds, flip-share VALUE

blind/ring/gossip x born/rob anchors (every knob explicit; blind+ring
rob twins registered as A/A instrument controls since the anchor is
consumed only on the gossip read). Clean-seed rule pre-registered
(blind-born != PermanentPeace), gossip-vs-ring value deltas per anchor,
cross-map reads rate-normalized distribution-vs-distribution (GEO-C3).
REPORTED, NEVER GATED (PDR-0006).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

---

### Task 3.7: the owner console packet (band re-judgment + W1–W12 readings)

Assemble the console-session materials (the OD-3 scheduled re-judgment plus
the rung's window readings) into the capture-practice posts directory.
Everything in the packet is RECORDED, never gated; /tmp is volatile so this
banking happens the same day the runs land.

**Files**
- Create: `/home/john/jumpgate/docs/superpowers/posts/2026-06-11-world-gets-big-console/packet.md`
- Create (copied banked outputs, same directory):
  `fit_trophic.txt fit_frontier.txt fit_pooled.txt frontier_control_first.txt
  i1_trophic_control.txt i1_frontier_s7.txt i2_frontier_s7.txt
  grid_sweep_stdout.txt w4_readout.txt`

- [ ] **Step 1: bank the run outputs (same-day capture practice)**

```bash
mkdir -p docs/superpowers/posts/2026-06-11-world-gets-big-console
cp /tmp/wgb-fit/fit_trophic.txt  docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_trophic.txt
cp /tmp/wgb-fit/fit_frontier.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_frontier.txt
cp /tmp/wgb-fit/fit_pooled.txt   docs/superpowers/posts/2026-06-11-world-gets-big-console/fit_pooled.txt
cp /tmp/wgb-fit/frontier-control-first/sweep_stdout.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/frontier_control_first.txt
cp /tmp/wgb-i1/i1_trophic_control.txt docs/superpowers/posts/2026-06-11-world-gets-big-console/i1_trophic_control.txt
cp /tmp/wgb-i1/i1_frontier_s7.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/i1_frontier_s7.txt
cp /tmp/wgb-i2/i2_frontier_s7.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/i2_frontier_s7.txt
cp /tmp/wgb-grid/sweep_stdout.txt     docs/superpowers/posts/2026-06-11-world-gets-big-console/grid_sweep_stdout.txt
cp /tmp/wgb-grid/w4_readout.txt       docs/superpowers/posts/2026-06-11-world-gets-big-console/w4_readout.txt
```

- [ ] **Step 2: write `packet.md`**

Full skeleton below; the `«measured: …»` cells are PASTED from the named
banked files (the same paste-only discipline as goldens — the planner and
the builder's memory are not data sources):

```markdown
# Owner console packet — world-gets-big (band re-judgment + W1–W12)

FRAME (PDR-0006): every number in this packet is RECORDED, never gated.
The session judges PLAY at the console; these are the windows beside it.

## 1. Agenda
1. Band re-judgment (OD-3 bundled consequence): the haven-lurk-leak fix
   changed the trophic band previously judged "a great story" (~86% of
   post-refuge draws predicted to become map-wide breakouts). Re-watch the
   post-fix band chronicles and re-judge.
2. Frontier first look: the §1 arc — the ship that ran dry, the lane nobody
   warned, the station that sold its last tank at four times the core price.
3. W4 anchor adoption call (born vs rob staleness anchor, gossip arms).
4. W11 read = the named pirate-fuel unification trigger (OD-6).

## 2. Pre-registered windows (spec §11, verbatim) and readings

| window | pre-registered text (spec §11) | measured | source |
|---|---|---|---|
| W1 | saturation leaves CommonKnowledge (escaped_milli < 950; desert map disambiguates) | «measured: grid MEDIA readings + escaped_milli» | grid_sweep_stdout.txt |
| W2 | median hearing lag > 2500 (registered alternative: rob-anchored staleness zeroes frontier evidence older than 4000 — "gossip degenerates toward blind on frontier routes") | «measured: MEDIA median_lag/p90_lag, gossip arms» | grid_sweep_stdout.txt |
| W3 | hub/backwater ratio > 3.0 | «measured: news-geography panel» | grid_sweep_stdout.txt |
| W4 | flip-share VALUE: gossip-vs-ring over 20 clean seeds × both anchor arms (registered alternative: mixing persists → the deferred dispatch-locality lever) | «measured: W4 VALUE deltas» | w4_readout.txt |
| W5 | I1 correlations with the position-blind-dispatch confound registered | «measured: spearman lines» | i1_frontier_s7.txt vs i1_trophic_control.txt |
| W6 | breakout share + landing distribution + lurk-dwell bimodality | «measured: LurkMoved breakout share (phase-2 instruments)» | grid windows JSONL |
| W7 | tier-2 service rate + regime onset vs the upgrade ladder | «measured: per-zone traffic, frontier zone» | i2_frontier_s7.txt |
| W8 | hauler per-leg burn/duty (the calibration input) | «measured: FUEL line fields» | grid_sweep_stdout.txt |
| W9 | strandings 0–2/run band + robbed→stranded chains + contract-age liveness | «measured: FUEL strandings/adrift_end + chronicle chains + LIVENESS max_open_contract_age» | grid_sweep_stdout.txt |
| W10 | station fuel stock-out map + price gradient + fuel_starve discriminator | «measured: per-zone fuel_starve readings» | i2_frontier_s7.txt |
| W11 | fleet attrition + per-role pirate fuel low-water (the OD-6 trigger input) | «measured: per-role JSONL fuel rows» | grid windows JSONL |
| W12 | trophic arms bit-identical digest + fuel_empty=0 (the control stays a control) | «measured: phase-1 digest test + trophic baseline fuel_empty» | fit_trophic sweep |

## 3. Instrument re-fit (dual-map, post-leak-fix)
Frontier positive control read FIRST: «measured: N/4 RiskEqualized»
(frontier_control_first.txt). Fit tables: fit_trophic.txt /
fit_frontier.txt / fit_pooled.txt. Adopted constants: «paste the
diagnostics.rs literals + whether the margin was open». Held-out seeds
11/31/57/101 sides: «paste».

## 4. Band re-judgment materials (post-fix trophic vs pre-fix bank)
Post-fix 20-seed trophic baseline: the phase-0b bank. Pre-fix judged band:
docs/superpowers/posts/2026-06-11-media-rung-story (chronicles + panels).
Diff to watch: post-refuge relocation pattern (LurkMoved breakout share on
the band), verdict mix, robs/trips rates — rate-normalized
distribution-vs-distribution, never paired.
Chronicle regen commands:
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario trophic --seed 7 --ticks 50000 --chronicle
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed 7 --ticks 50000 --chronicle

## 5. Registered confounds & rules carried into the readings
* PLAY-C3: position-blind dispatch — radius correlations may be
  capacity-ladder reads (printed by the I1 panel itself).
* A/A twins: blind/ring born-vs-rob arms must be identical; status:
  «measured: w4_readout.txt A/A line».
* Clean-seed rule: blind-born ≠ PermanentPeace; clean count: «measured».
* GEO-C3: cross-map reads are rate-normalized distributions, never
  same-seed pairs.
* fuel_empty=0 flips meaning on frontier ("no stranding this seed" —
  texture); --assert-no-fuel-empty stays on trophic arms only.
```

- [ ] **Step 3: verify nothing volatile or generated is being staged**

```bash
git status --short docs/superpowers/posts/2026-06-11-world-gets-big-console
git diff --stat        # expect: only the new posts directory contents
```

Confirm `runs/` is untouched and nothing under `/tmp` is referenced as a
deliverable (all copies are now in the posts dir).

- [ ] **Step 4: commit, then hand to the owner**

```bash
git add docs/superpowers/posts/2026-06-11-world-gets-big-console
git commit -F - <<'EOF'
docs(console): world-gets-big owner packet — band re-judgment + W1-W12

Banked same-day (capture practice): dual-map fit tables, frontier
positive-control-first reading, I1/I2 panel outputs, the 20-seed x 6-arm
grid stdout + W4 readout, and packet.md with the spec-s11 pre-registered
window text beside every measured reading. RECORDED, never gated
(PDR-0006); the console session re-judges the post-leak-fix band (OD-3)
and reads W11 as the OD-6 trigger input.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
```

Then surface the packet path to the owner and update the tracking issue
(`filigree issue-search "world gets big"`, `filigree comment <id> --actor
<name>` with the packet path) — the console session itself is the owner's.


---

## Cross-cutting checklist

- [ ] **The single GOLDEN_CONFIG_HASH re-pin rule.** Exactly ONE `GOLDEN_CONFIG_HASH` re-pin in the whole rung: Task 1.2.1's RefuelCfg tail fold. The literal is pasted from the `print_golden_config` `#[ignore]` printer output — never typed from this plan, never hand-computed — in a single-cause commit whose provenance comment cites the cause and the old literal. The only other golden movement is the NEW `FRONTIER_TRAJECTORY_GOLDEN` (created in Task 2.10, re-pinned once by the Task 2.12 v_e bake, cause documented), both pasted from `print_golden_frontier`. `HASH_FORMAT_VERSION` stays 5 throughout; `GOLDEN_ZERO_STATE_HASH` and every zero-world state-hash golden never move. Any other golden-test failure anywhere in the plan is a BUG to debug, never a re-pin.
- [ ] **The trophic digest exit criterion.** Phase 1 exits ONLY on the cross-branch 2000-tick digest (Task 1.2.7): trophic runs at seeds 7/23, stdout + JSONL byte-identical between the pre-phase-1 commit and HEAD, plus `replay-check OK` at HEAD. Any divergence is a determinism break — STOP and bisect commit-by-commit; never rationalize a diff, never re-bake fixtures to make it pass. The eps commit additionally proves band neutrality via the Task 1.2 seeds-1/7 digest, and Tasks 2.2/2.5 bracket their edits with before/after output diffs (the digest-tests-are-determinism discipline).
- [ ] **The never-gate rule (windows recorded).** PDR-0006: every run metric this plan emits — verdicts, HHI/slack readings, W1–W12, strandings, fuel_starve, flip-share value, spearman correlations, control restatements — is a designer's WINDOW, recorded in banks/panels and never a pass/fail gate. The only pass/fail surfaces are determinism checks (digests, replay-check, goldens) and unit tests. `--assert-no-fuel-empty` stays a TROPHIC-ARM endurance window only; on frontier `fuel_empty=0` reads as texture ("no stranding this seed"), recorded, never asserted. Procedure-ORDER steps (positive control first) gate the reading order, never the shipping of any mechanic.
- [ ] **The commit trailer.** Every commit message is written via `git commit -F -` heredoc and ends with the exact trailer line `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- [ ] **Explicit `git add` paths.** Every commit stages EXPLICIT file paths only — never `git add -A`, never `git add .`. Verify with `git status --short` before each commit that exactly the intended files are staged.
- [ ] **Never stage `runs/`.** `runs/` is gitignored (do not "fix" that) and is never staged or committed; digest artifacts, calibration ensembles, and baselines live there untracked. Anything worth keeping is banked same-day into `docs/superpowers/posts/<date>-<story>/` (the capture practice — `/tmp` and `runs/` are volatile-class) and committed from there with explicit paths.
- [ ] **Builder pre-flight notes (review minors, none blocking).** (1) Missing `use` imports in some plan snippets (e.g. `PriceCfg`, `WINDOW_TICKS`) — resolve from the file's existing import block, the neighbours show the idiom. (2) Line refs are HEAD `e7e490e` hints; earlier phases shift them — re-grep the SYMBOL before editing, symbols are authoritative. (3) The pending-refuel no-clobber guard is untestable by construction (the plan acknowledges it; don't manufacture a test). (4) Panel code: `None`-guard every optional anchored-line read (pre-FUEL banked outputs parse as `None`, not 0). (5) The Task 2.3 calibration-pending hauler v_e sentinel (`FRONTIER_HAULER_EXHAUST_VELOCITY = 1.0`) is the §4 analytic prior, NOT a measured value — Task 2.12 replaces it and writes the derivation into the doc comment; never ship a frontier reading off the sentinel. (6) Intra-tier `src_b == sink` is intentional (the tier loop's return leg) — the invariant test, not intuition, is the law. (7) Phase-3 positive control runs BEFORE any frontier reading is recorded (procedure order, not a gate).
