//! UART receive half.
//!
//! The matching write methods live in [`super::tx`]; keeping each functional half
//! in its own module mirrors the `usart` layout of the reference py32f030 HAL.

use super::{Error, Instance, Uart};
use crate::gpio::Pin;
use crate::mode::{Blocking, Mode};

#[cfg(feature = "embassy")]
use super::future;
#[cfg(feature = "embassy")]
use crate::mode::Async;

impl<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode> Uart<'d, T, Tx, Rx, M> {
    /// Blocking read of `buf`, returning once it is filled.
    pub fn blocking_read(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        for slot in buf {
            while !T::is_rx_not_empty() {}
            let (data, parity_error) = T::read_data();
            if parity_error {
                return Err(Error::Parity);
            }
            *slot = data;
        }
        Ok(())
    }
}

impl<'d, T: Instance, Tx: Pin, Rx: Pin> Uart<'d, T, Tx, Rx, Blocking> {
    /// Read a single byte, blocking until one is ready.
    pub fn read(&mut self) -> Result<u8, Error> {
        while !T::is_rx_not_empty() {}
        let (data, parity_error) = T::read_data();
        if parity_error {
            Err(Error::Parity)
        } else {
            Ok(data)
        }
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Tx: Pin, Rx: Pin> Uart<'d, T, Tx, Rx, Async> {
    /// Async read of a single byte.
    pub async fn read(&mut self) -> Result<u8, Error> {
        future::read::<T>().await
    }

    /// Async read of a whole buffer.
    pub async fn read_bytes(&mut self, buf: &mut [u8]) -> Result<(), Error> {
        future::read_bytes::<T>(buf).await
    }
}
