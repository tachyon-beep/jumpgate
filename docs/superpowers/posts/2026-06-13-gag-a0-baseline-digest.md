# A0 Instrument Baseline Digest — 2026-06-13

Commit tip: 4a0f6c32e9b5afc5b94af61f72534c0b193c1262
Pinned at: last A0 commit (feat(a0): META optional goods= tail + A0.5 follow-up refactor)
Run length: 50k ticks / 25 windows (W=2000)
Scenarios: trophic (s7, s23), frontier (s7, s23)

## SHA256SUMS (stdout + window-JSONL + gossip-log per scenario/seed)

```
0265fdb1e2840f97781f53550f8460d5f6c43481c4e6c37b6e66b49253dcf155  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.out
e34ddaacb5cc0b3023cebb96f43f26240d16b91ac984128fa3f4587277f299cf  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.jsonl
669e1b3243db4811ef9492717fa342ea8c326839627fa8013f7ecc4003f9116f  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s7.gossip.jsonl
3280c955955052d9ce446bc49e378cef40a981e9368e53ff75fcd3416f518ba7  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.out
7cb04b4cca80245c3095a56e4569c00312d9a1bae01f9b6908e2d45a01b839c6  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.jsonl
f6fafc2ce036b539d152790907d52e058cd8b5a9d58621bf920db42da6dba19f  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/trophic-s23.gossip.jsonl
6bf44ef3f436b555a9c4e04802a12dc671c3da609b1d5c21d04529ddcddd3f98  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.out
527f33509e8144c5c111646ea141a21459b28fb1612e0186bd9012cbbf216b0c  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.jsonl
22fae2e7a2ac3a2720605d2848d1a3a48fc97bff5a9d90da0a689a80a8a950fc  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s7.gossip.jsonl
08c85bc832684d3343c061b8e95494ecc60c26f547a90380c13b922408e0b1e6  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.out
a3ad9dfc24603baa2f370e96189d664fabe4c1f7a20adbb73a002eda8c522cdc  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.jsonl
1b99bed8a563119ade8e8db4ec84c0d59c7f405dea6113f1a3431bf069142aa8  /home/john/jumpgate/runs/2026-06-13-gag-a0-baseline/frontier-s23.gossip.jsonl
```

## Replay-check

All four runs: replay-check OK, 50 (tick, state_hash) samples bit-identical.

```
replay-check OK: 50 (tick, state_hash) samples bit-identical (every 1000 ticks)  [trophic s7]
replay-check OK: 50 (tick, state_hash) samples bit-identical (every 1000 ticks)  [trophic s23]
replay-check OK: 50 (tick, state_hash) samples bit-identical (every 1000 ticks)  [frontier s7]
replay-check OK: 50 (tick, state_hash) samples bit-identical (every 1000 ticks)  [frontier s23]
```

## Window JSONL spot-check (trophic s7)

- window count: 25
- per_station_stock: present, 6 x 2 matrix (n_stations x N_RESOURCES)
- per_station_price: present, 6 x 2 matrix
- transport_table tail row: present (1 row, empty array — trophic structural off)

## Gossip-log new row spot-check (trophic s7)

- "deliver" rows: present (ContractFulfilled now emits gossip-log row)
- "lie_low" rows: present (PirateLieLow now emits gossip-log row)
- "rob" rows: carry "pirate" field
- "accept" rows: carry "resource" + "reward" fields

## Comparison protocol for A1+

For each later-phase commit, repeat this procedure (replacing --scenario and
--seed as needed). Any file-level sha256 divergence vs this baseline is a
determinism break. For hash-neutral commits (A1 runtime-goods refactor),
ALL 12 digests must be identical. For behavior-changing commits (A3+),
record the new digests under a new dated directory.

## Regeneration (exact command shape — digest capture excludes --replay-check)

The digest-capture runs do NOT pass `--replay-check` (its OK line would land
in stdout and move the .out digest); replay-check is run separately. Per
(scenario, seed):

```bash
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario <trophic|frontier> --seed <7|23> --ticks 50000 \
  --jsonl <dir>/<scenario>-s<seed>.jsonl \
  --gossip-log <dir>/<scenario>-s<seed>.gossip.jsonl \
  > <dir>/<scenario>-s<seed>.out
sha256sum <dir>/<scenario>-s<seed>.{out,jsonl,gossip.jsonl}
```

Independently reproduced 2026-06-13 (main loop): frontier-s7 all three
digests bit-identical (the .out matches after stripping the replay-check
line from a run that had mistakenly included the flag).

## A5.5 clean-pass verification (2026-06-13, A5 tip)

Re-ran the digest-capture for trophic-s7 and frontier-s7 at the A5 phase tip.
Behavior-stream digests (the load-bearing ones) reproduce BIT-IDENTICAL:

- `trophic-s7.jsonl`  e34ddaac… ✓   `trophic-s7.gossip.jsonl`  669e1b32… ✓
- `frontier-s7.jsonl` 527f3350… ✓   `frontier-s7.gossip.jsonl` 22fae2e7… ✓

The `.out` (stdout) digests diverge by exactly ONE added line —
`EXCHANGE treasury_micros=… drain_per_100k=0` — which is the A4-era EXCHANGE
standing-read instrument (commit 7d80054), already present at the A4 tip
(6a20d53) BEFORE any A5 work. The A5 runner edits (data-driven `bazaar_mode`
= `scenario=="bazaar"` and `n_goods` = `cfg.goods.goods.len()`, and moving the
`exchange_treasury` binding ahead of the BAZAAR anchored line) produce
IDENTICAL stdout for trophic/frontier (both stay `bazaar_mode=false`, no
`goods=` META tail, no BAZAAR line). All four goldens
(`config_hash_golden_anchor_is_stable`, `state_hash_golden_zero_world`,
`golden_zero_state_hash`, `frontier_trajectory_golden`) pass unchanged; all
three scenarios (trophic/frontier/bazaar) replay-check bit-identical. The new
own-trade stages are inert on trophic/frontier (`exchange.active == false`).
