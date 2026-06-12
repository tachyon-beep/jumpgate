# Goods as Goods — the bazaar and the crate (two rungs)

**Date:** 2026-06-12 · **Status:** owner-approved at brainstorm (this doc is the
write-up of that session) · **Frame:** PDR-0006 — judged by emergent play at
the console; every number below is a window, never a gate.
**Ancestry:** world-gets-big (`2026-06-11-world-gets-big-design.md`, landed
`b446095`), PDR-0007 (haulage cost ≠ cargo value), PDR-0008 (rob-anchor
information horizon). Chosen over the navy arc 2026-06-12 (tankers-first
sequencing; the frigate lands on this rung's moral geography next).

---

## 1. Frame and the play bar

Two console-judged rungs, built and judged in order:

**Rung A — "the bazaar."** The world stops being a restocking script and
becomes a market. Stations want different amounts of different goods;
corporations chase price spreads with sealed packages; a ship with money
trades for itself, a ship without works for wages. Judged by: spreads that
visibly close, stations that visibly starve when no trade pays, and the first
**emergent tanker** — a fuel package flowing to a non-refinery station because
the price said so, with zero fuel-specific code.

**Rung B — "the toll and the crate."** Predation meets property. The
own-cargo trader buys off a hungry pirate with a slice of hold; the sealed
crate hauler is ride-or-die; stolen crates resurface at desperate stations.
Judged by: the robbed→broke→stranded chain narrating end-to-end — the beat
world-gets-big promised and never fired (0 strandings in 120 runs, min tank
745‰), closed here by supply fragility instead of a calibration re-bake.

**Why this rung (the measured motivations, 2026-06-12 first look):** fuel
never moves between stations (only the three refinery stations ever held
stock; everyone else sat at 0 stock / max price all run); every rob is a flat
6.0M (pirates have no target structure for PDR-0007's value×frequency
corridors to act on); the tragedy tail is unreachable through play.

## 2. Decisions record (resolved at the 2026-06-12 brainstorm)

- **D1 — goods differentiate by geography only** (identical physics; same
  mass/volume per unit). The store schema carries a per-good property table
  from day one, uniform values now — the named seam for INDUSTRY later.
- **D2 — channel knowledge comes from the public board, not a scan.** Corp
  package contracts are publicly listed; anyone (pirates included) can read
  which hauler carries a sealed crate, on what lane. Own-cargo traders are
  dark because they never post anything. Crate *contents* stay sealed to
  everyone at engagement (lottery ticket). The corp channel trades safety for
  publicity.
- **D3 — scoop-vs-press is pirate policy, not config.** No hard-coded
  satiation threshold: the pirate decides when it has had enough (greed dial
  in the scripted v1 brain; learnable head later). Greed prices itself through
  the existing notoriety/lie-low machinery — greedy pirates get notorious.
  Nothing is shaped.
- **D4 — arbitrage replaces restock** in this scenario line. Corps post
  packages when spread × quantity clears transport cost + premium; the
  order-up-to REPOST machinery retires here. Restocking must emerge from
  prices. (Trophic control scenario untouched, bit-identical.)
- **D5 — fencing is a gradient.** The haven always buys stolen goods (fence
  discount → food_micros; the goods enter its supply and resell into the gray
  market). Other stations gain a gray-market posture derived from desperation
  (scarcity) — which the measured world says means the rim. Feeds the
  law-and-order arc: policing the core later pushes predation AND gray
  markets outward.
- **D6 — one role, two modes.** Every cargo craft chooses per-trip: claim a
  public package (wage, escrowed, sealed) or buy goods with its own credits
  (margin, owned, jettisonable). Scripted v1 policy: spread-chasing when
  capitalized, wage-hauling when broke. The trader/hauler mix is emergent,
  not a knob.
- **D7 — two rungs, economy before predation.** Rung A proves the market
  feeds the world with piracy on today's rules; Rung B changes predation.
  A pricing collapse can never be confounded with a predation change.
- **D8 — hunger-gated scoop economics.** Whether jetsam tempts a pirate reads
  from its food_micros state (a fed pirate is bought off cheap; a starving one
  presses for the hold) — but the *decision* belongs to policy per D3.

## 3. Rung A — goods, boards, packages, two modes

**Goods.** Extend the resource set to 10. PANEL CORRECTION (2026-06-12,
re-verified): `Resource` is `{Ore, Fuel}` — **Food does not exist as a good**
(every chronicle "food" is the pirate's `food_micros` ledger). So: keep Ore /
Fuel + 8 new goods, of which **Food ships as a real good WITH a consumption
recipe** (input-only Recipe, the fuel-sink shape) or WA1 has nothing to read.
Remaining 7 with real names (working set: Alloys, Medicine, Machinery,
Luxuries, Electronics, Textiles, Chemicals — chronicle lines read better than
"Widget D"; final naming free at plan time). Representation per OD-1:
runtime `Good(u16)` + Vec-backed columns, count from a config-tail GoodsCfg —
the per-resource hash folds carry no count word, so the refactor commit is
provably hash-neutral and Experiment C's 10→20 is config-only. Per-good
property table ships **minimal-live** per OD-7: name (never folded) +
unit_mass_milli (uniform 1000, read by the capacity gate on every transfer);
further columns land each with their first reader.

**Boards.** Per-station per-good live pricing — the generalization OD-4
anticipated when world-gets-big made Fuel the first live-priced good. The
price curve machinery exists; this rung turns it on for everything.

**Supply/demand geography.** Producers become seed-derived produce/consume
sets per station on the frontier band geometry (sources, sinks, amounts per
good). The existing Producer/Recipe seam carries it; multi-input recipes
(A+B→C) are explicitly OUT (the INDUSTRY hook, with D1's property table).

**Corp arbitrage.** Corps scan the public board and post a sealed package
contract when `spread × qty > transport_cost + premium`. Posting, escrow,
settle reuse the existing contract machinery; what changes is the *demand
generator* (D4). The premium is a corp config knob in v1 — the seam where
CORPS/COOPS ownership structures bolt on later (who owns treasuries, who may
post, co-op profit sharing).

**Two-mode craft (D6).** The per-trip channel decision is a policy seam in
the scripted brain. Own-trade = a buy verb at origin (own credits, integer
quantization, BuyUpgrade/Refuel-clone settle idiom) + a sell verb at
destination. Package = claim from the public board (today's ASSIGN, reframed
as claiming).

**Scenario.** A new factory (working name `scenario_bazaar`) on the frontier
10-station band geometry, so the judged frontier world stays reproducible
forever. scenario_trophic and scenario_frontier stay bit-identical.

**Rung-A exit (recorded, then console-judged):** the economy demonstrably
feeds the same trophic dynamics — boom-bust survives the demand-mechanism
swap; no-starvation or rim-localized starvation read at the console; the
emergent-tanker window (WA4) read.

## 4. Rung B — jettison, ride-or-die, the gray market

**Jettison.** An own-cargo trader under pursuit may cut loose a chosen
fraction of hold (trader policy decides whether/how much). Jetsam is a
deterministic in-space object; scooped loose goods convert directly to
food_micros (it is food-equivalent plunder, not a crate).

**Scoop vs press (D3/D8).** On jettison, the pursuing pirate's policy chooses:
break off and scoop, or press for the hold (and may do both — scoop then
re-engage). The v1 scripted brain parameterizes greed; the heat ledger does
the rest: pressing after scooping → more robs → notoriety → forced lie-low.

**Sealed crates are ride-or-die.** Escrowed corp property cannot be
jettisoned by the hauler (it doesn't own it). A successful rob moves the
crate — contents still sealed — into the pirate's hold. A laden pirate is
itself a watchable arc (and robbable prey, later).

**Fencing (D5).** Stolen goods convert to food_micros only at a willing
buyer: the haven (always, at the fence discount), or any station whose
gray-market posture is open — posture derived from desperation (scarcity of
the goods in question). Fenced goods enter that station's supply and resell.
The rob-where-you-fence exclusion (TROPHIC-C3, the haven-lurk fix)
generalizes per-fence: a pirate's hunting ground and its market stay
distinct wherever it sells.

**Channel selection by pirates (D2).** Pirates read the public board like
everyone else. In v1 the board is global knowledge (matching today's
position-blind dispatch); localizing board knowledge inherits the PDR-0008
information horizon and is the named INFO-LAYER hook, not this rung.

## 5. Pre-registered windows (recorded, never gated)

Rung A:
- **WA1 survival-by-market:** essential stock (Food/Fuel) at every station
  stays above zero — or starvation localizes to the rim. Either is a finding.
- **WA2 spreads close:** posted spread on a route decays after package
  delivery (arbitrage arbitrages).
- **WA3 channel mix is emergent:** own-trade share tracks craft
  capitalization over the run (broke ships work wages; rich ships trade).
- **WA4 emergent tankers:** fuel packages to non-refinery stations appear,
  with zero fuel-specific dispatch code.
- **WA5 trophic preservation:** boom-bust verdict mix on the bazaar scenario
  is distribution-comparable to the frontier bank (never same-seed paired).

Rung B:
- **WB1 risk premium emerges:** corp package wages on pirate-active lanes
  exceed quiet-lane wages (the premium shows up in posting behavior, unshaped).
- **WB2 the ecosystem taxes greed:** greedier pirate parameterizations show
  higher rob rates AND higher lie-low fractions.
- **WB3 gray-market geography:** stolen-goods sales concentrate where
  scarcity is (rim + haven), tracking the desperation posture.
- **WB4 the dry-ship beat:** at least one robbed→broke→stranded chain
  narrates end-to-end in an ensemble (closing the WGB beat-3 gap through
  play).
- **WB5 toll equilibrium:** jettison fractions and buy-off success rates
  stabilize across an ensemble rather than collapsing to always-die or
  always-pay.
- **WB6 gossip self-selection watch (PDR-0008) re-read:** with jetsam and
  fence events now in the world, re-read the carried-alert age skew.

## 6. Determinism and hash discipline (amended per panel, 2026-06-12)

- **Two format bumps, one per rung:** v6 = the owned `hold` column on all
  craft (rung A; pirate rows hold zeros, the cargo-None precedent); v7 =
  JetsamStore + pirate fence state (rung B). The pirate crate hold REUSES the
  v6 column.
- **The exit measurement is a behavior digest, never cross-bump hash
  equality** (HASH_FORMAT_VERSION folds as word 2 of every state hash):
  sha256 over stdout ∪ window-JSONL ∪ gossip-log per (scenario, seed) — the
  WGB phase-1 procedure. State-hash sequence equality is available for
  exactly one commit: the OD-1 runtime-goods representation commit (no bump).
  Instruments land FIRST and pin the digest baseline.
- New events (PackagePosted, TradeBought/TradeSold, Jettisoned, Scooped,
  CrateSeized, Fenced) are unhashed event-stream additions per the WGB idiom.
- No new RNG streams: all v1 policies read hashed world state.
- New config (goods tables, arbitrage premium, gray-market posture, greed
  dial) folds at config tail → one GOLDEN_CONFIG_HASH re-pin per rung,
  cause-documented.

## 6b. Panel record (15 agents, 2026-06-12, banked
`docs/superpowers/posts/2026-06-12-goods-as-goods-panel/`)

All 17 adversarial CRITICALs dispositioned in the banked recommended cut,
which is the buildable reference for the plan. Owner resolved all seven new
ODs to the panel recommendations (2026-06-12/13, no overrides):

- **OD-1(a)** runtime `Good(u16)` goods representation (unanimous).
- **OD-2(a)** one **Exchange corp** as the goods money counterparty
  (Yard/Port idiom, merged, covers the haven), honestly sized as a wind-down
  battery with drain printed as a standing read — "we're not standing up the
  economy out of whole cloth, we're laying the tubes" (owner). Named trigger:
  consumption-minted money if the console shows universal heat death.
- **OD-3(a)** fuel base-price re-bake in scenario_bazaar only (≈50_000) so a
  fixed-size fuel package can clear the arbitrage trigger (WA4 reachable).
  Fixed package sizes v1; **ORDER-IN** recorded as a named hook (§7).
- **OD-4(a)** wage = static transport floor + share of spread surplus —
  dangerous-lane wages rise through pure price mechanics; WB1 stays an
  honest emergent read.
- **OD-5(a)** omniscience principle RATIFIED: sealing binds POLICY reads,
  never the unhashed event stream (events are the owner's omniscient
  window). Riders: contents unseal on seizure (fence-seeking; the engagement
  decision stays content-blind); no gossip/media mint may ever source a
  sealed payload field. Recurs at INFO-LAYER.
- **OD-6(a)** ransom takings are prey-agnostic — a pressed-and-robbed trader
  loses goods AND wallet (this is what arms WB4's robbed→broke→stranded).
- **OD-7(a)** minimal-live property table (amends §7's literal 4-field
  wording; D1's seam intent honored).

Load-bearing panel corrections (each re-verified in the main loop):
rung-A piracy is coherent unchanged but own-cargo traders are mechanically
invisible to pirates, so **WA3 and WA5 are a joint read** (own-trade share IS
the pirate food supply); pirates are structurally immortal (starvation
re-mints grubstake — fence re-timing creates a duty-cycle hazard, never
extinction; the band re-walk accounts for the welfare mint); jetsam is a real
hashed store (cursor-folded, TTL expiry → consumed leg; the press-arm
canister floats); partial seize at pirate capacity, remainder → jetsam at the
rob point; corp posting uses deterministic rotation and the lurk-relocation
weighting reads Offered rows only (the dumb-by-signature law is repealed in
exactly those words); per-good topology must be CLUMPED (partitioned
k-ranges) with a pre-registered rung-A route-concentration panel — the three
self-averaging costumes each carry a constraint + a read.

## 7. Named future hooks (seams, not promises)

- **INDUSTRY:** per-good property matrix (mass/volume/value-density/
  perishability) on the D1 table + multi-input recipes on the Producer seam.
- **CORPS/COOPS:** ownership structures over treasuries and posting rights on
  the arbitrage premium seam.
- **ORDER-IN (owner, 2026-06-13):** need-sized, demand-pull corp ordering —
  "I need 30 clothes, they're expensive here but cheaper at Y, so I'll order
  them in": package quantity derived from the corp's own demand gap + price
  comparison, replacing fixed lots. The ghost of order-up-to returning, but
  PRICED as arbitrage rather than free restock.
- **NAVY/LAW:** lands on the D5 moral geography — policing shifts gray-market
  posture and displaces predation outward (the two-regime hypothesis).
- **INFO-LAYER:** board localization/staleness inherits the PDR-0008 horizon;
  pirate board-reading becomes hearing-dependent.
- **LEARNED HEADS:** the three policy seams (channel choice, jettison
  fraction, greed) are the DRL player surfaces, per the two-layer agent
  architecture.

## 8. Experiment C — depth and headcount (pre-registered, after Rung A or B)

Owner-added 2026-06-12. A secondary scaling experiment on the bazaar
substrate, no new mechanics:

- **Goods 10 → 20** by adding a raw-materials layer and a processed layer
  (flavor names in the spirit of "wafer rods", "viscous gel", "rockite").
  Processing is single-input (raw X → processed Y) — today's `Recipe` already
  does this (Ore→Fuel is one); multi-input stays behind the INDUSTRY hook.
  The goods graph gains DEPTH: some stations mine, some process, demand sits
  at the end of two-leg chains that must be HAULED between.
- **Ship-count ratchet:** sweep arms scaling total craft of all kinds
  (e.g. ×1 / ×2 / ×4 over the bazaar baseline, exact ladder at plan time),
  same seeds, same geometry.

**The question:** does more goods/industry feed more underlying people, or do
the existing cycles snap? Three registered outcomes, all findings:

- **WC1 capacity grows:** larger fleets stay fed (utilization, credits,
  pirate food) — economic depth raises carrying capacity.
- **WC2 cycles snap — collapse:** starvation cascades propagate through the
  two-leg chains (the processed-goods chokepoint: the wafer-rod shortage that
  stalls every chain downstream of it).
- **WC3 cycles snap — flatline:** boom-bust amplitude self-averages away as
  headcount rises (the contention-game LLN risk; the partitioned tier loops
  were built against exactly this — this experiment measures whether they
  hold). Alternation counts and per-window HHI vs fleet size are the read.
- **WC4 niche formation watch:** does the two-mode policy sort by goods layer
  (some craft live on raw legs, others on processed legs) without being told
  to — emergent specialization.

## 9. Out of scope (this pair of rungs)

Navy/police craft; learned policies (all brains scripted v1); the property
matrix (table ships uniform); multi-input recipes (single-input processing
chains are IN for Experiment C); board localization; pirate-on-pirate
predation of laden pirates (recorded as a watch, not built); co-op/ownership
structures; population entry/exit economics for cargo craft (fleet size is a
sweep arm in Experiment C, not yet an endogenous quantity — the natural
follow-on if WC1 reads).
