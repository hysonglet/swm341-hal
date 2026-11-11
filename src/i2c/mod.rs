//! Inter-integrated circuit (I2C) bus driver.
//!
//! The module provides two objects, one per bus role:
//!
//! * [`I2c`] — the bus **master** (controller). It drives SCL and SDA, generates
//!   START / STOP and talks to slaves. See [`master`] for the blocking and async
//!   APIs, including the strict repeated-start `transaction`.
//! * [`I2cSlave`] — a bus **peripheral** (slave) answering to its own address. See
//!   [`slave`] for the event-driven primitives and the buffered `transfer`.
//!
//! Both objects are generic over [`crate::mode::Mode`]: `Blocking` busy-waits,
//! `Async` (with the `embassy` feature) awaits. The SCL/SDA pad functions are
//! derived from the pin type and the I2C instance through
//! [`I2cSclPin`]/[`I2cSdaPin`], so the constructors only need the pins.

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod master;
mod slave;
mod types;

use instance::Instance;

pub use master::I2c;
pub use slave::I2cSlave;
pub use types::*;

/// A pin that can drive the clock line of I2C instance `T`.
///
/// Implemented for every pad exposing an `I2Cx_SCL` function; the associated
/// [`FUNC`](I2cSclPin::FUNC) is the 4-bit `PORTx.FUNC` code that routes the pad
/// to `T`'s SCL path. [`I2c::new`] reads it straight from the pin type, so the
/// caller never names the function code by hand and a pad that cannot be `T`'s
/// SCL is a compile error.
pub trait I2cSclPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting this pad as `T`'s SCL.
    const FUNC: u8;
}

/// A pin that can drive the data line of I2C instance `T`.
///
/// See [`I2cSclPin`] for the rationale; this is the SDA counterpart.
pub trait I2cSdaPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting this pad as `T`'s SDA.
    const FUNC: u8;
}
