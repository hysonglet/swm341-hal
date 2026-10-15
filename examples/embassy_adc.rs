#![no_std]
#![no_main]

//! Async ADC sample: await the end-of-conversion interrupt on PC0 (ADC0_CH6)
//! and PC1 (ADC0_CH5), printing each sample over RTT.

use embassy_executor::Spawner;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use embassy_time::Timer;

use swm341_hal as hal;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // `read_async` takes `&mut self`: the channel mask and the sequence FIFO are
    // per-instance state, so concurrent readers would otherwise mix channels.
    let mut adc = hal::adc::Adc::new_async(p.adc0(), hal::adc::Config::default());

    // Route PC0 (ADC0_CH6) and PC1 (ADC0_CH5) to the analog mux and remember
    // the converter channel each one maps to.
    let pc0 = adc.add_pin(p.PC0);
    let pc1 = adc.add_pin(p.PC1);
    rprintln!(
        "ADC ready: PC0 -> CH{}, PC1 -> CH{}",
        pc0.channel,
        pc1.channel
    );

    loop {
        let s0 = adc.read_async(&pc0).await;
        let s1 = adc.read_async(&pc1).await;

        let mv0 = s0 as u32 * 3300 / 4095;
        let mv1 = s1 as u32 * 3300 / 4095;
        rprintln!(
            "PC0 (CH{}): raw={} ~ {}mV | PC1 (CH{}): raw={} ~ {}mV",
            pc0.channel,
            s0,
            mv0,
            pc1.channel,
            s1,
            mv1
        );

        // Rough ~1s async delay, yielding the executor.
        Timer::after_secs(1).await;
    }
}
