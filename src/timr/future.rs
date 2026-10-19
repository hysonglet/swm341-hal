//! Interrupt driven TIMR waits.
//!
//! A single [`AtomicWaker`] per instance is shared by every waiter of that
//! timer. The ISR only wakes it; the future performs the register work in its
//! `poll`, so no buffer storage lives in a static.

use core::future::poll_fn;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};
use super::types::{ClockSource, RunMode};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];
/// Per-instance one-shot "overflow happened" flag: set by the ISR, polled by `delay`.
static FIRED: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
///
/// The overflow flag (`IF.TO`) MUST be cleared here. Without it the (pending)
/// interrupt re-enters this handler forever and the executor never gets to poll
/// the awaiting task.
pub(crate) fn on_irq<T: Instance>() {
    let idx = T::index();
    // Clear IF.TO (write-1-to-clear on the `if` register) so the interrupt
    // does not immediately re-fire.
    unsafe { &*T::regs() }.if_.write(|w| w.to().set_bit());
    FIRED[idx].store(true, Ordering::SeqCst);
    WAKER[idx].wake();
}

/// Wait until the timer underflows once.
pub(crate) async fn delay<T: Instance>(period: u32) {
    let idx = T::index();
    // Unmask the NVIC line *before* arming the timer so the underflow can always
    // wake the task (the counter is only 24 bits, see below).
    unsafe { T::irq().enable() };
    T::clear_overflow();
    FIRED[idx].store(false, Ordering::SeqCst);
    T::set_overflow_irq(true);
    // The counter is only 24 bits, so pick the smallest prescaler whose divided
    // clock keeps `period` ticks inside `LOAD`; the timer then counts exactly
    // `period` ticks (clamped to the 24-bit maximum if even ÷256 cannot fit it).
    // `delay(n)` therefore always waits `n` timer ticks, whatever `n` is.
    let (prescaler, load) = super::types::Config::prescale_for_ticks(period);
    T::start(load, prescaler, ClockSource::Internal, RunMode::Timer);
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if FIRED[idx].load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;
    T::set_overflow_irq(false);
    FIRED[idx].store(false, Ordering::SeqCst);
}

/// Install the five NVIC handlers; the `no_mangle` names must match the vector
/// table in [`crate::vector_table`].
macro_rules! timr_handlers {
    ($($vector:ident => $peri:ident),* $(,)?) => {
        $(
            const _: crate::interrupt::ExtendedInterrupt = crate::interrupt::$vector;

            #[no_mangle]
            pub unsafe extern "C" fn $vector() {
                on_irq::<crate::$peri>();
            }
        )*
    };
}

timr_handlers! {
    TIMR0 => Timr0,
    TIMR1 => Timr1,
    TIMR2 => Timr2,
    TIMR3 => Timr3,
    TIMR4 => Timr4,
}
