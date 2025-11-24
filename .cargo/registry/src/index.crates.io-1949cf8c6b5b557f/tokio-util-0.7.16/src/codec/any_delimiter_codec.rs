use crate::codec::decoder::Decoder;
use crate::codec::encoder::Encoder;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{cmp, fmt, io, str};

const DEFAULT_SEEK_DELIMITERS: &[u8] = b",;\n\r";
const DEFAULT_SEQUENCE_WRITER: &[u8] = b",";
/// A simple [`Decoder`] and [`Encoder`] implementation that splits up data into chunks based on any character in the given delimiter string.
///
/// [`Decoder`]: crate::codec::Decoder
/// [`Encoder`]: crate::codec::Encoder
///
/// # Example
/// Decode string of bytes containing various different delimiters.
///
/// [`BytesMut`]: bytes::BytesMut
/// [`Error`]: std::io::Error
///
/// ```
/// use tokio_util::codec::{AnyDelimiterCodec, Decoder};
/// use bytes::{BufMut, BytesMut};
///
/// #
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), std::io::Error> {
/// let mut codec = AnyDelimiterCodec::new(b",;\r\n".to_vec(),b";".to_vec());
/// let buf = &mut BytesMut::new();
/// buf.reserve(200);
/// buf.put_slice(b"chunk 1,chunk 2;chunk 3\n\r");
/// assert_eq!("chunk 1", codec.decode(buf).unwrap().unwrap());
/// assert_eq!("chunk 2", codec.decode(buf).unwrap().unwrap());
/// assert_eq!("chunk 3", codec.decode(buf).unwrap().unwrap());
/// assert_eq!("", codec.decode(buf).unwrap().unwrap());
/// assert_eq!(None, codec.decode(buf).unwrap());
/// # Ok(())
/// # }
/// ```
///
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AnyDelimiterCodec {
    // Stored index of the next index to examine for the delimiter character.
    // This is used to optimize searching.
    // For example, if `decode` was called with `abc` and the delimiter is '{}', it would hold `3`,
    // because that is the next index to examine.
    // The next time `decode` is called with `abcde}`, the method will
    // only look at `de}` before returning.
    next_index: usize,

    /// The maximum length for a given chunk. If `usize::MAX`, chunks will be
    /// read until a delimiter character is reached.
    max_length: usize,

    /// Are we currently discarding the remainder of a chunk which was over
    /// the length limit?
    is_discarding: bool,

    /// The bytes that are using for search during decode
    seek_delimiters: Vec<u8>,

    /// The bytes that are using for encoding
    sequence_writer: Vec<u8>,
}

impl AnyDelimiterCodec {
    /// Returns a `AnyDelimiterCodec` for splitting up data into chunks.
    ///
    /// # Note
    ///
    /// The returned `AnyDelimiterCodec` will not have an upper bound on the length
    /// of a buffered chunk. See the documentation for [`new_with_max_length`]
    /// for information on why this could be a potential security risk.
    ///
    /// [`new_with_max_length`]: crate::codec::AnyDelimiterCodec::new_with_max_length()
    pub fn new(seek_delimiters: Vec<u8>, sequence_writer: Vec<u8>) -> AnyDelimiterCodec {
        AnyDelimiterCodec {
            next_index: 0,
            max_length: usize::MAX,
            is_discarding: false,
            seek_delimiters,
            sequence_writer,
        }
    }

    /// Returns a `AnyDelimiterCodec` with a maximum chunk length limit.
    ///
    /// If this is set, calls to `AnyDelimiterCodec::decode` will return a
    /// [`AnyDelimiterCodecError`] when a chunk exceeds the length limit. Subsequent calls
    /// will discard up to `limit` bytes from that chunk until a delimiter
    /// character is reached, returning `None` until the delimiter over the limit
    /// has been fully discarded. After that point, calls to `decode` will
    /// function as normal.
    ///
    /// # Note
    ///
    /// Setting a length limit is highly recommended for any `AnyDelimiterCodec` which
    /// will be exposed to untrusted input. Otherwise, the size of the buffer
    /// that holds the chunk currently being read is unbounded. An attacker could
    /// exploit this unbounded buffer by sending an unbounded amount of input
    /// without any delimiter characters, causing unbounded memory consumption.
    ///
    /// [`AnyDelimiterCodecError`]: crate::codec::AnyDelimiterCodecError
    pub fn new_with_max_length(
        seek_delimiters: Vec<u8>,
        sequence_writer: Vec<u8>,
        max_length: usize,
    ) -> Self {
        AnyDelimiterCodec {
            max_length,
            ..AnyDelimiterCodec::new(seek_delimiters, sequence_writer)
        }
    }

    /// Returns the maximum chunk length when decoding.
    ///
    /// ```
    /// use std::usize;
    /// use tokio_util::codec::AnyDelimiterCodec;
    ///
    /// let codec = AnyDelimiterCodec::new(b",;\n".to_vec(), b";".to_vec());
    /// assert_eq!(codec.max_length(), usize::MAX);
    /// ```
    /// ```
    /// use tokio_util::codec::AnyDelimiterCodec;
    ///
    /// let codec = AnyDelimiterCodec::new_with_max_length(b",;\n".to_vec(), b";".to_vec(), 256);
    /// assert_eq!(codec.max_length(), 256);
    /// ```
    pub fn max_length(&self) -> usize {
        self.max_length
    }
}

impl Decoder for AnyDelimiterCodec {
    type Item = Bytes;
    type Error = AnyDelimiterCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, AnyDelimiterCodecError> {
        loop {
            // Determine how far into the buffer we'll search for a delimiter. If
            // there's no max_length set, we'll read to the end of the buffer.
            let read_to = cmp::min(self.max_length.saturating_add(1), buf.len());

            let new_chunk_offset = buf[self.next_index..read_to]
                .iter()
                .position(|b| self.seek_delimiters.contains(b));

            match (self.is_discarding, new_chunk_offset) {
                (true, Some(offset)) => {
                    // If we found a new chunk, discard up to that offset and
                    // then stop discarding. On the next iteration, we'll try
                    // to read a chunk normally.
                    buf.advance(offset + self.next_index + 1);
                    self.is_discarding = false;
                    self.next_index = 0;
                }
                (true, None) => {
                    // Otherwise, we didn't find a new chunk, so we'll discard
                    // everything we read. On the next iteration, we'll continue
                    // discarding up to max_len bytes unless we find a new chunk.
                    buf.advance(read_to);
                    self.next_index = 0;
                    if buf.is_empty() {
                        return Ok(None);
                    }
                }
                (false, Some(offset)) => {
                    // Found a chunk!
                    let new_chunk_index = offset + self.next_index;
                    self.next_index = 0;
                    let mut chunk = buf.split_to(new_chunk_index + 1);
                    chunk.truncate(chunk.len() - 1);
                    let chunk = chunk.freeze();
                    return Ok(Some(chunk));
                }
                (false, None) if buf.len() > self.max_length => {
                    // Reached the maximum length without finding a
                    // new chunk, return an error and start discarding on the
                    // next call.
                    self.is_discarding = true;
                    return Err(AnyDelimiterCodecError::MaxChunkLengthExceeded);
                }
                (false, None) => {
                    // We didn't find a chunk or reach the length limit, so the next
                    // call will resume searching at the current offset.
                    self.next_index = read_to;
                    return Ok(None);
                }
            }
        }
    }

    fn decode_eof(&mut self, buf: &mut BytesMut) -> Result<Option<Bytes>, AnyDelimiterCodecError> {
        Ok(match self.decode(buf)? {
            Some(frame) => Some(frame),
            None => {
                // return remaining data, if any
                if buf.is_empty() {
                    None
                } else {
                    let chunk = buf.split_to(buf.len());
                    self.next_index = 0;
                    Some(chunk.freeze())
                }
            }
        })
    }
}

impl<T> Encoder<T> for AnyDelimiterCodec
where
    T: AsRef<str>,
{
    type Error = AnyDelimiterCodecError;

    fn encode(&mut self, chunk: T, buf: &mut BytesMut) -> Result<(), AnyDelimiterCodecError> {
        let chunk = chunk.as_ref();
        buf.reserve(chunk.len() + self.sequence_writer.len());
        buf.put(chunk.as_bytes());
        buf.put(self.sequence_writer.as_ref());

        Ok(())
    }
}

impl Default for AnyDelimiterCodec {
    fn default() -> Self {
        Self::new(
            DEFAULT_SEEK_DELIMITERS.to_vec(),
            DEFAULT_SEQUENCE_WRITER.to_vec(),
        )
    }
}

/// An error occurred while encoding or decoding a chunk.
#[derive(Debug)]
pub enum AnyDelimiterCodecError {
    /// The maximum chunk length was exceeded.
    MaxChunkLengthExceeded,
    /// An IO error occurred.
    Io(io::Error),
}

impl fmt::Display for AnyDelimiterCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnyDelimiterCodecError::MaxChunkLengthExceeded => {
                write!(f, "max chunk length exceeded")
            }
            AnyDelimiterCodecError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for AnyDelimiterCodecError {
    fn from(e: io::Error) -> AnyDelimiterCodecError {
        AnyDelimiterCodecError::Io(e)
    }
}

impl std::error::Error for AnyDelimiterCodecError {}
