# Phase 0b — lab instruments before mechanics (spec §8, §9 phase 0b)

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
