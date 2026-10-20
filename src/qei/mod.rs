//! QEI (quadrature encoder interface) driver.
//!
//! The SWM341 embeds one quadrature decoder that turns the A/B/Z phases of an
//! incremental encoder into a signed 16-bit position. It counts in ×2 or ×4
//! resolution, rejects signals that jitter at a direction reversal, and resets
//! the position counter either on the index (Z) pulse or when the count matches
//! `MAXCNT`. The raw interrupt status covers the index reset, the count match,
//! counter overflow/underflow and illegal A/B transitions.
//!
//! The three phase pads all live on port D at function code `1` (see
//! [`pins`]): `PD4` = phase A, `PD3` = phase B, `PD2` = index (Z), and `PD7` can
//! output the count direction (`UPDN`). A and B are handed to [`Qei::new`],
//! which muxes them and keeps them routed for as long as the decoder lives.
//! The direction has no status bit in the register block, so without routing
//! `PD7` it is inferred from two successive [`Qei::count`] reads.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! use hal::qei::{Config, CountMode, Interrupt, Interrupts, Qei, ResetMode};
//!
//! // The pads are arguments of `new`: it routes them and owns them.
//! let mut qei = Qei::new(
//!     p.qei(),
//!     p.PD4, // phase A
//!     p.PD3, // phase B
//!     p.PD2, // index (Z)
//!     Config {
//!         count_mode: CountMode::X4,
//!         reset_mode: ResetMode::Index,
//!         max_count: 3999, // lines-per-revolution minus one
//!         interrupts: Interrupts::NONE,
//!         enable: true,
//!         ..Default::default()
//!     },
//! );
//!
//! let position = qei.count();
//! if qei.is_index_present() {
//!     qei.clear_flag(Interrupt::Index);
//! }
//! let _ = position;
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::Flex;
use instance::Instance;

pub mod instance;
pub mod pins;
mod types;

pub use types::*;

/// A pad usable as the quadrature decoder's phase A input.
///
/// [`FUNC`](QeiAPin::FUNC) is the 4-bit `PORTx.FUNC` code (`1` on port D) that
/// selects `QEA_IN`, so [`Qei::new`] can route a pad from its type alone.
pub trait QeiAPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting `QEA_IN`.
    const FUNC: u8;
}

/// A pad usable as the quadrature decoder's phase B input.
pub trait QeiBPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting `QEB_IN`.
    const FUNC: u8;
}

/// A pad usable as the quadrature decoder's index (Z) input.
pub trait QeiIndexPin<T: Instance>: crate::gpio::Pin {
    /// `PORTx.FUNC` code selecting `INDEX_IN`.
    const FUNC: u8;
}

macro_rules! qei_pins {
    ($trait:ident => $($pin:ty => $peri:ty),* $(,)?) => {
        $( impl $trait<$peri> for $pin { const FUNC: u8 = 1; } )*
    };
}

// Transcribed from the vendor pin matrix (see `pins`): each phase has exactly
// one pad on port D, all at function code 1.
qei_pins!(QeiAPin     => crate::gpio::PD4 => crate::Qei);
qei_pins!(QeiBPin     => crate::gpio::PD3 => crate::Qei);
qei_pins!(QeiIndexPin => crate::gpio::PD2 => crate::Qei);

/// A QEI decoder driven by its peripheral clock, owning its three phase pads.
#[allow(dead_code)]
pub struct Qei<'d, T: Instance, A: QeiAPin<T>, B: QeiBPin<T>, I: QeiIndexPin<T>> {
    peri: PeripheralRef<'d, T>,
    _phase_a: Flex<'d, A>,
    _phase_b: Flex<'d, B>,
    _index: Flex<'d, I>,
}

impl<'d, T: Instance, A: QeiAPin<T>, B: QeiBPin<T>, I: QeiIndexPin<T>> Qei<'d, T, A, B, I> {
    /// Take ownership of the QEI peripheral and its three phase pads.
    ///
    /// The pads are muxed to their QEI function here (and their digital input
    /// buffers switched on), so the decoder can be used straight away; they stay
    /// routed until this handle is dropped.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        phase_a: impl Peripheral<P = A> + 'd,
        phase_b: impl Peripheral<P = B> + 'd,
        index: impl Peripheral<P = I> + 'd,
        config: Config,
    ) -> Self {
        into_ref!(peri);

        // Take the pads first: `Flex::new` turns the bank clock on and marks
        // each pad in use, then the mux is set from the pad's own `FUNC`.
        let _phase_a = Flex::new(phase_a);
        let _phase_b = Flex::new(phase_b);
        let _index = Flex::new(index);
        crate::gpio::set_function::<A>(A::FUNC);
        crate::gpio::set_function::<B>(B::FUNC);
        crate::gpio::set_function::<I>(I::FUNC);

        T::clock().open();
        T::configure(&config);
        Self {
            peri,
            _phase_a,
            _phase_b,
            _index,
        }
    }

    /// Start decoding A/B edges (`CR.ENA`).
    pub fn enable(&mut self) {
        T::set_enabled(true);
    }

    /// Stop the decoder; the position counter keeps its value (`CR.ENA`).
    pub fn disable(&mut self) {
        T::set_enabled(false);
    }

    /// Whether the decoder is enabled.
    pub fn is_enabled(&self) -> bool {
        T::is_enabled()
    }

    /// Freeze the module in idle mode (`CR.PAUSE`).
    pub fn pause(&mut self) {
        T::set_paused(true);
    }

    /// Leave idle mode and resume decoding.
    pub fn resume(&mut self) {
        T::set_paused(false);
    }

    /// Whether the module is paused (`CR.PAUSE`).
    pub fn is_paused(&self) -> bool {
        T::is_paused()
    }

    /// Swap the A and B phase inputs (`CR.ABSWAP`), reversing count direction.
    pub fn set_swap_ab(&mut self, swap: bool) {
        T::set_swap_ab(swap);
    }

    /// Current 16-bit position count (`POSCNT`), signed by the count direction.
    pub fn count(&self) -> u16 {
        T::count()
    }

    /// Load the position counter directly (`POSCNT`).
    ///
    /// The datasheet describes `POSCNT` as read-only -- the counter normally
    /// resets itself through the index phase or a count match -- but the SVD
    /// models the register as writable; prefer the hardware reset sources and
    /// treat this as a debug aid.
    pub fn set_count(&mut self, value: u16) {
        T::set_count(value);
    }

    /// Read the maximum-count / reverse-wrap reload (`MAXCNT`).
    pub fn max_count(&self) -> u16 {
        T::max_count()
    }

    /// Program the maximum-count / reverse-wrap reload (`MAXCNT`).
    pub fn set_max_count(&mut self, value: u16) {
        T::set_max_count(value);
    }

    /// Current level of the index (Z) pad (`CR.INDEX`, read-only).
    pub fn is_index_present(&self) -> bool {
        T::index_level()
    }

    /// Arm/disarm one interrupt source in the peripheral (`IE`).
    ///
    /// The source also has to be unmasked (see
    /// [`unmask_interrupt`](Self::unmask_interrupt)) and the NVIC `QEI` vector
    /// unmasked for the CPU to take the interrupt.
    pub fn enable_interrupt(&mut self, source: Interrupt, enable: bool) {
        T::set_interrupt_enabled(source, enable);
    }

    /// Mask (`false`) or unmask (`true`, the reset default) one source (`IM`).
    pub fn unmask_interrupt(&mut self, source: Interrupt, unmask: bool) {
        T::set_interrupt_unmasked(source, unmask);
    }

    /// Whether `source` has a pending event (`IF`).
    pub fn is_flag(&self, source: Interrupt) -> bool {
        T::is_flag(source)
    }

    /// Whether an event was lost for `source` because it stayed pending (`IFOV`).
    pub fn is_overflow(&self, source: Interrupt) -> bool {
        T::is_overflow(source)
    }

    /// Clear the pending flag of `source` (`IC`, write-1-to-clear).
    pub fn clear_flag(&mut self, source: Interrupt) {
        T::clear_flag(source);
    }
}

impl<'d, T: Instance, A: QeiAPin<T>, B: QeiBPin<T>, I: QeiIndexPin<T>> Drop
    for Qei<'d, T, A, B, I>
{
    fn drop(&mut self) {
        T::set_enabled(false);
        T::clock().close();
        // The three phase pads are released by their own `Flex` drops.
    }
}
