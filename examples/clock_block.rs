#![no_std]
#![no_main]

//! Print the core clock and the available clock sources.
//!
//! The SWM341 has no independent AHB/APB prescaler: a single system clock
//! drives the CPU and every peripheral bus, so the "bus clock" equals the core
//! (system) clock. This example reports the core frequency through both the
//! value the HAL cached at [`crate::init`] and the value read back from the
//! hardware, plus the frequencies of the oscillators the tree can select from.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use swm341_hal::clock::{
    self, HRC_20MHZ, HRC_40MHZ, LSE_HZ, LSI_HZ, MAX_SYSTEM_CLOCK_HZ,
};

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // Bring the chip up at 150 MHz from the internal RC (the datasheet maximum).
    let config = hal::Config::hrc_150mhz();
    let _p = hal::init(config);

    rprintln!("==== SWM341 clock report ====");

    // Core clock the HAL believes is running, set by `hal::init`.
    let cached = clock::sys_core_clock();
    // Core clock read back from the hardware registers.
    let readback = clock::current_system_clock();

    rprintln!("Core (system) clock from HAL cache : {} Hz", cached);
    match readback {
        Some(hz) => rprintln!("Core (system) clock from hardware : {} Hz", hz),
        None => rprintln!("Core (system) clock from hardware : unknown (external source)"),
    }

    // On the SWM341 the system clock feeds the CPU and every peripheral bus
    // directly, so there is no separate AHB/APB divider.
    rprintln!("CPU / AHB bus clock             : {} Hz", cached);
    rprintln!("Peripheral (APB) bus clock      : {} Hz", cached);

    // Clock sources the tree can be built from.
    rprintln!("---- available clock sources ----");
    rprintln!("Internal RC (low gear)  HRC 20 : {} Hz", HRC_20MHZ);
    rprintln!("Internal RC (high gear) HRC 40 : {} Hz", HRC_40MHZ);
    rprintln!("Internal low-speed RC  LSI     : {} Hz", LSI_HZ);
    rprintln!("External 32.768k crystal LSE    : {} Hz", LSE_HZ);
    rprintln!(
        "External high-speed crystal XTAL : {} Hz (default fitted crystal)",
        config.clock.xtal_frequency_hz
    );

    // What the current configuration resolves to.
    rprintln!("---- current configuration ----");
    rprintln!("HRC gear                        : {:?}", config.clock.hrc);
    rprintln!(
        "Core frequency (computed)        : {} Hz",
        config.clock.core_frequency_hz()
    );
    rprintln!("Clock source                    : {:?}", config.clock.source);
    rprintln!("Needs external crystal          : {}", config.clock.needs_xtal());

    rprintln!(
        "Maximum guaranteed core clock    : {} Hz",
        MAX_SYSTEM_CLOCK_HZ
    );

    rprintln!("==== end of report ====");

    loop {
        cortex_m::asm::wfi();
    }
}
