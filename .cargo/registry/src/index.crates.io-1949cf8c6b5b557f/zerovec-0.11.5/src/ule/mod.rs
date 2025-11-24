// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#![allow(clippy::upper_case_acronyms)]

//! Traits over unaligned little-endian data (ULE, pronounced "yule").
//!
//! The main traits for this module are [`ULE`], [`AsULE`] and, [`VarULE`].
//!
//! See [the design doc](https://github.com/unicode-org/icu4x/blob/main/utils/zerovec/design_doc.md) for details on how these traits
//! works under the hood.
mod chars;
#[cfg(doc)]
pub mod custom;
mod encode;
mod macros;
mod multi;
mod niche;
mod option;
mod plain;
mod slices;
#[cfg(test)]
pub mod test_utils;

pub mod tuple;
pub mod tuplevar;
pub mod vartuple;
pub use chars::CharULE;
#[cfg(feature = "alloc")]
pub use encode::encode_varule_to_box;
pub use encode::EncodeAsVarULE;
pub use multi::MultiFieldsULE;
pub use niche::{NicheBytes, NichedOption, NichedOptionULE};
pub use option::{OptionULE, OptionVarULE};
pub use plain::RawBytesULE;

use core::{any, fmt, mem, slice};

/// Fixed-width, byte-aligned data that can be cast to and from a little-endian byte slice.
///
/// If you need to implement this trait, consider using [`#[make_ule]`](crate::make_ule) or
///  [`#[derive(ULE)]`](macro@ULE) instead.
///
/// Types that are not fixed-width can implement [`VarULE`] instead.
///
/// "ULE" stands for "Unaligned little-endian"
///
/// # Safety
///
/// Safety checklist for `ULE`:
///
/// 1. The type *must not* include any uninitialized or padding bytes.
/// 2. The type must have an alignment of 1 byte, or it is a ZST that is safe to construct.
/// 3. The impl of [`ULE::validate_bytes()`] *must* return an error if the given byte slice
///    would not represent a valid slice of this type.
/// 4. The impl of [`ULE::validate_bytes()`] *must* return an error if the given byte slice
///    cannot be used in its entirety (if its length is not a multiple of `size_of::<Self>()`).
/// 5. All other methods *must* be left with their default impl, or else implemented according to
///    their respective safety guidelines.
/// 6. Acknowledge the following note about the equality invariant.
///
/// If the ULE type is a struct only containing other ULE types (or other types which satisfy invariants 1 and 2,
/// like `[u8; N]`), invariants 1 and 2 can be achieved via `#[repr(C, packed)]` or `#[repr(transparent)]`.
///
/// # Equality invariant
///
/// A non-safety invariant is that if `Self` implements `PartialEq`, the it *must* be logically
/// equivalent to byte equality on [`Self::slice_as_bytes()`].
///
/// It may be necessary to introduce a "canonical form" of the ULE if logical equality does not
/// equal byte equality. In such a case, [`Self::validate_bytes()`] should return an error
/// for any values that are not in canonical form. For example, the decimal strings "1.23e4" and
/// "12.3e3" are logically equal, but not byte-for-byte equal, so we could define a canonical form
/// where only a single digit is allowed before `.`.
///
/// Failure to follow this invariant will cause surprising behavior in `PartialEq`, which may
/// result in unpredictable operations on `ZeroVec`, `VarZeroVec`, and `ZeroMap`.
pub unsafe trait ULE
where
    Self: Sized,
    Self: Copy + 'static,
{
    /// Validates a byte slice, `&[u8]`.
    ///
    /// If `Self` is not well-defined for all possible bit values, the bytes should be validated.
    /// If the bytes can be transmuted, *in their entirety*, to a valid slice of `Self`, then `Ok`
    /// should be returned; otherwise, `Err` should be returned.
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError>;

    /// Parses a byte slice, `&[u8]`, and return it as `&[Self]` with the same lifetime.
    ///
    /// If `Self` is not well-defined for all possible bit values, the bytes should be validated,
    /// and an error should be returned in the same cases as [`Self::validate_bytes()`].
    ///
    /// The default implementation executes [`Self::validate_bytes()`] followed by
    /// [`Self::slice_from_bytes_unchecked`].
    ///
    /// Note: The following equality should hold: `bytes.len() % size_of::<Self>() == 0`. This
    /// means that the returned slice can span the entire byte slice.
    fn parse_bytes_to_slice(bytes: &[u8]) -> Result<&[Self], UleError> {
        Self::validate_bytes(bytes)?;
        debug_assert_eq!(bytes.len() % mem::size_of::<Self>(), 0);
        Ok(unsafe { Self::slice_from_bytes_unchecked(bytes) })
    }

    /// Takes a byte slice, `&[u8]`, and return it as `&[Self]` with the same lifetime, assuming
    /// that this byte slice has previously been run through [`Self::parse_bytes_to_slice()`] with
    /// success.
    ///
    /// The default implementation performs a pointer cast to the same region of memory.
    ///
    /// # Safety
    ///
    /// ## Callers
    ///
    /// Callers of this method must take care to ensure that `bytes` was previously passed through
    /// [`Self::validate_bytes()`] with success (and was not changed since then).
    ///
    /// ## Implementors
    ///
    /// Implementations of this method may call unsafe functions to cast the pointer to the correct
    /// type, assuming the "Callers" invariant above.
    ///
    /// Keep in mind that `&[Self]` and `&[u8]` may have different lengths.
    ///
    /// Safety checklist:
    ///
    /// 1. This method *must* return the same result as [`Self::parse_bytes_to_slice()`].
    /// 2. This method *must* return a slice to the same region of memory as the argument.
    #[inline]
    unsafe fn slice_from_bytes_unchecked(bytes: &[u8]) -> &[Self] {
        let data = bytes.as_ptr();
        let len = bytes.len() / mem::size_of::<Self>();
        debug_assert_eq!(bytes.len() % mem::size_of::<Self>(), 0);
        core::slice::from_raw_parts(data as *const Self, len)
    }

    /// Given `&[Self]`, returns a `&[u8]` with the same lifetime.
    ///
    /// The default implementation performs a pointer cast to the same region of memory.
    ///
    /// # Safety
    ///
    /// Implementations of this method should call potentially unsafe functions to cast the
    /// pointer to the correct type.
    ///
    /// Keep in mind that `&[Self]` and `&[u8]` may have different lengths.
    #[inline]
    fn slice_as_bytes(slice: &[Self]) -> &[u8] {
        unsafe {
            slice::from_raw_parts(slice as *const [Self] as *const u8, mem::size_of_val(slice))
        }
    }
}

/// A trait for any type that has a 1:1 mapping with an unaligned little-endian (ULE) type.
///
/// If you need to implement this trait, consider using [`#[make_ule]`](crate::make_ule) instead.
pub trait AsULE: Copy {
    /// The ULE type corresponding to `Self`.
    ///
    /// Types having infallible conversions from all bit values (Plain Old Data) can use
    /// `RawBytesULE` with the desired width; for example, `u32` uses `RawBytesULE<4>`.
    ///
    /// Types that are not well-defined for all bit values should implement a custom ULE.
    type ULE: ULE;

    /// Converts from `Self` to `Self::ULE`.
    ///
    /// This function may involve byte order swapping (native-endian to little-endian).
    ///
    /// For best performance, mark your implementation of this function `#[inline]`.
    fn to_unaligned(self) -> Self::ULE;

    /// Converts from `Self::ULE` to `Self`.
    ///
    /// This function may involve byte order swapping (little-endian to native-endian).
    ///
    /// For best performance, mark your implementation of this function `#[inline]`.
    ///
    /// # Safety
    ///
    /// This function is infallible because bit validation should have occurred when `Self::ULE`
    /// was first constructed. An implementation may therefore involve an `unsafe{}` block, like
    /// `from_bytes_unchecked()`.
    fn from_unaligned(unaligned: Self::ULE) -> Self;
}

/// A type whose byte sequence equals the byte sequence of its ULE type on
/// little-endian platforms.
///
/// This enables certain performance optimizations, such as
/// [`ZeroVec::try_from_slice`](crate::ZeroVec::try_from_slice).
///
/// # Implementation safety
///
/// This trait is safe to implement if the type's ULE (as defined by `impl `[`AsULE`]` for T`)
/// has an equal byte sequence as the type itself on little-endian platforms; i.e., one where
/// `*const T` can be cast to a valid `*const T::ULE`.
pub unsafe trait EqULE: AsULE {}

/// A trait for a type where aligned slices can be cast to unaligned slices.
///
/// Auto-implemented on all types implementing [`EqULE`].
pub trait SliceAsULE
where
    Self: AsULE + Sized,
{
    /// Converts from `&[Self]` to `&[Self::ULE]` if possible.
    ///
    /// In general, this function returns `Some` on little-endian and `None` on big-endian.
    fn slice_to_unaligned(slice: &[Self]) -> Option<&[Self::ULE]>;
}

#[cfg(target_endian = "little")]
impl<T> SliceAsULE for T
where
    T: EqULE,
{
    #[inline]
    fn slice_to_unaligned(slice: &[Self]) -> Option<&[Self::ULE]> {
        // This is safe because on little-endian platforms, the byte sequence of &[T]
        // is equivalent to the byte sequence of &[T::ULE] by the contract of EqULE,
        // and &[T::ULE] has equal or looser alignment than &[T].
        let ule_slice =
            unsafe { core::slice::from_raw_parts(slice.as_ptr() as *const Self::ULE, slice.len()) };
        Some(ule_slice)
    }
}

#[cfg(not(target_endian = "little"))]
impl<T> SliceAsULE for T
where
    T: EqULE,
{
    #[inline]
    fn slice_to_unaligned(_: &[Self]) -> Option<&[Self::ULE]> {
        None
    }
}

/// Variable-width, byte-aligned data that can be cast to and from a little-endian byte slice.
///
/// If you need to implement this trait, consider using [`#[make_varule]`](crate::make_varule) or
///  [`#[derive(VarULE)]`](macro@VarULE) instead.
///
/// This trait is mostly for unsized types like `str` and `[T]`. It can be implemented on sized types;
/// however, it is much more preferable to use [`ULE`] for that purpose. The [`custom`] module contains
/// additional documentation on how this type can be implemented on custom types.
///
/// If deserialization with `VarZeroVec` is desired is recommended to implement `Deserialize` for
/// `Box<T>` (serde does not do this automatically for unsized `T`).
///
/// For convenience it is typically desired to implement [`EncodeAsVarULE`] and [`ZeroFrom`](zerofrom::ZeroFrom)
/// on some stack type to convert to and from the ULE type efficiently when necessary.
///
/// # Safety
///
/// Safety checklist for `VarULE`:
///
/// 1. The type *must not* include any uninitialized or padding bytes.
/// 2. The type must have an alignment of 1 byte.
/// 3. The impl of [`VarULE::validate_bytes()`] *must* return an error if the given byte slice
///    would not represent a valid slice of this type.
/// 4. The impl of [`VarULE::validate_bytes()`] *must* return an error if the given byte slice
///    cannot be used in its entirety.
/// 5. The impl of [`VarULE::from_bytes_unchecked()`] must produce a reference to the same
///    underlying data assuming that the given bytes previously passed validation.
/// 6. All other methods *must* be left with their default impl, or else implemented according to
///    their respective safety guidelines.
/// 7. Acknowledge the following note about the equality invariant.
///
/// If the ULE type is a struct only containing other ULE/VarULE types (or other types which satisfy invariants 1 and 2,
/// like `[u8; N]`), invariants 1 and 2 can be achieved via `#[repr(C, packed)]` or `#[repr(transparent)]`.
///
/// # Equality invariant
///
/// A non-safety invariant is that if `Self` implements `PartialEq`, the it *must* be logically
/// equivalent to byte equality on [`Self::as_bytes()`].
///
/// It may be necessary to introduce a "canonical form" of the ULE if logical equality does not
/// equal byte equality. In such a case, [`Self::validate_bytes()`] should return an error
/// for any values that are not in canonical form. For example, the decimal strings "1.23e4" and
/// "12.3e3" are logically equal, but not byte-for-byte equal, so we could define a canonical form
/// where only a single digit is allowed before `.`.
///
/// There may also be cases where a `VarULE` has muiltiple canonical forms, such as a faster
/// version and a smaller version. The cleanest way to handle this case would be separate types.
/// However, if this is not feasible, then the application should ensure that the data it is
/// deserializing is in the expected form. For example, if the data is being loaded from an
/// external source, then requests could carry information about the expected form of the data.
///
/// Failure to follow this invariant will cause surprising behavior in `PartialEq`, which may
/// result in unpredictable operations on `ZeroVec`, `VarZeroVec`, and `ZeroMap`.
pub unsafe trait VarULE: 'static {
    /// Validates a byte slice, `&[u8]`.
    ///
    /// If `Self` is not well-defined for all possible bit values, the bytes should be validated.
    /// If the bytes can be transmuted, *in their entirety*, to a valid `&Self`, then `Ok` should
    /// be returned; otherwise, `Self::Error` should be returned.
    fn validate_bytes(_bytes: &[u8]) -> Result<(), UleError>;

    /// Parses a byte slice, `&[u8]`, and return it as `&Self` with the same lifetime.
    ///
    /// If `Self` is not well-defined for all possible bit values, the bytes should be validated,
    /// and an error should be returned in the same cases as [`Self::validate_bytes()`].
    ///
    /// The default implementation executes [`Self::validate_bytes()`] followed by
    /// [`Self::from_bytes_unchecked`].
    ///
    /// Note: The following equality should hold: `size_of_val(result) == size_of_val(bytes)`,
    /// where `result` is the successful return value of the method. This means that the return
    /// value spans the entire byte slice.
    fn parse_bytes(bytes: &[u8]) -> Result<&Self, UleError> {
        Self::validate_bytes(bytes)?;
        let result = unsafe { Self::from_bytes_unchecked(bytes) };
        debug_assert_eq!(mem::size_of_val(result), mem::size_of_val(bytes));
        Ok(result)
    }

    /// Takes a byte slice, `&[u8]`, and return it as `&Self` with the same lifetime, assuming
    /// that this byte slice has previously been run through [`Self::parse_bytes()`] with
    /// success.
    ///
    /// # Safety
    ///
    /// ## Callers
    ///
    /// Callers of this method must take care to ensure that `bytes` was previously passed through
    /// [`Self::validate_bytes()`] with success (and was not changed since then).
    ///
    /// ## Implementors
    ///
    /// Implementations of this method may call unsafe functions to cast the pointer to the correct
    /// type, assuming the "Callers" invariant above.
    ///
    /// Safety checklist:
    ///
    /// 1. This method *must* return the same result as [`Self::parse_bytes()`].
    /// 2. This method *must* return a slice to the same region of memory as the argument.
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self;

    /// Given `&Self`, returns a `&[u8]` with the same lifetime.
    ///
    /// The default implementation performs a pointer cast to the same region of memory.
    ///
    /// # Safety
    ///
    /// Implementations of this method should call potentially unsafe functions to cast the
    /// pointer to the correct type.
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self as *const Self as *const u8, mem::size_of_val(self)) }
    }

    /// Allocate on the heap as a `Box<T>`
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[inline]
    #[cfg(feature = "alloc")]
    fn to_boxed(&self) -> alloc::boxed::Box<Self> {
        use alloc::borrow::ToOwned;
        use alloc::boxed::Box;
        use core::alloc::Layout;
        let bytesvec = self.as_bytes().to_owned().into_boxed_slice();
        let bytesvec = mem::ManuallyDrop::new(bytesvec);
        unsafe {
            // Get the pointer representation
            let ptr: *mut Self = Self::from_bytes_unchecked(&bytesvec) as *const Self as *mut Self;
            assert_eq!(Layout::for_value(&*ptr), Layout::for_value(&**bytesvec));
            // Transmute the pointer to an owned pointer
            Box::from_raw(ptr)
        }
    }
}

// Proc macro reexports
//
// These exist so that our docs can use intra-doc links.
// Due to quirks of how rustdoc does documentation on reexports, these must be in this module and not reexported from
// a submodule

/// Custom derive for [`ULE`].
///
/// This can be attached to [`Copy`] structs containing only [`ULE`] types.
///
/// Most of the time, it is recommended one use [`#[make_ule]`](crate::make_ule) instead of defining
/// a custom ULE type.
#[cfg(feature = "derive")]
pub use zerovec_derive::ULE;

/// Custom derive for [`VarULE`]
///
/// This can be attached to structs containing only [`ULE`] types with one [`VarULE`] type at the end.
///
/// Most of the time, it is recommended one use [`#[make_varule]`](crate::make_varule) instead of defining
/// a custom [`VarULE`] type.
#[cfg(feature = "derive")]
pub use zerovec_derive::VarULE;

/// An error type to be used for decoding slices of ULE types
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum UleError {
    /// Attempted to parse a buffer into a slice of the given ULE type but its
    /// length was not compatible.
    ///
    /// Typically created by a [`ULE`] impl via [`UleError::length()`].
    ///
    /// [`ULE`]: crate::ule::ULE
    InvalidLength { ty: &'static str, len: usize },
    /// The byte sequence provided for `ty` failed to parse correctly in the
    /// given ULE type.
    ///
    /// Typically created by a [`ULE`] impl via [`UleError::parse()`].
    ///
    /// [`ULE`]: crate::ule::ULE
    ParseError { ty: &'static str },
}

impl fmt::Display for UleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            UleError::InvalidLength { ty, len } => {
                write!(f, "Invalid length {len} for slice of type {ty}")
            }
            UleError::ParseError { ty } => {
                write!(f, "Could not parse bytes to slice of type {ty}")
            }
        }
    }
}

impl UleError {
    /// Construct a parse error for the given type
    pub fn parse<T: ?Sized + 'static>() -> UleError {
        UleError::ParseError {
            ty: any::type_name::<T>(),
        }
    }

    /// Construct an "invalid length" error for the given type and length
    pub fn length<T: ?Sized + 'static>(len: usize) -> UleError {
        UleError::InvalidLength {
            ty: any::type_name::<T>(),
            len,
        }
    }
}

impl core::error::Error for UleError {}
