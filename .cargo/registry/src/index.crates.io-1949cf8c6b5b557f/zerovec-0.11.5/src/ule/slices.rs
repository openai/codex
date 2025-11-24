// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use crate::ule::*;

// Safety (based on the safety checklist on the ULE trait):
//  1. [T; N] does not include any uninitialized or padding bytes since T is ULE
//  2. [T; N] is aligned to 1 byte since T is ULE
//  3. The impl of validate_bytes() returns an error if any byte is not valid.
//  4. The impl of validate_bytes() returns an error if there are leftover bytes.
//  5. The other ULE methods use the default impl.
//  6. [T; N] byte equality is semantic equality since T is ULE
unsafe impl<T: ULE, const N: usize> ULE for [T; N] {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        // a slice of multiple Selfs is equivalent to just a larger slice of Ts
        T::validate_bytes(bytes)
    }
}

impl<T: AsULE, const N: usize> AsULE for [T; N] {
    type ULE = [T::ULE; N];
    #[inline]
    fn to_unaligned(self) -> Self::ULE {
        self.map(T::to_unaligned)
    }
    #[inline]
    fn from_unaligned(unaligned: Self::ULE) -> Self {
        unaligned.map(T::from_unaligned)
    }
}

unsafe impl<T: EqULE, const N: usize> EqULE for [T; N] {}

// Safety (based on the safety checklist on the VarULE trait):
//  1. str does not include any uninitialized or padding bytes.
//  2. str is aligned to 1 byte.
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//  6. `parse_bytes()` is equivalent to `validate_bytes()` followed by `from_bytes_unchecked()`
//  7. str byte equality is semantic equality
unsafe impl VarULE for str {
    #[inline]
    fn validate_bytes(bytes: &[u8]) -> Result<(), UleError> {
        core::str::from_utf8(bytes).map_err(|_| UleError::parse::<Self>())?;
        Ok(())
    }

    #[inline]
    fn parse_bytes(bytes: &[u8]) -> Result<&Self, UleError> {
        core::str::from_utf8(bytes).map_err(|_| UleError::parse::<Self>())
    }
    /// Invariant: must be safe to call when called on a slice that previously
    /// succeeded with `parse_bytes`
    #[inline]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        core::str::from_utf8_unchecked(bytes)
    }
}

/// Note: VarULE is well-defined for all `[T]` where `T: ULE`, but [`ZeroSlice`] is more ergonomic
/// when `T` is a low-level ULE type. For example:
///
/// ```no_run
/// # use zerovec::ZeroSlice;
/// # use zerovec::VarZeroVec;
/// # use zerovec::ule::AsULE;
/// // OK: [u8] is a useful type
/// let _: VarZeroVec<[u8]> = unimplemented!();
///
/// // Technically works, but [u32::ULE] is not very useful
/// let _: VarZeroVec<[<u32 as AsULE>::ULE]> = unimplemented!();
///
/// // Better: ZeroSlice<u32>
/// let _: VarZeroVec<ZeroSlice<u32>> = unimplemented!();
/// ```
///
/// [`ZeroSlice`]: crate::ZeroSlice
// Safety (based on the safety checklist on the VarULE trait):
//  1. [T] does not include any uninitialized or padding bytes (achieved by being a slice of a ULE type)
//  2. [T] is aligned to 1 byte (achieved by being a slice of a ULE type)
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//  6. All other methods are defaulted
//  7. `[T]` byte equality is semantic equality (achieved by being a slice of a ULE type)
unsafe impl<T> VarULE for [T]
where
    T: ULE,
{
    #[inline]
    fn validate_bytes(slice: &[u8]) -> Result<(), UleError> {
        T::validate_bytes(slice)
    }

    #[inline]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        T::slice_from_bytes_unchecked(bytes)
    }
}
