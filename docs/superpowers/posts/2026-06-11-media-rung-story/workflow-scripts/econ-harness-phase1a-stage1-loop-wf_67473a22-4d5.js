export const meta = {
  name: 'econ-harness-phase1a-stage1-loop',
  description: 'First economic loop harness — Phase 1 part A, tasks 9-15 (Stage-1 closed loop up to deliver+settle) via per-task TDD implement+verify',
  phases: [
    { title: 'Phase1a', detail: 'Tasks 9-15: events/commands, run_producers, mint-from-config, accept+load, deliver+settle' },
  ],
}

const ANCHORS = `
DETERMINISM ANCHORS (Phase 0 is DONE; Phase 1 must move NONE of these):
- GOLDEN_CONFIG_HASH        = 0xf4bc_85c3_7cb6_8a6b   (config.rs)
- HASH_FORMAT_VERSION       = 2                        (hash.rs)
- GOLDEN_ZERO_STATE_HASH    = 0x65d7_af3b_9a8a_8276   (hash.rs)
- zero-world state golden   = 0x64dd_5078_a3e0_5886   (hash.rs state_hash_golden_zero_world)
- provenance stamp          = hash_fmt_v=2            (provenance.rs)
`

const SHARED = `
PROJECT: Jumpgate v1 deterministic 3D Newtonian sim. Repo root: /home/john/jumpgate. Crate: jumpgate-core (Rust 2024, #![forbid(unsafe_code)]). Branch: jumpgate-v1-design (stay on it; never create branches).
PLAN (source of truth): docs/superpowers/plans/2026-06-09-first-economic-loop-harness.md
SPEC (context): docs/superpowers/specs/2026-06-09-first-economic-loop-harness-design.md

PHASE 0 IS COMPLETE (commits 97524cf..6bd4b35). All economy stores, ids, config fields, and the state-hash fold already exist and are committed. Specifically these ALREADY EXIST — do NOT recreate, re-pin, or re-fold them:
- crate::economy: Resource{Ore,Fuel}/N_RESOURCES/index(), Recipe{input,output,interval}, StationStore, ProducerStore, CorporationStore, ContractStore, ContractStatus{Offered..Failed}+rank(), EconCounters{mined:[i64;2],consumed:[i64;2]}+zero().
- crate::stores: CraftRole{Idle,Hauler}+rank(); CraftStore columns role/cargo(Option<(Resource,u32)>)/credits_micros(i64)/contract(Option<ContractId>).
- crate::ids: StationId/ProducerId/CorporationId/ContractId.
- crate::config: StationInit/ProducerInit/CorporationInit/ContractInit; PriceCfg{base_micros,cap,slope_milli,reprice_interval}; DispatchCfg{demand_low,demand_high,stagger_period,contract_reward_micros,contract_qty}; RunConfig has stations/producers/corporations/contracts/price_cfg/dispatch_cfg, all folded into config_hash.
- crate::world::World has pub(crate) stations/producers/corporations/contracts/econ, minted EMPTY at reset (Task 12 populates them from config).
- state_hash folds ALL the above via shared helpers write_craft_economy + write_economy_stores (words 16-24); the parity recompute uses the same helpers.

${ANCHORS}

NON-NEGOTIABLE LAWS:
1. LOCATE CODE BY SYMBOL, never by cited line number (the plan's line refs are stale after Phase 0 edits; Tasks 12-15 further shift world.rs). grep/Read for the symbol.
2. TDD EXACTLY: (a) write failing test, (b) run & CONFIRM it fails for the stated reason, (c) minimal impl, (d) run & CONFIRM pass, (e) run FULL 'cargo test -p jumpgate-core' — nothing regresses, (f) commit.
3. COMMIT: 'git add' ONLY the specific files the task names. NEVER 'git add -A'. The working tree has untracked .claude/, CLAUDE.md, AGENTS.md, .mcp.json, .filigree.conf and a MODIFIED .gitignore — never add any of those. Use the task's exact commit message + append a blank line then this trailer:
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
4. PHASE 1 MOVES ZERO GOLDENS. EventKind/CommandKind are NOT hashed (events are a stream; commands resolve into already-hashed state) — adding variants is hash-neutral. You only POPULATE/MUTATE already-folded economy state. Do NOT bump HASH_FORMAT_VERSION, do NOT add any new RunConfig/PriceCfg/DispatchCfg field, do NOT re-pin any golden. If a task seems to require a NEW hashed field or a NEW config field, STOP and report it as a blocker (do not improvise) — that is a design escalation, not a task.
5. Rust 2024 reserves 'gen' as a keyword — use 'generation' for slot/gen bindings.
6. NEVER remove from any economy store in v1 (no despawn). Contract lifecycle is STATUS-only: Completed/Failed are terminal STATUSES, not deletions. Do not "clean up" completed contracts — removal misaligns the SoA columns and corrupts the hash. Idle a hauler by clearing its role/cargo/contract columns, not by deleting rows.
7. All new step-stage selection/iteration is by SORTED id, NO RNG (the World rng field stays dead). Determinism depends on it.

ACCOUNTING RULE (Task 11 — the resource identity Σstock(r)+in_transit(r)==initial(r)+mined(r)-consumed(r) is the T18 gate; get the COUNTER LEGS right or it fails silently):
- A recipe firing updates a counter in lockstep with stock, PER LEG:
  * input  (r_in, q):  stock[r_in] -= q  AND  consumed[r_in] += q
  * output (r_out, q): stock[r_out] += q AND  mined[r_out] += q
- So the REFINER (Ore->Fuel) increments BOTH consumed[Ore] AND mined[Fuel]. The miner (∅->Ore) increments mined[Ore] only; the demand sink (Fuel->∅) increments consumed[Fuel] only.
- Cargo load (station stock -> craft cargo) and deliver (craft cargo -> station stock) are TRANSFERS between stock and in_transit_cargo — they touch NO counter.
- On a Failed contract where v1 loses the cargo: route the lost cargo into consumed[cargo_resource] so the identity stays exact.

CONTRACT ROUTES (Tasks 14-17): seed initial contracts via ContractInit (config) so accept/load/deliver run against config-seeded (from,to,resource,qty,reward) routes. When Task 17 reposts on persistent demand, CLONE the prior/seeded contract's (from_station,to_station,resource,qty,reward) — do NOT infer routes from producer/sink topology and do NOT add a route config field.

Report only what you actually did and observed; paste real command output. If you hit a genuine blocker (ambiguous spec, would-need-new-hashed-field, persistent test failure), set done=false and explain — do NOT fabricate green.
`

const IMPL_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    done: { type: 'boolean' },
    commit_sha: { type: 'string' },
    new_tests: { type: 'string', description: 'names of tests added by this task' },
    full_suite_result: { type: 'string', description: 'verbatim lib "test result:" line' },
    deviations: { type: 'string', description: 'stale line numbers corrected, spec deviations, or "none"' },
    blocker: { type: 'string', description: 'if done=false, the precise blocker; else "none"' },
  },
  required: ['done', 'commit_sha', 'new_tests', 'full_suite_result', 'deviations', 'blocker'],
}

const VERIFY_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    passed: { type: 'boolean' },
    suite_result: { type: 'string' },
    clippy_result: { type: 'string', description: 'PASS or first error/warning' },
    goldens_unmoved: { type: 'boolean', description: 'true iff all 5 anchors are still at the Phase-0 values and HASH_FORMAT_VERSION==2' },
    commit_present: { type: 'boolean' },
    problems: { type: 'string' },
  },
  required: ['passed', 'suite_result', 'clippy_result', 'goldens_unmoved', 'commit_present', 'problems'],
}

phase('Phase1a')

const T12_NOTE = `\n\nTASK-12-SPECIFIC: your new test must exercise the RESET-FROM-CONFIG minting path (distinct from the existing populated_economy_parity which mutates by hand): reset two worlds from the SAME economy config (2 stations + 1 miner via StationInit/ProducerInit) and assert (a) equal state_hash, and (b) the minted store contents match the RunConfig init vecs (station count==2, producer count==1, counters zero). Resolve body_index->the minted BodyId and station_index->the minted StationId; an out-of-range index is a new ResetError arm validated before tick 0.`

const tasks = [9, 10, 11, 12, 13, 14, 15]
const results = []

for (const n of tasks) {
  const extra = n === 12 ? T12_NOTE : ''
  const impl = await agent(
    `${SHARED}${extra}\n\nImplement **Task ${n}** of the plan EXACTLY (open the plan, find "### Task ${n}:", follow every step in order; read referenced source by symbol). Leave the crate building, the new test(s) passing, the full 'cargo test -p jumpgate-core' green with nothing regressed, and commit with the task's exact message + trailer. Return the structured result honestly (done=false + blocker if you cannot).`,
    { label: `impl:T${n}`, phase: 'Phase1a', schema: IMPL_SCHEMA }
  )

  if (!impl || !impl.done) {
    throw new Error(`Task ${n} not completed by implementer: ${impl ? impl.blocker : 'agent died'}. Halting Phase 1a for inspection.`)
  }

  let verify = await agent(
    `${SHARED}\n\nINDEPENDENT verifier for **Task ${n}** (do NOT trust the implementer). From repo root run yourself:\n1. 'cargo test -p jumpgate-core' — lib line must show 0 failed; the Task ${n} test(s) must be present and pass.\n2. 'cargo clippy --all-targets -- -D warnings' — must be clean.\n3. 'git log --oneline -1' — HEAD is the Task ${n} economy commit; 'git show --stat HEAD' touched only expected files (NOT .gitignore/.claude/etc.).\n4. GOLDENS UNMOVED: grep the anchors and confirm ALL FIVE still equal the Phase-0 values: config 0xf4bc_85c3_7cb6_8a6b, HASH_FORMAT_VERSION 2, GOLDEN_ZERO_STATE_HASH 0x65d7_af3b_9a8a_8276, zero-world 0x64dd_5078_a3e0_5886, provenance hash_fmt_v=2. goldens_unmoved=false if ANY moved (a Phase-1 red flag).\nReport real outputs.`,
    { label: `verify:T${n}`, phase: 'Phase1a', schema: VERIFY_SCHEMA }
  )

  if (!verify || !verify.passed || !verify.goldens_unmoved) {
    log(`Task ${n} verify FAILED (passed=${verify?.passed} goldens_unmoved=${verify?.goldens_unmoved}): ${verify?.problems} — one repair attempt`)
    await agent(
      `${SHARED}\n\nTask ${n} verification FAILED: ${verify ? verify.problems : 'verifier died'} (suite=${verify?.suite_result}, clippy=${verify?.clippy_result}, goldens_unmoved=${verify?.goldens_unmoved}). Diagnose and FIX so the full suite is green, clippy clean, and ALL FIVE goldens are back at the Phase-0 values (if you moved a golden you almost certainly introduced a new hashed field — revert that approach; Phase 1 moves no goldens). Amend or follow-up commit (specific files + trailer).`,
      { label: `repair:T${n}`, phase: 'Phase1a' }
    )
    verify = await agent(
      `${SHARED}\n\nRe-verify **Task ${n}** after repair: cargo test, clippy --all-targets -D warnings, git log -2, and the five goldens-unmoved check. Report honestly.`,
      { label: `reverify:T${n}`, phase: 'Phase1a', schema: VERIFY_SCHEMA }
    )
    if (!verify || !verify.passed || !verify.goldens_unmoved) {
      throw new Error(`Task ${n} still failing after repair: ${verify?.problems}. Halting Phase 1a for inspection.`)
    }
  }

  results.push({ task: n, impl, verify })
  log(`Task ${n} GREEN — ${verify.suite_result}`)
}

return results
