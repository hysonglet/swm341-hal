//! Data Watchpoint and Trace (DWT) unit.
//!
//! The DWT is the Arm Cortex-M core debug component present on the SWM341's
//! STAR-MC1 core (Armv8-M Mainline). It is not a SWM341 peripheral, so it has
//! no clock gate in [`crate::clock`] and no NVIC vector; [`Dwt::new`] switches
//! it on through the core's Debug Control Block.
//!
//! The headline feature is the free-running 32-bit cycle counter (`CYCCNT`),
//! a high-resolution wall clock driven by the CPU clock. It is the most useful
//! part of the unit for firmware: timing a block of code in cycles, running
//! micro-benchmarks, or seeding a cheap RNG. The unit also offers address /
//! data watchpoints (comparators) and a set of profiling counters.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut dwt = hal::dwt::Dwt::new(p.dwt());
//! dwt.enable_cycle_counter();
//! // ... do work ...
//! let cycles = dwt.cycle_count();
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use instance::sealed::Instance as _;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the core DWT unit.
#[allow(dead_code)]
pub struct Dwt<'d> {
    /// Keeps the `crate::Dwt` singleton unique for the lifetime of the handle.
    _peri: PeripheralRef<'d, crate::Dwt>,
}

impl<'d> Dwt<'d> {
    /// Take the DWT token, unlock the unit and turn the debug trace clock on.
    ///
    /// This does not start `CYCCNT`; call [`enable_cycle_counter`] (or use
    /// [`new_cycle_counter`]) once ready. The trace clock is required for every
    /// DWT feature, so it is enabled here unconditionally.
    pub fn new(peri: impl Peripheral<P = crate::Dwt> + 'd) -> Self {
        into_ref!(peri);
        crate::Dwt::unlock();
        crate::Dwt::enable_trace();
        Self { _peri: peri }
    }

    /// Take the token, enable the trace clock and immediately start the
    /// free-running cycle counter.
    pub fn new_cycle_counter(peri: impl Peripheral<P = crate::Dwt> + 'd) -> Self {
        let mut this = Self::new(peri);
        this.enable_cycle_counter();
        this
    }

    /// True when this core implements the cycle counter (`CYCCNT` present).
    pub fn has_cycle_counter(&self) -> bool {
        crate::Dwt::has_cycle_counter()
    }

    /// Enable the 32-bit free-running cycle counter.
    pub fn enable_cycle_counter(&mut self) {
        crate::Dwt::set_cycle_counter_enable(true);
    }

    /// Stop the cycle counter (it freezes; the count is retained).
    pub fn disable_cycle_counter(&mut self) {
        crate::Dwt::set_cycle_counter_enable(false);
    }

    /// True while the cycle counter is running.
    pub fn is_cycle_counter_enabled(&self) -> bool {
        crate::Dwt::cycle_counter_enabled()
    }

    /// Current value of the cycle counter.
    pub fn cycle_count(&self) -> u32 {
        crate::Dwt::read_cycle_count()
    }

    /// Overwrite the cycle counter (e.g. to zero it).
    pub fn set_cycle_count(&mut self, value: u32) {
        crate::Dwt::write_cycle_count(value);
    }

    /// Reset the cycle counter to zero.
    pub fn reset_cycle_count(&mut self) {
        self.set_cycle_count(0);
    }

    /// Measure `f` in CPU cycles.
    ///
    /// Enables the cycle counter for the duration if it was off, resets it,
    /// runs `f`, then returns `(result, cycles)` where `cycles` is the elapsed
    /// count (wrapping at 2^32). When the counter was already running the
    /// measurement is relative to the reset point.
    pub fn measure<F, R>(&mut self, f: F) -> (R, u32)
    where
        F: FnOnce() -> R,
    {
        let was_enabled = self.is_cycle_counter_enabled();
        if !was_enabled {
            self.enable_cycle_counter();
        }
        self.reset_cycle_count();
        let result = f();
        let cycles = self.cycle_count();
        if !was_enabled {
            self.disable_cycle_counter();
        }
        (result, cycles)
    }

    /// Start the CPI / exception / sleep / LSU / fold profiling counters.
    pub fn enable_profiling_counters(&mut self) {
        crate::Dwt::set_counters_enable(true);
    }

    /// Stop the profiling counters.
    pub fn disable_profiling_counters(&mut self) {
        crate::Dwt::set_counters_enable(false);
    }

    /// Cycles per instruction since the counters were last reset / enabled.
    pub fn cpi_count(&self) -> u8 {
        crate::Dwt::cpi_count()
    }
    /// Number of exceptions (excluding NMI / HardFault) since reset.
    pub fn exception_count(&self) -> u8 {
        crate::Dwt::exception_count()
    }
    /// Cycles spent sleeping.
    pub fn sleep_count(&self) -> u8 {
        crate::Dwt::sleep_count()
    }
    /// Cycles spent on load / store unit operations.
    pub fn lsu_count(&self) -> u8 {
        crate::Dwt::lsu_count()
    }
    /// Cycles spent in folded instructions.
    pub fn fold_count(&self) -> u8 {
        crate::Dwt::fold_count()
    }

    /// Configure comparator `index` as a watchpoint.
    ///
    /// Returns [`DwtError::ComparatorIndex`] when `index` exceeds the register
    /// map (max 15); the core may implement fewer comparators than that.
    pub fn set_watchpoint(&mut self, index: u8, watchpoint: Watchpoint) -> Result<(), DwtError> {
        if index > instance::MAX_COMPARATOR {
            return Err(DwtError::ComparatorIndex);
        }
        let function = watchpoint.kind.function_code();
        crate::Dwt::write_comparator(index, watchpoint.address, watchpoint.mask, function);
        Ok(())
    }

    /// Disable comparator `index`.
    pub fn clear_watchpoint(&mut self, index: u8) {
        if index <= instance::MAX_COMPARATOR {
            crate::Dwt::clear_comparator(index);
        }
    }
}
