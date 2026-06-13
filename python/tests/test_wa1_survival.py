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
