//! Register access for the TIMR blocks.
//!
//! All five TIMR instances (`TIMR0..TIMR4`) share the `pac::timr0::RegisterBlock`
//! type -- `TIMR1..TIMR4` are `pub use timr0 as timrN` aliases in the PAC -- so a
//! single set of register-access default methods on `sealed::Instance` serves
//! every instance. The only
//! per-instance data are the register-block pointer, the (shared) clock gate, the
//! async index and the NVIC vector, forwarded by [`sealed::Instance`].
//!
//! A TIMR is a 32-bit down-counter: writing `LOAD` arms and starts it, `VALUE` is
//! the live count, and `CR` only selects the clock source and run mode. `HALT`
//! freezes the counter. There is no per-instance enable bit; opening the `Timr`
//! clock gate is what powers the whole family.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every TIMR instance on the SWM341.
pub(crate) type Block = pac::timr0::RegisterBlock;

pub(crate) mod sealed {

    use super::Block;

    /// A TIMR peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; all register access lives in the default methods of this trait.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Arm and start the down-counter from `period` with prescaler `prediv`.
        #[inline(always)]
        fn start(period: u32, prediv: u32, clksrc: u8, mode: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.prediv.write(|w| unsafe { w.bits(prediv) });
            regs.load.write(|w| unsafe { w.bits(period) });
            regs.cr.write(|w| unsafe {
                w.clksrc().bits(clksrc as u8);
                w.mode().bits(mode as u8)
            });
        }

        /// Current count value (read-only `VALUE`).
        #[inline(always)]
        fn value() -> u32 {
            let regs = unsafe { &*Self::regs() };
            regs.value.read().bits()
        }

        /// Freeze the counter; the next `start` re-arms it.
        #[inline(always)]
        fn stop() {
            let regs = unsafe { &*Self::regs() };
            regs.halt.write(|w| unsafe { w.bits(1) });
        }

        /// True once the counter has underflowed since the last clear.
        #[inline(always)]
        fn overflow_flag() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().to().bit()
        }

        /// Clear the overflow flag (the `IF` flags are write-1-to-clear).
        #[inline(always)]
        fn clear_overflow() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.to().set_bit());
        }

        /// Enable or disable the overflow interrupt.
        #[inline(always)]
        fn set_overflow_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.to().bit(enable));
        }
    }
}

#[allow(private_bounds)]
/// A TIMR peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a TIMR singleton.
///
/// Only the values that actually differ between instances are supplied; the
/// register-access logic comes from the default methods of [`sealed::Instance`].
macro_rules! impl_timr_instance {
    ($ty:ty, $pac:ident, $irq:expr, $index:literal) => {
        crate::impl_instance_base!($ty, crate::pac::timr0::RegisterBlock, $irq, PeripheralClockIndex::Timr, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_timr_instance!(crate::Timr0, TIMR0, crate::interrupt::TIMR0, 0);
impl_timr_instance!(crate::Timr1, TIMR1, crate::interrupt::TIMR1, 1);
impl_timr_instance!(crate::Timr2, TIMR2, crate::interrupt::TIMR2, 2);
impl_timr_instance!(crate::Timr3, TIMR3, crate::interrupt::TIMR3, 3);
// TIMR4 is an `ExtendedInterrupt` (NVIC 79); like every other instance, `irq()`
// now returns `crate::interrupt::ExtendedInterrupt` directly.
impl_timr_instance!(crate::Timr4, TIMR4, crate::interrupt::TIMR4, 4);

/// Number of TIMR instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 5;
