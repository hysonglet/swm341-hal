//! Configuration and shared types for the CORDIC co-processor.

/// A signed 16-bit Q14 fixed-point number (14 fractional bits, 2 integer bits).
///
/// The range is roughly `[-2.0, 2.0)`; the integer part is encoded in the top
/// two bits and the fractional part in the lower 14 bits, matching the CORDIC
/// hardware result format.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Q14(pub i16);

impl Q14 {
    /// The value `1.0` in Q14.
    pub const ONE: Q14 = Q14(1 << 14);

    /// Build a `Q14` from a floating-point value.
    pub fn from_f32(v: f32) -> Q14 {
        Q14((v * (1 << 14) as f32) as i16)
    }

    /// Convert back to a floating-point value.
    pub fn to_f32(self) -> f32 {
        self.0 as f32 / (1 << 14) as f32
    }

    /// The raw 16-bit two's-complement representation.
    pub fn to_bits(self) -> i16 {
        self.0
    }

    /// Build a `Q14` from its raw 16-bit two's-complement representation.
    pub fn from_bits(bits: i16) -> Q14 {
        Q14(bits)
    }
}

/// Driver-level error type (currently unused; reserved for future async paths).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The computation did not complete in the polling budget.
    Timeout,
}

/// CORDIC configuration (currently empty; reserved for future tuning).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {}
