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
//! // 4-bit bus: CLK/CMD/D0..D3 are muxed by the constructor, which also sets
//! // `BusWidth::Bus4` to match the pads passed in.
//! let sd = hal::sdio::Sdio::new_4bit(
//!     p.sdio(),
//!     p.PM2, // CLK
//!     p.PM4, // CMD
//!     p.PM5, // D0
//!     p.PM6, // D1
//!     p.PN0, // D2
//!     p.PN1, // D3
//!     hal::sdio::Config::default(),
//! );
//! // Send CMD0 (GO_IDLE_STATE) and ignore its (no-response) result.
//! let _ = sd.send_command(0, 0, hal::sdio::ResponseType::None, false, false);
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::Flex;
use crate::mode::{Blocking, Mode};

use instance::Instance;

mod instance;
pub mod pins;
pub mod types;
pub use types::*;

/// `PORTx.FUNC` codes for the SDIO pads.
///
/// Read straight out of the generated [`pins`] table rather than transcribed, so
/// a typo cannot silently mis-route a pad. Every SDIO signal has exactly one pad
/// on this chip, hence one fixed code per signal.
mod func {
    use super::pins;
    pub const CLK: u8 = pins::m::Pin2Func::SdClk as u8;
    pub const CMD: u8 = pins::m::Pin4Func::SdCmd as u8;
    pub const D0: u8 = pins::m::Pin5Func::SdD0 as u8;
    pub const D1: u8 = pins::m::Pin6Func::SdD1 as u8;
    pub const D2: u8 = pins::n::Pin0Func::SdD2 as u8;
    pub const D3: u8 = pins::n::Pin1Func::SdD3 as u8;
    pub const D4: u8 = pins::n::Pin4Func::SdD4 as u8;
    pub const D5: u8 = pins::n::Pin5Func::SdD5 as u8;
    pub const D6: u8 = pins::b::Pin0Func::SdD6 as u8;
    pub const D7: u8 = pins::b::Pin1Func::SdD7 as u8;
}

/// The SDIO pads routed for an instance, held so their mux stays configured.
///
/// Each signal has exactly one pad on this chip, so the concrete pad types can be
/// stored directly instead of turning `Sdio` into a pile of generic parameters.
/// `D1..D7` are only routed for the wider bus modes, hence the `Option`s.
#[allow(dead_code)]
struct SdioPads<'d> {
    clk: Flex<'d, crate::gpio::PM2>,
    cmd: Flex<'d, crate::gpio::PM4>,
    d0: Flex<'d, crate::gpio::PM5>,
    d1: Option<Flex<'d, crate::gpio::PM6>>,
    d2: Option<Flex<'d, crate::gpio::PN0>>,
    d3: Option<Flex<'d, crate::gpio::PN1>>,
    d4: Option<Flex<'d, crate::gpio::PN4>>,
    d5: Option<Flex<'d, crate::gpio::PN5>>,
    d6: Option<Flex<'d, crate::gpio::PB0>>,
    d7: Option<Flex<'d, crate::gpio::PB1>>,
}

/// Mux `pin` to `func` and return the handle that keeps it routed.
fn route_pad<'d, P: crate::gpio::Pin>(
    pin: impl Peripheral<P = P> + 'd,
    func: u8,
) -> Flex<'d, P> {
    let flex = Flex::new(pin);
    crate::gpio::set_function::<P>(func);
    flex
}

/// A handle to the SDIO host controller, owning the pads it drives.
#[allow(dead_code)]
pub struct Sdio<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    pads: SdioPads<'d>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Drop for Sdio<'d, T, M> {
    fn drop(&mut self) {
        T::clock().close();
    }
}

impl<'d, T: Instance> Sdio<'d, T, Blocking> {
    /// Take ownership of the SDIO host and its pads for a **1-bit** bus.
    ///
    /// Muxes `CLK` (PM2), `CMD` (PM4) and `D0` (PM5), then powers the card on,
    /// enables the SD clock and applies `config`.
    ///
    /// Each constructor routes exactly the pads its bus width needs, so the
    /// width cannot disagree with the hardware: `Config::bus_width` is
    /// overridden to [`BusWidth::Bus1`] here.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        clk: impl Peripheral<P = crate::gpio::PM2> + 'd,
        cmd: impl Peripheral<P = crate::gpio::PM4> + 'd,
        d0: impl Peripheral<P = crate::gpio::PM5> + 'd,
        config: Config,
    ) -> Self {
        Self::build(
            peri,
            route_pad(clk, func::CLK),
            route_pad(cmd, func::CMD),
            route_pad(d0, func::D0),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            config.bus_width(BusWidth::Bus1),
        )
    }

    /// Same as [`new`](Self::new) but for a **4-bit** bus: also muxes `D1..D3`
    /// (PM6/PN0/PN1) and sets [`BusWidth::Bus4`].
    // One argument per pad: unavoidable for a parallel bus.
    #[allow(clippy::too_many_arguments)]
    pub fn new_4bit(
        peri: impl Peripheral<P = T> + 'd,
        clk: impl Peripheral<P = crate::gpio::PM2> + 'd,
        cmd: impl Peripheral<P = crate::gpio::PM4> + 'd,
        d0: impl Peripheral<P = crate::gpio::PM5> + 'd,
        d1: impl Peripheral<P = crate::gpio::PM6> + 'd,
        d2: impl Peripheral<P = crate::gpio::PN0> + 'd,
        d3: impl Peripheral<P = crate::gpio::PN1> + 'd,
        config: Config,
    ) -> Self {
        Self::build(
            peri,
            route_pad(clk, func::CLK),
            route_pad(cmd, func::CMD),
            route_pad(d0, func::D0),
            Some(route_pad(d1, func::D1)),
            Some(route_pad(d2, func::D2)),
            Some(route_pad(d3, func::D3)),
            None,
            None,
            None,
            None,
            config.bus_width(BusWidth::Bus4),
        )
    }

    /// Same as [`new`](Self::new) but for an **8-bit** bus: muxes `D0..D7` and
    /// sets [`BusWidth::Bus8`].
    // One argument per pad: unavoidable for a parallel bus.
    #[allow(clippy::too_many_arguments)]
    pub fn new_8bit(
        peri: impl Peripheral<P = T> + 'd,
        clk: impl Peripheral<P = crate::gpio::PM2> + 'd,
        cmd: impl Peripheral<P = crate::gpio::PM4> + 'd,
        d0: impl Peripheral<P = crate::gpio::PM5> + 'd,
        d1: impl Peripheral<P = crate::gpio::PM6> + 'd,
        d2: impl Peripheral<P = crate::gpio::PN0> + 'd,
        d3: impl Peripheral<P = crate::gpio::PN1> + 'd,
        d4: impl Peripheral<P = crate::gpio::PN4> + 'd,
        d5: impl Peripheral<P = crate::gpio::PN5> + 'd,
        d6: impl Peripheral<P = crate::gpio::PB0> + 'd,
        d7: impl Peripheral<P = crate::gpio::PB1> + 'd,
        config: Config,
    ) -> Self {
        Self::build(
            peri,
            route_pad(clk, func::CLK),
            route_pad(cmd, func::CMD),
            route_pad(d0, func::D0),
            Some(route_pad(d1, func::D1)),
            Some(route_pad(d2, func::D2)),
            Some(route_pad(d3, func::D3)),
            Some(route_pad(d4, func::D4)),
            Some(route_pad(d5, func::D5)),
            Some(route_pad(d6, func::D6)),
            Some(route_pad(d7, func::D7)),
            config.bus_width(BusWidth::Bus8),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        peri: impl Peripheral<P = T> + 'd,
        clk: Flex<'d, crate::gpio::PM2>,
        cmd: Flex<'d, crate::gpio::PM4>,
        d0: Flex<'d, crate::gpio::PM5>,
        d1: Option<Flex<'d, crate::gpio::PM6>>,
        d2: Option<Flex<'d, crate::gpio::PN0>>,
        d3: Option<Flex<'d, crate::gpio::PN1>>,
        d4: Option<Flex<'d, crate::gpio::PN4>>,
        d5: Option<Flex<'d, crate::gpio::PN5>>,
        d6: Option<Flex<'d, crate::gpio::PB0>>,
        d7: Option<Flex<'d, crate::gpio::PB1>>,
        config: Config,
    ) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::power_on();
        T::clock_enable(config.clock_divider);
        T::set_bus_width(config.bus_width);
        T::set_timeout(config.timeout);
        T::clear_interrupts();
        Self {
            peri,
            pads: SdioPads {
                clk,
                cmd,
                d0,
                d1,
                d2,
                d3,
                d4,
                d5,
                d6,
                d7,
            },
            _mode: core::marker::PhantomData,
        }
    }

    /// Re-apply the SD clock divider. The SD clock runs at
    /// `peripheral_clock / (divider + 1)`.
    pub fn configure_clock(&self, divider: u8) {
        T::clock_enable(divider);
    }

    /// Select the data bus width, but only if this instance owns the pads for it.
    ///
    /// Returns `false` and leaves the hardware unchanged when `width` needs pads
    /// that were not routed -- e.g. asking for [`BusWidth::Bus4`] on an instance
    /// built with [`new`](Self::new). The pads are chosen at construction time, so
    /// this can only ever narrow the bus, never widen it.
    pub fn set_bus_width(&self, width: BusWidth) -> bool {
        let routed = match width {
            BusWidth::Bus1 => true,
            BusWidth::Bus4 => {
                self.pads.d1.is_some() && self.pads.d2.is_some() && self.pads.d3.is_some()
            }
            // `D7` is only routed by `new_8bit`, so its presence alone proves the
            // whole D0..D7 set is there.
            BusWidth::Bus8 => self.pads.d7.is_some(),
        };
        if routed {
            T::set_bus_width(width);
        }
        routed
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
