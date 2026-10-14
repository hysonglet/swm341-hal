#![no_std]
#![no_main]

//! Blocking ADC sample: convert channel 0 and print the 12-bit result over RTT.
//!
//! The analog pad is routed through [`swm341_hal::adc::pins`] (function code `15`,
//! applied with [`swm341_hal::gpio::set_function`]) before this runs.

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

    rprintln!("adc_block: started, reading ADC0 channel 0");

    let adc = hal::adc::Adc::new(p.adc0(), hal::adc::Config::default());

    loop {
        let sample = adc.read(0);
        rprintln!("ADC ch0 = {} (0x{:x})", sample, sample);
        // ~1s delay at 150 MHz to avoid flooding the RTT channel.
        cortex_m::asm::delay(150_000_000);
    }
}
