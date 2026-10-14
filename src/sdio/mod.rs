//! SDIO / SD-memory host controller.
//!
//! The driver exposes the host-side command and data engine: power the card on,
//! divide the SD clock, pick the bus width, send commands, and move blocks in and
//! out of the data FIFO over PIO. It deliberately stops short of the full SD
//! card enumeration state machine (identification, switch, ...), which is built
//! on top of [`Sdio::send_command`] and [`Sdio::read_blocks`] /
//! [`Sdio::write_blocks`].
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let sd = hal::sdio::Sdio::new(p.SDIO, hal::sdio::Config::default());
//! // Send CMD0 (GO_IDLE_STATE) and ignore its (no-response) result.
//! let _ = sd.send_command(0, 0, hal::sdio::ResponseType::None, false, false);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the SDIO host controller.
#[allow(dead_code)]
pub struct Sdio<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Sdio<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Sdio<'d, T, Blocking> {
    /// Take ownership of the SDIO host, power the card on, enable the SD clock
    /// and apply `config`.
    pub fn new(peri: impl Peripheral<P = T> + 'd, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        let regs = unsafe { &*T::regs() };
        instance::hw::power_on(regs);
        instance::hw::clock_enable(regs, config.clock_divider);
        instance::hw::set_bus_width(regs, config.bus_width);
        instance::hw::set_timeout(regs, config.timeout);
        instance::hw::clear_interrupts(regs);
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Re-apply the SD clock divider. The SD clock runs at
    /// `peripheral_clock / (divider + 1)`.
    pub fn configure_clock(&self, divider: u8) {
        let regs = unsafe { &*T::regs() };
        instance::hw::clock_enable(regs, divider);
    }

    /// Select the data bus width.
    pub fn set_bus_width(&self, width: BusWidth) {
        instance::hw::set_bus_width(unsafe { &*T::regs() }, width);
    }

    /// Power the card on.
    pub fn power_on(&self) {
        instance::hw::power_on(unsafe { &*T::regs() });
    }

    /// Power the card off.
    pub fn power_off(&self) {
        instance::hw::power_off(unsafe { &*T::regs() });
    }

    /// Disable the SD clock (without powering the card off).
    pub fn disable_clock(&self) {
        instance::hw::clock_disable(unsafe { &*T::regs() });
    }

    /// Clear every SDIO status flag.
    pub fn clear_interrupts(&self) {
        instance::hw::clear_interrupts(unsafe { &*T::regs() });
    }

    /// Launch a command that carries no data phase and return its response.
    pub fn send_command(
        &self,
        index: u8,
        arg: u32,
        response: ResponseType,
        check_crc: bool,
        check_idx: bool,
    ) -> Result<[u32; 4], SdioError> {
        instance::hw::send_command(unsafe { &*T::regs() }, index, arg, response, check_crc, check_idx)
    }

    /// Read `block_count` blocks of `block_bytes` bytes each, launched by `cmd`
    /// with argument `arg`, into `buf` (one `u32` per four card bytes).
    pub fn read_blocks(
        &self,
        cmd: u8,
        arg: u32,
        buf: &mut [u32],
        block_bytes: u16,
        block_count: u16,
    ) -> Result<(), SdioError> {
        let regs = unsafe { &*T::regs() };
        instance::hw::start_transfer(regs, cmd, arg, ResponseType::Short, true, block_bytes, block_count, true)?;
        instance::hw::pio_read(regs, buf)
    }

    /// Write `block_count` blocks of `block_bytes` bytes each, launched by `cmd`
    /// with argument `arg`, from `buf` (one `u32` per four card bytes).
    pub fn write_blocks(
        &self,
        cmd: u8,
        arg: u32,
        buf: &[u32],
        block_bytes: u16,
        block_count: u16,
    ) -> Result<(), SdioError> {
        let regs = unsafe { &*T::regs() };
        instance::hw::start_transfer(regs, cmd, arg, ResponseType::Short, true, block_bytes, block_count, false)?;
        instance::hw::pio_write(regs, buf)
    }
}
