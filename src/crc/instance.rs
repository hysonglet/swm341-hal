//! Register access for the CRC block.
//!
//! The SWM341 carries a single hardware CRC engine fed through one register and
//! read through another:
//!
//! * `CR` selects the polynomial (`POLY`), the width of a single input word
//!   (`IBIT`), and the input/output transformations (`IREV`/`INOT` and
//!   `OREV`/`ONOT`) applied around the accumulation.
//! * `DATAIN` accepts one input word. The engine is purely combinatorial with
//!   respect to software: a write folds the word into the accumulator with no
//!   status flag to wait for, so feeding is a straight sequence of stores.
//! * `INIVAL` seeds the accumulator. Writing it is what starts a fresh
//!   computation, which is why [`crate::crc::Crc::reset`] reloads it.
//! * `RESULT` holds the running (or final) accumulator value.
//!
//! There is no interrupt line and no completion flag, so -- like `dac` -- this
//! peripheral deliberately stays outside the
//! [`crate::instance_base::InstanceBase`] abstraction: it has a clock gate and a
//! register block, but neither an NVIC vector nor an async index.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type of the single CRC instance on the SWM341.
pub(crate) type Block = pac::crc::RegisterBlock;

pub(crate) mod sealed {
    use super::{Block, PeripheralClockIndex};
    use crate::crc::types::Config;

    /// The CRC peripheral the driver can talk to.
    ///
    /// There is a single instance on the SWM341, so this is implemented for
    /// [`crate::Crc`] with the `CRC` register block and the `Crc` clock gate.
    pub trait Instance {
        /// Clock gate that powers this peripheral (`Crc`).
        fn clock() -> PeripheralClockIndex;
        /// Pointer to this instance's register block (`pac::CRC::PTR`).
        fn regs() -> *const Block;

        /// Enable the engine (`CR.EN`).
        ///
        /// The accumulator only advances while `EN` is set, so this is done once
        /// in `Crc::new` and left on.
        #[inline(always)]
        fn enable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().set_bit());
        }

        /// Disable the engine (`CR.EN`).
        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().clear_bit());
        }

        /// Program the whole control register from `config`.
        ///
        /// Only the polynomial / width / transformation fields are touched, so a
        /// previous `enable()` is preserved.
        #[inline(always)]
        fn set_config(config: Config) {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| unsafe {
                w.poly().bits(config.poly as u8);
                w.ibit().bits(config.input_width as u8);
                w.irev().bits(config.input_rev as u8);
                w.inot().bit(config.input_invert);
                w.orev().bits(config.output_rev as u8);
                w.onot().bit(config.output_invert)
            });
        }

        /// Seed the accumulator (`INIVAL`).
        ///
        /// The hardware loads the written value into the accumulator, so this
        /// doubles as the "start a new computation" operation.
        #[inline(always)]
        fn set_init_value(value: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.inival.write(|w| unsafe { w.bits(value) });
        }

        /// Fold one input word into the accumulator (`DATAIN`).
        #[inline(always)]
        fn write_data(data: u32) {
            let regs = unsafe { &*Self::regs() };
            regs.datain.write(|w| unsafe { w.bits(data) });
        }

        /// Read the current accumulator (`RESULT`).
        ///
        /// Valid at any point: the engine updates synchronously on each
        /// `DATAIN` write, so there is nothing to wait for.
        #[inline(always)]
        fn result() -> u32 {
            let regs = unsafe { &*Self::regs() };
            regs.result.read().bits()
        }
    }
}

#[allow(private_bounds)]
/// A CRC peripheral usable by the driver.
///
/// Kept as the public, user-facing bound (mirroring the other peripheral
/// drivers) even though the SWM341 has a single CRC instance.
#[allow(dead_code)]
pub trait Instance: sealed::Instance {}

macro_rules! impl_crc_instance {
    ($ty:ty, $clock:expr) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                $clock
            }

            #[inline]
            fn regs() -> *const Block {
                pac::CRC::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_crc_instance!(crate::Crc, PeripheralClockIndex::Crc);
