#![no_std]
#![no_main]

//! Blocking RTC example: set the wall-clock time and read it back.

use panic_halt as _;

use swm341_hal as hal;
use hal::rtc::{Datetime, Rtc};

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let mut rtc = Rtc::new(p.rtc(), Default::default());
    rtc.set_time(Datetime {
        year: 2024,
        month: 1,
        date: 1,
        weekday: 1,
        hour: 12,
        min: 0,
        sec: 0,
    });
    let now = rtc.get_time();
    let _ = now;

    loop {}
}
