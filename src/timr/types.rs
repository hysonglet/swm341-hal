//! TIMR configuration types.

/// Configuration of a timer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Value loaded into `TIMR.PREDIV` (0 = divide the timer clock by 1).
    pub prescaler: u32,
    /// Run mode written to `TIMR.CR.MODE`.
    pub mode: u8,
    /// Clock source written to `TIMR.CR.CLKSRC`.
    pub clock_source: u8,
}

impl Default for Config {
    fn default() -> Self {
        // Internal clock, free-running down-count, no prescale.
        Self {
            prescaler: 0,
            mode: 0,
            clock_source: 0,
        }
    }
}

impl Config {
    /// Override the prescaler.
    pub fn prescaler(self, prescaler: u32) -> Self {
        Self { prescaler, ..self }
    }

    /// Override the run mode.
    pub fn mode(self, mode: u8) -> Self {
        Self { mode, ..self }
    }

    /// Override the clock source.
    pub fn clock_source(self, clock_source: u8) -> Self {
        Self { clock_source, ..self }
    }
}
