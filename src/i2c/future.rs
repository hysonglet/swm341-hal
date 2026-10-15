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
enum Step {
    /// Decide the address / direction of the first operation and emit a START.
    Init,
    /// START + address (for the current operation) sent, waiting for TXE.
    Addr,
    /// A write data byte loaded, waiting for TXE before the next byte or the
    /// repeated START / STOP that ends the operation.
    WData,
    /// A read data byte requested, waiting for RXNE.
    RData,
    /// All read bytes of the current operation received, waiting for RXDONE
    /// before a repeated START (or, for the last op, before returning).
    RxDone,
    /// All write bytes of the final operation sent, waiting for TXDONE + STOP.
    TxDone,
    /// Current operation finished; load the next operation's address and emit a
    /// (repeated) START — the only place a START is generated mid-transfer.
    Next,
    /// Transfer finished; clear interrupts and return.
    Done,
}

/// Run a full I2C master transaction over `operations`.
///
/// Framing is strict, matching [`embedded_hal_async::i2c::I2c::transaction`]:
/// one START opens the first operation, a (repeated) START separates every
/// following operation, and a single STOP ends the whole transfer. This is what
/// register-address-then-read slaves need, where a STOP between the two phases
/// would abort the access.
///
/// A read byte carries `STO` only when it is the last byte of the *last*
/// operation, so a repeated START can follow an earlier read. (The SWM341 master
/// ACKs the final byte of a non-final read op; devices that require a NACK before
/// a repeated START are therefore not supported for read-then-read / read-then-write
/// sequences — the common write-then-read, write-only and read-only cases are fine.)
pub(crate) async fn master<T: Instance>(
    address: u8,
    operations: &mut [embedded_hal_async::i2c::Operation<'_>],
) -> Result<(), Error> {
    let idx = T::index();
    let mut step = Step::Init;
    let mut op_idx = 0usize;
    let mut byte_idx = 0usize;

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
                    let dir = direction(operations, op_idx, address);
                    T::txdata(dir);
                    T::start();
                    step = Step::Addr;
                }
                Step::Addr => {
                    if !T::is_txe() {
                        T::set_txe_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_txe();
                    let is_write = matches!(operations[op_idx], Operation::Write(_));
                    let len = op_len(operations, op_idx);
                    if is_write {
                        if byte_idx < len {
                            let byte = read_byte(operations, op_idx, byte_idx);
                            byte_idx += 1;
                            T::txdata(byte);
                            T::request_write();
                            step = Step::WData;
                        } else if op_idx + 1 == operations.len() {
                            step = Step::TxDone;
                        } else {
                            step = Step::Next;
                        }
                    } else if byte_idx < len {
                        let last = (byte_idx + 1 == len) && (op_idx + 1 == operations.len());
                        T::request_read(last);
                        step = Step::RData;
                    } else {
                        step = Step::RxDone;
                    }
                }
                Step::WData => {
                    if !T::is_txe() {
                        T::set_txe_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_txe();
                    let len = op_len(operations, op_idx);
                    if byte_idx < len {
                        let byte = read_byte(operations, op_idx, byte_idx);
                        byte_idx += 1;
                        T::txdata(byte);
                        T::request_write();
                    } else if op_idx + 1 == operations.len() {
                        step = Step::TxDone;
                    } else {
                        step = Step::Next;
                    }
                }
                Step::RData => {
                    if !T::is_rxne() {
                        T::set_rxne_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_rxne();
                    let len = op_len(operations, op_idx);
                    if byte_idx < len {
                        let slot = match &mut operations[op_idx] {
                            Operation::Read(buf) => &mut buf[byte_idx],
                            _ => unreachable!(),
                        };
                        *slot = T::rx_byte();
                        byte_idx += 1;
                        let last = (byte_idx + 1 == len) && (op_idx + 1 == operations.len());
                        T::request_read(last);
                    } else {
                        step = Step::RxDone;
                    }
                }
                Step::RxDone => {
                    if !T::is_rxdone() {
                        T::set_rxdone_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_rxdone();
                    if op_idx + 1 == operations.len() {
                        step = Step::Done;
                    } else {
                        step = Step::Next;
                    }
                }
                Step::Next => {
                    op_idx += 1;
                    byte_idx = 0;
                    let dir = direction(operations, op_idx, address);
                    T::txdata(dir);
                    T::start();
                    step = Step::Addr;
                }
                Step::TxDone => {
                    if !T::is_txdone() {
                        T::set_txdone_irq(true);
                        unsafe { T::irq().enable() };
                        return Poll::Pending;
                    }
                    T::clear_txdone();
                    T::stop();
                    step = Step::Done;
                }
                Step::Done => {
                    T::disable_all_irqs();
                    return Poll::Ready(Ok(()));
                }
            }
        }
    })
    .await
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
