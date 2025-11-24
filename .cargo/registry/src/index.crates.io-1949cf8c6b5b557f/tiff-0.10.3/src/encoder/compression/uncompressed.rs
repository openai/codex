use crate::encoder::compression::*;

/// The default algorithm which does not compress at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Uncompressed;

impl Compression for Uncompressed {
    const COMPRESSION_METHOD: CompressionMethod = CompressionMethod::None;

    fn get_algorithm(&self) -> Compressor {
        Compressor::Uncompressed(*self)
    }
}

impl CompressionAlgorithm for Uncompressed {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        writer.write(bytes).map(|byte_count| byte_count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::compression::tests::TEST_DATA;
    use std::io::Cursor;

    #[test]
    fn test_no_compression() {
        let mut compressed_data = Vec::<u8>::new();
        let mut writer = Cursor::new(&mut compressed_data);
        Uncompressed.write_to(&mut writer, TEST_DATA).unwrap();
        assert_eq!(TEST_DATA, compressed_data);
    }
}
