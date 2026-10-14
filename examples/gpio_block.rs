#![no_std]
#![no_main]

//! Blocking GPIO sample: read a button and follow it with an LED.
//!
//! Wiring matches the vendor `GPIO/KeyLED` example: a button on PA0 (pull-up,
//! pressed = low) and an LED on PA5 to ground.

use panic_halt as _;

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let button = hal::gpio::Input::new(p.PA0, hal::gpio::Pull::Up);
    let mut led = hal::gpio::Output::new(
        p.PA5,
        hal::gpio::OutputConfig::default().initial_level(hal::gpio::Level::Low),
    );

    loop {
        // Active low: the LED mirrors the button.
        if button.is_low() {
            led.set_high();
        } else {
            led.set_low();
        }
    }
}
