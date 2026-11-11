//! I2C bus master (controller) mode.
//!
//! An [`I2c`] drives SCL and SDA, generates START / STOP and talks to slaves on
//! the bus. Two API styles are provided:
//!
//! * **Embedded-hal traits** — [`I2c`] implements `embedded_hal::i2c::I2c` (and,
//!   with the `embassy` feature, `embedded_hal_async::i2c::I2c`) so it drops into
//!   any generic driver. The `transaction` calls frame each operation strictly:
//!   one START opens the transfer, a (repeated) START separates operations, and a
//!   single STOP ends it.
//! * **Convenience methods** — [`I2c::blocking_write`], [`I2c::blocking_read`] and
//!   [`I2c::blocking_write_read`] (plus their async siblings) cover the common
//!   single-operation cases without building an `Operation` slice.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut i2c = hal::i2c::I2c::new(
//!     p.i2c0(),
//!     p.PC2,  // SCL
//!     p.PC1,  // SDA
//!     hal::i2c::Config::default(),
//! );
//! i2c.blocking_write(0x50, &[0x00, 0x10]).unwrap();
//! let mut data = [0u8; 2];
//! i2c.blocking_read(0x50, &mut data).unwrap();
//! ```
//!
//! The SCL/SDA `PORTx.FUNC` codes come from the pin types and the I2C instance
//! via [`I2cSclPin`]/[`I2cSdaPin`], so no alternate-function value is needed.

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use crate::gpio::{
    read_level, set_direction, set_function, set_i2c_pad, set_output_options, set_pull, write_level,
    Level, OutputType, Pin, Pull, Speed,
};
use crate::mode::{Blocking, Mode};

use super::instance::Instance;
use super::{Config, Error, I2cSclPin, I2cSdaPin};

#[cfg(feature = "embassy")]
use crate::mode::Async;

#[cfg(feature = "embassy")]
use super::future;

/// An I2C master with its two pads.
#[allow(dead_code)]
pub struct I2c<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode = Blocking> {
    peri: PeripheralRef<'d, T>,
    scl: PeripheralRef<'d, Scl>,
    sda: PeripheralRef<'d, Sda>,
    _mode: core::marker::PhantomData<M>,
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> I2c<'d, T, Scl, Sda, M> {
    /// Build an I2C master from its peripheral and the two pads.
    ///
    /// The SCL/SDA `PORTx.FUNC` codes are derived from `scl`/`sda` and the I2C
    /// instance `T` via [`I2cSclPin`]/[`I2cSdaPin`], so only the pins themselves
    /// are needed; a pad that cannot serve the requested role is a compile
    /// error. This is the shared constructor used by [`I2c::new`] and
    /// [`I2c::new_async`].
    pub(crate) fn build(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        into_ref!(peri, scl, sda);

        T::clock().open();
        crate::gpio::acquire::<Scl>();
        crate::gpio::acquire::<Sda>();
        crate::gpio::set_function::<Scl>(<Scl as I2cSclPin<T>>::FUNC);
        crate::gpio::set_function::<Sda>(<Sda as I2cSdaPin<T>>::FUNC);
        crate::gpio::set_i2c_pad::<Scl>();
        crate::gpio::set_i2c_pad::<Sda>();

        // Registers must be programmed while EN = 0 (datasheet master init
        // flow). enable_master() also clears stale interrupt flags.
        T::disable();
        T::set_clock(config.frequency);
        T::enable_master();

        Self {
            peri,
            scl,
            sda,
            _mode: core::marker::PhantomData,
        }
    }

    /// Wait until the hardware clears the current MCR command (START, a byte
    /// or STOP), aborting on a bus fault or a stuck line.
    fn wait_cmd() -> Result<(), Error> {
        let mut timeout = TIMEOUT;
        loop {
            T::check_faults()?;
            if T::cmd_done() {
                return Ok(());
            }
            timeout -= 1;
            if timeout == 0 {
                return Err(Error::Timeout);
            }
        }
    }

    /// Send a (repeated) START followed by the 7-bit address / R-W byte and
    /// verify the slave acknowledged it.
    fn start_addr(address: u8, read: bool) -> Result<(), Error> {
        T::txdata((address << 1) | u8::from(read));
        T::start();
        Self::wait_cmd()?;
        if !T::acked() {
            return Err(Error::NoAcknowledge);
        }
        Ok(())
    }

    /// Transmit one data byte and verify its ACK.
    fn write_byte(byte: u8) -> Result<(), Error> {
        T::txdata(byte);
        T::request_write();
        Self::wait_cmd()?;
        if !T::acked() {
            return Err(Error::NoAcknowledge);
        }
        Ok(())
    }

    /// Receive one byte; `nack` makes the master NACK it (last byte of a read
    /// phase) instead of ACKing it.
    fn read_byte(nack: bool) -> Result<u8, Error> {
        T::request_read(nack);
        Self::wait_cmd()?;
        Ok(T::rx_byte())
    }

    /// Terminate the transfer with STOP and wait until the bus is idle.
    ///
    /// 注意：即使 `wait_cmd` 超时也**必须**清掉标志位并返回 Ok——否则故障标志会残留，
    /// 让后续每一次传输都立刻报错、总线再也无法恢复。`let _ = Self::finish();`
    /// 的调用方本就忽略返回值，所以这里以“尽力恢复”为准则。
    fn finish() -> Result<(), Error> {
        T::stop();
        let _ = Self::wait_cmd();
        Self::wait_bus_free();
        T::clear_flags();
        Ok(())
    }

    /// Spin until the bus returns to idle.
    fn wait_bus_free() {
        let mut timeout = TIMEOUT;
        while !T::bus_free() {
            timeout -= 1;
            if timeout == 0 {
                break;
            }
        }
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Blocking> {
    /// Build a blocking I2C master. See [`I2c::build`] for the pad wiring.
    pub fn new(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        Self::build(peri, scl, sda, config)
    }

    /// 总线锁死后的恢复（标准 I2C “bus clear”）。
    ///
    /// 若 GT911 在半字节传输途中被中断而没收到 STOP，它会长久把 SDA 拉低（继续等时钟）。
    /// 此时控制器再发 STOP 也解不开——`disable()` 只会让 MCU 释放 SCL，SDA 仍被从设备
    /// 拉住，于是之后每笔传输都超时、UI 直接卡死。唯一办法是手动给 SCL 补几个时钟脉冲，
    /// 让从设备把剩余位走完并释放 SDA，再重新初始化控制器。
    ///
    /// 全程在 `&mut self` 上完成，无需重新申请 GPIO 引脚。每次 I2C 事务返回 `Err` 后调一次
    /// 即可让总线在下一帧恢复正常。
    pub fn reset(&mut self)
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        // 1) 把 SCL/SDA 临时切回 GPIO：SCL 当开漏输出由 MCU 驱动，SDA 当输入让从设备
        //    自由驱动，以便观测它何时释放 SDA。
        set_function::<Scl>(0); // 0 = GPIO
        set_output_options::<Scl>(OutputType::OpenDrain, Speed::Normal);
        set_pull::<Scl>(Pull::Up);
        set_direction::<Scl>(true);

        set_function::<Sda>(0);
        set_pull::<Sda>(Pull::Up);
        set_direction::<Sda>(false); // 输入

        // 2) 补 9 个 SCL 脉冲；每补一个就检查 SDA 是否已被从设备释放（拉高）。
        write_level::<Scl>(Level::High);
        for _ in 0..9 {
            write_level::<Scl>(Level::Low);
            for _ in 0..600 {
                core::hint::spin_loop();
            }
            write_level::<Scl>(Level::High);
            for _ in 0..600 {
                core::hint::spin_loop();
            }
            if read_level::<Sda>() == Level::High {
                break;
            }
        }

        // 3) 补一个 STOP 序列：SCL 已为高，SDA 由低拉高。
        set_output_options::<Sda>(OutputType::OpenDrain, Speed::Normal);
        set_direction::<Sda>(true);
        write_level::<Sda>(Level::Low);
        for _ in 0..600 {
            core::hint::spin_loop();
        }
        write_level::<Sda>(Level::High);

        // 4) 恢复 I2C 复用与控制器寄存器。
        set_i2c_pad::<Scl>();
        set_i2c_pad::<Sda>();
        set_function::<Scl>(<Scl as I2cSclPin<T>>::FUNC);
        set_function::<Sda>(<Sda as I2cSdaPin<T>>::FUNC);

        T::clock().open();
        T::disable();
        T::set_clock(400_000);
        T::enable_master();
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Async> {
    /// Build an async I2C master. See [`I2c::build`] for the pad wiring.
    pub fn new_async(
        peri: impl Peripheral<P = T> + 'd,
        scl: impl Peripheral<P = Scl> + 'd,
        sda: impl Peripheral<P = Sda> + 'd,
        config: Config,
    ) -> Self
    where
        Scl: I2cSclPin<T>,
        Sda: I2cSdaPin<T>,
    {
        Self::build(peri, scl, sda, config)
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Blocking> {
    /// Blocking write of `bytes` to the slave at `address`.
    pub fn blocking_write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, false)?;
            for &b in bytes {
                Self::write_byte(b)?;
            }
            Ok(())
        })();
        // 关键：无论读写出错与否都要发出 STOP 并清标志，否则 NAK 等错误会把总线
        // 卡在传输中途，下一次操作就会一直超时（表现为触摸用久了卡死）。
        let _ = Self::finish();
        result
    }

    /// Blocking read of `buffer` from the slave at `address`.
    pub fn blocking_read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Error> {
        if buffer.is_empty() {
            return Ok(());
        }
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, true)?;
            let len = buffer.len();
            for (i, slot) in buffer.iter_mut().enumerate() {
                *slot = Self::read_byte(i + 1 == len)?;
            }
            Ok(())
        })();
        let _ = Self::finish();
        result
    }

    /// Blocking write followed by a repeated-start read, with no stop in between.
    pub fn blocking_write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error> {
        let result: Result<(), Error> = (|| {
            Self::start_addr(address, false)?;
            for &b in write {
                Self::write_byte(b)?;
            }
            if !read.is_empty() {
                Self::start_addr(address, true)?;
                let len = read.len();
                for (i, slot) in read.iter_mut().enumerate() {
                    *slot = Self::read_byte(i + 1 == len)?;
                }
            }
            Ok(())
        })();
        let _ = Self::finish();
        result
    }
}

impl<T: Instance, Scl: Pin, Sda: Pin, M: Mode> embedded_hal::i2c::ErrorType
    for I2c<'_, T, Scl, Sda, M>
{
    type Error = Error;
}

impl<T: Instance, Scl: Pin, Sda: Pin> embedded_hal::i2c::I2c<u8>
    for I2c<'_, T, Scl, Sda, Blocking>
{
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        if operations.is_empty() {
            return Ok(());
        }
        // Each operation is addressed on its own: the first START opens the
        // transfer, a repeated START separates operations, and one STOP closes
        // it. This is exactly what register-address-then-read slaves need.
        let result: Result<(), Error> = (|| {
            for op in operations.iter_mut() {
                match op {
                    embedded_hal::i2c::Operation::Write(bytes) => {
                        Self::start_addr(address, false)?;
                        for &b in bytes.iter() {
                            Self::write_byte(b)?;
                        }
                    }
                    embedded_hal::i2c::Operation::Read(buffer) => {
                        Self::start_addr(address, true)?;
                        let len = buffer.len();
                        for (i, slot) in buffer.iter_mut().enumerate() {
                            *slot = Self::read_byte(i + 1 == len)?;
                        }
                    }
                }
            }
            Ok(())
        })();
        result.and(Self::finish())
    }
}

#[cfg(feature = "embassy")]
impl<'d, T: Instance, Scl: Pin, Sda: Pin> I2c<'d, T, Scl, Sda, Async> {
    /// Async write of `bytes` to the slave at `address`.
    pub async fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Error> {
        future::write::<T>(address, bytes).await
    }

    /// Async read of `buffer` from the slave at `address`.
    pub async fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Error> {
        future::read::<T>(address, buffer).await
    }

    /// Async write followed by a repeated-start read.
    pub async fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> Result<(), Error> {
        future::write_read::<T>(address, write, read).await
    }
}

#[cfg(feature = "embassy")]
impl<T: Instance, Scl: Pin, Sda: Pin> embedded_hal_async::i2c::I2c<u8>
    for I2c<'_, T, Scl, Sda, Async>
{
    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal_async::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        // Run every operation as one strict I2C transfer: a single START opens the
        // first operation, a (repeated) START separates each following operation,
        // and one STOP ends the whole transfer. This is what register-address-then-read
        // slaves require.
        future::master::<T>(address, operations).await
    }
}

impl<'d, T: Instance, Scl: Pin, Sda: Pin, M: Mode> Drop for I2c<'d, T, Scl, Sda, M> {
    fn drop(&mut self) {
        T::disable();
        T::clock().close();
        crate::gpio::release::<Scl>();
        crate::gpio::release::<Sda>();
    }
}

/// Iterations of the flag poll before giving up; the loop is cheap and the bus
/// should never be busy for long, so this is a generous ceiling.
const TIMEOUT: u32 = 10_000_000;
