//! UART configuration types, events and errors.

/// Number of data bits per frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DataBits {
    /// 8 data bits (the common case).
    #[default]
    Data8,
    /// 9 data bits.
    Data9,
}

/// Parity applied to a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Parity {
    /// No parity bit.
    #[default]
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
    /// Parity bit forced to 1.
    One,
    /// Parity bit forced to 0.
    Zero,
}

impl Parity {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Parity::None => 0,
            Parity::Odd => 1,
            Parity::Even => 3,
            Parity::One => 5,
            Parity::Zero => 7,
        }
    }
}

/// Number of stop bits.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum StopBits {
    /// One stop bit.
    #[default]
    Stop1,
    /// Two stop bits.
    Stop2,
}

/// A failure reported by the hardware.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// A frame arrived with the wrong parity.
    Parity,
}

/// Configuration of a UART.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Desired baud rate in bits per second.
    pub baudrate: u32,
    /// Data bits per frame.
    pub data_bits: DataBits,
    /// Parity scheme.
    pub parity: Parity,
    /// Stop bits.
    pub stop_bits: StopBits,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            baudrate: 115_200,
            data_bits: DataBits::Data8,
            parity: Parity::None,
            stop_bits: StopBits::Stop1,
        }
    }
}

impl Config {
    /// Override the baud rate.
    pub fn baudrate(self, baudrate: u32) -> Self {
        Self { baudrate, ..self }
    }

    /// Override the data bits.
    pub fn data_bits(self, data_bits: DataBits) -> Self {
        Self { data_bits, ..self }
    }

    /// Override the parity.
    pub fn parity(self, parity: Parity) -> Self {
        Self { parity, ..self }
    }

    /// Override the stop bits.
    pub fn stop_bits(self, stop_bits: StopBits) -> Self {
        Self { stop_bits, ..self }
    }
}
