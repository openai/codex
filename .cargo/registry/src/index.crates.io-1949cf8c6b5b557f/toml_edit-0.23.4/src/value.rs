use std::iter::FromIterator;
use std::str::FromStr;

use toml_datetime::{Date, Datetime, Time};

use crate::key::Key;
use crate::repr::{Decor, Formatted};
use crate::{Array, InlineTable, RawString};

/// For [`Key`]/Value pairs under a [`Table`][crate::Table] header or inside another
/// Value
#[derive(Debug, Clone)]
pub enum Value {
    /// A string value.
    String(Formatted<String>),
    /// A 64-bit integer value.
    Integer(Formatted<i64>),
    /// A 64-bit float value.
    Float(Formatted<f64>),
    /// A boolean value.
    Boolean(Formatted<bool>),
    /// An RFC 3339 formatted date-time with offset.
    Datetime(Formatted<Datetime>),
    /// An inline array of values.
    Array(Array),
    /// An inline table of key/value pairs.
    InlineTable(InlineTable),
}

/// Downcasting
impl Value {
    /// Text description of value type
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::String(..) => "string",
            Self::Integer(..) => "integer",
            Self::Float(..) => "float",
            Self::Boolean(..) => "boolean",
            Self::Datetime(..) => "datetime",
            Self::Array(..) => "array",
            Self::InlineTable(..) => "inline table",
        }
    }

    /// Casts `self` to str.
    pub fn as_str(&self) -> Option<&str> {
        match *self {
            Self::String(ref value) => Some(value.value()),
            _ => None,
        }
    }

    /// Returns true if `self` is a string.
    pub fn is_str(&self) -> bool {
        self.as_str().is_some()
    }

    /// Casts `self` to integer.
    pub fn as_integer(&self) -> Option<i64> {
        match *self {
            Self::Integer(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true if `self` is an integer.
    pub fn is_integer(&self) -> bool {
        self.as_integer().is_some()
    }

    /// Casts `self` to float.
    pub fn as_float(&self) -> Option<f64> {
        match *self {
            Self::Float(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true if `self` is a float.
    pub fn is_float(&self) -> bool {
        self.as_float().is_some()
    }

    /// Casts `self` to boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match *self {
            Self::Boolean(ref value) => Some(*value.value()),
            _ => None,
        }
    }

    /// Returns true if `self` is a boolean.
    pub fn is_bool(&self) -> bool {
        self.as_bool().is_some()
    }

    /// Casts `self` to date-time.
    pub fn as_datetime(&self) -> Option<&Datetime> {
        match *self {
            Self::Datetime(ref value) => Some(value.value()),
            _ => None,
        }
    }

    /// Returns true if `self` is a date-time.
    pub fn is_datetime(&self) -> bool {
        self.as_datetime().is_some()
    }

    /// Casts `self` to array.
    pub fn as_array(&self) -> Option<&Array> {
        match *self {
            Self::Array(ref value) => Some(value),
            _ => None,
        }
    }

    /// Casts `self` to mutable array.
    pub fn as_array_mut(&mut self) -> Option<&mut Array> {
        match *self {
            Self::Array(ref mut value) => Some(value),
            _ => None,
        }
    }

    /// Returns true if `self` is an array.
    pub fn is_array(&self) -> bool {
        self.as_array().is_some()
    }

    /// Casts `self` to inline table.
    pub fn as_inline_table(&self) -> Option<&InlineTable> {
        match *self {
            Self::InlineTable(ref value) => Some(value),
            _ => None,
        }
    }

    /// Casts `self` to mutable inline table.
    pub fn as_inline_table_mut(&mut self) -> Option<&mut InlineTable> {
        match *self {
            Self::InlineTable(ref mut value) => Some(value),
            _ => None,
        }
    }

    /// Returns true if `self` is an inline table.
    pub fn is_inline_table(&self) -> bool {
        self.as_inline_table().is_some()
    }
}

impl Value {
    /// Get the decoration of the value.
    /// # Example
    /// ```rust
    /// let v = toml_edit::Value::from(true);
    /// assert_eq!(v.decor().suffix(), None);
    ///```
    pub fn decor_mut(&mut self) -> &mut Decor {
        match self {
            Self::String(f) => f.decor_mut(),
            Self::Integer(f) => f.decor_mut(),
            Self::Float(f) => f.decor_mut(),
            Self::Boolean(f) => f.decor_mut(),
            Self::Datetime(f) => f.decor_mut(),
            Self::Array(a) => a.decor_mut(),
            Self::InlineTable(t) => t.decor_mut(),
        }
    }

    /// Get the decoration of the value.
    /// # Example
    /// ```rust
    /// let v = toml_edit::Value::from(true);
    /// assert_eq!(v.decor().suffix(), None);
    ///```
    pub fn decor(&self) -> &Decor {
        match *self {
            Self::String(ref f) => f.decor(),
            Self::Integer(ref f) => f.decor(),
            Self::Float(ref f) => f.decor(),
            Self::Boolean(ref f) => f.decor(),
            Self::Datetime(ref f) => f.decor(),
            Self::Array(ref a) => a.decor(),
            Self::InlineTable(ref t) => t.decor(),
        }
    }

    /// Sets the prefix and the suffix for value.
    /// # Example
    /// ```rust
    /// # #[cfg(feature = "display")] {
    /// let mut v = toml_edit::Value::from(42);
    /// assert_eq!(&v.to_string(), "42");
    /// let d = v.decorated(" ", " ");
    /// assert_eq!(&d.to_string(), " 42 ");
    /// # }
    /// ```
    pub fn decorated(mut self, prefix: impl Into<RawString>, suffix: impl Into<RawString>) -> Self {
        self.decorate(prefix, suffix);
        self
    }

    pub(crate) fn decorate(&mut self, prefix: impl Into<RawString>, suffix: impl Into<RawString>) {
        let decor = self.decor_mut();
        *decor = Decor::new(prefix, suffix);
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        match self {
            Self::String(f) => f.span(),
            Self::Integer(f) => f.span(),
            Self::Float(f) => f.span(),
            Self::Boolean(f) => f.span(),
            Self::Datetime(f) => f.span(),
            Self::Array(a) => a.span(),
            Self::InlineTable(t) => t.span(),
        }
    }

    pub(crate) fn despan(&mut self, input: &str) {
        match self {
            Self::String(f) => f.despan(input),
            Self::Integer(f) => f.despan(input),
            Self::Float(f) => f.despan(input),
            Self::Boolean(f) => f.despan(input),
            Self::Datetime(f) => f.despan(input),
            Self::Array(a) => a.despan(input),
            Self::InlineTable(t) => t.despan(input),
        }
    }
}

#[cfg(feature = "parse")]
impl FromStr for Value {
    type Err = crate::TomlError;

    /// Parses a value from a &str
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let source = toml_parser::Source::new(s);
        let mut sink = crate::error::TomlSink::<Option<_>>::new(source);
        let mut value = crate::parser::parse_value(source, &mut sink);
        if let Some(err) = sink.into_inner() {
            Err(err)
        } else {
            // Only take the repr and not decor, as its probably not intended
            value.decor_mut().clear();
            value.despan(s);
            Ok(value)
        }
    }
}

impl<'b> From<&'b Self> for Value {
    fn from(s: &'b Self) -> Self {
        s.clone()
    }
}

impl<'b> From<&'b str> for Value {
    fn from(s: &'b str) -> Self {
        s.to_owned().into()
    }
}

impl<'b> From<&'b String> for Value {
    fn from(s: &'b String) -> Self {
        s.to_owned().into()
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(Formatted::new(s))
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::Integer(Formatted::new(i))
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        // Preserve sign of NaN. It may get written to TOML as `-nan`.
        Self::Float(Formatted::new(f))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Boolean(Formatted::new(b))
    }
}

impl From<Datetime> for Value {
    fn from(d: Datetime) -> Self {
        Self::Datetime(Formatted::new(d))
    }
}

impl From<Date> for Value {
    fn from(d: Date) -> Self {
        let d: Datetime = d.into();
        d.into()
    }
}

impl From<Time> for Value {
    fn from(d: Time) -> Self {
        let d: Datetime = d.into();
        d.into()
    }
}

impl From<Array> for Value {
    fn from(array: Array) -> Self {
        Self::Array(array)
    }
}

impl From<InlineTable> for Value {
    fn from(table: InlineTable) -> Self {
        Self::InlineTable(table)
    }
}

impl<V: Into<Self>> FromIterator<V> for Value {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = V>,
    {
        let array: Array = iter.into_iter().collect();
        Self::Array(array)
    }
}

impl<K: Into<Key>, V: Into<Self>> FromIterator<(K, V)> for Value {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
    {
        let table: InlineTable = iter.into_iter().collect();
        Self::InlineTable(table)
    }
}

#[cfg(feature = "display")]
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::encode::encode_value(self, f, None, ("", ""))
    }
}

// `key1 = value1`
pub(crate) const DEFAULT_VALUE_DECOR: (&str, &str) = (" ", "");
// `{ key = value }`
pub(crate) const DEFAULT_TRAILING_VALUE_DECOR: (&str, &str) = (" ", " ");
// `[value1, value2]`
pub(crate) const DEFAULT_LEADING_VALUE_DECOR: (&str, &str) = ("", "");

#[cfg(test)]
#[cfg(feature = "parse")]
#[cfg(feature = "display")]
mod tests {
    use super::*;

    #[test]
    fn from_iter_formatting() {
        let features = ["node".to_owned(), "mouth".to_owned()];
        let features: Value = features.iter().cloned().collect();
        assert_eq!(features.to_string(), r#"["node", "mouth"]"#);
    }
}

#[test]
#[cfg(feature = "parse")]
#[cfg(feature = "display")]
fn string_roundtrip() {
    Value::from("hello").to_string().parse::<Value>().unwrap();
}
