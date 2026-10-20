//! Register access for the QEI block.
//!
//! The SWM341 has a single quadrature encoder instance at `0x4004_C800`; its
//! clock gate is `SYS.CLKEN1.QEI` and its NVIC vector is `QEI` (number 90).

use crate::clock::PeripheralClockIndex;

use super::types::{Config, Interrupt};

/// The register-block type of the single QEI instance.
pub(crate) type Block = crate::pac::qei::RegisterBlock;

pub(crate) mod sealed {
    use super::{Block, Config, Interrupt};

    /// A QEI peripheral the driver can talk to.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        /// Apply `config` while the decoder is stopped.
        #[inline(always)]
        fn configure(config: &Config) {
            let regs = unsafe { &*Self::regs() };
            // Keep the decoder off while its operating bits are programmed.
            regs.cr.write(|w| {
                w.mode()
                    .set_bit()
                    .rstsrc()
                    .bit(config.reset_mode.bit())
                    .x2x4()
                    .bit(config.count_mode.bit())
                    .abswap()
                    .bit(config.swap_ab)
                    .ena()
                    .bit(config.enable)
            });
            regs.maxcnt
                .write(|w| unsafe { w.bits(u32::from(config.max_count)) });
            // `IE` arms a source, `IM` (1 = unmasked) gates it independently;
            // mirror the enabled set so enabled sources actually fire.
            regs.ie
                .write(|w| unsafe { w.bits(config.interrupts.bits()) });
            regs.im
                .write(|w| unsafe { w.bits(config.interrupts.bits()) });
        }

        /// Enable/disable the decoder (`CR.ENA`).
        #[inline(always)]
        fn set_enabled(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.ena().bit(enable));
        }

        /// Whether the decoder is enabled (`CR.ENA`).
        #[inline(always)]
        fn is_enabled() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.cr.read().ena().bit_is_set()
        }

        /// Pause/resume the decoder without clearing its state (`CR.PAUSE`).
        #[inline(always)]
        fn set_paused(pause: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.pause().bit(pause));
        }

        /// Whether the decoder is currently paused (`CR.PAUSE`).
        #[inline(always)]
        fn is_paused() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.cr.read().pause().bit_is_set()
        }

        /// Swap the A/B phase inputs (`CR.ABSWAP`).
        #[inline(always)]
        fn set_swap_ab(swap: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.abswap().bit(swap));
        }

        /// Current level sampled on the index (Z) pad (`CR.INDEX`, read-only).
        #[inline(always)]
        fn index_level() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.cr.read().index().bit_is_set()
        }

        /// Read the 16-bit position counter (`POSCNT`).
        #[inline(always)]
        fn count() -> u16 {
            let regs = unsafe { &*Self::regs() };
            (regs.poscnt.read().bits() & 0xFFFF) as u16
        }

        /// Load `value` into the position counter (`POSCNT`).
        ///
        /// The datasheet marks `POSCNT` read-only (the counter normally resets
        /// itself through the index phase or a count match), but the SVD models
        /// it as read-write, so the path is exposed for silicon that accepts
        /// the write.
        #[inline(always)]
        fn set_count(value: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.poscnt.write(|w| unsafe { w.bits(u32::from(value)) });
        }

        /// Read the maximum-count reload value (`MAXCNT`).
        #[inline(always)]
        fn max_count() -> u16 {
            let regs = unsafe { &*Self::regs() };
            (regs.maxcnt.read().bits() & 0xFFFF) as u16
        }

        /// Program the maximum-count reload value (`MAXCNT`).
        #[inline(always)]
        fn set_max_count(value: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.maxcnt.write(|w| unsafe { w.bits(u32::from(value)) });
        }

        /// Arm/disarm one source in the interrupt-enable register (`IE`).
        #[inline(always)]
        fn set_interrupt_enabled(source: Interrupt, enable: bool) {
            let regs = unsafe { &*Self::regs() };
            let mask = source.mask();
            regs.ie.modify(|r, w| unsafe {
                w.bits((r.bits() & !mask) | if enable { mask } else { 0 })
            });
        }

        /// Mask/unmask one source in the interrupt-mask register (`IM`;
        /// `1` = unmasked).
        #[inline(always)]
        fn set_interrupt_unmasked(source: Interrupt, unmask: bool) {
            let regs = unsafe { &*Self::regs() };
            let mask = source.mask();
            regs.im.modify(|r, w| unsafe {
                w.bits((r.bits() & !mask) | if unmask { mask } else { 0 })
            });
        }

        /// Clear one pending interrupt flag (`IC`, write-1-to-clear).
        #[inline(always)]
        fn clear_flag(source: Interrupt) {
            let regs = unsafe { &*Self::regs() };
            regs.ic.write(|w| unsafe { w.bits(source.mask()) });
        }

        /// Whether one interrupt source is pending (`IF`).
        #[inline(always)]
        fn is_flag(source: Interrupt) -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().bits() & source.mask() != 0
        }

        /// Whether one interrupt source lost an event because it was already
        /// pending (`IFOV`).
        #[inline(always)]
        fn is_overflow(source: Interrupt) -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.ifov.read().bits() & source.mask() != 0
        }
    }
}

#[allow(private_bounds)]
/// A QEI peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_qei_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        crate::impl_instance_base!(
            $ty,
            crate::pac::qei::RegisterBlock,
            crate::interrupt::$irq,
            PeripheralClockIndex::$clock,
            $index
        );

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_qei_instance!(crate::Qei, QEI, QEI, Qei, 0);
