//! Register access for the HALL (hall-sensor interface).
//!
//! On the SWM341 the HALL registers live inside the `TIMRG` peripheral block.
//! There are two hall interfaces; the first is named `HALL0`/`hall0` and the
//! second is named `HALL3`/`hall3` in the silicon (the interrupt is `HALL1`).
//! The driver selects which one with a `which` byte (0 or 1).

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type behind every HALL instance (`TIMRG`).
pub(crate) type Block = pac::timrg::RegisterBlock;

/// Register access shared by every HALL instance.
pub(crate) mod hw {
    use super::Block;

    pub(crate) fn enable(_regs: &Block) {}
    pub(crate) fn disable(_regs: &Block) {}

    pub(crate) fn enable_hall(regs: &Block, which: u8) {
        if which == 0 {
            regs.hallen.modify(|_, w| w.hall0().set_bit());
        } else {
            regs.hallen.modify(|_, w| w.hall3().set_bit());
        }
    }
    pub(crate) fn disable_hall(regs: &Block, which: u8) {
        if which == 0 {
            regs.hallen.modify(|_, w| w.hall0().clear_bit());
        } else {
            regs.hallen.modify(|_, w| w.hall3().clear_bit());
        }
    }
    pub(crate) fn enable_interrupt(regs: &Block, which: u8) {
        if which == 0 {
            regs.hallie.modify(|_, w| w.hall0().set_bit());
        } else {
            regs.hallie.modify(|_, w| w.hall3().set_bit());
        }
    }
    pub(crate) fn disable_interrupt(regs: &Block, which: u8) {
        if which == 0 {
            regs.hallie.modify(|_, w| w.hall0().clear_bit());
        } else {
            regs.hallie.modify(|_, w| w.hall3().clear_bit());
        }
    }

    pub(crate) fn is_flag(regs: &Block, which: u8, input: u8) -> bool {
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
    pub(crate) fn clear_flag(regs: &Block, which: u8, input: u8) {
        match (which, input) {
            (0, 0) => regs.hallif.write(|w| w.h0in0().set_bit()),
            (0, 1) => regs.hallif.write(|w| w.h0in1().set_bit()),
            (0, _) => regs.hallif.write(|w| w.h0in2().set_bit()),
            (_, 0) => regs.hallif.write(|w| w.h3in0().set_bit()),
            (_, 1) => regs.hallif.write(|w| w.h3in1().set_bit()),
            (_, _) => regs.hallif.write(|w| w.h3in2().set_bit()),
        }
    }
    pub(crate) fn read_value(regs: &Block, which: u8) -> u32 {
        if which == 0 {
            regs.hall0v.read().bits()
        } else {
            regs.hall3v.read().bits()
        }
    }
    pub(crate) fn read_status(regs: &Block, which: u8, input: u8) -> bool {
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

pub(crate) mod sealed {
    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// A HALL peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> pac::Interrupt;
        fn regs() -> *const Block;

        #[inline(always)]
        fn enable() {
            hw::enable(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() });
        }
    }
}

#[allow(private_bounds)]
/// A HALL peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_hall_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::$clock
            }
            #[inline]
            fn index() -> usize {
                $index
            }
            #[inline]
            fn irq() -> pac::Interrupt {
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

impl_hall_instance!(crate::Hall, TIMRG, HALL0, Hall, 0);
