#![no_std]
#![no_main]

//! Blocking QEI example: route the encoder pads and read the position counter.
//!
//! All three QEI inputs live on port D at function code `1` (datasheet table
//! 5-4 / `PORTD_SEL0`): PD4 = QEA (phase A), PD3 = QEB (phase B),
//! PD2 = INDEX_IN (index/Z phase). They are handed to `Qei::new`, which muxes
//! them; this sample then checks the mux codes back from `PORTD.FUNC0`, builds
//! the decoder in ×4 / match-reset mode, and prints the live position once a
//! second. With no encoder wired the pads float and the count may jitter;
//! attach an encoder (or tie QEA/QEB to fixed levels) for a stable reading.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use hal::qei::{Config, CountMode, Interrupt, Interrupts, Qei, ResetMode};
use swm341_hal as hal;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // --- Build the decoder: x4 resolution, reset on count match ----------
    // The three phase pads are muxed (function code 1) and owned by `new`.
    let mut qei = Qei::new(
        p.qei(),
        p.PD4, // phase A
        p.PD3, // phase B
        p.PD2, // index (Z)
        Config {
            count_mode: CountMode::X4,
            reset_mode: ResetMode::Match,
            swap_ab: false,
            max_count: 999,
            interrupts: Interrupts::NONE,
            enable: false,
        },
    );

    // Read the mux codes straight back out of PORTD.FUNC0: PD4 = bits 19:16,
    // PD3 = bits 15:12, PD2 = bits 11:8; each must read 0001.
    let sel0 = unsafe { (*hal::pac::PORTD::PTR).func0.read().bits() };
    assert_eq!((sel0 >> 16) & 0xF, 1, "PD4 must mux to QEA_IN");
    assert_eq!((sel0 >> 12) & 0xF, 1, "PD3 must mux to QEB_IN");
    assert_eq!((sel0 >> 8) & 0xF, 1, "PD2 must mux to INDEX_IN");
    rprintln!("qei_block: PD4=QEA, PD3=QEB, PD2=INDEX muxed");

    // Fresh out of reset the decoder is stopped and POSCNT reads zero.
    assert!(!qei.is_enabled());
    assert_eq!(qei.count(), 0);
    assert_eq!(qei.max_count(), 999);

    // MAXCNT is the match-reset top; reprogram it and read it back.
    qei.set_max_count(3999);
    assert_eq!(qei.max_count(), 3999);
    qei.set_max_count(999);

    // Pause/resume only flips CR.PAUSE.
    qei.pause();
    assert!(qei.is_paused());
    qei.resume();
    assert!(!qei.is_paused());

    // Exercise the interrupt plumbing without taking the NVIC vector: arm and
    // unmask the overflow source, then clear every stale flag.
    qei.enable_interrupt(Interrupt::CountOverflow, true);
    qei.unmask_interrupt(Interrupt::CountOverflow, true);
    for source in [
        Interrupt::Index,
        Interrupt::Match,
        Interrupt::CountOverflow,
        Interrupt::Error,
    ] {
        qei.clear_flag(source);
        assert!(!qei.is_flag(source));
    }

    qei.enable();
    assert!(qei.is_enabled());
    rprintln!("qei_block: decoder running in X4 match-reset mode");

    let mut last = qei.count();
    loop {
        let pos = qei.count();
        if pos != last {
            // Direction is not a register bit: diff two samples (or route
            // PD7 to UPDN_OUT and read it as GPIO).
            let forward = (pos as i16) - (last as i16) > 0;
            rprintln!(
                "position: {} ({})",
                pos,
                if forward { "fwd" } else { "rev" }
            );
            last = pos;
        }

        // ~1 s at 150 MHz.
        cortex_m::asm::delay(150_000_000);
    }
}
