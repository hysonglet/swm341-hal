//! Asynchronous pin interrupts.
//!
//! Every bank funnels all 16 pads into a single NVIC vector, so the handler has
//! to demultiplex by pin. Because the handler acknowledges the hardware flag
//! (and a level sensitive pin can re-assert it at any moment) the *fact* that an
//! interrupt happened is latched per pad here; the waiting future consumes that
//! latch instead of racing the hardware flag.

use core::cell::Cell;
use core::future::poll_fn;
use core::task::Poll;

use critical_section::{CriticalSection, Mutex};
use embassy_sync::waitqueue::AtomicWaker;

use super::instance::{Instance, PIN_COUNT, PORT_COUNT};
use super::{Pin, Trigger};

// `AtomicWaker` is not `Copy`, so the usual `const ITEM: .. = ..;` + repeat
// idiom does not apply; inline consts build each element in place instead.
static PIN_WAKERS: [[AtomicWaker; PIN_COUNT]; PORT_COUNT] =
    [const { [const { AtomicWaker::new() }; PIN_COUNT] }; PORT_COUNT];

static TRIGGERED: [Mutex<Cell<u16>>; PORT_COUNT] = [const { Mutex::new(Cell::new(0)) }; PORT_COUNT];

/// Arm `pin` for `trigger` and suspend until it fires.
pub(crate) async fn wait_for<P: Pin>(trigger: Trigger) {
    let port = P::port() as usize;
    let number = P::number() as usize;
    let mask = 1u16 << number;

    critical_section::with(|cs| {
        let latch = TRIGGERED[port].borrow(cs);
        latch.set(latch.get() & !mask);
    });

    set_trigger::<P>(trigger);
    clear_pending::<P>();
    enable_interrupt::<P>(true);
    // The vector is only ever unmasked here, once a handler is in place and a
    // future is waiting for the pad.
    unsafe { <P as super::sealed::Pin>::Port::enable_port_interrupt() };

    poll_fn(|cx| {
        // Registration is atomic, and the latch below is read after it, so an
        // interrupt landing in between can only ever be observed, never lost.
        PIN_WAKERS[port][number].register(cx.waker());
        if critical_section::with(|cs| TRIGGERED[port].borrow(cs).get() & mask != 0) {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    })
    .await;

    enable_interrupt::<P>(false);
}

#[inline(always)]
fn set_trigger<P: Pin>(trigger: Trigger) {
    <P as super::sealed::Pin>::Port::set_trigger(P::number(), trigger);
}

#[inline(always)]
fn clear_pending<P: Pin>() {
    <P as super::sealed::Pin>::Port::clear_pending(P::number());
}

#[inline(always)]
fn enable_interrupt<P: Pin>(enabled: bool) {
    <P as super::sealed::Pin>::Port::enable_interrupt(P::number(), enabled);
}

/// Latch every pad that just interrupted and wake whoever waits for it.
fn dispatch<T: Instance>(cs: CriticalSection) {
    let pending = T::acknowledge_interrupts();
    if pending == 0 {
        return;
    }

    let port = T::id() as usize;
    let latch = TRIGGERED[port].borrow(cs);
    latch.set(latch.get() | pending);

    for (pin, waker) in PIN_WAKERS[port].iter().enumerate() {
        if pending & (1 << pin) != 0 {
            waker.wake();
        }
    }
}

/// Install one NVIC handler per bank.
///
/// `#[interrupt]` is not used here: its sanity check needs a module literally
/// named `interrupt` in scope, which collides with [`crate::interrupt`]. The
/// plain `#[no_mangle]` form is what the `PROVIDE` aliases in this crate's
/// `device.x` resolve against, and the const item keeps the compile time check
/// that the vector really exists.
macro_rules! port_handlers {
    ($($vector:ident: $port:ident),* $(,)?) => {
        $(
            const _: crate::interrupt::ExtendedInterrupt = crate::interrupt::$vector;

            #[no_mangle]
            pub unsafe extern "C" fn $vector() {
                critical_section::with(|cs| dispatch::<super::instance::$port>(cs));
            }
        )*
    };
}

port_handlers! {
    GPIOA: PortA,
    GPIOB: PortB,
    GPIOC: PortC,
    GPIOD: PortD,
    GPIOE: PortE,
    GPIOM: PortM,
    GPION: PortN,
}
