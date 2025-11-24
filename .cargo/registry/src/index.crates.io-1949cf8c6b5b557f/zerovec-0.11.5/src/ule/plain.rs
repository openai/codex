// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

#![allow(clippy::upper_case_acronyms)]
//! ULE implementation for Plain Old Data types, including all sized integers.

use super::*;
use crate::impl_ule_from_array;
use crate::ZeroSlice;
use core::num::{NonZeroI8, NonZeroU8};

/// A u8 array of little-endian data with infallible conversions to and from &[u8].
#[repr(transparent)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
#[allow(clippy::exhaustive_structs)] // newtype
pub struct RawBytesULE<const N: usize>(pub [u8; N]);

impl<const N: usize> RawBytesULE<N> {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[inline]
    pub fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut [Self] {
        let data = bytes.as_mut_ptr();
        let len = bytes.len() / N;
        // Safe because Self is transparent over [u8; N]
        unsafe { core::slice::from_raw_parts_mut(data as *mut Self, len) }
    }
}

// Safety (based on the safety checklist on the ULE trait):
//  1. RawBytesULE does not include any uninitialized or padding bytes.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  2. RawBytesULE is aligned to 1 byte.
//     (achieved by `#[repr(transparent)]` on a type that satisfies this invariant)
//  3. The impl of validate_bytes() returns an error if any byte is not valid (never).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes.
//  5. The other ULE methods use the default impl.
//  6. RawBytesULE byte equality is semantic equality
unsafe impl<const N: usize> ULE for RawBytesULE<N> {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        if bytes.len() % N == 0 {
            // Safe because Self is transparent over [u8; N]
            Ok(())
        } else {
            Err(UleError::length::<Self>(bytes.len()))
        }
    }
}

impl<const N: usize> From<[u8; N]> for RawBytesULE<N> {
    #[inline]
    fn from(le_bytes: [u8; N]) -> Self {
        Self(le_bytes)
    }
}

macro_rules! impl_byte_slice_size {
    ($unsigned:ty, $size:literal) => {
        impl RawBytesULE<$size> {
            #[doc = concat!("Gets this `RawBytesULE` as a `", stringify!($unsigned), "`. This is equivalent to calling [`AsULE::from_unaligned()`] on the appropriately sized type.")]
            #[inline]
            pub fn as_unsigned_int(&self) -> $unsigned {
                <$unsigned as $crate::ule::AsULE>::from_unaligned(*self)
            }

            #[doc = concat!("Converts a `", stringify!($unsigned), "` to a `RawBytesULE`. This is equivalent to calling [`AsULE::to_unaligned()`] on the appropriately sized type.")]
            #[inline]
            pub const fn from_aligned(value: $unsigned) -> Self {
                Self(value.to_le_bytes())
            }

            impl_ule_from_array!(
                $unsigned,
                RawBytesULE<$size>,
                RawBytesULE([0; $size])
            );
        }
    };
}

macro_rules! impl_const_constructors {
    ($base:ty, $size:literal) => {
        impl ZeroSlice<$base> {
            /// This function can be used for constructing ZeroVecs in a const context, avoiding
            /// parsing checks.
            ///
            /// This cannot be generic over T because of current limitations in `const`, but if
            /// this method is needed in a non-const context, check out [`ZeroSlice::parse_bytes()`]
            /// instead.
            ///
            /// See [`ZeroSlice::cast()`] for an example.
            pub const fn try_from_bytes(bytes: &[u8]) -> Result<&Self, UleError> {
                let len = bytes.len();
                #[allow(clippy::modulo_one)]
                if len % $size == 0 {
                    Ok(unsafe { Self::from_bytes_unchecked(bytes) })
                } else {
                    Err(UleError::InvalidLength {
                        ty: concat!("<const construct: ", $size, ">"),
                        len,
                    })
                }
            }
        }
    };
}

macro_rules! impl_byte_slice_type {
    ($single_fn:ident, $type:ty, $size:literal) => {
        impl From<$type> for RawBytesULE<$size> {
            #[inline]
            fn from(value: $type) -> Self {
                Self(value.to_le_bytes())
            }
        }
        impl AsULE for $type {
            type ULE = RawBytesULE<$size>;
            #[inline]
            fn to_unaligned(self) -> Self::ULE {
                RawBytesULE(self.to_le_bytes())
            }
            #[inline]
            fn from_unaligned(unaligned: Self::ULE) -> Self {
                <$type>::from_le_bytes(unaligned.0)
            }
        }
        // EqULE is true because $type and RawBytesULE<$size>
        // have the same byte sequence on little-endian
        unsafe impl EqULE for $type {}

        impl RawBytesULE<$size> {
            pub const fn $single_fn(v: $type) -> Self {
                RawBytesULE(v.to_le_bytes())
            }
        }
    };
}

macro_rules! impl_byte_slice_unsigned_type {
    ($type:ty, $size:literal) => {
        impl_byte_slice_type!(from_unsigned, $type, $size);
    };
}

macro_rules! impl_byte_slice_signed_type {
    ($type:ty, $size:literal) => {
        impl_byte_slice_type!(from_signed, $type, $size);
    };
}

impl_byte_slice_size!(u16, 2);
impl_byte_slice_size!(u32, 4);
impl_byte_slice_size!(u64, 8);
impl_byte_slice_size!(u128, 16);

impl_byte_slice_unsigned_type!(u16, 2);
impl_byte_slice_unsigned_type!(u32, 4);
impl_byte_slice_unsigned_type!(u64, 8);
impl_byte_slice_unsigned_type!(u128, 16);

impl_byte_slice_signed_type!(i16, 2);
impl_byte_slice_signed_type!(i32, 4);
impl_byte_slice_signed_type!(i64, 8);
impl_byte_slice_signed_type!(i128, 16);

impl_const_constructors!(u8, 1);
impl_const_constructors!(u16, 2);
impl_const_constructors!(u32, 4);
impl_const_constructors!(u64, 8);
impl_const_constructors!(u128, 16);

// Note: The f32 and f64 const constructors currently have limited use because
// `f32::to_le_bytes` is not yet const.

impl_const_constructors!(bool, 1);

// Safety (based on the safety checklist on the ULE trait):
//  1. u8 does not include any uninitialized or padding bytes.
//  2. u8 is aligned to 1 byte.
//  3. The impl of validate_bytes() returns an error if any byte is not valid (never).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes (never).
//  5. The other ULE methods use the default impl.
//  6. u8 byte equality is semantic equality
unsafe impl ULE for u8 {
    #[inline]
    fn validate_bytes(_bytes: &[u8]) -> Result<(), UleError> {
        Ok(())
    }
}

impl AsULE for u8 {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

// EqULE is true because u8 is its own ULE.
unsafe impl EqULE for u8 {}

// Safety (based on the safety checklist on the ULE trait):
//  1. NonZeroU8 does not include any uninitialized or padding bytes.
//  2. NonZeroU8 is aligned to 1 byte.
//  3. The impl of validate_bytes() returns an error if any byte is not valid (0x00).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes (never).
//  5. The other ULE methods use the default impl.
//  6. NonZeroU8 byte equality is semantic equality
unsafe impl ULE for NonZeroU8 {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        bytes.iter().try_for_each(|b| {
            if *b == 0x00 {
                Err(UleError::parse::<Self>())
            } else {
                Ok(())
            }
        })
    }
}

impl AsULE for NonZeroU8 {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

unsafe impl EqULE for NonZeroU8 {}

impl NicheBytes<1> for NonZeroU8 {
    const NICHE_BIT_PATTERN: [u8; 1] = [0x00];
}

// Safety (based on the safety checklist on the ULE trait):
//  1. i8 does not include any uninitialized or padding bytes.
//  2. i8 is aligned to 1 byte.
//  3. The impl of validate_bytes() returns an error if any byte is not valid (never).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes (never).
//  5. The other ULE methods use the default impl.
//  6. i8 byte equality is semantic equality
unsafe impl ULE for i8 {
    #[inline]
    fn validate_bytes(_bytes: &[u8]) -> Result<(), UleError> {
        Ok(())
    }
}

impl AsULE for i8 {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

// EqULE is true because i8 is its own ULE.
unsafe impl EqULE for i8 {}

impl AsULE for NonZeroI8 {
    type ULE = NonZeroU8;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        // Safety: NonZeroU8 and NonZeroI8 have same size
        unsafe { core::mem::transmute(self) }
    }

    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        // Safety: NonZeroU8 and NonZeroI8 have same size
        unsafe { core::mem::transmute(unaligned) }
    }
}

// These impls are actually safe and portable due to Rust always using IEEE 754, see the documentation
// on f32::from_bits: https://doc.rust-lang.org/stable/std/primitive.f32.html#method.from_bits
//
// The only potential problem is that some older platforms treat signaling NaNs differently. This is
// still quite portable, signalingness is not typically super important.

impl AsULE for f32 {
    type ULE = RawBytesULE<4>;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self.to_bits().to_unaligned()
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        Self::from_bits(u32::from_unaligned(unaligned))
    }
}

impl AsULE for f64 {
    type ULE = RawBytesULE<8>;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self.to_bits().to_unaligned()
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        Self::from_bits(u64::from_unaligned(unaligned))
    }
}

// The from_bits documentation mentions that they have identical byte representations to integers
// and EqULE only cares about LE systems
unsafe impl EqULE for f32 {}
unsafe impl EqULE for f64 {}

// The bool impl is not as efficient as it could be
// We can, in the future, have https://github.com/unicode-org/icu4x/blob/main/utils/zerovec/design_doc.md#bitpacking
// for better bitpacking

// Safety (based on the safety checklist on the ULE trait):
//  1. bool does not include any uninitialized or padding bytes (the remaining 7 bytes in bool are by definition zero)
//  2. bool is aligned to 1 byte.
//  3. The impl of validate_bytes() returns an error if any byte is not valid (bytes that are not 0 or 1).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes (never).
//  5. The other ULE methods use the default impl.
//  6. bool byte equality is semantic equality
unsafe impl ULE for bool {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        for byte in bytes {
            // https://doc.rust-lang.org/reference/types/boolean.html
            // Rust booleans are always size 1, align 1 values with valid bit patterns 0x0 or 0x1
            if *byte > 1 {
                return Err(UleError::parse::<Self>());
            }
        }
        Ok(())
    }
}

impl AsULE for bool {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

// EqULE is true because bool is its own ULE.
unsafe impl EqULE for bool {}

// Safety (based on the safety checklist on the ULE trait):
//  1. () does not include any uninitialized or padding bytes (it has no bytes)
//  2. () is a ZST that is safe to construct
//  3. The impl of validate_bytes() returns an error if any byte is not valid (any byte).
//  4. The impl of validate_bytes() returns an error if there are leftover bytes (always).
//  5. The other ULE methods use the default impl.
//  6. () byte equality is semantic equality
unsafe impl ULE for () {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        if bytes.is_empty() {
            Ok(())
        } else {
            Err(UleError::length::<Self>(bytes.len()))
        }
    }
}

impl AsULE for () {
    type ULE = Self;
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned
    }
}

// EqULE is true because () is its own ULE.
unsafe impl EqULE for () {}
