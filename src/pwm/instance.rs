//! Register access for the PWM blocks.
//!
//! All five PWM instances (`PWM0..PWM4`) share the `pac::pwm0::RegisterBlock`
//! type -- `PWM1..PWM4` are `pub use pwm0 as pwmN` aliases in the PAC -- so a
//! single set of register-access default methods on `sealed::Instance` serves
//! every instance. The only
//! per-instance data are the register-block pointer, the (shared) clock gate, the
//! async index and the NVIC vector, forwarded by [`sealed::Instance`].
//!
//! A PWM channel compares its free-running counter against `CMPA`/`CMPB` to drive
//! the `A`/`B` outputs; `PERIOD` sets the top of the count. `CR` only selects the
//! clock and run mode -- there is no per-instance enable bit, so opening the
//! `Pwm` clock gate is what powers the family.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every PWM instance on the SWM341.
pub(crate) type Block = pac::pwm0::RegisterBlock;

pub(crate) mod sealed {

    use super::Block;

    /// A PWM peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; all register access lives in the default methods of this trait.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Set the counter top (`PERIOD`).
        #[inline(always)]
        fn set_period(period: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.period.write(|w| unsafe { w.bits(period) });
        }

        /// Set the `A` channel compare value (`CMPA`, the duty cycle).
        #[inline(always)]
        fn set_duty_a(duty: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.cmpa.write(|w| unsafe { w.bits(duty) });
        }

        /// Set the `B` channel compare value (`CMPB`, the duty cycle).
        #[inline(always)]
        fn set_duty_b(duty: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.cmpb.write(|w| unsafe { w.bits(duty) });
        }

        /// Select the clock source and divider in `CR` (run mode is applied by
        /// [`start`](super::super::Pwm::start)/[`stop`](super::super::Pwm::stop)).
        #[inline(always)]
        fn configure_clock(clksrc: u8, clkdiv: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.cr
                .write(|w| unsafe { w.clksrc().bits(clksrc as u8).clkdiv().bits(clkdiv as u16) });
        }

        /// Force the `A` output idle level low and non-inverted (a sane default).
        #[inline(always)]
        fn configure_output() {
            let regs = unsafe { &*Self::regs() };
            regs.ocr
                .write(|w| w.idlea().bit(false).inva().bit(false));
        }

        /// Set the run mode in `CR` (`1` runs, `0` stops).
        #[inline(always)]
        fn set_mode(mode: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| unsafe { w.mode().bits(mode) });
        }

        /// True once a counter period has elapsed since the last clear.
        #[inline(always)]
        fn overflow_flag() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().upovf().bit()
        }

        /// Clear the period-elapsed flag (the `IF` flags are write-1-to-clear).
        #[inline(always)]
        fn clear_overflow() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.upovf().set_bit());
        }

        /// Enable or disable the period-elapsed interrupt.
        #[inline(always)]
        fn set_overflow_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.upovf().bit(enable));
        }
    }
}

#[allow(private_bounds)]
/// A PWM peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a PWM singleton.
macro_rules! impl_pwm_instance {
    ($ty:ty, $pac:ident, $irq:expr, $index:literal) => {
        crate::impl_instance_base!($ty, crate::pac::pwm0::RegisterBlock, $irq, PeripheralClockIndex::Pwm, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
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
