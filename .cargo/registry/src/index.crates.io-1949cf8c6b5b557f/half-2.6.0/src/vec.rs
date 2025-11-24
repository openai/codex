//! Contains utility functions and traits to convert between vectors of [`u16`] bits and [`struct@f16`] or
//! [`bf16`] vectors.
//!
//! The utility [`HalfBitsVecExt`] sealed extension trait is implemented for [`Vec<u16>`] vectors,
//! while the utility [`HalfFloatVecExt`] sealed extension trait is implemented for both
//! [`Vec<f16>`] and [`Vec<bf16>`] vectors. These traits provide efficient conversions and
//! reinterpret casting of larger buffers of floating point values, and are automatically included
//! in the [`prelude`][crate::prelude] module.
//!
//! This module is only available with the `std` or `alloc` feature.

use super::{bf16, f16, slice::HalfFloatSliceExt};
#[cfg(feature = "alloc")]
#[allow(unused_imports)]
use alloc::{vec, vec::Vec};
use core::mem;

/// Extensions to [`Vec<f16>`] and [`Vec<bf16>`] to support reinterpret operations.
///
/// This trait is sealed and cannot be implemented outside of this crate.
pub trait HalfFloatVecExt: private::SealedHalfFloatVec {
    /// Reinterprets a vector of [`struct@f16`]or [`bf16`] numbers as a vector of [`u16`] bits.
    ///
    /// This is a zero-copy operation. The reinterpreted vector has the same memory location as
    /// `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let float_buffer = vec![f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)];
    /// let int_buffer = float_buffer.reinterpret_into();
    ///
    /// assert_eq!(int_buffer, [f16::from_f32(1.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()]);
    /// ```
    #[must_use]
    fn reinterpret_into(self) -> Vec<u16>;

    /// Converts all of the elements of a `[f32]` slice into a new [`struct@f16`] or [`bf16`] vector.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation][crate] for more information on hardware conversion
    /// support.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// let float_values = [1., 2., 3., 4.];
    /// let vec: Vec<f16> = Vec::from_f32_slice(&float_values);
    ///
    /// assert_eq!(vec, vec![f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.), f16::from_f32(4.)]);
    /// ```
    #[must_use]
    fn from_f32_slice(slice: &[f32]) -> Self;

    /// Converts all of the elements of a `[f64]` slice into a new [`struct@f16`] or [`bf16`] vector.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation][crate] for more information on hardware conversion
    /// support.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// let float_values = [1., 2., 3., 4.];
    /// let vec: Vec<f16> = Vec::from_f64_slice(&float_values);
    ///
    /// assert_eq!(vec, vec![f16::from_f64(1.), f16::from_f64(2.), f16::from_f64(3.), f16::from_f64(4.)]);
    /// ```
    #[must_use]
    fn from_f64_slice(slice: &[f64]) -> Self;
}

/// Extensions to [`Vec<u16>`] to support reinterpret operations.
///
/// This trait is sealed and cannot be implemented outside of this crate.
pub trait HalfBitsVecExt: private::SealedHalfBitsVec {
    /// Reinterprets a vector of [`u16`] bits as a vector of [`struct@f16`] or [`bf16`] numbers.
    ///
    /// `H` is the type to cast to, and must be either the [`struct@f16`] or [`bf16`] type.
    ///
    /// This is a zero-copy operation. The reinterpreted vector has the same memory location as
    /// `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let int_buffer = vec![f16::from_f32(1.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()];
    /// let float_buffer = int_buffer.reinterpret_into::<f16>();
    ///
    /// assert_eq!(float_buffer, [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)]);
    /// ```
    #[must_use]
    fn reinterpret_into<H>(self) -> Vec<H>
    where
        H: crate::private::SealedHalf;
}

mod private {
    use crate::{bf16, f16};
    #[cfg(feature = "alloc")]
    #[allow(unused_imports)]
    use alloc::vec::Vec;

    pub trait SealedHalfFloatVec {}
    impl SealedHalfFloatVec for Vec<f16> {}
    impl SealedHalfFloatVec for Vec<bf16> {}

    pub trait SealedHalfBitsVec {}
    impl SealedHalfBitsVec for Vec<u16> {}
}

impl HalfFloatVecExt for Vec<f16> {
    #[inline]
    fn reinterpret_into(mut self) -> Vec<u16> {
        // An f16 array has same length and capacity as u16 array
        let length = self.len();
        let capacity = self.capacity();

        // Actually reinterpret the contents of the Vec<f16> as u16,
        // knowing that structs are represented as only their members in memory,
        // which is the u16 part of `f16(u16)`
        let pointer = self.as_mut_ptr() as *mut u16;

        // Prevent running a destructor on the old Vec<u16>, so the pointer won't be deleted
        mem::forget(self);

        // Finally construct a new Vec<f16> from the raw pointer
        // SAFETY: We are reconstructing full length and capacity of original vector,
        // using its original pointer, and the size of elements are identical.
        unsafe { Vec::from_raw_parts(pointer, length, capacity) }
    }

    #[allow(clippy::uninit_vec)]
    fn from_f32_slice(slice: &[f32]) -> Self {
        let mut vec = vec![f16::from_bits(0); slice.len()];
        vec.convert_from_f32_slice(slice);
        vec
    }

    #[allow(clippy::uninit_vec)]
    fn from_f64_slice(slice: &[f64]) -> Self {
        let mut vec = vec![f16::from_bits(0); slice.len()];
        vec.convert_from_f64_slice(slice);
        vec
    }
}

impl HalfFloatVecExt for Vec<bf16> {
    #[inline]
    fn reinterpret_into(mut self) -> Vec<u16> {
        // An f16 array has same length and capacity as u16 array
        let length = self.len();
        let capacity = self.capacity();

        // Actually reinterpret the contents of the Vec<f16> as u16,
        // knowing that structs are represented as only their members in memory,
        // which is the u16 part of `f16(u16)`
        let pointer = self.as_mut_ptr() as *mut u16;

        // Prevent running a destructor on the old Vec<u16>, so the pointer won't be deleted
        mem::forget(self);

        // Finally construct a new Vec<f16> from the raw pointer
        // SAFETY: We are reconstructing full length and capacity of original vector,
        // using its original pointer, and the size of elements are identical.
        unsafe { Vec::from_raw_parts(pointer, length, capacity) }
    }

    #[allow(clippy::uninit_vec)]
    fn from_f32_slice(slice: &[f32]) -> Self {
        let mut vec = vec![bf16::from_bits(0); slice.len()];
        vec.convert_from_f32_slice(slice);
        vec
    }

    #[allow(clippy::uninit_vec)]
    fn from_f64_slice(slice: &[f64]) -> Self {
        let mut vec = vec![bf16::from_bits(0); slice.len()];
        vec.convert_from_f64_slice(slice);
        vec
    }
}

impl HalfBitsVecExt for Vec<u16> {
    // This is safe because all traits are sealed
    #[inline]
    fn reinterpret_into<H>(mut self) -> Vec<H>
    where
        H: crate::private::SealedHalf,
    {
        // An f16 array has same length and capacity as u16 array
        let length = self.len();
        let capacity = self.capacity();

        // Actually reinterpret the contents of the Vec<u16> as f16,
        // knowing that structs are represented as only their members in memory,
        // which is the u16 part of `f16(u16)`
        let pointer = self.as_mut_ptr() as *mut H;

        // Prevent running a destructor on the old Vec<u16>, so the pointer won't be deleted
        mem::forget(self);

        // Finally construct a new Vec<f16> from the raw pointer
        // SAFETY: We are reconstructing full length and capacity of original vector,
        // using its original pointer, and the size of elements are identical.
        unsafe { Vec::from_raw_parts(pointer, length, capacity) }
    }
}

#[cfg(test)]
mod test {
    use super::{HalfBitsVecExt, HalfFloatVecExt};
    use crate::{bf16, f16};
    #[cfg(all(feature = "alloc", not(feature = "std")))]
    use alloc::vec;

    #[test]
    fn test_vec_conversions_f16() {
        let numbers = vec![f16::E, f16::PI, f16::EPSILON, f16::FRAC_1_SQRT_2];
        let bits = vec![
            f16::E.to_bits(),
            f16::PI.to_bits(),
            f16::EPSILON.to_bits(),
            f16::FRAC_1_SQRT_2.to_bits(),
        ];
        let bits_cloned = bits.clone();

        // Convert from bits to numbers
        let from_bits = bits.reinterpret_into::<f16>();
        assert_eq!(&from_bits[..], &numbers[..]);

        // Convert from numbers back to bits
        let to_bits = from_bits.reinterpret_into();
        assert_eq!(&to_bits[..], &bits_cloned[..]);
    }

    #[test]
    fn test_vec_conversions_bf16() {
        let numbers = vec![bf16::E, bf16::PI, bf16::EPSILON, bf16::FRAC_1_SQRT_2];
        let bits = vec![
            bf16::E.to_bits(),
            bf16::PI.to_bits(),
            bf16::EPSILON.to_bits(),
            bf16::FRAC_1_SQRT_2.to_bits(),
        ];
        let bits_cloned = bits.clone();

        // Convert from bits to numbers
        let from_bits = bits.reinterpret_into::<bf16>();
        assert_eq!(&from_bits[..], &numbers[..]);

        // Convert from numbers back to bits
        let to_bits = from_bits.reinterpret_into();
        assert_eq!(&to_bits[..], &bits_cloned[..]);
    }
}
