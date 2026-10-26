#![no_std]
#![no_main]

//! Async TFT-LCD animated gradient, ported from `lcdc_gradient` but driven by
//! the embassy executor instead of a busy loop.
//!
//! The bring-up flow is identical to the blocking example: SDRAM framebuffer,
//! the parallel RGB bus routed through [`LcdPins`], a 1 ms reset pulse and the
//! backlight on. The difference is the run loop: instead of spinning on
//! `cortex_m::asm::delay` to pace the frames, the async [`Lcd`] is created with
//! [`Lcd::new_with_pins_async`] and each frame is gated on
//! [`Lcd::wait_frame`] -- an interrupt-backed future that resolves at the
//! frame-done edge, i.e. inside the vertical blanking interval. That hands the
//! CPU back to the executor between frames instead of burning it in a delay,
//! which is what `wait_frame` is for.
//!
//! The framebuffer is still in-place (no double buffering), so like
//! `lcdc_gradient` this demo does not eliminate tearing -- it just paces the
//! repaint on the frame boundary. Swap to two buffers and flip the layer's
//! `framebuffer` pointer after `wait_frame` to get a tear-free result.
//!
//! The pinout follows the vendor `SimplLCD_RGB` example (24-bit data bus,
//! PB2/3/4/5 = VSYNC/HSYNC/DEN/DCLK). The reset/backlight pads are
//! board-specific GPIOs: here `PD1` is reset and `PD9` is backlight, matching
//! the SWM34SVE 100-pin demo board; change them to `PD1`/`PB13` for the 64-pin
//! board.

use embassy_executor::Spawner;
use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m::asm::delay;

use hal::gpio::{Level, Output, OutputConfig};
use hal::lcdc::{Config, HvTiming, Lcd, LcdPins, PixelFormat};
use hal::sdramc::{CasLatency, SdramPins, Sdramc};
use swm341_hal as hal;

/// Core clock used for the 1 ms reset pulse below (matches `hrc_150mhz`).
const HCLK_HZ: u32 = 150_000_000;

/// Panel geometry.
const WIDTH: usize = 800;
const HEIGHT: usize = 480;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
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

    // 1. SDRAM: the framebuffer lives here.
    let sdram = Sdramc::new_with_pins(
        sdram_peri,
        SdramPins::default(),
        hal::sdramc::Config::default()
            .cas_latency(CasLatency::Latency3)
            .trc(8),
    );
    rprintln!(
        "SDRAM up: {:>9} bytes @ 0x{:08X}",
        sdram.size_bytes(),
        sdram.base_address()
    );

    // 2. Route the parallel RGB bus and take the controller in async mode.
    let lcd = Lcd::new_with_pins_async(lcd_peri, LcdPins::default());

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
    //    `SampleEdge = LCD_SAMPLE_FALL`.
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

    let fb = hal::sdramc::SDRAM_BASE as *mut u16;

    // 6. Paint the first gradient frame into the framebuffer *before* starting
    //    the scan-out, so the panel never shows stale SDRAM contents.
    draw_gradient(fb, 0);

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
    rprintln!("LCD scanning out -- drawing gradient (async)");

    // 7. Animation loop: repaint the framebuffer with a diagonally flowing
    //    rainbow gradient and gate each frame on the frame-done interrupt
    //    instead of a busy delay. The future yields the executor, so any other
    //    spawned task runs between frames.
    let mut frame: u32 = 1;
    loop {
        draw_gradient(fb, frame);
        frame = frame.wrapping_add(1);

        lcd.wait_frame().await;

        if frame % 60 == 0 {
            rprintln!("frame {}", frame);
        }
    }
}

/// Paint a diagonally flowing RGB565 rainbow gradient into `fb`.
///
/// `t` shifts the pattern diagonally each frame. The rainbow is built from three
/// phase-offset triangular waves so it stays smooth on a 5/6/5 color bus without
/// any floating point or trig tables.
#[inline(always)]
fn draw_gradient(fb: *mut u16, t: u32) {
    for y in 0..HEIGHT as u32 {
        for x in 0..WIDTH as u32 {
            // Vertical gradient: the phase depends only on the row `y` (and the
            // frame counter `t`), so each row is a solid color and the rainbow
            // sweeps top-to-bottom, scrolling downward over time.
            let s = (y.wrapping_mul(256) / HEIGHT as u32).wrapping_add(t) & 0xFF;
            let r = tri(s);
            let g = tri(s.wrapping_add(0x55));
            let b = tri(s.wrapping_add(0xAA));
            // 8-bit -> RGB565 (R 5 bit, G 6 bit, B 5 bit).
            let rgb = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3);
            unsafe {
                fb.add((y as usize) * WIDTH + (x as usize))
                    .write_volatile(rgb as u16);
            }
        }
    }
}

/// Triangular wave 0..255..0 over an 8-bit input (period 256).
///
/// The caller passes `s.wrapping_add(offset)`, which can exceed 255, so mask to
/// 8 bits first -- otherwise `255 - v` underflows on a `u32` and, in a debug
/// build, the resulting panic would halt the CPU before the framebuffer is ever
/// painted (the screen would stay black).
#[inline(always)]
fn tri(v: u32) -> u32 {
    let v = v & 0xFF;
    if v < 128 {
        v * 2
    } else {
        (255 - v) * 2
    }
}
