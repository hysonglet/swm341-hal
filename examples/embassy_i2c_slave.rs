#![no_std]
#![no_main]

//! Async I2C slave sample: answer to address 0x30 and loop back transfers.
//!
//! Pads: SCL = PC2, SDA = PC1.

use embassy_executor::Spawner;
use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::i2c::pins;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut slave = hal::i2c::I2cSlave::new_async(
        p.i2c0(),
        p.PC2,
        pins::c::Pin2Func::I2c0Scl,
        p.PC1,
        pins::c::Pin1Func::I2c0Sda,
        hal::i2c::SlaveConfig::default().address(0x30),
    );

    let mut rx = [0u8; 4];
    let tx = [0xABu8; 4];
    loop {
        let _ = slave.transfer(&mut rx, &tx).await;
    }
}
