# Handover → Guidance-Parameter / substructure line

**From:** Person+Ship (crew/command) design line
**To:** Guidance-Parameter system + universe/physics substructure line
**Date:** 2026-06-09
**Branch:** `jumpgate-v1-design`
**Status of my line:** design spec + Plan A committed; **nothing implemented**. Your line is also spec-stage (guidance spec committed, not implemented).

---

## TL;DR — what I need from you (5 things)

1. **Accept a shared PRELUDE that lands first** (`docs/superpowers/plans/2026-06-09-jumpgate-prelude-craftstore-confighash.md`): the `ShipStore → CraftStore` rename **plus** your D10 `config_hash` exhaustive-destructure, pulled out of guidance into one small hash-neutral unit that *both* our lines build on. (It's your D10 + the glossary rename — not new work, just sequenced first and owned jointly.)
2. **Rebase the rest of guidance onto `CraftStore` + the destructured `config_hash`** — append your `guidance: GuidanceParams` field *into the destructure pattern* (it becomes a compile error if you don't).
3. **Keep `effective_params(&BaseSpec)` pure — but don't treat it as permanently frozen.** My line legitimately adds a second arg (`&EffectiveMods`) later, for *capability* mods. That is orthogonal to your policy-in-`autopilot_command` decision. Don't let D1's old wording be cited to block it.
4. **Carry one forward-debt in your reset guard:** once my `EffectiveMods` multiplies `max_thrust`, your D6/§6.5 resolvability guard must read the **crew-modified** `max_thrust`. My line resolves the mods column **at reset, before your guard runs** — keep that ordering when both land.
5. **Goldens stay single-cause and separate:** you re-derive the **cruise-axis trajectory goldens** with **no `HASH_FORMAT_VERSION` bump**; my Plan B later bumps 1→2 and re-derives **both** state goldens. Don't batch them.

I also **edited your guidance spec's D1 section** (a clarifying note — see §"What I changed in your files"). Please review/keep or adjust.

---

## Why this happened (rationale)

Both our lines were designed independently and then found to **overlap on the hottest shared surfaces**: the craft store (`ShipStore`), `effective_params`/`autopilot_command`, `config_hash`, and `World::reset`. Both are unimplemented, so this is the cheap moment to align.

I ran a cross-spec review with two SME agents (the advisor timed out both calls):
- **architecture-critic** — decomposition + the seam.
- **determinism-reviewer** — hash/config_hash/reset ordering.

They converged. The user (who owns **both** lines) approved the outcome. The three load-bearing findings:

- **The `config_hash` silent-omission hole is real and HIGH.** `RunConfig::config_hash` (config.rs:112–152) manually enumerates fields with **no destructure**. Adding a config field (your `guidance`, my `PersonInit`) and forgetting to fold it produces **two different configs with the same `config_hash`** — which lets a divergent replay pass Task-14's config-hash provenance gate. Your D10 destructure fixes exactly this. It must land **before either** config field is added → it belongs in the prelude, first.
- **The `ShipStore→CraftStore` rename should be owned by neither spec.** The glossary hands it to guidance ("guidance already rewrites that store"), but *both* lines rewrite that struct. Whoever lands second eats a rename-vs-add conflict on the `stores.rs` / `world.rs::reset` literals. Extracting it (with D10) into the prelude removes the double-churn.
- **`effective_params` capability-vs-policy is a clean split, not a conflict.** Your D1 keeps the cruise cap (policy: dt/arrival-aware) out of `effective_params` and in `autopilot_command` — correct. My line puts crew/wear capability *into* `effective_params` (the founding `Effective = base × component-mods × wear` intent; the integrator's burn reads `Effective`). Different functions, different reasons, no behavioural overlap.

---

## What changed (the decisions)

**Land order:** `prelude → guidance → Person Plan A → B → C`.
- Prelude first (rename + `config_hash` destructure; hash-neutral).
- **Guidance before Person** so my Plan A's trajectory-equivalence proof is measured against your *settled* cruise baseline (you move the cruise-axis trajectory goldens once; I prove "bit-identical" against the post-guidance trajectory).

**Capability arg is a general bundle.** My line's `effective_params` second arg is `EffectiveMods { thrust_factor, /* reserved wear/component */ }` (a pre-reduced modifier bundle), **not** a crew-only struct — so `effective_params` changes signature exactly once, ever, even when wear/component systems land. The craft-store column is `mods: Vec<EffectiveMods>` (derived, unhashed, identity in v1).

**Terminology (your glossary, now canonical):** `craft` is the unit (drone→titan); "ship" rejected; `ShipStore→CraftStore`; "captain" = the per-craft command authority = my `controller` slot.

---

## What you need to do (concrete)

### Action 1 — Adopt the prelude as your first landing
Read `docs/superpowers/plans/2026-06-09-jumpgate-prelude-craftstore-confighash.md`. It is:
- **Task 1:** `ShipStore → CraftStore` rename (pub(crate), compiler-driven, hash-neutral; goldens unchanged).
- **Task 2:** `config_hash` exhaustive destructure (`let RunConfig { … } = self;`) + a pinned `GOLDEN_CONFIG_HASH` anchor.

**Decision for you:** either *you* implement the prelude as your first two commits (most natural — you're already rewriting that store and you own D10), or signal that my line should. Either way it lands **before** any behavioural guidance change and before any new config field.

→ **In your guidance spec, drop D10 and the rename from the guidance implementation scope** (they're now the prelude). Your spec text can stay as the rationale; just don't double-implement them.

### Action 2 — Rebase guidance onto the prelude
- Reference `CraftStore` (not `ShipStore`).
- Add `guidance: GuidanceParams` as the last `RunConfig` field **inside the destructure pattern** from prelude Task 2, and fold it at the tail. The destructure will *force* you to (compile error otherwise) — that's the point.
- Re-pin `GOLDEN_CONFIG_HASH` (prelude's anchor) when you add the `guidance` field — its value legitimately changes; that's an intentional re-pin, the same discipline as the state golden.

### Action 3 — Leave the `effective_params` seam alone, but don't fence it
- `effective_params` stays `&BaseSpec → Effective` **in your diff** — no change for you. ✓
- Do **not** add any "the pure seam must never change" assertion/comment that would later block my `(&BaseSpec, &EffectiveMods)` change. I patched D1's wording to prevent exactly this misreading (see below).
- Your `autopilot_command(…, guidance: &GuidanceParams, dt)` change is unaffected by my line. ✓

### Action 4 — Carry the reset-ordering forward-debt
Your D6/§6.5 `World::reset` resolvability guard checks `max_thrust/dry_mass · dt² < ARRIVAL_RADIUS`. Once my line ships, `max_thrust` is **crew-modified** (`effective_params(spec, &mods)`), and the guard must validate the *modified* value, not bare base. My line resolves the `mods` column **at reset, before your guard runs**, so the guard can call `effective_params(spec, &reset_mods)`. 
- **In v1 / Plan A, `mods` is identity**, so your guard is unaffected today.
- **When my Plan B lands**, honor the ordering: populate `mods` at reset → then run the guard reading the modified `max_thrust`. (I added a note to your spec D1 recording this.)

### Action 5 — Keep the two golden re-derivations separate
- **You:** the cruise cap goes absolute→per-ship → re-derive the **cruise-axis trajectory/physics goldens** (the `physics_sanity` transfer/arrival tests). **No `HASH_FORMAT_VERSION` bump** — your change is behavioural at the trajectory level, and the tick-0 state goldens (`GOLDEN_ZERO_STATE_HASH = 0xf0dd…`, the cfg-with-craft `0x532d…`) are computed at reset, *before* any step, so they don't move from a cruise-law change.
- **Me (Plan B):** bump `HASH_FORMAT_VERSION` 1→2 (folds Person input columns) → re-derive **both** state goldens (the version word seeds `FnvHasher::new()`, so both `0xf0dd…` and `0x532d…` move).
- Don't batch these into one re-baseline — single-cause attribution ("a moved golden = exactly this reason") is the whole point.

---

## What I changed in *your* files (transparency)

**`docs/superpowers/specs/2026-06-09-guidance-parameter-system-design.md` — D1 only.** I appended a clearly-marked "Cross-spec note (2026-06-09)" block under the "`effective_params` is unchanged" line. It:
- scopes "unchanged" to *your* diff (not a permanent invariant),
- states the capability(`effective_params`)/policy(`autopilot_command`) split,
- notes the rename + D10 are extracted into the prelude,
- records the reset-ordering forward-debt.

I changed **no decision** of yours and touched **no other section**. If the wording clashes with anything you have in flight, adjust freely — the intent is only to stop a future contributor citing "`effective_params` is unchanged" to reject my two-arg change.

---

## Determinism coordination summary (one table)

| Concern | Your line (guidance) | My line (Person) | Coordination |
|---|---|---|---|
| `config_hash` destructure | your D10 → **prelude** | relies on it for `PersonInit` | prelude lands first |
| new `RunConfig` field | `guidance` (tail) | `PersonInit` (after `guidance`) | both into the destructure; re-pin `GOLDEN_CONFIG_HASH` each |
| `HASH_FORMAT_VERSION` | **no bump** | **1→2** (Plan B) | separate; yours isn't a format change |
| state goldens `0xf0dd`/`0x532d` | unchanged | **both** re-derived (Plan B) | keep single-cause |
| trajectory/physics goldens | **re-derived** (cruise cap) | must NOT move them again | guidance-first settles them |
| `effective_params` sig | `&BaseSpec` (unchanged) | `&BaseSpec, &EffectiveMods` | orthogonal; don't fence |
| `World::reset` guard | reads `max_thrust` | resolves `mods` at reset first | honor ordering in Plan B |
| craft-store columns | `+prev_pos` (unhashed) | `+mods`, later `+roster/controller/...` | both append on `CraftStore` post-prelude |

---

## Open questions for you

1. **Who implements the prelude?** You (natural — you own the store rewrite + D10) or me? It's 2 hash-neutral tasks.
2. **Is the universe/physics substructure foundation merged/stable yet?** My Plan A and your guidance both edit `world.rs::step`/`reset`/`stores.rs`; we should not run on a moving foundation. The land order assumes the substructure is settled first.
3. **Any in-flight edit to guidance D1** that my note collides with? (You've been committing terminology + Q2/Q3 resolutions today — `a896248`, `5f89b25`, etc.)

---

## Reference index

- Prelude plan: `docs/superpowers/plans/2026-06-09-jumpgate-prelude-craftstore-confighash.md`
- My spec (§0 addendum has terminology + sequencing): `docs/superpowers/specs/2026-06-08-jumpgate-person-ship-foundation-design.md`
- My Plan A (`EffectiveMods` seam): `docs/superpowers/plans/2026-06-08-jumpgate-person-ship-plan-A-crewmods-seam.md`
- Your spec (D1 note added): `docs/superpowers/specs/2026-06-09-guidance-parameter-system-design.md`
- Glossary: `docs/glossary.md`
- Touchpoints: `stores.rs` (`effective_params`, `CraftStore`), `world.rs` (`reset`, `step`), `ingest.rs:165`, `config.rs` (`config_hash`), `hash.rs` (`HASH_FORMAT_VERSION`, both goldens).

---

## Response — guidance / substructure line (2026-06-09)

**Accepted in full. The prelude is LANDED (I implemented it).**

- **Prelude shipped** as two commits on `jumpgate-v1-design`: `403b74d` (Task 1, `ShipStore → CraftStore` rename) and `b856b67` (Task 2, `config_hash` exhaustive-destructure + `GOLDEN_CONFIG_HASH = 0x9767_52c4_8d05_053c`). Built via a build→adversarial-verify workflow and re-verified by me: **hash-neutral confirmed** — both state goldens byte-unchanged (`0xf0dd_a1ba_f433_3735`, `0x532d_07bf_95a2_abc5`) and green; the `config_hash` fold sequence audited byte-for-byte identical (the anchor alone is circular, so I checked the diff); zero `ShipStore` hits; 113 lib + 6 physics + 5 replay green; clippy `--all-targets` clean.
- **Action 2 (rebase):** done in the guidance spec — D10 + the rename are marked DONE-in-prelude; the guidance impl now appends `guidance` into the existing destructure and re-pins `GOLDEN_CONFIG_HASH`. Commit `06dbbe7`.
- **Action 3 (don't fence `effective_params`):** agreed; your D1 cross-spec note is kept verbatim. No "never change" assertion added. Your `(&BaseSpec, &EffectiveMods)` change is unopposed.
- **Action 4 (reset-ordering debt):** recorded as a forward-debt note in guidance §6.5 — when `EffectiveMods` multiplies `max_thrust`, the resolvability guard validates the *modified* value, and your line resolves `mods` at reset *before* the guard. Identity in v1, so no effect today.
- **Action 5 (single-cause goldens):** honoured — guidance re-derives only the **cruise-axis trajectory/physics goldens**, **no `HASH_FORMAT_VERSION` bump**; your Plan B owns the 1→2 bump + both state-golden re-derives. Not batched.

**Open questions answered:**
1. **Who implements the prelude?** — Me, done (above).
2. **Substructure foundation stable?** — Yes. Working tree clean of `src/` changes at handover; full suite + clippy green pre- and post-prelude. The guidance + Person edits to `world.rs::step/reset` / `stores.rs` are not running on a moving foundation.
3. **D1 collision with your note?** — None. My in-flight commits were terminology/glossary + Q2/Q3 spec resolutions (`a896248`, `5f89b25`, …); your D1 note coexists cleanly and is retained.

**Land order stands:** prelude ✅ → guidance (writing-plans next, on user's go) → Person Plan A → B → C.
