//! Universal asynchronous receiver / transmitter.
//!
//! Both blocking (`blocking_write`/`blocking_read` and the single-byte
//! `blocking_write_byte`/`blocking_read_byte`) and, with the `embassy` feature,
//! interrupt driven `async` transfers (`write`/`read`/`read_bytes`) are
//! supported. The TX/RX pad functions are derived from the pin type and the
//! UART instance through the [`UartTxPin`]/[`UartRxPin`] traits, so the
//! constructor only needs the pins themselves:
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut uart = hal::uart::Uart::new(
//!     p.uart1(),
//!     p.PA0,
//!     p.PA2,
//!     hal::uart::Config::default(),
//! );
//! uart.blocking_write(b"hello\r\n");
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::Pin;
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

/// A pin that can drive the transmit line of UART instance `T`.
///
/// Implemented for every pad exposing a `UARTx_TX` function; the associated
/// [`FUNC`](UartTxPin::FUNC) is the 4-bit `PORTx.FUNC` code that routes the pad
/// to `T`'s TX path. [`Uart::new`] reads it straight from the pin type, so the
/// caller never names the function code by hand and a pad that cannot be `T`'s TX
/// is a compile error.
pub trait UartTxPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting this pad as `T`'s TX.
    const FUNC: u8;
}

/// A pin that can drive the receive line of UART instance `T`.
///
/// See [`UartTxPin`] for the rationale; this is the RX counterpart.
pub trait UartRxPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting this pad as `T`'s RX.
    const FUNC: u8;
}

impl<'d, T: Instance, Tx: Pin, Rx: Pin, M: Mode> Uart<'d, T, Tx, Rx, M> {
    /// Build a UART from its peripheral and the two pads.
    ///
    /// The TX/RX `PORTx.FUNC` codes are derived from `tx`/`rx` and the UART
    /// instance `T` via [`UartTxPin`]/[`UartRxPin`], so only the pins themselves
    /// are needed; a pad that cannot serve the requested role is a compile error.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        tx: impl Peripheral<P = Tx> + 'd,
        rx: impl Peripheral<P = Rx> + 'd,
        config: Config,
    ) -> Self
    where
        Tx: UartTxPin<T>,
        Rx: UartRxPin<T>,
    {
        into_ref!(peri, tx, rx);

        T::clock().open();
        crate::gpio::acquire::<Tx>();
        crate::gpio::acquire::<Rx>();
        crate::gpio::set_function::<Tx>(Tx::FUNC);
        crate::gpio::set_function::<Rx>(Rx::FUNC);

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

    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode so
    /// the `write`/`read`/`read_bytes` futures are available.
    #[cfg(feature = "embassy")]
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        tx: impl Peripheral<P = Tx> + 'd,
        rx: impl Peripheral<P = Rx> + 'd,
        config: Config,
    ) -> Self
    where
        Tx: UartTxPin<T>,
        Rx: UartRxPin<T>,
    {
        into_ref!(peri, tx, rx);

        T::clock().open();
        crate::gpio::acquire::<Tx>();
        crate::gpio::acquire::<Rx>();
        crate::gpio::set_function::<Tx>(Tx::FUNC);
        crate::gpio::set_function::<Rx>(Rx::FUNC);

        T::set_baudrate(config.baudrate);
        T::configure_line(
            config.data_bits == DataBits::Data9,
            config.parity.code(),
            config.stop_bits == StopBits::Stop2,
        );
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
