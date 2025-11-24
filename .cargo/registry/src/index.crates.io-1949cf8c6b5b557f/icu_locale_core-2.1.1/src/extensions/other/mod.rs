// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Other Use Extensions is a list of extensions other than unicode,
//! transform or private.
//!
//! Those extensions are treated as a pass-through, and no Unicode related
//! behavior depends on them.
//!
//! The main struct for this extension is [`Other`] which is a list of [`Subtag`]s.
//!
//! # Examples
//!
//! ```
//! use icu::locale::extensions::other::Other;
//! use icu::locale::Locale;
//!
//! let mut loc: Locale = "en-US-a-foo-faa".parse().expect("Parsing failed.");
//! ```

#[cfg(feature = "alloc")]
use core::str::FromStr;

#[cfg(feature = "alloc")]
use super::ExtensionType;
#[cfg(feature = "alloc")]
use crate::parser::ParseError;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::shortvec::ShortBoxSlice;
use crate::subtags::Subtag;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// A list of [`Other Use Extensions`] as defined in [`Unicode Locale
/// Identifier`] specification.
///
/// Those extensions are treated as a pass-through, and no Unicode related
/// behavior depends on them.
///
/// # Examples
///
/// ```
/// use icu::locale::extensions::other::Other;
/// use icu::locale::subtags::Subtag;
///
/// let subtag1: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
/// let subtag2: Subtag = "bar".parse().expect("Failed to parse a Subtag.");
///
/// let other = Other::from_vec_unchecked(b'a', vec![subtag1, subtag2]);
/// assert_eq!(&other.to_string(), "a-foo-bar");
/// ```
///
/// [`Other Use Extensions`]: https://unicode.org/reports/tr35/#other_extensions
/// [`Unicode Locale Identifier`]: https://unicode.org/reports/tr35/#Unicode_locale_identifier
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash, PartialOrd, Ord)]
pub struct Other {
    // Safety invariant: must be ASCII
    ext: u8,
    keys: ShortBoxSlice<Subtag>,
}

impl Other {
    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Other`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    pub fn try_from_str(s: &str) -> Result<Self, ParseError> {
        Self::try_from_utf8(s.as_bytes())
    }

    /// See [`Self::try_from_str`]
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    pub fn try_from_utf8(code_units: &[u8]) -> Result<Self, ParseError> {
        let mut iter = SubtagIterator::new(code_units);

        let ext = iter.next().ok_or(ParseError::InvalidExtension)?;
        if let ExtensionType::Other(b) = ExtensionType::try_from_byte_slice(ext)? {
            return Self::try_from_iter(b, &mut iter);
        }

        Err(ParseError::InvalidExtension)
    }

    /// A constructor which takes a pre-sorted list of [`Subtag`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Panics
    ///
    /// Panics if `ext` is not ASCII alphabetic.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::other::Other;
    /// use icu::locale::subtags::Subtag;
    ///
    /// let subtag1: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
    /// let subtag2: Subtag = "bar".parse().expect("Failed to parse a Subtag.");
    ///
    /// let other = Other::from_vec_unchecked(b'a', vec![subtag1, subtag2]);
    /// assert_eq!(&other.to_string(), "a-foo-bar");
    /// ```
    #[cfg(feature = "alloc")]
    pub fn from_vec_unchecked(ext: u8, keys: Vec<Subtag>) -> Self {
        Self::from_short_slice_unchecked(ext, keys.into())
    }

    #[allow(dead_code)]
    pub(crate) fn from_short_slice_unchecked(ext: u8, keys: ShortBoxSlice<Subtag>) -> Self {
        assert!(ext.is_ascii_alphabetic());
        // Safety invariant upheld here: ext checked as ASCII above
        Self { ext, keys }
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(ext: u8, iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        debug_assert!(matches!(
            ExtensionType::try_from_byte(ext),
            Ok(ExtensionType::Other(_)),
        ));

        let mut keys = ShortBoxSlice::new();
        while let Some(subtag) = iter.peek() {
            if !Subtag::valid_key(subtag) {
                break;
            }
            if let Ok(key) = Subtag::try_from_utf8(subtag) {
                keys.push(key);
            }
            iter.next();
        }

        if keys.is_empty() {
            Err(ParseError::InvalidExtension)
        } else {
            Ok(Self::from_short_slice_unchecked(ext, keys))
        }
    }

    /// Gets the tag character for this extension as a &str.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let loc: Locale = "und-a-hello-world".parse().unwrap();
    /// let other_ext = &loc.extensions.other[0];
    /// assert_eq!(other_ext.get_ext_str(), "a");
    /// ```
    pub fn get_ext_str(&self) -> &str {
        debug_assert!(self.ext.is_ascii_alphabetic());
        // Safety: from safety invariant on self.ext (that it is ASCII)
        unsafe { core::str::from_utf8_unchecked(core::slice::from_ref(&self.ext)) }
    }

    /// Gets the tag character for this extension as a char.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let loc: Locale = "und-a-hello-world".parse().unwrap();
    /// let other_ext = &loc.extensions.other[0];
    /// assert_eq!(other_ext.get_ext(), 'a');
    /// ```
    pub fn get_ext(&self) -> char {
        self.ext as char
    }

    /// Gets the tag character for this extension as a byte.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::Locale;
    ///
    /// let loc: Locale = "und-a-hello-world".parse().unwrap();
    /// let other_ext = &loc.extensions.other[0];
    /// assert_eq!(other_ext.get_ext_byte(), b'a');
    /// ```
    pub fn get_ext_byte(&self) -> u8 {
        self.ext
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F, with_ext: bool) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        if self.keys.is_empty() {
            return Ok(());
        }

        if with_ext {
            f(self.get_ext_str())?;
        }
        self.keys.iter().map(|t| t.as_str()).try_for_each(f)
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Other {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

writeable::impl_display_with_writeable!(Other, #[cfg(feature = "alloc")]);

impl writeable::Writeable for Other {
    fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
        if self.keys.is_empty() {
            return Ok(());
        }
        sink.write_str(self.get_ext_str())?;
        for key in self.keys.iter() {
            sink.write_char('-')?;
            writeable::Writeable::write_to(key, sink)?;
        }

        Ok(())
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        if self.keys.is_empty() {
            return writeable::LengthHint::exact(0);
        };
        let mut result = writeable::LengthHint::exact(1);
        for key in self.keys.iter() {
            result += writeable::Writeable::writeable_length_hint(key) + 1;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_other_extension_fromstr() {
        let oe: Other = "o-foo-bar".parse().expect("Failed to parse Other");
        assert_eq!(oe.to_string(), "o-foo-bar");

        let oe: Result<Other, _> = "o".parse();
        assert!(oe.is_err());
    }
}
