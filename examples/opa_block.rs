#![no_std]
#![no_main]

//! Blocking OPA sample: power up the four on-chip operational amplifiers.
//!
//! The amplifiers are analog blocks whose control is the power switch in
//! `SYS.OPACR`, while their pads must first be routed to the OPA analog
//! function. This sample constructs all four units and verifies their power
//! state, then toggles OPA0 in a loop. OPA0 is wired as an external voltage
//! follower: its three pads are routed (`PC8` = OPA0_INP, `PC5` = OPA0_INN,
//! `PC9` = OPA0_OUT), so short PC9 to PC5 externally and feed the signal to
//! PC8 to read the buffered level on PC9.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use swm341_hal::opa::Opa;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    rprintln!("opa_block: powering up OPA0..OPA3");

    // Take every unit first: the pin singletons are moved below, which would
    // otherwise partially move `p` before the later `p.opaX()` calls.
    let mut opa0 = Opa::new(p.opa0(), hal::opa::Config { enable: true });
    let opa1 = Opa::new(p.opa1(), hal::opa::Config { enable: true });
    let opa2 = Opa::new(p.opa2(), hal::opa::Config { enable: true });
    let opa3 = Opa::new(p.opa3(), hal::opa::Config { enable: true });

    // OPA0 as an external voltage follower: route all three pads before use.
    opa0.add_positive_pin(p.PC8);
    opa0.add_negative_pin(p.PC5);
    opa0.add_output_pin(p.PC9);
    assert!(opa0.is_enabled());

    // The other three units are powered too (their pads stay digital unless
    // routed); prove the bit slices are independent.
    assert!(opa1.is_enabled());
    assert!(opa2.is_enabled());
    assert!(opa3.is_enabled());

    rprintln!("opa_block: OPA0..OPA3 all enabled, OPA0 pads routed (PC8/PC5/PC9)");

    // Toggle OPA0 periodically; OPA1..OPA3 must stay enabled throughout.
    let mut on = true;
    loop {
        if on {
            opa0.disable();
        } else {
            opa0.enable();
        }
        on = !on;

        rprintln!(
            "OPA0={} OPA1={} OPA2={} OPA3={}",
            opa0.is_enabled() as u8,
            opa1.is_enabled() as u8,
            opa2.is_enabled() as u8,
            opa3.is_enabled() as u8
        );

        // ~500 ms at 150 MHz.
        cortex_m::asm::delay(75_000_000);
    }
}
