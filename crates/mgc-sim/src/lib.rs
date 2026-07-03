//! The simulation core: pure, headless, deterministic.
//!
//! Ground rules (enforced by review, not yet by tooling):
//! - No I/O, no rendering, no wall-clock time, no threads.
//! - Advances only via [`Simulation::step`] at a fixed tick rate;
//!   rendering interpolates between ticks and never influences state.
//! - Given the same level package and the same input sequence, the
//!   resulting state is bit-identical on every platform. This is what
//!   makes replay, testing, and (eventually) multiplayer possible.

/// Fixed simulation tick rate.
///
/// Placeholder value. The original advanced one "game turn" per rendered
/// frame, capped by hardware and later by remc2's 24 FPS limiter; the
/// authentic cadence needs to be measured against the reference before
/// gameplay logic lands here.
pub const TICK_RATE_HZ: u32 = 30;

/// The whole game state and its single mutation entry point.
#[derive(Debug, Default)]
pub struct Simulation {
    /// Monotonic tick counter since level start.
    pub tick: u64,
}

impl Simulation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance exactly one fixed tick.
    pub fn step(&mut self) {
        self.tick += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_counted() {
        let mut sim = Simulation::new();
        for _ in 0..10 {
            sim.step();
        }
        assert_eq!(sim.tick, 10);
    }
}
