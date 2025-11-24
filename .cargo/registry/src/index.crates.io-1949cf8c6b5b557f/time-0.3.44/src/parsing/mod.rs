//! Parsing for various types.

pub(crate) mod combinator;
pub(crate) mod component;
mod iso8601;
pub(crate) mod parsable;
mod parsed;
pub(crate) mod shim;

pub use self::parsable::Parsable;
pub use self::parsed::Parsed;

/// An item that has been parsed. Represented as a `(remaining, value)` pair.
#[derive(Debug)]
pub(crate) struct ParsedItem<'a, T>(pub(crate) &'a [u8], pub(crate) T);

impl<'a, T> ParsedItem<'a, T> {
    /// Map the value to a new value, preserving the remaining input.
    #[inline]
    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> ParsedItem<'a, U> {
        ParsedItem(self.0, f(self.1))
    }

    /// Map the value to a new, optional value, preserving the remaining input.
    #[inline]
    pub(crate) fn flat_map<U>(self, f: impl FnOnce(T) -> Option<U>) -> Option<ParsedItem<'a, U>> {
        Some(ParsedItem(self.0, f(self.1)?))
    }

    /// Consume the stored value with the provided function. The remaining input is returned.
    #[must_use = "this returns the remaining input"]
    #[inline]
    pub(crate) fn consume_value(self, f: impl FnOnce(T) -> Option<()>) -> Option<&'a [u8]> {
        f(self.1)?;
        Some(self.0)
    }

    /// Filter the value with the provided function. If the function returns `false`, the value
    /// is discarded and `None` is returned. Otherwise, the value is preserved and `Some(self)` is
    /// returned.
    #[inline]
    pub(crate) fn filter(self, f: impl FnOnce(&T) -> bool) -> Option<Self> {
        f(&self.1).then_some(self)
    }
}

impl<'a> ParsedItem<'a, ()> {
    /// Discard the unit value, returning the remaining input.
    #[must_use = "this returns the remaining input"]
    #[inline]
    pub(crate) const fn into_inner(self) -> &'a [u8] {
        self.0
    }
}

impl<'a> ParsedItem<'a, Option<()>> {
    /// Discard the potential unit value, returning the remaining input.
    #[must_use = "this returns the remaining input"]
    #[inline]
    pub(crate) const fn into_inner(self) -> &'a [u8] {
        self.0
    }
}
