# Phase 1 — eps re-bake (spec §4 item 1; §9 phase 1 first clause)

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
