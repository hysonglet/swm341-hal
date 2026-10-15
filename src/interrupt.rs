//! NVIC interrupt vectors.
//!
//! The SVD that `swm341-pac` is generated from only describes the vectors
//! 0..=28, while the silicon implements 0..=88 (see the vendor header
//! `CSL/CMSIS/DeviceSupport/SWM341.h`). The vectors above index 28 therefore
//! have no `pac::Interrupt` variant and no weak alias in the PAC's `device.x`.
//!
//! This crate closes that gap with a single unified [`ExtendedInterrupt`]
//! enumeration that lists *all* 89 vectors (0..=88) with their real NVIC index
//! as the discriminant. Every vector also gets an upper-case datasheet alias
//! (`UART0`, `GPIOA`, ...), so name checks and the `device.x` weak aliases
//! resolve against one type. Because `ExtendedInterrupt` implements
//! [`InterruptNumber`] and carries `enable`/`disable` helpers, drivers return it
//! directly from `Instance::irq()` and toggle the NVIC with
//! `T::irq().enable()` / `T::irq().disable()`.
//!
//! Weak aliases for all 89 vectors are emitted by this crate's own `device.x`
//! (the PAC `rt` feature is left off to avoid a second, conflicting
//! `__INTERRUPTS`).

pub use cortex_m::interrupt::InterruptNumber;

use cortex_m::peripheral::NVIC;

/// All 89 NVIC vectors of the SWM341, indexed by their real NVIC number.
///
/// The discriminant of every variant is the NVIC index used by the hardware
/// and by [`crate::vector_table`]. Vectors 0..=28 are the ones the SVD
/// describes (spelled to match the datasheet, which differs from the SVD for
/// several entries: `PWM0` vs `PWM_CH0`, `ADC0` vs `SARADC0`, `TIMR2` vs
/// `TIMER2`, ...); vectors 29..=88 are the ones the SVD omits entirely.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
#[non_exhaustive]
pub enum ExtendedInterrupt {
    // --- vectors 0..=28 (described by the SVD) ---
    Uart0 = 0,
    Uart1 = 1,
    Timr0 = 2,
    Timr1 = 3,
    Dma = 4,
    Spi0 = 5,
    Pwm0 = 6,
    Wdt = 7,
    Uart2 = 8,
    Pwm1 = 9,
    Adc0 = 10,
    Btimr0 = 11,
    Hall0 = 12,
    Pwm2 = 13,
    PwmBrk = 14,
    I2c0 = 15,
    Can0 = 16,
    Spi1 = 17,
    Rtc = 18,
    Pwm3 = 19,
    Timr2 = 20,
    Uart3 = 21,
    Timr3 = 22,
    Adc1 = 23,
    Bod = 24,
    Cordic = 25,
    Btimr1 = 26,
    Pwm4 = 27,
    Hall3 = 28,
    // --- vectors 29..=88 (missing from the SVD) ---
    Btimr2 = 29,
    I2c1 = 30,
    Btimr3 = 31,
    Acmp = 32,
    XtalStop = 33,
    Fspi = 34,
    GpioA = 35,
    GpioB = 36,
    GpioC = 37,
    GpioD = 38,
    GpioM = 39,
    GpioN = 40,
    GpioA0 = 41,
    GpioA1 = 42,
    GpioA5 = 43,
    GpioA6 = 44,
    GpioA10 = 45,
    GpioA11 = 46,
    GpioA12 = 47,
    GpioA13 = 48,
    GpioB0 = 49,
    GpioB1 = 50,
    GpioB2 = 51,
    GpioC0 = 52,
    GpioC1 = 53,
    GpioC2 = 54,
    GpioC3 = 55,
    GpioC4 = 56,
    GpioD3 = 57,
    GpioD4 = 58,
    GpioD5 = 59,
    GpioD6 = 60,
    GpioD7 = 61,
    GpioD8 = 62,
    GpioC9 = 63,
    GpioC10 = 64,
    GpioC11 = 65,
    GpioC12 = 66,
    GpioM0 = 67,
    GpioM1 = 68,
    GpioM2 = 69,
    GpioM3 = 70,
    GpioM4 = 71,
    Div = 72,
    Lcd = 73,
    GpioE = 74,
    Jpeg = 75,
    Sdio = 76,
    Usb = 77,
    Can1 = 78,
    Timr4 = 79,
    Btimr4 = 80,
    Btimr5 = 81,
    Btimr6 = 82,
    Btimr7 = 83,
    Btimr8 = 84,
    Btimr9 = 85,
    Btimr10 = 86,
    Btimr11 = 87,
    Dma2d = 88,
}

unsafe impl InterruptNumber for ExtendedInterrupt {
    #[inline(always)]
    fn number(self) -> u16 {
        self as u16
    }
}

impl ExtendedInterrupt {
    /// NVIC index of this vector.
    #[inline]
    pub fn number(self) -> u16 {
        self as u16
    }

    /// Allow this vector to preempt the current execution.
    ///
    /// # Safety
    ///
    /// The handler must already be installed in the vector table and it
    /// must be able to deal with the peripheral state it is handed.
    #[inline]
    pub unsafe fn enable(self) {
        NVIC::unmask(self);
    }

    /// Stop this vector from preempting the current execution.
    #[inline]
    pub fn disable(self) {
        NVIC::mask(self);
    }
}

/// Datasheet-spelled aliases for every [`ExtendedInterrupt`] variant, so that
/// name checks (`interrupt::<NAME>`) and the `device.x` weak aliases resolve
/// against the canonical vector name.
macro_rules! interrupt_aliases {
    ($( ($alias:ident, $variant:ident, $number:literal, $doc:literal), )+) => {
        $(
            #[doc = concat!("Alias for `ExtendedInterrupt::", stringify!($variant), "` (NVIC ", stringify!($number), ").")]
            pub const $alias: ExtendedInterrupt = ExtendedInterrupt::$variant;
        )+
    };
}

interrupt_aliases! {
    (UART0, Uart0, 0, "UART0."),
    (UART1, Uart1, 1, "UART1."),
    (TIMR0, Timr0, 2, "TIMR0."),
    (TIMR1, Timr1, 3, "TIMR1."),
    (DMA, Dma, 4, "DMA."),
    (SPI0, Spi0, 5, "SPI0."),
    (PWM0, Pwm0, 6, "PWM0."),
    (WDT, Wdt, 7, "WDT."),
    (UART2, Uart2, 8, "UART2."),
    (PWM1, Pwm1, 9, "PWM1."),
    (ADC0, Adc0, 10, "ADC0."),
    (BTIMR0, Btimr0, 11, "BTIMR0."),
    (HALL0, Hall0, 12, "HALL0."),
    (PWM2, Pwm2, 13, "PWM2."),
    (PWMBRK, PwmBrk, 14, "PWMBRK."),
    (I2C0, I2c0, 15, "I2C0."),
    (CAN0, Can0, 16, "CAN0."),
    (SPI1, Spi1, 17, "SPI1."),
    (RTC, Rtc, 18, "RTC."),
    (PWM3, Pwm3, 19, "PWM3."),
    (TIMR2, Timr2, 20, "TIMR2."),
    (UART3, Uart3, 21, "UART3."),
    (TIMR3, Timr3, 22, "TIMR3."),
    (ADC1, Adc1, 23, "ADC1."),
    (BOD, Bod, 24, "BOD."),
    (CORDIC, Cordic, 25, "CORDIC."),
    (BTIMR1, Btimr1, 26, "BTIMR1."),
    (PWM4, Pwm4, 27, "PWM4."),
    (HALL3, Hall3, 28, "HALL3."),
    (BTIMR2, Btimr2, 29, "Basic timer 2."),
    (I2C1, I2c1, 30, "I2C1."),
    (BTIMR3, Btimr3, 31, "Basic timer 3."),
    (ACMP, Acmp, 32, "Analog comparator."),
    (XTALSTOP, XtalStop, 33, "Crystal oscillator stopped."),
    (FSPI, Fspi, 34, "Serial flash (SFC) interface."),
    (GPIOA, GpioA, 35, "GPIO port A."),
    (GPIOB, GpioB, 36, "GPIO port B."),
    (GPIOC, GpioC, 37, "GPIO port C."),
    (GPIOD, GpioD, 38, "GPIO port D."),
    (GPIOM, GpioM, 39, "GPIO port M."),
    (GPION, GpioN, 40, "GPIO port N."),
    (GPIOA0, GpioA0, 41, "GPIO port A pin 0."),
    (GPIOA1, GpioA1, 42, "GPIO port A pin 1."),
    (GPIOA5, GpioA5, 43, "GPIO port A pin 5."),
    (GPIOA6, GpioA6, 44, "GPIO port A pin 6."),
    (GPIOA10, GpioA10, 45, "GPIO port A pin 10."),
    (GPIOA11, GpioA11, 46, "GPIO port A pin 11."),
    (GPIOA12, GpioA12, 47, "GPIO port A pin 12."),
    (GPIOA13, GpioA13, 48, "GPIO port A pin 13."),
    (GPIOB0, GpioB0, 49, "GPIO port B pin 0."),
    (GPIOB1, GpioB1, 50, "GPIO port B pin 1."),
    (GPIOB2, GpioB2, 51, "GPIO port B pin 2."),
    (GPIOC0, GpioC0, 52, "GPIO port C pin 0."),
    (GPIOC1, GpioC1, 53, "GPIO port C pin 1."),
    (GPIOC2, GpioC2, 54, "GPIO port C pin 2."),
    (GPIOC3, GpioC3, 55, "GPIO port C pin 3."),
    (GPIOC4, GpioC4, 56, "GPIO port C pin 4."),
    (GPIOD3, GpioD3, 57, "GPIO port D pin 3."),
    (GPIOD4, GpioD4, 58, "GPIO port D pin 4."),
    (GPIOD5, GpioD5, 59, "GPIO port D pin 5."),
    (GPIOD6, GpioD6, 60, "GPIO port D pin 6."),
    (GPIOD7, GpioD7, 61, "GPIO port D pin 7."),
    (GPIOD8, GpioD8, 62, "GPIO port D pin 8."),
    (GPIOC9, GpioC9, 63, "GPIO port C pin 9."),
    (GPIOC10, GpioC10, 64, "GPIO port C pin 10."),
    (GPIOC11, GpioC11, 65, "GPIO port C pin 11."),
    (GPIOC12, GpioC12, 66, "GPIO port C pin 12."),
    (GPIOM0, GpioM0, 67, "GPIO port M pin 0."),
    (GPIOM1, GpioM1, 68, "GPIO port M pin 1."),
    (GPIOM2, GpioM2, 69, "GPIO port M pin 2."),
    (GPIOM3, GpioM3, 70, "GPIO port M pin 3."),
    (GPIOM4, GpioM4, 71, "GPIO port M pin 4."),
    (DIV, Div, 72, "Hardware divider."),
    (LCD, Lcd, 73, "TFT LCD controller."),
    (GPIOE, GpioE, 74, "GPIO port E."),
    (JPEG, Jpeg, 75, "JPEG decoder."),
    (SDIO, Sdio, 76, "SDIO interface."),
    (USB, Usb, 77, "USB device/host."),
    (CAN1, Can1, 78, "CAN1."),
    (TIMR4, Timr4, 79, "Timer 4."),
    (BTIMR4, Btimr4, 80, "Basic timer 4."),
    (BTIMR5, Btimr5, 81, "Basic timer 5."),
    (BTIMR6, Btimr6, 82, "Basic timer 6."),
    (BTIMR7, Btimr7, 83, "Basic timer 7."),
    (BTIMR8, Btimr8, 84, "Basic timer 8."),
    (BTIMR9, Btimr9, 85, "Basic timer 9."),
    (BTIMR10, Btimr10, 86, "Basic timer 10."),
    (BTIMR11, Btimr11, 87, "Basic timer 11."),
    (DMA2D, Dma2d, 88, "2D DMA engine."),
}
