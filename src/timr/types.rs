//! TIMR configuration types.

/// TIMR run mode, written to the `TIMR.CR.MODE` field (bits 2..=3).
///
/// Counting external pulses is *not* a separate `MODE` value: it is `Timer` mode
/// paired with [`ClockSource::External`] — the hardware selects the clock via
/// `CR.CLKSRC`, so a pulse counter is `RunMode::Timer` + `ClockSource::External`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunMode {
    /// Free-running down-counter driven from the timer clock (`TIMR_MODE_TIMER`).
    Timer,
    /// Input capture (`TIMR_MODE_IC`).
    InputCapture,
    /// Output compare (`TIMR_MODE_OC`).
    OutputCompare,
}

impl RunMode {
    /// Raw `TIMR.CR.MODE` field value.
    pub(crate) const fn bits(self) -> u8 {
        match self {
            RunMode::Timer => 0,
            RunMode::InputCapture => 1,
            RunMode::OutputCompare => 2,
        }
    }
}

/// TIMR clock source, written to the `TIMR.CR.CLKSRC` field (bits 0..=1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockSource {
    /// Internal `PCLK` / system clock (`CR.CLKSRC = 0`).
    Internal,
    /// External `TxI` pin pulse, rising edge (`CR.CLKSRC = 2`).
    External,
}

impl ClockSource {
    /// Raw `TIMR.CR.CLKSRC` field value.
    pub(crate) const fn bits(self) -> u8 {
        match self {
            ClockSource::Internal => 0,
            ClockSource::External => 2,
        }
    }
}

/// Configuration of a timer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Value written to `TIMR.PREDIV`; the timer clock is `PCLK / (prescaler + 1)`
    /// (0 = no prescale). The vendor library's `prediv` is the divider itself, so its
    /// `prediv - 1` equals this field.
    pub prescaler: u32,
    /// Run mode written to `TIMR.CR.MODE`.
    pub mode: RunMode,
    /// Clock source written to `TIMR.CR.CLKSRC`.
    pub clock_source: ClockSource,
}

impl Default for Config {
    fn default() -> Self {
        // Internal clock, free-running down-count, no prescale.
        Self {
            prescaler: 0,
            mode: RunMode::Timer,
            clock_source: ClockSource::Internal,
        }
    }
}

impl Config {
    /// Override the prescaler (raw `PREDIV` value).
    pub fn prescaler(self, prescaler: u32) -> Self {
        Self { prescaler, ..self }
    }

    /// Override the run mode.
    pub fn mode(self, mode: RunMode) -> Self {
        Self { mode, ..self }
    }

    /// Override the clock source.
    pub fn clock_source(self, clock_source: ClockSource) -> Self {
        Self { clock_source, ..self }
    }

    /// Pick the smallest 8-bit prescaler so that `ticks` TIMR-input-clock ticks fit
    /// the 24-bit `LOAD`, returning `(prescaler, load)`.
    ///
    /// The counter runs at `timer_hz / (prescaler + 1)` and counts `load` ticks, so
    /// `load * (prescaler + 1)` equals `ticks` (clamped to the 24-bit maximum
    /// `16_777_215` if even the ÷256 prescaler cannot fit it). `timer_hz` is the TIMR
    /// input clock before prescaling (PCLK, e.g. `120_000_000`). The largest interval
    /// representable at 120 MHz is therefore `16_777_215 * 256 / 120_000_000`
    /// ≈ 35.8 s.
    pub fn prescale_for_ticks(ticks: u32) -> (u32, u32) {
        const MAX: u32 = (1u32 << 24) - 1; // 16_777_215
        let mut p = 0u32;
        while p < 255 && ticks / (p + 1) > MAX {
            p += 1;
        }
        let load = ticks / (p + 1);
        (p, load.min(MAX))
    }

    /// Like [`prescale_for_ticks`](Self::prescale_for_ticks) but takes the interval in
    /// microseconds at the given TIMR input clock `timer_hz` (PCLK).
    pub fn prescale_for(timer_hz: u32, micros: u32) -> (u32, u32) {
        let ticks = ((timer_hz as u64) * (micros as u64) / 1_000_000) as u32;
        Self::prescale_for_ticks(ticks)
    }

    /// Build a `Config` plus the matching `LOAD` for a `micros`-microsecond interval at
    /// the TIMR input clock `timer_hz`, in internal timer mode. Returns `(config,
    /// load)`; feed `load` to [`Timr::start`](crate::timr::Timr::start).
    ///
    /// ```no_run
    /// # use swm341_hal as hal;
    /// # let p = hal::init(Default::default());
    /// // One second at the 120 MHz PCLK; the prescaler is derived from the interval.
    /// let (cfg, load) = hal::timr::Config::from_micros(120_000_000, 1_000_000);
    /// let mut timer = hal::timr::Timr::new(p.TIMR0, cfg);
    /// timer.clear_overflow();
    /// timer.start(load);
    /// while !timer.overflow_flag() {}
    /// ```
    pub fn from_micros(timer_hz: u32, micros: u32) -> (Self, u32) {
        let (prescaler, load) = Self::prescale_for(timer_hz, micros);
        (
            Self::default()
                .mode(RunMode::Timer)
                .clock_source(ClockSource::Internal)
                .prescaler(prescaler),
            load,
        )
    }
}
