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
        SdramPins {
            clk: p.PM13,
            cke: p.PM14,
            we: p.PB7,
            cas: p.PB8,
            ras: p.PB9,
            cs: p.PB10,
            ba0: p.PE15,
            ba1: p.PE14,
            ldqm: p.PB6,
            udqm: p.PM12,
            a0: p.PN14,
            a1: p.PN13,
            a2: p.PN12,
            a3: p.PN11,
            a4: p.PN10,
            a5: p.PN9,
            a6: p.PN8,
            a7: p.PN7,
            a8: p.PN6,
            a9: p.PN3,
            a10: p.PN15,
            a11: p.PN2,
            a12: p.PM15,
            d0: p.PE7,
            d1: p.PE6,
            d2: p.PE5,
            d3: p.PE4,
            d4: p.PE3,
            d5: p.PE2,
            d6: p.PE1,
            d7: p.PE0,
            d8: p.PE8,
            d9: p.PE9,
            d10: p.PE10,
            d11: p.PE11,
            d12: p.PE12,
            d13: p.PE13,
            d14: p.PC14,
            d15: p.PC15,
        },
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
