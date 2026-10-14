//! Configuration and shared types for the real-time clock.

/// A calendar date/time read from or written to the RTC.
///
/// `weekday` uses the convention `1` = Monday … `7` = Sunday (matching the RTC's
/// `MONDAY.DAY` field). Fields are stored as plain binary numbers (not BCD).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Datetime {
    /// Year (e.g. 2024). Stored as a raw value in the `YEAR` register.
    pub year: u32,
    /// Month, 1..=12.
    pub month: u8,
    /// Day of month, 1..=31.
    pub date: u8,
    /// Day of week, 1 (Monday) .. 7 (Sunday).
    pub weekday: u8,
    /// Hour, 0..=23.
    pub hour: u8,
    /// Minute, 0..=59.
    pub min: u8,
    /// Second, 0..=59.
    pub sec: u8,
}

/// RTC configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// Clock trimming value written to the `TRIM` register (0 = no trim).
    pub trim: u8,
}

/// Driver error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// A polled operation did not complete in budget.
    Timeout,
}
