//! On-chip operational amplifiers (OPA0–OPA3).
//!
//! The SWM341 integrates four operational amplifier units. Each is a dedicated
//! analog block whose non-inverting / inverting inputs and output sit on fixed
//! analog pads, so unlike digital peripherals there is no `PORTx.FUNC` mux to
//! program — the only software control is the unit's power switch
//! (`SYS.OPACR.OPAxON`, one bit per amplifier). The amplifiers also have no
//! status register and no NVIC vector, which is why this driver is a plain
//! power-control API with no blocking / async split.
//!
//! The analog control block is always clocked (no per-OPA gate exists in
//! `SYS.CLKEN0/1`), so construction needs no clock setup beyond the
//! documentation-only [`PeripheralClockIndex::Opa`](crate::clock::PeripheralClockIndex::Opa)
//! gate, which is a no-op like the comparator one.
//!
//! Note: the SVD behind the PAC models only the four `OPAxON` power bits; PGA
//! gain / mode selection is not part of that register description and is
//! therefore not exposed here.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! // Power OPA0 up immediately.
//! let opa = hal::opa::Opa::new(p.OPA0, hal::opa::Config { enable: true });
//! ```
//!
//! Or construct it powered-down and switch it later:
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let opa = hal::opa::Opa::new(p.OPA0, hal::opa::Config::default());
//! opa.enable();
//! assert!(opa.is_enabled());
//! opa.disable();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use instance::Instance;

mod instance;
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
}

impl<'d, T: Instance> Drop for Opa<'d, T> {
    fn drop(&mut self) {
        // The amplifiers live inside the always-on analog block, so this gate
        // is a no-op (see `clock::PeripheralClockIndex::Opa`). Mirroring the
        // comparator driver, the unit itself is left powered as configured.
        T::clock().close();
    }
}
