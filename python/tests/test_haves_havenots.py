"""I1 panel math pins (world-gets-big spec section 8 / W5)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import haves_havenots as hh


def test_hearing_lag_joins_on_born_tick_never_rob_tick():
    # The heard event carries a corrupted rob_tick (inflation/lying stays
    # armed-dormant, spec section 12): the lag must be 150-100=50.
    borns = [{"e": "born", "tick": 100, "alert": 1, "route": 0, "claimed": 5}]
    heards = [{
        "e": "heard", "tick": 150, "alert": 1, "carrier": "c3",
        "rob_tick": 999, "hops": 2, "claimed": 9,
    }]
    assert hh.hearing_lags(borns, heards) == {3: [50]}


def test_hearing_lag_takes_first_hearing_and_ignores_station_carriers():
    borns = [{"e": "born", "tick": 10, "alert": 7, "route": 0, "claimed": 5}]
    heards = [
        {"e": "heard", "tick": 90, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 40, "alert": 7, "carrier": "c1", "rob_tick": 10, "hops": 1, "claimed": 5},
        {"e": "heard", "tick": 20, "alert": 7, "carrier": "s0", "rob_tick": 10, "hops": 1, "claimed": 5},
    ]
    assert hh.hearing_lags(borns, heards) == {1: [30]}


def test_workplace_radius_is_floor_mean_of_accept_endpoints():
    # n=10 stations; route 29 = from 2 -> to 9. Radii in milli-AU.
    radii = [350, 444, 564, 716, 909, 1154, 1466, 1861, 2363, 3000]
    accepts = [
        {"e": "accept", "tick": 5, "route": 29, "hauler": 4},
        {"e": "accept", "tick": 9, "route": 29, "hauler": 4},
    ]
    # mean(564, 3000, 564, 3000) = 1782 exactly.
    assert hh.workplace_radius_milli(accepts, radii, 10) == {4: 1782}


def test_workplace_radius_skips_null_routes():
    assert hh.workplace_radius_milli(
        [{"e": "accept", "tick": 5, "route": None, "hauler": 0}], [1, 2], 2
    ) == {}


def test_spearman_perfect_monotone_and_tie_handling():
    assert hh.spearman([(1, 10), (2, 20), (3, 30)]) == 1.0
    assert hh.spearman([(1, 30), (2, 20), (3, 10)]) == -1.0
    assert hh.spearman([(1, 1), (1, 1), (1, 1)]) is None
    r = hh.spearman([(1, 10), (2, 10), (3, 30), (4, 40)])
    assert r is not None and 0.9 < r <= 1.0
