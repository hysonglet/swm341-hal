//! Interrupt driven basic timer waits.
//!
//! A single [`AtomicWaker`] per instance is shared by every waiter of that timer.
//! The ISR only wakes it; the future performs the register work in its `poll`,
//! so no buffer storage lives in a static.

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
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await;
    T::set_overflow_irq(false);
}

/// Install the twelve NVIC handlers; the `no_mangle` names must match the vector
/// table in [`crate::vector_table`].
macro_rules! btimr_handlers {
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

btimr_handlers! {
    BTIMR0 => Btimr0,
    BTIMR1 => Btimr1,
    BTIMR2 => Btimr2,
    BTIMR3 => Btimr3,
    BTIMR4 => Btimr4,
    BTIMR5 => Btimr5,
    BTIMR6 => Btimr6,
    BTIMR7 => Btimr7,
    BTIMR8 => Btimr8,
    BTIMR9 => Btimr9,
    BTIMR10 => Btimr10,
    BTIMR11 => Btimr11,
}
