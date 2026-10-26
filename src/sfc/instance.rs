//! Register access for the Serial Flash Controller (SFC).
//!
//! The SFC is the SWM341's hardware SPI master dedicated to NOR flash. Its pads
//! (D3..D8) are not routed to SPI0/SPI1 in the pin matrix, so this is the only
//! peripheral that can talk to the on-board W25Q128. The SFC has two faces:
//!
//! * a *manual command* path (`CFG.CMDTYPE` + `CMD`/`GO`) that sends one opcode
//!   and optionally returns bytes in `DATA` -- used for JEDEC ID, status
//!   registers, erase and status-register writes,
//! * an *AHB window* at `0x7000_0000` that transparently turns load/store
//!   instructions into read/program transactions (the XIP path). The datasheet
//!   states "读写操作仅支持 WORD", so every access to that window is a
//!   word-sized, word-aligned access; byte-level helpers assemble words.
//!
//! All register operations live in the default methods of the sealed `Instance`
//! trait; the public driver in [`crate::sfc`] only forwards to them.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the SFC.
pub(crate) type Block = pac::sfc::RegisterBlock;

/// `CFG.CMDTYPE` values (datasheet 6.28.6).
///
/// Types 5, 6 and 7 make the hardware send the write-enable opcode stored in
/// `CMDAHB.WREN` on its own before the command.
pub(crate) const CMDTYPE_READ_STATUS: u8 = 1; // no address, one status byte -> DATA
pub(crate) const CMDTYPE_READ_JEDEC: u8 = 2; // no address, three JEDEC bytes -> DATA
pub(crate) const CMDTYPE_CHIP_ERASE: u8 = 5; // no address, no data, with WIP
pub(crate) const CMDTYPE_WRITE_STATUS: u8 = 6; // no address, two data bytes -> status register
pub(crate) const CMDTYPE_SECTOR_ERASE: u8 = 7; // uses ADDR, no data, with WIP

/// Bit position of the Quad Enable flag inside status register 2.
///
/// The flash is written as a single 16-bit `SR2:SR1` pair, so the flag lives at
/// bit 9 of that pair -- i.e. bit 1 of SR2.
pub(crate) const STATUS_QUAD_BIT: u16 = 9;

pub(crate) mod sealed {

    use super::{
        Block, CMDTYPE_CHIP_ERASE, CMDTYPE_READ_JEDEC, CMDTYPE_READ_STATUS, CMDTYPE_SECTOR_ERASE,
        CMDTYPE_WRITE_STATUS, STATUS_QUAD_BIT,
    };
    use core::ptr;
    use cortex_m::asm;

    use crate::sfc::types::{
        Config, Error, Flags, ProgramWidth, ReadWidth, CMD_READ_JEDEC, CMD_READ_STATUS_REG1,
        CMD_READ_STATUS_REG2, CMD_WRITE_STATUS_REG1, FLASH_BASE, WIP_POLL_LIMIT,
    };

    /// Pulse `GO` and busy-wait for the controller to complete the transaction.
    ///
    /// The barriers bracket the write so the store is ordered with respect to
    /// the controller, which runs independently of the core.
    #[inline(always)]
    fn start(regs: &Block) {
        asm::dsb();
        asm::isb();
        unsafe { regs.go.write(|w| w.bits(1)) };
        asm::dsb();
        asm::isb();
        while regs.go.read().bits() != 0 {}
    }

    /// A Serial Flash Controller the driver can talk to.
    ///
    /// Implemented for the singleton in [`crate::mcu`]. The only per-instance
    /// datum is the register-block pointer; everything else (clock gate,
    /// async index, NVIC vector) comes from [`crate::instance_base`] and all
    /// register access lives in the default methods below.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;

        /// Program the controller from `config`: the vendor's undocumented
        /// `BASE + 0x3F4` register, the clock divider and data widths, the AHB
        /// read/program/write-enable/status opcodes, and the WIP polling
        /// interval. Mirrors `SFC_Init()`.
        ///
        /// When either width is quad and the flash's Quad Enable bit is not yet
        /// set, this also switches the flash into quad mode -- which the vendor
        /// `SFC_Init()` does too. Status reads always run in 1-1-1 mode, so
        /// that is safe before the switch.
        #[inline(always)]
        fn configure(config: Config) {
            let regs = unsafe { &*Self::regs() };
            unsafe {
                // Undocumented register poked by every vendor SFC routine.
                ptr::write_volatile((regs as *const _ as *mut u8).offset(0x3F4) as *mut u32, 7);

                regs.cfg.modify(|_, w| {
                    w.clkdiv()
                        .bits(config.clk_div as u8)
                        .data4l_rd()
                        .bits(config.read_width as u8)
                        .data4l_pp()
                        .bit(config.program_width == ProgramWidth::Quad)
                        .addr4l()
                        .bit(config.address_quad)
                        // The vendor sets CMDWREN around the CMDAHB write.
                        .cmdwren()
                        .set_bit()
                });

                regs.cmdahb.modify(|_, w| {
                    w.wren()
                        .bits(config.write_enable_command)
                        .pp()
                        .bits(config.program_command)
                        .rdsrl()
                        .bits(config.read_status_command)
                        .read()
                        .bits(config.read_command)
                });

                regs.cfg.modify(|_, w| w.cmdwren().clear_bit());

                // WIP polling interval (~0.2 ms: 2048 * (N + 1) HCLK cycles)
                // and limit (vendor default).
                let mhz = crate::clock::sys_core_clock() / 1_000_000;
                regs.tim.modify(|_, w| {
                    w.wip_chk_itv()
                        .bits((mhz / 10) as u8)
                        .wip_chk_lmt()
                        .bits(255)
                });
            }

            let wants_quad =
                config.read_width == ReadWidth::Quad || config.program_width == ProgramWidth::Quad;
            if wants_quad && !Self::quad_state() {
                Self::set_quad(true);
            }
        }

        /// Fire one manual SFC command: program `CMDTYPE`, write the opcode,
        /// pulse `GO`, wait, and return `DATA`. Same flow as the vendor
        /// `SFC_ReadJEDEC()` / `SFC_ReadStatusReg()`.
        #[inline(always)]
        fn command(cmd_type: u8, opcode: u8) -> u32 {
            let regs = unsafe { &*Self::regs() };
            unsafe {
                regs.cfg
                    .modify(|_, w| w.cmdtype().bits(cmd_type).cmdwren().set_bit());
                regs.cmd.write(|w| w.bits(opcode as u32));
            }

            start(regs);

            regs.data.read().bits()
        }

        /// Three JEDEC ID bytes, manufacturer first (bits `[23:16]`).
        #[inline(always)]
        fn read_jedec() -> u32 {
            Self::command(CMDTYPE_READ_JEDEC, CMD_READ_JEDEC)
        }

        /// One status register byte selected by `cmd` (e.g. `0x05` for SR1).
        #[inline(always)]
        fn read_status(cmd: u8) -> u8 {
            Self::command(CMDTYPE_READ_STATUS, cmd) as u8
        }

        /// True while the flash reports write-in-progress (SR1 bit 0).
        #[inline(always)]
        fn busy() -> bool {
            Self::read_status(CMD_READ_STATUS_REG1) & 0x01 != 0
        }

        /// True while the controller itself is busy: `SR.BUSY` covers both the
        /// SPI transaction and the flash's write-in-progress period.
        #[inline(always)]
        fn controller_busy() -> bool {
            unsafe { &*Self::regs() }.sr.read().busy().bit()
        }

        /// Poll the flash's write-in-progress flag until it clears.
        #[inline(always)]
        fn wait_wip(limit: u32) -> Result<(), Error> {
            for _ in 0..limit {
                if !Self::busy() {
                    return Ok(());
                }
            }
            Err(Error::Timeout)
        }

        /// Poll `SR.BUSY` until the controller goes idle.
        #[inline(always)]
        fn wait_controller(limit: u32) -> Result<(), Error> {
            let regs = unsafe { &*Self::regs() };
            for _ in 0..limit {
                if !regs.sr.read().busy().bit() {
                    return Ok(());
                }
            }
            Err(Error::Timeout)
        }

        /// Write `val` to the flash status register selected by `cmd`.
        ///
        /// Fires the command and waits for `GO`, but does *not* wait for the
        /// flash to finish writing -- that is [`Self::wait_wip`]'s job.
        #[inline(always)]
        fn write_status(cmd: u8, val: u16) {
            let regs = unsafe { &*Self::regs() };
            unsafe {
                regs.cfg.modify(|_, w| {
                    w.wren()
                        .set_bit()
                        .cmdwren()
                        .set_bit()
                        .cmdtype()
                        .bits(CMDTYPE_WRITE_STATUS)
                });
                regs.cmd.write(|w| w.bits(cmd as u32));
                regs.data.write(|w| w.bits(val as u32));
            }

            start(regs);

            regs.cfg.modify(|_, w| w.wren().clear_bit());
        }

        /// Set (`on`) or clear the flash's Quad Enable bit, which lives at bit 1
        /// of status register 2. Mirrors `SFC_QuadSwitch()`.
        #[inline(always)]
        fn set_quad(on: bool) {
            let reg = ((Self::read_status(CMD_READ_STATUS_REG2) as u16) << 8)
                | (Self::read_status(CMD_READ_STATUS_REG1) as u16);
            let reg = if on {
                reg | (1 << STATUS_QUAD_BIT)
            } else {
                reg & !(1 << STATUS_QUAD_BIT)
            };

            Self::write_status(CMD_WRITE_STATUS_REG1, reg);
        }

        /// Current value of the flash's Quad Enable bit. Mirrors
        /// `SFC_QuadState()`.
        #[inline(always)]
        fn quad_state() -> bool {
            Self::read_status(CMD_READ_STATUS_REG2) & 0x02 != 0
        }

        /// Erase `addr` with `cmd` (sector/32K block/64K block), or the whole
        /// chip when `addr == u32::MAX`. Mirrors `SFC_EraseEx()`.
        ///
        /// Returns as soon as the command has been issued; the flash keeps
        /// working until [`Self::wait_wip`] reports it idle.
        #[inline(always)]
        fn erase(addr: u32, cmd: u8) {
            let regs = unsafe { &*Self::regs() };
            let cmd_type = if addr == u32::MAX {
                CMDTYPE_CHIP_ERASE
            } else {
                CMDTYPE_SECTOR_ERASE
            };

            unsafe {
                // The datasheet wants the address written up front for every
                // command type; chip erase simply ignores it.
                regs.addr.write(|w| w.bits(addr));
                regs.cfg.modify(|_, w| {
                    w.wren()
                        .set_bit()
                        .cmdwren()
                        .set_bit()
                        .cmdtype()
                        .bits(cmd_type)
                });
                regs.cmd.write(|w| w.bits(cmd as u32));
            }

            start(regs);

            regs.cfg.modify(|_, w| w.wren().clear_bit());
        }

        /// Read `buf.len()` bytes starting at `addr` through the AHB window.
        ///
        /// The window only serves word accesses, so whole words are fetched and
        /// the requested sub-range is copied out; `addr` and the length may be
        /// arbitrary.
        #[inline(always)]
        fn read_memory(addr: u32, buf: &mut [u8]) {
            let mut word_addr = addr & !0x3;
            let mut offset = (addr & 0x3) as usize;
            let mut done = 0usize;

            while done < buf.len() {
                let word = unsafe { ptr::read_volatile((FLASH_BASE + word_addr) as *const u32) }
                    .to_le_bytes();
                let n = core::cmp::min(4 - offset, buf.len() - done);
                buf[done..done + n].copy_from_slice(&word[offset..offset + n]);

                done += n;
                offset = 0;
                word_addr += 4;
            }
        }

        /// Program `data` at `addr` through the AHB window (page program).
        ///
        /// The caller must ensure `data` fits inside a single 256-byte page and
        /// that the target range was erased first; the SFC issues the
        /// page-program opcode configured in `CMDAHB`. Bytes outside the
        /// requested range are padded with `0xFF`, which leaves an erased (or
        /// already programmed) flash byte untouched.
        #[inline(always)]
        fn program_memory(addr: u32, data: &[u8]) {
            let regs = unsafe { &*Self::regs() };

            // Enable writes so the AHB stores trigger a page program.
            regs.cfg.modify(|_, w| w.wren().set_bit());

            let mut word_addr = addr & !0x3;
            let mut offset = (addr & 0x3) as usize;
            let mut done = 0usize;

            while done < data.len() {
                let mut word = [0xFFu8; 4];
                let n = core::cmp::min(4 - offset, data.len() - done);
                word[offset..offset + n].copy_from_slice(&data[done..done + n]);
                unsafe {
                    ptr::write_volatile(
                        (FLASH_BASE + word_addr) as *mut u32,
                        u32::from_le_bytes(word),
                    )
                };

                done += n;
                offset = 0;
                word_addr += 4;
            }

            // SFC state machine settles, then the flash WIP clears.
            while regs.sr.read().busy().bit() {}
            regs.cfg.modify(|_, w| w.wren().clear_bit());
        }

        /// Read the controller's interrupt flags (`IF`).
        #[inline(always)]
        fn flags() -> Flags {
            let r = unsafe { &*Self::regs() }.if_.read();
            Flags {
                done: r.over().bit(),
                error: r.err().bit(),
                timeout: r.to().bit(),
            }
        }

        /// Clear the controller's interrupt flags (`IF` is write-one-to-clear).
        #[inline(always)]
        fn clear_flags() {
            unsafe { &*Self::regs() }
                .if_
                .write(|w| w.over().set_bit().err().set_bit().to().set_bit());
        }

        /// Wait for the flash to go idle using the default poll budget.
        #[inline(always)]
        fn wait_ready() -> Result<(), Error> {
            Self::wait_wip(WIP_POLL_LIMIT)
        }
    }
}

#[allow(private_bounds)]
/// A Serial Flash Controller usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for the SFC singleton.
macro_rules! impl_sfc_instance {
    ($ty:ty, $pac:ident, $irq:ident, $index:literal) => {
        crate::impl_instance_base!(
            $ty,
            crate::pac::sfc::RegisterBlock,
            crate::interrupt::$irq,
            PeripheralClockIndex::Sfc,
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

impl_sfc_instance!(crate::Sfc, SFC, FSPI, 0);
