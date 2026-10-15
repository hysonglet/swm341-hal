#![no_std]
#![no_main]

//! Blocking RTC example: set the wall-clock time and read it back.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;
use hal::rtc::{Datetime, Rtc};

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    rprintln!("rtc_block: started, setting time to 2024-01-01 12:00:00");

    let mut rtc = Rtc::new(p.rtc(), Default::default());
    rtc.set_time(Datetime {
        year: 2026,
        month: 10,
        date: 16,
        weekday: 5,
        hour: 12,
        min: 0,
        sec: 0,
    });
    let now = rtc.get_time();
    rprintln!(
        "rtc_block: read back time {:04}-{:02}-{:02} (weekday {}) {:02}:{:02}:{:02}",
        now.year,
        now.month,
        now.date,
        now.weekday,
        now.hour,
        now.min,
        now.sec
    );

    loop {
        // Empirically measured: ~50M cycles = 1s, so use 150M/3.
        cortex_m::asm::delay(150_000_000 / 3);
        let now = rtc.get_time();
        rprintln!(
            "rtc_block: {:04}-{:02}-{:02} (weekday {}) {:02}:{:02}:{:02}",
            now.year,
            now.month,
            now.date,
            now.weekday,
            now.hour,
            now.min,
            now.sec
        );
    }
}
