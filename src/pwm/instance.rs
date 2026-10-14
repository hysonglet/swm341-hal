//! Register access for the PWM blocks.
//!
//! All five PWM instances (`PWM0..PWM4`) share the `pac::pwm0::RegisterBlock`
//! type -- `PWM1..PWM4` are `pub use pwm0 as pwmN` aliases in the PAC -- so a
//! single non-generic [`hw`] implementation serves every instance. The only
//! per-instance data are the register-block pointer, the (shared) clock gate, the
//! async index and the NVIC vector, forwarded by [`sealed::Instance`].
//!
//! A PWM channel compares its free-running counter against `CMPA`/`CMPB` to drive
//! the `A`/`B` outputs; `PERIOD` sets the top of the count. `CR` only selects the
//! clock and run mode -- there is no per-instance enable bit, so opening the
//! `Pwm` clock gate is what powers the family.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every PWM instance on the SWM341.
pub(crate) type Block = pac::pwm0::RegisterBlock;

/// Register access shared by every PWM instance.
pub(crate) mod hw {
    use super::Block;

    /// Set the counter top (`PERIOD`).
    pub(crate) fn set_period(regs: &Block, period: u32) {
        regs.period.write(|w| unsafe { w.bits(period) });
    }

    /// Set the `A` channel compare value (`CMPA`, the duty cycle).
    pub(crate) fn set_duty_a(regs: &Block, duty: u32) {
        regs.cmpa.write(|w| unsafe { w.bits(duty) });
    }

    /// Set the `B` channel compare value (`CMPB`, the duty cycle).
    pub(crate) fn set_duty_b(regs: &Block, duty: u32) {
        regs.cmpb.write(|w| unsafe { w.bits(duty) });
    }

    /// Select the clock source and divider in `CR` (run mode is applied by
    /// [`start`](super::super::Pwm::start)/[`stop`](super::super::Pwm::stop)).
    pub(crate) fn configure_clock(regs: &Block, clksrc: u8, clkdiv: u8) {
        regs.cr
            .write(|w| unsafe { w.clksrc().bits(clksrc as u8).clkdiv().bits(clkdiv as u16) });
    }

    /// Force the `A` output idle level low and non-inverted (a sane default).
    pub(crate) fn configure_output(regs: &Block) {
        regs.ocr
            .write(|w| w.idlea().bit(false).inva().bit(false));
    }

    /// Set the run mode in `CR` (`1` runs, `0` stops).
    pub(crate) fn set_mode(regs: &Block, mode: u8) {
        regs.cr.modify(|_, w| unsafe { w.mode().bits(mode) });
    }

    /// True once a counter period has elapsed since the last clear.
    pub(crate) fn overflow_flag(regs: &Block) -> bool {
        regs.if_.read().upovf().bit()
    }

    /// Clear the period-elapsed flag (the `IF` flags are write-1-to-clear).
    pub(crate) fn clear_overflow(regs: &Block) {
        regs.if_.write(|w| w.upovf().set_bit());
    }

    /// Enable or disable the period-elapsed interrupt.
    pub(crate) fn set_overflow_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.upovf().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// A PWM peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; every register operation is forwarded to [`super::hw`].
    pub trait Instance {
        /// Clock gate that powers this peripheral (shared across PWM0..PWM4).
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block (always a `pwm0::RegisterBlock`).
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn set_period(period: u32) {
            hw::set_period(unsafe { &*Self::regs() }, period);
        }

        #[inline(always)]
        fn set_duty_a(duty: u32) {
            hw::set_duty_a(unsafe { &*Self::regs() }, duty);
        }

        #[inline(always)]
        fn set_duty_b(duty: u32) {
            hw::set_duty_b(unsafe { &*Self::regs() }, duty);
        }

        #[inline(always)]
        fn configure_clock(clksrc: u8, clkdiv: u8) {
            hw::configure_clock(unsafe { &*Self::regs() }, clksrc, clkdiv);
        }

        #[inline(always)]
        fn configure_output() {
            hw::configure_output(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_mode(mode: u8) {
            hw::set_mode(unsafe { &*Self::regs() }, mode);
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
/// A PWM peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a PWM singleton.
macro_rules! impl_pwm_instance {
    ($ty:ty, $pac:ident, $irq:expr, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Pwm
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

impl_pwm_instance!(crate::Pwm0, PWM0, crate::interrupt::PWM0, 0);
impl_pwm_instance!(crate::Pwm1, PWM1, crate::interrupt::PWM1, 1);
impl_pwm_instance!(crate::Pwm2, PWM2, crate::interrupt::PWM2, 2);
impl_pwm_instance!(crate::Pwm3, PWM3, crate::interrupt::PWM3, 3);
impl_pwm_instance!(crate::Pwm4, PWM4, crate::interrupt::PWM4, 4);

/// Number of PWM instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 5;
