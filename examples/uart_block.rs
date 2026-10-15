#![no_std]
#![no_main]

//! Blocking UART sample: echo every byte received on UART2 straight back to the
//! host. This is the blocking twin of `embassy_uart.rs` — it spins on the FIFO
//! status bits instead of sleeping on interrupts.
//!
//! Pads: TX = PC0 (UART2_TX), RX = PC1 (UART2_RX). The TX/RX `PORTx.FUNC` codes
//! are derived from the pin type via [`swm341_hal::uart::UartTxPin`] /
//! [`swm341_hal::uart::UartRxPin`], so only the pins need to be passed.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut uart: hal::uart::Uart<'_, _, _, _, hal::mode::Blocking> = hal::uart::Uart::new(
        p.uart2(),
        p.PC0,
        p.PC1,
        hal::uart::Config::default().baudrate(115_200),
    );

    rprintln!("uart echo ready: 115200 8N1 on PC0(TX)/PC1(RX)");
    uart.blocking_write(b"uart echo demo\r\n");

    let mut received: u32 = 0;
    loop {
        // Read one byte (spins until the RX FIFO has data).
        let byte = match uart.blocking_read_byte() {
            Ok(b) => b,
            Err(_) => {
                rprintln!("uart parity error");
                continue;
            }
        };

        // Echo it straight back.
        uart.blocking_write_byte(byte);

        received += 1;
        rprintln!("recv {}: 0x{:02x} '{}'", received, byte, byte as char);
    }
}
