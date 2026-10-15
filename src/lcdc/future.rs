//! Interrupt driven LCD waits.
//!
//! A single [`AtomicWaker`] backs the one LCD instance. The ISR only wakes it;
//! the future reads the register state in its `poll`, so no buffer storage lives
//! in a static.

use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT, IF_FRAMEDONE};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Wake the waiter of the instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    WAKER[T::index()].wake();
}

/// Wait until the controller signals that a frame has been completed.
pub(crate) async fn wait_frame<T: Instance>() {
    let idx = T::index();
    T::clear_interrupt(IF_FRAMEDONE);
    T::set_interrupt_enable(IF_FRAMEDONE, true);
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_frame_done() {
            Poll::Ready(())
        } else {
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await;
    T::set_interrupt_enable(IF_FRAMEDONE, false);
}

/// Install the NVIC handler; the `no_mangle` name must match the vector table in
/// [`crate::vector_table`].
const _: crate::interrupt::ExtendedInterrupt = crate::interrupt::LCD;

#[no_mangle]
pub unsafe extern "C" fn LCD() {
    on_irq::<crate::Lcd>();
}
