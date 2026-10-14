#![no_std]
#![no_main]

//! Blocking ADC sample: convert channel 0 and discard the 12-bit result.
//!
//! The analog pad is routed through [`swm341_hal::adc::pins`] (function code `15`,
//! applied with [`swm341_hal::gpio::set_function`]) before this runs.

use panic_halt as _;

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let adc = hal::adc::Adc::new(p.adc0(), hal::adc::Config::default());
    let sample = adc.read(0);
    let _ = sample;

    loop {}
}
