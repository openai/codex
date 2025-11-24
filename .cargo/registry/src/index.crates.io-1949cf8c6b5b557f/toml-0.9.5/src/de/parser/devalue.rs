//! Definition of a TOML [value][DeValue] for deserialization

use alloc::borrow::Cow;
use core::mem::discriminant;
use core::ops;

use serde_spanned::Spanned;
use toml_datetime::Datetime;

use crate::alloc_prelude::*;
use crate::de::DeArray;
use crate::de::DeTable;

/// Type representing a TOML string, payload of the `DeValue::String` variant
pub type DeString<'i> = Cow<'i, str>;

/// Represents a TOML integer
#[derive(Clone, Debug)]
pub struct DeInteger<'i> {
    pub(crate) inner: DeString<'i>,
    pub(crate) radix: u32,
}

impl DeInteger<'_> {
    pub(crate) fn to_u64(&self) -> Option<u64> {
        u64::from_str_radix(self.inner.as_ref(), self.radix).ok()
    }
    pub(crate) fn to_i64(&self) -> Option<i64> {
        i64::from_str_radix(self.inner.as_ref(), self.radix).ok()
    }
    pub(crate) fn to_u128(&self) -> Option<u128> {
        u128::from_str_radix(self.inner.as_ref(), self.radix).ok()
    }
    pub(crate) fn to_i128(&self) -> Option<i128> {
        i128::from_str_radix(self.inner.as_ref(), self.radix).ok()
    }

    /// [`from_str_radix`][i64::from_str_radix]-compatible representation of an integer
    ///
    /// Requires [`DeInteger::radix`] to interpret
    ///
    /// See [`Display`][std::fmt::Display] for a representation that includes the radix
    pub fn as_str(&self) -> &str {
        self.inner.as_ref()
    }

    /// Numeric base of [`DeInteger::as_str`]
    pub fn radix(&self) -> u32 {
        self.radix
    }
}

impl Default for DeInteger<'_> {
    fn default() -> Self {
        Self {
            inner: DeString::Borrowed("0"),
            radix: 10,
        }
    }
}

impl core::fmt::Display for DeInteger<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.radix {
            2 => "0b".fmt(formatter)?,
            8 => "0o".fmt(formatter)?,
            10 => {}
            16 => "0x".fmt(formatter)?,
            _ => {
                unreachable!(
                    "we should only ever have 2, 8, 10, and 16 radix, not {}",
                    self.radix
                )
            }
        }
        self.as_str().fmt(formatter)?;
        Ok(())
    }
}

/// Represents a TOML integer
#[derive(Clone, Debug)]
pub struct DeFloat<'i> {
    pub(crate) inner: DeString<'i>,
}

impl DeFloat<'_> {
    pub(crate) fn to_f64(&self) -> Option<f64> {
        let f: f64 = self.inner.as_ref().parse().ok()?;
        if f.is_infinite() && !self.as_str().contains("inf") {
            None
        } else {
            Some(f)
        }
    }

    /// [`FromStr`][std::str::FromStr]-compatible representation of a float
    pub fn as_str(&self) -> &str {
        self.inner.as_ref()
    }
}

impl Default for DeFloat<'_> {
    fn default() -> Self {
        Self {
            inner: DeString::Borrowed("0.0"),
        }
    }
}

impl core::fmt::Display for DeFloat<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.as_str().fmt(formatter)?;
        Ok(())
    }
}

/// Representation of a TOML value.
#[derive(Clone, Debug)]
pub enum DeValue<'i> {
    /// Represents a TOML string
    String(DeString<'i>),
    /// Represents a TOML integer
    Integer(DeInteger<'i>),
    /// Represents a TOML float
    Float(DeFloat<'i>),
    /// Represents a TOML boolean
    Boolean(bool),
    /// Represents a TOML datetime
    Datetime(Datetime),
    /// Represents a TOML array
    Array(DeArray<'i>),
    /// Represents a TOML table
    Table(DeTable<'i>),
}

impl<'i> DeValue<'i> {
    /// Parse a TOML value
    pub fn parse(input: &'i str) -> Result<Spanned<Self>, crate::de::Error> {
        let source = toml_parser::Source::new(input);
        let mut errors = crate::de::error::TomlSink::<Option<_>>::new(source);
        let value = crate::de::parser::parse_value(source, &mut errors);
        if let Some(err) = errors.into_inner() {
            Err(err)
        } else {
            Ok(value)
        }
    }

    /// Parse a TOML value, with best effort recovery on error
    pub fn parse_recoverable(input: &'i str) -> (Spanned<Self>, Vec<crate::de::Error>) {
        let source = toml_parser::Source::new(input);
        let mut errors = crate::de::error::TomlSink::<Vec<_>>::new(source);
        let value = crate::de::parser::parse_value(source, &mut errors);
        (value, errors.into_inner())
    }

    /// Ensure no data is borrowed
    pub fn make_owned(&mut self) {
        match self {
            DeValue::String(v) => {
                let owned = core::mem::take(v);
                *v = Cow::Owned(owned.into_owned());
            }
            DeValue::Integer(..)
            | DeValue::Float(..)
            | DeValue::Boolean(..)
            | DeValue::Datetime(..) => {}
            DeValue::Array(v) => {
                for e in v.iter_mut() {
                    e.get_mut().make_owned();
                }
            }
            DeValue::Table(v) => v.make_owned(),
        }
    }

    /// Index into a TOML array or map. A string index can be used to access a
    /// value in a map, and a usize index can be used to access an element of an
    /// array.
    ///
    /// Returns `None` if the type of `self` does not match the type of the
    /// index, for example if the index is a string and `self` is an array or a
    /// number. Also returns `None` if the given key does not exist in the map
    /// or the given index is not within the bounds of the array.
    pub fn get<I: Index>(&self, index: I) -> Option<&Spanned<Self>> {
        index.index(self)
    }

    /// Extracts the integer value if it is an integer.
    pub fn as_integer(&self) -> Option<&DeInteger<'i>> {
        match self {
            DeValue::Integer(i) => Some(i),
            _ => None,
        }
    }

    /// Tests whether this value is an integer.
    pub fn is_integer(&self) -> bool {
        self.as_integer().is_some()
    }

    /// Extracts the float value if it is a float.
    pub fn as_float(&self) -> Option<&DeFloat<'i>> {
        match self {
            DeValue::Float(f) => Some(f),
            _ => None,
        }
    }

    /// Tests whether this value is a float.
    pub fn is_float(&self) -> bool {
        self.as_float().is_some()
    }

    /// Extracts the boolean value if it is a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            DeValue::Boolean(b) => Some(b),
            _ => None,
        }
    }

    /// Tests whether this value is a boolean.
    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
    }

    /// Extracts the string of this value if it is a string.
    pub fn as_str(&self) -> Option<&str> {
        match *self {
            DeValue::String(ref s) => Some(&**s),
            _ => None,
        }
    }

    /// Tests if this value is a string.
    pub fn is_str(&self) -> bool {
        self.as_str().is_some()
    }

    /// Extracts the datetime value if it is a datetime.
    ///
    /// Note that a parsed TOML value will only contain ISO 8601 dates. An
    /// example date is:
    ///
    /// ```notrust
    /// 1979-05-27T07:32:00Z
    /// ```
    pub fn as_datetime(&self) -> Option<&Datetime> {
        match *self {
            DeValue::Datetime(ref s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is a datetime.
    pub fn is_datetime(&self) -> bool {
        self.as_datetime().is_some()
    }

    /// Extracts the array value if it is an array.
    pub fn as_array(&self) -> Option<&DeArray<'i>> {
        match *self {
            DeValue::Array(ref s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_array_mut(&mut self) -> Option<&mut DeArray<'i>> {
        match self {
            DeValue::Array(s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is an array.
    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    /// Extracts the table value if it is a table.
    pub fn as_table(&self) -> Option<&DeTable<'i>> {
        match *self {
            DeValue::Table(ref s) => Some(s),
            _ => None,
        }
    }

    pub(crate) fn as_table_mut(&mut self) -> Option<&mut DeTable<'i>> {
        match self {
            DeValue::Table(s) => Some(s),
            _ => None,
        }
    }

    /// Tests whether this value is a table.
    pub fn is_table(&self) -> bool {
        self.as_table().is_some()
    }

    /// Tests whether this and another value have the same type.
    pub fn same_type(&self, other: &DeValue<'_>) -> bool {
        discriminant(self) == discriminant(other)
    }

    /// Returns a human-readable representation of the type of this value.
    pub fn type_str(&self) -> &'static str {
        match *self {
            DeValue::String(..) => "string",
            DeValue::Integer(..) => "integer",
            DeValue::Float(..) => "float",
            DeValue::Boolean(..) => "boolean",
            DeValue::Datetime(..) => "datetime",
            DeValue::Array(..) => "array",
            DeValue::Table(..) => "table",
        }
    }
}

impl<I> ops::Index<I> for DeValue<'_>
where
    I: Index,
{
    type Output = Spanned<Self>;

    fn index(&self, index: I) -> &Spanned<Self> {
        self.get(index).expect("index not found")
    }
}

/// Types that can be used to index a `toml::Value`
///
/// Currently this is implemented for `usize` to index arrays and `str` to index
/// tables.
///
/// This trait is sealed and not intended for implementation outside of the
/// `toml` crate.
pub trait Index: Sealed {
    #[doc(hidden)]
    fn index<'r, 'i>(&self, val: &'r DeValue<'i>) -> Option<&'r Spanned<DeValue<'i>>>;
}

/// An implementation detail that should not be implemented, this will change in
/// the future and break code otherwise.
#[doc(hidden)]
pub trait Sealed {}
impl Sealed for usize {}
impl Sealed for str {}
impl Sealed for String {}
impl<T: Sealed + ?Sized> Sealed for &T {}

impl Index for usize {
    fn index<'r, 'i>(&self, val: &'r DeValue<'i>) -> Option<&'r Spanned<DeValue<'i>>> {
        match *val {
            DeValue::Array(ref a) => a.get(*self),
            _ => None,
        }
    }
}

impl Index for str {
    fn index<'r, 'i>(&self, val: &'r DeValue<'i>) -> Option<&'r Spanned<DeValue<'i>>> {
        match *val {
            DeValue::Table(ref a) => a.get(self),
            _ => None,
        }
    }
}

impl Index for String {
    fn index<'r, 'i>(&self, val: &'r DeValue<'i>) -> Option<&'r Spanned<DeValue<'i>>> {
        self[..].index(val)
    }
}

impl<T> Index for &T
where
    T: Index + ?Sized,
{
    fn index<'r, 'i>(&self, val: &'r DeValue<'i>) -> Option<&'r Spanned<DeValue<'i>>> {
        (**self).index(val)
    }
}
