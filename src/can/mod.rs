//! Controller-area-network (CAN) driver.
//!
//! The [`Can`] object configures the bit timing, arms the acceptance filter and
//! provides blocking [`Can::transmit`] and [`Can::receive`] over the shared
//! PeliCAN frame buffer. Polling uses the `SR` status flags (`TXBR` for a free
//! transmit buffer, `RXDA` for a received frame, `TXOK` for completion).
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::can::{Can, Frame};
//! let mut can = Can::new(p.CAN0, Default::default());
//! can.transmit(&Frame::new_data(0x123, false, &[1, 2, 3])).unwrap();
//! if let Some(frame) = can.receive() { /* ... */ }
//! ```
//!
//! # Frame buffer layout
//!
//! The identifier is packed into the leading bytes of the `DATA` area (standard:
//! 2 bytes, extended: 4 bytes) in big-endian order, and `INFO` carries
//! FF / RTR / DLC — the standard PeliCAN arrangement. This matches the SWM341
//! reference; the exact byte/word endianness in the 32-bit `DATA` words follows
//! the documented PeliCAN layout and can be adjusted if your silicon differs.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

pub mod instance;
mod types;

pub use types::*;

/// A CAN controller instance.
#[allow(dead_code)]
pub struct Can<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Can<'d, T, M> {
    /// Take ownership of the CAN controller and apply `config`.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::set_timing(config.brp, config.sjw, config.tseg1, config.tseg2, config.sam);
        T::set_loopback(config.loopback);
        if config.accept_all {
            T::set_accept_all();
        }
        T::enable();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, M: Mode> Drop for Can<'d, T, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
    }
}

/// Pack a [`Frame`] into the raw `(info, data_words)` buffer form.
fn pack(frame: &Frame) -> (u32, [u32; 12]) {
    let mut buf = [0u8; 48];
    let data_start = if frame.extended {
        buf[0] = ((frame.id >> 21) & 0xFF) as u8;
        buf[1] = ((frame.id >> 13) & 0xFF) as u8;
        buf[2] = ((frame.id >> 5) & 0xFF) as u8;
        buf[3] = ((frame.id & 0x1F) << 3) as u8;
        4
    } else {
        buf[0] = ((frame.id >> 3) & 0xFF) as u8;
        buf[1] = ((frame.id & 0x07) << 5) as u8;
        2
    };
    let len = (frame.len as usize).min(8);
    buf[data_start..data_start + len].copy_from_slice(&frame.data[..len]);

    let mut words = [0u32; 12];
    for i in 0..12 {
        words[i] =
            u32::from_le_bytes([buf[4 * i], buf[4 * i + 1], buf[4 * i + 2], buf[4 * i + 3]]);
    }
    let info = ((frame.extended as u32) << 7)
        | ((frame.rtr as u32) << 6)
        | (frame.len as u32 & 0x0F);
    (info, words)
}

/// Unpack a raw `(info, data_words)` buffer back into a [`Frame`].
fn unpack(info: u32, data: [u32; 12]) -> Frame {
    let extended = (info >> 7) & 1 != 0;
    let rtr = (info >> 6) & 1 != 0;
    let len = (info & 0x0F) as usize;
    let mut buf = [0u8; 48];
    for i in 0..12 {
        let w = data[i].to_le_bytes();
        buf[4 * i..4 * i + 4].copy_from_slice(&w);
    }
    let (id, data_start) = if extended {
        let id = ((buf[0] as u32) << 21)
            | ((buf[1] as u32) << 13)
            | ((buf[2] as u32) << 5)
            | ((buf[3] as u32) >> 3);
        (id, 4)
    } else {
        let id = ((buf[0] as u32) << 3) | ((buf[1] as u32) >> 5);
        (id, 2)
    };
    let mut payload = [0u8; 8];
    let len = len.min(8);
    payload[..len].copy_from_slice(&buf[data_start..data_start + len]);
    Frame {
        id,
        extended,
        rtr,
        data: payload,
        len: len as u8,
    }
}

impl<'d, T: Instance> Can<'d, T, Blocking> {
    /// Transmit a frame, blocking until the controller reports transmission OK.
    pub fn transmit(&mut self, frame: &Frame) -> Result<(), Error> {
        let (info, data) = pack(frame);
        let mut timeout = 10_000_000u32;
        while !T::is_tx_ready() {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
            if T::is_bus_off() {
                return Err(Error::BusOff);
            }
        }
        T::write_frame(info, data);
        T::cmd_transmit();

        let mut timeout = 10_000_000u32;
        while !T::is_tx_ok() {
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
            if T::is_bus_off() {
                return Err(Error::BusOff);
            }
        }
        Ok(())
    }

    /// Non-blocking receive: returns `Some(frame)` if a frame is waiting in the
    /// buffer, releasing it. Returns `None` when the buffer is empty.
    pub fn receive(&mut self) -> Option<Frame> {
        if !T::is_rx_available() {
            return None;
        }
        let (info, data) = T::read_frame();
        T::cmd_release_rx();
        Some(unpack(info, data))
    }
}
