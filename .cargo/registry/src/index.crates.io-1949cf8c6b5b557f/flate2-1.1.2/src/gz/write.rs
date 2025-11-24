use std::cmp;
use std::io;
use std::io::prelude::*;

use super::{corrupt, GzBuilder, GzHeader, GzHeaderParser};
use crate::crc::{Crc, CrcWriter};
use crate::zio;
use crate::{Compress, Compression, Decompress, Status};

/// A gzip streaming encoder
///
/// This structure exposes a [`Write`] interface that will emit compressed data
/// to the underlying writer `W`.
///
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use flate2::Compression;
/// use flate2::write::GzEncoder;
///
/// // Vec<u8> implements Write to print the compressed bytes of sample string
/// # fn main() {
///
/// let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// e.write_all(b"Hello World").unwrap();
/// println!("{:?}", e.finish().unwrap());
/// # }
/// ```
#[derive(Debug)]
pub struct GzEncoder<W: Write> {
    inner: zio::Writer<W, Compress>,
    crc: Crc,
    crc_bytes_written: usize,
    header: Vec<u8>,
}

pub fn gz_encoder<W: Write>(header: Vec<u8>, w: W, lvl: Compression) -> GzEncoder<W> {
    GzEncoder {
        inner: zio::Writer::new(w, Compress::new(lvl, false)),
        crc: Crc::new(),
        header,
        crc_bytes_written: 0,
    }
}

impl<W: Write> GzEncoder<W> {
    /// Creates a new encoder which will use the given compression level.
    ///
    /// The encoder is not configured specially for the emitted header. For
    /// header configuration, see the `GzBuilder` type.
    ///
    /// The data written to the returned encoder will be compressed and then
    /// written to the stream `w`.
    pub fn new(w: W, level: Compression) -> GzEncoder<W> {
        GzBuilder::new().write(w, level)
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutation of the writer may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
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
        self.write_header()?;
        self.inner.finish()?;

        while self.crc_bytes_written < 8 {
            let (sum, amt) = (self.crc.sum(), self.crc.amount());
            let buf = [
                sum as u8,
                (sum >> 8) as u8,
                (sum >> 16) as u8,
                (sum >> 24) as u8,
                amt as u8,
                (amt >> 8) as u8,
                (amt >> 16) as u8,
                (amt >> 24) as u8,
            ];
            let inner = self.inner.get_mut();
            let n = inner.write(&buf[self.crc_bytes_written..])?;
            self.crc_bytes_written += n;
        }
        Ok(())
    }

    /// Finish encoding this stream, returning the underlying writer once the
    /// encoding is done.
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
        self.try_finish()?;
        Ok(self.inner.take_inner())
    }

    fn write_header(&mut self) -> io::Result<()> {
        while !self.header.is_empty() {
            let n = self.inner.get_mut().write(&self.header)?;
            self.header.drain(..n);
        }
        Ok(())
    }
}

impl<W: Write> Write for GzEncoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        assert_eq!(self.crc_bytes_written, 0);
        self.write_header()?;
        let n = self.inner.write(buf)?;
        self.crc.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        assert_eq!(self.crc_bytes_written, 0);
        self.write_header()?;
        self.inner.flush()
    }
}

impl<R: Read + Write> Read for GzEncoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_mut().read(buf)
    }
}

impl<W: Write> Drop for GzEncoder<W> {
    fn drop(&mut self) {
        if self.inner.is_present() {
            let _ = self.try_finish();
        }
    }
}

/// A decoder for a single member of a [gzip file].
///
/// This structure exposes a [`Write`] interface, receiving compressed data and
/// writing uncompressed data to the underlying writer.
///
/// After decoding a single member of the gzip data this writer will return the number of bytes up to
/// to the end of the gzip member and subsequent writes will return Ok(0) allowing the caller to
/// handle any data following the gzip member.
///
/// To handle gzip files that may have multiple members, see [`MultiGzDecoder`]
/// or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// use flate2::Compression;
/// use flate2::write::{GzEncoder, GzDecoder};
///
/// # fn main() {
/// #    let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #    e.write(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    assert_eq!("Hello World", decode_writer(bytes).unwrap());
/// # }
/// // Uncompresses a gzip encoded vector of bytes and returns a string or error
/// // Here Vec<u8> implements Write
/// fn decode_writer(bytes: Vec<u8>) -> io::Result<String> {
///    let mut writer = Vec::new();
///    let mut decoder = GzDecoder::new(writer);
///    decoder.write_all(&bytes[..])?;
///    writer = decoder.finish()?;
///    let return_string = String::from_utf8(writer).expect("String parsing error");
///    Ok(return_string)
/// }
/// ```
#[derive(Debug)]
pub struct GzDecoder<W: Write> {
    inner: zio::Writer<CrcWriter<W>, Decompress>,
    crc_bytes: Vec<u8>,
    header_parser: GzHeaderParser,
}

const CRC_BYTES_LEN: usize = 8;

impl<W: Write> GzDecoder<W> {
    /// Creates a new decoder which will write uncompressed data to the stream.
    ///
    /// When this encoder is dropped or unwrapped the final pieces of data will
    /// be flushed.
    pub fn new(w: W) -> GzDecoder<W> {
        GzDecoder {
            inner: zio::Writer::new(CrcWriter::new(w), Decompress::new(false)),
            crc_bytes: Vec::with_capacity(CRC_BYTES_LEN),
            header_parser: GzHeaderParser::new(),
        }
    }

    /// Returns the header associated with this stream.
    pub fn header(&self) -> Option<&GzHeader> {
        self.header_parser.header()
    }

    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying writer.
    ///
    /// Note that mutating the output/input state of the stream may corrupt this
    /// object, so care must be taken when using this method.
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut().get_mut()
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
    /// This function will perform I/O to finish the stream, returning any
    /// errors which happen.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.finish_and_check_crc()?;
        Ok(())
    }

    /// Consumes this decoder, flushing the output stream.
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
        self.finish_and_check_crc()?;
        Ok(self.inner.take_inner().into_inner())
    }

    fn finish_and_check_crc(&mut self) -> io::Result<()> {
        self.inner.finish()?;

        if self.crc_bytes.len() != 8 {
            return Err(corrupt());
        }

        let crc = (self.crc_bytes[0] as u32)
            | ((self.crc_bytes[1] as u32) << 8)
            | ((self.crc_bytes[2] as u32) << 16)
            | ((self.crc_bytes[3] as u32) << 24);
        let amt = (self.crc_bytes[4] as u32)
            | ((self.crc_bytes[5] as u32) << 8)
            | ((self.crc_bytes[6] as u32) << 16)
            | ((self.crc_bytes[7] as u32) << 24);
        if crc != self.inner.get_ref().crc().sum() {
            return Err(corrupt());
        }
        if amt != self.inner.get_ref().crc().amount() {
            return Err(corrupt());
        }
        Ok(())
    }
}

impl<W: Write> Write for GzDecoder<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let buflen = buf.len();
        if self.header().is_none() {
            match self.header_parser.parse(&mut buf) {
                Err(err) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        // all data read but header still not complete
                        Ok(buflen)
                    } else {
                        Err(err)
                    }
                }
                Ok(_) => {
                    debug_assert!(self.header().is_some());
                    // buf now contains the unread part of the original buf
                    let n = buflen - buf.len();
                    Ok(n)
                }
            }
        } else {
            let (n, status) = self.inner.write_with_status(buf)?;

            if status == Status::StreamEnd && n < buf.len() && self.crc_bytes.len() < 8 {
                let remaining = buf.len() - n;
                let crc_bytes = cmp::min(remaining, CRC_BYTES_LEN - self.crc_bytes.len());
                self.crc_bytes.extend(&buf[n..n + crc_bytes]);
                return Ok(n + crc_bytes);
            }
            Ok(n)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Read + Write> Read for GzDecoder<W> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.get_mut().get_mut().read(buf)
    }
}

/// A gzip streaming decoder that decodes a [gzip file] with multiple members.
///
/// This structure exposes a [`Write`] interface that will consume compressed data and
/// write uncompressed data to the underlying writer.
///
/// A gzip file consists of a series of *members* concatenated one after another.
/// `MultiGzDecoder` decodes all members of a file and writes them to the
/// underlying writer one after another.
///
/// To handle members separately, see [GzDecoder] or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
#[derive(Debug)]
pub struct MultiGzDecoder<W: Write> {
    inner: GzDecoder<W>,
}

impl<W: Write> MultiGzDecoder<W> {
    /// Creates a new decoder which will write uncompressed data to the stream.
    /// If the gzip stream contains multiple members all will be decoded.
    pub fn new(w: W) -> MultiGzDecoder<W> {
        MultiGzDecoder {
            inner: GzDecoder::new(w),
        }
    }

    /// Returns the header associated with the current member.
    pub fn header(&self) -> Option<&GzHeader> {
        self.inner.header()
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
    /// This function will perform I/O to finish the stream, returning any
    /// errors which happen.
    pub fn try_finish(&mut self) -> io::Result<()> {
        self.inner.try_finish()
    }

    /// Consumes this decoder, flushing the output stream.
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
    pub fn finish(self) -> io::Result<W> {
        self.inner.finish()
    }
}

impl<W: Write> Write for MultiGzDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            Ok(0)
        } else {
            match self.inner.write(buf) {
                Ok(0) => {
                    // When the GzDecoder indicates that it has finished
                    // create a new GzDecoder to handle additional data.
                    self.inner.try_finish()?;
                    let w = self.inner.inner.take_inner().into_inner();
                    self.inner = GzDecoder::new(w);
                    self.inner.write(buf)
                }
                res => res,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STR: &str = "Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World \
                               Hello World Hello World Hello World Hello World Hello World";

    #[test]
    fn decode_writer_one_chunk() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        let n = decoder.write(&bytes[..]).unwrap();
        decoder.write(&bytes[n..]).unwrap();
        decoder.try_finish().unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_header() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..5]).unwrap(), 5);
        let n = decoder.write(&bytes[5..]).unwrap();
        if n < bytes.len() - 5 {
            decoder.write(&bytes[n + 5..]).unwrap();
        }
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_header_filename() {
        let filename = "test.txt";
        let mut e = GzBuilder::new()
            .filename(filename)
            .read(STR.as_bytes(), Compression::default());
        let mut bytes = Vec::new();
        e.read_to_end(&mut bytes).unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..12]).unwrap(), 12);
        let n = decoder.write(&bytes[12..]).unwrap();
        if n < bytes.len() - 12 {
            decoder.write(&bytes[n + 12..]).unwrap();
        }
        assert_eq!(
            decoder.header().unwrap().filename().unwrap(),
            filename.as_bytes()
        );
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_header_comment() {
        let comment = "test comment";
        let mut e = GzBuilder::new()
            .comment(comment)
            .read(STR.as_bytes(), Compression::default());
        let mut bytes = Vec::new();
        e.read_to_end(&mut bytes).unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..12]).unwrap(), 12);
        let n = decoder.write(&bytes[12..]).unwrap();
        if n < bytes.len() - 12 {
            decoder.write(&bytes[n + 12..]).unwrap();
        }
        assert_eq!(
            decoder.header().unwrap().comment().unwrap(),
            comment.as_bytes()
        );
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_exact_header() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        assert_eq!(decoder.write(&bytes[..10]).unwrap(), 10);
        decoder.write(&bytes[10..]).unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    #[test]
    fn decode_writer_partial_crc() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap();

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
        let l = bytes.len() - 5;
        let n = decoder.write(&bytes[..l]).unwrap();
        decoder.write(&bytes[n..]).unwrap();
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        assert_eq!(return_string, STR);
    }

    // Two or more gzip files concatenated form a multi-member gzip file. MultiGzDecoder will
    // concatenate the decoded contents of all members.
    #[test]
    fn decode_multi_writer() {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write(STR.as_ref()).unwrap();
        let bytes = e.finish().unwrap().repeat(2);

        let mut writer = Vec::new();
        let mut decoder = MultiGzDecoder::new(writer);
        let mut count = 0;
        while count < bytes.len() {
            let n = decoder.write(&bytes[count..]).unwrap();
            assert!(n != 0);
            count += n;
        }
        writer = decoder.finish().unwrap();
        let return_string = String::from_utf8(writer).expect("String parsing error");
        let expected = STR.repeat(2);
        assert_eq!(return_string, expected);
    }

    // GzDecoder consumes one gzip member and then returns 0 for subsequent writes, allowing any
    // additional data to be consumed by the caller.
    #[test]
    fn decode_extra_data() {
        let compressed = {
            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            e.write(STR.as_ref()).unwrap();
            let mut b = e.finish().unwrap();
            b.push(b'x');
            b
        };

        let mut writer = Vec::new();
        let mut decoder = GzDecoder::new(writer);
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
