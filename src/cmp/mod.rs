//! Analog comparators (CMP0, CMP1, CMP2).
//!
//! The SWM341 carries three analog comparators. Each compares an external
//! positive pad (`CMPx+`) against either an external negative pad (`CMPx−`) or
//! an internally generated reference voltage, and presents the result on
//! `CMPxOUT` (readable via [`Cmp::output`]). With the `embassy` feature the
//! [`Cmp::wait_for_output_change`] future awaits the output to toggle.
//!
//! All three units share one `ACMP` interrupt; the driver demultiplexes it
//! inside the ISR so each unit gets its own async waker.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut cmp = hal::cmp::Cmp::new(p.CMP0, hal::cmp::Config::default());
//! let high = cmp.output();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod types;
pub use types::*;

/// A comparator unit driven by its peripheral clock (the analog block).
#[allow(dead_code)]
pub struct Cmp<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Cmp<'d, T, M> {
    /// Enable or disable this comparator unit (`ACMPCR.CMPxON`).
    pub fn enable(&self, enable: bool) {
        if enable {
            T::enable_unit();
        } else {
            T::disable_unit();
        }
    }

    /// Toggle the input hysteresis (`ACMPCR.CMPxHYS`).
    pub fn set_hysteresis(&self, on: bool) {
        T::set_hysteresis(on);
    }

    /// Choose what drives this unit's negative input.
    pub fn set_negative_source(&self, src: NegativeSource) {
        T::set_negative_source(src);
    }

    /// Read the current comparator output (`true` => positive > negative).
    pub fn output(&self) -> bool {
        T::output()
    }

    /// Enable or disable this unit's interrupt (the shared `ACMP` vector).
    pub fn enable_interrupt(&self, enable: bool) {
        T::set_irq(enable);
    }

    /// True if this unit's latched interrupt flag is set.
    pub fn interrupt_pending(&self) -> bool {
        T::irq_flag()
    }

    /// Clear this unit's latched interrupt flag (write-1-to-clear).
    pub fn clear_interrupt(&self) {
        T::clear_irq_flag();
    }

    /// Program the shared internal reference divider (6-bit).
    ///
    /// Only meaningful when a unit's [`NegativeSource`] is
    /// [`InternalVref`](NegativeSource::InternalVref). The raw `VREF` field is
    /// exposed directly because its exact voltage mapping depends on the board's
    /// analog reference.
    pub fn set_reference_voltage(&self, vref: u8) {
        T::set_reference_voltage(vref & 0x3F);
    }
}

impl<'d, T: Instance, M: Mode> Drop for Cmp<'d, T, M> {
    fn drop(&mut self) {
        // The comparator has no dedicated clock gate (treated always-on), so this
        // is a no-op. The unit itself is left powered as configured.
        T::clock().close();
    }
}

impl<'d, T: Instance> Cmp<'d, T, Blocking> {
    /// Build a comparator from its peripheral and configuration.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        if config.enable {
            T::enable_unit();
        }
        T::set_hysteresis(config.hysteresis);
        T::set_negative_source(config.negative_source);
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
impl<'d, T: Instance> Cmp<'d, T, Async> {
    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode
    /// so the `wait_for_output_change` future is available.
    pub fn new_async(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        if config.enable {
            T::enable_unit();
        }
        T::set_hysteresis(config.hysteresis);
        T::set_negative_source(config.negative_source);
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Await a change of this comparator's output, returning the new state
    /// (`true` => positive input now above the negative input).
    ///
    /// The ISR demultiplexes the shared `ACMP` interrupt and wakes only this
    /// unit's waker, so all three comparators can be waited on concurrently.
    ///
    /// Deliberately takes `&mut self`: `CMPxIE`, the latched flag and the async
    /// waker are all per-unit state, so two concurrent waits on one unit would
    /// re-arm each other. `&mut self` makes that unrepresentable.
    pub async fn wait_for_output_change(&mut self) -> bool {
        future::wait_for_output_change::<T>().await
    }
}
