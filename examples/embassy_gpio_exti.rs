#![no_std]
#![no_main]

//! Async pin interrupt sample.
//!
//! The button is polled by nobody: `wait_for_falling_edge` parks the task until
//! the GPIO bank interrupt fires, which is the point of the async flavour.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::gpio::{Input, Output, OutputConfig, Pull};

#[embassy_executor::task]
async fn mirror(
    button: Input<'static, hal::gpio::PA0, hal::mode::Async>,
    mut led: Output<'static, hal::gpio::PA5>,
) {
    let mut button = button;
    loop {
        button.wait_for_falling_edge().await;
        led.set_high();
        button.wait_for_rising_edge().await;
        led.set_low();
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = hal::init(Default::default());

    spawner
        .spawn(mirror(
            Input::new_async(p.PA0, Pull::Up),
            Output::new(p.PA5, OutputConfig::default()),
        ))
        .unwrap();
}
