// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! Private Use Extensions is a list of extensions intended for
//! private use.
//!
//! Those extensions are treated as a pass-through, and no Unicode related
//! behavior depends on them.
//!
//! The main struct for this extension is [`Private`] which is a list of [`Subtag`]s.
//!
//! # Examples
//!
//! ```
//! use icu::locale::extensions::private::subtag;
//! use icu::locale::{locale, Locale};
//!
//! let mut loc: Locale = "en-US-x-foo-faa".parse().expect("Parsing failed.");
//!
//! assert!(loc.extensions.private.contains(&subtag!("foo")));
//! assert_eq!(loc.extensions.private.iter().next(), Some(&subtag!("foo")));
//!
//! loc.extensions.private.clear();
//!
//! assert!(loc.extensions.private.is_empty());
//! assert_eq!(loc, locale!("en-US"));
//! ```

mod other;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::ops::Deref;
#[cfg(feature = "alloc")]
use core::str::FromStr;

#[doc(inline)]
pub use other::{subtag, Subtag};

#[cfg(feature = "alloc")]
use super::ExtensionType;
#[cfg(feature = "alloc")]
use crate::parser::ParseError;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::shortvec::ShortBoxSlice;

pub(crate) const PRIVATE_EXT_CHAR: char = 'x';
pub(crate) const PRIVATE_EXT_STR: &str = "x";

/// A list of [`Private Use Extensions`] as defined in [`Unicode Locale
/// Identifier`] specification.
///
/// Those extensions are treated as a pass-through, and no Unicode related
/// behavior depends on them.
///
/// # Examples
///
/// ```
/// use icu::locale::extensions::private::{Private, Subtag};
///
/// let subtag1: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
/// let subtag2: Subtag = "bar".parse().expect("Failed to parse a Subtag.");
///
/// let private = Private::from_vec_unchecked(vec![subtag1, subtag2]);
/// assert_eq!(&private.to_string(), "x-foo-bar");
/// ```
///
/// [`Private Use Extensions`]: https://unicode.org/reports/tr35/#pu_extensions
/// [`Unicode Locale Identifier`]: https://unicode.org/reports/tr35/#Unicode_locale_identifier
#[derive(Clone, PartialEq, Eq, Debug, Default, Hash, PartialOrd, Ord)]
pub struct Private(ShortBoxSlice<Subtag>);

impl Private {
    /// Returns a new empty list of private-use extensions. Same as [`default()`](Default::default()), but is `const`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::private::Private;
    ///
    /// assert_eq!(Private::new(), Private::default());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self(ShortBoxSlice::new())
    }

    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Private`].
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
        if let ExtensionType::Private = ExtensionType::try_from_byte_slice(ext)? {
            return Self::try_from_iter(&mut iter);
        }

        Err(ParseError::InvalidExtension)
    }

    /// A constructor which takes a pre-sorted list of [`Subtag`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::private::{Private, Subtag};
    ///
    /// let subtag1: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
    /// let subtag2: Subtag = "bar".parse().expect("Failed to parse a Subtag.");
    ///
    /// let private = Private::from_vec_unchecked(vec![subtag1, subtag2]);
    /// assert_eq!(&private.to_string(), "x-foo-bar");
    /// ```
    #[cfg(feature = "alloc")]
    pub fn from_vec_unchecked(input: Vec<Subtag>) -> Self {
        Self(input.into())
    }

    /// A constructor which takes a single [`Subtag`].
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::private::{Private, Subtag};
    ///
    /// let subtag: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
    ///
    /// let private = Private::new_single(subtag);
    /// assert_eq!(&private.to_string(), "x-foo");
    /// ```
    pub const fn new_single(input: Subtag) -> Self {
        Self(ShortBoxSlice::new_single(input))
    }

    /// Empties the [`Private`] list.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::private::{Private, Subtag};
    ///
    /// let subtag1: Subtag = "foo".parse().expect("Failed to parse a Subtag.");
    /// let subtag2: Subtag = "bar".parse().expect("Failed to parse a Subtag.");
    /// let mut private = Private::from_vec_unchecked(vec![subtag1, subtag2]);
    ///
    /// assert_eq!(&private.to_string(), "x-foo-bar");
    ///
    /// private.clear();
    ///
    /// assert_eq!(private, Private::new());
    /// ```
    pub fn clear(&mut self) {
        self.0.clear();
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        let keys = iter
            .map(Subtag::try_from_utf8)
            .collect::<Result<ShortBoxSlice<_>, _>>()?;

        if keys.is_empty() {
            Err(ParseError::InvalidExtension)
        } else {
            Ok(Self(keys))
        }
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F, with_ext: bool) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        if self.is_empty() {
            return Ok(());
        }
        if with_ext {
            f(PRIVATE_EXT_STR)?;
        }
        self.deref().iter().map(|t| t.as_str()).try_for_each(f)
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Private {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

writeable::impl_display_with_writeable!(Private, #[cfg(feature = "alloc")]);

impl writeable::Writeable for Private {
    fn write_to<W: core::fmt::Write + ?Sized>(&self, sink: &mut W) -> core::fmt::Result {
        if self.is_empty() {
            return Ok(());
        }
        sink.write_char(PRIVATE_EXT_CHAR)?;
        for key in self.iter() {
            sink.write_char('-')?;
            writeable::Writeable::write_to(key, sink)?;
        }
        Ok(())
    }

    fn writeable_length_hint(&self) -> writeable::LengthHint {
        if self.is_empty() {
            return writeable::LengthHint::exact(0);
        }
        let mut result = writeable::LengthHint::exact(1);
        for key in self.iter() {
            result += writeable::Writeable::writeable_length_hint(key) + 1;
        }
        result
    }
}

impl Deref for Private {
    type Target = [Subtag];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_extension_fromstr() {
        let pe: Private = "x-foo-bar-l-baz".parse().expect("Failed to parse Private");
        assert_eq!(pe.to_string(), "x-foo-bar-l-baz");

        let pe: Result<Private, _> = "x".parse();
        assert!(pe.is_err());
    }
}
