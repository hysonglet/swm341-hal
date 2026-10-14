//! `embassy` integration.
//!
//! Enabling the `embassy` feature pulls in the `Async` driver flavour plus the
//! time base that `embassy-time` needs. Everything else in the HAL works
//! without it.

mod time_driver_systick;

pub(crate) use time_driver_systick::init_time_driver;
