//! Configuration and shared types for the CAN controller.

/// A CAN frame.
///
/// `id` is the 11-bit (standard) or 29-bit (extended) identifier. `len` is the
/// data length, 0..=8. `data` holds the payload (only `len` bytes are valid).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    /// Identifier (11-bit when `extended` is false, 29-bit when true).
    pub id: u32,
    /// `true` for a 29-bit extended frame, `false` for an 11-bit standard frame.
    pub extended: bool,
    /// Remote-transmit-request flag.
    pub rtr: bool,
    /// Data payload.
    pub data: [u8; 8],
    /// Number of valid data bytes, 0..=8.
    pub len: u8,
}

impl Frame {
    /// Build a data frame from an id and a payload slice (up to 8 bytes).
    pub fn new_data(id: u32, extended: bool, payload: &[u8]) -> Frame {
        let len = payload.len().min(8) as u8;
        let mut data = [0u8; 8];
        data[..len as usize].copy_from_slice(&payload[..len as usize]);
        Frame {
            id,
            extended,
            rtr: false,
            data,
            len,
        }
    }

    /// Build a remote frame (RTR) with no payload.
    pub fn new_remote(id: u32, extended: bool) -> Frame {
        Frame {
            id,
            extended,
            rtr: true,
            data: [0u8; 8],
            len: 0,
        }
    }
}

/// CAN configuration.
///
/// The bit timing follows the SJA1000 PeliCAN layout. `brp` is a 10-bit
/// prescaler; the bit rate is `sysclk / ((brp + 1) * (1 + tseg1 + tseg2))`
/// (assuming a 1-Tq sync segment). The [`Default`] values assume a ~60 MHz
/// system clock and target 500 kbit/s — recompute `brp` for your actual clock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// 10-bit prescaler (BRP0..BRP9).
    pub brp: u16,
    /// Synchronization jump width, 0..=3.
    pub sjw: u8,
    /// Time segment 1, 0..=15.
    pub tseg1: u8,
    /// Time segment 2, 0..=7.
    pub tseg2: u8,
    /// Sample point at the third sample (1 = triple sampling).
    pub sam: bool,
    /// Silent loopback mode (no pin activity, self-acknowledged).
    pub loopback: bool,
    /// Accept every received frame (disables the acceptance filter).
    pub accept_all: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            brp: 19,
            sjw: 1,
            tseg1: 3,
            tseg2: 2,
            sam: false,
            loopback: false,
            accept_all: true,
        }
    }
}

/// CAN driver error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A transmit/receive did not complete within the polling budget.
    Timeout,
    /// The controller entered bus-off state.
    BusOff,
}
