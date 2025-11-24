use std::io;
use std::io::prelude::*;

use super::bufread;
use super::{GzBuilder, GzHeader};
use crate::bufreader::BufReader;
use crate::Compression;

/// A gzip streaming encoder
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
/// use flate2::read::GzEncoder;
///
/// // Return a vector containing the GZ compressed version of hello world
///
/// fn gzencode_hello_world() -> io::Result<Vec<u8>> {
///     let mut ret_vec = Vec::new();
///     let bytestring = b"hello world";
///     let mut gz = GzEncoder::new(&bytestring[..], Compression::fast());
///     gz.read_to_end(&mut ret_vec)?;
///     Ok(ret_vec)
/// }
/// ```
#[derive(Debug)]
pub struct GzEncoder<R> {
    inner: bufread::GzEncoder<BufReader<R>>,
}

pub fn gz_encoder<R: Read>(inner: bufread::GzEncoder<BufReader<R>>) -> GzEncoder<R> {
    GzEncoder { inner }
}

impl<R: Read> GzEncoder<R> {
    /// Creates a new encoder which will use the given compression level.
    ///
    /// The encoder is not configured specially for the emitted header. For
    /// header configuration, see the `GzBuilder` type.
    ///
    /// The data read from the stream `r` will be compressed and available
    /// through the returned reader.
    pub fn new(r: R, level: Compression) -> GzEncoder<R> {
        GzBuilder::new().read(r, level)
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

impl<R: Read> Read for GzEncoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}

impl<R: Read + Write> Write for GzEncoder<R> {
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
/// compressed data from the underlying [`Read`] and provides the uncompressed data.
///
/// After reading a single member of the gzip data this reader will return
/// Ok(0) even if there are more bytes available in the underlying reader.
/// `GzDecoder` may have read additional bytes past the end of the gzip data.
/// If you need the following bytes, wrap the `Reader` in a `std::io::BufReader`
/// and use `bufread::GzDecoder` instead.
///
/// To handle gzip files that may have multiple members, see [`MultiGzDecoder`]
/// or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::GzEncoder;
/// use flate2::read::GzDecoder;
///
/// # fn main() {
/// #    let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #    e.write_all(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Gz Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements Read
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
    inner: bufread::GzDecoder<BufReader<R>>,
}

impl<R: Read> GzDecoder<R> {
    /// Creates a new decoder from the given reader, immediately parsing the
    /// gzip header.
    pub fn new(r: R) -> GzDecoder<R> {
        GzDecoder {
            inner: bufread::GzDecoder::new(BufReader::new(r)),
        }
    }
}

impl<R> GzDecoder<R> {
    /// Returns the header associated with this stream, if it was valid.
    pub fn header(&self) -> Option<&GzHeader> {
        self.inner.header()
    }

    /// Acquires a reference to the underlying reader.
    ///
    /// Note that the decoder may have read past the end of the gzip data.
    /// To prevent this use [`bufread::GzDecoder`] instead.
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder continues to be used.
    ///
    /// Note that the decoder may have read past the end of the gzip data.
    /// To prevent this use [`bufread::GzDecoder`] instead.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }

    /// Consumes this decoder, returning the underlying reader.
    ///
    /// Note that the decoder may have read past the end of the gzip data.
    /// Subsequent reads will skip those bytes. To prevent this use
    /// [`bufread::GzDecoder`] instead.
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }
}

impl<R: Read> Read for GzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}

impl<R: Read + Write> Write for GzDecoder<R> {
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
/// compressed data from the underlying [`Read`] and provides the uncompressed
/// data.
///
/// A gzip file consists of a series of *members* concatenated one after another.
/// MultiGzDecoder decodes all members of a file and returns Ok(0) once the
/// underlying reader does.
///
/// To handle members separately, see [GzDecoder] or read more
/// [in the introduction](../index.html#about-multi-member-gzip-files).
///
/// [gzip file]: https://www.rfc-editor.org/rfc/rfc1952#page-5
///
/// # Examples
///
/// ```
/// use std::io::prelude::*;
/// use std::io;
/// # use flate2::Compression;
/// # use flate2::write::GzEncoder;
/// use flate2::read::MultiGzDecoder;
///
/// # fn main() {
/// #    let mut e = GzEncoder::new(Vec::new(), Compression::default());
/// #    e.write_all(b"Hello World").unwrap();
/// #    let bytes = e.finish().unwrap();
/// #    println!("{}", decode_reader(bytes).unwrap());
/// # }
/// #
/// // Uncompresses a Gz Encoded vector of bytes and returns a string or error
/// // Here &[u8] implements Read
///
/// fn decode_reader(bytes: Vec<u8>) -> io::Result<String> {
///    let mut gz = MultiGzDecoder::new(&bytes[..]);
///    let mut s = String::new();
///    gz.read_to_string(&mut s)?;
///    Ok(s)
/// }
/// ```
#[derive(Debug)]
pub struct MultiGzDecoder<R> {
    inner: bufread::MultiGzDecoder<BufReader<R>>,
}

impl<R: Read> MultiGzDecoder<R> {
    /// Creates a new decoder from the given reader, immediately parsing the
    /// (first) gzip header. If the gzip stream contains multiple members all will
    /// be decoded.
    pub fn new(r: R) -> MultiGzDecoder<R> {
        MultiGzDecoder {
            inner: bufread::MultiGzDecoder::new(BufReader::new(r)),
        }
    }
}

impl<R> MultiGzDecoder<R> {
    /// Returns the current header associated with this stream, if it's valid.
    pub fn header(&self) -> Option<&GzHeader> {
        self.inner.header()
    }

    /// Acquires a reference to the underlying reader.
    pub fn get_ref(&self) -> &R {
        self.inner.get_ref().get_ref()
    }

    /// Acquires a mutable reference to the underlying stream.
    ///
    /// Note that mutation of the stream may result in surprising results if
    /// this decoder is continued to be used.
    pub fn get_mut(&mut self) -> &mut R {
        self.inner.get_mut().get_mut()
    }

    /// Consumes this decoder, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.inner.into_inner().into_inner()
    }
}

impl<R: Read> Read for MultiGzDecoder<R> {
    fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
        self.inner.read(into)
    }
}

impl<R: Read + Write> Write for MultiGzDecoder<R> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, ErrorKind, Read, Result, Write};

    use super::GzDecoder;

    //a cursor turning EOF into blocking errors
    #[derive(Debug)]
    pub struct BlockingCursor {
        pub cursor: Cursor<Vec<u8>>,
    }

    impl BlockingCursor {
        pub fn new() -> BlockingCursor {
            BlockingCursor {
                cursor: Cursor::new(Vec::new()),
            }
        }

        pub fn set_position(&mut self, pos: u64) {
            return self.cursor.set_position(pos);
        }
    }

    impl Write for BlockingCursor {
        fn write(&mut self, buf: &[u8]) -> Result<usize> {
            return self.cursor.write(buf);
        }
        fn flush(&mut self) -> Result<()> {
            return self.cursor.flush();
        }
    }

    impl Read for BlockingCursor {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            //use the cursor, except it turns eof into blocking error
            let r = self.cursor.read(buf);
            match r {
                Err(ref err) => {
                    if err.kind() == ErrorKind::UnexpectedEof {
                        return Err(ErrorKind::WouldBlock.into());
                    }
                }
                Ok(0) => {
                    //regular EOF turned into blocking error
                    return Err(ErrorKind::WouldBlock.into());
                }
                Ok(_n) => {}
            }
            return r;
        }
    }

    #[test]
    fn blocked_partial_header_read() {
        // this is a reader which receives data afterwards
        let mut r = BlockingCursor::new();
        let data = vec![1, 2, 3];

        match r.write_all(&data) {
            Ok(()) => {}
            _ => {
                panic!("Unexpected result for write_all");
            }
        }
        r.set_position(0);

        // this is unused except for the buffering
        let mut decoder = GzDecoder::new(r);
        let mut out = Vec::with_capacity(7);
        match decoder.read(&mut out) {
            Err(e) => {
                assert_eq!(e.kind(), ErrorKind::WouldBlock);
            }
            _ => {
                panic!("Unexpected result for decoder.read");
            }
        }
    }
}
