// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::parser::ParseError;
#[cfg(feature = "alloc")]
use crate::parser::SubtagIterator;
use crate::shortvec::ShortBoxSlice;
use crate::subtags::{subtag, Subtag};
use core::ops::RangeInclusive;
#[cfg(feature = "alloc")]
use core::str::FromStr;

/// A value used in a list of [`Fields`](super::Fields).
///
/// The value has to be a sequence of one or more alphanumerical strings
/// separated by `-`.
/// Each part of the sequence has to be no shorter than three characters and no
/// longer than 8.
///
/// # Examples
///
/// ```
/// use icu::locale::extensions::transform::Value;
///
/// "hybrid".parse::<Value>().expect("Valid Value.");
///
/// "hybrid-foobar".parse::<Value>().expect("Valid Value.");
///
/// "no".parse::<Value>().expect_err("Invalid Value.");
/// ```
#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord, Default)]
pub struct Value(ShortBoxSlice<Subtag>);

#[allow(dead_code)]
const TYPE_LENGTH: RangeInclusive<usize> = 3..=8;
const TRUE_TVALUE: Subtag = subtag!("true");

impl Value {
    /// A constructor which takes a str slice, parses it and
    /// produces a well-formed [`Value`].
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    ///
    /// # Examples
    ///
    /// ```
    /// use icu::locale::extensions::transform::Value;
    ///
    /// let value = Value::try_from_str("hybrid").expect("Parsing failed.");
    /// ```
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
        let mut v = ShortBoxSlice::default();
        let mut has_value = false;

        for subtag in SubtagIterator::new(code_units) {
            if !Self::is_type_subtag(subtag) {
                return Err(ParseError::InvalidExtension);
            }
            has_value = true;
            let val = Subtag::try_from_utf8(subtag).map_err(|_| ParseError::InvalidExtension)?;
            if val != TRUE_TVALUE {
                v.push(val);
            }
        }

        if !has_value {
            return Err(ParseError::InvalidExtension);
        }
        Ok(Self(v))
    }

    #[allow(dead_code)]
    pub(crate) fn from_short_slice_unchecked(input: ShortBoxSlice<Subtag>) -> Self {
        Self(input)
    }

    #[allow(dead_code)]
    pub(crate) fn is_type_subtag(t: &[u8]) -> bool {
        TYPE_LENGTH.contains(&t.len()) && t.iter().all(u8::is_ascii_alphanumeric)
    }

    #[allow(dead_code)]
    pub(crate) fn parse_subtag(t: &[u8]) -> Result<Option<Subtag>, ParseError> {
        if !TYPE_LENGTH.contains(&t.len()) {
            return Err(ParseError::InvalidExtension);
        }
        let s = Subtag::try_from_utf8(t).map_err(|_| ParseError::InvalidSubtag)?;

        let s = s.to_ascii_lowercase();

        if s == TRUE_TVALUE {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    pub(crate) fn for_each_subtag_str<E, F>(&self, f: &mut F) -> Result<(), E>
    where
        F: FnMut(&str) -> Result<(), E>,
    {
        if self.0.is_empty() {
            f(TRUE_TVALUE.as_str())?;
        } else {
            self.0.iter().map(Subtag::as_str).try_for_each(f)?;
        }
        Ok(())
    }
}

/// ✨ *Enabled with the `alloc` Cargo feature.*
#[cfg(feature = "alloc")]
impl FromStr for Value {
    type Err = ParseError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from_str(s)
    }
}

impl_writeable_for_each_subtag_str_no_test!(Value, selff, selff.0.is_empty() => Some("true"));

#[test]
fn test_writeable() {
    use writeable::assert_writeable_eq;

    let hybrid = "hybrid".parse().unwrap();
    let foobar = "foobar".parse().unwrap();

    assert_writeable_eq!(Value::default(), "true");
    assert_writeable_eq!(
        Value::from_short_slice_unchecked(vec![hybrid].into()),
        "hybrid"
    );
    assert_writeable_eq!(
        Value::from_short_slice_unchecked(vec![hybrid, foobar].into()),
        "hybrid-foobar"
    );
}

#[test]
fn test_short_tvalue() {
    let value = Value::try_from_str("foo-longstag");
    assert!(value.is_ok());
    let value = value.unwrap();
    assert_eq!(value.0.len(), 2);
    for (s, reference) in value.0.iter().zip(&[subtag!("foo"), subtag!("longstag")]) {
        assert_eq!(s, reference);
    }

    let value = Value::try_from_str("foo-ba");
    assert!(value.is_err());
}
