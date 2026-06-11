export const meta = {
  name: 'first-econ-loop-phase2',
  description: 'First economic loop harness — Phase 2 (tasks 19-22): demand-deflation pricing, tick-gated reprice clock, hysteresis+staggered dispatch (A/B stability), phase-2 gate — via per-task TDD implement+verify+repair',
  phases: [
    { title: 'T19 update_prices' },
    { title: 'T20 reprice clock' },
    { title: 'T21 hysteresis+stagger' },
    { title: 'T22 phase-2 gate' },
  ],
}

// ============================ SHARED GROUND TRUTH ============================
// Phase 0 + Phase 1 (a+b) are DONE and committed. HEAD = 6ae11ac. Working tree clean
// except standing untracked harness files (.claude/ .gitignore CLAUDE.md AGENTS.md
// .mcp.json .filigree.conf) — NEVER stage those.
//
// THE FIVE DETERMINISM ANCHORS (Phase 2 MUST move ZERO of them):
//   config.rs  GOLDEN_CONFIG_HASH = 0xf4bc_85c3_7cb6_8a6b
//   hash.rs    HASH_FORMAT_VERSION: u32 = 2
//   hash.rs    GOLDEN_ZERO_STATE_HASH = 0x65d7_af3b_9a8a_8276
//   hash.rs    zero-world state golden = 0x64dd_5078_a3e0_5886
//   provenance.rs hash_fmt_v=2
//
// PHASE 2 ADDS ZERO CONFIG FIELDS — every field you need ALREADY EXISTS (verified):
//  * config.rs PriceCfg { base_micros:[i64;2], cap:[i64;2], slope_milli:i64,
//    reprice_interval:u32 } + Default { base_micros:[0,0], cap:[1,1], slope_milli:1800,
//    reprice_interval:1 }. slope_milli is k*1000 (1800 == 1.8).
//  * config.rs DispatchCfg { demand_low:i64, demand_high:i64, stagger_period:u32,
//    contract_reward_micros:i64, contract_qty:u32 } + Default (inert: no auto-post).
//    demand_high (hysteresis upper edge) and stagger_period (T21) ALREADY EXIST.
//  * RunConfig already carries price_cfg + dispatch_cfg.
//  ADDING ANY config field is a HARD STOP — it silently re-pins GOLDEN_CONFIG_HASH. You
//  need none. If you think you do, you are wrong; reread PriceCfg/DispatchCfg above.
//
// EXISTING PRIMITIVES (landed Phase 0/1 — DO NOT redefine/re-pin/re-add):
//  * economy.rs StationStore: stock:Vec<[i64;2]>, price_micros:Vec<[i64;2]>. push(...).
//    station price_micros IS folded into state_hash (hash.rs ~line 304, plus a
//    field-sensitivity test "station price"). Writing price_micros is therefore
//    determinism-safe (recomputed, not pinned). The zero/empty-world goldens have NO
//    stations, so update_prices is a no-op there -> goldens unaffected.
//  * economy.rs run_producers(stations,producers,counters,tick,events) — two indep
//    if-let legs; input-only recipe = demand sink (bumps consumed). resolve_contracts(...)
//    accept/escrow/load/dispatch. resolve_deliveries(...) settle on Arrival.
//    resolve_failures(...) FuelEmpty->Failed+refund+cargo-loss->consumed.
//    run_scripted_dispatch(contracts,stations,ships,&DispatchCfg,tick,events) — REPOST
//    (terminal+stock<demand_low, clones route of latest contract for that route, one
//    rep per route via later_dup) + ASSIGN (Idle hauler sorted -> lowest-ContractId
//    Offered, mirrors ingest by setting craft.contract+role). T21 refines THIS fn.
//  * EconCounters{mined:[i64;2],consumed:[i64;2]}. Resource{Ore=0,Fuel=1}, .index(), N_RESOURCES=2.
//  * EventKind (contract.rs): ...PriceUpdate{station,resource,price_micros}, ContractOffered,
//    ContractAccepted, ContractFulfilled, FuelEmpty, Arrival, ThrustApplied... EventKind
//    NOT hashed -> emitting is hash-neutral. ContractStatus{Offered,Accepted,CargoLoaded,
//    InTransit,Delivered,Completed,Failed}. CraftRole{Idle,Hauler}.
//  * world.rs World::step STAGE ORDER (locate by COMMENTS, not line numbers):
//      (1) ingest_commands -> run_scripted_dispatch + run_producers + resolve_contracts
//          (pre-physics) -> (2) physics LOD loop -> (3) detect_boundary_events
//          (FuelEmpty/Arrival) -> (3b) resolve_deliveries -> (3c) resolve_failures
//          -> (4) copy-forward prev_* -> (5) tick++.
//      (Exact placement of the scripted/producer/contract stages: read the file. The
//       reprice call goes after resolve_contracts and before copy-forward — see T20.)
//  * world.rs test helpers (reuse/extend): two_body_contract_fixture(),
//    two_body_starved_contract_fixture(), full_stage1_self_running_fixture() (miner+
//    refiner at A, Fuel sink at B, 1 corp, 1 Idle hauler, seeded ContractInit). The
//    assert_resource_identity(world,&initial) helper and the credit-identity pattern
//    (Σtreasury+Σcredits+Σescrow) already exist in world.rs tests — reuse them.
//
// EXISTING FIXTURES USE PriceCfg::default() (base_micros=[0,0]) so once update_prices is
// wired in (T20), their prices compute to 0 -> no state change -> Phase 0/1 tests AND
// goldens stay green. CONFIRM the FULL suite is green after T20, not just new tests.
//
// PURE-INTEGER PRICING — DO NOT introduce a float or a `to_micros` helper. Formula
// (per station row, per resource r), with cap[r] > 0:
//   let s = stock[row][r].max(0).min(cap[r]);
//   price_micros = (base_micros[r] * (2000 - s * slope_milli / cap[r]) / 1000).max(0);
// At stock 0 -> base*2; at stock cap -> base*(2 - slope); monotone non-increasing in stock.
// Integer division truncates identically in both runs (the T22 cross-world hash test guards it).
//
// THE EVENT-LIFT BORROW PATTERN (copy from stage 3b/3c): collect (craft,...) tuples out
// of self.events.since(next) into a Vec FIRST (drops the immutable borrow), THEN mutate.
//
// ============================ NON-NEGOTIABLE LAWS ============================
const LAWS = `
LAW 1 — LOCATE BY SYMBOL, never by plan/comment line numbers (stale). grep for fn/struct/comment.
LAW 2 — STRICT TDD: write the failing test FIRST; RUN it and SEE the SPECIFIC red (git stash is operator-blocked — to confirm red, temporarily disable the new impl call / new branch, run, then restore); only then implement; run green.
LAW 3 — COMMIT exactly the named files with \`git add <explicit paths>\` (NEVER \`git add -A\`/\`.\`). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Message MUST end with the trailer line:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
LAW 4 — PHASE 2 MOVES ZERO GOLDENS and ADDS ZERO CONFIG FIELDS. Do NOT touch hash.rs/config.rs/provenance.rs golden constants. Do NOT add any RunConfig/PriceCfg/DispatchCfg field (HARD STOP — every needed field exists). Do NOT add a hashed-state field. Do NOT change struct shapes that feed the hash.
LAW 5 — Rust 2024 reserves \`gen\`: name loop vars \`generation\`, never \`gen\`.
LAW 6 — NEVER remove/despawn from any store (status-only lifecycle). Stores only grow.
LAW 7 — All selection/iteration in sorted dense-id order (0..len, slot==row). NO RNG, NO HashMap iteration order, NO float keys. PURE INTEGER pricing (no to_micros, no f64).
LAW 8 — CROSS-CRATE CLIPPY: the gate is \`cargo clippy --all-targets -- -D warnings\` which compiles the SIBLING crate jumpgate-py too. ALWAYS run --all-targets, not just \`-p jumpgate-core\`. (Phase 2 adds no enum variants, so no py exhaustiveness break is expected — but run it.)
LAW 9 — Report ONLY what you actually ran. Paste the real \`test result:\` line and the real clippy outcome. Subagents have fabricated gate claims before; the main loop re-verifies everything.
`

const VERIFY_SCHEMA = {
  type: 'object', additionalProperties: false,
  required: ['passed', 'suite_result', 'clippy_result', 'goldens_unmoved', 'commit_present', 'problems'],
  properties: {
    passed: { type: 'boolean', description: 'true ONLY if suite green AND clippy --all-targets clean AND goldens unmoved AND commit present (trailer, named files only, no forbidden files)' },
    suite_result: { type: 'string', description: 'verbatim lib `test result:` line + named new test(s) present/passing + read the BODY (substantive vs hollow)' },
    clippy_result: { type: 'string', description: 'verbatim outcome of cargo clippy --all-targets -- -D warnings' },
    goldens_unmoved: { type: 'boolean', description: 'all five anchors still at Phase-0 values; hash.rs/config.rs/provenance.rs NOT in HEAD diff' },
    commit_present: { type: 'boolean', description: 'HEAD is this task commit, trailer present, only named files, no forbidden files' },
    problems: { type: 'string', description: 'specific defects, or "None"' },
  },
}
const IMPL_SCHEMA = {
  type: 'object', additionalProperties: false,
  required: ['done', 'commit_sha', 'new_tests', 'deviations', 'blocker'],
  properties: {
    done: { type: 'boolean' },
    commit_sha: { type: 'string' },
    new_tests: { type: 'string' },
    deviations: { type: 'string' },
    blocker: { type: 'string', description: 'a blocker needing the main loop, or "none". A finding that damping does not help (T21) is NOT a blocker — report it honestly and still commit a true test.' },
  },
}

const SPECS = [
  {
    task: 19,
    phase: 'T19 update_prices',
    files: 'crates/jumpgate-core/src/economy.rs, crates/jumpgate-core/src/world.rs',
    title: 'feat(economy): update_prices linear deflation (integer micro-price)',
    spec: `Implement linear demand-deflation pricing as a PURE-INTEGER stage (no float, no to_micros).

STEP 1 (failing test, world.rs or economy.rs tests): a station whose Fuel stock varies; with a chosen PriceCfg (e.g. base_micros[Fuel]=100_000, cap[Fuel]=10, slope_milli=1800), assert EXACT integer price_micros at: stock 0 -> base*2 (200_000); stock cap -> base*(2 - 1.8) = base*0.2 (20_000); and monotone NON-INCREASING across a few intermediate stocks. Use the formula below.

STEP 2: run, see it fail.

STEP 3 (implement) economy.rs: pub fn update_prices(stations: &mut StationStore, price_cfg: &PriceCfg, tick: Tick, events: &mut EventStream). For each station row (0..len, sorted) and each resource r (0..N_RESOURCES) with cap[r] > 0:
    let s = stations.stock[row][r].max(0).min(price_cfg.cap[r]);
    let p = (price_cfg.base_micros[r] * (2000 - s * price_cfg.slope_milli / price_cfg.cap[r]) / 1000).max(0);
  If p != stations.price_micros[row][r]: write it AND emit EventKind::PriceUpdate{station, resource: r, price_micros: p} (build the StationId from ids.id_at(row)). Skip resources with cap[r] == 0 (avoid div-by-zero; leave price unchanged). Deterministic, sorted-id, integer-only. Do NOT wire it into World::step yet (that is T20) — but it is fine to import it.

STEP 4: run green + \`cargo clippy --all-targets -- -D warnings\`.

STEP 5: commit economy.rs (+ world.rs only if the test lives there), subject exactly: "feat(economy): update_prices linear deflation (integer micro-price)" + trailer.`,
  },
  {
    task: 20,
    phase: 'T20 reprice clock',
    files: 'crates/jumpgate-core/src/world.rs',
    title: 'feat(economy): tick-gated reprice clock invoked from World::step',
    spec: `Wire update_prices into World::step on a tick-gated clock. ADD NO CONFIG FIELD — reprice_interval already exists in PriceCfg (LAW 4).

STEP 1 (failing test, world.rs tests): build a fixture with a station that has base_micros[Fuel] > 0 and a producer that changes its stock over time, with PriceCfg.reprice_interval = 4. Step ~12 ticks; assert station price_micros only CHANGES on ticks that are multiples of 4 (capture price after each step; it is constant between reprice ticks even as stock moves, and updates at t=4,8,12). Add a determinism leg: two worlds, same config, identical empty inputs -> identical price sequence.

STEP 2: run, see it fail.

STEP 3 (implement) world.rs World::step: after the resolve_contracts stage and BEFORE the (4) copy-forward stage, call update_prices GUARDED:
    if self.config.price_cfg.reprice_interval > 0 && next.0 % (self.config.price_cfg.reprice_interval as u64) == 0 {
        // event-lift borrow pattern if needed; update_prices takes &mut self.stations + &mut self.events
        crate::economy::update_prices(&mut self.stations, &self.config.price_cfg, next, &mut self.events);
    }
  (The reprice_interval>0 guard prevents a modulo-by-zero panic if a fixture sets interval 0. NOT lazy-on-read — repricing happens in the step path.) Locate the exact insertion point and the config/stations/events field names by reading the file.

STEP 4: run green. RUN THE FULL SUITE \`cargo test -p jumpgate-core\` — existing fixtures use PriceCfg::default() (base_micros=0) so their prices stay 0 and ALL Phase 0/1 tests + goldens must remain green. Then \`cargo clippy --all-targets -- -D warnings\`.

STEP 5: commit world.rs only, subject exactly: "feat(economy): tick-gated reprice clock invoked from World::step" + trailer.`,
  },
  {
    task: 21,
    phase: 'T21 hysteresis+stagger',
    files: 'crates/jumpgate-core/src/economy.rs, crates/jumpgate-core/src/world.rs',
    title: 'feat(economy): hysteresis deadband + staggered dispatch — Stage-2 stable',
    spec: `Add hysteresis + staggered dispatch to run_scripted_dispatch and PROVE they stabilise the closed loop with a COMPARATIVE A/B test (the only honest design — an absolute threshold is fudgeable and a non-growth check is VACUOUS against a constant-amplitude limit cycle). Use the EXISTING DispatchCfg.demand_high and DispatchCfg.stagger_period (LAW 4 — add nothing).

STEP 1 (failing test, world.rs tests): build a Stage-2 fixture with base_micros[Fuel] > 0 (so price actually moves), a miner+refiner supply at A, a Fuel demand SINK at B, one corp, and MULTIPLE Idle haulers (>= 3, so staggered dispatch is meaningful), seeded with a ContractInit route template. Write ONE test body, parameterised by config, run on TWO configs that differ ONLY in the new knobs:
   - undamped baseline: dispatch_cfg.demand_high == demand_low (deadband collapsed) AND stagger_period == 1 (no stagger).
   - damped: demand_high > demand_low (a real deadband) AND stagger_period > 1.
  For each config, drive the closed loop 1000 ticks (NO external commands), recording station-B Fuel stock (or its price) each tick; compute band = max - min over the LAST 200 ticks. ASSERT:
   (i) NON-VACUITY FLOOR: undamped_band > 0 by a real margin (the undamped loop genuinely oscillates — if it does not, the dynamics are already self-damped; REPORT that as a finding rather than manufacturing oscillation, and make the test assert the true relationship you measured).
   (ii) DAMPING WORKS: damped_band is meaningfully smaller than undamped_band (e.g. damped_band * 2 <= undamped_band, or a factor you justify from the measured numbers).
  Also assert staggered dispatch does NOT fire all haulers on the same tick (e.g. in the damped run, accepts/role-flips are spread across >1 tick).

STEP 2: run, see it fail. PRE-IMPLEMENTATION both configs ignore the new knobs -> identical behaviour -> (ii) damped < undamped FAILS. That is the correct red.

STEP 3 (implement) economy.rs run_scripted_dispatch:
   (a) HYSTERESIS: gate REPOST with the deadband — post when destination stock < demand_low, and do NOT resume posting for that route until stock has recovered to >= demand_high (use the available store state to express the deadband deterministically; e.g. only post when stock < demand_low, and rely on demand_high to widen the "satisfied" band so a route is not re-posted while stock sits in [demand_low, demand_high)). Keep it integer, sorted-id, stateless-or-store-derived (no new field).
   (b) STAGGERED DISPATCH: an Idle hauler in dense row s may ACCEPT only on ticks where tick % stagger_period == (s as u64) % stagger_period (stagger_period==1 -> every tick = no stagger). This requires run_scripted_dispatch to know the tick (it already takes tick) and the hauler's dense row (the loop index).
  Deterministic, integer, sorted-id, no RNG, no new config/hashed field.

STEP 4: run green (both A/B legs) + FULL suite \`cargo test -p jumpgate-core\` + \`cargo clippy --all-targets -- -D warnings\`.

STEP 5: commit economy.rs (+ world.rs if the test lives there), subject exactly: "feat(economy): hysteresis deadband + staggered dispatch — Stage-2 stable" + trailer.`,
  },
  {
    task: 22,
    phase: 'T22 phase-2 gate',
    files: 'crates/jumpgate-core/src/world.rs (tests)',
    title: 'test(economy): phase-2 gate — full demand-deflation harness conserves + replays + stable',
    spec: `The PHASE-2 GATE. Do NOT close any issue (the main loop does that after re-verify). Tests only.

STEP 1 (conservation over Stage-2): using a full Stage-2 fixture (base_micros>0, supply+sink+multiple haulers, repricing ON), drive 1000 ticks and call BOTH the resource accounting identity (assert_resource_identity, reuse the existing helper) AND the global credit identity (Σtreasury+Σcredits+Σescrow == initial) EVERY tick. Add per-leg non-vacuity guards (mined Ore/Fuel > 0, consumed Fuel > 0) so all legs actually fire. Run -> PASS.

STEP 2 (replay determinism): two worlds from the SAME Stage-2 config + identical empty inputs; assert bit-identical state_hash tick-by-tick over 1000 ticks (with an evolves-non-vacuity guard). Run -> PASS.

STEP 3: confirm the T21 stability A/B test is green; run \`cargo test -p jumpgate-core\` AND \`cargo clippy --all-targets -- -D warnings\` -> all green.

STEP 4: commit the test file only, subject exactly: "test(economy): phase-2 gate — full demand-deflation harness conserves + replays + stable" + trailer.

CRITICAL: if either identity FAILS, the bug is in T19-T21 (pricing must NOT move resource/credit quantities — PriceUpdate is a price write + event only, it touches NO stock/treasury/escrow/cargo). Report the exact mismatch as a blocker; do NOT weaken the assertion.`,
  },
]

// ===================== SEQUENTIAL TDD: implement -> verify -> (repair) =====================
const results = []
for (const s of SPECS) {
  phase(s.phase)
  log(`Task ${s.task}: implementing`)

  const impl = await agent(
    `You are implementing Task ${s.task} of the Jumpgate first-economic-loop harness (Phase 2 — demand-deflation pricing), strict TDD.

${LAWS}

FILES YOU MAY TOUCH: ${s.files}

TASK ${s.task} SPEC:
${s.spec}

Work in the live repo at /home/john/jumpgate. Read the relevant existing code first (economy.rs StationStore/run_producers/run_scripted_dispatch/resolve_contracts, world.rs World::step + the existing test fixtures and assert_resource_identity helper, config.rs PriceCfg/DispatchCfg) to match signatures and style exactly. Follow the five TDD steps in order. Commit at the end with the exact subject and trailer. Return the structured result.`,
    { label: `impl:T${s.task}`, phase: s.phase, schema: IMPL_SCHEMA, agentType: 'general-purpose' },
  )

  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${s.task} BLOCKED at impl: ${impl ? impl.blocker : 'agent returned null'}`)
    results.push({ task: s.task, impl, verify: null, halted: true })
    break
  }

  log(`Task ${s.task}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `You are the INDEPENDENT VERIFIER for Task ${s.task} (Phase 2) of the Jumpgate harness. The implementer claims commit ${impl.commit_sha} is done. TRUST NOTHING — re-run everything yourself in /home/john/jumpgate.

${LAWS}

Run and report VERBATIM:
1. \`cargo test -p jumpgate-core\` — capture the lib \`test result:\` line; confirm the task's new test(s) are present and passing, and READ THE BODY (substantive vs hollow). New tests claimed: ${impl.new_tests}
2. \`cargo clippy --all-targets -- -D warnings\` — must be clean (compiles jumpgate-py too).
3. GOLDENS UNMOVED — grep that all five anchors hold (config 0xf4bc_85c3_7cb6_8a6b, HASH_FORMAT_VERSION=2, GOLDEN_ZERO_STATE_HASH=0x65d7_af3b_9a8a_8276, zero-world 0x64dd_5078_a3e0_5886, provenance hash_fmt_v=2) and that HEAD did NOT touch hash.rs/config.rs/provenance.rs and did NOT add any PriceCfg/DispatchCfg/RunConfig field.
4. COMMIT — \`git show --stat HEAD\`: subject matches "${s.title}", correct trailer, ONLY these files changed: ${s.files}, NO forbidden files staged.
5. TASK-SPECIFIC: ${s.task === 20 ? 'confirm the FULL Phase 0/1 suite still passes (existing default-PriceCfg fixtures keep price 0) and the reprice call is guarded reprice_interval>0.' : s.task === 21 ? 'confirm the stability test is a TRUE A/B comparison (undamped vs damped configs differing ONLY in demand_high/stagger_period), with a non-vacuity floor on undamped_band and damped_band meaningfully smaller — NOT an absolute magic threshold and NOT a vacuous non-growth check. Verify the new knobs are read from existing DispatchCfg (no new field).' : s.task === 22 ? 'confirm BOTH identities (resource AND credit) are checked every tick over 1000 ticks with per-leg non-vacuity guards, and the determinism test compares two worlds tick-by-tick; confirm NO issue was closed by the implementer.' : 'confirm exact integer price values at stock 0 (base*2) and stock cap (base*0.2) and monotonicity; confirm pure-integer (no f64/to_micros).'}

passed=true ONLY if all hold. Return the structured verdict with the real command outputs.`,
    { label: `verify:T${s.task}`, phase: s.phase, schema: VERIFY_SCHEMA, agentType: 'general-purpose' },
  )

  if (!verify || !verify.passed) {
    log(`Task ${s.task} verify FAILED: ${verify ? verify.problems : 'null'} — one repair attempt`)
    const repair = await agent(
      `Task ${s.task} (Phase 2) FAILED independent verification. Fix it in /home/john/jumpgate, keeping the single-cause subject "${s.title}" + trailer (a separate fix(py): commit is correct only for a cross-crate clippy exhaustiveness break — see LAW 8).

${LAWS}

FILES: ${s.files}

REPORTED PROBLEMS:
${verify ? verify.problems : 'verifier returned null'}
${verify ? 'suite=' + verify.suite_result + ' | clippy=' + verify.clippy_result + ' | goldens_unmoved=' + verify.goldens_unmoved + ' | commit_present=' + verify.commit_present : ''}

If the failure is that T21 damping does NOT actually reduce the band (the loop is already self-damping or your damping is ineffective), DO NOT fake it: report it as a blocker with the measured undamped vs damped numbers so the main loop can adjudicate. After fixing, RE-RUN \`cargo test -p jumpgate-core\` and \`cargo clippy --all-targets -- -D warnings\` and report real results. Return the impl-shaped structured result.`,
      { label: `repair:T${s.task}`, phase: s.phase, schema: IMPL_SCHEMA, agentType: 'general-purpose' },
    )
    log(`Task ${s.task} repair done (${repair ? repair.commit_sha : 'null'}, blocker=${repair ? repair.blocker : 'null'}); re-verifying`)
    if (repair && repair.blocker && repair.blocker !== 'none') {
      results.push({ task: s.task, impl, repair, verify, halted: true })
      log(`Task ${s.task} repair reported BLOCKER — HALTING for main-loop adjudication`)
      break
    }
    verify = await agent(
      `RE-VERIFY Task ${s.task} (Phase 2) after a repair, in /home/john/jumpgate. Run \`cargo test -p jumpgate-core\`, \`cargo clippy --all-targets -- -D warnings\`, confirm five goldens unmoved + no config field added, confirm HEAD commit(s) clean with trailer and no forbidden files. ${s.task === 21 ? 'Re-confirm the A/B stability test is a true comparison (non-vacuity floor + damped meaningfully < undamped).' : ''} Report verbatim outputs.`,
      { label: `reverify:T${s.task}`, phase: s.phase, schema: VERIFY_SCHEMA, agentType: 'general-purpose' },
    )
    results.push({ task: s.task, impl, repair, verify, halted: !verify || !verify.passed })
    if (!verify || !verify.passed) {
      log(`Task ${s.task} STILL FAILING after repair — HALTING for main-loop intervention`)
      break
    }
  } else {
    log(`Task ${s.task} GREEN — ${verify.suite_result}`)
    results.push({ task: s.task, impl, verify, halted: false })
  }
}

return { phase: '2', results }
