# Rung-A Exit Digest — 2026-06-13

The final deliverable of Goods-as-Goods Rung A: the behavior digest over the
20-seed `scenario_bazaar` exit ensemble, plus the determinism gate vs the A0
baseline.

## Rung-A exit digest (the 20-seed ensemble)

sha256 over the file-sorted concatenation of all 20-seed cells at both run
lengths (50k for WA5 bank-comparability + 100k for the per-good reads),
covering stdout + window-JSONL + gossip-log per cell. Reproduced bit-identical
twice:

```
0e87bb1f9bcf78f5f1af951b117cc47c2f845c8b81cf8690d151ce5efad5e957
```

Regeneration (the raw cells live in `runs/gag-rung-a/`, gitignored):

```bash
python3 python/analysis/sweep_bazaar.py \
  --out runs/gag-rung-a --baseline-dir runs/gag-a6-baseline \
  --ticks-short 50000 --ticks-long 100000 \
  --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101

cd runs/gag-rung-a && cat $(ls \
  baseline-bazaar_s*_t50000.stdout baseline-bazaar_s*_t50000.jsonl \
  baseline-bazaar_s*_t50000.gossip.jsonl baseline-bazaar_s*_t100000.stdout \
  baseline-bazaar_s*_t100000.jsonl baseline-bazaar_s*_t100000.gossip.jsonl \
  | sort) | sha256sum
```

## Determinism gate — A0 baseline vs HEAD (behavior streams)

All 8 behavior streams (window-JSONL + gossip-log for trophic/frontier × s7/s23)
reproduce **bit-identical** to the A0-tip baseline
(`docs/superpowers/posts/2026-06-13-gag-a0-baseline-digest.md`):

| stream | sha256 (prefix) |
| --- | --- |
| trophic-s7.jsonl | `e34ddaacb5cc…` |
| trophic-s7.gossip.jsonl | `669e1b3243db…` |
| trophic-s23.jsonl | `7cb04b4cca80…` |
| trophic-s23.gossip.jsonl | `f6fafc2ce036…` |
| frontier-s7.jsonl | `527f33509e81…` |
| frontier-s7.gossip.jsonl | `22fae2e7a2ac…` |
| frontier-s23.jsonl | `a3ad9dfc2460…` |
| frontier-s23.gossip.jsonl | `1b99bed8a563…` |

The only stdout (`.out`) delta vs A0 is the additive A4 `EXCHANGE` standing-read
line; the behavior stream is byte-equivalent (documented A5.5 clean-pass). Every
goods-as-goods rung-A mechanic is therefore behavior-neutral on trophic and
frontier (the OD-1 hash-neutrality confirmation).

## Reading summary (PDR-0006: RECORDED, NEVER GATED)

- **WA5 trophic preservation** — clean seeds (verdict != PermanentPeace) =
  12/20, **all Alive** (Alive‰ = 1000). Boom-bust survives the goods-as-goods
  demand-mechanism swap on every clean seed. 8 seeds read PermanentPeace
  (verdict-chain precedence; excluded from the distribution read).
- **WA1 survival** — ~71-77 / 100 station×good pairs hit zero-stock windows on
  every seed: pervasive *localized* starvation, the clumped-topology signature
  (goods do not reach every station). RimLocalized-at-scale, expected under a
  10-good clumped factory topology. Drill-down map = the A6.6 per-station
  epilogue (`chronicle-alive-seed99.txt`).
- **Per-good route concentration** — HHI medians 324-775‰ across the 10 goods,
  moderate→concentrated; Good(8) concentrated (median 775‰); no good
  self-averages (all ≥ 200‰). Clumped topology is working.
- **WA3 own-trade share** — 0‰ on every seed: own-cargo trade counts are not
  yet on the JSONL window (carried on the BAZAAR anchored line, stubbed to 0 in
  the A2-A5 mechanic). No prey-shrink confound possible; the mode-flip arc is
  not yet observable.
- **WA2 spread-closure & WA4 tanker** — both empty: the gossip log emits
  accept/deliver enrichment (resource/reward/route) but not the
  post/contract/to_station join keys these readers need. NoTanker recorded as a
  finding (PDR-0006: equally informative); the post-row gossip enrichment is a
  separate mechanic, outside the A6 science-panel scope.

## Owner console call (no gate)

Judge the rung from the WA5 preservation read + the chronicle materials. Decide
whether the post-row gossip enrichment (to unlock WA2/WA4) and the trade-count
JSONL field (to unlock WA3) warrant a follow-up before rung B.
