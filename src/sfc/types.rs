//! SFC configuration and error types.

/// Bus width the SFC uses for the AHB read burst and for page programming.
///
/// `Dual` / `Quad` only take effect once the flash's Quad Enable bit is set; the
/// driver leaves that to the caller through [`crate::sfc::Sfc::set_quad`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    /// 1-bit data (standard SPI).
    Single = 0,
    /// 2-bit (dual) data.
    Dual = 1,
    /// 4-bit (quad) data.
    Quad = 2,
}

/// Serial-clock divider relative to HCLK.
///
/// `SCK = HCLK / 2^(value + 1)`, i.e. `Div1` is the full HCLK and `Div8` is the
/// slowest setting -- the safe choice for a fly-wired board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClkDiv {
    /// No division.
    Div1 = 0,
    /// HCLK / 2.
    Div2 = 1,
    /// HCLK / 4.
    Div4 = 2,
    /// HCLK / 8.
    Div8 = 3,
}

/// Operational configuration for the SFC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Serial-clock divider. Defaults to [`ClkDiv::Div8`].
    pub clk_div: ClkDiv,
    /// Data width used when reading through the AHB window (`0x7000_0000`).
    pub read_width: Width,
    /// Data width used when programming a page.
    pub program_width: Width,
    /// Opcode the SFC issues for an AHB read (defaults to `0x03`, the standard
    /// 1-1-1 read).
    pub read_command: u8,
    /// Opcode the SFC issues for an AHB page program (defaults to `0x02`).
    pub program_command: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            clk_div: ClkDiv::Div8,
            read_width: Width::Single,
            program_width: Width::Single,
            read_command: 0x03,
            program_command: 0x02,
        }
    }
}

/// Errors returned by the SFC driver.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The flash did not clear its write-in-progress flag within the SFC's
    /// polling budget after an erase or program.
    Timeout,
}
