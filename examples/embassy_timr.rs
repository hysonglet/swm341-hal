#![no_std]
#![no_main]

//! Async TIMR sample: wait for a period without busy-spinning, then keep
//! ticking in a loop and logging each iteration over RTT. The real elapsed
//! time is measured with `embassy_time::Instant` so you can verify the timer
//! period against the wall clock and tell a *flood* from a *hard hang*.
//!
//! IMPORTANT: `timer.delay(n)` counts `n` TIMER TICKS, not milliseconds.
//! The 24-bit counter can't hold 150M ticks at prescaler 0, so `delay` now
//! auto-picks the smallest 8-bit prescaler (via `Config::prescale_for_ticks`)
//! that keeps `n` ticks inside `LOAD` — `delay(150_000_000)` therefore waits
//! exactly one second at the 150 MHz PCLK (prescaler 9, load 12M). At a smaller
//! `n`:
//!   - `delay(150)`   ≈ 1  µs   (will flood RTT)
//!   - `delay(1500)`  ≈ 10 µs   (still floods, 10x slower than 150)
//!   - `delay(150_000_000)` ≈ 1 s (the intended ~1 s cadence)

use embassy_executor::Spawner;
use embassy_time::Instant;
use panic_halt as _;

use swm341_hal as hal;

use rtt_target::{rprintln, rtt_init_print};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut timer = hal::timr::Timr::new_async(p.timr0(), hal::timr::Config::default());

    // Change this to experiment. Use 150_000_000 for a visible ~1 s cadence.
    const PERIOD_TICKS: u32 = 150_000_000;

    rprintln!("embassy_timr: started, PERIOD_TICKS = {}", PERIOD_TICKS);

    let mut count: u32 = 0;
    let mut last = Instant::now();
    loop {
        let t0 = Instant::now();
        timer.delay(PERIOD_TICKS).await;
        let elapsed = Instant::now().duration_since(t0);

        count += 1;
        rprintln!(
            "embassy_timr: tick {} (delay took {} ms, since last {} ms)",
            count,
            elapsed.as_millis(),
            Instant::now().duration_since(last).as_millis(),
        );
        last = Instant::now();
    }
}
