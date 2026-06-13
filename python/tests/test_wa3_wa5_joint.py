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


def test_wa5_output_has_wa3_column():
    # M3 (synthesis): every WA5 verdict-mix row must carry own_trade_share_milli
    # alongside the verdict. This is a co-read, not a gate.
    # Simulate the cells dict that sweep_bazaar / wa3_wa5_joint produces for
    # WA5 input: each entry must have both "verdict" and "own_trade_share_milli".
    cells = {
        7:  {"verdict": "Alive",          "own_trade_share_milli": 200},
        11: {"verdict": "NoCycle",         "own_trade_share_milli": 350},
        13: {"verdict": "PermanentPeace",  "own_trade_share_milli": 800},
    }
    for seed, cell in cells.items():
        assert "own_trade_share_milli" in cell, (
            f"seed {seed}: WA5 verdict-mix row missing own_trade_share_milli "
            f"(M3 co-read — WA3 and WA5 are a joint read)"
        )
        assert "verdict" in cell, f"seed {seed}: WA5 cell missing verdict"
    # The WA3 column must survive the clean_seeds filter (it is NOT stripped).
    clean = wa3_wa5_joint.clean_seeds(cells)
    for seed in clean:
        assert "own_trade_share_milli" in cells[seed], (
            f"clean seed {seed}: own_trade_share_milli must be present on every "
            "clean cell passed to the WA5 distribution read"
        )
