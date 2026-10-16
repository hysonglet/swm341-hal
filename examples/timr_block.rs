#![no_std]
#![no_main]

//! Blocking TIMR sample: busy-wait one second with TIMR0.
//!
//! TIMR has no mandatory pads (external clock input / compare output roles live in
//! [`swm341_hal::timr::pins`] and are applied with [`swm341_hal::gpio::set_function`]
//! only when a pad is actually used).

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_120mhz(),
    });

    // Hardware readback to verify what the PLL/clock tree actually ended up with.
    let sys = unsafe { &*hal::pac::SYS::PTR };
    let plldiv = sys.plldiv.read().bits();
    let clksel = sys.clksel.read().bits();
    let pllcr = sys.pllcr.read().bits();
    rprintln!(
        "diag: PLLDIV=0x{:08X} CLKSEL=0x{:08X} PLLCR=0x{:08X} CLKDIVX_ON={}",
        plldiv,
        clksel,
        pllcr,
        sys.clkdivx_on.read().bits()
    );
    rprintln!(
        "diag: fbdiv={} indiv={} outdiv={} insel={} outen={} sys={} clk={} clk_divx={}",
        plldiv & 0x1ff,
        (plldiv >> 16) & 0x1f,
        (plldiv >> 24) & 3,
        (pllcr >> 1) & 1,
        pllcr & 1,
        clksel & 1,
        (clksel >> 2) & 7,
        (clksel >> 1) & 1
    );

    rprintln!("timr_block: measuring TIMR (clock stabilized)");

    // Let the clock tree settle after the PLL/clock switch.
    cortex_m::asm::delay(1_200_000); // ~10 ms @ 120 MHz

    // DWT cycle counter: core runs at SYSCLK = 120 MHz. DWT base 0xE000_1000;
    // CTRL@0x00 (CYCCNTENA = bit0), CYCCNT@0x04. Counts real wall-clock core cycles,
    // so the flag-poll duration below is wall-time regardless of peripheral stalls.
    let dwt_base = 0xE000_1000 as *mut u32;
    let demcr = 0xE000_EDFC as *mut u32;
    unsafe {
        core::ptr::write_volatile(demcr, core::ptr::read_volatile(demcr) | (1u32 << 24));
        core::ptr::write_volatile(dwt_base, core::ptr::read_volatile(dwt_base) | 1);
        let cyccnt = dwt_base.add(1);

        // (A) Counter width: an over-wide LOAD truncates to 24 bits
        //     (SWM341.h: TIMR_LOAD_VALUE_Msk = 0xFFFFFF).
        {
            let t = hal::timr::Timr::new(p.timr0(), hal::timr::Config::default());
            t.start(120_000_000);
            rprintln!(
                "diag: start(120_000_000) value() reads {}  (24-bit max = {})",
                t.value(),
                (1u32 << 24) - 1
            );
        }

        // Measure one timer period via the overflow flag (no VALUE-register reads, whose
        // back-to-back accesses carry heavy wait-states). Each call builds a fresh timer
        // that is dropped on return, so the single TIMR0 is reconfigured cleanly.
        let measure = |prescaler: u32, load: u32| -> u64 {
            let t = hal::timr::Timr::new(p.timr0(), hal::timr::Config::default().prescaler(prescaler));
            t.clear_overflow();
            t.start(load);
            core::ptr::write_volatile(cyccnt, 0);
            while !t.overflow_flag() {}
            (core::ptr::read_volatile(cyccnt) as u64 * 1000) / 120_000_000
        };

        // (B) TIMR clock = 120 MHz: 1_000_000 ticks, PSC=0 -> ~8.3 ms.
        rprintln!("diag: LOAD=1_000_000   PREDIV=0 -> {} ms  (clock≈120MHz if ~8ms)", measure(0, 1_000_000));
        // (C) 24-bit truncation: LOAD=120_000_000 wraps to 2_559_488 -> ~21 ms, never 1 s.
        rprintln!("diag: LOAD=120_000_000 PREDIV=0 -> {} ms  (24-bit wrap)", measure(0, 120_000_000));
        // (D) Correct 1-second: PSC=9 (÷10 -> 12 MHz), LOAD=12_000_000.
        rprintln!("diag: LOAD=12_000_000 PREDIV=9 -> {} ms  (1s @120MHz/10)", measure(9, 12_000_000));
    }

    rprintln!("timr_block: done");

    loop {}
}
