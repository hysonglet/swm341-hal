//! 2D DMA / graphics accelerator (DMA2D).
//!
//! The DMA2D moves rectangular blocks of pixel data between memory regions, with
//! optional pixel-format conversion and two-layer blending. On the SWM341 the
//! destination is layer `L0` and the sources are layers `L1`/`L2`; each layer
//! carries a memory address (`MAR`), a line offset (`OR`), a pixel-format word
//! (`PFCCR`) and a constant colour (`COLOR`). [`Dma2d::copy`] and
//! [`Dma2d::fill`] are the two common operations; [`Dma2d::layer`] exposes the
//! raw registers for blending and format conversion.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let dma2d = hal::dma2d::Dma2d::new(p.DMA2D);
//! let mut dst = [0u32; 64 * 64];
//! let src = [0x1122_3344u32; 64 * 64];
//! // Fill `dst` with a constant ARGB8888 colour.
//! dma2d.fill(dst.as_mut_ptr(), 0x00FF_00FF, 64, 64, 0);
//! ```
//!
//! All operations are synchronous and busy-wait on the `DONE` flag.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::{Instance, LayerId};

mod instance;

/// DMA2D transfer mode (`CR.MODE`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TransferMode {
    /// Memory-to-memory copy.
    MemoryToMemory = 0,
    /// Memory-to-memory copy with pixel-format conversion.
    MemoryToMemoryPfc = 1,
    /// Blend two source layers into the destination.
    Blend = 2,
    /// Fill the destination with a constant colour (`COLOR`).
    Fill = 3,
}

/// A handle to the DMA2D engine.
#[allow(dead_code)]
pub struct Dma2d<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Dma2d<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Dma2d<'d, T, Blocking> {
    /// Take ownership of the DMA2D engine and open its clock gate.
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        T::clock().open();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Direct access to one of the three layer register clusters.
    pub fn layer(&self, layer: LayerIndex) -> Layer<T> {
        Layer {
            layer: match layer {
                LayerIndex::L0 => LayerId::L0,
                LayerIndex::L1 => LayerId::L1,
                LayerIndex::L2 => LayerId::L2,
            },
            _marker: core::marker::PhantomData,
        }
    }

    /// Program the rectangle size: `lines` rows of `pixels` each.
    pub fn set_size(&self, lines: u16, pixels: u16) {
        T::set_size(lines, pixels);
    }

    /// Start a transfer of the given `mode`.
    pub fn start(&self, mode: TransferMode) {
        T::clear_done();
        T::start(mode as u8, 0);
    }

    /// True once the transfer has finished.
    pub fn is_done(&self) -> bool {
        T::is_done()
    }

    /// Wait (busy-spin) until the current transfer completes.
    pub fn wait(&self) {
        while !T::is_done() {}
    }

    /// Copy a `width` x `height` rectangle from `src` to `dst` (bytes per pixel
    /// implied by the caller-set `pfccr` of both layers). The destination is
    /// layer `L0`, the source layer `L1`.
    pub fn copy(&self, src: *const u32, dst: *mut u32, width: u16, height: u16) {
        T::set_layer_addr(LayerId::L1, src as u32);
        T::set_layer_addr(LayerId::L0, dst as u32);
        T::set_size(height, width);
        T::clear_done();
        T::start(TransferMode::MemoryToMemory as u8, 0);
        while !T::is_done() {}
    }

    /// Fill a `width` x `height` rectangle at `dst` with the constant `color`.
    /// The destination is layer `L0`; `pfccr` selects the pixel format.
    pub fn fill(&self, dst: *mut u32, color: u32, width: u16, height: u16, pfccr: u32) {
        T::set_layer_addr(LayerId::L0, dst as u32);
        T::set_layer_color(LayerId::L0, color);
        T::set_layer_pfccr(LayerId::L0, pfccr);
        T::set_size(height, width);
        T::clear_done();
        T::start(TransferMode::Fill as u8, 0);
        while !T::is_done() {}
    }
}

/// Which of the three DMA2D layers to address.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerIndex {
    /// Destination / output layer.
    L0,
    /// First source layer.
    L1,
    /// Second source layer.
    L2,
}

/// A typed view over one DMA2D layer's registers.
pub struct Layer<T: Instance> {
    layer: LayerId,
    _marker: core::marker::PhantomData<T>,
}

impl<T: Instance> Layer<T> {
    /// Set the layer's memory address (`MAR`).
    pub fn set_addr(&self, addr: u32) {
        T::set_layer_addr(self.layer, addr);
    }

    /// Set the layer's line offset (`OR`).
    pub fn set_offset(&self, offset: u32) {
        T::set_layer_offset(self.layer, offset);
    }

    /// Set the layer's constant/fill colour (`COLOR`).
    pub fn set_color(&self, color: u32) {
        T::set_layer_color(self.layer, color);
    }

    /// Set the layer's pixel-format / alpha control word (`PFCCR`).
    pub fn set_pfccr(&self, pfccr: u32) {
        T::set_layer_pfccr(self.layer, pfccr);
    }
}
