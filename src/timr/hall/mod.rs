//! HALL (hall-sensor interface) driver.
//!
//! The SWM341 exposes two hall interfaces through the `TIMRG` block: `HALL0` and
//! `HALL3`. Each watches three input lines, is **dual-edge triggered**, and on
//! every edge latches its *paired* timer's elapsed count -- `HALL0` captures
//! TIMR0 into `TIMRG.HALL0V`, `HALL3` captures TIMR3 into `TIMRG.HALL3V`. The
//! current line levels are readable from `HALLSR`, and each line latches a
//! change flag in `HALLIF` (write 1 to clear).
//!
//! That timer coupling is why this lives under [`crate::timr`]: the constructors
//! take the paired [`Timr`] and arm it free-running, because without a running
//! counter the captured value never updates. The timer's prescaler sets the unit
//! of the reading -- the vendor `SimplHALL` example prescales to 1 MHz so
//! `HALL0V` reads directly in microseconds.
//!
//! The constructors also mux the three input pads, so the interface in use is
//! implied by the pads rather than passed separately: [`new_hall0`](Hall::new_hall0)
//! takes `HALL0_INy` pads, [`new_hall3`](Hall::new_hall3) the `HALL3_INy` ones.
//! Every input has several candidate pads (see [`pins`]) and the trait bounds
//! reject a pad belonging to the other interface or to a different input.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::timr::Timr;
//! use hal::timr::hall::Hall;
//! // TIMR0 is HALL0's time base.
//! let mut timer = Timr::new(p.timr0(), hal::timr::Config::default());
//! // HALL0 on port A: PA0/PA1/PA2 are inputs 0/1/2.
//! let mut hall = Hall::new_hall0(p.hall(), &mut timer, p.PA0, p.PA1, p.PA2, Default::default());
//! let level0 = hall.input_level(0);   // current level of input 0
//! if hall.is_flag(0) { hall.clear_flag(0); }
//! let elapsed_ticks = hall.read_value();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{Flex, Pull};
use crate::mode::{Blocking, Mode};
use crate::timr::Timr;

use instance::Instance;

pub mod instance;
pub mod pins;
mod types;

pub use types::*;

/// Period the paired timer is armed with for hall capture.
///
/// The hall value register is 24 bits, so the counter is loaded with its maximum
/// (`0xFFFFFF`) and simply wraps: the datasheet prescribes a maximal load and the
/// vendor `SimplHALL` example uses `0xFFFFFF` too. [`Timr::start`] writes
/// `period - 1` into `LOAD`, hence `0x100_0000`.
pub const TIMEBASE_PERIOD: u32 = 0x100_0000;

/// A pad that can serve as one HALL input.
///
/// `INTERFACE` is the silicon's interface number -- `0` for `HALL0`, `3` for
/// `HALL3` -- and `INPUT` the line number `0..=2`. Together they pin down both
/// the interface and the line, so a pad belonging to the other interface, or to a
/// different input, is rejected at compile time.
///
/// [`FUNC`](HallInputPin::FUNC) is the `PORTx.FUNC` code selecting that function;
/// it differs per pad, which is why it comes from the generated [`pins`] table
/// instead of being a constant.
pub trait HallInputPin<const INTERFACE: u8, const INPUT: u8>: crate::gpio::Pin {
    /// `PORTx.FUNC` code routing this pad to that hall input.
    const FUNC: u8;
}

// HALL0: IN0 = PA0/PB3, IN1 = PA1/PB4, IN2 = PA2/PB5.
impl HallInputPin<0, 0> for crate::gpio::PA0 {
    const FUNC: u8 = pins::a::Pin0Func::Hall0In0 as u8;
}
impl HallInputPin<0, 0> for crate::gpio::PB3 {
    const FUNC: u8 = pins::b::Pin3Func::Hall0In0 as u8;
}
impl HallInputPin<0, 1> for crate::gpio::PA1 {
    const FUNC: u8 = pins::a::Pin1Func::Hall0In1 as u8;
}
impl HallInputPin<0, 1> for crate::gpio::PB4 {
    const FUNC: u8 = pins::b::Pin4Func::Hall0In1 as u8;
}
impl HallInputPin<0, 2> for crate::gpio::PA2 {
    const FUNC: u8 = pins::a::Pin2Func::Hall0In2 as u8;
}
impl HallInputPin<0, 2> for crate::gpio::PB5 {
    const FUNC: u8 = pins::b::Pin5Func::Hall0In2 as u8;
}

// HALL3: IN0 = PA12/PA15/PD2, IN1 = PA13/PC0/PD3, IN2 = PA14/PC1/PD4.
impl HallInputPin<3, 0> for crate::gpio::PA12 {
    const FUNC: u8 = pins::a::Pin12Func::Hall3In0 as u8;
}
impl HallInputPin<3, 0> for crate::gpio::PA15 {
    const FUNC: u8 = pins::a::Pin15Func::Hall3In0 as u8;
}
impl HallInputPin<3, 0> for crate::gpio::PD2 {
    const FUNC: u8 = pins::d::Pin2Func::Hall3In0 as u8;
}
impl HallInputPin<3, 1> for crate::gpio::PA13 {
    const FUNC: u8 = pins::a::Pin13Func::Hall3In1 as u8;
}
impl HallInputPin<3, 1> for crate::gpio::PC0 {
    const FUNC: u8 = pins::c::Pin0Func::Hall3In1 as u8;
}
impl HallInputPin<3, 1> for crate::gpio::PD3 {
    const FUNC: u8 = pins::d::Pin3Func::Hall3In1 as u8;
}
impl HallInputPin<3, 2> for crate::gpio::PA14 {
    const FUNC: u8 = pins::a::Pin14Func::Hall3In2 as u8;
}
impl HallInputPin<3, 2> for crate::gpio::PC1 {
    const FUNC: u8 = pins::c::Pin1Func::Hall3In2 as u8;
}
impl HallInputPin<3, 2> for crate::gpio::PD4 {
    const FUNC: u8 = pins::d::Pin4Func::Hall3In2 as u8;
}

/// The three HALL input pads, held so their mux stays configured.
///
/// Never read back: the fields exist purely to keep the pads acquired (and their
/// port clock on) for as long as the hall interface lives.
#[allow(dead_code)]
struct HallPads<'d, I0: crate::gpio::Pin, I1: crate::gpio::Pin, I2: crate::gpio::Pin> {
    in0: Flex<'d, I0>,
    in1: Flex<'d, I1>,
    in2: Flex<'d, I2>,
}

/// Mux `pin` to `func`, apply `pull` and return the handle that keeps it routed.
///
/// The pads get a pull resistor because hall sensors are normally open-drain; the
/// vendor `SimplHALL` example enables the pads' pull-ups as well.
fn route_pad<'d, P: crate::gpio::Pin>(
    pin: impl Peripheral<P = P> + 'd,
    func: u8,
    pull: Pull,
) -> Flex<'d, P> {
    let flex = Flex::new(pin);
    crate::gpio::set_pull::<P>(pull);
    crate::gpio::set_function::<P>(func);
    flex
}

/// A HALL interface instance, owning the three input pads it drives.
#[allow(dead_code)]
pub struct Hall<
    'd,
    T: Instance,
    I0: crate::gpio::Pin,
    I1: crate::gpio::Pin,
    I2: crate::gpio::Pin,
    M: Mode = Blocking,
> {
    peri: PeripheralRef<'d, T>,
    which: u8,
    pads: HallPads<'d, I0, I1, I2>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, I0, I1, I2> Hall<'d, T, I0, I1, I2, Blocking>
where
    I0: HallInputPin<0, 0>,
    I1: HallInputPin<0, 1>,
    I2: HallInputPin<0, 2>,
{
    /// Drive **HALL0**, timed by TIMR0.
    ///
    /// `timer` must be TIMR0 -- the interface captures that timer's count, so
    /// passing any other one will not compile. It is armed free-running from
    /// [`TIMEBASE_PERIOD`]; its prescaler decides the unit of
    /// [`read_value`](Hall::read_value).
    pub fn new_hall0(
        peri: impl Peripheral<P = T> + 'd,
        timer: &mut Timr<'d, crate::Timr0, Blocking>,
        in0: impl Peripheral<P = I0> + 'd,
        in1: impl Peripheral<P = I1> + 'd,
        in2: impl Peripheral<P = I2> + 'd,
        config: Config,
    ) -> Self {
        timer.start(TIMEBASE_PERIOD);
        Self::build(
            peri,
            route_pad(in0, I0::FUNC, config.pull),
            route_pad(in1, I1::FUNC, config.pull),
            route_pad(in2, I2::FUNC, config.pull),
            HallSelect::Hall0,
            config,
        )
    }
}

impl<'d, T: Instance, I0, I1, I2> Hall<'d, T, I0, I1, I2, Blocking>
where
    I0: HallInputPin<3, 0>,
    I1: HallInputPin<3, 1>,
    I2: HallInputPin<3, 2>,
{
    /// Drive **HALL3**, timed by TIMR3; otherwise identical to
    /// [`new_hall0`](Hall::new_hall0).
    pub fn new_hall3(
        peri: impl Peripheral<P = T> + 'd,
        timer: &mut Timr<'d, crate::Timr3, Blocking>,
        in0: impl Peripheral<P = I0> + 'd,
        in1: impl Peripheral<P = I1> + 'd,
        in2: impl Peripheral<P = I2> + 'd,
        config: Config,
    ) -> Self {
        timer.start(TIMEBASE_PERIOD);
        Self::build(
            peri,
            route_pad(in0, I0::FUNC, config.pull),
            route_pad(in1, I1::FUNC, config.pull),
            route_pad(in2, I2::FUNC, config.pull),
            HallSelect::Hall3,
            config,
        )
    }
}

impl<'d, T: Instance, I0: crate::gpio::Pin, I1: crate::gpio::Pin, I2: crate::gpio::Pin>
    Hall<'d, T, I0, I1, I2, Blocking>
{
    fn build(
        peri: impl Peripheral<P = T> + 'd,
        in0: Flex<'d, I0>,
        in1: Flex<'d, I1>,
        in2: Flex<'d, I2>,
        select: HallSelect,
        config: Config,
    ) -> Self {
        into_ref!(peri);
        T::clock().open();
        let which = select.which();
        T::enable_hall(which);
        // Drop anything latched before we took ownership, so the first edge seen
        // by the caller is a real one.
        T::clear_all_flags(which);
        if config.enable_interrupt {
            T::enable_interrupt(which);
            // Each interface owns its NVIC line (`HALL0` / `HALL3`), so unmask the
            // one belonging to *this* interface.
            unsafe { T::irq_for(which).enable() };
        }
        Self {
            peri,
            which,
            pads: HallPads { in0, in1, in2 },
            _mode: core::marker::PhantomData,
        }
    }
}

impl<
        'd,
        T: Instance,
        I0: crate::gpio::Pin,
        I1: crate::gpio::Pin,
        I2: crate::gpio::Pin,
        M: Mode,
    > Drop for Hall<'d, T, I0, I1, I2, M>
{
    fn drop(&mut self) {
        // Leave the paired TIMR alone: it belongs to the caller.
        T::disable_interrupt(self.which);
        T::disable_hall(self.which);
        T::clock().close();
    }
}

impl<'d, T: Instance, I0: crate::gpio::Pin, I1: crate::gpio::Pin, I2: crate::gpio::Pin>
    Hall<'d, T, I0, I1, I2, Blocking>
{
    /// Which interface this handle drives.
    pub fn select(&self) -> HallSelect {
        if self.which == 0 {
            HallSelect::Hall0
        } else {
            HallSelect::Hall3
        }
    }

    /// Index of the TIMR whose count this interface captures (0 or 3).
    pub fn paired_timer_index(&self) -> usize {
        T::paired_timer_index(self.which)
    }

    /// Current logic level of input line `input` (0..=2), from `HALLSR`.
    pub fn input_level(&self, input: u8) -> bool {
        T::read_status(self.which, input)
    }

    /// `true` if input `input` latched a change since the last clear.
    pub fn is_flag(&self, input: u8) -> bool {
        T::is_flag(self.which, input)
    }

    /// Clear the change flag for input `input` (write 1 to clear).
    pub fn clear_flag(&mut self, input: u8) {
        T::clear_flag(self.which, input);
    }

    /// Clear the change flags of all three inputs at once.
    pub fn clear_all_flags(&mut self) {
        T::clear_all_flags(self.which);
    }

    /// Elapsed count of the paired timer at the last input edge.
    ///
    /// The hardware stores `load - current` of this interface's timer on **every**
    /// edge of **any** of the three inputs (the module is dual-edge triggered),
    /// overwriting the previous value. So this is the time since the last edge in
    /// timer ticks -- scale it by the timer's prescaler to get seconds. It is only
    /// meaningful once the paired timer runs, which
    /// [`new_hall0`](Hall::new_hall0) / [`new_hall3`](Hall::new_hall3) arrange.
    pub fn read_value(&self) -> u32 {
        T::read_value(self.which)
    }

    /// Arm the input-change interrupt (if not already armed).
    pub fn enable_interrupt(&mut self) {
        T::enable_interrupt(self.which);
        unsafe { T::irq_for(self.which).enable() };
    }

    /// Disarm the input-change interrupt.
    pub fn disable_interrupt(&mut self) {
        T::disable_interrupt(self.which);
        T::irq_for(self.which).disable();
    }
}
