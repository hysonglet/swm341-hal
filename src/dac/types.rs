//! DAC configuration types.

/// DAC configuration.
#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    /// How [`Dac::write`](crate::dac::Dac::write) packs the value into `DHR`.
    pub format: DataFormat,
}

/// Data format: where the conversion value sits inside the `DHR` register.
///
/// The converter always produces a 12-bit `DOR`; the format only controls which
/// bits of `DHR` feed it (matching the vendor `DAC_FORMAT_*` constants).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataFormat {
    /// 12-bit data in `DHR[11:0]` -> `DOR[11:0]` (vendor `DAC_FORMAT_LSB12B`).
    Lsb12,
    /// 12-bit data in `DHR[15:4]` -> `DOR[11:0]` (vendor `DAC_FORMAT_MSB12B`).
    Msb12,
    /// 8-bit data in `DHR[7:0]` -> `DOR[11:4]` (vendor `DAC_FORMAT_8B`).
    Bit8,
}

impl Default for DataFormat {
    fn default() -> Self {
        DataFormat::Lsb12
    }
}

impl DataFormat {
    /// 2-bit `CR.DHRFMT` value for this format.
    pub(crate) fn bits(self) -> u8 {
        match self {
            DataFormat::Lsb12 => 0,
            DataFormat::Msb12 => 1,
            DataFormat::Bit8 => 3,
        }
    }
}
