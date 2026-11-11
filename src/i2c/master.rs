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
//! let mut i2c = hal::i2c::I2c::new(
//!     p.i2c0(),
//!     p.PC2,  // SCL
//!     p.PC1,  // SDA
//!     hal::i2c::Config::default(),
//! );
//! i2c.blocking_write(0x50, &[0x00, 0x10]).unwrap();
//! let mut data = [0u8; 2];
//! i2c.blocking_read(0x50, &mut data).unwrap();
//! ```
//!
//! The SCL/SDA `PORTx.FUNC` codes come from the pin types and the I2C instance
//! via [`I2cSclPin`]/[`I2cSdaPin`], so no alternate-function value is needed.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::Pin;
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::{Config, Error, I2cSclPin, I2cSdaPin};

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
    /// The SCL/SDA `PORTx.FUNC` codes are derived from `scl`/`sda` and the I2C
    /// instance `T` via [`I2cSclPin`]/[`I2cSdaPin`], so only the pins themselves
    /// are needed; a pad that cannot serve the requested role is a compile
    /// error. This is the shared constructor used by [`I2c::new`] and
    /// [`I2c::new_async`].
    pub(crate) fn build(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        into_ref!(peri, scl, sda);

        T::clock().open();
        crate::gpio::acquire::<Scl>();
        crate::gpio::acquire::<Sda>();
        crate::gpio::set_function::<Scl>(<Scl as I2cSclPin<T>>::FUNC);
        crate::gpio::set_function::<Sda>(<Sda as I2cSdaPin<T>>::FUNC);
        crate::gpio::set_i2c_pad::<Scl>();
        crate::gpio::set_i2c_pad::<Sda>();

        // Registers must be programmed while EN = 0 (datasheet master init
        // flow). enable_master() also clears stale interrupt flags.
        T::disable();
        T::set_clock(config.frequency);
        T::enable_master();

        Self {
            peri,
            scl,
            sda,
            _mode: core::marker::PhantomData,
        }
    }

    /// Wait until the hardware clears the current MCR command (START, a byte
    /// or STOP), aborting on a bus fault or a stuck line.
    fn wait_cmd() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::cmd_done() {
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Send a (repeated) START followed by the 7-bit address / R-W byte and
    /// verify the slave acknowledged it.
    fn start_addr(address: u8, read: bool) -> Result<(), Error> {
        T::txdata((address << 1) | u8::from(read));
        T::start();
        Self::wait_cmd()?;
        if !T::acked() {
            return Err(Error::NoAcknowledge);
        }
        Ok(())
    }

    /// Transmit one data byte and verify its ACK.
    fn write_byte(byte: u8) -> Result<(), Error> {
        T::txdata(byte);
        T::request_write();
        Self::wait_cmd()?;
        if !T::acked() {
            return Err(Error::NoAcknowledge);
        }
        Ok(())
    }

    /// Receive one byte; `nack` makes the master NACK it (last byte of a read
    /// phase) instead of ACKing it.
    fn read_byte(nack: bool) -> Result<u8, Error> {
        T::request_read(nack);
        Self::wait_cmd()?;
        Ok(T::rx_byte())
    }

    /// Terminate the transfer with STOP and wait until the bus is idle.
    fn finish() -> Result<(), Error> {
        T::stop();
        Self::wait_cmd()?;
        Self::wait_bus_free();
        T::clear_flags();
        Ok(())
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
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        Self::build(peri, scl, sda, config)
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Async> {
    /// Build an async I2C master. See [`I2c::build`] for the pad wiring.
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        Self::build(peri, scl, sda, config)
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Blocking> {
    /// Blocking write of `bytes` to the slave at `address`.
    pub fn blocking_write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, false)?;
            for &b in bytes {
                Self::write_byte(b)?;
            }
            Ok(())
        })();
        result.and(Self::finish())
    }

    /// Blocking read of `buffer` from the slave at `address`.
    pub fn blocking_read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Error> {
        if buffer.is_empty() {
            return Ok(());
        }
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, true)?;
            let len = buffer.len();
            for (i, slot) in buffer.iter_mut().enumerate() {
                *slot = Self::read_byte(i + 1 == len)?;
            }
            Ok(())
        })();
        result.and(Self::finish())
    }

    /// Blocking write followed by a repeated-start read, with no stop in between.
    pub fn blocking_write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error> {
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, false)?;
            for &b in write {
                Self::write_byte(b)?;
            }
            if !read.is_empty() {
                Self::start_addr(address, true)?;
                let len = read.len();
                for (i, slot) in read.iter_mut().enumerate() {
                    *slot = Self::read_byte(i + 1 == len)?;
                }
            }
            Ok(())
        })();
        result.and(Self::finish())
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
        if operations.is_empty() {
            return Ok(());
        }
        // Each operation is addressed on its own: the first START opens the
        // transfer, a repeated START separates operations, and one STOP closes
        // it. This is exactly what register-address-then-read slaves need.
        let result: Result<(), Error> = (|| {
            for op in operations.iter_mut() {
                match op {
                    embedded_hal::i2c::Operation::Write(bytes) => {
                        Self::start_addr(address, false)?;
                        for &b in bytes.iter() {
                            Self::write_byte(b)?;
                        }
                    }
                    embedded_hal::i2c::Operation::Read(buffer) => {
                        Self::start_addr(address, true)?;
                        let len = buffer.len();
                        for (i, slot) in buffer.iter_mut().enumerate() {
                            *slot = Self::read_byte(i + 1 == len)?;
                        }
                    }
                }
            }
            Ok(())
        })();
        result.and(Self::finish())
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
