//! Error and status types for the embedded flash.

/// Errors a flash operation can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The address does not meet the alignment the operation requires:
    /// [`PAGE_SIZE`](crate::flash::PAGE_SIZE) for an erase,
    /// [`PROGRAM_ALIGN`](crate::flash::PROGRAM_ALIGN) for a program.
    Unaligned,
    /// The requested range reaches past the end of the 512 KB array.
    OutOfRange,
    /// The payload is not a whole number of four-word program groups.
    InvalidLength,
    /// The ROM routine rejected the request (it returned non-zero).
    Rejected,
}

/// Shorthand for a fallible flash operation.
pub type Result<T> = core::result::Result<T, Error>;

/// Snapshot of the `FMC.STAT` register (datasheet 6.29.5).
///
/// The IAP routines block until they are done, so callers rarely need this; it
/// is exposed for diagnosing a flash that appears stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    bits: u32,
}

impl Status {
    /// Wrap a raw `FMC.STAT` reading.
    ///
    /// The field stays private so the snapshot can only come from the hardware,
    /// but it has to be constructible from outside this file -- hence this
    /// `pub(crate)` constructor rather than a public field.
    pub(crate) const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// True when the eFlash is idle (`STAT.IDLE`).
    pub fn idle(&self) -> bool {
        self.bits & (1 << 31) != 0
    }

    /// True while an erase is in progress (`STAT.ERASEBUSY`).
    pub fn erase_busy(&self) -> bool {
        self.bits & (1 << 0) != 0
    }

    /// True while a program is in progress (`STAT.PROGBUSY`).
    pub fn program_busy(&self) -> bool {
        self.bits & (1 << 1) != 0
    }

    /// True while a read is in progress (`STAT.READBUSY`).
    pub fn read_busy(&self) -> bool {
        self.bits & (1 << 2) != 0
    }

    /// True when the write FIFO holds no data (`STAT.FIFOEMPTY`).
    pub fn fifo_empty(&self) -> bool {
        self.bits & (1 << 3) != 0
    }

    /// True when the write FIFO cannot accept more data (`STAT.FIFOFULL`).
    pub fn fifo_full(&self) -> bool {
        self.bits & (1 << 4) != 0
    }

    /// Raw register value.
    pub fn bits(&self) -> u32 {
        self.bits
    }
}

#[cfg(test)]
mod tests {
    //! Bit decoding tests. `Status` snapshots `FMC.STAT` verbatim, so this is
    //! the one piece of the register layer that can be checked off-target; see
    //! the `mod tests` in [`super::super`] for how these are run.
    use super::*;

    #[test]
    fn all_clear_reads_as_not_busy() {
        let idle = Status::from_bits(0);
        assert!(!idle.idle());
        assert!(!idle.erase_busy());
        assert!(!idle.program_busy());
        assert!(!idle.read_busy());
        assert!(!idle.fifo_empty());
        assert!(!idle.fifo_full());
    }

    /// A raw bit together with the accessor that is supposed to report it.
    type FlagCase = (u32, fn(&Status) -> bool);

    #[test]
    fn each_flag_is_read_from_its_own_bit() {
        // (raw value, accessor) pairs; a stray `<<` shows up as several
        // predicates flipping together in a single reading.
        let cases: [FlagCase; 6] = [
            (1 << 31, Status::idle),
            (1 << 0, Status::erase_busy),
            (1 << 1, Status::program_busy),
            (1 << 2, Status::read_busy),
            (1 << 3, Status::fifo_empty),
            (1 << 4, Status::fifo_full),
        ];

        for (bit, accessor) in cases {
            let status = Status::from_bits(bit);
            assert!(accessor(&status), "bit {bit:#010x} should set this flag");
            assert_eq!(status.bits(), bit);

            // No other flag responds to this bit.
            for (other, other_accessor) in cases {
                if other != bit {
                    assert!(!other_accessor(&status), "bit {bit:#010x} sets too much");
                }
            }
        }
    }

    #[test]
    fn unknown_bits_are_preserved() {
        for bits in [0x8000_001F, 0x0000_0010, 0x1234_5678] {
            assert_eq!(Status::from_bits(bits).bits(), bits);
        }
    }
}
