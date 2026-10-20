#![no_std]
// The `peripherals!` macro expands to one struct per pad plus a 120-field
// `Peripherals` struct; that expansion is large enough to bump the limit.
#![recursion_limit = "512"]
//! HAL for the Synwit SWM341 family.
//!
//! The chip is an Arm China STAR-MC1 core -- Armv8-M Mainline with FPU and MPU,
//! binary compatible with Cortex-M33 -- so the build target is
//! `thumbv8m.main-none-eabihf`.
//!
//! # Layers
//!
//! * [`mcu`] owns the peripheral singletons; [`init`] hands them out once.
//! * [`clock`] owns the clock tree and the per-peripheral clock gates.
//! * [`gpio`] owns pads. A pad is described by two register blocks (`PORTx` for
//!   the mux and pad options, `GPIOx` for data and interrupts), so the HAL takes
//!   the pad itself as the singleton.
//! * Driver types are generic over [`mode::Mode`]: `Blocking` busy-waits,
//!   `Async` awaits. The `embassy` feature enables the latter.
//!
//! # Interrupt vectors
//!
//! The SVD behind [`pac`] only describes NVIC vectors 0..=28 while the chip has
//! 91, so this crate ships its own vector table. See [`vector_table`] and
//! [`interrupt`].

pub use embassy_hal_internal::{Peripheral, PeripheralRef};
/// Raw register access crate backing this HAL.
pub use swm341_pac as pac;

pub mod adc;
pub mod can;
pub mod clock;
pub mod cordic;
pub mod dac;
pub mod dma;
#[cfg(feature = "embassy")]
pub mod embassy;
pub mod flash;
pub mod gpio;
pub mod hall;
pub mod i2c;
pub mod instance_base;
pub mod interrupt;
pub mod mcu;
pub mod mode;
pub mod pwm;
pub mod qei;
pub mod rtc;
pub mod spi;
pub mod timr;
pub mod uart;

// Newly added peripheral drivers, following the `uart`/`timr` five-file layout.
pub mod btimr;
pub mod cmp;
// Hardware CRC engine: no interrupt line, so no async flavour.
pub mod crc;
pub mod div;
pub mod dma2d;
pub mod jpeg;
pub mod lcdc;
pub mod opa;
// SDRAM controller. Like `dac` it has no NVIC vector, so its driver stops at
// the clock gate and the register block instead of using `InstanceBase`.
pub mod sdio;
pub mod sdramc;
pub mod sfc;
pub mod wdt;

// Core debug component (Armv8-M DWT), not a SWM341 peripheral: no clock gate,
// no NVIC vector -- switched on through the core's Debug Control Block.
pub mod dwt;

#[doc(hidden)]
pub mod vector_table;

pub use mcu::Peripherals;

// Serial peripherals are generated under `mcu::serial`; re-export them at the
// crate root so drivers can name them directly.
pub use mcu::devices::{
    Adc0,
    Adc1,
    Btimr0,
    Btimr1,
    Btimr10,
    Btimr11,
    Btimr2,
    Btimr3,
    Btimr4,
    Btimr5,
    Btimr6,
    Btimr7,
    Btimr8,
    Btimr9,
    // Analog comparators (CMP0/CMP1/CMP2).
    Cmp0,
    Cmp1,
    Cmp2,
    Cordic,
    // Hardware CRC engine.
    Crc,
    Dac,
    Div,
    Dma,
    Dma2d,
    // Data Watchpoint and Trace (Armv8-M core debug component).
    Dwt,
    Hall,
    Jpeg,
    Lcd,
    // On-chip operational amplifiers (OPA0..OPA3).
    Opa0,
    Opa1,
    Opa2,
    Opa3,
    Pwm0,
    Pwm1,
    Pwm2,
    Pwm3,
    Pwm4,
    // Quadrature encoder interface.
    Qei,
    Rtc,
    Sdio,
    // SDRAM controller (no NVIC vector, like `Dac`).
    Sdramc,
    // Serial Flash Controller (dedicated SPI-NOR master, pads D3..D8).
    Sfc,
    Timr0,
    Timr1,
    Timr2,
    Timr3,
    Timr4,
    Wdt,
};
pub use mcu::serial::{Can0, Can1, I2c0, I2c1, Spi0, Spi1, Uart0, Uart1, Uart2, Uart3};

impl Peripherals {
    /// Take the UART0 singleton.
    pub fn uart0(&self) -> Uart0 {
        unsafe { Uart0::steal() }
    }
    /// Take the UART1 singleton.
    pub fn uart1(&self) -> Uart1 {
        unsafe { Uart1::steal() }
    }
    /// Take the UART2 singleton.
    pub fn uart2(&self) -> Uart2 {
        unsafe { Uart2::steal() }
    }
    /// Take the UART3 singleton.
    pub fn uart3(&self) -> Uart3 {
        unsafe { Uart3::steal() }
    }
    /// Take the SPI0 singleton.
    pub fn spi0(&self) -> Spi0 {
        unsafe { Spi0::steal() }
    }
    /// Take the SPI1 singleton.
    pub fn spi1(&self) -> Spi1 {
        unsafe { Spi1::steal() }
    }
    /// Take the I2C0 singleton.
    pub fn i2c0(&self) -> I2c0 {
        unsafe { I2c0::steal() }
    }
    /// Take the I2C1 singleton.
    pub fn i2c1(&self) -> I2c1 {
        unsafe { I2c1::steal() }
    }
    /// Take the TIMR0 singleton.
    pub fn timr0(&self) -> Timr0 {
        unsafe { Timr0::steal() }
    }
    /// Take the TIMR1 singleton.
    pub fn timr1(&self) -> Timr1 {
        unsafe { Timr1::steal() }
    }
    /// Take the TIMR2 singleton.
    pub fn timr2(&self) -> Timr2 {
        unsafe { Timr2::steal() }
    }
    /// Take the TIMR3 singleton.
    pub fn timr3(&self) -> Timr3 {
        unsafe { Timr3::steal() }
    }
    /// Take the TIMR4 singleton.
    pub fn timr4(&self) -> Timr4 {
        unsafe { Timr4::steal() }
    }
    /// Take the PWM0 singleton.
    pub fn pwm0(&self) -> Pwm0 {
        unsafe { Pwm0::steal() }
    }
    /// Take the PWM1 singleton.
    pub fn pwm1(&self) -> Pwm1 {
        unsafe { Pwm1::steal() }
    }
    /// Take the PWM2 singleton.
    pub fn pwm2(&self) -> Pwm2 {
        unsafe { Pwm2::steal() }
    }
    /// Take the PWM3 singleton.
    pub fn pwm3(&self) -> Pwm3 {
        unsafe { Pwm3::steal() }
    }
    /// Take the PWM4 singleton.
    pub fn pwm4(&self) -> Pwm4 {
        unsafe { Pwm4::steal() }
    }
    /// Take the ADC0 singleton.
    pub fn adc0(&self) -> Adc0 {
        unsafe { Adc0::steal() }
    }
    /// Take the ADC1 singleton.
    pub fn adc1(&self) -> Adc1 {
        unsafe { Adc1::steal() }
    }
    /// Take the CAN0 singleton.
    pub fn can0(&self) -> Can0 {
        unsafe { Can0::steal() }
    }
    /// Take the CAN1 singleton.
    pub fn can1(&self) -> Can1 {
        unsafe { Can1::steal() }
    }
    /// Take the RTC singleton.
    pub fn rtc(&self) -> Rtc {
        unsafe { Rtc::steal() }
    }
    /// Take the CORDIC singleton.
    pub fn cordic(&self) -> Cordic {
        unsafe { Cordic::steal() }
    }
    /// Take the CRC (hardware CRC engine) singleton.
    pub fn crc(&self) -> Crc {
        unsafe { Crc::steal() }
    }
    /// Take the DMA singleton.
    pub fn dma(&self) -> Dma {
        unsafe { Dma::steal() }
    }
    /// Take the HALL singleton.
    pub fn hall(&self) -> Hall {
        unsafe { Hall::steal() }
    }
    /// Take the DIV (hardware divider) singleton.
    pub fn div(&self) -> Div {
        unsafe { Div::steal() }
    }
    /// Take the WDT (watchdog) singleton.
    pub fn wdt(&self) -> Wdt {
        unsafe { Wdt::steal() }
    }
    /// Take the BTIMR0 singleton.
    pub fn btimr0(&self) -> Btimr0 {
        unsafe { Btimr0::steal() }
    }
    /// Take the BTIMR1 singleton.
    pub fn btimr1(&self) -> Btimr1 {
        unsafe { Btimr1::steal() }
    }
    /// Take the BTIMR2 singleton.
    pub fn btimr2(&self) -> Btimr2 {
        unsafe { Btimr2::steal() }
    }
    /// Take the BTIMR3 singleton.
    pub fn btimr3(&self) -> Btimr3 {
        unsafe { Btimr3::steal() }
    }
    /// Take the BTIMR4 singleton.
    pub fn btimr4(&self) -> Btimr4 {
        unsafe { Btimr4::steal() }
    }
    /// Take the BTIMR5 singleton.
    pub fn btimr5(&self) -> Btimr5 {
        unsafe { Btimr5::steal() }
    }
    /// Take the BTIMR6 singleton.
    pub fn btimr6(&self) -> Btimr6 {
        unsafe { Btimr6::steal() }
    }
    /// Take the BTIMR7 singleton.
    pub fn btimr7(&self) -> Btimr7 {
        unsafe { Btimr7::steal() }
    }
    /// Take the BTIMR8 singleton.
    pub fn btimr8(&self) -> Btimr8 {
        unsafe { Btimr8::steal() }
    }
    /// Take the BTIMR9 singleton.
    pub fn btimr9(&self) -> Btimr9 {
        unsafe { Btimr9::steal() }
    }
    /// Take the BTIMR10 singleton.
    pub fn btimr10(&self) -> Btimr10 {
        unsafe { Btimr10::steal() }
    }
    /// Take the BTIMR11 singleton.
    pub fn btimr11(&self) -> Btimr11 {
        unsafe { Btimr11::steal() }
    }
    /// Take the SDIO singleton.
    pub fn sdio(&self) -> Sdio {
        unsafe { Sdio::steal() }
    }
    /// Take the DMA2D singleton.
    pub fn dma2d(&self) -> Dma2d {
        unsafe { Dma2d::steal() }
    }
    /// Take the JPEG singleton.
    pub fn jpeg(&self) -> Jpeg {
        unsafe { Jpeg::steal() }
    }
    /// Take the TFT LCD controller singleton.
    pub fn lcd(&self) -> Lcd {
        unsafe { Lcd::steal() }
    }
    /// Take the SDRAM controller singleton.
    pub fn sdramc(&self) -> Sdramc {
        unsafe { Sdramc::steal() }
    }
    /// Take the DAC singleton.
    pub fn dac(&self) -> Dac {
        unsafe { Dac::steal() }
    }
    /// Take the QEI (quadrature encoder interface) singleton.
    pub fn qei(&self) -> Qei {
        unsafe { Qei::steal() }
    }
    /// Take the DWT (Data Watchpoint and Trace) core component singleton.
    pub fn dwt(&self) -> Dwt {
        unsafe { Dwt::steal() }
    }
    /// Take the CMP0 (analog comparator 0) singleton.
    pub fn cmp0(&self) -> Cmp0 {
        unsafe { Cmp0::steal() }
    }
    /// Take the CMP1 (analog comparator 1) singleton.
    pub fn cmp1(&self) -> Cmp1 {
        unsafe { Cmp1::steal() }
    }
    /// Take the CMP2 (analog comparator 2) singleton.
    pub fn cmp2(&self) -> Cmp2 {
        unsafe { Cmp2::steal() }
    }
    /// Take the OPA0 (operational amplifier 0) singleton.
    pub fn opa0(&self) -> Opa0 {
        unsafe { Opa0::steal() }
    }
    /// Take the OPA1 (operational amplifier 1) singleton.
    pub fn opa1(&self) -> Opa1 {
        unsafe { Opa1::steal() }
    }
    /// Take the OPA2 (operational amplifier 2) singleton.
    pub fn opa2(&self) -> Opa2 {
        unsafe { Opa2::steal() }
    }
    /// Take the OPA3 (operational amplifier 3) singleton.
    pub fn opa3(&self) -> Opa3 {
        unsafe { Opa3::steal() }
    }
    /// Take the SFC (Serial Flash Controller) singleton.
    pub fn sfc(&self) -> Sfc {
        unsafe { Sfc::steal() }
    }
}

mod sealed {
    /// Prevents downstream crates from implementing marker traits of this HAL.
    pub trait Sealed {}
}

/// Options handed to [`init`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Config {
    /// Clock tree configuration.
    pub clock: clock::ClockConfig,
}

impl Config {
    /// Run the core at 150 MHz, the datasheet maximum, from the internal RC.
    pub fn hrc_150mhz() -> Self {
        Self {
            clock: clock::ClockConfig::hrc_150mhz(),
        }
    }
}

/// Take the peripherals and bring the chip into `config`.
///
/// # Panics
///
/// Panics when the requested clock cannot be produced, or when called twice --
/// the peripheral singletons must stay unique.
pub fn init(config: Config) -> Peripherals {
    clock::init(config.clock).expect("the requested clock configuration is invalid");

    #[cfg(feature = "embassy")]
    embassy::init_time_driver();

    Peripherals::take()
}
