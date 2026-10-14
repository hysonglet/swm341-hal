//! Register access for the ADC blocks.
//!
//! The two ADC instances (`ADC0`, `ADC1`) share the `pac::adc0::RegisterBlock`
//! type -- `ADC1` is a `pub use adc0 as adc1` alias in the PAC -- so a single
//! non-generic [`hw`] implementation serves both. They differ only in their clock
//! gate (`Adc0` vs `Adc1`) and NVIC vector, which is why the instance macro takes
//! those as arguments.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every ADC instance on the SWM341.
pub(crate) type Block = pac::adc0::RegisterBlock;

/// Register access shared by every ADC instance.
pub(crate) mod hw {
    use super::Block;
    use cortex_m::asm::nop;

    /// Power on and configure the converter.
    ///
    /// Mirrors the vendor `ADC_Init` + `ADC_Open` + `ADC_Calibrate` flow:
    ///  1. reset the converter (CR.RESET pulse),
    ///  2. clear the data FIFO (CR.FFCLR pulse),
    ///  3. disable interrupts and clear all flags,
    ///  4. power on the internal analog LDO and let it settle,
    ///  5. assert CR.EN, then calibrate.
    ///
    /// Skipping the reset/calibration (or running the ADC clock at full speed)
    /// leaves the SAR state machine unable to finish a conversion, so the EOC
    /// flag never latches and a blocking read spins forever.
    pub(crate) fn enable(regs: &Block) {
        // Select HRC / 8 as the ADC clock so the converter runs within spec
        // (the reset default of AD0DIV = 0 would clock it at the full 150 MHz).
        let sys = unsafe { &*crate::pac::SYS::ptr() };
        sys.clksel
            .modify(|_, w| unsafe { w.ad0().bits(0).ad0div().bits(3) });

        // 1. Reset the digital core.
        regs.cr.modify(|_, w| w.reset().set_bit());
        for _ in 0..(120 * 20) {
            nop();
        }
        regs.cr.modify(|_, w| w.reset().clear_bit());

        // 2. Clear the conversion FIFO.
        regs.cr.modify(|_, w| unsafe { w.ffclr().bits(0xF) });
        regs.cr.modify(|_, w| unsafe { w.ffclr().bits(0) });

        // 3. No averaging, no interrupts; clear every pending flag.
        regs.cr.modify(|_, w| unsafe { w.avg().bits(0) });
        regs.ie.write(|w| unsafe { w.bits(0) });
        regs.if_.write(|w| unsafe { w.bits(0x3F3F3F3F) });

        // 4. Power on the analog LDO and let it settle.
        regs.cr2.modify(|_, w| w.enldo().set_bit());
        for _ in 0..(64 * 20) {
            nop();
        }
        // 5. Enable the converter, then calibrate it.
        regs.cr.modify(|_, w| w.en().set_bit());
        calibrate(regs);
    }

    /// Select the trigger source for sequence 0 (`1` = software start via `GO`).
    ///
    /// Also arms a single conversion per start (`SEQCOV.seq0 = 0`).
    pub(crate) fn set_trigger(regs: &Block, src: u8) {
        regs.seqtrg
            .modify(|_, w| unsafe { w.seq0().bits(src) });
        regs.seqcov.modify(|_, w| unsafe { w.seq0().bits(0) });
    }

    /// Run the one-time factory calibration required before the first conversion.
    ///
    /// The vendor driver calls this right after enabling the converter; without
    /// it the SAR never settles and `EOC` never latches.
    pub(crate) fn calibrate(regs: &Block) {
        regs.calib.modify(|_, w| w.reset().set_bit());
        for _ in 0..(120 * 20) {
            nop();
        }
        regs.calib.modify(|_, w| w.start().set_bit());
        for _ in 0..(120 * 10) {
            nop();
        }
        regs.calib.modify(|_, w| w.start().clear_bit());
        while regs.calib.read().busy().bit() {
            nop();
        }
        regs.calib.modify(|_, w| w.load().set_bit());
    }

    /// True once sequence 0 has finished, read from the sequence status register
    /// (`SEQ0.SR.EOC`) -- this is the flag the vendor library polls.
    pub(crate) fn eoc_sr(regs: &Block) -> bool {
        regs.seq0.sr.read().eoc().bit()
    }

    /// Program the sampling-time register (`SEQ_SMP`).
    pub(crate) fn set_sampling(regs: &Block, sampling: u8) {
        regs.seqsmp.write(|w| unsafe { w.bits(sampling as u32) });
    }

    /// Route `channel` to the first slot of conversion sequence 0 (`SEQCHN0.SEQ0`).
    ///
    /// `SEQCHN0.SEQ0` is a *bitmask* of channels (bit `n` = channel `n`), not a
    /// channel number, so channel `n` is selected by setting bit `n`.
    pub(crate) fn select_channel(regs: &Block, channel: u8) {
        regs.seqchn0
            .modify(|_, w| unsafe { w.seq0().bits(1u16 << channel) });
    }

    /// Kick off sequence 0 (`GO.SEQ0`).
    pub(crate) fn start_conversion(regs: &Block) {
        regs.go.modify(|_, w| w.seq0().set_bit());
    }

    /// True while a conversion is in progress (`GO.BUSY`).
    pub(crate) fn busy(regs: &Block) -> bool {
        regs.go.read().busy().bit()
    }

    /// 12-bit conversion result of sequence 0 (`SEQ0.DR.VALUE`).
    pub(crate) fn result(regs: &Block) -> u16 {
        regs.seq0.dr.read().value().bits()
    }

    /// True once sequence 0 has finished (`IF.SEQ0EOC`).
    pub(crate) fn eoc_flag(regs: &Block) -> bool {
        regs.if_.read().seq0eoc().bit()
    }

    /// Clear the sequence-0 end-of-conversion flag (write-1-to-clear).
    pub(crate) fn clear_eoc(regs: &Block) {
        regs.if_.write(|w| w.seq0eoc().set_bit());
    }

    /// Enable or disable the end-of-conversion interrupt (`IE.SEQ0EOC`).
    pub(crate) fn set_eoc_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.seq0eoc().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// An ADC peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The register access is
    /// forwarded to [`super::hw`]; the clock gate and NVIC vector are supplied per
    /// instance because `ADC0` and `ADC1` are independently gated and vectored.
    pub trait Instance {
        /// Clock gate that powers this peripheral (`Adc0` or `Adc1`).
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block (always an `adc0::RegisterBlock`).
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
        fn set_trigger(src: u8) {
            hw::set_trigger(unsafe { &*Self::regs() }, src);
        }

        #[inline(always)]
        fn eoc_sr() -> bool {
            hw::eoc_sr(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn set_sampling(sampling: u8) {
            hw::set_sampling(unsafe { &*Self::regs() }, sampling);
        }

        #[inline(always)]
        fn select_channel(channel: u8) {
            hw::select_channel(unsafe { &*Self::regs() }, channel);
        }

        #[inline(always)]
        fn start_conversion() {
            hw::start_conversion(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn busy() -> bool {
            hw::busy(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn result() -> u16 {
            hw::result(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn eoc_flag() -> bool {
            hw::eoc_flag(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_eoc() {
            hw::clear_eoc(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_eoc_irq(enable: bool) {
            hw::set_eoc_irq(unsafe { &*Self::regs() }, enable);
        }
    }
}

#[allow(private_bounds)]
/// An ADC peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for an ADC singleton.
///
/// `ADC0` and `ADC1` differ in their clock gate and NVIC vector, so both are
/// passed in explicitly.
macro_rules! impl_adc_instance {
    ($ty:ty, $pac:ident, $clock:expr, $irq:expr, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                $clock
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

impl_adc_instance!(
    crate::Adc0,
    ADC0,
    PeripheralClockIndex::Adc0,
    crate::interrupt::ADC0,
    0
);
impl_adc_instance!(
    crate::Adc1,
    ADC1,
    PeripheralClockIndex::Adc1,
    crate::interrupt::ADC1,
    1
);

/// Number of ADC instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 2;
