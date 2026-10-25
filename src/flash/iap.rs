//! Factory-ROM (IAP) routines used to modify and tune the flash array.
//!
//! The ROM lives at `0x1100_04xx`, outside the array the routines modify, so it
//! stays callable while flash is being reprogrammed -- which is the whole reason
//! this driver uses it instead of the register route (see the module docs). The
//! ROM exports no linker symbol, so the only way to reach an entry point is to
//! transmute its address; the vendor header does the same with a cast.

use super::types::{Error, Result};

/// Magic token every IAP routine expects as its last argument.
pub(crate) const IAP_TOKEN: u32 = 0x0B11_FFAC;

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

/// Erase one page, addressed by *page index*.
pub(crate) fn iap_flash_erase() -> IapFlashErase {
    unsafe { core::mem::transmute::<usize, IapFlashErase>(IAP_FLASH_ERASE) }
}

/// Program `groups` four-word groups from `ram` into flash at `addr`.
pub(crate) fn iap_flash_write() -> IapFlashWrite {
    unsafe { core::mem::transmute::<usize, IapFlashWrite>(IAP_FLASH_WRITE) }
}

/// Flush the flash cache; `cfg` is the current `FMC.CACHE` value OR-ed with the
/// clear bit (the routine writes the whole register).
pub(crate) fn iap_cache_reset() -> IapCacheReset {
    unsafe { core::mem::transmute::<usize, IapCacheReset>(IAP_CACHE_RESET) }
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
        let iap: IapFlashParam =
            unsafe { core::mem::transmute::<usize, IapFlashParam>(IAP_FLASH_PARAM) };
        unsafe { iap(cfg0, cfg1, IAP_TOKEN) };
    });
}

/// Map an IAP return value onto [`Result`]; the routines answer `0` on success.
pub(crate) fn iap_result(ret: i32) -> Result<()> {
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

#[cfg(test)]
mod tests {
    //! Pure-logic tests. `flash` has no hardware-free way to reach the ROM, so
    //! only the lookup tables and the return-value mapping are covered; see the
    //! `mod tests` in [`super::super`] for how these are run.
    use super::*;

    /// `(cfg0, cfg1)` per frequency bucket, straight out of the vendor library.
    const BUCKETS: [(u32, u32); 5] = [
        (0x8949, 0x0FA1),
        (0x8A49, 0x14A9),
        (0xD349, 0x294A),
        (0x11C89, 0x3D6B),
        (0x16589, 0x4C74),
    ];

    #[test]
    fn parameters_match_the_vendor_buckets() {
        assert_eq!(parameters_for(0), BUCKETS[0]);
        // Default out of reset: the ~20 MHz internal RC.
        assert_eq!(parameters_for(20_000_000), BUCKETS[0]);
        assert_eq!(parameters_for(120_000_000), BUCKETS[3]);
        // Anything past 120 MHz wants the slowest timings.
        assert_eq!(parameters_for(1_000_000_000), BUCKETS[4]);
    }

    #[test]
    fn each_bucket_starts_at_its_first_megahertz() {
        // First MHz of bucket N.. and the last MHz before it.
        for (edge, index) in [(31, 1), (41, 2), (81, 3), (121, 4)] {
            assert_eq!(parameters_for((edge - 1) * 1_000_000), BUCKETS[index - 1]);
            assert_eq!(parameters_for(edge * 1_000_000), BUCKETS[index]);
            // Sub-MHz jitter inside a bucket must not change the timings.
            assert_eq!(parameters_for(edge * 1_000_000 + 999_999), BUCKETS[index]);
        }
    }

    #[test]
    fn iap_return_value_maps_onto_result() {
        assert_eq!(iap_result(0), Ok(()));
        for ret in [1, -1, 2, i32::MIN, i32::MAX] {
            assert_eq!(iap_result(ret), Err(Error::Rejected));
        }
    }
}
