//! Various error types returned by methods in the time crate.

mod component_range;
mod conversion_range;
mod different_variant;
#[cfg(feature = "formatting")]
mod format;
#[cfg(feature = "local-offset")]
mod indeterminate_offset;
#[cfg(all(any(feature = "formatting", feature = "parsing"), feature = "alloc"))]
mod invalid_format_description;
mod invalid_variant;
#[cfg(feature = "parsing")]
mod parse;
#[cfg(feature = "parsing")]
mod parse_from_description;
#[cfg(feature = "parsing")]
mod try_from_parsed;

#[cfg(feature = "parsing")]
use core::convert::Infallible;
use core::fmt;

pub use component_range::ComponentRange;
pub use conversion_range::ConversionRange;
pub use different_variant::DifferentVariant;
#[cfg(feature = "formatting")]
pub use format::Format;
#[cfg(feature = "local-offset")]
pub use indeterminate_offset::IndeterminateOffset;
#[cfg(all(any(feature = "formatting", feature = "parsing"), feature = "alloc"))]
pub use invalid_format_description::InvalidFormatDescription;
pub use invalid_variant::InvalidVariant;
#[cfg(feature = "parsing")]
pub use parse::Parse;
#[cfg(feature = "parsing")]
pub use parse_from_description::ParseFromDescription;
#[cfg(feature = "parsing")]
pub use try_from_parsed::TryFromParsed;

/// A unified error type for anything returned by a method in the time crate.
///
/// This can be used when you either don't know or don't care about the exact error returned.
/// `Result<_, time::Error>` (or its alias `time::Result<_>`) will work in these situations.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {
    #[expect(missing_docs)]
    ConversionRange(ConversionRange),
    #[expect(missing_docs)]
    ComponentRange(ComponentRange),
    #[cfg(feature = "local-offset")]
    #[expect(missing_docs)]
    IndeterminateOffset(IndeterminateOffset),
    #[cfg(feature = "formatting")]
    #[expect(missing_docs)]
    Format(Format),
    #[cfg(feature = "parsing")]
    #[expect(missing_docs)]
    ParseFromDescription(ParseFromDescription),
    #[cfg(feature = "parsing")]
    #[expect(missing_docs)]
    #[non_exhaustive]
    #[deprecated(
        since = "0.3.28",
        note = "no longer output. moved to the `ParseFromDescription` variant"
    )]
    UnexpectedTrailingCharacters {
        #[doc(hidden)]
        never: Infallible,
    },
    #[cfg(feature = "parsing")]
    #[expect(missing_docs)]
    TryFromParsed(TryFromParsed),
    #[cfg(all(any(feature = "formatting", feature = "parsing"), feature = "alloc"))]
    #[expect(missing_docs)]
    InvalidFormatDescription(InvalidFormatDescription),
    #[expect(missing_docs)]
    DifferentVariant(DifferentVariant),
    #[expect(missing_docs)]
    InvalidVariant(InvalidVariant),
}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversionRange(e) => e.fmt(f),
            Self::ComponentRange(e) => e.fmt(f),
            #[cfg(feature = "local-offset")]
            Self::IndeterminateOffset(e) => e.fmt(f),
            #[cfg(feature = "formatting")]
            Self::Format(e) => e.fmt(f),
            #[cfg(feature = "parsing")]
            Self::ParseFromDescription(e) => e.fmt(f),
            #[cfg(feature = "parsing")]
            #[allow(deprecated)]
            Self::UnexpectedTrailingCharacters { never } => match *never {},
            #[cfg(feature = "parsing")]
            Self::TryFromParsed(e) => e.fmt(f),
            #[cfg(all(any(feature = "formatting", feature = "parsing"), feature = "alloc"))]
            Self::InvalidFormatDescription(e) => e.fmt(f),
            Self::DifferentVariant(e) => e.fmt(f),
            Self::InvalidVariant(e) => e.fmt(f),
        }
    }
}

impl core::error::Error for Error {
    #[inline]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::ConversionRange(err) => Some(err),
            Self::ComponentRange(err) => Some(err),
            #[cfg(feature = "local-offset")]
            Self::IndeterminateOffset(err) => Some(err),
            #[cfg(feature = "formatting")]
            Self::Format(err) => Some(err),
            #[cfg(feature = "parsing")]
            Self::ParseFromDescription(err) => Some(err),
            #[cfg(feature = "parsing")]
            #[allow(deprecated)]
            Self::UnexpectedTrailingCharacters { never } => match *never {},
            #[cfg(feature = "parsing")]
            Self::TryFromParsed(err) => Some(err),
            #[cfg(all(any(feature = "formatting", feature = "parsing"), feature = "alloc"))]
            Self::InvalidFormatDescription(err) => Some(err),
            Self::DifferentVariant(err) => Some(err),
            Self::InvalidVariant(err) => Some(err),
        }
    }
}
