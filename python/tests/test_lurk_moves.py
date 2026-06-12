"""W6 LurkMoved panel pins (world-gets-big spec section 8 / W6)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import lurk_moves as lm


def test_breakout_share_is_floor_permille():
    events = [
        {"e": "lurk_moved", "tick": 10, "pirate": 1, "to_station": 7, "breakout": True},
        {"e": "lurk_moved", "tick": 20, "pirate": 1, "to_station": 8, "breakout": False},
        {"e": "lurk_moved", "tick": 30, "pirate": 2, "to_station": 2, "breakout": True},
    ]
    assert lm.breakout_share(events) == {"moves": 3, "breakouts": 2, "breakout_permille": 666}


def test_landing_counts_by_station_and_zone():
    events = [
        {"e": "lurk_moved", "tick": 10, "pirate": 1, "to_station": 7, "breakout": True},
        {"e": "lurk_moved", "tick": 20, "pirate": 1, "to_station": 8, "breakout": False},
        {"e": "lurk_moved", "tick": 30, "pirate": 2, "to_station": 2, "breakout": True},
    ]
    assert lm.station_counts(events) == {2: 1, 7: 1, 8: 1}
    assert lm.zone_counts(events, [[0, 1, 2], [3, 4, 5], [6], [7, 8, 9]]) == {
        "core": 1,
        "mid": 0,
        "haven": 0,
        "frontier": 2,
    }


def test_dwell_ticks_are_per_pirate_consecutive_move_deltas():
    events = [
        {"e": "lurk_moved", "tick": 10, "pirate": 1, "to_station": 7, "breakout": True},
        {"e": "lurk_moved", "tick": 20, "pirate": 2, "to_station": 2, "breakout": False},
        {"e": "lurk_moved", "tick": 25, "pirate": 1, "to_station": 8, "breakout": False},
        {"e": "lurk_moved", "tick": 80, "pirate": 1, "to_station": 9, "breakout": False},
        {"e": "lurk_moved", "tick": 120, "pirate": 2, "to_station": 3, "breakout": False},
    ]
    assert lm.dwell_ticks(events) == [15, 55, 100]
    assert lm.quartiles([15, 55, 100]) == (15, 55, 55)


def test_quartiles_use_phase3_lower_index_convention_for_even_samples():
    assert lm.quartiles([40, 10, 20, 30]) == (10, 20, 30)
