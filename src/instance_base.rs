//! Shared base trait for every memory-mapped peripheral driver.
//!
//! All SWM341 peripheral drivers carry the same three pieces of scalar
//! information: the clock gate that powers them, their stable async-state
//! index, and their NVIC vector. [`InstanceBase`] bundles those so the
//! per-driver `Instance` trait (which adds device-specific methods) only has to
//! extend it and forward the per-instance values to [`impl_instance_base!`].
//!
//! The register-block pointer (`regs()`) and its [`InstanceBase::Block`] type do
//! *not* live on this trait: Rust does not let a trait default method reach the
//! struct fields of an associated-type projection, so `regs()` must keep
//! returning the **concrete** per-driver `Block` alias. [`InstanceBase::Block`]
//! is still provided as a template/associated type for downstream code that
//! names the block type.
//!
//! `gpio` and `dac` deliberately stay outside this abstraction: `gpio` is a
//! pad/port model (no single register block, `irq()` is `#[cfg(embassy)]`
//! only) and `dac` has neither an async index nor an interrupt.

use crate::clock::PeripheralClockIndex;

/// A memory-mapped SWM341 peripheral the HAL can drive.
///
/// This is the shared base of every driver's `Instance` trait. The three fields
/// it carries are identical across all peripherals; only the per-instance
/// values differ, which [`impl_instance_base!`] fills in.
pub trait InstanceBase {
    /// The PAC register-block type for this peripheral.
    type Block;

    /// Clock gate that powers this peripheral.
    fn clock() -> PeripheralClockIndex;

    /// Stable index used to look up this instance's async state.
    fn index() -> usize;

    /// NVIC interrupt vector of this peripheral.
    fn irq() -> crate::interrupt::ExtendedInterrupt;
}

/// Implement [`InstanceBase`] for a single peripheral instance.
///
/// `$pac` is the PAC module (e.g. `SPI0`); `$irq` and `$clock` are full
/// paths/expressions, e.g. `crate::interrupt::SPI0` and
/// `PeripheralClockIndex::Spi0`, so each driver decides how its vector and
/// clock gate are named.
#[macro_export]
macro_rules! impl_instance_base {
    ($ty:ty, $block:ty, $irq:expr, $clock:expr, $index:literal) => {
        impl $crate::instance_base::InstanceBase for $ty {
            type Block = $block;

            fn clock() -> $crate::clock::PeripheralClockIndex {
                $clock
            }

            fn index() -> usize {
                $index
            }

            fn irq() -> $crate::interrupt::ExtendedInterrupt {
                $irq
            }
        }
    };
}
