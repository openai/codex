// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use core::marker::PhantomData;

#[cfg(feature = "alloc")]
use alloc::borrow::{Cow, ToOwned};
#[cfg(feature = "alloc")]
use alloc::string::String;

/// Trait for types that can be created from a reference to a different type `C` with no allocations,
/// i.e. a zero-copy (zero-alloc) version of "From"
///
/// A type can be the `ZeroFrom` target of multiple other types.
///
/// The intention is for `ZeroFrom` to produce a struct from a other with as little work as
/// possible. Although it is technically possible to implement `ZeroFrom` without being
/// zero-copy (using heap allocations), doing so defeats the purpose of `ZeroFrom`.
///
/// For example, `impl ZeroFrom<C> for Cow<str>` should return a `Cow::Borrowed` pointing at
/// data in the other type `C`, even if the other type is itself fully owned.
///
/// One can use the [`#[derive(ZeroFrom)]`](zerofrom_derive::ZeroFrom) custom derive to automatically
/// implement this trait.
///
/// # Examples
///
/// Implementing `ZeroFrom` on a custom data struct:
///
/// ```
/// use std::borrow::Cow;
/// use zerofrom::ZeroFrom;
///
/// struct MyStruct<'data> {
///     message: Cow<'data, str>,
/// }
///
/// // Reference from a borrowed version of self
/// impl<'zf> ZeroFrom<'zf, MyStruct<'_>> for MyStruct<'zf> {
///     fn zero_from(other: &'zf MyStruct<'_>) -> Self {
///         MyStruct {
///             message: Cow::Borrowed(&other.message),
///         }
///     }
/// }
///
/// // Reference from a string slice directly
/// impl<'zf> ZeroFrom<'zf, str> for MyStruct<'zf> {
///     fn zero_from(other: &'zf str) -> Self {
///         MyStruct {
///             message: Cow::Borrowed(other),
///         }
///     }
/// }
/// ```
pub trait ZeroFrom<'zf, C: ?Sized>: 'zf {
    /// Clone the other `C` into a struct that may retain references into `C`.
    fn zero_from(other: &'zf C) -> Self;
}

// Note: The following could be blanket implementations, but that would require constraining the
// blanket `T` on `T: 'static`, which may not be desirable for all downstream users who may wish
// to customize their `ZeroFrom` impl. The blanket implementation may be safe once Rust has
// specialization.

#[cfg(feature = "alloc")]
impl<'zf> ZeroFrom<'zf, str> for Cow<'zf, str> {
    #[inline]
    fn zero_from(other: &'zf str) -> Self {
        Cow::Borrowed(other)
    }
}

#[cfg(feature = "alloc")]
impl<'zf> ZeroFrom<'zf, String> for Cow<'zf, str> {
    #[inline]
    fn zero_from(other: &'zf String) -> Self {
        Cow::Borrowed(other)
    }
}

impl<'zf> ZeroFrom<'zf, str> for &'zf str {
    #[inline]
    fn zero_from(other: &'zf str) -> Self {
        other
    }
}

#[cfg(feature = "alloc")]
impl<'zf> ZeroFrom<'zf, String> for &'zf str {
    #[inline]
    fn zero_from(other: &'zf String) -> Self {
        other
    }
}

impl<'zf, C, T: ZeroFrom<'zf, C>> ZeroFrom<'zf, Option<C>> for Option<T> {
    fn zero_from(other: &'zf Option<C>) -> Self {
        other.as_ref().map(|c| <T as ZeroFrom<C>>::zero_from(c))
    }
}

// These duplicate the functionality from above and aren't quite necessary due
// to deref coercions, however for the custom derive to work, there always needs
// to be `impl ZeroFrom<T> for T`, otherwise it may fail to perform the necessary
// type inference. Deref coercions do not typically work when sufficient generics
// or inference are involved, and the proc macro does not necessarily have
// enough type information to figure this out on its own.
#[cfg(feature = "alloc")]
impl<'zf, B: ToOwned + ?Sized> ZeroFrom<'zf, Cow<'_, B>> for Cow<'zf, B> {
    #[inline]
    fn zero_from(other: &'zf Cow<'_, B>) -> Self {
        Cow::Borrowed(other)
    }
}

impl<'zf, T: ?Sized> ZeroFrom<'zf, &'_ T> for &'zf T {
    #[inline]
    fn zero_from(other: &'zf &'_ T) -> &'zf T {
        other
    }
}

impl<'zf, T> ZeroFrom<'zf, [T]> for &'zf [T] {
    #[inline]
    fn zero_from(other: &'zf [T]) -> &'zf [T] {
        other
    }
}

impl<'zf, T: ?Sized + 'zf> ZeroFrom<'zf, PhantomData<T>> for PhantomData<T> {
    fn zero_from(other: &'zf PhantomData<T>) -> Self {
        *other
    }
}
