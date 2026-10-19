//! Register access for the analog comparators (CMP0..CMP2).
//!
//! On the SWM341 the three comparators share the `SYS` register block. Their
//! control lives in `SYS.ACMPCR` (one bit slice per unit: enable, hysteresis,
//! negative-reference source and interrupt-enable), `SYS.ACMPCR2` carries the
//! shared internal `VREF` divider, and `SYS.ACMPSR` reports each unit's output
//! and latches its interrupt flag. All three units multiplex onto the single
//! `ACMP` NVIC vector, so the ISR (in `future.rs`) demultiplexes by reading
//! `ACMPSR`.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type behind every comparator instance (`SYS`).
pub(crate) type Block = pac::sys::RegisterBlock;

/// Bit position of a comparator unit's control field inside `ACMPCR`/`ACMPSR`.
///
/// `ACMPCR` lays out one slice per comparator at bits `n`, `8+n`, `16+n`,
/// `24+n` (ON / HYS / _NVREF / IE); `ACMPSR` puts OUT at bit `n` and IF at
/// `8+n`. The slice index is exactly the instance index `n` (0..2).
#[inline(always)]
fn unit_bits(index: usize) -> u32 {
    index as u32
}

pub(crate) mod sealed {
    use super::{Block, unit_bits};
    use crate::cmp::types::NegativeSource;

    /// A comparator unit the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. All three share one
    /// register block (`SYS`) and one NVIC vector (`ACMP`); only the per-unit
    /// bit slice differs, which is why the accessors below compute bit offsets
    /// from [`index`](crate::instance_base::InstanceBase::index) rather than
    /// storing it explicitly.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block (`SYS`).
        fn regs() -> *const Block;

        /// Power on the comparator unit (`ACMPCR.CMPxON`).
        #[inline(always)]
        fn enable_unit() {
            let regs = unsafe { &*Self::regs() };
            let bit = unit_bits(Self::index());
            regs.acmpcr.modify(|r, w| unsafe { w.bits(r.bits() | (1 << bit)) });
        }

        /// Power off the comparator unit (`ACMPCR.CMPxON`).
        #[inline(always)]
        fn disable_unit() {
            let regs = unsafe { &*Self::regs() };
            let bit = unit_bits(Self::index());
            regs.acmpcr.modify(|r, w| unsafe { w.bits(r.bits() & !(1 << bit)) });
        }

        /// Toggle the input hysteresis (`ACMPCR.CMPxHYS`, bit `8+n`).
        #[inline(always)]
        fn set_hysteresis(on: bool) {
            let regs = unsafe { &*Self::regs() };
            let bit = 8 + unit_bits(Self::index());
            regs.acmpcr.modify(|r, w| unsafe {
                let v = r.bits();
                w.bits(if on { v | (1 << bit) } else { v & !(1 << bit) })
            });
        }

        /// Select the negative input source (`ACMPCR._xNVREF`, bit `16+n`).
        #[inline(always)]
        fn set_negative_source(src: NegativeSource) {
            let regs = unsafe { &*Self::regs() };
            let bit = 16 + unit_bits(Self::index());
            regs.acmpcr.modify(|r, w| unsafe {
                let v = r.bits();
                match src {
                    NegativeSource::ExternalPin => w.bits(v & !(1 << bit)),
                    NegativeSource::InternalVref => w.bits(v | (1 << bit)),
                }
            });
        }

        /// Enable or disable this unit's interrupt (`ACMPCR.CMPxIE`, bit `24+n`).
        #[inline(always)]
        fn set_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            let bit = 24 + unit_bits(Self::index());
            regs.acmpcr.modify(|r, w| unsafe {
                let v = r.bits();
                w.bits(if enable { v | (1 << bit) } else { v & !(1 << bit) })
            });
        }

        /// True if this unit's latched interrupt flag is set (`ACMPSR.CMPxIF`).
        #[inline(always)]
        fn irq_flag() -> bool {
            let regs = unsafe { &*Self::regs() };
            let bit = 8 + unit_bits(Self::index());
            regs.acmpsr.read().bits() & (1 << bit) != 0
        }

        /// Clear this unit's latched interrupt flag (`ACMPSR.CMPxIF`,
        /// write-1-to-clear).
        #[inline(always)]
        fn clear_irq_flag() {
            let regs = unsafe { &*Self::regs() };
            let bit = 8 + unit_bits(Self::index());
            regs.acmpsr.write(|w| unsafe { w.bits(1 << bit) });
        }

        /// Read the comparator output (`ACMPSR.CMPxOUT`, bit `n`).
        #[inline(always)]
        fn output() -> bool {
            let regs = unsafe { &*Self::regs() };
            let bit = unit_bits(Self::index());
            regs.acmpsr.read().bits() & (1 << bit) != 0
        }

        /// Program the shared internal reference divider (`ACMPCR2.VREF`, bits 3:8).
        ///
        /// The exact voltage produced by `vref` depends on the board's analog
        /// reference, so the raw 6-bit field is exposed directly. Only meaningful
        /// for units whose [`NegativeSource`] is
        /// [`InternalVref`](NegativeSource::InternalVref).
        #[inline(always)]
        fn set_reference_voltage(vref: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.acmpcr2.modify(|r, w| unsafe {
                let v = r.bits() & !(0x3F << 3);
                w.bits(v | ((vref as u32 & 0x3F) << 3))
            });
        }
    }
}

#[allow(private_bounds)]
/// A comparator unit usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for a comparator singleton.
///
/// The three comparators (CMP0/1/2) differ only in their bit slice, expressed
/// here as `$index`. They share `SYS` (`regs()`) and the `ACMP` NVIC vector.
macro_rules! impl_cmp_instance {
    ($ty:ty, $index:literal) => {
        crate::impl_instance_base!(
            $ty,
            crate::pac::sys::RegisterBlock,
            crate::interrupt::ACMP,
            PeripheralClockIndex::Acmp,
            $index
        );

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::SYS::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_cmp_instance!(crate::Cmp0, 0);
impl_cmp_instance!(crate::Cmp1, 1);
impl_cmp_instance!(crate::Cmp2, 2);

/// Number of comparator instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 3;
