// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::components::VarZeroVecComponents;
use super::*;
use crate::ule::*;
use core::marker::PhantomData;
use core::mem;

/// A slice representing the index and data tables of a VarZeroVec,
/// *without* any length fields. The length field is expected to be stored elsewhere.
///
/// Without knowing the length this is of course unsafe to use directly.
#[repr(transparent)]
#[derive(PartialEq, Eq)]
pub(crate) struct VarZeroLengthlessSlice<T: ?Sized, F> {
    marker: PhantomData<(F, T)>,
    /// The original slice this was constructed from
    // Safety invariant: This field must have successfully passed through
    // VarZeroVecComponents::parse_bytes_with_length() with the length
    // associated with this value.
    entire_slice: [u8],
}

impl<T: VarULE + ?Sized, F: VarZeroVecFormat> VarZeroLengthlessSlice<T, F> {
    /// Obtain a [`VarZeroVecComponents`] borrowing from the internal buffer
    ///
    /// Safety: `len` must be the length associated with this value
    #[inline]
    pub(crate) unsafe fn as_components<'a>(&'a self, len: u32) -> VarZeroVecComponents<'a, T, F> {
        unsafe {
            // safety: VarZeroSlice is guaranteed to parse here
            VarZeroVecComponents::from_bytes_unchecked_with_length(len, &self.entire_slice)
        }
    }

    /// Parse a VarZeroLengthlessSlice from a slice of the appropriate format
    ///
    /// Slices of the right format can be obtained via [`VarZeroSlice::as_bytes()`]
    pub fn parse_bytes<'a>(len: u32, slice: &'a [u8]) -> Result<&'a Self, UleError> {
        let _ = VarZeroVecComponents::<T, F>::parse_bytes_with_length(len, slice)
            .map_err(|_| UleError::parse::<Self>())?;
        unsafe {
            // Safety: We just verified that it is of the correct format.
            Ok(Self::from_bytes_unchecked(slice))
        }
    }

    /// Uses a `&[u8]` buffer as a `VarZeroLengthlessSlice<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`VarZeroLengthlessSlice::as_bytes()`], or alternatively
    /// successfully pass through `parse_bytes` (with `len`)
    ///
    /// The length associated with this value will be the length associated with the original slice.
    pub(crate) const unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // self is really just a wrapper around a byte slice
        mem::transmute(bytes)
    }

    /// Uses a `&mut [u8]` buffer as a `VarZeroLengthlessSlice<T>` without any verification.
    ///
    /// # Safety
    ///
    /// `bytes` need to be an output from [`VarZeroLengthlessSlice::as_bytes()`], or alternatively
    /// be valid to be passed to `from_bytes_unchecked_with_length`
    ///
    /// The length associated with this value will be the length associated with the original slice.
    pub(crate) unsafe fn from_bytes_unchecked_mut(bytes: &mut [u8]) -> &mut Self {
        // self is really just a wrapper around a byte slice
        mem::transmute(bytes)
    }

    /// Get one of this slice's elements
    ///
    /// # Safety
    ///
    /// `index` must be in range, and `len` must be the length associated with this
    /// instance of VarZeroLengthlessSlice.
    pub(crate) unsafe fn get_unchecked(&self, len: u32, idx: usize) -> &T {
        self.as_components(len).get_unchecked(idx)
    }

    /// Get a reference to the entire encoded backing buffer of this slice
    ///
    /// The bytes can be passed back to [`Self::parse_bytes()`].
    ///
    /// To take the bytes as a vector, see [`VarZeroVec::into_bytes()`].
    #[inline]
    pub(crate) const fn as_bytes(&self) -> &[u8] {
        &self.entire_slice
    }

    /// Get the bytes behind this as a mutable slice
    ///
    /// # Safety
    ///
    ///  - `len` is the length associated with this VarZeroLengthlessSlice
    ///  - The resultant slice is only mutated in a way such that it remains a valid `T`
    ///
    /// # Panics
    ///
    ///  Panics when idx is not in bounds for this slice
    pub(crate) unsafe fn get_bytes_at_mut(&mut self, len: u32, idx: usize) -> &mut [u8] {
        let components = self.as_components(len);
        let range = components.get_things_range(idx);
        let offset = components.get_indices_size();

        // get_indices_size() returns the start of the things slice, and get_things_range()
        // returns a range in-bounds of the things slice
        #[expect(clippy::indexing_slicing)]
        &mut self.entire_slice[offset..][range]
    }
}
