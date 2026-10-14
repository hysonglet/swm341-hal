//! Register access for the CORDIC co-processor.
//!
//! The SWM341 CORDIC computes transcendental functions in hardware. There is a
//! single instance; it is selected through the `pac::CORDIC` register block.
//! Results are 16-bit Q14 fixed-point numbers (14 fractional bits, 2 integer
//! bits) read back from one of four result registers depending on the operation.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every CORDIC instance on the SWM341.
pub(crate) type Block = pac::cordic::RegisterBlock;

/// Register access shared by every CORDIC instance.
pub(crate) mod hw {
    use super::Block;

    pub(crate) fn enable(_regs: &Block) {}
    pub(crate) fn disable(_regs: &Block) {}

    /// Kick off the currently configured operation.
    pub(crate) fn start(regs: &Block) {
        regs.cmd.modify(|_, w| w.start().set_bit());
    }

    /// `true` once the result register(s) hold a valid computation.
    pub(crate) fn is_done(regs: &Block) -> bool {
        regs.if_.read().done().bit_is_set()
    }

    /// Clear the DONE interrupt flag (write 1 to clear).
    pub(crate) fn clear_done(regs: &Block) {
        regs.if_.write(|w| w.done().set_bit());
    }

    /// Select the operation. `sincos` routes to the sin/cos result registers,
    /// `muldiv` (only meaningful when `sincos` is false) selects the other modes.
    pub(crate) fn set_mode(regs: &Block, sincos: bool, muldiv: u8) {
        regs.cmd
            .modify(|_, w| unsafe { w.sincos().bit(sincos).muldiv().bits(muldiv) });
    }

    /// Prescaler range (2 bits) for the convergence; leave at 0 for the default.
    pub(crate) fn set_range(regs: &Block, range: u8) {
        regs.cmd.modify(|_, w| unsafe { w.range().bits(range) });
    }

    /// Load the two Q14 operands: `x = (xi << 14) | xf`, `y = (yi << 14) | yf`.
    pub(crate) fn write_input(regs: &Block, xf: u16, xi: u8, yf: u16, yi: u8) {
        regs.input
            .write(|w| unsafe { w.f().bits(xf).i().bits(xi).f2().bits(yf).i2().bits(yi) });
    }

    pub(crate) fn read_cos(regs: &Block) -> (u16, u8) {
        let r = regs.cos.read();
        (r.f().bits(), r.i().bits())
    }
    pub(crate) fn read_sin(regs: &Block) -> (u16, u8) {
        let r = regs.sin.read();
        (r.f().bits(), r.i().bits())
    }
    pub(crate) fn read_arctan(regs: &Block) -> (u16, u8) {
        let r = regs.arctan.read();
        (r.f().bits(), r.i().bits())
    }
    pub(crate) fn read_tanh(regs: &Block) -> (u16, u8) {
        let r = regs.tanh.read();
        (r.f().bits(), r.i().bits())
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// A CORDIC peripheral the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers this peripheral.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> pac::Interrupt;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn enable() {
            hw::enable(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn start() {
            hw::start(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn is_done() -> bool {
            hw::is_done(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn clear_done() {
            hw::clear_done(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn set_mode(sincos: bool, muldiv: u8) {
            hw::set_mode(unsafe { &*Self::regs() }, sincos, muldiv);
        }
        #[inline(always)]
        fn set_range(range: u8) {
            hw::set_range(unsafe { &*Self::regs() }, range);
        }
        #[inline(always)]
        fn write_input(xf: u16, xi: u8, yf: u16, yi: u8) {
            hw::write_input(unsafe { &*Self::regs() }, xf, xi, yf, yi);
        }
        #[inline(always)]
        fn read_cos() -> (u16, u8) {
            hw::read_cos(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn read_sin() -> (u16, u8) {
            hw::read_sin(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn read_arctan() -> (u16, u8) {
            hw::read_arctan(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn read_tanh() -> (u16, u8) {
            hw::read_tanh(unsafe { &*Self::regs() })
        }
    }
}

#[allow(private_bounds)]
/// A CORDIC peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_cordic_instance {
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

impl_cordic_instance!(crate::Cordic, CORDIC, CORDIC, Cordic, 0);
