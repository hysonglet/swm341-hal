//! Register access for the real-time clock.
//!
//! The SWM341 RTC keeps wall-clock time in BCD-ish registers: `MINSEC` (sec+min),
//! `DATHUR` (hour+date), `MONDAY` (day+month) and `YEAR` (raw). Writing the time
//! registers and then strobing `LOAD.TIME` copies them into the live counter.

use crate::clock::PeripheralClockIndex;

/// The register-block type shared by every RTC instance on the SWM341.
pub(crate) type Block = crate::pac::rtc::RegisterBlock;

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};

    /// An RTC peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        fn regs() -> *const Block;


        /// Enable the RTC counter (write the EN register's enable bit).
        #[inline(always)]
        fn enable() {
            let regs = unsafe { &*Self::regs() };
            regs.en.write(|w| unsafe { w.bits(1) });
        }
        /// Disable the RTC counter.
        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.en.write(|w| unsafe { w.bits(0) });
        }
        /// `true` while the RTC counter is running.
        #[inline(always)]
        fn is_enabled() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.en.read().bits() & 1 != 0
        }

        /// Copy the time registers that were just written into the live counter.
        #[inline(always)]
        fn load_time() {
            let regs = unsafe { &*Self::regs() };
            regs.load.write(|w| w.time().set_bit());
        }
        /// Copy the alarm registers that were just written into the alarm comparator.
        #[inline(always)]
        fn load_alarm() {
            let regs = unsafe { &*Self::regs() };
            regs.load.write(|w| w.alarm().set_bit());
        }

        #[inline(always)]
        fn write_time(year: u32, sec: u8, min: u8, hour: u8, date: u8, month: u8, weekday: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.year.write(|w| unsafe { w.bits(year) });
            regs.minsec
                .write(|w| unsafe { w.sec().bits(sec).min().bits(min) });
            regs.dathur
                .write(|w| unsafe { w.hour().bits(hour).date().bits(date) });
            regs.monday
                .write(|w| unsafe { w.day().bits(weekday).mon().bits(month) });
        }

        /// Read the live counter back into a
        /// `(year, sec, min, hour, date, month, weekday)` tuple.
        #[inline(always)]
        fn read_time() -> (u32, u8, u8, u8, u8, u8, u8) {
            let regs = unsafe { &*Self::regs() };
            let year = regs.year.read().bits();
            let ms = regs.minsec.read();
            let dh = regs.dathur.read();
            let md = regs.monday.read();
            (
                year,
                ms.sec().bits(),
                ms.min().bits(),
                dh.hour().bits(),
                dh.date().bits(),
                md.mon().bits(),
                md.day().bits(),
            )
        }

        /// Program the alarm comparator. `weekday_mask` has one bit per weekday
        /// (bit0 = SUN … bit6 = SAT) selecting which weekdays the alarm matches.
        #[inline(always)]
        fn write_alarm(sec: u8, min: u8, hour: u8, weekday_mask: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.minsecal
                .write(|w| unsafe { w.sec().bits(sec).min().bits(min) });
            // DAYHURAL holds HOUR (bits 0:4) and the weekday-match bits SUN..SAT
            // (bits 5:11); there is no date field, so write it as raw bits.
            regs.dayhural
                .write(|w| unsafe { w.bits((hour as u32) | ((weekday_mask as u32) << 5)) });
        }

        #[inline(always)]
        fn enable_alarm_interrupt() {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.alarm().set_bit());
        }
        #[inline(always)]
        fn disable_alarm_interrupt() {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.alarm().clear_bit());
        }
        #[inline(always)]
        fn is_alarm_flag() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().alarm().bit_is_set()
        }
        #[inline(always)]
        fn clear_alarm_flag() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.alarm().set_bit());
        }

        #[inline(always)]
        fn set_trim(trim: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.trim.write(|w| unsafe { w.bits(trim as u32) });
        }
    }
}

#[allow(private_bounds)]
/// An RTC peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_rtc_instance {
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
            fn irq() -> crate::interrupt::ExtendedInterrupt {
                crate::interrupt::$irq
            }
            #[inline]
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_rtc_instance!(crate::Rtc, RTC, RTC, Rtc, 0);
