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
//! `Async` (with the `embassy` feature) awaits. The pad wiring is type-checked
//! through [`pins`].

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod master;
mod slave;
mod types;

pub use master::I2c;
pub use slave::I2cSlave;
pub use types::*;
