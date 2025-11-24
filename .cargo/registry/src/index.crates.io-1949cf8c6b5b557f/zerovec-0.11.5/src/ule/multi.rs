// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use crate::varzerovec::lengthless::VarZeroLengthlessSlice;
use crate::vecs::VarZeroVecFormat;
use core::{fmt, mem};

/// This type is used by the custom derive to represent multiple [`VarULE`]
/// fields packed into a single end-of-struct field. It is not recommended
/// to use this type directly, use [`Tuple2VarULE`](crate::ule::tuplevar::Tuple2VarULE) etc instead.
///
/// Logically, consider it to be `(, , , ..)`
/// where `` etc are potentially different [`VarULE`] types.
///
/// Internally, it is represented by a VarZeroSlice without the length part.
#[derive(PartialEq, Eq)]
#[repr(transparent)]
pub struct MultiFieldsULE<const LEN: usize, Format: VarZeroVecFormat>(
    VarZeroLengthlessSlice<[u8], Format>,
);

impl<const LEN: usize, Format: VarZeroVecFormat> MultiFieldsULE<LEN, Format> {
    /// Compute the amount of bytes needed to support elements with lengths `lengths`
    #[inline]
    #[expect(clippy::expect_used)] // See #1410
    pub fn compute_encoded_len_for(lengths: [usize; LEN]) -> usize {
        let lengths = lengths.map(BlankSliceEncoder);
        crate::varzerovec::components::compute_serializable_len_without_length::<_, _, Format>(
            &lengths,
        )
        .expect("Too many bytes to encode") as usize
    }

    /// Construct a partially initialized MultiFieldsULE backed by a mutable byte buffer
    pub fn new_from_lengths_partially_initialized<'a>(
        lengths: [usize; LEN],
        output: &'a mut [u8],
    ) -> &'a mut Self {
        let lengths = lengths.map(BlankSliceEncoder);
        crate::varzerovec::components::write_serializable_bytes_without_length::<_, _, Format>(
            &lengths, output,
        );
        debug_assert!(
            <VarZeroLengthlessSlice<[u8], Format>>::parse_bytes(LEN as u32, output).is_ok(),
            "Encoded slice must be valid VarZeroSlice"
        );
        unsafe {
            // Safe since write_serializable_bytes produces a valid VarZeroLengthlessSlice buffer with the right format
            let slice = <VarZeroLengthlessSlice<[u8], Format>>::from_bytes_unchecked_mut(output);
            // safe since `Self` is transparent over VarZeroLengthlessSlice<[u8], Format>
            mem::transmute::<&mut VarZeroLengthlessSlice<[u8], Format>, &mut Self>(slice)
        }
    }

    /// Given a buffer of size obtained by [`Self::compute_encoded_len_for()`], write element A to index idx
    ///
    /// # Safety
    /// - `idx` must be in range
    /// - `T` must be the appropriate type expected by the custom derive in this usage of this type
    #[inline]
    pub unsafe fn set_field_at<T: VarULE + ?Sized, A: EncodeAsVarULE<T> + ?Sized>(
        &mut self,
        idx: usize,
        value: &A,
    ) {
        value.encode_var_ule_write(self.0.get_bytes_at_mut(LEN as u32, idx))
    }

    /// Validate field at `index` to see if it is a valid `T` VarULE type
    ///
    /// # Safety
    ///
    /// - `index` must be in range
    #[inline]
    pub unsafe fn validate_field<T: VarULE + ?Sized>(&self, index: usize) -> Result<(), UleError> {
        T::validate_bytes(self.0.get_unchecked(LEN as u32, index))
    }

    /// Get field at `index` as a value of type T
    ///
    /// # Safety
    ///
    /// - `index` must be in range
    /// - Element at `index` must have been created with the VarULE type T
    #[inline]
    pub unsafe fn get_field<T: VarULE + ?Sized>(&self, index: usize) -> &T {
        T::from_bytes_unchecked(self.0.get_unchecked(LEN as u32, index))
    }

    /// Construct from a byte slice
    ///
    /// # Safety
    /// - byte slice must be a valid VarZeroLengthlessSlice<[u8], Format> with length LEN
    #[inline]
    pub unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // &Self is transparent over &VZS<..> with the right format
        mem::transmute(<VarZeroLengthlessSlice<[u8], Format>>::from_bytes_unchecked(bytes))
    }

    /// Get the bytes behind this value
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl<const LEN: usize, Format: VarZeroVecFormat> fmt::Debug for MultiFieldsULE<LEN, Format> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MultiFieldsULE<{LEN}>({:?})", self.0.as_bytes())
    }
}
/// This lets us conveniently use the EncodeAsVarULE functionality to create
/// `VarZeroVec<[u8]>`s that have the right amount of space for elements
/// without having to duplicate any unsafe code
#[repr(transparent)]
struct BlankSliceEncoder(usize);

unsafe impl EncodeAsVarULE<[u8]> for BlankSliceEncoder {
    fn encode_var_ule_as_slices<R>(&self, _: impl FnOnce(&[&[u8]]) -> R) -> R {
        // unnecessary if the other two are implemented
        unreachable!()
    }

    #[inline]
    fn encode_var_ule_len(&self) -> usize {
        self.0
    }

    #[inline]
    fn encode_var_ule_write(&self, _dst: &mut [u8]) {
        // do nothing
    }
}

// Safety (based on the safety checklist on the VarULE trait):
//  1. MultiFieldsULE does not include any uninitialized or padding bytes (achieved by being transparent over a VarULE type)
//  2. MultiFieldsULE is aligned to 1 byte (achieved by being transparent over a VarULE type)
//  3. The impl of `validate_bytes()` returns an error if any byte is not valid.
//  4. The impl of `validate_bytes()` returns an error if the slice cannot be used in its entirety
//  5. The impl of `from_bytes_unchecked()` returns a reference to the same data.
//  6. All other methods are defaulted
//  7. `MultiFieldsULE` byte equality is semantic equality (achieved by being transparent over a VarULE type)
unsafe impl<const LEN: usize, Format: VarZeroVecFormat> VarULE for MultiFieldsULE<LEN, Format> {
    /// Note: MultiFieldsULE is usually used in cases where one should be calling .validate_field() directly for
    /// each field, rather than using the regular VarULE impl.
    ///
    /// This impl exists so that EncodeAsVarULE can work.
    #[inline]
    fn validate_bytes(slice: &[u8]) -> Result<(), UleError> {
        <VarZeroLengthlessSlice<[u8], Format>>::parse_bytes(LEN as u32, slice).map(|_| ())
    }

    #[inline]
    unsafe fn from_bytes_unchecked(bytes: &[u8]) -> &Self {
        // &Self is transparent over &VZS<..>
        mem::transmute(<VarZeroLengthlessSlice<[u8], Format>>::from_bytes_unchecked(bytes))
    }
}
