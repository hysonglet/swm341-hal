//! Register access for the Data Watchpoint and Trace (DWT) unit.
//!
//! The DWT is an Arm Cortex-M core debug component (the SWM341's STAR-MC1 core
//! implements the Armv8-M DWT), not a SWM341 peripheral. It therefore has no
//! clock gate in [`crate::clock`] and no NVIC vector; it is switched on through
//! the core's Debug Control Block (DCB) by setting `DEMCR.TRCENA`, and its
//! registers live at the fixed core address [`DWT_BASE`].
//!
//! The register block is modelled as a `#[repr(C)]` struct so the layout is
//! self-documenting. Fields are accessed with `addr_of!` + `volatile` reads /
//! writes because plain `&`-based access would let the compiler cache them.

// Fixed Arm core addresses.
const DWT_BASE: u32 = 0xE000_1000;
// DCB `DEMCR` is a separate core register that gates the trace clock.
const DCB_DEMCR: u32 = 0xE000_EDFC;
// `LAR` (lock access register) sits at the end of the DWT space; writing the
// unlock key there opens the unit's write-protected registers.
const LAR_OFFSET: u32 = 0xFB0;
const LAR_ABS: u32 = DWT_BASE + LAR_OFFSET;

// DWT->CTRL bits (stable across Armv7-M / Armv8-M).
const CTRL_NOCYCCNT: u32 = 1 << 1;
const CTRL_CYCCNTENA: u32 = 1 << 0;
const CTRL_CPIEVTENA: u32 = 1 << 24;
const CTRL_EXCEVTENA: u32 = 1 << 23;
const CTRL_SLEEPEVTENA: u32 = 1 << 22;
const CTRL_LSUEVTENA: u32 = 1 << 21;
const CTRL_FOLDEVTENA: u32 = 1 << 20;

// DCB->DEMCR: trace enable.
const DEMCR_TRCENA: u32 = 1 << 24;

// DWT->LAR unlock key.
const LAR_UNLOCK: u32 = 0xC5AC_CE55;

/// Highest comparator index the register map describes (16 comparators).
pub(crate) const MAX_COMPARATOR: u8 = 15;

/// One DWT comparator (watchpoint) slot.
///
/// `COMP` holds the address / data value, `MASK` the ignored low address bits
/// (range matching) and `FUNCTION` arms the comparator. The trailing word is
/// reserved on every slot, so the stride is exactly 16 bytes.
#[repr(C)]
struct Comparator {
    comp: u32,
    mask: u32,
    function: u32,
    _reserved: u32,
}

/// The DWT register block (Armv8-M layout).
///
/// Only the registers the driver touches are modelled; the rest of the block
/// (PID / CID, assertion control, ...) is not accessed and so omitted.
#[repr(C)]
struct RegisterBlock {
    /// Control and status.
    ctrl: u32,
    /// Free-running cycle counter.
    cyccnt: u32,
    /// Cycles-per-instruction profiling counter.
    cpicnt: u32,
    /// Exception profiling counter.
    exccnt: u32,
    /// Sleep profiling counter.
    sleepcnt: u32,
    /// Load/store-unit profiling counter.
    lsucnt: u32,
    /// Folded-instruction profiling counter.
    foldcnt: u32,
    _reserved0: u32,
    /// Comparator (watchpoint) bank.
    comp: [Comparator; 16],
}

#[inline(always)]
fn rb() -> *mut RegisterBlock {
    DWT_BASE as *mut RegisterBlock
}

#[inline(always)]
fn read_abs(address: u32) -> u32 {
    unsafe { core::ptr::read_volatile(address as *const u32) }
}

#[inline(always)]
fn write_abs(address: u32, value: u32) {
    unsafe { core::ptr::write_volatile(address as *mut u32, value) }
}

/// Volatile read of a scalar `RegisterBlock` field.
macro_rules! dwt_r {
    ($field:ident) => {{
        let r = unsafe { &*rb() };
        unsafe { core::ptr::read_volatile(core::ptr::addr_of!(r.$field)) }
    }};
}

/// Volatile write of a scalar `RegisterBlock` field.
macro_rules! dwt_w {
    ($field:ident, $value:expr) => {{
        let r = unsafe { &mut *rb() };
        unsafe { core::ptr::write_volatile(core::ptr::addr_of_mut!(r.$field), $value) };
    }};
}

#[inline(always)]
fn modify_ctrl(set: u32, clear: u32) {
    let v = dwt_r!(ctrl);
    dwt_w!(ctrl, (v & !clear) | set);
}

pub(crate) mod sealed {
    use super::*;

    /// The single DWT core component.
    ///
    /// Unlike the SWM341 peripherals it has no clock gate and no interrupt, so
    /// it deliberately does *not* extend [`crate::instance_base::InstanceBase`].
    pub trait Instance {
        /// Switch the debug trace clock on (`DCB.DEMCR.TRCENA`).
        #[inline]
        fn enable_trace() {
            write_abs(DCB_DEMCR, read_abs(DCB_DEMCR) | DEMCR_TRCENA);
        }

        /// Unlock the DWT write registers (`LAR = 0xC5ACCE55`).
        ///
        /// Some cores power up with the DWT locked; writing the unlock key is
        /// harmless when it is already open.
        #[inline]
        fn unlock() {
            write_abs(LAR_ABS, LAR_UNLOCK);
        }

        /// True when `CYCCNT` is implemented on this core (`CTRL.NOCYCCNT == 0`).
        #[inline]
        fn has_cycle_counter() -> bool {
            dwt_r!(ctrl) & CTRL_NOCYCCNT == 0
        }

        /// Enable or disable the free-running cycle counter (`CTRL.CYCCNTENA`).
        #[inline]
        fn set_cycle_counter_enable(on: bool) {
            if on {
                modify_ctrl(CTRL_CYCCNTENA, 0);
            } else {
                modify_ctrl(0, CTRL_CYCCNTENA);
            }
        }

        /// True while the cycle counter is running.
        #[inline]
        fn cycle_counter_enabled() -> bool {
            dwt_r!(ctrl) & CTRL_CYCCNTENA != 0
        }

        /// Read `CYCCNT`.
        #[inline]
        fn read_cycle_count() -> u32 {
            dwt_r!(cyccnt)
        }

        /// Write `CYCCNT`.
        #[inline]
        fn write_cycle_count(value: u32) {
            dwt_w!(cyccnt, value);
        }

        /// Enable or disable the CPI / exception / sleep / LSU / fold profiling
        /// counters (`CTRL.CPIEVTENA` + siblings).
        #[inline]
        fn set_counters_enable(on: bool) {
            let bits = CTRL_CPIEVTENA
                | CTRL_EXCEVTENA
                | CTRL_SLEEPEVTENA
                | CTRL_LSUEVTENA
                | CTRL_FOLDEVTENA;
            if on {
                modify_ctrl(bits, 0);
            } else {
                modify_ctrl(0, bits);
            }
        }

        /// Saturating cycles-per-instruction counter.
        #[inline]
        fn cpi_count() -> u8 {
            dwt_r!(cpicnt) as u8
        }
        /// Number of exceptions (excluding NMI / HardFault) since reset.
        #[inline]
        fn exception_count() -> u8 {
            dwt_r!(exccnt) as u8
        }
        /// Cycles spent in sleep mode.
        #[inline]
        fn sleep_count() -> u8 {
            dwt_r!(sleepcnt) as u8
        }
        /// Cycles spent on load / store unit operations.
        #[inline]
        fn lsu_count() -> u8 {
            dwt_r!(lsucnt) as u8
        }
        /// Cycles spent in folded instructions.
        #[inline]
        fn fold_count() -> u8 {
            dwt_r!(foldcnt) as u8
        }

        /// Program comparator `index` with `comp` / `mask` / `function`.
        ///
        /// `FUNCTION` is written last because writing it arms the comparator.
        #[inline]
        fn write_comparator(index: u8, comp: u32, mask: u8, function: u8) {
            let r = unsafe { &mut *rb() };
            let slot = &mut r.comp[index as usize];
            unsafe {
                core::ptr::write_volatile(core::ptr::addr_of_mut!(slot.comp), comp);
                core::ptr::write_volatile(core::ptr::addr_of_mut!(slot.mask), mask as u32);
                // `FUNCTION` last: writing it arms the comparator.
                core::ptr::write_volatile(core::ptr::addr_of_mut!(slot.function), function as u32);
            }
        }

        /// Disable comparator `index` (`FUNCTION = 0`).
        #[inline]
        fn clear_comparator(index: u8) {
            let r = unsafe { &mut *rb() };
            unsafe {
                core::ptr::write_volatile(
                    core::ptr::addr_of_mut!(r.comp[index as usize].function),
                    0,
                );
            }
        }
    }
}

#[allow(private_bounds)]
#[allow(dead_code)]
/// A DWT core component usable by the driver.
pub trait Instance: sealed::Instance {}

impl sealed::Instance for crate::Dwt {}
impl Instance for crate::Dwt {}
