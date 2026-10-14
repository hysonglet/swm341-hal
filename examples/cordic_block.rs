#![no_std]
#![no_main]

//! Blocking CORDIC example: compute sin/cos of a fixed angle in Q14.

use panic_halt as _;

use swm341_hal as hal;
use hal::cordic::{Cordic, Q14};

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let mut cordic = Cordic::new(p.cordic(), Default::default());
    // Q14::ONE is the angle fed to the hardware (~ quarter turn).
    let sin = cordic.sin(Q14::ONE);
    let cos = cordic.cos(Q14::ONE);
    let _ = (sin, cos);

    loop {}
}
