//! DWT configuration and result types.

/// A watchpoint (address / data comparator) configuration.
///
/// A watchpoint fires -- generating a debug event or a trace packet -- when the
/// configured condition matches. Without a debugger attached the event stalls
/// the core (it behaves like a breakpoint), so watchpoints are mainly useful
/// while debugging or to drive trace analysis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watchpoint {
    /// Address (for PC / data watchpoints) to compare against.
    pub address: u32,
    /// Number of low address bits ignored by the comparator (range matching).
    /// `0` means an exact match.
    pub mask: u8,
    /// What kind of access to match.
    pub kind: WatchpointKind,
}

impl Watchpoint {
    /// Watchpoint on the executed instruction address (PC).
    pub const fn instruction(address: u32) -> Self {
        Self {
            address,
            mask: 0,
            kind: WatchpointKind::Instruction,
        }
    }

    /// Watchpoint on a data read at `address`.
    pub const fn data_read(address: u32) -> Self {
        Self {
            address,
            mask: 0,
            kind: WatchpointKind::DataRead,
        }
    }

    /// Watchpoint on a data write at `address`.
    pub const fn data_write(address: u32) -> Self {
        Self {
            address,
            mask: 0,
            kind: WatchpointKind::DataWrite,
        }
    }

    /// Watchpoint on any data access (read or write) at `address`.
    pub const fn data_read_write(address: u32) -> Self {
        Self {
            address,
            mask: 0,
            kind: WatchpointKind::DataReadWrite,
        }
    }

    /// Set the comparator mask (range matching), in bits.
    pub const fn with_mask(mut self, mask: u8) -> Self {
        self.mask = mask;
        self
    }
}

/// What a [`Watchpoint`] matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchpointKind {
    /// Match the executed instruction address (PC).
    Instruction,
    /// Match a data read at the address.
    DataRead,
    /// Match a data write at the address.
    DataWrite,
    /// Match a data read or write at the address.
    DataReadWrite,
}

impl WatchpointKind {
    /// DWT `FUNCTION[3:0]` code for this kind.
    pub(crate) const fn function_code(self) -> u8 {
        match self {
            WatchpointKind::Instruction => 0x1,
            WatchpointKind::DataRead => 0x2,
            WatchpointKind::DataWrite => 0x3,
            WatchpointKind::DataReadWrite => 0x4,
        }
    }
}

/// Errors returned by the DWT driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwtError {
    /// The requested comparator index exceeds the register map (max 15).
    ComparatorIndex,
}
