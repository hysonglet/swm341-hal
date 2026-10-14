//! UART transmit half.
//!
//! The matching read methods live in [`super::rx`]; keeping each functional half
//! in its own module mirrors the `usart` layout of the reference py32f030 HAL.

use super::{Instance, Uart};
use crate::gpio::Pin;
use crate::mode::{Blocking, Mode};

#[cfg(feature = "embassy")]
use super::future;
#[cfg(feature = "embassy")]
use crate::mode::Async;

impl<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode> Uart<'d, T, Tx, Rx, M> {
    /// Blocking write of a whole buffer, returning once the last byte is queued.
    pub fn blocking_write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            while T::is_tx_fifo_full() {}
            T::write_data(b);
        }
    }
}

impl<'d, T: Instance, Tx: Pin, Rx: Pin> Uart<'d, T, Tx, Rx, Blocking> {
    /// Write a single byte, blocking until there is room.
    pub fn write(&mut self, byte: u8) {
        while T::is_tx_fifo_full() {}
        T::write_data(byte);
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Tx: Pin, Rx: Pin> Uart<'d, T, Tx, Rx, Async> {
    /// Async write of a whole buffer.
    pub async fn write(&mut self, bytes: &[u8]) {
        future::write::<T>(bytes).await;
    }
}
