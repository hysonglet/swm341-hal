//! Hardware CRC engine.
//!
//! The SWM341 folds a byte stream through a polynomial in hardware. Feed data
//! with [`Crc::update_bytes`] (or one word at a time with [`Crc::update`]) and
//! read the accumulator with [`Crc::result`]. The engine has no completion flag:
//! every `DATAIN` write updates the accumulator immediately, so the whole driver
//! is synchronous and no async flavour is provided.
//!
//! [`Crc::calculate`] is the one-shot form: it seeds the accumulator, feeds a
//! slice and returns the value. Use [`Crc::reset`] plus `update*` when the data
//! arrives in pieces.
//!
//! ```no_run
//! # use swm341_hal as hal;
//! # let p = hal::init(Default::default());
//! let mut crc = hal::crc::Crc::new(p.crc(), hal::crc::Config::crc32());
//! let value = crc.calculate(b"123456789");
//! ```

use embassy_hal_internal::{into_ref, Peripheral, PeripheralRef};

use instance::sealed::Instance as _;

mod instance;
pub mod types;
pub use types::*;

/// A handle to the hardware CRC engine.
pub struct Crc<'d> {
    #[allow(dead_code)]
    peri: PeripheralRef<'d, crate::Crc>,
    config: Config,
}

impl<'d> Drop for Crc<'d> {
    fn drop(&mut self) {
        // Gate the clock so the engine does not draw power once unused.
        crate::Crc::clock().close();
    }
}

impl<'d> Crc<'d> {
    /// Take ownership of the engine, open its clock gate and apply `config`.
    ///
    /// The engine is enabled here and the accumulator is seeded with
    /// `config.init_value`, so the handle is ready to [`update`](Self::update)
    /// straight away.
    pub fn new(peri: impl Peripheral<P = crate::Crc> + 'd, config: Config) -> Self {
        into_ref!(peri);

        crate::Crc::clock().open();
        crate::Crc::enable();
        crate::Crc::set_config(config);
        crate::Crc::set_init_value(config.init_value);

        Self { peri, config }
    }

    /// Reload the seed so the next update starts a fresh computation.
    ///
    /// Writing `INIVAL` is what resets the accumulator; call this before feeding
    /// a new message (or use [`calculate`](Self::calculate), which does it for
    /// you).
    pub fn reset(&mut self) {
        crate::Crc::set_init_value(self.config.init_value);
    }

    /// Fold one input word into the accumulator.
    ///
    /// Only the low `input_width` bits are consumed; see [`InputWidth`].
    pub fn update(&mut self, data: u32) {
        crate::Crc::write_data(data);
    }

    /// Feed a byte slice, packing it into words of `input_width` bytes.
    ///
    /// Bytes are packed little-endian (the first byte of the slice lands in the
    /// least significant byte of the word), which matches the engine's natural
    /// ordering for a byte stream. A trailing partial word is zero-padded.
    pub fn update_bytes(&mut self, bytes: &[u8]) {
        match self.config.input_width {
            InputWidth::Bits8 => {
                for &byte in bytes {
                    crate::Crc::write_data(byte as u32);
                }
            }
            InputWidth::Bits16 => {
                let mut chunks = bytes.chunks_exact(2);
                for chunk in &mut chunks {
                    crate::Crc::write_data(u16::from_le_bytes([chunk[0], chunk[1]]) as u32);
                }
                if let [tail] = chunks.remainder() {
                    // Odd length: fold the last byte on its own, zero-padded.
                    crate::Crc::write_data(*tail as u32);
                }
            }
            InputWidth::Bits32 => {
                let mut chunks = bytes.chunks_exact(4);
                for chunk in &mut chunks {
                    crate::Crc::write_data(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
                let tail = chunks.remainder();
                if !tail.is_empty() {
                    // Short tail: pad the unused high bytes with zeros.
                    let mut word = [0u8; 4];
                    word[..tail.len()].copy_from_slice(tail);
                    crate::Crc::write_data(u32::from_le_bytes(word));
                }
            }
            // Reserved width: the hardware defines no behaviour, so skip.
            InputWidth::Reserved => {}
        }
    }

    /// The current accumulator value (`RESULT`).
    pub fn result(&self) -> u32 {
        crate::Crc::result()
    }

    /// Seed, feed and read in one go: the CRC of `bytes` under the active
    /// configuration.
    pub fn calculate(&mut self, bytes: &[u8]) -> u32 {
        self.reset();
        self.update_bytes(bytes);
        self.result()
    }

    /// The configuration this handle was built with.
    pub fn config(&self) -> Config {
        self.config
    }

    /// Swap in a new configuration and reseed the accumulator.
    pub fn set_config(&mut self, config: Config) {
        self.config = config;
        crate::Crc::set_config(config);
        crate::Crc::set_init_value(config.init_value);
    }

    /// Enable the engine (`CR.EN`).
    pub fn enable(&self) {
        crate::Crc::enable();
    }

    /// Disable the engine; the accumulator keeps its last value.
    pub fn disable(&self) {
        crate::Crc::disable();
    }
}
