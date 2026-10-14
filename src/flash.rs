//! Wait-state parameters of the embedded flash.
//!
//! The timing parameters that let the flash array keep up with the core clock
//! are not exposed through documented registers; the vendor library reaches
//! them through a routine that lives in the factory bootloader ROM at
//! `0x1100_0431`, guarded by a magic token. Every frequency change must be
//! accompanied by such a call, otherwise instruction fetches return garbage
//! once the core leaves the ~20 MHz default range.

/// Magic token every bootloader call expects as third argument.
const IAP_TOKEN: u32 = 0x0B11_FFAC;

/// Signature of the bootloader's flash parameter routine.
type IapFlashParam = unsafe extern "C" fn(cfg0: u32, cfg1: u32, token: u32) -> i32;

/// Address of the routine inside the factory ROM; bit 0 is set because the
/// target only executes Thumb code, so the address doubles as a function
/// pointer without further fixup.
fn iap_flash_param() -> IapFlashParam {
    // The ROM has no linker symbol, so the address can only become callable by
    // transmutation. This matches what the vendor header does with a cast.
    unsafe { core::mem::transmute::<usize, IapFlashParam>(0x1100_0431) }
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
