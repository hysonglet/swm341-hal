//! Configuration and shared types for the quadrature encoder interface.

/// Decode resolution of the A/B phase pair.
///
/// Selects how many counts one A/B cycle produces (`CR.X2X4`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CountMode {
    /// Counts on both edges of phase A only; phase B only decides direction.
    X2,
    /// Counts on every edge of both phase A and phase B (four counts/cycle).
    #[default]
    X4,
}

impl CountMode {
    /// `CR.X2X4` value (`0` = X2, `1` = X4).
    pub(crate) fn bit(self) -> bool {
        match self {
            CountMode::X2 => false,
            CountMode::X4 => true,
        }
    }
}

/// What resets the 16-bit position counter (`CR.RSTSRC`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ResetMode {
    /// Reset when `POSCNT` reaches `MAXCNT` (forward) or zero (reverse).
    #[default]
    Match,
    /// Reset on the index (Z) phase pulse.
    Index,
}

impl ResetMode {
    /// `CR.RSTSRC` value (`0` = match, `1` = index).
    pub(crate) fn bit(self) -> bool {
        match self {
            ResetMode::Match => false,
            ResetMode::Index => true,
        }
    }
}

/// One of the four QEI interrupt sources.
///
/// Every source has four control bits spread over `IE` (enable), `IM`
/// (unmask), `IC` (write-1-to-clear) and `IF`/`IFOV` (status / overflow).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interrupt {
    /// Counter reset by the index (Z) phase.
    Index,
    /// Position counter matched `MAXCNT`.
    Match,
    /// Position counter overflowed/underflowed.
    CountOverflow,
    /// Illegal A/B transition (both phases changed at once).
    Error,
}

impl Interrupt {
    /// One-hit mask of this source inside `IE`/`IM`/`IC`/`IF`/`IFOV`.
    pub(crate) fn mask(self) -> u32 {
        1 << match self {
            Interrupt::Index => 0,
            Interrupt::Match => 1,
            Interrupt::CountOverflow => 2,
            Interrupt::Error => 3,
        }
    }
}

/// The four QEI interrupt sources as a set of bool flags.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Interrupts {
    /// Index-phase reset interrupt.
    pub index: bool,
    /// Count-match interrupt.
    pub compare_match: bool,
    /// Counter overflow/underflow interrupt.
    pub count_overflow: bool,
    /// Quadrature error interrupt.
    pub error: bool,
}

impl Interrupts {
    /// No sources selected.
    pub const NONE: Interrupts = Interrupts {
        index: false,
        compare_match: false,
        count_overflow: false,
        error: false,
    };

    /// Raw bit pattern for `IE`/`IM` (bits 0..=3).
    pub(crate) fn bits(self) -> u32 {
        (self.index as u32)
            | ((self.compare_match as u32) << 1)
            | ((self.count_overflow as u32) << 2)
            | ((self.error as u32) << 3)
    }
}

/// QEI configuration applied by [`Qei::new`](crate::qei::Qei::new).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// X2/X4 decode resolution (`CR.X2X4`).
    pub count_mode: CountMode,
    /// Counter reset source (`CR.RSTSRC`).
    pub reset_mode: ResetMode,
    /// Swap the A and B phase inputs (`CR.ABSWAP`), which inverts the counted
    /// direction without rewiring the encoder.
    pub swap_ab: bool,
    /// Top value used by [`ResetMode::Match`] and as the reverse-wrap reload
    /// (`MAXCNT`). Wrapped to 16 bits.
    pub max_count: u16,
    /// Interrupt sources enabled *and* unmasked on construction. The NVIC
    /// vector still has to be unmasked separately for the CPU to take it.
    pub interrupts: Interrupts,
    /// Enable the decoder immediately after configuration (`CR.ENA`).
    pub enable: bool,
}
