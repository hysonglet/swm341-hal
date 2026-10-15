#![no_std]
#![no_main]

//! Async pin interrupt sample.
//!
//! The button is polled by nobody: `wait_for_falling_edge` parks the task until
//! the GPIO bank interrupt fires, which is the point of the async flavour.

use embassy_executor::Spawner;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;
use swm341_hal::gpio::{Input, Output, OutputConfig, Pull};

#[embassy_executor::task]
async fn mirror(
    button: Input<'static, hal::gpio::PC0, hal::mode::Async>,
    mut led: Output<'static, hal::gpio::PC1>,
) {
    let mut button = button;
    let mut edges = 0u32;
    loop {
        button.wait_for_falling_edge().await;
        led.set_high();
        rprintln!("button pressed (LED on), edges={}", edges);
        button.wait_for_rising_edge().await;
        led.set_low();
        edges += 1;
        rprintln!("button released (LED off), edges={}", edges);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    rtt_init_print!();

    let p = hal::init(Default::default());

    rprintln!("gpio_exti: spawning mirror task");

    spawner
        .spawn(mirror(
            Input::new_async(p.PC0, Pull::Up),
            Output::new(p.PC1, OutputConfig::default()),
        ))
        .unwrap();
}
