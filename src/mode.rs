//! Blocking and async driver modes.
//!
//! Peripheral drivers are written once and instantiated in one of two flavours:
//! `Blocking`, where every operation spins until the hardware is done, and
//! `Async`, where the same operation becomes an `impl Future`. The mode is a
//! type parameter of the driver, so both flavours collapse to a single
//! monomorphisation each and carry no runtime cost.

use crate::sealed::Sealed;

#[allow(private_bounds)]
pub trait Mode: Sealed {
    /// True when this driver flavour parks tasks instead of spinning.
    fn is_async() -> bool;
}

/// Driver flavour whose operations busy-wait.
pub struct Blocking;

/// Driver flavour whose operations are awaited.
pub struct Async;

impl Sealed for Blocking {}
impl Sealed for Async {}

impl Mode for Blocking {
    #[inline]
    fn is_async() -> bool {
        false
    }
}

impl Mode for Async {
    #[inline]
    fn is_async() -> bool {
        true
    }
}
