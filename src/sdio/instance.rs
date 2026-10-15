//! Register access for the SDIO host controller.
//!
//! The SWM341 integrates a SDIO / SD-memory host. The driver is built around the
//! command / data engine: `CR1` powers the card and selects the bus width, `CR2`
//! gates and divides the SD clock, `CMD` launches a card command (with `ARG` as
//! its argument) and `BLK` sizes a data transfer. Responses land in `RESP[0..3]`
//! and the data register `DATA` is the PIO FIFO; `IF` flags completion or error.
//!
//! Because `DATA`, `RESP` and the per-bit `IF` flags are all modelled by the PAC,
//! the driver reads/writes them through the typed accessors where available and
//! through `bits()` for the raw ones.


use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the SDIO host.
pub(crate) type Block = pac::sdio::RegisterBlock;

pub(crate) mod sealed {

    use super::{Block, PeripheralClockIndex};
    use crate::sdio::types::{BusWidth, ResponseType, SdioError};

    /// A SDIO host the driver can talk to.
    pub trait Instance {
        /// Clock gate that powers the SDIO host.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        fn irq() -> crate::interrupt::ExtendedInterrupt;
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;


        /// Power the card on (`CR1.PWRON`).
        fn power_on() {
            let regs = unsafe { &*Self::regs() };
            regs.cr1.modify(|_, w| w.pwron().set_bit());
        }

        /// Power the card off.
        fn power_off() {
            let regs = unsafe { &*Self::regs() };
            regs.cr1.modify(|_, w| w.pwron().clear_bit());
        }

        /// Enable and divide the SD clock.
        ///
        /// `divider` is written to `CR2.SDCLKDIV`; the SD clock runs at
        /// `peripheral_clock / (divider + 1)`.
        fn clock_enable(divider: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.cr2.modify(|_, w| unsafe {
                w.clken().set_bit();
                w.sdclkdiv().bits(divider);
                w.sdclken().set_bit()
            });
        }

        /// Disable the SD clock.
        fn clock_disable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr2.modify(|_, w| {
                w.clken().clear_bit();
                w.sdclken().clear_bit()
            });
        }

        /// Select the data bus width (`CR1._4BIT` / `CR1._8BIT`).
        fn set_bus_width(width: BusWidth) {
            let regs = unsafe { &*Self::regs() };
            regs.cr1.modify(|_, w| {
                w._4bit().bit(width == BusWidth::Bus4);
                w._8bit().bit(width == BusWidth::Bus8)
            });
        }

        /// Set the data timeout (`CR2.TIMEOUT`, 4 bits).
        fn set_timeout(timeout: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.cr2.modify(|_, w| unsafe { w.timeout().bits(timeout & 0x0f) });
        }

        /// Wait until the command phase completes, returning the response or an error.
        fn wait_command_done(regs: &Block) -> Result<[u32; 4], SdioError> {
            loop {
                let f = regs.if_.read();
                if f.error().bit()
                    || f.cmdtimeout().bit()
                    || f.cmdcrcerr().bit()
                    || f.cmdenderr().bit()
                    || f.cmdidxerr().bit()
                    || f.resperr().bit()
                {
                    return Err(SdioError::from_flags(&f));
                }
                if f.cmddone().bit() {
                    // Clear the command-done flag and read the response.
                    regs.if_.write(|w| w.cmddone().set_bit());
                    return Ok([
                        regs.resp[0].read().bits(),
                        regs.resp[1].read().bits(),
                        regs.resp[2].read().bits(),
                        regs.resp[3].read().bits(),
                    ]);
                }
            }
        }

        /// Launch a command (no data phase) and return its response.
        fn send_command(
            index: u8,
            arg: u32,
            response: ResponseType,
            check_crc: bool,
            check_idx: bool,
        ) -> Result<[u32; 4], SdioError> {
            let regs = unsafe { &*Self::regs() };
            regs.arg.write(|w| unsafe { w.bits(arg) });
            regs.cmd.write(|w| unsafe {
                w.cmdindx().bits(index);
                w.resptype().bits(response as u8);
                w.crccheck().bit(check_crc);
                w.idxcheck().bit(check_idx);
                w.havedata().clear_bit();
                w.cmdtype().bits(0)
            });
            Self::wait_command_done(regs)
        }

        /// Configure a block transfer (`BLK`) and launch the command.
        ///
        /// `block_bytes` is the size of one block in bytes, `count` the number of
        /// blocks. `read` selects the transfer direction. The data phase is then
        /// driven by [`pio_read`]/[`pio_write`].
        fn start_transfer(
            index: u8,
            arg: u32,
            response: ResponseType,
            check_crc: bool,
            block_bytes: u16,
            count: u16,
            read: bool,
        ) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            regs.arg.write(|w| unsafe { w.bits(arg) });
            regs.blk.write(|w| unsafe {
                w.size().bits(block_bytes.saturating_sub(1));
                w.count().bits(count.saturating_sub(1))
            });
            regs.cmd.write(|w| unsafe {
                w.cmdindx().bits(index);
                w.resptype().bits(response as u8);
                w.crccheck().bit(check_crc);
                w.idxcheck().bit(true);
                w.havedata().set_bit();
                w.dirread().bit(read);
                w.multblk().bit(count > 1);
                w.cmdtype().bits(0)
            });
            // The command-complete flag is raised before the data phase; clear it.
            loop {
                let f = regs.if_.read();
                if f.error().bit()
                    || f.cmdtimeout().bit()
                    || f.cmdcrcerr().bit()
                    || f.cmdenderr().bit()
                    || f.cmdidxerr().bit()
                    || f.resperr().bit()
                {
                    return Err(SdioError::from_flags(&f));
                }
                if f.cmddone().bit() {
                    regs.if_.write(|w| w.cmddone().set_bit());
                    return Ok(());
                }
            }
        }

        /// PIO read of `buf` (in 32-bit words) from the data FIFO.
        fn pio_read(buf: &mut [u32]) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            let mut i = 0usize;
            while i < buf.len() {
                loop {
                    let f = regs.if_.read();
                    if f.error().bit()
                        || f.dattimeout().bit()
                        || f.datcrcerr().bit()
                        || f.datenderr().bit()
                    {
                        return Err(SdioError::from_flags(&f));
                    }
                    if f.bufrdrdy().bit() {
                        buf[i] = regs.data.read().bits();
                        regs.if_.write(|w| w.bufrdrdy().set_bit());
                        break;
                    }
                }
                i += 1;
            }
            Self::wait_transfer_done(regs)
        }

        /// PIO write of `buf` (in 32-bit words) to the data FIFO.
        fn pio_write(buf: &[u32]) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            let mut i = 0usize;
            while i < buf.len() {
                loop {
                    let f = regs.if_.read();
                    if f.error().bit()
                        || f.dattimeout().bit()
                        || f.datcrcerr().bit()
                        || f.datenderr().bit()
                    {
                        return Err(SdioError::from_flags(&f));
                    }
                    if f.bufwrrdy().bit() {
                        regs.data.write(|w| unsafe { w.bits(buf[i]) });
                        regs.if_.write(|w| w.bufwrrdy().set_bit());
                        break;
                    }
                }
                i += 1;
            }
            Self::wait_transfer_done(regs)
        }

        /// Wait for the whole data transfer to finish (`IF.TRXDONE`).
        fn wait_transfer_done(regs: &Block) -> Result<(), SdioError> {
            loop {
                let f = regs.if_.read();
                if f.error().bit() || f.datcrcerr().bit() || f.datenderr().bit() {
                    return Err(SdioError::from_flags(&f));
                }
                if f.trxdone().bit() {
                    regs.if_.write(|w| w.trxdone().set_bit());
                    return Ok(());
                }
            }
        }

        /// Clear every status flag (`IF`).
        fn clear_interrupts() {
            let regs = unsafe { &*Self::regs() };
            // Every `IF` bit is write-1-to-clear, so writing all ones resets them.
            regs.if_.write(|w| unsafe { w.bits(0xFFFF_FFFF) });
        }
    }
}

#[allow(private_bounds)]
/// A SDIO host usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the SDIO singleton.
macro_rules! impl_sdio_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::Sdio
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> crate::interrupt::ExtendedInterrupt {
                crate::interrupt::$irq
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_sdio_instance!(crate::Sdio, SDIO, SDIO, 0);
