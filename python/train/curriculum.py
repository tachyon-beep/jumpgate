"""Success-ratcheted curriculum: increasingly long sprints, gravity woven in.

Ratchets on ROLLING ARRIVAL RATE crossing a threshold — never on a fixed
schedule. Stage parameters feed JumpgateGymEnv.set_difficulty()."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Stage:
    name: str
    target_dist_min: float
    target_dist_max: float
    star_mass: float
    exhaust_velocity: float
    time_limit: int


STAGES = [
    #      name              dmin    dmax   star  v_e   ticks
    Stage("hop-no-gravity",  0.001,  0.005, 0.0,  0.1,   400),
    Stage("sprint",          0.005,  0.05,  0.0,  0.1,  1000),
    Stage("well-shallow",    0.005,  0.05,  0.3,  0.1,  1000),
    Stage("well-full",       0.02,   0.2,   1.0,  0.2,  2500),
    Stage("orbital",         0.1,    0.5,   1.0,  0.3,  6000),
]
PROMOTE_AT = 0.8     # rolling arrival rate to advance
WINDOW = 200         # episodes in the rolling window


class Curriculum:
    def __init__(self):
        self.idx, self.results = 0, []

    @property
    def stage(self):
        return STAGES[self.idx]

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
