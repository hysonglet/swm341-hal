//! Universal asynchronous receiver / transmitter.
//!
//! Both blocking (`write`/`read`) and, with the `embassy` feature, interrupt
//! driven `async` transfers are supported. The driver is generic over the pin
//! roles, which are selected with the type-checked [`crate::gpio::PinFunc`] values
//! from [`crate::uart::pins`]:
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::uart::pins;
//! let mut uart = hal::uart::Uart::new(
//!     p.UART1,
//!     p.PA0,  pins::a::Pin0Func::Uart1Tx,
//!     p.PA2,  pins::a::Pin2Func::Uart1Rx,
//!     hal::uart::Config::default(),
//! );
//! uart.blocking_write(b"hello\r\n");
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Pin, PinFuncOf};
use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod rx;
mod tx;

mod types;
pub use types::*;

/// A UART with its transmit and receive pads.
#[allow(dead_code)]
pub struct Uart<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    tx: PeripheralRef<'d, Tx>,
    rx: PeripheralRef<'d, Rx>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode> Uart<'d, T, Tx, Rx, M> {
    /// Build a UART from its peripheral and the two pads.
    ///
    /// `tx_func` / `rx_func` pick the alternate function of each pad; only the
    /// [`crate::gpio::PinFuncOf`] value that belongs to that exact pad type checks,
    /// so a wrong pad is a compile error.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        tx: impl Peripheral<P = Tx> + 'd,
        tx_func: impl PinFuncOf<Tx>,
        rx: impl Peripheral<P = Rx> + 'd,
        rx_func: impl PinFuncOf<Rx>,
        config: Config,
    ) -> Self {
        into_ref!(peri, tx, rx);

        T::clock().open();
        crate::gpio::acquire::<Tx>();
        crate::gpio::acquire::<Rx>();
        crate::gpio::set_function::<Tx>(tx_func.value());
        crate::gpio::set_function::<Rx>(rx_func.value());

        T::set_baudrate(config.baudrate);
        T::configure_line(
            config.data_bits == DataBits::Data9,
            config.parity.code(),
            config.stop_bits == StopBits::Stop2,
        );
        // RX threshold of 0 fires as soon as a byte is present; TX threshold of 7
        // lets the async writer refill once the FIFO has drained to 9 free slots.
        T::configure_fifo(0, 7);

        Self {
            peri,
            tx,
            rx,
            _mode: core::marker::PhantomData,
        }
    }

    /// Recompute and apply the baud rate divisor.
    pub fn set_baudrate(&mut self, baudrate: u32) {
        T::set_baudrate(baudrate);
    }

    /// True while the transmitter shift register is still draining.
    pub fn is_tx_idle(&self) -> bool {
        T::is_tx_idle()
    }
}

impl<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode> Drop for Uart<'d, T, Tx, Rx, M> {
    fn drop(&mut self) {
        // Bring the peripheral back to reset and release the pad clocks.
        T::disable();
        T::clock().close();
        crate::gpio::release::<Tx>();
        crate::gpio::release::<Rx>();
    }
}
