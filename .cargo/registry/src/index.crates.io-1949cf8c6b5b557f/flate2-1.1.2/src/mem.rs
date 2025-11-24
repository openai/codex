use std::error::Error;
use std::fmt;
use std::io;

use crate::ffi::{self, Backend, Deflate, DeflateBackend, ErrorMessage, Inflate, InflateBackend};
use crate::Compression;

/// Raw in-memory compression stream for blocks of data.
///
/// This type is the building block for the I/O streams in the rest of this
/// crate. It requires more management than the [`Read`]/[`Write`] API but is
/// maximally flexible in terms of accepting input from any source and being
/// able to produce output to any memory location.
///
/// It is recommended to use the I/O stream adaptors over this type as they're
/// easier to use.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
#[derive(Debug)]
pub struct Compress {
    inner: Deflate,
}

/// Raw in-memory decompression stream for blocks of data.
///
/// This type is the building block for the I/O streams in the rest of this
/// crate. It requires more management than the [`Read`]/[`Write`] API but is
/// maximally flexible in terms of accepting input from any source and being
/// able to produce output to any memory location.
///
/// It is recommended to use the I/O stream adaptors over this type as they're
/// easier to use.
///
/// [`Read`]: https://doc.rust-lang.org/std/io/trait.Read.html
/// [`Write`]: https://doc.rust-lang.org/std/io/trait.Write.html
#[derive(Debug)]
pub struct Decompress {
    inner: Inflate,
}

/// Values which indicate the form of flushing to be used when compressing
/// in-memory data.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[allow(clippy::unnecessary_cast)]
pub enum FlushCompress {
    /// A typical parameter for passing to compression/decompression functions,
    /// this indicates that the underlying stream to decide how much data to
    /// accumulate before producing output in order to maximize compression.
    None = ffi::MZ_NO_FLUSH as isize,

    /// All pending output is flushed to the output buffer, but the output is
    /// not aligned to a byte boundary.
    ///
    /// All input data so far will be available to the decompressor (as with
    /// `Flush::Sync`). This completes the current deflate block and follows it
    /// with an empty fixed codes block that is 10 bytes long, and it assures
    /// that enough bytes are output in order for the decompressor to finish the
    /// block before the empty fixed code block.
    Partial = ffi::MZ_PARTIAL_FLUSH as isize,

    /// All pending output is flushed to the output buffer and the output is
    /// aligned on a byte boundary so that the decompressor can get all input
    /// data available so far.
    ///
    /// Flushing may degrade compression for some compression algorithms and so
    /// it should only be used when necessary. This will complete the current
    /// deflate block and follow it with an empty stored block.
    Sync = ffi::MZ_SYNC_FLUSH as isize,

    /// All output is flushed as with `Flush::Sync` and the compression state is
    /// reset so decompression can restart from this point if previous
    /// compressed data has been damaged or if random access is desired.
    ///
    /// Using this option too often can seriously degrade compression.
    Full = ffi::MZ_FULL_FLUSH as isize,

    /// Pending input is processed and pending output is flushed.
    ///
    /// The return value may indicate that the stream is not yet done and more
    /// data has yet to be processed.
    Finish = ffi::MZ_FINISH as isize,
}

/// Values which indicate the form of flushing to be used when
/// decompressing in-memory data.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
#[allow(clippy::unnecessary_cast)]
pub enum FlushDecompress {
    /// A typical parameter for passing to compression/decompression functions,
    /// this indicates that the underlying stream to decide how much data to
    /// accumulate before producing output in order to maximize compression.
    None = ffi::MZ_NO_FLUSH as isize,

    /// All pending output is flushed to the output buffer and the output is
    /// aligned on a byte boundary so that the decompressor can get all input
    /// data available so far.
    ///
    /// Flushing may degrade compression for some compression algorithms and so
    /// it should only be used when necessary. This will complete the current
    /// deflate block and follow it with an empty stored block.
    Sync = ffi::MZ_SYNC_FLUSH as isize,

    /// Pending input is processed and pending output is flushed.
    ///
    /// The return value may indicate that the stream is not yet done and more
    /// data has yet to be processed.
    Finish = ffi::MZ_FINISH as isize,
}

/// The inner state for an error when decompressing
#[derive(Clone, Debug)]
pub(crate) enum DecompressErrorInner {
    General { msg: ErrorMessage },
    NeedsDictionary(u32),
}

/// Error returned when a decompression object finds that the input stream of
/// bytes was not a valid input stream of bytes.
#[derive(Clone, Debug)]
pub struct DecompressError(pub(crate) DecompressErrorInner);

impl DecompressError {
    /// Indicates whether decompression failed due to requiring a dictionary.
    ///
    /// The resulting integer is the Adler-32 checksum of the dictionary
    /// required.
    pub fn needs_dictionary(&self) -> Option<u32> {
        match self.0 {
            DecompressErrorInner::NeedsDictionary(adler) => Some(adler),
            _ => None,
        }
    }
}

#[inline]
pub(crate) fn decompress_failed<T>(msg: ErrorMessage) -> Result<T, DecompressError> {
    Err(DecompressError(DecompressErrorInner::General { msg }))
}

#[inline]
pub(crate) fn decompress_need_dict<T>(adler: u32) -> Result<T, DecompressError> {
    Err(DecompressError(DecompressErrorInner::NeedsDictionary(
        adler,
    )))
}

/// Error returned when a compression object is used incorrectly or otherwise
/// generates an error.
#[derive(Clone, Debug)]
pub struct CompressError {
    pub(crate) msg: ErrorMessage,
}

#[inline]
pub(crate) fn compress_failed<T>(msg: ErrorMessage) -> Result<T, CompressError> {
    Err(CompressError { msg })
}

/// Possible status results of compressing some data or successfully
/// decompressing a block of data.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Status {
    /// Indicates success.
    ///
    /// Means that more input may be needed but isn't available
    /// and/or there's more output to be written but the output buffer is full.
    Ok,

    /// Indicates that forward progress is not possible due to input or output
    /// buffers being empty.
    ///
    /// For compression it means the input buffer needs some more data or the
    /// output buffer needs to be freed up before trying again.
    ///
    /// For decompression this means that more input is needed to continue or
    /// the output buffer isn't large enough to contain the result. The function
    /// can be called again after fixing both.
    BufError,

    /// Indicates that all input has been consumed and all output bytes have
    /// been written. Decompression/compression should not be called again.
    ///
    /// For decompression with zlib streams the adler-32 of the decompressed
    /// data has also been verified.
    StreamEnd,
}

impl Compress {
    /// Creates a new object ready for compressing data that it's given.
    ///
    /// The `level` argument here indicates what level of compression is going
    /// to be performed, and the `zlib_header` argument indicates whether the
    /// output data should have a zlib header or not.
    pub fn new(level: Compression, zlib_header: bool) -> Compress {
        Compress {
            inner: Deflate::make(level, zlib_header, ffi::MZ_DEFAULT_WINDOW_BITS as u8),
        }
    }

    /// Creates a new object ready for compressing data that it's given.
    ///
    /// The `level` argument here indicates what level of compression is going
    /// to be performed, and the `zlib_header` argument indicates whether the
    /// output data should have a zlib header or not. The `window_bits` parameter
    /// indicates the base-2 logarithm of the sliding window size and must be
    /// between 9 and 15.
    ///
    /// # Panics
    ///
    /// If `window_bits` does not fall into the range 9 ..= 15,
    /// `new_with_window_bits` will panic.
    #[cfg(feature = "any_zlib")]
    pub fn new_with_window_bits(
        level: Compression,
        zlib_header: bool,
        window_bits: u8,
    ) -> Compress {
        assert!(
            window_bits > 8 && window_bits < 16,
            "window_bits must be within 9 ..= 15"
        );
        Compress {
            inner: Deflate::make(level, zlib_header, window_bits),
        }
    }

    /// Creates a new object ready for compressing data that it's given.
    ///
    /// The `level` argument here indicates what level of compression is going
    /// to be performed.
    ///
    /// The Compress object produced by this constructor outputs gzip headers
    /// for the compressed data.
    ///
    /// # Panics
    ///
    /// If `window_bits` does not fall into the range 9 ..= 15,
    /// `new_with_window_bits` will panic.
    #[cfg(feature = "any_zlib")]
    pub fn new_gzip(level: Compression, window_bits: u8) -> Compress {
        assert!(
            window_bits > 8 && window_bits < 16,
            "window_bits must be within 9 ..= 15"
        );
        Compress {
            inner: Deflate::make(level, true, window_bits + 16),
        }
    }

    /// Returns the total number of input bytes which have been processed by
    /// this compression object.
    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }

    /// Returns the total number of output bytes which have been produced by
    /// this compression object.
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }

    /// Specifies the compression dictionary to use.
    ///
    /// Returns the Adler-32 checksum of the dictionary.
    #[cfg(feature = "any_zlib")]
    pub fn set_dictionary(&mut self, dictionary: &[u8]) -> Result<u32, CompressError> {
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self.inner.inner.stream_wrapper`.
        let stream = self.inner.inner.stream_wrapper.inner;
        let rc = unsafe {
            (*stream).msg = std::ptr::null_mut();
            assert!(dictionary.len() < ffi::uInt::MAX as usize);
            ffi::deflateSetDictionary(stream, dictionary.as_ptr(), dictionary.len() as ffi::uInt)
        };

        match rc {
            ffi::MZ_STREAM_ERROR => compress_failed(self.inner.inner.msg()),
            #[allow(clippy::unnecessary_cast)]
            ffi::MZ_OK => Ok(unsafe { (*stream).adler } as u32),
            c => panic!("unknown return code: {}", c),
        }
    }

    /// Quickly resets this compressor without having to reallocate anything.
    ///
    /// This is equivalent to dropping this object and then creating a new one.
    pub fn reset(&mut self) {
        self.inner.reset();
    }

    /// Dynamically updates the compression level.
    ///
    /// This can be used to switch between compression levels for different
    /// kinds of data, or it can be used in conjunction with a call to reset
    /// to reuse the compressor.
    ///
    /// This may return an error if there wasn't enough output space to complete
    /// the compression of the available input data before changing the
    /// compression level. Flushing the stream before calling this method
    /// ensures that the function will succeed on the first call.
    #[cfg(feature = "any_zlib")]
    pub fn set_level(&mut self, level: Compression) -> Result<(), CompressError> {
        use std::os::raw::c_int;
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self.inner.inner.stream_wrapper`.
        let stream = self.inner.inner.stream_wrapper.inner;
        unsafe {
            (*stream).msg = std::ptr::null_mut();
        }
        let rc = unsafe { ffi::deflateParams(stream, level.0 as c_int, ffi::MZ_DEFAULT_STRATEGY) };

        match rc {
            ffi::MZ_OK => Ok(()),
            ffi::MZ_BUF_ERROR => compress_failed(self.inner.inner.msg()),
            c => panic!("unknown return code: {}", c),
        }
    }

    /// Compresses the input data into the output, consuming only as much
    /// input as needed and writing as much output as possible.
    ///
    /// The flush option can be any of the available `FlushCompress` parameters.
    ///
    /// To learn how much data was consumed or how much output was produced, use
    /// the `total_in` and `total_out` functions before/after this is called.
    pub fn compress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        self.inner.compress(input, output, flush)
    }

    /// Compresses the input data into the extra space of the output, consuming
    /// only as much input as needed and writing as much output as possible.
    ///
    /// This function has the same semantics as `compress`, except that the
    /// length of `vec` is managed by this function. This will not reallocate
    /// the vector provided or attempt to grow it, so space for the output must
    /// be reserved in the output vector by the caller before calling this
    /// function.
    pub fn compress_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushCompress,
    ) -> Result<Status, CompressError> {
        write_to_spare_capacity_of_vec(output, |out| {
            let before = self.total_out();
            let ret = self.compress(input, out, flush);
            let bytes_written = self.total_out() - before;
            (bytes_written as usize, ret)
        })
    }
}

impl Decompress {
    /// Creates a new object ready for decompressing data that it's given.
    ///
    /// The `zlib_header` argument indicates whether the input data is expected
    /// to have a zlib header or not.
    pub fn new(zlib_header: bool) -> Decompress {
        Decompress {
            inner: Inflate::make(zlib_header, ffi::MZ_DEFAULT_WINDOW_BITS as u8),
        }
    }

    /// Creates a new object ready for decompressing data that it's given.
    ///
    /// The `zlib_header` argument indicates whether the input data is expected
    /// to have a zlib header or not. The `window_bits` parameter indicates the
    /// base-2 logarithm of the sliding window size and must be between 9 and 15.
    ///
    /// # Panics
    ///
    /// If `window_bits` does not fall into the range 9 ..= 15,
    /// `new_with_window_bits` will panic.
    #[cfg(feature = "any_zlib")]
    pub fn new_with_window_bits(zlib_header: bool, window_bits: u8) -> Decompress {
        assert!(
            window_bits > 8 && window_bits < 16,
            "window_bits must be within 9 ..= 15"
        );
        Decompress {
            inner: Inflate::make(zlib_header, window_bits),
        }
    }

    /// Creates a new object ready for decompressing data that it's given.
    ///
    /// The Decompress object produced by this constructor expects gzip headers
    /// for the compressed data.
    ///
    /// # Panics
    ///
    /// If `window_bits` does not fall into the range 9 ..= 15,
    /// `new_with_window_bits` will panic.
    #[cfg(feature = "any_zlib")]
    pub fn new_gzip(window_bits: u8) -> Decompress {
        assert!(
            window_bits > 8 && window_bits < 16,
            "window_bits must be within 9 ..= 15"
        );
        Decompress {
            inner: Inflate::make(true, window_bits + 16),
        }
    }

    /// Returns the total number of input bytes which have been processed by
    /// this decompression object.
    pub fn total_in(&self) -> u64 {
        self.inner.total_in()
    }

    /// Returns the total number of output bytes which have been produced by
    /// this decompression object.
    pub fn total_out(&self) -> u64 {
        self.inner.total_out()
    }

    /// Decompresses the input data into the output, consuming only as much
    /// input as needed and writing as much output as possible.
    ///
    /// The flush option can be any of the available `FlushDecompress` parameters.
    ///
    /// If the first call passes `FlushDecompress::Finish` it is assumed that
    /// the input and output buffers are both sized large enough to decompress
    /// the entire stream in a single call.
    ///
    /// A flush value of `FlushDecompress::Finish` indicates that there are no
    /// more source bytes available beside what's already in the input buffer,
    /// and the output buffer is large enough to hold the rest of the
    /// decompressed data.
    ///
    /// To learn how much data was consumed or how much output was produced, use
    /// the `total_in` and `total_out` functions before/after this is called.
    ///
    /// # Errors
    ///
    /// If the input data to this instance of `Decompress` is not a valid
    /// zlib/deflate stream then this function may return an instance of
    /// `DecompressError` to indicate that the stream of input bytes is corrupted.
    pub fn decompress(
        &mut self,
        input: &[u8],
        output: &mut [u8],
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        self.inner.decompress(input, output, flush)
    }

    /// Decompresses the input data into the extra space in the output vector
    /// specified by `output`.
    ///
    /// This function has the same semantics as `decompress`, except that the
    /// length of `vec` is managed by this function. This will not reallocate
    /// the vector provided or attempt to grow it, so space for the output must
    /// be reserved in the output vector by the caller before calling this
    /// function.
    ///
    /// # Errors
    ///
    /// If the input data to this instance of `Decompress` is not a valid
    /// zlib/deflate stream then this function may return an instance of
    /// `DecompressError` to indicate that the stream of input bytes is corrupted.
    pub fn decompress_vec(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
        flush: FlushDecompress,
    ) -> Result<Status, DecompressError> {
        write_to_spare_capacity_of_vec(output, |out| {
            let before = self.total_out();
            let ret = self.decompress(input, out, flush);
            let bytes_written = self.total_out() - before;
            (bytes_written as usize, ret)
        })
    }

    /// Specifies the decompression dictionary to use.
    #[cfg(feature = "any_zlib")]
    pub fn set_dictionary(&mut self, dictionary: &[u8]) -> Result<u32, DecompressError> {
        // SAFETY: The field `inner` must always be accessed as a raw pointer,
        // since it points to a cyclic structure. No copies of `inner` can be
        // retained for longer than the lifetime of `self.inner.inner.stream_wrapper`.
        let stream = self.inner.inner.stream_wrapper.inner;
        let rc = unsafe {
            (*stream).msg = std::ptr::null_mut();
            assert!(dictionary.len() < ffi::uInt::MAX as usize);
            ffi::inflateSetDictionary(stream, dictionary.as_ptr(), dictionary.len() as ffi::uInt)
        };

        #[allow(clippy::unnecessary_cast)]
        match rc {
            ffi::MZ_STREAM_ERROR => decompress_failed(self.inner.inner.msg()),
            ffi::MZ_DATA_ERROR => decompress_need_dict(unsafe { (*stream).adler } as u32),
            ffi::MZ_OK => Ok(unsafe { (*stream).adler } as u32),
            c => panic!("unknown return code: {}", c),
        }
    }

    /// Performs the equivalent of replacing this decompression state with a
    /// freshly allocated copy.
    ///
    /// This function may not allocate memory, though, and attempts to reuse any
    /// previously existing resources.
    ///
    /// The argument provided here indicates whether the reset state will
    /// attempt to decode a zlib header first or not.
    pub fn reset(&mut self, zlib_header: bool) {
        self.inner.reset(zlib_header);
    }
}

impl Error for DecompressError {}

impl DecompressError {
    /// Retrieve the implementation's message about why the operation failed, if one exists.
    pub fn message(&self) -> Option<&str> {
        match &self.0 {
            DecompressErrorInner::General { msg } => msg.get(),
            _ => None,
        }
    }
}

impl From<DecompressError> for io::Error {
    fn from(data: DecompressError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, data)
    }
}

impl fmt::Display for DecompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match &self.0 {
            DecompressErrorInner::General { msg } => msg.get(),
            DecompressErrorInner::NeedsDictionary { .. } => Some("requires a dictionary"),
        };
        match msg {
            Some(msg) => write!(f, "deflate decompression error: {}", msg),
            None => write!(f, "deflate decompression error"),
        }
    }
}

impl Error for CompressError {}

impl CompressError {
    /// Retrieve the implementation's message about why the operation failed, if one exists.
    pub fn message(&self) -> Option<&str> {
        self.msg.get()
    }
}

impl From<CompressError> for io::Error {
    fn from(data: CompressError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, data)
    }
}

impl fmt::Display for CompressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.msg.get() {
            Some(msg) => write!(f, "deflate compression error: {}", msg),
            None => write!(f, "deflate compression error"),
        }
    }
}

/// Allows `writer` to write data into the spare capacity of the `output` vector.
/// This will not reallocate the vector provided or attempt to grow it, so space
/// for the `output` must be reserved by the caller before calling this
/// function.
///
/// `writer` needs to return the number of bytes written (and can also return
/// another arbitrary return value).
fn write_to_spare_capacity_of_vec<T>(
    output: &mut Vec<u8>,
    writer: impl FnOnce(&mut [u8]) -> (usize, T),
) -> T {
    let cap = output.capacity();
    let len = output.len();

    output.resize(output.capacity(), 0);
    let (bytes_written, ret) = writer(&mut output[len..]);

    let new_len = core::cmp::min(len + bytes_written, cap); // Sanitizes `bytes_written`.
    output.resize(new_len, 0 /* unused */);

    ret
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use crate::write;
    use crate::{Compression, Decompress, FlushDecompress};

    #[cfg(feature = "any_zlib")]
    use crate::{Compress, FlushCompress};

    #[test]
    fn issue51() {
        let data = vec![
            0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xb3, 0xc9, 0x28, 0xc9,
            0xcd, 0xb1, 0xe3, 0xe5, 0xb2, 0xc9, 0x48, 0x4d, 0x4c, 0xb1, 0xb3, 0x29, 0xc9, 0x2c,
            0xc9, 0x49, 0xb5, 0x33, 0x31, 0x30, 0x51, 0xf0, 0xcb, 0x2f, 0x51, 0x70, 0xcb, 0x2f,
            0xcd, 0x4b, 0xb1, 0xd1, 0x87, 0x08, 0xda, 0xe8, 0x83, 0x95, 0x00, 0x95, 0x26, 0xe5,
            0xa7, 0x54, 0x2a, 0x24, 0xa5, 0x27, 0xe7, 0xe7, 0xe4, 0x17, 0xd9, 0x2a, 0x95, 0x67,
            0x64, 0x96, 0xa4, 0x2a, 0x81, 0x8c, 0x48, 0x4e, 0xcd, 0x2b, 0x49, 0x2d, 0xb2, 0xb3,
            0xc9, 0x30, 0x44, 0x37, 0x01, 0x28, 0x62, 0xa3, 0x0f, 0x95, 0x06, 0xd9, 0x05, 0x54,
            0x04, 0xe5, 0xe5, 0xa5, 0x67, 0xe6, 0x55, 0xe8, 0x1b, 0xea, 0x99, 0xe9, 0x19, 0x21,
            0xab, 0xd0, 0x07, 0xd9, 0x01, 0x32, 0x53, 0x1f, 0xea, 0x3e, 0x00, 0x94, 0x85, 0xeb,
            0xe4, 0xa8, 0x00, 0x00, 0x00,
        ];

        let mut decoded = Vec::with_capacity(data.len() * 2);

        let mut d = Decompress::new(false);
        // decompressed whole deflate stream
        assert!(d
            .decompress_vec(&data[10..], &mut decoded, FlushDecompress::Finish)
            .is_ok());

        // decompress data that has nothing to do with the deflate stream (this
        // used to panic)
        drop(d.decompress_vec(&[0], &mut decoded, FlushDecompress::None));
    }

    #[test]
    fn reset() {
        let string = "hello world".as_bytes();
        let mut zlib = Vec::new();
        let mut deflate = Vec::new();

        let comp = Compression::default();
        write::ZlibEncoder::new(&mut zlib, comp)
            .write_all(string)
            .unwrap();
        write::DeflateEncoder::new(&mut deflate, comp)
            .write_all(string)
            .unwrap();

        let mut dst = [0; 1024];
        let mut decoder = Decompress::new(true);
        decoder
            .decompress(&zlib, &mut dst, FlushDecompress::Finish)
            .unwrap();
        assert_eq!(decoder.total_out(), string.len() as u64);
        assert!(dst.starts_with(string));

        decoder.reset(false);
        decoder
            .decompress(&deflate, &mut dst, FlushDecompress::Finish)
            .unwrap();
        assert_eq!(decoder.total_out(), string.len() as u64);
        assert!(dst.starts_with(string));
    }

    #[cfg(feature = "any_zlib")]
    #[test]
    fn set_dictionary_with_zlib_header() {
        let string = "hello, hello!".as_bytes();
        let dictionary = "hello".as_bytes();

        let mut encoded = Vec::with_capacity(1024);

        let mut encoder = Compress::new(Compression::default(), true);

        let dictionary_adler = encoder.set_dictionary(&dictionary).unwrap();

        encoder
            .compress_vec(string, &mut encoded, FlushCompress::Finish)
            .unwrap();

        assert_eq!(encoder.total_in(), string.len() as u64);
        assert_eq!(encoder.total_out(), encoded.len() as u64);

        let mut decoder = Decompress::new(true);
        let mut decoded = [0; 1024];
        let decompress_error = decoder
            .decompress(&encoded, &mut decoded, FlushDecompress::Finish)
            .expect_err("decompression should fail due to requiring a dictionary");

        let required_adler = decompress_error.needs_dictionary()
            .expect("the first call to decompress should indicate a dictionary is required along with the required Adler-32 checksum");

        assert_eq!(required_adler, dictionary_adler,
            "the Adler-32 checksum should match the value when the dictionary was set on the compressor");

        let actual_adler = decoder.set_dictionary(&dictionary).unwrap();

        assert_eq!(required_adler, actual_adler);

        // Decompress the rest of the input to the remainder of the output buffer
        let total_in = decoder.total_in();
        let total_out = decoder.total_out();

        let decompress_result = decoder.decompress(
            &encoded[total_in as usize..],
            &mut decoded[total_out as usize..],
            FlushDecompress::Finish,
        );
        assert!(decompress_result.is_ok());

        assert_eq!(&decoded[..decoder.total_out() as usize], string);
    }

    #[cfg(feature = "any_zlib")]
    #[test]
    fn set_dictionary_raw() {
        let string = "hello, hello!".as_bytes();
        let dictionary = "hello".as_bytes();

        let mut encoded = Vec::with_capacity(1024);

        let mut encoder = Compress::new(Compression::default(), false);

        encoder.set_dictionary(&dictionary).unwrap();

        encoder
            .compress_vec(string, &mut encoded, FlushCompress::Finish)
            .unwrap();

        assert_eq!(encoder.total_in(), string.len() as u64);
        assert_eq!(encoder.total_out(), encoded.len() as u64);

        let mut decoder = Decompress::new(false);

        decoder.set_dictionary(&dictionary).unwrap();

        let mut decoded = [0; 1024];
        let decompress_result = decoder.decompress(&encoded, &mut decoded, FlushDecompress::Finish);

        assert!(decompress_result.is_ok());

        assert_eq!(&decoded[..decoder.total_out() as usize], string);
    }

    #[cfg(feature = "any_zlib")]
    #[test]
    fn test_gzip_flate() {
        let string = "hello, hello!".as_bytes();

        let mut encoded = Vec::with_capacity(1024);

        let mut encoder = Compress::new_gzip(Compression::default(), 9);

        encoder
            .compress_vec(string, &mut encoded, FlushCompress::Finish)
            .unwrap();

        assert_eq!(encoder.total_in(), string.len() as u64);
        assert_eq!(encoder.total_out(), encoded.len() as u64);

        let mut decoder = Decompress::new_gzip(9);

        let mut decoded = [0; 1024];
        decoder
            .decompress(&encoded, &mut decoded, FlushDecompress::Finish)
            .unwrap();

        assert_eq!(&decoded[..decoder.total_out() as usize], string);
    }

    #[cfg(feature = "any_zlib")]
    #[test]
    fn test_error_message() {
        let mut decoder = Decompress::new(false);
        let mut decoded = [0; 128];
        let garbage = b"xbvxzi";

        let err = decoder
            .decompress(&*garbage, &mut decoded, FlushDecompress::Finish)
            .unwrap_err();

        assert_eq!(err.message(), Some("invalid stored block lengths"));
    }
}
