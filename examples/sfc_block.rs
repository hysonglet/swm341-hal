#![no_std]
#![no_main]

//! Blocking SFC sample: erase, program and read back the on-board SPI NOR.
//!
//! A NOR flash only ever clears bits (`1 -> 0`), so the three steps have to
//! happen in this order:
//!
//! 1. [`Sfc::erase_sector`] -- 4 KB, leaving every byte at `0xFF`;
//! 2. [`Sfc::program`] -- one 256-byte page at most, which is why a range that
//!    crosses a page boundary comes back as [`Error::PageCrossing`] instead of
//!    silently wrapping around inside the page;
//! 3. [`Sfc::write`] -- the same thing split over as many pages as it takes.
//!
//! Every step is read back through the `0x7000_0000` AHB window and compared.
//! The six Flash-SPI pads are handed over explicitly through [`SfcPins`];
//! `Sfc::new` would route the same ones by itself.
//!
//! The scratch area is the 4 KB sector at 64 KB: high enough to stay clear of
//! anything a bootloader could be fetching through the same window -- it is the
//! chip's XIP path -- and far below the 2 MB the vendor `SFC_FATFS` demo
//! formats. Change it if your board keeps something there.
//!
//! Run with `cargo run --example sfc_block`.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use hal::sfc::{Config, Error, Sfc, SfcPins, CMD_ERASE_SECTOR, PAGE_SIZE, SECTOR_SIZE};
use swm341_hal as hal;

/// Scratch sector: 64 KB into the flash.
const ADDR: u32 = 0x0001_0000;

/// Fill `buf` with a recognisable, position-dependent pattern.
fn fill(buf: &mut [u8], seed: u8) {
    for (i, byte) in buf.iter_mut().enumerate() {
        *byte = seed.wrapping_add(i as u8);
    }
}

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // The dedicated Flash-SPI pads, D3..D8 of bank D. `Sfc::new(p.sfc())`
    // routes the same six by itself; spelling them out makes the ownership
    // visible.
    let sfc = Sfc::new_with_pins(
        p.sfc(),
        SfcPins {
            d2: p.PD3,
            d3: p.PD4,
            sclk: p.PD5,
            ssel: p.PD6,
            miso: p.PD7,
            mosi: p.PD8,
        },
        Config::standard(),
    );

    let id = sfc.read_jedec_id();
    rprintln!(
        "JEDEC ID    = 0x{:02X} 0x{:02X} 0x{:02X}, capacity {:?} bytes",
        id.manufacturer,
        id.memory_type,
        id.capacity,
        id.capacity_bytes()
    );
    rprintln!(
        "status      = SR1 0x{:02X}, SR2 0x{:02X}, quad = {}",
        sfc.status1(),
        sfc.status2(),
        sfc.quad_enabled()
    );

    // --- 1. erase ----------------------------------------------------------
    let mut before = [0u8; 16];
    sfc.read(ADDR, &mut before).unwrap();
    rprintln!("before      {:02x?}", &before[..]);

    sfc.erase_sector(ADDR).unwrap();
    let mut erased = [0u8; 16];
    sfc.read(ADDR, &mut erased).unwrap();
    rprintln!("after erase {:02x?}", &erased[..]);
    assert!(
        erased.iter().all(|byte| *byte == 0xFF),
        "erase did not leave the sector at 0xFF"
    );

    // --- 2. program, one page ----------------------------------------------
    let mut page = [0u8; 16];
    fill(&mut page, 0x40);
    sfc.program(ADDR, &page).unwrap();

    let mut single = [0u8; 16];
    sfc.read(ADDR, &mut single).unwrap();
    rprintln!("program     {:02x?}", &single[..]);
    assert_eq!(single, page);

    // --- 3. crossing a page boundary ----------------------------------------
    // Start six bytes short of the next 256-byte page: `program` refuses, and
    // `write` splits the transfer into two page programs instead.
    let mut long = [0u8; 300];
    fill(&mut long, 0x80);
    let crossing = ADDR + PAGE_SIZE - 6;

    assert_eq!(sfc.program(crossing, &long), Err(Error::PageCrossing));
    rprintln!("program     {:?}, as expected", Error::PageCrossing);

    sfc.write(crossing, &long).unwrap();
    let mut split = [0u8; 300];
    sfc.read(crossing, &mut split).unwrap();
    rprintln!(
        "write       {:02x?} ... ({} bytes over two pages)",
        &split[..16],
        split.len()
    );
    assert_eq!(split, long);

    // --- 4. erase without waiting -------------------------------------------
    // `erase_with` returns as soon as the command is on the wire, so the caller
    // decides how long to wait: useful when a chip erase would outlast the
    // default poll budget.
    let other = ADDR + SECTOR_SIZE;
    sfc.erase_with(other, CMD_ERASE_SECTOR, false).unwrap();
    rprintln!("erase_with  issued, busy = {}", sfc.is_busy());
    sfc.wait_ready().unwrap();

    let mut check = [0u8; 16];
    sfc.read(other, &mut check).unwrap();
    assert!(check.iter().all(|byte| *byte == 0xFF));

    rprintln!("flags       {:?}", sfc.flags());
    rprintln!("sfc_block: OK");

    loop {}
}
