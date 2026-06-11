# Media Rung Cut 1 — "News Travels at Ship Speed" (gossip epidemiology design)

**Status: APPROVED 2026-06-11 — owner resolved OD-1…OD-6, all recommended options (§16). Build authorized.**

Designed 2026-06-11 by an 18-agent panel (5 grounding readers, 6 design lenses, 6
adversarial critics, 1 synthesis) over the owner's banked design authority
(`jumpgate-media-info-layer-design` memory, sessions 2026-06-10) and the landed
pirates rung 1. Every cited code fact was independently re-verified against HEAD
(`9166568`) by the main loop. Predecessor spec:
`2026-06-10-pirates-rung1-predation-and-upgrades-design.md` (esp. §7 evidence seam,
§14 forward commitments).

---

## 1. Frame and the play bar (PDR-0006)

Cut 1 is judged at the console by one arc — **a robbery becomes a story that travels
at ship speed; the story drains a lane at different times for different haulers
depending on where they have docked; the silence starves the pirate; the rumor ages
out per-reader and traffic returns.**

Pre-registered watchable shapes (windows, never gates):

1. Staggered `GossipHeard` chronicle lines ordered by lane distance from the
   victim's first dock — the news front, visible per craft.
2. Per-hauler accept divergence on the hot route — informed haulers avoid, ignorant
   haulers walk in.
3. Two regimes from one mechanism — big claims approach common knowledge, minor
   robs stay local and die.
4. Dead lanes carry no news — an effective blockade silences its own reporting.

Cut 1 closes the **hauler-information half + the metabolic pirate half** of the
boom-bust loop: robbery → victim carries news → gossip front → belief-scored ASSIGN
drains the hot route → pirates starve and relocate (existing hunger machinery) →
rumors age out per-reader → haulers return. The police half (information
*displacing* pirates, prey-gossip *attracting* them) waits for police and for the
owner-gated pirate-read arc (§16 OD-6).

Everything below is hashed, integer, bounded, reset-sized, inert by default, and a
pure function of (config, command log, Media stream). No gym change, no reward
change, no new action.

## 2. Data structures (hashed)

```rust
// crates/jumpgate-core/src/media.rs (new module)

/// One rumor as held by one node — the COVER. Truth lives in the run record
/// (Robbed + AlertBorn events, joined on alert_seq). EVIDENCE ONLY: raw ticks
/// and claimed integers; significance/staleness computed at the read, never
/// stored. No heat, threat, or confidence fields — ever.
pub struct GossipAlert {
    pub alert_seq: u32,            // hashed world mint counter — identity, dedup key,
                                   // eviction tie-break, lab join key
    pub route: u32,                // claimed directed route, from_row*n_stations+to_row
                                   // (TRUE in cut 1)
    pub pirate_slot: u32,          // claimed perpetrator CraftId.slot (TRUE in cut 1;
                                   // corruption = a later cut WITH its consumer)
    pub rob_tick: Tick,            // claimed when (TRUE in cut 1)
    pub claimed_value_micros: i64, // the ONLY mutating field — seeded from true loss,
                                   // inflates on retellings (§3)
    pub first_heard: Tick,         // when THIS node acquired this copy (raw; the
                                   // per-reader staleness anchor)
    pub hops: u8,                  // saturating; 0 = the victim's own copy
}

pub struct GossipBuffer { pub slots: Vec<Option<GossipAlert>> } // cap from config, fixed at reset
```

Placement:

- `World::station_gossip: Vec<GossipBuffer>` — len `n_stations`, sized ONCE at
  reset (the RouteEvidence sizing law, world.rs `reset`; no mid-run station spawn
  in v1), empty-slot init.
- `CraftStore::gossip: Vec<Option<GossipBuffer>>` — `Some(empty)` for
  **Hauler-role craft only** when media is live; `None` for pirates
  (information-blind by construction — `relocate_lurk_target`'s geometry-only
  signature stays the compile-level enforcement; §16 OD-6) and `None` everywhere
  when media is off. Initialized in BOTH `CraftStore::push` and `World::reset`'s
  mint loop, length-parallel slot==row.
- `World::next_alert_seq: u32` — hashed mint counter, 0 at reset.

Unhashed diagnostics (the `engagement_diag` pattern, never a behavior input):
`World::media_diag { evictions: u64 }` — eviction accounting stays out of golden
discipline.

## 3. Significance and the damped corruption loop

```rust
/// milli (0..=1000), computed at the read — never stored.
fn sig_milli(claimed_value_micros: i64, cfg: &MediaCfg) -> u32 {
    ((claimed_value_micros / cfg.sig_divisor_micros) + cfg.sig_floor_milli as i64)
        .clamp(cfg.sig_floor_milli as i64, 1000) as u32
}
/// Effective transfer P, hop-attenuated (integer pow, saturating):
/// p_milli = sig_milli * (1000 - hop_loss_milli)^hops / 1000^hops
```

**Seed honesty (verified at pirate.rs `resolve_encounters`):**
`Robbed.value_micros` is the wallet-clamped RANSOM — near-constant 6M in the
console-baked band — so it must NOT seed significance (a panel design built on
this and was caught by critique). The seed is the victim's true loss in hand at
settlement: **robbed contract `reward_micros` + ransom actually paid**. Contract
tiers (scenario qty/tier spread) give real variance, so transfer P genuinely spans
(0,1) and the minor-rumor-dies arc is witnessable.
**Implementation note (main-loop verification):** `contracts.reward_micros[kidx]`
must be read BEFORE `settle_contract_failure` tears the contract down — the
settlement call precedes the ransom computation at the write site.

**The double-feedback loop is damped by arithmetic** (critique CRITICAL: inflation
raises significance which raises both transfer P and eviction survival): inflation
fires only on retellings (hops ≥ 2); at `inflation_milli=125` against
`hop_loss_milli=150` the per-hop net factor on transfer probability is
1.125 × 0.85 ≈ 0.956 < 1 — monotone decay; the telephone game pays a distance tax
no matter how big the claim grows. Claims saturate at `claimed_value_cap_micros`.
Knob starting values are explicitly **re-derived from the measured band reward
distribution at console before pinning** — worked points against the real tier
spread, never invented. (Corruption scope is §16 OD-3.)

## 4. Generation (stage 3b2, inside `resolve_encounters` settlement)

At a Robbed settlement, co-located with the existing evidence-ring write, no draws:

1. Mint `alert_seq` (dense pirate-row order at a fixed stage ⇒ deterministic).
2. Push `GossipAlert { route, pirate_slot, rob_tick: now, claimed_value:
   contract_reward + ransom_paid, first_heard: now, hops: 0 }` into the
   **victim's** buffer. Cover == truth at hop 0. The pirate gets no copy.
3. **Origin-pier deposit** (critique fix): if the victim is currently within
   `ARRIVAL_RADIUS` of a station (the documented phase-0 origin-dock-ambush mode —
   rob-on-load is the dominant robbery class), deposit a copy (hops 1, no
   inflation — a firsthand report) into that station's reservoir the same tick.
   Without this the edge-trigger never re-fires for the dominant class and the
   pier never hears about the robbery on its own doorstep. Other docked craft
   still hear no earlier than their next dock edge.
4. Emit `AlertBorn` (§8). `DrivenOff` writes and emits nothing (evidence =
   successful robs, unchanged).

"News fronts travel at ship speed" and "dead lanes carry no news" are automatic
world properties of victim-as-index-case seeding — not tuned behaviors. The panel
unanimously named this the strongest single element.

## 5. Propagation: edge-triggered dock exchange (stage 3b2, before encounters)

Internal order of the existing gated 3b2 block, declared in-place per the stage
convention: (i) resolve `station_pos`; (ii) **dock-edge detection → exchanges →
info_tick refresh**; (iii) `resolve_encounters`. The 3b-before-3b2 ordering is
untouched (destination-dock sanctuary preserved); a robbery this tick propagates
to non-victims no earlier than their next dock edge.

- **Edge predicate** (the load-bearing fix — three independent critics flagged
  per-tick transfer as self-averaging rebuilt one layer up; verified: the
  info_tick loop is level-triggered every tick within `ARRIVAL_RADIUS`): computed
  during the same arrival-radius scan **reading the pre-refresh `info_tick`** — a
  craft is on a dock edge iff it is within `ARRIVAL_RADIUS` this tick AND its
  pre-refresh `info_tick` shows it was not docked last tick
  (`info_tick != next - 1`). Zero new state. Pinned with the unit test: *a craft
  parked N ticks produces exactly ONE exchange.* Oscillation across the radius
  re-fires; harmless (dedupe is idempotent), documented.
- **Partner**: the LOWEST station row within radius (explicit deterministic rule).
- **Role filter**: Hauler-role craft only — pirates neither upload nor download
  (no accidental pirate couriers; §16 OD-6).
- **Direction order pinned**: ship→station uploads (sender slot order), then
  station→ship downloads (sender slot order); documented + unit-tested.
- Per candidate item: (1) **dedupe first** — receiver holds `alert_seq` ⇒ skip,
  NO draw (draw count = pure function of hashed membership; the Class-3 stream
  cursor stays transitively pinned); (2) ONE `RngStream::Media` draw: transfer
  iff `u % 1000 < p_milli` (hop-attenuated, §3); (3) on transfer, the receiver
  copy gets `hops+1` (saturating), `first_heard = now`, and — iff resulting
  hops ≥ 2 — deterministic inflation
  `claimed = min(claimed × (1000 + inflation_milli) / 1000, cap)`. Sender
  untouched; first-heard sticks; re-hearing never re-inflates (dedupe kills the
  evict-replant ratchet at a node within retention).
- Bounded cost: ≤ (station_slots + craft_slots) draws per dock edge, only for
  craft on an edge.

The P(escape station) = 1 − (1−p)^k law is literal, with k = dock-visit EDGES.

## 6. Eviction (deterministic total order, no draws)

Insert algorithm: (a) lowest-index empty slot; (b) else any slot whose item has
`(now − first_heard) > trophic.evidence_window` is reclaimable space —
**forgetting has one owner, the read window**; eviction is purely overflow;
(c) else `priority = rob_tick.0 + claimed_value_micros × value_ticks_milli /
1_000_000` (i64): evict the argmin iff the incoming priority exceeds it, else DROP
the incoming; ties evict the lowest `alert_seq`. Unit tests: total order, tie
case, evict-replant cycle.

## 7. The read: `route_evidence` swapped behind its signature

```rust
pub fn route_evidence(&self, reader: CraftId, route: usize) -> u32 {
    if media_live { /* count of reader's OWN buffer items with item.route == route
                       && (tick - item.first_heard) <= trophic.evidence_window */ }
    else          { /* legacy ring + reader's info_tick — byte-identical */ }
}
```

- **Raw count, zero weighting** (critique CRITICAL: a significance-weighted count
  is a valuation crossing the world API): semantics preserved ("count of recent
  robs"); valence stays in the consumer (`evidence_penalty_milli`); the 900-clamp
  avoid-not-erase calibration at the ASSIGN site is untouched.
- **Per-reader forgetting clock** (critique CRITICAL: anchoring staleness on the
  global tick vs `rob_tick` installs one synchronized world forgetting clock):
  staleness anchors on `first_heard` at THIS node — each hauler forgets on its
  own acquisition clock, so the return to a cooled route staggers exactly as the
  avoidance did. The documented "desynchronize by lived docking rhythms" property
  is preserved and sharpened: per-craft *content*, not just per-craft staleness.
- `info_tick` keeps refreshing (it is the dock detector); its evidence-read role
  ends when media is live.
- **Config validation at reset** (BadEconomyRef precedent):
  `hauler_belief_scoring=true` with no live evidence path = hard error; exactly
  one of {station, craft} caps > 0 (half-on) = hard error.
- The legacy ring keeps being written as the media-off fallback; its retirement
  is §16 OD-2.

## 8. Events and chronicle (hash-neutral)

- **`AlertBorn { alert_seq, route, pirate, hauler, truth_value_micros,
  claimed_value_micros }`** at mint — the truth join, capturing the route at the
  only moment event↔route↔subjects are simultaneously resolvable (fixes the
  `route_of → None` offline-join caveat). No chronicle arm (it shadows `Robbed`).
- **`GossipHeard { carrier: GossipNode{Station|Craft}, alert_seq, route,
  pirate_slot, claimed_value_micros, hops, rob_tick }`** — latched on FIRST
  insertion per (node, alert); the victim's own hops-0 seed does NOT emit
  (`Robbed` tells that story). **Self-contained payload** so the chronicle prints
  without joins: `heard: route 2→0 robbed by P7 (claimed 7.1cr, 3 hops, 1,900
  ticks stale)`. `chronicle_subject` arm for Craft carriers only (the chronicle
  is craft-grouped; station hearings feed the gossip log and panels; a
  station-thread chronicle is a named deferral). Printer-side
  `--chronicle-gossip-min-micros` filter (default print-all; owner tunes at
  console). Volume is bounded by the latch (alerts × nodes); measured in the
  first console session — the seed-7 Arrival-spam lesson stands.

Events stay outside `state_hash`; propagation is a staged world-step mechanic
reading prior state, never an event-handler cascade (single-emit-path law).

## 9. Lab bench (windows, never gates)

- **TrophicSample additive integer fields** (half-open `(window_start, tick]`
  convention; JSONL keys additive; the anchored RESULT line untouched):
  `gossip_born`, `gossip_first_heard`, `alerts_carried`, `stations_with_news`,
  `per_station_alerts: Vec<u32>` (the news-desert map), `per_station_contacts:
  Vec<u32>` (dock EDGES per window — the P(escape) denominator),
  `heard_lag_ticks: Vec<u32>` + `heard_hops: Vec<u32>` (per first-hearing).
  Evictions ride the unhashed `media_diag` counter into JSONL.
- **`media_classify(&[TrophicSample]) -> MediaReading`** — a separate pure
  classifier; `Verdict`/`classify()`/RESULT stay byte-untouched: `NoMedia`
  (born==0), `NewsDesert` (born>0, zero hops≥1 hearings), `StaleEcho` (births
  first half, zero second half while held-alerts stay high AND robs continue —
  the PermanentPeace analogue: a false witness over a dead coupling),
  `CommonKnowledge` (run-aggregate escape ≥ 950‰ AND final coverage complete —
  the self-averaging alarm), `Localized` (the alive reading). An anchored
  `MEDIA seed=… born=… escaped_milli=… median_lag=… p90_lag=… reading=…` line +
  `MEDIA_RE` in `sweep_trophic.py` land in the SAME commit (the lockstep rule).
  `escaped_milli=0` sentinel at born==0. A labeled synthetic per reading,
  including the quiet-but-alive StaleEcho trap (the seed-7 rule: every new
  metric ships with a synthetic that would catch it lying).
- **Controls** (instrument-kill discipline; assertions conditional on born>0):
  `media_default_is_inert` (default config: zero Media draws + cross-branch
  event-digest equality with pre-media HEAD); **M-DEAD** (sig forced 0) must
  read NewsDesert; **M-ORACLE** (p=1000, huge caps/window) must read
  CommonKnowledge; **deaf control** (media live, belief OFF) must be
  element-identical to the belief-off baseline on the pre-registered behavioral
  trace (per-window traffic/accepts/robs/per-craft credits — NOT state_hash,
  which legitimately differs).
- **Panels** (`sweep_trophic.py`): knowledge-front (lag histograms, raw ticks,
  median/p90, lag-vs-hops); news-geography (hub/backwater ratio); bimodal reach
  by claimed-value quartile; **saturation window** (fraction of live alerts held
  by >800‰ of craft, with a pre-registered expected band at defaults);
  P(escape)=1−(1−p)^k per-visit analytic check; **avoidance-lag at EVENT
  resolution** from `--gossip-log` + ContractAccepted ticks; **paired ecosystem
  reading** — same seed grid, legacy-read vs gossip-read, verdict distributions
  REPORTED side by side (does the boom-bust cycle survive the swap — the swap
  touches the documented load-bearing desync mechanism); **value-of-information
  panel** — media-live vs media-dead, BOTH arms `hauler_belief_scoring=true`,
  across the stakes band, reported never gated. A ~0 reading is a finding that
  points the next bet at world prices — owner's call, no kill-criterion
  vocabulary anywhere.

## 10. Stakes (the world-price answer to the recorded ablation NULL)

The NULL is real (`runs/pirates_ablation.log`: contact-aware 1.620±1.745 vs
zero-masked 1.800±1.898 Δcredits/ep held-out; delta −0.180; per-decision robbery
costs ~2–20 cr). **Step 0 of the build: promote those numbers into a committed
lab note** (the run log is uncommitted bench state; the finding deserves
provenance). Cut 1's response is a `media_stakes` SWEEP knobset over existing
world knobs — ransom_cap and contract reward spread (note `scenario_trophic`
bakes ransom 6M while `TrophicCfg::default` is 2M; any probe must run in the band
it speaks for) — targeting "one robbery ≈ 3–5 trips' net profit" as a shape to
recognize, never a number to hit. Every stakes run keeps the rung-1 verdict panel
live (higher ransoms feed pirate metabolism; the band was console-baked).
Defaults change only by owner decision after readings (§16 OD-1). Reward stays
Δcredits; no shaping anywhere.

## 11. Config — `MediaCfg`

| knob | inert default | live start | meaning |
|---|---|---|---|
| `media.station_gossip_slots` | 0 | 16 | reservoir cap; part of the live predicate |
| `media.craft_gossip_slots` | 0 | 8 | hauler comms-log cap; part of the live predicate |
| `media.sig_floor_milli` | 50 | 50 | minimum transfer P (re-derive at console) |
| `media.sig_divisor_micros` | 10_000 | re-derived | claimed micros per sig-milli (against the real tier spread) |
| `media.hop_loss_milli` | 150 | 150 | per-hop transfer attenuation (the distance tax) |
| `media.inflation_milli` | 125 | 125 | retelling inflation (hops≥2 only) |
| `media.claimed_value_cap_micros` | 32_000_000 | 32_000_000 | claim saturation bound |
| `media.value_ticks_milli` | 1000 | 1000 | eviction-priority ticks per credit |

`media_live = station_gossip_slots > 0 && craft_gossip_slots > 0` (half-on =
reset error); the whole subsystem additionally rides the `engage_radius_au > 0`
trophic gate — **documented dual gating**, with the inertness test asserting both
single-lever cases. All knobs get `apply_knob` arms (unknown knobs abort sweeps).
Reuses `trophic.evidence_window` (4000) and `evidence_penalty_milli` (150)
unchanged. NOT shipped: identity-decay / route-blur / time-blur knobs — deferred
means not built (no dead knobs at zero).

## 12. Landing plan (single-cause commits)

0. **Hygiene first**: commit the ablation-NULL lab note; land the escrow-lock bug
   (jumpgate-2c0c2d92bb) and the heterogeneity-metric calibration
   (jumpgate-50c6a8a3bd) — fix the substrate and calibrate the instrument before
   changing the field it measures.
1. **B0**: `RngStream::Media` + `SALT_MEDIA = 0xBB67_AE85_84CA_A73B` (frac √3,
   continuing SALT_PIRACY's frac √2 convention) + pinned first-draw golden test.
   Hash-neutral, behaviorally inert.
2. **A (config-shape golden)**: `MediaCfg` + tail folds (the exhaustive-destructure
   compile error forces completeness) + `apply_knob` arms + GOLDEN_CONFIG_HASH
   re-pinned via `print_golden_config`.
3. **B (state-shape golden)**: hash words 30 (`write_craft_gossip` — its own
   `pub(crate)` helper invoked after `write_route_evidence`, keeping catalog
   order ≈ byte order), 31 (`write_station_gossip`), 32 (`next_alert_seq`);
   HASH_FORMAT_VERSION 4→5; both state goldens via `print_golden`;
   `manual_zero_fold` gains the words; per-field `moves!` tests. Nothing
   behavioral.
4. **C (mechanics)**: mint + pier deposit + edge exchange + eviction behind the
   gate, TDD.
5. **D (read swap)**: media-live path + media-off byte-identity parity test +
   reset validations.
6. **E (instruments)**: AlertBorn/GossipHeard + chronicle arm + TrophicSample/
   JSONL + MEDIA line/MEDIA_RE + panels + controls + `--gossip-log`.
7. **Console band session**: stakes sweep, owner reading, OD-1 resolution.

## 13. Test list

inert digest vs pre-media HEAD · replay-check on media-live runs · parked-N-ticks
⇒ one exchange · dedupe consumes no draw · direction-order pinned · eviction
total order + ties + evict-replant · witness seed cover==truth hops 0 ·
origin-pier deposit on phase-0 robbery · sanctuary ordering untouched ·
pirate-blind (role filter + no accessor reachable from pirate.rs) ·
route_evidence semantics (empty ⇒ 0; per-reader window; clamp behavior unchanged
at the economy site) · half-on config rejected · deaf-control trace identity ·
M-DEAD / M-ORACLE conditional assertions · MEDIA_RE smoke · synthetic battery +
StaleEcho trap · P(escape) labeled synthetic (hand-computable two-craft scenario).

## 14. CRITICAL-findings disposition and open risks

| Panel finding | Disposition |
|---|---|
| Per-tick transfer retry ⇒ self-averaging (substrate, lab critics) | FIXED: edge-triggered exchange, pre-refresh predicate, unit-tested |
| Seed value fabricated — ransom is near-constant (play critic; **re-verified in code**) | FIXED: seed = contract reward + ransom paid (real tier variance; read before teardown) |
| Valuation weighting inside the accessor (play critic) | FIXED: raw count; valence stays in the consumer |
| Inflation double positive feedback (artifact critic) | FIXED: hop-attenuated transfer, retelling-only inflation, net per-hop factor < 1 by arithmetic, claim cap, saturation window pre-registered |
| `event_seq` couples hashed state to the unhashed EventStream (scope critic) | FIXED: dedicated hashed `alert_seq` |
| Global forgetting clock (scope critic) | FIXED: per-reader `first_heard` anchor |
| Gym has no gossip generator / dead commitment token / trigger-as-gate (gym critic ×3 CRITICAL) | MOOT for cut 1 (no gym); recorded as binding revisions in the frozen obs-contract appendix; the gym pull is a plain sequencing call (§16 OD-4) |

**Open risks carried honestly:** saturation at 4–6-station scale even after
damping (pre-registered saturation band; the honest fix is map growth, not
artifact changes) · eviction may rarely bind at rung-1 rob rates (the loss leg is
carried by stochastic transfer; `alerts_evicted` + station-content divergence
watched; shrink caps if zero) · scripted-ASSIGN-only is the cut's play bet —
population-level waves, not individual visible detours, carry the judgment until
a detour surface lands · lag mismatch between gossip retention and pirate
lie-low timescales can phase-lock the cycle — the central console-tuning job,
read through the existing verdict machinery + MediaReading.

## 15. Deferred (named triggers — deferred means not built)

- Ship↔ship comms relay — first mid-flight decision surface or pirate
  eavesdropping mechanic.
- Identity/route/time corruption + per-source trust — a learner that can develop
  skepticism; source/channel id arrives with a second channel.
- Pirate couriers and pirate gossip reads — a deliberate owner-gated arc with
  value-seeking pirates (OD-6).
- AIR code: coverage radius (first frontier map), maydays (generator AND
  responder must both exist — OD-5b).
- Gym comms-log block + media head Rung A + discriminator suite (VoI mask,
  identity shuffle, accept-vs-value regression, act-on-rumor-vs-age) — frozen
  paper appendix, revised per critique (background scripted haulers; station
  entity codes on board slots; per-board-slot attention queries; commitment
  token waits for the detour verb); wiring is a sequencing call post-band (OD-4).
- LSTM Rung B — pre-registered moving-lurker gap test; never a live mechanics
  change.
- In-flight detour verb — lands with maydays.
- RouteEvidence ring + hash word 29 + the info_tick evidence-read path + the
  `risk_appetite` ghost column — retired together in cut 2's scheduled format
  bump (OD-2).
- Station-thread chronicle + Verdict-matrix integration — after the two
  instruments are observed to co-vary.
- `stolen_from` goods provenance hook (§14.6 first-class goods) — the alert
  schema is append-only, the superset stays open.

## 16. Owner decision points — RESOLVED 2026-06-11

Owner reviewed the presented decision points and authorized "plan and implement
the media cut-1 design" — all six recommended options stand approved: **OD-1(a)**
stakes as a sweep axis, defaults re-baked only after the owner console session;
**OD-2(a)** ring sunset in cut 2's format bump (digest control + console PDR,
bundling the `risk_appetite` ghost removal); **OD-3(a)** inflation-only
corruption, identifiers/routes TRUE; **OD-4(a)** console-only cut 1, obs
contract frozen as a paper appendix; **OD-5(a)+(b)** both ratifications (board/
prices = the AIR-mundane channel; the two-sided generators-AND-consumer rule);
**OD-6(a)** pirates information-blind. Original options preserved below for
provenance.

**OD-1 — Stakes re-pricing.** The recorded ablation NULL says robbery is too
cheap (~2–20 cr/decision) for risk information to matter; the lawful fix is world
prices. But the current band (ransom 6cr / upkeep 12) is your console-baked
calibration, and higher ransoms feed pirate metabolism. Options: (a) ship
`media_stakes` as a sweep axis now, you re-bake defaults only after a console
session reading the VoI panel and the rung-1 verdict panel together
(**recommended — all six designers converged here**); (b) re-bake defaults
immediately; (c) hold prices fixed and let cut 1 demonstrate propagation alone.

**OD-2 — RouteEvidence ring sunset.** The ring (hash word 29) is the documented
degenerate proto-channel. Options: (a) retire in cut 2's scheduled format bump,
triggered by media-off behavior preservation (digest control) AND your play
judgment at the console recorded as a PDR; bundle the `risk_appetite` ghost
removal into the same bump (**recommended**); (b) retire now in cut 1, no
fallback; (c) keep dual-mode indefinitely (the lingering-abstraction the
fleet-ledger caveat forbids). Note: the originally proposed "verdict parity"
trigger was rejected by critique as incoherent (gossip exists to CHANGE the
verdicts) and gate-smelling.

**OD-3 — Cover corruption scope (an in-fiction commitment: what can land gossip
lie about?).** Options: (a) magnitude inflation live, loop-damped, identifiers
and routes TRUE — the sole consumer counts route-matched alerts and is provably
immune to the lie, so the big-score heavy tail starts generating texture with
zero ghost-channel risk (**recommended**); (b) nothing live — cover==truth,
validate the propagation instrument alone this cut, corruption lands next cut
against a trusted baseline (the conservative call; costs one cut of
distributional texture); (c) inflation + identity decay now (critiqued as dead
freight: identity corruption has no consumer until something can be deceived).

**OD-4 — Gym sequencing.** Options: (a) console-only cut 1; the revised obs
contract is banked as a frozen paper appendix; gym wiring is a plain sequencing
decision you make after the band session, with the scripted ON/OFF spread
reported alongside (**recommended**); (b) wire and train in the same rung
regardless; (c) no paper contract either. The critique killed cut-1.5-as-written:
the gym world currently has NO gossip generator (scripted ASSIGN off ⇒ the trader
is the only robbable hauler — nothing third-party to hear), and a numeric
pull-trigger is the retired gate pattern.

**OD-5 — Two ratifications of media fiction/law.** (a) Declare the existing
contract board + price feeds retroactively as the AIR-mundane channel (zero
code; coverage radius lands with the first frontier map as the named sunset) —
critique established this is a PROPOSAL needing your sign-off, not existing
authority. (b) Harden "channels live WITH their generators" into the two-sided
rule "generators AND a consumer that can act" (used here to defer mayday and
DrivenOff channels). **Recommended: ratify both** — an unread channel is worse
than a missing one, and the air declaration costs a paragraph while foreclosing
nothing (the 4-station map is one coverage zone, so today's global board is
already correct under the declared fiction).

**OD-6 — Pirates stay information-blind through cuts 1–2.** No gossip buffers,
no reads; `relocate_lurk_target`'s geometry-only signature remains the
compile-level enforcement; pirate-side participation arrives only as a
deliberate owner-gated arc (value-seeking, fencing, prey selection). Options:
(a) blind (**recommended — unanimous post-critique**; the interdiction-RL lesson
says information-chasing pirates self-equalize risk and destroy the very
heterogeneity that makes hauler information valuable; prey-that-knows vs
predator-that-hungers is the watchable asymmetry); (b) pirates as mute couriers
now; (c) pirates read gossip now (reverses a hard-won empirical NO-GO).
