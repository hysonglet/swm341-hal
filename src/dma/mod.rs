//! Direct memory access (DMA) controller driver.
//!
//! The [`Dma`] object wraps the single DMA controller and lets you run transfers
//! on one of its four channels. The simplest use is [`Dma::memcopy`], a blocking
//! memory-to-memory copy; [`Dma::start`] exposes the raw channel setup for
//! peripheral-backed transfers (supply the DMA mux request number of the
//! peripheral on the handshake side).
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut dma = hal::dma::Dma::new(p.dma(), Default::default());
//! let src = [0xABu8; 16];
//! let mut dst = [0u8; 16];
//! dma.memcopy(0, &src, &mut dst).unwrap();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

pub mod instance;
mod types;

pub use types::*;

/// A DMA controller instance.
#[allow(dead_code)]
pub struct Dma<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Dma<'d, T, M> {
    /// Take ownership of the DMA controller.
    pub fn new(peri: impl Peripheral<P = T> + 'd, _config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::enable();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, M: Mode> Drop for Dma<'d, T, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
    }
}

impl<'d, T: Instance> Dma<'d, T, Blocking> {
    /// Program and start a channel for a transfer from `src` to `dst`.
    ///
    /// `req` is the DMA mux request number of the handshake peripheral on the
    /// relevant side; pass `None` for a software-triggered memory-to-memory copy.
    pub fn start(
        &mut self,
        ch: u8,
        src: *const u8,
        dst: *mut u8,
        len: usize,
        req: Option<u8>,
    ) -> Result<(), Error> {
        if ch > 3 {
            return Err(Error::InvalidChannel);
        }
        if len == 0 {
            return Ok(());
        }
        let len = len.min(u16::MAX as usize) as u32;
        T::configure_channel(ch, src as u32, dst as u32, len, req);
        T::clear_channel_done(ch);
        T::enable_channel(ch);
        Ok(())
    }

    /// Blocking memory-to-memory copy. `dst` receives the contents of `src`
    /// (up to the shorter of the two slices).
    pub fn memcopy(&mut self, ch: u8, src: &[u8], dst: &mut [u8]) -> Result<(), Error> {
        let len = src.len().min(dst.len());
        if len == 0 {
            return Ok(());
        }
        self.start(ch, src.as_ptr(), dst.as_mut_ptr(), len, None)?;

        let mut timeout = 10_000_000u32;
        while !T::channel_done(ch) {
            timeout -= 1;
            if timeout == 0 {
                T::disable_channel(ch);
                return Err(Error::Timeout);
            }
        }
        T::clear_channel_done(ch);
        Ok(())
    }
}
