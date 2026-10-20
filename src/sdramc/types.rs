//! SDRAM controller configuration and status types.

/// Base address of the memory-mapped SDRAM window (`SDRAMM_BASE`).
///
/// Once [`crate::sdramc::Sdramc::new`] has initialised the chip, the whole
/// capacity is readable and writable through this window.
pub const SDRAM_BASE: usize = 0x8000_0000;

/// Capacity of the connected SDRAM chip (`CFG.SIZE`).
///
/// The code also decides how the controller splits the AHB address into row,
/// bank and column, so it has to match the chip on the board.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Size {
    /// 2 MB — `HADDR[20:10]` row, `HADDR[9]` bank, `HADDR[8:1]` column.
    Mb2 = 3,
    /// 8 MB — `HADDR[22:11]` row, `HADDR[10:9]` bank, `HADDR[8:1]` column.
    Mb8 = 0,
    /// 16 MB — `HADDR[23:12]` row, `HADDR[11:10]` bank, `HADDR[9:1]` column.
    Mb16 = 1,
    /// 32 MB — `HADDR[24:12]` row, `HADDR[11:10]` bank, `HADDR[9:1]` column.
    Mb32 = 2,
}

impl Size {
    /// Capacity of the chip in bytes.
    pub const fn bytes(self) -> usize {
        match self {
            Size::Mb2 => 2 * 1024 * 1024,
            Size::Mb8 => 8 * 1024 * 1024,
            Size::Mb16 => 16 * 1024 * 1024,
            Size::Mb32 => 32 * 1024 * 1024,
        }
    }

    /// Number of rows the chip has.
    ///
    /// The controller refreshes one row per refresh cycle, so this is what
    /// turns the refresh window into the `T64` period.
    pub const fn rows(self) -> u32 {
        match self {
            Size::Mb2 => 2048,
            Size::Mb8 => 4096,
            Size::Mb16 => 4096,
            Size::Mb32 => 8192,
        }
    }
}

/// Divider between the core clock and the SDRAM clock (`CFG.CLKDIV`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ClkDiv {
    /// SDRAM runs at `HCLK / 1`; supported core frequency 20 MHz .. 140 MHz.
    Div1 = 0,
    /// SDRAM runs at `HCLK / 2`; supported core frequency 40 MHz .. 160 MHz.
    Div2 = 1,
}

/// CAS latency in SDRAM clock cycles (`CFG.CASDELAY`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum CasLatency {
    /// Two clock cycles.
    Latency2 = 0,
    /// Three clock cycles.
    Latency3 = 1,
}

/// Configuration handed to [`crate::sdramc::Sdramc::new`].
///
/// The timing fields are the chip's datasheet delays **minus one**, expressed
/// in SDRAM clock cycles: the vendor header spells them `SDRAM_TRP_2` (= 1),
/// `SDRAM_TRC_7` (= 6) and so on. When the chip specifies both `tRC` and
/// `tRFC`/`tRRC`, take the larger of the two for [`Config::trc`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Capacity of the chip, which also fixes the address split.
    pub size: Size,
    /// Divider from the core clock to the SDRAM clock.
    pub clk_div: ClkDiv,
    /// CAS latency of the read burst.
    pub cas_latency: CasLatency,
    /// Refresh window in milliseconds: the time in which the chip has to
    /// complete a full refresh of every row. Normally 64 ms.
    pub refresh_time_ms: u32,
    /// Row precharge delay minus one (`TIM.TRP`, 2 bits): valid range `0..=3`.
    pub trp: u8,
    /// Row-to-column delay minus one (`TIM.TRCD`, 2 bits): valid range `0..=3`.
    pub trcd: u8,
    /// Row cycle time (activate to activate on the same bank) minus one
    /// (`TIM.TRC`, 4 bits): valid range `0..=15`.
    pub trc: u8,
}

impl Default for Config {
    /// The vendor `SimplSDRAM` example: 8 MB, no clock divider, CAS 2, 64 ms
    /// refresh, `tRP` = 2, `tRCD` = 2 and `tRC` = 7 cycles.
    fn default() -> Self {
        Self {
            size: Size::Mb8,
            clk_div: ClkDiv::Div1,
            cas_latency: CasLatency::Latency2,
            refresh_time_ms: 64,
            trp: 1,
            trcd: 1,
            trc: 6,
        }
    }
}

impl Config {
    /// Override the chip capacity.
    pub fn size(self, size: Size) -> Self {
        Self { size, ..self }
    }

    /// Override the SDRAM clock divider.
    pub fn clk_div(self, clk_div: ClkDiv) -> Self {
        Self { clk_div, ..self }
    }

    /// Override the CAS latency.
    pub fn cas_latency(self, cas_latency: CasLatency) -> Self {
        Self { cas_latency, ..self }
    }

    /// Override the refresh window in milliseconds.
    pub fn refresh_time_ms(self, refresh_time_ms: u32) -> Self {
        Self {
            refresh_time_ms,
            ..self
        }
    }

    /// Override the row precharge delay (cycles minus one, `0..=3`).
    pub fn trp(self, trp: u8) -> Self {
        Self { trp, ..self }
    }

    /// Override the row-to-column delay (cycles minus one, `0..=3`).
    pub fn trcd(self, trcd: u8) -> Self {
        Self { trcd, ..self }
    }

    /// Override the row cycle time (cycles minus one, `0..=15`).
    pub fn trc(self, trc: u8) -> Self {
        Self { trc, ..self }
    }
}
