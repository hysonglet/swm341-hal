//! I2C slave (peripheral) mode.
//!
//! An [`I2cSlave`] answers to its own address on the bus and reacts to transfers
//! driven by a remote master. Two API styles are provided:
//!
//! * **Event-driven primitives** — [`I2cSlave::blocking_wait_address`],
//!   [`I2cSlave::blocking_read_byte`], [`I2cSlave::blocking_write_byte`] and
//!   [`I2cSlave::blocking_wait_stop`] (plus their async siblings). Call them in a
//!   loop and respond to each byte yourself; this is the most flexible form.
//! * **Buffered transfer** — [`I2cSlave::blocking_transfer`] (and the async
//!   [`I2cSlave::transfer`]) handle one whole transaction for you: after the
//!   master addresses us it either clocks bytes out of `write` (read request) or
//!   fills `read` (write request), then ends with STOP.
//!
//! The controller auto-acknowledges the matched address and every data byte; set
//! [`I2cSlave::blocking_set_tx_nack`] (async: [`I2cSlave::set_tx_nack`]) to
//! NACK the next byte the master reads when you have nothing left to send.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Pin, PinFuncOf};
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::Error;
use super::types::SlaveConfig;

#[cfg(feature = "embassy")]
use crate::mode::Async;

/// An I2C slave (peripheral) with its two pads.
#[allow(dead_code)]
pub struct I2cSlave<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    scl: PeripheralRef<'d, Scl>,
    sda: PeripheralRef<'d, Sda>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> I2cSlave<'d, T, Scl, Sda, M> {
    /// Build an I2C slave from its peripheral and the two pads.
    ///
    /// Each `x_func` picks the alternate function of its pad; only the
    /// [`crate::gpio::PinFuncOf`] value that belongs to that exact pad type checks,
    /// so a wrong pad is a compile error. This is the shared constructor used by
    /// [`I2cSlave::new`] and [`I2cSlave::new_async`].
    pub(crate) fn build(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: SlaveConfig,
    ) -> Self {
        into_ref!(peri, scl, sda);

        T::clock().open();
        crate::gpio::acquire::<Scl>();
        crate::gpio::acquire::<Sda>();
        crate::gpio::set_function::<Scl>(scl_func.value());
        crate::gpio::set_function::<Sda>(sda_func.value());

        T::enter_slave();
        T::set_slave_address(config.address, config.address_mask);
        T::set_slave_10bit(config.address_10bit);

        Self {
            peri,
            scl,
            sda,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2cSlave<'d, T, Scl, Sda, Blocking> {
    /// Build a blocking I2C slave. See [`I2cSlave::build`] for the pad wiring.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: SlaveConfig,
    ) -> Self {
        Self::build(peri, scl, scl_func, sda, sda_func, config)
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2cSlave<'d, T, Scl, Sda, Async> {
    /// Build an async I2C slave. See [`I2cSlave::build`] for the pad wiring.
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: SlaveConfig,
    ) -> Self {
        Self::build(peri, scl, scl_func, sda, sda_func, config)
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2cSlave<'d, T, Scl, Sda, Blocking> {
    /// Wait until the master addresses us, clear the match flag and return the
    /// transfer direction: `true` means the master wants to read (we must send).
    pub fn blocking_wait_address(&mut self) -> Result<bool, Error> {
        while !T::is_address_matched() {
            T::check_faults()?;
        }
        T::clear_address_matched();
        Ok(T::slave_is_read())
    }

    /// Wait for the master to send a byte and return it.
    ///
    /// In a write transaction call this repeatedly until you have read what you
    /// need, then call [`I2cSlave::blocking_wait_stop`]. If the master ends the
    /// transfer early, switch to [`I2cSlave::blocking_wait_stop`] instead of
    /// calling this again.
    pub fn blocking_read_byte(&mut self) -> Result<u8, Error> {
        while !T::is_rxne() {
            T::check_faults()?;
        }
        let byte = T::slave_rx_byte();
        T::clear_rxne();
        Ok(byte)
    }

    /// Wait until the controller can accept a byte and send it to the reading master.
    pub fn blocking_write_byte(&mut self, byte: u8) -> Result<(), Error> {
        while !T::is_txe() {
            T::check_faults()?;
        }
        T::slave_tx_byte(byte);
        Ok(())
    }

    /// NACK (`nack = true`) or ACK (`nack = false`) the next byte the master reads.
    pub fn blocking_set_tx_nack(&mut self, nack: bool) {
        T::slave_set_tx_nack(nack);
    }

    /// Wait until the master issues a STOP, ending the current transfer.
    pub fn blocking_wait_stop(&mut self) -> Result<(), Error> {
        while !T::is_stop_detected() {
            T::check_faults()?;
        }
        T::clear_stop_detected();
        Ok(())
    }

    /// Handle one whole transaction driven by the master.
    ///
    /// After the address is matched, the master either reads from us (so `write`
    /// is shifted out) or writes to us (so `read` is filled). The transfer ends on
    /// STOP. If the master sends more bytes than `read` can hold they are read and
    /// discarded (the RX FIFO must be drained or the bus stalls); if it reads more
    /// than `write` holds we NACK the surplus.
    pub fn blocking_transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        let is_read = self.blocking_wait_address()?;
        if is_read {
            for &b in write {
                self.blocking_write_byte(b)?;
            }
            if write.is_empty() {
                T::slave_set_tx_nack(true);
            }
            self.blocking_wait_stop()?;
            T::slave_set_tx_nack(false);
        } else {
            for slot in read.iter_mut() {
                if T::is_stop_detected() {
                    break;
                }
                *slot = self.blocking_read_byte()?;
            }
            // Drain any extra bytes so the bus does not stall.
            while T::is_rxne() && !T::is_stop_detected() {
                let _ = T::slave_rx_byte();
                T::clear_rxne();
            }
            self.blocking_wait_stop()?;
        }
        Ok(())
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2cSlave<'d, T, Scl, Sda, Async> {
    /// Async version of [`I2cSlave::blocking_wait_address`].
    pub async fn wait_address(&mut self) -> Result<bool, Error> {
        super::future::slave_wait_address::<T>().await
    }

    /// Async version of [`I2cSlave::blocking_read_byte`].
    pub async fn read_byte(&mut self) -> Result<u8, Error> {
        super::future::slave_read_byte::<T>().await
    }

    /// Async version of [`I2cSlave::blocking_write_byte`].
    pub async fn write_byte(&mut self, byte: u8) -> Result<(), Error> {
        super::future::slave_write_byte::<T>(byte).await
    }

    /// NACK (`nack = true`) or ACK (`nack = false`) the next byte the master reads.
    pub fn set_tx_nack(&mut self, nack: bool) {
        T::slave_set_tx_nack(nack);
    }

    /// Async version of [`I2cSlave::blocking_wait_stop`].
    pub async fn wait_stop(&mut self) -> Result<(), Error> {
        super::future::slave_wait_stop::<T>().await
    }

    /// Async version of [`I2cSlave::blocking_transfer`].
    pub async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        super::future::slave_transfer::<T>(read, write).await
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> Drop for I2cSlave<'d, T, Scl, Sda, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
        crate::gpio::release::<Scl>();
        crate::gpio::release::<Sda>();
    }
}
