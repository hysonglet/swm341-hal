//! SDIO configuration and error types.

use crate::pac::sdio::if_::R as IfFlags;

/// Data bus width of the SDIO / SD card interface.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BusWidth {
    /// 1-bit data bus.
    Bus1,
    /// 4-bit data bus.
    Bus4,
    /// 8-bit data bus.
    Bus8,
}

/// Expected response format of an SDIO command.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResponseType {
    /// No response expected.
    None = 0,
    /// 48-bit short response (`RESP[0]`).
    Short = 1,
    /// 136-bit long response (`RESP[0..3]`).
    Long = 2,
}

/// An error reported by the SDIO controller.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdioError {
    /// The command or data phase timed out.
    Timeout,
    /// A CRC check failed.
    Crc,
    /// The command or data ended prematurely.
    End,
    /// The command index did not match.
    Index,
    /// The card response was malformed.
    Response,
    /// An unspecified controller error.
    Other,
}

impl SdioError {
    /// Map the controller status flags onto an [`SdioError`].
    pub(crate) fn from_flags(f: &IfFlags) -> Self {
        if f.cmdtimeout().bit() || f.dattimeout().bit() {
            Self::Timeout
        } else if f.cmdcrcerr().bit() || f.datcrcerr().bit() {
            Self::Crc
        } else if f.cmdenderr().bit() || f.datenderr().bit() {
            Self::End
        } else if f.cmdidxerr().bit() {
            Self::Index
        } else if f.resperr().bit() {
            Self::Response
        } else {
            Self::Other
        }
    }
}

/// Configuration of the SDIO host.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// SD clock divider (`CR2.SDCLKDIV`); the SD clock runs at
    /// `peripheral_clock / (divider + 1)`.
    pub clock_divider: u8,
    /// Data bus width.
    pub bus_width: BusWidth,
    /// Data timeout (`CR2.TIMEOUT`, 4 bits).
    pub timeout: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // A slow, safe divider; tighten it once the card is identified.
            clock_divider: 0xFF,
            bus_width: BusWidth::Bus1,
            timeout: 0xF,
        }
    }
}

impl Config {
    /// Override the clock divider.
    pub fn clock_divider(self, clock_divider: u8) -> Self {
        Self {
            clock_divider,
            ..self
        }
    }

    /// Override the bus width.
    pub fn bus_width(self, bus_width: BusWidth) -> Self {
        Self {
            bus_width,
            ..self
        }
    }

    /// Override the data timeout.
    pub fn timeout(self, timeout: u8) -> Self {
        Self { timeout, ..self }
    }
}
