//! Register access for the JPEG decoder (JPEG).
//!
//! The decoder pulls a compressed bitstream from `CSBASE` (length `CODLEN`),
//! reads the quantisation tables from `QTBASE` and the Huffman tables from
//! `HTBASE`, and writes the decoded pixels to `YBASE`/`UBASE`/`VBASE` (planar
//! YUV) or to `RGBASE` when `CFG.YUV2RGB` is set. `IMGSIZ`/`IMGSTR` bound the
//! output, `CR.START` launches a decode and `SR.BUSY` clears when `IR.IFDONE`
//! raises. The address / length registers are raw, so they are written through
//! `bits()`; `CFG`, `CR`, `SR` and `IR` use the typed accessors.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the JPEG decoder.
pub(crate) type Block = pac::jpeg::RegisterBlock;

pub(crate) mod sealed {

    use super::Block;

    /// A JPEG decoder the driver can talk to.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Set the compressed bitstream base address and length.
        fn set_stream(base: u32, len: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.csbase.write(|w| unsafe { w.bits(base) });
            regs.codlen.write(|w| unsafe { w.bits(len) });
        }

        /// Set the quantisation-table and Huffman-table base addresses.
        fn set_tables(quant: u32, huffman: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.qtbase.write(|w| unsafe { w.bits(quant) });
            regs.htbase.write(|w| unsafe { w.bits(huffman) });
        }

        /// Set the Y/U/V planar output buffers.
        fn set_yuv_buffers(y: u32, u: u32, v: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.ybase().write(|w| unsafe { w.bits(y) });
            regs.ubase.write(|w| unsafe { w.bits(u) });
            regs.vbase.write(|w| unsafe { w.bits(v) });
        }

        /// Set the packed RGB output buffer (used when `CFG.YUV2RGB` is set).
        fn set_rgb_buffer(rgb: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.rgbase().write(|w| unsafe { w.bits(rgb) });
        }

        /// Set the output image size in pixels (`width` x `height`).
        fn set_image_size(width: u16, height: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.imgsiz.write(|w| unsafe {
                w.hpix().bits(width);
                w.vpix().bits(height)
            });
        }

        /// Set the output stride (`IMGSTR`, bytes/units per row).
        fn set_stride(stride: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.imgstr.write(|w| unsafe { w.bits(stride) });
        }

        /// Write the decoder configuration word (`CFG`).
        fn set_config(config: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.cfg.write(|w| unsafe { w.bits(config) });
        }

        /// Soft-reset the decoder (`CR.RESET`).
        fn reset() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.reset().set_bit());
        }

        /// Start a decode (`CR.START`).
        fn start() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.start().set_bit());
        }

        /// True while the decoder is busy (`SR.BUSY`).
        fn is_busy() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().busy().bit()
        }

        /// True when the decode finished (`IR.IFDONE`).
        fn is_done() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.ir.read().ifdone().bit()
        }

        /// True when the decode flagged an error (`IR.IFERROR`).
        fn is_error() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.ir.read().iferror().bit()
        }

        /// Clear the done / error flags (`IR.ICDONE` / `IR.ICERROR`).
        fn clear_flags() {
            let regs = unsafe { &*Self::regs() };
            regs.ir.write(|w| {
                w.icdone().set_bit();
                w.icerror().set_bit()
            });
        }
    }
}

#[allow(private_bounds)]
/// A JPEG decoder usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the JPEG singleton.
macro_rules! impl_jpeg_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        crate::impl_instance_base!($ty, crate::pac::jpeg::RegisterBlock, crate::interrupt::$irq, PeripheralClockIndex::Jpeg, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_jpeg_instance!(crate::Jpeg, JPEG, JPEG, 0);
