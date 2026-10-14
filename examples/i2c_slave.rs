#![no_std]
#![no_main]

//! Blocking I2C slave sample: answer to address 0x30 and echo a read back.
//!
//! Pads: SCL = PC2, SDA = PC1.

use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::i2c::pins;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut slave = hal::i2c::I2cSlave::new(
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
        // If the master reads from us we send `tx`; if it writes we capture it in `rx`.
        let _ = slave.blocking_transfer(&mut rx, &tx);
    }
}
