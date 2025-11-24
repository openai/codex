//! Conversion between URI/IRI types.

use core::fmt;

#[cfg(feature = "alloc")]
use alloc::collections::TryReserveError;
#[cfg(all(feature = "alloc", not(feature = "std")))]
use alloc::string::String;

#[cfg(feature = "alloc")]
use crate::format::{ToDedicatedString, ToStringFallible};
use crate::spec::Spec;
use crate::types::{
    RiAbsoluteStr, RiFragmentStr, RiQueryStr, RiReferenceStr, RiRelativeStr, RiStr,
};
#[cfg(feature = "alloc")]
use crate::types::{
    RiAbsoluteString, RiFragmentString, RiQueryString, RiReferenceString, RiRelativeString,
    RiString,
};
#[cfg(feature = "alloc")]
use crate::types::{
    UriAbsoluteString, UriFragmentString, UriQueryString, UriReferenceString, UriRelativeString,
    UriString,
};

/// Hexadecimal digits for a nibble.
const HEXDIGITS: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B', b'C', b'D', b'E', b'F',
];

/// A resource identifier mapped to a URI of some kind.
///
/// Supported `Src` type are:
///
/// * IRIs:
///     + [`IriAbsoluteStr`] (alias of `RiAbsoluteStr<IriSpec>`)
///     + [`IriReferenceStr`] (alias of `RiReferenceStr<IriSpec>`)
///     + [`IriRelativeStr`] (alias of `RiRelativeStr<IriSpec>`)
///     + [`IriStr`] (alias of `RiStr<IriSpec>`)
/// * URIs:
///     + [`UriAbsoluteStr`] (alias of `RiAbsoluteStr<UriSpec>`)
///     + [`UriReferenceStr`] (alias of `RiReferenceStr<UriSpec>`)
///     + [`UriRelativeStr`] (alias of `RiRelativeStr<UriSpec>`)
///     + [`UriStr`] (alias of `RiStr<UriSpec>`)
///
/// # Examples
///
/// ```
/// use iri_string::convert::MappedToUri;
/// use iri_string::types::{IriStr, UriStr};
///
/// let src = IriStr::new("http://example.com/?alpha=\u{03B1}")?;
/// // The type is `MappedToUri<IriStr>`, but you usually don't need to specify.
/// let mapped = MappedToUri::from(src).to_string();
/// assert_eq!(mapped, "http://example.com/?alpha=%CE%B1");
/// # Ok::<_, iri_string::validate::Error>(())
/// ```
///
/// [`IriAbsoluteStr`]: crate::types::IriAbsoluteStr
/// [`IriReferenceStr`]: crate::types::IriReferenceStr
/// [`IriRelativeStr`]: crate::types::IriRelativeStr
/// [`IriStr`]: crate::types::IriStr
/// [`UriAbsoluteStr`]: crate::types::UriAbsoluteStr
/// [`UriReferenceStr`]: crate::types::UriReferenceStr
/// [`UriRelativeStr`]: crate::types::UriRelativeStr
/// [`UriStr`]: crate::types::UriStr
#[derive(Debug, Clone, Copy)]
pub struct MappedToUri<'a, Src: ?Sized>(&'a Src);

/// Implement conversions for an IRI string type.
macro_rules! impl_for_iri {
    ($borrowed:ident, $owned:ident, $owned_uri:ident) => {
        impl<S: Spec> fmt::Display for MappedToUri<'_, $borrowed<S>> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write_percent_encoded(f, self.0.as_str())
            }
        }

        #[cfg(feature = "alloc")]
        impl<S: Spec> ToDedicatedString for MappedToUri<'_, $borrowed<S>> {
            type Target = $owned_uri;

            fn try_to_dedicated_string(&self) -> Result<Self::Target, TryReserveError> {
                let s = self.try_to_string()?;
                Ok(TryFrom::try_from(s)
                    .expect("[validity] the IRI must be encoded into a valid URI"))
            }
        }

        impl<'a, S: Spec> From<&'a $borrowed<S>> for MappedToUri<'a, $borrowed<S>> {
            #[inline]
            fn from(iri: &'a $borrowed<S>) -> Self {
                Self(iri)
            }
        }

        #[cfg(feature = "alloc")]
        impl<'a, S: Spec> From<&'a $owned<S>> for MappedToUri<'a, $borrowed<S>> {
            #[inline]
            fn from(iri: &'a $owned<S>) -> Self {
                Self(iri.as_slice())
            }
        }
    };
}

impl_for_iri!(RiReferenceStr, RiReferenceString, UriReferenceString);
impl_for_iri!(RiStr, RiString, UriString);
impl_for_iri!(RiAbsoluteStr, RiAbsoluteString, UriAbsoluteString);
impl_for_iri!(RiRelativeStr, RiRelativeString, UriRelativeString);
impl_for_iri!(RiQueryStr, RiQueryString, UriQueryString);
impl_for_iri!(RiFragmentStr, RiFragmentString, UriFragmentString);

/// Percent-encodes and writes the IRI string using the given buffer.
fn write_percent_encoded(f: &mut fmt::Formatter<'_>, mut s: &str) -> fmt::Result {
    while !s.is_empty() {
        // Skip ASCII characters.
        let non_ascii_pos = s.bytes().position(|b| !b.is_ascii()).unwrap_or(s.len());
        let (ascii, rest) = s.split_at(non_ascii_pos);
        if !ascii.is_empty() {
            f.write_str(ascii)?;
            s = rest;
        }

        if s.is_empty() {
            return Ok(());
        }

        // Search for the next ASCII character.
        let nonascii_end = s.bytes().position(|b| b.is_ascii()).unwrap_or(s.len());
        let (nonasciis, rest) = s.split_at(nonascii_end);
        debug_assert!(
            !nonasciis.is_empty(),
            "string without non-ASCII characters should have caused early return"
        );
        s = rest;

        // Escape non-ASCII characters as percent-encoded bytes.
        //
        // RFC 3987 (section 3.1 step 2) says "for each character in
        // 'ucschar' or 'iprivate'", but this simply means "for each
        // non-ASCII characters" since any non-ASCII characters that can
        // appear in an IRI match `ucschar` or `iprivate`.
        /// Number of source bytes to encode at once.
        const NUM_BYTES_AT_ONCE: usize = 21;
        percent_encode_bytes(f, nonasciis, &mut [0_u8; NUM_BYTES_AT_ONCE * 3])?;
    }

    Ok(())
}

/// Percent-encode the string and pass the encoded chunks to the given function.
///
/// `buf` is used as a temporary working buffer. It is initialized by this
/// function, so users can pass any mutable byte slice with enough size.
///
/// # Precondition
///
/// The length of `buf` must be 3 bytes or more.
fn percent_encode_bytes(f: &mut fmt::Formatter<'_>, s: &str, buf: &mut [u8]) -> fmt::Result {
    /// Fill the buffer by percent-encoded bytes.
    ///
    /// Note that this function applies percent-encoding to every characters,
    /// even if it is ASCII alphabet.
    ///
    /// # Precondition
    ///
    /// * The length of `buf` must be 3 bytes or more.
    /// * All of the `buf[i * 3]` elements should already be set to `b'%'`.
    // This function have many preconditions and I don't want checks for them
    // to be mandatory, so make this nested inner function.
    fn fill_by_percent_encoded<'a>(buf: &'a mut [u8], bytes: &mut core::str::Bytes<'_>) -> &'a str {
        let src_len = bytes.len();
        // `<[u8; N]>::array_chunks_mut` is unstable as of Rust 1.58.1.
        for (dest, byte) in buf.chunks_exact_mut(3).zip(bytes.by_ref()) {
            debug_assert_eq!(
                dest.len(),
                3,
                "[validity] `chunks_exact()` must return a slice with the exact length"
            );
            debug_assert_eq!(
                dest[0], b'%',
                "[precondition] the buffer must be properly initialized"
            );

            let upper = byte >> 4;
            let lower = byte & 0b1111;
            dest[1] = HEXDIGITS[usize::from(upper)];
            dest[2] = HEXDIGITS[usize::from(lower)];
        }
        let num_dest_written = (src_len - bytes.len()) * 3;
        let buf_filled = &buf[..num_dest_written];
        // SAFETY: `b'%'` and `HEXDIGITS[_]` are all ASCII characters, so
        // `buf_filled` is filled with ASCII characters and is valid UTF-8 bytes.
        unsafe {
            debug_assert!(core::str::from_utf8(buf_filled).is_ok());
            core::str::from_utf8_unchecked(buf_filled)
        }
    }

    assert!(
        buf.len() >= 3,
        "[precondition] length of `buf` must be 3 bytes or more"
    );

    // Drop the elements that will never be used.
    // The length to be used is always a multiple of three.
    let buf_len = buf.len() / 3 * 3;
    let buf = &mut buf[..buf_len];

    // Fill some bytes with `%`.
    // This will be vectorized by optimization (especially for long buffers),
    // so no need to selectively set `buf[i * 3]`.
    buf.fill(b'%');

    let mut bytes = s.bytes();
    // `<core::str::Bytes as ExactSizeIterator>::is_empty` is unstable as of Rust 1.58.1.
    while bytes.len() != 0 {
        let encoded = fill_by_percent_encoded(buf, &mut bytes);
        f.write_str(encoded)?;
    }

    Ok(())
}

/// Percent-encodes the given IRI using the given buffer.
#[cfg(feature = "alloc")]
pub(crate) fn try_percent_encode_iri_inline(
    iri: &mut String,
) -> Result<(), alloc::collections::TryReserveError> {
    // Calculate the result length and extend the buffer.
    let num_nonascii = count_nonascii(iri);
    if num_nonascii == 0 {
        // No need to escape.
        return Ok(());
    }
    let additional = num_nonascii * 2;
    iri.try_reserve(additional)?;
    let src_len = iri.len();

    // Temporarily take the ownership of the internal buffer.
    let mut buf = core::mem::take(iri).into_bytes();
    // `b'\0'` cannot appear in a valid IRI, so this default value would be
    // useful in case of debugging.
    buf.extend(core::iter::repeat(b'\0').take(additional));

    // Fill the buffer from the tail to the head.
    let mut dest_end = buf.len();
    let mut src_end = src_len;
    let mut rest_nonascii = num_nonascii;
    while rest_nonascii > 0 {
        debug_assert!(
            src_end > 0,
            "[validity] the source position should not overrun"
        );
        debug_assert!(
            dest_end > 0,
            "[validity] the destination position should not overrun"
        );
        src_end -= 1;
        dest_end -= 1;
        let byte = buf[src_end];
        if byte.is_ascii() {
            buf[dest_end] = byte;
            // Use the ASCII character directly.
        } else {
            // Percent-encode the byte.
            dest_end -= 2;
            buf[dest_end] = b'%';
            let upper = byte >> 4;
            let lower = byte & 0b1111;
            buf[dest_end + 1] = HEXDIGITS[usize::from(upper)];
            buf[dest_end + 2] = HEXDIGITS[usize::from(lower)];
            rest_nonascii -= 1;
        }
    }

    // Move the result from the temporary buffer to the destination.
    let s = String::from_utf8(buf).expect("[consistency] the encoding result is an ASCII string");
    *iri = s;
    Ok(())
}

/// Returns the number of non-ASCII characters.
#[cfg(feature = "alloc")]
#[inline]
#[must_use]
fn count_nonascii(s: &str) -> usize {
    s.bytes().filter(|b| !b.is_ascii()).count()
}
