//! Register access for the 2D DMA / graphics accelerator (DMA2D).
//!
//! The DMA2D moves pixel data between memory and memory, optionally converting
//! the pixel format or blending two source layers. The SWM341 models it as a set
//! of three layers `L0`/`L1`/`L2` (each `{MAR, OR, PFCCR, COLOR}`), a transfer
//! size register `NLR` and a control register `CR` (`START` / `MODE` / `WAIT`).
//! `IF` flags completion.
//!
//! The driver exposes the layer registers directly -- by convention on this chip
//! `L0` is the destination and `L1`/`L2` are the source layers -- plus the
//! typed `CR`/`NLR`/`IF` accessors, so the higher-level copy / fill / blend
//! operations can be built on top without guessing the bit layout of `PFCCR`.
//!
//! `CR`, `IF` and the per-layer registers are modelled by the PAC, so address
//! and colour fields are written through `bits()`.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the DMA2D engine.
pub(crate) type Block = pac::dma2d::RegisterBlock;

/// One of the three DMA2D layers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayerId {
    L0 = 0,
    L1 = 1,
    L2 = 2,
}

/// Run `$body` with `$c` bound to the concrete register cluster of `$layer`.
///
/// The three clusters (`L0`/`L1`/`L2`) are distinct types, so each match arm
/// gets its own concrete binding and the body resolves the register accessors.
macro_rules! with_layer {
    ($regs:expr, $layer:expr, |$c:ident| $body:expr) => {
        match $layer {
            LayerId::L0 => {
                let $c = &$regs.l0;
                $body
            }
            LayerId::L1 => {
                let $c = &$regs.l1;
                $body
            }
            LayerId::L2 => {
                let $c = &$regs.l2;
                $body
            }
        }
    };
}

pub(crate) mod sealed {

    use super::{Block, LayerId, PeripheralClockIndex};

    /// A DMA2D engine the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers the DMA2D engine.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Set a layer's source/destination memory address (`MAR`).
        fn set_layer_addr(layer: LayerId, addr: u32) {
            let regs = unsafe { &*Self::regs() };
            with_layer!(regs, layer, |c| c.mar.write(|w| unsafe { w.bits(addr) }));
        }

        /// Set a layer's line offset in bytes/format units (`OR`).
        fn set_layer_offset(layer: LayerId, offset: u32) {
            let regs = unsafe { &*Self::regs() };
            with_layer!(regs, layer, |c| c.or.write(|w| unsafe { w.bits(offset) }));
        }

        /// Set a layer's fill colour / constant colour (`COLOR`).
        fn set_layer_color(layer: LayerId, color: u32) {
            let regs = unsafe { &*Self::regs() };
            with_layer!(regs, layer, |c| c.color.write(|w| unsafe { w.bits(color) }));
        }

        /// Set a layer's pixel-format / alpha control word (`PFCCR`).
        fn set_layer_pfccr(layer: LayerId, pfccr: u32) {
            let regs = unsafe { &*Self::regs() };
            with_layer!(regs, layer, |c| c.pfccr.write(|w| unsafe { w.bits(pfccr) }));
        }

        /// Program the transfer size (`NLR`): `lines` rows of `pixels` each.
        fn set_size(lines: u16, pixels: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.nlr.write(|w| unsafe {
                w.nline().bits(lines);
                w.npixel().bits(pixels)
            });
        }

        /// Start a transfer with the given `mode`: `0` = memory-to-memory, `1` =
        /// memory-to-memory with pixel format conversion, `2` = blending, `3` =
        /// fill (register/colour to memory). `wait` inserts a small stall before the
        /// next AHB access.
        fn start(mode: u8, wait: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| unsafe {
                w.mode().bits(mode & 0x3);
                w.wait().bits(wait & 0x3ff);
                w.start().set_bit()
            });
        }

        /// True once the transfer has finished (`IF.DONE`).
        fn is_done() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().done().bit()
        }

        /// Clear the done flag (`IF.DONE`, write-1-to-clear).
        fn clear_done() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.done().set_bit());
        }
    }
}

#[allow(private_bounds)]
/// A DMA2D engine usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the DMA2D singleton.
macro_rules! impl_dma2d_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Dma2d
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> crate::interrupt::ExtendedInterrupt {
                crate::interrupt::$irq
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_dma2d_instance!(crate::Dma2d, DMA2D, DMA2D, 0);
