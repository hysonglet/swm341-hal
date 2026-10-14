//! `embassy-time` driver built on the SysTick timer.
//!
//! SysTick is the only timer that always exists and always runs off the core
//! clock, so it is the natural time base here. The counter is reloaded once per
//! tick and every reload increments a 64-bit cycle counter in software, which is
//! what keeps `now()` monotonic past the 24-bit hardware range.
//!
//! A single alarm slot is enough: `embassy-time` keeps its own queue and only
//! ever asks this driver to wake at the earliest deadline.

use core::cell::{Cell, RefCell};
use core::sync::atomic::{AtomicU32, Ordering};
use core::task::Waker;

use cortex_m::peripheral::{syst::SystClkSource, SYST};
use critical_section::Mutex;
use embassy_time_driver::{Driver, TICK_HZ};

use crate::clock;

struct AlarmState {
    timestamp: Cell<u64>,
    waker: RefCell<Option<Waker>>,
}

impl AlarmState {
    const fn idle() -> Self {
        Self {
            timestamp: Cell::new(u64::MAX),
            waker: RefCell::new(None),
        }
    }
}

struct SystickDriver {
    /// Core clocks per tick, used to program the reload value.
    period: AtomicU32,
    /// Whole ticks elapsed since initialisation.
    ///
    /// Extended to 64 bits in software because this target has no 64-bit
    /// atomics and a bare `u32` millisecond counter would wrap in 49 days.
    ticks: Mutex<Cell<u64>>,
    alarm: Mutex<AlarmState>,
}

embassy_time_driver::time_driver_impl!(
    static DRIVER: SystickDriver = SystickDriver {
        period: AtomicU32::new(0),
        ticks: Mutex::new(Cell::new(0)),
        alarm: Mutex::new(AlarmState::idle()),
    }
);

impl SystickDriver {
    /// Program SysTick for the current core frequency.
    ///
    /// Must run after [`crate::clock::sys_core_clock`] is known, i.e. from
    /// [`crate::init`].
    pub(crate) fn init(&'static self) {
        let period = (clock::sys_core_clock() / TICK_HZ as u32).max(1);
        self.period.store(period, Ordering::Relaxed);

        critical_section::with(|_| {
            let syst = unsafe { &mut *(SYST::PTR as *mut SYST) };
            syst.set_clock_source(SystClkSource::Core);
            syst.set_reload(period - 1);
            syst.clear_current();
            syst.enable_counter();
            syst.enable_interrupt();
        });
    }

    /// Called from the SysTick exception: advance time and fire a due alarm.
    fn on_interrupt(&self) {
        critical_section::with(|cs| {
            let ticks = self.ticks.borrow(cs);
            ticks.set(ticks.get() + 1);
            let now = ticks.get();
            let alarm = self.alarm.borrow(cs);
            if alarm.timestamp.get() <= now {
                alarm.timestamp.set(u64::MAX);
                if let Some(waker) = alarm.waker.borrow_mut().take() {
                    waker.wake();
                }
            }
        });
    }
}

impl Driver for SystickDriver {
    fn now(&self) -> u64 {
        critical_section::with(|cs| self.ticks.borrow(cs).get())
    }

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        critical_section::with(|cs| {
            let alarm = self.alarm.borrow(cs);
            if at <= self.now() {
                // Already due; wake straight away instead of arming the slot.
                waker.wake_by_ref();
                return;
            }
            alarm.timestamp.set(at);
            *alarm.waker.borrow_mut() = Some(waker.clone());
        });
    }
}

/// Start the `embassy-time` time base from [`crate::init`].
pub(crate) fn init_time_driver() {
    DRIVER.init();
}

#[cortex_m_rt::exception]
fn SysTick() {
    DRIVER.on_interrupt();
}
