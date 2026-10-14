//! Successive-approximation ADC (ADC0, ADC1).
//!
//! Each ADC converts a selected input channel and exposes a 12-bit result. With
//! the `embassy` feature a [`Adc::read_async`](Adc::read_async) future waits on
//! the end-of-conversion interrupt; otherwise [`Adc::read`] busy-waits.
//!
//! The type-checked analog pad roles (`ADCx_CHy`, plus the `ADCx_TRIGz` hardware
//! triggers) live in [`crate::adc::pins`] and are applied with
//! [`crate::gpio::set_function`] after [`crate::gpio::acquire`] (function code
//! `15` routes the pad to the analog mux).
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let adc = hal::adc::Adc::new(p.ADC0, hal::adc::Config::default());
//! let millivolts = adc.read(0) as u32 * 3300 / 4095;   // channel 0, 12-bit
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod types;
pub use types::*;

/// An ADC driven by its peripheral clock.
#[allow(dead_code)]
pub struct Adc<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Adc<'d, T, M> {
    /// Synchronously convert `channel` and return the 12-bit result.
    pub fn read(&self, channel: u8) -> u16 {
        T::clear_eoc();
        T::select_channel(channel);
        T::start_conversion();
        while !T::eoc_sr() {
            // Spin until the conversion completes (EOC latches in SEQ0.SR).
        }
        let value = T::result();
        T::clear_eoc();
        value
    }

    /// Clear the pending end-of-conversion flag.
    pub fn clear_eoc(&self) {
        T::clear_eoc();
    }

    /// Enable or disable the end-of-conversion interrupt.
    pub fn enable_eoc_interrupt(&self, enable: bool) {
        T::set_eoc_irq(enable);
    }
}

impl<'d, T: Instance, M: Mode> Drop for Adc<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Adc<'d, T, Blocking> {
    /// Build an ADC from its peripheral and configuration.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        T::enable();
        T::set_sampling(config.sampling_time);
        // Arm sequence 0 for software (GO) triggering, matching the vendor flow.
        T::set_trigger(0x1);
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
impl<'d, T: Instance> Adc<'d, T, Async> {
    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode so
    /// the `read_async` future is available.
    pub fn new_async(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::enable();
        T::set_sampling(config.sampling_time);
        T::set_trigger(0x1);
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Convert `channel`, awaiting the end-of-conversion interrupt.
    pub async fn read_async(&self, channel: u8) -> u16 {
        future::read_async::<T>(channel).await
    }
}
