//! A format item with owned data.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::error;
use crate::format_description::{BorrowedFormatItem, Component};

/// A complete description of how to format and parse a type.
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq)]
pub enum OwnedFormatItem {
    /// Bytes that are formatted as-is.
    ///
    /// **Note**: These bytes **should** be UTF-8, but are not required to be. The value is passed
    /// through `String::from_utf8_lossy` when necessary.
    Literal(Box<[u8]>),
    /// A minimal representation of a single non-literal item.
    Component(Component),
    /// A series of literals or components that collectively form a partial or complete
    /// description.
    Compound(Box<[Self]>),
    /// A `FormatItem` that may or may not be present when parsing. If parsing fails, there
    /// will be no effect on the resulting `struct`.
    ///
    /// This variant has no effect on formatting, as the value is guaranteed to be present.
    Optional(Box<Self>),
    /// A series of `FormatItem`s where, when parsing, the first successful parse is used. When
    /// formatting, the first element of the [`Vec`] is used. An empty [`Vec`] is a no-op when
    /// formatting or parsing.
    First(Box<[Self]>),
}

impl fmt::Debug for OwnedFormatItem {
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

impl From<BorrowedFormatItem<'_>> for OwnedFormatItem {
    #[inline]
    fn from(item: BorrowedFormatItem<'_>) -> Self {
        (&item).into()
    }
}

impl From<&BorrowedFormatItem<'_>> for OwnedFormatItem {
    #[inline]
    fn from(item: &BorrowedFormatItem<'_>) -> Self {
        match item {
            BorrowedFormatItem::Literal(literal) => {
                Self::Literal(literal.to_vec().into_boxed_slice())
            }
            BorrowedFormatItem::Component(component) => Self::Component(*component),
            BorrowedFormatItem::Compound(compound) => Self::Compound(
                compound
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            BorrowedFormatItem::Optional(item) => Self::Optional(Box::new((*item).into())),
            BorrowedFormatItem::First(items) => Self::First(
                items
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }
    }
}

impl From<Vec<BorrowedFormatItem<'_>>> for OwnedFormatItem {
    #[inline]
    fn from(items: Vec<BorrowedFormatItem<'_>>) -> Self {
        items.as_slice().into()
    }
}

impl<'a, T: AsRef<[BorrowedFormatItem<'a>]> + ?Sized> From<&T> for OwnedFormatItem {
    #[inline]
    fn from(items: &T) -> Self {
        Self::Compound(
            items
                .as_ref()
                .iter()
                .cloned()
                .map(Into::into)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )
    }
}

impl From<Component> for OwnedFormatItem {
    #[inline]
    fn from(component: Component) -> Self {
        Self::Component(component)
    }
}

impl TryFrom<OwnedFormatItem> for Component {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(value: OwnedFormatItem) -> Result<Self, Self::Error> {
        match value {
            OwnedFormatItem::Component(component) => Ok(component),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl From<Vec<Self>> for OwnedFormatItem {
    #[inline]
    fn from(items: Vec<Self>) -> Self {
        Self::Compound(items.into_boxed_slice())
    }
}

impl TryFrom<OwnedFormatItem> for Vec<OwnedFormatItem> {
    type Error = error::DifferentVariant;

    #[inline]
    fn try_from(value: OwnedFormatItem) -> Result<Self, Self::Error> {
        match value {
            OwnedFormatItem::Compound(items) => Ok(items.into_vec()),
            _ => Err(error::DifferentVariant),
        }
    }
}

impl PartialEq<Component> for OwnedFormatItem {
    #[inline]
    fn eq(&self, rhs: &Component) -> bool {
        matches!(self, Self::Component(component) if component == rhs)
    }
}

impl PartialEq<OwnedFormatItem> for Component {
    #[inline]
    fn eq(&self, rhs: &OwnedFormatItem) -> bool {
        rhs == self
    }
}

impl PartialEq<&[Self]> for OwnedFormatItem {
    #[inline]
    fn eq(&self, rhs: &&[Self]) -> bool {
        matches!(self, Self::Compound(compound) if &&**compound == rhs)
    }
}

impl PartialEq<OwnedFormatItem> for &[OwnedFormatItem] {
    #[inline]
    fn eq(&self, rhs: &OwnedFormatItem) -> bool {
        rhs == self
    }
}
