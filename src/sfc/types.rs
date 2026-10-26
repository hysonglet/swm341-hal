//! SFC configuration, geometry constants, opcodes and error types.

/// Size of the memory-mapped serial-flash window: `0x7000_0000 .. 0x7FFF_FFFF`.
pub const FLASH_WINDOW_SIZE: u32 = 0x1000_0000;

/// Base address of the memory-mapped serial-flash window (`SERIALFLASH` in
/// table 6-1 of the datasheet).
pub const FLASH_BASE: u32 = 0x7000_0000;

/// One page program may not cross this boundary (256 bytes on every SPI NOR
/// the SWM341 is used with).
pub const PAGE_SIZE: u32 = 256;

/// Smallest erasable unit (4 KiB sector erase, opcode `0x20`).
pub const SECTOR_SIZE: u32 = 4096;

/// 32 KiB block erase granularity (opcode `0x52`).
pub const BLOCK32K_SIZE: u32 = 32 * 1024;

/// 64 KiB block erase granularity (opcode `0xD8`).
pub const BLOCK64K_SIZE: u32 = 64 * 1024;

/// Number of status-register polls before a blocking operation gives up.
///
/// One poll is a full `GO`-driven `0x05` transaction, i.e. a couple of
/// microseconds at typical core clocks, so this budget covers sector (~400 ms)
/// and 64 KiB block erase (~2 s) with room to spare. A **chip** erase can take
/// tens of seconds: call [`crate::sfc::Sfc::erase_with`] with `wait = false`
/// and then [`crate::sfc::Sfc::wait_ready_with`] with a larger budget.
pub const WIP_POLL_LIMIT: u32 = 8_000_000;

/// Manufacturer byte of a Winbond SPI NOR (`0xEF`).
pub const MANUFACTURER_WINBOND: u8 = 0xEF;

// --- SPI NOR opcodes -------------------------------------------------------
// Mirrors the `SFC_CMD_*` list in the vendor `SWM341_sfc.h`.

/// Write enable (`0x06`). Sent automatically for every command type that the
/// datasheet marks "自动包含写使能", using `CMDAHB.WREN`.
pub const CMD_WRITE_ENABLE: u8 = 0x06;
/// Read status register 1 (`0x05`); bit 0 is the write-in-progress flag.
pub const CMD_READ_STATUS_REG1: u8 = 0x05;
/// Read status register 2 (`0x35`); bit 1 is the Quad Enable flag.
pub const CMD_READ_STATUS_REG2: u8 = 0x35;
/// Read status register 3 (`0x15`).
pub const CMD_READ_STATUS_REG3: u8 = 0x15;
/// Write status register 1 (`0x01`, 16-bit payload).
pub const CMD_WRITE_STATUS_REG1: u8 = 0x01;
/// Write status register 2 (`0x31`).
pub const CMD_WRITE_STATUS_REG2: u8 = 0x31;
/// Write status register 3 (`0x11`).
pub const CMD_WRITE_STATUS_REG3: u8 = 0x11;
/// Read JEDEC ID (`0x9F`); three bytes: manufacturer, memory type, capacity.
pub const CMD_READ_JEDEC: u8 = 0x9F;
/// Read the 64-bit unique ID (`0x4B`).
///
/// The SFC has no "read N bytes" command type, so this opcode is only usable
/// through the bit-banged SPI path the vendor library calls `SFC_GPIOReadUID`;
/// this driver does not implement that path.
pub const CMD_READ_UID: u8 = 0x4B;
/// Standard (1-1-1) page program (`0x02`).
pub const CMD_PAGE_PROGRAM: u8 = 0x02;
/// Quad-input page program (`0x32`, address on one line, data on four).
pub const CMD_PAGE_PROGRAM_QUAD: u8 = 0x32;
/// 4 KiB sector erase (`0x20`).
pub const CMD_ERASE_SECTOR: u8 = 0x20;
/// 32 KiB block erase (`0x52`).
pub const CMD_ERASE_BLOCK32K: u8 = 0x52;
/// 64 KiB block erase (`0xD8`).
pub const CMD_ERASE_BLOCK64K: u8 = 0xD8;
/// Whole-chip erase (`0x60`).
pub const CMD_ERASE_CHIP: u8 = 0x60;

/// Standard (1-1-1) read (`0x03`).
pub const CMD_READ: u8 = 0x03;
/// Dual-output (2xIO) fast read (`0xBB`).
pub const CMD_READ_DUAL: u8 = 0xBB;
/// Quad-output (4xIO) fast read (`0xEB`).
pub const CMD_READ_QUAD: u8 = 0xEB;

/// Data-line width the SFC uses when *reading* through the AHB window
/// (`CFG.DATA4L_RD`, 2 bits): `00` standard, `01` 2-line, `1x` 4-line.
///
/// `Dual` and `Quad` only take effect once the flash's Quad Enable bit is set;
/// [`crate::sfc::Sfc::new_with_config`] does that for you and
/// [`crate::sfc::Sfc::set_quad`] lets you redo it later.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReadWidth {
    /// 1-bit data (standard SPI).
    Single = 0,
    /// 2-bit (2xIO) data.
    Dual = 1,
    /// 4-bit (4xIO) data.
    Quad = 2,
}

/// Data-line width the SFC uses when *programming* a page
/// (`CFG.DATA4L_PP`, 1 bit).
///
/// The SFC has no dual program mode: the field is `0` for standard and `1` for
/// 4-line, which is why this enum has only two variants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProgramWidth {
    /// 1-bit data (standard SPI).
    Single = 0,
    /// 4-bit data. Requires the flash's Quad Enable bit and a matching program
    /// opcode (typically [`CMD_PAGE_PROGRAM_QUAD`]).
    Quad = 1,
}

/// Serial-clock divider relative to HCLK (`CFG.CLKDIV`).
///
/// Per the datasheet: `00` SCLK = HCLK, `01` HCLK/2, `10` HCLK/4, `11` HCLK/8.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClkDiv {
    /// SCLK = HCLK.
    Div1 = 0,
    /// SCLK = HCLK / 2.
    Div2 = 1,
    /// SCLK = HCLK / 4.
    Div4 = 2,
    /// SCLK = HCLK / 8 -- the conservative default.
    Div8 = 3,
}

/// Operational configuration for the SFC.
///
/// [`Config::standard`], [`Config::dual`] and [`Config::quad`] are the three
/// presets exercised by the vendor `SimplSFC` demo; start from one of those and
/// only touch the individual fields if your flash needs different opcodes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Serial-clock divider. Defaults to [`ClkDiv::Div8`].
    pub clk_div: ClkDiv,
    /// Data width used when reading through the AHB window.
    pub read_width: ReadWidth,
    /// Data width used when programming a page.
    pub program_width: ProgramWidth,
    /// Opcode the SFC issues for an AHB read. Must match `read_width`.
    pub read_command: u8,
    /// Opcode the SFC issues for an AHB page program. Must match
    /// `program_width`.
    pub program_command: u8,
    /// `CMDAHB.WREN`: opcode the SFC sends by itself before every command type
    /// the datasheet marks "auto write enable" (5, 6 and 7). The vendor library
    /// leaves this at `0`, which only works if the flash is already unlocked;
    /// `0x06` is the right value for every JEDEC-compatible part.
    pub write_enable_command: u8,
    /// `CMDAHB.RDSRL`: opcode the SFC uses for its own write-in-progress
    /// polling. `0x05` on every JEDEC-compatible part; the vendor library
    /// leaves it at `0`.
    pub read_status_command: u8,
    /// `CFG.ADDR4L`: send the address phase over four lines too. Only meaningful
    /// with [`ProgramWidth::Quad`]; leave it off for `0x32`, which sends the
    /// address on one line.
    pub address_quad: bool,
}

impl Config {
    /// Conservative 1-1-1 preset: standard read/program opcodes, slowest clock.
    pub const fn standard() -> Self {
        Self {
            clk_div: ClkDiv::Div8,
            read_width: ReadWidth::Single,
            program_width: ProgramWidth::Single,
            read_command: CMD_READ,
            program_command: CMD_PAGE_PROGRAM,
            write_enable_command: CMD_WRITE_ENABLE,
            read_status_command: CMD_READ_STATUS_REG1,
            address_quad: false,
        }
    }

    /// Dual preset from the vendor demo: `0xBB` 2xIO reads, standard program,
    /// HCLK/4.
    pub const fn dual() -> Self {
        Self {
            clk_div: ClkDiv::Div4,
            read_width: ReadWidth::Dual,
            program_width: ProgramWidth::Single,
            read_command: CMD_READ_DUAL,
            program_command: CMD_PAGE_PROGRAM,
            write_enable_command: CMD_WRITE_ENABLE,
            read_status_command: CMD_READ_STATUS_REG1,
            address_quad: false,
        }
    }

    /// Quad preset from the vendor demo: `0xEB` 4xIO reads, `0x32` quad-input
    /// page program, HCLK/4. The constructor switches the flash into quad mode
    /// when it sees [`ReadWidth::Quad`] or [`ProgramWidth::Quad`].
    pub const fn quad() -> Self {
        Self {
            clk_div: ClkDiv::Div4,
            read_width: ReadWidth::Quad,
            program_width: ProgramWidth::Quad,
            read_command: CMD_READ_QUAD,
            program_command: CMD_PAGE_PROGRAM_QUAD,
            write_enable_command: CMD_WRITE_ENABLE,
            read_status_command: CMD_READ_STATUS_REG1,
            address_quad: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::standard()
    }
}

/// The three JEDEC ID bytes (`0x9F`).
///
/// `SFC.DATA` holds the three bytes received from the flash with the first one
/// -- the manufacturer -- in bits `[23:16]`, so a Winbond W25Q128 reads back
/// `0xEF_40_18`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct JedecId {
    /// Manufacturer ID (`0xEF` for Winbond).
    pub manufacturer: u8,
    /// Memory type (`0x40` for the W25Q series).
    pub memory_type: u8,
    /// Capacity code, see [`JedecId::capacity_bytes`].
    pub capacity: u8,
}

impl JedecId {
    /// Split the raw `SFC.DATA` value: manufacturer in bits `[23:16]`.
    pub const fn from_raw(raw: u32) -> Self {
        Self {
            manufacturer: (raw >> 16) as u8,
            memory_type: (raw >> 8) as u8,
            capacity: raw as u8,
        }
    }

    /// The raw `SFC.DATA` value this ID was read from.
    pub const fn raw(&self) -> u32 {
        (self.manufacturer as u32) << 16 | (self.memory_type as u32) << 8 | self.capacity as u32
    }

    /// Density in bytes for the usual `2^capacity` JEDEC encoding.
    ///
    /// `0x18` -- the W25Q128's capacity byte -- yields 16 MiB. `None` when the
    /// capacity byte is out of range for a `u64`.
    pub const fn capacity_bytes(&self) -> Option<u64> {
        if self.capacity >= 64 {
            None
        } else {
            Some(1u64 << self.capacity)
        }
    }
}

/// Interrupt flags of the SFC (`IF`, write-one-to-clear).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Flags {
    /// `OVER`: a register-driven command finished and the hardware cleared `GO`.
    pub done: bool,
    /// `ERR`: the flash did *not* report write-in-progress on the hardware's
    /// first poll, i.e. the erase/program command was rejected.
    pub error: bool,
    /// `TO`: the hardware gave up polling write-in-progress after
    /// `TIM.WIP_CHK_LMT` attempts.
    pub timeout: bool,
}

/// Errors returned by the SFC driver.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    /// The flash did not clear its write-in-progress flag within the poll
    /// budget ([`WIP_POLL_LIMIT`] by default).
    Timeout,
    /// The requested range would run past the end of the 256 MiB AHB window.
    OutOfRange,
    /// A page program may not cross a [`PAGE_SIZE`] boundary. Use
    /// [`crate::sfc::Sfc::write`], which splits the transfer for you.
    PageCrossing,
}
