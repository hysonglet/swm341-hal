#![no_std]
#![no_main]

//! XiP test: run code straight out of the external SPI NOR flash.
//!
//! Once the SFC is up, the flash is mapped at `0x7000_0000` and the core can
//! use it like any other memory -- this is what the vendor application note
//! `数据&程序存储在SpiFlash中的Xip应用` calls XiP. Its demo1 links a `const`
//! array into the window and reads it; its demo2 links `led.c` into the same
//! window and *runs* it from there. This example does both, in eight bytes of
//! hand-written Thumb:
//!
//! * a magic word read back through the window with a plain pointer,
//! * two tiny functions called through function pointers.
//!
//! The machine code is written into the flash by this program rather than
//! linked there, because putting a Rust function at `0x7000_0000` needs a
//! custom `memory.x` plus a way to program the external flash (Keil's `.FLM`
//! algorithm in the note, §1.5). Handing the bytes to [`Sfc::write`] proves
//! the same thing -- the core fetches instructions from the window -- without
//! any of that toolchain work.
//!
//! Like the note, the controller runs in quad mode (`SFC_Config(4)`: `0xEB`
//! reads, `0x32` page programs). If your flash has no quad mode, switch to
//! [`Config::standard`]; a failed mapping is reported, not executed, so a
//! wrong configuration cannot hard-fault the board.
//!
//! Run with `cargo run --example sfc_xip`.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m::asm;
use cortex_m_rt::entry;
use hal::sfc::{Config, Sfc, SfcPins, FLASH_BASE};
use swm341_hal as hal;

/// Where in the SPI flash the tiny image is placed: 64 KB in, so nothing a
/// bootloader could be fetching is disturbed.
const IMAGE_ADDR: u32 = 0x0001_0000;

/// A magic word stored in front of the code, read back through the window.
const MAGIC: u32 = 0xDEAD_BEEF;

/// `adds r0, r1` (`0x4408`) then `bx lr` (`0x4770`).
///
/// Thumb-2 little-endian, AAPCS: returns its two arguments added together.
const ADD_FN: [u8; 4] = [0x08, 0x44, 0x70, 0x47];

/// `movs r0, #42` (`0x202A`) then `bx lr` (`0x4770`): returns 42.
const ANSWER_FN: [u8; 4] = [0x2A, 0x20, 0x70, 0x47];

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // Quad mode, exactly like the note's `SFC_Config(4)`.
    let sfc = Sfc::new_with_pins(p.sfc(), SfcPins::default(), Config::quad());

    let id = sfc.read_jedec_id();
    rprintln!(
        "JEDEC ID    = 0x{:02X} 0x{:02X} 0x{:02X}, quad = {}",
        id.manufacturer,
        id.memory_type,
        id.capacity,
        sfc.quad_enabled()
    );

    // --- build the image: magic, then the two functions ---------------------
    let mut image = [0u8; 12];
    image[..4].copy_from_slice(&MAGIC.to_le_bytes());
    image[4..8].copy_from_slice(&ADD_FN);
    image[8..].copy_from_slice(&ANSWER_FN);

    sfc.erase_sector(IMAGE_ADDR).unwrap();
    sfc.write(IMAGE_ADDR, &image).unwrap();
    rprintln!("programmed  {} bytes at {:#010X}", image.len(), IMAGE_ADDR);

    // --- 1. data: the window read as memory, no SFC call involved -----------
    let window = FLASH_BASE + IMAGE_ADDR;
    let magic = unsafe { core::ptr::read_volatile(window as *const u32) };
    rprintln!(
        "data  XIP   = {:#010X} at {:#010X} (want {:#010X})",
        magic,
        window,
        MAGIC
    );
    if magic != MAGIC {
        // Nothing is executed until the mapping is known to work, so a wrong
        // opcode or width shows up here instead of as a hard fault.
        rprintln!("sfc_xip: mapping failed -- not executing anything");
        loop {}
    }

    // --- 2. code: the core fetches instructions from the same window --------
    unsafe {
        asm::dsb();
        asm::isb();

        // Bit 0 of a function pointer selects Thumb state; the instructions
        // themselves stay halfword aligned.
        let add: unsafe extern "C" fn(u32, u32) -> u32 =
            core::mem::transmute(((window + 4) | 1) as usize as *const ());
        let sum = add(3, 4);
        rprintln!("code  XIP   = add(3, 4) -> {}", sum);
        assert_eq!(sum, 7);

        let answer: unsafe extern "C" fn() -> u32 =
            core::mem::transmute(((window + 8) | 1) as usize as *const ());
        let answer = answer();
        rprintln!("code  XIP   = answer() -> {}", answer);
        assert_eq!(answer, 42);
    }

    rprintln!("sfc_xip: OK -- code ran straight out of the SPI flash");
    loop {}
}
