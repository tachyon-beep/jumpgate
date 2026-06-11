# Media Rung Cut 1 ("News Travels at Ship Speed") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the gossip-epidemiology information layer — stations as reservoirs,
hauler craft as vectors, victim-as-index-case seeding, edge-triggered dock exchange,
hop-attenuated significance transfer, retelling inflation, deterministic eviction,
the `route_evidence` read swap, and the media lab bench — per
`docs/superpowers/specs/2026-06-11-media-rung1-gossip-design.md`
(THE SPEC; read it first, it resolves every "why"; §16 records the owner's OD-1..6
resolutions; §14 records the panel CRITICAL dispositions that are LAW for this build).

**Architecture:** all behavior is in-world deterministic tick stages over hashed
integer state; the ONLY new randomness is `RngStream::Media` (one draw per
candidate item after dedupe); exactly two single-cause golden commits (A config,
B state v5); every behavior commit proves itself replay-bit-identical and the
default (media-off) world behavior-identical to pre-media HEAD.

**Tech stack:** Rust 2024 (`gen` reserved), clippy `--all-targets -D warnings`
(NEVER `--lib`, it is a no-op here), serde_json (already a dev-dep), python3
analysis scripts, pytest (gym untouched this rung — OD-4: console-only).

**Project laws (verbatim, non-negotiable):**
- Goldens at HEAD: `HASH_FORMAT_VERSION = 4`, `GOLDEN_ZERO_STATE_HASH =
  0xafdc_5c35_6266_0ff0`, zero-world `state_hash` golden `0xa29b_6334_16f7_cd20`
  (hash.rs `state_hash_golden_zero_world`), `GOLDEN_CONFIG_HASH = 0x1798_b108_edae_5bb6`.
  They move ONLY in Tasks 5 (config) and 6 (state), one cause each, literals
  re-derived via the ignored `print_golden` / `print_golden_config` tests — NEVER
  invented. Every other task ends by asserting they are unchanged.
- Subagents do not report gate claims as fact; the main loop re-verifies
  (`cargo test --workspace`, clippy, the pinned hashes, the bench diffs).
- Never `git add -A` / `.`; explicit paths only; never stage `.gitignore`,
  `.claude/`, `CLAUDE.md`, `AGENTS.md`, `.mcp.json`, `.filigree.conf`, `runs/`.
  Commit messages with parens use `git commit -F` heredoc. Trailer:
  `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- PDR-0006: every metric/classifier/panel is a designer's WINDOW, never a gate.
  No kill-criterion vocabulary anywhere. Reward stays Δcredits; no shaping.
- No new per-craft taste scalars. No valuations crossing the world API:
  `route_evidence` returns a RAW count; valence stays in the ASSIGN consumer.
- Pirates are information-blind (OD-6): no gossip buffers, no reads;
  `relocate_lurk_target`'s geometry-only signature is the compile-level fence.
- Cover corruption = magnitude inflation ONLY (OD-3); identifiers/routes TRUE.
- The legacy RouteEvidence ring keeps being WRITTEN (media-off fallback); its
  retirement is cut 2 (OD-2). Do not touch hash word 29 or the ring write site.

---

## Task 0: hygiene — the ablation lab note

**Files:**
- Create: `docs/superpowers/lab-notes/2026-06-11-ppo-contact-ablation-null.md`

- [ ] **Step 0.1** Write the lab note from `runs/pirates_ablation.log` (read it; it is
  bench state and must be promoted to committed provenance). Content: the run recipe
  (20k steps/arm, num_pirates=2, held-out seeds 10000–10019, two arms contact-aware vs
  zero-masked), the exact numbers (aware 1.620±1.745 vs masked 1.800±1.898 Δcredits/ep,
  delta −0.180; action shares [0, .681, 0, 0, .319] vs [0,0,0,0,1]; per-decision robbery
  costs ~2–20 cr both arms), the PLAYER interpretation (avoidance not worth learning at
  these prices; the masked arm's degenerate always-slot-4 policy loses nothing; the
  aware arm consumed the obs but found no profit), and the frame line: REPORTED, NEVER
  GATED (PDR-0006) — the lawful response is world prices + information (this rung),
  never shaping. Cite `jumpgate-no-shaping-add-capacity-principle` and spec §10.
- [ ] **Step 0.2** Commit (explicit path):
  `git add docs/superpowers/lab-notes/2026-06-11-ppo-contact-ablation-null.md` →
  `lab(pirates): bank the PPO contact-ablation NULL as a committed note`.

## Task 1: hygiene — escrow-lock bug (filigree jumpgate-2c0c2d92bb)

**Files:**
- Modify: `crates/jumpgate-core/src/economy.rs` (`resolve_failures`,
  `FailureCause`, `settle_contract_failure` debug_assert)
- Test: in-module `#[cfg(test)]` in economy.rs

The bug: `resolve_failures` only fails `InTransit` contracts on FuelEmpty. A hauler
that runs dry on the DEADHEAD leg (status `Accepted`, escrow already debited) or in
the one-tick `CargoLoaded` window locks its escrow forever. Fix = option (a) of the
issue: fail+refund all three escrow-holding non-terminal statuses.

- [ ] **Step 1.1 — failing test** `fuel_empty_mid_deadhead_refunds_escrow`: build the
  existing `two_body_starved_contract_fixture` shape (see world.rs test
  `starved_hauler_fails_contract_refunds_escrow_and_accounts_cargo_loss` for the
  pattern) but exhaust fuel while the contract is `Accepted` (hauler bound + escrowed,
  NOT yet loaded — accept off-station so the load never happens). Step until FuelEmpty
  fires. Assert: contract `Failed`, `escrow_micros == 0`, corp treasury refunded
  exactly the reward, hauler released (`contract None`, role `Idle`), and NO cargo sink
  leg (`consumed` unchanged — there was no cargo). A second arm with status
  `CargoLoaded` (load, then drain fuel pre-dispatch via direct field write) asserts the
  same plus `consumed += qty`.
- [ ] **Step 1.2** Run: `cargo test -p jumpgate-core fuel_empty_mid_deadhead` → FAIL
  (contract stays Accepted, escrow locked).
- [ ] **Step 1.3 — fix.** In `resolve_failures`, replace the
  `status != InTransit → continue` filter with: skip only when the status is NOT one of
  `Accepted | CargoLoaded | InTransit`. In `settle_contract_failure`, widen the
  `FailureCause::FuelEmpty` debug_assert arm to those three statuses, with a comment
  citing the issue id. `settle_contract_failure` already handles `cargo == None`
  (refund leg + release; sink leg skipped). No other change.
- [ ] **Step 1.4** Run: the new test passes; `cargo test -p jumpgate-core` all green;
  goldens unchanged (no hash surface touched). The band has zero FuelEmpty events, so
  `scenario_trophic` runs are bit-identical by construction — note this in the commit.
- [ ] **Step 1.5** Commit economy.rs → `fix(economy): FuelEmpty fails Accepted/`
  `CargoLoaded contracts too — releases the deadhead-stranded escrow (jumpgate-2c0c2d92bb)`.
  Then `filigree --actor claude start-work jumpgate-2c0c2d92bb --advance` (bug: walks
  triage→confirmed→fixing) and `filigree --actor claude close jumpgate-2c0c2d92bb`.

## Task 2: hygiene — heterogeneity-metric recalibration (filigree jumpgate-50c6a8a3bd)

**Files:**
- Modify: `crates/jumpgate-core/src/diagnostics.rs` (`risk_is_heterogeneous` and/or
  its consts + synthetics)
- Modify: `python/analysis/sweep_trophic.py` (default control knobset + doc)
- Modify: `docs/superpowers/specs/2026-06-10-pirates-rung1-predation-and-upgrades-design.md`
  (§9 instrument-kill text: the new positive-control recipe)

- [ ] **Step 2.1 — labeled-run evidence.** Release-run the labeled set
  (`cargo run -q -p jumpgate-core --release --example trophic_run -- --seed S
  --ticks 50000 [--set …]`):
  (a) NEW disease control, seeds 7 and 23:
  `--set pirate_max_reach_au=999 --set stay_milli=0 --set upkeep_per_tick=200
  --set grubstake_micros=2000000000` → label TRUE-equalized;
  (b) band baseline seeds 23, 42, 99 → label TRUE-clumped (per console session 2);
  (c) band baseline seed 7 → label TRUE-clumped (the boundary case the current metric
  reads false: robs on 8/36 routes, predation through the final third).
  For each run RECORD: verdict, `risk_heterogeneous`, and from the sweep aggregator the
  per-window-HHI, RUN-AGGREGATE-HHI, routes-robbed (run
  `python3 python/analysis/sweep_trophic.py` with matching knobsets, or compute from
  the JSONL).
- [ ] **Step 2.2 — fit, never nudge.** Choose the smallest metric change that
  classifies ALL labeled runs correctly with margin. The pre-identified candidate (the
  sweep lab already prints it as the sparsity-robust read): replace the mean
  PER-WINDOW HHI with the RUN-AGGREGATE HHI over occupied routes (robs summed across
  windows before squaring), keeping the active-pirate normalization and the
  hot-route-vs-traffic persistence clause. If aggregate-HHI separates the labels,
  implement that; re-derive `HHI_NORM_MIN_MILLI` from the measured margin midpoint
  (document the measured values in a comment). If NO formula/threshold separates the
  labels, STOP: leave the metric unchanged, record the finding in the filigree issue,
  and skip to Step 2.5 (do not force a fit — the metric is a window, not a gate).
- [ ] **Step 2.3 — synthetics stay honest.** Keep the existing 4-corner synthetics
  green (update their hand-built series ONLY as far as the formula change requires,
  preserving each one's labeled intent). ADD one labeled synthetic encoding the seed-7
  shape: robs clumped on a small minority of occupied routes (e.g. 2 hot of 9 occupied)
  at 1–3 robs/window sparsity, persistent hot route → must read
  `risk_heterogeneous == true`.
- [ ] **Step 2.4** Run `cargo test -p jumpgate-core diagnostics` green; re-run the
  labeled set and confirm every label reads correctly; record the final numbers.
- [ ] **Step 2.5 — the new positive control everywhere it is documented.** In
  `sweep_trophic.py`: default control knobset becomes
  `control:pirate_max_reach_au=999,stay_milli=0,upkeep_per_tick=200,grubstake_micros=2000000000`
  and the module doc + `--knobset` help text explain WHY (the hunger gate neutralized
  the old recipe: fed pirates camp → genuinely clumped → correctly Alive; the disease
  injection is now perpetually-hungry roamers). Same correction in the pirates spec §9
  instrument-kill paragraph.
- [ ] **Step 2.6** Commit diagnostics.rs + sweep_trophic.py + the spec edit →
  `lab(pirates): recalibrate risk-heterogeneity against labeled runs; new`
  `hungry-roamer positive control (jumpgate-50c6a8a3bd)`. Then
  `filigree --actor claude start-work jumpgate-50c6a8a3bd` +
  `filigree --actor claude close jumpgate-50c6a8a3bd` (comment the fitted numbers).
- [ ] **Step 2.7 — bake the pre-media inert baseline (bench, NOT committed).**
  `mkdir -p runs/media_baseline` then for seeds 7 and 23:
  `cargo run -q -p jumpgate-core --release --example trophic_run -- --seed S
  --ticks 50000 --jsonl runs/media_baseline/sS.jsonl > runs/media_baseline/sS.out`.
  These are the cross-branch behavior-identity references for Task 9 (the media-off
  world must reproduce them byte-for-byte on JSONL and on every stdout line except the
  trailing MEDIA line added in Task 8).

## Task 3: B0 — `RngStream::Media`

**Files:**
- Modify: `crates/jumpgate-core/src/rng.rs`

- [ ] **Step 3.1 — failing tests** (extend the existing rng test module):
  `media_stream_reproduces_and_is_draw_order_independent` (clone of the Piracy test,
  draining Scenario AND Piracy must not perturb Media) and extend
  `distinct_streams_differ` + `golden_first_draws_are_pinned` with the Media arm.
- [ ] **Step 3.2** Run `cargo test -p jumpgate-core rng` → FAIL (no variant).
- [ ] **Step 3.3 — implement.** APPEND `Media` to `RngStream` (never reorder);
  `const SALT_MEDIA: u64 = 0xBB67_AE85_84CA_A73B;` (frac √3 — continues SALT_PIRACY's
  frac √2 convention; doc-comment that); `media: ChaCha8Rng` field on `RngStreams`,
  seeded `master ^ salt` in `from_master`; arm in `stream()` and `salt()`.
- [ ] **Step 3.4 — capture the golden.** Write the first-draw assert with placeholder
  `0`, run the test, paste the ACTUAL value from the failure message into the assert
  (a capture against rand_chacha 0.10.0, the existing test's discipline — never
  invented). Re-run green.
- [ ] **Step 3.5** `cargo test -p jumpgate-core` all green; goldens unchanged (the
  stream is minted but never drawn — hash-neutral, behaviorally inert). Commit rng.rs →
  `feat(media): RngStream::Media + SALT_MEDIA (frac sqrt3) + pinned first-draw golden`.

## Task 4: A — `MediaCfg` config-shape golden (single-cause commit 1 of 2)

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs` (MediaCfg + RunConfig field + fold + tests)
- Modify: `crates/jumpgate-core/src/scenario.rs` (`apply_knob` arms + test)
- Modify: every `RunConfig { … }` literal (the compiler enumerates them when the field
  lands: config.rs, hash.rs, world.rs, economy.rs, pirate.rs, ingest.rs, diagnostics.rs,
  scenario.rs, tests/physics_sanity.rs, tests/replay_equivalence.rs,
  jumpgate-py/src/env.rs ×2) — each gains `media: MediaCfg::default(),`
- Modify: `crates/jumpgate-core/src/lib.rs` (re-export `MediaCfg`)

- [ ] **Step 4.1 — the struct** (spec §11 verbatim; doc-comment each knob with the
  spec's meaning column):

```rust
/// Media/gossip knobs (media rung cut 1, spec §11). Inert by default: BOTH slot
/// caps 0 ⇒ no buffers, no Media draws, default worlds behavior-identical.
/// media-live = both caps > 0 AND trophic.engage_radius_au > 0 (documented DUAL
/// gating; exactly one cap > 0 is a reset error). DIAGNOSTIC/TUNING knobs, not
/// gates (PDR-0006).
#[derive(Clone, Copy, Debug)]
pub struct MediaCfg {
    pub station_gossip_slots: u32,      // 0 | live start 16
    pub craft_gossip_slots: u32,        // 0 | live start 8
    pub sig_floor_milli: u32,           // 50
    pub sig_divisor_micros: i64,        // 10_000
    pub hop_loss_milli: u32,            // 150
    pub inflation_milli: u32,           // 125
    pub claimed_value_cap_micros: i64,  // 32_000_000
    pub value_ticks_milli: u32,         // 1000
}
impl Default for MediaCfg { /* the inert column above */ }
impl MediaCfg {
    /// Both caps live (the config half of the dual gate).
    pub fn caps_live(&self) -> bool {
        self.station_gossip_slots > 0 && self.craft_gossip_slots > 0
    }
}
```

- [ ] **Step 4.2 — failing tests**: `changing_media_cfg_changes_config_hash` (flip
  `station_gossip_slots` and `sig_divisor_micros` on `sample()`, both must move the
  hash) and an `apply_knob` extension in scenario.rs tests (set
  `station_gossip_slots=16`, `hop_loss_milli=200`, assert applied; unknown still errs).
- [ ] **Step 4.3 — fold.** `pub media: MediaCfg` on `RunConfig` AFTER `shipyard`; the
  exhaustive destructure in `config_hash` forces the fold (compile error until added).
  Fold at the TAIL, declaration order, exhaustive `let MediaCfg { … } = media;`
  destructure (the D10/M6 discipline). Extend the CONFIG_FIELD_ORDER doc comment:
  `25. media: all fields in declaration order`. Add `media: MediaCfg::default(),` at
  every RunConfig literal the compiler names. Add the 8 `apply_knob` arms (bare knob
  names: `station_gossip_slots`, `craft_gossip_slots`, `sig_floor_milli`,
  `sig_divisor_micros`, `hop_loss_milli`, `inflation_milli`,
  `claimed_value_cap_micros`, `value_ticks_milli`). Re-export `MediaCfg` from lib.rs.
- [ ] **Step 4.4 — re-pin the config golden (THE single cause).**
  `cargo test -p jumpgate-core print_golden_config -- --ignored --nocapture`, paste the
  printed value into `GOLDEN_CONFIG_HASH` with a `// RE-PINNED: +MediaCfg (media rung
  cut 1). Was 0x1798_b108_edae_5bb6.` comment.
- [ ] **Step 4.5** `cargo test --workspace` green; `cargo clippy --all-targets -- -D
  warnings` clean. The STATE goldens are untouched — grep them. Commit (config.rs,
  scenario.rs, lib.rs, and every touched literal site) →
  `feat(media): commit A — MediaCfg folded at config tail (GOLDEN_CONFIG_HASH re-pinned, single cause)`.

## Task 5: B — media state + hash words 30–32, format v5 (single-cause commit 2 of 2)

**Files:**
- Create: `crates/jumpgate-core/src/media.rs`; register `pub mod media;` + re-exports
  (`GossipAlert`, `GossipBuffer`, `GossipNode`, `MediaDiag`) in lib.rs
- Modify: `crates/jumpgate-core/src/stores.rs` (CraftStore column), `world.rs`
  (World fields, reset init + validation, `media_live()`), `hash.rs` (words 30–32,
  version 5, goldens, `manual_zero_fold`, moves! tests)

- [ ] **Step 5.1 — media.rs data structures** (spec §2 verbatim; EVIDENCE ONLY — raw
  ticks and claimed integers; significance/staleness computed at the read, never
  stored; no heat/threat/confidence fields, ever):

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GossipAlert {
    pub alert_seq: u32,
    pub route: u32,
    pub pirate_slot: u32,
    pub rob_tick: Tick,
    pub claimed_value_micros: i64,
    pub first_heard: Tick,
    pub hops: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GossipBuffer { pub slots: Vec<Option<GossipAlert>> }
impl GossipBuffer {
    pub fn empty(cap: u32) -> Self { GossipBuffer { slots: vec![None; cap as usize] } }
    pub fn holds(&self, alert_seq: u32) -> bool { /* any Some with that seq */ }
    pub fn occupied(&self) -> u32 { /* count of Some */ }
}
/// Which node heard (the GossipHeard carrier).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GossipNode { Station(StationId), Craft(CraftId) }
/// UNHASHED diagnostics (the engagement_diag pattern — never a behavior input):
/// eviction count + one (tick, station_row) record per dock EDGE while media-live.
#[derive(Default)]
pub struct MediaDiag { pub evictions: u64, pub contacts: Vec<(Tick, u32)> }
```

- [ ] **Step 5.2 — columns + reset.** `CraftStore.gossip: Vec<Option<GossipBuffer>>`
  (added to `empty()` and `push()` as `None`; doc: `Some` only for non-pirate rows on a
  media-live world — pirates information-blind by construction, OD-6).
  `World { pub(crate) station_gossip: Vec<GossipBuffer>, pub(crate) next_alert_seq: u32,
  pub(crate) media_diag: crate::media::MediaDiag, … }`.
  `World::media_live(&self) -> bool` = `config.media.caps_live() &&
  config.trophic.engage_radius_au > 0.0` (pub — tests read it).
  In `reset`, BEFORE minting: the half-on validation —

```rust
if (cfg.media.station_gossip_slots > 0) != (cfg.media.craft_gossip_slots > 0) {
    return Err(ResetError::BadMediaCfg {
        reason: "half-on media: both gossip slot caps must be > 0 together",
    });
}
```

  (APPEND `BadMediaCfg { reason: &'static str }` to `ResetError` + Display arm.)
  Then `let media_live = cfg.media.caps_live() && cfg.trophic.engage_radius_au > 0.0;`;
  in the craft mint loop `ships.gossip.push(if media_live && c.role !=
  CraftRole::Pirate { Some(GossipBuffer::empty(cfg.media.craft_gossip_slots)) } else
  { None });`; after stations mint, `station_gossip = if media_live {
  vec![GossipBuffer::empty(cfg.media.station_gossip_slots); n_stations] } else
  { Vec::new() }` (sized ONCE at reset — the RouteEvidence sizing law);
  `next_alert_seq: 0`, `media_diag: Default::default()` in the World literal.
- [ ] **Step 5.3 — failing hash tests** in hash.rs: extend `state_v4_columns_are_folded`
  with a sibling `state_v5_columns_are_folded` using the `moves!` pattern — craft gossip
  presence (None → Some(empty) moves the hash), each `GossipAlert` field in a held craft
  alert (7 probes), a station-buffer alert (size one station buffer by hand on
  `populated_world` via direct field writes), and `next_alert_seq`. Also a reset test in
  world.rs: `half_on_media_config_is_rejected` (caps 16/0 → `BadMediaCfg`) and
  `media_live_reset_mints_buffers` (caps 16/8 + engage>0 on a 2-station fixture: station
  buffers len 2 cap 16, hauler rows `Some` cap 8, pirate rows `None`; with engage 0.0:
  everything `None`/empty — the dual gate's single-lever cases).
- [ ] **Step 5.4 — implement the fold.** In hash.rs, after `write_route_evidence`:

```rust
// HASH_FIELD_ORDER word 30 (format v5): per-craft gossip buffers, dense row
// order (slot == row), self-delimiting (len; per row tag 0 | 1 + buffer fold).
pub(crate) fn write_craft_gossip(h: &mut FnvHasher, world: &World) { … }
// word 31: station reservoirs (len; per row buffer fold).
pub(crate) fn write_station_gossip(h: &mut FnvHasher, world: &World) { … }
```

  Buffer fold: `slots.len()` then per slot `0` | `1` + (alert_seq, route, pirate_slot,
  rob_tick.0, claimed_value_micros as u64, first_heard.0, hops as u64). Word 32:
  `h.write_u64(world.next_alert_seq as u64)` inline in `state_hash` AND
  `recompute_with_cursors` (both call the two shared helpers too). Extend the
  HASH_FIELD_ORDER module doc (words 30–32, format v5 note). `manual_zero_fold`
  appends: `h.write_u64(1); h.write_u64(0);` (word 30: one craft, gossip None tag),
  `h.write_u64(0);` (word 31: zero stations), `h.write_u64(0);` (word 32: seq 0).
  `HASH_FORMAT_VERSION` 4 → 5 with doc line.
- [ ] **Step 5.5 — re-pin BOTH state goldens (THE single cause).**
  `cargo test -p jumpgate-core print_golden -- --ignored --nocapture`; paste the two
  printed literals into `state_hash_golden_zero_world` and `GOLDEN_ZERO_STATE_HASH`
  with `// RE-PINNED: HASH_FORMAT_VERSION 4->5 (+craft/station gossip, next_alert_seq).`
  comments recording the prior values.
- [ ] **Step 5.6** `cargo test --workspace` green; clippy clean; `GOLDEN_CONFIG_HASH`
  untouched (grep). Nothing behavioral changed: default worlds fold constant words.
  Commit (media.rs, stores.rs, world.rs, hash.rs, lib.rs) →
  `feat(media): commit B — gossip state + hash words 30-32, HASH_FORMAT_VERSION 5 (goldens re-pinned, single cause)`.

## Task 6: C — mechanics: mint, pier deposit, edge exchange, eviction, events

**Files:**
- Modify: `crates/jumpgate-core/src/media.rs` (sig/transfer/insert/exchange + unit tests)
- Modify: `crates/jumpgate-core/src/contract.rs` (two EventKind variants)
- Modify: `crates/jumpgate-core/src/pirate.rs` (`resolve_encounters` mint + signature)
- Modify: `crates/jumpgate-core/src/world.rs` (stage 3b2 internal order + call sites)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (`chronicle_subject` arm +
  `--chronicle-gossip-min-micros`)

- [ ] **Step 6.1 — events first** (hash-neutral; APPEND to `EventKind`):

```rust
/// Truth join at mint (spec §8): truth + claimed captured at the only moment
/// event↔route↔subjects are simultaneously resolvable. No chronicle arm
/// (shadows Robbed).
AlertBorn { alert_seq: u32, route: u32, pirate: CraftId, hauler: CraftId,
            truth_value_micros: i64, claimed_value_micros: i64 },
/// Latched on insertion into a node that does not currently hold the alert;
/// the victim's own hops-0 seed does NOT emit (Robbed tells that story).
/// Self-contained payload — the chronicle prints without joins.
GossipHeard { carrier: crate::media::GossipNode, alert_seq: u32, route: u32,
              pirate_slot: u32, claimed_value_micros: i64, hops: u8, rob_tick: Tick },
```

  NOTE (documented deviation, spec §8 "latched on FIRST insertion"): re-hearing after a
  genuine eviction re-emits — a node that evicted the rumor genuinely no longer holds
  it, and tracking lifetime hearings would be new state. Within retention, membership
  dedupe makes emission once-only. Record this comment on the variant.
- [ ] **Step 6.2 — pure helpers + failing unit tests in media.rs**:

```rust
/// milli (0..=1000), computed at the read — never stored (spec §3).
pub fn sig_milli(claimed_value_micros: i64, m: &MediaCfg) -> u32 {
    ((claimed_value_micros / m.sig_divisor_micros.max(1)) + m.sig_floor_milli as i64)
        .clamp(m.sig_floor_milli as i64, 1000) as u32
}
/// Hop-attenuated transfer P: sig × (1000 − hop_loss)^hops / 1000^hops (integer).
pub fn transfer_p_milli(claimed_value_micros: i64, hops: u8, m: &MediaCfg) -> u32 {
    let keep = 1000u64.saturating_sub(m.hop_loss_milli as u64);
    let mut p = sig_milli(claimed_value_micros, m) as u64;
    for _ in 0..hops { p = p * keep / 1000; }
    p as u32
}
/// Deterministic insert (spec §6): (a) lowest empty slot; (b) lowest-index slot
/// whose item has (now − first_heard) > evidence_window (forgetting has ONE
/// owner, the read window — eviction is purely overflow); (c) overflow: priority
/// = rob_tick + claimed×value_ticks_milli/1e6; evict the argmin (ties → lowest
/// alert_seq) iff the incoming priority exceeds it, else DROP the incoming.
/// Counts one eviction either way in (c). Returns whether `alert` was inserted.
pub fn insert_alert(buf: &mut GossipBuffer, alert: GossipAlert, now: Tick,
                    evidence_window: u64, m: &MediaCfg, evictions: &mut u64) -> bool { … }
```

  Unit tests: `sig_clamps_to_floor_and_1000`; `transfer_p_hand_computed` (sig 600 at
  hop_loss 150 → hops 0/1/2 = 600/510/433 — hand-derive in the test comment, the
  P(escape) analytic anchor); `insert_prefers_empty_then_stale_then_priority`;
  `insert_priority_tie_evicts_lowest_seq`; `evict_replant_cycle_terminates` (a loop of
  inserts at the cap converges, no panic, drops are deterministic).
- [ ] **Step 6.3 — mint at the Robbed settlement** (pirate.rs `resolve_encounters`;
  signature gains `station_gossip: &mut Vec<crate::media::GossipBuffer>`,
  `next_alert_seq: &mut u32`, `media: &MediaCfg`, `media_diag: &mut MediaDiag`; world.rs
  call site updated; the fixture `run()` in pirate.rs tests updated). Inside the robbed
  branch, ALL gated on `media.caps_live()` (engage>0 already holds here):
  1. **BEFORE** `settle_contract_failure`: `let reward = contracts.reward_micros[kidx];`
     (the seed-honesty law, spec §3 — the settlement precedes the ransom computation;
     `Robbed.value_micros` is the wallet-clamped ransom and must NOT seed significance).
  2. After the ransom transfer: `let claimed = reward.saturating_add(ransom);`
     mint `let seq = *next_alert_seq; *next_alert_seq = next_alert_seq.wrapping_add(1);`
     (dense pirate-row order at a fixed stage ⇒ deterministic), build the alert
     `{ alert_seq: seq, route: route as u32, pirate_slot: pirate_id.slot, rob_tick:
     tick, claimed_value_micros: claimed, first_heard: tick, hops: 0 }` (reuse the
     `route` already computed for the evidence-ring write; if the station rows were
     unresolvable, skip the mint — spec §8 degrade), insert into the VICTIM's buffer
     (`ships.gossip[hrow]`; `None` → skip mint entirely). Cover == truth at hop 0. The
     pirate gets no copy. NO GossipHeard for the seed.
  3. **Origin-pier deposit** (spec §4.3): if `station_pos` has a row within
     `ARRIVAL_RADIUS` of `ships.pos[hrow]` (lowest row wins), insert a copy with
     `hops: 1, first_heard: tick` (NO inflation — a firsthand report) into
     `station_gossip[srow]`; on insertion emit
     `GossipHeard { carrier: GossipNode::Station(sid), … }`.
  4. Emit `AlertBorn { alert_seq: seq, route, pirate, hauler, truth_value_micros:
     claimed, claimed_value_micros: claimed }`. `DrivenOff` writes and emits nothing.
- [ ] **Step 6.4 — edge-triggered exchange** (media.rs), called from world.rs stage 3b2
  with the pinned internal order — (i) station_pos resolve (existing); (ii) dock-edge
  detection reading the PRE-refresh `info_tick` → exchange → THEN the info_tick refresh
  loop (existing, unchanged semantics); (iii) `resolve_encounters`. Restructure:

```rust
// (ii) one arrival-radius scan: the LOWEST station row within radius, per craft.
let dock_station: Vec<Option<usize>> = (0..self.ships.ids.len()).map(|crow| {
    (0..station_pos.len()).find(|&s|
        self.ships.pos[crow].sub(station_pos[s]).length() <= crate::autopilot::ARRIVAL_RADIUS)
}).collect();
if self.media_live() {
    crate::media::run_gossip_exchange(&mut self.ships, &mut self.station_gossip,
        &dock_station, &self.config.media, self.config.trophic.evidence_window,
        &mut self.rng, next, &mut self.events, &mut self.media_diag);
}
for crow in 0..self.ships.ids.len() {            // the existing refresh, now after
    if dock_station[crow].is_some() { self.ships.info_tick[crow] = next; }
}
```

  `run_gossip_exchange`, dense craft-row order; per craft: skip if
  `dock_station[crow].is_none()` or `ships.gossip[crow].is_none()` (the role filter —
  pirates carry `None`, OD-6); **edge predicate** (the load-bearing fix, spec §5):
  `ships.info_tick[crow] != Tick(next.0 - 1)` — within radius now AND not docked last
  tick. Zero new state. (Doc notes: a craft docked since reset first edges only after
  its first departure — vacuous, buffers are empty at reset; radius oscillation
  re-fires — harmless, dedupe is idempotent.) On an edge: push
  `media_diag.contacts.push((next, srow as u32))`; then **pinned direction order** —
  ship→station uploads (sender slot index order), then station→ship downloads (sender
  slot index order; downloads run over the post-upload station content — dedupe makes
  self-download a no-draw). Per candidate `Some(alert)`: (1) **dedupe first** —
  receiver `holds(alert_seq)` ⇒ skip, NO draw (draw count = pure function of hashed
  membership; the Class-3 cursor stays transitively pinned); (2) ONE Media draw:
  `let u = (rng.stream(RngStream::Media).next_u64() % 1000) as u32;` transfer iff
  `u < transfer_p_milli(alert.claimed_value_micros, alert.hops, media)`; (3) on
  transfer build the receiver copy: `hops = hops.saturating_add(1)`,
  `first_heard = next`, and iff the RESULTING hops ≥ 2 the deterministic inflation
  `claimed = min(claimed.saturating_mul(1000 + inflation_milli as i64) / 1000, cap)`
  (sender untouched; first-heard sticks; re-hearing never re-inflates — dedupe kills
  the evict-replant ratchet within retention); insert via `insert_alert`; if inserted,
  emit `GossipHeard` with the receiver's carrier node and the receiver copy's fields.
- [ ] **Step 6.5 — failing → green behavior tests** (media.rs module tests build small
  fixtures by hand — the pirate.rs `fix()` pattern; world-level ones reuse
  `pirate_world_cfg` with media caps set via a local `media_live_cfg()` helper):
  - `parked_n_ticks_produces_exactly_one_exchange`: seed a station buffer, park a
    hauler in radius 5 ticks (info_tick refreshed each tick) → exactly ONE contact
    record and at most one GossipHeard per alert. THE pinned unit test (spec §5).
  - `dedupe_consumes_no_draw`: receiver already holds the alert; run an edge; then
    `assert_eq!(rng.stream(Media).next_u64(), RngStreams::from_master(M).stream(Media)
    .next_u64())` — zero draws consumed (the stream-cursor equivalence trick).
  - `direction_order_is_ship_then_station`: station holds X, craft holds Y, cap 2;
    brute-force in-test a master whose FIRST Media draw fails a p=500 gate and SECOND
    passes; after one edge assert the upload (Y→station) failed and the download
    (X→craft) succeeded — observable only if uploads draw first.
  - `witness_seed_is_truth_at_hops_zero`: robbed settlement (the `fix()` fixture with
    media caps live) → victim's buffer holds one alert, `claimed == reward + ransom`,
    `hops == 0`, `first_heard == rob tick`; NO GossipHeard emitted; AlertBorn emitted
    with matching seq/route/truth.
  - `origin_pier_deposit_lands_same_tick`: the rob-on-load world test (clone
    `rob_on_load_is_legal_at_origin` with media live) → origin station's reservoir
    holds the alert at hops 1, one `GossipHeard { carrier: Station(..) }` at the rob
    tick.
  - `pirates_are_information_blind`: media-live world; pirate row's `gossip` is `None`
    after reset; after a docked-pirate edge window no GossipHeard has a
    `GossipNode::Craft` carrier equal to the pirate id; (the read-side fence is
    compile-level: `relocate_lurk_target`'s signature — cite, no test needed).
  - `inflation_only_on_retellings_and_capped`: hand-run two transfers (hops 0→1: no
    inflation; 1→2: ×1.125 floor-div; at the cap: clamped).
  - existing `dock_is_sanctuary_at_destination` + the full pirate.rs suite still green
    (ordering untouched).
- [ ] **Step 6.6 — chronicle.** `chronicle_subject`: add
  `EventKind::GossipHeard { carrier: GossipNode::Craft(c), .. } => Some(c)` and
  `EventKind::GossipHeard { .. } | EventKind::AlertBorn { .. } => None` arms (station
  hearings feed the gossip log/panels; a station-thread chronicle is a named deferral).
  New flag `--chronicle-gossip-min-micros N` (default 0 = print all): the printer skips
  `GossipHeard` lines whose `claimed_value_micros < N`. Run a short live chronicle
  manually and eyeball one staggered news front (record the command + observation in
  the task log).
- [ ] **Step 6.7** Full check: `cargo test --workspace` green, clippy clean, ALL FOUR
  golden literals unchanged (grep — this commit is behavioral-off-by-default; media-off
  worlds take the `media_live() == false` early-outs and consume zero Media draws).
  Commit (media.rs, contract.rs, pirate.rs, world.rs, trophic_run.rs) →
  `feat(media): commit C — mint + pier deposit + edge-triggered exchange + eviction + AlertBorn/GossipHeard`.

## Task 7: D — the `route_evidence` read swap

**Files:**
- Modify: `crates/jumpgate-core/src/world.rs` (the accessor body ONLY — signature unchanged)
- Test: world.rs module tests + one trace-identity integration test

- [ ] **Step 7.1 — failing tests**:
  - `route_evidence_media_path_counts_own_recent_route_matches`: media-live world; hand
    sit two alerts in hauler A's buffer (route 3 fresh, route 3 stale by
    `> evidence_window`, route 5 fresh) → `route_evidence(A, 3) == 1`,
    `route_evidence(A, 5) == 1`, `route_evidence(A, 9) == 0`; hauler B (empty buffer)
    reads 0 on every route — per-reader CONTENT, not just per-reader staleness.
  - `route_evidence_media_off_is_byte_identical_legacy`: media-off world; the accessor
    returns exactly `count_recent(route, info_tick[reader], evidence_window)` on a
    hand-seeded ring (the legacy parity pin).
  - `per_reader_forgetting_clock`: same alert copied into A (first_heard t0) and B
    (first_heard t0 + 3000); advance the world tick past t0 + window but not past
    B's horizon → A reads 0, B reads 1 (the staggered-return mechanism, spec §7).
- [ ] **Step 7.2 — implement** (raw count; valence stays in the consumer — the 900
  clamp at the ASSIGN site is untouched):

```rust
pub fn route_evidence(&self, reader: CraftId, route: usize) -> u32 {
    let Some(crow) = self.ship_index(reader) else { return 0 };
    if self.media_live() {
        let Some(buf) = self.ships.gossip[crow].as_ref() else { return 0 };
        let w = self.config.trophic.evidence_window;
        buf.slots.iter().flatten()
            .filter(|a| a.route as usize == route
                && self.tick.0.saturating_sub(a.first_heard.0) <= w)
            .count() as u32
    } else {
        self.route_evidence.count_recent(route, self.ships.info_tick[crow],
            self.config.trophic.evidence_window)
    }
}
```

  Doc-comment update: the propagation model now lives BEHIND the unchanged signature
  (the spec-§7 promise kept); `info_tick` keeps refreshing (it is the dock detector);
  the legacy ring keeps being written as the media-off fallback (retirement = cut 2,
  OD-2).
- [ ] **Step 7.3 — the deaf control** (instrument-kill discipline, spec §9):
  integration test `deaf_control_behavioral_trace_identity` in world.rs tests (or
  `tests/`): `scenario_trophic(7)` for 6_000 ticks, arm 1 media-live
  (`station_gossip_slots=16, craft_gossip_slots=8`) + `hauler_belief_scoring=false`,
  arm 2 media-OFF + `hauler_belief_scoring=false`. Assert element-identical per-window
  `robs`, `laden_trips`, `per_route_accepts`, `per_craft_credits` (sample via
  `diagnostics::sample_window` every 2000 ticks). NOT state_hash (gossip state
  legitimately differs). Media is live but unread ⇒ behavior must be untouched.
- [ ] **Step 7.4** Full check (tests, clippy, goldens grep — unchanged). Commit →
  `feat(media): commit D — route_evidence swapped behind its signature (raw count, per-reader clock, legacy fallback)`.

## Task 8: E — instruments: lab fields, media classifier, MEDIA line, panels, gossip log

**Files:**
- Modify: `crates/jumpgate-core/src/diagnostics.rs` (TrophicSample fields +
  `sample_window` + `media_classify`/`MediaReading` + synthetics; promote `route_of` to
  `pub fn route_of(world, contract)`)
- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (JSONL keys, MEDIA line,
  `--gossip-log`)
- Modify: `python/analysis/sweep_trophic.py` (MEDIA_RE + media panels + VoI line —
  the SAME commit as the MEDIA line: the lockstep rule)
- Create: `python/analysis/media_log.py` (gossip-log panels)

- [ ] **Step 8.1 — TrophicSample additive fields** (half-open `(window_start, tick]`;
  JSONL keys additive; `Verdict`/`classify()`/the RESULT line byte-untouched):
  `gossip_born: u32` (AlertBorn in window), `gossip_first_heard: u32` (Craft-carrier
  GossipHeard in window — the propagation signal; pier deposits are draw-free so
  station hearings deliberately do not count), `gossip_born_cum: u32` +
  `gossip_escaped_cum: u32` (run-cumulative: total alerts born / distinct alert_seqs
  with ≥ 1 Craft-carrier hearing, read from `recent_events(Tick(0))` — pure read),
  `alerts_carried: u32` (Σ occupied craft-buffer slots at the sample point),
  `stations_with_news: u32` + `per_station_alerts: Vec<u32>` (occupied reservoir slots
  — the news-desert map), `per_station_contacts: Vec<u32>` (dock EDGES per window from
  `media_diag.contacts` — the P(escape) denominator), `heard_lag_ticks: Vec<u32>` +
  `heard_hops: Vec<u32>` (per Craft-carrier first-hearing in the window: lag =
  `e.tick − rob_tick`), `alerts_evicted_cum: u64` (the `media_diag.evictions`
  snapshot). Wire all into `sample_window` + `sample_json`.
- [ ] **Step 8.2 — `media_classify` + synthetics (failing first).** A SEPARATE pure
  classifier (the existing `classify` is byte-untouched):

```rust
/// DIAGNOSTIC WINDOWS, NOT GATES (PDR-0006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaReading { NoMedia, NewsDesert, StaleEcho, CommonKnowledge, Localized }
pub fn media_classify(samples: &[TrophicSample]) -> MediaReading
```

  Rules (operationalized; deviations from spec §9 wording are corrections recorded in
  doc comments): born total == 0 → `NoMedia`. Zero Craft-carrier hearings all run →
  `NewsDesert` (spec said "hops≥1 hearings"; pier deposits are draw-free hops-1, so the
  meaningful propagation zero is CRAFT hearings — M-DEAD must read NewsDesert).
  `StaleEcho` = hearings present in the first half, ZERO across the second half, while
  robs CONTINUE in the second half and mean `alerts_carried` over the second half ≥ 1
  (spec said "births first half zero second" — births ≡ robs by construction, so the
  dead coupling is HEARINGS dying under continuing robs: the network went deaf, stale
  copies echo in buffers; the PermanentPeace analogue). `CommonKnowledge` =
  `escape_milli ≥ 950` (escape = 1000 × escaped_cum / born_cum at the last sample,
  `escaped_milli = 0` sentinel at born 0) AND the last window's `stations_with_news ==
  per_station_alerts.len() > 0` — the self-averaging alarm. Else `Localized`.
  Labeled synthetics, one per reading, PLUS the quiet-but-alive StaleEcho trap (the
  seed-7 rule): hearings drop to zero in the second half but robs ALSO stop → must read
  `Localized`, NOT StaleEcho (an instrument that cries wolf over a peaceful world is
  lying).
- [ ] **Step 8.3 — MEDIA line + MEDIA_RE (one commit, lockstep).** trophic_run prints,
  after the RESULT line:
  `MEDIA seed={} born={} escaped_milli={} median_lag={} p90_lag={} reading={:?}`
  (born = Σ gossip_born; lags pooled over all windows' `heard_lag_ticks`, integer
  median/p90, 0 sentinel when empty; reading = `media_classify(&samples)`).
  sweep_trophic.py gains `MEDIA_RE` matching exactly that shape, parses it per run, and
  prints per knobset: the reading distribution, a pooled lag histogram (knowledge
  front), the news-geography ratio (max/min station of run-summed `per_station_alerts`
  over stations with any traffic), escaped_milli per run, and — when both a media-off
  and a media-on knobset are present in the same invocation — a **value-of-information
  line**: median final hauler-row credits per arm, REPORTED NEVER GATED (a ~0 reading
  is a finding that points the next bet at world prices — owner's call).
- [ ] **Step 8.4 — `--gossip-log PATH`.** trophic_run writes, post-run from the
  retained event stream, one JSONL line per media-relevant event:
  AlertBorn → `{"e":"born","tick":..,"alert":..,"route":..,"pirate":..,"hauler":..,"truth":..,"claimed":..}`;
  GossipHeard → `{"e":"heard","tick":..,"alert":..,"carrier":"s<row>"|"c<slot>","route":..,"hops":..,"claimed":..,"rob_tick":..}`;
  Robbed → `{"e":"rob","tick":..,"route":..}` and ContractAccepted →
  `{"e":"accept","tick":..,"route":..,"hauler":..}` (route via the now-public
  `diagnostics::route_of`). `python/analysis/media_log.py` reads one gossip log and
  prints: per-alert reach (nodes heard) by claimed-value quartile (the bimodal-reach
  panel), the saturation window (fraction of alerts reaching > 800‰ of craft —
  pre-registered expected band at defaults: LOW, single-digit ‰; record the actual),
  the avoidance-lag panel (for each hot route: first hearing tick vs the next
  ContractAccepted tick on that route), and the P(escape) analytic check
  (observed escape vs `1 − (1−p̂)^k̄`, p̂ from hops-1 transfer P of the mean claimed
  value, k̄ = mean dock edges per alert lifetime — REPORTED).
- [ ] **Step 8.5 — the inert control test.** `media_default_is_inert` (world.rs or
  media.rs tests): `scenario_trophic(7)` UNMODIFIED (media caps default 0) stepped
  3_000 ticks → zero `AlertBorn`/`GossipHeard` events, every `gossip` column `None`,
  `station_gossip` empty, `next_alert_seq == 0`, and the Media stream cursor untouched
  (draw-one-and-compare-to-fresh trick). Plus the single-lever case: caps 16/8 but
  `engage_radius_au=0.0` → same assertions (the dual gate).
- [ ] **Step 8.6** Full check (tests, clippy, goldens grep, pytest — gym untouched but
  run it: 21 green). Commit (diagnostics.rs, trophic_run.rs, sweep_trophic.py,
  media_log.py) →
  `feat(media): commit E — media lab bench (TrophicSample fields, media_classify, MEDIA line + MEDIA_RE lockstep, gossip log, panels)`.

## Task 9: verification + the bench session (no code commits expected)

- [ ] **Step 9.1 — the full battery, re-verified by the main loop, recorded:**
  `cargo test --workspace` (record the count; was 295 + new), `cargo clippy
  --all-targets -- -D warnings`, `python3 -m pytest python/ -q` (21), grep all four
  golden literals and `HASH_FORMAT_VERSION = 5`.
- [ ] **Step 9.2 — cross-branch inert digest** (the digest-tests law): re-run the Task
  2.7 baseline commands at HEAD; `diff runs/media_baseline/sS.jsonl` against the fresh
  JSONL → byte-identical (note: identical because the new media JSONL keys emit only
  when… they DON'T — keys are unconditional, so instead compare the PRE-EXISTING keys:
  `python3 - <<EOF` filter both files to the pre-media key set and diff EOF) and every
  pre-existing stdout line (RESULT, windows, laden_trips) identical; the only new line
  is MEDIA with `reading=NoMedia born=0`. Any divergence = a determinism break: STOP
  and bisect, do not rationalize.
- [ ] **Step 9.3 — media-live replay determinism:**
  `cargo run -q -p jumpgate-core --release --example trophic_run -- --seed 7 --ticks
  50000 --set station_gossip_slots=16 --set craft_gossip_slots=8 --replay-check`
  → `replay-check OK` (and once with `--chronicle --chronicle-gossip-min-micros
  2000000` to eyeball volume; record a news-front excerpt).
- [ ] **Step 9.4 — instrument-kill controls, recorded readings (conditional on born>0):**
  M-DEAD `--set station_gossip_slots=16 --set craft_gossip_slots=8 --set
  sig_floor_milli=0 --set sig_divisor_micros=1000000000000` → expect `NewsDesert`;
  M-ORACLE `… --set sig_floor_milli=1000 --set hop_loss_milli=0 --set
  claimed_value_cap_micros=2000000000 --set evidence_window=100000` → expect
  `CommonKnowledge`. If a control reads wrong, FIX THE INSTRUMENT before any tuning.
- [ ] **Step 9.5 — the first band sweep (windows, never gates):**
  `python3 python/analysis/sweep_trophic.py --seeds 7 23 42 99 --ticks 50000
  --knobset baseline --knobset "media_on:station_gossip_slots=16,craft_gossip_slots=8"
  --knobset "media_stakes:station_gossip_slots=16,craft_gossip_slots=8,ransom_cap_micros=12000000"`
  → record per knobset: verdict×reading distributions side by side (the paired
  ecosystem reading — does boom-bust survive the read swap), the VoI line, lag/news
  panels, saturation. Run one `--gossip-log` run through `media_log.py`. These are the
  evidence the OWNER reads at the next console session (OD-1 re-bake is the owner's
  call there); REPORT everything, gate nothing.
- [ ] **Step 9.6** Update filigree (comment on jumpgate-aec6e7bc14 with the commit
  ledger + readings) and memory.

---

## Self-review notes (writing-plans checklist applied)

- Spec coverage: §2→T5, §3→T6.2/6.3, §4→T6.3, §5→T6.4, §6→T6.2, §7→T7, §8→T6.1/6.6,
  §9→T8, §10→T9.5 (sweep axis; defaults untouched per OD-1), §11→T4, §12 order
  preserved (0=T0–2, B0=T3, A=T4, B=T5, C=T6, D=T7, E=T8), §13 test list mapped in
  T6.5/T7/T8.5 (M-DEAD/M-ORACLE/MEDIA_RE smoke = T9 bench runs by design — 50k-tick
  debug tests would be a CI tax; the synthetics carry the committed coverage), §14
  dispositions are inlined as law at their sites, §16 resolutions honored throughout.
- Two spec-wording corrections are made explicitly and recorded in code comments:
  NewsDesert/StaleEcho operationalized on CRAFT hearings (pier deposits are draw-free,
  births ≡ robs); GossipHeard re-emission after genuine eviction (latch-without-state).
- Type consistency: `GossipBuffer.slots: Vec<Option<GossipAlert>>` everywhere;
  `MediaCfg` field names identical in struct/fold/knobs; `GossipNode` lives in
  media.rs and is imported by contract.rs (the `stores::UpgradeKind` precedent).
