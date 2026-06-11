# Grounding: scenario factories + config + golden discipline
Verified at HEAD e7e490e (branch jumpgate-v1-design). All paths under
`/home/john/jumpgate/crates/jumpgate-core/` unless noted.

## 1. `scenario_trophic` — the factory `scenario_frontier` clones
File: `src/scenario.rs`. Signature: `pub fn scenario_trophic(seed: u64) -> RunConfig` (scenario.rs:70). Pure config; same seed ⇒ identical config_hash (scenario.rs:67-69).

Constants the frontier factory will parallel:
- `pub const STATION_ORBIT_AU: [f64; 6] = [0.35, 0.56, 0.77, 0.98, 1.19, 1.40];` (scenario.rs:37) — "body index k+1 hosts station row k; body 6 (1.4 AU) is the outermost — the hideout" (scenario.rs:34-36)
- `NUM_HAULERS = 12`, `NUM_PIRATES = 6` (scenario.rs:40,43)
- `PER_UNIT_BASE_MICROS: i64 = 200_000` (scenario.rs:46); `TIERS: [(u32, i64); 3] = [(5, 1000), (10, 1150), (15, 1300)]` (qty, per-unit mult in milli) (scenario.rs:51)

Seed-derived phases (anti-memorization): SplitMix64-style `fn mix(seed: u64, k: u64) -> u64` (scenario.rs:55-60) + `fn u64_to_unit_f64(x: u64) -> f64` top-53-bits (scenario.rs:63-65). Bodies: star `STAR_MASS 1.0e-3`, `BODY_MASS 1.0e-12` (scenario.rs:71-72); per station body `m0 = u64_to_unit_f64(mix(seed, (k+1) as u64)) * TAU`, elements `{a, e:0, i:0, raan:0, argp:0, m0}` (scenario.rs:79-85).

Craft spec (scenario.rs:89-95):
```rust
let spec = BaseSpec {
    base_dry_mass: 1.0e-9,
    base_max_thrust: 1.0e-12,
    base_exhaust_velocity: 20.0,   // ×10 vs trader's 2.0 (fuel endurance, spec §6)
    base_fuel_capacity: 1.0e-9,
    base_cargo_capacity: 5,
};
```
Co-orbit spawn closure `co_orbit(body_index) -> (Vec3, Vec3)`: circular velocity `v_circ = (G_CANONICAL*(STAR_MASS+BODY_MASS)/el.a).sqrt()`, pos/vel on the seeded phase (scenario.rs:98-105). Haulers: `co_orbit(1 + (k % STATION_ORBIT_AU.len()))`, `CraftInit { spec: spec.clone(), pos, vel, fuel_mass: 1.0e-9, role: CraftRole::Idle, scripted: true }` (scenario.rs:107-117). Pirates: all spawn at `co_orbit(STATION_ORBIT_AU.len())` (hideout), `role: CraftRole::Pirate` (scenario.rs:118-130). NOTE `fuel_mass: 1.0e-9` == `base_fuel_capacity` == today's `FUEL_EMPTY_EPS` (`pub const FUEL_EMPTY_EPS: f64 = 1e-9;` src/events.rs:16) — tank==eps, the FuelEmpty edge (`fuel_now <= EPS && fuel_prev > EPS`, events.rs:50) can never arm; the spec §4 re-bake (eps→1e-11, v_e calibrated) exists because of this.

Stations (scenario.rs:150-157): six `StationInit { body_index, initial_stock: stock(ore, fuel), initial_price_micros: [0, 0], sells_upgrades }`; `stock(ore, fuel)` helper builds `[i64; N_RESOURCES]` by `Resource::Ore.index()`/`Resource::Fuel.index()` (scenario.rs:144-149). Per-tier Schmitt-stagger initial stocks 18/14/10: source fuel stocks (18,14,10 at stations 0-2) and dest Ore stocks (18,14,10 at stations 3-5) against the ONE global 10/20 band — comment scenario.rs:138-143 explains the DEVIATION: `DispatchCfg` carries one global band and is config-golden-frozen, so per-tier Schmitt offsets are realized as per-tier INITIAL stocks + per-tier destination stations. Vendors (`sells_upgrades: true`) at station rows 3 and 6 only (scenario.rs:154,156); the hideout's station MUST be a vendor — the pirate-haven `resolve_purchases` settle path (scenario.rs:132-136, test scenario.rs:358-362).

Producers (scenario.rs:158-171): Ore miners (output 5/interval 40) at stations 0-2; refiners Ore→Fuel (5/60) at 3-5; Fuel sinks (input 5/80) back at 0-2.

Corps (scenario.rs:174-179): 3 tier corps `treasury_micros: 2_000_000_000`, plus the Yard at index 3 with treasury 0, `home_station_index: 3`.

Contracts (scenario.rs:185-207): per tier, `reward = qty as i64 * PER_UNIT_BASE_MICROS * mult_milli / 1000`; 3 Ore legs from {0,1,2} → `dest = 3 + tier`, plus 1 Fuel return leg dest→tier. 12 total.

TrophicCfg (scenario.rs:216-227) — console-session-2 band:
```rust
let trophic = TrophicCfg {
    engage_radius_au: 5.0e-4, upkeep_per_tick: 12,
    food_per_unit_micros: 10_000, grubstake_micros: 100_000,
    ransom_cap_micros: 6_000_000, starve_lie_low_ticks: 4_000,
    hideout_body_index: 6, hauler_belief_scoring: true,
    hauler_buy_policy: BuyPolicy::EscortFirst,
    ..TrophicCfg::default()
};
```
**`pirate_max_reach_au` IS inherited silently via `..TrophicCfg::default()` = 0.6** (default at src/config.rs:303; scenario sets no override; the only mentions of reach in scenario.rs are the knob arm :288 and the test :492-494). Same silent inheritance: `p_rob_milli 700`, `relocate_period 2500`, `stay_milli 500`, `evidence_window 4000`, etc. (config.rs:286-312). Spec §2-3 makes frontier reach EXPLICIT in the factory.

RunConfig tail (scenario.rs:229-253): `dt: Dt::new(0.25)`, `softening: 1.0e-4`, `substep_cfg { accel_ref: 3.0e-4, max_substeps: 64 }`, **`ephemeris_window: 100_000`** (scenario.rs:234; spec wants frontier 120_000 + a runner guard — today `Ephemeris::body_pos` silently CLAMPS ticks past the window: `.min(self.n_samples.saturating_sub(1))`, src/ephemeris.rs:106-111), `price_cfg: PriceCfg::default()`, `dispatch_cfg: DispatchCfg { demand_low: 10, demand_high: 20, stagger_period: 16, contract_reward_micros: 0, contract_qty: 0 }`, `shipyard: ShipyardCfg { corp_index: 3, ..default() }`, `media: MediaCfg::default()`.

## 2. `apply_knob` — the precedent for `craft.fuel_capacity_scale`
`pub fn apply_knob(cfg: &mut RunConfig, name: &str, value: &str) -> Result<(), String>` (scenario.rs:260). Inner generic parser `fn p<T: FromStr>(name, value) -> Result<T, String>` with `format!("--set {name}={value}: {e}")` errors (scenario.rs:261-266). Bindings `let t = &mut cfg.trophic; let y = &mut cfg.shipyard; let m = &mut cfg.media;` (scenario.rs:267-269). The newest single-field arm — the clone template:
```rust
"staleness_from_rob_tick" => m.staleness_from_rob_tick = p(name, value)?,
```
(scenario.rs:325). Unknown knob: `other => return Err(format!("--set {other}: unknown knob"))` (scenario.rs:326) — unknown/malformed are ERRORS by design ("a silent typo in a sweep grid would poison a whole matrix read", scenario.rs:258-259). NOTE: `fuel_capacity_scale` does NOT exist yet anywhere (grep clean); also note knobs currently target only cfg sub-structs — a craft-spec knob (mutating every `cfg.craft[i].spec.base_fuel_capacity`) has NO precedent arm; the dispatch arms `"demand_low" => cfg.dispatch_cfg.demand_low = ...` (scenario.rs:313-315) are the closest "not via a binding" shape. Knob tests: `apply_knob_overrides_and_rejects_unknown` (scenario.rs:489-514) — style: apply, assert field, then 3 loud-error asserts (`.is_err()` on unknown/malformed/bad-enum).

## 3. Runner scenario selection — there is NO `--scenario` flag yet
`examples/trophic_run.rs`: `scenario_trophic(args.seed)` is hardcoded in `simulate()` (trophic_run.rs:113); `parse_args()` (trophic_run.rs:55-101) knows `--seed --ticks --jsonl --chronicle --chronicle-gossip-min-micros --replay-check --assert-no-fuel-empty --gossip-log --set` and errors on anything else (`other => return Err(format!("unknown arg: {other}"))`, trophic_run.rs:97). Spec §9 phase 2 adds the `--scenario` flag. `--set` parsing splits on `=` and pushes `(String, String)` into `args.sets` (trophic_run.rs:90-96); knobs applied after factory build (trophic_run.rs:114-116). Public exports: `pub use scenario::{apply_knob, scenario_trophic};` (src/lib.rs:71). `WINDOW_TICKS = 2000` (src/diagnostics.rs:24).

## 4. config.rs — the MediaCfg fold precedent for RefuelCfg
RunConfig fields end `..., trophic: TrophicCfg, shipyard: ShipyardCfg, media: MediaCfg` (config.rs:407-436); media comment: "folded AFTER shipyard, append-only" (config.rs:433-435). Adding RefuelCfg = clone the MediaCfg pattern exactly:
1. Struct + Default, `#[derive(Clone, Copy, Debug)]`, inert-by-default doc (MediaCfg at config.rs:357-398; `lot_mass == 0.0` ⇒ inert is spec §5's analogue of "both slot caps 0").
2. Field on RunConfig at the TAIL (after `media`).
3. `config_hash` (config.rs:499-715): the top-level **exhaustive destructure** `let RunConfig { ..., media, } = self;` (config.rs:503-521) makes a new field a COMPILE ERROR until folded (D10/M6, config.rs:500-502). Fold at the very tail with its own exhaustive destructure, e.g. MediaCfg (config.rs:690-713): `let MediaCfg { station_gossip_slots, ... } = media;` then `h.write_u64(*field as u64)` per field in declaration order; f64 via `.to_bits()`, bool `as u64`, enums via `.rank()` (BuyPolicy::rank config.rs:209-218; APPEND-ONLY discriminants).
4. Extend the `CONFIG_FIELD_ORDER` doc list (config.rs:480-498; media is entry 25 — RefuelCfg becomes 26): "append-only; re-pin the golden on change" (config.rs:480).
5. Add a `changing_*_changes_config_hash` test — clone `changing_media_cfg_changes_config_hash` (config.rs:989-999): mutate one field off `sample()`, `assert_ne!` against `sample().config_hash()`, repeat per representative field.
6. Update `sample()` (config.rs:747-793) with `refuel: RefuelCfg::default()`.

Config hash internals (do not touch): FNV-1a, `CONFIG_FNV_OFFSET 0xcbf2_9ce4_8422_2325`, prime `0x0000_0100_0000_01b3`, `"CONFIG_1"` tag `0x434f_4e46_4947_5f31` (config.rs:445-457); DISTINCT from the per-tick state hash (config.rs:1-7).

### Golden literal + re-derivation workflow
```rust
const GOLDEN_CONFIG_HASH: u64 = 0xee02_df67_1889_78dc; // RE-PINNED: +media.staleness_from_rob_tick (WHY-panel probe knob). Was 0x5fda_1f2f_edf2_355c.
```
(config.rs:745). **The memory files say 0x5fda… — that is STALE; current is 0xee02_df67_1889_78dc.** Comment discipline: cause of re-pin + previous value, on the literal's line. Anchor test `config_hash_golden_anchor_is_stable` (config.rs:795-807) with message "config_hash drifted: re-pin only if intentional". Re-derivation: ignored printer test (config.rs:1026-1030)
```rust
#[test]
#[ignore = "prints the golden constant for config_hash_golden_anchor_is_stable"]
fn print_golden_config() { println!("GOLDEN_CONFIG_HASH=0x{:016x}", sample().config_hash().0); }
```
Run: `cargo test -p jumpgate-core --lib print_golden_config -- --ignored --nocapture`, paste output. Single-cause commit precedent: `88a5d85` "staleness_from_rob_tick probe knob — ... GOLDEN_CONFIG_HASH re-pinned, single cause" and `1795c57` "commit A — MediaCfg folded at config tail (GOLDEN_CONFIG_HASH re-pinned, single cause)" (git log on config.rs). Spec §9: exactly ONE re-pin this rung (RefuelCfg fields) + one NEW frontier trajectory golden; zero existing goldens move.

### PriceCfg — where it sits and trophic values
`pub struct PriceCfg { base_micros: [i64; N_RESOURCES], cap: [i64; N_RESOURCES], slope_milli: i64, reprice_interval: u32 }` (config.rs:143-150); curve doc config.rs:137-141: `price = base*(2000 - min(stock,cap)*slope_milli/cap)/1000`, clamped ≥0. Default: `base_micros: [0; N]`, `cap: [1; N]`, `slope_milli: 1800`, `reprice_interval: 1` (config.rs:152-161). **scenario_trophic uses `PriceCfg::default()`** (scenario.rs:242) — i.e. all base prices ZERO today; spec §5's reset error ("lot_mass > 0 while price_cfg.base_micros[Fuel] == 0") and §6's frontier values (`base_micros [0, 5_000]`, `cap [0, 40]`, slope 1800) hang off this. It sits in RunConfig between `contracts` and `dispatch_cfg` (config.rs:427); hash fold at config.rs:597-602 (CONFIG_FIELD_ORDER 19).

### ephemeris_window
`pub ephemeris_window: u64` "ticks precomputed in the ephemeris window" (config.rs:415-416), folded as hash word 6 (config.rs:485,529). Hash test `changing_ephemeris_window_changes_hash` (config.rs:884-889).

### Other structs drafters will touch
- `CraftInit { spec: BaseSpec, pos: Vec3, vel: Vec3, fuel_mass: f64, role: CraftRole, scripted: bool }` (config.rs:44-57); role default-doc + scripted gym-exclusion doc inline.
- `BaseSpec` fields config.rs:15-24 (`base_cargo_capacity` is DERIVED-read: `base + hulls * hull_step_units`, never stored).
- `StationInit { body_index: usize, initial_stock, initial_price_micros, sells_upgrades: bool }` (config.rs:101-108). `N_RESOURCES = 2`, `Resource::{Ore, Fuel}` (src/economy.rs:9-17).
- `TrophicCfg` full field list config.rs:229-282; defaults 284-313 (`hideout_body_index: 0`, `pirate_max_reach_au: 0.6`).
- `CraftRole { Idle, Hauler, Pirate }` + `rank()` (src/stores.rs:76-88).

## 5. Factory test style (clone for `scenario_frontier_shape`)
`scenario_trophic_shape` (scenario.rs:337-466): plain asserts with message strings; checks bodies count/ascending axes/band, station body_index vector, vendor count + hideout-station-is-vendor, craft counts by role, all-scripted, exhaust 20.0, the Saturated-guard arithmetic (scenario.rs:377-385), food-band identities (rob ≥ 2 windows upkeep; grubstake > 1 window; ransom ≥ escort L1 — scenario.rs:387-404), tier reward formula per contract, per-tier destination disjointness (Schmitt decoupling, scenario.rs:424-444), dispatch/belief/buy-policy values, hideout==outermost (scenario.rs:454-461 — spec §3 REPLACES this with a seam-haven assertion on frontier), and ends `World::reset(cfg).expect("scenario_trophic must resolve")` + pirate-row count (scenario.rs:463-465). `scenario_is_seed_derived_and_deterministic` (scenario.rs:468-486): hash-equal same seed, hash-differ across seeds, m0 differ.

## GOTCHAS
1. **Golden literals are NEVER invented or arithmetic-derived.** Re-pin = run `cargo test -p jumpgate-core --lib print_golden_config -- --ignored --nocapture`, paste the printed hex into config.rs:745, keep the "RE-PINNED: <cause>. Was 0x<old>." comment, and land it in a SINGLE-CAUSE commit (one config-surface change per re-pin — precedent 88a5d85/1795c57). Memory/older docs cite 0x5fda… — stale; current golden is 0xee02_df67_1889_78dc.
2. **A new RunConfig field will not compile until folded** — the exhaustive destructure in `config_hash` (config.rs:503-521) is deliberate. Fold APPEND-ONLY at the tail, extend CONFIG_FIELD_ORDER doc (config.rs:480-498), give the sub-struct its own exhaustive destructure, and update `sample()`.
3. **`pirate_max_reach_au = 0.6` reaches scenario_trophic silently** via `..TrophicCfg::default()` (scenario.rs:226 ← config.rs:303). The frontier factory must set reach EXPLICITLY (spec §2: the 8-9 gap 0.637 AU > 0.6 is a designed never-opens seam).
4. **Tank == eps today**: `fuel_mass`/`base_fuel_capacity` 1.0e-9 == `FUEL_EMPTY_EPS` (events.rs:16) — FuelEmpty is unfireable in trophic, by construction. Any frontier fuel work rides the spec-§4 re-bake order (eps commit first, fixtures REDESIGNED not literal-nudged).
5. **No `--scenario` flag and no `fuel_capacity_scale` knob exist yet** (trophic_run.rs:55-101 errors on unknown args; apply_knob errors on unknown knobs). Also `ephemeris_window` is 100_000 and `Ephemeris::body_pos` silently clamps past it (ephemeris.rs:106-111) — frontier needs 120_000 + the runner abort guard, or long runs freeze every orbit with no error.
6. **DispatchCfg is one GLOBAL Schmitt band** (10/20) — per-tier offsets are encoded as initial stocks 18/14/10 + disjoint per-tier destinations (scenario.rs:138-157), not per-tier config. Don't "fix" this without a golden re-pin decision.
