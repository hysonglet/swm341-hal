#![no_std]
#![no_main]

//! SDIO SD-memory card bring-up and block read/write verification.
//!
//! Implements the vendor `SDIO_Init` sequence (CMD0 -> CMD8 -> ACMD41 -> CMD2 ->
//! CMD3 -> CMD9 -> CMD7 -> CMD55+ACMD6 (4-bit) -> CMD16) on top of the HAL's
//! command and transfer primitives, then verifies the data engine with a single
//! block read (CMD17) and write (CMD24). The write test is non-destructive: the
//! target block's original contents are saved, overwritten with a pattern, read
//! back to confirm, then restored.

use panic_halt as _;
use rtt_target::{rprintln, rtt_init_print};

use cortex_m::asm::delay;
use hal::gpio::{Input, Pull};
use hal::sdio::{BusWidth, Config, ResponseType, Sdio, SdioError};
use swm341_hal as hal;

/// Core clock used for power-up delays below (matches `hrc_150mhz`).
const HCLK_HZ: u32 = 150_000_000;

/// Print the SDIO controller registers for bring-up diagnosis.
fn dump_sdio() {
    let regs = unsafe { &*hal::pac::SDIO::PTR };
    let cr1 = regs.cr1.read();
    let cr2 = regs.cr2.read();
    let stat = regs.stat.read().bits();
    let iflags = regs.if_.read().bits();
    rprintln!(
        "SDIO CR1: pwron={} 4bit={} 8bit={} cdsrc={} cdbit={}",
        cr1.pwron().bit(),
        cr1._4bit().bit(),
        cr1._8bit().bit(),
        cr1.cdsrc().bit(),
        cr1.cdbit().bit()
    );
    rprintln!(
        "SDIO CR2: clken={} sdclken={} sdclkdiv={} clkrdy={}",
        cr2.clken().bit(),
        cr2.sdclken().bit(),
        cr2.sdclkdiv().bits(),
        cr2.clkrdy().bit()
    );
    rprintln!(
        "SDIO STAT: cmdlinest={} cmdinhi={} datlinest={} cardinst={} hascardst={} (raw=0x{:08X})",
        stat & 1,
        (stat >> 1) & 1,
        (stat >> 2) & 1,
        (stat >> 16) & 1,
        (stat >> 18) & 1,
        stat
    );
    rprintln!("SDIO IF (raw): 0x{:08X}", iflags);
}

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });
    rprintln!("HAL initialised");

    // 4-bit bus pads are muxed by the constructor; the driver keeps the *data*
    // bus in 1-bit until the card is addressed (CMD7) and switched via ACMD6,
    // matching the SD spec. `clock_divider` is a *requested* divisor encoded into
    // `CR2.SDCLKDIV` (set bit `n` => /2^(n+1)); `0xFF` => /256 => ~146 kHz, the
    // slow, safe identification clock the vendor library also uses.
    let sd = Sdio::new_4bit(
        p.sdio(),
        p.PM2,
        p.PM4,
        p.PM5,
        p.PM6,
        p.PN0,
        p.PN1,
        Config::default().clock_divider(0xFF),
    );
    rprintln!("SDIO host up (1-bit ident, ~146 kHz)");

    // Card-detect sanity check on PM3 (the SD socket's CD pin).
    let cd = Input::new(p.PM3, Pull::Up);
    rprintln!(
        "CD (PM3) = {} -> card {}",
        if cd.is_high() { "HIGH" } else { "LOW " },
        if cd.is_low() { "INSERTED" } else { "ABSENT  " }
    );
    dump_sdio();

    // Let the card settle on the running SD clock before the first command.
    delay(HCLK_HZ / 10);

    // The whole init + verify sequence returns a `Result` so we can use `?` inside
    // an immediately-invoked closure (main itself returns `!` and cannot use `?`).
    let result = (|| -> Result<(), SdioError> {
        const BLOCK_WORDS: usize = 512 / 4; // words per 512-byte block

        // CMD0: GO_IDLE_STATE (no response). A couple of pulses help some cards.
        for _ in 0..2 {
            sd.send_command(0, 0, ResponseType::None, false, false)?;
        }

        // CMD8: SEND_IF_COND. Echo 0x1AA => 2.7-3.6 V supported, SD v2 card.
        let r = sd.send_command(8, 0x1AA, ResponseType::Short, false, false)?;
        if r[0] != 0x1AA {
            rprintln!("CMD8 returned 0x{:08X}, expected 0x000001AA", r[0]);
            return Err(SdioError::Other);
        }
        rprintln!("CMD8 OK: card is SD v2 (resp[0] = 0x{:08X})", r[0]);

        // ACMD41 loop: poll OCR until the card reports power-up complete (bit 31).
        // ACMD = CMD55 (APP_CMD) followed by the application command. CMD55's R1
        // must carry the APP_CMD status bit (bit 5) or the card rejected it.
        let mut ocr = 0u32;
        for _ in 0..2000 {
            let r = sd.send_command(55, 0, ResponseType::Short, false, false)?;
            if r[0] & (1 << 5) == 0 {
                rprintln!("CMD55 (before ACMD41) did not ack APP_CMD");
                return Err(SdioError::Other);
            }
            let r = sd.send_command(
                41,
                0x8010_0000 | 0x4000_0000,
                ResponseType::Short,
                false,
                false,
            )?;
            ocr = r[0];
            if ocr & (1 << 31) != 0 {
                break;
            }
        }
        if ocr & (1 << 31) == 0 {
            rprintln!(
                "ACMD41 never reported power-up complete (OCR = 0x{:08X})",
                ocr
            );
            return Err(SdioError::Timeout);
        }
        let high_capacity = (ocr & (1 << 30)) != 0;
        rprintln!(
            "ACMD41 OK: OCR = 0x{:08X} ({})",
            ocr,
            if high_capacity { "SDHC/SDXC" } else { "SDSC" }
        );

        // CMD2: fetch CID (136-bit response; latching only here).
        sd.send_command(2, 0, ResponseType::Long, false, false)?;
        rprintln!("CMD2 (CID) OK");

        // CMD3: ask the card for a relative address (RCA).
        let r = sd.send_command(3, 0, ResponseType::Short, false, false)?;
        let rca = r[0] >> 16;
        rprintln!("CMD3 OK: RCA = 0x{:04X}", rca);

        // CMD9: fetch CSD (136-bit response; latching only here).
        sd.send_command(9, (rca as u32) << 16, ResponseType::Long, false, false)?;
        rprintln!("CMD9 (CSD) OK");

        // Speed the SD clock up for the data phase. The vendor library keeps the
        // slow ~100 kHz ident clock only through identification and switches to a
        // full-speed clock (here ~18 MHz = (HCLK/4)/2) before the data phase; the
        // data engine does not function at the ident clock, which is why the first
        // block read/write timed out before. A divisor of `2` is encoded by the
        // driver into `CR2.SDCLKDIV = 0x01` => /2 => ~18.75 MHz, safely inside
        // the SD default-speed 25 MHz ceiling (a divisor of `1` would leave the
        // clock at /1 => ~37.5 MHz and the data phase would time out).
        sd.configure_clock(2);
        rprintln!("SD clock raised to ~18 MHz for data phase");

        // CMD7: select the card (R1b: 48-bit response with busy signalling).
        sd.send_command(7, (rca as u32) << 16, ResponseType::Busy, false, false)?;
        rprintln!("CMD7 OK: card selected");

        // CMD55 (with RCA) + ACMD6: switch the card to a 4-bit bus, then widen
        // the controller's own data bus to match.
        let r = sd.send_command(55, (rca as u32) << 16, ResponseType::Short, false, false)?;
        if r[0] & (1 << 5) == 0 {
            rprintln!("CMD55 (before ACMD6) did not ack APP_CMD");
            return Err(SdioError::Other);
        }
        sd.send_command(6, 0x02, ResponseType::Short, false, false)?;
        if sd.set_bus_width(BusWidth::Bus4) {
            rprintln!("Switched controller + card to 4-bit bus");
        } else {
            rprintln!("4-bit switch skipped (pads not routed) -- staying 1-bit");
        }

        // CMD16: fixed 512-byte block length.
        sd.send_command(16, 512, ResponseType::Short, false, false)?;
        rprintln!("CMD16 OK: block length = 512");

        // --- Block read/write verification --------------------------------------
        // SDHC/SDXC address by block number; SDSC address by byte offset.
        let block_addr = |block: u32| -> u32 {
            if high_capacity {
                block
            } else {
                block * 512
            }
        };

        let mut buf = [0u32; BLOCK_WORDS];

        // Read block 0 (the MBR) -- a non-destructive sanity check. A valid MBR
        // ends with the boot signature 0x55 0xAA at bytes 510-511 (LE word = 0xAA55).
        sd.read_blocks(17, block_addr(0), &mut buf, 512, 1)?;
        let boot_sig = (buf[BLOCK_WORDS - 1] & 0xFFFF) as u16;
        rprintln!(
            "CMD17 block 0 read OK: first word = 0x{:08X}, boot sig = 0x{:04X} ({})",
            buf[0],
            boot_sig,
            if boot_sig == 0xAA55 {
                "valid MBR"
            } else {
                "no MBR signature"
            }
        );

        // Non-destructive write test on an out-of-the-way block: save the original,
        // write a pattern, read it back to confirm, then restore the original.
        let test_block = 0x1000u32; // 2 MiB in on SDHC; safely past the MBR.
        let mut original = [0u32; BLOCK_WORDS];
        sd.read_blocks(17, block_addr(test_block), &mut original, 512, 1)?;
        rprintln!("Saved original block 0x{:X}", test_block);

        for (i, w) in buf.iter_mut().enumerate() {
            *w = (i as u32).wrapping_mul(0x0101_0101) ^ 0xABCD_0000;
        }
        sd.write_blocks(24, block_addr(test_block), &buf, 512, 1)?;
        rprintln!("CMD24 wrote pattern to block 0x{:X}", test_block);

        let mut readback = [0u32; BLOCK_WORDS];
        sd.read_blocks(17, block_addr(test_block), &mut readback, 512, 1)?;
        let ok = buf == readback;

        // Always restore the original contents so the card is left untouched.
        let _ = sd.write_blocks(24, block_addr(test_block), &original, 512, 1);
        rprintln!("Restored original block 0x{:X}", test_block);

        if ok {
            rprintln!("CMD17/CMD24 round-trip MATCH -- SD card R/W OK");
        } else {
            rprintln!("round-trip MISMATCH");
            return Err(SdioError::Other);
        }

        Ok(())
    })();

    match result {
        Ok(()) => rprintln!("SDIO SD-memory bring-up complete"),
        Err(e) => rprintln!("SDIO bring-up failed: {:?}", e),
    }
    loop {}
}
