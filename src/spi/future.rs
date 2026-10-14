//! Interrupt driven SPI transfers.
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
    WAKER[T::index()].wake();
}

/// Full-duplex transfer once the hardware has room / data.
pub(crate) async fn transfer<T: Instance>(read: &mut [u8], write: &[u8]) {
    let idx = T::index();
    let total = write.len().max(read.len());
    let mut pos = 0usize;
    let mut rpos = 0usize;
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        // Fill the TX FIFO while there is room and work to send.
        while pos < total && T::is_tx_fifo_not_full() {
            let out = *write.get(pos).unwrap_or(&0xFF);
            T::write_data(out);
            pos += 1;
        }
        // Drain the RX FIFO; store into `read`, otherwise just discard so the
        // FIFO cannot overflow on a pure write.
        while T::is_rx_fifo_not_empty() {
            let sample = T::read_data();
            if let Some(slot) = read.get_mut(rpos) {
                *slot = sample;
                rpos += 1;
            }
        }
        if pos >= total && rpos >= read.len() {
            T::set_rx_threshold_irq(false);
            Poll::Ready(())
        } else {
            T::set_rx_threshold_irq(true);
            T::unmask_irq();
            Poll::Pending
        }
    })
    .await;
}

/// In-place full-duplex transfer: each position is sent and its concurrent
/// response is written back into the same buffer.
pub(crate) async fn transfer_in_place<T: Instance>(buf: &mut [u8]) {
    let idx = T::index();
    let total = buf.len();
    let mut pos = 0usize;
    let mut rpos = 0usize;
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        while pos < total && T::is_tx_fifo_not_full() {
            T::write_data(buf[pos]);
            pos += 1;
        }
        while T::is_rx_fifo_not_empty() {
            buf[rpos] = T::read_data();
            rpos += 1;
        }
        if pos >= total && rpos >= total {
            T::set_rx_threshold_irq(false);
            Poll::Ready(())
        } else {
            T::set_rx_threshold_irq(true);
            T::unmask_irq();
            Poll::Pending
        }
    })
    .await;
}

// -- SPI slave (peripheral) mode async primitives --

/// Async version of [`crate::spi::SpiSlave::blocking_wait_start`].
pub(crate) async fn slave_wait_start<T: Instance>() -> Result<(), Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_cs_fall() {
            T::clear_cs_fall();
            return Poll::Ready(Ok(()));
        }
        T::set_cs_fall_irq(true);
        T::unmask_irq();
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::spi::SpiSlave::blocking_wait_end`].
pub(crate) async fn slave_wait_end<T: Instance>() -> Result<(), Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_cs_rise() {
            T::clear_cs_rise();
            return Poll::Ready(Ok(()));
        }
        T::set_cs_rise_irq(true);
        T::unmask_irq();
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::spi::SpiSlave::blocking_read_byte`].
pub(crate) async fn slave_read_byte<T: Instance>() -> Result<u8, Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_rx_fifo_not_empty() {
            return Poll::Ready(Ok(T::read_data()));
        }
        if T::is_cs_rise() {
            T::clear_cs_rise();
            return Poll::Ready(Err(Error::TransferEnded));
        }
        T::set_rx_threshold_irq(true);
        T::set_cs_rise_irq(true);
        T::unmask_irq();
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::spi::SpiSlave::blocking_write_byte`].
pub(crate) async fn slave_write_byte<T: Instance>(byte: u8) -> Result<(), Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if T::is_tx_fifo_not_full() {
            T::write_data(byte);
            return Poll::Ready(Ok(()));
        }
        // No TX-threshold IRQ exists; the master clocks on, so RX progress (or a
        // CS rise) will wake us to re-check the TX FIFO.
        T::set_rx_threshold_irq(true);
        T::set_cs_rise_irq(true);
        T::unmask_irq();
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::spi::SpiSlave::blocking_transfer`].
pub(crate) async fn slave_transfer<T: Instance>(
    read: &mut [u8],
    write: &[u8],
) -> Result<(), Error> {
    let idx = T::index();
    let mut started = false;
    let mut wpos = 0usize;
    let mut rpos = 0usize;
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        if !started {
            if T::is_cs_fall() {
                T::clear_cs_fall();
                started = true;
            } else {
                T::set_cs_fall_irq(true);
                T::unmask_irq();
                return Poll::Pending;
            }
        }
        if T::is_cs_rise() {
            T::clear_cs_rise();
            while T::is_rx_fifo_not_empty() {
                let _ = T::read_data();
            }
            let res = if T::is_rx_overflow() {
                T::clear_overflow();
                Err(Error::Overrun)
            } else {
                Ok(())
            };
            T::set_cs_fall_irq(false);
            T::set_cs_rise_irq(false);
            T::set_rx_threshold_irq(false);
            return Poll::Ready(res);
        }
        if wpos < write.len() && T::is_tx_fifo_not_full() {
            T::write_data(write[wpos]);
            wpos += 1;
        }
        if T::is_rx_fifo_not_empty() {
            let byte = T::read_data();
            if rpos < read.len() {
                read[rpos] = byte;
            }
            rpos += 1;
        }
        T::set_rx_threshold_irq(true);
        T::set_cs_rise_irq(true);
        T::unmask_irq();
        Poll::Pending
    })
    .await
}

/// Install the NVIC handlers; the `const` item keeps the vector names honest.
macro_rules! spi_handlers {
    ($($vector:ident => $peri:ident),* $(,)?) => {
        $(
            const _: crate::pac::Interrupt = crate::interrupt::$vector;

            #[no_mangle]
            pub unsafe extern "C" fn $vector() {
                on_irq::<crate::$peri>();
            }
        )*
    };
}

spi_handlers! {
    SPI0 => Spi0,
    SPI1 => Spi1,
}
