//! Digital-to-analog converter (DAC).
//!
//! The SWM341 has a single 12-bit DAC whose output pad is `PD2`. The pad is
//! routed to the converter through `PORTx.FUNC` code `15` (see
//! [`crate::dac::pins`] / [`Dac::add_pin`]); without that routing the converted
//! voltage never leaves the chip. Writing a value to the data-hold register
//! (`DHR`) and letting the converter load it into the output register (`DOR`)
//! produces the analog level -- [`Dac::write`] does both and waits for the load
//! to finish (`SR.DHRFULL` to clear), mirroring the vendor `SimplDAC` example.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut dac = hal::dac::Dac::new(p.dac(), hal::dac::Config::default());
//! dac.add_pin(p.PD2);                  // route PD2 to DAC_OUT (func 15)
//! dac.write(2048);                     // ~1.65 V with the 3.3 V full scale
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::pac;

use instance::sealed::Instance as _;

pub mod pins;
mod instance;
mod types;
pub use types::*;

/// A GPIO pad that can carry the DAC output.
///
/// Implemented for `PD2`, the only pad exposing the `DAC_OUT` function (code
/// `15`). Routing it (via [`Dac::add_pin`]) is what makes the converted voltage
/// appear off-chip.
pub trait DacPin: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting the DAC output path (`15`).
    const FUNC: u8;
}

impl DacPin for crate::gpio::PD2 {
    const FUNC: u8 = 15;
}

/// A DAC driven by its peripheral clock.
pub struct Dac<'d> {
    #[allow(dead_code)]
    peri: PeripheralRef<'d, crate::Dac>,
    format: DataFormat,
}

impl<'d> Dac<'d> {
    /// Build a DAC from its peripheral and configuration.
    ///
    /// Enables the peripheral clock, applies the factory reference-voltage trim
    /// (`SYS.DACCR.VRADJ` from `SYS.BACKUP[2]`, exactly like the vendor driver),
    /// programs the data format, and powers on the output driver. The output pad
    /// still has to be routed with [`add_pin`](Self::add_pin).
    pub fn new(peri: impl Peripheral<P = crate::Dac> + 'd, config: Config) -> Self {
        into_ref!(peri);

        crate::Dac::clock().open();

        // Reference-voltage trim, copied verbatim from the vendor `DAC_Init`.
        let sys = unsafe { &*pac::SYS::ptr() };
        let vradj = (sys.backup[2].read().bits() & 0x1F) as u8;
        // `vradj` is a 5-bit field; `& 0x1F` above guarantees the value is in
        // range, so the unsafe `bits` writer is sound here.
        sys.daccr.modify(|_, w| unsafe { w.vradj().bits(vradj) });

        crate::Dac::set_format(config.format.bits());
        crate::Dac::enable_output();

        Self {
            peri,
            format: config.format,
        }
    }

    /// Route `pin` to the DAC output and keep it routed while this handle lives.
    ///
    /// `PD2` is the only pad with a `DAC_OUT` function, so this both turns the
    /// port clock on and writes `PORTx.FUNC = 15`.
    pub fn add_pin<P: DacPin>(&mut self, pin: impl Peripheral<P = P> + 'd) {
        let _flex = crate::gpio::Flex::new(pin);
        crate::gpio::set_function::<P>(P::FUNC);
    }

    /// Write `value` to the converter and wait for it to load.
    ///
    /// `value` is interpreted according to [`Config::format`]: [`DataFormat::Lsb12`]
    /// packs it into `DHR[11:0]`, [`DataFormat::Msb12`] into `DHR[15:4]`, and
    /// [`DataFormat::Bit8`] into `DHR[7:0]`. After writing we spin until
    /// `SR.DHRFULL` clears so a following write cannot clobber a pending value.
    pub fn write(&self, value: u16) {
        let raw: u32 = match self.format {
            DataFormat::Lsb12 => (value & 0x0FFF) as u32,
            DataFormat::Msb12 => ((value & 0x0FFF) << 4) as u32,
            DataFormat::Bit8 => (value & 0x00FF) as u32,
        };
        crate::Dac::write_data(raw);
        while crate::Dac::data_full() {}
    }

    /// Pulse the software trigger to load `DHR` into `DOR`.
    ///
    /// The converter auto-loads in its default mode, so this is only needed when
    /// an external / timer trigger source has been selected.
    pub fn trigger(&self) {
        crate::Dac::software_trigger();
    }

    /// Gate the data-hold register onto a DMA request.
    pub fn set_dma(&self, on: bool) {
        crate::Dac::set_dma(on);
    }

    /// Power on the output driver.
    pub fn enable(&self) {
        crate::Dac::enable_output();
    }

    /// Power down the output driver (the pad floats / holds its last level).
    pub fn disable(&self) {
        crate::Dac::disable_output();
    }
}

impl<'d> Drop for Dac<'d> {
    fn drop(&mut self) {
        crate::Dac::clock().close();
    }
}
