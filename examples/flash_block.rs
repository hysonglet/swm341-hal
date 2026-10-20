#![no_std]
#![no_main]

//! Blocking embedded-flash (eFlash) example.
//!
//! Erases the *last* 4 KB page of the on-chip flash, programs four words into
//! it, and reads the result back. The last page is used on purpose: everything
//! below it belongs to the running firmware, and erasing that would brick the
//! board until it is re-flashed over SWD.
//!
//! Note what the datasheet requires of the program path (6.29.3): the address
//! must be 16-byte aligned and the payload a whole number of four-word groups,
//! and the page must be erased first because programming only clears bits.
//!
//! Other [`hal::flash`] APIs not exercised here: [`Flash::erase_range`],
//! [`Flash::cache_clear`] and [`Flash::status`].

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use hal::flash::{Flash, FLASH_SIZE, PAGE_SIZE};

#[entry]
fn main() -> ! {
    rtt_init_print!();

    // The flash needs no peripheral singleton, but the clock tree does have to
    // be running: `init` also retunes the flash wait states for 150 MHz.
    let _p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let flash = Flash::new();

    let addr = FLASH_SIZE - PAGE_SIZE; // 0x0007_F000, the last page
    rprintln!("flash_block: using page at {:#010X} ({} KB array)", addr, FLASH_SIZE / 1024);

    let mut before = [0u8; 16];
    flash.read(addr, &mut before).unwrap();
    rprintln!("flash_block: before        {:02x?}", &before[..]);

    // Erasing turns every bit in the page to 1.
    flash.erase_page(addr).unwrap();
    let mut erased = [0u8; 16];
    flash.read(addr, &mut erased).unwrap();
    rprintln!("flash_block: after erase   {:02x?}", &erased[..]);

    let words = [0x1234_5678, 0x9ABC_DEF0, 0x0F0F_0F0F, 0xDEAD_BEEF];
    flash.program(addr, &words).unwrap();

    let mut after = [0u8; 16];
    flash.read(addr, &mut after).unwrap();
    rprintln!("flash_block: after program {:02x?}", &after[..]);

    // The array is little-endian, so each word appears byte-reversed.
    let mut expected = [0u8; 16];
    for (i, word) in words.iter().enumerate() {
        expected[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    if after == expected {
        rprintln!("flash_block: read-back matches what was written");
    } else {
        rprintln!("flash_block: MISMATCH, expected {:02x?}", &expected[..]);
    }

    rprintln!("flash_block: done");
    loop {}
}
