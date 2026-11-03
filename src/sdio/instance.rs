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

    use super::Block;
    use crate::sdio::types::{BusWidth, ResponseType, SdioError};

    /// Software poll limit for the command / data engine waits.
    ///
    /// The controller only raises its own timeout flag when the SD clock is
    /// actually running and a card is answering. With an empty bus (no card) or
    /// a misconfigured clock the hardware may never flag an error, so every wait
    /// below is capped: a bring-up fixture must report a timeout, never spin
    /// forever.
    const POLL_LIMIT: u32 = 2_000_000;

    /// Encode a requested SD clock divisor into the `CR2.SDCLKDIV` bit field.
    ///
    /// The hardware does **not** divide linearly: a single bit set at position
    /// `n` selects a divisor of `2^(n+1)` (`0x00` => /1, `0x01` => /2, `0x02` =>
    /// /4, `0x04` => /8, ...). Writing a linear value (e.g. `9`) sets several
    /// bits and produces an undefined clock -- which is why a command then
    /// times out with `CR2.SDCLKEN` still asserted. We round the requested
    /// divisor up to the next power of two and encode that as `pow2 >> 1`
    /// (`2^(n+1) = 2 * (1 << n)` => the bit position is `n`, i.e. `pow2 >> 1`).
    ///
    /// Note the previous implementation could never emit `0x01` (/2): a request
    /// of `2` computed `n = 2` and produced `0x02` (/4), while `1` was special
    /// cased to `/1`. That left the data phase stuck at `/1` (~37.5 MHz on a
    /// 150 MHz HCLK), over the SD default-speed 25 MHz ceiling, so the first
    /// block read/write timed out even though every command (single-bit CMD
    /// line) succeeded. A request of `2` now yields the intended `/2`.
    fn encode_sdclkdiv(div: u8) -> u8 {
        if div <= 1 {
            0
        } else {
            let d = div as u32;
            // Smallest power of two >= d (exact powers stay, others round up).
            let pow2 = if d & (d - 1) == 0 {
                d
            } else {
                1u32 << (32 - d.leading_zeros())
            };
            (pow2 >> 1) as u8
        }
    }

    /// A SDIO host the driver can talk to.
    pub trait Instance: crate::instance_base::InstanceBase {
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

        /// Full bring-up of the SDIO clock and power, matching the vendor
        /// `SDIO_Init` sequence.
        ///
        /// This is the critical fix for a controller that never completes a
        /// command: the SD clock has **no source** until `SYS->CLKSEL` selects one,
        /// so without this step `CMD0` times out forever while `CR2.SDCLKEN` still
        /// reads as set. We also reset the controller, treat card-detect as an
        /// internal source (so commands are accepted without routing the CD pin),
        /// set the operating voltage, and wait for the clock to come up.
        fn init(divider: u8, timeout: u8) {
            // Select the SDIO clock source: SDCLK = SYSCLK / 4. This is valid for the
            // whole SWM341 speed range; the vendor library uses /2 only below 80 MHz
            // to get a faster source, but /4 is always safe and `divider` divides
            // further below.
            unsafe { &*crate::pac::SYS::PTR }
                .clksel
                .modify(|_, w| unsafe { w.sdio().bits(2) });

            let regs = unsafe { &*Self::regs() };

            // Reset the whole controller before configuring it.
            regs.cr2.modify(|_, w| w.rstall().set_bit());
            // Wait for the reset pulse to release before touching any other
            // register: writing CR1/CR2 while the controller is still in reset
            // silently drops (a "powered-off" controller ignores writes), which
            // leaves PWRON/CLKEN clear and makes CMD0 time out with no error flag.
            cortex_m::asm::delay(2000);

            // Power on and select the card-detect source. The vendor library sets
            // `CDSRC = 1` (internal card-detect source) and deliberately leaves
            // `CDBIT = 0`. With `CDSRC = 1` the controller reads `CDBIT` as the
            // card-present flag, and `CDBIT = 0` means "card present" -- setting
            // `CDBIT = 1` instead makes the controller believe the card is absent
            // and it then refuses every command (no error flag, just a command FSM
            // that never completes -- exactly the CMD0 timeout symptom seen before).
            // The CD pin (PM3) is left as a plain GPIO and only probed by the
            // example for a hardware sanity check.
            regs.cr1.modify(|_, w| {
                w.pwron().set_bit();
                w.cdsrc().set_bit();
                unsafe { w.volt().bits(7) }
            });

            // Enable and divide the SD clock, set the response timeout.
            regs.cr2.modify(|_, w| {
                w.clken().set_bit();
                w.sdclken().set_bit();
                unsafe {
                    w.sdclkdiv().bits(encode_sdclkdiv(divider));
                    w.timeout().bits(timeout)
                }
            });

            // Wait for the clock to come up before any command is issued.
            // (Bounded: an empty bus / misconfigured source may never raise it,
            // so spin at most `POLL_LIMIT` times instead of forever.)
            let mut clkrdy_guard = 0u32;
            while !regs.cr2.read().clkrdy().bit() {
                clkrdy_guard += 1;
                if clkrdy_guard > POLL_LIMIT {
                    break;
                }
            }

            // The vendor library waits ~10 us after the clock is ready before
            // touching the controller further. At 150 MHz that is ~1500 cycles.
            cortex_m::asm::delay(1500);

            // Unmask every interrupt source (the flags are still polled via `IF`).
            regs.im.write(|w| unsafe { w.bits(0xFFFF_FFFF) });
        }

        /// Enable and divide the SD clock.
        ///
        /// `divider` is the *requested* divisor; it is encoded into the
        /// `CR2.SDCLKDIV` bit field (see [`encode_sdclkdiv`]). The SD clock runs
        /// at `(HCLK / 4) / 2^n` where `2^n` is the encoded divisor.
        ///
        /// The vendor library switches the clock this way *after* the card has
        /// been identified and *before* the data phase: it clears `SDCLKEN` and
        /// `SDCLKDIV`, applies the new divisor, re-enables the clock, and then
        /// waits for `CLKRDY` before any further command. Skipping the CLKRDY
        /// wait leaves the clock source unstable and the data engine will never
        /// raise `BUFRDRDY`/`BUFWRRDY` -- a block transfer then times out.
        fn clock_enable(divider: u8) {
            let regs = unsafe { &*Self::regs() };
            // Disable the SD clock and clear the divider for a clean transition.
            regs.cr2
                .modify(|_, w| w.sdclken().clear_bit().clken().clear_bit());
            regs.cr2.modify(|_, w| unsafe {
                w.sdclkdiv().bits(encode_sdclkdiv(divider));
                w.clken().set_bit();
                w.sdclken().set_bit()
            });
            let mut clkrdy_guard = 0u32;
            while !regs.cr2.read().clkrdy().bit() {
                clkrdy_guard += 1;
                if clkrdy_guard > POLL_LIMIT {
                    break;
                }
            }
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
            regs.cr2
                .modify(|_, w| unsafe { w.timeout().bits(timeout & 0x0f) });
        }

        /// Wait until the command phase completes, returning the response or an error.
        fn wait_command_done(regs: &Block) -> Result<[u32; 4], SdioError> {
            let mut guard = 0u32;
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
                guard += 1;
                if guard > POLL_LIMIT {
                    return Err(SdioError::Timeout);
                }
            }
        }

        /// Launch a command (no data phase) and return its response.
        fn send_command(
            index: u8,
            arg: u32,
            response: ResponseType,
            _check_crc: bool,
            _check_idx: bool,
        ) -> Result<[u32; 4], SdioError> {
            let regs = unsafe { &*Self::regs() };
            // The vendor library always writes `BLK.COUNT` before a command; for a
            // command without a data phase (this path) it writes 0.
            regs.blk.write(|w| unsafe { w.count().bits(0) });
            regs.arg.write(|w| unsafe { w.bits(arg) });
            // The vendor `SDIO_SendCmd` always drives `CRCCHECK = 0` and `IDXCHECK = 0`
            // in `CMD` (the controller still reports CRC/index errors via `IF`, but
            // completion is not gated on them). `HAVEDATA = 0` marks a command-only
            // transfer. This matches the vendor exactly.
            regs.cmd.write(|w| unsafe {
                w.cmdindx().bits(index);
                w.resptype().bits(response as u8);
                w.crccheck().clear_bit();
                w.idxcheck().clear_bit();
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
            _check_crc: bool,
            block_bytes: u16,
            count: u16,
            read: bool,
        ) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            regs.arg.write(|w| unsafe { w.bits(arg) });
            // `BLK.SIZE` is the block size in BYTES (512 = 0x200) and `BLK.COUNT` is
            // the block count, both written directly (no -1). This mirrors the vendor
            // `SDIO->BLK = 512` exactly.
            regs.blk.write(|w| unsafe {
                w.size().bits(block_bytes);
                w.count().bits(count)
            });
            // `MULTBLK`/`BLKCNTEN` are set for multi-block transfers; `AUTOCMD12` is
            // used for multi-block memory commands but must stay clear for CMD53 (SDIO
            // I/O), exactly as the vendor `SDIO_SendCmd` does. `CRCCHECK`/`IDXCHECK`
            // are held at 0 to match the vendor, which never sets them.
            regs.cmd.write(|w| unsafe {
                w.cmdindx().bits(index);
                w.resptype().bits(response as u8);
                w.crccheck().clear_bit();
                w.idxcheck().clear_bit();
                w.havedata().set_bit();
                w.dirread().bit(read);
                w.multblk().bit(count > 1);
                w.blkcnten().bit(count > 1);
                w.autocmd12().bit(index != 53 && count > 1);
                w.dmaen().clear_bit();
                w.cmdtype().bits(0)
            });
            // The command-complete flag is raised before the data phase; clear it.
            let mut guard = 0u32;
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
                guard += 1;
                if guard > POLL_LIMIT {
                    return Err(SdioError::Timeout);
                }
            }
        }

        /// PIO read of `buf` (in 32-bit words) from the data FIFO.
        ///
        /// This mirrors the vendor `SDIO_BlockRead`: wait for `BUFRDRDY` once to
        /// learn the first word has landed, acknowledge it, then drain the whole
        /// block in a tight loop. The controller buffers at least one full 512-byte
        /// block and applies back-pressure on the card, so the FIFO cannot overrun
        /// and a per-word poll+clear is unnecessary. In fact clearing `BUFRDRDY`
        /// after every word stalls this controller's data engine: the transfer
        /// never completes and the only symptom is a data-timeout, which is exactly
        /// what the previous per-word implementation produced.
        fn pio_read(buf: &mut [u32]) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            let mut guard = 0u32;
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
                    break;
                }
                guard += 1;
                if guard > POLL_LIMIT {
                    return Err(SdioError::Timeout);
                }
            }
            // Acknowledge the ready flag once, then read the whole block without
            // re-polling: re-checking / clearing `BUFRDRDY` per word stalls the
            // data engine on this controller.
            regs.if_.write(|w| w.bufrdrdy().set_bit());
            for i in 0..buf.len() {
                buf[i] = regs.data.read().bits();
            }
            Self::wait_transfer_done(regs)
        }

        /// PIO write of `buf` (in 32-bit words) to the data FIFO.
        ///
        /// Mirrors the vendor `SDIO_BlockWrite`: wait for `BUFWRRDY` once,
        /// acknowledge it, then push the whole block in a tight loop, then wait
        /// for `TRXDONE`. As with [`pio_read`], per-word polling of the ready
        /// flag stalls the data engine.
        fn pio_write(buf: &[u32]) -> Result<(), SdioError> {
            let regs = unsafe { &*Self::regs() };
            let mut guard = 0u32;
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
                    break;
                }
                guard += 1;
                if guard > POLL_LIMIT {
                    return Err(SdioError::Timeout);
                }
            }
            regs.if_.write(|w| w.bufwrrdy().set_bit());
            for i in 0..buf.len() {
                regs.data.write(|w| unsafe { w.bits(buf[i]) });
            }
            Self::wait_transfer_done(regs)
        }

        /// Wait for the whole data transfer to finish (`IF.TRXDONE`).
        fn wait_transfer_done(regs: &Block) -> Result<(), SdioError> {
            let mut guard = 0u32;
            loop {
                let f = regs.if_.read();
                if f.error().bit() || f.datcrcerr().bit() || f.datenderr().bit() {
                    return Err(SdioError::from_flags(&f));
                }
                if f.trxdone().bit() {
                    regs.if_.write(|w| w.trxdone().set_bit());
                    return Ok(());
                }
                guard += 1;
                if guard > POLL_LIMIT {
                    return Err(SdioError::Timeout);
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
        crate::impl_instance_base!(
            $ty,
            crate::pac::sdio::RegisterBlock,
            crate::interrupt::$irq,
            PeripheralClockIndex::Sdio,
            $index
        );

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_sdio_instance!(crate::Sdio, SDIO, SDIO, 0);
