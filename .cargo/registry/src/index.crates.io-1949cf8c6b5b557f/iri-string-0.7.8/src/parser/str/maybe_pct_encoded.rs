//! Processor for possibly- or invalidly-percent-encoded strings.

use core::fmt::{self, Write as _};
use core::marker::PhantomData;
use core::num::NonZeroU8;
use core::ops::ControlFlow;

use crate::parser::str::find_split;
use crate::parser::trusted::hexdigits_to_byte;

/// Fragment in a possibly percent-encoded (and possibly broken) string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PctEncodedFragments<'a> {
    /// String fragment without percent-encoded triplets.
    NoPctStr(&'a str),
    /// Stray `%` (percent) character.
    StrayPercent,
    /// Valid percent-encoded triplets for a character.
    Char(&'a str, char),
    /// Percent-encoded triplets that does not consists of a valid UTF-8 sequence.
    InvalidUtf8PctTriplets(&'a str),
}

/// Processes characters in a string which may contain (possibly invalid) percent-encoded triplets.
pub(crate) fn process_percent_encoded_best_effort<T, F, B>(
    v: T,
    mut f: F,
) -> Result<ControlFlow<B>, fmt::Error>
where
    T: fmt::Display,
    F: FnMut(PctEncodedFragments<'_>) -> ControlFlow<B>,
{
    let mut buf = [0_u8; 12];
    let mut writer = DecomposeWriter {
        f: &mut f,
        decoder: Default::default(),
        buf: &mut buf,
        result: ControlFlow::Continue(()),
        _r: PhantomData,
    };

    if write!(writer, "{v}").is_err() {
        match writer.result {
            ControlFlow::Continue(_) => return Err(fmt::Error),
            ControlFlow::Break(v) => return Ok(ControlFlow::Break(v)),
        }
    }

    // Flush the internal buffer of the decoder.
    if let Some(len) = writer.decoder.flush(&mut buf).map(|v| usize::from(v.get())) {
        let len_suffix = len % 3;
        let triplets_end = len - len_suffix;
        let triplets = core::str::from_utf8(&buf[..triplets_end])
            .expect("[validity] percent-encoded triplets consist of ASCII characters");
        if let ControlFlow::Break(v) = f(PctEncodedFragments::InvalidUtf8PctTriplets(triplets)) {
            return Ok(ControlFlow::Break(v));
        }

        if len_suffix > 0 {
            if let ControlFlow::Break(v) = f(PctEncodedFragments::StrayPercent) {
                return Ok(ControlFlow::Break(v));
            }
        }
        if len_suffix > 1 {
            let after_percent = core::str::from_utf8(
                &buf[(triplets_end + 1)..(triplets_end + len_suffix)],
            )
            .expect("[consistency] percent-encoded triplets contains only ASCII characters");
            if let ControlFlow::Break(v) = f(PctEncodedFragments::NoPctStr(after_percent)) {
                return Ok(ControlFlow::Break(v));
            }
        }
    }

    Ok(ControlFlow::Continue(()))
}

/// Writer to decompose the input into fragments.
struct DecomposeWriter<'a, F, B> {
    /// Output function.
    f: &'a mut F,
    /// Decoder.
    decoder: DecoderBuffer,
    /// Buffer.
    buf: &'a mut [u8],
    /// Result of the last output function call.
    result: ControlFlow<B>,
    /// Dummy field for the type parameter of the return type of the function `f`.
    _r: PhantomData<fn() -> B>,
}
impl<F, B> DecomposeWriter<'_, F, B>
where
    F: FnMut(PctEncodedFragments<'_>) -> ControlFlow<B>,
{
    /// Returns `Ok(_)` if the stored result is `Continue`, and `Err(_)` otherwise.
    #[inline(always)]
    fn result_continue_or_err(&self) -> fmt::Result {
        if self.result.is_break() {
            return Err(fmt::Error);
        }
        Ok(())
    }

    /// Calls the output functions with the undecodable fragments.
    fn output_as_undecodable(&mut self, len_undecodable: u8) -> fmt::Result {
        let len_written = usize::from(len_undecodable);
        let frag = core::str::from_utf8(&self.buf[..len_written])
            .expect("[validity] `DecoderBuffer` writes a valid ASCII string");
        let len_incomplete = len_written % 3;
        let len_complete = len_written - len_incomplete;
        self.result = (self.f)(PctEncodedFragments::InvalidUtf8PctTriplets(
            &frag[..len_complete],
        ));
        self.result_continue_or_err()?;
        if len_incomplete > 0 {
            // At least the first `%` exists.
            self.result = (self.f)(PctEncodedFragments::StrayPercent);
            if self.result.is_break() {
                return Err(fmt::Error);
            }
            if len_incomplete > 1 {
                // A following hexdigit is available.
                debug_assert_eq!(
                    len_incomplete, 2,
                    "[consistency] the length of incomplete percent-encoded \
                         triplet must be less than 2 bytes"
                );
                self.result = (self.f)(PctEncodedFragments::NoPctStr(
                    &frag[(len_complete + 1)..len_written],
                ));
                self.result_continue_or_err()?;
            }
        }
        Ok(())
    }
}

impl<F, B> fmt::Write for DecomposeWriter<'_, F, B>
where
    F: FnMut(PctEncodedFragments<'_>) -> ControlFlow<B>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.result_continue_or_err()?;
        let mut rest = s;
        while !rest.is_empty() {
            let (len_consumed, result) = self.decoder.push_encoded(self.buf, rest);
            if len_consumed == 0 {
                // `rest` does not start with the percent-encoded triplets.
                // Flush the decoder before attempting to decode more data.
                if let Some(len_written) = self.decoder.flush(self.buf).map(NonZeroU8::get) {
                    self.output_as_undecodable(len_written)?;
                    rest = &rest[usize::from(len_written)..];
                }

                // Write plain string prefix (if found).
                let (plain_prefix, suffix) = find_split(rest, b'%').unwrap_or((rest, ""));
                debug_assert!(
                    !plain_prefix.is_empty(),
                    "[consistency] `len_consumed == 0` indicates non-empty \
                     `rest` not starting with `%`"
                );
                self.result = (self.f)(PctEncodedFragments::NoPctStr(plain_prefix));
                self.result_continue_or_err()?;
                rest = suffix;
                continue;
            }

            // Process decoding result.
            match result {
                PushResult::Decoded(len_written, c) => {
                    let len_written = usize::from(len_written.get());
                    let frag = core::str::from_utf8(&self.buf[..len_written])
                        .expect("[validity] `DecoderBuffer` writes a valid ASCII string");
                    self.result = (self.f)(PctEncodedFragments::Char(frag, c));
                    self.result_continue_or_err()?;
                }
                PushResult::Undecodable(len_written) => {
                    self.output_as_undecodable(len_written)?;
                }
                PushResult::NeedMoreBytes => {
                    // Nothing to write at this time.
                }
            }
            rest = &rest[len_consumed..];
        }
        Ok(())
    }
}

/// A type for result of feeding data to [`DecoderBuffer`].
#[derive(Debug, Clone, Copy)]
enum PushResult {
    /// Input is still incomplete, needs more bytes to get the decoding result.
    NeedMoreBytes,
    /// Bytes decodable to valid UTF-8 sequence.
    // `.0`: Length of decodable fragment.
    // `.1`: Decoded character.
    Decoded(NonZeroU8, char),
    /// Valid percent-encoded triplets but not decodable to valid UTF-8 sequence.
    // `.0`: Length of undecodable fragment.
    Undecodable(u8),
}

/// Buffer to contain (and to decode) incomplete percent-encoded triplets.
#[derive(Default, Debug, Clone, Copy)]
struct DecoderBuffer {
    /// Percent-encoded triplets that possibly consists a valid UTF-8 sequence after decoded.
    //
    // `3 * 4`: 3 ASCII characters for single percent-encoded triplet, and
    // 4 triplets at most for single Unicode codepoint in UTF-8.
    encoded: [u8; 12],
    /// Decoded bytes.
    decoded: [u8; 4],
    /// Number of bytes available in `buf_encoded` buffer.
    ///
    /// `buf_encoded_len / 3` also indicates the length of data in `decoded`.
    len_encoded: u8,
}

impl DecoderBuffer {
    /// Writes the data of the given length to the destination, and remove that part from buffer.
    fn write_and_pop(&mut self, dest: &mut [u8], remove_len: u8) {
        let new_len = self.len_encoded - remove_len;
        let remove_len = usize::from(remove_len);
        let src_range = remove_len..usize::from(self.len_encoded);
        dest[..remove_len].copy_from_slice(&self.encoded[..remove_len]);

        if new_len == 0 {
            *self = Self::default();
            return;
        }
        self.encoded.copy_within(src_range, 0);
        self.decoded
            .copy_within((remove_len / 3)..usize::from(self.len_encoded / 3), 0);
        self.len_encoded = new_len;
    }

    /// Pushes a byte of a (possible) percent-encoded tripet to the buffer.
    fn push_single_encoded_byte(&mut self, byte: u8) {
        debug_assert!(
            self.len_encoded < 12,
            "[consistency] four percent-encoded triplets are enough for a unicode code point"
        );
        let pos_enc = usize::from(self.len_encoded);
        self.len_encoded += 1;
        self.encoded[pos_enc] = byte;
        if self.len_encoded % 3 == 0 {
            // A new percent-encoded triplet is read. Decode and remember.
            let pos_dec = usize::from(self.len_encoded / 3 - 1);
            let upper = self.encoded[pos_enc - 1];
            let lower = byte;
            debug_assert!(
                upper.is_ascii_hexdigit() && lower.is_ascii_hexdigit(),
                "[consistency] the `encoded` buffer should contain valid percent-encoded triplets"
            );
            self.decoded[pos_dec] = hexdigits_to_byte([upper, lower]);
        }
    }

    /// Pushes the (possibly) encoded string to the buffer.
    ///
    /// When the push result is not `PctTripletPushResult::NeedMoreBytes`, the
    /// caller should call `Self::clear()` before pushing more bytes.
    ///
    /// # Preconditions
    ///
    /// * `buf` should be more than 12 bytes. If not, this method may panic.
    #[must_use]
    pub(crate) fn push_encoded(&mut self, buf: &mut [u8], s: &str) -> (usize, PushResult) {
        debug_assert!(
            buf.len() >= 12,
            "[internal precondition] destination buffer should be at least 12 bytes"
        );
        let mut chars = s.chars();
        let mut len_triplet_incomplete = self.len_encoded % 3;
        for c in &mut chars {
            if len_triplet_incomplete == 0 {
                // Expect `%`.
                if c != '%' {
                    // Undecodable.
                    // `-1`: the last byte is peeked but not consumed.
                    let len_consumed = s.len() - chars.as_str().len() - 1;
                    let len_result = self.len_encoded;
                    self.write_and_pop(buf, len_result);
                    return (len_consumed, PushResult::Undecodable(len_result));
                }
                self.push_single_encoded_byte(b'%');
                len_triplet_incomplete = 1;
                continue;
            }

            // Expect a nibble.
            if !c.is_ascii_hexdigit() {
                // Undecodable.
                // `-1`: the last byte is peeked but not consumed.
                let len_consumed = s.len() - chars.as_str().len() - 1;
                let len_result = self.len_encoded;
                self.write_and_pop(buf, len_result);
                return (len_consumed, PushResult::Undecodable(len_result));
            }
            self.push_single_encoded_byte(c as u8);
            if len_triplet_incomplete == 1 {
                len_triplet_incomplete = 2;
                continue;
            } else {
                // Now a new percent-encoded triplet is read!
                debug_assert_eq!(len_triplet_incomplete, 2);
                len_triplet_incomplete = 0;
            }

            // Now a new percent-encoded triplet is read.
            // Check if the buffer contains a valid decodable content.
            let len_decoded = usize::from(self.len_encoded) / 3;
            match core::str::from_utf8(&self.decoded[..len_decoded]) {
                Ok(decoded_str) => {
                    // Successfully decoded.
                    let len_consumed = s.len() - chars.as_str().len();
                    let c = decoded_str
                        .chars()
                        .next()
                        .expect("[validity] `decoded` buffer is nonempty");
                    let len_result = NonZeroU8::new(self.len_encoded).expect(
                        "[consistency] `encoded` buffer is nonempty since \
                         `push_single_encoded_byte()` was called",
                    );
                    self.write_and_pop(buf, len_result.get());
                    return (len_consumed, PushResult::Decoded(len_result, c));
                }
                Err(e) => {
                    // Undecodable.
                    assert_eq!(
                        e.valid_up_to(),
                        0,
                        "[consistency] `decoded` buffer contains at most one character"
                    );
                    let skip_len_decoded = match e.error_len() {
                        // Unexpected EOF. Wait for remaining input.
                        None => continue,
                        // Skip invalid bytes.
                        Some(v) => v,
                    };
                    let len_consumed = s.len() - chars.as_str().len();
                    let len_result = skip_len_decoded as u8 * 3;
                    assert_ne!(
                        skip_len_decoded, 0,
                        "[consistency] empty bytes cannot be invalid"
                    );
                    self.write_and_pop(buf, len_result);
                    return (len_consumed, PushResult::Undecodable(len_result));
                }
            };
        }
        let len_consumed = s.len() - chars.as_str().len();
        (len_consumed, PushResult::NeedMoreBytes)
    }

    /// Writes the incomplete data completely to the destination, and clears the internal buffer.
    #[must_use]
    pub(crate) fn flush(&mut self, buf: &mut [u8]) -> Option<NonZeroU8> {
        let len_result = NonZeroU8::new(self.len_encoded)?;
        // Emit the current (undecodable) buffer as is.
        self.write_and_pop(buf, len_result.get());
        debug_assert_eq!(
            self.len_encoded, 0,
            "[consistency] the buffer should be cleared after flushed"
        );
        Some(len_result)
    }
}
