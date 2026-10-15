#![no_std]
#![no_main]

//! Async GPIO sample: blink an LED and count button presses.

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;
use swm341_hal::gpio::{Input, Level, Output, OutputConfig, Pull};

#[embassy_executor::task]
async fn blink(mut led: Output<'static, hal::gpio::PC1>) {
    let mut toggles = 0u32;
    loop {
        led.set_high();
        Timer::after(Duration::from_millis(250)).await;
        led.set_low();
        Timer::after(Duration::from_millis(250)).await;
        toggles += 1;
        rprintln!("blink toggled {} times", toggles);
    }
}

#[embassy_executor::task]
async fn count_presses(mut button: Input<'static, hal::gpio::PC0, hal::mode::Async>) {
    let mut presses = 0u32;
    loop {
        // Wait for the release so a single press is counted once.
        button.wait_for_falling_edge().await;
        button.wait_for_rising_edge().await;
        presses += 1;
        rprintln!("button pressed {} times", presses);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    rprintln!("gpio_blink: spawning tasks");

    spawner
        .spawn(blink(Output::new(
            p.PC1,
            OutputConfig::default().initial_level(Level::Low),
        )))
        .unwrap();
    spawner
        .spawn(count_presses(Input::new_async(p.PC0, Pull::Up)))
        .unwrap();
}
