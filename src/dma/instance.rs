//! Register access for the direct memory access (DMA) controller.
//!
//! The SWM341 DMA has four channels (`ch0`..`ch3`), each with its own source,
//! destination, control and mux registers. The `EN` register arms a channel; the
//! `IE`/`IF` registers carry the per-channel transfer-complete interrupt.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every DMA instance on the SWM341.
pub(crate) type Block = pac::dma::RegisterBlock;

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

/// Register access shared by every DMA instance.
pub(crate) mod hw {
    use super::Block;

    pub(crate) fn enable(_regs: &Block) {}
    pub(crate) fn disable(_regs: &Block) {}

    /// Program one channel: source/destination addresses, length and (optional)
    /// handshake request number. `req == None` selects a software-triggered
    /// memory-to-memory transfer.
    pub(crate) fn configure_channel(
        regs: &Block,
        ch: u8,
        src: u32,
        dst: u32,
        len: u32,
        req: Option<u8>,
    ) {
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

    /// Arm (start) a channel.
    pub(crate) fn enable_channel(regs: &Block, ch: u8) {
        let cur = regs.en.read().bits();
        regs.en.write(|w| unsafe { w.bits(cur | (1u32 << ch)) });
    }

    /// Disarm a channel.
    pub(crate) fn disable_channel(regs: &Block, ch: u8) {
        let cur = regs.en.read().bits();
        regs.en.write(|w| unsafe { w.bits(cur & !(1u32 << ch)) });
    }

    /// `true` once channel `ch` raised its transfer-complete flag.
    pub(crate) fn channel_done(regs: &Block, ch: u8) -> bool {
        match ch {
            0 => regs.if_.read().ch0().bit_is_set(),
            1 => regs.if_.read().ch1().bit_is_set(),
            2 => regs.if_.read().ch2().bit_is_set(),
            _ => regs.if_.read().ch3().bit_is_set(),
        }
    }

    /// Clear channel `ch`'s transfer-complete flag (write 1 to clear).
    pub(crate) fn clear_channel_done(regs: &Block, ch: u8) {
        match ch {
            0 => regs.if_.write(|w| w.ch0().set_bit()),
            1 => regs.if_.write(|w| w.ch1().set_bit()),
            2 => regs.if_.write(|w| w.ch2().set_bit()),
            _ => regs.if_.write(|w| w.ch3().set_bit()),
        }
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// A DMA peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> pac::Interrupt;
        fn regs() -> *const Block;

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn enable() {
            hw::enable(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn configure_channel(ch: u8, src: u32, dst: u32, len: u32, req: Option<u8>) {
            hw::configure_channel(unsafe { &*Self::regs() }, ch, src, dst, len, req);
        }
        #[inline(always)]
        fn enable_channel(ch: u8) {
            hw::enable_channel(unsafe { &*Self::regs() }, ch);
        }
        #[inline(always)]
        fn disable_channel(ch: u8) {
            hw::disable_channel(unsafe { &*Self::regs() }, ch);
        }
        #[inline(always)]
        fn channel_done(ch: u8) -> bool {
            hw::channel_done(unsafe { &*Self::regs() }, ch)
        }
        #[inline(always)]
        fn clear_channel_done(ch: u8) {
            hw::clear_channel_done(unsafe { &*Self::regs() }, ch);
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
            fn irq() -> pac::Interrupt {
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

impl_dma_instance!(crate::Dma, DMA, DMA, Dma, 0);
