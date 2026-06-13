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

V4_STDOUT = V1_STDOUT + (
    "META seed=7 scenario=frontier stations=10 haulers=20 pirates_initial=10 "
    "station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]\n"
    "FUEL seed=7 hauler_duty_milli=537 hauler_burn_total_milli=3149 "
    "hauler_median_leg_burn_permille=2 hauler_min_tank_permille=745 "
    "refuels=49 refuel_spend_micros=128200 strandings=2 adrift_end=1\n"
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


def test_v4_fuel_line_stranding_tail_parses_and_older_tails_read_none():
    parsed = sweep.parse_stdout(V4_STDOUT)
    assert parsed["fuel"] is not None
    assert parsed["fuel"]["refuels"] == "49"
    assert parsed["fuel"]["refuel_spend_micros"] == "128200"
    assert parsed["fuel"]["strandings"] == "2"
    assert parsed["fuel"]["adrift_end"] == "1"
    for legacy_text in (V2_STDOUT, V3_STDOUT):
        legacy = sweep.parse_stdout(legacy_text)
        assert legacy["fuel"]["strandings"] is None
        assert legacy["fuel"]["adrift_end"] is None


# V5: adds optional BAZAAR anchored line (rung A, scenario_bazaar; config-gated
# so trophic/frontier stdout stays byte-identical). Regex lands in same commit
# as the Rust println! (lockstep rule).
V5_STDOUT = V4_STDOUT + (
    "BAZAAR seed=7 scenario=bazaar exchange_treasury_micros=1234567890 "
    "trade_buys=0 trade_sells=0 arb_posts=0 arb_withdrawals=0\n"
)


def test_v5_bazaar_line_parses_and_older_reads_none():
    parsed = sweep.parse_stdout(V5_STDOUT)
    assert parsed["bazaar"] is not None
    assert parsed["bazaar"]["exchange_treasury"] == "1234567890"
    assert parsed["bazaar"]["trade_buys"] == "0"
    for legacy_text in (V1_STDOUT, V2_STDOUT, V3_STDOUT, V4_STDOUT):
        legacy = sweep.parse_stdout(legacy_text)
        assert legacy["bazaar"] is None, "bazaar is None for pre-bazaar stdout"


def test_meta_goods_tail_is_none_for_trophic_frontier():
    # trophic/frontier META lines have no goods= tail; parser must return None.
    for text in (V2_STDOUT, V3_STDOUT, V4_STDOUT):
        parsed = sweep.parse_stdout(text)
        assert parsed["meta"] is not None
        assert parsed["meta"]["goods"] is None, \
            "goods= must be None for pre-bazaar META lines"


def test_meta_goods_tail_parses_when_present():
    # The positive arm of the A0.5 optional tail (a bazaar-mode META line).
    # Pinned against a synthetic single line, NOT a V6 fixture — V6_STDOUT
    # is reserved for the A3.7 BAZAAR-live ladder step.
    line = (
        "META seed=7 scenario=bazaar stations=10 haulers=20 "
        "pirates_initial=10 station_radii_milli_au=[350, 560] goods=10"
    )
    m = sweep.META_RE.match(line)
    assert m is not None, "bazaar META line with goods= tail must match META_RE"
    assert m.group("goods") == "10"
