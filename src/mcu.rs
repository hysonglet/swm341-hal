//! Peripheral singletons handed out by [`crate::init`].
//!
//! On the SWM341 a pad is described by two register blocks: `PORTx` owns the
//! function mux and the electrical pad options, `GPIOx` owns the data register
//! and the interrupt logic. Both are needed to drive any given pin, so the
//! singleton granularity chosen here is the **pin**, and a `PA0`-style name
//! grants exclusive access to that pad across both register blocks.
//!
//! The pin singletons (112 of them) live in the `peripherals!` call below. The
//! serial peripherals (UART/SPI/I2C) are generated in the [`serial`] submodule:
//! the `peripherals!` macro silently truncates its expansion past roughly 119
//! entries, so splitting keeps each invocation under that ceiling.

embassy_hal_internal::peripherals!(
    // GPIO banks A..E, M, N; the vendor pin numbers within each bank are 0..15.
    PA0, PA1, PA2, PA3, PA4, PA5, PA6, PA7, PA8, PA9, PA10, PA11, PA12, PA13, PA14, PA15, PB0, PB1,
    PB2, PB3, PB4, PB5, PB6, PB7, PB8, PB9, PB10, PB11, PB12, PB13, PB14, PB15, PC0, PC1, PC2, PC3,
    PC4, PC5, PC6, PC7, PC8, PC9, PC10, PC11, PC12, PC13, PC14, PC15, PD0, PD1, PD2, PD3, PD4, PD5,
    PD6, PD7, PD8, PD9, PD10, PD11, PD12, PD13, PD14, PD15, PE0, PE1, PE2, PE3, PE4, PE5, PE6, PE7,
    PE8, PE9, PE10, PE11, PE12, PE13, PE14, PE15, PM0, PM1, PM2, PM3, PM4, PM5, PM6, PM7, PM8, PM9,
    PM10, PM11, PM12, PM13, PM14, PM15, PN0, PN1, PN2, PN3, PN4, PN5, PN6, PN7, PN8, PN9, PN10,
    PN11, PN12, PN13, PN14, PN15,
);

/// UART / SPI / I2C singletons.
///
/// Kept in their own `peripherals_definition!` call so the macro does not hit its
/// expansion ceiling, and so the second `Peripherals` struct (and its
/// `#[no_mangle]` `take` guard) does not collide with the one above. The types
/// are re-exported at the crate root as [`crate::Uart0`] etc.
pub mod serial {
    embassy_hal_internal::peripherals_definition!(
        Uart0, Uart1, Uart2, Uart3, Spi0, Spi1, I2c0, I2c1, Can0, Can1,
    );

    pub use peripherals::*;
}

/// TIMR / PWM / ADC singletons.
///
/// Same split as [`serial`]: a separate `peripherals_definition!` call (no
/// colliding `Peripherals`/`take` symbol -- that macro only emits singleton
/// types). The types are re-exported at the crate root as [`crate::Timr0`] etc.
pub mod devices {
    embassy_hal_internal::peripherals_definition!(
        Timr0, Timr1, Timr2, Timr3, Timr4, Pwm0, Pwm1, Pwm2, Pwm3, Pwm4, Adc0, Adc1, Rtc, Cordic,
        Dma, Hall, // Hardware divider / square-root unit.
        Div, // Watchdog timer.
        Wdt, // Basic timers (12 instances, sharing one clock gate).
        Btimr0, Btimr1, Btimr2, Btimr3, Btimr4, Btimr5, Btimr6, Btimr7, Btimr8, Btimr9, Btimr10,
        Btimr11, // SDIO / SD-memory host controller.
        Sdio, // 2D DMA / graphics accelerator.
        Dma2d, // JPEG decoder.
        Jpeg, // TFT LCD controller.
        Lcd, // SDRAM controller (the only SWM341 peripheral with no NVIC vector).
        Sdramc, // 12-bit digital-to-analog converter.
        Dac, // Quadrature encoder interface (QEA/QEB/INDEX on PD4/PD3/PD2).
        Qei, // Analog comparators (CMP0/CMP1/CMP2, sharing the `ACMP` interrupt).
        Cmp0, Cmp1, Cmp2,
        // On-chip operational amplifiers (power control in `SYS.OPACR`).
        Opa0, Opa1, Opa2, Opa3,
        // Data Watchpoint and Trace (Armv8-M core debug component).
        Dwt, // Serial Flash Controller (dedicated SPI-NOR master, pads D3..D8).
        Sfc, // Hardware CRC engine.
        Crc,
    );

    pub use peripherals::*;
}
