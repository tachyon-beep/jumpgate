export const meta = {
  name: 'first-econ-loop-phase1b',
  description: 'First economic loop harness — Phase 1 part B, tasks 16-18 (Failed path, scripted dispatch+repost, Phase-1 accounting+determinism gate) via per-task TDD implement+verify+repair',
  phases: [
    { title: 'T16 failure-path' },
    { title: 'T17 dispatch+repost' },
    { title: 'T18 phase-1 gate' },
  ],
}

// ============================ SHARED GROUND TRUTH ============================
// Phase 0 + Phase 1a are DONE and committed. HEAD = db01b2f. Working tree clean
// except standing untracked harness files (.claude/ .gitignore CLAUDE.md AGENTS.md
// .mcp.json .filigree.conf) — NEVER stage those.
//
// THE FIVE DETERMINISM ANCHORS (Phase 1b MUST move ZERO of them):
//   config.rs:398   GOLDEN_CONFIG_HASH = 0xf4bc_85c3_7cb6_8a6b
//   hash.rs:66      HASH_FORMAT_VERSION: u32 = 2
//   hash.rs:72      GOLDEN_ZERO_STATE_HASH = 0x65d7_af3b_9a8a_8276
//   hash.rs:693     zero-world state golden = 0x64dd_5078_a3e0_5886
//   provenance.rs   hash_fmt_v=2
//
// EXISTING PRIMITIVES (all landed Phase 0/1a — DO NOT redefine, re-pin, or re-add):
//  * economy.rs:241 run_producers(stations,producers,counters,tick,events) — TWO
//    independent `if let Some(...)` legs: input leg debits stock + bumps
//    counters.consumed[r_in]; output leg credits stock + bumps counters.mined[r_out]
//    + emits Production. An INPUT-ONLY recipe (input=Some, output=None) therefore
//    works for free: it debits stock and bumps consumed, emits nothing. THIS IS THE
//    DEMAND SINK — a Producer with input=Some((Fuel,k)), output=None. Verified.
//  * economy.rs:315 resolve_contracts(...) — accept/escrow/load/dispatch lifecycle,
//    one transition-group per tick (match on status). Loading is a TRANSFER (no
//    counter). Sorted-ContractId (0..len) order, no RNG.
//  * economy.rs resolve_deliveries(contracts,stations,ships,&arrivals,tick,events) —
//    settle InTransit on Arrival: unload cargo->station stock, pay escrow->craft
//    credits, zero escrow, clear cargo/contract/role->Idle, emit ContractFulfilled.
//  * EconCounters { mined:[i64;2], consumed:[i64;2] } (economy.rs). Resource{Ore=0,Fuel=1}, N_RESOURCES=2, .index().
//  * ContractStore NEVER shrinks (status-only lifecycle). ContractStatus
//    { Offered, Accepted, CargoLoaded, InTransit, Delivered, Completed, Failed } + rank().
//    Fields: status[], escrow_micros[] (i64), hauler[] (Option<CraftId>), corp[],
//    resource[], qty[], from_station[], to_station[], reward_micros[]. ContractStore::push exists.
//  * CraftStore columns: role[]:CraftRole{Idle,Hauler}, cargo[]:Option<(Resource,u32)>,
//    credits_micros[]:i64, contract[]:Option<ContractId>, fuel_mass[]:f64 (PROPELLANT,
//    NOT economy Resource::Fuel), nav[]:NavState.
//  * EventKind (contract.rs:45): Production, Trade, PriceUpdate, ContractOffered{contract},
//    ContractAccepted{contract,hauler}, ContractFulfilled{contract,hauler},
//    FuelEmpty{craft} (contract.rs:50, emitted by detect_boundary_events), Arrival{craft,dest},
//    ThrustApplied{craft,dv}. EventKind is NOT hashed -> adding/using variants is hash-neutral.
//  * CommandKind: Destination, AcceptContract{contract}, SetRole{...}. NOT hashed.
//  * config.rs DispatchCfg { demand_low, demand_high, stagger_period, contract_reward_micros,
//    contract_qty } + Default — ALREADY EXISTS from Task 5. PriceCfg also exists. READ from
//    these; ADD NOTHING. RunConfig already has stations/producers/corporations/contracts/
//    price_cfg/dispatch_cfg + ContractInit/StationInit/ProducerInit/CorporationInit.
//  * world.rs World::step STAGE ORDER (locate by these comments, NOT line numbers):
//      (1) ingest_commands  ->  run_producers + resolve_contracts (post-ingest, pre-physics)
//      (2) physics LOD loop  ->  (3) detect_boundary_events (FuelEmpty/Arrival fire here)
//      (3b) resolve_deliveries (lifts Arrival events)  ->  (4) copy-forward prev_*  ->  (5) tick++
//  * world.rs test helper two_body_contract_fixture() (one corp, one Idle craft co-located
//    at body0=origin star mass 1e-9, body1 at a=0.3 AU, one seeded Fuel contract A->B,
//    station A stock [0,10]). Reuse/extend as a base.
//
// THE EVENT-LIFT BORROW PATTERN (mandatory, copy from stage 3b): to act on this tick's
// events you must FIRST collect the (craft,...) tuples out of self.events.since(next)
// into a Vec (drops the immutable borrow), THEN mutate stores. Passing &mut self.events
// alongside &self.ships is E0502.
//
// ============================ NON-NEGOTIABLE LAWS ============================
const LAWS = `
LAW 1 — LOCATE BY SYMBOL, never by the plan's line numbers (they are stale). grep for fn/struct/comment.
LAW 2 — STRICT TDD: write the failing test FIRST; RUN it and SEE it fail (git stash is operator-blocked — to confirm red, temporarily comment out the new impl call or assert the pre-impl value in a scratch run, then restore); only then implement; run green.
LAW 3 — COMMIT exactly the named files with \`git add <explicit paths>\` (NEVER \`git add -A\`/\`.\`). NEVER stage .claude/ .gitignore CLAUDE.md AGENTS.md .mcp.json .filigree.conf. Commit message MUST end with the trailer line:
    Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
LAW 4 — PHASE 1b MOVES ZERO GOLDENS. Do NOT touch hash.rs/config.rs/provenance.rs golden constants. Do NOT add any RunConfig/PriceCfg/DispatchCfg field (a new config field silently re-pins GOLDEN_CONFIG_HASH and is a HARD STOP — if you think you need one, you are wrong; use the existing DispatchCfg). Do NOT add a hashed state field. EventKind/CommandKind additions are hash-neutral but none are needed here.
LAW 5 — Rust 2024 reserves \`gen\`: name loop vars \`generation\`, never \`gen\`.
LAW 6 — NEVER remove/despawn from any store (status-only lifecycle). ContractStore only grows.
LAW 7 — All selection/iteration in sorted dense-id order (0..len, slot==row). NO RNG, NO HashMap iteration order, NO float keys.
LAW 8 — CROSS-CRATE CLIPPY (this bit Task 10): the gate is \`cargo clippy --all-targets -- -D warnings\` which compiles the SIBLING crate jumpgate-py too. If you add/alter any enum variant that jumpgate-py matches on exhaustively, py won't compile (E0004). Phase 1b should add NO enum variants, but ALWAYS run the --all-targets clippy gate, not just \`cargo test -p jumpgate-core\`.
LAW 9 — Report ONLY what you actually ran. Paste the real \`test result:\` line and the real clippy outcome. Do not claim a command you did not execute (subagents have fabricated gate claims before; the main loop re-verifies everything).
`

const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['passed', 'suite_result', 'clippy_result', 'goldens_unmoved', 'commit_present', 'problems'],
  properties: {
    passed: { type: 'boolean', description: 'true ONLY if suite green AND clippy --all-targets clean AND goldens unmoved AND commit present with trailer and no forbidden files' },
    suite_result: { type: 'string', description: 'the verbatim lib `test result:` line + the named new test(s) present/passing' },
    clippy_result: { type: 'string', description: 'verbatim outcome of cargo clippy --all-targets -- -D warnings' },
    goldens_unmoved: { type: 'boolean', description: 'all five anchors still at Phase-0 values (grep config.rs/hash.rs/provenance.rs)' },
    commit_present: { type: 'boolean', description: 'HEAD is this task commit, correct trailer, only the named files, no forbidden files staged' },
    problems: { type: 'string', description: 'specific defects, or "None"' },
  },
}
const IMPL_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  required: ['done', 'commit_sha', 'new_tests', 'deviations', 'blocker'],
  properties: {
    done: { type: 'boolean' },
    commit_sha: { type: 'string' },
    new_tests: { type: 'string' },
    deviations: { type: 'string', description: 'every deviation from the plan, stated honestly' },
    blocker: { type: 'string', description: 'a blocker that needs the main loop, or "none"' },
  },
}

const SPECS = [
  {
    task: 16,
    phase: 'T16 failure-path',
    files: 'crates/jumpgate-core/src/world.rs, crates/jumpgate-core/src/economy.rs',
    title: 'feat(economy): contract Failed path on FuelEmpty — escrow refund, accounted cargo loss',
    spec: `Implement the contract FAILURE path: a hauler that runs out of propellant mid-transit fails its contract, the escrow is refunded to the corp, and the lost cargo is accounted so the resource identity stays exact.

STEP 1 (failing test, in world.rs tests): build a STARVED fixture — base it on two_body_contract_fixture() but give the craft so little fuel_mass that it CANNOT reach body 1 (e.g. set cfg.craft[0].fuel_mass to a tiny value, OR push body 1 farther out, so the propellant empties mid-transit). The craft must still be able to ACCEPT + LOAD at A first (loading pulls economy Fuel from station A stock — that is cargo, independent of propellant fuel_mass). Accept the contract (one step), assert it reaches CargoLoaded/InTransit, then step (no commands) until a FuelEmpty event fires while the contract is InTransit. ASSERT: contract status -> Failed; escrow_micros refunded to the corp treasury (corp treasury back to its pre-accept value; escrow_micros[k]==0); craft contract handle cleared, role back to Idle; the global CREDIT identity Σtreasury+Σcredits+Σescrow == initial still holds; and econ.consumed[Fuel] increased by the lost cargo qty (the cargo-loss accounting leg).

STEP 2: run, see it fail (per LAW 2).

STEP 3 (implement): add a new stage (3c) in World::step IMMEDIATELY AFTER (3b) resolve_deliveries, OR fold into a small economy.rs fn called there. Use the EVENT-LIFT BORROW PATTERN: collect this tick's FuelEmpty{craft} craft-ids from self.events.since(next) into a Vec FIRST, then mutate. For each such craft that holds an InTransit contract (find its contract via ships.contract / the contract's hauler binding, sorted order): set status -> Failed; refund escrow_micros back into the owning corp's treasury_micros; zero escrow_micros; if the craft has cargo Some((r,q)), add q to econ.consumed[r.index()] (cargo is LOST — this leg keeps the resource identity exact because the loaded cargo was already debited from origin stock at load) and set cargo None; clear ships.contract -> None and role -> Idle. Document "v1: cargo is lost on Failed" in a doc-comment. Do NOT remove anything from any store (LAW 6).

STEP 4: run green. Also run \`cargo clippy --all-targets -- -D warnings\` (LAW 8).

STEP 5: commit world.rs + economy.rs only, message subject exactly: "feat(economy): contract Failed path on FuelEmpty — escrow refund, accounted cargo loss" + trailer.`,
  },
  {
    task: 17,
    phase: 'T17 dispatch+repost',
    files: 'crates/jumpgate-core/src/economy.rs, crates/jumpgate-core/src/world.rs',
    title: 'feat(economy): scripted dispatch + repost — Stage-1 loop self-runs',
    spec: `Implement a deterministic scripted dispatch+repost policy so the Stage-1 loop SELF-RUNS with no external commands. ZERO new config fields (LAW 4) — read thresholds from the EXISTING DispatchCfg.

STEP 1 (failing test, in world.rs tests): build a FULL Stage-1 fixture — a miner producer (input=None, output=Some((Ore,k))) at station A; a refiner producer (input=Some((Ore,k)), output=Some((Fuel,m))) at station A; a DEMAND SINK at station B = an input-only producer (input=Some((Fuel,s)), output=None) which consumes Fuel and (for free, via run_producers' input leg) bumps consumed[Fuel]; one corp; one Idle hauler co-located at A's body. SEED ONE initial contract via ContractInit (the route TEMPLATE: corp, Fuel, qty, from A, to B, reward). Step ~N ticks (e.g. up to a few thousand, break on success) with NO external commands. ASSERT the loop self-runs: at least one contract reaches Completed; station B Fuel stock saw deliveries (>0 at some point or cumulative); and the resource accounting identity Σstation.stock + Σin_transit_cargo == initial + econ.mined - econ.consumed holds EVERY tick (per resource; in_transit_cargo = Σ over ALL craft cargo Options).

STEP 2: run, see it fail.

STEP 3 (implement): add a scripted-policy stage (call it stage (7) / a new economy.rs fn run_scripted_dispatch) invoked from World::step (place it pre-physics, e.g. right after ingest/run_producers, before resolve_contracts so the same-tick accept can resolve, OR document the chosen placement). DETERMINISTIC, sorted-id, no RNG:
  (a) REPOST: for the corp's route(s), if NO contract for that route (same corp+from+to+resource) is currently in a NON-TERMINAL status {Offered, Accepted, CargoLoaded, InTransit} — i.e. the prior one reached Completed or Failed — and the sink at 'to' shows demand (stock below DispatchCfg.demand_low), then ContractStore::push a NEW Offered contract that CLONES the route of the most-recent terminal contract for that route (from/to/resource/qty/reward — DO NOT INFER the route from producer topology; clone an existing contract's fields). Emit ContractOffered. The seeded ContractInit is the first template; every repost clones a prior contract's route.
  (b) ASSIGN: for each Idle hauler (role==Idle, sorted dense row), emit an AcceptContract command for the lowest-ContractId Offered contract through the SAME ingest path (push onto the command buffer that ingest consumes, OR set the craft.contract+role directly the way ingest does — match the existing single ingest path; resolve_contracts settles it next tick). One hauler -> one contract; sorted order.
NEVER add a config field; DispatchCfg.demand_low/demand_high/stagger_period/contract_reward_micros/contract_qty already exist — use them. Stores only grow (LAW 6).

STEP 4: run green + \`cargo clippy --all-targets -- -D warnings\`.

STEP 5: commit economy.rs + world.rs only, subject exactly: "feat(economy): scripted dispatch + repost — Stage-1 loop self-runs" + trailer.`,
  },
  {
    task: 18,
    phase: 'T18 phase-1 gate',
    files: 'crates/jumpgate-core/src/world.rs (tests) [or a test module]',
    title: 'test(economy): phase-1 gate — accounting identity + replay determinism green',
    spec: `The PHASE-1 GATE. Two new tests + the full green gate. This is the FIRST end-to-end exercise of the RESOURCE identity, so make the fixture hit every leg (mine, refine, load, deliver, sink-consume) — reuse the T17 full Stage-1 fixture.

STEP 1 (accounting-identity test): write a helper assert_resource_identity(world, initial) that checks, PER RESOURCE r: Σ over stations of stock[*][r] + Σ over ALL craft of (cargo qty where cargo==Some((r,q))) == initial[r] + econ.mined[r] - econ.consumed[r]. Capture initial[r] = Σ station stock per resource right after World::reset (tick 0). Run the full Stage-1 fixture for 200 ticks calling assert_resource_identity EVERY tick. Run -> PASS. (Note: the 200-tick success run likely never fires a Failed event, so the Failed->consumed leg is covered by T16's targeted test, not this sweep — that is fine.)

STEP 2 (determinism test, per the digest-tests-are-determinism-not-golden principle): build TWO worlds from the SAME Stage-1 config and the SAME (empty) command inputs; step both 200 ticks; assert their state_hash() sequences are bit-identical tick-by-tick. Run -> PASS. (This proves replay determinism WITHOUT moving any golden — it is a cross-instance equality, not a pinned constant.)

STEP 3: run \`cargo test -p jumpgate-core\` AND \`cargo clippy --all-targets -- -D warnings\` — both green (LAW 8).

STEP 4: commit the test file(s) only, subject exactly: "test(economy): phase-1 gate — accounting identity + replay determinism green" + trailer.

CRITICAL: if the identity test FAILS, the bug is in T11-T16 (an unaccounted leg) — report it as a blocker with the exact per-resource mismatch (which resource, expected vs actual, at which tick); do NOT paper over it by adjusting the assertion.`,
  },
]

// ===================== SEQUENTIAL TDD: implement -> verify -> (repair) =====================
const results = []
for (const s of SPECS) {
  phase(s.phase)
  log(`Task ${s.task}: implementing`)

  const impl = await agent(
    `You are implementing Task ${s.task} of the Jumpgate first-economic-loop harness (Phase 1b), strict TDD.

${LAWS}

FILES YOU MAY TOUCH: ${s.files}

TASK ${s.task} SPEC:
${s.spec}

Work in the live repo at /home/john/jumpgate. Follow the five TDD steps in order. Read the relevant existing code first (economy.rs run_producers/resolve_contracts/resolve_deliveries, world.rs World::step, the ContractStore/CraftStore/EconCounters structs) to match existing signatures and style exactly. Commit at the end with the exact subject and trailer. Return the structured result.`,
    { label: `impl:T${s.task}`, phase: s.phase, schema: IMPL_SCHEMA, agentType: 'general-purpose' },
  )

  if (!impl || !impl.done || (impl.blocker && impl.blocker !== 'none')) {
    log(`Task ${s.task} BLOCKED at impl: ${impl ? impl.blocker : 'agent returned null'}`)
    results.push({ task: s.task, impl, verify: null, halted: true })
    break
  }

  log(`Task ${s.task}: verifying ${impl.commit_sha}`)
  let verify = await agent(
    `You are the INDEPENDENT VERIFIER for Task ${s.task} (Phase 1b) of the Jumpgate harness. The implementer claims commit ${impl.commit_sha} is done. TRUST NOTHING — re-run everything yourself in /home/john/jumpgate.

${LAWS}

Run and report VERBATIM:
1. \`cargo test -p jumpgate-core\` — capture the lib \`test result:\` line; confirm the task's new test(s) are present and passing (read the test BODY — is it substantive or hollow?). New tests claimed: ${impl.new_tests}
2. \`cargo clippy --all-targets -- -D warnings\` — must be clean (this compiles jumpgate-py too).
3. GOLDENS UNMOVED — grep that all five anchors are still at Phase-0 values: config.rs GOLDEN_CONFIG_HASH=0xf4bc_85c3_7cb6_8a6b, hash.rs HASH_FORMAT_VERSION=2 + GOLDEN_ZERO_STATE_HASH=0x65d7_af3b_9a8a_8276 + zero-world 0x64dd_5078_a3e0_5886, provenance hash_fmt_v=2. Confirm hash.rs/config.rs/provenance.rs were NOT touched by HEAD.
4. COMMIT — \`git show --stat HEAD\`: subject matches "${s.title}", correct Co-Authored-By trailer, ONLY these files changed: ${s.files}, and NO forbidden files (.claude/.gitignore/CLAUDE.md/AGENTS.md/.mcp.json/.filigree.conf) staged.
5. For Task 18 specifically: confirm the resource-identity test actually calls the identity check EVERY tick over 200 ticks and the determinism test compares state_hash sequences across TWO worlds.

passed=true ONLY if all of the above hold. Return the structured verdict with the real command outputs.`,
    { label: `verify:T${s.task}`, phase: s.phase, schema: VERIFY_SCHEMA, agentType: 'general-purpose' },
  )

  // One repair attempt on a failed verify.
  if (!verify || !verify.passed) {
    log(`Task ${s.task} verify FAILED: ${verify ? verify.problems : 'null'} — one repair attempt`)
    const repair = await agent(
      `Task ${s.task} (Phase 1b) FAILED independent verification. Fix it in /home/john/jumpgate, then AMEND or add a fixup commit (keep the single-cause subject "${s.title}" + trailer; if the fix is in a sibling crate like jumpgate-py exhaustiveness, a separate fix(py): commit is correct — see LAW 8).

${LAWS}

FILES: ${s.files} (plus jumpgate-py ONLY if the failure is cross-crate clippy exhaustiveness).

REPORTED PROBLEMS:
${verify ? verify.problems : 'verifier returned null'}
${verify ? 'suite=' + verify.suite_result + ' | clippy=' + verify.clippy_result + ' | goldens_unmoved=' + verify.goldens_unmoved + ' | commit_present=' + verify.commit_present : ''}

After fixing, RE-RUN \`cargo test -p jumpgate-core\` and \`cargo clippy --all-targets -- -D warnings\` and report the real results. Return the impl-shaped structured result for the repaired state.`,
      { label: `repair:T${s.task}`, phase: s.phase, schema: IMPL_SCHEMA, agentType: 'general-purpose' },
    )
    log(`Task ${s.task} repair done (${repair ? repair.commit_sha : 'null'}); re-verifying`)
    verify = await agent(
      `RE-VERIFY Task ${s.task} (Phase 1b) after a repair. Same rigor as before, in /home/john/jumpgate. Run \`cargo test -p jumpgate-core\`, \`cargo clippy --all-targets -- -D warnings\`, confirm the five goldens unmoved, confirm HEAD commit(s) are clean with trailer and no forbidden files. Report verbatim outputs.`,
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

return { phase: '1b', results }
