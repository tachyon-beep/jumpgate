"""CLI-seam tests for sweep_trophic (world-gets-big phase 3.1)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import sweep_trophic


def test_runner_cmd_carries_scenario_seed_and_knobs():
    cmd = sweep_trophic.runner_cmd(
        "frontier", 7, 50_000, "/tmp/x.jsonl", [("pirate_max_reach_au", "999")]
    )
    assert cmd[cmd.index("--scenario") + 1] == "frontier"
    assert cmd[cmd.index("--seed") + 1] == "7"
    assert cmd[cmd.index("--ticks") + 1] == "50000"
    assert cmd[cmd.index("--set") + 1] == "pirate_max_reach_au=999"


def test_runner_cmd_trophic_is_still_explicit():
    # The flag is passed UNCONDITIONALLY: the runner owns the
    # unknown-scenario error (a silent default would hide a typo'd arm).
    cmd = sweep_trophic.runner_cmd("trophic", 11, 1_000, "/tmp/y.jsonl", [])
    assert cmd[cmd.index("--scenario") + 1] == "trophic"
    assert "--set" not in cmd


def test_default_control_recipe_includes_frontier_geometry_equalizer():
    specs = sweep_trophic.default_knobsets()
    assert specs[0] == "baseline"
    name, knobs = sweep_trophic.parse_knobset(specs[1])
    assert name == "control"
    assert dict(knobs) == {
        "pirate_max_reach_au": "999",
        "stay_milli": "0",
        "upkeep_per_tick": "200",
        "grubstake_micros": "2000000000",
        "engage_radius_au": "0.05",
    }
