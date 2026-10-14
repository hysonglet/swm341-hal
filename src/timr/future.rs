//! Interrupt driven TIMR waits.
//!
//! A single [`AtomicWaker`] per instance is shared by every waiter of that
//! timer. The ISR only wakes it; the future performs the register work in its
//! `poll`, so no buffer storage lives in a static.

use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    WAKER[T::index()].wake();
}

/// Wait until the timer underflows once.
pub(crate) async fn delay<T: Instance>(period: u32) {
    let idx = T::index();
    T::clear_overflow();
    T::set_overflow_irq(true);
    T::start(period, 0, 0, 0);
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::overflow_flag() {
            Poll::Ready(())
        } else {
            T::unmask_irq();
            Poll::Pending
        }
    })
    .await;
    T::set_overflow_irq(false);
}

/// Install the five NVIC handlers; the `no_mangle` names must match the vector
/// table in [`crate::vector_table`].
macro_rules! timr_handlers {
    ($($vector:ident => $peri:ident),* $(,)?) => {
        $(
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
