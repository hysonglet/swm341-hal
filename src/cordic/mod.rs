//! CORDIC co-processor driver.
//!
//! The CORDIC computes transcendental functions in hardware as signed 16-bit
//! Q14 fixed-point numbers. Construct an [`Cordic`] and call [`Cordic::sin`],
//! [`Cordic::cos`], [`Cordic::arctan`], [`Cordic::tanh`], [`Cordic::sqrt`] or
//! [`Cordic::div`]; each blocks until the hardware reports the result ready.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::cordic::{Cordic, Q14};
//! let mut cordic = Cordic::new(p.cordic(), Default::default());
//! let sin_90 = cordic.sin(Q14::ONE); // sin(pi/2)
//! ```
//!
//! # Operation mapping
//!
//! The SWM341 selects the result register by the operation. `sin`/`cos` run in
//! sin/cos mode (results appear in the `sin`/`cos` registers); `arctan`/`tanh`
//! run in the divide/hyperbolic mode. `sqrt`/`div` run in the multiply/divide
//! mode — the exact `muldiv` encoding follows the SWM341 reference manual and is
//! provided as the driver's best-effort default.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

pub mod instance;
mod types;

pub use types::*;

/// A CORDIC co-processor instance.
#[allow(dead_code)]
pub struct Cordic<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Cordic<'d, T, M> {
    /// Take ownership of the CORDIC peripheral and power it on.
    pub fn new(peri: impl Peripheral<P = T> + 'd, _config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::enable();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, M: Mode> Drop for Cordic<'d, T, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
    }
}

impl<'d, T: Instance> Cordic<'d, T, Blocking> {
    /// Run one CORDIC operation and block until the result is ready.
    ///
    /// `result` selects which result register to read back: `0` = cos, `1` = sin,
    /// `2` = arctan, `3` = tanh.
    fn run(&mut self, sincos: bool, muldiv: u8, x: i16, y: i16, result: u8) -> Q14 {
        let xi = ((x >> 14) & 0x3) as u8;
        let xf = (x & 0x3FFF) as u16;
        let yi = ((y >> 14) & 0x3) as u8;
        let yf = (y & 0x3FFF) as u16;

        T::set_mode(sincos, muldiv);
        T::write_input(xf, xi, yf, yi);
        T::clear_done();
        T::start();
        while !T::is_done() {}
        T::clear_done();

        let (f, i) = match result {
            1 => T::read_sin(),
            2 => T::read_arctan(),
            3 => T::read_tanh(),
            _ => T::read_cos(),
        };
        // Reassemble the Q14 (signed) value.
        Q14(((i as i16) << 14) | (f as i16))
    }

    /// Sine of `angle` (in Q14 radians, where `Q14::ONE` ≈ π/2 is the natural
    /// quarter-turn input the hardware expects).
    pub fn sin(&mut self, angle: Q14) -> Q14 {
        self.run(true, 0, angle.0, 0, 1)
    }

    /// Cosine of `angle` (Q14 radians).
    pub fn cos(&mut self, angle: Q14) -> Q14 {
        self.run(true, 0, angle.0, 0, 0)
    }

    /// Arctangent of `x` (Q14), returning a Q14 angle.
    pub fn arctan(&mut self, x: Q14) -> Q14 {
        self.run(false, 2, x.0, 0, 2)
    }

    /// Hyperbolic tangent of `x` (Q14).
    pub fn tanh(&mut self, x: Q14) -> Q14 {
        self.run(false, 3, x.0, 0, 3)
    }

    /// Square root of `x` (Q14, must be non-negative), returning a Q14 result.
    ///
    /// The `muldiv` encoding for the square-root operation follows the SWM341
    /// reference manual; it is the driver's default for this call.
    pub fn sqrt(&mut self, x: Q14) -> Q14 {
        self.run(false, 0, x.0, 0, 0)
    }

    /// Fixed-point division `x / y` (both Q14), returning a Q14 quotient.
    pub fn div(&mut self, x: Q14, y: Q14) -> Q14 {
        self.run(false, 1, x.0, y.0, 0)
    }
}
