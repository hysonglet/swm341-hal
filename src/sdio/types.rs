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

/// Expected response format of an SDIO command, matching `CR2.CMD.RESPTYPE`.
///
/// Encoding (see the SWM341 datasheet, `RESPTYPE` field):
/// * `0` -- no response (e.g. CMD0)
/// * `1` -- 136-bit long response, R2 (CID/CSD)
/// * `2` -- 48-bit short response, R1/R3/R4/R6/R7
/// * `3` -- 48-bit short response with busy signalling (R1b/R5b)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResponseType {
    /// No response expected.
    None = 0,
    /// 136-bit long response (`RESP[0..3]`), used for R2 (CID/CSD).
    Long = 1,
    /// 48-bit short response (`RESP[0]`), used for R1/R3/R4/R6/R7 and SDIO
    /// CMD52/CMD53.
    Short = 2,
    /// 48-bit short response followed by a busy signal on DAT0 (R1b), used for
    /// CMD7 (select card) and other commands that may keep the data line low
    /// while the card processes.
    Busy = 3,
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
    /// Requested SD clock divisor. It is encoded into `CR2.SDCLKDIV` by the
    /// driver (a single set bit at position `n` selects a divisor of `2^(n+1)`:
    /// `0x00` => /1, `0x01` => /2, `0x02` => /4, `0x04` => /8, ...). The SD clock
    /// ends up at `(HCLK / 4) / 2^n`, where the SDIO source clock is `HCLK / 4`.
    pub clock_divider: u8,
    /// Data bus width.
    pub bus_width: BusWidth,
    /// Data timeout (`CR2.TIMEOUT`, 4 bits).
    pub timeout: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // A slow, safe divider (~146 kHz), matching the vendor's ~100 kHz init
            // clock; tighten it once the card is identified.
            clock_divider: 0xFF,
            bus_width: BusWidth::Bus1,
            // `0xC` is the vendor's value and a legal `CR2.TIMEOUT` encoding
            // (`0xF` is reserved, so the previous default was invalid).
            timeout: 0xC,
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
        Self { bus_width, ..self }
    }

    /// Override the data timeout.
    pub fn timeout(self, timeout: u8) -> Self {
        Self { timeout, ..self }
    }
}
