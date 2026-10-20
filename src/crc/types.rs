//! CRC configuration types.

/// Polynomial selection (`CR.POLY`, 2 bits).
///
/// The engine implements four hard-wired polynomials selected by index. The
/// names below follow the usual mapping for this IP; confirm against the
/// datasheet table if bit-exactness against a specific standard matters.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Poly {
    /// CRC-16, `x^16 + x^15 + x^2 + 1` (`0x8005`).
    Crc16 = 0,
    /// CRC-32, `x^32 + x^26 + x^23 + x^22 + x^16 + x^12 + x^11 + x^10 + x^8 +
    /// x^7 + x^5 + x^4 + x^2 + x + 1` (`0x04C11DB7`).
    Crc32 = 1,
    /// CRC-16/CCITT, `x^16 + x^12 + x^5 + 1` (`0x1021`).
    Crc16Ccitt = 2,
    /// The fourth (vendor-defined) polynomial.
    Vendor = 3,
}

/// Width of one `DATAIN` write (`CR.IBIT`, 2 bits).
///
/// This only decides how many bytes of the input stream the engine consumes per
/// write; it does not shorten the polynomial. The driver packs a byte slice
/// accordingly in [`crate::crc::Crc::update_bytes`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputWidth {
    /// One 32-bit word per write (four bytes).
    Bits32 = 0,
    /// One 16-bit half-word per write (two bytes).
    Bits16 = 1,
    /// One byte per write.
    Bits8 = 2,
    /// Reserved by the hardware; feeding bytes with this setting is a no-op.
    Reserved = 3,
}

/// Input / output reordering (`CR.IREV` for input, `CR.OREV` for output).
///
/// Two independent transformations are packed into the 2-bit field: bit 0 swaps
/// the byte order inside the word, bit 1 reverses the bit order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reverse {
    /// Pass the data through unchanged.
    None = 0,
    /// Swap the byte order inside the word (little <-> big endian).
    Bytes = 1,
    /// Reverse the bit order (MSB first <-> LSB first).
    Bits = 2,
    /// Both, byte swap first and then bit reversal.
    BytesAndBits = 3,
}

/// CRC configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Config {
    /// Polynomial used for the accumulation (`CR.POLY`).
    pub poly: Poly,
    /// How many bytes one `DATAIN` write consumes (`CR.IBIT`).
    pub input_width: InputWidth,
    /// Reordering applied to each input word before it is folded in (`CR.IREV`).
    pub input_rev: Reverse,
    /// Bit-wise invert every input word (`CR.INOT`).
    pub input_invert: bool,
    /// Reordering applied to the accumulator when it is read (`CR.OREV`).
    pub output_rev: Reverse,
    /// Bit-wise invert the output (`CR.ONOT`).
    pub output_invert: bool,
    /// Seed loaded into the accumulator by `INIVAL` at the start of every
    /// computation.
    pub init_value: u32,
}

impl Default for Config {
    /// CRC-32 over 32-bit words, no reflections or inversions, zero seed.
    fn default() -> Self {
        Self {
            poly: Poly::Crc32,
            input_width: InputWidth::Bits32,
            input_rev: Reverse::None,
            input_invert: false,
            output_rev: Reverse::None,
            output_invert: false,
            init_value: 0,
        }
    }
}

impl Config {
    /// Standard reflected CRC-32 (the "CRC-32/ISO-HDLC" flavour used by zlib,
    /// Ethernet and PKZIP).
    ///
    /// Reflected input and output with a final XOR-out, seeded with
    /// `0xFFFF_FFFF`, fed one byte at a time.
    pub fn crc32() -> Self {
        Self {
            poly: Poly::Crc32,
            input_width: InputWidth::Bits8,
            input_rev: Reverse::None,
            input_invert: false,
            output_rev: Reverse::Bits,
            output_invert: true,
            init_value: 0xFFFF_FFFF,
        }
    }

    /// CRC-16/CCITT-FALSE: `0x1021`, seed `0xFFFF`, no reflection.
    pub fn crc16_ccitt() -> Self {
        Self {
            poly: Poly::Crc16Ccitt,
            input_width: InputWidth::Bits8,
            input_rev: Reverse::None,
            input_invert: false,
            output_rev: Reverse::None,
            output_invert: false,
            init_value: 0xFFFF,
        }
    }

    /// CRC-16 (ARC): `0x8005`, zero seed, no reflection.
    pub fn crc16() -> Self {
        Self {
            poly: Poly::Crc16,
            input_width: InputWidth::Bits8,
            input_rev: Reverse::None,
            input_invert: false,
            output_rev: Reverse::None,
            output_invert: false,
            init_value: 0,
        }
    }
}
