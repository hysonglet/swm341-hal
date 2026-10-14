//! JPEG configuration and error types.

/// An error reported by the JPEG decoder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JpegError {
    /// The decoder signalled an error during the decode.
    Decode,
}

/// Full description of one JPEG decode job.
#[derive(Clone, Copy, Debug)]
pub struct DecodeConfig {
    /// Base address of the compressed bitstream.
    pub code: *const u8,
    /// Length of the compressed bitstream in bytes.
    pub code_len: u32,
    /// Base address of the quantisation tables.
    pub quant_tables: *const u8,
    /// Base address of the Huffman tables.
    pub huffman_tables: *const u8,
    /// Output buffer for the Y (luma) plane.
    pub y: *mut u8,
    /// Output buffer for the U (chroma) plane.
    pub u: *mut u8,
    /// Output buffer for the V (chroma) plane.
    pub v: *mut u8,
    /// Optional packed RGB output buffer (used when `config` enables `YUV2RGB`).
    pub rgb: *mut u8,
    /// Output image width in pixels.
    pub width: u16,
    /// Output image height in pixels.
    pub height: u16,
    /// Output stride.
    pub stride: u32,
    /// Raw `CFG` register value (source format, output format, YUV2RGB, swaps...).
    pub config: u32,
}
