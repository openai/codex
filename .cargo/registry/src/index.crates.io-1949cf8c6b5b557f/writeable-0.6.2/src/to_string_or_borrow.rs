// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::Writeable;
use alloc::borrow::Cow;
use alloc::string::String;
use core::fmt;

/// Bytes that have been partially validated as UTF-8 up to an offset.
struct PartiallyValidatedUtf8<'a> {
    // Safety Invariants:
    // 1. The offset is less than or equal to the length of the slice.
    // 2. The slice is valid UTF-8 up to the offset.
    slice: &'a [u8],
    offset: usize,
}

impl<'a> PartiallyValidatedUtf8<'a> {
    fn new(slice: &'a [u8]) -> Self {
        // Safety: Field invariants maintained here trivially:
        //   1. The offset 0 is ≤ all possible lengths of slice
        //   2. The slice contains nothing up to the offset zero
        Self { slice, offset: 0 }
    }

    /// Check whether the given string is the next chunk of unvalidated bytes.
    /// If so, increment offset and return true. Otherwise, return false.
    fn try_push(&mut self, valid_str: &str) -> bool {
        let new_offset = self.offset + valid_str.len();
        if self.slice.get(self.offset..new_offset) == Some(valid_str.as_bytes()) {
            // Safety: Field invariants maintained here:
            //   1. In the line above, `self.slice.get()` returned `Some()` for `new_offset` at
            //      the end of a `Range`, so `new_offset` is ≤ the length of `self.slice`.
            //   2. By invariant, we have already validated the string up to `self.offset`, and
            //      the portion of the slice between `self.offset` and `new_offset` is equal to
            //      `valid_str`, which is a `&str`, so the string is valid up to `new_offset`.
            self.offset = new_offset;
            true
        } else {
            false
        }
    }

    /// Return the validated portion as `&str`.
    fn validated_as_str(&self) -> &'a str {
        debug_assert!(self.offset <= self.slice.len());
        // Safety: self.offset is a valid end index in a range (from field invariant)
        let valid_slice = unsafe { self.slice.get_unchecked(..self.offset) };
        debug_assert!(core::str::from_utf8(valid_slice).is_ok());
        // Safety: the UTF-8 of slice has been validated up to offset (from field invariant)
        unsafe { core::str::from_utf8_unchecked(valid_slice) }
    }
}

enum SliceOrString<'a> {
    Slice(PartiallyValidatedUtf8<'a>),
    String(String),
}

/// This is an infallible impl. Functions always return Ok, not Err.
impl fmt::Write for SliceOrString<'_> {
    #[inline]
    fn write_str(&mut self, other: &str) -> fmt::Result {
        match self {
            SliceOrString::Slice(slice) => {
                if !slice.try_push(other) {
                    // We failed to match. Convert to owned.
                    let valid_str = slice.validated_as_str();
                    let mut owned = String::with_capacity(valid_str.len() + other.len());
                    owned.push_str(valid_str);
                    owned.push_str(other);
                    *self = SliceOrString::String(owned);
                }
                Ok(())
            }
            SliceOrString::String(owned) => owned.write_str(other),
        }
    }
}

impl<'a> SliceOrString<'a> {
    #[inline]
    fn new(slice: &'a [u8]) -> Self {
        Self::Slice(PartiallyValidatedUtf8::new(slice))
    }

    #[inline]
    fn finish(self) -> Cow<'a, str> {
        match self {
            SliceOrString::Slice(slice) => Cow::Borrowed(slice.validated_as_str()),
            SliceOrString::String(owned) => Cow::Owned(owned),
        }
    }
}

/// Writes the contents of a `Writeable` to a string, returning a reference
/// to a slice if it matches the provided reference bytes, and allocating a
/// String otherwise.
///
/// This function is useful if you have borrowed bytes which you expect
/// to be equal to a writeable a high percentage of the time.
///
/// You can also use this function to make a more efficient implementation of
/// [`Writeable::write_to_string`].
///
/// # Examples
///
/// Basic usage and behavior:
///
/// ```
/// use std::fmt;
/// use std::borrow::Cow;
/// use writeable::Writeable;
///
/// struct WelcomeMessage<'s> {
///     pub name: &'s str,
/// }
///
/// impl<'s> Writeable for WelcomeMessage<'s> {
///     // see impl in Writeable docs
/// #    fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
/// #        sink.write_str("Hello, ")?;
/// #        sink.write_str(self.name)?;
/// #        sink.write_char('!')?;
/// #        Ok(())
/// #    }
/// }
///
/// let message = WelcomeMessage { name: "Alice" };
///
/// assert!(matches!(
///     writeable::to_string_or_borrow(&message, b""),
///     Cow::Owned(s) if s == "Hello, Alice!"
/// ));
/// assert!(matches!(
///     writeable::to_string_or_borrow(&message, b"Hello"),
///     Cow::Owned(s) if s == "Hello, Alice!"
/// ));
/// assert!(matches!(
///     writeable::to_string_or_borrow(&message, b"Hello, Bob!"),
///     Cow::Owned(s) if s == "Hello, Alice!"
/// ));
/// assert!(matches!(
///     writeable::to_string_or_borrow(&message, b"Hello, Alice!"),
///     Cow::Borrowed("Hello, Alice!")
/// ));
///
/// // Borrowing can use a prefix:
/// assert!(matches!(
///     writeable::to_string_or_borrow(&message, b"Hello, Alice!..\xFF\x00\xFF"),
///     Cow::Borrowed("Hello, Alice!")
/// ));
/// ```
///
/// Example use case: a function that transforms a string to lowercase.
/// We are also able to write a more efficient implementation of
/// [`Writeable::write_to_string`] in this situation.
///
/// ```
/// use std::fmt;
/// use std::borrow::Cow;
/// use writeable::Writeable;
///
/// struct MakeAsciiLower<'a>(&'a str);
///
/// impl<'a> Writeable for MakeAsciiLower<'a> {
///     fn write_to<W: fmt::Write + ?Sized>(&self, sink: &mut W) -> fmt::Result {
///         for c in self.0.chars() {
///             sink.write_char(c.to_ascii_lowercase())?;
///         }
///         Ok(())
///     }
///     #[inline]
///     fn writeable_borrow(&self) -> Option<&str> {
///         self.0.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_whitespace()).then_some(self.0)
///     }
/// }
///
/// fn make_lowercase(input: &str) -> Cow<'_, str> {
///     let writeable = MakeAsciiLower(input);
///     writeable::to_string_or_borrow(&writeable, input.as_bytes())
/// }
///
/// assert!(matches!(
///     make_lowercase("this is lowercase"),
///     Cow::Borrowed("this is lowercase")
/// ));
/// assert!(matches!(
///     make_lowercase("this is UPPERCASE"),
///     Cow::Owned(s) if s == "this is uppercase"
/// ));
///
/// assert!(matches!(
///     MakeAsciiLower("this is lowercase").write_to_string(),
///     Cow::Borrowed("this is lowercase")
/// ));
/// assert!(matches!(
///     MakeAsciiLower("this is UPPERCASE").write_to_string(),
///     Cow::Owned(s) if s == "this is uppercase"
/// ));
/// ```
pub fn to_string_or_borrow<'a>(
    writeable: &impl Writeable,
    reference_bytes: &'a [u8],
) -> Cow<'a, str> {
    let mut sink = SliceOrString::new(reference_bytes);
    let _ = writeable.write_to(&mut sink);
    sink.finish()
}
