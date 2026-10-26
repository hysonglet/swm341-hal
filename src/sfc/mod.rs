//! Serial Flash Controller (SFC): the SWM341's dedicated NOR-flash SPI master.
//!
//! The SFC drives the chip's fixed Flash-SPI pads (D3..D8); those pads are not
//! wired to SPI0/SPI1, so this driver is the only way to reach the on-board
//! SPI NOR (e.g. W25Q128). The constructor configures the pads, the clock, the
//! AHB read/program opcodes and -- for the quad presets -- the flash's Quad
//! Enable bit, after which you can read the JEDEC ID, peek status registers,
//! read the whole array through the `0x7000_0000` window, and erase/program it.
//!
//! # Reading
//!
//! [`Sfc::read`] goes through the AHB window, so it is a plain memory copy: no
//! opcode, no command. That window only serves word accesses ("读写操作仅支持
//! WORD" in the datasheet), which the driver hides by fetching whole words and
//! copying out the sub-range you asked for.
//!
//! # Writing
//!
//! NOR flash only ever clears bits, so a range must be erased before it is
//! programmed, and one page program may not cross a 256-byte page boundary:
//!
//! * [`Sfc::program`] writes up to one page and rejects a crossing range with
//!   [`Error::PageCrossing`],
//! * [`Sfc::write`] splits an arbitrary range into pages for you.
//!
//! Both leave bytes outside the requested range untouched by padding the words
//! they push with `0xFF`. Neither erases: call [`Sfc::erase_sector`] (or one of
//! its siblings) first.
//!
//! # Pads
//!
//! The controller only reaches the chip's dedicated Flash-SPI pads, D3..D8 of
//! bank D, and nothing else in the pin matrix can drive them. [`Sfc::new`]
//! therefore routes them itself; pass your own [`SfcPins`] to
//! [`Sfc::new_with_pins`] if you would rather hand the tokens over explicitly.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let sfc = hal::sfc::Sfc::new(p.sfc());
//! let id = sfc.read_jedec_id();
//! assert_eq!(id.manufacturer, hal::sfc::MANUFACTURER_WINBOND); // Winbond
//! assert_eq!(id.capacity_bytes(), Some(16 * 1024 * 1024)); // W25Q128
//!
//! // Erase a sector, write a short string into it, and read it back.
//! sfc.erase_sector(0x1_0000).unwrap();
//! sfc.write(0x1_0000, b"Hi from Synwit!").unwrap();
//!
//! let mut buf = [0u8; 15];
//! sfc.read(0x1_0000, &mut buf).unwrap();
//! assert_eq!(&buf, b"Hi from Synwit!");
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{self, Flex};
use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod pins;
pub mod types;
pub use types::*;

/// The six pads of the dedicated Flash-SPI interface.
///
/// The SFC cannot reach any other pad -- D3..D8 are the only ones the pin
/// matrix wires to it -- so this is the whole story: one pad per signal.
/// `d2`/`d3` are the WP/HOLD lines, which quad read and quad program need.
pub struct SfcPins {
    /// `FSPI_DATA2` / `WP`.
    pub d2: gpio::PD3,
    /// `FSPI_DATA3` / `HOLD`.
    pub d3: gpio::PD4,
    /// `FSPI_SCLK`.
    pub sclk: gpio::PD5,
    /// `FSPI_SSEL` (chip select).
    pub ssel: gpio::PD6,
    /// `FSPI_MISO` / `IO1`.
    pub miso: gpio::PD7,
    /// `FSPI_MOSI` / `IO0`.
    pub mosi: gpio::PD8,
}

impl Default for SfcPins {
    /// The only wiring there is, so callers can write `SfcPins::default()`
    /// instead of spelling out every pad.
    ///
    /// Each pad is taken with the generated singleton's `steal()` -- the same
    /// token `p.Pxy` would hand over -- so it must not be used elsewhere at the
    /// same time.
    fn default() -> Self {
        unsafe {
            Self {
                d2: gpio::PD3::steal(),
                d3: gpio::PD4::steal(),
                sclk: gpio::PD5::steal(),
                ssel: gpio::PD6::steal(),
                miso: gpio::PD7::steal(),
                mosi: gpio::PD8::steal(),
            }
        }
    }
}

/// The same pads, muxed onto the SFC and kept alive by the driver.
///
/// Nothing ever reads these fields: holding the [`Flex`] is what keeps the mux
/// setting and the bank D clock in place.
#[allow(dead_code)]
struct SfcPads<'d> {
    d2: Flex<'d, gpio::PD3>,
    d3: Flex<'d, gpio::PD4>,
    sclk: Flex<'d, gpio::PD5>,
    ssel: Flex<'d, gpio::PD6>,
    miso: Flex<'d, gpio::PD7>,
    mosi: Flex<'d, gpio::PD8>,
}

impl<'d> SfcPads<'d> {
    /// Mux every pad of `pins` onto the SFC.
    fn route(pins: SfcPins) -> Self {
        let mut d2 = Flex::new(pins.d2);
        d2.set_func(pins::d::Pin3Func::FspiData2);
        let mut d3 = Flex::new(pins.d3);
        d3.set_func(pins::d::Pin4Func::FspiData3);
        let mut sclk = Flex::new(pins.sclk);
        sclk.set_func(pins::d::Pin5Func::FspiSclk);
        let mut ssel = Flex::new(pins.ssel);
        ssel.set_func(pins::d::Pin6Func::FspiSsel);
        let mut miso = Flex::new(pins.miso);
        miso.set_func(pins::d::Pin7Func::FspiMiso);
        let mut mosi = Flex::new(pins.mosi);
        mosi.set_func(pins::d::Pin8Func::FspiMosi);

        Self {
            d2,
            d3,
            sclk,
            ssel,
            miso,
            mosi,
        }
    }
}

/// Reject a range that would run past the end of the AHB window.
///
/// An empty range is always fine: every caller short-circuits on it before
/// touching the window.
fn check_range(addr: u32, len: usize) -> Result<(), Error> {
    let len = u32::try_from(len).map_err(|_| Error::OutOfRange)?;
    if len == 0 {
        return Ok(());
    }
    match addr.checked_add(len - 1) {
        Some(end) if end < FLASH_WINDOW_SIZE => Ok(()),
        _ => Err(Error::OutOfRange),
    }
}

/// Reject a page program whose range would cross a [`PAGE_SIZE`] boundary.
fn check_page(addr: u32, len: usize) -> Result<(), Error> {
    let len = u32::try_from(len).map_err(|_| Error::OutOfRange)?;
    if len == 0 {
        return Ok(());
    }
    check_range(addr, len as usize)?;

    let end = addr + len - 1;
    if addr & !(PAGE_SIZE - 1) == end & !(PAGE_SIZE - 1) {
        Ok(())
    } else {
        Err(Error::PageCrossing)
    }
}

/// A handle to the Serial Flash Controller, owning the pads it drives.
#[allow(dead_code)]
pub struct Sfc<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    pads: SfcPads<'d>,
    config: Config,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Sfc<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Sfc<'d, T, Blocking> {
    /// Take the SFC, route the Flash-SPI pads, open its clock gate and apply the
    /// default [`Config`] (slowest clock, 1-bit data, `0x03`/`0x02` opcodes).
    ///
    /// Uses [`SfcPins::default()`], i.e. the only wiring the chip offers. Hand
    /// the pads over yourself with [`Sfc::new_with_pins`].
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        Self::new_with_pins(peri, SfcPins::default(), Config::default())
    }

    /// Like [`Sfc::new`] but with a caller-supplied [`Config`].
    ///
    /// [`Config::quad`] (and any other config whose read or program width is
    /// [`ReadWidth::Quad`] / [`ProgramWidth::Quad`]) also switches the flash
    /// into quad mode if it is not there yet.
    pub fn new_with_config(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        Self::new_with_pins(peri, SfcPins::default(), config)
    }

    /// Like [`Sfc::new`] but taking the pads explicitly, so nothing is `steal`ed
    /// behind your back.
    pub fn new_with_pins(peri: impl Peripheral<P = T> + 'd, pins: SfcPins, config: Config) -> Self {
        into_ref!(peri);
        let pads = SfcPads::route(pins);

        T::clock().open();
        T::configure(config);

        Self {
            peri,
            pads,
            config,
            _mode: core::marker::PhantomData,
        }
    }

    /// The configuration this controller was built with.
    pub fn config(&self) -> Config {
        self.config
    }

    /// Three JEDEC ID bytes as a raw `SFC.DATA` value: manufacturer in bits
    /// `[23:16]`, then memory type, then capacity.
    ///
    /// For a Winbond W25Q128 this is `0xEF_40_18`.
    pub fn read_jedec(&self) -> u32 {
        T::read_jedec()
    }

    /// The JEDEC ID split into its three bytes. See [`JedecId`].
    pub fn read_jedec_id(&self) -> JedecId {
        JedecId::from_raw(T::read_jedec())
    }

    /// Read one status-register byte. `cmd` is the opcode (`0x05` for SR1,
    /// `0x35` for SR2, `0x15` for SR3); the busy flag is bit 0 of SR1.
    pub fn read_status(&self, cmd: u8) -> u8 {
        T::read_status(cmd)
    }

    /// Status register 1: bit 0 is the write-in-progress flag.
    pub fn status1(&self) -> u8 {
        T::read_status(CMD_READ_STATUS_REG1)
    }

    /// Status register 2: bit 1 is the Quad Enable flag.
    pub fn status2(&self) -> u8 {
        T::read_status(CMD_READ_STATUS_REG2)
    }

    /// Status register 3.
    pub fn status3(&self) -> u8 {
        T::read_status(CMD_READ_STATUS_REG3)
    }

    /// True while the flash is busy with an erase or program (SR1 bit 0).
    pub fn is_busy(&self) -> bool {
        T::busy()
    }

    /// True while the controller is busy. Unlike [`Sfc::is_busy`] this also
    /// covers the SPI transaction itself (`SR.BUSY`), not just the flash.
    pub fn controller_busy(&self) -> bool {
        T::controller_busy()
    }

    /// Wait until the flash clears its write-in-progress flag, using the
    /// default [`WIP_POLL_LIMIT`] budget.
    pub fn wait_ready(&self) -> Result<(), Error> {
        T::wait_ready()
    }

    /// Like [`Sfc::wait_ready`] with a caller-chosen number of status-register
    /// polls. Needed for a chip erase, which can outlast the default budget.
    pub fn wait_ready_with(&self, limit: u32) -> Result<(), Error> {
        T::wait_wip(limit)
    }

    /// Wait until the controller itself goes idle (`SR.BUSY`).
    pub fn wait_controller_idle(&self) -> Result<(), Error> {
        T::wait_controller(WIP_POLL_LIMIT)
    }

    /// The controller's interrupt flags.
    pub fn flags(&self) -> Flags {
        T::flags()
    }

    /// Clear the controller's interrupt flags.
    pub fn clear_flags(&self) {
        T::clear_flags()
    }

    /// True when the flash's Quad Enable bit is set.
    pub fn quad_enabled(&self) -> bool {
        T::quad_state()
    }

    /// Set (`on = true`) or clear the flash's Quad Enable bit, which the
    /// `Dual`/`Quad` read and program widths depend on.
    ///
    /// Waits for the flash to finish writing its status register.
    pub fn set_quad(&self, on: bool) -> Result<(), Error> {
        T::set_quad(on);
        self.wait_ready()
    }

    /// Write `val` to the flash status register selected by `cmd`
    /// (`0x01` for SR1, which takes both bytes as `SR2:SR1`).
    pub fn write_status(&self, cmd: u8, val: u16) -> Result<(), Error> {
        T::write_status(cmd, val);
        self.wait_ready()
    }

    /// Read `buf.len()` bytes starting at `addr` through the AHB window.
    pub fn read(&self, addr: u32, buf: &mut [u8]) -> Result<(), Error> {
        check_range(addr, buf.len())?;
        if buf.is_empty() {
            return Ok(());
        }

        T::read_memory(addr, buf);
        Ok(())
    }

    /// Program `data` at `addr`, staying inside one 256-byte page.
    ///
    /// Returns [`Error::PageCrossing`] if `data` would cross a page boundary --
    /// use [`Sfc::write`], which splits the transfer. The target range must
    /// have been erased first.
    pub fn program(&self, addr: u32, data: &[u8]) -> Result<(), Error> {
        check_page(addr, data.len())?;
        if data.is_empty() {
            return Ok(());
        }

        T::program_memory(addr, data);
        self.wait_ready()
    }

    /// Program `data` at `addr`, splitting it into as many page programs as
    /// needed. The target range must have been erased first.
    pub fn write(&self, addr: u32, data: &[u8]) -> Result<(), Error> {
        let mut addr = addr;
        let mut rest = data;

        while !rest.is_empty() {
            let page_offset = addr & (PAGE_SIZE - 1);
            let n = core::cmp::min((PAGE_SIZE - page_offset) as usize, rest.len());
            self.program(addr, &rest[..n])?;

            addr += n as u32;
            rest = &rest[n..];
        }

        Ok(())
    }

    /// Erase the 4 KiB sector containing `addr`, and wait for it to finish.
    pub fn erase_sector(&self, addr: u32) -> Result<(), Error> {
        self.erase_with(addr, CMD_ERASE_SECTOR, true)
    }

    /// Erase the 32 KiB block containing `addr`, and wait for it to finish.
    pub fn erase_block32k(&self, addr: u32) -> Result<(), Error> {
        self.erase_with(addr, CMD_ERASE_BLOCK32K, true)
    }

    /// Erase the 64 KiB block containing `addr`, and wait for it to finish.
    pub fn erase_block64k(&self, addr: u32) -> Result<(), Error> {
        self.erase_with(addr, CMD_ERASE_BLOCK64K, true)
    }

    /// Erase the entire chip, and wait for it to finish.
    ///
    /// A chip erase can take tens of seconds, well past the default
    /// [`WIP_POLL_LIMIT`] budget, so prefer
    /// `erase_with(u32::MAX, CMD_ERASE_CHIP, false)` followed by
    /// [`Sfc::wait_ready_with`] with a larger budget if you see
    /// [`Error::Timeout`].
    pub fn erase_chip(&self) -> Result<(), Error> {
        self.erase_with(u32::MAX, CMD_ERASE_CHIP, true)
    }

    /// Erase with an explicit opcode, mirroring the vendor `SFC_EraseEx()`.
    ///
    /// `addr` is the sector/block address (any address inside it -- the flash
    /// masks off the low bits itself), or `u32::MAX` to select the chip-erase
    /// command type. With `wait = false` this returns as soon as the command
    /// has been issued; poll [`Sfc::is_busy`] or call [`Sfc::wait_ready`].
    pub fn erase_with(&self, addr: u32, cmd: u8, wait: bool) -> Result<(), Error> {
        T::erase(addr, cmd);
        if wait {
            self.wait_ready()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the argument checks and the pure helpers.
    //!
    //! Nothing here may touch a register: `.cargo/config.toml` pins the default
    //! target to the microcontroller and a `no_std` target has no `test` crate,
    //! so these run on the host:
    //!
    //! ```text
    //! cargo test --target aarch64-apple-darwin --lib
    //! ```

    use super::*;

    #[test]
    fn range_checks_reject_the_end_of_the_window() {
        assert_eq!(check_range(0, 4), Ok(()));
        assert_eq!(check_range(FLASH_WINDOW_SIZE - 4, 4), Ok(()));

        // One byte past the end, and straddling the end.
        assert_eq!(check_range(FLASH_WINDOW_SIZE, 1), Err(Error::OutOfRange));
        assert_eq!(
            check_range(FLASH_WINDOW_SIZE - 1, 4),
            Err(Error::OutOfRange)
        );
        assert_eq!(check_range(u32::MAX, 4), Err(Error::OutOfRange));

        // An empty transfer short-circuits before the arithmetic.
        assert_eq!(check_range(u32::MAX, 0), Ok(()));
    }

    #[test]
    fn page_checks_reject_a_crossing_program() {
        assert_eq!(check_page(0, PAGE_SIZE as usize), Ok(()));
        assert_eq!(check_page(1, (PAGE_SIZE - 1) as usize), Ok(()));

        // 4 bytes straddling the 256-byte page boundary.
        assert_eq!(check_page(254, 4), Err(Error::PageCrossing));
        assert_eq!(check_page(255, 2), Err(Error::PageCrossing));

        // Range wins over page alignment.
        assert_eq!(check_page(FLASH_WINDOW_SIZE, 4), Err(Error::OutOfRange));

        assert_eq!(check_page(u32::MAX, 0), Ok(()));
    }

    #[test]
    fn page_size_divides_the_erase_granularities() {
        // Every erase unit is a whole number of pages, so erasing never leaves
        // a partially writable page behind.
        assert_eq!(SECTOR_SIZE % PAGE_SIZE, 0);
        assert_eq!(BLOCK32K_SIZE % PAGE_SIZE, 0);
        assert_eq!(BLOCK64K_SIZE % PAGE_SIZE, 0);
        assert_eq!(FLASH_WINDOW_SIZE % SECTOR_SIZE, 0);
    }

    #[test]
    fn jedec_id_splits_the_raw_register() {
        // Winbond W25Q128, as printed by the vendor `SimplSFC` demo.
        let id = JedecId::from_raw(0x00EF_4018);
        assert_eq!(id.manufacturer, MANUFACTURER_WINBOND);
        assert_eq!(id.memory_type, 0x40);
        assert_eq!(id.capacity, 0x18);
        assert_eq!(id.capacity_bytes(), Some(16 * 1024 * 1024));
        assert_eq!(id.raw(), 0x00EF_4018);

        // Manufacturer first, i.e. in the high byte of the three received.
        assert_eq!(JedecId::from_raw(0x00BA_2018).manufacturer, 0xBA);

        // Out-of-range capacity bytes report `None` rather than overflowing.
        assert_eq!(JedecId::from_raw(0x00EF_40FF).capacity_bytes(), None);
    }

    #[test]
    fn presets_match_the_vendor_demo() {
        let standard = Config::standard();
        assert_eq!(standard.read_width, ReadWidth::Single);
        assert_eq!(standard.program_width, ProgramWidth::Single);
        assert_eq!(standard.read_command, CMD_READ);
        assert_eq!(standard.program_command, CMD_PAGE_PROGRAM);

        let dual = Config::dual();
        assert_eq!(dual.read_width, ReadWidth::Dual);
        assert_eq!(dual.read_command, CMD_READ_DUAL);
        assert_eq!(dual.program_width, ProgramWidth::Single);

        let quad = Config::quad();
        assert_eq!(quad.read_width, ReadWidth::Quad);
        assert_eq!(quad.program_width, ProgramWidth::Quad);
        assert_eq!(quad.read_command, CMD_READ_QUAD);
        assert_eq!(quad.program_command, CMD_PAGE_PROGRAM_QUAD);
        // `0x32` sends its address on one line, so ADDR4L must stay off.
        assert!(!quad.address_quad);

        // The auto write-enable and WIP-poll opcodes the vendor leaves at 0.
        for config in [standard, dual, quad] {
            assert_eq!(config.write_enable_command, CMD_WRITE_ENABLE);
            assert_eq!(config.read_status_command, CMD_READ_STATUS_REG1);
        }
    }

    #[test]
    fn generated_pad_codes_match_the_vendor_header() {
        use crate::gpio::PinFunc;

        // `SWM341_port.h`: PORTD_PIN3_FSPI_DATA2 = 2, ..._PIN5_FSPI_SCLK = 1, ...
        assert_eq!(pins::d::Pin3Func::FspiData2.value(), 2);
        assert_eq!(pins::d::Pin4Func::FspiData3.value(), 2);
        assert_eq!(pins::d::Pin5Func::FspiSclk.value(), 1);
        assert_eq!(pins::d::Pin6Func::FspiSsel.value(), 1);
        assert_eq!(pins::d::Pin7Func::FspiMiso.value(), 3);
        assert_eq!(pins::d::Pin8Func::FspiMosi.value(), 2);
    }

    #[test]
    fn widths_encode_the_datasheet_bit_patterns() {
        // CFG.DATA4L_RD: 00 standard, 01 2-line, 1x 4-line.
        assert_eq!(ReadWidth::Single as u8, 0);
        assert_eq!(ReadWidth::Dual as u8, 1);
        assert_eq!(ReadWidth::Quad as u8, 2);

        // CFG.DATA4L_PP is a single bit: there is no dual program mode.
        assert_eq!(ProgramWidth::Single as u8, 0);
        assert_eq!(ProgramWidth::Quad as u8, 1);

        // CFG.CLKDIV: 00 = HCLK, 01 = /2, 10 = /4, 11 = /8.
        assert_eq!(ClkDiv::Div1 as u8, 0);
        assert_eq!(ClkDiv::Div8 as u8, 3);
    }
}
