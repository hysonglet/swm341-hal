#![no_std]
#![no_main]

//! Async UART sample: echo every byte received on UART2 back to the host, logging
//! each exchange over RTT.
//!
//! Pads: TX = PC0 (UART2_TX, func 1), RX = PC1 (UART2_RX, func 2).

use embassy_executor::Spawner;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut uart = hal::uart::Uart::new_async(
        p.uart2(),
        p.PC0,
        p.PC1,
        hal::uart::Config::default().baudrate(115_200),
    );

    rprintln!("uart echo ready: 115200 8N1 on PC0(TX)/PC1(RX)");
    uart.write(b"uart echo demo\r\n").await;

    let mut received: u32 = 0;
    loop {
        // Wait for one byte from the host.
        let byte = match uart.read().await {
            Ok(b) => b,
            Err(_) => {
                rprintln!("uart parity error");
                continue;
            }
        };

        // Echo it straight back.
        uart.write(&[byte]).await;

        received += 1;
        rprintln!(
            "recv {}: 0x{:02x} '{}'",
            received,
            byte,
            byte as char
        );
    }
}
