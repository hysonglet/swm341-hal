//! Register access for the I2C blocks.
//!
//! Both SWM341 I2Cs share the `pac::i2c0::RegisterBlock` type: the PAC aliases
//! `i2c1` to `i2c0`, and every `I2Cx::PTR` is a `*const i2c0::RegisterBlock`.
//! The C vendor driver exploits the same fact — `I2C_Init(I2C_TypeDef *I2Cx, …)`
//! is one implementation operating on an instance pointer, differing only in
//! the clock gate and NVIC vector.
//!
//! We mirror that here. [`sealed::Instance`] carries only the per-instance data
//! — the register-block pointer, clock gate, async index and NVIC vector — while
//! the actual register twiddling lives once in the [`hw`] module as free
//! functions over `&pac::i2c0::RegisterBlock`. Each trait method is a one-line
//! forward, so the heavy instructions are emitted a single time instead of once
//! per I2C.

use crate::clock::PeripheralClockIndex;
use crate::pac;

/// The register-block type shared by every I2C instance on the SWM341.
///
/// The PAC already types `I2C0`/`I2C1::PTR` as `*const i2c0::RegisterBlock`,
/// so no pointer cast is ever needed — the two singletons agree on layout.
pub(crate) type Block = pac::i2c0::RegisterBlock;

/// Register access shared by every I2C instance.
///
/// Each function is non-generic (it takes the common [`Block`] type), so the
/// compiler emits its body exactly once regardless of how many I2Cs exist.
pub(crate) mod hw {
    use super::Block;
    use crate::i2c::Error;

    pub(crate) fn enable_master(regs: &Block) {
        regs.cr.write(|w| w.en().set_bit().master().set_bit());
    }

    pub(crate) fn disable(regs: &Block) {
        regs.cr.modify(|_, w| w.en().clear_bit());
    }

    pub(crate) fn set_clock(regs: &Block, frequency: u32) {
        let clk =
            crate::clock::current_system_clock().expect("peripheral clock not running") as u32;
        let target = clk / frequency.max(1);
        // SCL period (in HCLK cycles) = (DIV + 1) * (SCLL + SCLH + 2);
        // pick the smallest DIV that keeps each half within the 8-bit
        // field, then split the remainder across the two halves for a 50%
        // duty cycle.
        let div = ((target + 511) / 512).saturating_sub(1).min(255) as u8;
        let per = target / (div as u32 + 1);
        let scll = ((per / 2).saturating_sub(1)).min(255) as u8;
        regs.clk.write(|w| unsafe {
            w.scll().bits(scll);
            w.sclh().bits(scll);
            w.div().bits(div);
            w.sdah().bits(0)
        });
    }

    pub(crate) fn txdata(regs: &Block, byte: u8) {
        regs.txdata.write(|w| unsafe { w.bits(byte as u32) });
    }

    pub(crate) fn start(regs: &Block) {
        regs.mcr.write(|w| w.sta().set_bit());
    }

    pub(crate) fn stop(regs: &Block) {
        regs.mcr.write(|w| w.sto().set_bit());
    }

    pub(crate) fn request_write(regs: &Block) {
        regs.mcr.write(|w| w.wr().set_bit());
    }

    pub(crate) fn request_read(regs: &Block, last: bool) {
        regs.mcr.write(|w| {
            w.rd().set_bit();
            if last {
                w.sto().set_bit();
            }
            w
        });
    }

    pub(crate) fn rx_byte(regs: &Block) -> u8 {
        regs.rxdata.read().bits() as u8
    }

    pub(crate) fn is_txe(regs: &Block) -> bool {
        regs.if_.read().txe().bit()
    }

    pub(crate) fn clear_txe(regs: &Block) {
        regs.if_.write(|w| w.txe().set_bit());
    }

    pub(crate) fn set_txe_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.txe().bit(enable));
    }

    pub(crate) fn is_rxne(regs: &Block) -> bool {
        regs.if_.read().rxne().bit()
    }

    pub(crate) fn clear_rxne(regs: &Block) {
        regs.if_.write(|w| w.rxne().set_bit());
    }

    pub(crate) fn set_rxne_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.rxne().bit(enable));
    }

    pub(crate) fn is_txdone(regs: &Block) -> bool {
        regs.if_.read().txdone().bit()
    }

    pub(crate) fn clear_txdone(regs: &Block) {
        regs.if_.write(|w| w.txdone().set_bit());
    }

    pub(crate) fn set_txdone_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.txdone().bit(enable));
    }

    pub(crate) fn is_rxdone(regs: &Block) -> bool {
        regs.if_.read().rxdone().bit()
    }

    pub(crate) fn clear_rxdone(regs: &Block) {
        regs.if_.write(|w| w.rxdone().set_bit());
    }

    pub(crate) fn set_rxdone_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.rxdone().bit(enable));
    }

    pub(crate) fn check_faults(regs: &Block) -> Result<(), Error> {
        let s = regs.if_.read();
        if s.al().bit() {
            return Err(Error::ArbitrationLost);
        }
        if s.mlto().bit() {
            return Err(Error::Timeout);
        }
        if s.rxov().bit() {
            regs.if_.write(|w| w.rxov().set_bit());
            return Err(Error::Overrun);
        }
        Ok(())
    }

    pub(crate) fn bus_free(regs: &Block) -> bool {
        !regs.sr.read().busy().bit()
    }

    pub(crate) fn disable_all_irqs(regs: &Block) {
        regs.ie.write(|w| w);
    }

    // -- I2C slave (peripheral) mode --

    /// Put the block in slave mode: clear `MASTER`, keep `EN` set.
    pub(crate) fn enter_slave(regs: &Block) {
        regs.cr.write(|w| w.en().set_bit().master().clear_bit());
    }

    /// Program the own 7-bit address and the match mask (0x7F = exact match).
    pub(crate) fn set_slave_address(regs: &Block, addr: u8, mask: u8) {
        regs.saddr.write(|w| unsafe {
            w.addr7().bits(addr & 0x7F).mask7().bits(mask & 0x7F)
        });
    }

    /// Enable (1) or disable (0) 10-bit address matching.
    pub(crate) fn set_slave_10bit(regs: &Block, enable: bool) {
        regs.scr.modify(|_, w| w.addr10().bit(enable));
    }

    /// `true` once a START with a matching address was seen (address phase done).
    pub(crate) fn is_address_matched(regs: &Block) -> bool {
        regs.if_.read().rxsta().bit()
    }

    pub(crate) fn clear_address_matched(regs: &Block) {
        regs.if_.write(|w| w.rxsta().set_bit());
    }

    /// `true` once a STOP was detected (current transfer finished).
    pub(crate) fn is_stop_detected(regs: &Block) -> bool {
        regs.if_.read().rxsto().bit()
    }

    pub(crate) fn clear_stop_detected(regs: &Block) {
        regs.if_.write(|w| w.rxsto().set_bit());
    }

    /// `true` once the matched address was a read: the master expects us to send.
    pub(crate) fn slave_is_read(regs: &Block) -> bool {
        regs.tr.read().slvrd().bit()
    }

    /// `true` once the matched address was a write: the master sends to us.
    pub(crate) fn slave_is_write(regs: &Block) -> bool {
        regs.tr.read().slvwr().bit()
    }

    /// `true` while the slave is addressed and a transfer is in progress.
    pub(crate) fn slave_active(regs: &Block) -> bool {
        regs.tr.read().slvact().bit()
    }

    pub(crate) fn slave_rx_byte(regs: &Block) -> u8 {
        regs.rxdata.read().bits() as u8
    }

    pub(crate) fn slave_tx_byte(regs: &Block, byte: u8) {
        regs.txdata.write(|w| unsafe { w.bits(byte as u32) });
    }

    /// NACK the next byte the master reads from us (TXACK = 1); ACK again with `false`.
    pub(crate) fn slave_set_tx_nack(regs: &Block, nack: bool) {
        regs.tr.modify(|_, w| w.txack().bit(nack));
    }

    /// Whether the master acknowledged the last byte we transmitted (RXACK == 0).
    pub(crate) fn slave_master_acked(regs: &Block) -> bool {
        !regs.tr.read().rxack().bit()
    }

    pub(crate) fn set_start_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.rxsta().bit(enable));
    }

    pub(crate) fn set_stop_irq(regs: &Block, enable: bool) {
        regs.ie.modify(|_, w| w.rxsto().bit(enable));
    }
}

pub(crate) mod sealed {
    use cortex_m::interrupt::InterruptNumber;
    use cortex_m::peripheral::NVIC;

    use super::{hw, Block, PeripheralClockIndex};

    /// An I2C peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; every register operation is forwarded to [`super::hw`], which
    /// holds a single shared implementation.
    pub trait Instance {
        /// Clock gate that powers this peripheral.
        fn clock() -> PeripheralClockIndex;
        /// Stable index used to look up the async state of this instance.
        fn index() -> usize;
        /// NVIC interrupt of this peripheral.
        ///
        /// Returns `impl InterruptNumber` rather than `pac::Interrupt` because
        /// `I2C1` lives above the 29 vectors the PAC's SVD describes and is
        /// therefore an [`crate::interrupt::ExtendedInterrupt`].
        fn irq() -> impl InterruptNumber;
        /// Pointer to this instance's register block (always a `i2c0::RegisterBlock`).
        fn regs() -> *const Block;

        // -- thin forwards to the shared implementation in `hw` --
        // Marked `#[inline(always)]` so they collapse into the single `hw`
        // body at every call site rather than duplicating it per I2C.

        #[inline(always)]
        fn unmask_irq() {
            unsafe { NVIC::unmask(Self::irq()) };
        }

        #[inline(always)]
        fn enable_master() {
            hw::enable_master(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn disable() {
            hw::disable(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_clock(frequency: u32) {
            hw::set_clock(unsafe { &*Self::regs() }, frequency);
        }

        #[inline(always)]
        fn txdata(byte: u8) {
            hw::txdata(unsafe { &*Self::regs() }, byte);
        }

        #[inline(always)]
        fn start() {
            hw::start(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn stop() {
            hw::stop(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn request_write() {
            hw::request_write(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn request_read(last: bool) {
            hw::request_read(unsafe { &*Self::regs() }, last);
        }

        #[inline(always)]
        fn rx_byte() -> u8 {
            hw::rx_byte(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn is_txe() -> bool {
            hw::is_txe(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_txe() {
            hw::clear_txe(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_txe_irq(enable: bool) {
            hw::set_txe_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn is_rxne() -> bool {
            hw::is_rxne(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_rxne() {
            hw::clear_rxne(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_rxne_irq(enable: bool) {
            hw::set_rxne_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn is_txdone() -> bool {
            hw::is_txdone(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_txdone() {
            hw::clear_txdone(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_txdone_irq(enable: bool) {
            hw::set_txdone_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn is_rxdone() -> bool {
            hw::is_rxdone(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_rxdone() {
            hw::clear_rxdone(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_rxdone_irq(enable: bool) {
            hw::set_rxdone_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn check_faults() -> Result<(), crate::i2c::Error> {
            hw::check_faults(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn bus_free() -> bool {
            hw::bus_free(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn disable_all_irqs() {
            hw::disable_all_irqs(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn enter_slave() {
            hw::enter_slave(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn set_slave_address(addr: u8, mask: u8) {
            hw::set_slave_address(unsafe { &*Self::regs() }, addr, mask);
        }

        #[inline(always)]
        fn set_slave_10bit(enable: bool) {
            hw::set_slave_10bit(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn is_address_matched() -> bool {
            hw::is_address_matched(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_address_matched() {
            hw::clear_address_matched(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn is_stop_detected() -> bool {
            hw::is_stop_detected(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn clear_stop_detected() {
            hw::clear_stop_detected(unsafe { &*Self::regs() });
        }

        #[inline(always)]
        fn slave_is_read() -> bool {
            hw::slave_is_read(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn slave_is_write() -> bool {
            hw::slave_is_write(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn slave_active() -> bool {
            hw::slave_active(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn slave_rx_byte() -> u8 {
            hw::slave_rx_byte(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn slave_tx_byte(byte: u8) {
            hw::slave_tx_byte(unsafe { &*Self::regs() }, byte);
        }

        #[inline(always)]
        fn slave_set_tx_nack(nack: bool) {
            hw::slave_set_tx_nack(unsafe { &*Self::regs() }, nack);
        }

        #[inline(always)]
        fn slave_master_acked() -> bool {
            hw::slave_master_acked(unsafe { &*Self::regs() })
        }

        #[inline(always)]
        fn set_start_irq(enable: bool) {
            hw::set_start_irq(unsafe { &*Self::regs() }, enable);
        }

        #[inline(always)]
        fn set_stop_irq(enable: bool) {
            hw::set_stop_irq(unsafe { &*Self::regs() }, enable);
        }
    }
}

#[allow(private_bounds)]
/// An I2C peripheral usable by the driver.
pub trait Instance: sealed::Instance {}

/// Build the per-instance data for an I2C singleton.
///
/// Only the four values that actually differ between I2Cs are supplied; the
/// (large) register-access logic comes from the default methods of
/// [`sealed::Instance`], which forward to the single [`hw`] implementation.
macro_rules! impl_i2c_instance {
    ($ty:ty, $pac:ident, $irq:ident, $clock:ident, $index:literal) => {
        impl sealed::Instance for $ty {
            #[inline]
            fn clock() -> PeripheralClockIndex {
                PeripheralClockIndex::$clock
            }

            #[inline]
            fn index() -> usize {
                $index
            }

            #[inline]
            fn irq() -> impl cortex_m::interrupt::InterruptNumber {
                crate::interrupt::$irq
            }

            #[inline]
            fn regs() -> *const Block {
                pac::$pac::PTR
            }
        }

        impl Instance for $ty {}
    };
}

impl_i2c_instance!(crate::I2c0, I2C0, I2C0, I2c0, 0);
impl_i2c_instance!(crate::I2c1, I2C1, I2C1, I2c1, 1);

/// Number of I2C instances, used to size the async state arrays.
#[cfg(feature = "embassy")]
pub(crate) const COUNT: usize = 2;
