//! Watchdog configuration types.

/// Configuration of the watchdog.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Counter reload value (`RSTVAL`), in watchdog-clock ticks.
    pub reload: u32,
    /// Warning threshold (`INTVAL`): the interrupt is raised when the counter
    /// reaches this value, before it rolls over to zero and resets.
    pub warning: u32,
    /// Clock prescaler `CKDIV`, dividing the watchdog clock by `2^ckdiv`.
    pub prescaler: u8,
    /// Reset the chip when the counter reaches zero.
    pub reset_on_timeout: bool,
    /// Raise a warning interrupt at the `warning` threshold.
    pub interrupt_on_warning: bool,
    /// Enable windowed mode (feed only allowed inside the window).
    pub windowed: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // A conservative 16-bit reload with a half-way warning threshold.
            reload: 0xFFFF,
            warning: 0x8000,
            prescaler: 0,
            reset_on_timeout: true,
            interrupt_on_warning: false,
            windowed: false,
        }
    }
}

impl Config {
    /// Override the reload value.
    pub fn reload(self, reload: u32) -> Self {
        Self { reload, ..self }
    }

    /// Override the warning threshold.
    pub fn warning(self, warning: u32) -> Self {
        Self { warning, ..self }
    }

    /// Override the prescaler.
    pub fn prescaler(self, prescaler: u8) -> Self {
        Self { prescaler, ..self }
    }

    /// Override whether a reset is generated on timeout.
    pub fn reset_on_timeout(self, reset: bool) -> Self {
        Self {
            reset_on_timeout: reset,
            ..self
        }
    }

    /// Override whether a warning interrupt is generated.
    pub fn interrupt_on_warning(self, interrupt: bool) -> Self {
        Self {
            interrupt_on_warning: interrupt,
            ..self
        }
    }

    /// Override whether windowed mode is used.
    pub fn windowed(self, windowed: bool) -> Self {
        Self { windowed, ..self }
    }
}
