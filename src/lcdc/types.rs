//! TFT LCD controller configuration types.

/// Horizontal or vertical blanking / timing parameters, in pixels (or lines).
#[derive(Clone, Copy, Debug)]
pub struct HvTiming {
    /// Sync pulse width (HSW for horizontal, VSW for vertical).
    pub sync: u16,
    /// Back porch (HBP / VBP).
    pub back_porch: u16,
    /// Active pixels / lines.
    pub pixels: u16,
    /// Front porch (HFP / VFP).
    pub front_porch: u16,
}

/// Pixel format written to `CR.FORMAT`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelFormat {
    /// 16-bit RGB565.
    Rgb565 = 0,
    /// YUV422 (byte-interleaved).
    Yuv422 = 1,
}

/// Configuration of the LCD controller.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Dot-clock divisor register value (`CR.CLKDIV`, 0..=63).
    ///
    /// The SWM341 divides `HCLK` by `clock_div + 2`, so `0` gives `/2`, `1` gives
    /// `/3`, and so on. For a 150 MHz `HCLK`, `3` yields 30 MHz DOTCLK.
    pub clock_div: u8,
    /// Pixel format.
    pub format: PixelFormat,
    /// Invert the dot clock (`CR.CLKINV`).
    pub invert_clock: bool,
    /// Invert VSYNC (`CR.VSYNCINV`).
    pub invert_vsync: bool,
    /// Invert HSYNC (`CR.HSYNCINV`).
    pub invert_hsync: bool,
    /// Serial (RGB dot-matrix) interface mode (`CR.SEREN`).
    pub serial: bool,
    /// Enable the external memory (MPU) bus interface (`CR.MPUEN`).
    pub mpu_enable: bool,
    /// Auto-restart the scan-out after every completed frame (`CR.AUTORESTA`).
    ///
    /// This mirrors the vendor `LCD_Init` `IntEOTEn == 0` behaviour: with it on
    /// the controller keeps refreshing the panel on its own, which is what a
    /// plain `start_frame()` bring-up wants. Turn it off when you drive the
    /// panel from an end-of-frame interrupt and re-trigger `start_frame()`
    /// yourself (the async `wait_frame` path expects this).
    pub auto_restart: bool,
    /// Horizontal timing.
    pub h: HvTiming,
    /// Vertical timing.
    pub v: HvTiming,
    /// RGB565 background color shown behind disabled layers (`BGC`).
    pub bg_color: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            clock_div: 1,
            format: PixelFormat::Rgb565,
            invert_clock: false,
            invert_vsync: false,
            invert_hsync: false,
            serial: false,
            mpu_enable: false,
            auto_restart: true,
            h: HvTiming {
                sync: 1,
                back_porch: 46,
                pixels: 480,
                front_porch: 16,
            },
            v: HvTiming {
                sync: 1,
                back_porch: 23,
                pixels: 272,
                front_porch: 4,
            },
            bg_color: 0,
        }
    }
}

/// Configuration of one overlay layer (L0 or L1).
#[derive(Clone, Copy, Debug)]
pub struct LayerConfig {
    /// Enable the layer.
    pub enable: bool,
    /// Window horizontal start, in pixels.
    pub x_start: u16,
    /// Window horizontal stop, in pixels.
    pub x_stop: u16,
    /// Window vertical start, in lines.
    pub y_start: u16,
    /// Window vertical stop, in lines.
    pub y_stop: u16,
    /// Framebuffer base address (`ADDR`).
    pub framebuffer: u32,
    /// Bytes per scanline (`LLEN`).
    pub stride_bytes: u32,
    /// Global alpha, 0 = transparent .. 255 = opaque (`LCR.ALPHA`).
    pub alpha: u8,
    /// Optional color key (`CK`); `None` disables color-keying.
    pub color_key: Option<u32>,
}
