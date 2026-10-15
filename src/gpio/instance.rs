//! Register access behind [`super::Pin`].
//!
//! This is the only module of the GPIO driver that touches registers. Every
//! method is static because a port is not an object: the concrete peripheral is
//! selected by the type implementing the trait, so a call like
//! `PortA::set_function(3, 15)` monomorphises down to the register write alone.

use core::cell::Cell;

use critical_section::Mutex;

#[cfg(feature = "embassy")]
use super::Trigger;
use super::{Id, Level, OutputType, Pull, Speed};
use crate::clock::PeripheralClockIndex;
#[cfg(feature = "embassy")]
use crate::interrupt::ExtendedInterrupt;
use crate::pac;

/// Number of GPIO banks present on the chip: A, B, C, D, E, M and N.
pub(crate) const PORT_COUNT: usize = Id::N as usize + 1;

/// Pins per bank; the register layout fixes this at 16.
#[cfg(feature = "embassy")]
pub(crate) const PIN_COUNT: usize = 16;

/// Reference counted pin owners per bank, used to gate the port clock.
#[allow(clippy::declare_interior_mutable_const)]
const UNUSED: Mutex<Cell<u8>> = Mutex::new(Cell::new(0));
static PIN_USES: [Mutex<Cell<u8>>; PORT_COUNT] = [UNUSED; PORT_COUNT];

/// Word offset of `GPIOx.DATAPIN0` inside the register block.
const DATA_PIN_OFFSET: usize = 0x40 / core::mem::size_of::<u32>();

/// Sets one bit of a register that carries a single bit per pin.
macro_rules! put_bit {
    ($register:expr, $pin:expr, $value:expr) => {
        $register.modify(|r, w| unsafe {
            w.bits(
                (r.bits() & !(1u32 << ($pin as u32 & 0xF)))
                    | (u32::from($value) << ($pin as u32 & 0xF)),
            )
        })
    };
}

/// `pub` for the same reason as the bank types: it appears in the public `Pin`
/// trait's associated type while living in a crate-private module.
pub trait Instance {
    fn id() -> Id;
    fn gpio_block() -> &'static pac::gpioa::RegisterBlock;
    fn port_block() -> &'static pac::porta::RegisterBlock;
    fn clock() -> PeripheralClockIndex;
    #[cfg(feature = "embassy")]
    fn irq() -> ExtendedInterrupt;

    /// Count one pin owner and turn the port clock on for the first one.
    fn acquire() {
        critical_section::with(|cs| {
            let uses = PIN_USES[Self::id() as usize].borrow(cs);
            let count = uses.get();
            if count == 0 {
                Self::clock().open();
            }
            uses.set(count + 1);
        });
    }

    /// Forget one pin owner, closing the clock once the last one is gone.
    fn release() {
        critical_section::with(|cs| {
            let uses = PIN_USES[Self::id() as usize].borrow(cs);
            let count = uses.get() - 1;
            uses.set(count);
            if count == 0 {
                Self::clock().close();
            }
        });
    }

    /// Route pin output through the requested alternate function.
    fn set_function(pin: u8, function: u8) {
        let function = (function as u32 & 0xF) << ((pin as u32 % 8) * 4);
        let mask = !(0xF << ((pin as u32 % 8) * 4));
        let port = Self::port_block();
        match pin / 8 {
            0 => port
                .func0
                .modify(|r, w| unsafe { w.bits((r.bits() & mask) | function) }),
            _ => port
                .func1
                .modify(|r, w| unsafe { w.bits((r.bits() & mask) | function) }),
        }
        // `INEN` (digital input buffer) resets to 0; any alternate function that
        // receives data -- UART RX, SPI MISO, I2C SDA/SCL -- needs it on, or the
        // peripheral never samples the pad. Enabling it for output-only functions
        // is harmless, so just turn it on for every muxed pad.
        put_bit!(port.inen, pin, true);
    }

    fn set_direction(pin: u8, output: bool) {
        put_bit!(Self::gpio_block().dir, pin, output);
    }

    fn set_pull(pin: u8, pull: Pull) {
        let port = Self::port_block();
        put_bit!(port.pullu, pin, pull == Pull::Up);
        put_bit!(port.pulld, pin, pull == Pull::Down);
        // A pad used as a digital input only samples a level once its input
        // buffer is enabled, which is a separate bit from the direction.
        put_bit!(port.inen, pin, true);
    }

    fn set_output_options(pin: u8, output_type: OutputType, speed: Speed) {
        let port = Self::port_block();
        put_bit!(port.opend, pin, output_type == OutputType::OpenDrain);
        put_bit!(port.drvst, pin, speed == Speed::High);
    }

    /// Write a pin through its single-bit alias of the port data register.
    ///
    /// `GPIOx.DATAPIN{n}` accepts any word write and applies only bit 0 to pad
    /// `n`, which makes the update atomic without disturbing the rest of the
    /// bank. The PAC models the 16 aliases as distinct fields instead of an
    /// array, hence the address arithmetic.
    fn write(pin: u8, level: bool) {
        let alias = unsafe {
            (Self::gpio_block() as *const pac::gpioa::RegisterBlock as *mut u32)
                .add(DATA_PIN_OFFSET + (pin as usize & 0xF))
        };
        unsafe { core::ptr::write_volatile(alias, level as u32) };
    }

    fn set_level(pin: u8, level: Level) {
        Self::write(pin, level == Level::High);
    }

    fn read(pin: u8) -> bool {
        let alias = unsafe {
            (Self::gpio_block() as *const pac::gpioa::RegisterBlock as *mut u32)
                .add(DATA_PIN_OFFSET + (pin as usize & 0xF))
        };
        unsafe { core::ptr::read_volatile(alias) & 1 != 0 }
    }

    fn toggle(pin: u8) {
        Self::write(pin, !Self::read(pin));
    }

    #[cfg(feature = "embassy")]
    fn set_trigger(pin: u8, trigger: Trigger) {
        let gpio = Self::gpio_block();
        let level = matches!(trigger, Trigger::LevelHigh | Trigger::LevelLow);
        let both = matches!(trigger, Trigger::EdgeBoth);
        let rising = matches!(trigger, Trigger::LevelHigh | Trigger::EdgeRising);
        put_bit!(gpio.intlvltrg, pin, level);
        put_bit!(gpio.intbe, pin, both);
        put_bit!(gpio.intriseen, pin, rising);
    }

    #[cfg(feature = "embassy")]
    fn enable_interrupt(pin: u8, enabled: bool) {
        put_bit!(Self::gpio_block().inten, pin, enabled);
    }

    /// Drop a pending edge interrupt before arming it again.
    #[cfg(feature = "embassy")]
    fn clear_pending(pin: u8) {
        Self::gpio_block()
            .intclr
            .write(|w| unsafe { w.bits(1 << (pin as u32 & 0xF)) });
    }

    /// Allow the shared port vector through the NVIC.
    ///
    /// # Safety
    ///
    /// The vector's handler must be installed; see [`crate::vector_table`].
    #[cfg(feature = "embassy")]
    unsafe fn enable_port_interrupt() {
        Self::irq().enable();
    }

    /// Report and acknowledge the pins whose interrupt is currently pending.
    ///
    /// Sources are *disabled before* the flags are cleared: a level sensitive
    /// pin re-asserts immediately while its input stays at the triggering level,
    /// and clearing first would leave such a pin pending forever.
    #[cfg(feature = "embassy")]
    fn acknowledge_interrupts() -> u16 {
        let gpio = Self::gpio_block();
        let pending = gpio.intstat.read().bits() as u16;
        if pending != 0 {
            gpio.inten
                .modify(|r, w| unsafe { w.bits(r.bits() & !(pending as u32)) });
            gpio.intclr.write(|w| unsafe { w.bits(pending as u32) });
        }
        pending
    }
}

macro_rules! impl_port_instance {
    ($name:ident, $id:ident, $gpio:ident, $port:ident, $clock:ident, $irq:ident) => {
        /// Sealed access to one bank of pads.
        ///
        /// `pub` because it appears in an associated type of the public `Pin`
        /// trait; the surrounding module is crate-private, so downstream code
        /// still cannot name it.
        pub struct $name;

        impl Instance for $name {
            #[inline]
            fn id() -> Id {
                Id::$id
            }

            #[inline]
            fn gpio_block() -> &'static pac::gpioa::RegisterBlock {
                unsafe { &*pac::$gpio::PTR }
            }

            #[inline]
            fn port_block() -> &'static pac::porta::RegisterBlock {
                unsafe { &*pac::$port::PTR }
            }

            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::$clock
            }

            #[cfg(feature = "embassy")]
            #[inline]
            fn irq() -> ExtendedInterrupt {
                ExtendedInterrupt::$irq
            }
        }
    };
}

/// Route a pad of bank A while the GPIO driver may not even exist yet.
///
/// The clock driver needs PA3 and PA4 in their oscillator function before it can
/// bring the crystal up, which happens long before any pin object is taken.
pub(crate) fn set_port_a_function(pin: u8, function: u8) {
    PortA::set_function(pin, function);
}

impl_port_instance!(PortA, A, GPIOA, PORTA, GpioA, GpioA);
impl_port_instance!(PortB, B, GPIOB, PORTB, GpioB, GpioB);
impl_port_instance!(PortC, C, GPIOC, PORTC, GpioC, GpioC);
impl_port_instance!(PortD, D, GPIOD, PORTD, GpioD, GpioD);
impl_port_instance!(PortE, E, GPIOE, PORTE, GpioE, GpioE);
impl_port_instance!(PortM, M, GPIOM, PORTM, GpioM, GpioM);
impl_port_instance!(PortN, N, GPION, PORTN, GpioN, GpioN);
