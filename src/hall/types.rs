//! Configuration and shared types for the HALL interface.

/// Which of the two hall interfaces to drive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HallSelect {
    /// The first hall interface (interrupt `HALL0`).
    Hall0,
    /// The second hall interface (the silicon calls it `HALL3`; interrupt `HALL1`).
    Hall1,
}

impl HallSelect {
    pub(crate) fn which(self) -> u8 {
        match self {
            HallSelect::Hall0 => 0,
            HallSelect::Hall1 => 1,
        }
    }
}

/// HALL configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// Arm the input-change interrupt on creation.
    pub enable_interrupt: bool,
}
