//! Clock gating for the individual peripherals.
//!
//! The SWM341 has no AHB/APB enable registers keyed by bus ownership; instead a
//! flat pair of registers, `SYS.CLKEN0` and `SYS.CLKEN1`, carries one enable bit
//! per peripheral. Some bits are shared by a whole family (one `TIMR` bit for
//! TIMR0..=TIMR4, one `PWM` bit for PWM0..=PWM4, ...), which is why the
//! [`PeripheralClockIndex`] variants below are named after the *gate* rather
//! than after the singleton using it.

use super::sys;

/// One clock gate of a peripheral family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum PeripheralClockIndex {
    /// `GPIOA`/`PORTA` (remember that a pad is driven through both blocks).
    GpioA,
    GpioB,
    GpioC,
    GpioD,
    GpioE,
    GpioM,
    GpioN,
    Uart0,
    Uart1,
    Uart2,
    Uart3,
    Wdt,
    Timr,
    Pwm,
    Spi0,
    Spi1,
    I2c0,
    I2c1,
    Crc,
    Cordic,
    Div,
    Sdio,
    Usb,
    /// Analog control block. Must stay enabled to reach `HRCCR`, `PLLCR`,
    /// `XTALCR` and friends, which sit far away at `SYS + 0xAA000`.
    Anac,
    Adc0,
    Can0,
    Dma2d,
    Lcd,
    Sdram,
    Sfc,
    Adc1,
    Can1,
    Rtc,
    Iofilt,
    Btimr,
    Jpeg,
    Dac,
    Qei,
    /// Main DMA controller. The SVD does not model a dedicated clock gate, so the
    /// controller is treated as always-on (see [`impl_clock_gates!`]).
    Dma,
    /// HALL (hall-sensor interface, exposed through `TIMRG`). Like DMA, the SVD
    /// omits its clock gate, so it is treated as always-on.
    Hall,
    /// Analog comparator (ACMP). The SVD omits its clock gate (the registers
    /// live inside `SYS` alongside the always-on analog block), so it is
    /// treated as always-on.
    Acmp,
    /// Operational amplifiers (OPA0..OPA3). Like the comparators their only
    /// control (`SYS.OPACR`) lives inside the always-on analog `SYS` block and
    /// the SVD models no per-OPA gate, so they are treated as always-on.
    Opa,
}

/// Binds every index to the single bit that gates it.
///
/// Writing these against named SVD fields -- instead of hand-computed bit
/// positions -- is what keeps this table honest: a misplaced bit would enable an
/// unrelated peripheral and never fail to compile.
macro_rules! impl_clock_gates {
    (
        gated { $( $index:ident: $register:ident.$field:ident ),* $(,)? }
        always_on { $( $aon:ident ),* $(,)? }
    ) => {
        impl PeripheralClockIndex {
            /// Set the peripheral's clock gate.
            #[inline(always)]
            pub fn open(self) {
                match self {
                    $( Self::$index => sys().$register.modify(|_, w| w.$field().set_bit()), )*
                    $( Self::$aon => {}, )*
                }
            }

            /// Clear the peripheral's clock gate.
            #[inline(always)]
            pub fn close(self) {
                match self {
                    $( Self::$index => sys().$register.modify(|_, w| w.$field().clear_bit()), )*
                    $( Self::$aon => {}, )*
                }
            }

            // set or clear the peripheral's clock gate.
            pub fn set(self, gate: bool) {
                if gate {
                    self.open()
                }
                else {
                    self.close()
                }
            }
        }
    };
}

impl_clock_gates! {
    gated {
        GpioA: clken0.gpioa,
        GpioB: clken0.gpiob,
        GpioC: clken0.gpioc,
        GpioD: clken0.gpiod,
        GpioM: clken0.gpiom,
        GpioN: clken0.gpion,
        Uart0: clken0.uart0,
        Uart1: clken0.uart1,
        Uart2: clken0.uart2,
        Uart3: clken0.uart3,
        Wdt: clken0.wdt,
        Timr: clken0.timr,
        Pwm: clken0.pwm,
        Spi0: clken0.spi0,
        Spi1: clken0.spi1,
        I2c0: clken0.i2c0,
        I2c1: clken0.i2c1,
        Crc: clken0.crc,
        Cordic: clken0.cordic,
        Div: clken0.div,
        Sdio: clken0.sdio,
        Usb: clken0.usb,
        Anac: clken0.anac,
        Adc0: clken0.adc0,
        Can0: clken0.can0,
        Dma2d: clken0.dma2d,
        Lcd: clken0.lcd,
        GpioE: clken1.gpioe,
        Sdram: clken1.sdram,
        Sfc: clken1.sfc,
        Adc1: clken1.adc1,
        Can1: clken1.can1,
        Rtc: clken1.rtc,
        Iofilt: clken1.iofilt,
        Btimr: clken1.btimr,
        Jpeg: clken1.jpeg,
        Dac: clken1.dac,
        Qei: clken1.qei,
    }
    always_on {
        Dma,
        Hall,
        Acmp,
        Opa,
    }
}
