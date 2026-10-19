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

pub(crate) mod sealed {

    use super::Block;
    use crate::i2c::Error;

    /// An I2C peripheral the driver can talk to.
    ///
    /// Implemented for the singletons in [`crate::mcu`]. The only per-instance
    /// data are the register-block pointer, clock gate, async index and NVIC
    /// vector; all register access lives in the default methods of this trait.
    pub trait Instance: crate::instance_base::InstanceBase {
        /// Pointer to this instance's register block.
        fn regs() -> *const Block;
        /// NVIC interrupt of this peripheral.
        ///
        /// Returns the NVIC vector as [`crate::interrupt::ExtendedInterrupt`].
        /// Every SWM341 vector -- including `I2C1` at NVIC 30, which the PAC's
        /// SVD omits -- lives in that one enumeration, so the IRQ handler can use


        #[inline(always)]
        fn enable_master() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.en().set_bit().master().set_bit());
        }

        #[inline(always)]
        fn disable() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.modify(|_, w| w.en().clear_bit());
        }

        #[inline(always)]
        fn set_clock(frequency: u32) {
            let regs = unsafe { &*Self::regs() };
            let clk =
                crate::clock::sys_core_clock();
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

        #[inline(always)]
        fn txdata(byte: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.txdata.write(|w| unsafe { w.bits(byte as u32) });
        }

        #[inline(always)]
        fn start() {
            let regs = unsafe { &*Self::regs() };
            regs.mcr.write(|w| w.sta().set_bit());
        }

        #[inline(always)]
        fn stop() {
            let regs = unsafe { &*Self::regs() };
            regs.mcr.write(|w| w.sto().set_bit());
        }

        #[inline(always)]
        fn request_write() {
            let regs = unsafe { &*Self::regs() };
            regs.mcr.write(|w| w.wr().set_bit());
        }

        #[inline(always)]
        fn request_read(last: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.mcr.write(|w| {
                w.rd().set_bit();
                if last {
                    w.sto().set_bit();
                }
                w
            });
        }

        #[inline(always)]
        fn rx_byte() -> u8 {
            let regs = unsafe { &*Self::regs() };
            regs.rxdata.read().bits() as u8
        }

        #[inline(always)]
        fn is_txe() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().txe().bit()
        }

        #[inline(always)]
        fn clear_txe() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.txe().set_bit());
        }

        #[inline(always)]
        fn set_txe_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.txe().bit(enable));
        }

        #[inline(always)]
        fn is_rxne() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().rxne().bit()
        }

        #[inline(always)]
        fn clear_rxne() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.rxne().set_bit());
        }

        #[inline(always)]
        fn set_rxne_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.rxne().bit(enable));
        }

        #[inline(always)]
        fn is_txdone() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().txdone().bit()
        }

        #[inline(always)]
        fn clear_txdone() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.txdone().set_bit());
        }

        #[inline(always)]
        fn set_txdone_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.txdone().bit(enable));
        }

        #[inline(always)]
        fn is_rxdone() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().rxdone().bit()
        }

        #[inline(always)]
        fn clear_rxdone() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.rxdone().set_bit());
        }

        #[inline(always)]
        fn set_rxdone_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.rxdone().bit(enable));
        }

        #[inline(always)]
        fn check_faults() -> Result<(), Error> {
            let regs = unsafe { &*Self::regs() };
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

        #[inline(always)]
        fn bus_free() -> bool {
            let regs = unsafe { &*Self::regs() };
            !regs.sr.read().busy().bit()
        }

        #[inline(always)]
        fn disable_all_irqs() {
            let regs = unsafe { &*Self::regs() };
            regs.ie.write(|w| w);
        }

        // -- I2C slave (peripheral) mode --

        /// Put the block in slave mode: clear `MASTER`, keep `EN` set.
        #[inline(always)]
        fn enter_slave() {
            let regs = unsafe { &*Self::regs() };
            regs.cr.write(|w| w.en().set_bit().master().clear_bit());
        }

        /// Program the own 7-bit address and the match mask (0x7F = exact match).
        #[inline(always)]
        fn set_slave_address(addr: u8, mask: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.saddr.write(|w| unsafe {
                w.addr7().bits(addr & 0x7F).mask7().bits(mask & 0x7F)
            });
        }

        /// Enable (1) or disable (0) 10-bit address matching.
        #[inline(always)]
        fn set_slave_10bit(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.scr.modify(|_, w| w.addr10().bit(enable));
        }

        /// `true` once a START with a matching address was seen (address phase done).
        #[inline(always)]
        fn is_address_matched() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().rxsta().bit()
        }

        #[inline(always)]
        fn clear_address_matched() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.rxsta().set_bit());
        }

        /// `true` once a STOP was detected (current transfer finished).
        #[inline(always)]
        fn is_stop_detected() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.if_.read().rxsto().bit()
        }

        #[inline(always)]
        fn clear_stop_detected() {
            let regs = unsafe { &*Self::regs() };
            regs.if_.write(|w| w.rxsto().set_bit());
        }

        /// `true` once the matched address was a read: the master expects us to send.
        #[inline(always)]
        fn slave_is_read() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.tr.read().slvrd().bit()
        }

        /// `true` once the matched address was a write: the master sends to us.
        #[inline(always)]
        fn slave_is_write() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.tr.read().slvwr().bit()
        }

        /// `true` while the slave is addressed and a transfer is in progress.
        #[inline(always)]
        fn slave_active() -> bool {
            let regs = unsafe { &*Self::regs() };
            regs.tr.read().slvact().bit()
        }

        #[inline(always)]
        fn slave_rx_byte() -> u8 {
            let regs = unsafe { &*Self::regs() };
            regs.rxdata.read().bits() as u8
        }

        #[inline(always)]
        fn slave_tx_byte(byte: u8) {
            let regs = unsafe { &*Self::regs() };
            regs.txdata.write(|w| unsafe { w.bits(byte as u32) });
        }

        /// NACK the next byte the master reads from us (TXACK = 1); ACK again with `false`.
        #[inline(always)]
        fn slave_set_tx_nack(nack: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.tr.modify(|_, w| w.txack().bit(nack));
        }

        /// Whether the master acknowledged the last byte we transmitted (RXACK == 0).
        #[inline(always)]
        fn slave_master_acked() -> bool {
            let regs = unsafe { &*Self::regs() };
            !regs.tr.read().rxack().bit()
        }

        #[inline(always)]
        fn set_start_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.rxsta().bit(enable));
        }

        #[inline(always)]
        fn set_stop_irq(enable: bool) {
            let regs = unsafe { &*Self::regs() };
            regs.ie.modify(|_, w| w.rxsto().bit(enable));
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
        crate::impl_instance_base!($ty, crate::pac::i2c0::RegisterBlock, crate::interrupt::$irq, PeripheralClockIndex::$clock, $index);

        impl sealed::Instance for $ty {
            fn regs() -> *const Block {
                crate::pac::$pac::PTR
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
