//! General purpose timer (TIMR0..TIMR4).
//!
//! Each TIMR is a 24-bit down-counter clocked from `PCLK` (which equals `SYSCLK`,
//! i.e. 120 MHz with [`clock::ClockConfig::hrc_120mhz`]). An 8-bit prescaler
//! (`Config::prescaler`) divides that clock by `prescaler + 1`. [`Timr::new`] opens
//! the shared clock gate and [`Timr::start`] arms it from a period value;
//! [`Timr::value`] reads the live count and [`Timr::stop`] freezes it. Starting also
//! sets this instance's run-enable bit in `TIMRG.EN` — without it the counter never
//! runs. With the `embassy` feature a [`Timr::delay`](Timr::delay) future waits on the
//! overflow interrupt instead of busy-spinning.
//!
//! Because the counter is only 24 bits (the `LOAD` field is `0xFFFFFF`), a single
//! 120 MHz period can hold at most ~139 ms. To time one second, prescale by 10
//! (`prescaler(9)` -> 12 MHz) and load `12_000_000` ticks. Prefer polling
//! [`Timr::overflow_flag`] (or the `embassy` future) to busy-waiting on `value() > 0`,
//! whose back-to-back reads carry heavy wait-states.
//!
//! The type-checked pad roles -- external clock input (`TIMRx_IN`) and compare
//! output (`TIMRx_OUT`) -- live in [`crate::timr::pins`] and are applied with
//! [`crate::gpio::set_function`] after [`crate::gpio::acquire`] when a pad is
//! actually used.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! // One second at the 120 MHz PCLK: 120_000_000 ticks.
//! let mut timer = hal::timr::Timr::new(p.TIMR0, hal::timr::Config::default());
//! timer.clear_overflow();
//! timer.start(120_000_000);
//! while !timer.overflow_flag() {}  // wait for the underflow
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod types;
pub use types::*;

/// A TIMR driven by its peripheral clock.
#[allow(dead_code)]
pub struct Timr<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    config: Config,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Timr<'d, T, M> {
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

    /// Freeze the counter. [`Timr::start`] re-arms it on the next call.
    pub fn stop(&self) {
        T::stop();
    }

    /// Clear the pending overflow flag.
    pub fn clear_overflow(&self) {
        T::clear_overflow();
    }

    /// True once the counter has underflowed since the last clear.
    ///
    /// Unlike [`Timr::value`], polling this flag does not touch the live count
    /// register, so it is the reliable way to wait for the timer to elapse.
    pub fn overflow_flag(&self) -> bool {
        T::overflow_flag()
    }

    /// Enable or disable the overflow interrupt.
    pub fn enable_overflow_interrupt(&self, enable: bool) {
        T::set_overflow_irq(enable);
    }
}

impl<'d, T: Instance, M: Mode> Drop for Timr<'d, T, M> {
    fn drop(&mut self) {
        T::stop();
        T::clock().close();
    }
}

impl<'d, T: Instance> Timr<'d, T, Blocking> {
    /// Build a TIMR from its peripheral.
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
impl<'d, T: Instance> Timr<'d, T, Async> {
    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode so
    /// the `delay` future is available.
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
