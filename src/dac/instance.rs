//! Register access for the DAC block.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type of the single DAC instance on the SWM341.
pub(crate) type Block = pac::dac::RegisterBlock;

/// Register access shared by the DAC instance.
pub(crate) mod hw {
    use super::Block;

    /// Power on the output driver (`CR.EN`).
    pub(crate) fn enable_output(regs: &Block) {
        regs.cr.modify(|_, w| w.en().set_bit());
    }

    /// Power down the output driver (`CR.EN`).
    pub(crate) fn disable_output(regs: &Block) {
        regs.cr.modify(|_, w| w.en().clear_bit());
    }

    /// Select how [`write_data`](super::hw::write_data) packs the value into
    /// `DHR` (`CR.DHRFMT`).
    pub(crate) fn set_format(regs: &Block, fmt: u8) {
        regs.cr.modify(|_, w| unsafe { w.dhrfmt().bits(fmt) });
    }

    /// Gate the data-hold register onto a DMA request (`CR.DMAEN`).
    pub(crate) fn set_dma(regs: &Block, on: bool) {
        regs.cr.modify(|_, w| w.dmaen().bit(on));
    }

    /// Pulse the software trigger (`SWTRG`) to load `DHR` into `DOR`.
    pub(crate) fn software_trigger(regs: &Block) {
        regs.swtrg.write(|w| unsafe { w.bits(1) });
    }

    /// Write the raw data-hold value (`DHR`).
    pub(crate) fn write_data(regs: &Block, raw: u32) {
        regs.dhr.write(|w| unsafe { w.bits(raw) });
    }

    /// True while a previously written value has not yet been loaded
    /// (`SR.DHRFULL`).
    pub(crate) fn data_full(regs: &Block) -> bool {
        regs.sr.read().dhrfull().bit()
    }
}

pub(crate) mod sealed {
    use super::{hw, Block, PeripheralClockIndex};

    /// The DAC peripheral the driver can talk to.
    ///
    /// There is a single instance on the SWM341, so this is implemented for
    /// [`crate::Dac`] with the `DAC` register block and the `Dac` clock gate.
    pub trait Instance {
        /// Clock gate that powers this peripheral (`Dac`).
        fn clock() -> PeripheralClockIndex;
        /// Pointer to this instance's register block (`pac::DAC::PTR`).
        fn regs() -> *const Block;

        #[inline(always)]
        fn enable_output() {
            hw::enable_output(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn disable_output() {
            hw::disable_output(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_format(fmt: u8) {
            hw::set_format(unsafe { &*Self::regs() }, fmt);
        }

        #[inline(always)]
        fn set_dma(on: bool) {
            hw::set_dma(unsafe { &*Self::regs() }, on);
        }

        #[inline(always)]
        fn software_trigger() {
            hw::software_trigger(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn write_data(raw: u32) {
            hw::write_data(unsafe { &*Self::regs() }, raw);
        }

        #[inline(always)]
        fn data_full() -> bool {
            hw::data_full(unsafe { &*Self::regs() })
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
