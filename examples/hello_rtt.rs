#![no_std]
#![no_main]

//! Minimal `Hello, world!` sample that prints over RTT (Segger RTT / `rtt_target`).

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // Initialize the HAL (clocks, peripherals). The clock config is required by
    // `hal::init`; the exact frequency does not matter for RTT output.
    let _p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    rprintln!("Hello, world!");

    let mut counter: u32 = 0;
    loop {
        counter += 1;
        rprintln!("tick {}", counter);
        // Rough ~1s delay (150 MHz core): 150_000_000 cycles.
        cortex_m::asm::delay(150_000_000);
    }
}
