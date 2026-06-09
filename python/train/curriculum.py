"""Success-ratcheted curriculum: increasingly long sprints, gravity woven in.

Ratchets on ROLLING ARRIVAL RATE crossing a threshold — never on a fixed
schedule. Stage parameters feed JumpgateGymEnv.set_difficulty().

The arrival gate (radius + rendezvous speed) SCALES with the stage: a fixed
1e-4 AU sphere checked once per 1-day tick is a single-tick capture window the
craft steps clean over at sprint+ speeds (at the old 5e-4 gate speed one tick
moves 5 radii). Gates are sized so v <= 2*R/dt (a tick cannot skip the sphere)
and R is a roughly constant fraction of the stage distance.

time_penalty is sized per stage so the worst-case accumulated time cost stays
at ~arrival_bonus/4 (10/4 = 2.5 over the full time limit) — a flat 0.001/tick
made long-stage episodes net-negative regardless of behaviour, teaching
"moving loses points"."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Stage:
    name: str
    target_dist_min: float
    target_dist_max: float
    star_mass: float
    exhaust_velocity: float
    time_limit: int
    arrival_radius: float
    arrival_speed: float

    @property
    def time_penalty(self) -> float:
        return 2.5 / self.time_limit


STAGES = [
    #      name              dmin    dmax   star  v_e   ticks   R       v_gate
    Stage("hop-no-gravity",  0.001,  0.005, 0.0,  0.1,   400,   0.001,  0.002),
    Stage("sprint",          0.005,  0.05,  0.0,  0.1,  1000,   0.005,  0.005),
    Stage("well-shallow",    0.005,  0.05,  0.3,  0.1,  1000,   0.01,   0.01),
    Stage("well-full",       0.02,   0.2,   1.0,  0.2,  2500,   0.01,   0.01),
    Stage("orbital",         0.1,    0.5,   1.0,  0.3,  6000,   0.02,   0.02),
]
PROMOTE_AT = 0.8     # rolling arrival rate to advance
WINDOW = 200         # episodes in the rolling window


class Curriculum:
    def __init__(self):
        self.idx, self.results = 0, []

    @property
    def stage(self):
        return STAGES[self.idx]

    def rolling_rate(self) -> float:
        """Arrival rate over the SAME window the promotion gate uses."""
        recent = self.results[-WINDOW:]
        return (sum(recent) / len(recent)) if recent else 0.0

    def record(self, arrived: bool) -> bool:
        """Record an episode; return True if we just promoted."""
        self.results.append(arrived)
        recent = self.results[-WINDOW:]
        if (len(recent) == WINDOW
                and sum(recent) / WINDOW >= PROMOTE_AT
                and self.idx < len(STAGES) - 1):
            self.idx += 1
            self.results = []
            return True
        return False
