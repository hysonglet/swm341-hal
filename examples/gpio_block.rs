#![no_std]
#![no_main]

//! Blocking GPIO sample: read a button and follow it with an LED, logging state
//! changes over RTT.
//!
//! Wiring matches the vendor `GPIO/KeyLED` example: a button on PA2 (pull-up,
//! pressed = low) and an LED on PB15 to ground.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let button = hal::gpio::Input::new(p.PA2, hal::gpio::Pull::Up);
    let mut led = hal::gpio::Output::new(
        p.PB15,
        hal::gpio::OutputConfig::default().initial_level(hal::gpio::Level::Low),
    );

    rprintln!("gpio_block: started, watching PA2 button");

    let mut prev_pressed = false;
    loop {
        // Active low: the LED mirrors the button.
        let pressed = button.is_low();
        if pressed {
            led.set_high();
        } else {
            led.set_low();
        }

        // Log only on state changes to avoid flooding the RTT channel.
        if pressed != prev_pressed {
            if pressed {
                rprintln!("button pressed -> LED on");
            } else {
                rprintln!("button released -> LED off");
            }
            prev_pressed = pressed;
        }
    }
}
