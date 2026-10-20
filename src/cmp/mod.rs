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
//! Each comparator's inputs are fixed pads -- there is no mux between the
//! comparator and its pad, only the shared analog path (function code `15`).
//! The type-checked pad roles (`ACMPx_INP` / `ACMPx_INN`) live in
//! [`crate::cmp::pins`], and the high-level [`Cmp::set_positive_pin`] /
//! [`Cmp::set_negative_pin`] route a pad for you, so you typically don't touch
//! [`crate::gpio::set_function`] directly.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut cmp = hal::cmp::Cmp::new(p.CMP0, hal::cmp::Config::default());
//! // PA2 is ACMP0's positive input, PB12 its negative input. Bind the handles:
//! // dropping them would release the pads again.
//! let _pos = cmp.set_positive_pin(p.PA2);
//! let _neg = cmp.set_negative_pin(p.PB12);
//! let high = cmp.output();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
pub mod types;
pub use types::*;

/// A GPIO pad that can serve as a comparator's positive (non-inverting) input.
///
/// Implemented for every pin exposing an `ACMPx_INP` analog function.
/// [`FUNC`](CmpPositivePin::FUNC) is the 4-bit `PORTx.FUNC` code (`15`) that
/// selects the analog path, so the driver can route a pad from its type alone.
pub trait CmpPositivePin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code routing the pad to the analog path (`15`).
    const FUNC: u8;
}

/// A GPIO pad that can serve as a comparator's negative (inverting) input.
///
/// Implemented for every pin exposing an `ACMPx_INN` analog function.
pub trait CmpNegativePin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code routing the pad to the analog path (`15`).
    const FUNC: u8;
}

macro_rules! cmp_positive_pins {
    ($($pin:ty => $cmp:ty),* $(,)?) => {
        $(
            impl CmpPositivePin<$cmp> for $pin {
                const FUNC: u8 = 15;
            }
        )*
    };
}

macro_rules! cmp_negative_pins {
    ($($pin:ty => $cmp:ty),* $(,)?) => {
        $(
            impl CmpNegativePin<$cmp> for $pin {
                const FUNC: u8 = 15;
            }
        )*
    };
}

// Straight from the vendor pin matrix: each comparator has exactly one
// positive and one negative pad.
cmp_positive_pins! {
    crate::gpio::PA2 => crate::Cmp0,
    crate::gpio::PA1 => crate::Cmp1,
    crate::gpio::PA0 => crate::Cmp2,
}
cmp_negative_pins! {
    crate::gpio::PB12 => crate::Cmp0,
    crate::gpio::PB5  => crate::Cmp1,
    crate::gpio::PB4  => crate::Cmp2,
}

/// An analog pad that has been routed to one of a comparator's inputs.
///
/// Returned by [`Cmp::set_positive_pin`] / [`Cmp::set_negative_pin`]; it owns
/// the pad so its `PORTx.FUNC` routing stays in effect for as long as the
/// handle lives.
pub struct CmpInput<'d, P: crate::gpio::Pin> {
    _flex: crate::gpio::Flex<'d, P>,
}

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

    /// Route `pin` to this comparator's positive input and return a handle.
    ///
    /// This replaces the manual `Flex::new(pin).set_func(..)` dance: the pad a
    /// comparator can use is known from its type, so the driver programs
    /// `PORTx.FUNC` (code `15`, the analog path) for you.
    pub fn set_positive_pin<P: CmpPositivePin<T>>(
        &mut self,
        pin: impl Peripheral<P = P> + 'd,
    ) -> CmpInput<'d, P> {
        let flex = crate::gpio::Flex::new(pin);
        // Float the pad: a leftover pull resistor biases the comparison.
        crate::gpio::set_pull::<P>(crate::gpio::Pull::None);
        crate::gpio::set_function::<P>(P::FUNC);
        CmpInput { _flex: flex }
    }

    /// Route `pin` to this comparator's negative input and return a handle.
    ///
    /// Also switches the comparator's negative source to the external pad (it
    /// would otherwise keep comparing against the internal reference).
    pub fn set_negative_pin<P: CmpNegativePin<T>>(
        &mut self,
        pin: impl Peripheral<P = P> + 'd,
    ) -> CmpInput<'d, P> {
        let flex = crate::gpio::Flex::new(pin);
        crate::gpio::set_pull::<P>(crate::gpio::Pull::None);
        crate::gpio::set_function::<P>(P::FUNC);
        T::set_negative_source(NegativeSource::ExternalPin);
        CmpInput { _flex: flex }
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
