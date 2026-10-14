//! Interrupt driven PWM waits.
//!
//! A single [`AtomicWaker`] per instance is shared by every waiter of that PWM.
//! The ISR only wakes it; the future performs the register work in its `poll`.

use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    WAKER[T::index()].wake();
}

/// Wait until one full PWM period has elapsed.
pub(crate) async fn wait_period<T: Instance>() {
    let idx = T::index();
    T::clear_overflow();
    T::set_overflow_irq(true);
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
macro_rules! pwm_handlers {
    ($($vector:ident => $peri:ident),* $(,)?) => {
        $(
            #[no_mangle]
            pub unsafe extern "C" fn $vector() {
                on_irq::<crate::$peri>();
            }
        )*
    };
}

pwm_handlers! {
    PWM0 => Pwm0,
    PWM1 => Pwm1,
    PWM2 => Pwm2,
    PWM3 => Pwm3,
    PWM4 => Pwm4,
}
