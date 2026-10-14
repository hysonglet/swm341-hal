#![no_std]
#![no_main]

//! Blocking DMA example: copy 16 bytes from one buffer to another on channel 0.

use panic_halt as _;

use swm341_hal as hal;
use hal::dma::Dma;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    let mut dma = Dma::new(p.dma(), Default::default());
    let src = [0xABu8; 16];
    let mut dst = [0u8; 16];
    let _ = dma.memcopy(0, &src, &mut dst);

    loop {}
}
