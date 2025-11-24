//! Percent-encoding normalization and case normalization.

use core::cmp::Ordering;
use core::fmt::{self, Write as _};
use core::marker::PhantomData;

use crate::format::eq_str_display;
use crate::parser::char::{is_ascii_unreserved, is_unreserved, is_utf8_byte_continue};
use crate::parser::str::{find_split_hole, take_first_char};
use crate::parser::trusted::take_xdigits2;
use crate::spec::Spec;

/// Returns true if the given string is percent-encoding normalized and case
/// normalized.
///
/// Note that normalization of ASCII-only host requires additional case
/// normalization, so checking by this function is not sufficient for that case.
pub(crate) fn is_pct_case_normalized<S: Spec>(s: &str) -> bool {
    eq_str_display(s, &PctCaseNormalized::<S>::new(s))
}

/// Returns a character for the slice.
///
/// Essentially equivalent to `core::str::from_utf8(bytes).unwrap().and_then(|s| s.get(0))`,
/// but this function fully trusts that the input is a valid UTF-8 string with
/// only one character.
fn into_char_trusted(bytes: &[u8]) -> Result<char, ()> {
    /// The bit mask to get the content part in a continue byte.
    const CONTINUE_BYTE_MASK: u8 = 0b_0011_1111;
    /// Minimum valid values for a code point in a UTF-8 sequence of 2, 3, and 4 bytes.
    const MIN: [u32; 3] = [0x80, 0x800, 0x1_0000];

    let len = bytes.len();
    let c: u32 = match len {
        2 => (u32::from(bytes[0] & 0b_0001_1111) << 6) | u32::from(bytes[1] & CONTINUE_BYTE_MASK),
        3 => {
            (u32::from(bytes[0] & 0b_0000_1111) << 12)
                | (u32::from(bytes[1] & CONTINUE_BYTE_MASK) << 6)
                | u32::from(bytes[2] & CONTINUE_BYTE_MASK)
        }
        4 => {
            (u32::from(bytes[0] & 0b_0000_0111) << 18)
                | (u32::from(bytes[1] & CONTINUE_BYTE_MASK) << 12)
                | (u32::from(bytes[2] & CONTINUE_BYTE_MASK) << 6)
                | u32::from(bytes[3] & CONTINUE_BYTE_MASK)
        }
        len => unreachable!(
            "[consistency] expected 2, 3, or 4 bytes for a character, but got {len} as the length"
        ),
    };
    if c < MIN[len - 2] {
        // Redundant UTF-8 encoding.
        return Err(());
    }
    // Can be an invalid Unicode code point.
    char::from_u32(c).ok_or(())
}

/// Writable as a normalized path segment percent-encoding IRI.
///
/// This wrapper does the things below when being formatted:
///
/// * Decode unnecessarily percent-encoded characters.
/// * Convert alphabetic characters uppercase in percent-encoded triplets.
///
/// Note that this does not newly encode raw characters.
///
/// # Safety
///
/// The given string should be the valid path segment.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PctCaseNormalized<'a, S> {
    /// Valid segment name to normalize.
    segname: &'a str,
    /// Spec.
    _spec: PhantomData<fn() -> S>,
}

impl<'a, S: Spec> PctCaseNormalized<'a, S> {
    /// Creates a new `PctCaseNormalized` value.
    #[inline]
    #[must_use]
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            segname: source,
            _spec: PhantomData,
        }
    }
}

impl<S: Spec> fmt::Display for PctCaseNormalized<'_, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = self.segname;

        'outer_loop: while !rest.is_empty() {
            // Scan the next percent-encoded triplet.
            let (prefix, after_percent) = match find_split_hole(rest, b'%') {
                Some(v) => v,
                None => return f.write_str(rest),
            };
            // Write the string before the percent-encoded triplet.
            f.write_str(prefix)?;
            // Decode the percent-encoded triplet.
            let (first_decoded, after_first_triplet) = take_xdigits2(after_percent);
            rest = after_first_triplet;

            if first_decoded.is_ascii() {
                if is_ascii_unreserved(first_decoded) {
                    // Unreserved. Print the decoded.
                    f.write_char(char::from(first_decoded))?;
                } else {
                    write!(f, "%{:02X}", first_decoded)?;
                }
                continue 'outer_loop;
            }

            // Continue byte cannot be the first byte of a character.
            if is_utf8_byte_continue(first_decoded) {
                write!(f, "%{:02X}", first_decoded)?;
                continue 'outer_loop;
            }

            // Get the expected length of decoded char.
            let expected_char_len = match (first_decoded & 0xf0).cmp(&0b1110_0000) {
                Ordering::Less => 2,
                Ordering::Equal => 3,
                Ordering::Greater => 4,
            };

            // Get continue bytes.
            let c_buf = &mut [first_decoded, 0, 0, 0][..expected_char_len];
            for (i, buf_dest) in c_buf[1..].iter_mut().enumerate() {
                match take_first_char(rest) {
                    Some(('%', after_percent)) => {
                        let (byte, after_triplet) = take_xdigits2(after_percent);
                        if !is_utf8_byte_continue(byte) {
                            // Note that `byte` can start the new string.
                            // Leave the byte in the `rest` for next try (i.e.
                            // don't update `rest` in this case).
                            c_buf[..=i]
                                .iter()
                                .try_for_each(|b| write!(f, "%{:02X}", b))?;
                            continue 'outer_loop;
                        }
                        *buf_dest = byte;
                        rest = after_triplet;
                    }
                    // If the next character is not `%`, decoded bytes so far
                    // won't be valid UTF-8 byte sequence.
                    // Write the read percent-encoded triplets without decoding.
                    // Note that all characters in `&c_buf[1..]` (if available)
                    // will be decoded to "continue byte" of UTF-8, so they
                    // cannot be the start of a valid UTF-8 byte sequence if
                    // decoded.
                    Some((c, after_percent)) => {
                        c_buf[..=i]
                            .iter()
                            .try_for_each(|b| write!(f, "%{:02X}", b))?;
                        f.write_char(c)?;
                        rest = after_percent;
                        continue 'outer_loop;
                    }
                    None => {
                        c_buf[..=i]
                            .iter()
                            .try_for_each(|b| write!(f, "%{:02X}", b))?;
                        // Reached the end of the string.
                        break 'outer_loop;
                    }
                }
            }

            // Decode the bytes into a character.
            match into_char_trusted(&c_buf[..expected_char_len]) {
                Ok(decoded_c) => {
                    if is_unreserved::<S>(decoded_c) {
                        // Unreserved. Print the decoded.
                        f.write_char(decoded_c)?;
                    } else {
                        c_buf[0..expected_char_len]
                            .iter()
                            .try_for_each(|b| write!(f, "%{:02X}", b))?;
                    }
                }
                Err(_) => {
                    // Skip decoding of the entire sequence of pct-encoded triplets loaded
                    // in `c_buf`. This is valid from the reasons below.
                    //
                    // * The first byte in `c_buf` is valid as the first byte, and it tells the
                    //   expected number of bytes for a code unit. The cases the bytes being too
                    //   short and the sequence being incomplete have already been handled, and
                    //   the execution does not reach here then.
                    // * All of the non-first bytes are checked if they are valid as UTF8 continue
                    //   bytes by `is_utf8_byte_continue()`. If they're not, the decoding of
                    //   that codepoint is aborted and the bytes in the buffer are immediately
                    //   emitted as pct-encoded, and the execution does not reach here. This
                    //   means that the bytes in the current `c_buf` have passed these tests.
                    // * Since all of the the non-first bytes are UTF8 continue bytes, any of
                    //   them cannot start the new valid UTF-8 byte sequence. This means that
                    //   if the bytes in the buffer does not consitute a valid UTF-8 bytes
                    //   sequence, the whole buffer can immediately be emmitted as pct-encoded.

                    debug_assert!(
                        c_buf[1..expected_char_len]
                            .iter()
                            .copied()
                            .all(is_utf8_byte_continue),
                        "[consistency] all non-first bytes have been \
                         confirmed that they are UTF-8 continue bytes"
                    );
                    // Note that the first pct-encoded triplet is stripped from
                    // `after_first_triplet`.
                    rest = &after_first_triplet[((expected_char_len - 1) * 3)..];
                    c_buf[0..expected_char_len]
                        .iter()
                        .try_for_each(|b| write!(f, "%{:02X}", b))?;
                }
            }
        }

        Ok(())
    }
}

/// Writable as a normalized ASCII-only `host` (and optionally `port` followed).
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalizedAsciiOnlyHost<'a> {
    /// Valid host (and additionaly port) to normalize.
    host_port: &'a str,
}

impl<'a> NormalizedAsciiOnlyHost<'a> {
    /// Creates a new `NormalizedAsciiOnlyHost` value.
    ///
    /// # Preconditions
    ///
    /// The given string should be the valid ASCII-only `host` or
    /// `host ":" port` after percent-encoding normalization.
    /// In other words, [`parser::trusted::is_ascii_only_host`] should return
    /// true for the given value.
    ///
    /// [`parser::trusted::is_ascii_only_host`]: `crate::parser::trusted::is_ascii_only_host`
    #[inline]
    #[must_use]
    pub(crate) fn new(host_port: &'a str) -> Self {
        Self { host_port }
    }
}

impl fmt::Display for NormalizedAsciiOnlyHost<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = self.host_port;

        while !rest.is_empty() {
            // Scan the next percent-encoded triplet.
            let (prefix, after_percent) = match find_split_hole(rest, b'%') {
                Some(v) => v,
                None => {
                    return rest
                        .chars()
                        .try_for_each(|c| f.write_char(c.to_ascii_lowercase()));
                }
            };
            // Write the string before the percent-encoded triplet.
            prefix
                .chars()
                .try_for_each(|c| f.write_char(c.to_ascii_lowercase()))?;
            // Decode the percent-encoded triplet.
            let (first_decoded, after_triplet) = take_xdigits2(after_percent);
            rest = after_triplet;

            assert!(
                first_decoded.is_ascii(),
                "[consistency] this function requires ASCII-only host as an argument"
            );

            if is_ascii_unreserved(first_decoded) {
                // Unreserved. Convert to lowercase and print.
                f.write_char(char::from(first_decoded.to_ascii_lowercase()))?;
            } else {
                write!(f, "%{:02X}", first_decoded)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "alloc")]
mod tests {
    use super::*;

    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::string::ToString;

    use crate::spec::{IriSpec, UriSpec};

    #[test]
    fn invalid_utf8() {
        assert_eq!(
            PctCaseNormalized::<UriSpec>::new("%80%cc%cc%cc").to_string(),
            "%80%CC%CC%CC"
        );
        assert_eq!(
            PctCaseNormalized::<IriSpec>::new("%80%cc%cc%cc").to_string(),
            "%80%CC%CC%CC"
        );
    }

    #[test]
    fn iri_unreserved() {
        assert_eq!(
            PctCaseNormalized::<UriSpec>::new("%ce%b1").to_string(),
            "%CE%B1"
        );
        assert_eq!(
            PctCaseNormalized::<IriSpec>::new("%ce%b1").to_string(),
            "\u{03B1}"
        );
    }

    #[test]
    fn iri_middle_decode() {
        assert_eq!(
            PctCaseNormalized::<UriSpec>::new("%ce%ce%b1%b1").to_string(),
            "%CE%CE%B1%B1"
        );
        assert_eq!(
            PctCaseNormalized::<IriSpec>::new("%ce%ce%b1%b1").to_string(),
            "%CE\u{03B1}%B1"
        );
    }

    #[test]
    fn ascii_reserved() {
        assert_eq!(PctCaseNormalized::<UriSpec>::new("%3f").to_string(), "%3F");
        assert_eq!(PctCaseNormalized::<IriSpec>::new("%3f").to_string(), "%3F");
    }

    #[test]
    fn ascii_forbidden() {
        assert_eq!(
            PctCaseNormalized::<UriSpec>::new("%3c%3e").to_string(),
            "%3C%3E"
        );
        assert_eq!(
            PctCaseNormalized::<IriSpec>::new("%3c%3e").to_string(),
            "%3C%3E"
        );
    }

    #[test]
    fn ascii_unreserved() {
        assert_eq!(PctCaseNormalized::<UriSpec>::new("%7ea").to_string(), "~a");
        assert_eq!(PctCaseNormalized::<IriSpec>::new("%7ea").to_string(), "~a");
    }
}
