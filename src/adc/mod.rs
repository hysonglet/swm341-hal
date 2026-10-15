//! Successive-approximation ADC (ADC0, ADC1).
//!
//! Each ADC converts a selected input channel and exposes a 12-bit result. With
//! the `embassy` feature a [`Adc::read_async`](Adc::read_async) future waits on
//! the end-of-conversion interrupt; otherwise [`Adc::read`] busy-waits.
//!
//! The type-checked analog pad roles (`ADCx_CHy`, plus the `ADCx_TRIGz` hardware
//! triggers) live in [`crate::adc::pins`]. The high-level [`Adc::add_pin`] routes
//! a pin to the analog mux (function code `15`) and remembers its channel, so you
//! typically don't touch [`crate::gpio::set_function`] directly.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut adc = hal::adc::Adc::new(p.ADC0, hal::adc::Config::default());
//! let pc6 = adc.add_pin(p.PC6);              // PC6 -> ADC0 channel 0
//! let millivolts = adc.read_input(&pc6) as u32 * 3300 / 4095;
//! ```

use cortex_m::asm::nop;
use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

#[cfg(feature = "embassy")]
mod future;
mod instance;
pub mod pins;
mod types;
pub use types::*;

/// A GPIO pad that can be sampled by a specific ADC instance.
///
/// Implemented for every pin exposing an `ADCx_CHy` analog function. The
/// associated [`CHANNEL`](AdcPin::CHANNEL) is the converter channel the pad
/// becomes once routed through the analog mux, and [`FUNC`](AdcPin::FUNC) is the
/// 4-bit `PORTx.FUNC` code (`15`) that selects the analog path. This lets the
/// driver configure a pin's channel from its type alone -- see [`Adc::add_pin`].
pub trait AdcPin<T: Instance>: crate::gpio::Pin {
    /// Converter channel this pad feeds (`0`..`11`).
    const CHANNEL: u8;
    /// `PORTx.FUNC` code routing the pad to the analog mux (`15`).
    const FUNC: u8;
}

macro_rules! adc_pins {
    ($($pin:ty => $adc:ty : $chan:expr),* $(,)?) => {
        $(
            impl AdcPin<$adc> for $pin {
                const CHANNEL: u8 = $chan;
                const FUNC: u8 = 15;
            }
        )*
    };
}

adc_pins! {
    crate::gpio::PC6  => crate::Adc0 : 0,
    crate::gpio::PC5  => crate::Adc0 : 1,
    crate::gpio::PC4  => crate::Adc0 : 2,
    crate::gpio::PC3  => crate::Adc0 : 3,
    crate::gpio::PC2  => crate::Adc0 : 4,
    crate::gpio::PC1  => crate::Adc0 : 5,
    crate::gpio::PC0  => crate::Adc0 : 6,
    crate::gpio::PA15 => crate::Adc0 : 7,
    crate::gpio::PA14 => crate::Adc0 : 8,
    crate::gpio::PA13 => crate::Adc0 : 9,
    crate::gpio::PA12 => crate::Adc0 : 10,
    crate::gpio::PA10 => crate::Adc0 : 11,
    crate::gpio::PC9  => crate::Adc1 : 6,
    crate::gpio::PC10 => crate::Adc1 : 5,
    crate::gpio::PC11 => crate::Adc1 : 4,
    crate::gpio::PC12 => crate::Adc1 : 3,
    crate::gpio::PC13 => crate::Adc1 : 2,
    crate::gpio::PD0  => crate::Adc1 : 1,
    crate::gpio::PD1  => crate::Adc1 : 0,
}

use core::marker::PhantomData;

/// An ADC input pad that has been routed to the analog mux.
///
/// Returned by [`Adc::add_pin`]; it owns the pad so its `PORTx.FUNC` routing
/// stays in effect for as long as the handle lives, and it records the channel
/// the pad maps to (via [`channel`](AdcInput::channel)) for later reads.
pub struct AdcInput<'d, T: Instance, P: AdcPin<T>> {
    _flex: crate::gpio::Flex<'d, P>,
    /// ADC channel this pad feeds.
    pub channel: u8,
    _adc: PhantomData<T>,
}

/// An ADC driven by its peripheral clock.
#[allow(dead_code)]
pub struct Adc<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Adc<'d, T, M> {
    /// Synchronously convert `channel`, reporting why it failed if it did.
    ///
    /// The sequence FIFO is flushed before arming, so the entry that is read
    /// back belongs to this conversion -- and the channel the hardware tags it
    /// with (`DR.CHNUM`) is checked against `channel`. Prefer this over
    /// [`read`](Self::read) wherever a wrong-channel sample must never be acted
    /// on silently.
    pub fn try_read(&self, channel: u8) -> Result<u16, Error> {
        if channel >= CHANNEL_COUNT {
            // `SEQCHN0.SEQ0` is a 12-bit mask: an out-of-range channel would
            // select no channel at all and the conversion never completes.
            return Err(Error::InvalidChannel);
        }

        T::clear_eoc();
        // Drop anything a previous (possibly cancelled) conversion left behind.
        T::flush_fifo();
        T::select_channel(channel);
        T::start_conversion();

        // `SR.EMPTY` tracks the FIFO and is not sticky, so unlike `SR.EOC` it
        // needs no "wait for the start to clear the old flag" delay.
        while !T::data_available() {
            nop();
        }

        let (got, value) = T::result();
        if got != channel {
            return Err(Error::ChannelMismatch {
                expected: channel,
                got,
            });
        }
        T::clear_eoc();
        Ok(value)
    }

    /// Synchronously convert `channel` and return the 12-bit result.
    ///
    /// Panics if the sample turned out to come from a different channel; see
    /// [`try_read`](Self::try_read) for the fallible variant.
    pub fn read(&self, channel: u8) -> u16 {
        match self.try_read(channel) {
            Ok(value) => value,
            Err(Error::InvalidChannel) => panic!("adc: channel out of range"),
            Err(Error::ChannelMismatch { expected, got }) => {
                panic!("adc: result came from channel {got}, expected {expected}")
            }
        }
    }

    /// Route `pin` to the analog mux and return an [`AdcInput`] describing it.
    ///
    /// This replaces the manual `Flex::new(pin).set_func(Adc0ChN)` dance: the
    /// channel a pad maps to is known from its type, so the driver both programs
    /// `PORTx.FUNC` (code `15`) and records the channel for later reads.
    pub fn add_pin<P: AdcPin<T>>(&mut self, pin: impl Peripheral<P = P> + 'd) -> AdcInput<'d, T, P> {
        let flex = crate::gpio::Flex::new(pin);
        // Float the pad: a leftover pull resistor biases the sample. The vendor
        // samples do the same (`PORT_Init(..., 0)` -- last arg = pull off).
        crate::gpio::set_pull::<P>(crate::gpio::Pull::None);
        crate::gpio::set_function::<P>(P::FUNC);
        AdcInput {
            _flex: flex,
            channel: P::CHANNEL,
            _adc: PhantomData,
        }
    }

    /// Synchronously convert the channel behind `input`, reporting failures.
    pub fn try_read_input<P: AdcPin<T>>(
        &self,
        input: &AdcInput<'_, T, P>,
    ) -> Result<u16, Error> {
        self.try_read(input.channel)
    }

    /// Synchronously convert the channel behind `input` and return its value.
    pub fn read_input<P: AdcPin<T>>(&self, input: &AdcInput<'_, T, P>) -> u16 {
        self.read(input.channel)
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

    /// Convert the channel behind `input`, awaiting the end-of-conversion
    /// interrupt.
    ///
    /// Taking an [`AdcInput`] (rather than a bare channel number) ties the read
    /// to a pin that was actually routed through the analog mux, so a wrong
    /// channel cannot be handed in by accident.
    ///
    /// Deliberately takes `&mut self`: the channel mask, the sequence FIFO and
    /// the interrupt waker are all per-instance state, so two concurrent readers
    /// would reprogram the channel under each other and then read whatever the
    /// other one converted. `&mut self` makes that unrepresentable.
    pub async fn try_read_async<P: AdcPin<T>>(
        &mut self,
        input: &AdcInput<'_, T, P>,
    ) -> Result<u16, Error> {
        future::read_async::<T>(input.channel).await
    }

    /// Convert the channel behind `input`, awaiting the end-of-conversion
    /// interrupt.
    ///
    /// Panics if the sample turned out to come from a different channel; see
    /// [`try_read_async`](Self::try_read_async) for the fallible variant.
    pub async fn read_async<P: AdcPin<T>>(
        &mut self,
        input: &AdcInput<'_, T, P>,
    ) -> u16 {
        match self.try_read_async(input).await {
            Ok(value) => value,
            Err(Error::InvalidChannel) => panic!("adc: channel out of range"),
            Err(Error::ChannelMismatch { expected, got }) => {
                panic!("adc: result came from channel {got}, expected {expected}")
            }
        }
    }
}
