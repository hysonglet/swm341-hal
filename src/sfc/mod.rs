//! Serial Flash Controller (SFC): the SWM341's dedicated NOR-flash SPI master.
//!
//! The SFC drives the chip's fixed Flash-SPI pads (D3..D8); those pads are not
//! wired to SPI0/SPI1, so this driver is the only way to reach the on-board
//! SPI NOR (e.g. W25Q128). The constructor configures the pads, the clock, and
//! the AHB read/program opcodes in one shot, after which you can read the
//! JEDEC ID, peek status registers, read the whole array through the
//! `0x7000_0000` window, and erase/program it.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let sfc = hal::sfc::Sfc::new(p.sfc());
//! let raw = sfc.read_jedec();
//! let mfr = (raw >> 16) & 0xFF;
//! assert_eq!(mfr, 0xEF); // Winbond
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the Serial Flash Controller.
#[allow(dead_code)]
pub struct Sfc<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Sfc<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Sfc<'d, T, Blocking> {
    /// Take the SFC, open its clock gate, mux the Flash-SPI pads, and apply the
    /// default [`Config`] (slowest clock, 1-bit data, `0x03`/`0x02` opcodes).
    pub fn new(peri: impl Peripheral<P = T> + 'd) -> Self {
        Self::new_with_config(peri, Config::default())
    }

    /// Like [`Sfc::new`] but with a caller-supplied [`Config`].
    pub fn new_with_config(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);

        T::clock().open();
        T::configure_pins();
        T::configure(
            config.clk_div as u8,
            config.read_width as u8,
            config.program_width as u8,
            config.read_command,
            config.program_command,
        );

        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Three JEDEC ID bytes: manufacturer, memory type, capacity.
    ///
    /// For a Winbond W25Q128 this is `0xEF 0x40 0x18`.
    pub fn read_jedec(&self) -> u32 {
        T::read_jedec()
    }

    /// Read one status-register byte. `cmd` is the opcode (`0x05` for SR1,
    /// `0x35` for SR2); the busy flag is bit 0 of SR1.
    pub fn read_status(&self, cmd: u8) -> u8 {
        T::read_status(cmd)
    }

    /// True while the flash is busy with an erase or program.
    pub fn is_busy(&self) -> bool {
        T::busy()
    }

    /// Read `buf.len()` bytes starting at `addr` through the XIP/AHB window.
    pub fn read(&self, addr: u32, buf: &mut [u8]) {
        T::read_memory(addr, buf);
    }

    /// Program `data` at `addr`.
    ///
    /// # Panics
    ///
    /// The SFC page program can only touch bytes within one 256-byte page; do
    /// not cross a page boundary. Erase the target range first.
    pub fn program(&self, addr: u32, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let page = addr & !0xFF;
        assert_eq!(
            (addr + data.len() as u32 - 1) & !0xFF,
            page,
            "SFC program must not cross a 256-byte page boundary"
        );
        T::program_memory(addr, data);
    }

    /// Erase the 4 KiB sector containing `addr`.
    pub fn erase_sector(&self, addr: u32) {
        T::erase(addr, instance::CMD_ERASE_SECTOR);
    }

    /// Erase the 64 KiB block containing `addr`.
    pub fn erase_block64k(&self, addr: u32) {
        T::erase(addr, instance::CMD_ERASE_BLOCK64K);
    }

    /// Erase the entire chip.
    pub fn erase_chip(&self) {
        T::erase(0xFFFF_FFFF, instance::CMD_ERASE_CHIP);
    }

    /// Enable (`on = true`) or disable the flash's Quad Enable bit, so the
    /// `Dual`/`Quad` widths configured at construction become effective.
    pub fn set_quad(&self, on: bool) {
        T::set_quad(on);
    }
}
