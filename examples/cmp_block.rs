#![no_std]
#![no_main]

//! Blocking comparator sample: enable CMP0 and poll its output over RTT.
//!
//! CMP0's positive input is the `CMP0+` pad and its negative input is the
//! `CMP0−` pad (or the internal reference when [`NegativeSource::InternalVref`]
//! is selected). Wire the pads appropriately on the board to see the output
//! toggle as the positive input crosses the negative input threshold.

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

    rprintln!("cmp_block: started, polling CMP0 output");

    let cmp = hal::cmp::Cmp::new(
        p.cmp0(),
        hal::cmp::Config {
            enable: true,
            hysteresis: true,
            ..Default::default()
        },
    );

    let mut prev = cmp.output();
    rprintln!("cmp_block: initial CMP0 output = {}", prev as u8);

    loop {
        let out = cmp.output();
        if out != prev {
            rprintln!("cmp_block: CMP0 output changed -> {}", out as u8);
            prev = out;
        }
        // ~100ms delay at 150 MHz to avoid flooding the RTT channel.
        cortex_m::asm::delay(15_000_000);
    }
}
