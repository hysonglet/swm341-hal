#![no_std]
#![no_main]

//! Blocking DAC sample: drive `PD2` with a triangle ramp and print the code.
//!
//! The DAC output pad is `PD2`; [`swm341_hal::dac::Dac::add_pin`] routes it to
//! `DAC_OUT` (function code `15`) so the converted voltage leaves the chip.

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

    rprintln!("dac_block: started, driving PD2 (DAC_OUT)");

    let mut dac = hal::dac::Dac::new(p.dac(), hal::dac::Config::default());
    dac.add_pin(p.PD2);

    // Triangle ramp over the 12-bit range; with the 3.3 V full scale each step
    // is ~200 mV.
    let mut rising = true;
    let mut v: u16 = 0;
    loop {
        dac.write(v);
        rprintln!("DAC = {} (0x{:x})  ~{} mV", v, v, v as u32 * 3300 / 4095);

        if rising {
            v += 256;
            if v >= 4095 {
                v = 4095;
                rising = false;
            }
        } else {
            v = v.saturating_sub(256);
            if v == 0 {
                rising = true;
            }
        }

        // ~33 ms delay at 150 MHz to keep the RTT channel readable.
        cortex_m::asm::delay(5_000_000);
    }
}
