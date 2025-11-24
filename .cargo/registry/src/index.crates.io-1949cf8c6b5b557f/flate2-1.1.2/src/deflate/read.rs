use std::io;
use std::io::prelude::*;

use super::bufread;
use crate::bufreader::BufReader;

/// A DEFLATE encoder, or compressor.
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
/// use std::io;
/// use flate2::Compression;
/// use flate2::read::DeflateEncoder;
///
/// # fn main() {
/// #    println!("{:?}", deflateencoder_read_hello_world().unwrap());
/// # }
/// #
/// // Return a vector containing the Deflate compressed version of hello world
/// fn deflateencoder_read_hello_world() -> io::Result<Vec<u8>> {
///    let mut ret_vec = Vec::new();
///    let c = b"hello world";
///    let mut deflater = DeflateEncoder::new(&c[..], Compression::fast());
///    deflater.read_to_end(&mut ret_vec)?;
///    Ok(ret_vec)
/// }
/// ```
#[derive(Debug)]
pub struct DeflateEncoder<R> {
    inner: bufread::DeflateEncoder<BufReader<R>>,
}

impl<R: Read> DeflateEncoder<R> {
    /// Creates a new encoder which will read uncompressed data from the given
    /// stream and emit the compressed stream.
    pub fn new(r: R, level: crate::Compression) -> DeflateEncoder<R> {
        DeflateEncoder {
            inner: bufread::DeflateEncoder::new(BufReader::new(r), level),
        }
    }
}

impl<R> DeflateEncoder<R> {
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

    /// Acquires a reference to the underlying reader
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

impl<R: Read> Read for DeflateEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<W: Read + Write> Write for DeflateEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A DEFLATE decoder, or decompressor.
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// compressed data from the underlying [`Read`] and provides the uncompressed data.
///
/// After reading a single member of the DEFLATE data this reader will return
/// Ok(0) even if there are more bytes available in the underlying reader.
/// `DeflateDecoder` may have read additional bytes past the end of the DEFLATE data.
/// If you need the following bytes, wrap the `Reader` in a `std::io::BufReader`
/// and use `bufread::DeflateDecoder` instead.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::DeflateEncoder;
/// use flate2::read::DeflateDecoder;
///
/// # fn main() {
/// #    let mut e = DeflateEncoder::new(Vec::new(), Compression::default());
/// #    e.write_all(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    println!("{}", decode_reader(bytes).unwrap());
/// # }
/// // Uncompresses a Deflate Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements Read
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut deflater = DeflateDecoder::new(&bytes[..]);
///    let mut s = String::new();
///    deflater.read_to_string(&mut s)?;
///    Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct DeflateDecoder<R> {
    inner: bufread::DeflateDecoder<BufReader<R>>,
}

impl<R: Read> DeflateDecoder<R> {
    /// Creates a new decoder which will decompress data read from the given
    /// stream.
    pub fn new(r: R) -> DeflateDecoder<R> {
        DeflateDecoder::new_with_buf(r, vec![0; 32 * 1024])
    }

    /// Same as `new`, but the intermediate buffer for data is specified.
    ///
    /// Note that the capacity of the intermediate buffer is never increased,
    /// and it is recommended for it to be large.
    pub fn new_with_buf(r: R, buf: Vec<u8>) -> DeflateDecoder<R> {
        DeflateDecoder {
            inner: bufread::DeflateDecoder::new(BufReader::with_buf(buf, r)),
        }
    }
}

impl<R> DeflateDecoder<R> {
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

impl<R: Read> Read for DeflateDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}

impl<W: Read + Write> Write for DeflateDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}
