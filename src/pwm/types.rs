//! PWM configuration types.

/// Configuration of a PWM channel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Counter top (`PERIOD`); one full cycle spans this many timer ticks.
    pub period: u32,
    /// `A` channel compare value (`CMPA`), i.e. the duty cycle in ticks.
    pub duty_a: u32,
    /// Clock source written to `PWM.CR.CLKSRC`.
    pub clock_source: u8,
    /// Clock divider written to `PWM.CR.CLKDIV`.
    pub clock_divider: u8,
    /// Run mode written to `PWM.CR.MODE` by [`start`](super::Pwm::start).
    pub mode: u8,
}

impl Default for Config {
    fn default() -> Self {
        // Internal clock, run mode 1, ~50% duty on a 1000-tick period.
        Self {
            period: 1000,
            duty_a: 500,
            clock_source: 0,
            clock_divider: 0,
            mode: 1,
        }
    }
}

impl Config {
    /// Override the period.
    pub fn period(self, period: u32) -> Self {
        Self { period, ..self }
    }

    /// Override the `A` channel duty.
    pub fn duty_a(self, duty_a: u32) -> Self {
        Self { duty_a, ..self }
    }

    /// Override the clock source.
    pub fn clock_source(self, clock_source: u8) -> Self {
        Self { clock_source, ..self }
    }

    /// Override the clock divider.
    pub fn clock_divider(self, clock_divider: u8) -> Self {
        Self { clock_divider, ..self }
    }

    /// Override the run mode.
    pub fn mode(self, mode: u8) -> Self {
        Self { mode, ..self }
    }
}
