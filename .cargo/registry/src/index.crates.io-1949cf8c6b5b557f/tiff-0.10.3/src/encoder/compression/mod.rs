use crate::tags::CompressionMethod;
use std::io::{self, Write};

#[cfg(feature = "deflate")]
mod deflate;
#[cfg(feature = "lzw")]
mod lzw;
mod packbits;
mod uncompressed;

#[cfg(feature = "deflate")]
pub use self::deflate::{Deflate, DeflateLevel};

#[cfg(feature = "lzw")]
pub use self::lzw::Lzw;

pub use self::packbits::Packbits;
pub use self::uncompressed::Uncompressed;

/// An algorithm used for compression
pub trait CompressionAlgorithm {
    /// The algorithm writes data directly into the writer.
    /// It returns the total number of bytes written.
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error>;
}

/// An algorithm used for compression with associated enums and optional configurations.
pub trait Compression: CompressionAlgorithm {
    /// The corresponding tag to the algorithm.
    const COMPRESSION_METHOD: CompressionMethod;

    /// Method to optain a type that can store each variant of comression algorithm.
    fn get_algorithm(&self) -> Compressor;
}

/// An enum to store each compression algorithm.
#[non_exhaustive]
pub enum Compressor {
    Uncompressed(Uncompressed),
    #[cfg(feature = "lzw")]
    Lzw(Lzw),
    #[cfg(feature = "deflate")]
    Deflate(Deflate),
    Packbits(Packbits),
}

impl Default for Compressor {
    /// The default compression strategy does not apply any compression.
    fn default() -> Self {
        Compressor::Uncompressed(Uncompressed)
    }
}

impl CompressionAlgorithm for Compressor {
    fn write_to<W: Write>(&mut self, writer: &mut W, bytes: &[u8]) -> Result<u64, io::Error> {
        match self {
            Compressor::Uncompressed(algorithm) => algorithm.write_to(writer, bytes),
            #[cfg(feature = "lzw")]
            Compressor::Lzw(algorithm) => algorithm.write_to(writer, bytes),
            #[cfg(feature = "deflate")]
            Compressor::Deflate(algorithm) => algorithm.write_to(writer, bytes),
            Compressor::Packbits(algorithm) => algorithm.write_to(writer, bytes),
        }
    }
}

#[cfg(test)]
mod tests {
    pub const TEST_DATA: &[u8] = b"This is a string for checking various compression algorithms.";
}
