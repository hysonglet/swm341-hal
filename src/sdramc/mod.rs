//! SDRAM controller.
//!
//! The SWM341 maps an external SDRAM chip into the address space at
//! [`SDRAM_BASE`] (0x8000_0000). [`Sdramc::new`] opens the controller's clock
//! gate, programs the chip's geometry and timings, and kicks off the hardware
//! initialisation sequence; once `CR.PWRON` clears itself the whole capacity is
//! readable and writable through the mapped window.
//!
//! Two constructors exist because the bus is spread over five GPIO banks:
//! [`Sdramc::new_with_pins`] routes every pad of the reference wiring before
//! starting the chip, while [`Sdramc::new`] leaves the pads alone for boards
//! whose bootloader already routed them.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! // Pads already routed elsewhere: just bring the chip up.
//! let sdram = hal::sdramc::Sdramc::new(p.sdramc(), hal::sdramc::Config::default());
//! assert_eq!(sdram.size_bytes(), hal::sdramc::Size::Mb8.bytes());
//! let _first_word = unsafe { *(hal::sdramc::SDRAM_BASE as *const u32) };
//! ```
//!
//! Dropping the handle releases the pads it routed but leaves the controller
//! clocked and the window mapped: anything still pointing into SDRAM (a heap, a
//! frame buffer) stays valid. Use [`Sdramc::disable`] to park the chip in
//! self-refresh explicitly.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{self, Flex};

use instance::sealed::Instance as _;

mod instance;
pub mod pins;
mod types;
pub use types::*;

/// Build the pad bundle, the routed-pad holder and the routing helper.
///
/// Every SDRAM signal sits on its own pad of one of five banks, and the whole
/// bus has to stay muxed for as long as the chip is used, so the list lives in
/// one macro: it declares [`SdramPins`] (what the caller hands in), a private
/// [`RoutedPins`] holding one [`Flex`] per pad (what keeps the mux and the bank
/// clock alive), and the code turning the former into the latter.
macro_rules! sdram_pins {
    ( $( $name:ident : $pin:ident = $func:expr , )+ ) => {
        /// Every pad of the SDRAM interface, in the wiring of the vendor
        /// `SimplSDRAM` example.
        ///
        /// Hand this to [`Sdramc::new_with_pins`]; the pads are then muxed onto
        /// the controller and stay that way for as long as the handle lives.
        ///
        /// `LDQM`/`UDQM` also exist on `PA5`/`PD10` with different function
        /// codes -- see [`pins`] -- for boards that wire the byte masks there.
        pub struct SdramPins {
            $( pub $name: gpio::$pin, )+
        }

        impl Default for SdramPins {
            /// Default SDRAM wiring of the vendor `SimplSDRAM` example, so callers
            /// can write `SdramPins::default()` instead of spelling out every pad.
            ///
            /// Each pad is taken with the generated singleton's `steal()` -- the
            /// same token `p.Pxy` would hand over -- so it must not be used
            /// elsewhere at the same time.
            fn default() -> Self {
                unsafe {
                    Self {
                        $( $name: gpio::$pin::steal(), )+
                    }
                }
            }
        }

        /// The same pads, already routed, kept alive by the driver.
        ///
        /// Nothing ever reads these fields: holding the [`Flex`] is what keeps
        /// the mux setting and the bank clock in place.
        #[allow(dead_code)]
        struct RoutedPins<'d> {
            $( $name: Flex<'d, gpio::$pin>, )+
        }

        impl<'d> RoutedPins<'d> {
            /// Mux every pad of `pins` onto the SDRAM controller.
            fn route(pins: SdramPins) -> Self {
                $(
                    let mut $name = Flex::new(pins.$name);
                    $name.set_func($func);
                )+
                Self { $( $name, )+ }
            }
        }
    };
}

sdram_pins! {
    clk:  PM13 = pins::m::Pin13Func::SdrClk,
    cke:  PM14 = pins::m::Pin14Func::SdrCke,
    we:   PB7  = pins::b::Pin7Func::SdrWe,
    cas:  PB8  = pins::b::Pin8Func::SdrCas,
    ras:  PB9  = pins::b::Pin9Func::SdrRas,
    cs:   PB10 = pins::b::Pin10Func::SdrCs,
    ba0:  PE15 = pins::e::Pin15Func::SdrBa0,
    ba1:  PE14 = pins::e::Pin14Func::SdrBa1,
    ldqm: PB6  = pins::b::Pin6Func::SdrLdqm,
    udqm: PM12 = pins::m::Pin12Func::SdrUdqm,
    a0:   PN14 = pins::n::Pin14Func::SdrA0,
    a1:   PN13 = pins::n::Pin13Func::SdrA1,
    a2:   PN12 = pins::n::Pin12Func::SdrA2,
    a3:   PN11 = pins::n::Pin11Func::SdrA3,
    a4:   PN10 = pins::n::Pin10Func::SdrA4,
    a5:   PN9  = pins::n::Pin9Func::SdrA5,
    a6:   PN8  = pins::n::Pin8Func::SdrA6,
    a7:   PN7  = pins::n::Pin7Func::SdrA7,
    a8:   PN6  = pins::n::Pin6Func::SdrA8,
    a9:   PN3  = pins::n::Pin3Func::SdrA9,
    a10:  PN15 = pins::n::Pin15Func::SdrA10,
    a11:  PN2  = pins::n::Pin2Func::SdrA11,
    a12:  PM15 = pins::m::Pin15Func::SdrA12,
    d0:   PE7  = pins::e::Pin7Func::SdrD0,
    d1:   PE6  = pins::e::Pin6Func::SdrD1,
    d2:   PE5  = pins::e::Pin5Func::SdrD2,
    d3:   PE4  = pins::e::Pin4Func::SdrD3,
    d4:   PE3  = pins::e::Pin3Func::SdrD4,
    d5:   PE2  = pins::e::Pin2Func::SdrD5,
    d6:   PE1  = pins::e::Pin1Func::SdrD6,
    d7:   PE0  = pins::e::Pin0Func::SdrD7,
    d8:   PE8  = pins::e::Pin8Func::SdrD8,
    d9:   PE9  = pins::e::Pin9Func::SdrD9,
    d10:  PE10 = pins::e::Pin10Func::SdrD10,
    d11:  PE11 = pins::e::Pin11Func::SdrD11,
    d12:  PE12 = pins::e::Pin12Func::SdrD12,
    d13:  PE13 = pins::e::Pin13Func::SdrD13,
    d14:  PC14 = pins::c::Pin14Func::SdrD14,
    d15:  PC15 = pins::c::Pin15Func::SdrD15,
}

/// A handle to the SDRAM controller with its chip already initialised.
#[allow(dead_code)]
pub struct Sdramc<'d> {
    peri: PeripheralRef<'d, crate::Sdramc>,
    /// Pads routed by [`Sdramc::new_with_pins`]; `None` when the caller muxed
    /// them some other way.
    pins: Option<RoutedPins<'d>>,
    size: Size,
}

impl<'d> Sdramc<'d> {
    /// Bring the chip up, assuming its pads are already routed.
    ///
    /// Blocks until the hardware initialisation sequence completes.
    pub fn new(peri: impl Peripheral<P = crate::Sdramc> + 'd, config: Config) -> Self {
        into_ref!(peri);
        Self::start(peri, config, None)
    }

    /// Mux every pad of `pins` onto the controller and bring the chip up.
    ///
    /// The bus has to be routed *before* the initialisation sequence starts,
    /// because that sequence is what clocks the mode-register settings out to
    /// the chip.
    pub fn new_with_pins(
        peri: impl Peripheral<P = crate::Sdramc> + 'd,
        pins: SdramPins,
        config: Config,
    ) -> Self {
        into_ref!(peri);
        let routed = RoutedPins::route(pins);
        Self::start(peri, config, Some(routed))
    }

    /// Open the clock gate, program `config` and run the init sequence.
    fn start(
        peri: PeripheralRef<'d, crate::Sdramc>,
        config: Config,
        pins: Option<RoutedPins<'d>>,
    ) -> Self {
        crate::Sdramc::clock().open();

        // Every timing is counted in SDRAM clock cycles.
        let core_clock = crate::clock::sys_core_clock();
        let divider = config.clk_div as u32 + 1;
        let sdram_clock = core_clock / divider;
        // At least one cycle per microsecond, or `TIM` and `T64` would be zero
        // on a very slow core clock.
        let cycles_per_us = (core_clock / 1_000_000 / divider).max(1);

        // `TIM.T100US` counts SDRAM clocks and has to cover 100 us; the vendor
        // driver writes twice the nominal count to stay clear of that.
        let t100us = (cycles_per_us * 200) as u16;

        // One refresh command every `T64` clocks, paced so that every row is
        // refreshed once inside the chip's refresh window.
        let rows = config.size.rows();
        let t64 = (config.refresh_time_ms * 1000 / rows + 1) * cycles_per_us;

        crate::Sdramc::set_timing(config.trcd, config.trc, config.trp, t100us);
        crate::Sdramc::set_config(
            config.size,
            config.clk_div,
            config.cas_latency,
            // The controller needs the flag above 66 MHz SDRAM clock.
            sdram_clock > 66_000_000,
        );
        crate::Sdramc::set_refresh_period(t64);
        crate::Sdramc::start_init();
        while !crate::Sdramc::is_init_done() {}

        Self {
            peri,
            pins,
            size: config.size,
        }
    }

    /// Capacity of the chip in bytes, i.e. the length of the mapped window.
    pub fn size_bytes(&self) -> usize {
        self.size.bytes()
    }

    /// Geometry the controller was configured for.
    pub fn size(&self) -> Size {
        self.size
    }

    /// Address the chip is mapped at, [`SDRAM_BASE`].
    pub fn base_address(&self) -> usize {
        SDRAM_BASE
    }

    /// The whole mapped window as a byte slice.
    ///
    /// # Safety
    ///
    /// The caller must not create a second mutable slice over the same window,
    /// and must hold this handle (or otherwise keep the controller clocked) for
    /// as long as the slice is used.
    pub unsafe fn memory(&self) -> &'static mut [u8] {
        core::slice::from_raw_parts_mut(SDRAM_BASE as *mut u8, self.size.bytes())
    }

    /// True once the initialisation sequence has finished.
    pub fn is_ready(&self) -> bool {
        crate::Sdramc::is_init_done()
    }

    /// Raw controller status (`SR`).
    pub fn status(&self) -> u32 {
        crate::Sdramc::status()
    }

    /// Leave self-refresh so the chip can be read and written again.
    pub fn enable(&self) {
        crate::Sdramc::exit_self_refresh();
    }

    /// Park the chip in self-refresh: it keeps its contents but cannot be
    /// accessed until [`Sdramc::enable`].
    pub fn disable(&self) {
        crate::Sdramc::enter_self_refresh();
    }
}
