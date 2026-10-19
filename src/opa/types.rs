//! Configuration types for the operational amplifiers.

/// Configuration for one operational amplifier unit.
///
/// The amplifiers expose only a power switch in hardware, so this is the sole
/// construction-time setting.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// Power the amplifier on immediately after construction.
    ///
    /// When `false` (the [`Default`]) the unit stays powered down until
    /// [`Opa::enable`](crate::opa::Opa::enable) is called.
    pub enable: bool,
}
