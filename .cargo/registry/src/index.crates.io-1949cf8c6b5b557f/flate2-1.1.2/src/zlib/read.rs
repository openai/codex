use std::io;
use std::io::prelude::*;

use super::bufread;
use crate::bufreader::BufReader;
use crate::Decompress;

/// A ZLIB encoder, or compressor.
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// uncompressed data from the underlying [`Read`] and provides the compressed data.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::read::ZlibEncoder;
/// use std::fs::File;
///
/// // Open example file and compress the contents using Read interface
///
/// # fn open_hello_world() -> std::io::Result<Vec<u8>> {
/// let f = File::open("examples/hello_world.txt")?;
/// let mut z = ZlibEncoder::new(f, Compression::fast());
/// let mut buffer = Vec::new();
/// z.read_to_end(&mut buffer)?;
/// # Ok(buffer)
/// # }
/// ```
#[derive(Debug)]
pub struct ZlibEncoder<R> {
    inner: bufread::ZlibEncoder<BufReader<R>>,
}

impl<R: Read> ZlibEncoder<R> {
    /// Creates a new encoder which will read uncompressed data from the given
    /// stream and emit the compressed stream.
    pub fn new(r: R, level: crate::Compression) -> ZlibEncoder<R> {
        ZlibEncoder {
            inner: bufread::ZlibEncoder::new(BufReader::new(r), level),
        }
    }

    /// Creates a new encoder with the given `compression` settings which will
    /// read uncompressed data from the given stream `r` and emit the compressed stream.
    pub fn new_with_compress(r: R, compression: crate::Compress) -> ZlibEncoder<R> {
        ZlibEncoder {
            inner: bufread::ZlibEncoder::new_with_compress(BufReader::new(r), compression),
        }
    }
}

impl<R> ZlibEncoder<R> {
    /// Resets the state of this encoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This function will reset the internal state of this encoder and replace
    /// the input stream with the one provided, returning the previous input
    /// stream. Future data read from this encoder will be the compressed
    /// version of `r`'s data.
    ///
    /// Note that there may be currently buffered data when this function is
    /// called, and in that case the buffered data is discarded.
    pub fn reset(&mut self, r: R) -> R {
        super::bufread::reset_encoder_data(&mut self.inner);
        self.inner.get_mut().reset(r)
    }

    /// Acquires a reference to the underlying stream
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }

    /// Consumes this encoder, returning the underlying reader.
    ///
    /// Note that there may be buffered bytes which are not re-acquired as part
    /// of this transition. It's recommended to only call this function after
    /// EOF has been reached.
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }

    /// Returns the number of bytes that have been read into this compressor.
    ///
    /// Note that not all bytes read from the underlying object may be accounted
    /// for, there may still be some active buffering.
    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }

    /// Returns the number of bytes that the compressor has produced.
    ///
    /// Note that not all bytes may have been read yet, some may still be
    /// buffered.
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
}

impl<R: Read> Read for ZlibEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<W: Read + Write> Write for ZlibEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A ZLIB decoder, or decompressor.
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// compressed data from the underlying [`Read`] and provides the uncompressed data.
///
/// After reading a single member of the ZLIB data this reader will return
/// Ok(0) even if there are more bytes available in the underlying reader.
/// `ZlibDecoder` may have read additional bytes past the end of the ZLIB data.
/// If you need the following bytes, wrap the `Reader` in a `std::io::BufReader`
/// and use `bufread::ZlibDecoder` instead.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::ZlibEncoder;
/// use flate2::read::ZlibDecoder;
///
/// # fn main() {
/// # let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
/// # e.write_all(b"Hello World").unwrap();
/// # let bytes = e.finish().unwrap();
/// # println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Zlib Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements Read
///
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///     let mut z = ZlibDecoder::new(&bytes[..]);
///     let mut s = String::new();
///     z.read_to_string(&mut s)?;
///     Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct ZlibDecoder<R> {
    inner: bufread::ZlibDecoder<BufReader<R>>,
}

impl<R: Read> ZlibDecoder<R> {
    /// Creates a new decoder which will decompress data read from the given
    /// stream.
    pub fn new(r: R) -> ZlibDecoder<R> {
        ZlibDecoder::new_with_buf(r, vec![0; 32 * 1024])
    }

    /// Creates a new decoder which will decompress data read from the given
    /// stream `r`, using `buf` as backing to speed up reading.
    ///
    /// Note that the specified buffer will only be used up to its current
    /// length. The buffer's capacity will also not grow over time.
    pub fn new_with_buf(r: R, buf: Vec<u8>) -> ZlibDecoder<R> {
        ZlibDecoder {
            inner: bufread::ZlibDecoder::new(BufReader::with_buf(buf, r)),
        }
    }

    /// Creates a new decoder which will decompress data read from the given
    /// stream `r`, along with `decompression` settings.
    pub fn new_with_decompress(r: R, decompression: Decompress) -> ZlibDecoder<R> {
        ZlibDecoder::new_with_decompress_and_buf(r, vec![0; 32 * 1024], decompression)
    }

    /// Creates a new decoder which will decompress data read from the given
    /// stream `r`, using `buf` as backing to speed up reading,
    /// along with `decompression` settings to configure decoder.
    ///
    /// Note that the specified buffer will only be used up to its current
    /// length. The buffer's capacity will also not grow over time.
    pub fn new_with_decompress_and_buf(
        r: R,
        buf: Vec<u8>,
        decompression: Decompress,
    ) -> ZlibDecoder<R> {
        ZlibDecoder {
            inner: bufread::ZlibDecoder::new_with_decompress(
                BufReader::with_buf(buf, r),
                decompression,
            ),
        }
    }
}

impl<R> ZlibDecoder<R> {
    /// Resets the state of this decoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This will reset the internal state of this decoder and replace the
    /// input stream with the one provided, returning the previous input
    /// stream. Future data read from this decoder will be the decompressed
    /// version of `r`'s data.
    ///
    /// Note that there may be currently buffered data when this function is
    /// called, and in that case the buffered data is discarded.
    pub fn reset(&mut self, r: R) -> R {
        super::bufread::reset_decoder_data(&mut self.inner);
        self.inner.get_mut().reset(r)
    }

    /// Acquires a reference to the underlying stream
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }

    /// Consumes this decoder, returning the underlying reader.
    ///
    /// Note that there may be buffered bytes which are not re-acquired as part
    /// of this transition. It's recommended to only call this function after
    /// EOF has been reached.
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }

    /// Returns the number of bytes that the decompressor has consumed.
    ///
    /// Note that this will likely be smaller than what the decompressor
    /// actually read from the underlying stream due to buffering.
    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }

    /// Returns the number of bytes that the decompressor has produced.
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }
}

impl<R: Read> Read for ZlibDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}

impl<R: Read + Write> Write for ZlibDecoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}
