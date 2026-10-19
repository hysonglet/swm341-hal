#![no_std]
#![no_main]

//! SPI cross-controller sample: SPI0 is the bus **master** and transmits a frame
//! while SPI1 is a bus **slave** and receives it. Wire the two controllers'
//! pads together (hookup list below) and the bytes shifted out on MOSI by SPI0
//! should appear in SPI1's receive buffer, printed over RTT.
//!
//! Pads (type-checked through [`swm341_hal::spi::pins`]):
//! * Master  (SPI0): SCLK = PM2, MOSI = PM5, MISO = PM4, SSEL = PM3
//!   (SPI0 has no function on the PN bank — PN0/PN1/PN4/PN5 belong to SPI1 — so
//!    PM is the only bank left that does not collide with the slave below.)
//! * Slave   (SPI1): SCLK = PB2, MOSI = PB4, MISO = PB3, SSEL = PB5
//!
//! Hookup (run a wire between each pair):
//! * PM2  (SPI0 SCLK)  <-> PB2  (SPI1 SCLK)
//! * PM5  (SPI0 MOSI)  <-> PB4  (SPI1 MOSI)
//! * PM3  (SPI0 SSEL)  <-> PB5  (SPI1 SSEL)
//! * (optional) PM4   (SPI0 MISO) <-> PB3 (SPI1 MISO)
//!
//! The slave runs in its own executor task so it can receive *concurrently*
//! with the master transfer. This is required: SPI1 can only capture bytes
//! while SPI0 holds CS low and clocks the bus, so the two sides must be driven
//! in lock-step by the async executor rather than sequentially.

use embassy_executor::Spawner;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use swm341_hal as hal;
use swm341_hal::spi::pins;

/// Background task: park SPI1 in slave mode and capture one whole CS-active
/// frame clocked in by the master.
#[embassy_executor::task]
async fn slave_receive(
    mut spi: hal::spi::SpiSlave<
        'static,
        hal::Spi1,
        hal::gpio::PB2,
        hal::gpio::PB4,
        hal::gpio::PB3,
        hal::gpio::PB5,
        hal::mode::Async,
    >,
) {
    let mut rx = [0u8; 4];
    rprintln!("spi_dual: slave awaiting CS");
    if let Err(e) = spi.wait_start().await {
        rprintln!("spi_dual: slave wait_start err {:?}", e);
        return;
    }
    rprintln!("spi_dual: slave CS asserted, reading bytes");
    for i in 0..4 {
        match spi.read_byte().await {
            Ok(b) => {
                rx[i] = b;
                rprintln!("spi_dual: slave rx[{}] = {}", i, b);
            }
            Err(e) => {
                rprintln!("spi_dual: slave read_byte[{}] err {:?}", i, e);
                break;
            }
        }
    }
    let _ = spi.wait_end().await;
    rprintln!("SPI1 (slave) received: {:?}", &rx);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    rtt_init_print!();
    rprintln!("spi_dual: entered main, rtt up");

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    // Take the two peripherals first (their accessors borrow `p` by reference),
    // then move the individual pads out of `p` below.
    let spi0 = p.spi0();
    let spi1 = p.spi1();

    // SPI1 as a slave, parked in its own task so it can receive concurrently with
    // the master transfer below.
    let slave = hal::spi::SpiSlave::new_async(
        spi1,
        p.PB2,
        pins::b::Pin2Func::Spi1Sclk,
        p.PB4,
        pins::b::Pin4Func::Spi1Mosi,
        p.PB3,
        pins::b::Pin3Func::Spi1Miso,
        p.PB5,
        pins::b::Pin5Func::Spi1Ssel,
        hal::spi::Config::default(),
    );
    spawner.spawn(slave_receive(slave)).unwrap();
    rprintln!("spi_dual: slave task spawned");

    // SPI0 as the master, wired on the PM bank.
    let mut master = hal::spi::Spi::new_async(
        spi0,
        p.PM2,
        pins::m::Pin2Func::Spi0Sclk,
        p.PM5,
        pins::m::Pin5Func::Spi0Mosi,
        p.PM4,
        pins::m::Pin4Func::Spi0Miso,
        p.PM3,
        pins::m::Pin3Func::Spi0Ssel,
        hal::spi::Config::default().frequency(1_000_000),
    );

    let tx = [0x11u8, 0x22, 0x33, 0x44];
    let mut rx_dummy = [0u8; 4];

    // --- debug: confirm the RX-threshold interrupt actually reaches the NVIC ---
    // The blocking test proved `rfne` (STAT) goes high, but the async future
    // relies on the *RX-threshold interrupt* (IF.rfthr + IE.rfthr) to wake it.
    // Here we arm IE.rfthr, send one byte, wait for RX, then read back the IF
    // flag and NVIC pending state (NVIC stays masked so the ISR can't clear it
    // before we observe it).
    {
        let r = unsafe { &*hal::pac::SPI0::PTR };
        r.if_.write(|w| w.rfthr().set_bit()); // clear any stale RX flag
        r.ie.write(|w| w.rfthr().set_bit()); // enable RX-threshold interrupt
        rprintln!(
            "spi_dual: diag before: IF={:#x} IE={:#x} nvic_pend={}",
            r.if_.read().bits(),
            r.ie.read().bits(),
            cortex_m::peripheral::NVIC::is_pending(hal::interrupt::SPI0)
        );
        r.data.write(|w| unsafe { w.bits(tx[0] as u32) });
        let mut g = 0u32;
        while !r.stat.read().rfne().bit() && g < 5_000_000 {
            g += 1;
        }
        let _ = r.data.read().bits(); // drain the one byte
        rprintln!(
            "spi_dual: diag after byte: IF={:#x} IE={:#x} nvic_pend={} g={}",
            r.if_.read().bits(),
            r.ie.read().bits(),
            cortex_m::peripheral::NVIC::is_pending(hal::interrupt::SPI0),
            g
        );
    }

    rprintln!("spi_dual: starting master transfer of {:?}", &tx);
    // Drive the bus: SPI0 clocks out `tx` on MOSI; SPI1 captures it on its own
    // MOSI line. (The bytes read back into `rx_dummy` come from SPI1's MISO,
    // which is left open in this one-way master->slave demo.)
    master.transfer(&mut rx_dummy, &tx).await;
    rprintln!("spi_dual: master transfer returned; master RX dummy = {:?}", &rx_dummy);

    // --- debug: did SPI1 (slave) ever see a CS edge? IF bit10 = ssfall, bit11 = ssrise ---
    {
        let s = unsafe { &*hal::pac::SPI1::PTR };
        rprintln!(
            "spi_dual: SPI1 after transfer: IF={:#x} IE={:#x}",
            s.if_.read().bits(),
            s.ie.read().bits()
        );
    }

    rprintln!("SPI0 (master) sent: {:?}", &tx);
    loop {}
}
