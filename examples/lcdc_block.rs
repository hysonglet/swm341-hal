#![no_std]
#![no_main]

//! Blocking TFT-LCD sample: route the parallel RGB bus and scan out one frame.
//!
//! The 28 pads are muxed through [`swm341_hal::lcdc::LcdPins`] (wired like the
//! vendor `SimplLCD_RGB` example) before the controller is touched. The frame
//! buffer is assumed to live in SDRAM at `0x8000_0000` (bring it up with
//! [`swm341_hal::sdramc`] first on a real board); here it is only pointed at,
//! not allocated.

use panic_halt as _;

use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let lcd = hal::lcdc::Lcd::new_with_pins(
        p.lcd(),
        hal::lcdc::LcdPins {
            vsync: p.PB2,
            hsync: p.PB3,
            den: p.PB4,
            dclk: p.PB5,
            r0: p.PC4,
            r1: p.PC5,
            r2: p.PC8,
            r3: p.PC9,
            r4: p.PC10,
            r5: p.PC11,
            r6: p.PC12,
            r7: p.PC13,
            g0: p.PA12,
            g1: p.PA13,
            g2: p.PA14,
            g3: p.PA15,
            g4: p.PC0,
            g5: p.PC1,
            g6: p.PC2,
            g7: p.PC3,
            b0: p.PB1,
            b1: p.PB11,
            b2: p.PB13,
            b3: p.PB15,
            b4: p.PA2,
            b5: p.PA9,
            b6: p.PA10,
            b7: p.PA11,
        },
    );

    // RGB565, 480x272 panel, 6 MHz dot clock off the 150 MHz core.
    lcd.configure(&hal::lcdc::Config::default());

    let layer = hal::lcdc::LayerConfig {
        enable: true,
        x_start: 0,
        x_stop: 480,
        y_start: 0,
        y_stop: 272,
        framebuffer: 0x8000_0000,
        stride_bytes: 480 * 2,
        alpha: 255,
        color_key: None,
    };
    lcd.set_layer(0, &layer);
    lcd.start_frame();

    loop {}
}
