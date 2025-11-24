use crate::engine::Engine;
use std::{
    cmp, fmt, io,
    io::{ErrorKind, Result},
};

pub(crate) const BUF_SIZE: usize = 1024;
/// The most bytes whose encoding will fit in `BUF_SIZE`
const MAX_INPUT_LEN: usize = BUF_SIZE / 4 * 3;
// 3 bytes of input = 4 bytes of base64, always (because we don't allow line wrapping)
const MIN_ENCODE_CHUNK_SIZE: usize = 3;

/// A `Write` implementation that base64 encodes data before delegating to the wrapped writer.
///
/// Because base64 has special handling for the end of the input data (padding, etc), there's a
/// `finish()` method on this type that encodes any leftover input bytes and adds padding if
/// appropriate. It's called automatically when deallocated (see the `Drop` implementation), but
/// any error that occurs when invoking the underlying writer will be suppressed. If you want to
/// handle such errors, call `finish()` yourself.
///
/// # Examples
///
/// ```
/// use std::io::Write;
/// use base64::engine::general_purpose;
///
/// // use a vec as the simplest possible `Write` -- in real code this is probably a file, etc.
/// let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);
///
/// // handle errors as you normally would
/// enc.write_all(b"asdf").unwrap();
///
/// // could leave this out to be called by Drop, if you don't care
/// // about handling errors or getting the delegate writer back
/// let delegate = enc.finish().unwrap();
///
/// // base64 was written to the writer
/// assert_eq!(b"YXNkZg==", &delegate[..]);
///
/// ```
///
/// # Panics
///
/// Calling `write()` (or related methods) or `finish()` after `finish()` has completed without
/// error is invalid and will panic.
///
/// # Errors
///
/// Base64 encoding itself does not generate errors, but errors from the wrapped writer will be
/// returned as per the contract of `Write`.
///
/// # Performance
///
/// It has some minor performance loss compared to encoding slices (a couple percent).
/// It does not do any heap allocation.
///
/// # Limitations
///
/// Owing to the specification of the `write` and `flush` methods on the `Write` trait and their
/// implications for a buffering implementation, these methods may not behave as expected. In
/// particular, calling `write_all` on this interface may fail with `io::ErrorKind::WriteZero`.
/// See the documentation of the `Write` trait implementation for further details.
pub struct EncoderWriter<'e, E: Engine, W: io::Write> {
    engine: &'e E,
    /// Where encoded data is written to. It's an Option as it's None immediately before Drop is
    /// called so that finish() can return the underlying writer. None implies that finish() has
    /// been called successfully.
    delegate: Option<W>,
    /// Holds a partial chunk, if any, after the last `write()`, so that we may then fill the chunk
    /// with the next `write()`, encode it, then proceed with the rest of the input normally.
    extra_input: [u8; MIN_ENCODE_CHUNK_SIZE],
    /// How much of `extra` is occupied, in `[0, MIN_ENCODE_CHUNK_SIZE]`.
    extra_input_occupied_len: usize,
    /// Buffer to encode into. May hold leftover encoded bytes from a previous write call that the underlying writer
    /// did not write last time.
    output: [u8; BUF_SIZE],
    /// How much of `output` is occupied with encoded data that couldn't be written last time
    output_occupied_len: usize,
    /// panic safety: don't write again in destructor if writer panicked while we were writing to it
    panicked: bool,
}

impl<'e, E: Engine, W: io::Write> fmt::Debug for EncoderWriter<'e, E, W> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "extra_input: {:?} extra_input_occupied_len:{:?} output[..5]: {:?} output_occupied_len: {:?}",
            self.extra_input,
            self.extra_input_occupied_len,
            &self.output[0..5],
            self.output_occupied_len
        )
    }
}

impl<'e, E: Engine, W: io::Write> EncoderWriter<'e, E, W> {
    /// Create a new encoder that will write to the provided delegate writer.
    pub fn new(delegate: W, engine: &'e E) -> EncoderWriter<'e, E, W> {
        EncoderWriter {
            engine,
            delegate: Some(delegate),
            extra_input: [0u8; MIN_ENCODE_CHUNK_SIZE],
            extra_input_occupied_len: 0,
            output: [0u8; BUF_SIZE],
            output_occupied_len: 0,
            panicked: false,
        }
    }

    /// Encode all remaining buffered data and write it, including any trailing incomplete input
    /// triples and associated padding.
    ///
    /// Once this succeeds, no further writes or calls to this method are allowed.
    ///
    /// This may write to the delegate writer multiple times if the delegate writer does not accept
    /// all input provided to its `write` each invocation.
    ///
    /// If you don't care about error handling, it is not necessary to call this function, as the
    /// equivalent finalization is done by the Drop impl.
    ///
    /// Returns the writer that this was constructed around.
    ///
    /// # Errors
    ///
    /// The first error that is not of `ErrorKind::Interrupted` will be returned.
    pub fn finish(&mut self) -> Result<W> {
        // If we could consume self in finish(), we wouldn't have to worry about this case, but
        // finish() is retryable in the face of I/O errors, so we can't consume here.
        if self.delegate.is_none() {
            panic!("Encoder has already had finish() called");
        };

        self.write_final_leftovers()?;

        let writer = self.delegate.take().expect("Writer must be present");

        Ok(writer)
    }

    /// Write any remaining buffered data to the delegate writer.
    fn write_final_leftovers(&mut self) -> Result<()> {
        if self.delegate.is_none() {
            // finish() has already successfully called this, and we are now in drop() with a None
            // writer, so just no-op
            return Ok(());
        }

        self.write_all_encoded_output()?;

        if self.extra_input_occupied_len > 0 {
            let encoded_len = self
                .engine
                .encode_slice(
                    &self.extra_input[..self.extra_input_occupied_len],
                    &mut self.output[..],
                )
                .expect("buffer is large enough");

            self.output_occupied_len = encoded_len;

            self.write_all_encoded_output()?;

            // write succeeded, do not write the encoding of extra again if finish() is retried
            self.extra_input_occupied_len = 0;
        }

        Ok(())
    }

    /// Write as much of the encoded output to the delegate writer as it will accept, and store the
    /// leftovers to be attempted at the next write() call. Updates `self.output_occupied_len`.
    ///
    /// # Errors
    ///
    /// Errors from the delegate writer are returned. In the case of an error,
    /// `self.output_occupied_len` will not be updated, as errors from `write` are specified to mean
    /// that no write took place.
    fn write_to_delegate(&mut self, current_output_len: usize) -> Result<()> {
        self.panicked = true;
        let res = self
            .delegate
            .as_mut()
            .expect("Writer must be present")
            .write(&self.output[..current_output_len]);
        self.panicked = false;

        res.map(|consumed| {
            debug_assert!(consumed <= current_output_len);

            if consumed < current_output_len {
                self.output_occupied_len = current_output_len.checked_sub(consumed).unwrap();
                // If we're blocking on I/O, the minor inefficiency of copying bytes to the
                // start of the buffer is the least of our concerns...
                // TODO Rotate moves more than we need to; copy_within now stable.
                self.output.rotate_left(consumed);
            } else {
                self.output_occupied_len = 0;
            }
        })
    }

    /// Write all buffered encoded output. If this returns `Ok`, `self.output_occupied_len` is `0`.
    ///
    /// This is basically write_all for the remaining buffered data but without the undesirable
    /// abort-on-`Ok(0)` behavior.
    ///
    /// # Errors
    ///
    /// Any error emitted by the delegate writer abort the write loop and is returned, unless it's
    /// `Interrupted`, in which case the error is ignored and writes will continue.
    fn write_all_encoded_output(&mut self) -> Result<()> {
        while self.output_occupied_len > 0 {
            let remaining_len = self.output_occupied_len;
            match self.write_to_delegate(remaining_len) {
                // try again on interrupts ala write_all
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                // other errors return
                Err(e) => return Err(e),
                // success no-ops because remaining length is already updated
                Ok(_) => {}
            };
        }

        debug_assert_eq!(0, self.output_occupied_len);
        Ok(())
    }

    /// Unwraps this `EncoderWriter`, returning the base writer it writes base64 encoded output
    /// to.
    ///
    /// Normally this method should not be needed, since `finish()` returns the inner writer if
    /// it completes successfully. That will also ensure all data has been flushed, which the
    /// `into_inner()` function does *not* do.
    ///
    /// Calling this method after `finish()` has completed successfully will panic, since the
    /// writer has already been returned.
    ///
    /// This method may be useful if the writer implements additional APIs beyond the `Write`
    /// trait. Note that the inner writer might be in an error state or have an incomplete
    /// base64 string written to it.
    pub fn into_inner(mut self) -> W {
        self.delegate
            .take()
            .expect("Encoder has already had finish() called")
    }
}

impl<'e, E: Engine, W: io::Write> io::Write for EncoderWriter<'e, E, W> {
    /// Encode input and then write to the delegate writer.
    ///
    /// Under non-error circumstances, this returns `Ok` with the value being the number of bytes
    /// of `input` consumed. The value may be `0`, which interacts poorly with `write_all`, which
    /// interprets `Ok(0)` as an error, despite it being allowed by the contract of `write`. See
    /// <https://github.com/rust-lang/rust/issues/56889> for more on that.
    ///
    /// If the previous call to `write` provided more (encoded) data than the delegate writer could
    /// accept in a single call to its `write`, the remaining data is buffered. As long as buffered
    /// data is present, subsequent calls to `write` will try to write the remaining buffered data
    /// to the delegate and return either `Ok(0)` -- and therefore not consume any of `input` -- or
    /// an error.
    ///
    /// # Errors
    ///
    /// Any errors emitted by the delegate writer are returned.
    fn write(&mut self, input: &[u8]) -> Result<usize> {
        if self.delegate.is_none() {
            panic!("Cannot write more after calling finish()");
        }

        if input.is_empty() {
            return Ok(0);
        }

        // The contract of `Write::write` places some constraints on this implementation:
        // - a call to `write()` represents at most one call to a wrapped `Write`, so we can't
        // iterate over the input and encode multiple chunks.
        // - Errors mean that "no bytes were written to this writer", so we need to reset the
        // internal state to what it was before the error occurred

        // before reading any input, write any leftover encoded output from last time
        if self.output_occupied_len > 0 {
            let current_len = self.output_occupied_len;
            return self
                .write_to_delegate(current_len)
                // did not read any input
                .map(|_| 0);
        }

        debug_assert_eq!(0, self.output_occupied_len);

        // how many bytes, if any, were read into `extra` to create a triple to encode
        let mut extra_input_read_len = 0;
        let mut input = input;

        let orig_extra_len = self.extra_input_occupied_len;

        let mut encoded_size = 0;
        // always a multiple of MIN_ENCODE_CHUNK_SIZE
        let mut max_input_len = MAX_INPUT_LEN;

        // process leftover un-encoded input from last write
        if self.extra_input_occupied_len > 0 {
            debug_assert!(self.extra_input_occupied_len < 3);
            if input.len() + self.extra_input_occupied_len >= MIN_ENCODE_CHUNK_SIZE {
                // Fill up `extra`, encode that into `output`, and consume as much of the rest of
                // `input` as possible.
                // We could write just the encoding of `extra` by itself but then we'd have to
                // return after writing only 4 bytes, which is inefficient if the underlying writer
                // would make a syscall.
                extra_input_read_len = MIN_ENCODE_CHUNK_SIZE - self.extra_input_occupied_len;
                debug_assert!(extra_input_read_len > 0);
                // overwrite only bytes that weren't already used. If we need to rollback extra_len
                // (when the subsequent write errors), the old leading bytes will still be there.
                self.extra_input[self.extra_input_occupied_len..MIN_ENCODE_CHUNK_SIZE]
                    .copy_from_slice(&input[0..extra_input_read_len]);

                let len = self.engine.internal_encode(
                    &self.extra_input[0..MIN_ENCODE_CHUNK_SIZE],
                    &mut self.output[..],
                );
                debug_assert_eq!(4, len);

                input = &input[extra_input_read_len..];

                // consider extra to be used up, since we encoded it
                self.extra_input_occupied_len = 0;
                // don't clobber where we just encoded to
                encoded_size = 4;
                // and don't read more than can be encoded
                max_input_len = MAX_INPUT_LEN - MIN_ENCODE_CHUNK_SIZE;

            // fall through to normal encoding
            } else {
                // `extra` and `input` are non empty, but `|extra| + |input| < 3`, so there must be
                // 1 byte in each.
                debug_assert_eq!(1, input.len());
                debug_assert_eq!(1, self.extra_input_occupied_len);

                self.extra_input[self.extra_input_occupied_len] = input[0];
                self.extra_input_occupied_len += 1;
                return Ok(1);
            };
        } else if input.len() < MIN_ENCODE_CHUNK_SIZE {
            // `extra` is empty, and `input` fits inside it
            self.extra_input[0..input.len()].copy_from_slice(input);
            self.extra_input_occupied_len = input.len();
            return Ok(input.len());
        };

        // either 0 or 1 complete chunks encoded from extra
        debug_assert!(encoded_size == 0 || encoded_size == 4);
        debug_assert!(
            // didn't encode extra input
            MAX_INPUT_LEN == max_input_len
                // encoded one triple
                || MAX_INPUT_LEN == max_input_len + MIN_ENCODE_CHUNK_SIZE
        );

        // encode complete triples only
        let input_complete_chunks_len = input.len() - (input.len() % MIN_ENCODE_CHUNK_SIZE);
        let input_chunks_to_encode_len = cmp::min(input_complete_chunks_len, max_input_len);
        debug_assert_eq!(0, max_input_len % MIN_ENCODE_CHUNK_SIZE);
        debug_assert_eq!(0, input_chunks_to_encode_len % MIN_ENCODE_CHUNK_SIZE);

        encoded_size += self.engine.internal_encode(
            &input[..(input_chunks_to_encode_len)],
            &mut self.output[encoded_size..],
        );

        // not updating `self.output_occupied_len` here because if the below write fails, it should
        // "never take place" -- the buffer contents we encoded are ignored and perhaps retried
        // later, if the consumer chooses.

        self.write_to_delegate(encoded_size)
            // no matter whether we wrote the full encoded buffer or not, we consumed the same
            // input
            .map(|_| extra_input_read_len + input_chunks_to_encode_len)
            .map_err(|e| {
                // in case we filled and encoded `extra`, reset extra_len
                self.extra_input_occupied_len = orig_extra_len;

                e
            })
    }

    /// Because this is usually treated as OK to call multiple times, it will *not* flush any
    /// incomplete chunks of input or write padding.
    /// # Errors
    ///
    /// The first error that is not of [`ErrorKind::Interrupted`] will be returned.
    fn flush(&mut self) -> Result<()> {
        self.write_all_encoded_output()?;
        self.delegate
            .as_mut()
            .expect("Writer must be present")
            .flush()
    }
}

impl<'e, E: Engine, W: io::Write> Drop for EncoderWriter<'e, E, W> {
    fn drop(&mut self) {
        if !self.panicked {
            // like `BufWriter`, ignore errors during drop
            let _ = self.write_final_leftovers();
        }
    }
}
