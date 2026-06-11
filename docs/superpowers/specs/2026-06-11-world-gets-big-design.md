# The World Gets Big — map growth + the propellant loop (scenario_frontier design)

**Status: APPROVED — owner resolved OD-1…OD-7 to the recommended options (2026-06-11, "start planning the 'world gets big' sprint", no overrides). Build authorized; §13 records the resolutions.**

Designed 2026-06-11 by a 15-agent panel (4 citation-grade grounding readers, 5
design lenses, 5 adversarial critics, 1 synthesis) over the owner-approved
direction ("the world gets big": map growth + refuel, one rung) and the measured
findings of the media rung's basin-clean ensemble. Every load-bearing code claim
was independently re-verified against HEAD (`db08b51`) by the main loop.
Predecessor specs: `2026-06-11-media-rung1-gossip-design.md` (esp. §9
pre-registered saturation risk, §15 deferred ladder),
`2026-06-10-pirates-rung1-predation-and-upgrades-design.md`.

Panel raw output banked at
`docs/superpowers/posts/2026-06-11-world-gets-big-panel/` (the capture practice).

---

## 1. Frame and the play bar (PDR-0006)

This rung is judged at the console by one arc — **the world is finally bigger
than the news: a frontier hauler works rich lanes the core barely hears about,
pays frontier prices for propellant, and sometimes doesn't make it home; the
chronicle gains the ship that ran dry, the lane nobody warned, and the station
that sold its last tank of fuel at four times the core price.**

Pre-registered watchable shapes (windows, never gates — full list §11):
news deserts on the frontier; the robbed→broke→stranded chain narrated
end-to-end; per-hauler knowledge horizons that differ by where they work;
regional boom-bust; the gossip-vs-ring flip share's surviving fraction finally
carrying VALUE (the media rung measured it cost-free at 6 stations but
value-free — the map was smaller than the news).

Three measured motivations (2026-06-11): saturation (6/6 alerts reach >800‰ of
craft; ~3-hop diameter; the media spec's risk #1 whose honest fix is MAP
GROWTH); the basin-clean retention-bleed arc resolved by `staleness_from_rob_tick`
(88a5d85) leaving asymmetry cost-free but value-free; and the fuel ledger —
**FuelEmpty is arithmetically unfireable in the band today** (tank 1e-9 at v_e
20 = 80,000 full-throttle ticks ≥ any run; worse, tank == FUEL_EMPTY_EPS so the
edge predicate `prev > eps` can never arm — verified events.rs:16/50,
ship.rs:42-48, scenario.rs:89-95). Making fuel real is a deliberate re-bake
(§4), not a side effect.

Everything below is hashed-integer or pure-f64-physics, bounded, reset-sized,
inert by default on existing scenarios, and a pure function of (config, command
log, named RNG streams). No gym change, no reward change, no new RNG stream.

## 2. The map: `scenario_frontier` — 10 stations, geometric band 0.35→3.0 AU

`FRONTIER_ORBIT_AU: [f64; 10]` = `a_k = 0.35·r^k`, `r = (3.0/0.35)^(1/9)` →
`[0.35, 0.4444, 0.5642, 0.7163, 0.9095, 1.1547, 1.4660, 1.8613, 2.3631, 3.0]`
(endpoints exact, law pinned by test; main-loop recomputed). Radial gaps run
0.094 → 0.637 AU; against `pirate_max_reach_au = 0.6` the 8–9 gap (0.637)
**never opens** — one hop haulers can fly and pirates can never walk. Inner
adjacent pairs are open ~54% of synodic phase; the outermost pair 0%. Outer
synodic periods (~1.4M ticks) freeze frontier adjacency per seed — the
cross-seed haves/have-nots substrate. Star + 10 station bodies (body k+1 hosts
station row k), seed-derived phases via the existing `mix` (anti-memorization
unchanged). Trip times scale **√d** (verified: the cruise cap never binds at
band thrust; peak speed is curve-limited): worst leg ~1010 ticks vs today's
~445 — pre-registered lag expectations come from hop-chain length, not leg time.

Populations: **20 haulers (2/station), 10 pirates** — a 2:1 predator:prey
DESIGN CHOICE carried from the band, not a guard-derived cap (the Saturated
guard's integer floor admits up to 13 at n=10; guard kept as ceiling
documentation). Physics block verbatim from the band: dt 0.25, softening 1e-4,
substeps {3e-4, 64}; brakability arithmetic unchanged (6.25e-5 < 1e-4; note the
dt ceiling is ~0.316 at these specs — dt is NOT a trip-compression lever).
`ephemeris_window: 120_000` + a NEW runner guard aborting when
`ticks > ephemeris_window` (today the ephemeris silently clamps —
ephemeris.rs:106-111 — and a long run would freeze every orbit).

n² growth (route vectors 36→100, gossip reservoirs ×10/6) is trivial; all
sizing is reset-derived and n-generic already (verified world.rs reset,
RouteEvidence sizing, per-route lab vectors).

## 3. Wiring: partitioned tier loops (the self-averaging fix — wiring, not AU)

The critique proved (GEO-C2) that scaling today's hub-and-spoke — all tiers
sourcing at {0,1,2} with a position-blind reward-argmax ASSIGN — keeps gossip
diameter ~3 hops at ANY width: the rung's motivation #1 would fail by
construction. The traffic graph is therefore **partitioned by radial band**:

- **Tier 0 (1.0M, qty 5, core):** sources {0,1} → dest 2; Fuel return 2→3
  (sink at 3). Vendor at 2.
- **Tier 1 (2.3M, qty 10, mid):** sources {3,4} → dest 5; Fuel return 5→4
  (sink at 4). Vendor at 5.
- **Tier 2 (3.9M, qty 15, frontier):** sources {7,8} → dest 9; Fuel return
  9→8 (sink at 8). Vendor at 9. The tier-2 return rides the never-walkable gap.
- **Haven = station 6 (1.4660 AU, body 7, `hideout_body_index = 7`):** vendor
  (the pirate escort settle path requires a vendor at the hideout dock —
  verified economy.rs `run_purchase_policies`), hosts NO producer and NO
  contract endpoint — a dark port at the seam.

Factory invariants (tests): per-tier dests/sinks disjoint; every station in
sources∪dests∪sinks∪{haven}; haven in no contract endpoint; every tier loop
touches a vendor (the heavy-haulers-shop-where-they-deliver mechanism,
restored); haulers ≡ 0 mod n; the seam-haven assertion replaces the old
hideout-outermost factory law; per-tier Schmitt-stagger initial stocks carried.

Honest registration: ASSIGN is position-blind, so cross-band deadheads exist
and the capacity ladder (qty 10/15 vs base 5) is the *plausible* workplace
stratification mechanism — W4/W5 measure whether locality emerges; the
registered alternative is "the graph still mixes → dispatch locality as a
priced world mechanic" (deferred, §12). Band economics carried as the STARTING
WALK, never as "same band": `food_per_unit_micros` 10_000→15_000 (dock-exposure
dilution derivation; the band identities still pass), everything else inherited
and re-walked at the console against measured `laden_trips_per_window`.
Comparability claim withdrawn (GEO-C3): scenario_frontier is a NEW world
sharing the band's economic constants; all cross-map reads are
rate-normalized distribution-vs-distribution, never same-seed paired deltas.

## 4. The fuel re-bake: eps, exhaust velocity, calibration-first

PLAY-C2 proved no tank-only value works: a watchable tank at v_e 20 sits 20–50×
BELOW `FUEL_EMPTY_EPS` (the whole gauge inside the edge's dead zone). Adopted
mechanism (preserves every trip-time/brakability number — thrust accel
unchanged):

1. **`FUEL_EMPTY_EPS: 1e-9 → 1e-11`** (events.rs:16), its own single-cause
   commit; the two fixture families straddling the old eps are REDESIGNED
   (lower starting fuel), not literal-nudged. Band runs unaffected (band fuel
   never approaches 1e-11 in 50k ticks) — hash-neutral on the band.
2. **Hauler `exhaust_velocity` (frontier spec only): 20.0 → calibrated;
   analytic prior 1.0.** At 1.0: burn 2.5e-13/tick, endurance ~4,000 thrusting
   ticks ≈ 2.5× the worst round trip; tank = 100× eps (edge LIVE, ~40 limp
   ticks); Δv 0.693 ≫ worst-leg need 0.073. Intra-tank mass-loss shifts a_max
   up to ~2× — absorbed by the margin and MEASURED by the calibration.
3. **Calibration chain (instrument before field):** phase-0 role-split FUEL
   baseline on trophic → frontier calibration ensemble with a new
   `craft.fuel_capacity_scale` apply_knob arm (endurance provably exceeds run
   length, burn tail uncorrupted) → derive v_e from the measured worst
   HAULER-leg burn × the owner's k (OD-5) → bake with the derivation in the
   factory doc comment. Pirates keep v_e 20.0 per-craft (OD-6).

## 5. The refuel verb: BuyUpgrade clone with three critique-driven closures

**Config:** `RefuelCfg { lot_mass: f64, corp_index: u32 }`. `lot_mass == 0.0`
⇒ both stages early-return — **the named trophic-inertness gate**:
scenario_trophic leaves RefuelCfg default-off, proven by a cross-branch
2000-tick digest test. Reset error if `lot_mass > 0` while
`price_cfg.base_micros[Fuel] == 0` or any station's seeded
`initial_price_micros[Fuel] == 0` (the media half-on idiom). Frontier
`lot_mass 5e-11` (20 lots/tank; ~1 lot core leg, ~3–4 frontier leg).

**Surfaces:** transient `CraftStore.pending_refuel: Vec<Option<()>>` (joins the
all-None-at-every-hash-point assert — the pending_upgrade precedent, NOT
hashed); ingest verb `Refuel`; events §7. **Stage ordering amendment
(explicit):** 1c3b `run_refuel_policies` after `run_purchase_policies` —
scripted non-pirate craft, docked at ANY station (t−1 frame), headroom ≥ 1 lot,
wallet covers a unit ⇒ intent; top-to-full, threshold-free, no taste scalar.
1d2 `resolve_refuels` after `resolve_purchases`, pre-physics: always-consume-
then-gate; deterministic no-op skips for undocked / stock 0 / wallet short /
tank full / `unit_price < 1` / stale corp row. Same-tick burn draws from the
refilled tank; `prev_fuel` untouched (stage-4 copy-forward preserves Class-3
pinning).

**Quantization (the integer decision precedes every write):**
`need = floor((cap_eff − fuel)/lot)`; `afford = credits / price` (price ≥ 1 by
the skip); `units = min(need, stock[Fuel], afford)`; then four legs —
`stock −= units` · `consumed[Fuel] += units` (the sink leg the resource
identity demands) · wallet→**Port corp** treasury (pure transfer, no credit
sink) · `fuel_mass += units·lot` clamped to cap (one rounding; propellant lives
outside both identities by design, documented).

**The two reopened escrow-lock doors, both shut:**
- FUEL-C1: `resolve_refuels` **re-derives `dv_remaining`** for any refueled
  craft currently Seeking (a pure function of hashed state — the same
  tsiolkovsky derivation both dispatch sites already use). The same-tick
  dispatch-then-refuel race can no longer trap a full-tank craft coasting at
  `dv_remaining <= 0` (autopilot.rs:61, verified) with escrow locked.
- PLAY-C1: **dispatch eligibility requires `fuel_mass > FUEL_EMPTY_EPS`** —
  one feasibility filter-at-choice in ASSIGN (the capacity-filter precedent).
  A stranded craft stays Idle forever; the ADRIFT detector (role Idle ∧ fuel ≤
  eps) matches the true end state. World-truth feasibility, not shaping. Plus
  a liveness window: max non-terminal contract age per run (W9).

**Pricing — the first live price:** PriceCfg live for **Fuel only** —
`base_micros [0, 5_000]`, `cap [0, 40]`, slope 1800; `cap[Ore] == 0` keeps Ore
structurally dead (verified: update_prices skips cap-0 rows). Curve: full
stock 1,000 → dry 10,000 micros/unit; a full fill ≈ the grubstake ≈ 10% of a
tier-1 reward. The ransom-clamped wallet arms the robbed→broke→stranded arc at
low-stock/high-price stations. `initial_price_micros[Fuel]` seeded from the
curve at factory build. Revenue → a new **Port corp** (treasury 0; the Yard
precedent) — keeps the Yard-circulation panel clean and yields a
propellant-revenue read free. Generator AND consumer land in one rung (the
OD-5b two-sided law). Pre-registered null: fuel spend ≈ 1–3% of revenue — the
value of asymmetry rides the stranding tail, not the price arc; a flat price
gradient is a recorded finding, not a failure.

**Stranded fate:** drift forever, ON THE RECORD — `ContractFailed` event +
chronicle epilogue tombstone. No rescue, no reserve, no range filter (no
demonstrated gap; some stranding is the watchable point). Named trigger to
revisit: median ≥ ~2/20 haulers lost per run reads as attrition-noise.

## 6. Pirates on the big map

- **The haven-lurk leak, fixed phase 0** (TROPHIC-C3; main-loop verified): a
  post-refuge pirate whose nav still resolves the hideout body inherits the
  HAVEN as its hunting lurk — the nav-derived lurk path bypasses the haven
  exclusion that guards only fresh draws (pirate.rs `nav_lurk`), contradicting
  the code's own doc ("a pirate does not rob where it fences"). On today's
  band the haven is also the tier-2 destination, so one rob mints 150k food >
  the 100k grubstake — a self-reinforcing rob-where-you-fence attractor inside
  every banked baseline. Fix: `nav_lurk == haven_station` → treated as None →
  fresh reach-bounded draw. Cost stated honestly: ~86% of post-refuge draws
  become map-wide breakouts on today's band — **a console re-judgment session
  is scheduled**, and the 6-station HHI/slack calibrations (contaminated by
  the leak) are re-fitted on BOTH maps post-fix. No golden literals move.
- **Re-entry honesty (TROPHIC-C1):** from the seam haven, post-refuge re-entry
  is breakout-dominated (uniform over huntable stations); the frontier-core
  danger gradient rests on **fed-camping selection** (a fed pirate never
  relocates — the hunger gate), not on hideout-adjacent priority. Stated as
  the mechanism; breakout share + landing distribution pre-registered (W6);
  reach-vs-outer-gap named as the likely console lever.
- **Pirate fuel = per-class endurance spec** (OD-6): pirates keep v_e 20 (80k
  thrusting ticks) via per-craft `CraftInit.spec` — zero new mechanism, no
  taste scalar (roles already differ structurally); predator survival pressure
  stays food-only this rung; per-role FUEL windows make the asymmetry visible;
  named unification trigger = ransom income comfortably covers projected
  pirate fuel spend (W11). Pirates cannot strand this rung.
- Reach 0.6 set EXPLICITLY in both factories (today inherited silently); the
  stale "nearest station" marooned doc fixed in the same commit; haven
  exclusion / hunger gate / reset scatter verified n-generic — zero code change.

## 7. Events and chronicle (hash-neutral, single-emit)

- `Refueled { craft, station, units, price_micros, tank_before_permille,
  tank_after_permille }` (before-permille FLOOR-rounded, pinned; after derives
  from the decided integer purchase).
- `ContractFailed { contract, hauler, cause, escrow_refunded_micros,
  cargo_lost }` — emitted in `settle_contract_failure` for
  `FailureCause::FuelEmpty` ONLY (Robbed keeps its own narration; single emit
  path preserved); the stale-corp degrade arm reports the actual 0 refund.
  Today the failure path is verified silent — the tragedy becomes visible.
- `LurkMoved { pirate, to_station, breakout: bool }` at the relocation write
  (no relocation event exists today; backs W6).
- Chronicle epilogue per craft (printer-side): role, workplace radius, tank
  permille, credits, `ADRIFT since t=…` — adrift computed from final world
  state.

## 8. Lab bench (windows, never gates)

- **`META` line** (`seed= scenario= stations= haulers= pirates_initial=
  station_radii_milli_au=[…]`) — kills the N_HAULERS mirror class in
  sweep_trophic.py; sweep parsing version-gated so banked pre-FUEL outputs
  still parse.
- **`FUEL` line, role-split** (LAB-C2): the anchored line carries HAULER
  numbers only (`hauler_duty_milli, hauler_burn_total_milli,
  hauler_median_leg_burn_permille, hauler_min_tank_permille, refuels,
  refuel_spend_micros, strandings, adrift_end`); per-role JSONL rows carry
  pirates. Phase 0 ships measured fields only; refuel fields append with the
  mechanic. Zero-refuel sentinel per the MEDIA precedent. `fuel_empty=0` flips
  meaning on frontier ("no stranding this seed" — texture);
  `--assert-no-fuel-empty` stays on trophic arms only.
- **TrophicSample additive pure-read fields** (TROPHIC-C2: the lab cannot read
  pub(crate) nav/food from an example binary — data flows through
  `sample_window`, a named core diagnostics change):
  `per_station_lurking_pirates, pirates_commuting, pirates_at_haven,
  per_station_fuel_stock, per_station_fuel_price, refuels, refuel_units,
  refuel_spend_micros`.
- **Panels:** I1 `haves_havenots.py` (per-craft workplace radius × hearing lag
  × end credits; lag joined on BORN tick, never the corruptible rob_tick) with
  the PLAY-C3 confound pre-registered AND the 6-station control actually run;
  I2 radial-zone panel with the `fuel_starve` death-spiral-vs-boom-bust
  discriminator; coverage re-denominated to contract-endpoint stations
  (scenario-conditional — the dark haven makes all-stations coverage
  structurally unsatisfiable); per-row refuel fill share (row-order rationing
  named); HHI/slack re-fit on BOTH maps post-leak-fix (labeled-run method,
  held-out seeds, hungry-roamer positive control must read RiskEqualized on
  frontier before any frontier reading is recorded).
- **Ensembles:** the standard grid is 20 seeds × arms (the basin-chaos
  lesson); cross-map comparisons rate-normalized, distribution-vs-distribution.

## 9. Hash/golden inventory and landing order

**No HASH_FORMAT_VERSION bump** (v5 stays; refuel intent is a transient column
on the pending_upgrade precedent; n² ring growth is content, not format).
Exactly ONE `GOLDEN_CONFIG_HASH` re-pin (RefuelCfg fields, single cause) and
one NEW frontier trajectory golden. Zero existing goldens move.

- **Phase 0a:** haven-lurk fix (single-cause behavior commit; console
  re-judgment scheduled; no literals move).
- **Phase 0b:** META line + N_HAULERS fix + role-split FUEL line (measured
  fields) + version-gated sweep parsing; bank the 20-seed trophic baseline
  POST-fix with the new instruments.
- **Phase 1:** eps 1e-11 + fixture redesign (own commit); RefuelCfg +
  pending_refuel + stages 1c3b/1d2 + dv-rederive + dispatch fuel-eligibility +
  Refueled/ContractFailed + ingest verb, gated off at lot 0; GOLDEN_CONFIG_HASH
  re-pin; trophic cross-branch digest green.
- **Phase 2:** `scenario_frontier` factory (§2–3, prices, Port corp, per-class
  specs, explicit reach, ephemeris 120k + runner guard) + `--scenario` flag +
  LurkMoved + TrophicSample fields + frontier trajectory golden; calibration
  ensemble at `fuel_capacity_scale=100` → bake hauler v_e per OD-5's k.
- **Phase 3:** dual-map HHI/slack re-fit; I1/I2 panels + pre-registered band
  text; frontier positive control; THEN the headline 20-seed × 6-arm grid;
  owner console session.

## 10. Cross-lens conflicts — resolved, losers named

1. Hideout placement: SEAM (station 6) over outermost — band-continuous
   lie-low distance, deep frontier stays honest prey.
2. Pirate fuel: per-class spec over free haven resupply (one survival pressure
   per rung; the "tether = tank/2" was a hope, not a mechanism).
3. Wiring: partitioned tier loops over scaled hub-and-spoke.
4. Tank mechanism: v_e + eps re-bake over a coordinated dry/thrust re-bake
   (preserves all trip/brakability math).
5. Escrow-lock closure: dv re-derivation at settle over a stage move.
6. Cross-map comparison: rate-normalized distributions over same-seed pairing.
7. Stranding visibility: `ContractFailed` (FuelEmpty-cause-only) over a silent
   settle.

## 11. Pre-registered windows (recorded, never gated)

W1 saturation leaves CommonKnowledge (escaped_milli < 950; desert map
disambiguates) · W2 median hearing lag > 2500 (registered alternative:
rob-anchored staleness zeroes frontier evidence older than 4000 — "gossip
degenerates toward blind on frontier routes") · W3 hub/backwater ratio > 3.0 ·
W4 flip-share VALUE: gossip-vs-ring over 20 clean seeds × both anchor arms
(registered alternative: mixing persists → the deferred dispatch-locality
lever) · W5 I1 correlations with the position-blind-dispatch confound
registered · W6 breakout share + landing distribution + lurk-dwell bimodality ·
W7 tier-2 service rate + regime onset vs the upgrade ladder · W8 hauler
per-leg burn/duty (the calibration input) · W9 strandings 0–2/run band +
robbed→stranded chains + contract-age liveness · W10 station fuel stock-out
map + price gradient + `fuel_starve` discriminator · W11 fleet attrition +
per-role pirate fuel low-water (the OD-6 trigger input) · W12 trophic arms
bit-identical digest + `fuel_empty=0` (the control stays a control).

## 12. Deferred (named triggers — deferred means not built)

Police/navy/bounties · maydays + coverage radius · in-flight detour verb ·
comms relay · pirate gossip reads · corruption consumers (rob_tick lying stays
armed-dormant) · gym pull (media OD-4) · media OD-2 bundle (ring +
risk_appetite ghost — no format bump exists this rung, and the ring is the
live counterfactual instrument for W4; retiring it mid-measurement would blind
the comparison) · **dispatch locality as a priced world mechanic** (trigger:
W4/W6 read mixing-persists) · **pirate fuel unification / haven resupply**
(trigger: W11) · **rescue/salvage** (trigger: strandings read boring or the W9
band blows) · **ASSIGN range filter** (trigger: carnage instead of drama) ·
Saturated classifier arm (trigger: ≥ half of frontier baseline windows show
all pirates active; ships with a labeled synthetic per the seed-7 rule) ·
breakout reachability clamp · scenario DSL · dt/thrust retune · per-craft
taste scalars (never).

## 13. Owner decision points — RESOLVED 2026-06-11

> **Owner resolution (2026-06-11):** all seven decided as **recommended** —
> OD-1(a), OD-2(a), OD-3(a), OD-4(a), OD-5(b), OD-6(a), OD-7(a). Conveyed as
> "start planning the 'world gets big' sprint" against the presented
> recommendations, with no overrides. The bundled consequences are accepted:
> the haven-lurk-leak fix changes the judged band (console re-judgment
> scheduled), the food band re-walk starts at 15k, and W11 is the named
> pirate-fuel unification trigger.

**OD-1 — Map shape and populations.** (a) **n=10, geometric 0.35→3.0 AU; 20
haulers, 10 pirates** (gradient 54%→never-opens with exactly one
pirate-unwalkable hop; worst leg ~1010 ticks; the Saturated guard passes with
slack — 10 pirates is a predator:prey choice, the guard ceiling is 13)
(**recommended**); (b) n=8, 0.35→2.2 — cheaper but the frontier is one station
deep; (c) n=12, 0.35→3.6 — richer but more walking and nothing the lab can't
see at n=10 (√d means width buys little).

**OD-2 — Traffic topology.** (a) **Partitioned tier loops by radial band**
(core/mid/frontier; every loop touches a vendor) — the named fix for the
verified self-averaging risk; honest scope: changes the prey-value landscape,
so the food band starts at 15k and is re-walked at your console
(**recommended**); (b) scale today's hub-and-spoke — three critiques
independently predict CommonKnowledge persists and the rung measures nothing
new; (c) hybrid — muddles both readings.

**OD-3 — Haven placement + the lurk-leak fix (one bundled call).** (a) **Haven
at the seam (station 6), vendor, dark (no contracts), AND fix the haven-lurk
leak now** — the leak is a verified band bug (a self-reinforcing
rob-where-you-fence attractor sitting inside every banked baseline); cost
stated honestly: the fix changes the band you judged "a great story" (~86% of
post-refuge draws become breakouts) — console re-judgment scheduled, dual-map
re-calibration of the heterogeneity instrument (**recommended**); (b) haven at
the new outermost (3.0 AU) — raider-coast flavor but lie-low commutes eat
25–50% of the refuge; (c) keep the leak and watch — zero churn, but it scales
with the map and contaminates every frontier danger window.

**OD-4 — Propellant economy.** (a) **Unify: refuel draws station `stock[Fuel]`
(miners→refiners→tanks closes) at the first live demand-deflation price (Fuel
only; full 1,000 → dry 10,000 micros/unit), revenue to a new Port corp** —
scarcity gets geography, the robbed→broke→stranded arc is armed, and the
death-spiral question becomes a measurement (`fuel_starve` discriminator;
either answer is a finding) (**recommended**); (b) flat config price — a
vending machine; no scarcity, no geography; (c) price both resources live —
two confounds in one rung.

**OD-5 — Stranding flavor (sets k, the tank-endurance multiple).** (a) k ≥ 4 —
zero-by-construction; the gauge is scenery; (b) **k ≈ 2.5 — tail-event tragedy
(~1–5% of craft-runs): stranding needs compound misfortune (robbed +
dry/unaffordable docks); the chronicle gets "the ship that didn't make it";
drift-forever stays the honest fate, narrated end-to-end; k applies to the
MEASURED worst-leg burn from the calibration ensemble, never spec arithmetic**
(**recommended**); (c) k ≈ 1 — routine strandings with no rescue mechanic read
as attrition noise.

**OD-6 — Pirate fuel this rung.** (a) **Pirates keep the ×10 endurance spec
(per-craft CraftInit.spec — zero new mechanism, no taste scalar; predator
survival pressure stays food-only; per-role FUEL windows make the asymmetry
owner-visible; named unification trigger = W11)** (**recommended**); (b) free
top-up at the haven — machinery for a hope (the tether claim didn't survive
critique); (c) shared scarce spec — stranded pirates are permanently lost
predators (no despawn exists): monotone decay into accidental PermanentPeace.

**OD-7 — Is this rung the cut-2 format bump?** (a) **NO** — the cut needs no
HASH_FORMAT_VERSION change; the RouteEvidence ring + risk_appetite ghost keep
their named trigger, and the ring is still the live counterfactual instrument
for this rung's own W4 gossip-vs-ring window (retiring it mid-measurement
would blind the comparison) (**recommended**); (b) yes, bundle now — retires
the ghost a rung early at the cost of manufacturing a format change and
blinding W4.
