//! SPI slave (peripheral) mode.
//!
//! An [`SpiSlave`] is driven by an external clock and chip-select: the master
//! supplies SCK and SSN, [`SpiSlave`] answers on MISO and samples MOSi. Two API
//! styles are provided:
//!
//! * **Event-driven primitives** — [`SpiSlave::blocking_wait_start`],
//!   [`SpiSlave::blocking_read_byte`], [`SpiSlave::blocking_write_byte`] and
//!   [`SpiSlave::blocking_wait_end`] (plus their async siblings). Wait for the
//!   master to assert CS, then exchange bytes until it deasserts CS.
//! * **Buffered transfer** — [`SpiSlave::blocking_transfer`] (and the async
//!   [`SpiSlave::transfer`]) handle one whole CS-active transfer for you: every
//!   byte you push into `write` is shifted out while every byte the master clocks
//!   in is stored into `read`, until CS rises.
//!
//! The transfer length is set by the master (how many clocks it generates); the
//! buffered call stops as soon as CS is deasserted.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Pin, PinFuncOf};
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::Error;
use super::types::{BitOrder, Config, SpiMode};

#[cfg(feature = "embassy")]
use crate::mode::Async;

/// An SPI slave (peripheral) with its four pads.
#[allow(dead_code)]
pub struct SpiSlave<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    sck: PeripheralRef<'d, Sck>,
    mosi: PeripheralRef<'d, Mosi>,
    miso: PeripheralRef<'d, Miso>,
    cs: PeripheralRef<'d, Cs>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode> SpiSlave<'d, T, Sck, Mosi, Miso, Cs, M> {
    /// Build an SPI slave from its peripheral and the four pads.
    ///
    /// Each `x_func` picks the alternate function of its pad; only the
    /// [`crate::gpio::PinFuncOf`] value that belongs to that exact pad type checks,
    /// so a wrong pad is a compile error. This is the shared constructor used by
    /// [`SpiSlave::new`] and [`SpiSlave::new_async`]. `config.frequency` is ignored
    /// in slave mode (the clock is supplied by the master).
    pub(crate) fn build(
        peri: impl Peripheral<P = T> + 'd,
        sck: impl Peripheral<P = Sck> + 'd,
        sck_func: impl PinFuncOf<Sck>,
        mosi: impl Peripheral<P = Mosi> + 'd,
        mosi_func: impl PinFuncOf<Mosi>,
        miso: impl Peripheral<P = Miso> + 'd,
        miso_func: impl PinFuncOf<Miso>,
        cs: impl Peripheral<P = Cs> + 'd,
        cs_func: impl PinFuncOf<Cs>,
        config: Config,
    ) -> Self {
        into_ref!(peri, sck, mosi, miso, cs);

        T::clock().open();
        crate::gpio::acquire::<Sck>();
        crate::gpio::acquire::<Mosi>();
        crate::gpio::acquire::<Miso>();
        crate::gpio::acquire::<Cs>();
        crate::gpio::set_function::<Sck>(sck_func.value());
        crate::gpio::set_function::<Mosi>(mosi_func.value());
        crate::gpio::set_function::<Miso>(miso_func.value());
        crate::gpio::set_function::<Cs>(cs_func.value());

        let cpol = matches!(config.mode, SpiMode::Mode2 | SpiMode::Mode3);
        let cpha = matches!(config.mode, SpiMode::Mode1 | SpiMode::Mode3);

        T::configure_slave_line(cpol, cpha, config.bit_order == BitOrder::LsbFirst, config.frame_size);
        T::enter_slave();

        Self {
            peri,
            sck,
            mosi,
            miso,
            cs,
            _mode: core::marker::PhantomData,
        }
    }

    /// Return `Err(Overrun)` if the RX FIFO overflowed since the last check.
    fn check_overflow(&self) -> Result<(), Error> {
        if T::is_rx_overflow() {
            T::clear_overflow();
            Err(Error::Overrun)
        } else {
            Ok(())
        }
    }
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> SpiSlave<'d, T, Sck, Mosi, Miso, Cs, Blocking> {
    /// Build a blocking SPI slave. See [`SpiSlave::build`] for the pad wiring.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        sck: impl Peripheral<P = Sck> + 'd,
        sck_func: impl PinFuncOf<Sck>,
        mosi: impl Peripheral<P = Mosi> + 'd,
        mosi_func: impl PinFuncOf<Mosi>,
        miso: impl Peripheral<P = Miso> + 'd,
        miso_func: impl PinFuncOf<Miso>,
        cs: impl Peripheral<P = Cs> + 'd,
        cs_func: impl PinFuncOf<Cs>,
        config: Config,
    ) -> Self {
        Self::build(
            peri, sck, sck_func, mosi, mosi_func, miso, miso_func, cs, cs_func, config,
        )
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> SpiSlave<'d, T, Sck, Mosi, Miso, Cs, Async> {
    /// Build an async SPI slave. See [`SpiSlave::build`] for the pad wiring.
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        sck: impl Peripheral<P = Sck> + 'd,
        sck_func: impl PinFuncOf<Sck>,
        mosi: impl Peripheral<P = Mosi> + 'd,
        mosi_func: impl PinFuncOf<Mosi>,
        miso: impl Peripheral<P = Miso> + 'd,
        miso_func: impl PinFuncOf<Miso>,
        cs: impl Peripheral<P = Cs> + 'd,
        cs_func: impl PinFuncOf<Cs>,
        config: Config,
    ) -> Self {
        Self::build(
            peri, sck, sck_func, mosi, mosi_func, miso, miso_func, cs, cs_func, config,
        )
    }
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> SpiSlave<'d, T, Sck, Mosi, Miso, Cs, Blocking> {
    /// Wait until the master asserts CS, opening a transfer.
    pub fn blocking_wait_start(&mut self) -> Result<(), Error> {
        while !T::is_cs_fall() {}
        T::clear_cs_fall();
        Ok(())
    }

    /// Wait until the master deasserts CS, closing the current transfer.
    pub fn blocking_wait_end(&mut self) -> Result<(), Error> {
        while !T::is_cs_rise() {}
        T::clear_cs_rise();
        Ok(())
    }

    /// Wait for the master to clock in a byte and return it.
    pub fn blocking_read_byte(&mut self) -> Result<u8, Error> {
        while !T::is_rx_fifo_not_empty() {
            if T::is_cs_rise() {
                T::clear_cs_rise();
                return Err(Error::TransferEnded);
            }
        }
        Ok(T::read_data())
    }

    /// Wait for room in the TX FIFO and push `byte` to be shifted out.
    pub fn blocking_write_byte(&mut self, byte: u8) -> Result<(), Error> {
        while !T::is_tx_fifo_not_full() {}
        T::write_data(byte);
        Ok(())
    }

    /// Handle one whole CS-active transfer driven by the master.
    ///
    /// Every byte of `write` is shifted out (preloaded as space allows, then topped
    /// up as the master clocks); every byte the master clocks in is stored into
    /// `read` until CS rises. Bytes beyond `read.len()` are received and discarded
    /// (the RX FIFO must be drained or it overflows).
    pub fn blocking_transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        self.blocking_wait_start()?;
        let mut wpos = 0usize;
        let mut rpos = 0usize;
        loop {
            if wpos < write.len() && T::is_tx_fifo_not_full() {
                T::write_data(write[wpos]);
                wpos += 1;
            }
            if T::is_rx_fifo_not_empty() {
                let byte = T::read_data();
                if rpos < read.len() {
                    read[rpos] = byte;
                }
                rpos += 1;
            }
            if T::is_cs_rise() {
                break;
            }
        }
        T::clear_cs_rise();
        while T::is_rx_fifo_not_empty() {
            let _ = T::read_data();
        }
        self.check_overflow()
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> SpiSlave<'d, T, Sck, Mosi, Miso, Cs, Async> {
    /// Async version of [`SpiSlave::blocking_wait_start`].
    pub async fn wait_start(&mut self) -> Result<(), Error> {
        super::future::slave_wait_start::<T>().await
    }

    /// Async version of [`SpiSlave::blocking_wait_end`].
    pub async fn wait_end(&mut self) -> Result<(), Error> {
        super::future::slave_wait_end::<T>().await
    }

    /// Async version of [`SpiSlave::blocking_read_byte`].
    pub async fn read_byte(&mut self) -> Result<u8, Error> {
        super::future::slave_read_byte::<T>().await
    }

    /// Async version of [`SpiSlave::blocking_write_byte`].
    pub async fn write_byte(&mut self, byte: u8) -> Result<(), Error> {
        super::future::slave_write_byte::<T>(byte).await
    }

    /// Async version of [`SpiSlave::blocking_transfer`].
    pub async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Error> {
        super::future::slave_transfer::<T>(read, write).await
    }
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode> Drop
    for SpiSlave<'d, T, Sck, Mosi, Miso, Cs, M>
{
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
        crate::gpio::release::<Sck>();
        crate::gpio::release::<Mosi>();
        crate::gpio::release::<Miso>();
        crate::gpio::release::<Cs>();
    }
}
