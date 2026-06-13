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
