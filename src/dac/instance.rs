//! Register access for the DAC block.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type of the single DAC instance on the SWM341.
pub(crate) type Block = pac::dac::RegisterBlock;

pub(crate) mod sealed {
    use super::{Block, PeripheralClockIndex};

    /// The DAC peripheral the driver can talk to.
    ///
    /// There is a single instance on the SWM341, so this is implemented for
    /// [`crate::Dac`] with the `DAC` register block and the `Dac` clock gate.
    pub trait Instance {
        /// Clock gate that powers this peripheral (`Dac`).
        fn clock() -> PeripheralClockIndex;
        /// Pointer to this instance's register block (`pac::DAC::PTR`).
        fn regs() -> *const Block;

        /// Power on the output driver (`CR.EN`).
        #[inline(always)]
        fn enable_output() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().set_bit());
        }

        /// Power down the output driver (`CR.EN`).
        #[inline(always)]
        fn disable_output() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().clear_bit());
        }

        /// Select how [`write_data`] packs the value into `DHR` (`CR.DHRFMT`).
        #[inline(always)]
        fn set_format(fmt: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| unsafe { w.dhrfmt().bits(fmt) });
        }

        /// Gate the data-hold register onto a DMA request (`CR.DMAEN`).
        #[inline(always)]
        fn set_dma(on: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.dmaen().bit(on));
        }

        /// Pulse the software trigger (`SWTRG`) to load `DHR` into `DOR`.
        #[inline(always)]
        fn software_trigger() {
            let regs = unsafe { &*Self::regs() };
            regs.swtrg.write(|w| unsafe { w.bits(1) });
        }

        /// Write the raw data-hold value (`DHR`).
        #[inline(always)]
        fn write_data(raw: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.dhr.write(|w| unsafe { w.bits(raw) });
        }

        /// True while a previously written value has not yet been loaded
        /// (`SR.DHRFULL`).
        #[inline(always)]
        fn data_full() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().dhrfull().bit()
        }
    }
}

#[allow(private_bounds)]
/// A DAC peripheral usable by the driver.
///
/// Kept as the public, user-facing bound (mirroring the other peripheral
/// drivers) even though the SWM341 has a single DAC instance.
#[allow(dead_code)]
pub trait Instance: sealed::Instance {}

macro_rules! impl_dac_instance {
    ($ty:ty, $clock:expr) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                $clock
            }

            #[inline]
            fn regs() -> *const Block {
                pac::DAC::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_dac_instance!(crate::Dac, PeripheralClockIndex::Dac);
