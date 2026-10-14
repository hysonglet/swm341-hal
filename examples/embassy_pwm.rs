#![no_std]
#![no_main]

//! Async PWM sample: start the output, then await each period boundary.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let pwm = hal::pwm::Pwm::new_async(
        p.pwm0(),
        hal::pwm::Config::default().period(1000).duty_a(250),
    );
    pwm.start();
    pwm.wait_period().await;

    loop {}
}
