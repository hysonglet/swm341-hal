//! Register access for the basic timers (BTIMR0..BTIMR11).
//!
//! Each basic timer shares the `pac::timr0::RegisterBlock` layout with the
//! general purpose [`crate::timr`] family: `LOAD` arms and starts a 32-bit
//! down-counter, `VALUE` is the live count, `CR` selects the clock source and
//! run mode, `HALT` freezes it, and `IE`/`IF` carry the overflow interrupt. The
//! only differences are that the basic timers are gated by the shared
//! `PeripheralClockIndex::Btimr` bit and carry their own NVIC vector each.
//!
//! A single non-generic [`hw`] implementation serves every instance.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every basic timer instance on the SWM341.
pub(crate) type Block = pac::timr0::RegisterBlock;

/// Register access shared by every basic timer instance.
pub(crate) mod hw {
    use super::Block;

    /// Arm and start the down-counter from `period` with prescaler `prediv`.
    pub(crate) fn start(regs: &Block, period: u32, prediv: u32, clksrc: u8, mode: u8) {
        regs.prediv.write(|w| unsafe { w.bits(prediv) });
        regs.load.write(|w| unsafe { w.bits(period) });
        regs.cr.write(|w| unsafe {
            w.clksrc().bits(clksrc as u8);
            w.mode().bits(mode as u8)
        });
    }

    /// Current count value.
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

    /// Clear the overflow flag (write-1-to-clear).
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

    /// A basic timer the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers this peripheral (shared across BTIMR0..BTIMR11).
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
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
/// A basic timer usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a basic timer singleton.
macro_rules! impl_btimr_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Btimr
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

impl_btimr_instance!(crate::Btimr0, BTIMR0, BTIMR0, 0);
impl_btimr_instance!(crate::Btimr1, BTIMR1, BTIMR1, 1);
impl_btimr_instance!(crate::Btimr2, BTIMR2, BTIMR2, 2);
impl_btimr_instance!(crate::Btimr3, BTIMR3, BTIMR3, 3);
impl_btimr_instance!(crate::Btimr4, BTIMR4, BTIMR4, 4);
impl_btimr_instance!(crate::Btimr5, BTIMR5, BTIMR5, 5);
impl_btimr_instance!(crate::Btimr6, BTIMR6, BTIMR6, 6);
impl_btimr_instance!(crate::Btimr7, BTIMR7, BTIMR7, 7);
impl_btimr_instance!(crate::Btimr8, BTIMR8, BTIMR8, 8);
impl_btimr_instance!(crate::Btimr9, BTIMR9, BTIMR9, 9);
impl_btimr_instance!(crate::Btimr10, BTIMR10, BTIMR10, 10);
impl_btimr_instance!(crate::Btimr11, BTIMR11, BTIMR11, 11);

/// Number of basic timer instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 12;
