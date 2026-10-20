//! Embedded flash (eFlash): wait-state tuning, erase and programming.
//!
//! Chapter 6.29 of the SWM341 datasheet describes a 512 KB *parallel*-interface
//! flash array mapped at [`FLASH_BASE`] and driven by the `FMC` register block
//! at `0x4004_A000`. Reads are ordinary memory reads; erasing and programming
//! are not, because the array cannot be served while it is being modified.
//!
//! The datasheet documents two routes for modifying the array:
//!
//! * a register route (`CACHE.PROGEN` write-enable, `ERASE`/`ADDR`/`DATA` and
//!   the write FIFO, then poll `STAT`), and
//! * IAP routines resident in the factory ROM.
//!
//! This driver only uses the second one. The register route carries the note
//! "以上操作均需要在SRAM中执行" -- every step must execute from SRAM, since the
//! core cannot fetch instructions out of the very array being reprogrammed.
//! Rust gives no way to promise that a function is linked into RAM, so that
//! route is unusable here. The ROM routines live at `0x1100_04xx`, outside the
//! array, so they are safe to call from flash; the vendor library
//! (`SWM341_flash.c`) uses exactly this route.
//!
//! Both routes still need interrupts off for the duration of the operation --
//! an ISR that fetches from flash mid-program would read garbage -- so every
//! mutating call below runs inside a [`critical_section`].
//!
//! # Granularity (datasheet 6.29.3)
//!
//! | Operation | Alignment     | Unit                              |
//! |-----------|---------------|-----------------------------------|
//! | Erase     | [`PAGE_SIZE`] | one 4 KB page                     |
//! | Program   | 16 bytes      | N words, N a multiple of 4        |
//! | Read      | none          | byte                              |
//!
//! Programming only ever clears bits (`1 -> 0`), never sets them, so the target
//! must be erased first.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! let flash = hal::flash::Flash::new();
//!
//! let mut buf = [0u8; 16];
//! flash.read(0x0004_0000, &mut buf).unwrap(); // plain memory read
//!
//! flash.erase_page(0x0004_0000).unwrap();     // 4 KB, must be page aligned
//! flash.program(0x0004_0000, &[0x1234_5678; 4]).unwrap();
//! ```

use crate::pac;

/// Address the flash array is mapped at.
pub const FLASH_BASE: u32 = 0x0000_0000;

/// Size of the flash array: 512 KB on every SWM341 part.
pub const FLASH_SIZE: u32 = 512 * 1024;

/// Erase granularity. One page erase clears [`PAGE_SIZE`] bytes.
pub const PAGE_SIZE: u32 = 4096;

/// Number of erasable pages in the array (512 KB / 4 KB).
pub const PAGE_COUNT: u32 = FLASH_SIZE / PAGE_SIZE;

/// Programming granularity, in bytes: the datasheet requires the address to be
/// "four-word aligned" and the payload to be a whole number of four-word groups.
pub const PROGRAM_ALIGN: u32 = 16;

/// Words per program group (`PROGRAM_ALIGN / 4`).
const WORDS_PER_GROUP: usize = (PROGRAM_ALIGN / 4) as usize;

/// Magic token every IAP routine expects as its last argument.
const IAP_TOKEN: u32 = 0x0B11_FFAC;

/// Value written to `FMC.CFG2` to allow erasing and programming.
const UNLOCK_KEY: u32 = 0x55FA_64B4;

/// `FMC.CACHE` bit that triggers a cache clear (vendor `FMC_CACHE_CCLR_Pos`,
/// datasheet `ICCLR`). The bit clears itself once the flush is done.
const CACHE_CCLR: u32 = 1 << 18;

// Factory-ROM entry points. All four already have the Thumb bit (bit 0) set, so
// they double as function pointers without fixup. The addresses are identical to
// the ones the vendor header binds (`SWM341_flash.c`).
const IAP_CACHE_RESET: usize = 0x1100_0401;
const IAP_FLASH_PARAM: usize = 0x1100_0431;
const IAP_FLASH_ERASE: usize = 0x1100_0471;
const IAP_FLASH_WRITE: usize = 0x1100_04C1;

/// `void IAP_Cache_Reset(uint32_t cfg, uint32_t flag)`
type IapCacheReset = unsafe extern "C" fn(cfg: u32, token: u32);

/// `int IAP_Flash_Param(uint32_t cfg0, uint32_t cfg1, uint32_t flag)`
type IapFlashParam = unsafe extern "C" fn(cfg0: u32, cfg1: u32, token: u32) -> i32;

/// `int IAP_Flash_Erase(uint32_t sector, uint32_t flag)`
type IapFlashErase = unsafe extern "C" fn(sector: u32, token: u32) -> i32;

/// `int IAP_Flash_Write(uint32_t flash_addr, uint32_t ram_addr, uint32_t count, uint32_t flag)`
type IapFlashWrite = unsafe extern "C" fn(addr: u32, ram: u32, groups: u32, token: u32) -> i32;

// The ROM has no linker symbol, so each address can only become callable by
// transmutation -- this matches what the vendor header does with a cast.

fn iap_cache_reset() -> IapCacheReset {
    unsafe { core::mem::transmute::<usize, IapCacheReset>(IAP_CACHE_RESET) }
}

fn iap_flash_erase() -> IapFlashErase {
    unsafe { core::mem::transmute::<usize, IapFlashErase>(IAP_FLASH_ERASE) }
}

fn iap_flash_write() -> IapFlashWrite {
    unsafe { core::mem::transmute::<usize, IapFlashWrite>(IAP_FLASH_WRITE) }
}

fn iap_flash_param() -> IapFlashParam {
    unsafe { core::mem::transmute::<usize, IapFlashParam>(IAP_FLASH_PARAM) }
}

/// The flash controller register block.
fn fmc() -> &'static pac::fmc::RegisterBlock {
    unsafe { &*pac::FMC::ptr() }
}

/// Errors a flash operation can report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The address does not meet the alignment the operation requires:
    /// [`PAGE_SIZE`] for an erase, [`PROGRAM_ALIGN`] for a program.
    Unaligned,
    /// The requested range reaches past the end of the 512 KB array.
    OutOfRange,
    /// The payload is not a whole number of four-word program groups.
    InvalidLength,
    /// The ROM routine rejected the request (it returned non-zero).
    Rejected,
}

/// Shorthand for a fallible flash operation.
pub type Result<T> = core::result::Result<T, Error>;

/// Snapshot of the `FMC.STAT` register (datasheet 6.29.5).
///
/// The IAP routines block until they are done, so callers rarely need this; it
/// is exposed for diagnosing a flash that appears stuck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    bits: u32,
}

impl Status {
    /// True when the eFlash is idle (`STAT.IDLE`).
    pub fn idle(&self) -> bool {
        self.bits & (1 << 31) != 0
    }

    /// True while an erase is in progress (`STAT.ERASEBUSY`).
    pub fn erase_busy(&self) -> bool {
        self.bits & (1 << 0) != 0
    }

    /// True while a program is in progress (`STAT.PROGBUSY`).
    pub fn program_busy(&self) -> bool {
        self.bits & (1 << 1) != 0
    }

    /// True while a read is in progress (`STAT.READBUSY`).
    pub fn read_busy(&self) -> bool {
        self.bits & (1 << 2) != 0
    }

    /// True when the write FIFO holds no data (`STAT.FIFOEMPTY`).
    pub fn fifo_empty(&self) -> bool {
        self.bits & (1 << 3) != 0
    }

    /// True when the write FIFO cannot accept more data (`STAT.FIFOFULL`).
    pub fn fifo_full(&self) -> bool {
        self.bits & (1 << 4) != 0
    }

    /// Raw register value.
    pub fn bits(&self) -> u32 {
        self.bits
    }
}

/// A handle to the embedded flash.
///
/// The array is a single global resource with no clock gate of its own (the
/// `FMC` block is not among `SYS.CLKEN`'s gates) and no interrupt, so this
/// handle is zero-sized: any number of them may exist, and they all reach the
/// same hardware.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flash;

impl Flash {
    /// Take a handle to the embedded flash.
    pub fn new() -> Self {
        Self
    }

    /// Read `buf.len()` bytes out of the flash array starting at `addr`.
    ///
    /// The array is memory mapped, so this is a volatile byte copy; no unlock,
    /// cache or interrupt handling is involved.
    pub fn read(&self, addr: u32, buf: &mut [u8]) -> Result<()> {
        let end = addr.checked_add(buf.len() as u32).ok_or(Error::OutOfRange)?;
        if end > FLASH_SIZE {
            return Err(Error::OutOfRange);
        }

        let src = (FLASH_BASE + addr) as *const u8;
        for (i, byte) in buf.iter_mut().enumerate() {
            *byte = unsafe { src.add(i).read_volatile() };
        }
        Ok(())
    }

    /// Erase the single 4 KB page containing `addr`.
    ///
    /// `addr` must be [`PAGE_SIZE`]-aligned and inside the array. Erasing turns
    /// every bit in the page to `1`.
    pub fn erase_page(&self, addr: u32) -> Result<()> {
        if addr % PAGE_SIZE != 0 {
            return Err(Error::Unaligned);
        }
        if addr >= FLASH_SIZE {
            return Err(Error::OutOfRange);
        }

        // The ROM routine takes a *page index*, not an address -- the vendor
        // driver passes `addr / 4096` even though the datasheet text says
        // "address".
        let sector = addr / PAGE_SIZE;

        critical_section::with(|_| {
            unlock();
            let ret = unsafe { iap_flash_erase()(sector, IAP_TOKEN) };
            lock();
            cache_reset();
            iap_result(ret)
        })
    }

    /// Erase every page the half-open range `addr..addr + len` touches.
    ///
    /// A zero length erases nothing. This is the call to make before
    /// [`program`](Self::program), which cannot set bits back to `1`.
    pub fn erase_range(&self, addr: u32, len: u32) -> Result<()> {
        if len == 0 {
            return Ok(());
        }

        let end = addr.checked_add(len).ok_or(Error::OutOfRange)?;
        if end > FLASH_SIZE {
            return Err(Error::OutOfRange);
        }

        let first = addr / PAGE_SIZE;
        let last = (end - 1) / PAGE_SIZE;
        for page in first..=last {
            self.erase_page(page * PAGE_SIZE)?;
        }
        Ok(())
    }

    /// Program `words` into the flash starting at `addr`.
    ///
    /// `addr` must be [`PROGRAM_ALIGN`] (16 byte / four word) aligned and
    /// `words.len()` must be a multiple of four; the datasheet states both as
    /// hard requirements of the program path. The destination must already be
    /// erased -- programming only clears bits.
    ///
    /// The data buffer is read by the ROM routine, so it lives wherever the
    /// caller put it (flash or RAM both work); only the routine itself has to
    /// stay out of the array.
    pub fn program(&self, addr: u32, words: &[u32]) -> Result<()> {
        if addr % PROGRAM_ALIGN != 0 {
            return Err(Error::Unaligned);
        }
        if words.len() % WORDS_PER_GROUP != 0 {
            return Err(Error::InvalidLength);
        }
        if words.is_empty() {
            return Ok(());
        }

        let len = (words.len() * core::mem::size_of::<u32>()) as u32;
        if addr.checked_add(len).ok_or(Error::OutOfRange)? > FLASH_SIZE {
            return Err(Error::OutOfRange);
        }

        let ram = words.as_ptr() as u32;
        let groups = (words.len() / WORDS_PER_GROUP) as u32;

        critical_section::with(|_| {
            unlock();
            let ret = unsafe { iap_flash_write()(addr, ram, groups, IAP_TOKEN) };
            lock();
            cache_reset();
            iap_result(ret)
        })
    }

    /// Current state of the flash controller.
    pub fn status(&self) -> Status {
        Status {
            bits: fmc().stat.read().bits(),
        }
    }

    /// Flush the flash cache.
    ///
    /// The datasheet calls this out after every program: until the cache is
    /// flushed a read can still hand back the value that was there before the
    /// write. [`erase_page`](Self::erase_page) and [`program`](Self::program)
    /// already do it, so this only matters when the array was changed by
    /// something else (a bootloader, or the register route).
    pub fn cache_clear(&self) {
        critical_section::with(|_| cache_reset());
    }
}

/// Allow the flash to be erased and programmed (`FMC.CFG2`).
///
/// The vendor library brackets the write with eight NOPs on each side; those are
/// reproduced here since the datasheet does not state the unlock latency.
fn unlock() {
    fmc().cfg2.write(|w| unsafe { w.bits(UNLOCK_KEY) });
    nops();
}

/// Put the flash back into read-only mode.
fn lock() {
    nops();
    fmc().cfg2.write(|w| unsafe { w.bits(0) });
}

/// The eight NOPs the vendor library puts around every `CFG2` write.
fn nops() {
    for _ in 0..8 {
        cortex_m::asm::nop();
    }
}

/// Drop the cache, keeping the rest of `FMC.CACHE` as it is.
///
/// Must run with interrupts masked: the ROM routine waits for the flush, and a
/// fetch from flash while the array is being reprogrammed reads garbage.
fn cache_reset() {
    // `CACHE` carries the enable bits as well as the clear bit, and the ROM
    // routine writes the whole register, so the current value has to be OR-ed
    // in -- exactly what `SWM341_flash.c` does with `FMC->CACHE | CCLR_Msk`.
    let cfg = fmc().cache.read().bits() | CACHE_CCLR;
    unsafe { iap_cache_reset()(cfg, IAP_TOKEN) };
}

/// Map an IAP return value onto [`Result`]; the routines answer `0` on success.
fn iap_result(ret: i32) -> Result<()> {
    if ret == 0 {
        Ok(())
    } else {
        Err(Error::Rejected)
    }
}

/// Opaque vendor parameters indexed by core frequency, copied from
/// `Flash_Param_at_xMHz()` in the reference library.
fn parameters_for(frequency_hz: u32) -> (u32, u32) {
    match frequency_hz / 1_000_000 {
        0..=30 => (0x8949, 0x0FA1),
        31..=40 => (0x8A49, 0x14A9),
        41..=80 => (0xD349, 0x294A),
        81..=120 => (0x11C89, 0x3D6B),
        _ => (0x16589, 0x4C74),
    }
}

/// Reprogram the flash timings for `frequency_hz`.
///
/// The timing parameters that let the flash array keep up with the core clock
/// are not exposed through documented registers; only a routine in the factory
/// bootloader ROM can set them. Every frequency change must be accompanied by
/// such a call, otherwise instruction fetches return garbage once the core
/// leaves the ~20 MHz default range.
///
/// # Safety
///
/// Interrupts are disabled for the duration of the ROM call because code
/// executing from flash must not be fetched while the array is reprogrammed.
/// Calling this with a frequency *lower* than the one the core actually runs at
/// will corrupt instruction fetches.
pub unsafe fn tune_for_frequency(frequency_hz: u32) {
    let (cfg0, cfg1) = parameters_for(frequency_hz);
    critical_section::with(|_| {
        iap_flash_param()(cfg0, cfg1, IAP_TOKEN);
    });
}
