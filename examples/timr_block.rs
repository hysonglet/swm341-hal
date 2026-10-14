#![no_std]
#![no_main]

//! Blocking TIMR sample: busy-wait one second with TIMR0.
//!
//! TIMR has no mandatory pads (external clock input / compare output roles live in
//! [`swm341_hal::timr::pins`] and are applied with [`swm341_hal::gpio::set_function`]
//! only when a pad is actually used).

use panic_halt as _;

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // TIMR0 is clocked from the 150 MHz peripheral clock; a period of 150_000_000
    // counts down in ~1 second with the default (no-prescale) config.
    let timer = hal::timr::Timr::new(p.timr0(), hal::timr::Config::default());
    timer.start(150_000_000);
    while timer.value() > 0 {}

    loop {}
}
