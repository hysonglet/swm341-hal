//! Register access for the JPEG decoder (JPEG).
//!
//! The decoder pulls a compressed bitstream from `CSBASE` (length `CODLEN`),
//! reads the quantisation tables from `QTBASE` and the Huffman tables from
//! `HTBASE`, and writes the decoded pixels to `YBASE`/`UBASE`/`VBASE` (planar
//! YUV) or to `RGBASE` when `CFG.YUV2RGB` is set. `IMGSIZ`/`IMGSTR` bound the
//! output, `CR.START` launches a decode and `SR.BUSY` clears when `IR.IFDONE`
//! raises. The address / length registers are raw, so they are written through
//! `bits()`; `CFG`, `CR`, `SR` and `IR` use the typed accessors.

use cortex_m::interrupt::InterruptNumber;

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the JPEG decoder.
pub(crate) type Block = pac::jpeg::RegisterBlock;

/// Register access shared by the JPEG decoder.
pub(crate) mod hw {
    use super::Block;

    /// Set the compressed bitstream base address and length.
    pub(crate) fn set_stream(regs: &Block, base: u32, len: u32) {
        regs.csbase.write(|w| unsafe { w.bits(base) });
        regs.codlen.write(|w| unsafe { w.bits(len) });
    }

    /// Set the quantisation-table and Huffman-table base addresses.
    pub(crate) fn set_tables(regs: &Block, quant: u32, huffman: u32) {
        regs.qtbase.write(|w| unsafe { w.bits(quant) });
        regs.htbase.write(|w| unsafe { w.bits(huffman) });
    }

    /// Set the Y/U/V planar output buffers.
    pub(crate) fn set_yuv_buffers(regs: &Block, y: u32, u: u32, v: u32) {
        regs.ybase().write(|w| unsafe { w.bits(y) });
        regs.ubase.write(|w| unsafe { w.bits(u) });
        regs.vbase.write(|w| unsafe { w.bits(v) });
    }

    /// Set the packed RGB output buffer (used when `CFG.YUV2RGB` is set).
    pub(crate) fn set_rgb_buffer(regs: &Block, rgb: u32) {
        regs.rgbase().write(|w| unsafe { w.bits(rgb) });
    }

    /// Set the output image size in pixels (`width` x `height`).
    pub(crate) fn set_image_size(regs: &Block, width: u16, height: u16) {
        regs.imgsiz.write(|w| unsafe {
            w.hpix().bits(width);
            w.vpix().bits(height)
        });
    }

    /// Set the output stride (`IMGSTR`, bytes/units per row).
    pub(crate) fn set_stride(regs: &Block, stride: u32) {
        regs.imgstr.write(|w| unsafe { w.bits(stride) });
    }

    /// Write the decoder configuration word (`CFG`).
    pub(crate) fn set_config(regs: &Block, config: u32) {
        regs.cfg.write(|w| unsafe { w.bits(config) });
    }

    /// Soft-reset the decoder (`CR.RESET`).
    pub(crate) fn reset(regs: &Block) {
        regs.cr.write(|w| w.reset().set_bit());
    }

    /// Start a decode (`CR.START`).
    pub(crate) fn start(regs: &Block) {
        regs.cr.write(|w| w.start().set_bit());
    }

    /// True while the decoder is busy (`SR.BUSY`).
    pub(crate) fn is_busy(regs: &Block) -> bool {
        regs.sr.read().busy().bit()
    }

    /// True when the decode finished (`IR.IFDONE`).
    pub(crate) fn is_done(regs: &Block) -> bool {
        regs.ir.read().ifdone().bit()
    }

    /// True when the decode flagged an error (`IR.IFERROR`).
    pub(crate) fn is_error(regs: &Block) -> bool {
        regs.ir.read().iferror().bit()
    }

    /// Clear the done / error flags (`IR.ICDONE` / `IR.ICERROR`).
    pub(crate) fn clear_flags(regs: &Block) {
        regs.ir.write(|w| {
            w.icdone().set_bit();
            w.icerror().set_bit()
        });
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{Block, PeripheralClockIndex};
    use cortex_m::interrupt::InterruptNumber;

    /// A JPEG decoder the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers the JPEG decoder.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }
    }
}

#[allow(private_bounds)]
/// A JPEG decoder usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the JPEG singleton.
macro_rules! impl_jpeg_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Jpeg
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> impl InterruptNumber {
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

impl_jpeg_instance!(crate::Jpeg, JPEG, JPEG, 0);
