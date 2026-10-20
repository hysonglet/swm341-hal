//! Configuration and shared types for the HALL interface.

use crate::gpio::Pull;

/// Which of the two hall interfaces to drive.
///
/// The silicon names them `HALL0` and `HALL3`, after their capture registers
/// (`TIMRG.HALL0V` / `TIMRG.HALL3V`) and interrupts (`HALL0` = NVIC 12,
/// `HALL3` = NVIC 28); there is no `HALL1`/`HALL2` interface.
///
/// Beware two overlapping numberings: the *interfaces* are 0 and 3, while the
/// six *input pins* are individually numbered `HALL0..HALL5` -- `HALL0..2` are
/// inputs 0..2 of [`HallSelect::Hall0`], and `HALL3..5` the inputs of
/// [`HallSelect::Hall3`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HallSelect {
    /// First hall interface. Paired with TIMR0 (its edges latch TIMR0's count
    /// into `TIMRG.HALL0V`); interrupt `HALL0` (NVIC 12).
    Hall0,
    /// Second hall interface. Paired with TIMR3 (edges latch TIMR3's count into
    /// `TIMRG.HALL3V`); interrupt `HALL3` (NVIC 28).
    Hall3,
}

impl HallSelect {
    /// Register selector used by the driver: `0` for `HALL0`, `1` for `HALL3`.
    ///
    /// Not the interface number -- `HALLEN`/`HALLIE`/`HALLIF`/`HALLSR` expose the
    /// two interfaces as separate fields, so the driver only needs to pick one.
    pub(crate) fn which(self) -> u8 {
        match self {
            HallSelect::Hall0 => 0,
            HallSelect::Hall3 => 1,
        }
    }
}

/// HALL configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Arm the input-change interrupt on creation.
    pub enable_interrupt: bool,
    /// Resistor applied to the three hall input pads.
    ///
    /// Defaults to [`Pull::Up`]: hall sensors are normally open-drain, and the
    /// vendor `SimplHALL` example enables the pads' pull-ups too. Set
    /// [`Pull::None`] for a sensor that drives the lines push-pull.
    pub pull: Pull,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enable_interrupt: false,
            pull: Pull::Up,
        }
    }
}
