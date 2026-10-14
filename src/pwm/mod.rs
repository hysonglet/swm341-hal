//! Pulse-width modulation (PWM0..PWM4).
//!
//! Each PWM drives up to two complementary outputs (`A` and `B`) by comparing a
//! free-running counter against [`Config::period`] and [`Config::duty_a`]
//! ([`Config::duty_b`] through [`Pwm::set_duty_b`]). [`Pwm::new`] opens the shared
//! clock gate and programs the defaults; [`Pwm::start`]/`[`Pwm::stop`] arm and
//! halt the output. With the `embassy` feature, [`Pwm::wait_period`] awaits the
//! period-elapsed interrupt instead of polling.
//!
//! The type-checked pad roles (`PWMx{A,B,AN,BN}`, plus the `PWM_BRKx` break and
//! `PWM_CLKx` clock inputs) live in [`crate::pwm::pins`] and are applied with
//! [`crate::gpio::set_function`] after [`crate::gpio::acquire`].
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut pwm = hal::pwm::Pwm::new(p.PWM0, hal::pwm::Config::default());
//! pwm.set_duty_a(250);          // 25% duty on the A output
//! pwm.start();
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

/// A PWM driven by its peripheral clock.
#[allow(dead_code)]
pub struct Pwm<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    config: Config,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Pwm<'d, T, M> {
    /// Set the counter top (`PERIOD`).
    pub fn set_period(&self, period: u32) {
        T::set_period(period);
    }

    /// Set the `A` channel duty (`CMPA`).
    pub fn set_duty_a(&self, duty: u32) {
        T::set_duty_a(duty);
    }

    /// Set the `B` channel duty (`CMPB`).
    pub fn set_duty_b(&self, duty: u32) {
        T::set_duty_b(duty);
    }

    /// Start the output (run mode from [`Config::mode`]).
    pub fn start(&self) {
        T::set_mode(self.config.mode);
    }

    /// Stop the output (`CR.MODE = 0`).
    pub fn stop(&self) {
        T::set_mode(0);
    }

    /// Clear the pending period-elapsed flag.
    pub fn clear_overflow(&self) {
        T::clear_overflow();
    }

    /// Enable or disable the period-elapsed interrupt.
    pub fn enable_overflow_interrupt(&self, enable: bool) {
        T::set_overflow_irq(enable);
    }
}

impl<'d, T: Instance, M: Mode> Drop for Pwm<'d, T, M> {
    fn drop(&mut self) {
        T::set_mode(0);
        T::clock().close();
    }
}

impl<'d, T: Instance> Pwm<'d, T, Blocking> {
    /// Build a PWM from its peripheral and configuration.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        T::set_period(config.period);
        T::set_duty_a(config.duty_a);
        T::configure_clock(config.clock_source, config.clock_divider);
        T::configure_output();
        Self {
            peri,
            config,
            _mode: core::marker::PhantomData,
        }
    }
}

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
impl<'d, T: Instance> Pwm<'d, T, Async> {
    /// Async constructor; like [`new`](Self::new) but bound to the `Async` mode so
    /// the `wait_period` future is available.
    pub fn new_async(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::set_period(config.period);
        T::set_duty_a(config.duty_a);
        T::configure_clock(config.clock_source, config.clock_divider);
        T::configure_output();
        Self {
            peri,
            config,
            _mode: core::marker::PhantomData,
        }
    }

    /// Wait until one full PWM period has elapsed.
    pub async fn wait_period(&self) {
        future::wait_period::<T>().await;
    }
}
