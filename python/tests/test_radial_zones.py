"""I2 radial-zone panel pins (world-gets-big spec section 8 / W10)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import radial_zones as rz


def test_fuel_starve_readings_cover_the_pre_registered_table():
    assert rz.fuel_starve([5, 4, 3, 2], [9, 9, 9, 9]) == "NoStockout"
    assert rz.fuel_starve([5, 0, 3, 4], [9, 5, 7, 8]) == "BoomBust"
    assert rz.fuel_starve([5, 2, 0, 0], [9, 9, 1, 0]) == "DeathSpiral"
    assert rz.fuel_starve([5, 0, 0, 0], [5, 5, 5, 5]) == "Stockout"
    assert rz.fuel_starve([5, 0, 1], [1, 1, 1]) == "ShortRun"


def test_fill_share_is_floor_permille_of_remaining_headroom():
    ev = {
        "e": "refuel",
        "tick": 9,
        "craft": 4,
        "station": 2,
        "units": 12,
        "price_micros": 7,
        "before_permille": 250,
        "after_permille": 850,
    }
    assert rz.fill_permille(ev) == 800
    full = dict(ev, before_permille=1000, after_permille=1000)
    assert rz.fill_permille(full) is None


def test_zone_series_sums_stock_and_routes_touching_the_zone():
    w = {
        "tick": 2000,
        "per_route_robs": [0, 1, 0, 2],
        "per_route_traffic": [3, 1, 0, 5],
        "per_station_fuel_stock": [7, 11],
        "per_station_fuel_price": [5000, 9000],
        "per_station_alerts": [1, 0],
    }
    z = rz.zone_series([w], [1])
    assert z == [{
        "tick": 2000,
        "traffic": 6,
        "robs": 3,
        "stock": 11,
        "price_max": 9000,
        "alerts": 0,
    }]


def test_parse_zones():
    assert rz.parse_zones("0,1,2|3,4,5|6|7,8,9") == [
        [0, 1, 2], [3, 4, 5], [6], [7, 8, 9]
    ]
