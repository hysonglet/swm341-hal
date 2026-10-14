//! Register access for the UART blocks.
//!
//! All four UARTs share the `pac::uart0::RegisterBlock` type: the PAC aliases
//! `uart1`/`uart2`/`uart3` to `uart0`, and every `UARTx::PTR` is a
//! `*const uart0::RegisterBlock`. The C vendor driver exploits the same fact —
//! `UART_Init(UART_TypeDef *UARTx, …)` is a single implementation that operates
//! on an instance pointer, with only the clock gate and NVIC vector differing
//! per instance (handled there by a `switch ((uint32_t)UARTx)`).
//!
//! We mirror that here. [`sealed::Instance`] carries only the per-instance data
//! — the register-block pointer, clock gate, async index and NVIC vector — while
//! the actual register twiddling lives once in the [`hw`] module as free
//! functions over `&pac::uart0::RegisterBlock`. Each trait method is a one-line
//! forward, so the heavy instructions are emitted a single time instead of once
//! per UART.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every UART instance on the SWM341.
///
/// The PAC already types `UART0..UART3::PTR` as `*const uart0::RegisterBlock`,
/// so no pointer cast is ever needed — the four singletons agree on layout.
pub(crate) type Block = pac::uart0::RegisterBlock;

/// Register access shared by every UART instance.
///
/// Each function is non-generic (it takes the common [`Block`] type), so the
/// compiler emits its body exactly once regardless of how many UARTs exist.
pub(crate) mod hw {
    use super::Block;

    pub(crate) fn enable(regs: &Block) {
        regs.ctrl.modify(|_, w| w.en().bit(true));
    }

    pub(crate) fn disable(regs: &Block) {
        regs.ctrl.modify(|_, w| w.en().bit(false));
    }

    pub(crate) fn configure_line(regs: &Block, data9: bool, parity_code: u8, stop2: bool) {
        regs.ctrl.modify(|_, w| unsafe {
            w.data9b().bit(data9);
            w.parity().bits(parity_code);
            w.stop2b().bits(stop2 as u8);
            w.en().bit(true)
        });
    }

    pub(crate) fn set_baudrate(regs: &Block, baudrate: u32) {
        let clk =
            crate::clock::current_system_clock().expect("peripheral clock not running") as u32;
        // BAUD = (clk / rate - 1) / 16, FRAC = (clk / rate - 1) % 16, so the
        // realised rate is clk / ((BAUD * 16) + FRAC + 1).
        let step = (clk / baudrate.max(1)).saturating_sub(1);
        regs.baud.write(|w| unsafe {
            w.baud().bits((step / 16) as u16);
            w.frac().bits((step % 16) as u8)
        });
    }

    pub(crate) fn configure_fifo(regs: &Block, rx_thr: u8, tx_thr: u8) {
        regs.fifo
            .modify(|_, w| unsafe { w.rxthr().bits(rx_thr).txthr().bits(tx_thr) });
        // Receiver timeout resets the line-idle accounting after each byte.
        regs.tocr.modify(|_, w| w.mode().bit(true));
    }

    pub(crate) fn write_data(regs: &Block, byte: u8) {
        regs.data.write(|w| unsafe { w.bits(byte as u32) });
    }

    pub(crate) fn read_data(regs: &Block) -> (u8, bool) {
        let r = regs.data.read();
        (r.data().bits() as u8, r.paerr().bit())
    }

    pub(crate) fn is_tx_fifo_full(regs: &Block) -> bool {
        regs.ctrl.read().txff().bit()
    }

    pub(crate) fn is_rx_not_empty(regs: &Block) -> bool {
        regs.ctrl.read().rxne().bit()
    }

    pub(crate) fn is_tx_idle(regs: &Block) -> bool {
        regs.ctrl.read().txidle().bit()
    }

    pub(crate) fn set_rxie(regs: &Block, enable: bool) {
        regs.ctrl.modify(|_, w| w.rxie().bit(enable));
    }

    pub(crate) fn set_txie(regs: &Block, enable: bool) {
        regs.ctrl.modify(|_, w| w.txie().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// A UART peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; every register operation is forwarded to [`super::hw`], which
    /// holds a single shared implementation.
    pub trait Instance {
        /// Clock gate that powers this peripheral.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> pac::Interrupt;
        /// Pointer to this instance's register block (always a `uart0::RegisterBlock`).
        fn regs() -> *const Block;

        // -- thin forwards to the shared implementation in `hw` --
        // Marked `#[inline(always)]` so they collapse into the single `hw`
        // body at every call site rather than duplicating it per UART.

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn enable() {
            hw::enable(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn configure_line(data9: bool, parity_code: u8, stop2: bool) {
            hw::configure_line(unsafe { &*Self::regs() }, data9, parity_code, stop2);
        }

        #[inline(always)]
        fn set_baudrate(baudrate: u32) {
            hw::set_baudrate(unsafe { &*Self::regs() }, baudrate);
        }

        #[inline(always)]
        fn configure_fifo(rx_thr: u8, tx_thr: u8) {
            hw::configure_fifo(unsafe { &*Self::regs() }, rx_thr, tx_thr);
        }

        #[inline(always)]
        fn write_data(byte: u8) {
            hw::write_data(unsafe { &*Self::regs() }, byte);
        }

        #[inline(always)]
        fn read_data() -> (u8, bool) {
            hw::read_data(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_tx_fifo_full() -> bool {
            hw::is_tx_fifo_full(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_rx_not_empty() -> bool {
            hw::is_rx_not_empty(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_tx_idle() -> bool {
            hw::is_tx_idle(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn set_rxie(enable: bool) {
            hw::set_rxie(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn set_txie(enable: bool) {
            hw::set_txie(unsafe { &*Self::regs() }, enable);
        }
    }
}

#[allow(private_bounds)]
/// A UART peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a UART singleton.
///
/// Only the four values that actually differ between UARTs are supplied; the
/// (large) register-access logic comes from the default methods of
/// [`sealed::Instance`], which forward to the single [`hw`] implementation.
macro_rules! impl_uart_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::$clock
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> pac::Interrupt {
                crate::interrupt::$irq
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_uart_instance!(crate::Uart0, UART0, UART0, Uart0, 0);
impl_uart_instance!(crate::Uart1, UART1, UART1, Uart1, 1);
impl_uart_instance!(crate::Uart2, UART2, UART2, Uart2, 2);
impl_uart_instance!(crate::Uart3, UART3, UART3, Uart3, 3);

/// Number of UART instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 4;
