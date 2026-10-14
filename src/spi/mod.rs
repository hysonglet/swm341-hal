//! Serial peripheral interface (SPI) bus driver.
//!
//! The module provides two objects, one per bus role:
//!
//! * [`Spi`] — the bus **master** (controller). It supplies SCK and drives the
//!   hardware CS line, shifting data over MOSI / MISO. See [`master`] for the
//!   blocking and async `SpiBus` APIs.
//! * [`SpiSlave`] — a bus **peripheral** (slave) answering to an external clock
//!   and chip-select. See [`slave`] for the event-driven primitives and the
//!   buffered `transfer`.
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

pub use master::Spi;
pub use slave::SpiSlave;
pub use types::*;
