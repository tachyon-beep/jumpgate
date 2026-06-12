# Phase A6 — Science + Console (Rung-A windows, digest exit)

> **Frame (PDR-0006):** every number produced here is a designer's WINDOW for
> the console observe→steer→re-observe loop — never an acceptance gate, never a
> build trigger. Recorded, not gated.
>
> **Ordering invariant:** A6 tasks run AFTER the last A0 commit (digest
> baseline pinned there) and AFTER A1-A5 mechanics. The A0 baseline is the
> rung-A exit reference; A6.0 pins it, then A6.1-A6.6 build the readers, then
> A6.7 runs the 20-seed ensemble and produces the console packet.
>
> **Scope reminder:** Rung A only. No jettison, fencing, JetsamStore, posture,
> greed, or rung-B config. BAZAAR/CRATE line — only BAZAAR lands here; CRATE
> is rung B.

---

### Task A6.0: Behavior digest baseline (A0 tip pinned)

**Files**

- Create: `runs/gag-a6-baseline/` (directory, builder creates at run time)
- Modify: nothing (procedure only — no source changes, no commit)

**Steps**

- [ ] **Step 1: Verify A0 is the last landed commit**

  ```
  git log --oneline -5
  ```

  Confirm the HEAD commit is the final A0 instruments commit (no A1+ mechanics
  present). If A1+ has already landed, the baseline must have been pinned at the
  A0 tip; check `runs/gag-a6-baseline/` for pre-existing digests and skip
  forward to A6.1.

- [ ] **Step 2: Build release binaries**

  ```bash
  cargo build -p jumpgate-core --release --example trophic_run 2>&1 | tail -5
  ```

  Expected: `Finished release profile`.

- [ ] **Step 3: Run A0-tip digest across both reference scenarios**

  For each `SCENARIO` in `trophic frontier`:

  ```bash
  mkdir -p runs/gag-a6-baseline
  for S in 7 23; do
    cargo run -q -p jumpgate-core --release --example trophic_run -- \
      --scenario $SCENARIO --seed $S --ticks 2000 \
      --gossip-log runs/gag-a6-baseline/${SCENARIO}-base-s${S}.gossip.jsonl \
      --jsonl    runs/gag-a6-baseline/${SCENARIO}-base-s${S}.jsonl \
      > runs/gag-a6-baseline/${SCENARIO}-base-s${S}.out
  done
  ```

- [ ] **Step 4: Compute and record digests**

  ```bash
  for SCENARIO in trophic frontier; do
    for S in 7 23; do
      BASE=runs/gag-a6-baseline/${SCENARIO}-base-s${S}
      sha256sum ${BASE}.out ${BASE}.jsonl ${BASE}.gossip.jsonl
    done
  done | tee runs/gag-a6-baseline/BASELINE_DIGESTS.txt
  ```

  Paste the output into `runs/gag-a6-baseline/BASELINE_DIGESTS.txt`.
  This file is the rung-A exit reference. It must NOT be in the git staging
  area (`runs/` is `.gitignore`d by HOUSE RULES — never stage it).

- [ ] **Step 5: Sanity check — no A1+ behavior yet**

  The RESULT lines should show `verdict` values matching the pre-A5 banked
  runs. If trophic seed=7 shows `PermanentPeace` on a 2000-tick run that is
  expected (PermanentPeace can fire in short runs on some seeds). The digest
  pinning is the gate; absolute verdict is not.

---

### Task A6.1: WA1 survival-by-market reader

**Files**

- Create: `python/analysis/wa1_survival.py`
- Create: `python/tests/test_wa1_survival.py`

**Steps**

- [ ] **Step 1: Write the failing test first**

  `python/tests/test_wa1_survival.py`:

  ```python
  """WA1 survival-by-market reader pins (spec §5 WA1; windows, not gates).

  WA1 reads: per-station per-good minimum stock over the run (zero-stock
  run-length) + the consumer-starved hauler count (stalled-consumer read from
  JSONL). Either answer is a finding: localized starvation at the rim is
  expected and interesting; universal starvation means the market broke.
  The anti-mirroring (L4-F4) transport table tail row is read here, never
  mirrored as a Python constant.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa1_survival


  def _make_windows(n_stations, n_goods, stocks):
      """Synthetic window list with per_station_stock flat matrix.

      stocks: list of length n_stations * n_goods (row-major: station-major,
      good-minor), per window. Pass a list of lists (one per window).
      """
      return [
          {
              "tick": (i + 1) * 2000,
              "per_station_stock": stk,
              "per_station_price": [100_000] * (n_stations * n_goods),
          }
          for i, stk in enumerate(stocks)
      ]


  def test_zero_stock_run_length_all_good():
      # 2 stations, 2 goods, no zeros ever
      windows = _make_windows(2, 2, [
          [10, 20, 30, 40],
          [15, 25, 35, 45],
      ])
      result = wa1_survival.stock_runs(windows, n_stations=2, n_goods=2)
      # max zero-stock run-length = 0 for every (station, good)
      for row in result:
          assert row["max_zero_run"] == 0


  def test_zero_stock_run_length_detects_consecutive_zeros():
      # station 0, good 1 has zeros in windows 0 and 1 (run of 2)
      stocks = [
          [10, 0, 30, 40],
          [12, 0, 32, 42],
          [14, 5, 34, 44],
      ]
      windows = _make_windows(2, 2, stocks)
      result = wa1_survival.stock_runs(windows, n_stations=2, n_goods=2)
      by_key = {(r["station"], r["good"]): r for r in result}
      assert by_key[(0, 1)]["max_zero_run"] == 2
      assert by_key[(0, 0)]["max_zero_run"] == 0


  def test_stalled_consumer_count_from_jsonl():
      # deliver rows with same craft back-to-back should count stalls
      # stalled = craft has no deliver events in a window that has traffic
      hauler_slots = [0, 1, 2]
      windows = _make_windows(2, 2, [[10, 20, 30, 40]] * 3)
      # craft 0 never delivers; craft 1 delivers once; craft 2 delivers twice
      deliver_rows = [
          {"e": "deliver", "tick": 2001, "hauler": 1, "good": 0},
          {"e": "deliver", "tick": 2001, "hauler": 2, "good": 0},
          {"e": "deliver", "tick": 2002, "hauler": 2, "good": 1},
      ]
      result = wa1_survival.stalled_consumers(windows, deliver_rows, hauler_slots)
      # craft 0 stalled in all windows that have any deliver activity
      assert result["craft_0_deliver_count"] == 0
      assert result["craft_1_deliver_count"] == 1
      assert result["craft_2_deliver_count"] == 2


  def test_transport_table_tail_row_is_read_not_mirrored():
      # The factory transport table is echoed as a no-tick JSONL tail row
      # (L4-F4 anti-mirroring). wa1_survival must read it from the JSONL,
      # not from a module-level constant.
      tail_row = {
          "e": "transport_table",
          "routes": [[0, 1], [1, 0]],
          "transport_micros": [50000, 60000],
      }
      t = wa1_survival.read_transport_table([tail_row])
      assert t is not None
      assert t["transport_micros"] == [50000, 60000]


  def test_transport_table_absent_returns_none():
      rows = [{"e": "refuel", "tick": 100, "craft": 0, "station": 1,
               "units": 5, "price_micros": 10000,
               "before_permille": 800, "after_permille": 900}]
      assert wa1_survival.read_transport_table(rows) is None
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa1_survival.py -x 2>&1 | tail -20
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa1_survival'
  ```

- [ ] **Step 2: Write `python/analysis/wa1_survival.py`**

  ```python
  """wa1_survival — WA1 survival-by-market reader (spec §5 WA1).

  FRAME (PDR-0006): windows, not gates. Either answer is a finding:
  - localized starvation at rim stations = expected with clumped topology
  - universal starvation = the market broke
  Anti-mirroring (L4-F4): the factory transport table is read from the no-tick
  JSONL tail row emitted by the runner, never mirrored as a Python constant.

  Usage:
      python3 python/analysis/wa1_survival.py <windows.jsonl> \\
          [--gossip-log <gossip.jsonl>] [--n-stations N] [--n-goods G]
  """

  import argparse
  import json
  import pathlib
  import sys


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def stock_runs(windows, n_stations, n_goods):
      """Per-(station, good) max consecutive-zero-stock window run-length.

      Returns list of dicts: {station, good, max_zero_run, zero_windows, total_windows}.
      Zero windows = the consumer-starve raw count; max_zero_run = the worst
      localized drought.
      """
      results = []
      for st in range(n_stations):
          for g in range(n_goods):
              idx = st * n_goods + g
              cur_run = 0
              max_run = 0
              zero_count = 0
              for w in windows:
                  stock = w.get("per_station_stock", [])
                  if idx < len(stock) and stock[idx] == 0:
                      cur_run += 1
                      zero_count += 1
                      max_run = max(max_run, cur_run)
                  else:
                      cur_run = 0
              results.append({
                  "station": st,
                  "good": g,
                  "max_zero_run": max_run,
                  "zero_windows": zero_count,
                  "total_windows": len(windows),
              })
      return results


  def stalled_consumers(windows, deliver_rows, hauler_slots):
      """Per-craft deliver counts over the run.

      A craft with zero delivers is a stalled consumer: either broke (wage
      mode, waiting for work) or stranded. Either reading is a finding.
      Returns dict with craft_{slot}_deliver_count keys.
      """
      counts = {s: 0 for s in hauler_slots}
      for row in deliver_rows:
          if row.get("e") == "deliver":
              h = row.get("hauler")
              if h in counts:
                  counts[h] += 1
      return {f"craft_{s}_deliver_count": v for s, v in counts.items()}


  def read_transport_table(rows):
      """Read the factory transport table from the no-tick JSONL tail row.

      Returns the tail row dict if found, else None (anti-mirroring: L4-F4).
      The runner emits one row with e='transport_table' at run end; older
      runs without the row return None — version gate, no abort.
      """
      for row in rows:
          if row.get("e") == "transport_table":
              return row
      return None


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("windows", help="per-window JSONL file from trophic_run")
      ap.add_argument("--gossip-log", help="gossip-log JSONL with deliver rows")
      ap.add_argument("--n-stations", type=int, required=True)
      ap.add_argument("--n-goods", type=int, required=True)
      args = ap.parse_args()

      all_rows = load(args.windows)
      windows = [r for r in all_rows if "tick" in r]
      gossip = load(args.gossip_log) if args.gossip_log else []

      transport = read_transport_table(all_rows + gossip)
      if transport is None:
          print("WA1 transport table: absent (pre-A0 run — anti-mirroring: L4-F4 not yet wired)")
      else:
          print(f"WA1 transport table: {transport}")

      # Hauler slots: derive from per_craft_role if present (role 1 = hauler),
      # else assume all crafts in per_craft_credits are haulers.
      hauler_slots = []
      if windows and "per_craft_role" in windows[0]:
          for slot, role in enumerate(windows[0]["per_craft_role"]):
              if role == 1:
                  hauler_slots.append(slot)
      elif windows and "per_craft_credits" in windows[0]:
          # Fallback: no role info. Use META haulers count if available.
          hauler_slots = list(range(len(windows[0]["per_craft_credits"])))

      deliver_rows = [r for r in gossip if r.get("e") == "deliver"]
      stall = stalled_consumers(windows, deliver_rows, hauler_slots)
      zero_delivers = sum(1 for v in stall.values() if v == 0)
      total_haulers = len(hauler_slots)
      print(
          f"WA1 stalled consumers (zero delivers over run): "
          f"{zero_delivers}/{total_haulers} haulers "
          "(RECORDED, never gated — PDR-0006; zero = no deliver events or pre-A0)"
      )

      runs = stock_runs(windows, args.n_stations, args.n_goods)
      print(
          f"\nWA1 survival-by-market ({len(windows)} windows, "
          f"{args.n_stations} stations × {args.n_goods} goods) "
          "(RECORDED, never gated — PDR-0006):"
      )
      print(f"  {'station':>7}  {'good':>4}  {'max_zero_run':>12}  "
            f"{'zero_windows':>12}  {'total':>5}")
      for r in runs:
          flag = " <-- STARVATION" if r["max_zero_run"] > 0 else ""
          print(
              f"  {r['station']:>7}  {r['good']:>4}  {r['max_zero_run']:>12}  "
              f"{r['zero_windows']:>12}  {r['total_windows']:>5}{flag}"
          )

      # Summary: either answer is a finding
      starving = [r for r in runs if r["max_zero_run"] > 0]
      if not starving:
          print("\nWA1 reading: NoStarvation — all goods at all stations held stock "
                "above zero in every window (RECORDED; finding: market feeds the world)")
      else:
          stations_hit = {r["station"] for r in starving}
          all_stations = set(range(args.n_stations))
          rim = max(all_stations)
          rim_only = stations_hit <= {rim, rim - 1}
          if rim_only:
              print(f"\nWA1 reading: RimLocalized — starvation confined to "
                    f"rim stations {sorted(stations_hit)} (RECORDED; "
                    "finding: market feeds core; rim wants supply or a lane)")
          else:
              print(f"\nWA1 reading: Universal — starvation at stations "
                    f"{sorted(stations_hit)} (RECORDED; finding: market broke or "
                    "topology mismatch — owner's call)")


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa1_survival.py -v 2>&1 | tail -20
  ```

  Expected:
  ```
  test_zero_stock_run_length_all_good PASSED
  test_zero_stock_run_length_detects_consecutive_zeros PASSED
  test_stalled_consumer_count_from_jsonl PASSED
  test_transport_table_tail_row_is_read_not_mirrored PASSED
  test_transport_table_absent_returns_none PASSED
  5 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa1_survival.py python/tests/test_wa1_survival.py
  git commit -F - <<'EOF'
  feat(lab): WA1 survival-by-market reader — zero-stock run-lengths + stalled consumers

  Reads per-station × per-good max-consecutive-zero-stock run-length and the
  per-craft deliver count (stalled-consumer proxy) from the gossip-log deliver
  rows. Anti-mirroring: transport table read from the no-tick tail row emitted
  by the runner (L4-F4), never mirrored as a Python constant. Either answer is
  a finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.2: WA2 spread-closure reader

**Files**

- Create: `python/analysis/wa2_spread_closure.py`
- Create: `python/tests/test_wa2_spread_closure.py`

**Steps**

- [ ] **Step 1: Write the failing test first**

  `python/tests/test_wa2_spread_closure.py`:

  ```python
  """WA2 spread-closure reader pins (spec §5 WA2; windows, not gates).

  WA2: posted spread on a route decays after package delivery (arbitrage
  arbitrages). Join: post → accept → deliver rows in gossip-log, keyed by
  contract; measure price-at-post minus price-at-deliver per route per good;
  decay over successive contracts on the same route/good pair is the signal.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa2_spread_closure


  def _post(contract, tick, route, good, spread_micros):
      return {"e": "post", "tick": tick, "contract": contract,
              "route": route, "good": good, "spread_micros": spread_micros}


  def _accept(contract, tick):
      return {"e": "accept", "tick": tick, "contract": contract}


  def _deliver(contract, tick, spread_at_deliver):
      return {"e": "deliver", "tick": tick, "contract": contract,
              "spread_at_deliver": spread_at_deliver}


  def test_spread_closure_detects_decay():
      rows = [
          _post(1, 100, 0, 2, 80_000),
          _accept(1, 200),
          _deliver(1, 500, 30_000),
          _post(2, 600, 0, 2, 40_000),
          _accept(2, 700),
          _deliver(2, 900, 10_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      # route 0, good 2: spreads started at 80k, closed to 10k
      assert (0, 2) in result
      series = result[(0, 2)]
      assert series[0]["spread_at_post"] == 80_000
      assert series[1]["spread_at_post"] == 40_000


  def test_spread_closure_open_contract_excluded():
      # Contract without a deliver row = in-flight, excluded from the series
      rows = [
          _post(1, 100, 0, 0, 50_000),
          _accept(1, 200),
          # no deliver for contract 1
          _post(2, 300, 0, 0, 45_000),
          _accept(2, 400),
          _deliver(2, 600, 15_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      # only contract 2 completes; series length = 1
      assert (0, 0) in result
      assert len(result[(0, 0)]) == 1


  def test_spread_closure_no_deliver_rows_returns_empty():
      rows = [
          _post(1, 100, 0, 0, 60_000),
          _accept(1, 200),
      ]
      result = wa2_spread_closure.spread_series(rows)
      assert result == {}


  def test_decay_flag_detected():
      rows = [
          _post(1, 100, 3, 1, 100_000),
          _deliver(1, 500, 40_000),
          _post(2, 600, 3, 1, 60_000),
          _deliver(2, 900, 20_000),
          _post(3, 1000, 3, 1, 30_000),
          _deliver(3, 1200, 5_000),
      ]
      result = wa2_spread_closure.spread_series(rows)
      summary = wa2_spread_closure.summarize(result)
      # route 3, good 1 should show decay
      hit = [r for r in summary if r["route"] == 3 and r["good"] == 1]
      assert len(hit) == 1
      assert hit[0]["decaying"] is True
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa2_spread_closure.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa2_spread_closure'
  ```

- [ ] **Step 2: Write `python/analysis/wa2_spread_closure.py`**

  ```python
  """wa2_spread_closure — WA2 spread-closure reader (spec §5 WA2).

  FRAME (PDR-0006): windows, not gates.
  Decay is the signal: posted spread on a (route, good) pair should fall
  after delivery as the arbitrage opportunity self-eliminates. Any trend
  (rising = routes opening up, flat = persistent gap, falling = closure)
  is a finding.

  Join order: post → accept → deliver rows per contract_id. The "post"
  gossip-log row is runner-enriched (from the ContractOffered event + current
  prices at log time) with spread_micros; the "deliver" row (ContractFulfilled,
  A0 instrument) carries spread_at_deliver (price spread at delivery tick).
  Contracts without a deliver row (still in flight) are excluded.

  Usage:
      python3 python/analysis/wa2_spread_closure.py <gossip.jsonl>
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def spread_series(rows):
      """Build per-(route, good) series of completed contracts.

      Returns dict[(route, good)] = list of {contract, spread_at_post,
      spread_at_deliver, post_tick, deliver_tick} in post-tick order.
      """
      posts = {}
      accepts = {}
      delivers = {}
      for row in rows:
          e = row.get("e")
          c = row.get("contract")
          if c is None:
              continue
          if e == "post":
              posts[c] = row
          elif e == "accept":
              accepts[c] = row
          elif e == "deliver":
              delivers[c] = row

      result = {}
      for c, d in delivers.items():
          p = posts.get(c)
          if p is None:
              continue
          key = (p.get("route"), p.get("good"))
          if None in key:
              continue
          entry = {
              "contract": c,
              "spread_at_post": p.get("spread_micros", 0),
              "spread_at_deliver": d.get("spread_at_deliver", 0),
              "post_tick": p.get("tick", 0),
              "deliver_tick": d.get("tick", 0),
          }
          result.setdefault(key, []).append(entry)

      for key in result:
          result[key].sort(key=lambda x: x["post_tick"])
      return result


  def summarize(result):
      """Per-(route, good) decay summary rows for printing."""
      out = []
      for (route, good), series in sorted(result.items()):
          if len(series) < 2:
              decaying = None  # too few data points
          else:
              first_half = series[: len(series) // 2]
              second_half = series[len(series) // 2 :]
              first_avg = sum(s["spread_at_post"] for s in first_half) // len(first_half)
              second_avg = sum(s["spread_at_post"] for s in second_half) // len(second_half)
              decaying = second_avg < first_avg
          out.append({
              "route": route,
              "good": good,
              "n_contracts": len(series),
              "first_spread": series[0]["spread_at_post"] if series else None,
              "last_spread": series[-1]["spread_at_post"] if series else None,
              "decaying": decaying,
          })
      return out


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      result = spread_series(rows)
      summary = summarize(result)

      print(
          f"WA2 spread-closure ({len(result)} route×good pairs with completed contracts) "
          "(RECORDED, never gated — PDR-0006):"
      )
      if not summary:
          print("  no completed contracts in gossip log (pre-A2 run or no bazaar traffic)")
          return

      print(f"  {'route':>5}  {'good':>4}  {'n':>4}  {'first_spread':>12}  "
            f"{'last_spread':>11}  {'decaying':>8}")
      for r in summary:
          d = str(r["decaying"]) if r["decaying"] is not None else "?"
          print(
              f"  {r['route']:>5}  {r['good']:>4}  {r['n_contracts']:>4}  "
              f"{r['first_spread']!s:>12}  {r['last_spread']!s:>11}  {d:>8}"
          )

      decaying_count = sum(1 for r in summary if r["decaying"] is True)
      flat_count = sum(1 for r in summary if r["decaying"] is False)
      pending_count = sum(1 for r in summary if r["decaying"] is None)
      print(
          f"\nWA2 reading: decaying={decaying_count} flat={flat_count} "
          f"pending={pending_count} (decaying = arbitrage arbitrages; "
          "flat = persistent gap = priced-in transport or no competition; "
          "either is a finding — owner's call)"
      )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa2_spread_closure.py -v 2>&1 | tail -15
  ```

  Expected:
  ```
  test_spread_closure_detects_decay PASSED
  test_spread_closure_open_contract_excluded PASSED
  test_spread_closure_no_deliver_rows_returns_empty PASSED
  test_decay_flag_detected PASSED
  4 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa2_spread_closure.py python/tests/test_wa2_spread_closure.py
  git commit -F - <<'EOF'
  feat(lab): WA2 spread-closure reader — post/deliver join, decay detection per route×good

  Joins gossip-log post→deliver rows per contract; measures spread-at-post vs
  spread-at-deliver per (route, good) pair; flags routes where second-half
  average spread is below first-half (arbitrage arbitrages). Either trend is a
  finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.3: WA3 + WA5 joint reader (channel mix vs capitalization, trophic preservation)

**Files**

- Create: `python/analysis/wa3_wa5_joint.py`
- Create: `python/tests/test_wa3_wa5_joint.py`

**Steps**

- [ ] **Step 1: Failing test**

  `python/tests/test_wa3_wa5_joint.py`:

  ```python
  """WA3+WA5 joint reader pins (spec §5 WA3/WA5; panel joint-read warning).

  WA3 and WA5 are a JOINT READ: own-trade share IS the pirate food supply.
  Shrunken own-trade share → less prey → PermanentPeace masquerade.
  The panel's warning: read WA3 and WA5 side-by-side; a high WA3 own-trade
  share that correlates with PermanentPeace verdict on the SAME seeds is the
  prey-shrink confound, not a finding that bazaar killed boom-bust.

  WA5: verdict distribution-vs-frontier-bank, NEVER same-seed paired.
  The clean_seeds filter (blind-born != PermanentPeace) is mandatory before
  reading the verdict mix; PermanentPeace is first in the verdict chain
  (diagnostics.rs:288) and overrides cycled.

  This module holds the joint reader. It does NOT make a decision; it prints
  the side-by-side and names the confound.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa3_wa5_joint


  def _window(credits, trade_sold=0, trade_bought=0, robs=0, laden_trips=0):
      return {
          "tick": 2000,
          "per_craft_credits": credits,
          "trade_sold_count": trade_sold,
          "trade_bought_count": trade_bought,
          "robs": robs,
          "laden_trips": laden_trips,
      }


  def test_own_trade_share_zero_when_no_trades():
      windows = [_window([100, 200, 50])]
      share = wa3_wa5_joint.own_trade_share_milli(windows, n_haulers=3)
      assert share == 0


  def test_own_trade_share_is_trade_over_trade_plus_contract():
      # 3 trade_sold vs 7 laden_trips (contract delivers)
      windows = [_window([100, 200, 50], trade_sold=3, laden_trips=7)]
      share = wa3_wa5_joint.own_trade_share_milli(windows, n_haulers=3)
      # 3 / (3 + 7) = 0.3 = 300 milli
      assert share == 300


  def test_clean_seeds_filters_permanent_peace():
      cells = {
          7:  {"verdict": "Alive",         "own_trade_share_milli": 200},
          11: {"verdict": "PermanentPeace","own_trade_share_milli": 800},
          13: {"verdict": "NoCycle",       "own_trade_share_milli": 100},
      }
      clean = wa3_wa5_joint.clean_seeds(cells)
      assert clean == [7, 13]


  def test_prey_shrink_confound_flagged():
      # High own-trade share AND PermanentPeace on same seed = confound
      bazaar_cells = {
          7:  {"verdict": "PermanentPeace", "own_trade_share_milli": 750},
          11: {"verdict": "Alive",          "own_trade_share_milli": 150},
      }
      confound_seeds = wa3_wa5_joint.prey_shrink_confound_seeds(bazaar_cells, threshold_milli=500)
      assert confound_seeds == [7]


  def test_prey_shrink_confound_empty_when_no_pp():
      cells = {
          7:  {"verdict": "Alive", "own_trade_share_milli": 750},
          11: {"verdict": "Alive", "own_trade_share_milli": 300},
      }
      assert wa3_wa5_joint.prey_shrink_confound_seeds(cells, threshold_milli=500) == []


  def test_verdict_distribution_never_same_seed_paired():
      # WA5 compares bazaar distribution vs frontier bank as distributions,
      # never same-seed paired — the function must not accept a shared seed list
      # and must operate on independent sample bags.
      bazaar_bag = ["Alive", "NoCycle", "Alive", "Alive"]
      frontier_bag = ["Alive", "Alive", "NoCycle", "Alive"]
      dist = wa3_wa5_joint.verdict_distributions(bazaar_bag, frontier_bag)
      assert dist["bazaar"]["Alive"] == 3
      assert dist["frontier"]["Alive"] == 3
      assert "same_seed_pairing" not in dist  # must not exist
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa3_wa5_joint.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa3_wa5_joint'
  ```

- [ ] **Step 2: Write `python/analysis/wa3_wa5_joint.py`**

  ```python
  """wa3_wa5_joint — WA3 + WA5 joint reader (spec §5 WA3/WA5).

  FRAME (PDR-0006): windows, not gates.
  JOINT READ (panel directive): own-trade share (WA3) IS the pirate food
  supply. High own-trade share → fewer crate haulers on the public board →
  fewer targets → PermanentPeace masquerade. NEVER read WA5 without reading
  WA3 first and checking for the prey-shrink confound on the same seeds.

  WA5: verdict distribution-vs-frontier-bank, NEVER same-seed paired.
  Use clean_seeds() (blind-born != PermanentPeace) before reading the
  verdict mix — PermanentPeace is first in the verdict chain and overrides
  cycled even when boom-bust is live (diagnostics.rs:288).

  Usage:
      python3 python/analysis/wa3_wa5_joint.py <bazaar-out-dir> \\
          --frontier-dir <frontier-out-dir> [--seeds 7 11 13 ...]
  """

  import argparse
  import json
  import pathlib
  import sys
  from collections import Counter

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
  from sweep_trophic import META_RE, RESULT_RE


  SEEDS = [
      7, 11, 13, 23, 29, 31, 37, 41, 42, 43,
      47, 53, 57, 59, 61, 67, 71, 73, 99, 101,
  ]


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def own_trade_share_milli(windows, n_haulers):
      """Fraction of (own-trade sells) / (own-trade sells + contract delivers),
      milli, FLOOR. Returns 0 if no activity.

      WA3 signal: starts near 0 (all craft broke, wage mode) and rises as
      capital accumulates (rich craft go own-trade). The joint WA5 read:
      if this rises AND PermanentPeace appears on the same seeds, that is the
      prey-shrink confound (panel warning).
      """
      total_trade = sum(w.get("trade_sold_count", 0) for w in windows)
      total_contract = sum(w.get("laden_trips", 0) for w in windows)
      denom = total_trade + total_contract
      if denom == 0:
          return 0
      return total_trade * 1000 // denom


  def clean_seeds(cells):
      """CLEAN = blind-born (or baseline) verdict != PermanentPeace.

      Cells is dict[seed] -> {verdict, ...}.
      PermanentPeace is first in the verdict chain and overrides cycled
      (diagnostics.rs:288) — seeds where war ended before market dynamics
      play out contaminate the WA5 distribution.
      """
      return [s for s, c in sorted(cells.items()) if c["verdict"] != "PermanentPeace"]


  def prey_shrink_confound_seeds(bazaar_cells, threshold_milli=500):
      """Seeds where own-trade share >= threshold AND verdict == PermanentPeace.

      These are the prey-shrink confound seeds: the bazaar's own-trade share
      shrank the pool of crate-hauler targets, causing PermanentPeace through
      prey depletion rather than predator extinction. RECORDED, not a gate;
      owner judges whether to tune the capitalization curve or accept the read.
      """
      return [
          s for s, c in sorted(bazaar_cells.items())
          if c.get("verdict") == "PermanentPeace"
          and c.get("own_trade_share_milli", 0) >= threshold_milli
      ]


  def verdict_distributions(bazaar_bag, frontier_bag):
      """Compare verdict distributions as independent sample bags (NEVER same-seed paired).

      WA5 is a DISTRIBUTION comparison, not a per-seed comparison. The two
      bags are drawn from independent campaigns; the caller must not pair them
      by seed. Returns {bazaar: Counter, frontier: Counter}.
      """
      return {
          "bazaar": dict(Counter(bazaar_bag)),
          "frontier": dict(Counter(frontier_bag)),
      }


  def load_cell_stdout(path):
      """Parse one stdout file for RESULT + META."""
      result = meta = None
      for line in path.read_text().splitlines():
          stripped = line.strip()
          m = RESULT_RE.match(stripped)
          if m:
              result = m.groupdict()
          m = META_RE.match(stripped)
          if m:
              meta = m.groupdict()
      return result, meta


  def load_windows(path):
      return [r for r in load(path) if "tick" in r]


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("bazaar_dir", help="bazaar sweep output directory")
      ap.add_argument("--frontier-dir", help="frontier bank sweep directory for WA5")
      ap.add_argument("--arm", default="baseline",
                      help="stdout arm prefix (default: baseline)")
      ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
      ap.add_argument("--prey-shrink-threshold-milli", type=int, default=500,
                      help="own-trade share threshold for prey-shrink confound flag")
      args = ap.parse_args()

      bazaar_dir = pathlib.Path(args.bazaar_dir)

      # --- WA3: own-trade share per seed (bazaar) ---
      print(
          "WA3+WA5 JOINT READ (panel directive): own-trade share IS the pirate "
          "food supply — read side-by-side always (PDR-0006: RECORDED, NEVER GATED)"
      )
      print()

      bazaar_cells = {}
      for seed in args.seeds:
          stdout_path = bazaar_dir / f"{args.arm}_s{seed}.stdout"
          jsonl_path = bazaar_dir / f"{args.arm}_s{seed}.jsonl"
          if not stdout_path.exists() or not jsonl_path.exists():
              continue
          result, meta = load_cell_stdout(stdout_path)
          if result is None or meta is None:
              continue
          windows = load_windows(jsonl_path)
          n_haulers = int(meta.get("haulers", 0))
          share = own_trade_share_milli(windows, n_haulers)
          bazaar_cells[seed] = {
              "verdict": result["verdict"],
              "own_trade_share_milli": share,
              "robs": int(result.get("robs", 0)),
              "trips": int(result.get("trips", 0)),
          }

      print("WA3 own-trade share per seed (bazaar, 50k ticks):")
      print(f"  {'seed':>6}  {'verdict':>20}  {'trade_share‰':>12}  "
            f"{'robs':>6}  {'trips':>6}")
      for seed, c in sorted(bazaar_cells.items()):
          print(
              f"  {seed:>6}  {c['verdict']:>20}  {c['own_trade_share_milli']:>12}  "
              f"  {c['robs']:>4}  {c['trips']:>5}"
          )

      # Prey-shrink confound check
      confound = prey_shrink_confound_seeds(bazaar_cells, args.prey_shrink_threshold_milli)
      if confound:
          print(
              f"\n  PREY-SHRINK CONFOUND WARNING (panel directive): seeds {confound} "
              f"show PermanentPeace WITH own-trade share >= "
              f"{args.prey_shrink_threshold_milli}‰ — "
              "this is NOT a finding that bazaar killed boom-bust; it means the "
              "own-trade share shrank the crate-hauler prey pool. Owner's call: "
              "tune capitalization curve, or accept read as 'prey-limited regime'."
          )
      else:
          print(
              f"\n  Prey-shrink confound (threshold {args.prey_shrink_threshold_milli}‰): "
              "none detected on these seeds."
          )

      # --- WA5: verdict distribution vs frontier bank ---
      print()
      clean = clean_seeds(bazaar_cells)
      dirty = [s for s in args.seeds if s in bazaar_cells and s not in clean]
      bazaar_bag = [bazaar_cells[s]["verdict"] for s in clean if s in bazaar_cells]
      print(
          f"WA5 trophic preservation — clean seeds (bazaar baseline != PermanentPeace): "
          f"{len(clean)}/{len(bazaar_cells)}; excluded={dirty}"
      )
      print(f"  bazaar verdict distribution (clean, 50k): {Counter(bazaar_bag)}")

      if args.frontier_dir:
          frontier_dir = pathlib.Path(args.frontier_dir)
          frontier_bag = []
          for seed in args.seeds:
              stdout_path = frontier_dir / f"baseline_s{seed}.stdout"
              if not stdout_path.exists():
                  continue
              result, _ = load_cell_stdout(stdout_path)
              if result is not None and result["verdict"] != "PermanentPeace":
                  frontier_bag.append(result["verdict"])
          dist = verdict_distributions(bazaar_bag, frontier_bag)
          print(f"  frontier verdict distribution (bank, 50k): {Counter(frontier_bag)}")
          print(
              f"\n  WA5 reading: distribution-vs-distribution (NEVER same-seed paired). "
              "Comparable Alive fractions = trophic dynamics preserved through "
              "the demand-mechanism swap. Divergence = the market changed piracy ecology. "
              "Either is a finding (PDR-0006 — owner's call)."
          )
          # Alive fraction comparison
          def alive_frac(bag):
              if not bag:
                  return None
              return bag.count("Alive") * 1000 // len(bag)
          ba = alive_frac(bazaar_bag)
          fa = alive_frac(frontier_bag)
          print(f"  Alive‰: bazaar={ba} frontier={fa}")
      else:
          print("  (no --frontier-dir; WA5 distribution comparison skipped)")


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa3_wa5_joint.py -v 2>&1 | tail -20
  ```

  Expected:
  ```
  test_own_trade_share_zero_when_no_trades PASSED
  test_own_trade_share_is_trade_over_trade_plus_contract PASSED
  test_clean_seeds_filters_permanent_peace PASSED
  test_prey_shrink_confound_flagged PASSED
  test_prey_shrink_confound_empty_when_no_pp PASSED
  test_verdict_distribution_never_same_seed_paired PASSED
  6 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa3_wa5_joint.py python/tests/test_wa3_wa5_joint.py
  git commit -F - <<'EOF'
  feat(lab): WA3+WA5 joint reader — own-trade share, prey-shrink confound, verdict distribution

  Encodes the panel's mandatory joint read: WA3 own-trade share and WA5 verdict
  distribution must be read side-by-side because own-trade share IS the pirate
  food supply (prey-shrink / PermanentPeace masquerade). clean_seeds filters
  PermanentPeace before the WA5 distribution read (diagnostics.rs:288 precedent).
  WA5 is distribution-vs-frontier-bank, NEVER same-seed paired (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.4: WA4 emergent-tanker reader

**Files**

- Create: `python/analysis/wa4_tanker.py`
- Create: `python/tests/test_wa4_tanker.py`

**Steps**

- [ ] **Step 1: Failing test**

  `python/tests/test_wa4_tanker.py`:

  ```python
  """WA4 emergent-tanker reader pins (spec §5 WA4; windows, not gates).

  WA4: fuel packages to non-refinery stations appear with zero fuel-specific
  dispatch code. The read: join gossip-log 'post' rows where good==FUEL_GOOD
  (good index from the META 'goods=' tail; defaults to 1 = the Fuel slot in
  scenario_bazaar) to the destination station, then filter out the three
  refinery stations. Any such package = a tanker event. The first tanker
  contract sequence (post → accept → deliver) is the console chronicle arc.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa4_tanker


  REFINERY_STATIONS = {2, 5, 9}  # scenario_bazaar refinery positions


  def _post(contract, tick, route, good, to_station):
      return {"e": "post", "tick": tick, "contract": contract,
              "route": route, "good": good, "to_station": to_station}


  def _accept(contract, tick, hauler):
      return {"e": "accept", "tick": tick, "contract": contract, "hauler": hauler}


  def _deliver(contract, tick):
      return {"e": "deliver", "tick": tick, "contract": contract}


  def test_tanker_detected_on_non_refinery_fuel_delivery():
      rows = [
          _post(1, 100, 3, 1, 4),   # good=1 (Fuel), to_station=4 (not refinery)
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert len(tankers) == 1
      assert tankers[0]["contract"] == 1
      assert tankers[0]["to_station"] == 4


  def test_no_tanker_when_fuel_goes_to_refinery():
      rows = [
          _post(1, 100, 3, 1, 2),   # good=1 (Fuel), to_station=2 (refinery)
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []


  def test_no_tanker_when_non_fuel_good_to_non_refinery():
      rows = [
          _post(1, 100, 3, 3, 4),   # good=3 (not Fuel), to_station=4
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []


  def test_first_tanker_is_earliest_by_post_tick():
      rows = [
          _post(2, 500, 5, 1, 3),   # later fuel tanker
          _accept(2, 600, 8),
          _deliver(2, 900),
          _post(1, 100, 3, 1, 4),   # earlier fuel tanker
          _accept(1, 200, 7),
          _deliver(1, 800),
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert len(tankers) == 2
      first = wa4_tanker.first_tanker(tankers)
      assert first["contract"] == 1  # earliest post_tick


  def test_tanker_undelivered_not_counted():
      # post + accept but no deliver = in-flight, not a confirmed tanker event
      rows = [
          _post(1, 100, 3, 1, 4),
          _accept(1, 200, 7),
          # no deliver
      ]
      tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                        refinery_stations=REFINERY_STATIONS)
      assert tankers == []
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa4_tanker.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa4_tanker'
  ```

- [ ] **Step 2: Write `python/analysis/wa4_tanker.py`**

  ```python
  """wa4_tanker — WA4 emergent-tanker reader (spec §5 WA4).

  FRAME (PDR-0006): windows, not gates.
  Signal: fuel packages posted to non-refinery stations, with zero
  fuel-specific dispatch code. The tanker is emergent because the poster
  (the Exchange corp) computes the same spread-clearing trigger for Fuel as
  for any other good — the refinery-to-rim price gradient is what makes the
  economics work.

  The read: gossip-log post rows where good == fuel_good_index AND
  to_station ∉ refinery_stations, joined to accept + deliver rows for
  completion confirmation. The fuel_good_index is read from the META goods=
  tail field (A0 instrument), defaulting to 1 (Fuel slot in scenario_bazaar).

  Usage:
      python3 python/analysis/wa4_tanker.py <gossip.jsonl> \\
          [--fuel-good 1] [--refinery-stations 2 5 9]
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def find_tankers(rows, fuel_good, refinery_stations):
      """Return completed fuel-package deliveries to non-refinery stations.

      A 'tanker event' = a contract where:
        - the 'post' row has good == fuel_good
        - the destination station is not a refinery
        - the contract has both an 'accept' and a 'deliver' row (completed)

      Returns list of dicts: {contract, post_tick, accept_tick, deliver_tick,
      to_station, hauler}.
      """
      posts = {}
      accepts = {}
      delivers = {}
      for row in rows:
          e = row.get("e")
          c = row.get("contract")
          if c is None:
              continue
          if e == "post":
              posts[c] = row
          elif e == "accept":
              accepts[c] = row
          elif e == "deliver":
              delivers[c] = row

      result = []
      for c, p in posts.items():
          if p.get("good") != fuel_good:
              continue
          if p.get("to_station") in refinery_stations:
              continue
          d = delivers.get(c)
          if d is None:
              continue  # in-flight, not confirmed
          a = accepts.get(c)
          result.append({
              "contract": c,
              "post_tick": p.get("tick", 0),
              "accept_tick": a["tick"] if a else None,
              "deliver_tick": d.get("tick", 0),
              "to_station": p.get("to_station"),
              "hauler": a["hauler"] if a else None,
              "route": p.get("route"),
          })

      result.sort(key=lambda x: x["post_tick"])
      return result


  def first_tanker(tankers):
      """The first (earliest post_tick) confirmed tanker event."""
      if not tankers:
          return None
      return min(tankers, key=lambda x: x["post_tick"])


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with post/accept/deliver rows")
      ap.add_argument("--fuel-good", type=int, default=1,
                      help="good index for Fuel (default 1; read from META goods= tail "
                           "when the A0 instrument is present)")
      ap.add_argument("--refinery-stations", type=int, nargs="+", default=[2, 5, 9],
                      help="station indices that are refineries in scenario_bazaar "
                           "(default: 2 5 9; read from META when the A0 instrument "
                           "carries station roles)")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      refinery_set = set(args.refinery_stations)

      tankers = find_tankers(rows, fuel_good=args.fuel_good,
                             refinery_stations=refinery_set)

      print(
          f"WA4 emergent tankers (fuel good={args.fuel_good}, "
          f"refinery stations={sorted(refinery_set)}) "
          "(RECORDED, never gated — PDR-0006):"
      )
      if not tankers:
          print(
              "  WA4 reading: NoTanker — no completed fuel packages to non-refinery "
              "stations observed. Either the fuel price gradient is too flat to clear "
              "the arbitrage trigger, or too few ticks. Recorded as a finding: "
              "the tanker is the WA4 test of price-driven emergence — its absence "
              "is equally informative (PDR-0006)."
          )
          return

      first = first_tanker(tankers)
      print(
          f"  WA4 reading: Tanker — {len(tankers)} confirmed fuel packages to "
          f"non-refinery stations."
      )
      print(
          f"  First tanker: contract={first['contract']} "
          f"post_tick={first['post_tick']} accept_tick={first['accept_tick']} "
          f"deliver_tick={first['deliver_tick']} "
          f"to_station={first['to_station']} hauler={first['hauler']}"
      )
      print(
          "  (zero fuel-specific dispatch code — pure price-driven emergence; "
          "the console chronicle arc starts here)"
      )
      print()
      print(f"  {'contract':>10}  {'post_tick':>9}  {'deliver_tick':>12}  "
            f"{'to_station':>10}  {'hauler':>6}  {'route':>5}")
      for t in tankers:
          print(
              f"  {t['contract']:>10}  {t['post_tick']:>9}  {t['deliver_tick']:>12}  "
              f"  {t['to_station']:>9}  {t['hauler']!s:>6}  {t['route']!s:>5}"
          )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 3: Run the tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa4_tanker.py -v 2>&1 | tail -15
  ```

  Expected:
  ```
  test_tanker_detected_on_non_refinery_fuel_delivery PASSED
  test_no_tanker_when_fuel_goes_to_refinery PASSED
  test_no_tanker_when_non_fuel_good_to_non_refinery PASSED
  test_first_tanker_is_earliest_by_post_tick PASSED
  test_tanker_undelivered_not_counted PASSED
  5 passed
  ```

- [ ] **Step 4: Commit**

  ```bash
  git add python/analysis/wa4_tanker.py python/tests/test_wa4_tanker.py
  git commit -F - <<'EOF'
  feat(lab): WA4 emergent-tanker reader — fuel packages to non-refinery stations, post→deliver join

  Joins gossip-log post/accept/deliver rows per contract; selects completed Fuel
  packages whose destination is not a refinery station. Any such event is the
  emergent tanker: zero fuel-specific dispatch code, pure price-driven emergence.
  Absence is equally a finding (PDR-0006).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.5: Rung-A BAZAAR anchored line + per-good route-concentration panel

**Files**

- Modify: `crates/jumpgate-core/examples/trophic_run.rs` (BAZAAR anchored line + JSONL per_station_stock/price)
- Modify: `python/analysis/sweep_trophic.py` (BAZAAR_RE + ANCHORED entry, same commit — lockstep rule)
- Modify: `python/tests/test_sweep_parsing.py` (V5 fixture, same commit — lockstep rule)
- Create: `python/analysis/wa_route_concentration.py`
- Create: `python/tests/test_wa_route_concentration.py`

**Steps**

- [ ] **Step 1: Write the failing parsing test for V5 (BAZAAR line)**

  In `python/tests/test_sweep_parsing.py`, append after the existing
  `test_v4_fuel_line_stranding_tail_parses_and_older_tails_read_none` test.
  The V5_STDOUT fixture uses a V4 base (frontier scenario with strandings):

  ```python
  V5_STDOUT = V4_STDOUT + (
      "BAZAAR seed=7 goods=10 exchange_drain_micros=-12345678 "
      "trade_sold=42 trade_bought=38 packages_posted=27 packages_delivered=24 "
      "own_trade_share_milli=350\n"
  )


  def test_v5_bazaar_line_parses_and_older_stdout_reads_none():
      parsed = sweep.parse_stdout(V5_STDOUT)
      assert parsed["bazaar"] is not None
      assert parsed["bazaar"]["goods"] == "10"
      assert parsed["bazaar"]["exchange_drain_micros"] == "-12345678"
      assert parsed["bazaar"]["trade_sold"] == "42"
      assert parsed["bazaar"]["own_trade_share_milli"] == "350"
      for legacy_text in (V1_STDOUT, V2_STDOUT, V3_STDOUT, V4_STDOUT):
          legacy = sweep.parse_stdout(legacy_text)
          assert legacy["bazaar"] is None
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py::test_v5_bazaar_line_parses_and_older_stdout_reads_none -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  KeyError: 'bazaar'
  ```
  (The `ANCHORED` dict doesn't have a `bazaar` key yet.)

- [ ] **Step 2: Write failing test for route-concentration panel**

  `python/tests/test_wa_route_concentration.py`:

  ```python
  """Per-good route-concentration panel pins (L4-C3/L5-C3 fix).

  Computed script-side from the gossip-log accept-row resource key.
  Route vectors have no goods dimension in TrophicSample (grounding §4);
  per-good HHI is derived from the accept rows, never from new per-good
  route vectors.

  Run in the same campaign as the WA5 threshold fit so the open/closed margin
  is interpretable.
  """
  import pathlib
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
  import wa_route_concentration


  def _accept(route, good, hauler=0, tick=100):
      return {"e": "accept", "tick": tick, "route": route,
              "hauler": hauler, "resource": good}


  def test_per_good_hhi_uniform_routes():
      # 4 accepts on 4 different routes for good 0: perfectly distributed = low HHI
      rows = [_accept(r, 0) for r in range(4)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      assert 0 in result
      # each route has share 1/4; HHI = 4*(1/4)^2 = 1/4 = 250 milli
      assert result[0] == 250


  def test_per_good_hhi_concentrated_routes():
      # All 4 accepts on route 0 for good 1: fully concentrated = HHI 1000
      rows = [_accept(0, 1) for _ in range(4)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      assert 1 in result
      assert result[1] == 1000


  def test_per_good_hhi_excludes_unoccupied_routes():
      # 2 routes occupied by good 2
      rows = [_accept(0, 2, tick=100), _accept(0, 2, tick=101), _accept(3, 2, tick=102)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=4)
      # route 0: 2 accepts, route 3: 1 accept; HHI = (4+1)*1000/9 = 555
      assert 2 in result
      assert result[2] == (4 + 1) * 1000 // 9


  def test_good_not_in_accepts_excluded():
      rows = [_accept(0, 0)]
      result = wa_route_concentration.per_good_hhi_milli(rows, n_routes=2)
      assert 1 not in result  # good 1 never accepted


  def test_ensemble_good_hhi_quartiles():
      # Two seeds, good 0: seed1 HHI 250, seed2 HHI 1000
      per_seed = {7: {0: 250}, 11: {0: 1000}}
      q = wa_route_concentration.ensemble_quartiles(per_seed, good=0)
      assert q is not None
      assert q[0] <= q[1] <= q[2]
  ```

  Run:
  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_wa_route_concentration.py -x 2>&1 | tail -15
  ```

  Expected failure:
  ```
  ModuleNotFoundError: No module named 'wa_route_concentration'
  ```

- [ ] **Step 3: Add BAZAAR anchored line to `trophic_run.rs` and add JSONL fields**

  The BAZAAR line and JSONL per_station_stock/price additions must land in the
  SAME commit as the BAZAAR_RE and V5 fixture (lockstep rule).

  In `trophic_run.rs`, in `sample_json` (after the existing `refuel_spend_micros`
  key, around line 302), add the per_station_stock and per_station_price fields
  following the additive block pattern:

  ```rust
          // --- bazaar lab fields (rung A, spec §5 WA1-4; windows, not gates).
          // Additive: every pre-bazaar key above is byte-untouched.
          // per_station_stock / per_station_price: n_stations × n_goods flat
          // matrices (station-major, good-minor). Existing
          // per_station_fuel_stock / per_station_fuel_price stay byte-identical.
          "per_station_stock": s.per_station_stock,
          "per_station_price": s.per_station_price,
          "trade_sold_count": s.trade_sold_count,
          "trade_bought_count": s.trade_bought_count,
  ```

  In `diagnostics.rs` `TrophicSample` struct, add after the last field of the
  world-gets-big group (after `refuel_spend_micros`, line ~234):

  ```rust
      // --- bazaar lab fields (rung A, spec §5 WA1-5; windows, not gates).
      // Additive: every pre-bazaar JSONL key is untouched. ---
      /// Per-station × per-good stock snapshot at the sample point.
      /// Flat matrix: station-major, good-minor. Length = n_stations × n_goods.
      /// Zero-len when the scenario has no bazaar goods (trophic/frontier
      /// are byte-identical: the field is present but empty — additive).
      pub per_station_stock: Vec<i64>,
      /// Per-station × per-good price snapshot. Same shape as per_station_stock.
      pub per_station_price: Vec<i64>,
      /// TradeSold events in the window (own-cargo sells, WA3 numerator).
      pub trade_sold_count: u32,
      /// TradeBought events in the window.
      pub trade_bought_count: u32,
  ```

  In `sample_window` (`diagnostics.rs:541`), populate these fields from the
  event stream (TradeSold/TradeBought counts, window-filtered) and from the
  world's station stock/price columns at the sample tick. When the scenario has
  no goods beyond Ore/Fuel, the Vec is empty — that is correct: trophic and
  frontier produce zero-length per_station_stock, which is byte-identical in
  their JSONL (field present, value `[]`).

  In `trophic_run.rs`, after the FUEL line (around line 788), add the BAZAAR
  anchored line. It is config-gated: only print when the world has more than 2
  goods (i.e., scenario_bazaar). In v1 the check is `n_goods > 2`; trophic and
  frontier both have n_goods=2 and thus never print BAZAAR.

  ```rust
      // The BAZAAR line (rung A, spec §5; a window, not a gate).
      // Config-gated: only printed when the scenario has bazaar goods (n_goods>2).
      // Lockstep rule: BAZAAR_RE in sweep_trophic.py lands in the SAME commit.
      let n_goods = world.n_goods(); // method on World returning GoodsCfg count
      if n_goods > 2 {
          let trade_sold_total: u64 = samples.iter().map(|s| u64::from(s.trade_sold_count)).sum();
          let trade_bought_total: u64 = samples.iter().map(|s| u64::from(s.trade_bought_count)).sum();
          let packages_posted: u64 = samples.iter().map(|s| u64::from(s.packages_posted)).sum();
          let packages_delivered: u64 = samples.iter().map(|s| u64::from(s.laden_trips)).sum();
          let total_trips = trade_sold_total + packages_delivered;
          let own_trade_share_milli = if total_trips > 0 {
              trade_sold_total.saturating_mul(1000) / total_trips
          } else {
              0
          };
          // Exchange drain: sum of per-window exchange_treasury_micros deltas.
          // Negative = net outflow from the Exchange (the solvency honesty read,
          // OD-2; printed as a standing read, not a gate).
          let exchange_drain = exchange_drain_micros(&samples);
          println!(
              "BAZAAR seed={} goods={} exchange_drain_micros={} \
               trade_sold={} trade_bought={} packages_posted={} \
               packages_delivered={} own_trade_share_milli={}",
              args.seed, n_goods, exchange_drain,
              trade_sold_total, trade_bought_total,
              packages_posted, packages_delivered, own_trade_share_milli,
          );
      }
  ```

  The `exchange_drain_micros` helper computes the run-cumulative Exchange
  treasury delta from the sample sequence:

  ```rust
  fn exchange_drain_micros(samples: &[TrophicSample]) -> i64 {
      // Sum of per-window exchange treasury deltas (negative = drain).
      // Uses the exchange_treasury_micros field added to TrophicSample in A3.
      // Zero sentinel when the field is absent (pre-bazaar scenarios).
      if samples.is_empty() {
          return 0;
      }
      let first = samples.first().map_or(0, |s| s.exchange_treasury_micros);
      let last = samples.last().map_or(0, |s| s.exchange_treasury_micros);
      last - first
  }
  ```

  **Note to builder:** `exchange_treasury_micros` is a new field in
  `TrophicSample` that must be added alongside the other bazaar fields. It
  holds the Exchange corp's treasury snapshot per window (A3 unlocks this;
  if A3 has not landed, zero-init the field). Also add `packages_posted` to
  `TrophicSample` (ContractOffered events in the window from the Exchange corp,
  which requires knowing the Exchange corp index — derive from config.

  **After confirming the impl builds**, add the BAZAAR_RE to
  `python/analysis/sweep_trophic.py` in the same commit.

  In `sweep_trophic.py`, add after `FUEL_RE`:

  ```python
  # The BAZAAR line (rung A, spec §5) — config-gated (only when n_goods > 2).
  # Lockstep rule: this regex and the Rust println! land in the SAME commit.
  # Optional: absent from trophic/frontier runs, reads None in parse_stdout.
  BAZAAR_RE = re.compile(
      r"^BAZAAR seed=(?P<seed>\d+) goods=(?P<goods>\d+) "
      r"exchange_drain_micros=(?P<exchange_drain_micros>-?\d+) "
      r"trade_sold=(?P<trade_sold>\d+) trade_bought=(?P<trade_bought>\d+) "
      r"packages_posted=(?P<packages_posted>\d+) "
      r"packages_delivered=(?P<packages_delivered>\d+) "
      r"own_trade_share_milli=(?P<own_trade_share_milli>\d+)$"
  )
  ```

  And in the `ANCHORED` dict, add:

  ```python
  ANCHORED = {
      "result": (True, RESULT_RE),
      "media": (True, MEDIA_RE),
      "meta": (False, META_RE),
      "fuel": (False, FUEL_RE),
      "bazaar": (False, BAZAAR_RE),  # config-gated: absent on trophic/frontier
  }
  ```

- [ ] **Step 4: Write `python/analysis/wa_route_concentration.py`**

  ```python
  """wa_route_concentration — rung-A per-good route-concentration panel.

  L4-C3 / L5-C3 fix: per-good route traffic and rob HHI beside WA5 verdict,
  from the accept-row 'resource' key in the gossip-log. Route vectors have no
  goods dimension today (grounding §4); this panel is entirely script-side.

  Run in the SAME campaign as the WA5 threshold fit (Part 3, DL5-2) so the
  open/closed margin is interpretable. The clumped-topology factory constraint
  (L1-C2) means goods should travel on a small subset of routes; high HHI per
  good is EXPECTED and is the design proof that clumped topology is working.
  A low HHI (< ~200) means self-averaging has started — the L5-C3 warning.

  Usage:
      python3 python/analysis/wa_route_concentration.py <gossip.jsonl> \\
          [--n-routes N]
  """

  import argparse
  import json


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def per_good_hhi_milli(rows, n_routes):
      """Per-good occupied-route HHI (milli), from gossip-log accept rows.

      Only rows with e='accept' and a 'resource' key are counted (A0
      instrument: accept row gains resource+reward). Returns dict[good_index]
      -> HHI_milli for goods with at least one accept.
      """
      counts = {}  # good -> {route -> count}
      for row in rows:
          if row.get("e") != "accept":
              continue
          g = row.get("resource")
          r = row.get("route")
          if g is None or r is None:
              continue
          counts.setdefault(g, {}).setdefault(r, 0)
          counts[g][r] += 1

      result = {}
      for good, route_counts in counts.items():
          total = sum(route_counts.values())
          if total == 0:
              continue
          hhi = sum(c * c for c in route_counts.values()) * 1000 // (total * total)
          result[good] = hhi
      return result


  def ensemble_quartiles(per_seed, good):
      """Lower-index quartiles of per-good HHI across seeds.

      per_seed: dict[seed] -> dict[good -> hhi_milli]
      Returns (q1, median, q3) or None if fewer than 2 seeds have data.
      """
      vals = sorted(v[good] for v in per_seed.values() if good in v)
      n = len(vals)
      if n < 2:
          return None
      return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("gossip_log", help="gossip-log JSONL with accept rows (resource key required)")
      ap.add_argument("--n-routes", type=int, default=90,
                      help="n_stations^2 (default 90 = 10^2 for scenario_bazaar)")
      args = ap.parse_args()

      rows = load(args.gossip_log)
      result = per_good_hhi_milli(rows, n_routes=args.n_routes)

      print(
          f"rung-A per-good route concentration (HHI, milli) "
          f"— {sum(1 for r in rows if r.get('e') == 'accept')} accept events "
          "(RECORDED, never gated — PDR-0006; L4-C3/L5-C3 fix):"
      )
      if not result:
          print(
              "  no accept rows with 'resource' key (pre-A0 run — the A0 instrument "
              "must add resource+reward to the accept row)"
          )
          return

      print(f"  {'good':>4}  {'HHI‰':>6}  reading")
      for good in sorted(result):
          hhi = result[good]
          if hhi >= 600:
              reading = "concentrated (clumped topology working)"
          elif hhi >= 200:
              reading = "moderate"
          else:
              reading = "low (self-averaging warning — L5-C3)"
          print(f"  {good:>4}  {hhi:>6}  {reading}")

      print(
          "\n  Interpretation (panel L4-C3/L5-C3): high HHI per good is EXPECTED "
          "under clumped topology — it is the design proof. Low HHI means a good "
          "is spreading over all routes (self-averaging); increase goods-topology "
          "concentration or run the DL5-2 threshold fit to check margin."
      )


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 5: Run all tests**

  ```bash
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/test_sweep_parsing.py python/tests/test_wa_route_concentration.py -v 2>&1 | tail -30
  ```

  Expected:
  ```
  test_v5_bazaar_line_parses_and_older_stdout_reads_none PASSED
  test_per_good_hhi_uniform_routes PASSED
  test_per_good_hhi_concentrated_routes PASSED
  test_per_good_hhi_excludes_unoccupied_routes PASSED
  test_good_not_in_accepts_excluded PASSED
  test_ensemble_good_hhi_quartiles PASSED
  ```

  Also run Rust tests to confirm the TrophicSample additions compile and the
  JSONL emitter still passes the existing sample_json shape tests:

  ```bash
  cargo test -p jumpgate-core --all-targets -- sample 2>&1 | tail -15
  ```

  Expected: all existing sample tests pass.

- [ ] **Step 6: Commit (LOCKSTEP — Rust println! + Python regex + V5 fixture all in one commit)**

  ```bash
  git add \
    crates/jumpgate-core/examples/trophic_run.rs \
    crates/jumpgate-core/src/diagnostics.rs \
    python/analysis/sweep_trophic.py \
    python/tests/test_sweep_parsing.py \
    python/analysis/wa_route_concentration.py \
    python/tests/test_wa_route_concentration.py
  git commit -F - <<'EOF'
  feat(lab): BAZAAR anchored line + per_station_stock/price JSONL + route-concentration panel

  BAZAAR line (config-gated: n_goods > 2; absent on trophic/frontier) carries
  exchange drain, channel mix counts, and own_trade_share_milli. BAZAAR_RE lands
  in the same commit as the println! (lockstep rule); V5 fixture appended to
  test_sweep_parsing.py. per_station_stock / per_station_price fields added to
  TrophicSample (additive; empty vec on trophic/frontier — byte-identical).
  wa_route_concentration panel computes per-good HHI from gossip-log accept-row
  resource keys (script-side, L4-C3/L5-C3 fix).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.6: Per-station epilogue block + chronicle enrichment

**Files**

- Modify: `crates/jumpgate-core/examples/trophic_run.rs`

**Steps**

- [ ] **Step 1: Write failing test for the per-station epilogue output**

  Add to `crates/jumpgate-core/tests/` or as a doc-test; here we use an
  integration check that the chronicle printer produces per-station rows in the
  JSONL-free path. Because the chronicle is stdout-only, test by running a
  short trophic_run with `--chronicle` and checking the output contains the
  per-station epilogue marker string.

  In `crates/jumpgate-core/tests/chronicle_epilogue.rs`:

  ```rust
  /// Confirm that `--chronicle` output contains per-station epilogue lines.
  /// These are printer-only (PDR-0006: windows, never gates) and must be
  /// present whenever the chronicle flag is set on a scenario with stations.
  #[test]
  fn per_station_epilogue_appears_in_chronicle() {
      use std::process::Command;
      let out = Command::new(env!("CARGO_BIN_EXE_trophic_run"))
          .args([
              "--scenario", "trophic",
              "--seed", "7",
              "--ticks", "500",
              "--chronicle",
          ])
          .output()
          .expect("trophic_run");
      let stdout = String::from_utf8_lossy(&out.stdout);
      // Per-station epilogue block header must appear
      assert!(
          stdout.contains("=== per-station epilogue"),
          "no per-station epilogue in chronicle output:\n{stdout}"
      );
  }
  ```

  Run:
  ```bash
  cargo test -p jumpgate-core --all-targets -- per_station_epilogue 2>&1 | tail -15
  ```

  Expected failure:
  ```
  assertion `left == right` failed: no per-station epilogue in chronicle output
  ```

- [ ] **Step 2: Add per-station epilogue block to `trophic_run.rs`**

  In the `print_chronicle` function in `trophic_run.rs`, after the per-craft
  loop (after the closing `}` of the craft loop around line 611), add the
  per-station epilogue block. It receives a `&[TrophicSample]` reference
  (already available from the caller's `simulate` result):

  ```rust
  /// Per-station summary epilogue in the chronicle (synthesis cut Part 3).
  /// Threaded &[TrophicSample] for the WA1 protagonist (station starvation map).
  /// Printer-side only (PDR-0006: windows, never gates).
  fn print_station_epilogue(world: &World, samples: &[TrophicSample]) {
      let n_stations = world.n_stations();
      if n_stations == 0 || samples.is_empty() {
          return;
      }
      println!("=== per-station epilogue (PDR-0006: recorded, never gated) ===");
      let n_goods = world.n_goods();
      let last = samples.last().unwrap();

      for s in 0..n_stations {
          // Final stock + price snapshot from the last sample window.
          let stock_range: Vec<i64> = if n_goods > 0 && !last.per_station_stock.is_empty() {
              (0..n_goods)
                  .map(|g| last.per_station_stock.get(s * n_goods + g).copied().unwrap_or(0))
                  .collect()
          } else {
              // Pre-bazaar scenario: fall back to fuel-only read
              vec![
                  last.per_station_fuel_stock.get(s).copied().unwrap_or(0),
              ]
          };
          let price_range: Vec<i64> = if n_goods > 0 && !last.per_station_price.is_empty() {
              (0..n_goods)
                  .map(|g| last.per_station_price.get(s * n_goods + g).copied().unwrap_or(0))
                  .collect()
          } else {
              vec![
                  last.per_station_fuel_price.get(s).copied().unwrap_or(0),
              ]
          };
          // Count zero-stock windows per good (WA1 starvation map).
          let zero_runs: Vec<u32> = if n_goods > 0 && !samples[0].per_station_stock.is_empty() {
              (0..n_goods)
                  .map(|g| {
                      samples
                          .iter()
                          .filter(|w| {
                              let idx = s * n_goods + g;
                              w.per_station_stock.get(idx).copied().unwrap_or(0) == 0
                          })
                          .count() as u32
                  })
                  .collect()
          } else {
              vec![]
          };
          // Lurking pirates at this station in the final window.
          let lurking = last.per_station_lurking_pirates.get(s).copied().unwrap_or(0);
          println!(
              "  station {s}: final_stock={stock_range:?} final_price={price_range:?} \
               zero_stock_windows={zero_runs:?} lurking_pirates={lurking}"
          );
      }
  }
  ```

  In `print_chronicle`, call `print_station_epilogue(world, samples)` after the
  per-craft loop. Update the function signature to accept `samples`:

  ```rust
  fn print_chronicle(world: &World, samples: &[TrophicSample], gossip_min_micros: i64) {
      // ...existing per-craft loop unchanged...
      print_station_epilogue(world, samples);
  }
  ```

  In `main`, update the call site:

  ```rust
  if args.chronicle {
      print_chronicle(&world, &samples, args.chronicle_gossip_min_micros);
  }
  ```

- [ ] **Step 3: Run the test**

  ```bash
  cargo test -p jumpgate-core --all-targets -- per_station_epilogue 2>&1 | tail -10
  ```

  Expected:
  ```
  test per_station_epilogue_appears_in_chronicle ... ok
  ```

- [ ] **Step 4: Run full workspace tests to confirm no regressions**

  ```bash
  cargo test --workspace 2>&1 | tail -20
  ```

  Expected: all tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add \
    crates/jumpgate-core/examples/trophic_run.rs \
    crates/jumpgate-core/tests/chronicle_epilogue.rs
  git commit -F - <<'EOF'
  feat(chronicle): per-station epilogue block — starvation map + lurking pirates

  Adds per-station epilogue to the chronicle printer (synthesis cut Part 3):
  final stock/price per good, zero-stock window count per good (WA1 protagonist),
  and lurking pirate count. Threaded &[TrophicSample] into print_chronicle.
  Printer-side only (PDR-0006: windows, never gates).

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

---

### Task A6.7: 20-seed ensemble + rung-A exit digest + console packet

**Files**

- Create: `python/analysis/sweep_bazaar.py` (the WA sweep runner)
- Create: `runs/gag-rung-a/` (builder creates at run time; never staged)
- No Rust changes (pure Python + shell procedure)

**Steps**

- [ ] **Step 1: Write the sweep runner**

  `python/analysis/sweep_bazaar.py`:

  ```python
  """sweep_bazaar — rung-A WA1-5 20-seed campaign runner.

  FRAME (PDR-0006): every number here is a designer's window, never a gate.
  Runs the bazaar scenario over the 20 SEEDS at both run lengths (50k for WA5
  bank-comparability; 100k for per-good/WA1-4 reads), aggregates the
  readers (WA1 survival, WA2 spread closure, WA3+WA5 joint, WA4 tanker,
  per-good route concentration), and prints the rung-A console packet.

  The 50k run is bank-comparable with frontier (WA5 distribution). The 100k
  run carries the per-good reads. Both use the 20-seed W4 SEEDS ladder.

  Exit condition (RECORDED, NEVER GATED — PDR-0006):
    1. sha256 digest of trophic + frontier vs the A0 baseline (from A6.0).
    2. WA1-5 readings printed.
    3. First-look chronicle materials banked to runs/gag-rung-a/.
  Any divergence in step 1 is a determinism break — STOP, bisect, never rationalize.

  Usage:
      python3 python/analysis/sweep_bazaar.py \\
          --out runs/gag-rung-a \\
          --baseline-dir runs/gag-a6-baseline \\
          [--seeds 7 11 13] [--ticks-short 50000] [--ticks-long 100000]
  """

  import argparse
  import hashlib
  import json
  import pathlib
  import subprocess
  import sys

  sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
  from sweep_trophic import META_RE, RESULT_RE, parse_stdout, runner_cmd
  from w4_grid import SEEDS, quartiles, clean_seeds as w4_clean_seeds
  from wa1_survival import stock_runs, stalled_consumers, read_transport_table
  from wa2_spread_closure import spread_series, summarize as wa2_summarize
  from wa3_wa5_joint import (
      own_trade_share_milli,
      clean_seeds as wa3_clean_seeds,
      prey_shrink_confound_seeds,
      verdict_distributions,
      load_cell_stdout,
  )
  from wa4_tanker import find_tankers, first_tanker
  from wa_route_concentration import per_good_hhi_milli


  def load(path):
      with open(path) as f:
          return [json.loads(line) for line in f if line.strip()]


  def sha256_file(path):
      h = hashlib.sha256()
      with open(path, "rb") as f:
          for chunk in iter(lambda: f.read(65536), b""):
              h.update(chunk)
      return h.hexdigest()


  def run_one_seed(scenario, seed, ticks, out_dir, arm="baseline"):
      """Run trophic_run for one (scenario, seed, ticks) cell."""
      jsonl = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.jsonl"
      gossip = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.gossip.jsonl"
      stdout_path = out_dir / f"{arm}-{scenario}_s{seed}_t{ticks}.stdout"

      if stdout_path.exists() and jsonl.exists() and gossip.exists():
          # Already banked; skip re-run (idempotent sweep).
          stdout_text = stdout_path.read_text()
      else:
          cmd = runner_cmd(scenario, seed, ticks, jsonl, [])
          cmd += ["--gossip-log", str(gossip), "--chronicle"]
          proc = subprocess.run(cmd, capture_output=True, text=True)
          if proc.returncode != 0:
              sys.stderr.write(proc.stdout + proc.stderr)
              raise SystemExit(f"run failed: {scenario} seed={seed} ticks={ticks}")
          stdout_path.write_text(proc.stdout)
          stdout_text = proc.stdout

      parsed = parse_stdout(stdout_text)
      all_rows = load(jsonl)
      windows = [r for r in all_rows if "tick" in r]
      gossip_rows = load(gossip)
      return parsed, windows, gossip_rows, jsonl, gossip


  def verify_digest_vs_baseline(baseline_dir, out_dir, seeds=(7, 23)):
      """Cross-branch digest: trophic + frontier vs A0 baseline.

      Compares sha256 of stdout + JSONL + gossip-log for seeds 7 and 23
      at 2000 ticks (the same cells pinned in A6.0). Any divergence is a
      determinism break — print and abort; never rationalize.
      """
      print("\n=== rung-A exit digest (A0 baseline vs HEAD) ===")
      ok = True
      for scenario in ("trophic", "frontier"):
          for s in seeds:
              base = pathlib.Path(baseline_dir)
              head_dir = out_dir
              # Re-run at 2000 ticks for the digest comparison
              for ext in ("out", "jsonl", "gossip.jsonl"):
                  base_f = base / f"{scenario}-base-s{s}.{ext}"
                  head_f = head_dir / f"digest-{scenario}_s{s}_t2000.{ext}"
                  if not base_f.exists():
                      print(f"  SKIP {base_f.name} (no baseline — run A6.0 first)")
                      continue
                  if not head_f.exists():
                      # Need to produce the head file
                      jsonl_h = head_dir / f"digest-{scenario}_s{s}_t2000.jsonl"
                      gossip_h = head_dir / f"digest-{scenario}_s{s}_t2000.gossip.jsonl"
                      stdout_h = head_dir / f"digest-{scenario}_s{s}_t2000.out"
                      cmd = runner_cmd(scenario, s, 2000, jsonl_h, [])
                      cmd += ["--gossip-log", str(gossip_h)]
                      proc = subprocess.run(cmd, capture_output=True, text=True)
                      if proc.returncode != 0:
                          raise SystemExit(f"digest run failed: {scenario} seed={s}")
                      stdout_h.write_text(proc.stdout)
                  base_digest = sha256_file(base_f)
                  head_f_resolved = head_dir / f"digest-{scenario}_s{s}_t2000.{ext}"
                  head_digest = sha256_file(head_f_resolved)
                  match = "OK" if base_digest == head_digest else "DIVERGE"
                  print(f"  {match}  {scenario} s={s} {ext}: "
                        f"base={base_digest[:12]}... head={head_digest[:12]}...")
                  if match != "OK":
                      ok = False
      if not ok:
          print("\n  DETERMINISM BREAK: one or more files diverged from the A0 baseline.")
          print("  STOP — bisect commit-by-commit. Do NOT rationalize.")
      else:
          print("  All digest checks pass — rung-A mechanics are behavior-equivalent "
                "on trophic and frontier (the OD-1 hash-neutrality confirmation).")
      return ok


  def main():
      ap = argparse.ArgumentParser(description=__doc__)
      ap.add_argument("--out", default="runs/gag-rung-a")
      ap.add_argument("--baseline-dir", default="runs/gag-a6-baseline")
      ap.add_argument("--seeds", type=int, nargs="+", default=SEEDS)
      ap.add_argument("--ticks-short", type=int, default=50_000,
                      help="50k for WA5 bank-comparability")
      ap.add_argument("--ticks-long", type=int, default=100_000,
                      help="100k for per-good reads (WA1-4)")
      ap.add_argument("--frontier-baseline-dir",
                      help="frontier bank sweep dir for WA5 comparison")
      args = ap.parse_args()

      out_dir = pathlib.Path(args.out)
      out_dir.mkdir(parents=True, exist_ok=True)

      print(
          "sweep_bazaar rung-A exit campaign "
          f"(PDR-0006: RECORDED, NEVER GATED) — "
          f"seeds={len(args.seeds)} ticks_short={args.ticks_short} "
          f"ticks_long={args.ticks_long}"
      )

      # ---- Step 1: Behavior digest ----
      ok = verify_digest_vs_baseline(
          args.baseline_dir, out_dir, seeds=(7, 23)
      )
      if not ok:
          raise SystemExit(1)

      # ---- Step 2: 50k ensemble (WA5 bank-comparability) ----
      print("\n=== 50k ensemble (20 seeds, WA5 verdict distribution) ===")
      cells_50k = {}
      for seed in args.seeds:
          parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
              "bazaar", seed, args.ticks_short, out_dir
          )
          haulers = int(parsed["meta"]["haulers"]) if parsed.get("meta") else 0
          share = own_trade_share_milli(windows, haulers)
          cells_50k[seed] = {
              "verdict": parsed["result"]["verdict"] if parsed.get("result") else "Unknown",
              "own_trade_share_milli": share,
              "robs": int(parsed["result"].get("robs", 0)) if parsed.get("result") else 0,
              "trips": int(parsed["result"].get("trips", 0)) if parsed.get("result") else 0,
              "windows": windows,
              "gossip": gossip_rows,
          }
          print(
              f"  seed={seed}: verdict={cells_50k[seed]['verdict']} "
              f"robs={cells_50k[seed]['robs']} trips={cells_50k[seed]['trips']} "
              f"own_trade‰={share}"
          )

      # WA3+WA5 joint
      confound = prey_shrink_confound_seeds(cells_50k, threshold_milli=500)
      if confound:
          print(f"\n  PREY-SHRINK CONFOUND WARNING: seeds {confound} — "
                "PermanentPeace with high own-trade share. See wa3_wa5_joint for detail.")
      clean = wa3_clean_seeds(cells_50k)
      bazaar_bag = [cells_50k[s]["verdict"] for s in clean]
      print(f"\nWA5 verdict distribution (clean seeds, n={len(clean)}): "
            f"{dict(sorted((v, bazaar_bag.count(v)) for v in set(bazaar_bag)))}")

      if args.frontier_baseline_dir:
          frontier_bag = []
          fdir = pathlib.Path(args.frontier_baseline_dir)
          for p in sorted(fdir.glob("baseline_s*.stdout")):
              result, _ = load_cell_stdout(p)
              if result and result["verdict"] != "PermanentPeace":
                  frontier_bag.append(result["verdict"])
          dist = verdict_distributions(bazaar_bag, frontier_bag)
          print(f"WA5 frontier distribution (bank): "
                f"{dict(sorted((v, frontier_bag.count(v)) for v in set(frontier_bag)))}")
          alive_b = bazaar_bag.count("Alive") * 1000 // max(len(bazaar_bag), 1)
          alive_f = frontier_bag.count("Alive") * 1000 // max(len(frontier_bag), 1)
          print(f"WA5 Alive‰: bazaar={alive_b} frontier={alive_f} "
                "(distribution-vs-distribution, NEVER same-seed paired)")

      # ---- Step 3: 100k ensemble (WA1-4 per-good reads) ----
      print("\n=== 100k ensemble (20 seeds, WA1-4 per-good reads) ===")
      all_hhi = {}
      tanker_total = 0
      first_tanker_global = None

      for seed in args.seeds:
          parsed, windows, gossip_rows, jsonl_path, gossip_path = run_one_seed(
              "bazaar", seed, args.ticks_long, out_dir
          )
          haulers = int(parsed["meta"]["haulers"]) if parsed.get("meta") else 0
          n_stations = int(parsed["meta"]["stations"]) if parsed.get("meta") else 10
          n_goods = int(parsed["bazaar"]["goods"]) if parsed.get("bazaar") else 0

          # WA1: survival
          if n_goods > 0:
              runs = stock_runs(windows, n_stations, n_goods)
              starving = [r for r in runs if r["max_zero_run"] > 0]
              print(
                  f"  WA1 seed={seed}: {len(starving)}/{n_stations * n_goods} "
                  f"station×good pairs had zero-stock windows"
              )

          # WA2: spread closure
          s_result = spread_series(gossip_rows)
          wa2_rows = wa2_summarize(s_result)
          decaying = sum(1 for r in wa2_rows if r["decaying"] is True)
          print(
              f"  WA2 seed={seed}: {decaying}/{len(wa2_rows)} route×good pairs "
              "show spread decay"
          )

          # WA4: tankers
          fuel_good = 1  # Fuel slot in scenario_bazaar
          refinery_set = {2, 5, 9}  # OD-3 refinery stations in scenario_bazaar
          tankers = find_tankers(gossip_rows, fuel_good=fuel_good,
                                 refinery_stations=refinery_set)
          tanker_total += len(tankers)
          ft = first_tanker(tankers)
          if ft is not None:
              if first_tanker_global is None or ft["post_tick"] < first_tanker_global["post_tick"]:
                  first_tanker_global = {**ft, "seed": seed}
          print(
              f"  WA4 seed={seed}: {len(tankers)} tanker events "
              f"({'first at t=' + str(ft['post_tick']) if ft else 'none'})"
          )

          # Per-good route concentration
          hhi = per_good_hhi_milli(gossip_rows, n_routes=n_stations * n_stations)
          all_hhi[seed] = hhi

      # Route concentration ensemble summary (L4-C3/L5-C3)
      if all_hhi:
          all_goods = sorted({g for h in all_hhi.values() for g in h})
          print("\nPer-good route concentration (HHI‰ quartiles, 100k, 20 seeds):")
          print(f"  {'good':>4}  {'q1':>6}  {'median':>6}  {'q3':>6}  reading")
          for g in all_goods:
              q = ensemble_quartiles_good(all_hhi, g)
              if q is None:
                  continue
              q1, med, q3 = q
              reading = ("concentrated" if med >= 600
                         else "moderate" if med >= 200
                         else "LOW (self-averaging — L5-C3)")
              print(f"  {g:>4}  {q1:>6}  {med:>6}  {q3:>6}  {reading}")

      # WA4 summary
      print(f"\nWA4 emergent tankers (100k, 20 seeds): {tanker_total} total tanker events")
      if first_tanker_global:
          print(
              f"  First tanker across ensemble: seed={first_tanker_global['seed']} "
              f"contract={first_tanker_global['contract']} "
              f"to_station={first_tanker_global['to_station']} "
              f"post_tick={first_tanker_global['post_tick']} "
              "(console chronicle: 'the market fixed the fuel desert')"
          )
      else:
          print("  WA4 reading: NoTanker across ensemble — recorded as a finding.")

      print(
          "\n=== rung-A exit complete (PDR-0006: RECORDED, NEVER GATED) ===\n"
          "Judgment sessions to bank same-day:\n"
          "  1. 'the market fixed the fuel desert' (WA4 tanker arc)\n"
          "  2. 'the trader who flew too close' (WA3 mode-flip arc)\n"
          "  3. trophic preservation read (WA5 distribution vs frontier bank)"
      )


  def ensemble_quartiles_good(all_hhi, good):
      vals = sorted(v[good] for v in all_hhi.values() if good in v)
      n = len(vals)
      if n < 2:
          return None
      return (vals[(n - 1) // 4], vals[(n - 1) // 2], vals[3 * (n - 1) // 4])


  if __name__ == "__main__":
      main()
  ```

- [ ] **Step 2: Run the ensemble (50k pass first, then 100k)**

  ```bash
  # 50k pass (WA5 bank-comparability)
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 50000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101
  ```

  If the digest step diverges, STOP — bisect commit-by-commit. Do NOT
  rationalize.

  ```bash
  # 100k pass (per-good reads)
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 100000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101
  ```

  The runner is idempotent: cells already in `runs/gag-rung-a/` are not re-run.

- [ ] **Step 3: Bank the console packet**

  Capture the full output of both passes to the same-day post directory:

  ```bash
  DATE=$(date +%Y-%m-%d)
  mkdir -p docs/superpowers/posts/${DATE}-bazaar-rung-a
  python3 python/analysis/sweep_bazaar.py \
    --out runs/gag-rung-a \
    --baseline-dir runs/gag-a6-baseline \
    --ticks-short 50000 --ticks-long 100000 \
    --seeds 7 11 13 23 29 31 37 41 42 43 47 53 57 59 61 67 71 73 99 101 \
    2>&1 | tee docs/superpowers/posts/${DATE}-bazaar-rung-a/console-packet.txt
  ```

  Also run the chronicle for the first-tanker seed (the WA4 arc):

  ```bash
  # Identify the first-tanker seed from the console-packet above, then:
  TANKER_SEED=<seed from console-packet>
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario bazaar --seed $TANKER_SEED --ticks 100000 \
    --chronicle \
    --gossip-log runs/gag-rung-a/chronicle-bazaar-s${TANKER_SEED}.gossip.jsonl \
    --jsonl     runs/gag-rung-a/chronicle-bazaar-s${TANKER_SEED}.jsonl \
    > docs/superpowers/posts/${DATE}-bazaar-rung-a/chronicle-tanker-seed${TANKER_SEED}.txt
  ```

  The chronicle output is the "the market fixed the fuel desert" story artifact.
  Bank it same-day (the capture-story-artifacts standing directive).

- [ ] **Step 4: Commit the sweep runner**

  ```bash
  git add python/analysis/sweep_bazaar.py
  git commit -F - <<'EOF'
  feat(lab): sweep_bazaar — rung-A 20-seed campaign runner + exit digest

  20-seed × 50k+100k ensemble runner for WA1-5 and per-good route
  concentration (L4-C3/L5-C3). Behavior-digest step verifies trophic + frontier
  vs the A0 baseline before any WA reads (determinism gate; any divergence = STOP).
  Idempotent: banked cells in runs/ are not re-run.

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  ```

- [ ] **Step 5: Confirm full workspace tests still pass**

  ```bash
  cargo test --workspace 2>&1 | tail -15
  PYTHONPATH=/home/john/jumpgate/python pytest python/tests/ -q 2>&1 | tail -15
  ```

  Expected: all green.

- [ ] **Step 6: Record the WA1-5 readings as observations in filigree**

  After reading the console packet, use `filigree observation_create` (MCP) or
  `filigree observation create` (CLI) to record:
  - WA1 reading (starvation map): `file_path` = `runs/gag-rung-a/`, one
    observation per surprising finding (rim starvation → finding; universal →
    finding requiring owner attention).
  - WA4 first tanker tick and seed (if found): a positive finding.
  - WA5 Alive‰ comparison vs frontier bank.
  - Any prey-shrink confound seeds (WA3 warning).

  The owner reads the console packet and the chronicle materials, then judges
  the rung (PDR-0006). No gate. No threshold. The story arc is the criterion.

---

## Phase A6 task index

| ID | Title |
|----|-------|
| A6.0 | Behavior digest baseline (A0 tip pinned) |
| A6.1 | WA1 survival-by-market reader |
| A6.2 | WA2 spread-closure reader |
| A6.3 | WA3 + WA5 joint reader |
| A6.4 | WA4 emergent-tanker reader |
| A6.5 | BAZAAR anchored line + per_station_stock/price JSONL + route-concentration panel |
| A6.6 | Per-station epilogue block + chronicle enrichment |
| A6.7 | 20-seed ensemble + rung-A exit digest + console packet |

## Cross-task constraints (encoded in steps above)

- **Lockstep rule (A6.5):** BAZAAR anchored println! + BAZAAR_RE + V5 fixture
  in the same commit. No exceptions.
- **Joint-read mandate (A6.3):** WA3 and WA5 are never read in isolation;
  `wa3_wa5_joint.py` enforces this structurally.
- **Clean seeds (A6.3, A6.7):** `clean_seeds()` filters PermanentPeace before
  every WA5 distribution read. PermanentPeace is first in the verdict chain
  (diagnostics.rs:288) and overrides cycled.
- **Distribution-vs-distribution (A6.3, A6.7):** WA5 is never same-seed
  paired; the two bags are independent.
- **Hauler slice from META (A6.3, A6.7):** `int(meta["haulers"])` from the
  META line; never a module-level constant (L5-C2).
- **Digest-first (A6.7):** `verify_digest_vs_baseline` runs before any WA
  reads; divergence aborts the sweep.
- **No gates:** every window is RECORDED, NEVER GATED. No metric makes a step
  fail. The only binary check is the behavior digest (determinism, not a WA
  reading).
- **Reward surfaces untouched:** no A6 step modifies any reward function.
- **runs/ never staged:** all run outputs go to `runs/` (gitignored by
  HOUSE RULES). Story artifacts go to `docs/superpowers/posts/`.
