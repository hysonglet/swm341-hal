//! Register access for the flash controller (`FMC`).
//!
//! Every mutating operation follows the same protocol: unlock the array, call
//! the ROM routine (see [`iap`](super::iap)), lock it again and drop the cache.
//! The helpers below are the steps of that protocol; [`fmc`] is the raw handle
//! for the diagnostics exposed by [`Flash::status`](super::Flash::status).

use crate::pac;

use super::iap::{iap_cache_reset, IAP_TOKEN};

/// Value written to `FMC.CFG2` to allow erasing and programming.
const UNLOCK_KEY: u32 = 0x55FA_64B4;

/// `FMC.CACHE` bit that triggers a cache clear (vendor `FMC_CACHE_CCLR_Pos`,
/// datasheet `ICCLR`). The bit clears itself once the flush is done.
const CACHE_CCLR: u32 = 1 << 18;

/// The flash controller register block.
pub(crate) fn fmc() -> &'static pac::fmc::RegisterBlock {
    unsafe { &*pac::FMC::ptr() }
}

/// Allow the flash to be erased and programmed (`FMC.CFG2`).
///
/// The vendor library brackets the write with eight NOPs on each side; those are
/// reproduced here since the datasheet does not state the unlock latency.
pub(crate) fn unlock() {
    fmc().cfg2.write(|w| unsafe { w.bits(UNLOCK_KEY) });
    nops();
}

/// Put the flash back into read-only mode.
pub(crate) fn lock() {
    nops();
    fmc().cfg2.write(|w| unsafe { w.bits(0) });
}

/// Drop the cache, keeping the rest of `FMC.CACHE` as it is.
///
/// Must run with interrupts masked: the ROM routine waits for the flush, and a
/// fetch from flash while the array is being reprogrammed reads garbage.
pub(crate) fn cache_reset() {
    // `CACHE` carries the enable bits as well as the clear bit, and the ROM
    // routine writes the whole register, so the current value has to be OR-ed
    // in -- exactly what `SWM341_flash.c` does with `FMC->CACHE | CCLR_Msk`.
    let cfg = fmc().cache.read().bits() | CACHE_CCLR;
    unsafe { iap_cache_reset()(cfg, IAP_TOKEN) };
}

/// The eight NOPs the vendor library puts around every `CFG2` write.
fn nops() {
    for _ in 0..8 {
        cortex_m::asm::nop();
    }
}
