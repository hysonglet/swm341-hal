//! TFT LCD controller (LCD).
//!
//! The SWM341 LCDC drives a parallel RGB TFT panel. [`Lcd::configure`] programs
//! the dot-clock and the horizontal/vertical timing; [`Lcd::set_layer`] places
//! one of the two overlay layers over the background color; [`Lcd::start_frame`]
//! kicks off a frame scan-out. With the `embassy` feature a [`Lcd::wait_frame`]
//! future sleeps until the controller signals that the frame completed.
//!
//! Two constructors exist because the bus is spread over four GPIO banks:
//! [`Lcd::new_with_pins`] routes every pad of the reference wiring onto the
//! controller, while [`Lcd::new`] leaves the pads alone for boards whose
//! bootloader already routed them.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let lcd = hal::lcdc::Lcd::new(p.lcd());
//! lcd.configure(&hal::lcdc::Config::default());
//! // Place a layer over the whole panel and point it at a framebuffer.
//! let layer = hal::lcdc::LayerConfig {
//!     enable: true,
//!     x_start: 0,
//!     x_stop: 480,
//!     y_start: 0,
//!     y_stop: 272,
//!     framebuffer: 0x8000_0000,
//!     stride_bytes: 480 * 2,
//!     alpha: 255,
//!     color_key: None,
//! };
//! lcd.set_layer(0, &layer);
//! lcd.start_frame();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{self, Flex};
use crate::mode::{Blocking, Mode};

use instance::{IF_ERROR, IF_FRAMEDONE, Instance};

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
pub mod types;
pub use types::*;

/// Build the pad bundle, the routed-pad holder and the routing helper.
///
/// The parallel bus needs 28 pads over four banks, and it has to stay muxed for
/// as long as the controller scans out frames, so the list lives in one macro:
/// it declares [`LcdPins`] (what the caller hands in), a private
/// [`RoutedPins`] holding one [`Flex`] per pad (what keeps the mux and the bank
/// clock alive), and the code turning the former into the latter.
macro_rules! lcd_pins {
    ( $( $name:ident : $pin:ident = $func:expr , )+ ) => {
        /// Every pad of the LCD interface, in the wiring of the vendor
        /// `SimplLCD_RGB` example.
        ///
        /// Hand this to [`Lcd::new_with_pins`]; the pads are then muxed onto the
        /// controller and stay that way for as long as the handle lives. The
        /// full 24-bit data bus is routed even though RGB565 and YUV422 only
        /// drive part of it, because that is how the panel is wired.
        ///
        /// In MPU mode the same pads carry the memory-bus strobes: `dclk` is
        /// `RD`, `den` is `RS`, `hsync` is `WR` and `vsync` is `CS` -- see
        /// [`pins`] for the alternative spellings and their alternate pads.
        pub struct LcdPins {
            $( pub $name: gpio::$pin, )+
        }

        /// The same pads, already routed, kept alive by the driver.
        ///
        /// Nothing ever reads these fields: holding the [`Flex`] is what keeps
        /// the mux setting and the bank clock in place.
        #[allow(dead_code)]
        struct RoutedPins<'d> {
            $( $name: Flex<'d, gpio::$pin>, )+
        }

        impl<'d> RoutedPins<'d> {
            /// Mux every pad of `pins` onto the LCD controller.
            fn route(pins: LcdPins) -> Self {
                $(
                    let mut $name = Flex::new(pins.$name);
                    $name.set_func($func);
                )+
                Self { $( $name, )+ }
            }
        }
    };
}

lcd_pins! {
    vsync: PB2  = pins::b::Pin2Func::LcdVsync,
    hsync: PB3  = pins::b::Pin3Func::LcdHsync,
    den:   PB4  = pins::b::Pin4Func::LcdDen,
    dclk:  PB5  = pins::b::Pin5Func::LcdDclk,
    r0:    PC4  = pins::c::Pin4Func::LcdR0,
    r1:    PC5  = pins::c::Pin5Func::LcdR1,
    r2:    PC8  = pins::c::Pin8Func::LcdR2,
    r3:    PC9  = pins::c::Pin9Func::LcdR3,
    r4:    PC10 = pins::c::Pin10Func::LcdR4,
    r5:    PC11 = pins::c::Pin11Func::LcdR5,
    r6:    PC12 = pins::c::Pin12Func::LcdR6,
    r7:    PC13 = pins::c::Pin13Func::LcdR7,
    g0:    PA12 = pins::a::Pin12Func::LcdG0,
    g1:    PA13 = pins::a::Pin13Func::LcdG1,
    g2:    PA14 = pins::a::Pin14Func::LcdG2,
    g3:    PA15 = pins::a::Pin15Func::LcdG3,
    g4:    PC0  = pins::c::Pin0Func::LcdG4,
    g5:    PC1  = pins::c::Pin1Func::LcdG5,
    g6:    PC2  = pins::c::Pin2Func::LcdG6,
    g7:    PC3  = pins::c::Pin3Func::LcdG7,
    b0:    PB1  = pins::b::Pin1Func::LcdB0,
    b1:    PB11 = pins::b::Pin11Func::LcdB1,
    b2:    PB13 = pins::b::Pin13Func::LcdB2,
    b3:    PB15 = pins::b::Pin15Func::LcdB3,
    b4:    PA2  = pins::a::Pin2Func::LcdB4,
    b5:    PA9  = pins::a::Pin9Func::LcdB5,
    b6:    PA10 = pins::a::Pin10Func::LcdB6,
    b7:    PA11 = pins::a::Pin11Func::LcdB7,
}

/// A handle to the LCD controller.
#[allow(dead_code)]
pub struct Lcd<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    /// Pads routed by [`Lcd::new_with_pins`]; `None` when the caller muxed them
    /// some other way.
    pins: Option<RoutedPins<'d>>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Lcd<'d, T, M> {
    /// Open the clock gate and put the routed pads, if any, behind the handle.
    fn build(peri: PeripheralRef<'d, T>, pins: Option<RoutedPins<'d>>) -> Self {
        T::clock().open();
        Self {
            peri,
            pins,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, M: Mode> Drop for Lcd<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Lcd<'d, T, Blocking> {
    /// Take ownership of the LCD controller and open its clock gate.
    ///
    /// The pads are left alone: use this when something else already muxed them
    /// onto the controller, otherwise [`Lcd::new_with_pins`].
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        Self::build(peri, None)
    }

    /// Mux every pad of `pins` onto the controller and take ownership of it.
    pub fn new_with_pins(peri: impl Peripheral<P = T> + 'd, pins: LcdPins) -> Self {
        into_ref!(peri);
        Self::build(peri, Some(RoutedPins::route(pins)))
    }

    /// Configure the controller timing, format and background color.
    pub fn configure(&self, config: &Config) {
        T::set_control(
            config.clock_div,
            config.format == PixelFormat::Yuv422,
            config.invert_clock,
            config.invert_vsync,
            config.invert_hsync,
            config.serial,
            config.mpu_enable,
            config.auto_restart,
        );
        T::set_timing(&config.h, &config.v);
        T::set_bg_color(config.bg_color);
    }

    /// Configure one of the two overlay layers.
    pub fn set_layer(&self, layer: usize, config: &LayerConfig) {
        T::set_layer(layer, config);
    }

    /// Set the RGB565 background color.
    pub fn set_bg_color(&self, color: u32) {
        T::set_bg_color(color);
    }

    /// Launch a single frame scan-out.
    pub fn start_frame(&self) {
        T::start_frame();
    }

    /// True once the most recent frame completed.
    pub fn is_frame_done(&self) -> bool {
        T::is_frame_done()
    }

    /// Clear the frame-done flag.
    pub fn clear_frame_done(&self) {
        T::clear_interrupt(IF_FRAMEDONE);
    }

    /// Clear the error flag.
    pub fn clear_error(&self) {
        T::clear_interrupt(IF_ERROR);
    }

    /// True on a transfer / underrun error.
    pub fn is_error(&self) -> bool {
        T::is_error()
    }

    /// Enable or disable the frame-done interrupt.
    pub fn set_frame_interrupt(&self, enable: bool) {
        T::set_interrupt_enable(IF_FRAMEDONE, enable);
    }

    /// Enable or disable the error interrupt.
    pub fn set_error_interrupt(&self, enable: bool) {
        T::set_interrupt_enable(IF_ERROR, enable);
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance> Lcd<'d, T, Async> {
    /// Async constructor: like [`new`](Self::new) but bound to `Async` so the
    /// `wait_frame` future is available.
    pub fn new_async(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        Self::build(peri, None)
    }

    /// Async constructor: like [`new_with_pins`](Self::new_with_pins) but bound
    /// to `Async` so the `wait_frame` future is available.
    pub fn new_with_pins_async(peri: impl Peripheral<P = T> + 'd, pins: LcdPins) -> Self {
        into_ref!(peri);
        Self::build(peri, Some(RoutedPins::route(pins)))
    }

    /// Wait until the controller signals that a frame has been completed.
    pub async fn wait_frame(&self) {
        future::wait_frame::<T>().await;
    }
}
