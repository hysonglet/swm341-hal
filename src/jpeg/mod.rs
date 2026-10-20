//! JPEG decoder (JPEG).
//!
//! The hardware decoder turns a compressed JPEG bitstream into planar YUV
//! (`YBASE`/`UBASE`/`VBASE`) or, with `CFG.YUV2RGB` set, into packed RGB
//! (`RGBASE`). The driver supplies the stream, the quantisation / Huffman table
//! bases, the output buffers and the image geometry, then [`Jpeg::decode`] starts
//! the engine and busy-waits on the done / error flags. The raw register
//! setters are also exposed for table streaming and partial decoding.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let jpeg = hal::jpeg::Jpeg::new(p.jpeg());
//! let (mut y, mut u, mut v) = ([0u8; 0], [0u8; 0], [0u8; 0]);
//! // Provide a real bitstream, tables and buffers in practice.
//! let cfg = hal::jpeg::DecodeConfig {
//!     code: core::ptr::null(),
//!     code_len: 0,
//!     quant_tables: core::ptr::null(),
//!     huffman_tables: core::ptr::null(),
//!     y: y.as_mut_ptr(),
//!     u: u.as_mut_ptr(),
//!     v: v.as_mut_ptr(),
//!     rgb: core::ptr::null_mut(),
//!     width: 0,
//!     height: 0,
//!     stride: 0,
//!     config: 0,
//! };
//! // let _ = jpeg.decode(&cfg);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the JPEG decoder.
#[allow(dead_code)]
pub struct Jpeg<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Jpeg<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Jpeg<'d, T, Blocking> {
    /// Take ownership of the JPEG decoder and open its clock gate.
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        T::clock().open();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Point the decoder at the compressed bitstream (`CSBASE` + `CODLEN`).
    pub fn set_stream(&self, base: *const u8, len: u32) {
        T::set_stream(base as u32, len);
    }

    /// Point the decoder at the quantisation and Huffman tables.
    pub fn set_tables(&self, quant: *const u8, huffman: *const u8) {
        T::set_tables(quant as u32, huffman as u32);
    }

    /// Point the decoder at the planar Y/U/V output buffers.
    pub fn set_yuv_buffers(&self, y: *mut u8, u: *mut u8, v: *mut u8) {
        T::set_yuv_buffers(y as u32, u as u32, v as u32);
    }

    /// Point the decoder at the packed RGB output buffer.
    pub fn set_rgb_buffer(&self, rgb: *mut u8) {
        T::set_rgb_buffer(rgb as u32);
    }

    /// Set the output image size in pixels.
    pub fn set_image_size(&self, width: u16, height: u16) {
        T::set_image_size(width, height);
    }

    /// Set the output stride.
    pub fn set_stride(&self, stride: u32) {
        T::set_stride(stride);
    }

    /// Write the raw decoder configuration word (`CFG`).
    pub fn set_config(&self, config: u32) {
        T::set_config(config);
    }

    /// Soft-reset the decoder.
    pub fn reset(&self) {
        T::reset();
    }

    /// True while the decoder is busy.
    pub fn is_busy(&self) -> bool {
        T::is_busy()
    }

    /// True once the decode finished.
    pub fn is_done(&self) -> bool {
        T::is_done()
    }

    /// True if the decode flagged an error.
    pub fn is_error(&self) -> bool {
        T::is_error()
    }

    /// Clear the done / error flags.
    pub fn clear_flags(&self) {
        T::clear_flags();
    }

    /// Decode one image synchronously.
    ///
    /// Configures the stream, tables, output buffers, size, stride and `CFG`, then
    /// starts the engine and busy-waits until it signals done or error.
    pub fn decode(&self, cfg: &DecodeConfig) -> Result<(), JpegError> {
        T::set_stream(cfg.code as u32, cfg.code_len);
        T::set_tables(cfg.quant_tables as u32, cfg.huffman_tables as u32);
        T::set_yuv_buffers(cfg.y as u32, cfg.u as u32, cfg.v as u32);
        if !cfg.rgb.is_null() {
            T::set_rgb_buffer(cfg.rgb as u32);
        }
        T::set_image_size(cfg.width, cfg.height);
        T::set_stride(cfg.stride);
        T::set_config(cfg.config);
        T::clear_flags();
        T::start();
        while T::is_busy() && !T::is_done() {
            core::hint::spin_loop();
        }
        if T::is_error() {
            T::clear_flags();
            return Err(JpegError::Decode);
        }
        Ok(())
    }
}
