#![no_std]
#![no_main]

//! Blocking SPI sample: read the JEDEC ID from a flash chip wired to SPI0.
//!
//! Pads (type-checked through [`swm341_hal::gpio::PinFuncOf`]):
//! SCLK = PB8, MOSI = PB10, MISO = PB9, SSEL = PB14.

use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::spi::pins;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut spi = hal::spi::Spi::new(
        p.spi0(),
        p.PB8,
        pins::b::Pin8Func::Spi0Sclk,
        p.PB10,
        pins::b::Pin10Func::Spi0Mosi,
        p.PB9,
        pins::b::Pin9Func::Spi0Miso,
        p.PB14,
        pins::b::Pin14Func::Spi0Ssel,
        hal::spi::Config::default().frequency(1_000_000),
    );

    // Read the 3-byte JEDEC ID (command 0x9F).
    let mut id = [0u8; 3];
    spi.blocking_transfer(&mut id, &[0x9F, 0x00, 0x00, 0x00]);

    let _ = id;
    loop {}
}
