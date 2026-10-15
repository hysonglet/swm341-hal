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


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the divider.
pub(crate) type Block = pac::div::RegisterBlock;

pub(crate) mod sealed {

    use super::Block;

    /// A divider peripheral the driver can talk to.
    ///
    /// Implemented for the singleton in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, the clock gate, the async index and
    /// the NVIC vector; all register access lives in the default methods of this
    /// trait.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Unsigned division: returns `(quotient, remainder)`.
        ///
        /// Writing the operands and pulsing `CR.DIVGO` starts the operation; the
        /// routine busy-waits on `SR.DIVBUSY` and then reads back `QUO`/`REMAIN`.
        #[inline(always)]
        fn divide(dividend: u32, divisor: u32) -> (u32, u32) {
            let regs = unsafe { &*Self::regs() };
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
        #[inline(always)]
        fn sqrt(radicand: u32) -> u32 {
            let regs = unsafe { &*Self::regs() };
            regs.radicand.write(|w| unsafe { w.bits(radicand) });
            regs.cr.modify(|_, w| w.rootgo().set_bit());
            while regs.sr.read().rootbusy().bit() {}
            regs.root.read().bits()
        }

        /// True while either the divider or the root unit is still computing.
        #[inline(always)]
        fn is_busy() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().divbusy().bit() || regs.sr.read().rootbusy().bit()
        }
    }
}

#[allow(private_bounds)]
/// A divider peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the divider singleton.
macro_rules! impl_div_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        crate::impl_instance_base!($ty, crate::pac::div::RegisterBlock, crate::interrupt::$irq, PeripheralClockIndex::Div, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_div_instance!(crate::Div, DIV, DIV, 0);
