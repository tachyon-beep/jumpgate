"""Version-gated anchored-line parsing for sweep_trophic.

Banked pre-FUEL stdout (RESULT+MEDIA only) must still parse: META and later
FUEL read None rather than aborting. Presence is the instrument-format gate;
these are windows, not build gates (PDR-0006).
"""

import importlib.util
import pathlib

_SPEC = importlib.util.spec_from_file_location(
    "sweep_trophic",
    pathlib.Path(__file__).resolve().parents[1] / "analysis" / "sweep_trophic.py",
)
sweep = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(sweep)

V1_STDOUT = """\
trophic_run: seed=7 ticks=50000 windows=25 (W=2000) sets=[]
RESULT seed=7 ticks=50000 verdict=Alive cycled=true risk_heterogeneous=true \
outcomes_disperse=true fuel_empty=0 robs=63 laden_trips=410 purchases=9
MEDIA seed=7 born=12 escaped_milli=833 median_lag=410 p90_lag=1290 reading=Localized
"""

V2_STDOUT = V1_STDOUT + (
    "META seed=7 scenario=trophic stations=6 haulers=12 pirates_initial=6 "
    "station_radii_milli_au=[350, 560, 770, 980, 1190, 1400]\n"
    "FUEL seed=7 hauler_duty_milli=412 hauler_burn_total_milli=3180 "
    "hauler_median_leg_burn_permille=24 hauler_min_tank_permille=507\n"
)

V3_STDOUT = V1_STDOUT + (
    "META seed=7 scenario=frontier stations=10 haulers=25 pirates_initial=10 "
    "station_radii_milli_au=[350, 442, 558, 705, 890, 1124, 1420, 1793, 2265, 2861]\n"
    "FUEL seed=7 hauler_duty_milli=670 hauler_burn_total_milli=598 "
    "hauler_median_leg_burn_permille=6 hauler_min_tank_permille=948 "
    "refuels=3 refuel_spend_micros=45000\n"
)


def test_v1_banked_output_still_parses():
    parsed = sweep.parse_stdout(V1_STDOUT)
    assert parsed["result"]["verdict"] == "Alive"
    assert parsed["media"]["reading"] == "Localized"
    assert parsed["meta"] is None


def test_v2_meta_line_parses():
    parsed = sweep.parse_stdout(V2_STDOUT)
    assert parsed["meta"] is not None
    assert parsed["meta"]["scenario"] == "trophic"
    assert parsed["meta"]["stations"] == "6"
    assert parsed["meta"]["haulers"] == "12"
    assert parsed["meta"]["pirates_initial"] == "6"
    assert parsed["meta"]["radii"] == "350, 560, 770, 980, 1190, 1400"


def test_v2_fuel_line_parses_and_v1_reads_none():
    parsed = sweep.parse_stdout(V2_STDOUT)
    assert parsed["fuel"] is not None
    assert parsed["fuel"]["duty"] == "412"
    assert parsed["fuel"]["burn"] == "3180"
    assert parsed["fuel"]["leg"] == "24"
    assert parsed["fuel"]["min_tank"] == "507"
    assert sweep.parse_stdout(V1_STDOUT)["fuel"] is None


def test_v3_fuel_line_refuel_tail_parses_and_v2_tail_reads_none():
    parsed = sweep.parse_stdout(V3_STDOUT)
    assert parsed["fuel"] is not None
    assert parsed["fuel"]["refuels"] == "3"
    assert parsed["fuel"]["refuel_spend_micros"] == "45000"
    legacy = sweep.parse_stdout(V2_STDOUT)
    assert legacy["fuel"]["refuels"] is None
    assert legacy["fuel"]["refuel_spend_micros"] is None
