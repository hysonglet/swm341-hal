//! Register access for the watchdog timer (WDT).
//!
//! The watchdog is a single peripheral clocked from the (divided) system clock.
//! `RSTVAL` holds the counter reload value and `INTVAL` the threshold at which a
//! warning interrupt is raised before the counter reaches zero. `CR` arms the
//! unit, selects the prescaler (`CKDIV`) and enables the reset and/or interrupt
//! responses. A running watchdog is reloaded by writing the magic feed sequence
//! (`0x55` then `0xAA`) to `FEED`; writing `RSTVAL` reloads the counter too.
//!
//! All of `RSTVAL`, `INTVAL`, `IF` and `FEED` are modelled by the PAC as raw
//! words, so the driver addresses them through `bits()`.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the watchdog.
pub(crate) type Block = pac::wdt::RegisterBlock;

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};

    /// A watchdog peripheral the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers the watchdog.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Write the counter reload value (`RSTVAL`).
        #[inline(always)]
        fn set_reload(reload: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.rstval.write(|w| unsafe { w.bits(reload) });
        }

        /// Write the warning threshold (`INTVAL`).
        #[inline(always)]
        fn set_warning(warning: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.intval.write(|w| unsafe { w.bits(warning) });
        }

        /// Configure and arm the watchdog.
        #[inline(always)]
        fn configure(prescaler: u8, reset: bool, interrupt: bool, window: bool, enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| unsafe {
                w.en().bit(enable);
                w.rsten().bit(reset);
                w.inten().bit(interrupt);
                w.winen().bit(window);
                w.ckdiv().bits(prescaler & 0x0f)
            });
        }

        /// Disable the watchdog (`CR.EN = 0`).
        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().clear_bit());
        }

        /// Reload the counter by writing the feed sequence to `FEED`.
        #[inline(always)]
        fn feed() {
            let regs = unsafe { &*Self::regs() };
            regs.feed.write(|w| unsafe { w.bits(0x55) });
            regs.feed.write(|w| unsafe { w.bits(0xAA) });
        }

        /// True when the warning interrupt is pending (`IF` bit 0).
        #[inline(always)]
        fn interrupt_pending() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().bits() & 0x1 != 0
        }

        /// Clear the warning interrupt pending flag (`IF` bit 0).
        #[inline(always)]
        fn clear_interrupt() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| unsafe { w.bits(0x1) });
        }
    }
}

#[allow(private_bounds)]
/// A watchdog peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the watchdog singleton.
macro_rules! impl_wdt_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Wdt
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

impl_wdt_instance!(crate::Wdt, WDT, WDT, 0);
