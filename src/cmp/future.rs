//! Interrupt driven comparator output-change waits.
//!
//! All three comparators share the `ACMP` NVIC vector, so the ISR demultiplexes
//! by reading `SYS.ACMPSR` and wakes the waker of whichever unit raised its
//! `CMPxIF` flag (which it clears before waking, so the line does not
//! re-trigger forever).

use core::future::poll_fn;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Per-instance completion flag, set by the ISR.
///
/// Kept separate from the hardware flag because the ISR clears that one
/// immediately (see [`on_irq`]), which would otherwise leave the future with
/// nothing to observe.
static DONE: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];

/// Wake the waiters of whichever unit triggered the shared `ACMP` interrupt.
/// Called from the NVIC handler.
pub(crate) fn on_irq() {
    let regs = unsafe { &*crate::pac::SYS::PTR };
    let sr = regs.acmpsr.read().bits();
    for unit in 0..COUNT {
        let if_bit = 8 + unit;
        if sr & (1u32 << if_bit) != 0 {
            // Quench the interrupt line before waking: clear the write-1-to-clear
            // flag, then mark the unit done and wake its waiter.
            regs.acmpsr.write(|w| unsafe { w.bits(1u32 << if_bit) });
            DONE[unit].store(true, Ordering::SeqCst);
            WAKER[unit].wake();
        }
    }
}

/// Await a change of comparator `T`'s output, returning the new state
/// (`true` => positive input now above the negative input).
pub(crate) async fn wait_for_output_change<T: Instance>() -> bool {
    let idx = T::index();

    // Capture the starting output and clear any stale flag before arming.
    let start = T::output();
    T::clear_irq_flag();
    DONE[idx].store(false, Ordering::SeqCst);
    T::set_irq(true);

    let new = poll_fn(|cx| {
        // Register before sampling `DONE` so an interrupt landing between the
        // arm and this point still wakes us instead of being lost.
        WAKER[idx].register(cx.waker());

        // Output already differs from the value captured at the start: no need
        // to wait for the interrupt.
        let now = T::output();
        if now != start {
            return Poll::Ready(now);
        }

        if DONE[idx].load(Ordering::SeqCst) {
            // The flag fired but the output matches `start` again (it toggled
            // and toggled back, or a spurious edge) -- re-arm and keep waiting.
            DONE[idx].store(false, Ordering::SeqCst);
            T::clear_irq_flag();
            T::set_irq(true);
            Poll::Pending
        } else {
            T::set_irq(true);
            // The `ACMP` vector is shared by all three comparators, so it must
            // stay unmasked: only *this* unit's `CMPxIE` is touched here. The
            // ISR demultiplexes, so the others keep working.
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await;

    // Stop this unit from requesting the shared interrupt. The NVIC vector is
    // deliberately left enabled -- another comparator may still be waiting on
    // it. (A cancelled future skips this and leaves `CMPxIE` set; the ISR still
    // clears the flag, so it cannot wedge the interrupt line.)
    T::set_irq(false);
    T::clear_irq_flag();

    new
}

/// Install the shared `ACMP` NVIC handler; the `no_mangle` name must match the
/// vector table in [`crate::vector_table`].
macro_rules! cmp_handler {
    ($vector:ident) => {
        const _: crate::interrupt::ExtendedInterrupt = crate::interrupt::$vector;

        #[no_mangle]
        pub unsafe extern "C" fn $vector() {
            on_irq();
        }
    };
}

cmp_handler!(ACMP);
