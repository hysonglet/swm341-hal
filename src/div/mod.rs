//! Hardware integer divider and square-root unit.
//!
//! The SWM341 ships a dedicated divide / square-root engine that offloads the
//! slow multi-cycle operations from the core. [`Div::divide`] returns both the
//! quotient and the remainder of an unsigned division, and [`Div::sqrt`] returns
//! the integer square root. Both busy-wait on the peripheral's `SR` register:
//! for short operands that is a handful of cycles, so the blocking API here is
//! the natural fit (no async flavour is provided).
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let div = hal::div::Div::new(p.div());
//! let result = div.divide(1_000_000, 7);
//! assert_eq!(result.quotient, 142_857);
//! assert_eq!(result.remainder, 1);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the hardware divider.
#[allow(dead_code)]
pub struct Div<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Div<'d, T, M> {
    fn drop(&mut self) {
        // Gate the clock so the unit does not draw power once unused.
        T::clock().close();
    }
}

impl<'d, T: Instance> Div<'d, T, Blocking> {
    /// Take ownership of the divider and open its clock gate.
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        into_ref!(peri);
        T::clock().open();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Unsigned division of `dividend` by `divisor, returning the quotient and
    /// remainder as a [`DivResult`].
    pub fn divide(&self, dividend: u32, divisor: u32) -> DivResult {
        let (quotient, remainder) = T::divide(dividend, divisor);
        DivResult::new(quotient, remainder)
    }

    /// Integer square root of `radicand` (floor of the exact root).
    pub fn sqrt(&self, radicand: u32) -> u32 {
        T::sqrt(radicand)
    }

    /// True while the unit is still busy with a previous operation.
    pub fn is_busy(&self) -> bool {
        T::is_busy()
    }
}
