#![no_std]
#![no_main]

//! Async TIMR sample: wait one second without busy-spinning.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut timer = hal::timr::Timr::new_async(p.timr0(), hal::timr::Config::default());
    timer.delay(150_000_000).await;

    loop {}
}
