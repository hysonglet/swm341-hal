//! Register access for the TFT LCD controller (LCD).
//!
//! The SWM341 LCDC drives a parallel RGB TFT panel. [`CR`]/`CRH`/`CRV` set the
//! dot-clock divisor and the horizontal/vertical timing; [`BGC`] is the
//! background color shown behind disabled layers; each of the two overlay layers
//! `L0`/`L1` has a window position, a framebuffer address, a line stride, a
//! global alpha and an optional color key. [`START`] launches one frame and
//! [`IF`]/[`IE`] carry the frame-done / error interrupts.
//!
//! The `IF` and `IE` registers are left fieldless by the SVD, so the driver names
//! their bits through the `IF_FRAMEDONE` / `IF_ERROR` constants below. Their bit
//! positions follow the SWM341 reference manual.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the LCD controller.
pub(crate) type Block = pac::lcd::RegisterBlock;

/// Frame-done interrupt flag (`IF`/`IE` bit 0).
pub(crate) const IF_FRAMEDONE: u32 = 1 << 0;
/// Transfer / underrun error flag (`IF`/`IE` bit 1).
pub(crate) const IF_ERROR: u32 = 1 << 1;

pub(crate) mod sealed {

    use super::{Block, IF_ERROR, IF_FRAMEDONE};

    /// An LCD controller the driver can talk to.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        /// Borrow layer `layer` (0 or 1) register cluster.
        #[inline(always)]
        fn layer(layer: usize) -> &'static crate::pac::lcd::L0 {
            let base = Self::regs() as *const u8;
            let off = if layer == 0 { 0x40 } else { 0x80 };
            unsafe { &*((base.add(off)) as *const crate::pac::lcd::L0) }
        }

        /// Write the dot-clock divisor and signal-polarity / format control.
        ///
        /// Mirrors the vendor `LCD_Init`: beyond the polarity/format bits it also
        /// turns on the AHB burst (`BURSTEN`/`BURSTLEN`, needed to pull a whole
        /// scanline out of SDRAM) and, when `auto_restart` is set, `AUTORESTA`
        /// so the controller keeps refreshing without an end-of-frame ISR.
        #[inline(always)]
        fn set_control(
            clkdiv: u8,
            format_yuv: bool,
            inv_clk: bool,
            inv_vsync: bool,
            inv_hsync: bool,
            serial: bool,
            mpu: bool,
            auto_restart: bool,
        ) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| unsafe {
                w.clkdiv().bits(clkdiv);
                w.clkinv().bit(inv_clk);
                w.vsyncinv().bit(inv_vsync);
                w.hsyncinv().bit(inv_hsync);
                w.format().bit(format_yuv);
                w.seren().bit(serial);
                w.mpuen().bit(mpu);
                w.bursten().set_bit();
                w.burstlen().set_bit();
                w.autoresta().bit(auto_restart);
                w
            });
            // `CR.CLKALW` (bit 7) keeps DOTCLK running through blanking. The PAC
            // omits the accessor and svd2rust `write` would otherwise force the
            // bit to 0, stopping the clock between frames -- which makes many RGB
            // panels flicker hard. The vendor `LCD_Init` always sets it.
            unsafe {
                regs.cr.modify(|r, w| w.bits(r.bits() | (1 << 7)));
            }
        }

        /// Write the horizontal and vertical timing.
        ///
        /// NOTE: the vendor `LCD_Init` writes `count - 1` into these fields, but
        /// on this silicon the observed working values are the raw counts, so we
        /// write them verbatim (the PAC field already maps to the active count).
        #[inline(always)]
        fn set_timing(h: &crate::lcdc::types::HvTiming, v: &crate::lcdc::types::HvTiming) {
            let regs = unsafe { &*Self::regs() };
            regs.crh.write(|w| unsafe {
                w.hsw().bits(h.sync as u8);
                w.hbp().bits(h.back_porch as u8);
                w.pix().bits(h.pixels);
                w.hfp().bits(h.front_porch as u8);
                w
            });
            regs.crv.write(|w| unsafe {
                w.vsw().bits(v.sync as u8);
                w.vbp().bits(v.back_porch as u8);
                w.pix().bits(v.pixels);
                w.vfp().bits(v.front_porch as u8);
                w
            });
        }

        /// Write the RGB565 background color.
        #[inline(always)]
        fn set_bg_color(color: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.bgc.write(|w| unsafe { w.bits(color) });
        }

        /// Configure one layer's window, framebuffer, stride, alpha and color key.
        #[inline(always)]
        fn set_layer(layer: usize, cfg: &crate::lcdc::types::LayerConfig) {
            let l = Self::layer(layer);
            l.whp.write(|w| unsafe {
                w.sta().bits(cfg.x_start);
                w.stp().bits(cfg.x_stop);
                w
            });
            l.wvp.write(|w| unsafe {
                w.sta().bits(cfg.y_start);
                w.stp().bits(cfg.y_stop);
                w
            });
            l.addr.write(|w| unsafe { w.bits(cfg.framebuffer) });
            // `LLEN` is the scanline length in half-words, so the vendor writes
            // `HnPixel - 1`; equivalently `stride_bytes / 2 - 1` for RGB565.
            l.llen.write(|w| unsafe { w.bits(cfg.stride_bytes / 2 - 1) });
            l.lcr.modify(|_, w| unsafe {
                w.alpha().bits(cfg.alpha);
                w.en().bit(cfg.enable);
                w.cken().bit(cfg.color_key.is_some());
                w
            });
            if let Some(key) = cfg.color_key {
                l.ck.write(|w| unsafe { w.bits(key) });
            }
        }

        /// Launch a single frame (`START.GO`).
        #[inline(always)]
        fn start_frame() {
            let regs = unsafe { &*Self::regs() };
            regs.start.write(|w| w.go().set_bit());
        }

        /// True once a frame has been completed since the last clear.
        #[inline(always)]
        fn is_frame_done() -> bool {
            let regs = unsafe { &*Self::regs() };
            (regs.if_.read().bits() & IF_FRAMEDONE) != 0
        }

        /// True on a transfer / underrun error.
        #[inline(always)]
        fn is_error() -> bool {
            let regs = unsafe { &*Self::regs() };
            (regs.if_.read().bits() & IF_ERROR) != 0
        }

        /// Clear interrupt flags by writing the given bitmask (write-1-clear).
        #[inline(always)]
        fn clear_interrupt(mask: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| unsafe { w.bits(mask) });
        }

        /// Enable or disable the interrupt flags in `mask`.
        #[inline(always)]
        fn set_interrupt_enable(mask: u32, enable: bool) {
            let regs = unsafe { &*Self::regs() };
            let cur = regs.ie.read().bits();
            let val = if enable { cur | mask } else { cur & !mask };
            regs.ie.write(|w| unsafe { w.bits(val) });
        }
    }
}

#[allow(private_bounds)]
/// An LCD controller usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the LCD singleton.
macro_rules! impl_lcd_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        crate::impl_instance_base!(
            $ty,
            crate::pac::lcd::RegisterBlock,
            crate::interrupt::$irq,
            PeripheralClockIndex::Lcd,
            $index
        );

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_lcd_instance!(crate::Lcd, LCD, LCD, 0);

/// Number of LCD instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 1;
