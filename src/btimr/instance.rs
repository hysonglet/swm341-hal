//! Register access for the basic timers (BTIMR0..BTIMR11).
//!
//! Each basic timer shares the `pac::timr0::RegisterBlock` layout with the
//! general purpose [`crate::timr`] family: `LOAD` arms and starts a 32-bit
//! down-counter, `VALUE` is the live count, `CR` selects the clock source and
//! run mode, `HALT` freezes it, and `IE`/`IF` carry the overflow interrupt. The
//! only differences are that the basic timers are gated by the shared
//! `PeripheralClockIndex::Btimr` bit and carry their own NVIC vector each.
//!
//! All instances share one set of register-access default methods on
//! `sealed::Instance`; the per-instance macro only fills in `clock`, `index`,
//! `irq` and `regs`.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every basic timer instance on the SWM341.
pub(crate) type Block = pac::timr0::RegisterBlock;

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};

    /// A basic timer the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers this peripheral (shared across BTIMR0..BTIMR11).
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        /// Pointer to this instance's register block (always a `timr0::RegisterBlock`).
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

        /// Current count value.
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

        /// Clear the overflow flag (write-1-to-clear).
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
            fn irq() -> crate::interrupt::ExtendedInterrupt {
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
