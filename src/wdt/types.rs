//! Watchdog configuration types.

/// Configuration of the watchdog.
///
/// The counter runs off the watchdog clock, which is `HCLK` divided by
/// `2^CKDIV` (see [`Config::prescaler`]). The `reload` and `warning` values are
/// expressed in those watchdog-clock ticks, but the `reload_ms` / `reload_secs`
/// / `warning_ms` / `warning_secs` helpers below let you specify them in real
/// time units instead.
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
    /// Watchdog input clock frequency (before `CKDIV`), in hertz.
    ///
    /// `0` (the default) means "fall back to [`crate::clock::sys_core_clock`]",
    /// i.e. assume the watchdog is driven straight from `HCLK`. Set it with
    /// [`Config::clock_hz`] only if the chip routes the watchdog from another
    /// source. The `ms` / `secs` helpers use this to convert time into ticks.
    pub wdt_clock_hz: u32,
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
            wdt_clock_hz: 0,
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

    /// Override the assumed watchdog input clock (Hz), used by the `ms` / `secs`
    /// helpers. Leave at `0` to use [`crate::clock::sys_core_clock`] (the common
    /// case where the watchdog is clocked from `HCLK`).
    ///
    /// Call this *before* [`Config::reload_ms`] / [`Config::reload_secs`] /
    /// [`Config::warning_ms`] / [`Config::warning_secs`].
    pub fn clock_hz(mut self, hz: u32) -> Self {
        self.wdt_clock_hz = hz;
        self
    }

    /// Watchdog counter frequency in hertz after the `CKDIV` prescaler.
    #[inline]
    fn counter_hz(&self) -> u64 {
        let base = if self.wdt_clock_hz != 0 {
            self.wdt_clock_hz
        } else {
            crate::clock::sys_core_clock()
        };
        base as u64 / (1u64 << (self.prescaler as u32 & 0x0f))
    }

    /// Set [`Config::reload`] from a duration in milliseconds.
    ///
    /// Panics are avoided: if the result overflows the 32-bit `RSTVAL` register
    /// it is clamped to [`u32::MAX`]. Set [`Config::prescaler`] first if you
    /// need a larger window.
    pub fn reload_ms(mut self, ms: u32) -> Self {
        let ticks = self.counter_hz() * ms as u64 / 1000;
        self.reload = if ticks > u32::MAX as u64 {
            u32::MAX
        } else {
            ticks as u32
        };
        self
    }

    /// Set [`Config::reload`] from a duration in seconds.
    pub fn reload_secs(mut self, secs: u32) -> Self {
        let ticks = self.counter_hz() * secs as u64;
        self.reload = if ticks > u32::MAX as u64 {
            u32::MAX
        } else {
            ticks as u32
        };
        self
    }

    /// Set [`Config::warning`] from a duration in milliseconds.
    pub fn warning_ms(mut self, ms: u32) -> Self {
        let ticks = self.counter_hz() * ms as u64 / 1000;
        self.warning = if ticks > u32::MAX as u64 {
            u32::MAX
        } else {
            ticks as u32
        };
        self
    }

    /// Set [`Config::warning`] from a duration in seconds.
    pub fn warning_secs(mut self, secs: u32) -> Self {
        let ticks = self.counter_hz() * secs as u64;
        self.warning = if ticks > u32::MAX as u64 {
            u32::MAX
        } else {
            ticks as u32
        };
        self
    }
}
