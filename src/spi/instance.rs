//! Register access for the SPI blocks.
//!
//! Both SWM341 SPIs share the `pac::spi0::RegisterBlock` type: the PAC aliases
//! `spi1` to `spi0`, and every `SPIx::PTR` is a `*const spi0::RegisterBlock`.
//! The C vendor driver exploits the same fact — `SPI_Init(SPI_TypeDef *SPIx, …)`
//! is one implementation operating on an instance pointer, differing only in
//! the clock gate and NVIC vector.
//!
//! We mirror that here. [`sealed::Instance`] carries only the per-instance data
//! — the register-block pointer, clock gate, async index and NVIC vector — while
//! the actual register twiddling lives once in the [`hw`] module as free
//! functions over `&pac::spi0::RegisterBlock`. Each trait method is a one-line
//! forward, so the heavy instructions are emitted a single time instead of once
//! per SPI.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every SPI instance on the SWM341.
///
/// The PAC already types `SPI0`/`SPI1::PTR` as `*const spi0::RegisterBlock`,
/// so no pointer cast is ever needed — the two singletons agree on layout.
pub(crate) type Block = pac::spi0::RegisterBlock;

/// Register access shared by every SPI instance.
///
/// Each function is non-generic (it takes the common [`Block`] type), so the
/// compiler emits its body exactly once regardless of how many SPIs exist.
pub(crate) mod hw {
    use super::Block;

    pub(crate) fn enable(regs: &Block) {
        regs.ctrl.modify(|_, w| w.en().bit(true));
    }

    pub(crate) fn disable(regs: &Block) {
        regs.ctrl.modify(|_, w| w.en().bit(false));
    }

    pub(crate) fn set_clkdiv(regs: &Block, clkdiv: u8) {
        regs.ctrl
            .modify(|_, w| unsafe { w.clkdiv().bits(clkdiv.min(7)) });
    }

    pub(crate) fn configure_line(regs: &Block, cpol: bool, cpha: bool, lsbf: bool, frame_size: u8) {
        regs.ctrl.modify(|_, w| unsafe {
            w.cpol().bit(cpol);
            w.cpha().bit(cpha);
            w.mstr().bit(true);
            w.size().bits(frame_size.max(4).min(16) - 1);
            w.lsbf().bit(lsbf);
            w.ffs().bits(0); // Motorola SPI frame format.
            w.filte().bit(false);
            w.ssn_h().bit(false); // Keep SSN low for the whole transfer.
            w.fast().bit(false);
            w.rfthr().bits(0); // Wake as soon as a byte is received.
            w.en().bit(true)
        });
    }

    pub(crate) fn write_data(regs: &Block, byte: u8) {
        regs.data.write(|w| unsafe { w.bits(byte as u32) });
    }

    /// Line configuration for slave mode: same as [`configure_line`] but the
    /// `MSTR` bit is left cleared so the block answers as a peripheral.
    pub(crate) fn configure_slave_line(
        regs: &Block,
        cpol: bool,
        cpha: bool,
        lsbf: bool,
        frame_size: u8,
    ) {
        regs.ctrl.modify(|_, w| unsafe {
            w.cpol().bit(cpol);
            w.cpha().bit(cpha);
            w.size().bits(frame_size.max(4).min(16) - 1);
            w.lsbf().bit(lsbf);
            w.ffs().bits(0); // Motorola SPI frame format.
            w.filte().bit(false);
            w.ssn_h().bit(false); // Driven by the master's CS line.
            w.fast().bit(false);
            w.rfthr().bits(0); // Wake as soon as a byte is received.
            w.en().bit(true)
        });
    }

    pub(crate) fn read_data(regs: &Block) -> u8 {
        regs.data.read().bits() as u8
    }

    pub(crate) fn is_tx_fifo_not_full(regs: &Block) -> bool {
        regs.stat.read().tfnf().bit()
    }

    pub(crate) fn is_rx_fifo_not_empty(regs: &Block) -> bool {
        regs.stat.read().rfne().bit()
    }

    pub(crate) fn is_busy(regs: &Block) -> bool {
        regs.stat.read().busy().bit()
    }

    pub(crate) fn is_rx_overflow(regs: &Block) -> bool {
        regs.stat.read().rfov().bit()
    }

    pub(crate) fn clear_overflow(regs: &Block) {
        regs.stat.modify(|_, w| w.rfov().clear_bit());
    }

    pub(crate) fn set_rx_threshold_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.rfthr().bit(enable));
    }

    // -- SPI slave (peripheral) mode --

    /// Put the block in slave mode: clear `MSTR`, keep `EN` and the line config.
    pub(crate) fn enter_slave(regs: &Block) {
        regs.ctrl
            .modify(|_, w| w.en().set_bit().mstr().clear_bit());
    }

    /// `true` once the CS (SSN) line fell, marking the start of a transfer.
    pub(crate) fn is_cs_fall(regs: &Block) -> bool {
        regs.if_.read().ssfall().bit()
    }

    pub(crate) fn clear_cs_fall(regs: &Block) {
        regs.if_.write(|w| w.ssfall().set_bit());
    }

    /// `true` once the CS (SSN) line rose, marking the end of a transfer.
    pub(crate) fn is_cs_rise(regs: &Block) -> bool {
        regs.if_.read().ssrise().bit()
    }

    pub(crate) fn clear_cs_rise(regs: &Block) {
        regs.if_.write(|w| w.ssrise().set_bit());
    }

    pub(crate) fn set_cs_fall_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.ssfall().bit(enable));
    }

    pub(crate) fn set_cs_rise_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.ssrise().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// An SPI peripheral the driver can talk to.
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
        /// Pointer to this instance's register block (always a `spi0::RegisterBlock`).
        fn regs() -> *const Block;

        // -- thin forwards to the shared implementation in `hw` --
        // Marked `#[inline(always)]` so they collapse into the single `hw`
        // body at every call site rather than duplicating it per SPI.

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
        fn set_clkdiv(clkdiv: u8) {
            hw::set_clkdiv(unsafe { &*Self::regs() }, clkdiv);
        }

        #[inline(always)]
        fn configure_line(cpol: bool, cpha: bool, lsbf: bool, frame_size: u8) {
            hw::configure_line(unsafe { &*Self::regs() }, cpol, cpha, lsbf, frame_size);
        }

        #[inline(always)]
        fn write_data(byte: u8) {
            hw::write_data(unsafe { &*Self::regs() }, byte);
        }

        #[inline(always)]
        fn configure_slave_line(cpol: bool, cpha: bool, lsbf: bool, frame_size: u8) {
            hw::configure_slave_line(unsafe { &*Self::regs() }, cpol, cpha, lsbf, frame_size);
        }

        #[inline(always)]
        fn read_data() -> u8 {
            hw::read_data(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_tx_fifo_not_full() -> bool {
            hw::is_tx_fifo_not_full(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_rx_fifo_not_empty() -> bool {
            hw::is_rx_fifo_not_empty(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_busy() -> bool {
            hw::is_busy(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_rx_overflow() -> bool {
            hw::is_rx_overflow(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_overflow() {
            hw::clear_overflow(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_rx_threshold_irq(enable: bool) {
            hw::set_rx_threshold_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn enter_slave() {
            hw::enter_slave(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn is_cs_fall() -> bool {
            hw::is_cs_fall(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_cs_fall() {
            hw::clear_cs_fall(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn is_cs_rise() -> bool {
            hw::is_cs_rise(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_cs_rise() {
            hw::clear_cs_rise(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_cs_fall_irq(enable: bool) {
            hw::set_cs_fall_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn set_cs_rise_irq(enable: bool) {
            hw::set_cs_rise_irq(unsafe { &*Self::regs() }, enable);
        }
    }
}

#[allow(private_bounds)]
/// An SPI peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for an SPI singleton.
///
/// Only the four values that actually differ between SPIs are supplied; the
/// (large) register-access logic comes from the default methods of
/// [`sealed::Instance`], which forward to the single [`hw`] implementation.
macro_rules! impl_spi_instance {
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

impl_spi_instance!(crate::Spi0, SPI0, SPI0, Spi0, 0);
impl_spi_instance!(crate::Spi1, SPI1, SPI1, Spi1, 1);

/// Number of SPI instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 2;
