#![no_std]
#![no_main]

//! Read the JEDEC ID of the on-board W25Q128 NOR flash through the new
//! [`swm341_hal::sfc::Sfc`] driver and print it over RTT.
//!
//! The flash is wired to the SWM341's dedicated Flash-SPI pads (D3..D8), which
//! belong to the SFC rather than to SPI0/SPI1 -- so the driver configures those
//! pads for us. Run with `cargo run --example sfc_flash_id`.

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

    let sfc = hal::sfc::Sfc::new(p.sfc());

    // Idle flash: SR1 busy bit must read 0.
    let sr1 = sfc.read_status(0x05);
    rprintln!("status reg1 = 0x{:02X} (busy = {})", sr1, sr1 & 1);

    // Three JEDEC bytes: manufacturer, memory type, capacity.
    let raw = sfc.read_jedec();
    rprintln!("SFC.DATA    = 0x{:06X}", raw & 0x00FF_FFFF);

    // The first byte received normally sits in bits [23:16]; if a different
    // silicon revision aligns the shift register the other way, the
    // manufacturer byte lands in bits [7:0] -- accept both.
    let (mfr, mtype, cap) = if raw & 0xFF == 0xEF {
        (raw & 0xFF, (raw >> 8) & 0xFF, (raw >> 16) & 0xFF)
    } else {
        ((raw >> 16) & 0xFF, (raw >> 8) & 0xFF, raw & 0xFF)
    };
    rprintln!("JEDEC ID    = 0x{:02X} 0x{:02X} 0x{:02X}", mfr, mtype, cap);

    if (mfr, mtype, cap) == (0xEF, 0x40, 0x18) {
        rprintln!("OK: Winbond W25Q128 (128 Mbit / 16 MiB) detected, ID read works.");
    } else {
        rprintln!("unexpected ID -- check wiring/pull-ups (expected EF 40 18).");
    }

    loop {}
}
