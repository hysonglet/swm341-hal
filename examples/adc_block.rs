#![no_std]
#![no_main]

//! Blocking ADC sample: convert channel 0 and print the 12-bit result over RTT.
//!
//! `ADC0` channel 0 is wired to `PC6`. The pin is configured through the typed
//! [`swm341_hal::adc::Adc::add_pin`] API, which routes the pad to the analog mux
//! (function code `15`) and remembers the channel it maps to.

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

    let mut adc = hal::adc::Adc::new(p.adc0(), hal::adc::Config::default());

    // Route PC0 to the analog mux; `add_pin` records that PC0 is ADC0 channel 0.
    let pc0 = adc.add_pin(p.PC0);
    // Route PC1 to the analog mux; `add_pin` records that PC1 is ADC0 channel 1.
    let pc1 = adc.add_pin(p.PC1);

    loop {
        rprintln!("start");
        let pc0_sample = adc.read_input(&pc0);
        let pc1_sample = adc.read_input(&pc1);

        rprintln!(
            "ADC ch{} = {} (0x{:x}) ch{} = {} (0x{:x})",
            pc0.channel,
            pc0_sample,
            pc0_sample,
            pc1.channel,
            pc1_sample,
            pc1_sample
        );
        // ~1s delay at 150 MHz to avoid flooding the RTT channel.
        cortex_m::asm::delay(150_000_000);
    }
}
