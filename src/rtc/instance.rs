//! Register access for the real-time clock.
//!
//! The SWM341 RTC keeps wall-clock time in BCD-ish registers: `MINSEC` (sec+min),
//! `DATHUR` (hour+date), `MONDAY` (day+month) and `YEAR` (raw). Writing the time
//! registers and then strobing `LOAD.TIME` copies them into the live counter.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every RTC instance on the SWM341.
pub(crate) type Block = pac::rtc::RegisterBlock;

/// Register access shared by every RTC instance.
pub(crate) mod hw {
    use super::Block;

    /// Enable the RTC counter (write the EN register's enable bit).
    pub(crate) fn enable(regs: &Block) {
        regs.en.write(|w| unsafe { w.bits(1) });
    }
    /// Disable the RTC counter.
    pub(crate) fn disable(regs: &Block) {
        regs.en.write(|w| unsafe { w.bits(0) });
    }
    /// `true` while the RTC counter is running.
    pub(crate) fn is_enabled(regs: &Block) -> bool {
        regs.en.read().bits() & 1 != 0
    }

    /// Copy the time registers that were just written into the live counter.
    pub(crate) fn load_time(regs: &Block) {
        regs.load.write(|w| w.time().set_bit());
    }
    /// Copy the alarm registers that were just written into the alarm comparator.
    pub(crate) fn load_alarm(regs: &Block) {
        regs.load.write(|w| w.alarm().set_bit());
    }

    pub(crate) fn write_time(
        regs: &Block,
        year: u32,
        sec: u8,
        min: u8,
        hour: u8,
        date: u8,
        month: u8,
        weekday: u8,
    ) {
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
    pub(crate) fn read_time(regs: &Block) -> (u32, u8, u8, u8, u8, u8, u8) {
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
    pub(crate) fn write_alarm(regs: &Block, sec: u8, min: u8, hour: u8, weekday_mask: u8) {
        regs.minsecal
            .write(|w| unsafe { w.sec().bits(sec).min().bits(min) });
        // DAYHURAL holds HOUR (bits 0:4) and the weekday-match bits SUN..SAT
        // (bits 5:11); there is no date field, so write it as raw bits.
        regs.dayhural
            .write(|w| unsafe { w.bits((hour as u32) | ((weekday_mask as u32) << 5)) });
    }

    pub(crate) fn enable_alarm_interrupt(regs: &Block) {
        regs.ie.modify(|_, w| w.alarm().set_bit());
    }
    pub(crate) fn disable_alarm_interrupt(regs: &Block) {
        regs.ie.modify(|_, w| w.alarm().clear_bit());
    }
    pub(crate) fn is_alarm_flag(regs: &Block) -> bool {
        regs.if_.read().alarm().bit_is_set()
    }
    pub(crate) fn clear_alarm_flag(regs: &Block) {
        regs.if_.write(|w| w.alarm().set_bit());
    }

    pub(crate) fn set_trim(regs: &Block, trim: u8) {
        regs.trim.write(|w| unsafe { w.bits(trim as u32) });
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// An RTC peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> pac::Interrupt;
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
        fn is_enabled() -> bool {
            hw::is_enabled(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn load_time() {
            hw::load_time(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn load_alarm() {
            hw::load_alarm(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn write_time(year: u32, sec: u8, min: u8, hour: u8, date: u8, month: u8, weekday: u8) {
            hw::write_time(
                unsafe { &*Self::regs() },
                year, sec, min, hour, date, month, weekday,
            );
        }
        #[inline(always)]
        fn read_time() -> (u32, u8, u8, u8, u8, u8, u8) {
            hw::read_time(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn write_alarm(sec: u8, min: u8, hour: u8, weekday_mask: u8) {
            hw::write_alarm(unsafe { &*Self::regs() }, sec, min, hour, weekday_mask);
        }
        #[inline(always)]
        fn enable_alarm_interrupt() {
            hw::enable_alarm_interrupt(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn disable_alarm_interrupt() {
            hw::disable_alarm_interrupt(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn is_alarm_flag() -> bool {
            hw::is_alarm_flag(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn clear_alarm_flag() {
            hw::clear_alarm_flag(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn set_trim(trim: u8) {
            hw::set_trim(unsafe { &*Self::regs() }, trim);
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

impl_rtc_instance!(crate::Rtc, RTC, RTC, Rtc, 0);
