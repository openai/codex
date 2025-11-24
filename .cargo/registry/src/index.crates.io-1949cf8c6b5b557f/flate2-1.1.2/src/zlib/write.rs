use std::io;
use std::io::prelude::*;

use crate::zio;
use crate::{Compress, Decompress};

/// A ZLIB encoder, or compressor.
///
/// This structure implements a [`Write`] interface and takes a stream of
/// uncompressed data, writing the compressed data to the wrapped writer.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::write::ZlibEncoder;
///
/// // Vec<u8> implements Write, assigning the compressed bytes of sample string
///
/// # fn zlib_encoding() -> std::io::Result<()> {
/// let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
/// e.write_all(b"Hello World")?;
/// let compressed = e.finish()?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct ZlibEncoder<W: Write> {
    inner: zio::Writer<W, Compress>,
}

impl<W: Write> ZlibEncoder<W> {
    /// Creates a new encoder which will write compressed data to the stream
    /// given at the given compression level.
    ///
    /// When this encoder is dropped or unwrapped the final pieces of data will
    /// be flushed.
    pub fn new(w: W, level: crate::Compression) -> ZlibEncoder<W> {
        ZlibEncoder {
            inner: zio::Writer::new(w, Compress::new(level, true)),
        }
    }

    /// Creates a new encoder which will write compressed data to the stream
    /// `w` with the given `compression` settings.
    pub fn new_with_compress(w: W, compression: Compress) -> ZlibEncoder<W> {
        ZlibEncoder {
            inner: zio::Writer::new(w, compression),
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt this
    /// object, so care must be taken when using this method.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// Resets the state of this encoder entirely, swapping out the output
    /// stream for another.
    ///
    /// This function will finish encoding the current stream into the current
    /// output stream before swapping out the two output streams.
    ///
    /// After the current stream has been finished, this will reset the internal
    /// state of this encoder and replace the output stream with the one
    /// provided, returning the previous output stream. Future data written to
    /// this encoder will be the compressed into the stream `w` provided.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn reset(&mut self, w: W) -> io::Result<W> {
        self.inner.finish()?;
        self.inner.data.reset();
        Ok(self.inner.replace(w))
    }

    /// Attempt to finish this output stream, writing out final chunks of data.
    ///
    /// Note that this function can only be used once data has finished being
    /// written to the output stream. After this function is called then further
    /// calls to `write` may result in a panic.
    ///
    /// # Panics
    ///
    /// Attempts to write data to this stream may result in a panic after this
    /// function is called.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.inner.finish()
    }

    /// Consumes this encoder, flushing the output stream.
    ///
    /// This will flush the underlying data stream, close off the compressed
    /// stream and, if successful, return the contained writer.
    ///
    /// Note that this function may not be suitable to call in a situation where
    /// the underlying stream is an asynchronous I/O stream. To finish a stream
    /// the `try_finish` (or `shutdown`) method should be used instead. To
    /// re-acquire ownership of a stream it is safe to call this method after
    /// `try_finish` or `shutdown` has returned `Ok`.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn finish(mut self) -> io::Result<W> {
        self.inner.finish()?;
        Ok(self.inner.take_inner())
    }

    /// Consumes this encoder, flushing the output stream.
    ///
    /// This will flush the underlying data stream and then return the contained
    /// writer if the flush succeeded.
    /// The compressed stream will not closed but only flushed. This
    /// means that obtained byte array can by extended by another deflated
    /// stream. To close the stream add the two bytes 0x3 and 0x0.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn flush_finish(mut self) -> io::Result<W> {
        self.inner.flush()?;
        Ok(self.inner.take_inner())
    }

    /// Returns the number of bytes that have been written to this compressor.
    ///
    /// Note that not all bytes written to this object may be accounted for,
    /// there may still be some active buffering.
    pub fn total_in(&self) -> u64 {
        self.inner.data.total_in()
    }

    /// Returns the number of bytes that the compressor has produced.
    ///
    /// Note that not all bytes may have been written yet, some may still be
    /// buffered.
    pub fn total_out(&self) -> u64 {
        self.inner.data.total_out()
    }
}

impl<W: Write> Write for ZlibEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Read + Write> Read for ZlibEncoder<W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_mut().read(buf)
    }
}

/// A ZLIB decoder, or decompressor.
///
/// This structure implements a [`Write`] and will emit a stream of decompressed
/// data when fed a stream of compressed data.
///
/// After decoding a single member of the ZLIB data this writer will return the number of bytes up
/// to the end of the ZLIB member and subsequent writes will return Ok(0) allowing the caller to
/// handle any data following the ZLIB member.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::ZlibEncoder;
/// use flate2::write::ZlibDecoder;
///
/// # fn main() {
/// #    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
/// #    e.write_all(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Zlib Encoded vector of bytes and returns a string or error
/// // Here Vec<u8> implements Write
///
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut writer = Vec::new();
///    let mut z = ZlibDecoder::new(writer);
///    z.write_all(&bytes[..])?;
///    writer = z.finish()?;
///    let return_string = String::from_utf8(writer).expect("String parsing error");
///    Ok(return_string)
/// }
/// ```
#[derive(Debug)]
pub struct ZlibDecoder<W: Write> {
    inner: zio::Writer<W, Decompress>,
}

impl<W: Write> ZlibDecoder<W> {
    /// Creates a new decoder which will write uncompressed data to the stream.
    ///
    /// When this decoder is dropped or unwrapped the final pieces of data will
    /// be flushed.
    pub fn new(w: W) -> ZlibDecoder<W> {
        ZlibDecoder {
            inner: zio::Writer::new(w, Decompress::new(true)),
        }
    }

    /// Creates a new decoder which will write uncompressed data to the stream `w`
    /// using the given `decompression` settings.
    ///
    /// When this decoder is dropped or unwrapped the final pieces of data will
    /// be flushed.
    pub fn new_with_decompress(w: W, decompression: Decompress) -> ZlibDecoder<W> {
        ZlibDecoder {
            inner: zio::Writer::new(w, decompression),
        }
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt this
    /// object, so care must be taken when using this method.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// Resets the state of this decoder entirely, swapping out the output
    /// stream for another.
    ///
    /// This will reset the internal state of this decoder and replace the
    /// output stream with the one provided, returning the previous output
    /// stream. Future data written to this decoder will be decompressed into
    /// the output stream `w`.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn reset(&mut self, w: W) -> io::Result<W> {
        self.inner.finish()?;
        self.inner.data = Decompress::new(true);
        Ok(self.inner.replace(w))
    }

    /// Attempt to finish this output stream, writing out final chunks of data.
    ///
    /// Note that this function can only be used once data has finished being
    /// written to the output stream. After this function is called then further
    /// calls to `write` may result in a panic.
    ///
    /// # Panics
    ///
    /// Attempts to write data to this stream may result in a panic after this
    /// function is called.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.inner.finish()
    }

    /// Consumes this encoder, flushing the output stream.
    ///
    /// This will flush the underlying data stream and then return the contained
    /// writer if the flush succeeded.
    ///
    /// Note that this function may not be suitable to call in a situation where
    /// the underlying stream is an asynchronous I/O stream. To finish a stream
    /// the `try_finish` (or `shutdown`) method should be used instead. To
    /// re-acquire ownership of a stream it is safe to call this method after
    /// `try_finish` or `shutdown` has returned `Ok`.
    ///
    /// # Errors
    ///
    /// This function will perform I/O to complete this stream, and any I/O
    /// errors which occur will be returned from this function.
    pub fn finish(mut self) -> io::Result<W> {
        self.inner.finish()?;
        Ok(self.inner.take_inner())
    }

    /// Returns the number of bytes that the decompressor has consumed for
    /// decompression.
    ///
    /// Note that this will likely be smaller than the number of bytes
    /// successfully written to this stream due to internal buffering.
    pub fn total_in(&self) -> u64 {
        self.inner.data.total_in()
    }

    /// Returns the number of bytes that the decompressor has written to its
    /// output stream.
    pub fn total_out(&self) -> u64 {
        self.inner.data.total_out()
    }
}

impl<W: Write> Write for ZlibDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Read + Write> Read for ZlibDecoder<W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.get_mut().read(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Compression;

    const STR: &str = "Hello World Hello World Hello World Hello World Hello World \
        Hello World Hello World Hello World Hello World Hello World \
        Hello World Hello World Hello World Hello World Hello World \
        Hello World Hello World Hello World Hello World Hello World \
        Hello World Hello World Hello World Hello World Hello World";

    // ZlibDecoder consumes one zlib archive and then returns 0 for subsequent writes, allowing any
    // additional data to be consumed by the caller.
    #[test]
    fn decode_extra_data() {
        let compressed = {
            let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
            e.write(STR.as_ref()).unwrap();
            let mut b = e.finish().unwrap();
            b.push(b'x');
            b
        };

        let mut writer = Vec::new();
        let mut decoder = ZlibDecoder::new(writer);
        let mut consumed_bytes = 0;
        loop {
            let n = decoder.write(&compressed[consumed_bytes..]).unwrap();
            if n == 0 {
                break;
            }
            consumed_bytes += n;
        }
        writer = decoder.finish().unwrap();
        let actual = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(actual, STR);
        assert_eq!(&compressed[consumed_bytes..], b"x");
    }
}
