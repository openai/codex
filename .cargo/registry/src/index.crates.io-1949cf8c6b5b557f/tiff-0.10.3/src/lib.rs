//! Decoding and Encoding of TIFF Images
//!
//! TIFF (Tagged Image File Format) is a versatile image format that supports
//! lossless and lossy compression.
//!
//! # Related Links
//! * <https://web.archive.org/web/20210108073850/https://www.adobe.io/open/standards/TIFF.html> - The TIFF specification

mod bytecast;
pub mod decoder;
mod directory;
pub mod encoder;
mod error;
pub mod tags;

pub use self::directory::Directory;
pub use self::error::{TiffError, TiffFormatError, TiffResult, TiffUnsupportedError, UsageError};

/// An enumeration over supported color types and their bit depths
#[derive(Copy, PartialEq, Eq, Debug, Clone, Hash)]
#[non_exhaustive]
pub enum ColorType {
    /// Pixel is grayscale
    Gray(u8),

    /// Pixel contains R, G and B channels
    RGB(u8),

    /// Pixel is an index into a color palette
    Palette(u8),

    /// Pixel is grayscale with an alpha channel
    GrayA(u8),

    /// Pixel is RGB with an alpha channel
    RGBA(u8),

    /// Pixel is CMYK
    CMYK(u8),

    /// Pixel is CMYK with an alpha channel
    CMYKA(u8),

    /// Pixel is YCbCr
    YCbCr(u8),

    /// Pixel has multiple bands/channels
    Multiband { bit_depth: u8, num_samples: u16 },
}
impl ColorType {
    fn bit_depth(&self) -> u8 {
        match *self {
            ColorType::Gray(b)
            | ColorType::RGB(b)
            | ColorType::Palette(b)
            | ColorType::GrayA(b)
            | ColorType::RGBA(b)
            | ColorType::CMYK(b)
            | ColorType::CMYKA(b)
            | ColorType::YCbCr(b)
            | ColorType::Multiband { bit_depth: b, .. } => b,
        }
    }
}
