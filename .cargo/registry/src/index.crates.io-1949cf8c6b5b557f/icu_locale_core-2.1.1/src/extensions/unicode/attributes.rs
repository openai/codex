// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::Attribute;

#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::shortvec::ShortBoxSlice;
#[cfg(feature = "alloc")]
use crate::ParseError;
#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::ops::Deref;
#[cfg(feature = "alloc")]
use core::str::FromStr;

/// A set of [`Attribute`] elements as defined in [`Unicode Extension Attributes`].
///
/// [`Unicode Extension Attributes`]: https://unicode.org/reports/tr35/tr35.html#u_Extension
///
/// # Examples
///
/// ```
/// use icu::locale::extensions::unicode::{Attribute, Attributes};
///
/// let attribute1: Attribute =
///     "foobar".parse().expect("Failed to parse a variant subtag.");
///
/// let attribute2: Attribute = "testing"
///     .parse()
///     .expect("Failed to parse a variant subtag.");
/// let mut v = vec![attribute1, attribute2];
/// v.sort();
/// v.dedup();
///
/// let attributes: Attributes = Attributes::from_vec_unchecked(v);
/// assert_eq!(attributes.to_string(), "foobar-testing");
/// ```
#[derive(Default, Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct Attributes(ShortBoxSlice<Attribute>);

impl Attributes {
    /// Returns a new empty set of attributes. Same as [`default()`](Default::default()), but is `const`.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Attributes;
    ///
    /// assert_eq!(Attributes::new(), Attributes::default());
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self(ShortBoxSlice::new())
    }

    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Attributes`].
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
        Self::try_from_iter(&mut iter)
    }

    /// A constructor which takes a pre-sorted list of [`Attribute`] elements.
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{Attribute, Attributes};
    ///
    /// let attribute1: Attribute = "foobar".parse().expect("Parsing failed.");
    /// let attribute2: Attribute = "testing".parse().expect("Parsing failed.");
    /// let mut v = vec![attribute1, attribute2];
    /// v.sort();
    /// v.dedup();
    ///
    /// let attributes = Attributes::from_vec_unchecked(v);
    /// ```
    ///
    /// Notice: For performance- and memory-constrained environments, it is recommended
    /// for the caller to use [`binary_search`](slice::binary_search) instead of [`sort`](slice::sort)
    /// and [`dedup`](Vec::dedup()).
    #[cfg(feature = "alloc")]
    pub fn from_vec_unchecked(input: Vec<Attribute>) -> Self {
        Self(input.into())
    }

    /// Empties the [`Attributes`] list.
    ///
    /// Returns the old list.
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::unicode::{attribute, Attributes};
    /// use writeable::assert_writeable_eq;
    ///
    /// let mut attributes = Attributes::from_vec_unchecked(vec![
    ///     attribute!("foobar"),
    ///     attribute!("testing"),
    /// ]);
    ///
    /// assert_writeable_eq!(attributes, "foobar-testing");
    ///
    /// attributes.clear();
    ///
    /// assert_writeable_eq!(attributes, "");
    /// ```
    pub fn clear(&mut self) -> Self {
        core::mem::take(self)
    }

    #[cfg(feature = "alloc")]
    pub(crate) fn try_from_iter(iter: &mut SubtagIterator) -> Result<Self, ParseError> {
        let mut attributes = ShortBoxSlice::new();

        while let Some(subtag) = iter.peek() {
            if let Ok(attr) = Attribute::try_from_utf8(subtag) {
                if let Err(idx) = attributes.binary_search(&attr) {
                    attributes.insert(idx, attr);
                }
            } else {
                break;
            }
            iter.next();
        }
        Ok(Self(attributes))
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        self.deref().iter().map(|t| t.as_str()).try_for_each(f)
    }

    /// Extends the `Attributes` with values from another `Attributes`.
    ///
    /// # Example
    ///
    /// ```
    /// use icu::locale::extensions::unicode::Attributes;
    ///
    /// let mut attrs: Attributes = "foobar-foobaz".parse().unwrap();
    /// let attrs2: Attributes = "foobar-fooqux".parse().unwrap();
    ///
    /// attrs.extend_from_attributes(attrs2);
    ///
    /// assert_eq!(attrs, "foobar-foobaz-fooqux".parse().unwrap());
    /// ```
    #[cfg(feature = "alloc")]
    pub fn extend_from_attributes(&mut self, other: Attributes) {
        for attr in other.0 {
            if let Err(idx) = self.binary_search(&attr) {
                self.0.insert(idx, attr);
            }
        }
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Attributes {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

impl_writeable_for_subtag_list!(Attributes, "foobar", "testing");

impl Deref for Attributes {
    type Target = [Attribute];

    fn deref(&self) -> &[Attribute] {
        self.0.deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_fromstr() {
        let attrs: Attributes = "foo-bar".parse().expect("Failed to parse Attributes");
        assert_eq!(attrs.to_string(), "bar-foo");
    }
}
