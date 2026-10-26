#![no_std]
#![no_main]

//! Blocking SDRAM sample: route the bus and read/write the mapped window.
//!
//! The pads are muxed through [`swm341_hal::sdramc::SdramPins`] (wired like the
//! vendor `SimplSDRAM` example) and the chip is brought up with
//! [`swm341_hal::sdramc::Config::default`]. After init the whole capacity is
//! mapped at [`swm341_hal::sdramc::SDRAM_BASE`]; here we write a word and read
//! it back to prove the bus is alive. Progress and results are printed over RTT.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;
use hal::sdramc::{Config, SdramPins, Sdramc};

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });
    rprintln!("HAL initialised");

    let sdram = Sdramc::new_with_pins(
        p.sdramc(),
        SdramPins::default(),
        Config::default(),
    );

    let base = sdram.base_address();
    let size = sdram.size_bytes();
    assert!(size >= 4);
    rprintln!(
        "SDRAM up: {:>9} bytes mapped at 0x{:08X}",
        size,
        base
    );

    // Write a word at offset 0 and read it back across the AHB window.
    let w = unsafe { &mut *(base as *mut u32) };
    let probe: u32 = 0xDEAD_BEEF;
    *w = probe;
    let read_back = *w;
    rprintln!(
        "wrote 0x{:08X}, read back 0x{:08X}",
        probe,
        read_back
    );
    assert_eq!(read_back, probe);
    rprintln!("SDRAM OK");

    loop {}
}
