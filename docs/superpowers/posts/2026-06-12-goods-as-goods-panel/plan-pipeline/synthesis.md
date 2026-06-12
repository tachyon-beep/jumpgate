# Synthesis — Goods as Goods Rung A implementation plan

**Plan:** `/tmp/gag-plan-a/assembled-plan.md` (10 888 lines, phases A0–A6)
**Spec:** `docs/superpowers/specs/2026-06-12-goods-as-goods-design.md` (D1–D8, OD-1..7 — settled)
**Cut:** `docs/superpowers/posts/2026-06-12-goods-as-goods-panel/synthesis-recommended-cut.md` (authoritative; 17 CRITICAL dispositions)
**Reviews synthesized:** reality, quality, architecture, systems
**Codebase verified at:** HEAD b446095 (`jumpgate-v1-design`)
**Date:** 2026-06-13

---

## 1. Verdict: **FIX-THEN-SHIP**

The architecture is sound and the cut is faithfully followed (intent/settle split, transfer-vs-sink, single-cause golden discipline, structural-inert defaults, exhaustive-match policy reversal all correct). There are **no design defects and no spec/cut violations** — every blocking issue is a plan-*assembly* defect: the same struct, column, event, and golden re-pin are each authored two or three times across phase drafts that were stitched together, with mutually incompatible types. As written the plan **will not compile** at several phase boundaries. These are mechanical de-duplications, not redesigns. Fix the six CRITICALs (all confirmed against live code), apply the MAJORs, then ship.

The plan was assembled from per-phase drafts (`draft-a0..a6.md`); the duplications are seam artifacts where a later phase re-declared scaffolding an earlier phase already landed. One reviewer (systems) reviewed a stale `draft-a5.md` whose `run_trade_policies` lacked a sell branch — the assembled plan does **not** have that defect (see rejected findings).

---

## 2. CRITICAL — must fix before execution (6, deduplicated)

### C1. `ArbitrageCfg` is defined THREE times with incompatible fields; THREE GOLDEN_CONFIG_HASH re-pins violate the cut's "ONE per rung"
*Caught by: Quality CRITICAL-2, Architecture MINOR-4 (partial). Confirmed: plan lines 3964 (A3.2), 5509 (A4.2), 7524 (A5.2); re-pins at 4068, 5563, 7642. Cut §1.1-A3: "ONE GOLDEN_CONFIG_HASH re-pin."*

Three structurally different definitions of the same struct in the same crate:
- **A3.2** (`3964`): `{scan_interval, wage_flat_micros, wage_share_milli, max_posts_per_scan: u32}` — no transport table.
- **A4.2** (`5509`): `{scan_interval, transport_micros: Vec<i64>, wage_share_milli, qty_ladder: Vec<u32>, max_posts_per_scan: usize}`.
- **A5.2** (`7524`): `{scan_interval, wage_flat_micros, wage_share_milli, transport_micros: Vec<Vec<i64>>, qty_ladder, max_posts_per_scan: usize, arb_premium_micros: Vec<i64>}`.

**Edit:** In **A3.2** define the ONE complete struct — the A5.2 field set is the superset and matches the cut §1.2 ("ArbitrageCfg {scan_interval, wage_flat_micros, wage_per_route from the transport table, wage_share_milli, qty ladder, max_posts_per_scan}" plus per-corp premium): `{scan_interval: u32, wage_flat_micros: i64, wage_share_milli: u32, transport_micros: Vec<Vec<i64>>, qty_ladder: Vec<u32>, max_posts_per_scan: usize, arb_premium_micros: Vec<i64>}`, all-inert default (`scan_interval: 0`, empty Vecs). The transport table is folded in the **A3.2** config commit so there is exactly one re-pin. **Delete** the `ArbitrageCfg` re-definition and the GOLDEN_CONFIG_HASH re-pin step from **A4.2** (`5509`, `5563`) and from **A5.2** (`7524`, `7642`); both must *reference* the A3.2 struct, not redefine it. Per cut, `arb_premium_micros` lives on the config (the cut puts it as `CorporationInit.arb_premium_micros`; keep it wherever A3.2 folds it, but fold it once).

### C2. `EventKind::Trade` is deleted twice (A0.3 and A3.4); A3.4's test resurrects deleted `Resource::Ore`
*Caught by: Quality CRITICAL-3, Architecture CRITICAL-1. Confirmed: A0.3 Step 3 (`~601`) removes the variant; A3.4 Step 2 (`~4290`) deletes it again and imports `crate::economy::{Good, Resource}` though `Resource` is gone post-A1b.*

**Edit:** In **A3.4**, remove the `EventKind::Trade` deletion entirely (it lands in A0.3). A3.4's job is only: add `TradeBought`/`TradeSold`, add their exhaustive-match arms, and extend `economy_event_kinds_are_copy_and_partial_eq` to witness `TradeBought` (not replace a `Trade` assertion — that test body already changed in A0.3). Change A3.4's test import to `use crate::economy::Good;` and any `Resource::Ore` to `Good::ORE`. Update the A3.4 task title/preamble to drop the "delete dead EventKind::Trade" framing.

### C3. `pending_trade_buy/sell` columns added twice (A2.3 and A3.3) with incompatible payload types
*Caught by: Quality MAJOR-2, Architecture CRITICAL-3 (also Quality MAJOR-1-adjacent). Confirmed: A2.3 (`3266`) uses `Vec<Option<TradeBuyIntent>>`/`Vec<Option<TradeSellIntent>>` named structs; A3.3 (`4172`) uses `Vec<Option<(Good,u32,StationId)>>`/`Vec<Option<StationId>>`; A3.5's implementation (`4546`) writes `Some(sid: StationId)` — i.e. the A3.3 form is the one actually consumed. Both tasks modify the same files (stores.rs/world.rs/hash.rs) and neither marks the other superseded.*

**Edit:** **Delete Task A2.3 in its entirety** (struct defs, column adds, debug_asserts) and remove it from the A2 commit summary (`3645`). Keep **A3.3** as the sole column-addition task (the `StationId`-tuple payload matches A3.5/A3.6 and the existing pending-column idiom). The debug_assert infrastructure A2.3 carried is identical to A3.3's, so nothing is lost. Confirm A3.5/A3.6 destructure against the A3.3 tuple types. (Note: removing A2.3 also dissolves Architecture MAJOR-1's vacuous-TDD-guard concern.)

### C4. `assert_resource_identity` hold extension is authored three times with two signatures
*Caught by: Quality CRITICAL-1. Confirmed: A1b Step 10 (`2461`) migrates the signature to `&[i64]`; A2.2 Step 2 (`3091`) re-presents it as `&[i64]` (good); A3.6 Step 4 (`5132`/`5141`) re-extends it AGAIN using `&[i64; crate::economy::N_RESOURCES]` — a signature A1b already removed. Cut §1.1-A2: the hold sum is added "in this commit" (A2).*

**Edit:** Keep the hold-sum extension in **A2.2** only, using the A1b-migrated Vec signature (`fn assert_resource_identity(world: &World, initial: &[i64])`, loop `for r in 0..initial.len()`). **Delete A3.6 Step 4** (`5132`–`5147`) — the hold sum already participates after A2.2; A3.6 must not re-touch this function or reference `N_RESOURCES`. Where A3.6 currently calls `assert_resource_identity_with_hold` (`4787`, `4856`), rename those calls to `assert_resource_identity` (the A2.2 version already includes hold).

### C5. A0.2 exhaustive `gossip_log_event_json` omits `EventKind::Trade` from the None arm → compile error
*Caught by: Reality CRITICAL-1. Confirmed: plan None-arm group (lines 432–446) lists 15 variants ending `UpgradePurchased { .. } => None`, with NO `Trade` arm, while the plan's own note (`451`) says Trade "is handled by the exhaustive None arm group above." `Trade` is still in the enum at A0.2 (removed in A0.3), so the match is non-exhaustive.*

**Edit:** In **A0.2** Step 2, add `| EventKind::Trade { .. }` to the None-arm group (e.g. after `UpgradePurchased { .. }`). The plan's note already says to remove it again in A0.3 when the variant is deleted — keep that instruction.

### C6. `GoodSpec.name` declared as both `&'static str` and `String` across the three GoodsCfg definitions
*Caught by: Architecture CRITICAL-2 + MINOR-1, Quality MINOR-3. Confirmed: A1.2 (`2147`) `&'static str`; A3.2 (`3925`) `String`; A5.2 (`7482`) `&'static str`. `&'static str` is `Copy`/no-Clone-cost; `String` is not — the derive set and all `GoodSpec{...}` literal sites diverge (`"Ore"` vs `"Ore".into()`).*

**Edit:** Pick `String` (the cut calls name human-readable display, "NEVER folded," and it will eventually come from config files — Architecture's reasoning). Define `GoodSpec { name: String, unit_mass_milli: u32 }` once in **A1.2** (`2145`); use `.to_string()`/`.into()` at every literal site (the `GoodsCfg::default()` impl and `scenario_bazaar`). **Remove the GoodSpec/GoodsCfg re-definitions from A3.2 (`3923`) and A5.2 (`7480`)** — they reference the A1.2 type. Harmonize the `RunConfig` field name to `goods` everywhere (A1b adds `goods`; A3.2 must fold `goods`, not a second `goods_cfg` field — Quality MINOR-3).

---

## 3. MAJOR — should fix (5, deduplicated)

### M1. A3.4 / A3.5 chronicle + gossip-log match instructions assume a `_ => None` wildcard that A0.2/A0.3 already removed
*Caught by: Architecture MAJOR-2, Systems m-1 (same root). A3.4 Step 3 (`4315`) says "REMOVE the `_ => None` arm"; A0.3 already made both matches exhaustive.*

**Edit:** Reword A3.4 Step 3/4 (and the equivalent in A5.1 Step 11) to: "The matches are already exhaustive (A0.3 removed the wildcard). ADD arms for `TradeBought` (subject = buying craft) and `TradeSold` (subject = selling craft); a missing arm is a compile error." No wildcard-removal instruction.

### M2. A3.1 `update_prices` test fixture calls non-existent constructors `StationStore::empty_with_goods(n)` and `station.push_goods(...)`
*Caught by: Quality MAJOR-1. Confirmed against economy.rs — no such methods; the station `push` takes a `BodyId` + Vec stock/price after A1. Plan lines 3728–3730.*

**Edit:** Replace `StationStore::empty_with_goods(n)` with the real empty constructor and a `push(BodyId{slot:0,generation:0}, vec![0i64; n], vec![0i64; n])` call (the A1-migrated Vec signature). Fix imports so `GoodsCfg`/`GoodSpec` come from `crate::config`, not `crate::economy`. Verify the third `update_prices` argument (`&goods_cfg`) matches the A3.1 Step-2 signature.

### M3. WA3/WA5 joint-read is not encoded as a co-reported column anywhere in the plan's lab tasks
*Caught by: Quality CRITICAL-4, Systems M-3 (instrument half). Cut §1.2 ("Day-0 wallets are 0 so WA3 opens at share 0 — that IS the story") + Part 3 ("carry the WA5 caveats... the WA3 joint read") + spec §6b make this a mandatory co-report, NOT a gate (PDR-0006 respected). The A6 lab tasks describe the TradeBought→TradeSold join (Part 3 "Own-trade traffic honesty") but do not bind it to the WA5 verdict row.*

**Edit:** In **A6**, add an `own_trade_share` column (script-side `TradeBought`→`TradeSold` per-craft join over the read window) and require every WA5 verdict-mix row in the sweep output to carry it alongside the verdict, with a panel-script test (`test_wa5_output_has_wa3_column`). This is a recorded co-read, not a window gate — windows remain recorded-never-gated per the cut. *(Downgraded from Quality's CRITICAL: it does not block compilation or any A0–A5 commit; it is a science-readout completeness gap in the final phase. Still must land before the rung-A exit console session.)*

### M4. `run_scripted_dispatch` signature gains `&mut CraftStore` in A4.5 but direct test callers and the world.rs call site are not inventoried
*Caught by: Architecture MAJOR-3. Cut §1.2 puts the withdrawal sweep (incl. Accepted-never-loaded release) in `run_scripted_dispatch`, which today lacks `ships: &mut CraftStore`.*

**Edit:** In **A4.5** Files section, explicitly list `world.rs` (stage-1b2 call site) and require a grep for direct `run_scripted_dispatch(` callers in `economy.rs`/`world.rs` test modules; co-land the signature change and all call-site updates in the same commit. (Verification note: the cut already authorizes this signature change — "the withdrawal sweep belongs in the same `run_scripted_dispatch` function" — so it is sound; the gap is only the un-inventoried callers.)

### M5. ASSIGN empty-hold gate (L3-M3) has no test in any task
*Caught by: Quality MAJOR-4. Cut §1.2 names it: "ASSIGN gates package claims on an empty hold (L3-M3)." Mentioned in A3.5 prose, never tested or implemented as a step.*

**Edit:** In **A3.5** (or A5.1 where ASSIGN is touched), add a failing test `scripted_assign_skips_craft_with_nonempty_hold` and the implementation step adding the hold-nonempty guard to the ASSIGN arm of `run_scripted_dispatch`. Keeps the rung-B prey taxonomy exact (no craft carrying own-goods + contract cargo).

---

## 4. Findings REJECTED (with one-line reasons)

- **Systems C-1 (own-trade sell branch entirely missing from `run_trade_policies`)** — REJECTED: read stale `draft-a5.md`; the assembled plan has the SELL path at A3.5 Step 2 (lines 4544–4550) and `resolve_trade_sells` at A3.6. The duplicated-column issue is real (→ C3) but "no sell branch" is false.
- **Systems M-1 (corp rotation vacuous, n_corps=1)** — REJECTED as a defect: the cut already states the Exchange is the single corp ("Exchange == Port == Yard"); modulo-1 rotation being identity is by-design for rung A, and the cut's anti-self-averaging guarantee rests on clumped topology + the HHI panel (L1-C2 disposition), not rotation variance in bazaar. At most a one-line doc note; not a plan fix.
- **Systems M-2 (Exchange battery 5.4B uncalibrated)** — REJECTED as a plan defect: the cut §1.2 explicitly ships "option (a) as a sized battery... seed from a calibration run's measured drain window, and print the Exchange-drain as an anchored read." The plan carries the 5.4B as the panel estimate and the BAZAAR/anchored-read instrument exists; calibration is a runtime science step, not a planning error. PDR-0006: no metric gate.
- **Systems M-3 (monoculture / `trade_reserve_micros=0`)** — PARTIALLY REJECTED: the proposed soft-gate WARNING at `own_trade_share>0.8` is a metric gate (PDR-0006 forbids). The legitimate instrument half (own_trade_share co-read) is absorbed into M3 above. `trade_reserve_micros=0` is the cut's deliberate choice ("Day-0 wallets are 0... that IS the story; no initial-wallet knob"), so a non-zero default contradicts the cut — rejected.
- **Systems m-2 (`endpoint_station_rows` all-false in bazaar → CommonKnowledge N/A)** — REJECTED as new: the cut §1.3 already mandates the fix ("`media_classify` endpoint fallback: producer-derived rows when `cfg.contracts` is empty (G4#14)"). Verify the plan implements it, but it is not an unaddressed gap.
- **Reality MAJOR-1 (FRONTIER_TRAJECTORY_GOLDEN in wrong file)** — REJECTED: self-retracted in the review; constant is at `scenario.rs:1118` and the plan names it correctly.
- **Reality MAJOR-1-revised (frontier ContractInit at scenario.rs:156-161)** — KEPT as MINOR not MAJOR: `scenario_bazaar` has zero ContractInit rows (cut §1.3), so the frontier ContractInit edit only matters for the A1 Resource→Good Vec migration of the *existing* frontier scenario; correct line is ~470-481. Worth a one-line correction in A1.1 Step 10 but low-impact (compiler/grep finds it).
- **Reality MINOR-1 / MINOR-2 (off-by-one line refs: ContractOffered 511-514, grubstake 724-725)** — NOTED, not actioned: cosmetic line-number drift, no behavioral impact.
- **Quality MINOR-2 (A4.3 REPOST proof is documentation-not-TDD)** — KEPT as a small follow-up, not blocking: the cut authorizes the early-return prelude "proven by within-build digest"; add the explicit before/after `sha256sum` step to A4.3 so the commit message's claim is actually executed. (Minor.)
- **Quality MINOR-1 (A3.7 module path `python/jumpgate/` vs `python/analysis/`)** — KEPT as MINOR: correct the path to `python/analysis/sweep_trophic.py` and the versioned-fixture pattern (`V6_STDOUT = V5_STDOUT + ...`); trivial.

---

## 5. SME sections

### Confidence Assessment — **High**
All six CRITICALs were verified by direct reads of both the plan and the live codebase (struct defs, signatures, enum arms, golden-hash re-pin sites). The two reviewer disagreements (systems C-1; the A2.3↔A3.3 canonical type) were adjudicated against the assembled plan's actual A3.5/A3.6 implementation, not the drafts.

| Finding | Confidence | Basis |
|---|---|---|
| C1 ArbitrageCfg ×3 / golden ×3 | High | plan lines 3964/5509/7524 + re-pins 4068/5563/7642 read; cut §1.1-A3 |
| C2 Trade deleted twice | High | A0.3 ~601 + A3.4 ~4290; Resource removed in A1b |
| C3 pending columns ×2 | High | 3266 vs 4172; A3.5 writes StationId at 4546 |
| C4 assert_resource_identity ×3 | High | 2461/3091/5141; live world.rs:2870 confirms cargo-only today |
| C5 A0.2 missing Trade arm | High | None-arm group 432-446 read; note at 451 contradicts code |
| C6 GoodSpec.name type split | High | 2147 &str / 3925 String / 7482 &str |
| Systems C-1 rejection | High | A3.5 Step 2 sell path (4544-4550) + resolve_trade_sells present |

### Risk Assessment
**Implementation Risk:** High (plan does not compile as written at the A2/A3/A4/A5 seams). **Reversibility:** Moderate — A0/A1 are additive/hash-neutral and rollable; the v6 bump (A2) and the single GOLDEN_CONFIG_HASH re-pin (A3) are one-way within the branch but bisectable via the A0 behavior-digest baseline.

| Risk | Severity | Likelihood | Mitigation |
|---|---|---|---|
| Compile failure at A3.2/A4.2/A5.2 (ArbitrageCfg) | Critical | Certain | C1 — one struct in A3.2 |
| Compile failure mid-A3 (Trade re-delete, Resource::Ore) | High | Certain | C2 |
| Compile failure at A3.3 (duplicate columns) | High | Certain | C3 — delete A2.3 |
| Compile failure at A3.6 (N_RESOURCES re-extension) | High | Certain | C4 |
| Compile failure at A0.2 (non-exhaustive match) | High | Certain | C5 |
| WA5 read uninterpretable without WA3 co-column | Medium | Likely | M3 |

### Information Gaps
1. [ ] **A6 lab phase** read only via the cut + assembled summary; M3's WA3 co-column may be partially present in unread A6 detail (Reality + Quality both flagged A6 as not exhaustively reviewed).
2. [ ] **Direct `run_scripted_dispatch` test callers** not enumerated by any reviewer (M4) — a grep is required during A4.5.
3. [ ] **`scenario_bazaar` Food-sink station-row topology** (rows 3/4/8) asserted but not verified against band geometry (Quality gap 2).
4. [ ] **`credit_identity_trade_fixture` definition** referenced in A3.6 but not shown (Quality MAJOR-3) — builder must author it; recommend also extending the world-level `phase2_credit_identity_holds_every_tick` with an `exchange.active=true` bazaar variant.
5. [ ] **Synthesis-specific:** systems vs assembled-plan divergence shows the assembled plan and the per-phase drafts have drifted — a re-assembly from current drafts should precede execution to avoid re-introducing fixed duplications.

### Caveats & Required Follow-ups
- [ ] After de-duplication, re-run a full `cargo build --workspace` mentally/actually at each phase tip — the six CRITICALs were all compile-blockers; confirm none remain.
- [ ] Confirm each de-dup keeps the canonical type that A3.5/A3.6 *consume* (StationId tuple; A5.2-superset ArbitrageCfg; A1.2 String GoodSpec) — picking the wrong survivor reintroduces the conflict elsewhere.
- [ ] M3 (WA3 co-read) and M5 (ASSIGN empty-hold test) must land before the rung-A exit console session, per the cut.
- **Assumptions:** the cut and spec are authoritative; windows are recorded-never-gated (PDR-0006); D1–D8/OD-1..7 settled. Findings proposing metric gates (systems M-3 soft-gate) or contradicting the cut's deliberate choices (n_corps=1, trade_reserve=0, 5.4B battery) were rejected on that authority.
- **Limitations:** this synthesis does not re-verify A3–A6 line refs exhaustively (Reality's scope ended at A2); it inherits reviewer line numbers where not independently checked, though all six CRITICALs were checked.
