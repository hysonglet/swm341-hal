//! Register access for the on-chip operational amplifiers (OPA0–OPA3).
//!
//! Unlike most SWM341 peripherals the four amplifiers have no register block of
//! their own and no NVIC vector. Their only control is the four
//! `OPA0ON..OPA3ON` bits in the shared [`SYS`](crate::pac::SYS) register block
//! (`SYS.OPACR`), one bit slice per unit at bit `n`. The amplifier pads are
//! dedicated analog pads — no `PORTx.FUNC` muxing is involved — and the analog
//! control block is always clocked, so neither pin routing nor a real clock
//! gate is required.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type behind every amplifier instance (`SYS`).
pub(crate) type Block = pac::sys::RegisterBlock;

pub(crate) mod sealed {
    use super::{Block, PeripheralClockIndex};

    /// An operational amplifier unit the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. All four units share
    /// the `SYS` block; only the per-unit bit slice differs, which is why the
    /// accessors below derive the bit offset from [`index`] rather than storing
    /// it explicitly. Deliberately does not extend
    /// [`crate::instance_base::InstanceBase`]: the amplifiers have no NVIC
    /// vector.
    pub trait Instance {
        /// Conceptual clock gate of the unit (always-on, kept so the driver
        /// shape matches the other analog drivers).
        fn clock() -> PeripheralClockIndex;
        /// Bit-slice index of this unit inside `OPACR` (0..=3).
        fn index() -> usize;
        /// Pointer to the shared `SYS` register block.
        fn regs() -> *const Block;

        /// Power the unit on (`OPACR.OPAxON` set at bit `n`).
        #[inline(always)]
        fn enable_unit() {
            let regs = unsafe { &*Self::regs() };
            let bit = Self::index() as u32;
            regs.opacr
                .modify(|r, w| unsafe { w.bits(r.bits() | (1u32 << bit)) });
        }

        /// Power the unit down (`OPACR.OPAxON` clear at bit `n`).
        #[inline(always)]
        fn disable_unit() {
            let regs = unsafe { &*Self::regs() };
            let bit = Self::index() as u32;
            regs.opacr
                .modify(|r, w| unsafe { w.bits(r.bits() & !(1u32 << bit)) });
        }

        /// True if the unit is currently powered (`OPACR.OPAxON` at bit `n`).
        #[inline(always)]
        fn is_enabled() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.opacr.read().bits() & (1u32 << Self::index() as u32) != 0
        }
    }
}

#[allow(private_bounds)]
/// An amplifier unit usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for one amplifier singleton.
///
/// The four amplifiers (OPA0–OPA3) differ only in their `OPACR` bit slice,
/// expressed here as `$index`; they all share `SYS` ([`pac::SYS::PTR`]).
macro_rules! impl_opa_instance {
    ($ty:ty, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Opa
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn regs() -> *const Block {
                crate::pac::SYS::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_opa_instance!(crate::Opa0, 0);
impl_opa_instance!(crate::Opa1, 1);
impl_opa_instance!(crate::Opa2, 2);
impl_opa_instance!(crate::Opa3, 3);
