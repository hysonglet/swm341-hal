//! Register access for the CAN controller.
//!
//! The SWM341 CAN is a SJA1000-style (PeliCAN) controller. The `frame` cluster is
//! a single 13-word buffer (one `INFO` word + twelve `DATA` words) shared by the
//! transmit and receive sides; the 11/29-bit identifier is packed into the first
//! bytes of the `DATA` area (standard: 2 bytes, extended: 4 bytes) and the
//! `INFO` word carries FF / RTR / DLC, exactly like PeliCAN.

use crate::clock::PeripheralClockIndex;

/// The register-block type shared by every CAN instance on the SWM341.
pub(crate) type Block = crate::pac::can0::RegisterBlock;

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};

    /// A CAN peripheral the driver can talk to.
    pub trait Instance {
        fn clock() -> PeripheralClockIndex;
        fn index() -> usize;
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        fn regs() -> *const Block;


        /// Take the controller out of reset (clock gate already powered it).
        #[inline(always)]
        fn enable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.rst().clear_bit());
        }
        /// Hold the controller in reset.
        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.rst().set_bit());
        }

        /// Program the bit timing (SJA1000-style): `brp` is a 10-bit prescaler split
        /// across `BT0.BRP` (low 6) and `BT2.BRP` (high 4); `sjw`/`tseg1`/`tseg2`
        /// follow the PeliCAN `BTR0`/`BTR1` layout.
        #[inline(always)]
        fn set_timing(brp: u16, sjw: u8, tseg1: u8, tseg2: u8, sam: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.bt0
                .write(|w| unsafe { w.bits(((sjw as u32 & 3) << 6) | (brp as u32 & 0x3F)) });
            regs.bt1.write(|w| unsafe {
                w.bits(((sam as u32) << 7) | ((tseg2 as u32 & 7) << 4) | (tseg1 as u32 & 0x0F))
            });
            regs.bt2
                .write(|w| unsafe { w.bits(((brp as u32 >> 6) & 0x0F) as u32) });
        }

        /// `true` puts the controller in silent loopback mode.
        #[inline(always)]
        fn set_loopback(on: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.stm().bit(on));
        }

        /// Configure the acceptance filter to accept every frame.
        #[inline(always)]
        fn set_accept_all() {
            let regs = unsafe { &*Self::regs() };
            for i in 0..16 {
                regs.amr[i].write(|w| unsafe { w.bits(0xFF) });
                regs.acr[i].write(|w| unsafe { w.bits(0) });
            }
            regs.afe.write(|w| unsafe { w.bits(1) });
            regs.afm.write(|w| unsafe { w.bits(0) });
        }

        #[inline(always)]
        fn is_rx_available() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().rxda().bit_is_set()
        }
        #[inline(always)]
        fn is_tx_ready() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().txbr().bit_is_set()
        }
        #[inline(always)]
        fn is_tx_ok() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().txok().bit_is_set()
        }
        #[inline(always)]
        fn is_bus_off() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.sr.read().busoff().bit_is_set()
        }

        /// Write `info` (raw `INFO` word) and the twelve `DATA` words into the buffer.
        #[inline(always)]
        fn write_frame(info: u32, data: [u32; 12]) {
            let regs = unsafe { &*Self::regs() };
            regs.frame.info.write(|w| unsafe { w.bits(info) });
            for i in 0..12 {
                regs.frame.data[i].write(|w| unsafe { w.bits(data[i]) });
            }
        }

        /// Read the buffer back into `(info, data)`.
        #[inline(always)]
        fn read_frame() -> (u32, [u32; 12]) {
            let regs = unsafe { &*Self::regs() };
            let info = regs.frame.info.read().bits();
            let mut data = [0u32; 12];
            for i in 0..12 {
                data[i] = regs.frame.data[i].read().bits();
            }
            (info, data)
        }

        /// Command: transmit request (bit 0).
        #[inline(always)]
        fn cmd_transmit() {
            let regs = unsafe { &*Self::regs() };
            regs.cmd.write(|w| unsafe { w.bits(1) });
        }
        /// Command: release the receive buffer (bit 2).
        #[inline(always)]
        fn cmd_release_rx() {
            let regs = unsafe { &*Self::regs() };
            regs.cmd.write(|w| unsafe { w.bits(1 << 2) });
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
            // report the same `ExtendedInterrupt`.
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

impl_can_instance!(crate::Can0, CAN0, CAN0, Can0, 0);
impl_can_instance!(crate::Can1, CAN1, CAN0, Can1, 1);
