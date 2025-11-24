use crate::encoder::compression::*;
use flate2::{write::ZlibEncoder, Compression as FlateCompression};

/// The Deflate algorithm used to compress image data in TIFF files.
#[derive(Debug, Clone, Copy)]
pub struct Deflate {
    level: FlateCompression,
}

/// The level of compression used by the Deflate algorithm.
/// It allows trading compression ratio for compression speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
#[derive(Default)]
pub enum DeflateLevel {
    /// The fastest possible compression mode.
    Fast = 1,
    /// The conserative choice between speed and ratio.
    #[default]
    Balanced = 6,
    /// The best compression available with Deflate.
    Best = 9,
}

impl Deflate {
    /// Create a new deflate compressor with a specific level of compression.
    pub fn with_level(level: DeflateLevel) -> Self {
        Self {
            level: FlateCompression::new(level as u32),
        }
    }
}

impl Default for Deflate {
    fn default() -> Self {
        Self::with_level(DeflateLevel::default())
    }
}

impl Compression for Deflate {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::Deflate;

    fn get_algorithm(&self) -> Compressor {
        Compressor::Deflate(*self)
    }
}

impl CompressionAlgorithm for Deflate {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        let mut encoder = ZlibEncoder::new(writer, self.level);
        encoder.write_all(bytes)?;
        encoder.try_finish()?;
        Ok(encoder.total_out())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::compression::tests::TEST_DATA;
    use std::io::Cursor;

    #[test]
    fn test_deflate() {
        const EXPECTED_COMPRESSED_DATA: [u8; 64] = [
            0x78, 0x9C, 0x15, 0xC7, 0xD1, 0x0D, 0x80, 0x20, 0x0C, 0x04, 0xD0, 0x55, 0x6E, 0x02,
            0xA7, 0x71, 0x81, 0xA6, 0x41, 0xDA, 0x28, 0xD4, 0xF4, 0xD0, 0xF9, 0x81, 0xE4, 0xFD,
            0xBC, 0xD3, 0x9C, 0x58, 0x04, 0x1C, 0xE9, 0xBD, 0xE2, 0x8A, 0x84, 0x5A, 0xD1, 0x7B,
            0xE7, 0x97, 0xF4, 0xF8, 0x08, 0x8D, 0xF6, 0x66, 0x21, 0x3D, 0x3A, 0xE4, 0xA9, 0x91,
            0x3E, 0xAC, 0xF1, 0x98, 0xB9, 0x70, 0x17, 0x13,
        ];

        let mut compressed_data = Vec::<u8>::new();
        let mut writer = Cursor::new(&mut compressed_data);
        Deflate::default().write_to(&mut writer, TEST_DATA).unwrap();
        assert_eq!(EXPECTED_COMPRESSED_DATA, compressed_data.as_slice());
    }
}
