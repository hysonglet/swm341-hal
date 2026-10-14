//! ADC configuration types.

/// Configuration of an ADC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Sampling-time register value (`SEQ_SMP`); larger = slower, cleaner samples.
    pub sampling_time: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { sampling_time: 0 }
    }
}

impl Config {
    /// Override the sampling time.
    pub fn sampling_time(self, sampling_time: u8) -> Self {
        Self {
            sampling_time,
            ..self
        }
    }
}
