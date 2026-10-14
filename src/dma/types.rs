//! Configuration and shared types for the DMA controller.

/// DMA driver error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A transfer did not complete within the polling budget.
    Timeout,
    /// The requested channel index is out of range (valid channels are 0..3).
    InvalidChannel,
}

/// DMA configuration (currently empty; all setup is per-transfer).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {}
