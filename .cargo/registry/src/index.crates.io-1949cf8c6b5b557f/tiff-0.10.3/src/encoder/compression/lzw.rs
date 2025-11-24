use crate::encoder::compression::*;
use weezl::encode::Encoder as LZWEncoder;

/// The LZW algorithm used to compress image data in TIFF files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lzw;

impl Compression for Lzw {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::LZW;

    fn get_algorithm(&self) -> Compressor {
        Compressor::Lzw(*self)
    }
}

impl CompressionAlgorithm for Lzw {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        let mut encoder = LZWEncoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
        let result = encoder.into_stream(writer).encode_all(bytes);
        let byte_count = result.bytes_written as u64;
        result.status.map(|_| byte_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::compression::tests::TEST_DATA;
    use std::io::Cursor;

    #[test]
    fn test_lzw() {
        const EXPECTED_COMPRESSED_DATA: [u8; 63] = [
            0x80, 0x15, 0x0D, 0x06, 0x93, 0x98, 0x82, 0x08, 0x20, 0x30, 0x88, 0x0E, 0x67, 0x43,
            0x91, 0xA4, 0xDC, 0x67, 0x10, 0x19, 0x8D, 0xE7, 0x21, 0x01, 0x8C, 0xD0, 0x65, 0x31,
            0x9A, 0xE1, 0xD1, 0x03, 0xB1, 0x86, 0x1A, 0x6F, 0x3A, 0xC1, 0x4C, 0x66, 0xF3, 0x69,
            0xC0, 0xE4, 0x65, 0x39, 0x9C, 0xCD, 0x26, 0xF3, 0x74, 0x20, 0xD8, 0x67, 0x89, 0x9A,
            0x4E, 0x86, 0x83, 0x69, 0xCC, 0x5D, 0x01,
        ];

        let mut compressed_data = Vec::<u8>::new();
        let mut writer = Cursor::new(&mut compressed_data);
        Lzw.write_to(&mut writer, TEST_DATA).unwrap();
        assert_eq!(EXPECTED_COMPRESSED_DATA, compressed_data.as_slice());
    }
}
