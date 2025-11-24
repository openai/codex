//! A format item with borrowed data.

#[cfg(feature = "alloc")]
use alloc::string::String;
#[cfg(feature = "alloc")]
use core::fmt;

use crate::error;
use crate::format_description::Component;

/// A complete description of how to format and parse a type.
#[non_exhaustive]
#[cfg_attr(not(feature = "alloc"), derive(Debug))]
#[derive(Clone, PartialEq, Eq)]
pub enum BorrowedFormatItem<'a> {
    /// Bytes that are formatted as-is.
    ///
    /// **Note**: These bytes **should** be UTF-8, but are not required to be. The value is passed
    /// through `String::from_utf8_lossy` when necessary.
    Literal(&'a [u8]),
    /// A minimal representation of a single non-literal item.
    Component(Component),
    /// A series of literals or components that collectively form a partial or complete
    /// description.
    Compound(&'a [Self]),
    /// A `FormatItem` that may or may not be present when parsing. If parsing fails, there
    /// will be no effect on the resulting `struct`.
    ///
    /// This variant has no effect on formatting, as the value is guaranteed to be present.
    Optional(&'a Self),
    /// A series of `FormatItem`s where, when parsing, the first successful parse is used. When
    /// formatting, the first element of the slice is used.  An empty slice is a no-op when
    /// formatting or parsing.
    First(&'a [Self]),
}

#[cfg(feature = "alloc")]
impl fmt::Debug for BorrowedFormatItem<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Literal(literal) => f.write_str(&String::from_utf8_lossy(literal)),
            Self::Component(component) => component.fmt(f),
            Self::Compound(compound) => compound.fmt(f),
            Self::Optional(item) => f.debug_tuple("Optional").field(item).finish(),
            Self::First(items) => f.debug_tuple("First").field(items).finish(),
        }
    }
}

impl From<Component> for BorrowedFormatItem<'_> {
    #[inline]
    fn from(component: Component) -> Self {
        Self::Component(component)
    }
}

impl TryFrom<BorrowedFormatItem<'_>> for Component {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(value: BorrowedFormatItem<'_>) -> Result<Self, Self::Error> {
        match value {
            BorrowedFormatItem::Component(component) => Ok(component),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl<'a> From<&'a [BorrowedFormatItem<'_>]> for BorrowedFormatItem<'a> {
    #[inline]
    fn from(items: &'a [BorrowedFormatItem<'_>]) -> Self {
        Self::Compound(items)
    }
}

impl<'a> TryFrom<BorrowedFormatItem<'a>> for &[BorrowedFormatItem<'a>] {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(value: BorrowedFormatItem<'a>) -> Result<Self, Self::Error> {
        match value {
            BorrowedFormatItem::Compound(items) => Ok(items),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl PartialEq<Component> for BorrowedFormatItem<'_> {
    #[inline]
    fn eq(&self, rhs: &Component) -> bool {
        matches!(self, Self::Component(component) if component == rhs)
    }
}

impl PartialEq<BorrowedFormatItem<'_>> for Component {
    #[inline]
    fn eq(&self, rhs: &BorrowedFormatItem<'_>) -> bool {
        rhs == self
    }
}

impl PartialEq<&[Self]> for BorrowedFormatItem<'_> {
    #[inline]
    fn eq(&self, rhs: &&[Self]) -> bool {
        matches!(self, Self::Compound(compound) if compound == rhs)
    }
}

impl PartialEq<BorrowedFormatItem<'_>> for &[BorrowedFormatItem<'_>] {
    #[inline]
    fn eq(&self, rhs: &BorrowedFormatItem<'_>) -> bool {
        rhs == self
    }
}
