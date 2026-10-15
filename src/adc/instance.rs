//! Register access for the ADC blocks.
//!
//! The two ADC instances (`ADC0`, `ADC1`) share the `pac::adc0::RegisterBlock`
//! type -- `ADC1` is a `pub use adc0 as adc1` alias in the PAC -- so every
//! register access is written once, as a default method of [`sealed::Instance`].
//! The instances differ only in their clock gate (`Adc0` vs `Adc1`) and NVIC
//! vector, which is why the instance macro takes those as arguments.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every ADC instance on the SWM341.
pub(crate) type Block = pac::adc0::RegisterBlock;

pub(crate) mod sealed {
    use cortex_m::asm::nop;
    use cortex_m::peripheral::NVIC;

    use super::{Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// An ADC peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. Because both converters
    /// share one register-block layout, the register access lives directly in
    /// these default methods; everything that actually differs per instance --
    /// the clock gate, the NVIC vector, and which `SYS.CLKSEL` divider pair the
    /// converter uses -- is reachable from [`clock`](Self::clock),
    /// [`irq`](Self::irq) and [`regs`](Self::regs).
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

        /// Power on and configure the converter.
        ///
        /// Mirrors the vendor `ADC_Init` + `ADC_Open` + `ADC_Calibrate` flow:
        ///  1. reset the converter (CR.RESET pulse),
        ///  2. clear the data FIFO (CR.FFCLR pulse),
        ///  3. disable interrupts and clear all flags,
        ///  4. power on the internal analog LDO and let it settle,
        ///  5. assert CR.EN, then calibrate.
        ///
        /// Skipping the reset/calibration (or running the ADC clock at full
        /// speed) leaves the SAR state machine unable to finish a conversion, so
        /// the EOC flag never latches and a blocking read spins forever.
        fn enable() {
            let regs = unsafe { &*Self::regs() };

            // Select HRC / 8 as the ADC clock so the converter runs within spec
            // (the reset default of AD0DIV = 0 would clock it at the full 150 MHz).
            // `clock()` tells us which converter this is: `SYS.CLKSEL` carries an
            // independent `AD0/AD0DIV` and `AD1/AD1DIV` pair, so bringing one up
            // must not touch the other's fields.
            Self::clock().open(); 

            // 1. Reset the digital core.
            regs.cr.modify(|_, w| w.reset().set_bit());
            for _ in 0..(120 * 20) {
                nop();
            }
            regs.cr.modify(|_, w| w.reset().clear_bit());

            // 2. Clear the conversion FIFO.
            Self::flush_fifo();

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
            Self::calibrate();
        }

        /// Run the one-time factory calibration required before the first
        /// conversion.
        ///
        /// The vendor driver calls this right after enabling the converter;
        /// without it the SAR never settles and `EOC` never latches.
        fn calibrate() {
            let regs = unsafe { &*Self::regs() };
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

        /// Select the trigger source for sequence 0 (`1` = software start via `GO`).
        ///
        /// Also arms a single conversion per start (`SEQCOV.seq0 = 0`).
        #[inline(always)]
        fn set_trigger(src: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.seqtrg.modify(|_, w| unsafe { w.seq0().bits(src) });
            regs.seqcov.modify(|_, w| unsafe { w.seq0().bits(0) });
        }

        /// Program the sampling-time register (`SEQ_SMP`).
        #[inline(always)]
        fn set_sampling(sampling: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.seqsmp.write(|w| unsafe { w.bits(sampling as u32) });
        }

        /// Route `channel` to the first slot of conversion sequence 0 (`SEQCHN0.SEQ0`).
        ///
        /// `SEQCHN0.SEQ0` is a *bitmask* of channels (bit `n` = channel `n`), not
        /// a channel number, so channel `n` is selected by setting bit `n`.
        #[inline(always)]
        fn select_channel(channel: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.seqchn0
                .modify(|_, w| unsafe { w.seq0().bits(1u16 << channel) });
        }

        /// Discard any result left in sequence 0's FIFO.
        ///
        /// `CR.FFCLR` is a pulse register (one bit per sequence); bit 0 covers
        /// sequence 0. Used before arming a conversion so a leftover entry -- e.g.
        /// from an async conversion that was cancelled after it had already
        /// started -- cannot be mistaken for the new result.
        #[inline(always)]
        fn flush_fifo() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| unsafe { w.ffclr().bits(0x1) });
            regs.cr.modify(|_, w| unsafe { w.ffclr().bits(0) });
        }

        /// Kick off sequence 0 (`GO.SEQ0`).
        #[inline(always)]
        fn start_conversion() {
            let regs = unsafe { &*Self::regs() };
            regs.go.modify(|_, w| w.seq0().set_bit());
        }

        /// True when sequence 0's FIFO holds at least one result (`!SEQ0.SR.EMPTY`).
        ///
        /// This is the flag the vendor library polls (`ADC_DataAvailable`). Unlike
        /// `SR.EOC` it is not sticky, so polling it needs no "wait for the start
        /// to clear the previous flag" delay.
        #[inline(always)]
        fn data_available() -> bool {
            let regs = unsafe { &*Self::regs() };
            !regs.seq0.sr.read().empty().bit()
        }

        /// Pop the next result of sequence 0 as `(channel, value)`.
        ///
        /// The hardware tags every FIFO entry with its channel (`DR.CHNUM`), so
        /// callers must check it: a sequence can legitimately hold results of
        /// several channels.
        #[inline(always)]
        fn result() -> (u8, u16) {
            let regs = unsafe { &*Self::regs() };
            let dr = regs.seq0.dr.read();
            (dr.chnum().bits(), dr.value().bits())
        }

        /// True once sequence 0 has finished (`IF.SEQ0EOC`).
        #[inline(always)]
        fn eoc_flag() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().seq0eoc().bit()
        }

        /// Clear the sequence-0 end-of-conversion flag (write-1-to-clear).
        #[inline(always)]
        fn clear_eoc() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.seq0eoc().set_bit());
        }

        /// Enable or disable the end-of-conversion interrupt (`IE.SEQ0EOC`).
        #[inline(always)]
        fn set_eoc_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.seq0eoc().bit(enable));
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
