//! Register access for the direct memory access (DMA) controller.
//!
//! The SWM341 DMA has four channels (`ch0`..`ch3`), each with its own source,
//! destination, control and mux registers. The `EN` register arms a channel; the
//! `IE`/`IF` registers carry the per-channel transfer-complete interrupt.

use crate::clock::PeripheralClockIndex;

/// The register-block type shared by every DMA instance on the SWM341.
pub(crate) type Block = crate::pac::dma::RegisterBlock;

/// Run `$body` with `$cr` bound to the register view of DMA channel `$ch` (0..3).
macro_rules! with_dma_ch {
    ($regs:ident, $ch:expr, |$cr:ident| $body:block) => {
        match $ch {
            0 => {
                let $cr = &$regs.ch0;
                $body
            }
            1 => {
                let $cr = &$regs.ch1;
                $body
            }
            2 => {
                let $cr = &$regs.ch2;
                $body
            }
            _ => {
                let $cr = &$regs.ch3;
                $body
            }
        }
    };
}

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};

    /// A DMA peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        fn regs() -> *const Block;


        #[inline(always)]
        fn enable() {}

        #[inline(always)]
        fn disable() {}

        #[inline(always)]
        fn configure_channel(ch: u8, src: u32, dst: u32, len: u32, req: Option<u8>) {
            let regs = unsafe { &*Self::regs() };
            with_dma_ch!(regs, ch, |c| {
                c.cr
                    .write(|w| unsafe { w.len().bits(len).rxen().set_bit().txen().set_bit() });
                c.src.write(|w| unsafe { w.bits(src) });
                c.dst.write(|w| unsafe { w.bits(dst) });
                match req {
                    Some(num) => c
                        .mux
                        .write(|w| unsafe { w.srchsen().set_bit().srchssig().bits(num) }),
                    None => c
                        .mux
                        .write(|w| unsafe { w.srchsen().clear_bit().srchssig().bits(0) }),
                }
            });
        }

        #[inline(always)]
        fn enable_channel(ch: u8) {
            let regs = unsafe { &*Self::regs() };
            let cur = regs.en.read().bits();
            regs.en.write(|w| unsafe { w.bits(cur | (1u32 << ch)) });
        }

        #[inline(always)]
        fn disable_channel(ch: u8) {
            let regs = unsafe { &*Self::regs() };
            let cur = regs.en.read().bits();
            regs.en.write(|w| unsafe { w.bits(cur & !(1u32 << ch)) });
        }

        #[inline(always)]
        fn channel_done(ch: u8) -> bool {
            let regs = unsafe { &*Self::regs() };
            match ch {
                0 => regs.if_.read().ch0().bit_is_set(),
                1 => regs.if_.read().ch1().bit_is_set(),
                2 => regs.if_.read().ch2().bit_is_set(),
                _ => regs.if_.read().ch3().bit_is_set(),
            }
        }

        #[inline(always)]
        fn clear_channel_done(ch: u8) {
            let regs = unsafe { &*Self::regs() };
            match ch {
                0 => regs.if_.write(|w| w.ch0().set_bit()),
                1 => regs.if_.write(|w| w.ch1().set_bit()),
                2 => regs.if_.write(|w| w.ch2().set_bit()),
                _ => regs.if_.write(|w| w.ch3().set_bit()),
            }
        }
    }
}

#[allow(private_bounds)]
/// A DMA peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_dma_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::$clock
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
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_dma_instance!(crate::Dma, DMA, DMA, Dma, 0);
