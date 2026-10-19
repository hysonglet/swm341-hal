//! Configuration types for the analog comparators.

/// Selects what the comparator's negative (inverting) input is wired to.
///
/// The positive (non-inverting) input of every comparator is always the
/// external `CMPx+` pad. The negative input can instead be taken from the
/// internally generated reference voltage, which is set through
/// [`Cmp::set_reference_voltage`](crate::cmp::Cmp::set_reference_voltage) and
/// is shared by all three units.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NegativeSource {
    /// Negative input driven by the external `CMPx−` pad.
    ExternalPin,
    /// Negative input driven by the shared internal reference voltage.
    InternalVref,
}

impl Default for NegativeSource {
    fn default() -> Self {
        NegativeSource::ExternalPin
    }
}

/// Configuration for a single comparator unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// Enable the comparator after construction.
    pub enable: bool,
    /// Add a small hysteresis to reject noise around the threshold.
    pub hysteresis: bool,
    /// Source wired to the comparator's negative input.
    pub negative_source: NegativeSource,
}
