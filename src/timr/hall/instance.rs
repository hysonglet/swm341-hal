//! Register access for the HALL (hall-sensor interface).
//!
//! On the SWM341 the HALL registers live inside the `TIMRG` peripheral block,
//! which is why this driver is a submodule of [`crate::timr`]. There are two
//! interfaces, `HALL0` and `HALL3` (no `HALL1`/`HALL2`); the driver selects one
//! with a `which` byte (0 = `HALL0`, 1 = `HALL3`, see
//! [`HallSelect`](crate::timr::hall::HallSelect)).
//!
//! Each interface captures its *paired* timer -- `HALL0` latches TIMR0's count
//! into `HALL0V`, `HALL3` latches TIMR3's into `HALL3V` -- on every edge of any
//! of its three inputs (the module is dual-edge triggered). The counter has to be
//! running for `HALL0V`/`HALL3V` to mean anything, which is why the driver owns
//! the paired [`Timr`](crate::timr::Timr) rather than leaving it to the caller.

use crate::clock::PeripheralClockIndex;

/// The register-block type behind every HALL instance (`TIMRG`).
pub(crate) type Block = crate::pac::timrg::RegisterBlock;

pub(crate) mod sealed {
    use super::Block;

    /// A HALL peripheral the driver can talk to.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        /// NVIC vector of hall interface `which` (`0` => `HALL0`, `1` => `HALL3`).
        ///
        /// The two interfaces have their own interrupt -- `HALL0` (NVIC 12) and
        /// `HALL3` (NVIC 28) -- so the vector cannot come from the single
        /// [`irq`](crate::instance_base::InstanceBase::irq) of the shared `TIMRG`
        /// instance; it has to follow the selected interface.
        #[inline(always)]
        fn irq_for(which: u8) -> crate::interrupt::ExtendedInterrupt {
            if which == 0 {
                crate::interrupt::HALL0
            } else {
                crate::interrupt::HALL3
            }
        }

        /// Index of the TIMR paired with hall interface `which`.
        ///
        /// `HALL0` is timed by TIMR0 and `HALL3` by TIMR3: the datasheet states
        /// the input edge stores `Timer(load - current)` into `HALL0V` / `HALL3V`
        /// respectively, so a hall reading is only meaningful once that specific
        /// timer is running.
        #[inline(always)]
        fn paired_timer_index(which: u8) -> usize {
            if which == 0 { 0 } else { 3 }
        }

        #[inline(always)]
        fn enable_hall(which: u8) {
            let regs = unsafe { &*Self::regs() };
            if which == 0 {
                regs.hallen.modify(|_, w| w.hall0().set_bit());
            } else {
                regs.hallen.modify(|_, w| w.hall3().set_bit());
            }
        }
        #[inline(always)]
        fn disable_hall(which: u8) {
            let regs = unsafe { &*Self::regs() };
            if which == 0 {
                regs.hallen.modify(|_, w| w.hall0().clear_bit());
            } else {
                regs.hallen.modify(|_, w| w.hall3().clear_bit());
            }
        }
        #[inline(always)]
        fn enable_interrupt(which: u8) {
            let regs = unsafe { &*Self::regs() };
            if which == 0 {
                regs.hallie.modify(|_, w| w.hall0().set_bit());
            } else {
                regs.hallie.modify(|_, w| w.hall3().set_bit());
            }
        }
        #[inline(always)]
        fn disable_interrupt(which: u8) {
            let regs = unsafe { &*Self::regs() };
            if which == 0 {
                regs.hallie.modify(|_, w| w.hall0().clear_bit());
            } else {
                regs.hallie.modify(|_, w| w.hall3().clear_bit());
            }
        }

        /// True if input `input` of interface `which` latched a change.
        #[inline(always)]
        fn is_flag(which: u8, input: u8) -> bool {
            let regs = unsafe { &*Self::regs() };
            let r = regs.hallif.read();
            match (which, input) {
                (0, 0) => r.h0in0().bit_is_set(),
                (0, 1) => r.h0in1().bit_is_set(),
                (0, _) => r.h0in2().bit_is_set(),
                (_, 0) => r.h3in0().bit_is_set(),
                (_, 1) => r.h3in1().bit_is_set(),
                (_, _) => r.h3in2().bit_is_set(),
            }
        }
        /// Clear the change flag of one input (`HALLIF` is write-1-to-clear).
        #[inline(always)]
        fn clear_flag(which: u8, input: u8) {
            let regs = unsafe { &*Self::regs() };
            match (which, input) {
                (0, 0) => regs.hallif.write(|w| w.h0in0().set_bit()),
                (0, 1) => regs.hallif.write(|w| w.h0in1().set_bit()),
                (0, _) => regs.hallif.write(|w| w.h0in2().set_bit()),
                (_, 0) => regs.hallif.write(|w| w.h3in0().set_bit()),
                (_, 1) => regs.hallif.write(|w| w.h3in1().set_bit()),
                (_, _) => regs.hallif.write(|w| w.h3in2().set_bit()),
            }
        }
        /// Clear every change flag of one interface at once.
        #[inline(always)]
        fn clear_all_flags(which: u8) {
            for input in 0..3 {
                Self::clear_flag(which, input);
            }
        }
        /// The captured timer count for interface `which` (`HALL0V` / `HALL3V`).
        #[inline(always)]
        fn read_value(which: u8) -> u32 {
            let regs = unsafe { &*Self::regs() };
            if which == 0 {
                regs.hall0v.read().bits()
            } else {
                regs.hall3v.read().bits()
            }
        }
        /// Current level of input `input` of interface `which` (`HALLSR`).
        #[inline(always)]
        fn read_status(which: u8, input: u8) -> bool {
            let regs = unsafe { &*Self::regs() };
            let r = regs.hallsr.read();
            match (which, input) {
                (0, 0) => r.h0in0().bit_is_set(),
                (0, 1) => r.h0in1().bit_is_set(),
                (0, _) => r.h0in2().bit_is_set(),
                (_, 0) => r.h3in0().bit_is_set(),
                (_, 1) => r.h3in1().bit_is_set(),
                (_, _) => r.h3in2().bit_is_set(),
            }
        }
    }
}

#[allow(private_bounds)]
/// A HALL peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_hall_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        crate::impl_instance_base!($ty, crate::pac::timrg::RegisterBlock, crate::interrupt::$irq, PeripheralClockIndex::$clock, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_hall_instance!(crate::Hall, TIMRG, HALL0, Hall, 0);
