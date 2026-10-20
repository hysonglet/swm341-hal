#![no_std]
#![no_main]

//! Blocking HALL example: measure the interval between hall input edges.
//!
//! Mirrors the vendor `SimplHALL` sample. HALL0 watches PA0/PA1/PA2 and captures
//! TIMR0 on every edge (the module is dual-edge triggered), so arming TIMR0 at
//! 1 MHz makes the captured count read directly in microseconds.
//!
//! `Hall::new_hall0` does the wiring the sample does by hand: muxes the three
//! pads (with pull-ups, since hall sensors are open-drain), arms TIMR0
//! free-running, and enables the interface.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use hal::timr::Timr;
use hal::timr::hall::Hall;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // TIMR0 is HALL0's time base. 150 MHz / 150 = 1 MHz ticks, so `read_value`
    // below is microseconds (the vendor sample prescales to 1 MHz as well).
    let mut timer = Timr::new(p.timr0(), hal::timr::Config::default().prescaler(149));

    // HALL0 on port A: PA0/PA1/PA2 are inputs 0/1/2. The constructor muxes them
    // and starts TIMR0, so nothing else needs configuring.
    let mut hall = Hall::new_hall0(p.hall(), &mut timer, p.PA0, p.PA1, p.PA2, Default::default());

    rprintln!(
        "hall_block: HALL0 on PA0/PA1/PA2, time base TIMR{}",
        hall.paired_timer_index()
    );

    let mut last = hall.read_value();
    loop {
        for input in 0..3 {
            if hall.is_flag(input) {
                hall.clear_flag(input);
                rprintln!(
                    "hall_block: input {} changed, level = {}, interval = {} us",
                    input,
                    hall.input_level(input) as u8,
                    hall.read_value()
                );
            }
        }

        let now = hall.read_value();
        if now != last {
            last = now;
            rprintln!("hall_block: {} us since the last edge", now);
        }
    }
}
