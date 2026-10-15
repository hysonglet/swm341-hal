#![no_std]
#![no_main]

//! Async ADC sample: await the end-of-conversion interrupt on channel 0.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // `read_async` takes `&mut self`: the channel mask and the sequence FIFO are
    // per-instance state, so concurrent readers would otherwise mix channels.
    let mut adc = hal::adc::Adc::new_async(p.adc0(), hal::adc::Config::default());
    let sample = adc.read_async(0).await;
    let _ = sample;

    loop {}
}
