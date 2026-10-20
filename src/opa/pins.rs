//! OPA pad roles of the SWM341 pin matrix.
//!
//! Pad assignments and 4-bit function selectors (`PORTx_SEL0/1`) below come
//! from the SWM341 datasheet V1.33, "放大器（OPA）" chapter and the PORTA /
//! PORTC function-select register tables:
//!
//! | unit | `INP` (non-inverting) | `INN` (inverting) | `OUT` |
//! |------|-----------------------|-------------------|-------|
//! | OPA0 | `PC8` func `2`        | `PC5` func `15`   | `PC9` func `15` |
//! | OPA1 | `PA11` func `15`      | `PA9` func `15`   | `PA10` func `15` |
//! | OPA2 | `PC2` func `15`       | `PC1` func `15`   | `PC3` func `15` |
//! | OPA3 | `PA14` func `15`      | `PA13` func `15`  | `PA15` func `15` |
//!
//! Every pad is analog-only for the OPA function (`15` is the analog path),
//! except `PC8` whose OPA0 positive input is selected by code `2`. On several
//! pads code `15` simultaneously selects the ADC channel and the OPA node, so
//! an amplifier output routed this way can be sampled by the ADC without any
//! external wiring (see the datasheet "ADC 复用" paragraph).
//!
//! The traits are generic over the amplifier [`Instance`] so the compiler
//! rejects wiring, say, OPA0's pad to an [`Opa`](crate::opa::Opa) handle for
//! OPA1 -- each pad is only implemented for the unit it is physically wired
//! to. Apply them through [`Opa::add_positive_pin`](crate::opa::Opa::add_positive_pin)
//! / [`add_negative_pin`](crate::opa::Opa::add_negative_pin) /
//! [`add_output_pin`](crate::opa::Opa::add_output_pin).

use crate::gpio::Pin;

use super::instance::Instance;

/// Positive (non-inverting) input pad of amplifier `T` (`OPAx_INP`).
pub trait OpaInputPPin<T: Instance>: Pin {
    /// 4-bit `PORTx_SEL` code selecting this pad's OPA input path.
    const FUNC: u8;
}

/// Negative (inverting) input pad of amplifier `T` (`OPAx_INN`).
pub trait OpaInputNPin<T: Instance>: Pin {
    /// 4-bit `PORTx_SEL` code selecting this pad's OPA input path.
    const FUNC: u8;
}

/// Output pad of amplifier `T` (`OPAx_OUT`).
pub trait OpaOutputPin<T: Instance>: Pin {
    /// 4-bit `PORTx_SEL` code selecting this pad's OPA output path.
    const FUNC: u8;
}

// ---- OPA0: PC8 (P, code 2), PC5 (N), PC9 (OUT) ----

impl OpaInputPPin<crate::Opa0> for crate::gpio::PC8 {
    const FUNC: u8 = 2;
}
impl OpaInputNPin<crate::Opa0> for crate::gpio::PC5 {
    const FUNC: u8 = 15;
}
impl OpaOutputPin<crate::Opa0> for crate::gpio::PC9 {
    const FUNC: u8 = 15;
}

// ---- OPA1: PA11 (P), PA9 (N), PA10 (OUT) ----

impl OpaInputPPin<crate::Opa1> for crate::gpio::PA11 {
    const FUNC: u8 = 15;
}
impl OpaInputNPin<crate::Opa1> for crate::gpio::PA9 {
    const FUNC: u8 = 15;
}
impl OpaOutputPin<crate::Opa1> for crate::gpio::PA10 {
    const FUNC: u8 = 15;
}

// ---- OPA2: PC2 (P), PC1 (N), PC3 (OUT) ----

impl OpaInputPPin<crate::Opa2> for crate::gpio::PC2 {
    const FUNC: u8 = 15;
}
impl OpaInputNPin<crate::Opa2> for crate::gpio::PC1 {
    const FUNC: u8 = 15;
}
impl OpaOutputPin<crate::Opa2> for crate::gpio::PC3 {
    const FUNC: u8 = 15;
}

// ---- OPA3: PA14 (P), PA13 (N), PA15 (OUT) ----

impl OpaInputPPin<crate::Opa3> for crate::gpio::PA14 {
    const FUNC: u8 = 15;
}
impl OpaInputNPin<crate::Opa3> for crate::gpio::PA13 {
    const FUNC: u8 = 15;
}
impl OpaOutputPin<crate::Opa3> for crate::gpio::PA15 {
    const FUNC: u8 = 15;
}
