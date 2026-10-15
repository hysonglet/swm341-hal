//! TFT LCD controller (LCD).
//!
//! The SWM341 LCDC drives a parallel RGB TFT panel. [`Lcd::configure`] programs
//! the dot-clock and the horizontal/vertical timing; [`Lcd::set_layer`] places
//! one of the two overlay layers over the background color; [`Lcd::start_frame`]
//! kicks off a frame scan-out. With the `embassy` feature a [`Lcd::wait_frame`]
//! future sleeps until the controller signals that the frame completed.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let lcd = hal::lcdc::Lcd::new(p.LCD);
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

use crate::mode::{Blocking, Mode};

use instance::{IF_ERROR, IF_FRAMEDONE, Instance};

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod types;
pub use types::*;

/// A handle to the LCD controller.
#[allow(dead_code)]
pub struct Lcd<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Lcd<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Lcd<'d, T, Blocking> {
    /// Take ownership of the LCD controller and open its clock gate.
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        T::clock().open();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
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
        T::clock().open();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Wait until the controller signals that a frame has been completed.
    pub async fn wait_frame(&self) {
        future::wait_frame::<T>().await;
    }
}
