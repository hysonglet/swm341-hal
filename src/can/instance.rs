//! Register access for the CAN controller.
//!
//! The SWM341 CAN is a SJA1000-style (PeliCAN) controller. The `frame` cluster is
//! a single 13-word buffer (one `INFO` word + twelve `DATA` words) shared by the
//! transmit and receive sides; the 11/29-bit identifier is packed into the first
//! bytes of the `DATA` area (standard: 2 bytes, extended: 4 bytes) and the
//! `INFO` word carries FF / RTR / DLC, exactly like PeliCAN.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every CAN instance on the SWM341.
pub(crate) type Block = pac::can0::RegisterBlock;

/// Register access shared by every CAN instance.
pub(crate) mod hw {
    use super::Block;

    /// Take the controller out of reset (clock gate already powered it).
    pub(crate) fn enable(regs: &Block) {
        regs.cr.write(|w| w.rst().clear_bit());
    }
    /// Hold the controller in reset.
    pub(crate) fn disable(regs: &Block) {
        regs.cr.write(|w| w.rst().set_bit());
    }

    /// Program the bit timing (SJA1000-style): `brp` is a 10-bit prescaler split
    /// across `BT0.BRP` (low 6) and `BT2.BRP` (high 4); `sjw`/`tseg1`/`tseg2`
    /// follow the PeliCAN `BTR0`/`BTR1` layout.
    pub(crate) fn set_timing(regs: &Block, brp: u16, sjw: u8, tseg1: u8, tseg2: u8, sam: bool) {
        regs.bt0
            .write(|w| unsafe { w.bits(((sjw as u32 & 3) << 6) | (brp as u32 & 0x3F)) });
        regs.bt1.write(|w| unsafe {
            w.bits(((sam as u32) << 7) | ((tseg2 as u32 & 7) << 4) | (tseg1 as u32 & 0x0F))
        });
        regs.bt2
            .write(|w| unsafe { w.bits(((brp as u32 >> 6) & 0x0F) as u32) });
    }

    /// `true` puts the controller in silent loopback mode.
    pub(crate) fn set_loopback(regs: &Block, on: bool) {
        regs.cr.modify(|_, w| w.stm().bit(on));
    }

    /// Configure the acceptance filter to accept every frame.
    pub(crate) fn set_accept_all(regs: &Block) {
        for i in 0..16 {
            regs.amr[i].write(|w| unsafe { w.bits(0xFF) });
            regs.acr[i].write(|w| unsafe { w.bits(0) });
        }
        regs.afe.write(|w| unsafe { w.bits(1) });
        regs.afm.write(|w| unsafe { w.bits(0) });
    }

    pub(crate) fn is_rx_available(regs: &Block) -> bool {
        regs.sr.read().rxda().bit_is_set()
    }
    pub(crate) fn is_tx_ready(regs: &Block) -> bool {
        regs.sr.read().txbr().bit_is_set()
    }
    pub(crate) fn is_tx_ok(regs: &Block) -> bool {
        regs.sr.read().txok().bit_is_set()
    }
    pub(crate) fn is_bus_off(regs: &Block) -> bool {
        regs.sr.read().busoff().bit_is_set()
    }

    /// Write `info` (raw `INFO` word) and the twelve `DATA` words into the buffer.
    pub(crate) fn write_frame(regs: &Block, info: u32, data: [u32; 12]) {
        regs.frame.info.write(|w| unsafe { w.bits(info) });
        for i in 0..12 {
            regs.frame.data[i].write(|w| unsafe { w.bits(data[i]) });
        }
    }

    /// Read the buffer back into `(info, data)`.
    pub(crate) fn read_frame(regs: &Block) -> (u32, [u32; 12]) {
        let info = regs.frame.info.read().bits();
        let mut data = [0u32; 12];
        for i in 0..12 {
            data[i] = regs.frame.data[i].read().bits();
        }
        (info, data)
    }

    /// Command: transmit request (bit 0).
    pub(crate) fn cmd_transmit(regs: &Block) {
        regs.cmd.write(|w| unsafe { w.bits(1) });
    }
    /// Command: release the receive buffer (bit 2).
    pub(crate) fn cmd_release_rx(regs: &Block) {
        regs.cmd.write(|w| unsafe { w.bits(1 << 2) });
    }
}

pub(crate) mod sealed {
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};
    use crate::pac;

    /// A CAN peripheral the driver can talk to.
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
        fn set_timing(brp: u16, sjw: u8, tseg1: u8, tseg2: u8, sam: bool) {
            hw::set_timing(unsafe { &*Self::regs() }, brp, sjw, tseg1, tseg2, sam);
        }
        #[inline(always)]
        fn set_loopback(on: bool) {
            hw::set_loopback(unsafe { &*Self::regs() }, on);
        }
        #[inline(always)]
        fn set_accept_all() {
            hw::set_accept_all(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn is_rx_available() -> bool {
            hw::is_rx_available(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn is_tx_ready() -> bool {
            hw::is_tx_ready(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn is_tx_ok() -> bool {
            hw::is_tx_ok(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn is_bus_off() -> bool {
            hw::is_bus_off(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn write_frame(info: u32, data: [u32; 12]) {
            hw::write_frame(unsafe { &*Self::regs() }, info, data);
        }
        #[inline(always)]
        fn read_frame() -> (u32, [u32; 12]) {
            hw::read_frame(unsafe { &*Self::regs() })
        }
        #[inline(always)]
        fn cmd_transmit() {
            hw::cmd_transmit(unsafe { &*Self::regs() });
        }
        #[inline(always)]
        fn cmd_release_rx() {
            hw::cmd_release_rx(unsafe { &*Self::regs() });
        }
    }
}

#[allow(private_bounds)]
/// A CAN peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

macro_rules! impl_can_instance {
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
            // CAN0 and CAN1 share the single CAN interrupt vector (16), so both
            // report the same `pac::Interrupt`.
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

impl_can_instance!(crate::Can0, CAN0, CAN0, Can0, 0);
impl_can_instance!(crate::Can1, CAN1, CAN0, Can1, 1);
