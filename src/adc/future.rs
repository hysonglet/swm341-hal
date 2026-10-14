//! Interrupt driven ADC reads.
//!
//! A single [`AtomicWaker`] per instance is shared by every waiter of that ADC.
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

/// Convert `channel`, awaiting the end-of-conversion interrupt.
pub(crate) async fn read_async<T: Instance>(channel: u8) -> u16 {
    let idx = T::index();
    T::clear_eoc();
    T::select_channel(channel);
    T::start_conversion();
    let value = poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::eoc_flag() {
            Poll::Ready(T::result())
        } else {
            T::set_eoc_irq(true);
            T::unmask_irq();
            Poll::Pending
        }
    })
    .await;
    T::clear_eoc();
    T::set_eoc_irq(false);
    value
}

/// Install the two NVIC handlers; the `no_mangle` names must match the vector
/// table in [`crate::vector_table`].
macro_rules! adc_handlers {
    ($($vector:ident => $peri:ident),* $(,)?) => {
        $(
            #[no_mangle]
            pub unsafe extern "C" fn $vector() {
                on_irq::<crate::$peri>();
            }
        )*
    };
}

adc_handlers! {
    ADC0 => Adc0,
    ADC1 => Adc1,
}
