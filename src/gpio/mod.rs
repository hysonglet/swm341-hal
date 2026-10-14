//! General purpose input / output.
//!
//! A pad on this chip is described by two register blocks: `PORTx` holds the
//! function mux and the electrical options (pull resistors, open drain, drive
//! strength, input buffer), `GPIOx` holds the data register and the interrupt
//! logic. Both are needed for any useful configuration, so the driver is written
//! against the *pad*: `PA3` is one singleton that owns the pad in both worlds.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! let p = hal::init(Default::default());
//!
//! let mut led = hal::gpio::Output::new(p.PA5, hal::gpio::Level::Low, Default::default());
//! led.set_high();
//!
//! let button = hal::gpio::Input::new(p.PA0, hal::gpio::Pull::Up);
//! if button.is_low() { /* pressed */ }
//! ```
//!
//! Selecting an alternate function is a type checked operation: [`Flex::set_func`]
//! only accepts the enum that belongs to that exact pad. The type-checked
//! `PinFuncOf` bindings for every pad are generated from the vendor pin matrix
//! and sunk into each peripheral's own `pins` module (e.g. [`crate::uart::pins`]).
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut pin = hal::gpio::Flex::new(p.PA2);
//! pin.set_func(hal::uart::pins::a::Pin2Func::Uart1Rx);
//! ```

mod hal;
pub(crate) mod instance;

#[cfg(feature = "embassy")]
mod future;

use core::marker::PhantomData;

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use instance::Instance;

#[cfg(feature = "embassy")]
use crate::mode::Async;
use crate::mode::{Blocking, Mode};

/// Binds one pad singleton to the bank it belongs to.
macro_rules! impl_singleton_pin {
    ($port:ident, $id:ident, $pin:ident, $number:literal) => {
        // Full paths: `pin_defs` is a child module and does not inherit imports.
        impl crate::gpio::Pin for crate::mcu::peripherals::$pin {}
        impl crate::gpio::sealed::Pin for crate::mcu::peripherals::$pin {
            const PORT: crate::gpio::Id = crate::gpio::Id::$id;
            type Port = crate::gpio::instance::$port;
            const NUMBER: u8 = $number;
        }
    };
}

mod pin_defs;

pub use crate::mcu::peripherals::{
    PA0, PA1, PA10, PA11, PA12, PA13, PA14, PA15, PA2, PA3, PA4, PA5, PA6, PA7, PA8, PA9, PB0, PB1,
    PB10, PB11, PB12, PB13, PB14, PB15, PB2, PB3, PB4, PB5, PB6, PB7, PB8, PB9, PC0, PC1, PC10,
    PC11, PC12, PC13, PC14, PC15, PC2, PC3, PC4, PC5, PC6, PC7, PC8, PC9, PD0, PD1, PD10, PD11,
    PD12, PD13, PD14, PD15, PD2, PD3, PD4, PD5, PD6, PD7, PD8, PD9, PE0, PE1, PE10, PE11, PE12,
    PE13, PE14, PE15, PE2, PE3, PE4, PE5, PE6, PE7, PE8, PE9, PM0, PM1, PM10, PM11, PM12, PM13,
    PM14, PM15, PM2, PM3, PM4, PM5, PM6, PM7, PM8, PM9, PN0, PN1, PN10, PN11, PN12, PN13, PN14,
    PN15, PN2, PN3, PN4, PN5, PN6, PN7, PN8, PN9,
};

/// GPIO bank of a pad.
///
/// The discriminants double as the index into the per-bank interrupt state, so
/// they are part of the ABI of this module and must stay contiguous from zero.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Id {
    A = 0,
    B,
    C,
    D,
    E,
    M,
    N,
}

/// Logic level of a pad.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Level {
    #[default]
    Low,
    High,
}

impl From<bool> for Level {
    fn from(value: bool) -> Self {
        if value {
            Level::High
        } else {
            Level::Low
        }
    }
}

impl From<Level> for bool {
    fn from(value: Level) -> Self {
        value == Level::High
    }
}

impl From<embedded_hal::digital::PinState> for Level {
    fn from(value: embedded_hal::digital::PinState) -> Self {
        match value {
            embedded_hal::digital::PinState::Low => Level::Low,
            embedded_hal::digital::PinState::High => Level::High,
        }
    }
}

impl From<Level> for embedded_hal::digital::PinState {
    fn from(value: Level) -> Self {
        match value {
            Level::Low => embedded_hal::digital::PinState::Low,
            Level::High => embedded_hal::digital::PinState::High,
        }
    }
}

/// Resistor configuration of an input.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pull {
    /// Floating input.
    #[default]
    None,
    /// Weak pull towards `VDD`.
    Up,
    /// Weak pull towards ground.
    Down,
}

/// Drive mode of an output.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OutputType {
    /// Driven in both directions.
    #[default]
    PushPull,
    /// Only pulled low, which needs an external pull-up.
    OpenDrain,
}

/// Drive strength of an output.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Speed {
    #[default]
    Normal,
    /// Higher current drive, at the cost of edge ringing.
    High,
}

/// Condition that raises a pin interrupt.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Trigger {
    /// Interrupt while the pad reads high; no software acknowledgement needed.
    LevelHigh,
    /// Interrupt while the pad reads low; no software acknowledgement needed.
    LevelLow,
    /// Interrupt on a low to high transition.
    EdgeRising,
    /// Interrupt on a high to low transition.
    EdgeFalling,
    /// Interrupt on either transition.
    EdgeBoth,
}

/// One entry of a pad's function mux.
pub trait PinFunc {
    /// The 4-bit code written into `PORTx.FUNC0` / `FUNC1`.
    fn value(self) -> u8;
}

/// Marks the [`PinFunc`] enum that belongs to the pad `P`.
///
/// Only the generated code in [`pins`] implements this, which is what makes
/// `set_func(B::Pin3Func::..)` on pin `A3` a compile error.
pub trait PinFuncOf<P: Pin>: PinFunc {}

pub(crate) mod sealed {
    pub trait Pin {
        /// Bank the pad belongs to.
        const PORT: super::Id;
        /// Index of the pad inside its bank.
        const NUMBER: u8;
        /// Sealed register access for that bank.
        type Port: super::instance::Instance;
    }
}

#[allow(private_bounds)]
pub trait Pin: sealed::Pin {
    /// Bank this pad belongs to.
    #[inline]
    fn port() -> Id {
        <Self as sealed::Pin>::PORT
    }

    /// Index of this pad within its bank, 0..=15.
    #[inline]
    fn number() -> u8 {
        <Self as sealed::Pin>::NUMBER
    }
}

#[inline(always)]
pub(crate) fn acquire<P: Pin>() {
    <P as sealed::Pin>::Port::acquire();
}

#[inline(always)]
pub(crate) fn release<P: Pin>() {
    <P as sealed::Pin>::Port::release();
}

#[inline(always)]
pub(crate) fn set_function<P: Pin>(function: u8) {
    <P as sealed::Pin>::Port::set_function(P::number(), function);
}

#[inline(always)]
fn set_direction<P: Pin>(output: bool) {
    <P as sealed::Pin>::Port::set_direction(P::number(), output);
}

#[inline(always)]
fn set_pull<P: Pin>(pull: Pull) {
    <P as sealed::Pin>::Port::set_pull(P::number(), pull);
}

#[inline(always)]
fn set_output_options<P: Pin>(output_type: OutputType, speed: Speed) {
    <P as sealed::Pin>::Port::set_output_options(P::number(), output_type, speed);
}

#[inline(always)]
fn write_level<P: Pin>(level: Level) {
    <P as sealed::Pin>::Port::set_level(P::number(), level);
}

#[inline(always)]
fn read_level<P: Pin>() -> Level {
    if <P as sealed::Pin>::Port::read(P::number()) {
        Level::High
    } else {
        Level::Low
    }
}

#[inline(always)]
fn toggle<P: Pin>() {
    <P as sealed::Pin>::Port::toggle(P::number());
}

/// A pad that has not been assigned a role.
///
/// `Flex` is the escape hatch of this module: it can change direction and mux
/// setting at any time, which is also why the typed wrappers below keep their
/// pad inside a `Flex` instead of re-implementing the register logic.
pub struct Flex<'d, P: Pin> {
    _pin: PeripheralRef<'d, P>,
}

impl<'d, P: Pin> Flex<'d, P> {
    /// Take ownership of `pin`, turning its bank clock on.
    pub fn new(pin: impl Peripheral<P = P> + 'd) -> Self {
        into_ref!(pin);
        acquire::<P>();
        Self { _pin: pin }
    }

    /// Take ownership of `pin` and configure it as an input with `pull`.
    fn new_into_input(pin: impl Peripheral<P = P> + 'd, pull: Pull) -> Self {
        let mut pin = Self::new(pin);
        pin.set_as_input(pull);
        pin
    }

    /// Route the pad through one of its alternate functions.
    pub fn set_func<F: PinFuncOf<P>>(&mut self, function: F) {
        set_function::<P>(function.value());
    }

    /// Route the pad back to the plain digital input/output path.
    pub fn set_as_gpio(&mut self) {
        set_function::<P>(0);
    }

    /// Configure the pad as an input.
    pub fn set_as_input(&mut self, pull: Pull) {
        set_function::<P>(0);
        set_direction::<P>(false);
        set_pull::<P>(pull);
    }

    /// Configure the pad as an output driving `initial`.
    pub fn set_as_output(&mut self, initial: Level, output_type: OutputType, speed: Speed) {
        set_function::<P>(0);
        set_output_options::<P>(output_type, speed);
        write_level::<P>(initial);
        set_direction::<P>(true);
    }

    /// Drive `level` while the pad is an output.
    pub fn set_level(&mut self, level: Level) {
        write_level::<P>(level);
    }

    /// Set the pad high.
    pub fn set_high(&mut self) {
        write_level::<P>(Level::High);
    }

    /// Set the pad low.
    pub fn set_low(&mut self) {
        write_level::<P>(Level::Low);
    }

    /// Invert the driven level.
    pub fn toggle(&mut self) {
        toggle::<P>();
    }

    /// Sample the pad, which works whether it is an input or an output.
    pub fn level(&self) -> Level {
        read_level::<P>()
    }

    /// True when the pad samples high.
    pub fn is_high(&self) -> bool {
        read_level::<P>() == Level::High
    }

    /// True when the pad samples low.
    pub fn is_low(&self) -> bool {
        read_level::<P>() == Level::Low
    }
}

impl<'d, P: Pin> Drop for Flex<'d, P> {
    fn drop(&mut self) {
        release::<P>();
    }
}

/// Configuration handed to [`Output::new`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct OutputConfig {
    /// Level driven before the pad becomes an output.
    pub initial_level: Level,
    /// Push-pull or open drain.
    pub output_type: OutputType,
    /// Output drive strength.
    pub speed: Speed,
}

impl OutputConfig {
    /// Set the level driven before the pad becomes an output.
    pub fn initial_level(self, initial_level: Level) -> Self {
        Self {
            initial_level,
            ..self
        }
    }

    /// Set the drive mode.
    pub fn output_type(self, output_type: OutputType) -> Self {
        Self {
            output_type,
            ..self
        }
    }

    /// Set the drive strength.
    pub fn speed(self, speed: Speed) -> Self {
        Self { speed, ..self }
    }
}

/// A pad driven as an output.
pub struct Output<'d, P: Pin> {
    inner: Flex<'d, P>,
}

impl<'d, P: Pin> Output<'d, P> {
    /// Drive `pin` as configured by `config`.
    pub fn new(pin: impl Peripheral<P = P> + 'd, config: OutputConfig) -> Self {
        let mut inner = Flex::new(pin);
        inner.set_as_output(config.initial_level, config.output_type, config.speed);
        Self { inner }
    }

    /// Drive `level` on the pad.
    pub fn set_level(&mut self, level: Level) {
        self.inner.set_level(level);
    }

    /// Drive the pad high.
    pub fn set_high(&mut self) {
        self.inner.set_high();
    }

    /// Drive the pad low.
    pub fn set_low(&mut self) {
        self.inner.set_low();
    }

    /// Invert the driven level.
    pub fn toggle(&mut self) {
        self.inner.toggle();
    }

    /// Level currently driven, as sampled on the pad.
    pub fn level(&self) -> Level {
        self.inner.level()
    }

    /// True when the pad reads high.
    pub fn is_set_high(&self) -> bool {
        self.inner.is_high()
    }

    /// True when the pad reads low.
    pub fn is_set_low(&self) -> bool {
        self.inner.is_low()
    }
}

/// A pad sampled as an input.
///
/// `M` selects whether waits are available: [`Blocking`] inputs only sample,
/// [`Async`] inputs additionally expose `wait_for_*`.
pub struct Input<'d, P: Pin, M: Mode = Blocking> {
    inner: Flex<'d, P>,
    _mode: PhantomData<M>,
}

impl<'d, P: Pin> Input<'d, P, Blocking> {
    /// Configure `pin` as an input with `pull`.
    ///
    /// Bound to [`Blocking`] on purpose: `Input::new` can then be used without
    /// spelling out the mode, and the async flavour has to be asked for through
    /// [`Input::new_async`].
    pub fn new(pin: impl Peripheral<P = P> + 'd, pull: Pull) -> Self {
        Self::build(Flex::new_into_input(pin, pull))
    }
}

#[cfg(feature = "embassy")]
impl<'d, P: Pin> Input<'d, P, Async> {
    /// Configure `pin` as an input whose `wait_for_*` methods are available.
    pub fn new_async(pin: impl Peripheral<P = P> + 'd, pull: Pull) -> Self {
        Self::build(Flex::new_into_input(pin, pull))
    }
}

impl<'d, P: Pin, M: Mode> Input<'d, P, M> {
    fn build(inner: Flex<'d, P>) -> Self {
        Self {
            inner,
            _mode: PhantomData,
        }
    }

    /// Sample the pad.
    pub fn level(&self) -> Level {
        self.inner.level()
    }

    /// True when the pad reads high.
    pub fn is_high(&self) -> bool {
        self.inner.is_high()
    }

    /// True when the pad reads low.
    pub fn is_low(&self) -> bool {
        self.inner.is_low()
    }
}

#[cfg(feature = "embassy")]
impl<'d, P: Pin> Input<'d, P, Async> {
    /// Wait until the pad reads high.
    pub async fn wait_for_high(&mut self) {
        future::wait_for::<P>(Trigger::LevelHigh).await;
    }

    /// Wait until the pad reads low.
    pub async fn wait_for_low(&mut self) {
        future::wait_for::<P>(Trigger::LevelLow).await;
    }

    /// Wait for a low to high transition.
    pub async fn wait_for_rising_edge(&mut self) {
        future::wait_for::<P>(Trigger::EdgeRising).await;
    }

    /// Wait for a high to low transition.
    pub async fn wait_for_falling_edge(&mut self) {
        future::wait_for::<P>(Trigger::EdgeFalling).await;
    }

    /// Wait for either transition.
    pub async fn wait_for_any_edge(&mut self) {
        future::wait_for::<P>(Trigger::EdgeBoth).await;
    }
}
