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

mod iap;
mod instance;
mod types;

pub use iap::tune_for_frequency;
pub use types::{Error, Result, Status};

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
        let sector = page_index(addr);

        critical_section::with(|_| {
            instance::unlock();
            let ret = unsafe { iap::iap_flash_erase()(sector, iap::IAP_TOKEN) };
            instance::lock();
            instance::cache_reset();
            iap::iap_result(ret)
        })
    }

    /// Erase every page the half-open range `addr..addr + len` touches.
    ///
    /// A zero length erases nothing. This is the call to make before
    /// [`program`](Self::program), which cannot set bits back to `1`.
    pub fn erase_range(&self, addr: u32, len: u32) -> Result<()> {
        let Some((first, last)) = page_span(addr, len)? else {
            return Ok(());
        };
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
        let groups = program_groups(addr, words.len())?;
        if groups == 0 {
            return Ok(());
        }

        let ram = words.as_ptr() as u32;

        critical_section::with(|_| {
            instance::unlock();
            let ret = unsafe { iap::iap_flash_write()(addr, ram, groups, iap::IAP_TOKEN) };
            instance::lock();
            instance::cache_reset();
            iap::iap_result(ret)
        })
    }

    /// Current state of the flash controller.
    pub fn status(&self) -> Status {
        Status::from_bits(instance::fmc().stat.read().bits())
    }

    /// Flush the flash cache.
    ///
    /// The datasheet calls this out after every program: until the cache is
    /// flushed a read can still hand back the value that was there before the
    /// write. [`erase_page`](Self::erase_page) and [`program`](Self::program)
    /// already do it, so this only matters when the array was changed by
    /// something else (a bootloader, or the register route).
    pub fn cache_clear(&self) {
        critical_section::with(|_| instance::cache_reset());
    }
}

/// Index of the page holding `addr`.
const fn page_index(addr: u32) -> u32 {
    addr / PAGE_SIZE
}

/// Inclusive range of page indices the half-open byte range `addr..addr + len`
/// touches, or `None` when there is nothing to erase.
///
/// Kept apart from [`Flash::erase_range`] so the page arithmetic can be
/// exercised without talking to hardware.
fn page_span(addr: u32, len: u32) -> Result<Option<(u32, u32)>> {
    if len == 0 {
        return Ok(None);
    }

    let end = addr.checked_add(len).ok_or(Error::OutOfRange)?;
    if end > FLASH_SIZE {
        return Err(Error::OutOfRange);
    }

    Ok(Some((page_index(addr), page_index(end - 1))))
}

/// Number of four-word groups a program of `words.len()` words writes, once
/// every datasheet constraint has been checked. Zero means "nothing to do".
///
/// Kept apart from [`Flash::program`] for the same reason as [`page_span`].
fn program_groups(addr: u32, words: usize) -> Result<u32> {
    if addr % PROGRAM_ALIGN != 0 {
        return Err(Error::Unaligned);
    }
    if words % WORDS_PER_GROUP != 0 {
        return Err(Error::InvalidLength);
    }
    if words == 0 {
        return Ok(0);
    }

    let len = (words * core::mem::size_of::<u32>()) as u32;
    if addr.checked_add(len).ok_or(Error::OutOfRange)? > FLASH_SIZE {
        return Err(Error::OutOfRange);
    }

    Ok((words / WORDS_PER_GROUP) as u32)
}

#[cfg(test)]
mod tests {
    //! Unit tests for everything that does not need real hardware.
    //!
    //! `.cargo/config.toml` pins the default target to the microcontroller, and
    //! a `no_std` target has no `test` crate, so these run on the host:
    //!
    //! ```text
    //! cargo test --target aarch64-apple-darwin --lib
    //! ```
    //!
    //! Everything that would touch the ROM or the `FMC` registers is left out
    //! on purpose: calling it off-target dereferences address 0 (or worse, calls
    //! `0x1100_04xx`). Only the arithmetic and the checks that run *before* the
    //! hardware is reached are covered.

    use super::*;

    #[test]
    fn geometry_is_self_consistent() {
        assert_eq!(FLASH_SIZE, 512 * 1024);
        assert_eq!(PAGE_COUNT * PAGE_SIZE, FLASH_SIZE);
        assert_eq!(PAGE_COUNT, 128);
        assert_eq!(PROGRAM_ALIGN, 16);
        assert_eq!(WORDS_PER_GROUP, 4);
        // A page holds a whole number of program groups, so erasing never
        // leaves a partially writable group behind.
        assert_eq!(PAGE_SIZE % PROGRAM_ALIGN, 0);
    }

    #[test]
    fn erase_page_checks_before_touching_hardware() {
        let flash = Flash::new();

        assert_eq!(flash.erase_page(1), Err(Error::Unaligned));
        assert_eq!(flash.erase_page(PAGE_SIZE - 1), Err(Error::Unaligned));
        assert_eq!(flash.erase_page(FLASH_SIZE), Err(Error::OutOfRange));

        assert_eq!(page_index(0), 0);
        assert_eq!(page_index(PAGE_SIZE - 1), 0);
        assert_eq!(page_index(PAGE_SIZE), 1);
        assert_eq!(page_index(FLASH_SIZE - PAGE_SIZE), PAGE_COUNT - 1);
    }

    #[test]
    fn read_checks_range_before_dereferencing() {
        let flash = Flash::new();
        let mut buf = [0u8; 4];

        // Past the end, and straddling the end. None of these may dereference.
        assert_eq!(flash.read(FLASH_SIZE, &mut buf), Err(Error::OutOfRange));
        assert_eq!(flash.read(FLASH_SIZE - 1, &mut buf), Err(Error::OutOfRange));
        assert_eq!(flash.read(u32::MAX, &mut buf), Err(Error::OutOfRange));

        // An empty buffer always fits.
        assert_eq!(flash.read(0, &mut []), Ok(()));
    }

    #[test]
    fn page_span_covers_the_whole_range() {
        assert_eq!(page_span(0, 0), Ok(None));
        assert_eq!(page_span(FLASH_SIZE, 0), Ok(None));

        assert_eq!(page_span(0, 1), Ok(Some((0, 0))));
        assert_eq!(page_span(PAGE_SIZE - 1, 1), Ok(Some((0, 0))));
        // A range that ends exactly on a boundary must not erase past it.
        assert_eq!(page_span(0, PAGE_SIZE), Ok(Some((0, 0))));
        assert_eq!(page_span(PAGE_SIZE, 1), Ok(Some((1, 1))));
        assert_eq!(page_span(0, PAGE_SIZE + 1), Ok(Some((0, 1))));
        assert_eq!(page_span(0, FLASH_SIZE), Ok(Some((0, PAGE_COUNT - 1))));
        assert_eq!(
            page_span(FLASH_SIZE - PAGE_SIZE, PAGE_SIZE),
            Ok(Some((PAGE_COUNT - 1, PAGE_COUNT - 1)))
        );

        assert_eq!(page_span(0, FLASH_SIZE + 1), Err(Error::OutOfRange));
        assert_eq!(page_span(u32::MAX, 1), Err(Error::OutOfRange));
    }

    #[test]
    fn program_groups_enforces_the_datasheet_rules() {
        assert_eq!(program_groups(0, 4), Ok(1));
        assert_eq!(program_groups(16, 8), Ok(2));
        assert_eq!(program_groups(FLASH_SIZE - 16, 4), Ok(1));
        // Nothing to write is not an error.
        assert_eq!(program_groups(0, 0), Ok(0));

        assert_eq!(program_groups(4, 4), Err(Error::Unaligned));
        assert_eq!(program_groups(8, 4), Err(Error::Unaligned));
        // Whole groups only: three words leave a group half written.
        assert_eq!(program_groups(16, 3), Err(Error::InvalidLength));
        assert_eq!(program_groups(16, 7), Err(Error::InvalidLength));

        assert_eq!(program_groups(FLASH_SIZE, 4), Err(Error::OutOfRange));
        assert_eq!(program_groups(FLASH_SIZE - 16, 8), Err(Error::OutOfRange));
    }

    #[test]
    fn program_checks_are_reachable_through_the_handle() {
        let flash = Flash::new();

        assert_eq!(flash.program(8, &[0x1234_5678; 4]), Err(Error::Unaligned));
        assert_eq!(flash.program(16, &[0, 1, 2]), Err(Error::InvalidLength));
        assert_eq!(flash.program(FLASH_SIZE, &[0; 4]), Err(Error::OutOfRange));
        // Empty payload short-circuits before anything else.
        assert_eq!(flash.program(0, &[]), Ok(()));
        assert_eq!(flash.program(FLASH_SIZE, &[]), Ok(()));
    }
}
