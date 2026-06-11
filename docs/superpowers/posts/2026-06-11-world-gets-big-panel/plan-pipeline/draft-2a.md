# Phase 2 (first half) — the frontier factory

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

Spec §6: "Reach 0.6 set EXPLICITLY in both factories (today inherited silently); the stale 'nearest station' marooned doc fixed in the same commit." The frontier factory sets reach at birth (Task 2.3); this task fixes the existing factory and the doc, in ONE commit. Both edits are behavior-neutral (value identical to the default; doc-only comment), so the TDD red step is replaced by a behavior-preservation digest (the digest-tests-are-determinism discipline) — see deviations.

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

If present with a hardcoded `scenario=trophic` token, change ONLY the value source so the same format string prints `args.scenario` (e.g. the `scenario={}` placeholder fed by `args.scenario` in the existing `println!` argument list). Do not add/move/reorder tokens — the sweep regexes are line-anchored and phase-0b-owned. If phase 0b has not landed yet, skip this step and leave a one-line `// TODO(phase-0b ordering)` is NOT acceptable — instead coordinate the rebase so phase 0b lands first (it precedes phase 2 in the spec's landing order).

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
