#![no_std]
#![no_main]

//! Blocking WDT (watchdog) example.
//!
//! The SWM341 watchdog is a free-running down-counter that resets the whole
//! chip when it reaches zero. It must be reloaded periodically by "feeding" it
//! with the magic `0x55`/`0xAA` sequence. This example arms the watchdog with
//! a 2-second timeout (and a 1.5-second warning) expressed in real time units,
//! feeds it from the main loop so the firmware stays alive, then deliberately
//! stops feeding after a few iterations to demonstrate the reset.
//!
//! Other [`hal::wdt`] APIs not exercised here: [`Wdt::set_reload`],
//! [`Wdt::set_warning`], [`Wdt::disable`], [`Wdt::is_interrupt_pending`] and
//! [`Wdt::clear_interrupt`], plus the `interrupt_on_warning` / `windowed` /
//! `clock_hz` fields of [`hal::wdt::Config`].

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use hal::wdt::{Config, Wdt};

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let wdt = Wdt::new(
        p.wdt(),
        // 2s timeout, warn at 1.5s, reset the chip on timeout, no warning IRQ.
        // `reload_secs` / `warning_ms` convert time into watchdog-clock ticks
        // using the HCLK frequency set by `init` (150 MHz here).
        Config::default()
            .reload_secs(2)
            .warning_ms(1500)
            .reset_on_timeout(true)
            .interrupt_on_warning(false),
    );

    rprintln!("wdt_block: watchdog armed (2s timeout); feeding for 10 loops, then stop");

    for i in 0..10 {
        // Keep the watchdog happy. Remove this call and the chip resets early.
        wdt.feed();
        rprintln!("wdt_block: alive, fed {} times", i + 1);
        // ~200 ms pause at 150 MHz.
        cortex_m::asm::delay(150_000_000 / 5);
    }

    rprintln!("wdt_block: stopped feeding -> watchdog should reset the MCU soon");
    loop {
        // No feed: the counter runs out and the watchdog resets the chip,
        // restarting this example from the top. The RTT log will show the
        // "watchdog armed" line again after the reset.
    }
}
