//! Serial peripheral interface (SPI) in master mode.
//!
//! An [`Spi`] is generic over the four pad roles (`sck`, `mosi`, `miso`, `cs`),
//! selected with the type-checked [`crate::spi::pins`] values. The `cs` pad is
//! routed to the peripheral's hardware slave-select line, so it is driven by the
//! controller automatically.
//!
//! [`Spi`] implements `embedded_hal::spi::SpiBus` (and, with the `embassy` feature,
//! `embedded_hal_async::spi::SpiBus`), so it drops into any generic SPI driver.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::spi::pins;
//! let mut spi = hal::spi::Spi::new(
//!     p.spi0(),
//!     p.PB8,  pins::b::Pin8Func::Spi0Sclk,
//!     p.PB10, pins::b::Pin10Func::Spi0Mosi,
//!     p.PB9,  pins::b::Pin9Func::Spi0Miso,
//!     p.PB14, pins::b::Pin14Func::Spi0Ssel,
//!     hal::spi::Config::default(),
//! );
//! let mut rx = [0u8; 4];
//! spi.blocking_transfer(&mut rx, &[0x9f, 0x00, 0x00, 0x00]);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Pin, PinFuncOf};
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::{BitOrder, Config, Error, SpiMode};

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
use super::future;

/// An SPI master with its four pads.
#[allow(dead_code)]
pub struct Spi<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    sck: PeripheralRef<'d, Sck>,
    mosi: PeripheralRef<'d, Mosi>,
    miso: PeripheralRef<'d, Miso>,
    cs: PeripheralRef<'d, Cs>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode>
    Spi<'d, T, Sck, Mosi, Miso, Cs, M>
{
    /// Build an SPI master from its peripheral and the four pads.
    ///
    /// Each `x_func` picks the alternate function of its pad; only the
    /// [`crate::gpio::PinFuncOf`] value that belongs to that exact pad type
    /// checks, so a wrong pad is a compile error. This is the shared constructor
    /// used by [`Spi::new`] and [`Spi::new_async`].
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

        let clk = crate::clock::sys_core_clock();
        // SCK = sysclk / (2 * (clkdiv + 1)); pick the smallest divider that meets
        // the requested rate and clamp it into the 3-bit field.
        let clkdiv = ((clk + 2 * config.frequency.max(1) - 1) / (2 * config.frequency.max(1)))
            .saturating_sub(1)
            .min(7) as u8;

        let cpol = matches!(config.mode, SpiMode::Mode2 | SpiMode::Mode3);
        let cpha = matches!(config.mode, SpiMode::Mode1 | SpiMode::Mode3);

        T::set_clkdiv(clkdiv);
        T::configure_line(
            cpol,
            cpha,
            config.bit_order == BitOrder::LsbFirst,
            config.frame_size,
        );

        Self {
            peri,
            sck,
            mosi,
            miso,
            cs,
            _mode: core::marker::PhantomData,
        }
    }

    /// Recompute the clock divider for a new SCK frequency.
    pub fn set_frequency(&mut self, frequency: u32) {
        let clk = crate::clock::sys_core_clock();
        let clkdiv = ((clk + 2 * frequency.max(1) - 1) / (2 * frequency.max(1)))
            .saturating_sub(1)
            .min(7) as u8;
        T::set_clkdiv(clkdiv);
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

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin>
    Spi<'d, T, Sck, Mosi, Miso, Cs, Blocking>
{
    /// Build a blocking SPI master. See [`Spi::build`] for the pad wiring.
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
impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin>
    Spi<'d, T, Sck, Mosi, Miso, Cs, Async>
{
    /// Build an async SPI master. See [`Spi::build`] for the pad wiring.
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

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode>
    Spi<'d, T, Sck, Mosi, Miso, Cs, M>
{
    /// Full-duplex blocking transfer: each byte of `write` is shifted out while a
    /// byte is shifted in and stored into `read`. The two slices may differ in
    /// length; `read` is filled with whatever is received and the remainder of a
    /// longer `write` is padded with `0xFF` on the wire.
    pub fn blocking_transfer(&mut self, read: &mut [u8], write: &[u8]) {
        let total = write.len().max(read.len());
        for i in 0..total {
            while !T::is_tx_fifo_not_full() {}
            let out = *write.get(i).unwrap_or(&0xFF);
            T::write_data(out);
            while !T::is_rx_fifo_not_empty() {}
            let sample = T::read_data();
            if let Some(slot) = read.get_mut(i) {
                *slot = sample;
            }
        }
        while T::is_busy() {}
    }

    /// Blocking write, discarding the bytes shifted in.
    pub fn blocking_write(&mut self, bytes: &[u8]) {
        self.blocking_transfer(&mut [], bytes);
    }

    /// Blocking read, clocked by `0xFF` on the wire.
    pub fn blocking_read(&mut self, bytes: &mut [u8]) {
        self.blocking_transfer(bytes, &[]);
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin>
    Spi<'d, T, Sck, Mosi, Miso, Cs, Async>
{
    /// Full-duplex async transfer.
    pub async fn transfer(&mut self, read: &mut [u8], write: &[u8]) {
        future::transfer::<T>(read, write).await;
    }

    /// Async write, discarding the bytes shifted in.
    pub async fn write(&mut self, bytes: &[u8]) {
        future::transfer::<T>(&mut [], bytes).await;
    }

    /// Async read, clocked by `0xFF` on the wire.
    pub async fn read(&mut self, bytes: &mut [u8]) {
        future::transfer::<T>(bytes, &[]).await;
    }

    /// Async in-place full-duplex exchange.
    pub async fn transfer_in_place(&mut self, bytes: &mut [u8]) {
        future::transfer_in_place::<T>(bytes).await;
    }
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode> Drop
    for Spi<'d, T, Sck, Mosi, Miso, Cs, M>
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

impl<T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin, M: Mode> embedded_hal::spi::ErrorType
    for Spi<'_, T, Sck, Mosi, Miso, Cs, M>
{
    type Error = Error;
}

impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> embedded_hal::spi::SpiBus<u8>
    for Spi<'d, T, Sck, Mosi, Miso, Cs, Blocking>
{
    fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        self.blocking_transfer(words, &[]);
        self.check_overflow()
    }

    fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        self.blocking_transfer(&mut [], words);
        self.check_overflow()
    }

    fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        self.blocking_transfer(read, write);
        self.check_overflow()
    }

    fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        let total = words.len();
        for i in 0..total {
            while !T::is_tx_fifo_not_full() {}
            T::write_data(words[i]);
            while !T::is_rx_fifo_not_empty() {}
            words[i] = T::read_data();
        }
        while T::is_busy() {}
        self.check_overflow()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        while T::is_busy() {}
        self.check_overflow()
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Sck: Pin, Mosi: Pin, Miso: Pin, Cs: Pin> embedded_hal_async::spi::SpiBus<u8>
    for Spi<'d, T, Sck, Mosi, Miso, Cs, Async>
{
    async fn read(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        future::transfer::<T>(words, &[]).await;
        Ok(())
    }

    async fn write(&mut self, words: &[u8]) -> Result<(), Self::Error> {
        future::transfer::<T>(&mut [], words).await;
        Ok(())
    }

    async fn transfer(&mut self, read: &mut [u8], write: &[u8]) -> Result<(), Self::Error> {
        future::transfer::<T>(read, write).await;
        Ok(())
    }

    async fn transfer_in_place(&mut self, words: &mut [u8]) -> Result<(), Self::Error> {
        future::transfer_in_place::<T>(words).await;
        Ok(())
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
