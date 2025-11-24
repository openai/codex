use crate::{engine::Engine, DecodeError, DecodeSliceError, PAD_BYTE};
use std::{cmp, fmt, io};

// This should be large, but it has to fit on the stack.
pub(crate) const BUF_SIZE: usize = 1024;

// 4 bytes of base64 data encode 3 bytes of raw data (modulo padding).
const BASE64_CHUNK_SIZE: usize = 4;
const DECODED_CHUNK_SIZE: usize = 3;

/// A `Read` implementation that decodes base64 data read from an underlying reader.
///
/// # Examples
///
/// ```
/// use std::io::Read;
/// use std::io::Cursor;
/// use base64::engine::general_purpose;
///
/// // use a cursor as the simplest possible `Read` -- in real code this is probably a file, etc.
/// let mut wrapped_reader = Cursor::new(b"YXNkZg==");
/// let mut decoder = base64::read::DecoderReader::new(
///     &mut wrapped_reader,
///     &general_purpose::STANDARD);
///
/// // handle errors as you normally would
/// let mut result = Vec::new();
/// decoder.read_to_end(&mut result).unwrap();
///
/// assert_eq!(b"asdf", &result[..]);
///
/// ```
pub struct DecoderReader<'e, E: Engine, R: io::Read> {
    engine: &'e E,
    /// Where b64 data is read from
    inner: R,

    /// Holds b64 data read from the delegate reader.
    b64_buffer: [u8; BUF_SIZE],
    /// The start of the pending buffered data in `b64_buffer`.
    b64_offset: usize,
    /// The amount of buffered b64 data after `b64_offset` in `b64_len`.
    b64_len: usize,
    /// Since the caller may provide us with a buffer of size 1 or 2 that's too small to copy a
    /// decoded chunk in to, we have to be able to hang on to a few decoded bytes.
    /// Technically we only need to hold 2 bytes, but then we'd need a separate temporary buffer to
    /// decode 3 bytes into and then juggle copying one byte into the provided read buf and the rest
    /// into here, which seems like a lot of complexity for 1 extra byte of storage.
    decoded_chunk_buffer: [u8; DECODED_CHUNK_SIZE],
    /// Index of start of decoded data in `decoded_chunk_buffer`
    decoded_offset: usize,
    /// Length of decoded data after `decoded_offset` in `decoded_chunk_buffer`
    decoded_len: usize,
    /// Input length consumed so far.
    /// Used to provide accurate offsets in errors
    input_consumed_len: usize,
    /// offset of previously seen padding, if any
    padding_offset: Option<usize>,
}

// exclude b64_buffer as it's uselessly large
impl<'e, E: Engine, R: io::Read> fmt::Debug for DecoderReader<'e, E, R> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("DecoderReader")
            .field("b64_offset", &self.b64_offset)
            .field("b64_len", &self.b64_len)
            .field("decoded_chunk_buffer", &self.decoded_chunk_buffer)
            .field("decoded_offset", &self.decoded_offset)
            .field("decoded_len", &self.decoded_len)
            .field("input_consumed_len", &self.input_consumed_len)
            .field("padding_offset", &self.padding_offset)
            .finish()
    }
}

impl<'e, E: Engine, R: io::Read> DecoderReader<'e, E, R> {
    /// Create a new decoder that will read from the provided reader `r`.
    pub fn new(reader: R, engine: &'e E) -> Self {
        DecoderReader {
            engine,
            inner: reader,
            b64_buffer: [0; BUF_SIZE],
            b64_offset: 0,
            b64_len: 0,
            decoded_chunk_buffer: [0; DECODED_CHUNK_SIZE],
            decoded_offset: 0,
            decoded_len: 0,
            input_consumed_len: 0,
            padding_offset: None,
        }
    }

    /// Write as much as possible of the decoded buffer into the target buffer.
    /// Must only be called when there is something to write and space to write into.
    /// Returns a Result with the number of (decoded) bytes copied.
    fn flush_decoded_buf(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        debug_assert!(self.decoded_len > 0);
        debug_assert!(!buf.is_empty());

        let copy_len = cmp::min(self.decoded_len, buf.len());
        debug_assert!(copy_len > 0);
        debug_assert!(copy_len <= self.decoded_len);

        buf[..copy_len].copy_from_slice(
            &self.decoded_chunk_buffer[self.decoded_offset..self.decoded_offset + copy_len],
        );

        self.decoded_offset += copy_len;
        self.decoded_len -= copy_len;

        debug_assert!(self.decoded_len < DECODED_CHUNK_SIZE);

        Ok(copy_len)
    }

    /// Read into the remaining space in the buffer after the current contents.
    /// Must only be called when there is space to read into in the buffer.
    /// Returns the number of bytes read.
    fn read_from_delegate(&mut self) -> io::Result<usize> {
        debug_assert!(self.b64_offset + self.b64_len < BUF_SIZE);

        let read = self
            .inner
            .read(&mut self.b64_buffer[self.b64_offset + self.b64_len..])?;
        self.b64_len += read;

        debug_assert!(self.b64_offset + self.b64_len <= BUF_SIZE);

        Ok(read)
    }

    /// Decode the requested number of bytes from the b64 buffer into the provided buffer. It's the
    /// caller's responsibility to choose the number of b64 bytes to decode correctly.
    ///
    /// Returns a Result with the number of decoded bytes written to `buf`.
    ///
    /// # Panics
    ///
    /// panics if `buf` is too small
    fn decode_to_buf(&mut self, b64_len_to_decode: usize, buf: &mut [u8]) -> io::Result<usize> {
        debug_assert!(self.b64_len >= b64_len_to_decode);
        debug_assert!(self.b64_offset + self.b64_len <= BUF_SIZE);
        debug_assert!(!buf.is_empty());

        let b64_to_decode = &self.b64_buffer[self.b64_offset..self.b64_offset + b64_len_to_decode];
        let decode_metadata = self
            .engine
            .internal_decode(
                b64_to_decode,
                buf,
                self.engine.internal_decoded_len_estimate(b64_len_to_decode),
            )
            .map_err(|dse| match dse {
                DecodeSliceError::DecodeError(de) => {
                    match de {
                        DecodeError::InvalidByte(offset, byte) => {
                            match (byte, self.padding_offset) {
                                // if there was padding in a previous block of decoding that happened to
                                // be correct, and we now find more padding that happens to be incorrect,
                                // to be consistent with non-reader decodes, record the error at the first
                                // padding
                                (PAD_BYTE, Some(first_pad_offset)) => {
                                    DecodeError::InvalidByte(first_pad_offset, PAD_BYTE)
                                }
                                _ => {
                                    DecodeError::InvalidByte(self.input_consumed_len + offset, byte)
                                }
                            }
                        }
                        DecodeError::InvalidLength(len) => {
                            DecodeError::InvalidLength(self.input_consumed_len + len)
                        }
                        DecodeError::InvalidLastSymbol(offset, byte) => {
                            DecodeError::InvalidLastSymbol(self.input_consumed_len + offset, byte)
                        }
                        DecodeError::InvalidPadding => DecodeError::InvalidPadding,
                    }
                }
                DecodeSliceError::OutputSliceTooSmall => {
                    unreachable!("buf is sized correctly in calling code")
                }
            })
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if let Some(offset) = self.padding_offset {
            // we've already seen padding
            if decode_metadata.decoded_len > 0 {
                // we read more after already finding padding; report error at first padding byte
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    DecodeError::InvalidByte(offset, PAD_BYTE),
                ));
            }
        }

        self.padding_offset = self.padding_offset.or(decode_metadata
            .padding_offset
            .map(|offset| self.input_consumed_len + offset));
        self.input_consumed_len += b64_len_to_decode;
        self.b64_offset += b64_len_to_decode;
        self.b64_len -= b64_len_to_decode;

        debug_assert!(self.b64_offset + self.b64_len <= BUF_SIZE);

        Ok(decode_metadata.decoded_len)
    }

    /// Unwraps this `DecoderReader`, returning the base reader which it reads base64 encoded
    /// input from.
    ///
    /// Because `DecoderReader` performs internal buffering, the state of the inner reader is
    /// unspecified. This function is mainly provided because the inner reader type may provide
    /// additional functionality beyond the `Read` implementation which may still be useful.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<'e, E: Engine, R: io::Read> io::Read for DecoderReader<'e, E, R> {
    /// Decode input from the wrapped reader.
    ///
    /// Under non-error circumstances, this returns `Ok` with the value being the number of bytes
    /// written in `buf`.
    ///
    /// Where possible, this function buffers base64 to minimize the number of read() calls to the
    /// delegate reader.
    ///
    /// # Errors
    ///
    /// Any errors emitted by the delegate reader are returned. Decoding errors due to invalid
    /// base64 are also possible, and will have `io::ErrorKind::InvalidData`.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // offset == BUF_SIZE when we copied it all last time
        debug_assert!(self.b64_offset <= BUF_SIZE);
        debug_assert!(self.b64_offset + self.b64_len <= BUF_SIZE);
        debug_assert!(if self.b64_offset == BUF_SIZE {
            self.b64_len == 0
        } else {
            self.b64_len <= BUF_SIZE
        });

        debug_assert!(if self.decoded_len == 0 {
            // can be = when we were able to copy the complete chunk
            self.decoded_offset <= DECODED_CHUNK_SIZE
        } else {
            self.decoded_offset < DECODED_CHUNK_SIZE
        });

        // We shouldn't ever decode into decoded_buffer when we can't immediately write at least one
        // byte into the provided buf, so the effective length should only be 3 momentarily between
        // when we decode and when we copy into the target buffer.
        debug_assert!(self.decoded_len < DECODED_CHUNK_SIZE);
        debug_assert!(self.decoded_len + self.decoded_offset <= DECODED_CHUNK_SIZE);

        if self.decoded_len > 0 {
            // we have a few leftover decoded bytes; flush that rather than pull in more b64
            self.flush_decoded_buf(buf)
        } else {
            let mut at_eof = false;
            while self.b64_len < BASE64_CHUNK_SIZE {
                // Copy any bytes we have to the start of the buffer.
                self.b64_buffer
                    .copy_within(self.b64_offset..self.b64_offset + self.b64_len, 0);
                self.b64_offset = 0;

                // then fill in more data
                let read = self.read_from_delegate()?;
                if read == 0 {
                    // we never read into an empty buf, so 0 => we've hit EOF
                    at_eof = true;
                    break;
                }
            }

            if self.b64_len == 0 {
                debug_assert!(at_eof);
                // we must be at EOF, and we have no data left to decode
                return Ok(0);
            };

            debug_assert!(if at_eof {
                // if we are at eof, we may not have a complete chunk
                self.b64_len > 0
            } else {
                // otherwise, we must have at least one chunk
                self.b64_len >= BASE64_CHUNK_SIZE
            });

            debug_assert_eq!(0, self.decoded_len);

            if buf.len() < DECODED_CHUNK_SIZE {
                // caller requested an annoyingly short read
                // have to write to a tmp buf first to avoid double mutable borrow
                let mut decoded_chunk = [0_u8; DECODED_CHUNK_SIZE];
                // if we are at eof, could have less than BASE64_CHUNK_SIZE, in which case we have
                // to assume that these last few tokens are, in fact, valid (i.e. must be 2-4 b64
                // tokens, not 1, since 1 token can't decode to 1 byte).
                let to_decode = cmp::min(self.b64_len, BASE64_CHUNK_SIZE);

                let decoded = self.decode_to_buf(to_decode, &mut decoded_chunk[..])?;
                self.decoded_chunk_buffer[..decoded].copy_from_slice(&decoded_chunk[..decoded]);

                self.decoded_offset = 0;
                self.decoded_len = decoded;

                // can be less than 3 on last block due to padding
                debug_assert!(decoded <= 3);

                self.flush_decoded_buf(buf)
            } else {
                let b64_bytes_that_can_decode_into_buf = (buf.len() / DECODED_CHUNK_SIZE)
                    .checked_mul(BASE64_CHUNK_SIZE)
                    .expect("too many chunks");
                debug_assert!(b64_bytes_that_can_decode_into_buf >= BASE64_CHUNK_SIZE);

                let b64_bytes_available_to_decode = if at_eof {
                    self.b64_len
                } else {
                    // only use complete chunks
                    self.b64_len - self.b64_len % 4
                };

                let actual_decode_len = cmp::min(
                    b64_bytes_that_can_decode_into_buf,
                    b64_bytes_available_to_decode,
                );
                self.decode_to_buf(actual_decode_len, buf)
            }
        }
    }
}
