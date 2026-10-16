//! System clock tree.
//!
//! The tree has two stages: `CLKSEL.SYS` picks either the internal RC directly
//! or whatever `CLKSEL.CLK` has selected, and that intermediate clock can be
//! divided by eight through `CLKSEL.CLK_DIVx`. Because only the RC bypass has a
//! guaranteed source, every other configuration is reached by bringing the new
//! source up first, switching the core onto the RC, then moving it to the target
//! -- which is exactly the sequence the vendor library implements and which
//! [`init`] reproduces.
//!
//! ```text
//!   LSI 32k -+
//!   PLL     -+-[ CLK mux ]-+-[/8]-+
//!   LSE 32k -+                    +-[ CLKSEL.SYS ]-> core clock
//!   XTAL    -+                    |
//!   HRC --------------------------+
//! ```

mod peripheral;

pub use peripheral::PeripheralClockIndex;

use crate::flash;
use crate::pac;
use core::sync::atomic::{AtomicU32, Ordering};

/// Register block of the system controller.
///
/// `svd2rust` models every peripheral as a type carrying a `PTR` constant rather
/// than as a value, which is why the address has to be unwrapped here. This is
/// the only place in the HAL that borrows `SYS`, whose registers are the clock
/// tree and the oscillator control block at `SYS + 0xAA000`.
#[inline]
pub(crate) fn sys() -> &'static pac::sys::RegisterBlock {
    // SAFETY: SYS is always mapped, and nothing hands the block out elsewhere.
    unsafe { &*pac::SYS::PTR }
}

/// Internal RC in its low gear.
pub const HRC_20MHZ: u32 = 20_000_000;
/// Internal RC in its high gear (`HRCCR.DBL`).
pub const HRC_40MHZ: u32 = 40_000_000;
/// Internal low speed RC.
pub const LSI_HZ: u32 = 32_000;
/// Typical 32.768 kHz watch crystal; only relevant if it is fitted.
pub const LSE_HZ: u32 = 32_768;
/// Highest core frequency the datasheet guarantees.
pub const MAX_SYSTEM_CLOCK_HZ: u32 = 150_000_000;

/// Values of the 3-bit `CLKSEL.CLK` mux.
const CLK_SOURCE_LSI: u8 = 0;
const CLK_SOURCE_PLL: u8 = 1;
const CLK_SOURCE_LSE: u8 = 2;
const CLK_SOURCE_XTAL: u8 = 3;
const CLK_SOURCE_HRC: u8 = 4;

/// Clock source selected by [`ClockConfig::source`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockSource {
    /// Internal RC straight onto the core.
    Hrc,
    /// Internal RC through the `CLK`/8 path.
    HrcDiv8,
    /// External crystal through the `CLK` mux.
    Xtal(XtalSource),
    /// PLL through the `CLK` mux.
    Pll(PllConfig),
}

/// Speed of the internal high speed RC.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HrcSpeed {
    /// 20 MHz, the hardware default.
    Hz20,
    /// 40 MHz, obtained by doubling the same RC.
    Hz40,
}

impl HrcSpeed {
    #[inline]
    pub const fn frequency_hz(self) -> u32 {
        match self {
            Self::Hz20 => HRC_20MHZ,
            Self::Hz40 => HRC_40MHZ,
        }
    }

    #[inline]
    const fn doubled(self) -> bool {
        matches!(self, Self::Hz40)
    }
}

/// Post-divider of the PLL.
///
/// The `PLLDIV.OUTDIV` codes map onto a plain division factor, see
/// [`PllOutDiv::factor`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PllOutDiv {
    /// Divide the VCO by eight.
    Div8,
    /// Divide the VCO by four.
    Div4,
    /// Divide the VCO by two.
    Div2,
}

impl PllOutDiv {
    #[inline]
    const fn code(self) -> u8 {
        match self {
            Self::Div8 => 0,
            Self::Div4 => 1,
            Self::Div2 => 2,
        }
    }

    #[inline]
    const fn factor(self) -> u32 {
        // Codes are reversed relative to the factor: 0 -> /8, 1 -> /4, 2 -> /2.
        2 << (2 - self.code())
    }
}

/// Input of the PLL.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PllSource {
    /// Internal RC, in whichever gear [`ClockConfig::hrc`] selects.
    Hrc,
    /// External crystal.
    Xtal,
}

/// PLL configuration.
///
/// Resulting frequency is `source / input_divider * feedback_divider * 4 /
/// output_divider.factor()`, optionally divided by eight afterwards.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PllConfig {
    pub source: PllSource,
    pub input_divider: u32,
    pub feedback_divider: u32,
    pub output_divider: PllOutDiv,
    /// Feed the PLL output through `CLKSEL.CLK_DIVx`.
    pub output_div8: bool,
}

impl PllConfig {
    #[inline]
    pub const fn new(
        source: PllSource,
        input_divider: u32,
        feedback_divider: u32,
        output_divider: PllOutDiv,
        output_div8: bool,
    ) -> Self {
        Self {
            source,
            input_divider,
            feedback_divider,
            output_divider,
            output_div8,
        }
    }

    /// 150 MHz driven from the external 12 MHz crystal (12 / 3 * 75 * 4 / 8).
    ///
    /// The internal RC is an unreliable PLL reference on this silicon: feeding the
    /// PLL from `Hrc` leaves the core on its slow crystal fallback clock, so the
    /// vendor reference library drives the PLL from `Xtal` instead. Mirror that here.
    #[inline]
    pub const fn hrc_150mhz() -> Self {
        Self::new(PllSource::Xtal, 3, 75, PllOutDiv::Div8, false)
    }

    /// 120 MHz driven from the external 12 MHz crystal (12 / 3 * 60 * 4 / 8).
    ///
    /// Same reasoning as [`PllConfig::hrc_150mhz`]: the PLL reference is the crystal,
    /// not the internal RC. These are exactly the vendor library's `SYS_CLK_PLL` values.
    #[inline]
    pub const fn hrc_120mhz() -> Self {
        Self::new(PllSource::Xtal, 3, 60, PllOutDiv::Div8, false)
    }

    /// 80 MHz, the configuration the vendor's own example uses.
    #[inline]
    pub const fn hrc_80mhz() -> Self {
        Self::new(PllSource::Hrc, 5, 40, PllOutDiv::Div8, false)
    }
}

/// External crystal configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct XtalSource {
    /// Frequency of the fitted crystal, 2..32 MHz according to the datasheet.
    pub frequency_hz: u32,
    /// Feed the crystal through `CLKSEL.CLK_DIVx`.
    pub output_div8: bool,
}

impl XtalSource {
    #[inline]
    pub const fn new(frequency_hz: u32, output_div8: bool) -> Self {
        Self {
            frequency_hz,
            output_div8,
        }
    }
}

/// Reasons why [`init`] refuses a configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClockError {
    /// Requested core clock exceeds [`MAX_SYSTEM_CLOCK_HZ`].
    TooFast,
    /// A PLL divider is out of the range its bit field can encode.
    DividerOutOfRange,
}

/// Full clock configuration handed to [`crate::init`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClockConfig {
    /// Gear of the internal RC. It stays enabled no matter what the core runs
    /// from, since every clock switch falls back onto it.
    pub hrc: HrcSpeed,
    /// Frequency of the fitted external crystal.
    pub xtal_frequency_hz: u32,
    /// Source of the core clock.
    pub source: ClockSource,
    /// Reprogram flash wait states (a bootloader ROM call) around every
    /// frequency change. Leave enabled unless it is already known to be done.
    pub tune_flash: bool,
}

impl ClockConfig {
    /// Core frequency this configuration results in, in hertz.
    pub const fn core_frequency_hz(&self) -> u32 {
        match self.source {
            ClockSource::Hrc => self.hrc.frequency_hz(),
            ClockSource::HrcDiv8 => self.hrc.frequency_hz() / 8,
            ClockSource::Xtal(xtal) => {
                if xtal.output_div8 {
                    xtal.frequency_hz / 8
                } else {
                    xtal.frequency_hz
                }
            }
            ClockSource::Pll(pll) => {
                let input = match pll.source {
                    PllSource::Hrc => self.hrc.frequency_hz(),
                    PllSource::Xtal => self.xtal_frequency_hz,
                };
                if pll.input_divider == 0 {
                    return 0;
                }
                let vco = input as u64 / pll.input_divider as u64 * pll.feedback_divider as u64 * 4
                    / pll.output_divider.factor() as u64;
                if pll.output_div8 {
                    (vco / 8) as u32
                } else {
                    vco as u32
                }
            }
        }
    }

    /// 150 MHz driven from the external 12 MHz crystal via the PLL (the datasheet
    /// maximum). The PLL reference is the crystal, not the internal RC — see
    /// [`PllConfig::hrc_150mhz`].
    #[inline]
    pub const fn hrc_150mhz() -> Self {
        Self {
            hrc: HrcSpeed::Hz20,
            xtal_frequency_hz: 12_000_000,
            source: ClockSource::Pll(PllConfig::hrc_150mhz()),
            tune_flash: true,
        }
    }

    /// 120 MHz driven from the external 12 MHz crystal via the PLL. These are the
    /// vendor library's proven `SYS_CLK_PLL` values (VCO = 960 MHz); prefer this over
    /// [`ClockConfig::hrc_150mhz`] when the 150 MHz setting is marginal on a given part.
    #[inline]
    pub const fn hrc_120mhz() -> Self {
        Self {
            hrc: HrcSpeed::Hz20,
            xtal_frequency_hz: 12_000_000,
            source: ClockSource::Pll(PllConfig::hrc_120mhz()),
            tune_flash: true,
        }
    }

    /// True when an external oscillator has to be running for this config.
    pub const fn needs_xtal(&self) -> bool {
        match self.source {
            ClockSource::Xtal(_) => true,
            ClockSource::Pll(pll) => matches!(pll.source, PllSource::Xtal),
            ClockSource::Hrc | ClockSource::HrcDiv8 => false,
        }
    }

    const fn default_internal() -> Self {
        Self {
            hrc: HrcSpeed::Hz20,
            xtal_frequency_hz: 12_000_000,
            source: ClockSource::Hrc,
            tune_flash: true,
        }
    }
}

impl Default for ClockConfig {
    /// The reset state of the chip: core clocked from the 20 MHz internal RC.
    fn default() -> Self {
        Self::default_internal()
    }
}

static CORE_CLOCK_HZ: AtomicU32 = AtomicU32::new(HRC_20MHZ);

/// Core frequency established by the last successful [`init`].
#[inline]
pub fn sys_core_clock() -> u32 {
    CORE_CLOCK_HZ.load(Ordering::Relaxed)
}

/// Core frequency currently selected in hardware.
///
/// `None` is returned when the core runs off an external crystal, whose
/// frequency cannot be read back from any register. The reference crystal is
/// 12 MHz (the value the vendor's default build assumes).
pub fn current_system_clock() -> Option<u32> {
    let clksel = sys().clksel.read();
    let hrc = if sys().hrccr.read().dbl().bit() {
        HRC_40MHZ
    } else {
        HRC_20MHZ
    };

    let mut frequency: u64 = if clksel.sys().bit() {
        hrc as u64
    } else {
        match clksel.clk().bits() {
            CLK_SOURCE_LSI => LSI_HZ as u64,
            CLK_SOURCE_HRC => hrc as u64,
            CLK_SOURCE_PLL => {
                // Reading back a PLL fed by either oscillator is possible, but
                // an external one would need its frequency supplied by hand.
                if !sys().pllcr.read().insel().bit() {
                    return None;
                }
                let plldiv = sys().plldiv.read();
                let indiv = plldiv.indiv().bits() as u32;
                let fbdiv = plldiv.fbdiv().bits() as u32;
                if indiv == 0 {
                    return None;
                }
                (hrc / indiv) as u64 * fbdiv as u64 * 4
                    / PllOutDiv::from_code(plldiv.outdiv().bits()).factor() as u64
            }
            // LSE and XTAL frequencies are properties of the fitted crystal.
            CLK_SOURCE_LSE | CLK_SOURCE_XTAL => return None,
            _ => 0,
        }
    };

    if clksel.clk_divx().bit() {
        frequency /= 8;
    }
    Some(frequency as u32)
}

impl PllOutDiv {
    fn from_code(code: u8) -> Self {
        match code {
            0 => Self::Div8,
            1 => Self::Div4,
            _ => Self::Div2,
        }
    }
}

fn validate(config: &ClockConfig) -> Result<(), ClockError> {
    // Dividers first: a zero divider makes `core_frequency_hz` report 0 Hz,
    // which would otherwise be mistaken for a too-slow configuration.
    if let ClockSource::Pll(pll) = config.source {
        if pll.input_divider == 0
            || pll.input_divider > 0x1F
            || pll.feedback_divider == 0
            || pll.feedback_divider > 0x1FF
        {
            return Err(ClockError::DividerOutOfRange);
        }
    }

    let target = config.core_frequency_hz();
    if target == 0 || target > MAX_SYSTEM_CLOCK_HZ {
        return Err(ClockError::TooFast);
    }
    Ok(())
}

/// Apply `config` and update [`sys_core_clock`].
pub(crate) fn init(config: ClockConfig) -> Result<(), ClockError> {
    validate(&config)?;
    let target = config.core_frequency_hz();

    // The oscillator control block lives at SYS+0xAA000 and is clock gated.
    PeripheralClockIndex::Anac.open();

    if config.needs_xtal() {
        bring_up_xtal();
    }

    if config.tune_flash {
        // Set the wait states for whichever frequency is in effect *before* the
        // switch -- including the unknown one we may be running at. Guessing
        // high only costs performance, guessing low corrupts fetches.
        let before = current_system_clock().unwrap_or(MAX_SYSTEM_CLOCK_HZ);
        unsafe { flash::tune_for_frequency(target.max(before)) };
    }

    let running_at = switch_to_hrc(config.hrc);

    match config.source {
        ClockSource::Hrc => {}
        ClockSource::HrcDiv8 => switch_to_clk(CLK_SOURCE_HRC, true, running_at),
        ClockSource::Xtal(xtal) => switch_to_clk(CLK_SOURCE_XTAL, xtal.output_div8, running_at),
        ClockSource::Pll(pll) => {
            enable_pll(pll);
            switch_to_clk(CLK_SOURCE_PLL, pll.output_div8, running_at);
        }
    }

    if config.tune_flash {
        unsafe { flash::tune_for_frequency(target) };
    }
    CORE_CLOCK_HZ.store(target, Ordering::Relaxed);
    Ok(())
}

/// Turn the internal RC on in the requested gear and put the core on it.
///
/// Returns the frequency now in effect, needed to time the settling delays of
/// whatever comes next.
fn switch_to_hrc(hrc: HrcSpeed) -> u32 {
    sys().hrccr.write(|w| {
        w.on().set_bit();
        w.dbl().bit(hrc.doubled())
    });
    busy_delay_ms(3, HRC_20MHZ);
    sys().clksel.modify(|_, w| w.sys().set_bit());
    hrc.frequency_hz()
}

/// Move the core onto the `CLK` mux, which must already be fed.
fn switch_to_clk(source: u8, div8: bool, running_at_hz: u32) {
    // The vendor library leaves `CLKDIVx_ON` set (gating the internal source-clock
    // divider) while the core runs from PLL/XTAL. Clearing it feeds the raw divider
    // output to the core and can hang it, so it stays set here, matching the
    // reference `switchToPLL`/`switchToXTAL` sequences.
    sys().clkdivx_on.write(|w| unsafe { w.bits(1) });
    sys().clksel.modify(|_, w| unsafe { w.clk().bits(source) });
    sys().clksel.modify(|_, w| w.clk_divx().bit(div8));
    busy_delay_ms(3, running_at_hz);
    sys().clksel.modify(|_, w| w.sys().clear_bit());
}

/// Start the external oscillator on PA3/PA4.
fn bring_up_xtal() {
    // The pads are analog while used as an oscillator; the mux is in `PORTA`,
    // whose bus clock is gated together with `GPIOA`.
    PeripheralClockIndex::GpioA.open();
    // Function 15 is `XTAL_IN` / `XTAL_OUT` on PA3 / PA4, see `uart::pins::a`.
    crate::gpio::instance::set_port_a_function(3, 15);
    crate::gpio::instance::set_port_a_function(4, 15);

    sys().xtalcr.modify(|_, w| unsafe {
        w.on().set_bit();
        w.drv().bits(0xF);
        w.det().set_bit()
    });
}

/// Configure the PLL and wait for its lock.
fn enable_pll(pll: PllConfig) {
    if pll.source == PllSource::Hrc {
        sys().pllcr.modify(|_, w| w.insel().set_bit());
    } else {
        sys().pllcr.modify(|_, w| w.insel().clear_bit());
    }

    sys().plldiv.modify(|_, w| unsafe {
        w.indiv().bits(pll.input_divider as u8);
        w.fbdiv().bits(pll.feedback_divider as u16);
        w.outdiv().bits(pll.output_divider.code())
    });

    sys().pllcr.modify(|_, w| w.off().clear_bit());
    while sys().plllock.read().bits() == 0 {}
    sys().pllcr.modify(|_, w| w.outen().set_bit());
}

/// Spin for `ms` milliseconds assuming the core runs at `at_hz`.
///
/// Used only before any timer is available; `CLKSEL` changes insist on a settle
/// time measured in milliseconds, not cycles.
fn busy_delay_ms(ms: u32, at_hz: u32) {
    // The loop body is one nop plus branch overhead, four cycles per turn.
    let iterations = at_hz as u64 / 1000 * ms as u64 / 4;
    for _ in 0..iterations {
        cortex_m::asm::nop();
    }
}
