# Pirates Rung 1 (Predation + Upgrades) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land choke-point pirate predation, the ransom robbery economy, the
Hull/Escort upgrades arms race, dock-gated route evidence, the diagnostics lab, and the
trader-gym contact extension — per `docs/superpowers/specs/2026-06-10-pirates-rung1-predation-and-upgrades-design.md`
(THE SPEC; read it first, it resolves every "why").

**Architecture:** all behavior is in-world deterministic tick stages (the
`run_scripted_dispatch` precedent) over hashed integer state; randomness only via
`RngStream::Piracy`; exactly two single-cause golden commits (A config, B state v4);
every behavior commit proves itself golden-neutral + replay-bit-identical.

**Tech stack:** Rust 2024 (`gen` reserved), clippy `--all-targets -D warnings`,
PyO3/jumpgate-py, SB3 PPO (CPU), pytest.

**Project laws (verbatim, non-negotiable):**
- Goldens at HEAD: `HASH_FORMAT_VERSION = 3`, `GOLDEN_ZERO_STATE_HASH = 0x1d44_b373_5ccd_33f7`,
  `GOLDEN_CONFIG_HASH = 0xf4bc_85c3_7cb6_8a6b`. They move ONLY in Tasks 2 and 3, one
  cause each, literals re-derived via the ignored `print_golden` tests — NEVER invented.
  Every other task ends by asserting they are unchanged.
- Subagents do not report gate claims as fact; the main loop re-verifies (`cargo test
  --workspace`, clippy, pytest, grep the pinned hashes).
- Never `git add -A` / `.`; explicit paths only; never stage `.gitignore`, `.claude/`,
  `CLAUDE.md`, `AGENTS.md`, `.mcp.json`, `.filigree.conf`. Commit messages with parens
  use `git commit -F` heredoc. Trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- No `Date::now`/wall-clock anywhere in core. No per-craft taste scalars (retired
  premise). Obs: fixed compile-time scales, no VecNormalize. Reward: Δcredits only.

---

## Task 0 (P0): the instrument — diagnostics + runner over the CURRENT world

**Files:**
- Create: `crates/jumpgate-core/src/diagnostics.rs`; register `pub mod diagnostics;` in `lib.rs`
- Create: `crates/jumpgate-core/examples/trophic_run.rs`
- Test: in-module `#[cfg(test)]` in diagnostics.rs

- [ ] **Step 0.1 — failing tests: the 4-corner classifier.** `TrophicSample` (per-window,
  all integer): `{ tick: u64, active_pirates: u32, lying_low: u32, laden_in_transit: u32,
  laden_trips: u32, robs: u32, drivenoffs: u32, purchases_hull: u32, purchases_escort: u32,
  per_route_robs: Vec<u32>, per_route_accepts: Vec<u32>, per_route_traffic: Vec<u32>,
  yard_treasury_micros: i64, per_craft_credits: Vec<i64>, engagement_phase_milli: Vec<u32> }`.
  `Diagnosis { cycled: bool, risk_heterogeneous: bool, outcomes_disperse: bool,
  verdict: Verdict }`, `enum Verdict { Alive, NoCycle, RiskEqualized, Saturated,
  DecisionNotTranslating, PermanentPeace, ArmsRaceFlat }` (NO gate vocabulary in names).
  Pure `pub fn classify(samples: &[TrophicSample]) -> Diagnosis`. Heterogeneity metric =
  HHI of robs over OCCUPIED routes (`per_route_traffic > 0`), normalized by
  active-pirate count, PLUS rank-persistence of the hot route vs the traffic gradient
  (see spec §1/§9 — NOT top/median). Write 4 synthetic-series tests with hand-built
  labeled data: `cycling_heterogeneous → Alive`, `flat → NoCycle`,
  `cycling_equalized → RiskEqualized`, `cycling_hetero_no_dispersion → DecisionNotTranslating`.
  Thresholds = named consts with doc comment `// DIAGNOSTIC WINDOWS, NOT GATES (PDR-0006)`.
- [ ] **Step 0.2** run: `cargo test -p jumpgate-core diagnostics` → FAIL (module absent).
- [ ] **Step 0.3** implement `classify` minimally until the 4 corners pass.
- [ ] **Step 0.4 — the runner.** `examples/trophic_run.rs`: args `--seed --ticks
  --jsonl PATH --chronicle --replay-check` (plain `std::env::args` parsing). For P0 it
  runs the EXISTING trader-style config (no pirates): build via a temporary local config
  fn cloned from the trader template shape; loop `world.step(&mut vec![])`; sample
  `TrophicSample` every `W = 2000` ticks (laden_trips counts `ContractFulfilled` events
  per window — THE §4 CALIBRATION INPUT); serialize JSONL (serde_json dev-dep is already
  in the workspace; if not: `serde_json = "1"` under `[dev-dependencies]`). Chronicle
  printer: group `recent_events` by craft id, one tick-stamped line per event.
  `--replay-check`: run the identical config twice, assert the `(tick, state_hash)`
  streams (sampled every 1000 ticks) are equal.
- [ ] **Step 0.5** run: `cargo run -p jumpgate-core --example trophic_run -- --seed 7
  --ticks 10000 --jsonl /tmp/p0.jsonl --replay-check` → completes; RECORD
  `laden_trips_per_window` from the JSONL into the task log (it parameterizes Task 5's
  food band). `cargo clippy --all-targets -- -D warnings` clean.
- [ ] **Step 0.6** commit: `feat(lab): trophic diagnostics + classifier (4-corner tested) + trophic_run example`

## Task 1 (Commit A): config surface — ONE config-golden re-pin

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` (RunConfig tail; the exhaustive destructure
  at the config_hash fold makes omissions a compile error)
- Modify: `crates/jumpgate-core/src/world.rs` (reset: mint pirates from role)
- Modify: `crates/jumpgate-core/src/stores.rs` (BaseSpec field)

- [ ] **Step 1.1 — failing test:** `config.rs` test `trophic_cfg_defaults_are_inert`:
  default `TrophicCfg` has `engage_radius_au == 0.0` and a `RunConfig` built without
  pirates produces a `World` with zero `Some(pirate)` rows; plus
  `pirate_role_mints_pirate_state`: a `CraftInit { role: CraftRole::Pirate, .. }` row
  yields `ships.pirate[r].is_some()` with `food_micros == cfg.trophic.grubstake_micros`.
- [ ] **Step 1.2** run → FAIL (fields absent).
- [ ] **Step 1.3 — implement.** Append to RunConfig tail (NEVER reorder):
  ```rust
  pub struct TrophicCfg {
      pub engage_radius_au: f64,        // 0.0 = whole machinery inert (default)
      pub engage_speed: f64,            // 2.0e-3
      pub p_rob_milli: u32,             // 700
      pub ransom_cap_micros: i64,       // 2_000_000
      pub food_per_unit_micros: i64,    // placeholder; console-calibrated from P0 (spec §4 formulas)
      pub upkeep_per_tick: i64,
      pub grubstake_micros: i64,
      pub starve_lie_low_ticks: u64,    // 2000
      pub heat_threshold: u32,          // 250
      pub notoriety_per_rob: u32,       // 100
      pub notoriety_decay_milli: u32,   // 950
      pub decay_interval: u64,          // 200
      pub heat_lie_low_ticks: u64,      // 1500
      pub rob_cooldown: u64,            // 600
      pub driveoff_cooldown: u64,       // 200
      pub pirate_base_strength: u8,     // 1
      pub pirate_max_reach_au: f64,     // 0.6  (PRIMARY locality lever)
      pub relocate_period: u64,         // 2500
      pub stay_milli: u32,              // 500
      pub hideout_body_index: u32,
      pub evidence_window: u64,         // 4000
      pub evidence_penalty_milli: u32,  // 150 per recent rob, clamped at 900
      pub hauler_belief_scoring: bool,  // false (trader gym untouched)
      pub hauler_buy_policy: BuyPolicy, // Off | EscortFirst | HullFirst
  }
  pub struct ShipyardCfg {
      pub corp_index: u32,
      pub hull_price_micros: [i64; 2],   // [8_000_000, 20_000_000]
      pub escort_price_micros: [i64; 2], // [5_000_000, 12_000_000]
      pub hull_step_units: u32,          // 5
      pub max_hull_level: u8,            // 2
      pub max_escort_level: u8,          // 2
      pub buy_headroom_milli: u32,       // 1500
  }
  ```
  Plus `CraftInit.role: CraftRole` (default `Idle`), `CraftInit.scripted: bool` (default
  `true`), `StationInit.sells_upgrades: bool` (default `false`),
  `BaseSpec.base_cargo_capacity: u32` (default 5). Fold ALL of it into config_hash at the
  tail (f64 via the existing to_bits convention). `World::reset` mints
  `PirateState { food_micros: grubstake, notoriety: 0, lie_low_until: Tick(0), engage_cooldown_until: Tick(0) }`
  for Pirate-role rows — `engage_cooldown_until` arrives in Task 2; until then mint the
  existing fields only and leave a `// Task-2` marker the Task-2 diff removes.
- [ ] **Step 1.4 — re-pin (single cause).** `cargo test -p jumpgate-core config -- --ignored print_golden`
  (or the crate's equivalent ignored printer) → copy the RE-DERIVED literal into
  `GOLDEN_CONFIG_HASH` with comment `// RE-PINNED: +trophic/shipyard/role/scripted/sells_upgrades/base_cargo_capacity config surface (pirates rung A). Was 0xf4bc_85c3_7cb6_8a6b.`
- [ ] **Step 1.5** full gates: `cargo test --workspace` green; clippy clean; grep
  confirms `GOLDEN_ZERO_STATE_HASH` UNTOUCHED.
- [ ] **Step 1.6** commit (single-cause): `feat(config): pirates-rung config surface — TrophicCfg/ShipyardCfg/role/scripted/vendor/capacity [config golden re-pin]`

## Task 2 (Commit B): state v4 — ONE state-golden re-pin

**Files:**
- Modify: `crates/jumpgate-core/src/stores.rs`, `hash.rs`, `world.rs`, `lib.rs`

- [ ] **Step 2.1 — failing test:** hash completeness test extended: flipping any bit of
  `upgrades[r].hull/escort`, `pirate[r].engage_cooldown_until`, `info_tick[r]`, or any
  `route_evidence` ring slot changes `state_hash` (the existing completeness-test
  pattern, one assertion per new field).
- [ ] **Step 2.2** run → FAIL.
- [ ] **Step 2.3 — implement.**
  ```rust
  // stores.rs
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
  pub struct UpgradeLevels { pub hull: u8, pub escort: u8 }   // strength & capacity DERIVED, never stored
  // CraftStore: pub upgrades: Vec<UpgradeLevels>, pub info_tick: Vec<Tick>,
  //             pub pending_upgrade: Vec<Option<UpgradeKind>>   // TRANSIENT, unhashed (prev_* doc pattern)
  // PirateState: + pub engage_cooldown_until: Tick   (append INSIDE the word-26 self-delimiting fold)
  pub enum UpgradeKind { Hull, Escort }                        // append-only
  // world.rs
  pub struct RouteEvidence { pub robs: Vec<[Tick; 8]>, pub cursor: Vec<u8> }  // dense n_stations², ring
  ```
  Hash: `upgrades` + `info_tick` appended after word 26 in `write_craft_economy` (BOTH
  the main fold and the parity recompute site); `route_evidence` as world-level words
  after the craft fold; HASH_FIELD_ORDER doc updated; `debug_assert!(pending_upgrade.iter().all(Option::is_none))`
  in `state_hash`. Add the Class-3 transitively-pinned doc paragraph for the Piracy
  stream cursor (the `prev_*` precedent). Bump `HASH_FORMAT_VERSION` to 4.
- [ ] **Step 2.4 — re-pin (single cause).** Re-derive via `print_golden`:
  `GOLDEN_ZERO_STATE_HASH` + the manual zero-fold in `golden_zero_state_hash` + comment
  `// RE-PINNED: HASH_FORMAT_VERSION 3->4 (+upgrades/info_tick/engage_cooldown/route_evidence). Was 0x1d44_b373_5ccd_33f7.`
- [ ] **Step 2.5** full gates; grep confirms `GOLDEN_CONFIG_HASH` carries Task-1's value untouched.
- [ ] **Step 2.6** commit (single-cause): `feat(core): state schema v4 for pirates rung [state golden re-pin]`

## Task 3 (Commit C): purchase verb + capacity gate (goldens FROZEN from here on)

**Files:**
- Modify: `crates/jumpgate-core/src/types.rs` (CommandKind), `ingest.rs`, `economy.rs`,
  `world.rs` (stage 1d), `contract.rs` (EventKind)

- [ ] **Step 3.1 — failing tests** (economy.rs, exact-integer per-arm assertions — the
  identities alone cannot catch wrong-price bugs):
  - `purchase_settles_at_vendor`: craft docked at a `sells_upgrades` station with 10 cr,
    `pending_upgrade = Some(Escort)` → after stage 1d: escort == 1, credits debited
    EXACTLY `escort_price_micros[0]`, Yard treasury credited the same, intent None,
    `UpgradePurchased` emitted.
  - `purchase_skips_deterministically`: not-docked / underfunded / at-cap (level == 2) /
    non-vendor station → no-op, intent cleared, NO event, zero credit movement.
  - `capacity_gate_reverts_oversized_accept`: qty-10 contract + hull 0 → accept-settle
    REVERTS (craft Idle, contract stays Offered — the underfunded-escrow precedent);
    succeeds after hull = 1.
  - `credit_identity_holds_across_purchases` (existing identity test extended).
- [ ] **Step 3.2** run → FAIL.
- [ ] **Step 3.3 — implement.** `CommandKind::BuyUpgrade { kind: UpgradeKind }` (unhashed,
  additive) + ingest arm (AcceptContract template: write intent only, `ActionIngested`
  always, ActionLog). NEW `resolve_purchases(...)` called as stage 1d in `World::step`
  (after `resolve_contracts`, pre-physics, `body_pos(t-1)` frame for the dock predicate —
  the try_load precedent). `cargo_capacity(r) = spec.base_cargo_capacity + hull as u32 * shipyard.hull_step_units`;
  gate at the Offered→Accepted settle + the same filter in scripted ASSIGN. Saturating
  arithmetic throughout (the spec §8 totality discipline). New `EventKind::UpgradePurchased
  { craft: CraftId, kind: UpgradeKind, level: u8, price_micros: i64 }` (check the single
  non-exhaustive `matches!` in jumpgate-py env.rs:734 — panel verified no exhaustive-match
  cost; fix the stale economy.rs:804-807 comment as a drive-by).
- [ ] **Step 3.4** full gates + grep BOTH goldens unchanged.
- [ ] **Step 3.5** commit: `feat(economy): BuyUpgrade verb, vendor settle stage, capacity-gated contracts`

## Task 4 (Commit D): encounter + robbery + population

**Files:**
- Create: `crates/jumpgate-core/src/pirate.rs` (`resolve_encounters`, `update_pirate_population`)
- Modify: `world.rs` (stages 3b2/3b3 between resolve_deliveries and resolve_failures),
  `economy.rs` (generalize the resolve_failures settle body to take a cause),
  `contract.rs` (first emitters of pre-pinned `Robbed`/`DrivenOff`/`PirateLieLow`)

- [ ] **Step 4.1 — failing tests:**
  - `escort_threshold_is_a_step`: (S_p=1 vs S_h=0) → engagement occurs (Robbed or
    DrivenOff by roll); (1 vs 1) and (1 vs 2) → NO engagement event at all (eligibility
    excludes non-weaker defenders — ties to defender); (2 vs 1) → engagement again.
  - `dock_is_sanctuary_at_destination`: same-tick Arrival + in-envelope pirate →
    delivery settles, no rob (3b before 3b2).
  - `rob_on_load_is_legal_at_origin`: pirate lurking at origin, hauler loads → engagement
    fires that tick (the owned headline behavior).
  - `robbery_settlement_exact`: Robbed → cargo to `consumed[res] += qty`; contract
    (CargoLoaded AND InTransit variants both tested) → Failed with escrow refund EXACTLY
    `reward_micros` to the right corp; ransom EXACTLY `min(wallet, cap)` hauler→pirate;
    hauler Idle; `food_micros += qty * food_per_unit`.
  - `cooldown_prevents_rerolls`: post-rob, no second engagement for `rob_cooldown` ticks.
  - `lie_low_and_heat`: food → 0 forces lie-low + grubstake reset; notoriety ≥ threshold
    forces lie-low; notoriety decays on the interval.
  - `replay_bit_identical_with_piracy_draws`: 5k-tick two-run `(tick, state_hash)` equality
    on a pirate scenario (FIRST RngStream::Piracy runtime draws — the new replay surface).
  - Conservation: both identities over a 5k-tick robbed run (zero new legs — Σtreasury+Σcredits+Σescrow
    constant, consumed-leg balances).
- [ ] **Step 4.2** run → FAIL.
- [ ] **Step 4.3 — implement** per spec §§2-4 verbatim (eligibility incl. strength-skip;
  nearest-eligible; one engagement per pirate per tick; sequential re-check;
  `u < p_rob_milli` on `RngStream::Piracy` in dense-row order at exactly stage 3b2;
  trip-phase logging: each `Robbed`/`DrivenOff` event's emission site also pushes
  `engagement_phase_milli` = elapsed-trip-fraction × 1000 into the diagnostics sampler's
  window accumulator).
- [ ] **Step 4.4** full gates + goldens-unchanged grep. Temp-revert proof on
  `escort_threshold_is_a_step` (comment out the strength check, watch it fail, restore —
  the discriminating-test discipline).
- [ ] **Step 4.5** commit: `feat(core): choke-point encounters, ransom robbery, pirate population dynamics`

## Task 5 (Commit E): brains, evidence, scripted policies

**Files:**
- Modify: `crates/jumpgate-core/src/pirate.rs` (`run_pirate_brains` — stage 1c2),
  `economy.rs` (evidence-scored ASSIGN behind `hauler_belief_scoring`; scripted purchase
  policies writing `pending_upgrade`), `world.rs` (stage wiring; dock-refresh of `info_tick`)

- [ ] **Step 5.1 — failing tests:**
  - `initial_lurks_are_seed_drawn`: two master seeds → different pirate→station maps
    (Piracy stream at reset; the gym-memorization guard).
  - `relocation_respects_reach`: with reach 0.6 AU, no relocation target ever beyond it;
    relocation draw is uniform-in-reach (statistical over 64 staggered draws), NEVER
    traffic-weighted (no traffic input in the fn signature — enforce by construction).
  - `reseek_threshold_covers_dock`: a settled lurker re-seeks at drift > engage_radius/2;
    property: lurker within engagement range of a body-docked hauler whenever settled.
  - `info_tick_refreshes_only_docked`: in-flight craft keeps stale info_tick; docked
    craft updates to current tick.
  - `route_evidence_read_is_dock_gated`: rob at tick T; hauler last docked at T-1 sees
    count 0; hauler docked at T+1 sees 1; entries age out past `evidence_window`.
  - `evidence_scored_assign_avoids_hot_routes`: with scoring on, a fresh rob on route A
    flips the scripted claim to route B (and decays back).
  - `purchases_desynchronize`: 4 haulers with different wealth/docking patterns buy
    Escort L1 at different ticks (spread > 0; the synchronization-death guard).
- [ ] **Step 5.2** run → FAIL.
- [ ] **Step 5.3 — implement** per spec §§5-7. `route_evidence(reader: CraftId, route: usize) -> u32`
  takes the READER now (media-seam signature; doc comment: "the degenerate proto-channel —
  replace the propagation model behind this signature"). Scripted stages skip
  `!scripted` craft everywhere (grep audit: every new stage iterates with the skip).
- [ ] **Step 5.4** full gates + goldens grep + `--replay-check` 10k ticks.
- [ ] **Step 5.5** commit: `feat(core): lurker brains, dock-gated route evidence, scripted purchase/avoidance policies`

## Task 6 (Commit F): the lab — scenario, sweeps, positive control

**Files:**
- Create: `crates/jumpgate-core/src/scenario.rs` (`scenario_trophic`), `python/analysis/sweep_trophic.py`
- Modify: `examples/trophic_run.rs` (use scenario_trophic; `--set knob=value` overrides)

- [ ] **Step 6.1 — failing test:** `scenario_trophic_shape`: 6 station bodies a ∈
  0.35–1.4 AU, 12 haulers (scripted, ASSIGN on, stagger 16, belief scoring ON), 6-pirate
  pool / 2 initially active (expected-active ≤ stations − 2 — the Saturated guard),
  ≥ 12 directed routes across 3 tier corps (qty 5/10/15, per-unit 1.00/1.15/1.30×, demand
  bands 10/20, 5/15, 0/10), Yard corp + 2 vendor stations, hideout = outermost,
  `exhaust_velocity` ×10 vs trader spec (fuel endurance, spec §6), seed-derived mean
  anomalies (anti-memorization, the trader-template precedent).
- [ ] **Step 6.2** run → FAIL; implement; pass.
- [ ] **Step 6.3 — endurance + control runs.** 50k-tick baseline: assert ZERO FuelEmpty
  events (a determinism-cheap window, not an aliveness gate). Live positive control:
  `--set pirate_max_reach_au=999 --set stay_milli=0` run MUST classify RiskEqualized — if
  not, STOP: the instrument is broken (spec §1 instrument-kill).
- [ ] **Step 6.4** `sweep_trophic.py`: subprocess grid (seeds × knob sets), aggregate
  JSONL, print matrix-row counts + the per-mechanic discriminator panels (spec §9 list,
  incl. the endpoint-ambush trip-phase histogram and purchase-desync spread).
- [ ] **Step 6.5** full gates; commit: `feat(lab): scenario_trophic, sweep aggregator, positive-control ablation`
- [ ] **Step 6.6 — CONSOLE TUNING PHASE (the actual rung bar).** Protocol: ≤ 24
  parameterizations; each run logged as (knobs, seed, verdict, matrix row, knob moved,
  chronicle excerpt); calibrate the food band FIRST from P0's measured
  `laden_trips_per_window` via spec §4's formulas. The owner reads chronicles; "alive +
  watchable" is the owner's holistic judgment (PDR-0006), metrics are the evidence. A
  green build is NOT a met bar.

## Task 7 (Commit G): gym extension — the trader as player

**Files:**
- Modify: `crates/jumpgate-py/src/env.rs` (num_pirates kwarg, horizon 5000 variant,
  TRADER_OBS_DIM 34 when pirates on), `obs.rs` (contact blocks), `world.rs`
  (`pirate_contacts` accessor)
- Modify: `python/jumpgate/gym_env.py` (obs_dim passthrough only — action space UNCHANGED)
- Create: `python/train/eval_pirates_ablation.py`
- Test: `python/tests/test_trader_pirates_mode.py`

- [ ] **Step 7.1 — failing tests (pytest):** layout test for dims 20-33 (stride 7:
  present, unit-bearing xyz, `log1p(d/0.01)/5.5`, `strength/4.0`, active; contacts sorted
  by distance); `num_pirates=0` ⇒ obs_dim 20 and EVERY existing test + the keystone
  learning smoke byte-identical; two reset seeds ⇒ different initial lurk stations
  (memorization guard); robbery inside an episode debits Δcredits (ransom + forfeited
  payout) with NO reward shaping anywhere.
- [ ] **Step 7.2** run → FAIL; implement (`World::pirate_contacts(observer) ->
  Vec<(Vec3, Vec3, u32, bool)>` — plain read over hashed state, the trader-accessor
  pattern); pass. Action space stays `Discrete(5)` — grep-assert no change.
- [ ] **Step 7.3 — train + report (REPORTED, NEVER GATED).** Train PPO at horizon 5000,
  `num_pirates=2`; eval on held-out seeds vs the SAME policy class with contact dims
  zero-masked; report Δcredits delta + route-share shift vs lurk positions +
  per-decision-index robbery cost (the early-episode-ransom note). A null is a player
  finding at these prices — it triggers NOTHING (PDR-0006; the report script prints that
  sentence in its header).
- [ ] **Step 7.4** full gates (`cargo test --workspace`, clippy, `pytest python/tests -x`);
  goldens grep; commit: `feat(gym): pirate-contact evidence obs (20→34), horizon-5000 pirates variant, ablation report`

## Task 8: books

- [ ] Filigree: close/comment the rung issue with the evidence trail; file follow-ups
  (cut-2 package: refuel+tanker+fence+value-seeking+escrow-lock-fix; chase/tether
  package; effects-table migration trigger). Memory: write the landed/NO-GO finding per
  the console phase's outcome. Update the spec's Status line.

---

## Self-review notes (writing-plans checklist)

- Spec coverage: §2→T4, §3→T4, §4→T4+T6.6, §5→T5, §6→T1/T3/T6, §7→T5, §8→T1/T2, §9→T0/T6,
  §10→T0/T6, §11→T7, §12 = the task order itself, §14 forward-only (no tasks — correct).
- The two golden re-pins are exactly Tasks 1 and 2; every later task greps both unchanged.
- Type names consistent: `UpgradeLevels`/`UpgradeKind`/`TrophicCfg`/`ShipyardCfg`/
  `RouteEvidence`/`Verdict` used identically across tasks.
- Constants that are deliberately NOT pinned here (food band) say so and point at the
  P0 measurement + spec §4 formulas — that is a calibration procedure, not a placeholder.
