//! This module contains the `Never` type.
//!
//! Values of this type can never be created and will never exist.

/// A type with no possible values.
///
/// This is used to indicate values which can never be created, such as the
/// error type of infallible futures.
///
/// This type is a stable equivalent to the `!` type from `std`.
///
/// This is currently an alias for [`std::convert::Infallible`], but in
/// the future it may be an alias for [`!`][never].
/// See ["Future compatibility" section of `std::convert::Infallible`][infallible] for more.
///
/// [never]: https://doc.rust-lang.org/nightly/std/primitive.never.html
/// [infallible]: https://doc.rust-lang.org/nightly/std/convert/enum.Infallible.html#future-compatibility
pub type Never = core::convert::Infallible;
