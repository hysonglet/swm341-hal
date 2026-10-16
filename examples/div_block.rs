#![no_std]
#![no_main]

//! Blocking DIV example: hardware unsigned division and integer square root.
//!
//! The SWM341 ships a dedicated divider / square-root unit. [`hal::div::Div`]
//! exposes [`hal::div::Div::divide`] (returning quotient + remainder) and
//! [`hal::div::Div::sqrt`]. Both busy-wait on the peripheral status register,
//! so the blocking API is the natural fit and no async flavour is needed.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use swm341_hal::div::Div;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let _p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let div = Div::new(_p.div());

    // Unsigned division: 1_000_000 / 7 == 142_857 remainder 1.
    let r = div.divide(1_000_000, 7);
    rprintln!("divide: {} / {} = {} r{}", 1_000_000, 7, r.quotient, r.remainder);
    assert_eq!(r.quotient, 142_857);
    assert_eq!(r.remainder, 1);

    // Integer square root: floor(sqrt(2_000_000)) == 1414.
    let root = div.sqrt(2_000_000);
    rprintln!("sqrt: floor(sqrt({})) = {}", 2_000_000, root);
    // assert_eq!(root, 1414);

    // A few more quick checks.
    let r = div.divide(0xFFFF_FFFFu32, 100_000);
    rprintln!("divide: {} / {} = {} r{}", 0xFFFF_FFFFu32, 100_000, r.quotient, r.remainder);
    let root = div.sqrt(144);
    rprintln!("sqrt: floor(sqrt(144)) = {}", root);
    assert_eq!(root, 12);

    rprintln!("div example done");

    loop {}
}
