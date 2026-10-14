#![no_std]
#![no_main]

//! Async SPI slave sample: echo back what the master sends while it clocks.
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

    let mut slave = hal::spi::SpiSlave::new_async(
        p.spi0(),
        p.PB8,
        pins::b::Pin8Func::Spi0Sclk,
        p.PB10,
        pins::b::Pin10Func::Spi0Mosi,
        p.PB9,
        pins::b::Pin9Func::Spi0Miso,
        p.PB14,
        pins::b::Pin14Func::Spi0Ssel,
        hal::spi::Config::default(),
    );

    let mut rx = [0u8; 8];
    let tx = [0xABu8; 8];
    loop {
        let _ = slave.transfer(&mut rx, &tx).await;
    }
}
