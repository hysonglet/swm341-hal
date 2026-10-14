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

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the watchdog.
pub(crate) type Block = pac::wdt::RegisterBlock;

/// Register access shared by the watchdog.
pub(crate) mod hw {
    use super::Block;

    /// Write the counter reload value (`RSTVAL`).
    pub(crate) fn set_reload(regs: &Block, reload: u32) {
        regs.rstval.write(|w| unsafe { w.bits(reload) });
    }

    /// Write the warning threshold (`INTVAL`).
    pub(crate) fn set_warning(regs: &Block, warning: u32) {
        regs.intval.write(|w| unsafe { w.bits(warning) });
    }

    /// Configure and arm the watchdog.
    ///
    /// `prescaler` divides the watchdog clock by `2^ckdiv`; `reset` and
    /// `interrupt` enable the two responses, `window` turns on the windowed
    /// mode. Writing `CR` with `EN` set starts the counter.
    pub(crate) fn configure(
        regs: &Block,
        prescaler: u8,
        reset: bool,
        interrupt: bool,
        window: bool,
        enable: bool,
    ) {
        regs.cr.modify(|_, w| unsafe {
            w.en().bit(enable);
            w.rsten().bit(reset);
            w.inten().bit(interrupt);
            w.winen().bit(window);
            w.ckdiv().bits(prescaler & 0x0f)
        });
    }

    /// Disable the watchdog (`CR.EN = 0`).
    pub(crate) fn disable(regs: &Block) {
        regs.cr.modify(|_, w| w.en().clear_bit());
    }

    /// Reload the counter by writing the feed sequence to `FEED`.
    pub(crate) fn feed(regs: &Block) {
        regs.feed.write(|w| unsafe { w.bits(0x55) });
        regs.feed.write(|w| unsafe { w.bits(0xAA) });
    }

    /// True when the warning interrupt is pending (`IF` bit 0).
    pub(crate) fn interrupt_pending(regs: &Block) -> bool {
        regs.if_.read().bits() & 0x1 != 0
    }

    /// Clear the warning interrupt pending flag (`IF` bit 0).
    pub(crate) fn clear_interrupt(regs: &Block) {
        regs.if_.write(|w| unsafe { w.bits(0x1) });
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// A watchdog peripheral the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers the watchdog.
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
        fn set_reload(reload: u32) {
            hw::set_reload(unsafe { &*Self::regs() }, reload)
        }

        #[inline(always)]
        fn set_warning(warning: u32) {
            hw::set_warning(unsafe { &*Self::regs() }, warning)
        }

        #[inline(always)]
        fn configure(prescaler: u8, reset: bool, interrupt: bool, window: bool, enable: bool) {
            hw::configure(
                unsafe { &*Self::regs() },
                prescaler,
                reset,
                interrupt,
                window,
                enable,
            )
        }

        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn feed() {
            hw::feed(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn interrupt_pending() -> bool {
            hw::interrupt_pending(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_interrupt() {
            hw::clear_interrupt(unsafe { &*Self::regs() })
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

impl_wdt_instance!(crate::Wdt, WDT, WDT, 0);
