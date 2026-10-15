//! Register access for the SPI blocks.
//!
//! Both SWM341 SPIs share the `crate::pac::spi0::RegisterBlock` type: the PAC aliases
//! `spi1` to `spi0`, and every `SPIx::PTR` is a `*const spi0::RegisterBlock`.
//! The C vendor driver exploits the same fact — `SPI_Init(SPI_TypeDef *SPIx, …)`
//! is one implementation operating on an instance pointer, differing only in
//! the clock gate and NVIC vector.
//!
//! We mirror that here. [`sealed::Instance`] carries only the per-instance data
//! — the register-block pointer, clock gate, async index and NVIC vector — while
//! the actual register twiddling lives once in the [`hw`] module as free
//! functions over `&crate::pac::spi0::RegisterBlock`. Each trait method is a one-line
//! forward, so the heavy instructions are emitted a single time instead of once
//! per SPI.

use crate::clock::PeripheralClockIndex;

/// The register-block type shared by every SPI instance on the SWM341.
///
/// The PAC already types `SPI0`/`SPI1::PTR` as `*const spi0::RegisterBlock`,
/// so no pointer cast is ever needed — the two singletons agree on layout.
pub(crate) type Block = crate::pac::spi0::RegisterBlock;

pub(crate) mod sealed {

    use super::Block;

    /// An SPI peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; all register access lives in the default methods of this trait.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        #[inline(always)]
        fn enable() {
            let regs = unsafe { &*Self::regs() };
            regs.ctrl.modify(|_, w| w.en().bit(true));
        }

        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.ctrl.modify(|_, w| w.en().bit(false));
        }

        #[inline(always)]
        fn set_clkdiv(clkdiv: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.ctrl
                .modify(|_, w| unsafe { w.clkdiv().bits(clkdiv.min(7)) });
        }

        #[inline(always)]
        fn configure_line(cpol: bool, cpha: bool, lsbf: bool, frame_size: u8) {
            let regs = unsafe { &*Self::regs() };
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

        #[inline(always)]
        fn write_data(byte: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.data.write(|w| unsafe { w.bits(byte as u32) });
        }

        /// Line configuration for slave mode: same as [`configure_line`] but the
        /// `MSTR` bit is left cleared so the block answers as a peripheral.
        #[inline(always)]
        fn configure_slave_line(cpol: bool, cpha: bool, lsbf: bool, frame_size: u8) {
            let regs = unsafe { &*Self::regs() };
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

        #[inline(always)]
        fn read_data() -> u8 {
            let regs = unsafe { &*Self::regs() };
            regs.data.read().bits() as u8
        }

        #[inline(always)]
        fn is_tx_fifo_not_full() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.stat.read().tfnf().bit()
        }

        #[inline(always)]
        fn is_rx_fifo_not_empty() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.stat.read().rfne().bit()
        }

        #[inline(always)]
        fn is_busy() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.stat.read().busy().bit()
        }

        #[inline(always)]
        fn is_rx_overflow() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.stat.read().rfov().bit()
        }

        #[inline(always)]
        fn clear_overflow() {
            let regs = unsafe { &*Self::regs() };
            regs.stat.modify(|_, w| w.rfov().clear_bit());
        }

        #[inline(always)]
        fn set_rx_threshold_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.rfthr().bit(enable));
        }

        // -- SPI slave (peripheral) mode --

        /// Put the block in slave mode: clear `MSTR`, keep `EN` and the line config.
        #[inline(always)]
        fn enter_slave() {
            let regs = unsafe { &*Self::regs() };
            regs.ctrl
                .modify(|_, w| w.en().set_bit().mstr().clear_bit());
        }

        /// `true` once the CS (SSN) line fell, marking the start of a transfer.
        #[inline(always)]
        fn is_cs_fall() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().ssfall().bit()
        }

        #[inline(always)]
        fn clear_cs_fall() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.ssfall().set_bit());
        }

        /// `true` once the CS (SSN) line rose, marking the end of a transfer.
        #[inline(always)]
        fn is_cs_rise() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().ssrise().bit()
        }

        #[inline(always)]
        fn clear_cs_rise() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.ssrise().set_bit());
        }

        #[inline(always)]
        fn set_cs_fall_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.ssfall().bit(enable));
        }

        #[inline(always)]
        fn set_cs_rise_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.ssrise().bit(enable));
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
        crate::impl_instance_base!($ty, crate::pac::spi0::RegisterBlock, crate::interrupt::$irq, PeripheralClockIndex::$clock, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
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
