//! Register access for the TIMR blocks.
//!
//! All five TIMR instances (`TIMR0..TIMR4`) share the `pac::timr0::RegisterBlock`
//! type -- `TIMR1..TIMR4` are `pub use timr0 as timrN` aliases in the PAC -- so a
//! single non-generic [`hw`] implementation serves every instance. The only
//! per-instance data are the register-block pointer, the (shared) clock gate, the
//! async index and the NVIC vector, forwarded by [`sealed::Instance`].
//!
//! A TIMR is a 32-bit down-counter: writing `LOAD` arms and starts it, `VALUE` is
//! the live count, and `CR` only selects the clock source and run mode. `HALT`
//! freezes the counter. There is no per-instance enable bit; opening the `Timr`
//! clock gate is what powers the whole family.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every TIMR instance on the SWM341.
pub(crate) type Block = pac::timr0::RegisterBlock;

/// Register access shared by every TIMR instance.
pub(crate) mod hw {
    use super::Block;

    /// Arm and start the down-counter from `period` with prescaler `prediv`.
    ///
    /// `CLKSRC` selects the clock feeding the timer and `MODE` its run mode; both
    /// are written verbatim to `CR` and default to the internal clock / free-run
    /// in [`crate::timr::Config`].
    pub(crate) fn start(regs: &Block, period: u32, prediv: u32, clksrc: u8, mode: u8) {
        regs.prediv.write(|w| unsafe { w.bits(prediv) });
        regs.load.write(|w| unsafe { w.bits(period) });
        regs.cr.write(|w| unsafe {
            w.clksrc().bits(clksrc as u8);
            w.mode().bits(mode as u8)
        });
    }

    /// Current count value (read-only `VALUE`).
    pub(crate) fn value(regs: &Block) -> u32 {
        regs.value.read().bits()
    }

    /// Freeze the counter; the next `start` re-arms it.
    pub(crate) fn stop(regs: &Block) {
        regs.halt.write(|w| unsafe { w.bits(1) });
    }

    /// True once the counter has underflowed since the last clear.
    pub(crate) fn overflow_flag(regs: &Block) -> bool {
        regs.if_.read().to().bit()
    }

    /// Clear the overflow flag (the `IF` flags are write-1-to-clear).
    pub(crate) fn clear_overflow(regs: &Block) {
        regs.if_.write(|w| w.to().set_bit());
    }

    /// Enable or disable the overflow interrupt.
    pub(crate) fn set_overflow_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.to().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// A TIMR peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; every register operation is forwarded to [`super::hw`].
    pub trait Instance {
        /// Clock gate that powers this peripheral (shared across TIMR0..TIMR4).
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral (TIMR4 is an `ExtendedInterrupt`).
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block (always a `timr0::RegisterBlock`).
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn start(period: u32, prediv: u32, clksrc: u8, mode: u8) {
            hw::start(unsafe { &*Self::regs() }, period, prediv, clksrc, mode);
        }

        #[inline(always)]
        fn value() -> u32 {
            hw::value(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn stop() {
            hw::stop(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn overflow_flag() -> bool {
            hw::overflow_flag(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_overflow() {
            hw::clear_overflow(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_overflow_irq(enable: bool) {
            hw::set_overflow_irq(unsafe { &*Self::regs() }, enable);
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
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Timr
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> impl InterruptNumber {
                $irq
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_timr_instance!(crate::Timr0, TIMR0, crate::interrupt::TIMR0, 0);
impl_timr_instance!(crate::Timr1, TIMR1, crate::interrupt::TIMR1, 1);
impl_timr_instance!(crate::Timr2, TIMR2, crate::interrupt::TIMR2, 2);
impl_timr_instance!(crate::Timr3, TIMR3, crate::interrupt::TIMR3, 3);
// TIMR4 is an `ExtendedInterrupt` (NVIC 79); `irq()` still returns a value that
// implements `InterruptNumber`, so the trait signature is unchanged.
impl_timr_instance!(crate::Timr4, TIMR4, crate::interrupt::TIMR4, 4);

/// Number of TIMR instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 5;
