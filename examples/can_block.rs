#![no_std]
#![no_main]

//! Blocking CAN example: build a controller and transmit one data frame.
//!
//! On real hardware the CAN0 TX/RX pins must be routed to the CAN function with
//! [`swm341_hal::gpio::set_function`] before the bus is usable; this example only
//! exercises the driver API.

use panic_halt as _;

use swm341_hal as hal;
use hal::can::{Can, Config, Frame};

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let mut can = Can::new(p.can0(), Config::default());
    let frame = Frame::new_data(0x123, false, &[1, 2, 3]);
    let _ = can.transmit(&frame);

    loop {}
}
