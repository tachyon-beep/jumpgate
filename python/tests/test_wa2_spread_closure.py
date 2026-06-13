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
