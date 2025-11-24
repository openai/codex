use std::borrow::Cow;

use crate::RawString;

/// A scalar TOML [`Value`][crate::Value]'s logical value and its representation in a `&str`
///
/// This includes the surrounding whitespace and comments.
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Formatted<T> {
    value: T,
    repr: Option<Repr>,
    decor: Decor,
}

impl<T> Formatted<T>
where
    T: ValueRepr,
{
    /// Default-formatted value
    pub fn new(value: T) -> Self {
        Self {
            value,
            repr: None,
            decor: Default::default(),
        }
    }

    pub(crate) fn set_repr_unchecked(&mut self, repr: Repr) {
        self.repr = Some(repr);
    }

    /// The wrapped value
    pub fn value(&self) -> &T {
        &self.value
    }

    /// The wrapped value
    pub fn into_value(self) -> T {
        self.value
    }

    /// Returns the raw representation, if available.
    pub fn as_repr(&self) -> Option<&Repr> {
        self.repr.as_ref()
    }

    /// Returns the default raw representation.
    #[cfg(feature = "display")]
    pub fn default_repr(&self) -> Repr {
        self.value.to_repr()
    }

    /// Returns a raw representation.
    #[cfg(feature = "display")]
    pub fn display_repr(&self) -> Cow<'_, str> {
        self.as_repr()
            .and_then(|r| r.as_raw().as_str())
            .map(Cow::Borrowed)
            .unwrap_or_else(|| {
                Cow::Owned(self.default_repr().as_raw().as_str().unwrap().to_owned())
            })
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.repr.as_ref().and_then(|r| r.span())
    }

    pub(crate) fn despan(&mut self, input: &str) {
        self.decor.despan(input);
        if let Some(repr) = &mut self.repr {
            repr.despan(input);
        }
    }

    /// Returns the surrounding whitespace
    pub fn decor_mut(&mut self) -> &mut Decor {
        &mut self.decor
    }

    /// Returns the surrounding whitespace
    pub fn decor(&self) -> &Decor {
        &self.decor
    }

    /// Auto formats the value.
    pub fn fmt(&mut self) {
        self.repr = None;
    }
}

impl<T> std::fmt::Debug for Formatted<T>
where
    T: std::fmt::Debug,
{
    #[inline]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut d = formatter.debug_struct("Formatted");
        d.field("value", &self.value);
        match &self.repr {
            Some(r) => d.field("repr", r),
            None => d.field("repr", &"default"),
        };
        d.field("decor", &self.decor);
        d.finish()
    }
}

#[cfg(feature = "display")]
impl<T> std::fmt::Display for Formatted<T>
where
    T: ValueRepr,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        crate::encode::encode_formatted(self, f, None, ("", ""))
    }
}

pub trait ValueRepr: crate::private::Sealed {
    /// The TOML representation of the value
    #[cfg(feature = "display")]
    fn to_repr(&self) -> Repr;
}

#[cfg(not(feature = "display"))]
mod inner {
    use super::ValueRepr;

    impl ValueRepr for String {}
    impl ValueRepr for i64 {}
    impl ValueRepr for f64 {}
    impl ValueRepr for bool {}
    impl ValueRepr for toml_datetime::Datetime {}
}

/// A TOML [`Value`][crate::Value] encoded as a `&str`
#[derive(Eq, PartialEq, Clone, Hash)]
pub struct Repr {
    raw_value: RawString,
}

impl Repr {
    pub(crate) fn new_unchecked(raw: impl Into<RawString>) -> Self {
        Self {
            raw_value: raw.into(),
        }
    }

    /// Access the underlying value
    pub fn as_raw(&self) -> &RawString {
        &self.raw_value
    }

    /// The location within the original document
    ///
    /// This generally requires a [`Document`][crate::Document].
    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        self.raw_value.span()
    }

    pub(crate) fn despan(&mut self, input: &str) {
        self.raw_value.despan(input);
    }

    #[cfg(feature = "display")]
    pub(crate) fn encode(&self, buf: &mut dyn std::fmt::Write, input: &str) -> std::fmt::Result {
        self.as_raw().encode(buf, input)
    }
}

impl std::fmt::Debug for Repr {
    #[inline]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        self.raw_value.fmt(formatter)
    }
}

/// A prefix and suffix,
///
/// Including comments, whitespaces and newlines.
#[derive(Eq, PartialEq, Clone, Default, Hash)]
pub struct Decor {
    prefix: Option<RawString>,
    suffix: Option<RawString>,
}

impl Decor {
    /// Creates a new decor from the given prefix and suffix.
    pub fn new(prefix: impl Into<RawString>, suffix: impl Into<RawString>) -> Self {
        Self {
            prefix: Some(prefix.into()),
            suffix: Some(suffix.into()),
        }
    }

    /// Go back to default decor
    pub fn clear(&mut self) {
        self.prefix = None;
        self.suffix = None;
    }

    /// Get the prefix.
    pub fn prefix(&self) -> Option<&RawString> {
        self.prefix.as_ref()
    }

    #[cfg(feature = "display")]
    pub(crate) fn prefix_encode(
        &self,
        buf: &mut dyn std::fmt::Write,
        input: Option<&str>,
        default: &str,
    ) -> std::fmt::Result {
        if let Some(prefix) = self.prefix() {
            prefix.encode_with_default(buf, input, default)
        } else {
            write!(buf, "{default}")
        }
    }

    /// Set the prefix.
    pub fn set_prefix(&mut self, prefix: impl Into<RawString>) {
        self.prefix = Some(prefix.into());
    }

    /// Get the suffix.
    pub fn suffix(&self) -> Option<&RawString> {
        self.suffix.as_ref()
    }

    #[cfg(feature = "display")]
    pub(crate) fn suffix_encode(
        &self,
        buf: &mut dyn std::fmt::Write,
        input: Option<&str>,
        default: &str,
    ) -> std::fmt::Result {
        if let Some(suffix) = self.suffix() {
            suffix.encode_with_default(buf, input, default)
        } else {
            write!(buf, "{default}")
        }
    }

    /// Set the suffix.
    pub fn set_suffix(&mut self, suffix: impl Into<RawString>) {
        self.suffix = Some(suffix.into());
    }

    pub(crate) fn despan(&mut self, input: &str) {
        if let Some(prefix) = &mut self.prefix {
            prefix.despan(input);
        }
        if let Some(suffix) = &mut self.suffix {
            suffix.despan(input);
        }
    }
}

impl std::fmt::Debug for Decor {
    #[inline]
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut d = formatter.debug_struct("Decor");
        match &self.prefix {
            Some(r) => d.field("prefix", r),
            None => d.field("prefix", &"default"),
        };
        match &self.suffix {
            Some(r) => d.field("suffix", r),
            None => d.field("suffix", &"default"),
        };
        d.finish()
    }
}
