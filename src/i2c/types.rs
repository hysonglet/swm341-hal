//! I2C configuration types, events and errors.

/// A failure reported by the hardware during a transfer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// Another master won the bus (or the peripheral lost arbitration).
    ArbitrationLost,
    /// The bus stayed busy past the internal timeout (no pull-up, stuck line, or
    /// an unacknowledged address / data byte).
    Timeout,
    /// Received data was not read before the next byte arrived.
    Overrun,
}

impl embedded_hal::i2c::Error for Error {
    fn kind(&self) -> embedded_hal::i2c::ErrorKind {
        match self {
            Error::ArbitrationLost => embedded_hal::i2c::ErrorKind::ArbitrationLoss,
            Error::Timeout => embedded_hal::i2c::ErrorKind::NoAcknowledge(
                embedded_hal::i2c::NoAcknowledgeSource::Unknown,
            ),
            Error::Overrun => embedded_hal::i2c::ErrorKind::Overrun,
        }
    }
}

/// Configuration of an I2C master.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Target SCL frequency in hertz.
    pub frequency: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self { frequency: 100_000 }
    }
}

impl Config {
    /// Override the target SCL frequency.
    pub fn frequency(self, frequency: u32) -> Self {
        Self { frequency }
    }
}

/// Configuration of an I2C slave (peripheral).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlaveConfig {
    /// Own 7-bit address the controller answers to.
    pub address: u8,
    /// Address match mask. Bits set to `1` must match `address`; `0x7F` requires an
    /// exact match, a narrower mask lets the slave answer to a range of addresses.
    pub address_mask: u8,
    /// Use 10-bit address matching instead of 7-bit.
    pub address_10bit: bool,
}

impl Default for SlaveConfig {
    fn default() -> Self {
        Self {
            address: 0x30,
            address_mask: 0x7F,
            address_10bit: false,
        }
    }
}

impl SlaveConfig {
    /// Set the own 7-bit address.
    pub fn address(self, address: u8) -> Self {
        Self { address, ..self }
    }

    /// Set the address match mask.
    pub fn address_mask(self, address_mask: u8) -> Self {
        Self {
            address_mask,
            ..self
        }
    }

    /// Enable 10-bit address matching.
    pub fn address_10bit(self, address_10bit: bool) -> Self {
        Self {
            address_10bit,
            ..self
        }
    }
}
