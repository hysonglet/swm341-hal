//! Watchdog timer (WDT).
//!
//! The watchdog must be reloaded ("fed") periodically or it resets the chip. The
//! driver arms the unit in [`Wdt::new`] with a reload value, a warning threshold
//! and the chosen responses, then [`Wdt::feed`] reloads it by writing the magic
//! `0x55`/`0xAA` sequence. A warning interrupt fires when the counter reaches the
//! `warning` threshold, giving software a last chance to feed before the reset.
//!
//! Dropping the handle gates the peripheral clock, which stops the watchdog -- so
//! keep the [`Wdt`] alive for the whole program.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut wdt = hal::wdt::Wdt::new(
//!     p.wdt(),
//!     hal::wdt::Config::default().reload(0xFFFF).warning(0x8000),
//! );
//! // ... periodically:
//! wdt.feed();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the watchdog timer.
#[allow(dead_code)]
pub struct Wdt<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Wdt<'d, T, M> {
    fn drop(&mut self) {
        // Stopping the watchdog when the handle is dropped is intentional: the
        // clock gate is the only reset-independent way to silence it.
        T::clock().close();
    }
}

impl<'d, T: Instance> Wdt<'d, T, Blocking> {
    /// Arm the watchdog with `config` and open its clock gate.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::set_reload(config.reload);
        T::set_warning(config.warning);
        T::configure(
            config.prescaler,
            config.reset_on_timeout,
            config.interrupt_on_warning,
            config.windowed,
            true,
        );
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Reload the counter, preventing a timeout as long as this is called before
    /// the counter reaches zero.
    pub fn feed(&self) {
        T::feed();
    }

    /// Change the counter reload value (also reloads the counter).
    pub fn set_reload(&self, reload: u32) {
        T::set_reload(reload);
    }

    /// Change the warning interrupt threshold.
    pub fn set_warning(&self, warning: u32) {
        T::set_warning(warning);
    }

    /// Stop the watchdog (`CR.EN = 0`).
    pub fn disable(&self) {
        T::disable();
    }

    /// True when the warning interrupt is pending.
    pub fn is_interrupt_pending(&self) -> bool {
        T::interrupt_pending()
    }

    /// Clear the pending warning interrupt.
    pub fn clear_interrupt(&self) {
        T::clear_interrupt();
    }
}
