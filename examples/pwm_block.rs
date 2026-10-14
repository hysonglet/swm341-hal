#![no_std]
#![no_main]

//! Blocking PWM sample: drive the `A` output at ~1 kHz with 25% duty.
//!
//! The output pad is muxed through [`swm341_hal::pwm::pins`] (applied with
//! [`swm341_hal::gpio::set_function`]) before this runs; the driver only
//! programs the counter / compare values.

use panic_halt as _;

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let pwm = hal::pwm::Pwm::new(
        p.pwm0(),
        hal::pwm::Config::default().period(1000).duty_a(250),
    );
    pwm.start();

    loop {}
}
