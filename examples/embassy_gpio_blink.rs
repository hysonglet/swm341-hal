#![no_std]
#![no_main]

//! Async GPIO sample: blink an LED and count button presses.

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::gpio::{Input, Level, Output, OutputConfig, Pull};

#[embassy_executor::task]
async fn blink(mut led: Output<'static, hal::gpio::PA5>) {
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(250)).await;
        led.set_low();
        Timer::after(Duration::from_millis(250)).await;
    }
}

#[embassy_executor::task]
async fn count_presses(mut button: Input<'static, hal::gpio::PA0, hal::mode::Async>) {
    let mut presses = 0u32;
    loop {
        // Wait for the release so a single press is counted once.
        button.wait_for_falling_edge().await;
        button.wait_for_rising_edge().await;
        presses += 1;
        let _ = presses;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    spawner
        .spawn(blink(Output::new(
            p.PA5,
            OutputConfig::default().initial_level(Level::Low),
        )))
        .unwrap();
    spawner
        .spawn(count_presses(Input::new_async(p.PA0, Pull::Up)))
        .unwrap();
}
