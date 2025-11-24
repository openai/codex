use std::io;
use std::io::prelude::*;
use std::mem;

use crate::zio;
use crate::{Compress, Decompress};

/// A ZLIB encoder, or compressor.
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// uncompressed data from the underlying [`BufRead`] and provides the compressed data.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::bufread::ZlibEncoder;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// // Use a buffered file to compress contents into a Vec<u8>
///
/// # fn open_hello_world() -> std::io::Result<Vec<u8>> {
/// let f = File::open("examples/hello_world.txt")?;
/// let b = BufReader::new(f);
/// let mut z = ZlibEncoder::new(b, Compression::fast());
/// let mut buffer = Vec::new();
/// z.read_to_end(&mut buffer)?;
/// # Ok(buffer)
/// # }
/// ```
#[derive(Debug)]
pub struct ZlibEncoder<R> {
    obj: R,
    data: Compress,
}

impl<R: BufRead> ZlibEncoder<R> {
    /// Creates a new encoder which will read uncompressed data from the given
    /// stream and emit the compressed stream.
    pub fn new(r: R, level: crate::Compression) -> ZlibEncoder<R> {
        ZlibEncoder {
            obj: r,
            data: Compress::new(level, true),
        }
    }

    /// Creates a new encoder with the given `compression` settings which will
    /// read uncompressed data from the given stream `r` and emit the compressed stream.
    pub fn new_with_compress(r: R, compression: Compress) -> ZlibEncoder<R> {
        ZlibEncoder {
            obj: r,
            data: compression,
        }
    }
}

pub fn reset_encoder_data<R>(zlib: &mut ZlibEncoder<R>) {
    zlib.data.reset()
}

impl<R> ZlibEncoder<R> {
    /// Resets the state of this encoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This function will reset the internal state of this encoder and replace
    /// the input stream with the one provided, returning the previous input
    /// stream. Future data read from this encoder will be the compressed
    /// version of `r`'s data.
    pub fn reset(&mut self, r: R) -> R {
        reset_encoder_data(self);
        mem::replace(&mut self.obj, r)
    }

    /// Acquires a reference to the underlying reader
    pub fn get_ref(&self) -> &R {
        &self.obj
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.obj
    }

    /// Consumes this encoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.obj
    }

    /// Returns the number of bytes that have been read into this compressor.
    ///
    /// Note that not all bytes read from the underlying object may be accounted
    /// for, there may still be some active buffering.
    pub fn total_in(&self) -> u64 {
        self.data.total_in()
    }

    /// Returns the number of bytes that the compressor has produced.
    ///
    /// Note that not all bytes may have been read yet, some may still be
    /// buffered.
    pub fn total_out(&self) -> u64 {
        self.data.total_out()
    }
}

impl<R: BufRead> Read for ZlibEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, buf)
    }
}

impl<R: BufRead + Write> Write for ZlibEncoder<R> {
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
/// compressed data from the underlying [`BufRead`] and provides the uncompressed data.
///
/// After reading a single member of the ZLIB data this reader will return
/// Ok(0) even if there are more bytes available in the underlying reader.
/// If you need the following bytes, call `into_inner()` after Ok(0) to
/// recover the underlying reader.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::ZlibEncoder;
/// use flate2::bufread::ZlibDecoder;
///
/// # fn main() {
/// # let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
/// # e.write_all(b"Hello World").unwrap();
/// # let bytes = e.finish().unwrap();
/// # println!("{}", decode_bufreader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Zlib Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements BufRead
///
/// fn decode_bufreader(bytes: Vec<u8>) -> io::Result<String> {
///     let mut z = ZlibDecoder::new(&bytes[..]);
///     let mut s = String::new();
///     z.read_to_string(&mut s)?;
///     Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct ZlibDecoder<R> {
    obj: R,
    data: Decompress,
}

impl<R: BufRead> ZlibDecoder<R> {
    /// Creates a new decoder which will decompress data read from the given
    /// stream.
    pub fn new(r: R) -> ZlibDecoder<R> {
        ZlibDecoder {
            obj: r,
            data: Decompress::new(true),
        }
    }

    /// Creates a new decoder which will decompress data read from the given
    /// stream, using the given `decompression` settings.
    pub fn new_with_decompress(r: R, decompression: Decompress) -> ZlibDecoder<R> {
        ZlibDecoder {
            obj: r,
            data: decompression,
        }
    }
}

pub fn reset_decoder_data<R>(zlib: &mut ZlibDecoder<R>) {
    zlib.data = Decompress::new(true);
}

impl<R> ZlibDecoder<R> {
    /// Resets the state of this decoder entirely, swapping out the input
    /// stream for another.
    ///
    /// This will reset the internal state of this decoder and replace the
    /// input stream with the one provided, returning the previous input
    /// stream. Future data read from this decoder will be the decompressed
    /// version of `r`'s data.
    pub fn reset(&mut self, r: R) -> R {
        reset_decoder_data(self);
        mem::replace(&mut self.obj, r)
    }

    /// Acquires a reference to the underlying stream
    pub fn get_ref(&self) -> &R {
        &self.obj
    }

    /// Acquires a mutable reference to the underlying stream
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.obj
    }

    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.obj
    }

    /// Returns the number of bytes that the decompressor has consumed.
    ///
    /// Note that this will likely be smaller than what the decompressor
    /// actually read from the underlying stream due to buffering.
    pub fn total_in(&self) -> u64 {
        self.data.total_in()
    }

    /// Returns the number of bytes that the decompressor has produced.
    pub fn total_out(&self) -> u64 {
        self.data.total_out()
    }
}

impl<R: BufRead> Read for ZlibDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        zio::read(&mut self.obj, &mut self.data, into)
    }
}

impl<R: BufRead + Write> Write for ZlibDecoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

#[cfg(test)]
mod test {
    use crate::bufread::ZlibDecoder;
    use crate::zlib::write;
    use crate::Compression;
    use std::io::{Read, Write};

    // ZlibDecoder consumes one zlib archive and then returns 0 for subsequent reads, allowing any
    // additional data to be consumed by the caller.
    #[test]
    fn decode_extra_data() {
        let expected = "Hello World";

        let compressed = {
            let mut e = write::ZlibEncoder::new(Vec::new(), Compression::default());
            e.write(expected.as_ref()).unwrap();
            let mut b = e.finish().unwrap();
            b.push(b'x');
            b
        };

        let mut output = Vec::new();
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let decoded_bytes = decoder.read_to_end(&mut output).unwrap();
        assert_eq!(decoded_bytes, output.len());
        let actual = std::str::from_utf8(&output).expect("String parsing error");
        assert_eq!(
            actual, expected,
            "after decompression we obtain the original input"
        );

        output.clear();
        assert_eq!(
            decoder.read(&mut output).unwrap(),
            0,
            "subsequent read of decoder returns 0, but inner reader can return additional data"
        );
        let mut reader = decoder.into_inner();
        assert_eq!(
            reader.read_to_end(&mut output).unwrap(),
            1,
            "extra data is accessible in underlying buf-read"
        );
        assert_eq!(output, b"x");
    }
}
