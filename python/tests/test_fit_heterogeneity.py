"""Pins fit_heterogeneity's integer math against diagnostics.rs (:270-316)
and against the DOCUMENTED 2026-06-11 fit (2204 / 3) -- the mirror is the
method's instrument, so it ships with synthetics that would catch it lying
(the diagnostics.rs:896-899 house rule, applied lab-side)."""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "analysis"))
import fit_heterogeneity as fh


def w(robs, traffic, active):
    return {
        "per_route_robs": robs,
        "per_route_traffic": traffic,
        "active_pirates": active,
    }


def test_mean_norm_hhi_masks_unoccupied_routes_and_normalizes_by_active():
    # Window 1: route 1 has robs but NO traffic -> masked to 0; occupied
    # robs [4, 0], HHI = 16*1000 // 16 = 1000 milli; x active.max(1)=3
    # -> 3000. Window 2: zero robs -> not a robbing window.
    ws = [w([4, 5], [1, 0], 3), w([0, 0], [1, 1], 9)]
    assert fh.mean_norm_hhi_milli(ws) == 3000


def test_mean_norm_hhi_floor_division_over_robbing_windows():
    # ([3,1] occupied: HHI=(9+1)*1000//16=625, x2=1250) and
    # ([1,1]: HHI=2*1000//4=500, x1=500) -> (1250+500)//2 = 875.
    ws = [w([3, 1], [1, 2], 2), w([1, 1], [3, 1], 1)]
    assert fh.mean_norm_hhi_milli(ws) == 875


def test_mean_norm_hhi_none_when_no_robbing_window():
    assert fh.mean_norm_hhi_milli([w([0, 0], [1, 1], 5)]) is None


def test_hot_change_excess_argmax_ties_to_lowest_index():
    # hot argmax: [2,1]->0, [1,2]->1, [2,2]->0 (tie -> LOWEST) = 2 changes;
    # traffic argmax constant at 0 -> 0 changes; excess = +2.
    ws = [w([2, 1], [1, 1], 1), w([1, 2], [1, 1], 1), w([2, 2], [1, 1], 1)]
    assert fh.hot_change_excess(ws) == 2


def test_fit_reproduces_the_documented_2026_06_11_constants():
    # The diagnostics.rs:30-56 doc tables ARE the regression fixture: the
    # method must reproduce threshold 2204 and slack 3 from them.
    t = fh.fit_threshold([3070, 2962, 2918, 3498], [1490, 1472])
    assert t == {
        "threshold": 2204,
        "clumped_min": 2918,
        "equalized_max": 1490,
        "margin_open": True,
    }
    s = fh.fit_slack([1, -1, -3, -6], [6, 5])
    assert s == {
        "slack": 3,
        "clumped_max": 1,
        "equalized_min": 5,
        "margin_open": True,
    }


def test_fit_reports_a_closed_margin_instead_of_inventing_a_boundary():
    t = fh.fit_threshold([1500, 1600], [1700])
    assert t["margin_open"] is False


def test_load_filters_non_window_jsonl_rows(tmp_path):
    p = tmp_path / "run.jsonl"
    p.write_text(
        '{"meta_seed":7,"meta_scenario":"frontier"}\n'
        '{"tick":2000,"per_route_robs":[1],"per_route_traffic":[1],"active_pirates":2}\n'
    )
    assert fh.load(p) == [
        {"tick": 2000, "per_route_robs": [1], "per_route_traffic": [1], "active_pirates": 2}
    ]
