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
        T::power_on();
        T::clock_enable(config.clock_divider);
        T::set_bus_width(config.bus_width);
        T::set_timeout(config.timeout);
        T::clear_interrupts();
        Self {
            peri,
            _mode: core::marker::PhantomData,
        }
    }

    /// Re-apply the SD clock divider. The SD clock runs at
    /// `peripheral_clock / (divider + 1)`.
    pub fn configure_clock(&self, divider: u8) {
        T::clock_enable(divider);
    }

    /// Select the data bus width.
    pub fn set_bus_width(&self, width: BusWidth) {
        T::set_bus_width(width);
    }

    /// Power the card on.
    pub fn power_on(&self) {
        T::power_on();
    }

    /// Power the card off.
    pub fn power_off(&self) {
        T::power_off();
    }

    /// Disable the SD clock (without powering the card off).
    pub fn disable_clock(&self) {
        T::clock_disable();
    }

    /// Clear every SDIO status flag.
    pub fn clear_interrupts(&self) {
        T::clear_interrupts();
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
        T::send_command(index, arg, response, check_crc, check_idx)
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
        T::start_transfer(cmd, arg, ResponseType::Short, true, block_bytes, block_count, true)?;
        T::pio_read(buf)
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
        T::start_transfer(cmd, arg, ResponseType::Short, true, block_bytes, block_count, false)?;
        T::pio_write(buf)
    }
}
