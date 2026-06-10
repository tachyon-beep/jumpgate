# Pirates Rung 1 — Choke-Point Predation + the Upgrades Arms Race

Date: 2026-06-10   Status: APPROVED — all §15 decisions resolved, build authorized   Author: acting-PM (Claude)
Provenance: synthesized from a 14-agent design panel (5 code scouts → 3 designers:
ecology / economy / substrate → 6 adversarial critiques across frame + feasibility
lenses). Supersedes the pirate half of `2026-06-10-trophic-cut-1-boom-bust-and-decisions-design.md`
(its deferral condition — "returns after the tactical-flight rung proves the pipeline" — is
satisfied by the flight AND trader rungs landing). Salvages that spec's reviewed capital
(encounter accounting, lie-low/notoriety, food-driven population, the aliveness
discriminator + diagnosis matrix, conservation identities) and retires its
`risk_appetite`-driven decision layer permanently.

> **FRAME (PDR-0006 — read first).** Jumpgate v1 is a GAME judged by EMERGENT PLAY. The
> primary deliverable of this rung is a long, chronicle-narrated GAME run — heuristic
> population, owner-watchable, alive — with the RL trader entering afterward as a PLAYER
> facing the live risk field. Every metric below is a designer's window for the
> observe→steer→re-observe loop, never an acceptance gate. No DRL result gates or triggers
> world iteration. The presolvability/fraction-of-ceiling frame is retired; do not
> reintroduce it.

---

## 0. Thesis and the ghost

**Thesis:** a small number of *anchored, hungry, heat-pulsed* lurker pirates over many
routes — with robbery feeding a hunger clock (population dynamics) and a credit wallet
(arms race), and haulers responding through dock-refreshed evidence and escort purchases —
produces a boom/bust cycle whose risk field stays spatially clumped and temporally
persistent, and whose escort threshold displaces risk onto the unescorted poor: an
emergent class story.

**The ghost:** both prior NO-GOs (`interdiction-rl` ~0.4%, `contention-game` ~4%) died of
RISK EQUALIZATION. The panel identified FOUR re-entry paths and this spec carries an
explicit countermeasure for each:

| Re-entry path | Countermeasure |
|---|---|
| Pirates chase traffic (the interdiction equalizer) | Deliberately DUMB lurkers: no value/traffic-seeking targeting at all in rung 1; relocation is a seeded uniform draw among in-reach stations, decorrelated from traffic |
| Field self-averages (LLN, the contention equalizer) | Few active pirates over many routes by scenario law: expected-active ≤ stations − 2; lie-low duty cycle is the coverage lever |
| Shared beliefs + identical policies synchronize the prey response (equalization one level up — flagged independently by three critics) | Dock-gated evidence reads: each hauler acts on route evidence as of its OWN last dock; staleness varies per craft by lived docking pattern, desynchronizing avoidance and purchases. This is also the media layer's v0 seam |
| Upgrade monotonicity ratchets into an absorbing all-escorted peace | Level caps (escort ≤ 2 both roles) with pirates ending structurally stronger (base 1 + escort 2 = 3 > hauler max 2); `PermanentPeace` is a named diagnosis row, not a surprise |

## 1. Success bar (ALIVE + GAME, co-equal, owner-judged)

Judged on seeded 50,000-tick GAME runs (25 windows, W = 2,000 ticks), primarily by the
owner reading the chronicle — the metrics are evidence feeding that read:

- **ALIVE** — sustained predator-prey boom/bust: anti-phase movement of active-pirate
  count vs laden-hauler count across windows; multiple alternations; not damping to a
  fixed point; neither population pinned or saturated for long stretches.
- **A GAME** — decisions disperse peer outcomes and the dispersion tracks decisions:
  - Risk heterogeneous & persistent, measured over OCCUPIED routes (concentration via
    HHI/Gini of robberies among routes with nonzero traffic, normalized by active-pirate
    count) plus persistence relative to the traffic gradient (the hot route must move
    slower than laden traffic does). *The naive top/median metric is structurally gamed
    by sparsity (median ≈ 0 passes vacuously) — rejected by the panel.*
  - Outcome dispersion among identically-specced haulers, with the **initial-conditions
    swap test**: same seed, swap two haulers' spawn stations only; the outcome ranking
    must follow the position, not the craft slot. (No per-craft taste/policy scalars —
    the retired-premise rule. Heterogeneity comes from state: position, wealth, escort
    level, dock-refreshed evidence.)
  - **Stratification (the upgrades axis):** within-hauler engagement rate drops to ~0
    after its escort purchase (before/after, causal), and risk visibly re-concentrates on
    the unescorted. (The Spearman times-robbed × escort-level correlation is
    sign-confounded — being robbed CAUSES escort purchase — and is demoted to a window.)
  - **Watchability (owner-judged, co-equal):** the chronicle yields ≥ 3 distinguishable
    life-arcs readable as story.

**Kill discipline (re-anchored per PDR-0006):** the tuning loop is bounded (≤ 24
parameterizations, walked through the diagnosis matrix, each run naming its row and the
knob moved). If the maximal-locality corner still reads RiskEqualized, that is the third
equalization death — an owner escalation with the matrix evidence, decided on the owner's
holistic "not alive / not watchable" read. The metrics inform that read; no numeric
threshold is itself a kill trigger. The side bet is severable: if predation is alive but
the arms race is flat after ≤ 8 escort-knob parameterizations, cut the upgrades layer
back out and ship the predation rung alone. Instrument-kill stands: if the reach=∞
positive control does not read RiskEqualized, stop and fix the instrument first.

## 2. Encounter mechanic: choke-point lurkers (NOT chase)

**Why not chase (verified):** craft-target nav is a documented best-effort stub
(world.rs:646-655), `detect_boundary_events` skips `Entity(Craft)` (events.rs:139-140),
pursuit is lead-less, and shared BaseSpecs make a stern chase unwinnable (the pursuer
flies the target's own profile, lagged — panel math: most chases end against a hauler
already docked). None of it is needed: haulers MUST brake to ~rest within
`ARRIVAL_RADIUS` (1e-4 AU) of a station body to load, and decelerate through low speeds
on every approach. **Stations are the natural ambush sites by Newtonian mechanics, not
designer fiat** — which is the owner's pre-registered theory (engagements cluster at
launch and deceleration) given to the sim to confirm or refute, not scripted into it.

**Trigger — NEW post-physics stage 3b2 `resolve_encounters`** (AFTER `resolve_deliveries`
3b, BEFORE `resolve_failures` 3c, extending the proven 3b-before-3c ordering precedent,
economy.rs:766-768). For each active pirate p (dense order), engage the NEAREST eligible
hauler h:

- Pirate eligible iff `role == Pirate`, `tick >= lie_low_until`, `tick >= engage_cooldown_until`.
- Hauler eligible iff `role == Hauler`, `cargo.is_some()`, contract status ∈
  {CargoLoaded, InTransit} (the one-tick CargoLoaded window is robbable — settlement
  handles both identically; both hold escrow), AND `strength(h) < strength(p)` — pirates
  do not start fights they have already lost (the tether-coercion logic: the threat must
  be credible). Escort level is visible evidence (a wing is an observable physical
  configuration, not a valuation).
- Geometry: `|pos_p − pos_h| <= engage_radius_au` (default 5e-4 AU = 5× ARRIVAL_RADIUS)
  AND `|vel_p − vel_h| <= engage_speed` (default 2e-3 AU/day = ARRIVAL_SPEED). Post-physics
  current-tick positions (craft-craft distance needs no body frame). A Δv-advantaged
  hauler under way is out of the envelope — flee-by-physics is preserved.
- One engagement per pirate per tick; eligibility re-checked sequentially (a just-robbed
  hauler can't be robbed twice in a tick).

**Ambush windows, owned:** engagement is legal at the ORIGIN dock — rob-on-load/departure
is the headline behavior, narrated as the departure ambush ("P2 was waiting off the
gantry"). The DESTINATION dock is sanctuary by ordering: a same-tick Arrival settles the
delivery first (3b before 3b2) — "made port with the corsair an engine-length behind."
Inbound laden approach (the deceleration window) is fair game. Predicted engagement-phase
histogram: bimodal at trip endpoints — **the owner's launch/decel hypothesis as the
rung's pre-registered discriminator** (log fraction-of-trip-elapsed and speed per
engagement event; the same instrument later measures whether grapples/tethers grow a
mid-cruise mode, cut 2+).

**Outcome resolution (statistical coercion — the PERMANENT top layer):**

```
strength(craft) = upgrades.escorts + (role == Pirate ? cfg.pirate_base_strength : 0)   // base 1; escorts = wing SIZE (ship count), see §6
```

1. Eligibility already guarantees `strength(p) > strength(h)` (the owner's "fight off
   weaker-or-equal pirates", verbatim: ties go to the defender, deterministically, no
   draw — protection is reliable, which is what makes buying it a real decision).
2. One draw u ∈ [0,1000) from `RngStream::Piracy` (its FIRST runtime consumer; draws at
   exactly this stage, dense-row order): `u < p_rob_milli` (default 700) → **Robbed**;
   else **DrivenOff** (the hauler slips away / the bluff fails).
3. Either outcome: `engage_cooldown_until = tick + (robbed ? rob_cooldown : driveoff_cooldown)`
   (defaults 600 / 200) — a rob "digests" for roughly one trip-time; no per-tick re-rolls.

Kill does not exist in rung 1 — not only because despawn conflicts with slot==row, but
because the coercion economics make it irrational ("don't turn the big score into
relativistic shrapnel"). Cuts 2+ decompose these probabilities (tethers widen the
envelope, boarding, morale); the resolution stays a seeded roll whose odds are shaped by
state. **The dice never leave; they get more informed.**

**Discriminators (encounter):** robbery concentration over occupied routes with
persistence (the §1 metric); the strength step function (matched-seed A/B at escort 0 vs
1 — rob rate collapses to exactly 0 above threshold); the endpoint-bimodal trip-phase
histogram. Falsification arms route to the diagnosis matrix (§9).

## 3. Robbery settlement: the ransom model (zero new identity legs)

The panel's economy designer proposed cargo-loot + fencing; its own critique then proved
the fence self-cancels (fencing at the price-gradient optimum = the starved destination
of the predated route, which restocks the sink, suppresses REPOST, and turns the pirate
into a lossy courier that damps the very boom/bust the rung needs). The substrate
designer's **ransom model** dissolves the contradiction structurally and is adopted:

On **Robbed(p, h, contract c)** — all in stage 3b2, mirroring `resolve_failures`' shape:

- **Cargo:** hauler slot → `counters.consumed[res] += qty` (the accounted sink leg, the
  FuelEmpty precedent). The resource identity `Σstock + Σin_transit == initial + mined −
  consumed` holds untouched. Chronicle narrates it as jettisoned-and-lost; ballistic
  dumps that can be RETRIEVED are the cut-2/3 salvage package (tether lore) — the
  accounting is explicit now so nothing is silently destroyed.
- **Contract:** status (CargoLoaded | InTransit) → Failed; `escrow → corp treasury`
  refund (TRANSFER, the existing precedent); hauler cleared to Idle (incidentally giving
  robbed haulers the exit the no-abandonment gap currently denies).
- **Takings:** `ransom = min(hauler.credits_micros, ransom_cap_micros)` (default
  2_000_000 ≈ one mid-tier contract) transferred hauler → pirate wallet. Pure TRANSFER —
  `Σtreasury + Σcredits + Σescrow` invariant with NO new leg, no mint, no ledger. This is
  what pirates spend on upgrades, and it debits the RL trader's Δcredits directly (a real
  loss on top of the forfeited payout) with zero reward shaping.
- **Metabolism:** `food_micros += qty × food_per_unit_micros`; `notoriety += notoriety_per_rob`.
- **Events:** `Robbed { pirate, hauler, contract, value_micros: ransom }`;
  DrivenOff emits `DrivenOff { pirate, hauler }`. Evidence-shaped: who/what/where/magnitude.
  Both emission sites also log the engagement's kinematic snapshot (relative bearing +
  speed, free at that point in 3b2) into the diagnostics window — it feeds the
  endpoint-ambush histogram now and is the hit-location data seam for the future
  part-graph damage model (§14.7).

Robbed cargo never reaches the destination → destination stock stays low → REPOST keeps
the predated route bursting: **prey regrows where wolves hunt, for free** (verified
against the hysteresis code), with no fence to cancel it. Fencing + stolen-goods supply
shocks + lawless-haven geography land in cut 2 WITH value-seeking pirates.

Known boundary: early-episode gym ransoms are near 0 (wallet starts empty) — noted for
the ablation analysis (report per-decision-index robbery cost), not a mechanic change.

## 4. Pirate lifecycle: food, heat, lie-low (population without spawn/despawn)

NEW stage 3b3 `update_pirate_population` (after 3b2, before 3c), per pirate, dense order:

- **Upkeep:** `food_micros −= upkeep_per_tick` while active.
- **Starvation:** `food_micros <= 0` → `lie_low_until = tick + starve_lie_low_ticks`,
  food reset to `grubstake_micros` (re-emerges hungry). Emit `PirateLieLow`.
- **Heat:** `notoriety >= heat_threshold` → forced `lie_low_until = tick + heat_lie_low_ticks`.
  Notoriety decays geometrically (integer milli arithmetic) every `decay_interval`.
  Desynchronized duty cycles give each route a risk HISTORY — the persistence the
  heterogeneity axis measures.
- Lying-low pirates fly to the hideout body (outermost) — a refuge OFF the predation
  field (the navy-design `LIE_LOW < WANTED` crux: the structural mechanism that stops
  prey extermination). Lie-low IS the rung-1 population dynamic; the active count is the
  boom/bust variable. `PirateSpawned`/`PirateLeft` stay unemitted this rung.

**Calibration is DERIVED, not asserted** (the panel caught the designers' prey-flux
guess running 2-4× low): Phase 0's instrumented no-pirate baseline measures
`laden_trips_per_window` (a TrophicSample field from day one); then set

```
upkeep_per_tick  such that  one qty-5 rob sustains  ≈ (W × k_digest) ticks   (k_digest ≈ 1 window)
food_per_unit    = upkeep_per_tick × W × k_digest / 5
viable band      : an active pirate needs ≈ measured_flux / n_active_target robs/window to persist
```

with `n_active_target = 2-3` of a 6-pool. The spec states the formulas; the constants are
filled from the P0 measurement at the console.

## 5. Pirate brain: bounded dumb lurker (NEW pre-physics stage 1c2 `run_pirate_brains`)

- **Initial lurk stations are drawn from the Piracy stream at reset** — never config-fixed
  (a fixed pirate→station map would let the gym PPO memorize geography instead of reading
  contacts; layout test required: two seeds → different assignments).
- If lying low and not at hideout: `Seeking{Body(hideout)}`.
- Relocation (staggered `tick % relocate_period == row % relocate_period`, default 2500
  ≈ 4 trips — sticky on the prey timescale): with `stay_milli` (500) keep station; else
  uniform Piracy draw among stations within `pirate_max_reach_au` (default 0.6 — reaches
  1-2 neighbors, never the whole map: the PRIMARY locality lever); none in reach →
  nearest. **Uniform-in-reach, NOT traffic-weighted** — the relocation attractor is
  deliberately decorrelated from traffic (the interdiction equalizer). No per-pirate
  traffic memory is built this rung (YAGNI; cut by the panel).
- Loiter: re-issue `Seeking{Body(lurk)}` when drift > `engage_radius / 2` — strictly
  inside the engagement envelope so a settled lurker is geometrically guaranteed to cover
  a docked hauler (the panel caught the original threshold leaving a dead zone).
- Deliberately NO value-seeking target scoring. Dumbness + locality + persistence is the
  antidote, not a placeholder; value-seeking enters cut 2 only if heterogeneity holds
  without it first.

Scripted stages skip gym-controlled craft via `CraftInit.scripted: bool` (config, Commit
A — decided NOW so the config golden moves once, not twice).

## 6. Upgrades: catalog, the Yard, purchase verb, capacity tiers

**Catalog this rung (both roles, same prices, levels only increase, no resale):**

| Line | What you buy | Prices (micros) | Cap |
|---|---|---|---|
| **Escort** | ONE escort ship for your wing (`escorts += 1`; strength = wing size while un-simulated); deterministic protection threshold (§2) | 1st 5_000_000, 2nd 12_000_000 | 2 (both roles) |
| **Hull** | ONE additional cargo hull (`hulls += 1`; capacity = base 5 + 5×hulls); gates contract tiers | 1st 8_000_000, 2nd 20_000_000 | 2 (capacity 15) |

**Fleet-ledger honesty (owner caveat, decision-3 sign-off):** the columns are
`UpgradeLevels { hulls: u8, escorts: u8 }` — **counts of ships the craft owns that the
sim does not yet individually fly**, never abstract stat levels. The purchase verb buys a
SHIP; the chronicle narrates a wing ("H7's escorts drove them off"), never a level-up. A
fleet is "a collection of ships with a single policy acting as a strategic head" (the
commodore chair, per the glossary captain→commodore→admiral taxonomy) — so the migration
is a DEMOTION of this ledger, not a reinterpretation: when the commodore rung lands,
each count mints real craft into a fleet under one GuidanceParams policy, and the columns
die. Named sunset debt: **the ledger must not outlive the fleet rung.** Implementation
rules that keep the demotion honest: nothing may fold escorts/hulls into physics or
EffectiveMods (ships fly, stats don't); nothing may assume the wing is unlosable
(attrition becomes possible the day they're real); caps stay small (2) so the
un-simulated wing never grows past what a chronicle line can carry.

Caps are STRUCTURAL (`ShipyardCfg.max_*_level`, settle no-op at cap, unit-tested) — they
keep the obs scale stationary (strength ∈ [0, 3], `PIRATE_STRENGTH_SCALE = 4`), kill the
dead-stat tail (a 40-capacity hull in a 15-max world was in the draft catalog — cut), and
bound the ratchet. End-state asymmetry is intentional: a maxed pirate (1+2=3) out-ranks a
maxed hauler (2) — predation never goes extinct; the arms race produces visible regime
changes, not permanent peace. `PermanentPeace` is in the diagnosis matrix anyway.

**Tanker is DEFERRED to the refuel package (cut 2)** — flagged as an owner decision
(verbatim ask). Three independent panel findings force it: (a) no refuel path exists —
fuel only decreases, so a capacity boost is inert dead state; (b) `fuel_capacity` feeds
the autopilot's cruise cap, so a tanker is secretly a +32%..+81% SPEED upgrade — which
channel it buys must be decided deliberately (one-line seam change either way); (c)
binding fuel promotes the filed escrow-lock bug `jumpgate-2c0c2d92bb` onto the critical
path. The cut-2 package = refuel verb + priced fuel + tanker + that bug's fix, landed
together. Rung-1 fuel endurance is instead sized by scenario: `exhaust_velocity` raised
~10× in `scenario_trophic` (tank ≈ 80k thrusting ticks; the reset guard reads dry-mass
a_max and is unaffected), with a standing zero-FuelEmpty window on the 50k-tick baseline
(the panel's burn math shows trader-spec craft strand at tick ~10-25k otherwise).

**The Yard:** one config-minted corporation receives all upgrade payments
(`ShipyardCfg.corp_index`) — credits recycle corp → escrow → hauler wallet → upgrade →
corp instead of draining monotonically (verified: today's only treasury inflow is the
escrow refund; long runs otherwise stall when REPOST escrow reverts on empty treasuries).
Zero new identity legs anywhere in the rung. The Yard treasury time series is a free
broken-flow diagnostic (must stay bounded and non-monotone).

**Where you can buy: `StationInit.sells_upgrades: bool`** — the first station capability
mixin (the MudOS lesson: capabilities compose on entities; "a pirate haven is a station
with the right column set"). The game scenario grants it to 2 of 6 stations →
upgrade-pilgrimage traffic is itself watchable geography. Vendor logic is written against
"an entity with a position and a catalog", not station-ness (the carriage-door
discipline) — but only stations carry it this rung.

**Purchase verb — intent/settle split (the AcceptContract template):**
- NEW `CommandKind::BuyUpgrade { kind: UpgradeKind }` (unhashed, additive) + ingest arm
  writing `pending_upgrade[crow] = Some(kind)` only (ActionIngested even on skip; the
  single-ingestion-path lever invariant). The verb ships now WITH a real caller: the
  scripted policies write the same intent column directly (the ASSIGN precedent), and
  one ingest-arm test drives the command path.
- NEW pre-physics stage 1d `resolve_purchases` (after `resolve_contracts`), dense order,
  `body_pos(t−1)` frame for the dock check: settle iff within ARRIVAL_RADIUS of a
  `sells_upgrades` station AND `credits >= price` AND `level < cap` → debit, credit Yard,
  level += 1, emit `UpgradePurchased { craft, kind, level, price_micros }`; else clear
  intent (deterministic no-op). `pending_upgrade` is transient (always None at hash
  points; debug_assert in state_hash; documented like `prev_*`).

**Scripted purchase policies (deterministic, desynchronized by construction):**
- Hauler (docked at a vendor, idle, `credits >= price × 1500/1000` working-capital
  headroom): buy Escort to L1, then Hull to L1, then Escort L2, then Hull L2. Purchase
  timing varies per hauler through wealth and docking history — no taste scalars.
- Pirate (at hideout, lying low, `credits >= price`): buy Escort. Pirates shop while
  hiding — narratively right, and it phase-lags the pirate ladder behind the hauler
  ladder (the arms race gets phase structure instead of lockstep).

**Bigger jobs:** contract tiers qty {5, 10, 15} at per-unit reward ladder 1.00× / 1.15× /
1.30× (premium = the market price of value-concentration: a bigger lot is juicier prey —
the cargo-value-vs-risk coupling, the canonical decision). Capacity gate at the
accept-settle (deterministic REVERT, the underfunded-escrow precedent; ASSIGN and
scripted choice filter the same way — never claim-and-revert). **REPOST trap (verified):**
the dedup key omits qty, so tiers ship as one corp per tier (retail/bulk/heavy) — zero
dispatch-code change, independent per-tier treasuries. **The retail qty-5 floor is
load-bearing and named:** persistent small-lot supply is the anti-extinction guarantee on
the capacity dimension; standing diagnostic = swallowable-prey fraction per window.
Per-tier demand bands offset (retail 10/20, bulk 5/15, heavy 0/10) so the three corps'
Schmitt triggers interleave instead of triple-bursting.

## 7. Evidence & avoidance: dock-gated route evidence (the media seam, v0)

The trophic spec's per-route decayed risk scalar is REJECTED as specified — three
independent critiques converged on it: (a) a decayed "risk number" in world state is a
pre-computed valuation (the evidence-not-valuations law); (b) a global register with
uniform lag synchronizes the whole prey population (common-mode flight = equalization one
level up + a damping pressure); (c) a global oracle is the OPPOSITE of the local/lossy
seam the media layer needs and would have to be ripped out of hashed state later.

**Replacement — evidence rings + dock-gated reads:**
- World state holds EVIDENCE ONLY: per-route ring buffer of recent rob ticks
  (`RouteEvidence { robs: [Tick; 8] ring }`, hashed, dense `n_stations²`). Bumped on each
  Robbed settle. No decay arithmetic, no risk scalar — staleness is a property of the
  READ, not the store.
- Per-craft `info_tick: Tick` (hashed): refreshed to `tick` whenever the craft is docked
  (within ARRIVAL_RADIUS of any station body). **A hauler acts on the world as of its own
  last dock** — information refreshes by docking, which makes it a positioned resource
  (the media-layer principle, mechanically real from day one) and desynchronizes the
  population's beliefs by lived experience.
- Read path: `World::route_evidence(reader: CraftId, route) -> u32` = count of ring
  entries in `(info_tick − evidence_window, info_tick]` (default window 4,000 ticks). The
  accessor takes the READER now so the media layer later swaps the propagation model
  behind an unchanged signature (documented in code as "the degenerate proto-channel").
- ASSIGN scoring (gated by `TrophicCfg.hauler_belief_scoring`, default false — trader gym
  and all existing tests untouched): scripted haulers pick
  `argmax reward_micros × (1000 − route_evidence × evidence_penalty_milli) / 1000`
  (clamped), ties → lowest ContractId. A scripted role computing its own valence from
  evidence is exactly what the law permits; what is forbidden is broadcasting the
  valuation — and the RL obs (§11) carries raw contacts only, never this score.

Staleness remains load-bearing for the cycle (prey-response delay sustains oscillation) —
it now arises from docking rhythms instead of a hand-tuned global lag, and the
cross-hauler belief-divergence + purchase-time-dispersion diagnostics watch for the
synchronization death mode.

## 8. Determinism, hash plan, config surface

**Exactly two single-cause golden commits** (the v2→v3 worked precedent; literals
re-derived via the ignored `print_golden` test, never invented; current values at HEAD:
`HASH_FORMAT_VERSION = 3`, `GOLDEN_ZERO_STATE_HASH = 0x1d44_b373_5ccd_33f7`,
`GOLDEN_CONFIG_HASH = 0xf4bc_85c3_7cb6_8a6b`):

- **Commit A — config surface** (GOLDEN_CONFIG_HASH re-pin, one cause): `CraftInit.role`
  (closes the no-way-to-mint-a-pirate gap; reset mints `PirateState` for Pirate roles),
  `CraftInit.scripted: bool` (the gym-exclusion flag, decided now), `BaseSpec.base_cargo_capacity`
  (default 5 — existing scenarios identical), `StationInit.sells_upgrades`, `TrophicCfg`
  (all knobs of §§2-7: engage/cooldown/food/heat/lie-low/reach/relocate/evidence/policy
  enums — everything the sweep lab needs is per-run config), `ShipyardCfg { corp_index,
  prices, caps, hull_step_units }`. `engage_radius = 0` or zero pirates ⇒ the whole
  trophic machinery inert.
- **Commit B — state v4** (HASH_FORMAT_VERSION 3→4, both state goldens re-pinned, one
  cause): `CraftStore.upgrades: Vec<UpgradeLevels { hulls: u8, escorts: u8 }>` (the
  fleet ledger, §6) (words
  appended after 26 in `write_craft_economy`; strength and capacity are DERIVED, never
  stored), `PirateState.engage_cooldown_until` (append inside the self-delimiting word-26
  fold), `CraftStore.info_tick`, `World.route_evidence` rings (world-level words).
  HASH_FIELD_ORDER doc + manual zero-fold + completeness test updated together.

**RNG:** all encounter rolls + brain draws from `RngStream::Piracy` (append-only enum,
already landed). Draw sites at fixed stages, dense-row order. The stream cursor is
documented as Class-3 transitively-pinned state (the `prev_*` precedent paragraph) — replay
rebuilds from reset + log, so bit-identity holds; mid-run state_hash equality is
correspondingly scoped (documented, not left implicit).

**Conservation:** the rung adds ZERO new identity legs. Credits:
`Σtreasury + Σcredits + Σescrow` constant (ransom, refund, purchase are all transfers).
Resources: `Σstock + Σin_transit == initial + mined − consumed` (robbed cargo uses the
existing consumed leg). Per-arm golden-value unit tests cover what identities can't see
(exact ransom/refund/purchase integer amounts — the identities stay green under
wrong-price bugs; the panel's point).

**Engine totality discipline (forward commitment, enforced from this rung):** all new
arithmetic on strength/levels/credits is saturating/checked; no unwraps on "impossible"
states. The world must deterministically degrade around absurd values — this is what
makes the future state-surgery interface (recorded counterfactual overrides; the
narrative-chaos layer's front door) cheap later. Surgery itself is NOT built this rung.

## 9. The lab: diagnostics FIRST, then the diagnosis matrix

**P0 builds the instrument before any pirate behaves** (`diagnostics.rs`: `TrophicSample`
per window — active/lying-low pirates, laden haulers, per-route robs/accepts/traffic,
laden-trips-per-window (the calibration input), engagements by trip-phase, purchases,
Yard treasury, per-craft credits; pure `classify() -> Diagnosis`). Validated by 4-corner
synthetic series AND the live positive control: `pirate_max_reach = ∞` + zero
relocation-stickiness MUST read RiskEqualized — re-validated against the OCCUPIED-route
metric. The integration test asserts classify() RUNS, never what verdict it returns.

Verdicts: `Alive | NoCycle | RiskEqualized | Saturated | DecisionNotTranslating |
PermanentPeace | ArmsRaceFlat` (no gate vocabulary in identifiers).

| Verdict | Signature | Knobs (in preference order) |
|---|---|---|
| NoCycle | populations flat / pinned | food band (from P0 formulas), lie-low durations, evidence_window ↑ (staleness sustains oscillation) |
| RiskEqualized | occupied-route concentration low / hot route tracks traffic | reach ↓, relocate_period ↑, stay_milli ↑, heat_threshold ↓ (more pulsing) |
| Saturated | every station covered most ticks | active pirates ↓ (pool / lie-low duty cycle) — coverage ≠ locality; distinct row, distinct lever |
| DecisionNotTranslating | risk heterogeneous but outcomes don't track choices | qty spread ↑, deadhead geometry spread ↑, evidence penalty ↑ |
| PermanentPeace | engagements → 0, all haulers ≥ pirate strength | caps/prices (pirate ladder must out-reach hauler ladder), ransom_cap ↑ |
| ArmsRaceFlat | no purchases / no regime changes | escort price ↓, ransom_cap ↑, vendor placement |

**Per-mechanic falsifiable hypotheses (baked in, all from one run's JSONL):** lurker
locality (reach=∞ ablation equalizes — the positive control); heat/lie-low (desynchronized
per-pirate duty cycles; heat=∞ ablation flattens risk-in-time); food-driven population
(upkeep=0 ablation kills the cycle); dock-gated evidence (evidence-off ablation damps the
cycle ≥ the staleness hypothesis predicts; belief-divergence > 0 across haulers); escort
threshold (step function at Δstrength = +1; within-hauler engagement collapse
after purchase); endpoint ambush (the owner's bimodal trip-phase histogram); Yard
circulation (treasury bounded, non-monotone); purchase-desync (ticks between first and
last Escort-L1 purchases across the fleet — near-zero = the synchronization death).

## 10. Chronicle + game runner

`crates/jumpgate-core/examples/trophic_run.rs` (an example, not a new crate — clippy
`--all-targets` covers it, zero packaging overhead): builds `scenario_trophic(seed,
knobs)` (NEW config fn beside the trader template: 1 star at the proven 1e-3 calibration,
6 station bodies a ∈ 0.35-1.4 AU, 12 scripted haulers, 6-pirate pool / 2 initially
active (expected-active ≤ stations − 2 by scenario law), ≥ 12 directed route templates
across 3 tier corps, Yard corp, 2 vendor stations, hideout = outermost body, v_e ×10),
steps 50k ticks, JSONL `TrophicSample` per window, `--replay-check` (bit-identical
`(tick, state_hash)` streams — the digest discipline for every behavior commit), and the
chronicle printer: per-craft life-arc lines from `recent_events` ("P2 robbed H7 off
Station 3's gantry for 1.8cr; heat 200", "H7 bought Escort L1 at the Yard", "P2 lies low
until t=14500"). The owner loop: run 50k, read 3 lives, ask "is this alive?" — the
classifier is the instrument beside the window, not the judge. `python/analysis/sweep_trophic.py`
grids seeds × knobs and prints matrix-row counts.

## 11. Gym extension (the second half: the trader as a player in the risk field)

- `trader_config_template(num_pirates: usize = 0)` — default 0 keeps every existing test
  and the keystone learning smoke byte-identical. Pirates' initial lurks seed-drawn (§5).
- Horizon 5000 for the pirates variant (≈ 6-10 decisions; a robbery's Δcredits lands
  inside the episode that chose the route). Baselines re-rolled at that horizon.
- **Obs: append K=2 contact blocks** (dims 20-33, stride 7: `[present, unit_bearing xyz,
  log1p(d/0.01)/PIRATE_LOG_DIST_SCALE, strength/PIRATE_STRENGTH_SCALE, active]`,
  `TRADER_OBS_DIM = 34`; consts 5.5 / 4.0; contacts sorted by distance). Raw evidence —
  positions, capability magnitude, lying-low visibility — NEVER route_evidence counts or
  any score (the agent derives route danger from contacts + geometry; detour-vs-cargo-value
  is the learned signature). Fixed compile-time scales, stationary by construction, no
  VecNormalize. Accessor: `World::pirate_contacts(observer) -> Vec<(Vec3, Vec3, u32, bool)>`.
- **NO purchase actions this rung** (Discrete(5) unchanged): upgrade payoff spans
  episodes (investment credit-assignment would confound the rung's clean question — "does
  the trader route around lurkers"); purchases are proven alive by scripted policies
  first. Upgrade actions are the natural NEXT gym rung.
- **Reported diagnostic, never a gate, never a trigger:** contact-aware PPO vs the same
  PPO with contact dims zero-masked, held-out seeds; plus route-share shift vs lurk
  positions. A null result is a PLAYER finding at these prices, logged and reported — it
  does not by itself send anyone back to the world design (PDR-0006; the panel excised
  the one sentence that said otherwise). The existing learning smoke stays the only gate,
  on the untouched 0-pirate scenario.

## 12. Build order (commit-sized, each independently green)

| Phase | Contents | Goldens |
|---|---|---|
| **P0 — instrument** | diagnostics.rs + classify + 4-corner synthetics + trophic_run.rs over the CURRENT world (no pirates; measures laden-trips/window for §4 calibration) + chronicle printer | untouched |
| **A — config** | CraftInit.role + scripted, BaseSpec.base_cargo_capacity, StationInit.sells_upgrades, TrophicCfg, ShipyardCfg; reset mints pirates | config re-pin (single-cause) |
| **B — state v4** | upgrades column, engage_cooldown_until, info_tick, route_evidence rings; HASH_FORMAT_VERSION 4 | state re-pin (single-cause) |
| **C — purchase verb** | BuyUpgrade + pending_upgrade + stage 1d + capacity gate at accept + UpgradePurchased + per-arm golden-value tests | unchanged |
| **D — encounter + robbery + population** | stages 3b2/3b3, first Piracy draws, ransom transfer, resolve_failures generalization, event emitters, threshold/ordering/conservation tests | unchanged |
| **E — brains + evidence** | run_pirate_brains (1c2), evidence rings bump/read, dock-refresh, belief-scored ASSIGN flag, scripted purchase policies, locality/determinism tests | unchanged |
| **F — lab** | scenario_trophic, JSONL, sweep script, replay-check integration, positive control | unchanged |
| **G — gym** | pirate_contacts, obs 34, num_pirates kwarg, horizon-5000 variant, layout + seed-variation tests, masked-ablation report script | unchanged |

Then the **console tuning loop** (≤ 24 parameterizations through the matrix, owner
reading chronicles) — a green build is not a met bar; the cycle is found by looking.

## 13. Scope cuts (each with its forcing reason)

Tanker + refuel + priced fuel (+ escrow-lock bug fix) → cut 2 package (§6). Fence /
stolen-cargo loot / value-seeking pirates / lawless-haven geography → cut 2 (the
self-cancellation finding; needs the fence-vs-REPOST diagnostic in hand). Chase/intercept
package (lead pursuit, craft contact events, tethers) → cut 2/3. Ballistic cargo dumps +
salvage/tugs → cut 3. HaulerKilled / despawn / mid-run spawn → cut 3 (slot==row; kill is
economically irrational under coercion logic anyway). Police/navy/FOB/bounties → cut 5.
Media engine → after pirates are alive (channels live WITH their generators). Multi-craft
fleets / formation escorts → the commodore era. Station treasuries / real markets →
their own rung. Narrative-event chaos layer + state surgery verb → last (forward
commitments in §8/§14 keep them additive).

## 14. Forward-design commitments (owner session 2026-06-10, banked)

1. **Endpoint-ambush hypothesis** (owner): engagement-phase histogram bimodal at
   launch/decel — pre-registered in §2; the same instrument later detects the tether-era
   mid-cruise mode. When pirates become RL players: do they REDISCOVER endpoint ambush?
2. **Tether coercion** (owner): rung-1 statistical resolution is the PERMANENT top layer
   of an economically-rational coercion game ("dump or lose your engines"); kill stays
   rare/tail forever; cargo dumps become retrievable ballistic objects in cut 3.
3. **High-g boarding / morale / narrative events** (owner): crew-level effects abstracted
   as EffectiveMods penalties + chronicle events; the narrative-chaos layer is the FINAL
   system, deliberately non-deterministic, quarantined behind the recorded ingest seam
   (record-at-injection ⇒ replay-from-record: chaotic runs stay bit-replayable; debugging
   step 1 remains "turn it off").
4. **State surgery** (owner): "what if that pirate were Darth Vader" — a sanctioned,
   recorded, provenance-stamped override verb; the engine is total over its state space
   and deterministically degrades around the damage; conservation identities extend to
   "modulo recorded miracles". Totality discipline starts NOW (§8); the verb lands with
   the chaos layer.
5. **Effects, not shadows** (owner, Discworld MUD): the end-state for all
   temporary/permanent modifiers is a hashed effects table {target, class, magnitude,
   applied/expires, provenance} folding into EffectiveMods, with a classification tree
   and declared stacking algebra. Per-instance specialness is DATA, never attached code
   (no shadows — behavior stays a function of (state, config, seed)). `lie_low_until` and
   upgrade levels are recognized hand-rolled instances; migrate when a third coexisting
   modifier family appears (morale).
6. **First-class goods** (owner, at decision-1 sign-off): eventually cargo stops being
   `(Resource, qty)` and becomes goods with size, mass, unique hauling requirements
   ("vibrates ominously", "needs cooling"), and provenance flags — notably
   `stolen_from: <tag>`. Consequences the rung-1 abstractions must not foreclose:
   (a) the ransom model's cargo→consumed leg is already explicit accounting, so swapping
   in a goods table later changes the leg's type, not its existence; (b) provenance is
   what makes cut-2 fencing real (the fence discount exists BECAUSE goods are traceably
   stolen; police seizure and media gossip key off the same tag); (c) hauling
   requirements are capability-matching — refrigerated holds etc. become catalog lines
   on the existing upgrade verb; (d) mass-bearing cargo touches accel and the reset
   guard/brakability — a physics rung, sequenced deliberately, never a side effect. Keep
   `Resource` append-only; the goods table arrives as a superset, not a migration.
7. **Ships as first-class part-graphs** (owner): the future ship = a base hull carrying
   a configuration of aftermarket parts — and combat damage is LOCATIONAL without FPS
   modeling: "hit in the rear → hit the engines → the engines are armoured →
   therefore…". The chain is: hit location = a pure function of engagement KINEMATICS
   (a stern-chase robbery hits engines; a head-on hits the bow — real Newtonian
   geometry, zero new simulation); location → parts via the hull's configuration map;
   armor = per-part modifier; consequence = the part's effects rows (§14.5) feeding
   EffectiveMods, so capability loss propagates as data. Rung-1 seams shaped for it:
   (a) capability access stays exclusively through `effective_params`/EffectiveMods
   (already law) — the parts model later replaces the INPUTS to mods, never the call
   sites; (b) the upgrade catalog + purchase verb IS the proto parts market
   (`UpgradeKind` append-only; "buy upgrade" generalizes to "install part");
   (c) engagement events carry a kinematic snapshot (relative bearing + speed at
   engagement — available free at the emission site, §2) so the hit-location model's
   data seam exists from rung 1, and the same snapshot feeds the endpoint-ambush
   discriminator today.
8. **Capability mixins** (owner): orthogonal Option-column capabilities on positioned
   entities; `sells_upgrades` debuts the pattern (§6). Latent content the composition
   rules must not foreclose: craft+fence (smuggler barge), craft+vendor (fleet tender),
   craft+refuel (tanker logistics), craft+patrol+contract_board+rearm (the navy frigate
   FOB — a roaming policed-core that suppresses blockade-grade piracy), and
   capability-claims-vs-truth deception (Q-ships) once media lands. Bounty hunting =
   the existing contract engine with a new contract type (completion = a named pirate's
   destruction).

9. **The crime/boarding dissertation** (`docs/superpowers/concepts/crimes.md`) is adopted
   as the cut-3+ blueprint for boarding, with one owner correction and three
   reconciliations against project law:
   - **Owner correction (the ambition):** the dissertation's closing frame — "texture
     without requiring Dwarf Fortress in space" — has it backwards. **Dwarf Fortress in
     space IS the point**; the abstractions are practical compromises until home
     supercomputers arrive, not design ceilings. Consequence: every abstraction in this
     project must carry its DEEPENING PATH as a first-class design artifact (the fleet
     ledger demotes to real ships; statistical resolution decomposes into staged
     boarding; crew pools promote to named characters; focus budgets are functions of
     COMPUTE, not philosophy — they grow). An abstraction without a written deepening
     path is a bug.
   - **Adopted as-is:** boarding = an interrupt-driven crisis mini-sim (zoom in around
     selected systems/characters/compartments, collapse back to macro with a CONSEQUENCE
     BUNDLE, never `success=true`); the staged operation
     (Commit→Suppress→Attach→Cross→Breach→Secure→Exploit→Consolidate); "DRL chooses
     intent/allocation/escalation/abort — procedural modules resolve technical
     execution" (our two-layer architecture, generalized); modules-propose-options,
     captain-arbitrates; crew as pools with named characters PROMOTED by events (the
     chronicle as character foundry); the explicit `IncidentFocusBudget` (formal LoD
     budget); prize-control as a separate problem ("can you actually run the bastard
     thing?"); intercept-combat / boarding / prize-control as three modules with
     distinct state spaces.
   - **Unified with §14.7:** "control domains" (bridge/engineering/cargo/propulsion as
     separate contested/controlled states) ARE per-location control state on the
     part-graph — one model, not two; hit-location and leverage-points read the same
     hull configuration map. Likewise the Commit phase's intelligence inputs (hull map
     quality, manifest confidence) are EVIDENCE-quality attributes — the media layer's
     streams feeding the boarding module, same seam.
   - **Reconciliation 1 (reward law):** the dissertation's pirate/freighter "reward
     component" lists (legal heat, reputation, crew loyalty, …) are utility models for
     SCRIPTED captains' doctrine only. For DRL players, reward stays currency-only;
     legal heat and reputation must price themselves through consequences (patrol
     intensity → lost income; fear → fewer compliant victims), never as reward terms.
   - **Reconciliation 2 (retired premise):** `doctrine: cautious|violent|desperate` is
     legitimate CK-style content on HEURISTIC captains; it must never become a taste
     scalar substituting for learned risk on a DRL agent (the risk_appetite ghost).
   - **Reconciliation 3 (determinism):** the "stochastic procedural resolver" draws only
     from seeded RngStreams (append-only), and event-promoted characters/narrative
     beats ride the recorded seam (§14.3) — a boarding replays bit-identically.

## 15. Owner decision points

1. ✅ **APPROVED (owner, 2026-06-10)** — with first-class goods banked as §14.6.
   **Takings model:** ransom transfer (RECOMMENDED — zero new identity legs, no
   fence-vs-REPOST self-cancellation, robbery debits the RL trader directly) vs
   cargo-loot + fencing now (richer, but the panel proved the fence damps the boom/bust
   it feeds unless fencing is restricted to lawless stations — cut-2 scope either way).
2. ✅ **APPROVED (owner, 2026-06-10).**
   **Tanker deferral:** Hull + Escort this rung; Tanker lands WITH the refuel package
   (cut 2) — accepts that one of your three verbatim lines arrives one rung late, for the
   three reasons in §6. Also requires deciding then whether tanker buys range or speed.
3. ✅ **APPROVED WITH CAVEAT (owner, 2026-06-10):** no "fleet as level" system lingering
   past its due date — fleets are "collections of ships with a single policy acting as a
   strategic head" (the commodore chair, not fleetLevel 3), and escorts get the same
   care. Hardened into §6's fleet-ledger rules (counts of un-simulated ships; demotion
   not reinterpretation; named sunset debt; never folded into physics).
4. ✅ **APPROVED (owner, 2026-06-10).**
   **Gym stance:** contacts-in-obs yes / purchase-actions no this rung. The trader reads
   the risk field; it cannot yet spend on protection.
5. ✅ **APPROVED (owner, 2026-06-10: "that is expected").**
   **Endgame asymmetry:** maxed pirates (3) out-rank maxed haulers (2) — predation never
   goes extinct, the frontier never becomes fully safe (until the navy). "The arms race
   favors the lawless" is the rung's intended end-state texture.

All five decision points are resolved; the spec is BUILD-AUTHORIZED as of 2026-06-10.
