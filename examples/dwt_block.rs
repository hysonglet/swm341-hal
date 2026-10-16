#![no_std]
#![no_main]

//! DWT (Data Watchpoint and Trace) usage example.
//!
//! Shows the free-running cycle counter (`CYCCNT`) for timing, the [`measure`]
//! helper, a comparator watchpoint and the profiling counters. Output goes over
//! RTT -- view it with `probe-rs` / a J-Link RTT viewer. The cycle counts are
//! only meaningful on real hardware (the core must implement `CYCCNT`).

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;

/// A sentinel we point a (harmless) watchpoint at, just to exercise the API.
/// A comparator armed on this address never matches the PC, so the demo stays
/// runnable without a debugger attached.
static MARKER: u32 = 0;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config::hrc_150mhz());
    let core_hz = hal::clock::sys_core_clock();

    rprintln!("==== SWM341 DWT demo ====");
    rprintln!("Core clock: {} Hz", core_hz);

    let mut dwt = hal::dwt::Dwt::new(p.dwt());

    if !dwt.has_cycle_counter() {
        rprintln!("This core does not implement CYCCNT; demo cannot continue.");
        loop {
            cortex_m::asm::wfi();
        }
    }

    // --- cycle counter basics -------------------------------------------------
    dwt.enable_cycle_counter();
    dwt.reset_cycle_count();

    // Busy-wait roughly 1 ms worth of core cycles.
    //
    // NOTE: `cortex_m::asm::delay(n)` runs a `n`-iteration loop; each iteration
    // costs a few cycles (subs + branch-taken), so the elapsed cycle count is
    // several times larger than `n` -- this is expected, not a counter error.
    let one_ms = core_hz / 1_000;
    let start = dwt.cycle_count();
    cortex_m::asm::delay(one_ms);
    let elapsed = dwt.cycle_count().wrapping_sub(start);
    rprintln!(
        "cortex_m::asm::delay({}) loop took ~{} cycles (~{:.1}x the loop count)",
        one_ms,
        elapsed,
        elapsed as f32 / one_ms as f32
    );

    // --- measure() helper -----------------------------------------------------
    let (acc, cycles) = dwt.measure(|| {
        let mut acc: u32 = 0;
        for i in 0..1000 {
            acc = acc.wrapping_add(i);
        }
        acc
    });
    rprintln!("sum loop of 1000 iters returned {} and took {} cycles", acc, cycles);

    // --- profiling counters ---------------------------------------------------
    dwt.enable_profiling_counters();
    dwt.reset_cycle_count();
    cortex_m::asm::delay(one_ms * 10);
    rprintln!(
        "over ~{} cycles: cpi={} exc={} sleep={} lsu={} fold={}",
        dwt.cycle_count(),
        dwt.cpi_count(),
        dwt.exception_count(),
        dwt.sleep_count(),
        dwt.lsu_count(),
        dwt.fold_count(),
    );
    dwt.disable_profiling_counters();

    // --- watchpoint (comparator) ---------------------------------------------
    // Arm a PC watchpoint on the address of `MARKER`. Without a debugger
    // attached a match would stall the core (it behaves like a breakpoint), so
    // we only *configure* it here and then clear it to stay runnable. Pointing
    // it at a data address means the PC can never match it, so it is harmless.
    match dwt.set_watchpoint(0, hal::dwt::Watchpoint::instruction(&MARKER as *const u32 as usize as u32)) {
        Ok(()) => rprintln!("watchpoint 0 armed on MARKER address (cleared below)"),
        Err(e) => rprintln!("watchpoint config failed: {:?}", e),
    }
    dwt.clear_watchpoint(0);

    // An out-of-range comparator index is rejected with an error.
    if let Err(e) = dwt.set_watchpoint(99, hal::dwt::Watchpoint::instruction(0)) {
        rprintln!("expected error for bad index: {:?}", e);
    }

    dwt.disable_cycle_counter();
    rprintln!("==== DWT demo done ====");

    loop {
        cortex_m::asm::wfi();
    }
}
