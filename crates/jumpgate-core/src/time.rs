//! Time primitives. `tick: u64` is authoritative (spec §6); `dt` is fixed at
//! init and stored as its u64 bit pattern; `sim_time = (tick as f64) * dt` is a
//! DERIVED helper, never authoritative state, and `dt` is NEVER a step() arg.

/// Authoritative integer tick. `sim_time` is derived from it where needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tick(pub u64);

/// Fixed timestep. Stores f64 but exposes its exact u64 bit pattern for hashing.
#[derive(Clone, Copy, Debug)]
pub struct Dt(f64);

impl Dt {
    pub fn new(dt: f64) -> Dt {
        Dt(dt)
    }
    pub fn get(self) -> f64 {
        self.0
    }
    pub fn bits(self) -> u64 {
        self.0.to_bits()
    }
}

/// Derived: (tick as f64) * dt. Computed only where needed, not stored.
pub fn sim_time(tick: Tick, dt: Dt) -> f64 {
    (tick.0 as f64) * dt.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dt_get_round_trips() {
        assert_eq!(Dt::new(0.5).get(), 0.5);
    }

    #[test]
    fn dt_bits_are_the_f64_bit_pattern() {
        assert_eq!(Dt::new(0.25).bits(), 0.25f64.to_bits());
    }

    #[test]
    fn dt_bits_distinguish_different_dts() {
        assert_ne!(Dt::new(0.25).bits(), Dt::new(0.5).bits());
    }

    #[test]
    fn sim_time_is_tick_times_dt() {
        let dt = Dt::new(0.5);
        assert_eq!(sim_time(Tick(0), dt), 0.0);
        assert_eq!(sim_time(Tick(4), dt), 2.0);
        assert_eq!(sim_time(Tick(10), dt), 5.0);
    }
}
