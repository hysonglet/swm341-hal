//! Interrupt driven ADC reads.
//!
//! Only one conversion per ADC can be in flight -- the driver hands out
//! `&mut Adc` for async reads -- so one [`AtomicWaker`] plus one `DONE` flag per
//! instance is enough.
//!
//! The ISR has to clear the interrupt itself. `IF.SEQ0EOC` is write-1-to-clear
//! and keeps the controller's interrupt line asserted for as long as it is set,
//! so if the handler only woke the task, NVIC would re-enter it in a tight loop
//! and the task would never get scheduled to clear the flag.

use core::future::poll_fn;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};
use super::types::{Error, CHANNEL_COUNT};

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Per-instance completion flag, set by the ISR.
///
/// Kept separate from the hardware flag because the ISR clears that one
/// straight away (see the module docs), which would otherwise leave the future
/// with nothing to observe.
static DONE: [AtomicBool; COUNT] = [const { AtomicBool::new(false) }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    let idx = T::index();
    // Quench the interrupt line before waking: disable the source, then clear
    // the (write-1-to-clear) flag. Both are needed or the handler re-enters
    // forever and starves the executor.
    T::set_eoc_irq(false);
    T::clear_eoc();
    DONE[idx].store(true, Ordering::SeqCst);
    WAKER[idx].wake();
}

/// Convert `channel`, awaiting the end-of-conversion interrupt.
pub(crate) async fn read_async<T: Instance>(channel: u8) -> Result<u16, Error> {
    if channel >= CHANNEL_COUNT {
        // `SEQCHN0.SEQ0` is a 12-bit mask: an out-of-range channel would select
        // no channel at all and the conversion would never complete.
        return Err(Error::InvalidChannel);
    }

    let idx = T::index();
    DONE[idx].store(false, Ordering::SeqCst);
    T::set_eoc_irq(false);
    T::clear_eoc();
    // A previously cancelled conversion still runs to completion and pushes its
    // result into the FIFO, so drop anything left over before arming this one.
    T::flush_fifo();

    T::select_channel(channel);
    T::start_conversion();
    let (got, value) = poll_fn(|cx| {
        // Register before sampling `DONE` so an interrupt landing between the
        // start and this point still wakes us instead of being lost.
        WAKER[idx].register(cx.waker());
        if DONE[idx].load(Ordering::SeqCst) {
            Poll::Ready(T::result())
        } else {
            T::set_eoc_irq(true);
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await;

    T::set_eoc_irq(false);
    T::clear_eoc();

    if got != channel {
        return Err(Error::ChannelMismatch {
            expected: channel,
            got,
        });
    }
    Ok(value)
}

/// Install the two NVIC handlers; the `no_mangle` names must match the vector
/// table in [`crate::vector_table`].
macro_rules! adc_handlers {
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

adc_handlers! {
    ADC0 => Adc0,
    ADC1 => Adc1,
}
