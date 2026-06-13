use jumpgate_core::{
    record_run, replay_run, BaseSpec, BodyInit, Command, CommandKind, CraftId, CraftInit, Dt,
    EntityRef, EventKind, GuidanceParams, NavDest, OrbitalElements, Provenance, RunConfig,
    StateView, SubstepCfg, Target, Tick, Vec3, World,
};

/// A 2-body, 1-craft scenario big enough to exercise gravity + a thrust burn.
fn base_config() -> RunConfig {
    RunConfig {
        master_seed: 0x9E37_79B9_7F4A_7C15_u64, // arbitrary fixed seed (golden-ratio bits)
        dt: Dt::new(0.5),
        softening: 1e-4,
        substep_cfg: SubstepCfg { accel_ref: 1.0, max_substeps: 16 },
        ephemeris_window: 4096,
        bodies: vec![
            BodyInit {
                mass: 1.0,
                elements: OrbitalElements { a: 0.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            },
            BodyInit {
                mass: 3.0e-6,
                elements: OrbitalElements { a: 1.0, e: 0.0, i: 0.0, raan: 0.0, argp: 0.0, m0: 0.0 },
            },
        ],
        craft: vec![CraftInit {
            spec: BaseSpec {
                base_dry_mass: 1.0e-9,
                // Recalibrated for the §6 reset resolvability guard (Task 4): the old
                // 1e-6 gave a_max_empty = 1e-6/1e-9 = 1000, 1000*0.5^2 = 250 >> the
                // limit R/(2*k_brake) = 1e-4, an unbrakable (tunnelling) config. 1e-13
                // gives a_max_empty = 1e-4, 1e-4*0.5^2 = 2.5e-5 < 1e-4 -> passes the
                // guard with ~4x margin while still commanding a real (non-trivial)
                // thrust burn (ThrustApplied fires; the tick-1 corruption diverges).
                base_max_thrust: 1.0e-13,
                base_exhaust_velocity: 0.02,
                base_fuel_capacity: 1.0e-9,
                base_cargo_capacity: 5,
            },
            pos: Vec3::new(1.2, 0.0, 0.0),
            vel: Vec3::new(0.0, 0.9, 0.0),
            // Half a tank: a real multi-hundred-tick burn for the replay/corruption
            // tests. NOTE (eps re-bake, spec §4 item 1): 5.0e-10 sat BELOW the old
            // FUEL_EMPTY_EPS (1e-9, edge unarmed) and is ABOVE the new 1e-11.
            // Deliberately NOT lowered: this config has no contracts, so a
            // FuelEmpty event (reachable only near tick ~196 of a full-throttle
            // 200-tick run; burn/tick = 1e-13/0.02*0.5 = 2.5e-12) is state-inert
            // and fires identically in the record and replay arms. Lowering the
            // tank below 1e-11 would gut the burn these tests exist to record.
            fuel_mass: 5.0e-10,
            role: jumpgate_core::stores::CraftRole::Idle,
            scripted: true,
        }],
        guidance: GuidanceParams::default(),
        stations: vec![],
        producers: vec![],
        corporations: vec![],
        contracts: vec![],
        price_cfg: jumpgate_core::config::PriceCfg::default(),
        dispatch_cfg: jumpgate_core::config::DispatchCfg::default(),
        trophic: jumpgate_core::config::TrophicCfg::default(),
        shipyard: jumpgate_core::config::ShipyardCfg::default(),
        media: jumpgate_core::config::MediaCfg::default(),
        refuel: jumpgate_core::config::RefuelCfg::default(),
        goods: jumpgate_core::config::GoodsCfg::default(),
        exchange: jumpgate_core::config::ExchangeCfg::default(),
        arbitrage: jumpgate_core::config::ArbitrageCfg::default(),
    }
}

/// The single v1 craft is deterministically `CraftId { slot: 0, generation: 0 }`.
/// Discover it from a fresh reset rather than hardcoding, and assert the stable
/// value so a slot-map generation-convention drift (Task 4) fails HERE, loudly.
fn discover_craft_id() -> CraftId {
    let (world, _hash) = World::reset(base_config()).expect("resolvable config");
    let ids = world.craft_ids();
    assert_eq!(ids.len(), 1, "v1 scenario has exactly one craft");
    assert_eq!(
        ids[0],
        CraftId { slot: 0, generation: 0 },
        "first-minted craft must be slot 0 / generation 0 (slot-map convention from Task 4: a fresh slot starts at generation 0)"
    );
    ids[0]
}

/// Driver factory: command a destination on tick 0 ADDRESSED TO THE REAL CRAFT
/// (autopilot flies it, burning fuel), then issue no further commands.
/// Deterministic, no RNG, no clock. Routing to `Target::Entity(Craft(id))` (NOT
/// `Target::Sim`, which `ingest_commands` no-ops) is what makes the corruption
/// test causally meaningful.
fn transfer_driver(craft: CraftId) -> impl FnMut(Tick) -> Vec<Command> {
    move |tick: Tick| {
        if tick == Tick(0) {
            vec![Command {
                target: Target::Entity(EntityRef::Craft(craft)),
                kind: CommandKind::Destination {
                    dest: NavDest::Position(Vec3::new(-1.2, 0.0, 0.0)),
                    burn_budget: Some(0.01),
                },
            }]
        } else {
            Vec::new()
        }
    }
}

/// PRECONDITION: the recorded run must actually thrust. If it coasted (e.g. the
/// command was mis-routed to `Target::Sim`), the corruption test below would be
/// vacuous. We assert at least one `ThrustApplied` event by re-running the same
/// driver against a world we can read events from.
#[test]
fn recorded_run_actually_thrusts() {
    let craft = discover_craft_id();
    let mut driver = transfer_driver(craft);
    let (mut world, _hash) = World::reset(base_config()).expect("resolvable config");
    let mut saw_thrust = false;
    for _ in 0..50 {
        let pre = world.tick();
        let mut cmds = driver(pre);
        world.step(&mut cmds);
        if world
            .recent_events(pre)
            .iter()
            .any(|e| matches!(e.kind, EventKind::ThrustApplied { dv, .. } if dv > 0.0 && {
                // craft binding is implicit (single craft); dv>0 proves a burn
                let _ = e;
                true
            }))
        {
            saw_thrust = true;
        }
    }
    assert!(
        saw_thrust,
        "craft-targeted destination must produce a ThrustApplied event; \
         a Target::Sim no-op would make the corruption test vacuous"
    );
}

#[test]
fn record_then_replay_is_bit_identical() {
    let craft = discover_craft_id();
    let rec = record_run(base_config(), 200, transfer_driver(craft));
    assert_eq!(rec.hashes.len(), 200, "one hash per stepped tick");
    assert_eq!(replay_run(&rec), Ok(()), "faithful re-feed must reproduce every tick hash");
}

#[test]
fn thrust_mode_record_then_replay_is_bit_identical() {
    // Same harness as record_then_replay_is_bit_identical, but the recorded run
    // feeds a VARYING CommandKind::Thrust each tick (the held stick is re-aimed
    // every tick), exercising the DirectThrust nav fold + replay re-feed. Some
    // ticks exceed |v|=1 ((6/7)^2+(2/3)^2 > 1) so the autopilot's throttle clamp
    // is exercised under replay too; tick 0 is the zero vector (coast).
    let craft = discover_craft_id();
    let driver = move |tick: Tick| {
        let t = tick.0;
        vec![Command {
            target: Target::Entity(EntityRef::Craft(craft)),
            kind: CommandKind::Thrust {
                throttle_vec: Vec3::new(((t % 7) as f64) / 7.0, ((t % 3) as f64) / 3.0, 0.0),
            },
        }]
    };
    let rec = record_run(base_config(), 50, driver);
    assert_eq!(rec.hashes.len(), 50, "one hash per stepped tick");
    assert_eq!(
        replay_run(&rec),
        Ok(()),
        "faithful re-feed of a thrust-mode run must reproduce every tick hash"
    );
}

#[test]
fn corrupting_one_logged_command_reports_first_differing_tick() {
    let craft = discover_craft_id();
    let mut rec = record_run(base_config(), 200, transfer_driver(craft));
    // Find the logged tick-0 craft-targeted destination command and corrupt its
    // destination. Because the command sets a NavState that drives thrust on the
    // very next step, the post-step-tick-1 hash diverges.
    let idx = rec
        .log
        .entries
        .iter()
        .position(|(t, c)| {
            *t == Tick(0)
                && matches!(c.kind, CommandKind::Destination { .. })
                && matches!(c.target, Target::Entity(EntityRef::Craft(_)))
        })
        .expect("driver logged a tick-0 craft-targeted destination command");
    let corrupted = Command {
        target: Target::Entity(EntityRef::Craft(craft)),
        kind: CommandKind::Destination {
            dest: NavDest::Position(Vec3::new(99.0, 99.0, 99.0)), // different destination
            burn_budget: Some(0.01),
        },
    };
    // `ActionLog` keeps `entries` and `commands_flat` as index-aligned parallel
    // vecs (Task 10): `at()` derives its bounds from `entries` but serves content
    // from `commands_flat`, so the corruption must hit BOTH to reach `replay_run`
    // (which re-feeds via `rec.log.at(pre_tick)`) and to preserve the documented
    // lockstep invariant.
    rec.log.entries[idx].1 = corrupted;
    rec.log.commands_flat[idx] = corrupted;
    // Re-feeding the corrupted log thrusts toward a different point; the recorded
    // hashes are the originals. First divergence = the first post-step tick.
    assert_eq!(replay_run(&rec), Err(Tick(1)));
}

#[test]
fn config_hash_mismatch_is_rejected() {
    // Swap `rec.config` for a DIFFERENT config AFTER recording, WITHOUT updating
    // the stored `rec.config_hash`. replay_run compares the stored hash (taken at
    // record time) against a fresh hash of the now-swapped config; they disagree,
    // so the guard fires and returns Err(Tick(0)) BEFORE any tick is reproduced.
    // This is the non-tautological guard: it proves a recording's hashes are bound
    // to the exact config they were generated under.
    let craft = discover_craft_id();
    let mut rec = record_run(base_config(), 50, transfer_driver(craft));
    let differing = RunConfig {
        master_seed: rec.config.master_seed ^ 0xABCD,
        softening: rec.config.softening * 2.0, // also perturb the gravity kernel
        ..rec.config.clone()
    };
    rec.config = differing; // config_hash field intentionally left stale
    assert_eq!(
        replay_run(&rec),
        Err(Tick(0)),
        "stored config-hash must reject a recording whose config was swapped"
    );
}

#[test]
fn provenance_mismatch_is_rejected() {
    // Tier B is same-binary: a recording made under a DIFFERENT build (rustc /
    // edition / rand pin / hash-format) must not be silently replayed. The stamp
    // is a compile-time const, so within this binary record==replay provenance;
    // we mutate the stored field to a deliberately-wrong stamp to prove the guard
    // is reachable (non-tautological), mirroring `config_hash_mismatch_is_rejected`.
    // The guard runs FIRST, so it returns Err(Tick(0)) before any tick is stepped.
    let craft = discover_craft_id();
    let mut rec = record_run(base_config(), 50, transfer_driver(craft));
    let mut wrong = Provenance::current();
    wrong.hash_format_version = wrong.hash_format_version.wrapping_add(1);
    rec.provenance = wrong; // simulate a recording from a different build
    assert_eq!(
        replay_run(&rec),
        Err(Tick(0)),
        "a recording whose provenance differs from the replaying binary must be rejected"
    );
}
