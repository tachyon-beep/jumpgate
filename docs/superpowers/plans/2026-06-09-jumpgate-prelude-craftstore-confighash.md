# Prelude — CraftStore Rename + config_hash Destructure Guard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the two mechanical, hash-neutral changes that *both* the Guidance-Parameter spec and the Person+Ship spec depend on, so neither spec owns them and neither eats a merge conflict: (1) rename the internal `ShipStore → CraftStore`; (2) make `RunConfig::config_hash` an exhaustive destructure so a future config field can never be silently omitted from the hash.

**Architecture:** This is a shared *prelude* extracted on the advice of an architecture + determinism review of the two specs. Both are pure de-risking: the rename is `pub(crate)` and behaviour-/hash-neutral; the destructure changes no hash value, it only converts "forgot to fold a field" from a silent provenance bug into a compile error. **No `HASH_FORMAT_VERSION` bump, no golden changes** — `GOLDEN_ZERO_STATE_HASH = 0xf0dd_a1ba_f433_3735` and the cfg-with-craft golden `0x532d_07bf_95a2_abc5` both stay green; the config-hash sample values stay identical.

**Tech Stack:** Rust 2024, `cargo test` / `cargo clippy --all-targets`, crate `jumpgate-core`.

**Why this exists / sources:** glossary.md (`craft` canonical, `ShipStore → CraftStore`); guidance spec D10/M6 (`config_hash` exhaustive destructure); the cross-spec review (architecture-critic + determinism-reviewer) that recommended extracting both as a prelude both specs build on.

**Lands:** FIRST, before the Guidance-Parameter spec and the Person+Ship spec.

---

### Task 1: Rename `ShipStore` → `CraftStore` (mechanical, hash-neutral)

The public id/contract surface already uses `craft` correctly (`CraftId`, `craft_pos`, `craft_ids`, `CraftInit`) — those **stay**. Only the internal `pub(crate)` storage type and a few `ship`-named locals change. This is a compiler-driven rename: the proof of correctness is that the full suite + both goldens stay green.

**Files (all current `ShipStore` references — re-grep before editing):**
- Modify: `crates/jumpgate-core/src/stores.rs` (`ShipStore` struct + `impl ShipStore` + doc comments)
- Modify: `crates/jumpgate-core/src/world.rs` (`use`, the `ships: ShipStore` field type, `reset` literal)
- Modify: `crates/jumpgate-core/src/lib.rs` (re-export)
- Modify: any other hit from the grep below (e.g. `hash.rs`, `events.rs` if they name the type)

- [ ] **Step 1: Enumerate every site**

Run: `git grep -n "ShipStore"`
Expected: a finite list across `stores.rs`, `world.rs`, `lib.rs` (and possibly `hash.rs`/`events.rs`). Note them all before editing.

- [ ] **Step 2: Rename the type and update all sites**

Rename `ShipStore` → `CraftStore` everywhere the grep found it (the struct definition, `impl ShipStore` → `impl CraftStore`, the `use crate::stores::{… ShipStore …}` imports, the `lib.rs` re-export, and the `ships: ShipStore` field type in `world.rs`). Keep the field name `ships` and the local `ships`/`ship` variable names **as-is for now** (renaming locals is optional polish, not required; do it only if a site reads `ShipStore` in a doc/name). Do NOT touch `CraftId`, `craft_*`, or `CraftInit` — already correct.

Update the doc comment on the struct to read "SoA store for mobile **craft**" (it likely already says craft/ships mixed — make it say craft).

- [ ] **Step 3: Build + full suite (the proof it's behaviour-neutral)**

Run: `cargo build -p jumpgate-core`
Expected: compiles (a missed site is a compile error — that's the safety net).
Run: `cargo test -p jumpgate-core`
Expected: PASS — every test, unchanged count.
Run: `cargo test -p jumpgate-core golden_zero_state_hash state_hash_golden`
Expected: PASS — `GOLDEN_ZERO_STATE_HASH` and the `0x532d…` golden are **unchanged** (a rename touches no hashed bytes).
Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(core): rename ShipStore -> CraftStore (craft is canonical; hash-neutral) (prelude/1)"
```

---

### Task 2: `config_hash` exhaustive destructure guard (D10/M6)

Make `RunConfig::config_hash` destructure `self`, so adding any future `RunConfig` field (guidance's `guidance`, Person's `PersonInit`) is a **compile error** until it is explicitly folded — closing the silent-omission provenance hole (two configs differing only in an unfolded field would collide, letting a divergent replay pass its config-hash gate).

**Files:**
- Modify: `crates/jumpgate-core/src/config.rs:112-152` (`config_hash`)
- Test: `crates/jumpgate-core/src/config.rs` (inline `tests` — add a config-hash golden anchor)

- [ ] **Step 1: Write the golden-anchor test**

Add to the `tests` module in `crates/jumpgate-core/src/config.rs` (pins the current value so the destructure refactor is proven value-preserving, and any future field that changes the stream is caught):

```rust
#[test]
fn config_hash_golden_anchor_is_stable() {
    // Drift-lock: the sample config's hash must not move under a refactor that
    // is meant to be value-preserving (e.g. the exhaustive-destructure change).
    // If a NEW field is added and folded, this value SHOULD change and be re-pinned
    // deliberately (mirrors the state_hash golden discipline).
    let got = sample().config_hash();
    assert_eq!(got, ConfigHash(GOLDEN_CONFIG_HASH), "config_hash drifted: re-pin only if intentional");
}
```

- [ ] **Step 2: Run it to capture the current value (expected FAIL → read the number)**

Add a placeholder const above the test so it compiles:

```rust
const GOLDEN_CONFIG_HASH: u64 = 0; // placeholder; replaced in Step 3
```

Run: `cargo test -p jumpgate-core config_hash_golden_anchor_is_stable -- --nocapture`
Expected: FAIL — the assert prints `left == ConfigHash(<actual>)`. Record `<actual>` (the real current config hash of `sample()`).

- [ ] **Step 3: Pin the golden + convert config_hash to an exhaustive destructure**

Set the const to the captured value:

```rust
const GOLDEN_CONFIG_HASH: u64 = 0x____________; // value captured in Step 2
```

Rewrite the head of `config_hash` (config.rs:112) to destructure `self` so every field is named (the body's fold order is unchanged — value-preserving):

```rust
    pub fn config_hash(&self) -> ConfigHash {
        // Exhaustive destructure: a NEW RunConfig field is a COMPILE ERROR here
        // until it is explicitly folded below (D10/M6 — closes the silent-omission
        // provenance hole). Field FOLD ORDER below is unchanged (value-preserving).
        let RunConfig {
            master_seed,
            dt,
            softening,
            substep_cfg,
            ephemeris_window,
            bodies,
            craft,
        } = self;
        let mut h = ConfigFnv::new();
        h.write_u64(*master_seed);
        h.write_u64(dt.bits());
        h.write_u64(softening.to_bits());
        h.write_u64(substep_cfg.accel_ref.to_bits());
        h.write_u64(substep_cfg.max_substeps as u64);
        h.write_u64(*ephemeris_window);
        h.write_u64(bodies.len() as u64);
        h.write_u64(craft.len() as u64);
        for b in bodies {
            h.write_u64(b.mass.to_bits());
            h.write_u64(b.elements.a.to_bits());
            h.write_u64(b.elements.e.to_bits());
            h.write_u64(b.elements.i.to_bits());
            h.write_u64(b.elements.raan.to_bits());
            h.write_u64(b.elements.argp.to_bits());
            h.write_u64(b.elements.m0.to_bits());
        }
        for c in craft {
            h.write_u64(c.spec.base_dry_mass.to_bits());
            h.write_u64(c.spec.base_max_thrust.to_bits());
            h.write_u64(c.spec.base_exhaust_velocity.to_bits());
            h.write_u64(c.spec.base_fuel_capacity.to_bits());
            let p = c.pos.to_bits();
            h.write_u64(p[0]);
            h.write_u64(p[1]);
            h.write_u64(p[2]);
            let v = c.vel.to_bits();
            h.write_u64(v[0]);
            h.write_u64(v[1]);
            h.write_u64(v[2]);
            h.write_u64(c.fuel_mass.to_bits());
        }
        ConfigHash(h.finish())
    }
```

(Adjust the destructured field list to match the EXACT current `RunConfig` fields from config.rs:58-70 — re-read them; do not trust this list if the struct has drifted.)

- [ ] **Step 4: Verify value-preserving + the existing field-change tests**

Run: `cargo test -p jumpgate-core config_hash_golden_anchor_is_stable`
Expected: PASS — the destructure did not change the value.
Run: `cargo test -p jumpgate-core -- config_hash changing_`
Expected: PASS — all existing `changing_*_changes_hash` / `same_config_same_hash` tests still green.
Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/jumpgate-core/src/config.rs
git commit -m "feat(core): exhaustive-destructure config_hash + golden anchor (forgotten field = compile error) (prelude/2)"
```

---

## Prelude self-review

- **Coverage:** glossary rename mandate → Task 1; guidance D10/M6 destructure → Task 2. Both hash-neutral (no `HASH_FORMAT_VERSION` bump, state goldens unchanged, config-hash value pinned identical).
- **No placeholders:** the only literal to fill is `GOLDEN_CONFIG_HASH`, captured empirically in Task 2 Step 2 (the standard golden-capture loop).
- **Downstream:** after this, the Guidance spec drops its rename + D10 tasks (now done here) and the Person spec's Plan A targets `CraftStore`. Both specs append their config field (`guidance`, then `PersonInit`) into the now-exhaustive destructure — a compile error guides them.
