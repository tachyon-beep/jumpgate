# World Gets Big Phase 2 Calibration

Date: 2026-06-12

Frame: recorded calibration evidence, not a gate. The volatile run products are
under `runs/wgb-calibration/` and are intentionally not staged.

## Commands

Preflight:

```bash
cargo test -p jumpgate-core scenario_frontier
grep -n 'scenario' crates/jumpgate-core/examples/trophic_run.rs | grep -i 'frontier\|--scenario'
grep -n '"FUEL ' crates/jumpgate-core/examples/trophic_run.rs
grep -n 'leg_burn' crates/jumpgate-core/src/diagnostics.rs crates/jumpgate-core/examples/trophic_run.rs
```

Scale-100 sanity and ensemble:

```bash
mkdir -p runs/wgb-calibration
cargo run -q -p jumpgate-core --release --example trophic_run -- \
  --scenario frontier --seed 7 --ticks 100000 \
  --set fuel_capacity_scale=100 --jsonl runs/wgb-calibration/cal-s7.jsonl \
  | grep -E '^(META|RESULT|FUEL) ' | tee runs/wgb-calibration/cal-s7.txt

rm -f runs/wgb-calibration/scale100.txt
for seed in $(seq 1 20); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed "$seed" --ticks 100000 \
    --set fuel_capacity_scale=100 \
    --jsonl "runs/wgb-calibration/cal-s$seed.jsonl" \
    | grep -E '^(META|RESULT|FUEL) ' >> runs/wgb-calibration/scale100.txt
done
grep -c '^FUEL ' runs/wgb-calibration/scale100.txt
```

Worst-leg extraction:

```bash
python3 - <<'PY'
import json, glob
KEY = "leg_burn_permille"
worst, where = 0, None
for path in sorted(glob.glob("runs/wgb-calibration/cal-s*.jsonl")):
    for line in open(path):
        row = json.loads(line)
        for v in row.get(KEY, []):
            if v > worst:
                worst, where = v, (path, row["tick"])
print(f"worst hauler-leg burn = {worst} permille of the SCALED tank, at {where}")
PY
```

Golden re-derive and verification after the bake:

```bash
cargo test -p jumpgate-core print_golden_frontier -- --ignored --nocapture
cargo test -p jumpgate-core frontier_trajectory_golden
cargo test -p jumpgate-core golden
cargo test --workspace
cargo clippy --all-targets -- -D warnings
```

Scale-1 sanity:

```bash
rm -f runs/wgb-calibration/scale1.txt
for seed in $(seq 1 20); do
  cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed "$seed" --ticks 100000 \
    | grep -E '^(META|RESULT|FUEL) ' >> runs/wgb-calibration/scale1.txt
done
grep -c '^FUEL ' runs/wgb-calibration/scale1.txt

rm -f runs/wgb-calibration/scale1-adrift.txt
for seed in $(seq 1 20); do
  n=$(cargo run -q -p jumpgate-core --release --example trophic_run -- \
    --scenario frontier --seed "$seed" --ticks 100000 --chronicle \
    | grep -c 'ADRIFT since' || true)
  printf 'seed=%s adrift_end=%s\n' "$seed" "$n" >> runs/wgb-calibration/scale1-adrift.txt
done
```

## Endurance Arithmetic

At the pre-calibration prior `v_e = 1.0`, full-throttle burn is:

```text
burn_per_tick = thrust / v_e * dt = 1e-12 / 1.0 * 0.25 = 2.5e-13
scaled_tank   = 100 * 1e-9 = 1e-7
endurance     = 1e-7 / 2.5e-13 = 400000 full-throttle ticks
```

The scale-100 100000-tick runs therefore preserve the burn tail for the
measurement. All 20 scale-100 `RESULT` lines below have `fuel_empty=0`.

## Measurement

JSONL key: `leg_burn_permille`.

Extraction output:

```text
worst hauler-leg burn = 170 permille of the SCALED tank, at ('runs/wgb-calibration/cal-s9.jsonl', 86000)
```

Derivation:

```text
P = 170
B_worst = P * scaled_tank / 1000 = 170 * 1e-7 / 1000 = 170e-10 = 1.70e-8
v_e = k * B_worst / tank * v_e_prior
    = 2.5 * 170e-10 / 1.0e-9 * 1.0
    = 42.5
```

Baked constant: `FRONTIER_HAULER_EXHAUST_VELOCITY = 42.5`.

Frontier trajectory golden:

```text
old = 0xe5b3c68a9b4f727c
new = 0x050de98bd4b6793c
```

The new literal was printed by `print_golden_frontier` after the `v_e` bake.

## Scale-100 Anchored Bank

```text
META seed=1 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=1 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=34 laden_trips=43 purchases=0
FUEL seed=1 hauler_duty_milli=299 hauler_burn_total_milli=1483 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=775 refuels=42 refuel_spend_micros=2068975
META seed=2 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=2 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=17 laden_trips=61 purchases=1
FUEL seed=2 hauler_duty_milli=290 hauler_burn_total_milli=1445 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=810 refuels=59 refuel_spend_micros=2924600
META seed=3 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=3 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=7 laden_trips=58 purchases=1
FUEL seed=3 hauler_duty_milli=255 hauler_burn_total_milli=1266 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=750 refuels=54 refuel_spend_micros=2657500
META seed=4 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=4 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=9 laden_trips=34 purchases=0
FUEL seed=4 hauler_duty_milli=339 hauler_burn_total_milli=1687 hauler_median_leg_burn_permille=9 hauler_min_tank_permille=750 refuels=31 refuel_spend_micros=1524600
META seed=5 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=5 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=42 laden_trips=67 purchases=1
FUEL seed=5 hauler_duty_milli=284 hauler_burn_total_milli=1411 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=807 refuels=65 refuel_spend_micros=3263350
META seed=6 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=6 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=36 laden_trips=73 purchases=2
FUEL seed=6 hauler_duty_milli=279 hauler_burn_total_milli=1387 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=773 refuels=65 refuel_spend_micros=3384600
META seed=7 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=7 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=33 laden_trips=60 purchases=0
FUEL seed=7 hauler_duty_milli=291 hauler_burn_total_milli=1451 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=750 refuels=53 refuel_spend_micros=2729600
META seed=8 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=8 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=31 laden_trips=55 purchases=1
FUEL seed=8 hauler_duty_milli=261 hauler_burn_total_milli=1300 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=761 refuels=54 refuel_spend_micros=2711325
META seed=9 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=9 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=11 laden_trips=89 purchases=5
FUEL seed=9 hauler_duty_milli=369 hauler_burn_total_milli=1839 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=750 refuels=79 refuel_spend_micros=4067000
META seed=10 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=10 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=37 laden_trips=52 purchases=1
FUEL seed=10 hauler_duty_milli=246 hauler_burn_total_milli=1225 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=768 refuels=51 refuel_spend_micros=2616025
META seed=11 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=11 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=9 laden_trips=88 purchases=6
FUEL seed=11 hauler_duty_milli=337 hauler_burn_total_milli=1677 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=767 refuels=80 refuel_spend_micros=4040850
META seed=12 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=12 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=21 laden_trips=53 purchases=1
FUEL seed=12 hauler_duty_milli=249 hauler_burn_total_milli=1236 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=791 refuels=51 refuel_spend_micros=2518975
META seed=13 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=13 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=22 laden_trips=43 purchases=0
FUEL seed=13 hauler_duty_milli=339 hauler_burn_total_milli=1688 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=750 refuels=39 refuel_spend_micros=1963350
META seed=14 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=14 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=7 laden_trips=21 purchases=0
FUEL seed=14 hauler_duty_milli=358 hauler_burn_total_milli=1780 hauler_median_leg_burn_permille=9 hauler_min_tank_permille=750 refuels=21 refuel_spend_micros=1024600
META seed=15 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=15 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=39 laden_trips=70 purchases=1
FUEL seed=15 hauler_duty_milli=364 hauler_burn_total_milli=1814 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=755 refuels=66 refuel_spend_micros=3423750
META seed=16 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=16 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=13 laden_trips=46 purchases=0
FUEL seed=16 hauler_duty_milli=332 hauler_burn_total_milli=1653 hauler_median_leg_burn_permille=9 hauler_min_tank_permille=750 refuels=41 refuel_spend_micros=2102100
META seed=17 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=17 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=16 laden_trips=34 purchases=2
FUEL seed=17 hauler_duty_milli=262 hauler_burn_total_milli=1302 hauler_median_leg_burn_permille=8 hauler_min_tank_permille=772 refuels=33 refuel_spend_micros=1663350
META seed=18 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=18 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=23 laden_trips=34 purchases=0
FUEL seed=18 hauler_duty_milli=313 hauler_burn_total_milli=1553 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=759 refuels=34 refuel_spend_micros=1663350
META seed=19 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=19 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=12 laden_trips=28 purchases=1
FUEL seed=19 hauler_duty_milli=324 hauler_burn_total_milli=1613 hauler_median_leg_burn_permille=7 hauler_min_tank_permille=750 refuels=26 refuel_spend_micros=1313350
META seed=20 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=20 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=14 laden_trips=67 purchases=3
FUEL seed=20 hauler_duty_milli=309 hauler_burn_total_milli=1537 hauler_median_leg_burn_permille=6 hauler_min_tank_permille=761 refuels=61 refuel_spend_micros=3101875
```

## Scale-1 Anchored Bank

```text
META seed=1 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=1 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=214 laden_trips=2167 purchases=89
FUEL seed=1 hauler_duty_milli=611 hauler_burn_total_milli=7187 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=939 refuels=138 refuel_spend_micros=456825
META seed=2 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=2 ticks=100000 verdict=PermanentPeace cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=12 laden_trips=2235 purchases=81
FUEL seed=2 hauler_duty_milli=624 hauler_burn_total_milli=7335 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=925 refuels=139 refuel_spend_micros=360850
META seed=3 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=3 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=228 laden_trips=2149 purchases=92
FUEL seed=3 hauler_duty_milli=597 hauler_burn_total_milli=7022 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=931 refuels=134 refuel_spend_micros=367100
META seed=4 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=4 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=119 laden_trips=2152 purchases=87
FUEL seed=4 hauler_duty_milli=617 hauler_burn_total_milli=7252 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=943 refuels=138 refuel_spend_micros=417000
META seed=5 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=5 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=160 laden_trips=2139 purchases=87
FUEL seed=5 hauler_duty_milli=612 hauler_burn_total_milli=7194 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=923 refuels=136 refuel_spend_micros=350650
META seed=6 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=6 ticks=100000 verdict=PermanentPeace cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=15 laden_trips=2240 purchases=82
FUEL seed=6 hauler_duty_milli=617 hauler_burn_total_milli=7254 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=934 refuels=137 refuel_spend_micros=300350
META seed=7 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=7 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=215 laden_trips=1920 purchases=87
FUEL seed=7 hauler_duty_milli=613 hauler_burn_total_milli=7206 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=451 refuels=124 refuel_spend_micros=294550
META seed=8 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=8 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=113 laden_trips=2195 purchases=87
FUEL seed=8 hauler_duty_milli=614 hauler_burn_total_milli=7219 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=943 refuels=136 refuel_spend_micros=354250
META seed=9 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=9 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=151 laden_trips=2094 purchases=89
FUEL seed=9 hauler_duty_milli=602 hauler_burn_total_milli=7083 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=942 refuels=132 refuel_spend_micros=319650
META seed=10 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=10 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=110 laden_trips=2104 purchases=88
FUEL seed=10 hauler_duty_milli=618 hauler_burn_total_milli=7265 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=944 refuels=139 refuel_spend_micros=355000
META seed=11 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=11 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=71 laden_trips=2200 purchases=84
FUEL seed=11 hauler_duty_milli=616 hauler_burn_total_milli=7238 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=945 refuels=137 refuel_spend_micros=382250
META seed=12 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=12 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=151 laden_trips=2140 purchases=84
FUEL seed=12 hauler_duty_milli=614 hauler_burn_total_milli=7223 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=926 refuels=136 refuel_spend_micros=327025
META seed=13 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=13 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=181 laden_trips=2077 purchases=91
FUEL seed=13 hauler_duty_milli=611 hauler_burn_total_milli=7181 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=859 refuels=136 refuel_spend_micros=411625
META seed=14 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=14 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=215 laden_trips=1984 purchases=86
FUEL seed=14 hauler_duty_milli=623 hauler_burn_total_milli=7328 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=437 refuels=128 refuel_spend_micros=290675
META seed=15 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=15 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=129 laden_trips=2130 purchases=88
FUEL seed=15 hauler_duty_milli=610 hauler_burn_total_milli=7167 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=944 refuels=136 refuel_spend_micros=399250
META seed=16 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=16 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=114 laden_trips=2150 purchases=85
FUEL seed=16 hauler_duty_milli=604 hauler_burn_total_milli=7106 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=940 refuels=133 refuel_spend_micros=358675
META seed=17 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=17 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=192 laden_trips=2039 purchases=91
FUEL seed=17 hauler_duty_milli=614 hauler_burn_total_milli=7215 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=939 refuels=135 refuel_spend_micros=372375
META seed=18 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=18 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=130 laden_trips=2175 purchases=83
FUEL seed=18 hauler_duty_milli=617 hauler_burn_total_milli=7253 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=939 refuels=139 refuel_spend_micros=392125
META seed=19 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=19 ticks=100000 verdict=Alive cycled=true risk_heterogeneous=true outcomes_disperse=true fuel_empty=0 robs=112 laden_trips=2145 purchases=89
FUEL seed=19 hauler_duty_milli=610 hauler_burn_total_milli=7169 hauler_median_leg_burn_permille=3 hauler_min_tank_permille=935 refuels=135 refuel_spend_micros=478125
META seed=20 scenario=frontier stations=10 haulers=20 pirates_initial=10 station_radii_milli_au=[350, 444, 564, 716, 909, 1154, 1465, 1861, 2362, 3000]
RESULT seed=20 ticks=100000 verdict=RiskEqualized cycled=true risk_heterogeneous=false outcomes_disperse=true fuel_empty=0 robs=212 laden_trips=2175 purchases=89
FUEL seed=20 hauler_duty_milli=620 hauler_burn_total_milli=7286 hauler_median_leg_burn_permille=2 hauler_min_tank_permille=934 refuels=137 refuel_spend_micros=405200
```

## W9/W10 Scale-1 Readings

`strandings fuel_empty` is the runner's `fuel_empty` count. `adrift_end` is
counted from chronicle epilogues containing `ADRIFT since`.

| seed | verdict | strandings fuel_empty | adrift_end | refuels | refuel_spend_micros | median_leg_permille | min_tank_permille | robs | laden_trips | purchases |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | RiskEqualized | 0 | 0 | 138 | 456825 | 2 | 939 | 214 | 2167 | 89 |
| 2 | PermanentPeace | 0 | 0 | 139 | 360850 | 3 | 925 | 12 | 2235 | 81 |
| 3 | RiskEqualized | 0 | 0 | 134 | 367100 | 2 | 931 | 228 | 2149 | 92 |
| 4 | RiskEqualized | 0 | 0 | 138 | 417000 | 2 | 943 | 119 | 2152 | 87 |
| 5 | RiskEqualized | 0 | 0 | 136 | 350650 | 3 | 923 | 160 | 2139 | 87 |
| 6 | PermanentPeace | 0 | 0 | 137 | 300350 | 2 | 934 | 15 | 2240 | 82 |
| 7 | RiskEqualized | 0 | 0 | 124 | 294550 | 2 | 451 | 215 | 1920 | 87 |
| 8 | RiskEqualized | 0 | 0 | 136 | 354250 | 3 | 943 | 113 | 2195 | 87 |
| 9 | Alive | 0 | 0 | 132 | 319650 | 2 | 942 | 151 | 2094 | 89 |
| 10 | RiskEqualized | 0 | 0 | 139 | 355000 | 3 | 944 | 110 | 2104 | 88 |
| 11 | RiskEqualized | 0 | 0 | 137 | 382250 | 2 | 945 | 71 | 2200 | 84 |
| 12 | RiskEqualized | 0 | 0 | 136 | 327025 | 3 | 926 | 151 | 2140 | 84 |
| 13 | RiskEqualized | 0 | 0 | 136 | 411625 | 2 | 859 | 181 | 2077 | 91 |
| 14 | RiskEqualized | 0 | 0 | 128 | 290675 | 2 | 437 | 215 | 1984 | 86 |
| 15 | RiskEqualized | 0 | 0 | 136 | 399250 | 3 | 944 | 129 | 2130 | 88 |
| 16 | RiskEqualized | 0 | 0 | 133 | 358675 | 2 | 940 | 114 | 2150 | 85 |
| 17 | RiskEqualized | 0 | 0 | 135 | 372375 | 3 | 939 | 192 | 2039 | 91 |
| 18 | RiskEqualized | 0 | 0 | 139 | 392125 | 3 | 939 | 130 | 2175 | 83 |
| 19 | Alive | 0 | 0 | 135 | 478125 | 3 | 935 | 112 | 2145 | 89 |
| 20 | RiskEqualized | 0 | 0 | 137 | 405200 | 2 | 934 | 212 | 2175 | 89 |

Summary readings:

- `fuel_empty`: 0/20 seeds.
- `adrift_end`: 0/20 seeds.
- `refuels`: 124 to 139 per run.
- `refuel_spend_micros`: 290675 to 478125.
- `hauler_median_leg_burn_permille`: 2 to 3 after bake.
- `hauler_min_tank_permille`: 437 to 945.

The post-bake reading is a console finding, not a pass/fail: the calibration
removed strandings in this 20-seed reading and shifted the field toward dense
refuel texture plus mostly `RiskEqualized` diagnosis rows.
