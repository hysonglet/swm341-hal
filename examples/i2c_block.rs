#![no_std]
#![no_main]

//! Blocking I2C sample: read two bytes from an EEPROM at address 0x50.
//!
//! Pads (type-checked through [`swm341_hal::gpio::PinFuncOf`]):
//! SCL = PC2, SDA = PC1.

use panic_halt as _;

use swm341_hal as hal;
use swm341_hal::i2c::pins;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut i2c = hal::i2c::I2c::new(
        p.i2c0(),
        p.PC2,
        pins::c::Pin2Func::I2c0Scl,
        p.PC1,
        pins::c::Pin1Func::I2c0Sda,
        hal::i2c::Config::default().frequency(100_000),
    );

    let mut data = [0u8; 2];
    // Write the memory address, then read two bytes back (repeated start).
    let _ = i2c.blocking_write_read(0x50, &[0x00], &mut data);

    loop {}
}
