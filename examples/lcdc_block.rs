#![no_std]
#![no_main]

//! Blocking TFT-LCD bring-up, ported from the vendor `SimplLCD_RGB` / Synwit
//! `swm341-lvgl-rtt` board code.
//!
//! Order mirrors the C `main()`: bring up SDRAM first (the framebuffer lives
//! there), pulse the panel reset, turn on the backlight, route the 28-pad
//! parallel RGB bus through [`LcdPins`], configure an 800x480 RGB565 panel and
//! start the scan-out. Progress is printed over RTT.
//!
//! The pinout follows the vendor `SimplLCD_RGB` example (24-bit data bus, PB2/3/4/5
//! = VSYNC/HSYNC/DEN/DCLK). The reset/backlight pads are board-specific GPIOs:
//! here `PD1` is reset and `PD9` is backlight, matching the SWM34SVE 100-pin demo
//! board; change them to `PD1`/`PB13` for the 64-pin board.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m::asm::delay;

use swm341_hal as hal;
use hal::gpio::{Level, Output, OutputConfig};
use hal::lcdc::{Config, HvTiming, Lcd, LcdPins, PixelFormat};
use hal::sdramc::{CasLatency, SdramPins, Sdramc};

/// Core clock used for the 1 ms reset pulse below (matches `hrc_150mhz`).
const HCLK_HZ: u32 = 150_000_000;

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });
    rprintln!("HAL initialised");

    // Grab the peripheral handles before moving any GPIO field out of `p`
    // (each `p.Pxy` move would otherwise forbid the later `p.lcd()`/`p.sdramc()`
    // borrows). The handles are stolen singletons, so they don't move `p`.
    let lcd_peri = p.lcd();
    let sdram_peri = p.sdramc();

    // 1. SDRAM: the framebuffer (and its clear below) live here.
    let sdram = Sdramc::new_with_pins(
        sdram_peri,
        SdramPins {
            clk: p.PM13,
            cke: p.PM14,
            we: p.PB7,
            cas: p.PB8,
            ras: p.PB9,
            cs: p.PB10,
            ba0: p.PE15,
            ba1: p.PE14,
            ldqm: p.PB6,
            udqm: p.PM12,
            a0: p.PN14,
            a1: p.PN13,
            a2: p.PN12,
            a3: p.PN11,
            a4: p.PN10,
            a5: p.PN9,
            a6: p.PN8,
            a7: p.PN7,
            a8: p.PN6,
            a9: p.PN3,
            a10: p.PN15,
            a11: p.PN2,
            a12: p.PM15,
            d0: p.PE7,
            d1: p.PE6,
            d2: p.PE5,
            d3: p.PE4,
            d4: p.PE3,
            d5: p.PE2,
            d6: p.PE1,
            d7: p.PE0,
            d8: p.PE8,
            d9: p.PE9,
            d10: p.PE10,
            d11: p.PE11,
            d12: p.PE12,
            d13: p.PE13,
            d14: p.PC14,
            d15: p.PC15,
        },
        hal::sdramc::Config::default()
            .cas_latency(CasLatency::Latency3)
            .trc(8),
    );
    rprintln!(
        "SDRAM up: {:>9} bytes @ 0x{:08X}",
        sdram.size_bytes(),
        sdram.base_address()
    );

    // 2. Route the parallel RGB bus and take the controller. This borrows `p`
    //    only to steal the peripheral, so it must happen before any GPIO field
    //    of `p` is moved below.
    let lcd = Lcd::new_with_pins(
        lcd_peri,
        LcdPins {
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

    // 3. Reset pulse: hold low ~1 ms, then release.
    let mut lcd_rst = Output::new(p.PD1, OutputConfig::default().initial_level(Level::Low));
    delay(HCLK_HZ / 1000);
    lcd_rst.set_high();

    // 4. Backlight on (SWM34SVE 100-pin board: PD9; 64-pin board uses PB13).
    let mut lcd_bl = Output::new(p.PD9, OutputConfig::default().initial_level(Level::High));
    let _ = &mut lcd_bl; // kept alive for as long as the panel is lit
    rprintln!("LCD reset released, backlight on");

    // 5. 800x480 RGB565 timing of the vendor demo. DOTCLK = HCLK / (clock_div + 2)
    //    = 150 MHz / 5 = 30 MHz at clock_div = 3; `invert_clock` is the vendor
    //    `SampleEdge = LCD_SAMPLE_FALL`. (25 MHz via clock_div = 4 also lights the
    //    panel but tends to flicker on this board -- 30 MHz matches the synwit
    //    reference.)
    lcd.configure(&Config {
        clock_div: 3,
        format: PixelFormat::Rgb565,
        invert_clock: true,
        invert_vsync: false,
        invert_hsync: false,
        serial: false,
        mpu_enable: false,
        auto_restart: true,
        h: HvTiming {
            sync: 5,
            back_porch: 46,
            pixels: 800,
            front_porch: 64,
        },
        v: HvTiming {
            sync: 5,
            back_porch: 23,
            pixels: 480,
            front_porch: 22,
        },
        bg_color: 0xFFFFFF,
    });
    rprintln!("LCD configured: 800x480 RGB565, auto-restart on");

    // 6. Solid red start-up screen (RGB565 0xF800) so the panel isn't showing
    //    SDRAM garbage and you can confirm the timing/route is correct. Swap to
    //    0x0000 (black), 0x07E0 (green) or 0x001F (blue) to sanity-check.
    const WIDTH: usize = 800;
    const HEIGHT: usize = 480;
    let fb = hal::sdramc::SDRAM_BASE as *mut u16;
    let pixels = WIDTH * HEIGHT;
    for i in 0..pixels {
        unsafe {
            fb.add(i).write_volatile(0xF800);
        }
    }

    lcd.set_layer(
        0,
        &hal::lcdc::LayerConfig {
            enable: true,
            x_start: 0,
            x_stop: WIDTH as u16 - 1,
            y_start: 0,
            y_stop: HEIGHT as u16 - 1,
            framebuffer: hal::sdramc::SDRAM_BASE as u32,
            stride_bytes: WIDTH as u32 * 2,
            alpha: 255,
            color_key: None,
        },
    );
    lcd.start_frame();
    rprintln!("LCD scanning out");

    loop {}
}
