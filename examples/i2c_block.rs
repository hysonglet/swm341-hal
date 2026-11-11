#![no_std]
#![no_main]

//! Blocking I2C test against the LCD panel's GT911 touch controller.
//!
//! Verified wiring (the vendor LVGL demo prints `tp: GT9xx on I2C1 PC5/PC4,
//! addr 0x5D, id "911"` on this board):
//! * I2C1: SCL = PC5, SDA = PC4
//! * address 0x5D. The board has no INT pin, and CS (PD9) does not gate the
//!   bus (either level works), so neither pad is driven here.
//!
//! Exercises `blocking_write`, `blocking_write_read`, the embedded-hal
//! `transaction` trait and a register write, then polls the touch status /
//! first point over RTT. Run with `cargo run --example i2c_block`.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use embedded_hal::i2c::{I2c as _, Operation};
use swm341_hal as hal;

const TP_ADDR: u8 = 0x5D;
const TP_ALT_ADDR: u8 = 0x14;
const TP_FREQ: u32 = 400_000;
const REG_PID: u16 = 0x8140;
const REG_STATUS: u16 = 0x814E;
const REG_TP1: u16 = 0x8150;

type TouchBus<'d> = hal::i2c::I2c<'d, hal::I2c1, hal::gpio::PC5, hal::gpio::PC4>;

fn delay_ms(ms: u32) {
    // 150 MHz core; only used for start-up settling and polling.
    cortex_m::asm::delay(150_000 * ms);
}

fn reg_addr(reg: u16) -> [u8; 2] {
    [(reg >> 8) as u8, reg as u8]
}

/// GT911 registers use big-endian 16-bit addresses followed by the data.
fn read_reg(
    i2c: &mut TouchBus<'_>,
    addr: u8,
    reg: u16,
    buf: &mut [u8],
) -> Result<(), hal::i2c::Error> {
    i2c.blocking_write_read(addr, &reg_addr(reg), buf)
}

fn write_reg(
    i2c: &mut TouchBus<'_>,
    addr: u8,
    reg: u16,
    value: u8,
) -> Result<(), hal::i2c::Error> {
    let a = reg_addr(reg);
    i2c.blocking_write(addr, &[a[0], a[1], value])
}

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });
    rprintln!(
        "i2c_block: GT911 on I2C1 PC5(SCL)/PC4(SDA) @ {} kHz",
        TP_FREQ / 1000
    );

    let mut i2c = hal::i2c::I2c::new(
        p.i2c1(),
        p.PC5,
        p.PC4,
        hal::i2c::Config::default().frequency(TP_FREQ),
    );
    delay_ms(50);

    // 1. Address probe (START + address + STOP).
    rprintln!(
        "probe 0x{:02X}: {:?}",
        TP_ADDR,
        i2c.blocking_write(TP_ADDR, &[])
    );
    rprintln!(
        "probe 0x{:02X}: {:?}",
        TP_ALT_ADDR,
        i2c.blocking_write(TP_ALT_ADDR, &[])
    );

    // 2. Convenience write_read: product id at 0x8140 ("911\0").
    let mut pid = [0u8; 4];
    rprintln!(
        "read PID: {:?}",
        read_reg(&mut i2c, TP_ADDR, REG_PID, &mut pid)
    );
    rprintln!(
        "  PID: {:02X?}  ascii={}{}{}",
        pid,
        pid[0] as char,
        pid[1] as char,
        pid[2] as char
    );

    // 3. The embedded-hal 1.0 `transaction` path must return the same bytes.
    let mut pid2 = [0u8; 4];
    let mut ops = [
        Operation::Write(&reg_addr(REG_PID)),
        Operation::Read(&mut pid2),
    ];
    rprintln!(
        "transaction PID: {:?} -> {:02X?}",
        i2c.transaction(TP_ADDR, &mut ops),
        pid2
    );

    // 4. Poll status / first touch point; clear READY with a register write.
    let mut tick: u32 = 0;
    let mut errors: u32 = 0;
    loop {
        let mut status = [0u8; 1];
        match read_reg(&mut i2c, TP_ADDR, REG_STATUS, &mut status) {
            Ok(()) => {
                let points = status[0] & 0x0F;
                if (status[0] & 0x80) != 0 {
                    if (1..6).contains(&points) {
                        let mut xy = [0u8; 4];
                        match read_reg(&mut i2c, TP_ADDR, REG_TP1, &mut xy) {
                            Ok(()) => rprintln!(
                                "touch: {} point(s), first = ({}, {})",
                                points,
                                u16::from_le_bytes([xy[0], xy[1]]),
                                u16::from_le_bytes([xy[2], xy[3]])
                            ),
                            Err(e) => rprintln!("read point: {:?}", e),
                        }
                    }
                    let _ = write_reg(&mut i2c, TP_ADDR, REG_STATUS, 0);
                }
            }
            Err(e) => {
                errors += 1;
                if errors <= 5 || errors % 20 == 0 {
                    rprintln!("read status: {:?} (total {})", e, errors);
                }
            }
        }

        tick += 1;
        if tick % 20 == 0 {
            rprintln!("heartbeat: {} polls, {} errors", tick, errors);
        }
        delay_ms(50);
    }
}
