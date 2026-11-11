//! Interrupt driven I2C master transfers.
//!
//! The ISR only wakes the per-instance [`AtomicWaker`]; the future runs the whole
//! start / address / data / stop state machine in `poll`, so no transfer state
//! lives in a static.

use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::waitqueue::AtomicWaker;

use embedded_hal_async::i2c::Operation;

use super::instance::{Instance, COUNT};
use super::Error;

static WAKER: [AtomicWaker; COUNT] = [const { AtomicWaker::new() }; COUNT];

/// Wake the waiter of one instance; called from the NVIC handler.
pub(crate) fn on_irq<T: Instance>() {
    WAKER[T::index()].wake();
}

/// State of the [`master`] state machine.
#[derive(Clone, Copy)]
enum Step {
    /// Emit START + the address byte for the current operation.
    Init,
    /// Address byte sent; waiting for TXDONE to check the ACK.
    Addr,
    /// A write data byte sent; waiting for TXDONE.
    WData,
    /// A read byte requested; waiting for RXDONE.
    RData,
    /// Current operation finished; move on to the next one.
    Next,
    /// Transfer finished; STOP, release the IRQ and return.
    Done,
}

/// Run a full I2C master transaction over `operations`.
///
/// Framing matches [`embedded_hal_async::i2c::I2c::transaction`]: a START
/// opens each operation (a repeated START after the first), and one STOP ends
/// the whole transfer. Every byte is waited out to completion before the next
/// MCR command is issued — the hardware ignores a new WR/RD while one is still
/// in flight — and the ACK/NACK of transmitted bytes is checked.
pub(crate) async fn master<T: Instance>(
    address: u8,
    operations: &mut [embedded_hal_async::i2c::Operation<'_>],
) -> Result<(), Error> {
    let idx = T::index();
    let mut step = Step::Init;
    let mut op_idx = 0usize;
    let mut byte_idx = 0usize;

    T::disable_all_irqs();
    T::clear_flags();

    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());

        // Bus-level faults abort the whole transfer.
        T::check_faults()?;

        loop {
            match step {
                Step::Init => {
                    if op_idx >= operations.len() {
                        step = Step::Done;
                        continue;
                    }
                    T::txdata(direction(operations, op_idx, address));
                    T::start();
                    step = Step::Addr;
                }
                Step::Addr => {
                    if !T::is_txdone() {
                        T::set_txdone_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_txdone();
                    if !T::acked() {
                        T::disable_all_irqs();
                        return Poll::Ready(Err(Error::NoAcknowledge));
                    }
                    step = dispatch::<T>(
                        operations,
                        op_idx,
                        &mut byte_idx,
                        op_idx + 1 == operations.len(),
                    );
                }
                Step::WData => {
                    if !T::is_txdone() {
                        T::set_txdone_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_txdone();
                    if !T::acked() {
                        T::disable_all_irqs();
                        return Poll::Ready(Err(Error::NoAcknowledge));
                    }
                    step = dispatch::<T>(
                        operations,
                        op_idx,
                        &mut byte_idx,
                        op_idx + 1 == operations.len(),
                    );
                }
                Step::RData => {
                    if !T::is_rxdone() {
                        T::set_rxdone_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_rxdone();
                    let slot = match &mut operations[op_idx] {
                        Operation::Read(buf) => &mut buf[byte_idx - 1],
                        _ => unreachable!(),
                    };
                    *slot = T::rx_byte();
                    step = dispatch::<T>(
                        operations,
                        op_idx,
                        &mut byte_idx,
                        op_idx + 1 == operations.len(),
                    );
                }
                Step::Next => {
                    op_idx += 1;
                    byte_idx = 0;
                    step = Step::Init;
                }
                Step::Done => {
                    T::stop();
                    // STOP is short (a few SCL periods); keep the executor
                    // blocked only until the bus is idle again.
                    let mut timeout = 1_000_000u32;
                    while !T::bus_free() && timeout > 0 {
                        timeout -= 1;
                        core::hint::spin_loop();
                    }
                    T::disable_all_irqs();
                    return Poll::Ready(Ok(()));
                }
            }
        }
    })
    .await
}

/// Load the next byte of the current operation, or move the state machine to
/// the next operation / STOP. Called once an address or data byte completed.
fn dispatch<T: Instance>(
    operations: &mut [Operation<'_>],
    op_idx: usize,
    byte_idx: &mut usize,
    last_op: bool,
) -> Step {
    let is_write = matches!(operations[op_idx], Operation::Write(_));
    let len = op_len(operations, op_idx);
    if *byte_idx < len {
        if is_write {
            let byte = read_byte(operations, op_idx, *byte_idx);
            *byte_idx += 1;
            T::txdata(byte);
            T::request_write();
            Step::WData
        } else {
            // NACK the last byte of a read phase so a repeated START can
            // follow; ACK all earlier bytes.
            let nack = *byte_idx + 1 == len;
            *byte_idx += 1;
            T::request_read(nack);
            Step::RData
        }
    } else if last_op {
        Step::Done
    } else {
        Step::Next
    }
}

/// Encoded address byte (R/W bit cleared for a write, set for a read).
#[inline(always)]
fn direction(operations: &[Operation<'_>], op_idx: usize, address: u8) -> u8 {
    match operations[op_idx] {
        Operation::Write(_) => address << 1,
        Operation::Read(_) => (address << 1) | 1,
    }
}

/// Length of the buffer belonging to the operation at `op_idx`.
#[inline(always)]
fn op_len(operations: &[Operation<'_>], op_idx: usize) -> usize {
    match &operations[op_idx] {
        Operation::Write(b) => b.len(),
        Operation::Read(b) => b.len(),
    }
}

/// Copy of the write byte at `byte_idx` (read ops never reach here).
#[inline(always)]
fn read_byte(operations: &[Operation<'_>], op_idx: usize, byte_idx: usize) -> u8 {
    match &operations[op_idx] {
        Operation::Write(b) => b[byte_idx],
        Operation::Read(_) => unreachable!(),
    }
}

/// Async write of `bytes` to `address` (a single write operation).
pub(crate) async fn write<T: Instance>(address: u8, bytes: &[u8]) -> Result<(), Error> {
    let mut op = Operation::Write(bytes);
    master::<T>(address, core::slice::from_mut(&mut op)).await
}

/// Async read of `buffer` from `address` (a single read operation).
pub(crate) async fn read<T: Instance>(address: u8, buffer: &mut [u8]) -> Result<(), Error> {
    let mut op = Operation::Read(buffer);
    master::<T>(address, core::slice::from_mut(&mut op)).await
}

/// Async write followed by a repeated-start read (two operations, one transfer).
pub(crate) async fn write_read<T: Instance>(
    address: u8,
    write: &[u8],
    read: &mut [u8],
) -> Result<(), Error> {
    let mut ops = [Operation::Write(write), Operation::Read(read)];
    master::<T>(address, &mut ops).await
}

// -- I2C slave (peripheral) mode async primitives --

/// Async version of [`crate::i2c::I2cSlave::blocking_wait_address`].
pub(crate) async fn slave_wait_address<T: Instance>() -> Result<bool, Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        T::check_faults()?;
        if T::is_address_matched() {
            T::clear_address_matched();
            return Poll::Ready(Ok(T::slave_is_read()));
        }
        T::set_start_irq(true);
        T::set_stop_irq(true);
        unsafe { T::irq().enable() };
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::i2c::I2cSlave::blocking_read_byte`].
pub(crate) async fn slave_read_byte<T: Instance>() -> Result<u8, Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        T::check_faults()?;
        if T::is_rxne() {
            let byte = T::slave_rx_byte();
            T::clear_rxne();
            return Poll::Ready(Ok(byte));
        }
        T::set_rxne_irq(true);
        T::set_stop_irq(true);
        unsafe { T::irq().enable() };
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::i2c::I2cSlave::blocking_write_byte`].
pub(crate) async fn slave_write_byte<T: Instance>(byte: u8) -> Result<(), Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        T::check_faults()?;
        if T::is_txe() {
            T::slave_tx_byte(byte);
            return Poll::Ready(Ok(()));
        }
        T::set_txe_irq(true);
        T::set_stop_irq(true);
        unsafe { T::irq().enable() };
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::i2c::I2cSlave::blocking_wait_stop`].
pub(crate) async fn slave_wait_stop<T: Instance>() -> Result<(), Error> {
    let idx = T::index();
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        T::check_faults()?;
        if T::is_stop_detected() {
            T::clear_stop_detected();
            return Poll::Ready(Ok(()));
        }
        T::set_stop_irq(true);
        unsafe { T::irq().enable() };
        Poll::Pending
    })
    .await
}

/// Async version of [`crate::i2c::I2cSlave::blocking_transfer`].
pub(crate) async fn slave_transfer<T: Instance>(
    read: &mut [u8],
    write: &[u8],
) -> Result<(), Error> {
    let idx = T::index();
    let mut addressed = false;
    let mut is_read = false;
    let mut rpos = 0usize;
    let mut wpos = 0usize;
    let mut nacked = false;
    poll_fn(|cx| {
        WAKER[idx].register(cx.waker());
        T::check_faults()?;
        loop {
            if !addressed {
                if T::is_address_matched() {
                    T::clear_address_matched();
                    is_read = T::slave_is_read();
                    addressed = true;
                    if is_read && write.is_empty() {
                        T::slave_set_tx_nack(true);
                        nacked = true;
                    }
                } else {
                    T::set_start_irq(true);
                    T::set_stop_irq(true);
                    unsafe { T::irq().enable() };
                    return Poll::Pending;
                }
            }
            if T::is_stop_detected() {
                T::clear_stop_detected();
                if nacked {
                    T::slave_set_tx_nack(false);
                }
                T::disable_all_irqs();
                return Poll::Ready(Ok(()));
            }
            if is_read {
                if T::is_txe() {
                    if wpos < write.len() {
                        T::slave_tx_byte(write[wpos]);
                        wpos += 1;
                    } else {
                        if !nacked {
                            T::slave_set_tx_nack(true);
                            nacked = true;
                        }
                        T::set_stop_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                } else {
                    T::set_txe_irq(true);
                    T::set_stop_irq(true);
                    unsafe { T::irq().enable() };
                    return Poll::Pending;
                }
            } else if T::is_rxne() {
                let byte = T::slave_rx_byte();
                T::clear_rxne();
                if rpos < read.len() {
                    read[rpos] = byte;
                    rpos += 1;
                }
            } else {
                T::set_rxne_irq(true);
                T::set_stop_irq(true);
                unsafe { T::irq().enable() };
                return Poll::Pending;
            }
        }
    })
    .await
}

/// Install the NVIC handlers. Every vector -- including `I2C1` -- is now an
/// `ExtendedInterrupt`, so the `const _: ExtendedInterrupt = ...` name check is
/// applied like the other drivers.
macro_rules! i2c_handlers {
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

i2c_handlers! {
    I2C0 => I2c0,
    I2C1 => I2c1,
}
