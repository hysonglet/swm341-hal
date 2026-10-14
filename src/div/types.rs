//! Divider result and configuration types.

/// Result of [`crate::div::Div::divide`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DivResult {
    /// Quotient of the division.
    pub quotient: u32,
    /// Remainder of the division.
    pub remainder: u32,
}

impl DivResult {
    /// Build a result from raw register reads.
    pub(crate) fn new(quotient: u32, remainder: u32) -> Self {
        Self {
            quotient,
            remainder,
        }
    }
}
