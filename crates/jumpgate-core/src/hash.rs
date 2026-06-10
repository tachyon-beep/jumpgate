//! Per-tick STATE-hash specification + the shared FNV-1a hasher (spec §6).
//! Landed early as the DRIFT-LOCK ANCHOR: the canonical field order
//! (`HASH_FIELD_ORDER`) is authoritative here, so a later task that adds a
//! hashed field (e.g. Task 11 adds prev_fuel/prev_inside_dest) MUST append to
//! `HASH_FIELD_ORDER`, bump `HASH_FORMAT_VERSION`, and update the golden test —
//! the change cannot silently alter the hash uncaught.
//!
//! DISTINCT from the CONFIG hash (`config::RunConfig::config_hash`): that one
//! folds immutable initial conditions ONCE with a "CONFIG_1" tag. This one
//! hashes evolving world state each tick seeded with `HASH_MAGIC`.
//!
//! ## HASH_FIELD_ORDER — canonical per-tick state-hash field order
//!
//! `state_hash` (Task 13) writes EXACTLY these u64 words in EXACTLY this order.
//! Every f64 is encoded via `f64::to_bits()`; every word is folded
//! little-endian by `FnvHasher::write_u64`. Numbering is stable; APPEND only.
//!
//!  1. HASH_MAGIC                              (Task 3, header)
//!  2. HASH_FORMAT_VERSION as u64              (Task 3, header)
//!  3. tick.0                                  (Task 3, time)
//!  4. body_store.ids.cursor()                 (Task 4/13, slot-map high-water)
//!  5. ship_store.ids.cursor()                 (Task 4/13, slot-map high-water)
//!
//! Bodies, sorted by BodyId (slot, generation):
//!
//!  6. body.slot as u64, body.generation as u64       (Task 13)
//!  7. body.mass.to_bits()                     (Task 13)
//!     (body POSITION is derived from tick via ephemeris, NOT stored, so it is
//!     NOT hashed independently — it is a pure function of tick already hashed)
//!
//! Craft, sorted by CraftId (slot, generation):
//!
//!  8. craft.slot as u64, craft.generation as u64     (Task 13)
//!  9. pos.x,pos.y,pos.z to_bits()             (Task 13)
//! 10. vel.x,vel.y,vel.z to_bits()             (Task 13)
//! 11. fuel_mass.to_bits()                     (Task 13)
//! 12. nav discriminant as u64 (+ resolved dest/dv_remaining bits)  (Task 13)
//!     APPEND-ONLY tags: Idle=0, Seeking=1, DirectThrust=2 (+ throttle_vec bits;
//!     tactical Rung 1 — new tag only, existing encodings untouched)
//! 13. lod discriminant as u64                 (Task 13)
//!
//! APPEND BELOW THIS LINE (bump HASH_FORMAT_VERSION + golden test on change):
//!
//! 14. prev_fuel[i].to_bits()      (RESERVED — deferred; transitively pinned, NOT folded)
//! 15. prev_inside_dest[i] as u64  (RESERVED — deferred; transitively pinned, NOT folded)
//!
//! ECONOMY (HASH_FORMAT_VERSION 2 — folded by `write_craft_economy` (per-craft, in
//! the sorted-craft loop) + `write_economy_stores` (after it). Both are shared by
//! `state_hash` and the parity recompute so they cannot drift):
//!
//!  Per craft (in the sorted-CraftId loop, AFTER word 13/lod):
//!   16. role.rank() as u64
//!   17. cargo: 0 (None) | 1 then (resource.index(), qty)
//!   18. credits_micros as u64
//!   19. contract: 0 (None) | 1 then (slot, generation)
//!
//! TROPHIC (HASH_FORMAT_VERSION 3 — folded by `write_craft_economy` after word 19,
//! in the per-craft sorted loop; shared by `state_hash` and the parity recompute):
//!   25. risk_appetite as u64
//!   26. pirate: 0 (None) | 1 then (food_micros as u64, notoriety as u64, lie_low_until.0)
//!
//!  Store-level (after the craft loop; each store: cursor, then sorted-id elements):
//!   20. EconCounters: mined[0..N], consumed[0..N]
//!   21. stations:     cursor; per station: slot, gen, body(slot,gen), per-resource (stock, price_micros)
//!   22. producers:    cursor; per producer: slot, gen, station(slot,gen), recipe(in disc+payload, out disc+payload, interval)
//!   23. corporations: cursor; per corp: slot, gen, treasury_micros, home_station(slot,gen)
//!   24. contracts:    cursor; per contract: slot, gen, status.rank(), corp(slot,gen), resource.index(), qty,
//!                     from(slot,gen), to(slot,gen), reward_micros, escrow_micros, hauler(0|1 then slot,gen)

/// Header magic for the per-tick STATE hash (little-endian, spec §6).
pub const HASH_MAGIC: u64 = 0x4a55_4d50_4741_5445; // "JUMPGATE"
/// Bump whenever HASH_FIELD_ORDER changes. v2: + economy state (words 16..=24).
/// v3: + trophic state (per-craft `risk_appetite` + `pirate`, words 25..=26).
pub const HASH_FORMAT_VERSION: u32 = 3;

/// Golden per-tick hash of the minimal zero-init slice under HASH_FIELD_ORDER
/// words 1..=13. Pinned so any change to the canonical encoding is caught.
/// Captured from the first run of `golden_zero_state_hash`; if HASH_FIELD_ORDER
/// or HASH_FORMAT_VERSION changes, recapture AND bump the version.
pub const GOLDEN_ZERO_STATE_HASH: u64 = 0x1d44_b373_5ccd_33f7; // RE-PINNED: HASH_FORMAT_VERSION 2->3 (+trophic state words 25..=26). Was 0x65d7_af3b_9a8a_8276.

/// Shared FNV-1a 64-bit hasher for the per-tick state hash. Folds each u64 as 8
/// little-endian bytes. `new()` seeds with `HASH_MAGIC` then the version word.
pub struct FnvHasher {
    state: u64,
}

const STATE_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const STATE_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

impl FnvHasher {
    pub fn new() -> Self {
        let mut h = FnvHasher {
            state: STATE_FNV_OFFSET,
        };
        h.write_u64(HASH_MAGIC); // HASH_FIELD_ORDER word 1
        h.write_u64(HASH_FORMAT_VERSION as u64); // HASH_FIELD_ORDER word 2
        h
    }
    /// Folds one u64 as 8 little-endian bytes (HASH_FIELD_ORDER words).
    pub fn write_u64(&mut self, v: u64) {
        for b in v.to_le_bytes() {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(STATE_FNV_PRIME);
        }
    }
    pub fn finish(self) -> u64 {
        self.state
    }
}

impl Default for FnvHasher {
    fn default() -> Self {
        Self::new()
    }
}

use crate::contract::StateView;
use crate::ids::SlotMap;
use crate::math::Vec3;
use crate::stores::NavState;
use crate::types::{EntityRef, NavDest};
use crate::world::World;

/// Mix a store's allocator cursor (high-water) into the hash. Present per §6 /
/// HASH_FIELD_ORDER so a future mid-run Spawn does not invalidate prior-tick
/// hashes. Generic so both BodyStore.ids and CraftStore.ids reuse it.
pub fn write_store_cursor<T>(h: &mut FnvHasher, store: &SlotMap<T>) {
    h.write_u64(store.cursor());
}

fn write_vec3(h: &mut FnvHasher, v: Vec3) {
    let [bx, by, bz] = v.to_bits();
    h.write_u64(bx);
    h.write_u64(by);
    h.write_u64(bz);
}

/// Canonical per-tick state hash. Mixes the words listed in HASH_FIELD_ORDER
/// (module doc) in that exact order. Body positions derive from `tick`
/// (already hashed) so only body identity + store cursor participate; ship
/// dynamic state is hashed in full.
pub fn state_hash(world: &World) -> u64 {
    // new() ALREADY folds HASH_FIELD_ORDER words 1-2 (HASH_MAGIC, HASH_FORMAT_VERSION).
    // Do NOT re-write them here, or the digest will not match the committed anchor.
    let mut h = FnvHasher::new();
    h.write_u64(world.tick().0); // word 3: tick

    // HASH_FIELD_ORDER words 4-5: both store allocator cursors (hashed state, §6),
    // body-store then ship-store, BEFORE any body/craft data.
    write_store_cursor(&mut h, &world.bodies.ids);
    write_store_cursor(&mut h, &world.ships.ids);

    // HASH_FIELD_ORDER words 6-7: bodies, sorted by id — slot, generation, mass.
    // (Body POSITIONS are tick-derived via ephemeris, so they are NOT hashed.)
    let mut bodies = world.body_ids();
    bodies.sort();
    for b in bodies {
        h.write_u64(b.slot as u64);
        h.write_u64(b.generation as u64);
        let bi = world.bodies.ids.dense_index(b.slot, b.generation).unwrap();
        h.write_u64(world.bodies.mass[bi].to_bits()); // word 7: body mass
    }

    // HASH_FIELD_ORDER words 8-13: craft, sorted id, then full dynamic state.
    let mut craft = world.craft_ids();
    craft.sort();
    for c in craft {
        h.write_u64(c.slot as u64);
        h.write_u64(c.generation as u64);
        // Dense, length-parallel SoA columns: in v1 every live craft row has every
        // component (slot == row; no mid-run despawn). A missing component would make
        // the encoding non-self-delimiting and silently corrupt the hash, so fail loud
        // rather than drop words. `recompute_with_cursors` (the parity spec) likewise
        // unwraps these.
        let p = world
            .craft_pos(c)
            .expect("dense SoA invariant: live craft row missing craft_pos column");
        write_vec3(&mut h, p);
        let v = world
            .craft_vel(c)
            .expect("dense SoA invariant: live craft row missing craft_vel column");
        write_vec3(&mut h, v);
        let f = world
            .craft_fuel(c)
            .expect("dense SoA invariant: live craft row missing craft_fuel column");
        h.write_u64(f.to_bits());
        // HASH_FIELD_ORDER word 12: NavState (discriminant-first, self-delimiting).
        // Read the dense row via the public SlotMap accessor (ship_index is private
        // to world.rs). The NavDest discriminant is folded BEFORE its payload so
        // Position(x,0,0) and Entity(slot=x) cannot collide.
        let idx = world.ships.ids.dense_index(c.slot, c.generation).unwrap();
        match world.ships.nav[idx] {
            NavState::Idle => h.write_u64(0),
            NavState::Seeking { dest, dv_remaining } => {
                h.write_u64(1);
                match dest {
                    NavDest::Position(p) => {
                        h.write_u64(0);
                        let [dx, dy, dz] = p.to_bits();
                        h.write_u64(dx);
                        h.write_u64(dy);
                        h.write_u64(dz);
                    }
                    NavDest::Entity(EntityRef::Craft(id)) => {
                        h.write_u64(1);
                        h.write_u64(0); // kind: craft
                        h.write_u64(id.slot as u64);
                        h.write_u64(id.generation as u64);
                    }
                    NavDest::Entity(EntityRef::Body(id)) => {
                        h.write_u64(1);
                        h.write_u64(1); // kind: body
                        h.write_u64(id.slot as u64);
                        h.write_u64(id.generation as u64);
                    }
                }
                h.write_u64(dv_remaining.to_bits());
            }
            // NEW tag 2 (APPEND-ONLY): DirectThrust. Existing Idle(0)/Seeking(1)
            // encodings are untouched, so all pre-existing goldens are stable.
            NavState::DirectThrust { throttle_vec } => {
                h.write_u64(2);
                let [tx, ty, tz] = throttle_vec.to_bits();
                h.write_u64(tx);
                h.write_u64(ty);
                h.write_u64(tz);
            }
        }
        // HASH_FIELD_ORDER word 13: Lod discriminant (lod() is on StateView).
        // Dense SoA invariant as above: every live craft row has a lod column.
        let l = world
            .lod(c)
            .expect("dense SoA invariant: live craft row missing lod column");
        h.write_u64(l as u64);
        // HASH_FIELD_ORDER words 16-19: this craft's economy columns.
        write_craft_economy(&mut h, world, idx);
    }

    // HASH_FIELD_ORDER words 20-24: store-level economy state (counters + the four
    // economy stores in sorted-id order). Folded AFTER the craft loop.
    write_economy_stores(&mut h, world);

    h.finish()
}

/// Fold one craft's HASHED economy columns — HASH_FIELD_ORDER words 16..=19.
/// `idx` is the craft's dense SoA row. SHARED by `state_hash` and the parity
/// recompute so the two encodings cannot drift; enums fold discriminant-first.
pub(crate) fn write_craft_economy(h: &mut FnvHasher, world: &World, idx: usize) {
    h.write_u64(world.ships.role[idx].rank() as u64); // 16
    match world.ships.cargo[idx] {
        // 17
        None => h.write_u64(0),
        Some((res, qty)) => {
            h.write_u64(1);
            h.write_u64(res.index() as u64);
            h.write_u64(qty as u64);
        }
    }
    h.write_u64(world.ships.credits_micros[idx] as u64); // 18
    match world.ships.contract[idx] {
        // 19
        None => h.write_u64(0),
        Some(cid) => {
            h.write_u64(1);
            h.write_u64(cid.slot as u64);
            h.write_u64(cid.generation as u64);
        }
    }
    // HASH_FIELD_ORDER words 25-26: trophic columns (format v3). risk_appetite
    // then pirate (self-delimiting: tag 0 for None, tag 1 + fields for Some).
    h.write_u64(world.ships.risk_appetite[idx] as u64); // 25
    match world.ships.pirate[idx] {
        // 26
        None => h.write_u64(0),
        Some(ps) => {
            h.write_u64(1);
            h.write_u64(ps.food_micros as u64);
            h.write_u64(ps.notoriety as u64);
            h.write_u64(ps.lie_low_until.0);
        }
    }
}

/// Fold a `Recipe` into the state hash (input disc+payload, output disc+payload,
/// interval) — self-delimiting so a `None` input cannot alias a present one.
fn write_recipe_hash(h: &mut FnvHasher, r: &crate::economy::Recipe) {
    match r.input {
        None => h.write_u64(0),
        Some((res, qty)) => {
            h.write_u64(1);
            h.write_u64(res.index() as u64);
            h.write_u64(qty as u64);
        }
    }
    match r.output {
        None => h.write_u64(0),
        Some((res, qty)) => {
            h.write_u64(1);
            h.write_u64(res.index() as u64);
            h.write_u64(qty as u64);
        }
    }
    h.write_u64(r.interval as u64);
}

/// Fold store-level HASHED economy state — HASH_FIELD_ORDER words 20..=24: the
/// audited counters, then the four economy stores each as (cursor, then elements
/// in sorted-id order). SHARED by `state_hash` and the parity recompute. Each
/// store's allocator cursor is folded (like the ship/body cursors) so a mid-run
/// `ContractStore::push` is reflected and a future spawn cannot rewrite history.
pub(crate) fn write_economy_stores(h: &mut FnvHasher, world: &World) {
    use crate::economy::N_RESOURCES;
    // 20. EconCounters.
    for r in 0..N_RESOURCES {
        h.write_u64(world.econ.mined[r] as u64);
    }
    for r in 0..N_RESOURCES {
        h.write_u64(world.econ.consumed[r] as u64);
    }
    // 21. stations.
    h.write_u64(world.stations.ids.cursor());
    let mut sids: Vec<(u32, u32)> = world.stations.ids.iter_ids().collect();
    sids.sort();
    for (slot, generation) in sids {
        let i = world.stations.ids.dense_index(slot, generation).unwrap();
        h.write_u64(slot as u64);
        h.write_u64(generation as u64);
        h.write_u64(world.stations.body[i].slot as u64);
        h.write_u64(world.stations.body[i].generation as u64);
        for r in 0..N_RESOURCES {
            h.write_u64(world.stations.stock[i][r] as u64);
            h.write_u64(world.stations.price_micros[i][r] as u64);
        }
    }
    // 22. producers.
    h.write_u64(world.producers.ids.cursor());
    let mut pids: Vec<(u32, u32)> = world.producers.ids.iter_ids().collect();
    pids.sort();
    for (slot, generation) in pids {
        let i = world.producers.ids.dense_index(slot, generation).unwrap();
        h.write_u64(slot as u64);
        h.write_u64(generation as u64);
        h.write_u64(world.producers.station[i].slot as u64);
        h.write_u64(world.producers.station[i].generation as u64);
        write_recipe_hash(h, &world.producers.recipe[i]);
    }
    // 23. corporations.
    h.write_u64(world.corporations.ids.cursor());
    let mut cids: Vec<(u32, u32)> = world.corporations.ids.iter_ids().collect();
    cids.sort();
    for (slot, generation) in cids {
        let i = world.corporations.ids.dense_index(slot, generation).unwrap();
        h.write_u64(slot as u64);
        h.write_u64(generation as u64);
        h.write_u64(world.corporations.treasury_micros[i] as u64);
        h.write_u64(world.corporations.home_station[i].slot as u64);
        h.write_u64(world.corporations.home_station[i].generation as u64);
    }
    // 24. contracts.
    h.write_u64(world.contracts.ids.cursor());
    let mut kids: Vec<(u32, u32)> = world.contracts.ids.iter_ids().collect();
    kids.sort();
    for (slot, generation) in kids {
        let i = world.contracts.ids.dense_index(slot, generation).unwrap();
        h.write_u64(slot as u64);
        h.write_u64(generation as u64);
        h.write_u64(world.contracts.status[i].rank() as u64);
        h.write_u64(world.contracts.corp[i].slot as u64);
        h.write_u64(world.contracts.corp[i].generation as u64);
        h.write_u64(world.contracts.resource[i].index() as u64);
        h.write_u64(world.contracts.qty[i] as u64);
        h.write_u64(world.contracts.from_station[i].slot as u64);
        h.write_u64(world.contracts.from_station[i].generation as u64);
        h.write_u64(world.contracts.to_station[i].slot as u64);
        h.write_u64(world.contracts.to_station[i].generation as u64);
        h.write_u64(world.contracts.reward_micros[i] as u64);
        h.write_u64(world.contracts.escrow_micros[i] as u64);
        match world.contracts.hauler[i] {
            None => h.write_u64(0),
            Some(cid) => {
                h.write_u64(1);
                h.write_u64(cid.slot as u64);
                h.write_u64(cid.generation as u64);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        BaseSpec, BodyInit, CraftInit, GuidanceParams, OrbitalElements, RunConfig, SubstepCfg,
    };
    use crate::math::Vec3;
    use crate::time::Dt;
    use crate::world::World;

    fn base_spec() -> BaseSpec {
        BaseSpec {
            base_dry_mass: 1.0,
            base_max_thrust: 0.1,
            base_exhaust_velocity: 0.05,
            base_fuel_capacity: 0.5,
            base_cargo_capacity: 5,
        }
    }

    fn cfg_with_craft_x(craft_x: f64) -> RunConfig {
        RunConfig {
            master_seed: 42,
            dt: Dt::new(0.01),
            softening: 1e-4,
            substep_cfg: SubstepCfg {
                accel_ref: 1.0,
                max_substeps: 16,
            },
            ephemeris_window: 64,
            bodies: vec![BodyInit {
                mass: 1.0,
                elements: OrbitalElements {
                    a: 1.0,
                    e: 0.0,
                    i: 0.0,
                    raan: 0.0,
                    argp: 0.0,
                    m0: 0.0,
                },
            }],
            craft: vec![CraftInit {
                spec: base_spec(),
                pos: Vec3::new(craft_x, 0.0, 0.0),
                vel: Vec3::ZERO,
                fuel_mass: 0.5,
                role: crate::stores::CraftRole::Idle,
                scripted: true,
            }],
            guidance: GuidanceParams::default(),
            stations: vec![],
            producers: vec![],
            corporations: vec![],
            contracts: vec![],
            price_cfg: crate::config::PriceCfg::default(),
            dispatch_cfg: crate::config::DispatchCfg::default(),
            trophic: crate::config::TrophicCfg::default(),
            shipyard: crate::config::ShipyardCfg::default(),
        }
    }

    #[test]
    fn identical_worlds_hash_equal() {
        let (wa, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let (wb, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        assert_eq!(state_hash(&wa), state_hash(&wb));
    }

    #[test]
    fn perturbing_one_f64_changes_hash() {
        // Two worlds identical except one craft x-coordinate differs slightly.
        let (wa, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let (wb, _) = World::reset(cfg_with_craft_x(2.0 + 1e-9)).expect("resolvable config");
        assert_ne!(state_hash(&wa), state_hash(&wb));
    }

    #[test]
    fn header_words_are_present_but_not_the_whole_hash() {
        // The first three words must be MAGIC, VERSION, tick. Recompute the
        // header-only hash independently; state_hash mixes MORE after it, so it
        // must NOT equal the header-only hash (proves header present AND body
        // follows). This pins HASH_FIELD_ORDER entries 1-3.
        let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let mut header_only = FnvHasher::new();
        header_only.write_u64(0u64); // word 3: tick 0 after reset
        assert_ne!(state_hash(&w), header_only.finish());
    }

    #[test]
    fn seeking_navdest_discriminant_is_folded_before_payload() {
        // Word 12 must fold the NavDest discriminant BEFORE its payload, so a craft
        // Seeking Position(Vec3::new(x,0,0)) MUST hash differently from one Seeking
        // Entity(Craft(CraftId{slot: x as u32, generation: 0})). Build two otherwise-identical
        // worlds, set the single craft's nav through the ingestion path, and assert
        // the two state hashes differ. (Pins word 12's encoding, which the Idle-only
        // golden zero-world test would otherwise leave unexercised.)
        use crate::contract::{Command, StateView};
        use crate::ids::CraftId;
        use crate::types::{CommandKind, EntityRef, NavDest, Target};
        let x: f64 = 7.0;

        let (mut wp, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let cp = wp.craft_ids()[0];
        let mut cmds_p = vec![Command {
            target: Target::Entity(EntityRef::Craft(cp)),
            kind: CommandKind::Destination {
                dest: NavDest::Position(Vec3::new(x, 0.0, 0.0)),
                burn_budget: None,
            },
        }];
        let tp = wp.tick();
        crate::ingest::ingest_commands(&mut wp, tp, &mut cmds_p);

        let (mut we, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let ce = we.craft_ids()[0];
        let mut cmds_e = vec![Command {
            target: Target::Entity(EntityRef::Craft(ce)),
            kind: CommandKind::Destination {
                dest: NavDest::Entity(EntityRef::Craft(CraftId {
                    slot: x as u32,
                    generation: 0,
                })),
                burn_budget: None,
            },
        }];
        let te = we.tick();
        crate::ingest::ingest_commands(&mut we, te, &mut cmds_e);

        assert_ne!(
            state_hash(&wp),
            state_hash(&we),
            "NavDest discriminant must be folded before payload (Position(x) != Entity(slot=x))"
        );
    }

    /// Independent recomputation of HASH_FIELD_ORDER, WITH the two store
    /// cursors written (words 4 and 5). This is the executable spec; if a
    /// field is added to `state_hash` without updating this helper, the golden
    /// test below diverges and forces the author to bump HASH_FORMAT_VERSION.
    fn recompute_with_cursors(w: &World) -> u64 {
        use crate::contract::StateView;
        // Mirrors the committed HASH_FIELD_ORDER exactly: new() folds words 1-2,
        // then tick(3), body cursor(4), ship cursor(5), per-body slot/generation/mass(6-7),
        // per-craft(8-13). Do NOT re-write the header — new() already did.
        let mut h = FnvHasher::new();
        h.write_u64(w.tick().0); // word 3
        write_store_cursor(&mut h, &w.bodies.ids); // word 4
        write_store_cursor(&mut h, &w.ships.ids); // word 5
        let mut bodies = w.body_ids();
        bodies.sort();
        for b in bodies {
            h.write_u64(b.slot as u64);
            h.write_u64(b.generation as u64);
            let bi = w.bodies.ids.dense_index(b.slot, b.generation).unwrap();
            h.write_u64(w.bodies.mass[bi].to_bits()); // word 7: body mass
        }
        let mut craft = w.craft_ids();
        craft.sort();
        for c in craft {
            h.write_u64(c.slot as u64);
            h.write_u64(c.generation as u64);
            let p = w.craft_pos(c).unwrap();
            let [px, py, pz] = p.to_bits();
            h.write_u64(px);
            h.write_u64(py);
            h.write_u64(pz);
            let v = w.craft_vel(c).unwrap();
            let [vx, vy, vz] = v.to_bits();
            h.write_u64(vx);
            h.write_u64(vy);
            h.write_u64(vz);
            h.write_u64(w.craft_fuel(c).unwrap().to_bits());
            // HASH_FIELD_ORDER word 12: NavState (discriminant-first, self-delimiting).
            // Map the sorted CraftId back to its dense row; ship_index is private to
            // world.rs, so resolve the row via the public SlotMap accessor.
            let idx = w.ships.ids.dense_index(c.slot, c.generation).unwrap();
            match w.ships.nav[idx] {
                NavState::Idle => h.write_u64(0),
                NavState::Seeking { dest, dv_remaining } => {
                    h.write_u64(1);
                    match dest {
                        NavDest::Position(p) => {
                            h.write_u64(0);
                            let [dx, dy, dz] = p.to_bits();
                            h.write_u64(dx);
                            h.write_u64(dy);
                            h.write_u64(dz);
                        }
                        NavDest::Entity(EntityRef::Craft(id)) => {
                            h.write_u64(1);
                            h.write_u64(0); // kind: craft
                            h.write_u64(id.slot as u64);
                            h.write_u64(id.generation as u64);
                        }
                        NavDest::Entity(EntityRef::Body(id)) => {
                            h.write_u64(1);
                            h.write_u64(1); // kind: body
                            h.write_u64(id.slot as u64);
                            h.write_u64(id.generation as u64);
                        }
                    }
                    h.write_u64(dv_remaining.to_bits());
                }
                // NEW tag 2 (APPEND-ONLY): DirectThrust — mirrors state_hash exactly.
                NavState::DirectThrust { throttle_vec } => {
                    h.write_u64(2);
                    let [tx, ty, tz] = throttle_vec.to_bits();
                    h.write_u64(tx);
                    h.write_u64(ty);
                    h.write_u64(tz);
                }
            }
            // HASH_FIELD_ORDER word 13: Lod discriminant (lod() is on StateView).
            h.write_u64(w.lod(c).unwrap() as u64);
            // HASH_FIELD_ORDER words 16-19: this craft's economy columns (shared fold).
            super::write_craft_economy(&mut h, w, idx);
        }
        // HASH_FIELD_ORDER words 20-24: store-level economy state (shared fold).
        super::write_economy_stores(&mut h, w);
        h.finish()
    }

    #[test]
    fn cursor_participates_in_state_hash() {
        // state_hash MUST include both store cursors (HASH_FIELD_ORDER 4, 5).
        // The independent recompute writes them; until Step 7 wires the cursors
        // into state_hash, the two digests diverge. Step 7 makes them equal.
        let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        assert_eq!(
            state_hash(&w),
            recompute_with_cursors(&w),
            "state_hash must mix both store cursors per HASH_FIELD_ORDER 4 and 5"
        );
    }

    /// Build a world whose economy stores + hauler columns + counters are all
    /// non-trivially populated, so the economy fold is exercised (the zero-world
    /// goldens and the economy-free parity test leave it as dead code otherwise —
    /// the advisor's "vacuous guard" gap). Deterministic: no RNG in population.
    fn populated_world() -> World {
        use crate::economy::{ContractStatus, Recipe, Resource};
        use crate::stores::CraftRole;
        let (mut w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let body = w.body_ids()[0];
        let st = w.stations.push(body, [10, 5], [100, 200]);
        let corp = w.corporations.push(1_000_000, st);
        let _prod = w.producers.push(
            st,
            Recipe { input: Some((Resource::Ore, 1)), output: Some((Resource::Fuel, 2)), interval: 3 },
        );
        let k = w.contracts.push(corp, Resource::Fuel, 3, st, st, 500_000);
        w.contracts.status[0] = ContractStatus::Accepted;
        w.contracts.escrow_micros[0] = 250_000;
        w.contracts.hauler[0] = Some(w.craft_ids()[0]);
        w.econ.mined[0] = 5;
        w.econ.consumed[1] = 3;
        w.ships.role[0] = CraftRole::Hauler;
        w.ships.cargo[0] = Some((Resource::Fuel, 2));
        w.ships.credits_micros[0] = 42;
        w.ships.contract[0] = Some(k);
        w
    }

    #[test]
    fn economy_free_world_replays_bit_identically_after_version_bump() {
        // PHASE-0 GATE: the HASH_FORMAT_VERSION 1->2 bump must NOT introduce
        // nondeterminism for the legacy (no-economy) path. Two worlds from the same
        // economy-free config (cfg_with_craft_x has empty economy stores), stepped
        // identically with empty commands, must produce the same state_hash each tick.
        use crate::contract::Command;
        let (mut a, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        let (mut b, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        for t in 0..20 {
            let mut ca: Vec<Command> = Vec::new();
            let mut cb: Vec<Command> = Vec::new();
            a.step(&mut ca);
            b.step(&mut cb);
            assert_eq!(
                state_hash(&a),
                state_hash(&b),
                "economy-free replay diverged at tick {}",
                t + 1
            );
        }
    }

    #[test]
    fn populated_world_hashes_deterministically() {
        assert_eq!(state_hash(&populated_world()), state_hash(&populated_world()));
    }

    #[test]
    fn populated_economy_parity() {
        // state_hash and the independent parity recompute must agree on a POPULATED
        // economy world (not just the empty-economy world cfg_with_craft_x gives).
        let w = populated_world();
        assert_eq!(
            state_hash(&w),
            recompute_with_cursors(&w),
            "state_hash and recompute must agree on populated economy state"
        );
    }

    #[test]
    fn economy_state_is_fully_folded() {
        use crate::economy::{ContractStatus, Resource};
        use crate::stores::CraftRole;
        // Each single-field mutation MUST move the hash — proving the field is folded
        // (completeness guard: a forgotten field would let two distinct economy states
        // collide and silently break replay-divergence detection).
        let h0 = state_hash(&populated_world());
        macro_rules! moves {
            ($name:expr, $mut:expr) => {{
                let mut w = populated_world();
                $mut(&mut w);
                assert_ne!(state_hash(&w), h0, concat!($name, " must be folded into state_hash"));
            }};
        }
        moves!("craft role", |w: &mut World| w.ships.role[0] = CraftRole::Idle);
        moves!("cargo qty", |w: &mut World| w.ships.cargo[0] = Some((Resource::Fuel, 99)));
        moves!("cargo presence", |w: &mut World| w.ships.cargo[0] = None);
        moves!("cargo resource", |w: &mut World| w.ships.cargo[0] = Some((Resource::Ore, 2)));
        moves!("craft credits", |w: &mut World| w.ships.credits_micros[0] = 43);
        moves!("craft contract handle", |w: &mut World| w.ships.contract[0] = None);
        moves!("econ.mined", |w: &mut World| w.econ.mined[0] = 6);
        moves!("econ.consumed", |w: &mut World| w.econ.consumed[1] = 4);
        moves!("station stock", |w: &mut World| w.stations.stock[0][0] = 11);
        moves!("station price", |w: &mut World| w.stations.price_micros[0][1] = 201);
        moves!("corp treasury", |w: &mut World| w.corporations.treasury_micros[0] = 999_999);
        moves!("producer recipe interval", |w: &mut World| w.producers.recipe[0].interval = 4);
        moves!("contract status", |w: &mut World| w.contracts.status[0] = ContractStatus::Completed);
        moves!("contract escrow", |w: &mut World| w.contracts.escrow_micros[0] = 250_001);
        moves!("contract qty", |w: &mut World| w.contracts.qty[0] = 4);
        moves!("contract reward", |w: &mut World| w.contracts.reward_micros[0] = 500_001);
        moves!("contract hauler", |w: &mut World| w.contracts.hauler[0] = None);
    }

    #[test]
    fn trophic_columns_are_folded_into_state_hash() {
        use crate::stores::{CraftRole, PirateState};
        use crate::time::Tick;
        // risk_appetite is folded (a non-zero value moves the hash).
        let h0 = state_hash(&populated_world());
        let mut w1 = populated_world();
        w1.ships.risk_appetite[0] = 500;
        assert_ne!(state_hash(&w1), h0, "risk_appetite must be folded into state_hash");

        // pirate folds self-delimitingly: None vs Some differ, and each is stable.
        let mut none_a = populated_world();
        let mut none_b = populated_world();
        none_a.ships.pirate[0] = None;
        none_b.ships.pirate[0] = None;
        assert_eq!(
            state_hash(&none_a),
            state_hash(&none_b),
            "None pirate state must hash stably"
        );

        let mut some_a = populated_world();
        let mut some_b = populated_world();
        let ps = PirateState { food_micros: 1_000, notoriety: 3, lie_low_until: Tick(50) };
        some_a.ships.role[0] = CraftRole::Pirate;
        some_a.ships.pirate[0] = Some(ps);
        some_b.ships.role[0] = CraftRole::Pirate;
        some_b.ships.pirate[0] = Some(ps);
        assert_eq!(
            state_hash(&some_a),
            state_hash(&some_b),
            "Some pirate state must hash stably"
        );
        assert_ne!(
            state_hash(&some_a),
            state_hash(&none_a),
            "Some vs None pirate state must hash differently (self-delimiting fold)"
        );

        // Each PirateState field is folded (mutating any field moves the hash).
        let base = state_hash(&some_a);
        for mutate in [
            |p: &mut PirateState| p.food_micros = 2_000,
            |p: &mut PirateState| p.notoriety = 4,
            |p: &mut PirateState| p.lie_low_until = Tick(51),
        ] {
            let mut w = populated_world();
            w.ships.role[0] = CraftRole::Pirate;
            let mut p = ps;
            mutate(&mut p);
            w.ships.pirate[0] = Some(p);
            assert_ne!(state_hash(&w), base, "each PirateState field must be folded");
        }
    }

    #[test]
    fn state_hash_golden_zero_world() {
        // Hardcoded digest of the canonical zero-init world (cfg_with_craft_x(2.0),
        // tick 0). Pins HASH_FIELD_ORDER + HASH_FORMAT_VERSION. If this changes,
        // a field was added/reordered or the version bumped: update HASH_FIELD_ORDER
        // (module doc), bump HASH_FORMAT_VERSION, and re-paste from `print_golden`.
        let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        // RE-PINNED: HASH_FORMAT_VERSION 2->3 (+trophic state words 25..=26). Was 0x64dd_5078_a3e0_5886.
        assert_eq!(state_hash(&w), 0x2d92_c7ce_4daa_4567u64);
    }

    #[test]
    #[ignore = "prints the golden constant for state_hash_golden_zero_world"]
    fn print_golden() {
        let (w, _) = World::reset(cfg_with_craft_x(2.0)).expect("resolvable config");
        println!("GOLDEN=0x{:016x}", state_hash(&w));
    }

    #[test]
    fn write_store_cursor_is_cursor_sensitive() {
        // Self-contained unit guard on the helper itself: assert the helper
        // mixes the cursor word at all — an empty map's helper-hash differs from
        // a bare FnvHasher (i.e. a cursor word WAS written).
        use crate::ids::SlotMap;
        let empty: SlotMap<()> = SlotMap::new();
        let mut with = FnvHasher::new();
        write_store_cursor(&mut with, &empty);
        assert_ne!(
            with.finish(),
            FnvHasher::new().finish(),
            "write_store_cursor must mix a cursor word into the hasher"
        );
    }

    #[test]
    fn fresh_hasher_is_deterministic() {
        assert_eq!(FnvHasher::new().finish(), FnvHasher::new().finish());
    }

    #[test]
    fn write_order_matters() {
        let mut a = FnvHasher::new();
        a.write_u64(1);
        a.write_u64(2);
        let mut b = FnvHasher::new();
        b.write_u64(2);
        b.write_u64(1);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn writing_changes_the_hash() {
        let base = FnvHasher::new().finish();
        let mut h = FnvHasher::new();
        h.write_u64(0);
        assert_ne!(base, h.finish(), "even writing 0 must move the hash");
    }

    /// GOLDEN HASH. This pins the canonical encoding of the HASH_FIELD_ORDER
    /// header + a zero-initialized single-body single-craft state slice (the
    /// same words Task 13's zero-init `state_hash` will reproduce). If this value
    /// changes, the canonical hash encoding changed — that is ONLY allowed
    /// alongside a HASH_FORMAT_VERSION bump and a HASH_FIELD_ORDER edit.
    #[test]
    fn golden_zero_state_hash() {
        let mut h = FnvHasher::new();
        // header (words 1-2) are already folded by new(); now the rest of a
        // minimal zero-init slice per HASH_FIELD_ORDER words 3..=13:
        h.write_u64(0); // 3. tick
        h.write_u64(0); // 4. body cursor
        h.write_u64(0); // 5. ship cursor
        // one body (slot 0, generation 0, mass 0.0):
        h.write_u64(0); // body slot
        h.write_u64(0); // body generation
        h.write_u64(0.0f64.to_bits()); // body mass
        // one craft (slot 0, generation 0; zero pos/vel/fuel; nav Idle=0; lod Player=0):
        h.write_u64(0); // craft slot
        h.write_u64(0); // craft generation
        h.write_u64(0.0f64.to_bits()); // pos.x
        h.write_u64(0.0f64.to_bits()); // pos.y
        h.write_u64(0.0f64.to_bits()); // pos.z
        h.write_u64(0.0f64.to_bits()); // vel.x
        h.write_u64(0.0f64.to_bits()); // vel.y
        h.write_u64(0.0f64.to_bits()); // vel.z
        h.write_u64(0.0f64.to_bits()); // fuel_mass
        h.write_u64(0); // nav discriminant (Idle)
        h.write_u64(0); // lod discriminant (Player)
        // economy words 16-19 for the one craft (Idle role, no cargo, 0 credits, no contract):
        h.write_u64(0); // 16. role.rank() (Idle)
        h.write_u64(0); // 17. cargo discriminant (None)
        h.write_u64(0); // 18. credits_micros
        h.write_u64(0); // 19. contract discriminant (None)
        // trophic words 25-26 for the one craft (0 risk_appetite, no pirate state):
        h.write_u64(0); // 25. risk_appetite
        h.write_u64(0); // 26. pirate discriminant (None)
        // store-level words 20-24, all empty (zero counters; each store cursor 0, no elements):
        h.write_u64(0); // 20a. econ.mined[Ore]
        h.write_u64(0); // 20b. econ.mined[Fuel]
        h.write_u64(0); // 20c. econ.consumed[Ore]
        h.write_u64(0); // 20d. econ.consumed[Fuel]
        h.write_u64(0); // 21. stations cursor
        h.write_u64(0); // 22. producers cursor
        h.write_u64(0); // 23. corporations cursor
        h.write_u64(0); // 24. contracts cursor
        assert_eq!(h.finish(), GOLDEN_ZERO_STATE_HASH);
    }
}
