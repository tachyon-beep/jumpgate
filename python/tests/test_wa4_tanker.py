"""WA4 emergent-tanker reader pins (spec §5 WA4; windows, not gates).

WA4: fuel packages to non-refinery stations appear with zero fuel-specific
dispatch code. The read: join gossip-log 'post' rows where good==FUEL_GOOD
(good index from the META 'goods=' tail; defaults to 1 = the Fuel slot in
scenario_bazaar) to the destination station, then filter out the three
refinery stations. Any such package = a tanker event. The first tanker
contract sequence (post → accept → deliver) is the console chronicle arc.
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import wa4_tanker


REFINERY_STATIONS = {2, 5, 9}  # scenario_bazaar refinery positions


def _post(contract, tick, route, good, to_station):
    return {"e": "post", "tick": tick, "contract": contract,
            "route": route, "good": good, "to_station": to_station}


def _accept(contract, tick, hauler):
    return {"e": "accept", "tick": tick, "contract": contract, "hauler": hauler}


def _deliver(contract, tick):
    return {"e": "deliver", "tick": tick, "contract": contract}


def test_tanker_detected_on_non_refinery_fuel_delivery():
    rows = [
        _post(1, 100, 3, 1, 4),   # good=1 (Fuel), to_station=4 (not refinery)
        _accept(1, 200, 7),
        _deliver(1, 800),
    ]
    tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                      refinery_stations=REFINERY_STATIONS)
    assert len(tankers) == 1
    assert tankers[0]["contract"] == 1
    assert tankers[0]["to_station"] == 4


def test_no_tanker_when_fuel_goes_to_refinery():
    rows = [
        _post(1, 100, 3, 1, 2),   # good=1 (Fuel), to_station=2 (refinery)
        _accept(1, 200, 7),
        _deliver(1, 800),
    ]
    tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                      refinery_stations=REFINERY_STATIONS)
    assert tankers == []


def test_no_tanker_when_non_fuel_good_to_non_refinery():
    rows = [
        _post(1, 100, 3, 3, 4),   # good=3 (not Fuel), to_station=4
        _accept(1, 200, 7),
        _deliver(1, 800),
    ]
    tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                      refinery_stations=REFINERY_STATIONS)
    assert tankers == []


def test_first_tanker_is_earliest_by_post_tick():
    rows = [
        _post(2, 500, 5, 1, 3),   # later fuel tanker
        _accept(2, 600, 8),
        _deliver(2, 900),
        _post(1, 100, 3, 1, 4),   # earlier fuel tanker
        _accept(1, 200, 7),
        _deliver(1, 800),
    ]
    tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                      refinery_stations=REFINERY_STATIONS)
    assert len(tankers) == 2
    first = wa4_tanker.first_tanker(tankers)
    assert first["contract"] == 1  # earliest post_tick


def test_tanker_undelivered_not_counted():
    # post + accept but no deliver = in-flight, not a confirmed tanker event
    rows = [
        _post(1, 100, 3, 1, 4),
        _accept(1, 200, 7),
        # no deliver
    ]
    tankers = wa4_tanker.find_tankers(rows, fuel_good=1,
                                      refinery_stations=REFINERY_STATIONS)
    assert tankers == []
