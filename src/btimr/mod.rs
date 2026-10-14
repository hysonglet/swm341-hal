//! Basic timer (BTIMR0..BTIMR11).
//!
//! Each basic timer is a 32-bit down-counter with the same register layout as the
//! general purpose [`crate::timr`]. [`Btimr::new`] opens the shared clock gate
//! and [`Btimr::start`] arms it from a period value; [`Btimr::value`] reads the
//! live count and [`Btimr::stop`] freezes it. With the `embassy` feature a
//! [`Btimr::delay`](Btimr::delay) future waits on the overflow interrupt instead
//! of busy-spinning.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut timer = hal::btimr::Btimr::new(p.BTIMR0, hal::btimr::Config::default());
//! timer.start(1_000_000);          // count down from 1,000,000
//! while timer.value() > 0 {}       // busy wait for the underflow
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
mod future;
mod instance;
mod types;
pub use types::*;

/// A basic timer driven by its peripheral clock.
#[allow(dead_code)]
pub struct Btimr<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    config: Config,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Btimr<'d, T, M> {
    /// Arm and start the down-counter from `period` (in timer-clock ticks).
    pub fn start(&self, period: u32) {
        T::start(
            period,
            self.config.prescaler,
            self.config.clock_source,
            self.config.mode,
        );
    }

    /// Read the live count value.
    pub fn value(&self) -> u32 {
        T::value()
    }

    /// Freeze the counter. [`Btimr::start`] re-arms it on the next call.
    pub fn stop(&self) {
        T::stop();
    }

    /// Clear the pending overflow flag.
    pub fn clear_overflow(&self) {
        T::clear_overflow();
    }

    /// Enable or disable the overflow interrupt.
    pub fn enable_overflow_interrupt(&self, enable: bool) {
        T::set_overflow_irq(enable);
    }
}

impl<'d, T: Instance, M: Mode> Drop for Btimr<'d, T, M> {
    fn drop(&mut self) {
        T::stop();
        T::clock().close();
    }
}

impl<'d, T: Instance> Btimr<'d, T, Blocking> {
    /// Build a basic timer from its peripheral.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        Self {
            peri,
            config,
            _mode: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance> Btimr<'d, T, Async> {
    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode
    /// so the `delay` future is available.
    pub fn new_async(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        Self {
            peri,
            config,
            _mode: core::marker::PhantomData,
        }
    }

    /// Delay for `period` timer ticks, awaiting the overflow interrupt instead of
    /// busy-spinning.
    pub async fn delay(&mut self, period: u32) {
        future::delay::<T>(period).await;
    }
}
