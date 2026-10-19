//! Register access for the Serial Flash Controller (SFC).
//!
//! The SFC is the SWM341's hardware SPI master dedicated to NOR flash. Its pads
//! (D3..D8) are not routed to SPI0/SPI1 in the pin matrix, so this is the only
//! peripheral that can talk to the on-board W25Q128. The SFC has two faces:
//!
//! * a *manual command* path (`CFG.CMDWREN` + `CMD`/`GO`) that sends one opcode
//!   and optionally returns bytes in `DATA` -- used for JEDEC ID, status
//!   registers, and erase,
//! * an *AHB window* at `0x7000_0000` that transparently turns load/store
//!   instructions into read/program transactions (the XIP path).
//!
//! All register operations live in the default methods of the sealed `Instance`
//! trait; the public driver in [`crate::sfc`] only forwards to them.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type backing the SFC.
pub(crate) type Block = pac::sfc::RegisterBlock;

/// Base address of the SPI-flash AHB window.
pub(crate) const FLASH_BASE: u32 = 0x7000_0000;

/// Serial-flash opcodes shared with the vendor `SWM341_sfc.h`.
pub(crate) const CMD_READ_JEDEC: u8 = 0x9F;
pub(crate) const CMD_READ_SR1: u8 = 0x05;
pub(crate) const CMD_READ_SR2: u8 = 0x35;
pub(crate) const CMD_WRITE_SR1: u8 = 0x01;
pub(crate) const CMD_ERASE_SECTOR: u8 = 0x20;
pub(crate) const CMD_ERASE_BLOCK64K: u8 = 0xD8;
pub(crate) const CMD_ERASE_CHIP: u8 = 0x60;

/// `CFG.CMDTYPE` values for the manual-command path.
pub(crate) const CMDTYPE_READ_STATUS: u8 = 1; // one status byte -> DATA
pub(crate) const CMDTYPE_READ_JEDEC: u8 = 2; // three JEDEC bytes -> DATA
pub(crate) const CMDTYPE_WRITE_STATUS: u8 = 6; // DATA -> status register
pub(crate) const CMDTYPE_CHIP_ERASE: u8 = 5; // whole-chip erase
pub(crate) const CMDTYPE_SECTOR_ERASE: u8 = 7; // sector/block erase (uses ADDR)

pub(crate) mod sealed {

    use super::Block;
    use core::ptr;
    use cortex_m::asm;

    use super::{
        CMD_READ_JEDEC, CMD_READ_SR1, CMD_READ_SR2, CMD_WRITE_SR1, CMDTYPE_CHIP_ERASE,
        CMDTYPE_READ_JEDEC, CMDTYPE_READ_STATUS, CMDTYPE_SECTOR_ERASE, CMDTYPE_WRITE_STATUS,
        FLASH_BASE,
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

        /// Mux the dedicated Flash-SPI pads (D3..D8) onto the SFC and enable
        /// their digital inputs. Called once from the driver constructor.
        #[inline(always)]
        fn configure_pins() {
            let portd = unsafe { &*crate::pac::PORTD::ptr() };
            unsafe {
                // FUNC0 carries pads 0..=7, FUNC1 pads 8..=15 (4 bits each).
                // D3/D4 = FSPI_DATA2/3 (WP/HOLD), D5 = SCLK, D6 = SSEL,
                // D7 = MISO, D8 = MOSI.
                portd.func0.modify(|_, w| {
                    w.pin3()
                        .bits(2)
                        .pin4()
                        .bits(2)
                        .pin5()
                        .bits(1)
                        .pin6()
                        .bits(1)
                        .pin7()
                        .bits(3)
                });
                portd.func1.modify(|_, w| w.pin8().bits(2));
                portd.inen.modify(|_, w| {
                    w.pin3()
                        .set_bit()
                        .pin4()
                        .set_bit()
                        .pin7()
                        .set_bit()
                        .pin8()
                        .set_bit()
                });
            }
        }

        /// Program the controller: the vendor's undocumented `BASE + 0x3F4`
        /// register, the clock divider and data widths, the AHB read/program
        /// opcodes, and the WIP polling interval. Mirrors `SFC_Init()`.
        #[inline(always)]
        fn configure(
            clk_div: u8,
            read_width: u8,
            program_width: u8,
            read_cmd: u8,
            program_cmd: u8,
        ) {
            let regs = unsafe { &*Self::regs() };
            unsafe {
                // Undocumented register poked by every vendor SFC routine.
                ptr::write_volatile((regs as *const _ as *mut u8).offset(0x3F4) as *mut u32, 7);

                // Clear the clock-divider and data-width fields, then set them.
                let cfg = regs.cfg.read().bits();
                let cfg = (cfg & !(0x3 << 6) & !(0x1 << 9) & !(0x1 << 10))
                    | ((clk_div as u32 & 0x3) << 6)
                    | ((program_width as u32 & 0x1) << 9)
                    | ((read_width as u32 & 0x1) << 10);
                regs.cfg.write(|w| w.bits(cfg));

                // AHB opcodes: bits 8..15 = page-program opcode, 24..31 = read.
                regs.cmdahb
                    .modify(|_, w| w.pp().bits(program_cmd).read().bits(read_cmd));

                // WIP polling interval (~0.2 ms) and limit (vendor default).
                let mhz = crate::clock::sys_core_clock() / 1_000_000;
                regs.tim.modify(|_, w| {
                    w.wip_chk_itv()
                        .bits((mhz / 10) as u8)
                        .wip_chk_lmt()
                        .bits(255)
                });
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

        /// Three JEDEC ID bytes (manufacturer, memory type, capacity).
        #[inline(always)]
        fn read_jedec() -> u32 {
            Self::command(CMDTYPE_READ_JEDEC, CMD_READ_JEDEC)
        }

        /// One status register byte selected by `cmd` (e.g. `0x05` for SR1).
        #[inline(always)]
        fn read_status(cmd: u8) -> u8 {
            Self::command(CMDTYPE_READ_STATUS, cmd) as u8
        }

        /// True while the flash is busy with an erase or program.
        #[inline(always)]
        fn busy() -> bool {
            Self::read_status(CMD_READ_SR1) & 0x01 != 0
        }

        /// Erase `addr` with `cmd` (sector/block), or the whole chip when
        /// `addr == 0xFFFF_FFFF`. Mirrors `SFC_EraseEx()`.
        #[inline(always)]
        fn erase(addr: u32, cmd: u8) {
            let regs = unsafe { &*Self::regs() };
            let cmd_type = if addr == 0xFFFF_FFFF {
                CMDTYPE_CHIP_ERASE
            } else {
                CMDTYPE_SECTOR_ERASE
            };

            unsafe {
                if addr != 0xFFFF_FFFF {
                    regs.addr.write(|w| w.bits(addr));
                }
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

            // Wait for the flash itself to finish (WIP bit in SR1).
            while Self::busy() {}
        }

        /// Set the flash's Quad Enable (status-register-2) bit on (`on`).
        #[inline(always)]
        fn set_quad(on: bool) {
            let regs = unsafe { &*Self::regs() };
            let reg = (Self::read_status(CMD_READ_SR2) as u16) << 8
                | (Self::read_status(CMD_READ_SR1) as u16);
            let reg = if on { reg | (1 << 9) } else { reg & !(1 << 9) };

            unsafe {
                regs.cfg.modify(|_, w| {
                    w.wren()
                        .set_bit()
                        .cmdwren()
                        .set_bit()
                        .cmdtype()
                        .bits(CMDTYPE_WRITE_STATUS)
                });
                regs.data.write(|w| w.bits(reg as u32));
                regs.cmd.write(|w| w.bits(CMD_WRITE_SR1 as u32));
            }

            start(regs);

            regs.cfg.modify(|_, w| w.wren().clear_bit());
            while Self::busy() {}
        }

        /// Read `buf.len()` bytes starting at `addr` through the AHB window.
        #[inline(always)]
        fn read_memory(addr: u32, buf: &mut [u8]) {
            let base = (FLASH_BASE + addr) as *const u8;
            for (i, slot) in buf.iter_mut().enumerate() {
                *slot = unsafe { ptr::read_volatile(base.add(i)) };
            }
        }

        /// Program `data` at `addr` through the AHB window (page program).
        ///
        /// The caller must ensure `data` fits inside a single 256-byte page and
        /// that the target range was erased first; the SFC issues the
        /// page-program opcode configured in `CMDAHB`.
        #[inline(always)]
        fn program_memory(addr: u32, data: &[u8]) {
            let regs = unsafe { &*Self::regs() };

            // Enable writes so the AHB store triggers a page program.
            regs.cfg.modify(|_, w| w.wren().set_bit());

            let base = (FLASH_BASE + addr) as *mut u8;
            for (i, &byte) in data.iter().enumerate() {
                unsafe { ptr::write_volatile(base.add(i), byte) };
            }

            // SFC state machine settles, then the flash WIP clears.
            while regs.sr.read().busy().bit() {}
            regs.cfg.modify(|_, w| w.wren().clear_bit());
            while Self::busy() {}
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
