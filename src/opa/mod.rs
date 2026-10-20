//! On-chip operational amplifiers (OPA0–OPA3).
//!
//! The SWM341 integrates four operational amplifier units. Each has a
//! non-inverting input (`OPAx_INP`), an inverting input (`OPAx_INN`) and an
//! output (`OPAx_OUT`) on fixed pads listed in [`pins`]. The pads default to
//! their digital function, so before a unit can be used externally every pad
//! the application needs must be routed to its analog function with
//! [`add_positive_pin`](Opa::add_positive_pin) /
//! [`add_negative_pin`](Opa::add_negative_pin) /
//! [`add_output_pin`](Opa::add_output_pin) (they program the per-pad
//! `PORTx_SEL` code). A unit is powered through `SYS.OPACR.OPAxON`; there is no
//! status register and no NVIC vector, which is why this driver is a plain
//! pin-routing + power-control API with no blocking / async split.
//!
//! The analog control block is always clocked (no per-OPA gate exists in
//! `SYS.CLKEN0/1`), so construction needs no clock setup beyond the
//! documentation-only [`PeripheralClockIndex::Opa`](crate::clock::PeripheralClockIndex::Opa)
//! gate, which is a no-op like the comparator one.
//!
//! Note: the SVD behind the PAC models only the four `OPAxON` power bits; PGA
//! gain / bias options described in the datasheet (`PGACR`, `OPAxIB`, ...) are
//! not part of that register description and are therefore not exposed here.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! // OPA0 as an external voltage follower: short PC9 (OUT) to PC5 (INN),
//! // feed the signal into PC8 (INP), and read the buffered level on PC9.
//! let mut opa = hal::opa::Opa::new(p.opa0(), hal::opa::Config { enable: true });
//! opa.add_positive_pin(p.PC8);
//! opa.add_negative_pin(p.PC5);
//! opa.add_output_pin(p.PC9);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use instance::Instance;
use pins::{OpaInputNPin, OpaInputPPin, OpaOutputPin};

mod instance;
pub mod pins;
pub mod types;
pub use types::*;

/// A handle to one on-chip operational amplifier.
///
/// Owning the singleton guarantees exclusive control of the unit; all
/// configuration lives in the shared `SYS.OPACR` register and is reached
/// through the per-unit bit slice described by [`Instance`].
#[allow(dead_code)]
pub struct Opa<'d, T: Instance> {
    peri: PeripheralRef<'d, T>,
}

impl<'d, T: Instance> Opa<'d, T> {
    /// Take ownership of an amplifier unit and apply `config`.
    ///
    /// The unit stays powered down unless [`Config::enable`] is set; use
    /// [`enable`](Self::enable) / [`disable`](Self::disable) afterwards to
    /// switch it at any time.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        if config.enable {
            T::enable_unit();
        }

        Self { peri }
    }

    /// Power the amplifier on (`SYS.OPACR.OPAxON` set).
    pub fn enable(&self) {
        T::enable_unit();
    }

    /// Power the amplifier down (`SYS.OPACR.OPAxON` clear).
    pub fn disable(&self) {
        T::disable_unit();
    }

    /// Read the current power state of the unit (`true` => enabled).
    pub fn is_enabled(&self) -> bool {
        T::is_enabled()
    }

    /// Route the positive (non-inverting) input pad to the OPA analog path.
    ///
    /// Turns the pad's port clock on and writes the pad's `PORTx_SEL` code
    /// from [`pins::OpaInputPPin`]. The compiler only accepts the pad wired to
    /// this unit (`PC8` for OPA0, `PA11` for OPA1, `PC2` for OPA2,
    /// `PA14` for OPA3).
    pub fn add_positive_pin<P: OpaInputPPin<T>>(&mut self, pin: impl Peripheral<P = P> + 'd) {
        let _flex = crate::gpio::Flex::new(pin);
        crate::gpio::set_function::<P>(P::FUNC);
    }

    /// Route the negative (inverting) input pad to the OPA analog path.
    ///
    /// Pad per unit: `PC5` (OPA0), `PA9` (OPA1), `PC1` (OPA2), `PA13` (OPA3).
    pub fn add_negative_pin<P: OpaInputNPin<T>>(&mut self, pin: impl Peripheral<P = P> + 'd) {
        let _flex = crate::gpio::Flex::new(pin);
        crate::gpio::set_function::<P>(P::FUNC);
    }

    /// Route the output pad to the OPA analog path.
    ///
    /// Pad per unit: `PC9` (OPA0), `PA10` (OPA1), `PC3` (OPA2), `PA15` (OPA3).
    /// With the pad in analog mode the amplifier output can also be sampled by
    /// the ADC channel multiplexed on the same pad without external wiring.
    pub fn add_output_pin<P: OpaOutputPin<T>>(&mut self, pin: impl Peripheral<P = P> + 'd) {
        let _flex = crate::gpio::Flex::new(pin);
        crate::gpio::set_function::<P>(P::FUNC);
    }
}

impl<'d, T: Instance> Drop for Opa<'d, T> {
    fn drop(&mut self) {
        // The amplifiers live inside the always-on analog block, so this gate
        // is a no-op (see `clock::PeripheralClockIndex::Opa`). Mirroring the
        // comparator driver, the unit itself is left powered as configured.
        T::clock().close();
    }
}
