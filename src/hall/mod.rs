//! HALL (hall-sensor interface) driver.
//!
//! The SWM341 exposes two hall interfaces through the `TIMRG` block; select one
//! with [`HallSelect`]. Each interface has three input lines; the driver reports
//! their current logic level, latches change flags (cleared by writing 1), and
//! exposes the captured value register.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::hall::{Hall, HallSelect};
//! let mut hall = Hall::new(p.TIMRG, HallSelect::Hall0, Default::default());
//! let level0 = hall.input_level(0);   // current level of input 0
//! if hall.is_flag(0) { hall.clear_flag(0); }
//! let captured = hall.read_value();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::mode::{Blocking, Mode};

use instance::Instance;
use instance::hw;

pub mod instance;
mod types;

pub use types::*;

/// A HALL interface instance.
#[allow(dead_code)]
pub struct Hall<'d, T: Instance, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    which: u8,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, M: Mode> Hall<'d, T, M> {
    /// Take ownership of the HALL peripheral and enable the selected interface.
    pub fn new(peri: impl Peripheral<P = T> + 'd, select: HallSelect, config: Config) -> Self {
        into_ref!(peri);
        T::clock().open();
        T::enable();
        let which = select.which();
        let regs = unsafe { &*T::regs() };
        hw::enable_hall(regs, which);
        if config.enable_interrupt {
            hw::enable_interrupt(regs, which);
        }
        Self {
            peri,
            which,
            _mode: core::marker::PhantomData,
        }
    }
}

impl<'d, T: Instance, M: Mode> Drop for Hall<'d, T, M> {
    fn drop(&mut self) {
        let regs = unsafe { &*T::regs() };
        hw::disable_hall(regs, self.which);
        T::disable();
        T::clock().close();
    }
}

impl<'d, T: Instance> Hall<'d, T, Blocking> {
    /// Current logic level of input line `input` (0..=2).
    pub fn input_level(&self, input: u8) -> bool {
        hw::read_status(unsafe { &*T::regs() }, self.which, input)
    }

    /// `true` if input `input` latched a change since the last clear.
    pub fn is_flag(&self, input: u8) -> bool {
        hw::is_flag(unsafe { &*T::regs() }, self.which, input)
    }

    /// Clear the change flag for input `input` (write 1 to clear).
    pub fn clear_flag(&mut self, input: u8) {
        hw::clear_flag(unsafe { &*T::regs() }, self.which, input);
    }

    /// The captured value register for this interface (e.g. a timestamp/state).
    pub fn read_value(&self) -> u32 {
        hw::read_value(unsafe { &*T::regs() }, self.which)
    }

    /// Arm the input-change interrupt (if not already armed).
    pub fn enable_interrupt(&mut self) {
        hw::enable_interrupt(unsafe { &*T::regs() }, self.which);
    }

    /// Disarm the input-change interrupt.
    pub fn disable_interrupt(&mut self) {
        hw::disable_interrupt(unsafe { &*T::regs() }, self.which);
    }
}
