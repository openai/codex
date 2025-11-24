use std::cmp;
use std::io;
use std::io::prelude::*;
use std::mem;

use super::{corrupt, read_into, GzBuilder, GzHeader, GzHeaderParser};
use crate::crc::CrcReader;
use crate::deflate;
use crate::Compression;

fn copy(into: &mut [u8], from: &[u8], pos: &mut usize) -> usize {
    let min = cmp::min(into.len(), from.len() - *pos);
    into[..min].copy_from_slice(&from[*pos..*pos + min]);
    *pos += min;
    min
}

/// A gzip streaming encoder
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
/// use std::io;
/// use flate2::Compression;
/// use flate2::bufread::GzEncoder;
/// use std::fs::File;
/// use std::io::BufReader;
///
/// // Opens sample file, compresses the contents and returns a Vector or error
/// // File wrapped in a BufReader implements BufRead
///
/// fn open_hello_world() -> io::Result<Vec<u8>> {
///     let f = File::open("examples/hello_world.txt")?;
///     let b = BufReader::new(f);
///     let mut gz = GzEncoder::new(b, Compression::fast());
///     let mut buffer = Vec::new();
///     gz.read_to_end(&mut buffer)?;
///     Ok(buffer)
/// }
/// ```
#[derive(Debug)]
pub struct GzEncoder<R> {
    inner: deflate::bufread::DeflateEncoder<CrcReader<R>>,
    header: Vec<u8>,
    pos: usize,
    eof: bool,
}

pub fn gz_encoder<R: BufRead>(header: Vec<u8>, r: R, lvl: Compression) -> GzEncoder<R> {
    let crc = CrcReader::new(r);
    GzEncoder {
        inner: deflate::bufread::DeflateEncoder::new(crc, lvl),
        header,
        pos: 0,
        eof: false,
    }
}

impl<R: BufRead> GzEncoder<R> {
    /// Creates a new encoder which will use the given compression level.
    ///
    /// The encoder is not configured specially for the emitted header. For
    /// header configuration, see the `GzBuilder` type.
    ///
    /// The data read from the stream `r` will be compressed and available
    /// through the returned reader.
    pub fn new(r: R, level: Compression) -> GzEncoder<R> {
        GzBuilder::new().buf_read(r, level)
    }

    fn read_footer(&mut self, into: &mut [u8]) -> io::Result<usize> {
        if self.pos == 8 {
            return Ok(0);
        }
        let crc = self.inner.get_ref().crc();
        let calced_crc_bytes = crc.sum().to_le_bytes();
        let arr = [
            calced_crc_bytes[0],
            calced_crc_bytes[1],
            calced_crc_bytes[2],
            calced_crc_bytes[3],
            crc.amount() as u8,
            (crc.amount() >> 8) as u8,
            (crc.amount() >> 16) as u8,
            (crc.amount() >> 24) as u8,
        ];
        Ok(copy(into, &arr, &mut self.pos))
    }
}

impl<R> GzEncoder<R> {
    /// Acquires a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying reader.
    ///
    /// Note that mutation of the reader may result in surprising results if
    /// this encoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }

    /// Returns the underlying stream, consuming this encoder
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }
}

#[inline]
fn finish(buf: &[u8; 8]) -> (u32, u32) {
    let crc = (buf[0] as u32)
        | ((buf[1] as u32) << 8)
        | ((buf[2] as u32) << 16)
        | ((buf[3] as u32) << 24);
    let amt = (buf[4] as u32)
        | ((buf[5] as u32) << 8)
        | ((buf[6] as u32) << 16)
        | ((buf[7] as u32) << 24);
    (crc, amt)
}

impl<R: BufRead> Read for GzEncoder<R> {
    fn read(&mut self, mut into: &mut [u8]) -> io::Result<usize> {
        let mut amt = 0;
        if self.eof {
            return self.read_footer(into);
        } else if self.pos < self.header.len() {
            amt += copy(into, &self.header, &mut self.pos);
            if amt == into.len() {
                return Ok(amt);
            }
            let tmp = into;
            into = &mut tmp[amt..];
        }
        match self.inner.read(into)? {
            0 => {
                self.eof = true;
                self.pos = 0;
                self.read_footer(into)
            }
            n => Ok(amt + n),
        }
    }
}

impl<R: BufRead + Write> Write for GzEncoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A decoder for a single member of a [gzip file].
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// compressed data from the underlying [`BufRead`] and provides the uncompressed data.
///
/// After reading a single member of the gzip data this reader will return
/// Ok(0) even if there are more bytes available in the underlying reader.
/// If you need the following bytes, call `into_inner()` after Ok(0) to
/// recover the underlying reader.
///
/// To handle gzip files that may have multiple members, see [`MultiGzDecoder`]
/// or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::GzEncoder;
/// use flate2::bufread::GzDecoder;
///
/// # fn main() {
/// #   let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #   e.write_all(b"Hello World").unwrap();
/// #   let bytes = e.finish().unwrap();
/// #   println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Gz Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements BufRead
///
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut gz = GzDecoder::new(&bytes[..]);
///    let mut s = String::new();
///    gz.read_to_string(&mut s)?;
///    Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct GzDecoder<R> {
    state: GzState,
    reader: CrcReader<deflate::bufread::DeflateDecoder<R>>,
    multi: bool,
}

#[derive(Debug)]
enum GzState {
    Header(GzHeaderParser),
    Body(GzHeader),
    Finished(GzHeader, usize, [u8; 8]),
    Err(io::Error),
    End(Option<GzHeader>),
}

impl<R: BufRead> GzDecoder<R> {
    /// Creates a new decoder from the given reader, immediately parsing the
    /// gzip header.
    pub fn new(mut r: R) -> GzDecoder<R> {
        let mut header_parser = GzHeaderParser::new();

        let state = match header_parser.parse(&mut r) {
            Ok(_) => GzState::Body(GzHeader::from(header_parser)),
            Err(ref err) if io::ErrorKind::WouldBlock == err.kind() => {
                GzState::Header(header_parser)
            }
            Err(err) => GzState::Err(err),
        };

        GzDecoder {
            state,
            reader: CrcReader::new(deflate::bufread::DeflateDecoder::new(r)),
            multi: false,
        }
    }

    fn multi(mut self, flag: bool) -> GzDecoder<R> {
        self.multi = flag;
        self
    }
}

impl<R> GzDecoder<R> {
    /// Returns the header associated with this stream, if it was valid
    pub fn header(&self) -> Option<&GzHeader> {
        match &self.state {
            GzState::Body(header) | GzState::Finished(header, _, _) => Some(header),
            GzState::End(header) => header.as_ref(),
            _ => None,
        }
    }

    /// Acquires a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.reader.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.reader.get_mut().get_mut()
    }

    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader.into_inner().into_inner()
    }
}

impl<R: BufRead> Read for GzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        loop {
            match &mut self.state {
                GzState::Header(parser) => {
                    parser.parse(self.reader.get_mut().get_mut())?;
                    self.state = GzState::Body(GzHeader::from(mem::take(parser)));
                }
                GzState::Body(header) => {
                    if into.is_empty() {
                        return Ok(0);
                    }
                    match self.reader.read(into)? {
                        0 => {
                            self.state = GzState::Finished(mem::take(header), 0, [0; 8]);
                        }
                        n => {
                            return Ok(n);
                        }
                    }
                }
                GzState::Finished(header, pos, buf) => {
                    if *pos < buf.len() {
                        *pos += read_into(self.reader.get_mut().get_mut(), &mut buf[*pos..])?;
                    } else {
                        let (crc, amt) = finish(buf);

                        if crc != self.reader.crc().sum() || amt != self.reader.crc().amount() {
                            self.state = GzState::End(Some(mem::take(header)));
                            return Err(corrupt());
                        } else if self.multi {
                            let is_eof = self
                                .reader
                                .get_mut()
                                .get_mut()
                                .fill_buf()
                                .map(|buf| buf.is_empty())?;

                            if is_eof {
                                self.state = GzState::End(Some(mem::take(header)));
                            } else {
                                self.reader.reset();
                                self.reader.get_mut().reset_data();
                                self.state = GzState::Header(GzHeaderParser::new())
                            }
                        } else {
                            self.state = GzState::End(Some(mem::take(header)));
                        }
                    }
                }
                GzState::Err(err) => {
                    let result = Err(mem::replace(err, io::ErrorKind::Other.into()));
                    self.state = GzState::End(None);
                    return result;
                }
                GzState::End(_) => return Ok(0),
            }
        }
    }
}

impl<R: BufRead + Write> Write for GzDecoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

/// A gzip streaming decoder that decodes a [gzip file] that may have multiple members.
///
/// This structure implements a [`Read`] interface. When read from, it reads
/// compressed data from the underlying [`BufRead`] and provides the uncompressed data.
///
/// A gzip file consists of a series of *members* concatenated one after another.
/// MultiGzDecoder decodes all members from the data and only returns Ok(0) when the
/// underlying reader does. For a file, this reads to the end of the file.
///
/// To handle members separately, see [GzDecoder] or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`BufRead`]: https://doc.rust-lang.org/std/io/trait.BufRead.html
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::GzEncoder;
/// use flate2::bufread::MultiGzDecoder;
///
/// # fn main() {
/// #   let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #   e.write_all(b"Hello World").unwrap();
/// #   let bytes = e.finish().unwrap();
/// #   println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Gz Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements BufRead
///
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut gz = MultiGzDecoder::new(&bytes[..]);
///    let mut s = String::new();
///    gz.read_to_string(&mut s)?;
///    Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct MultiGzDecoder<R>(GzDecoder<R>);

impl<R: BufRead> MultiGzDecoder<R> {
    /// Creates a new decoder from the given reader, immediately parsing the
    /// (first) gzip header. If the gzip stream contains multiple members all will
    /// be decoded.
    pub fn new(r: R) -> MultiGzDecoder<R> {
        MultiGzDecoder(GzDecoder::new(r).multi(true))
    }
}

impl<R> MultiGzDecoder<R> {
    /// Returns the current header associated with this stream, if it's valid
    pub fn header(&self) -> Option<&GzHeader> {
        self.0.header()
    }

    /// Acquires a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.0.get_ref()
    }

    /// Acquires a mutable reference to the underlying stream.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.0.get_mut()
    }

    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.0.into_inner()
    }
}

impl<R: BufRead> Read for MultiGzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.0.read(into)
    }
}

#[cfg(test)]
mod test {
    use crate::bufread::GzDecoder;
    use crate::gz::write;
    use crate::Compression;
    use std::io::{Read, Write};

    // GzDecoder consumes one gzip member and then returns 0 for subsequent reads, allowing any
    // additional data to be consumed by the caller.
    #[test]
    fn decode_extra_data() {
        let expected = "Hello World";

        let compressed = {
            let mut e = write::GzEncoder::new(Vec::new(), Compression::default());
            e.write(expected.as_ref()).unwrap();
            let mut b = e.finish().unwrap();
            b.push(b'x');
            b
        };

        let mut output = Vec::new();
        let mut decoder = GzDecoder::new(compressed.as_slice());
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
