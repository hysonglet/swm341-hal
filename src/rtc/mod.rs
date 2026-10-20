//! Real-time clock (RTC) driver.
//!
//! The [`Rtc`] object enables the counter, lets you set/get the wall-clock time,
//! and supports an alarm comparator that can raise an interrupt. After writing
//! the time (or alarm) registers, strobe the matching load bit so the hardware
//! latches the new value.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::rtc::{Rtc, Datetime};
//! let mut rtc = Rtc::new(p.rtc(), Default::default());
//! rtc.set_time(Datetime { year: 2024, month: 1, date: 1, weekday: 1, hour: 12, min: 0, sec: 0 });
//! let now = rtc.get_time();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

pub mod instance;
mod types;

pub use types::*;

/// An RTC instance.
#[allow(dead_code)]
pub struct Rtc<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Rtc<'d, T, M> {
    /// Take ownership of the RTC and start its counter.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::enable();
        if config.trim != 0 {
            T::set_trim(config.trim);
        }
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// `true` while the counter is running.
    pub fn is_running(&self) -> bool {
        T::is_enabled()
    }
}

impl<'d, T: Instance, M: Mode> Drop for Rtc<'d, T, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
    }
}

impl<'d, T: Instance> Rtc<'d, T, Blocking> {
    /// Latch `time` into the live counter.
    pub fn set_time(&mut self, time: Datetime) {
        T::write_time(
            time.year,
            time.sec,
            time.min,
            time.hour,
            time.date,
            time.month,
            time.weekday,
        );
        T::load_time();
    }

    /// Read the live counter.
    pub fn get_time(&self) -> Datetime {
        let (year, sec, min, hour, date, month, weekday) = T::read_time();
        Datetime {
            year,
            month,
            date,
            weekday,
            hour,
            min,
            sec,
        }
    }

    /// Program the alarm comparator. `weekday_mask` has one bit per weekday
    /// (bit0 = Sunday … bit6 = Saturday); set a bit to make the alarm fire on
    /// that weekday. When `weekday_mask` is 0 the alarm fires daily.
    pub fn set_alarm(&mut self, time: Datetime, weekday_mask: u8) {
        T::write_alarm(time.sec, time.min, time.hour, weekday_mask);
        T::load_alarm();
    }

    /// Arm the alarm interrupt.
    pub fn enable_alarm_interrupt(&mut self) {
        T::enable_alarm_interrupt();
    }

    /// Disarm the alarm interrupt.
    pub fn disable_alarm_interrupt(&mut self) {
        T::disable_alarm_interrupt();
    }

    /// `true` if the alarm comparator matched since the last clear.
    pub fn is_alarm_flag(&self) -> bool {
        T::is_alarm_flag()
    }

    /// Clear the alarm interrupt flag (write 1 to clear).
    pub fn clear_alarm_flag(&mut self) {
        T::clear_alarm_flag();
    }
}
