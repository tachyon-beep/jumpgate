# Ground extract — lab bench (HEAD e7e490e)

All paths absolute; all line numbers verified this session.

## 1. TrophicSample — full current field list

`crates/jumpgate-core/src/diagnostics.rs:63-140`. `#[derive(Clone, Debug, Default, PartialEq, Eq)]` (line 65) — **all integers** ("samples are hash-adjacent evidence, never float analytics", line 63-64). Fields in declaration order (== JSONL order):

```
tick: u64                      // window CLOSE tick (:68)
active_pirates: u32            // :70   lying_low: u32 (:72)
laden_in_transit: u32          // :74   laden_trips: u32 (:77, ContractFulfilled count)
robs: u32 (:79)  drivenoffs: u32 (:81)
purchases_hull: u32 (:83)  purchases_escort: u32 (:85)
per_route_robs: Vec<u32>       // :87, dense from_row*n_stations+to_row
per_route_accepts: Vec<u32>    // :89
per_route_traffic: Vec<u32>    // :92 fulfilled+robbed occupancy mask
yard_treasury_micros: i64      // :95
per_craft_credits: Vec<i64>    // :97 dense row order
engagement_phase_milli: Vec<u32> // :100
// --- media lab fields, comment ":101-102: Additive: every pre-media JSONL key is untouched" ---
gossip_born: u32 (:104)  gossip_first_heard: u32 (:108, Craft-carrier only)
gossip_born_cum: u32 (:110)  gossip_escaped_cum: u32 (:113)
alerts_carried: u32 (:115)  stations_with_news: u32 (:117)
per_station_alerts: Vec<u32> (:120, empty when media off)
per_station_contacts: Vec<u32> (:123, dock EDGES per station per window)
heard_lag_ticks: Vec<u32> (:126)  heard_hops: Vec<u32> (:128)
alerts_evicted_cum: u64 (:130)
assign_decisions_cum: u64 (:132)  assign_flips_cum: u64 (:136)
assign_counts_cum: Vec<u64> (:139, buckets 0..=5 then >=6)
```

**Additive-field precedent (the assign_* clone target):** new fields APPEND at the struct end (diagnostics.rs:131-139), are gathered in `sample_window` as pure snapshots of UNHASHED world diag state (`world.assign_diag.decisions/flips/candidate_counts.to_vec()`, diagnostics.rs:601-603), and append at the END of `sample_json` (trophic_run.rs:172-174) under the comment "ADDITIVE: every pre-media key above is byte-untouched" (trophic_run.rs:159-160).

## 2. sample_window

`pub fn sample_window(world: &World, window_start: Tick) -> TrophicSample` — diagnostics.rs:437. Pure read, no mutation. Gathering pattern:
- Windowed event counts: one `for e in world.recent_events(Tick(window_start.0.saturating_add(1)))` loop matching `e.kind` (diagnostics.rs:453-505); `saturating_add` everywhere.
- Run-cumulative reads: a second full scan `world.recent_events(Tick(0))` (diagnostics.rs:510-524).
- Instantaneous snapshots at sample point: ships/gossip buffers (526-535), `media_diag.contacts` filtered to `t.0 > window_start.0 && t.0 <= tick.0` (537-544), per-craft loop over `world.ships.pirate/cargo` (548-559), yard treasury via `world.corporations.treasury_micros.get(world.shipyard_cfg().corp_index as usize)` (575-580), `per_craft_credits: world.ships.credits_micros.clone()` (581), `engagement_diag` filtered by tick range (584-589).
- `WINDOW_TICKS: u64 = 2000` — diagnostics.rs:24.
- `route_of(world, contract) -> Option<usize>` is `pub` (diagnostics.rs:611-622), dense `fr*n + tr`.

**Where per_station_fuel_stock/price read from:** `World.stations: crate::economy::StationStore` is **pub(crate)** (world.rs:59) — readable from diagnostics.rs (same crate), NOT from the example binary (this is spec TROPHIC-C2: data must flow through `sample_window`). `StationStore { stock: Vec<[i64; N_RESOURCES]>, price_micros: Vec<[i64; N_RESOURCES]> }` — economy.rs:44-45. `Resource::Fuel.index() == 1`, `N_RESOURCES = 2` (`[Ore, Fuel]`) — economy.rs:9-21. So e.g. `world.stations.stock.iter().map(|s| s[Resource::Fuel.index()]).collect()`.

**Fuel on craft:** `CraftStore.fuel_mass: Vec<f64>` (stores.rs:160), capacity via `spec: Vec<BaseSpec>` (`base_fuel_capacity`, see stores.rs test at :64) × mods. Note stores.rs:178: traded `Fuel` resource is CARGO, distinct from propellant `fuel_mass`. Tank permille for an integer sample must be derived from f64 — spec §7 pins FLOOR rounding for `tank_before_permille`.

## 3. trophic_run.rs — anchored stdout lines (exact formats)

`crates/jumpgate-core/examples/trophic_run.rs` (477 lines). Emission order in `main`: header (332), per-window human lines (340-355), `laden_trips_per_window` (360), `diagnosis` debug line (368), **RESULT** (383-396), **MEDIA** (410-418), **ASSIGN** (426-437), then gossip-log/chronicle/fuel-assert/replay-check.

```
RESULT seed={} ticks={} verdict={:?} cycled={} risk_heterogeneous={} outcomes_disperse={} fuel_empty={} robs={} laden_trips={} purchases={}
MEDIA seed={} born={} escaped_milli={} median_lag={} p90_lag={} reading={:?}
ASSIGN seed={} decisions={} flips={} flip_milli={} counts={:?}
```
(RESULT trophic_run.rs:383-396; MEDIA :410-418 — lags pool over all windows, integer lower-median `lags[(len-1)/2]` and p90 `lags[(len*9/10).min(len-1)]`, **0 sentinel when empty** (:407-409) — the "zero-refuel sentinel MEDIA precedent" the spec cites for FUEL; ASSIGN :426-437, guarded `if let Some(last) = samples.last()`, `flip_milli = flips*1000 / decisions.max(1)`.)

**Where META slots:** spec §8 wants `META seed= scenario= stations= haulers= pirates_initial= station_radii_milli_au=[…]`. Natural slot: with the other anchored lines, before/beside RESULT (the existing non-anchored header at :332-339 already prints seed/ticks/windows/sets but is NOT machine-parsed). The "lockstep rule": an anchored line and its sweep regex land in the SAME commit (trophic_run.rs:398-401, sweep_trophic.py:58-59).

**No FUEL line exists today.** Fuel observability today = (a) `fuel_empty` count computed post-run by scanning `recent_events(Tick(0))` for `EventKind::FuelEmpty` (trophic_run.rs:371-375), printed inside RESULT; (b) `--assert-no-fuel-empty` exit-nonzero check (:447-450). TrophicSample has ZERO fuel fields. No fuel knob/stat is sampled per window anywhere.

**JSONL emission:** `sample_json` (trophic_run.rs:142-177) — `serde_json::json!` field-for-field, key names == struct field names, written per window inside `simulate` (:129-136). `simulate` rebuilds config per run from `(seed, sets)`: `scenario_trophic(args.seed)` + `apply_knob` loop (:113-118). `HASH_SAMPLE_EVERY = 1000` (:36) for the replay check.

**Gossip log** (`--gossip-log`, trophic_run.rs:186-236): one JSONL line per event, keys `e`("born"/"heard"/"rob"/"accept"), `tick`, plus per-kind keys; carrier encoding `"s<row>"`/`"c<slot>"` (:212-215).

## 4. sweep_trophic.py — parsing contract

`/home/john/jumpgate/python/analysis/sweep_trophic.py` (365 lines).
- `RESULT_RE` (:51-56) and `MEDIA_RE` (:60-64): **fully anchored `^...$` regexes**; named groups, all values kept as strings.
- `run_one` (:89-118) shells `cargo run -q -p jumpgate-core --release --example trophic_run`, scans stdout; **hard `SystemExit` if RESULT (:111-113) or MEDIA (:114-116) line missing** — this is what "version-gated parsing" must relax: today a missing anchored line kills the sweep; new META/FUEL parsing must tolerate banked pre-FUEL outputs. **There is NO existing version-gating precedent in any parser** (grep for "version" in python/analysis returns nothing); the only related discipline is the lockstep rule + media_log.py's `MediaCfg` mirror CLI defaults (`--evidence-window 4000 --sig-floor 50 --sig-divisor 10000 --hop-loss 150`, media_log.py:198-202).
- **The N_HAULERS mirror the spec kills:** `N_HAULERS = 12` at sweep_trophic.py:70-72 ("mirrors scenario::NUM_HAULERS"), used ONLY in `voi_line`'s wallet slice `ws[-1]["per_craft_credits"][:N_HAULERS]` (:288). Rust source of truth: `pub const NUM_HAULERS: usize = 12` scenario.rs:40 (haulers are dense rows 0..12, pirates after). META's `haulers=` field replaces it.
- Per-knobset `panel()` (:142-218): verdict Counter, fuel_empty list (:151-152), trip-phase histogram (PHASE_BINS=10, bin `min(p*10//1001, 9)`, `#`-bar scaled to peak·40, :156-167), purchase-desync spread (:169-176), yard treasury monotone check (first run only, :179-185), per-seed line with per-window normalized HHI + run-aggregate HHI (:195-216).
- `media_panel` (:221-265): reading Counter, escaped_milli list, pooled lag histogram (LAG_BINS=10, ceil width), news-geography hub/backwater ratio per seed (max/min summed per_station_alerts over stations with contacts>0).
- `occupied_hhi_milli` (:121-127): HHI over routes with traffic>0, integer milli, None if no robs.
- `voi_line` (:273-298): media-on arm detected by `station_gossip_slots>0` knob (:268-270); median final hauler credits per arm; "REPORTED, NEVER GATED".
- Default knobsets (:319-322): `baseline` + `control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000`.
- **Hungry-roamer positive control restatement** (:355-361): counts control runs reading `RiskEqualized`, "expected ALL — anything else means the instrument is broken". Recipe rationale docstring :31-40; also trophic_run.rs:11-18.

**Panel templates for I1/I2:** `media_panel` (sweep_trophic.py:221) is the in-sweep template; `/home/john/jumpgate/python/analysis/media_log.py` (227 lines) is the standalone-script template — argparse with positional JSONL + optional `--windows` (the runner's per-window file), `load()` helper (:33-35), per-panel functions printing "REPORTED, never gated"/"windows, not gates — PDR-0006" framing, pre-registered band stated inline in the print (:89-94). I1 `haves_havenots.py` / I2 radial-zone should clone this shape.

## 5. HHI/slack calibration constants + fit method

- `pub const HHI_NORM_MIN_MILLI: u64 = 2204` — diagnostics.rs:44. Doc :30-43: fitted against the 2026-06-11 **labeled real-run set** (filigree jumpgate-50c6a8a3bd; 50k ticks): TRUE-clumped baseline s7/s23/s42/s99 measured 3070/2962/2918/3498 vs TRUE-equalized hungry-roamer control s7/s23 = 1490/1472; **threshold = margin midpoint (2918+1490)/2 = 2204**. Run-aggregate HHI does NOT separate (548-714 vs 551-629 normalized) — stays printed as context only (sweep_trophic.py:187-194).
- `pub const HOT_PERSISTENCE_SLACK_CHANGES: u32 = 3` — diagnostics.rs:57. Doc :46-56: same labeled set; clumped hot-change excess max +1, equalized min +5; **slack = (1+5)/2 = 3**. Used at diagnostics.rs:314-315: `mean_norm_milli >= HHI_NORM_MIN_MILLI && hot_changes <= traffic_changes + SLACK`.
- Spec §8 requires re-fit on BOTH maps post-haven-leak-fix using this same labeled-run method (held-out seeds, frontier positive control must read RiskEqualized first).
- `OUTCOME_DISPERSION_MIN_MILLI: u64 = 200` — diagnostics.rs:61.
- `COMMON_KNOWLEDGE_ESCAPE_MILLI: u32 = 950` — diagnostics.rs:380.

## 6. media_classify + escaped_milli (W1 reads)

- `escaped_milli(samples) -> u32` — diagnostics.rs:385-394: `1000*gossip_escaped_cum/gossip_born_cum` at the LAST sample (fields run-cumulative); 0 sentinel when nothing born.
- `media_classify(samples) -> MediaReading` — diagnostics.rs:400-431. Precedence: NoMedia (born==0) → NewsDesert (heard==0) → StaleEcho (late half: heard==0 && robs>0 && mean carried >=1, :413-422) → CommonKnowledge (escaped_milli>=950 AND final-window full station coverage, :423-429) → Localized (the alive reading). Spec §8 re-denominates coverage to **contract-endpoint stations** (scenario-conditional) — today's check is `stations_with_news as usize == per_station_alerts.len()` (:426), i.e. ALL stations.

## 7. Existing test names + assertion style (diagnostics.rs)

Classifier synthetics via helper `fn s(tick, active, laden, robs_by_route, traffic, credits)` building TrophicSample with `..Default::default()` for media fields (:630-657); media synthetics via `fn m(...)` (:904). Builders: `cycling_heterogeneous` (:667, doc notes its values sit inside the labeled TRUE-clumped band), `early_burst_then_silence` (:689, "seed-7's caught lie"), `sparse_two_hot_of_nine` (:774). Tests (one per matrix row + traps): `early_burst_then_silence_reads_permanent_peace` :706, `cycling_heterogeneous_reads_alive` :716, `flat_reads_no_cycle` :725, `cycling_equalized_reads_risk_equalized` :735, `cycling_hetero_no_dispersion_reads_decision_not_translating` :759, `sparse_clumped_minority_routes_read_heterogeneous` :798, `sample_window_counts_purchases_and_reads_yard_treasury` :812 (builds a full minimal RunConfig in-test :824-863, steps a real world, asserts each field with message strings; media-OFF world asserts all media fields zero/empty :881-893), media tests `media_no_media_when_nothing_born` :929 through `media_localized_is_the_alive_reading` :995 — each `assert_eq!(media_classify(&samples), MediaReading::X)`. House rule (comment :896-899): "every new metric ships with a synthetic that would catch it lying".

## GOTCHAS

1. **`World` fields are `pub(crate)`** (world.rs:54-85): the example binary CANNOT read `world.stations`/`ships.fuel_mass` directly. New per-station fuel stock/price and pirate-location fields MUST be added to TrophicSample and gathered inside `sample_window` (spec TROPHIC-C2 names this).
2. **TrophicSample is all-integer by law** (diagnostics.rs:63-64) and derives `Default`+`Eq` — no f64 fields ever (Eq would break); fuel quantities enter as permille/micros integers (FLOOR rounding pinned for tank permille, spec §7). New fields must append at the END of struct + `sample_json` to keep pre-existing JSONL keys byte-untouched (the media/assign additive precedent).
3. **sweep_trophic.py hard-exits on a missing anchored line** (run_one :111-116) and both regexes are `^...$`-anchored — adding a token to RESULT/MEDIA breaks the parser; new META/FUEL lines are new regexes, version-gated (made optional for banked pre-FUEL stdout). There is NO existing version-gating precedent — you are creating it; the only existing discipline is the lockstep rule (line + regex in the same commit, trophic_run.rs:398-401 / sweep_trophic.py:58-59).
4. **No FUEL line or per-window fuel field exists today** — only the post-run `fuel_empty` event count inside RESULT (trophic_run.rs:371-375). Don't "extend" something that isn't there; clone the MEDIA line+regex+0-sentinel pattern (trophic_run.rs:398-418).
5. **`fuel_empty=0` semantics flip on frontier** (spec §8): on trophic arms it's the endurance window (`--assert-no-fuel-empty` stays there ONLY); on frontier it's texture ("no stranding this seed"). Also: craft propellant is `ships.fuel_mass` (f64, stores.rs:160); the traded `Resource::Fuel` (index 1, economy.rs:21) in `stations.stock`/`price_micros` is cargo-side — two different "fuels", don't conflate.
6. `N_HAULERS=12` (sweep_trophic.py:70-72) is used ONLY in `voi_line`'s wallet slice (:288); the META `haulers=` field replaces it — leave the constant nowhere else to hide (grep shows no other Python use).
