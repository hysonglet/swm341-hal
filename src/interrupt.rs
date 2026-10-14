//! NVIC interrupt vectors.
//!
//! The SVD that `swm341-pac` is generated from only describes the vectors
//! 0..=28, while the silicon implements 0..=88 (see the vendor header
//! `CSL/CMSIS/DeviceSupport/SWM341.h`). Everything above index 28 therefore has
//! no `pac::Interrupt` variant and no weak alias in the PAC's `device.x`.
//!
//! This module closes that gap:
//!
//! * the vectors 0..=28 are re-exported from the PAC, unchanged;
//! * the missing ones are listed in [`ExtendedInterrupt`], together with an
//!   upper-case alias per vector so that `#[interrupt]`'s sanity check
//!   (`interrupt::<NAME>`) resolves for e.g. the GPIO port handlers;
//! * weak aliases for *all* 89 vectors are emitted by this crate's own
//!   `device.x`, which is why the `rt` feature of the PAC is not enabled.

pub use cortex_m::interrupt::InterruptNumber;

use cortex_m::peripheral::NVIC;

/// Aliases for the 29 vectors the PAC does know about.
///
/// Deliberately spelled the way the datasheet and `SWM341.h` spell them, which
/// differs from the SVD for several entries (`PWM0` vs `PWM_CH0`, `ADC0` vs
/// `SARADC0`, `TIMR2` vs `TIMER2`, ...). Only the datasheet spelling has a slot
/// in [`crate::vector_table`], so importing the PAC's variants here would make
/// `#[interrupt] fn PWM_CH0()` compile while silently never being installed.
macro_rules! pac_vectors {
    ($($alias:ident = $variant:ident,)+) => {
        $(
            #[doc = concat!("NVIC alias matching the vector named `", stringify!($alias), "` in the datasheet.")]
            pub const $alias: crate::pac::Interrupt = crate::pac::Interrupt::$variant;
        )+
    };
}

pac_vectors! {
    UART0 = UART0,
    UART1 = UART1,
    TIMR0 = TIMR0,
    TIMR1 = TIMR1,
    DMA = DMA,
    SPI0 = SPI0,
    PWM0 = PWM_CH0,
    WDT = WDT,
    UART2 = UART2,
    PWM1 = PWM_CH1,
    ADC0 = SARADC0,
    BTIMR0 = BTIMER0,
    HALL0 = HALL0,
    PWM2 = PWM_CH2,
    PWMBRK = PWM_HALT,
    I2C0 = I2C0,
    CAN0 = CAN0,
    SPI1 = SPI1,
    RTC = RTC_BASE,
    PWM3 = PWM_CH3,
    TIMR2 = TIMER2,
    UART3 = UART3,
    TIMR3 = TIMER3,
    ADC1 = SARADC1,
    BOD = BOD,
    CORDIC = CORDIC,
    BTIMR1 = BTIMER1,
    PWM4 = PWM_CH4,
    HALL3 = HALL1,
}

macro_rules! extended_vectors {
    ($( ($alias:ident, $variant:ident, $number:literal, $doc:literal), )+) => {
        /// NVIC vectors 29..=88 that the PAC's SVD is missing.
        ///
        /// The numeric discriminant of every variant is the real NVIC index.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        #[repr(u16)]
        #[non_exhaustive]
        pub enum ExtendedInterrupt {
            $(
                #[doc = $doc]
                $variant = $number,
            )+
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

        $(
            #[doc = concat!("Alias for `ExtendedInterrupt::", stringify!($variant), "` (NVIC ", stringify!($number), ").")]
            pub const $alias: ExtendedInterrupt = ExtendedInterrupt::$variant;
        )+
    };
}

extended_vectors! {
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
