//! Register access for the hardware divider (DIV).
//!
//! The SWM341 has a single integer divider / square-root unit shared by every
//! caller. It exposes dedicated registers for the operands and results:
//! `DIVIDEND`/`DIVISOR` feed the divider and `QUO`/`REMAIN` hold its result,
//! while `RADICAND`/`ROOT` drive the square-root side. A single `CR` starts each
//! operation (`DIVGO` / `ROOTGO`) and `SR` reports completion (`DIVBUSY` /
//! `ROOTBUSY`).
//!
//! The whole unit is powered by the `PeripheralClockIndex::Div` gate; there is no
//! per-register enable bit, so the driver only toggles that gate.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the divider.
pub(crate) type Block = pac::div::RegisterBlock;

/// Register access shared by the divider.
pub(crate) mod hw {
    use super::Block;

    /// Unsigned division: returns `(quotient, remainder)`.
    ///
    /// Writing the operands and pulsing `CR.DIVGO` starts the operation; the
    /// routine busy-waits on `SR.DIVBUSY` and then reads back `QUO`/`REMAIN`.
    pub(crate) fn divide(regs: &Block, dividend: u32, divisor: u32) -> (u32, u32) {
        regs.dividend.write(|w| unsafe { w.bits(dividend) });
        regs.divisor.write(|w| unsafe { w.bits(divisor) });
        regs.cr.modify(|_, w| w.divgo().set_bit());
        while regs.sr.read().divbusy().bit() {}
        let quo = regs.quo.read().bits();
        let rem = regs.remain.read().bits();
        (quo, rem)
    }

    /// Unsigned integer square root of `radicand`.
    ///
    /// `CR.ROOTGO` starts the operation and `SR.ROOTBUSY` clears when `ROOT`
    /// holds the result.
    pub(crate) fn sqrt(regs: &Block, radicand: u32) -> u32 {
        regs.radicand.write(|w| unsafe { w.bits(radicand) });
        regs.cr.modify(|_, w| w.rootgo().set_bit());
        while regs.sr.read().rootbusy().bit() {}
        regs.root.read().bits()
    }

    /// True while either the divider or the root unit is still computing.
    pub(crate) fn is_busy(regs: &Block) -> bool {
        regs.sr.read().divbusy().bit() || regs.sr.read().rootbusy().bit()
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// A divider peripheral the driver can talk to.
    ///
    /// Implemented for the singleton in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, the clock gate, the async index and
    /// the NVIC vector; every register operation is forwarded to [`super::hw`].
    pub trait Instance {
        /// Clock gate that powers the divider.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn divide(dividend: u32, divisor: u32) -> (u32, u32) {
            hw::divide(unsafe { &*Self::regs() }, dividend, divisor)
        }

        #[inline(always)]
        fn sqrt(radicand: u32) -> u32 {
            hw::sqrt(unsafe { &*Self::regs() }, radicand)
        }
    }
}

#[allow(private_bounds)]
/// A divider peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the divider singleton.
macro_rules! impl_div_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Div
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> impl InterruptNumber {
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

impl_div_instance!(crate::Div, DIV, DIV, 0);
