# Grounding Extract: Scenario Factories + Config Folds + Golden Discipline
## Beat: scenario factories, config fold order, hash/golden touchpoints

All file:line citations verified by direct read this session.
Source tree HEAD: `jumpgate-v1-design` branch.

---

## 1. `scenario_frontier` — full factory shape

**File:** `crates/jumpgate-core/src/scenario.rs`

### 1.1 Orbit constants

```rust
// scenario.rs:45-56
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
Geometric law: `a_k = 0.35 * r^k`, `r = (3.0/0.35)^(1/9)`. Endpoints EXACT; interior pinned by `frontier_orbit_band_is_the_pinned_geometric_law` test (scenario.rs:849-874). The 8→9 gap (0.637 AU) intentionally exceeds `pirate_max_reach_au` 0.6 — the "never-opens seam" (scenario.rs:42-44, 869-873).

### 1.2 Population constants and calibrated `v_e`

```rust
// scenario.rs:62-63
pub const FRONTIER_NUM_HAULERS: usize = 20;   // 2 per station
pub const FRONTIER_NUM_PIRATES: usize = 10;  // 2:1 predator:prey design choice

// scenario.rs:77
pub const FRONTIER_HAULER_EXHAUST_VELOCITY: f64 = 42.5;
```
`FRONTIER_HAULER_EXHAUST_VELOCITY` is **calibrated, not derived from spec arithmetic**: OD-5b, k=2.5 applied to the measured worst hauler-leg burn (170‰ of scaled tank, seed 9, tick 86000). Pirates keep `base_exhaust_velocity: 20.0` (the band's x10 spec — OD-6, scenario.rs:350-355).

### 1.3 Haven and tier wiring constants

```rust
// scenario.rs:82
pub const FRONTIER_HAVEN_STATION: usize = 6;  // hosted by body 7 (1.4660 AU), NOT outermost

// scenario.rs:89-90
pub const FRONTIER_TIER_WIRING: [(usize, usize, usize, usize); 3] =
    [(0, 1, 2, 3), (3, 4, 5, 4), (7, 8, 9, 8)];
// tuple = (source_a, source_b, dest, fuel_sink)
```
Dests per tier: 2, 5, 9. Sinks per tier: 3, 4, 8. Pairwise disjoint (independent Schmitt triggers). The tier-2 return (9→8) rides the 8-9 never-walkable gap (scenario.rs:429).

### 1.4 Shared tier table (also used by `scenario_trophic`)

```rust
// scenario.rs:104
pub const TIERS: [(u32, i64); 3] = [(5, 1000), (10, 1150), (15, 1300)];
// (qty, per-unit multiplier in milli)

// scenario.rs:99
pub const PER_UNIT_BASE_MICROS: i64 = 200_000;
```
Reward formula: `qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000` → 1_000_000 / 2_300_000 / 3_900_000 micros (scenario.rs:1019-1023).

### 1.5 Bodies (scenario.rs:322-336)
Star at origin. 10 station bodies: one per `FRONTIER_ORBIT_AU` entry, with seed-derived mean anomaly via `mix(seed, k+1)` (scenario.rs:331). Same `mix` function used by `scenario_trophic` (the "trader template" precedent).

```rust
// scenario.rs:108-113
fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
// scenario.rs:116-118
fn u64_to_unit_f64(x: u64) -> f64 {
    ((x >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}
```

### 1.6 Craft construction (scenario.rs:341-388)

Two `BaseSpec` definitions — **per-class, not shared**:
```rust
// scenario.rs:341-355
let hauler_spec = BaseSpec {
    base_dry_mass: 1.0e-9,
    base_max_thrust: 1.0e-12,
    base_exhaust_velocity: FRONTIER_HAULER_EXHAUST_VELOCITY,  // 42.5 calibrated
    base_fuel_capacity: 1.0e-9,
    base_cargo_capacity: 5,
};
let pirate_spec = BaseSpec {
    base_dry_mass: 1.0e-9,
    base_max_thrust: 1.0e-12,
    base_exhaust_velocity: 20.0,
    base_fuel_capacity: 1.0e-9,
    base_cargo_capacity: 5,
};
```

`CraftInit` fields in full:
```rust
// scenario.rs:368-376 (hauler exemplar)
CraftInit {
    spec: hauler_spec.clone(),
    pos,   // co_orbit spawn at body k % n_stations
    vel,
    fuel_mass: 1.0e-9,   // full tank at spawn
    role: CraftRole::Idle,
    scripted: true,
}
// Pirates: role: CraftRole::Pirate, spawn co-orbiting body 1+FRONTIER_HAVEN_STATION (body 7)
```

### 1.7 Stations (scenario.rs:391-433)

10 stations: `station(body_index, ore, fuel, vendor)` helper populates `initial_stock` and `initial_price_micros`. **`initial_price_micros` for Fuel is seeded from the curve**, not zero:

```rust
// scenario.rs:405-408
let fuel_price = |fuel_stock: i64| -> i64 {
    let s = fuel_stock.clamp(0, 40);
    (5_000 * (2000 - s * 1800 / 40) / 1000).max(0)
};
// [0, fuel_price(fuel)] for [Ore, Fuel]
```

Schmitt stagger via **per-tier initial stocks** (18/14/10 dest Ore, 18/14/10 sink Fuel) against the one global 10/20 band (scenario.rs:392-395, 1079-1080). Vendors at tier dests (rows 2, 5, 9) and the haven (row 6, `sells_upgrades: true`).

### 1.8 Producers (scenario.rs:434-451)

12 total: 6 miners (Ore output, interval 40) at source rows {0,1,3,4,7,8}, 3 refiners (Ore→Fuel, interval 60) at dest rows {2,5,9}, 3 Fuel sinks (Fuel input None output, interval 80) at sink rows {3,4,8}. Haven (row 6) has **no producer** (scenario.rs:1058-1060).

### 1.9 Corporations (scenario.rs:455-461)

5 corps in order:
- Corps 0-2: tier corps, `treasury_micros: 2_000_000_000`, home at dests 2/5/9
- Corp 3: Yard, `treasury_micros: 0`, home at 2
- Corp 4: Port (refuel revenue), `treasury_micros: 0`, home at 2 — `ShipyardCfg { corp_index: 3 }` (scenario.rs:537)

### 1.10 Contracts (scenario.rs:465-487)

9 routes (3 tiers × 3 legs). Built via `FRONTIER_TIER_WIRING`:
```rust
// scenario.rs:466-487
for (tier, &(qty, mult_milli)) in TIERS.iter().enumerate() {
    let reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000;
    let (src_a, src_b, dest, sink) = FRONTIER_TIER_WIRING[tier];
    for from in [src_a, src_b] {
        contracts.push(ContractInit { corp_index: tier, resource: Resource::Ore,
            qty, from_station_index: from, to_station_index: dest, reward_micros: reward });
    }
    contracts.push(ContractInit { corp_index: tier, resource: Resource::Fuel,
        qty, from_station_index: dest, to_station_index: sink, reward_micros: reward });
}
```

### 1.11 Trophic constants (scenario.rs:492-504)

```rust
let trophic = TrophicCfg {
    engage_radius_au: 5.0e-4,
    upkeep_per_tick: 12,
    food_per_unit_micros: 15_000,   // NOTE: 15k not 10k — dock-exposure dilution re-walk
    grubstake_micros: 100_000,
    ransom_cap_micros: 6_000_000,
    starve_lie_low_ticks: 4_000,
    hideout_body_index: 7,          // NOTE: body 7, NOT outermost (body 10)
    pirate_max_reach_au: 0.6,
    hauler_belief_scoring: true,
    hauler_buy_policy: BuyPolicy::EscortFirst,
    ..TrophicCfg::default()
};
```

### 1.12 Full RunConfig tail (scenario.rs:506-543)

```rust
RunConfig {
    master_seed: seed,
    dt: Dt::new(0.25),
    softening: 1.0e-4,
    substep_cfg: SubstepCfg { accel_ref: 3.0e-4, max_substeps: 64 },
    ephemeris_window: 120_000,   // trophic uses 100_000
    ...
    price_cfg: PriceCfg {
        base_micros: [0, 5_000],   // Fuel-only live; cap[Ore]==0 = structural off
        cap: [0, 40],
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
    refuel: RefuelCfg { lot_mass: 5.0e-11, corp_index: 4 },  // LIVE — 20 lots/tank
}
```

### 1.13 `scenario_trophic` differences (scenario.rs:123-308)

- 6 stations (STATION_ORBIT_AU 0.35–1.4 AU, 6 bodies), `ephemeris_window: 100_000`
- All craft: `base_exhaust_velocity: 20.0` (uniform x10 band spec)
- `food_per_unit_micros: 10_000` (not 15k)
- `hideout_body_index: 6` (outermost, 1.4 AU body)
- `RefuelCfg::default()` (`lot_mass: 0.0` — refuel verb OFF; proven gate scenario.rs:764-766)
- `price_cfg: PriceCfg::default()` (all-zero base_micros, all-1 cap — price machinery effectively dead)
- 4 corporations (no Port corp)

---

## 2. `apply_knob` match arms

**File:** `crates/jumpgate-core/src/scenario.rs:550-633`

Full arm list extracted from the match:
- **TrophicCfg** arms: `engage_radius_au`, `engage_speed`, `p_rob_milli`, `ransom_cap_micros`, `food_per_unit_micros`, `upkeep_per_tick`, `grubstake_micros`, `starve_lie_low_ticks`, `heat_threshold`, `notoriety_per_rob`, `notoriety_decay_milli`, `decay_interval`, `heat_lie_low_ticks`, `rob_cooldown`, `driveoff_cooldown`, `pirate_base_strength`, `pirate_max_reach_au`, `relocate_period`, `stay_milli`, `hideout_body_index`, `evidence_window`, `evidence_penalty_milli`, `hauler_belief_scoring`, `hauler_buy_policy` (enum: `Off|EscortFirst|HullFirst`)
- **ShipyardCfg** arms: `hull_price_1`, `hull_price_2`, `escort_price_1`, `escort_price_2`, `hull_step_units`, `max_hulls`, `max_escorts`, `buy_headroom_milli`
- **DispatchCfg** arms: `demand_low`, `demand_high`, `stagger_period`
- **MediaCfg** arms: `station_gossip_slots`, `craft_gossip_slots`, `sig_floor_milli`, `sig_divisor_micros`, `hop_loss_milli`, `inflation_milli`, `claimed_value_cap_micros`, `value_ticks_milli`, `staleness_from_rob_tick`
- **Special knob** `fuel_capacity_scale` (scenario.rs:621-630): scales EVERY craft's `base_fuel_capacity` AND `fuel_mass` by the scalar. Rejects zero/negative/non-finite loudly.
- Unknown knob → `Err` (loud; scenario.rs:631)

`staleness_from_rob_tick` (MediaCfg, scenario.rs:615): `bool` knob; `false` (default) anchors gossip evidence on `first_heard`; `true` anchors on the alert's carried `rob_tick` (owner OD-1 probe, config.rs:381).

---

## 3. RunConfig field order and config-tail fold discipline

**File:** `crates/jumpgate-core/src/config.rs`

### 3.1 RunConfig struct (config.rs:434-464)

Fields in declaration order — **this is the exhaustive destructure order** in `config_hash`:
```
master_seed, dt, softening, substep_cfg, ephemeris_window,
bodies, craft, guidance,
stations, producers, corporations, contracts, price_cfg, dispatch_cfg,
trophic, shipyard,
media,
refuel
```

### 3.2 Config-tail fold discipline

The latest precedent is `RefuelCfg` (config.rs:749-751). Its fold:
```rust
let RefuelCfg { lot_mass, corp_index } = refuel;
h.write_u64(lot_mass.to_bits());
h.write_u64(*corp_index as u64);
```

Pattern for adding a new config group:
1. Add `let NewCfg { field1, field2 } = new_cfg;` exhaustive destructure at the tail of `config_hash` — missing field = compile error (D10/M6 discipline, config.rs:530-532)
2. Write each field in declaration order
3. Update `CONFIG_FIELD_ORDER` comment (config.rs:508-528)
4. Re-pin `GOLDEN_CONFIG_HASH`

The exhaustive destructure of `RunConfig` itself (config.rs:533-552) ensures a new `RunConfig` field is a compile error until explicitly folded.

**Counts folded BEFORE field values** (config.rs:562-564): cardinality changes always move the hash even when new elements are all-zero. GoodsCfg spec (from synthesis): count word first as anti-aliasing delimiter.

### 3.3 GOLDEN_CONFIG_HASH

```rust
// config.rs:783
const GOLDEN_CONFIG_HASH: u64 = 0x128c_1299_5c48_4fdc;
// RE-PINNED: +RefuelCfg{lot_mass,corp_index} folded at config tail (world-gets-big §5).
// Was 0xee02_df67_1889_78dc.
```

The `sample()` function (config.rs:785-831) is the fixture: `master_seed=42`, `dt=0.5`, one body, one craft, empty economy, all defaults. Re-derive via `config_hash_golden_anchor_is_stable` test (config.rs:835-846); NEVER invent the value. The `sample()` struct must also be updated when new RunConfig fields are added.

---

## 4. HASH_FORMAT_VERSION and the per-tick state hash

**File:** `crates/jumpgate-core/src/hash.rs`

```rust
// hash.rs:126
pub const HASH_FORMAT_VERSION: u32 = 5;
// v2: economy (words 16-24)
// v3: trophic (words 25-26)
// v4: pirates-rung (words 27-29: upgrades, info_tick, route_evidence)
// v5: media-rung (words 30-32: craft gossip, station gossip, next_alert_seq)

// hash.rs:132
pub const GOLDEN_ZERO_STATE_HASH: u64 = 0x0f20_843f_ccfd_8c70;
// RE-PINNED: v4->v5 (+craft/station gossip, next_alert_seq). Was 0xafdc_5c35_6266_0ff0.
```

### 4.1 What a v6 bump touches (verified from the pattern)

A v6 bump requires updating ALL of the following, in a **single-cause commit**:

1. `HASH_FORMAT_VERSION` constant at hash.rs:126
2. `GOLDEN_ZERO_STATE_HASH` at hash.rs:132 — re-derive via `print_golden` (hash.rs:1123-1127)
3. `state_hash_golden_zero_world` assertion at hash.rs:1118 — paste from `print_golden` output
4. `manual_zero_fold()` function at hash.rs:1172-1227 — add the new words explicitly
5. `golden_zero_state_hash` test at hash.rs:1235-1237 — validates `manual_zero_fold()` matches `GOLDEN_ZERO_STATE_HASH`
6. `FRONTIER_TRAJECTORY_GOLDEN` at scenario.rs:1118 — re-derive via `print_golden_frontier` (scenario.rs:1098-1107):
   ```rust
   // scenario.rs:1098-1107
   #[test]
   #[ignore = "prints the golden constant for frontier_trajectory_golden"]
   fn print_golden_frontier() {
       let (mut w, _) = World::reset(scenario_frontier(7)).expect("...");
       let mut cmds = Vec::new();
       for _ in 0..2_000 { w.step(&mut cmds); }
       println!("FRONTIER_TRAJECTORY_GOLDEN=0x{:016x}", crate::hash::state_hash(&w));
   }
   // scenario.rs:1118 (current value)
   const FRONTIER_TRAJECTORY_GOLDEN: u64 = 0x050de98bd4b6793c;
   ```
7. `frontier_trajectory_golden` test at scenario.rs:1120-1134
8. The `recompute_with_cursors` function (hash.rs:685-773) — must mirror `state_hash` exactly; a drift in this function is a test infrastructure bug

The zero-world golden (`hash.rs:1116-1119`) uses `cfg_with_craft_x(2.0)` — a minimal single-body, single-craft, empty-economy world:
```rust
// hash.rs:1116-1118
let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
// RE-PINNED: HASH_FORMAT_VERSION 4->5 ... Was 0xa29b_6334_16f7_cd20.
assert_eq!(state_hash(&w), 0x274b_6874_3b8d_2700u64);
```

### 4.2 `print_golden` — the re-derivation fixture

```rust
// hash.rs:1122-1127
#[test]
#[ignore = "prints the golden constants for state_hash_golden_zero_world AND golden_zero_state_hash"]
fn print_golden() {
    let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
    println!("GOLDEN=0x{:016x}", state_hash(&w));
    println!("GOLDEN_ZERO_STATE_HASH=0x{:016x}", manual_zero_fold());
}
```
Run with `cargo test -p jumpgate-core -- print_golden --ignored --nocapture`.

### 4.3 A1 is hash-neutral (no version bump)

The synthesis spec states A1 (`Good(u16)` newtype, `Vec<i64>` stocks) is proven hash-neutral by per-tick state-hash sequence equality cross-branch. The v6 bump has ONE cause: the per-craft `hold: Vec<Vec<(Good, u32)>>` column (word 28 new entry in the per-craft fold, after current word 28 `info_tick`). A1 does NOT bump the version.

---

## 5. `--scenario` flag parsing in `trophic_run.rs`

```rust
// trophic_run.rs:148-156
let (scenario_name, mut cfg): (&'static str, RunConfig) = match args.scenario.as_str() {
    "trophic" => ("trophic", scenario_trophic(args.seed)),
    "frontier" => ("frontier", scenario_frontier(args.seed)),
    other => {
        return Err(format!(
            "--scenario {other}: unknown scenario (trophic|frontier)"
        ));
    }
};
```
Default `scenario = "trophic"` (trophic_run.rs:66). Parsed from `--scenario <name>` (trophic_run.rs:85-87). `scenario_bazaar` will be added here as a third arm (following this exact pattern).

### 5.1 Ephemeris window guard (trophic_run.rs:161-167)

```rust
if args.ticks > cfg.ephemeris_window {
    return Err(format!(
        "--ticks {} > ephemeris_window {}: past-window orbits silently freeze...",
        args.ticks, cfg.ephemeris_window
    ));
}
```
Applied **after** knob overrides. `ephemeris_window: 120_000` for frontier, `100_000` for trophic.

---

## 6. Existing golden tests — full inventory

| Test name | File | What it pins |
|---|---|---|
| `config_hash_golden_anchor_is_stable` | config.rs:835 | `GOLDEN_CONFIG_HASH = 0x128c_1299_5c48_4fdc` via `sample()` |
| `state_hash_golden_zero_world` | hash.rs:1111 | `0x274b_6874_3b8d_2700` at tick 0 via `cfg_with_craft_x(2.0)` |
| `golden_zero_state_hash` | hash.rs:1235 | `GOLDEN_ZERO_STATE_HASH = 0x0f20_843f_ccfd_8c70` via `manual_zero_fold()` |
| `frontier_trajectory_golden` | scenario.rs:1121 | `FRONTIER_TRAJECTORY_GOLDEN = 0x050de98bd4b6793c` — seed-7 frontier stepped 2000 ticks |
| `print_golden` (ignored) | hash.rs:1122 | Re-derivation fixture for zero-world + manual_zero_fold |
| `print_golden_frontier` (ignored) | scenario.rs:1098 | Re-derivation fixture for frontier trajectory |

There is **no trophic trajectory golden** (deliberate omission, per synthesis DL5-5).

---

## 7. `scenario_bazaar` construction guidance (what to clone)

The spec (synthesis §1.3) calls for the frontier 10-station band geometry with `ephemeris_window: 240_000`. Key differences from `scenario_frontier`:

- Zero `ContractInit` rows (`contracts: vec![]`)
- Goods money/trade bases at 200_000 (PER_UNIT_BASE_MICROS)
- `dispatch_cfg.demand_low = 0`, `demand_high = 0` (REPOST structural off — burst needs `stock < demand_low`, verified at economy.rs:488-516)
- Clumped per-good topology: partitioned `mix(seed, k)` k-ranges (the FRONTIER_TIER_WIRING shape), never i.i.d. scatter
- Differentiated initial stocks (sources near cap, sinks at 0) via a shared `seed_price` helper (one integer curve expression per good)
- `GoodsCfg` fold count first (anti-aliasing delimiter, per synthesis L5-F7)
- Pirate capacity stays 5

The clumped topology k-range pattern precedent is `mix(seed, k)` as used for body phases (scenario.rs:133) — same function, different k range per good per station role.

---

## GOTCHAS

1. **FRONTIER_HAULER_EXHAUST_VELOCITY is calibrated, not spec-derived.** It is 42.5 (scenario.rs:77), derived from a measured worst-leg burn × k=2.5. Inventing a different value or deriving it from spec prose will produce a wrong constant that silently passes until `frontier_trajectory_golden` is re-run. Pirates keep 20.0 (the band spec), NOT the calibrated 42.5 — the two craft classes have different specs in `scenario_frontier`.

2. **`hideout_body_index` is body 7 in frontier, body 6 in trophic.** In `scenario_frontier` the haven is at the seam (body 7, station row 6, ~1.4660 AU), NOT the outermost body (body 10, 3.0 AU). In `scenario_trophic` it is the outermost body (body 6, 1.4 AU). Any code that copies trophic's `hideout_body_index` for frontier will wire the wrong body.

3. **Single-cause golden commits.** Each golden re-pin must be a separate commit with one named reason. The comment discipline is visible at hash.rs:132 and scenario.rs:1115-1117. The OD-1 runtime-goods A1 commit is hash-neutral and MUST land before the v6 bump commit. The v6 bump has exactly one cause: adding the `hold` word to `write_craft_economy`. If A1 is landed in the same commit as the v6 bump the causality chain is broken and later re-pins cannot be attributed.

4. **GOLDEN_CONFIG_HASH re-pin requires updating `sample()`'s struct literal, not just the constant.** `sample()` is the fixture (config.rs:785-831). Adding a new `RunConfig` field to `config_hash` without adding it to `sample()`'s exhaustive struct literal will be a compile error (Rust struct literal exhaustiveness). The literal currently ends with `refuel: RefuelCfg::default()` (config.rs:830). New fields (e.g. `goods_cfg: GoodsCfg::default()`) must be appended here, and the constant re-derived by running the test.

5. **`cap[Ore] == 0` is the structural-off switch for Ore pricing, not a coincidence.** `update_prices` skips resource rows where `cap[r] == 0` (verified by `scenario_frontier_fuel_pricing_and_port` at scenario.rs:1162-1166 and `frontier_ore_price_never_updates_and_fuel_rides_the_curve`). If a new goods-flavored resource gets `cap: 0` it will never get priced. This is the documented way to keep a resource's price dead while keeping it hashed — do not add a branch-check; change the cap.

---

*Cap: ~9.8KB. All literals verified from direct file reads; none invented.*
