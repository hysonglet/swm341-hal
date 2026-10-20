//! I2C bus master (controller) mode.
//!
//! An [`I2c`] drives SCL and SDA, generates START / STOP and talks to slaves on
//! the bus. Two API styles are provided:
//!
//! * **Embedded-hal traits** — [`I2c`] implements `embedded_hal::i2c::I2c` (and,
//!   with the `embassy` feature, `embedded_hal_async::i2c::I2c`) so it drops into
//!   any generic driver. The `transaction` calls frame each operation strictly:
//!   one START opens the transfer, a (repeated) START separates operations, and a
//!   single STOP ends it.
//! * **Convenience methods** — [`I2c::blocking_write`], [`I2c::blocking_read`] and
//!   [`I2c::blocking_write_read`] (plus their async siblings) cover the common
//!   single-operation cases without building an `Operation` slice.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::i2c::pins;
//! let mut i2c = hal::i2c::I2c::new(
//!     p.i2c0(),
//!     p.PC2,  pins::c::Pin2Func::I2c0Scl,
//!     p.PC1,  pins::c::Pin1Func::I2c0Sda,
//!     hal::i2c::Config::default(),
//! );
//! i2c.blocking_write(0x50, &[0x00, 0x10]).unwrap();
//! let mut data = [0u8; 2];
//! i2c.blocking_read(0x50, &mut data).unwrap();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Pin, PinFuncOf};
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::{Config, Error};

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
use super::future;

/// An I2C master with its two pads.
#[allow(dead_code)]
pub struct I2c<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    scl: PeripheralRef<'d, Scl>,
    sda: PeripheralRef<'d, Sda>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> I2c<'d, T, Scl, Sda, M> {
    /// Build an I2C master from its peripheral and the two pads.
    ///
    /// Each `x_func` picks the alternate function of its pad; only the
    /// [`crate::gpio::PinFuncOf`] value that belongs to that exact pad type checks,
    /// so a wrong pad is a compile error. This is the shared constructor used by
    /// [`I2c::new`] and [`I2c::new_async`].
    pub(crate) fn build(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: Config,
    ) -> Self {
        into_ref!(peri, scl, sda);

        T::clock().open();
        crate::gpio::acquire::<Scl>();
        crate::gpio::acquire::<Sda>();
        crate::gpio::set_function::<Scl>(scl_func.value());
        crate::gpio::set_function::<Sda>(sda_func.value());

        T::enable_master();
        T::set_clock(config.frequency);

        Self {
            peri,
            scl,
            sda,
            _mode: core::marker::PhantomData,
        }
    }

    /// Wait until TXE is set, clearing it; abort on a bus fault.
    fn wait_txe() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::is_txe() {
                T::clear_txe();
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Wait until RXNE is set, clearing it; abort on a bus fault.
    fn wait_rxne() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::is_rxne() {
                T::clear_rxne();
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Wait until TXDONE is set, clearing it; abort on a bus fault.
    fn wait_txdone() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::is_txdone() {
                T::clear_txdone();
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Wait until RXDONE is set, clearing it; abort on a bus fault.
    fn wait_rxdone() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::is_rxdone() {
                T::clear_rxdone();
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Spin until the bus returns to idle.
    fn wait_bus_free() {
        let mut timeout = TIMEOUT;
        while !T::bus_free() {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
        }
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Blocking> {
    /// Build a blocking I2C master. See [`I2c::build`] for the pad wiring.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: Config,
    ) -> Self {
        Self::build(peri, scl, scl_func, sda, sda_func, config)
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Async> {
    /// Build an async I2C master. See [`I2c::build`] for the pad wiring.
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        scl_func: impl PinFuncOf<Scl>,
        sda: impl Peripheral<P = Sda> + 'd,
        sda_func: impl PinFuncOf<Sda>,
        config: Config,
    ) -> Self {
        Self::build(peri, scl, scl_func, sda, sda_func, config)
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Blocking> {
    /// Blocking write of `bytes` to the slave at `address`.
    pub fn blocking_write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        T::txdata(address << 1);
        T::start();
        Self::wait_txe()?;
        for &b in bytes {
            T::txdata(b);
            T::request_write();
            Self::wait_txe()?;
        }
        Self::wait_txdone()?;
        T::stop();
        Self::wait_bus_free();
        Ok(())
    }

    /// Blocking read of `buffer` from the slave at `address`.
    pub fn blocking_read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Error> {
        if buffer.is_empty() {
            return Ok(());
        }
        T::txdata((address << 1) | 1);
        T::start();
        Self::wait_txe()?;
        let len = buffer.len();
        for (i, slot) in buffer.iter_mut().enumerate() {
            let last = i + 1 == len;
            T::request_read(last);
            Self::wait_rxne()?;
            *slot = T::rx_byte();
        }
        Self::wait_rxdone()?;
        Self::wait_bus_free();
        Ok(())
    }

    /// Blocking write followed by a repeated-start read, with no stop in between.
    pub fn blocking_write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error> {
        T::txdata(address << 1);
        T::start();
        Self::wait_txe()?;
        for &b in write {
            T::txdata(b);
            T::request_write();
            Self::wait_txe()?;
        }
        if read.is_empty() {
            Self::wait_txdone()?;
            T::stop();
            Self::wait_bus_free();
            return Ok(());
        }
        T::txdata((address << 1) | 1);
        T::start();
        Self::wait_txe()?;
        let len = read.len();
        for (i, slot) in read.iter_mut().enumerate() {
            let last = i + 1 == len;
            T::request_read(last);
            Self::wait_rxne()?;
            *slot = T::rx_byte();
        }
        Self::wait_rxdone()?;
        Self::wait_bus_free();
        Ok(())
    }
}

impl<T: Instance, Scl: Pin, Sda: Pin, M: Mode> embedded_hal::i2c::ErrorType
    for I2c<'_, T, Scl, Sda, M>
{
    type Error = Error;
}

impl<T: Instance, Scl: Pin, Sda: Pin> embedded_hal::i2c::I2c<u8>
    for I2c<'_, T, Scl, Sda, Blocking>
{
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        let last = operations.len().saturating_sub(1);
        // `true` = a write phase is in progress, `false` = a read phase.
        let mut prev_write: Option<bool> = None;
        for (oi, op) in operations.iter_mut().enumerate() {
            let is_last = oi == last;
            match op {
                embedded_hal::i2c::Operation::Write(bytes) => {
                    if prev_write != Some(true) {
                        T::txdata(address << 1);
                        T::start();
                        Self::wait_txe()?;
                    }
                    for (bi, &b) in bytes.iter().enumerate() {
                        let last_byte = is_last && bi + 1 == bytes.len();
                        T::txdata(b);
                        T::request_write();
                        Self::wait_txe()?;
                        if last_byte {
                            T::stop();
                        }
                    }
                    prev_write = Some(true);
                }
                embedded_hal::i2c::Operation::Read(buffer) => {
                    if prev_write != Some(false) {
                        T::txdata((address << 1) | 1);
                        T::start();
                        Self::wait_txe()?;
                    }
                    let len = buffer.len();
                    for (bi, slot) in buffer.iter_mut().enumerate() {
                        let last_byte = is_last && bi + 1 == len;
                        T::request_read(last_byte);
                        Self::wait_rxne()?;
                        *slot = T::rx_byte();
                    }
                    prev_write = Some(false);
                }
            }
        }
        Self::wait_bus_free();
        Ok(())
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Async> {
    /// Async write of `bytes` to the slave at `address`.
    pub async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        future::write::<T>(address, bytes).await
    }

    /// Async read of `buffer` from the slave at `address`.
    pub async fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Error> {
        future::read::<T>(address, buffer).await
    }

    /// Async write followed by a repeated-start read.
    pub async fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error> {
        future::write_read::<T>(address, write, read).await
    }
}

#[cfg(feature = "embassy")]
impl<T: Instance, Scl: Pin, Sda: Pin> embedded_hal_async::i2c::I2c<u8>
    for I2c<'_, T, Scl, Sda, Async>
{
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal_async::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        // Run every operation as one strict I2C transfer: a single START opens the
        // first operation, a (repeated) START separates each following operation,
        // and one STOP ends the whole transfer. This is what register-address-then-read
        // slaves require.
        future::master::<T>(address, operations).await
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> Drop for I2c<'d, T, Scl, Sda, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
        crate::gpio::release::<Scl>();
        crate::gpio::release::<Sda>();
    }
}

/// Iterations of the flag poll before giving up; the loop is cheap and the bus
/// should never be busy for long, so this is a generous ceiling.
const TIMEOUT: u32 = 10_000_000;
