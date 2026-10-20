#![no_std]
#![no_main]

//! Blocking CRC example: one-shot and incremental hardware CRC.
//!
//! The SWM341 has a dedicated CRC engine. [`hal::crc::Crc::calculate`] seeds the
//! accumulator, feeds a slice and returns the value; [`hal::crc::Crc::reset`]
//! plus [`hal::crc::Crc::update_bytes`] do the same when the data arrives in
//! pieces. The engine is synchronous -- there is no completion flag to wait on
//! -- so the blocking API is the natural fit and no async flavour is provided.
//!
//! The expected values below are the standard check values for the configured
//! algorithms over the canonical `"123456789"` test vector, which is what makes
//! this example a useful hardware self-test of the `CR.POLY` / `CR.OREV`
//! mapping.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use swm341_hal::crc::{Config, Crc};

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // Reflected CRC-32 (zlib / Ethernet flavour) over the test vector.
    let mut crc = Crc::new(p.crc(), Config::crc32());
    let value = crc.calculate(b"123456789");
    rprintln!("crc32(\"123456789\") = {:#010x}", value);
    assert_eq!(value, 0xCBF4_3926);

    // Same engine, reconfigured for CRC-16/CCITT-FALSE.
    crc.set_config(Config::crc16_ccitt());
    let value = crc.calculate(b"123456789");
    rprintln!("crc16_ccitt(\"123456789\") = {:#06x}", value);
    assert_eq!(value, 0x29B1);

    // Incremental use: feed the same message in two chunks. Splitting the
    // stream must not change the result, so this matches the one-shot value.
    crc.reset();
    crc.update_bytes(b"12345");
    crc.update_bytes(b"6789");
    let split = crc.result();
    rprintln!("crc16_ccitt (two chunks) = {:#06x}", split);
    assert_eq!(split, value);

    // A CRC is at least order sensitive: swapping two bytes must change it.
    crc.reset();
    crc.update_bytes(b"123456789");
    let swapped = crc.calculate(b"123456798");
    rprintln!("crc16_ccitt(\"123456798\") = {:#06x}", swapped);
    assert_ne!(swapped, value);

    rprintln!("crc example done");

    loop {}
}
