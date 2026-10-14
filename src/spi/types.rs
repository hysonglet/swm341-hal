//! SPI configuration types, events and errors.

/// SPI clock polarity / phase, the four standard modes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SpiMode {
    /// CPOL = 0, CPHA = 0.
    #[default]
    Mode0,
    /// CPOL = 0, CPHA = 1.
    Mode1,
    /// CPOL = 1, CPHA = 0.
    Mode2,
    /// CPOL = 1, CPHA = 1.
    Mode3,
}

/// Bit transmission order within a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BitOrder {
    /// Most significant bit first.
    #[default]
    MsbFirst,
    /// Least significant bit first.
    LsbFirst,
}

/// A failure reported by the hardware during a transfer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The RX FIFO overflowed because the caller did not drain it fast enough.
    Overrun,
    /// A transfer completed but the controller reported an unspecified fault.
    Other,
    /// A slave read was cut short because the master deasserted CS.
    TransferEnded,
}

impl embedded_hal::spi::Error for Error {
    fn kind(&self) -> embedded_hal::spi::ErrorKind {
        match self {
            Error::Overrun => embedded_hal::spi::ErrorKind::Overrun,
            Error::Other => embedded_hal::spi::ErrorKind::Other,
            Error::TransferEnded => embedded_hal::spi::ErrorKind::Other,
        }
    }
}

/// Configuration of an SPI master.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Target SCK frequency in hertz. The hardware divider is 3 bits wide, so the
    /// realised rate is `sysclk / (2 * (clkdiv + 1))` rounded up.
    pub frequency: u32,
    /// Clock polarity / phase.
    pub mode: SpiMode,
    /// Bit transmission order.
    pub bit_order: BitOrder,
    /// Bits per frame, 4..=16. The blocking / async transfer helpers operate on
    /// `u8` buffers, so 8 (the default) is what most callers want.
    pub frame_size: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            frequency: 1_000_000,
            mode: SpiMode::Mode0,
            bit_order: BitOrder::MsbFirst,
            frame_size: 8,
        }
    }
}

impl Config {
    /// Override the target SCK frequency.
    pub fn frequency(self, frequency: u32) -> Self {
        Self { frequency, ..self }
    }

    /// Override the clock polarity / phase.
    pub fn mode(self, mode: SpiMode) -> Self {
        Self { mode, ..self }
    }

    /// Override the bit transmission order.
    pub fn bit_order(self, bit_order: BitOrder) -> Self {
        Self { bit_order, ..self }
    }
}
