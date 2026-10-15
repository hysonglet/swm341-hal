#![no_std]
#![no_main]

//! Blocking DMA example: copy 16 bytes from one buffer to another on channel 0.
//! The source/destination contents and the transfer result are printed over RTT.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m_rt::entry;
use swm341_hal as hal;
use hal::dma::Dma;

#[entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    rprintln!("dma_block: started, copying 16 bytes on channel 0");

    let mut dma = Dma::new(p.dma(), Default::default());
    let src = [0xABu8; 16];
    let mut dst = [0u8; 16];

    rprintln!("src = {:02x?}", &src[..]);

    match dma.memcopy(0, &src, &mut dst) {
        Ok(()) => {
            rprintln!("dma_block: memcopy done");
            rprintln!("dst = {:02x?}", &dst[..]);
            let equal = src.iter().zip(dst.iter()).all(|(a, b)| a == b);
            rprintln!("src == dst: {}", equal);
        }
        Err(e) => rprintln!("dma_block: memcopy failed: {:?}", e),
    }

    loop {}
}
