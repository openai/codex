//! A private module for sealed trait and internal implementations.
//!
//! Note that this MUST be a private module.
//! See [Rust API Guidelines][sealed-trait] about the necessity of being private.
//!
//! [sealed-trait]:
//! https://rust-lang.github.io/api-guidelines/future-proofing.html#sealed-traits-protect-against-downstream-implementations-c-sealed

use crate::parser::char::is_ucschar;
use crate::spec::{IriSpec, UriSpec};

/// A trait to prohibit user-defined types from implementing `Spec`.
///
/// About sealed trait, see [Rust API Guidelines][future-proofing].
///
/// [future-proofing]: https://rust-lang.github.io/api-guidelines/future-proofing.html
pub trait Sealed: SpecInternal {}

impl Sealed for IriSpec {}
impl Sealed for UriSpec {}

/// Internal implementations for spec types.
pub trait SpecInternal: Sized {
    /// Checks if the given non-ASCII character matches `unreserved` or `iunreserved` rule.
    #[must_use]
    fn is_nonascii_char_unreserved(c: char) -> bool;
    /// Checks if the given character matches `iprivate` rule.
    #[must_use]
    fn is_nonascii_char_private(c: char) -> bool;
}

impl SpecInternal for IriSpec {
    #[inline]
    fn is_nonascii_char_unreserved(c: char) -> bool {
        is_ucschar(c)
    }

    fn is_nonascii_char_private(c: char) -> bool {
        matches!(
            u32::from(c),
            0xE000..=0xF8FF |
            0xF_0000..=0xF_FFFD |
            0x10_0000..=0x10_FFFD
        )
    }
}

impl SpecInternal for UriSpec {
    #[inline]
    fn is_nonascii_char_unreserved(_: char) -> bool {
        false
    }

    #[inline]
    fn is_nonascii_char_private(_: char) -> bool {
        false
    }
}
