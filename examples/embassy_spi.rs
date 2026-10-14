#![no_std]
#![no_main]

//! Async SPI sample: read a flash JEDEC ID using the embassy-based driver.
//!
//! Pads: SCLK = PB8, MOSI = PB10, MISO = PB9, SSEL = PB14.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::spi::pins;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut spi = hal::spi::Spi::new_async(
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

    // Full-duplex read of the 3-byte JEDEC ID.
    let mut id = [0u8; 3];
    spi.transfer(&mut id, &[0x9F, 0x00, 0x00, 0x00]).await;

    // In-place loopback-style exchange.
    let mut echo = [0xAAu8, 0xBB, 0xCC];
    spi.transfer_in_place(&mut echo).await;

    let _ = (id, echo);
    loop {}
}
