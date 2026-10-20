//! Register access for the SDRAM controller.
//!
//! The SWM341 integrates a single SDRAM host. Its five registers carry the
//! whole bring-up: `TIM` holds the chip's timing in SDRAM clock cycles, `CFG`
//! the capacity, clock divider, CAS latency and the "high frequency" flag,
//! `T64` the refresh period, `CR` starts the hardware initialisation sequence
//! (`PWRON`, which clears itself once the chip is up) and later parks the chip
//! in self-refresh (`ENTERSRF`), and `SR` reports controller status.
//!
//! The controller has no NVIC vector -- initialisation is polled through
//! `CR.PWRON`, not signalled by an interrupt -- so, exactly like the DAC, this
//! trait stops at the clock gate and the register block instead of going
//! through [`crate::instance_base::InstanceBase`].

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type of the single SDRAM controller.
pub(crate) type Block = pac::sdramc::RegisterBlock;

pub(crate) mod sealed {
    use super::{Block, PeripheralClockIndex};
    use crate::sdramc::types::{CasLatency, ClkDiv, Size};

    /// The SDRAM controller the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers this peripheral (`Sdram`).
        fn clock() -> PeripheralClockIndex;

        /// Pointer to this instance's register block (`pac::SDRAMC::PTR`).
        fn regs() -> *const Block;

        /// Program the timing register (`TIM`).
        ///
        /// `trc` lands in bits 2:5, the field the PAC spells `trfc`: the vendor
        /// header calls it `TRC` (activate to activate on the same bank), and
        /// the SVD named it after `tRFC`. `t100us` is the number of SDRAM clocks
        /// that make up 100 us and must end up greater than that.
        #[inline(always)]
        fn set_timing(trcd: u8, trc: u8, trp: u8, t100us: u16) {
            let regs = unsafe { &*Self::regs() };
            regs.tim.write(|w| unsafe {
                w.trcd().bits(trcd & 0x03);
                w.trfc().bits(trc & 0x0f);
                w.trp().bits(trp & 0x03);
                w.t100us().bits(t100us & 0x7fff)
            });
        }

        /// Program the configuration register (`CFG`).
        ///
        /// `high_freq` must be set whenever the SDRAM clock exceeds 66 MHz.
        #[inline(always)]
        fn set_config(size: Size, clk_div: ClkDiv, cas_latency: CasLatency, high_freq: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cfg.write(|w| unsafe {
                w.size().bits(size as u8 & 0x03);
                w.clkdiv().bit(clk_div as u8 != 0);
                w.casdelay().bit(cas_latency as u8 != 0);
                w.highfreq().bit(high_freq)
            });
        }

        /// Program the refresh period (`T64`).
        ///
        /// The controller issues one refresh command every `t64` SDRAM clocks;
        /// `T64` is derived so that every row is refreshed once within the
        /// chip's refresh window.
        #[inline(always)]
        fn set_refresh_period(t64: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.t64.write(|w| unsafe { w.bits(t64) });
        }

        /// Kick off the hardware initialisation sequence (`CR.PWRON`).
        ///
        /// Writing `CR` also clears `ENTERSRF`, so the chip leaves self-refresh
        /// as part of the sequence.
        #[inline(always)]
        fn start_init() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.pwron().set_bit());
        }

        /// True once the initialisation sequence has finished (`CR.PWRON` is
        /// cleared by hardware).
        #[inline(always)]
        fn is_init_done() -> bool {
            let regs = unsafe { &*Self::regs() };
            !regs.cr.read().pwron().bit()
        }

        /// Park the chip in self-refresh (`CR.ENTERSRF`); it then keeps its
        /// contents but cannot be read or written.
        #[inline(always)]
        fn enter_self_refresh() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.entersrf().set_bit());
        }

        /// Leave self-refresh (`CR.ENTERSRF`) so the chip is accessible again.
        #[inline(always)]
        fn exit_self_refresh() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.entersrf().clear_bit());
        }

        /// Raw controller status (`SR`).
        #[inline(always)]
        fn status() -> u32 {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().bits()
        }
    }
}

#[allow(private_bounds)]
/// An SDRAM controller usable by the driver.
///
/// Kept as the public, user-facing bound (mirroring the other peripheral
/// drivers) even though the SWM341 has a single SDRAM controller instance.
#[allow(dead_code)]
pub trait Instance: sealed::Instance {}

macro_rules! impl_sdramc_instance {
    ($ty:ty, $pac:ident, $clock:expr) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                $clock
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_sdramc_instance!(crate::Sdramc, SDRAMC, PeripheralClockIndex::Sdram);
