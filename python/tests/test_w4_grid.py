"""W4 grid reader pins (world-gets-big spec section 8 / 11 W4)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import w4_grid


def cell(verdict="Alive", robs="6", trips="40", credits=None, haulers=3):
    return {
        "result": {
            "seed": "7",
            "ticks": "50000",
            "verdict": verdict,
            "cycled": "true",
            "hetero": "true",
            "disperse": "true",
            "fuel_empty": "0",
            "robs": robs,
            "trips": trips,
            "purchases": "2",
        },
        "meta": {"haulers": str(haulers)},
        "windows": [{"per_craft_credits": credits or [30, 10, 20, 999]}],
    }


def test_clean_seeds_rule_is_blind_born_not_permanent_peace():
    cells = {
        ("blind-born", 7): cell(verdict="Alive"),
        ("blind-born", 11): cell(verdict="PermanentPeace"),
        ("gossip-born", 7): cell(verdict="PermanentPeace"),
    }
    assert w4_grid.clean_seeds(cells, [7, 11]) == [7]


def test_run_value_takes_hauler_slice_median_and_per_trip_rate():
    v = w4_grid.run_value(cell())
    assert v == {
        "median_credits": 20,
        "laden_trips": 40,
        "credits_per_trip_milli": 500,
    }


def test_aa_twin_divergence_names_the_differing_fields():
    cells = {
        ("blind-born", 7): cell(robs="6"),
        ("blind-rob", 7): cell(robs="7"),
        ("ring-born", 7): cell(),
        ("ring-rob", 7): cell(),
    }
    bad = w4_grid.aa_twin_divergences(cells, [7])
    assert len(bad) == 1
    fam, seed, fields = bad[0]
    assert (fam, seed) == ("blind", 7)
    assert fields == {"robs": ("6", "7")}


def test_quartiles_are_lower_index_integers():
    assert w4_grid.quartiles([5, 1, 9, 3, 7]) == (3, 5, 7)
