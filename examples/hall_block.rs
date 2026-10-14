#![no_std]
#![no_main]

//! Blocking HALL example: enable the first hall interface and read its inputs.
//!
//! The three hall-input pads must be routed with [`swm341_hal::gpio::set_function`]
//! on real hardware for the interface to see anything.

use panic_halt as _;

use swm341_hal as hal;
use hal::hall::{Hall, HallSelect};

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let hall = Hall::new(p.hall(), HallSelect::Hall0, Default::default());
    let level = hall.input_level(0);
    let _ = hall.read_value();
    let _ = level;

    loop {}
}
