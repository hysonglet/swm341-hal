#![no_std]
#![no_main]

//! Blocking comparator sample: enable CMP0 and poll its output over RTT.
//!
//! CMP0's positive input is PA2 (`ACMP0_INP`) and its negative input is PB12
//! (`ACMP0_INN`); both are routed through the type-checked
//! [`swm341_hal::cmp::Cmp::set_positive_pin`] / `set_negative_pin` API. The
//! output toggles as the positive input crosses the negative input threshold.

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

    let mut cmp = hal::cmp::Cmp::new(
        p.cmp0(),
        hal::cmp::Config {
            enable: true,
            hysteresis: true,
            ..Default::default()
        },
    );

    // PA2 -> ACMP0 positive input, PB12 -> ACMP0 negative input. The handles
    // keep both pads routed for as long as they are alive.
    let _pos = cmp.set_positive_pin(p.PA2);
    let _neg = cmp.set_negative_pin(p.PB12);

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
