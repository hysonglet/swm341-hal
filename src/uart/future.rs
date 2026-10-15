//! Interrupt driven UART transfers.
//!
//! A single [`AtomicWaker`] per instance is shared by the transmit and receive
//! paths. The ISR only wakes it; the future performs the actual register work in
//! its `poll`, so no buffer storage lives in a static.

use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, COUNT};
use super::Error;

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    // The UART drives a single combined interrupt line for both TX and RX, so we
    // cannot tell here which event fired. Quench both sources: the interrupt is
    // level-sensitive, so leaving either IE bit set would keep the line asserted
    // and the handler would re-enter in a tight loop, starving the executor
    // before the woken task runs to read/clear the condition. The awaiting future
    // re-enables the bit it still needs on its next poll.
    T::set_rxie(false);
    T::set_txie(false);
    WAKER[T::index()].wake();
}

/// Read one byte once it is received.
pub(crate) async fn read<T: Instance>() -> Result<u8, Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_rx_not_empty() {
            T::set_rxie(false);
            let (data, parity_error) = T::read_data();
            if parity_error {
                Poll::Ready(Err(Error::Parity))
            } else {
                Poll::Ready(Ok(data))
            }
        } else {
            T::set_rxie(true);
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await
}

/// Write a whole buffer, resuming whenever the FIFO has room.
pub(crate) async fn write<T: Instance>(bytes: &[u8]) {
    let idx = T::index();
    let mut pos = 0usize;
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        while pos < bytes.len() && !T::is_tx_fifo_full() {
            T::write_data(bytes[pos]);
            pos += 1;
        }
        if pos >= bytes.len() {
            T::set_txie(false);
            Poll::Ready(())
        } else {
            T::set_txie(true);
            unsafe { T::irq().enable() };
            Poll::Pending
        }
    })
    .await;

    // Make sure the last byte actually leaves the shift register before we return,
    // so a subsequent `drop` (which gates the clock) cannot truncate it.
    while !T::is_tx_idle() {
        core::hint::spin_loop();
    }
}

/// Read into `buf` one byte at a time.
pub(crate) async fn read_bytes<T: Instance>(buf: &mut [u8]) -> Result<(), Error> {
    for slot in buf.iter_mut() {
        *slot = read::<T>().await?;
    }
    Ok(())
}

/// Install the four NVIC handlers; the `const` items keep the vector names honest.
macro_rules! uart_handlers {
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

uart_handlers! {
    UART0 => Uart0,
    UART1 => Uart1,
    UART2 => Uart2,
    UART3 => Uart3,
}
