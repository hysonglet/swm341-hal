//! ADC configuration types.

/// Number of analog channels an ADC instance exposes (`0`..`12`).
pub const CHANNEL_COUNT: u8 = 12;

/// Sampling-time selectors for the `SEQ_SMP` register.
///
/// Longer sampling gives the SAR hold capacitor more time to charge through the
/// analog mux. This matters most when several pads share one ADC: the driver
/// re-selects the channel before every conversion, and with a short sampling
/// window the *previous* channel's charge is still on the capacitor and bleeds
/// into the result. High-impedance sources (dividers, NTCs, long traces) need
/// the longer settings.
pub mod sample_time {
    pub const CLOCKS_1: u8 = 0;
    pub const CLOCKS_2: u8 = 1;
    pub const CLOCKS_4: u8 = 2;
    pub const CLOCKS_8: u8 = 3;
    pub const CLOCKS_16: u8 = 4;
    pub const CLOCKS_32: u8 = 5;
    pub const CLOCKS_64: u8 = 6;
    pub const CLOCKS_128: u8 = 7;
}

/// Errors an ADC conversion can report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The requested channel does not exist on this ADC (`>= CHANNEL_COUNT`).
    ///
    /// `SEQCHN0.SEQ0` is a 12-bit mask, so an out-of-range channel silently
    /// selects *no* channel and the conversion never completes.
    InvalidChannel,
    /// The value read back was produced by a different channel than requested.
    ///
    /// `SEQn.DR` carries the channel that produced every result (`DR.CHNUM`). A
    /// mismatch means a stale entry was sitting in the sequence FIFO -- for
    /// example because a previous async conversion was cancelled -- so the
    /// driver refuses to hand it back as if it belonged to `expected`.
    ChannelMismatch { expected: u8, got: u8 },
}

/// Configuration of an ADC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Sampling-time register value (`SEQ_SMP`); larger = slower, cleaner samples.
    ///
    /// See [`sample_time`] for the meaningful values.
    pub sampling_time: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Not `CLOCKS_1`: with several pads sharing the converter the hold
            // capacitor barely charges in one ADC clock and neighbouring
            // channels bleed into each other.
            sampling_time: sample_time::CLOCKS_8,
        }
    }
}

impl Config {
    /// Override the sampling time.
    ///
    /// See [`sample_time`] for the meaningful values; raise it when several pads
    /// share the converter or the source impedance is high.
    pub fn sampling_time(self, sampling_time: u8) -> Self {
        Self { sampling_time }
    }
}
