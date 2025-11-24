//! Contains utility functions and traits to convert between slices of [`u16`] bits and [`struct@f16`] or
//! [`struct@bf16`] numbers.
//!
//! The utility [`HalfBitsSliceExt`] sealed extension trait is implemented for `[u16]` slices,
//! while the utility [`HalfFloatSliceExt`] sealed extension trait is implemented for both `[f16]`
//! and `[bf16]` slices. These traits provide efficient conversions and reinterpret casting of
//! larger buffers of floating point values, and are automatically included in the
//! [`prelude`][crate::prelude] module.

use crate::{bf16, binary16::arch, f16};
#[cfg(feature = "alloc")]
#[allow(unused_imports)]
use alloc::{vec, vec::Vec};
use core::slice;

/// Extensions to `[f16]` and `[bf16]` slices to support conversion and reinterpret operations.
///
/// This trait is sealed and cannot be implemented outside of this crate.
pub trait HalfFloatSliceExt: private::SealedHalfFloatSlice {
    /// Reinterprets a slice of [`struct@f16`] or [`struct@bf16`] numbers as a slice of [`u16`] bits.
    ///
    /// This is a zero-copy operation. The reinterpreted slice has the same lifetime and memory
    /// location as `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let float_buffer = [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)];
    /// let int_buffer = float_buffer.reinterpret_cast();
    ///
    /// assert_eq!(int_buffer, [float_buffer[0].to_bits(), float_buffer[1].to_bits(), float_buffer[2].to_bits()]);
    /// ```
    #[must_use]
    fn reinterpret_cast(&self) -> &[u16];

    /// Reinterprets a mutable slice of [`struct@f16`] or [`struct@bf16`] numbers as a mutable slice of [`u16`].
    /// bits
    ///
    /// This is a zero-copy operation. The transmuted slice has the same lifetime as the original,
    /// which prevents mutating `self` as long as the returned `&mut [u16]` is borrowed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let mut float_buffer = [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)];
    ///
    /// {
    ///     let int_buffer = float_buffer.reinterpret_cast_mut();
    ///
    ///     assert_eq!(int_buffer, [f16::from_f32(1.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()]);
    ///
    ///     // Mutating the u16 slice will mutating the original
    ///     int_buffer[0] = 0;
    /// }
    ///
    /// // Note that we need to drop int_buffer before using float_buffer again or we will get a borrow error.
    /// assert_eq!(float_buffer, [f16::from_f32(0.), f16::from_f32(2.), f16::from_f32(3.)]);
    /// ```
    #[must_use]
    fn reinterpret_cast_mut(&mut self) -> &mut [u16];

    /// Converts all of the elements of a `[f32]` slice into [`struct@f16`] or [`struct@bf16`] values in `self`.
    ///
    /// The length of `src` must be the same as `self`.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// // Initialize an empty buffer
    /// let mut buffer = [0u16; 4];
    /// let buffer = buffer.reinterpret_cast_mut::<f16>();
    ///
    /// let float_values = [1., 2., 3., 4.];
    ///
    /// // Now convert
    /// buffer.convert_from_f32_slice(&float_values);
    ///
    /// assert_eq!(buffer, [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.), f16::from_f32(4.)]);
    /// ```
    fn convert_from_f32_slice(&mut self, src: &[f32]);

    /// Converts all of the elements of a `[f64]` slice into [`struct@f16`] or [`struct@bf16`] values in `self`.
    ///
    /// The length of `src` must be the same as `self`.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// // Initialize an empty buffer
    /// let mut buffer = [0u16; 4];
    /// let buffer = buffer.reinterpret_cast_mut::<f16>();
    ///
    /// let float_values = [1., 2., 3., 4.];
    ///
    /// // Now convert
    /// buffer.convert_from_f64_slice(&float_values);
    ///
    /// assert_eq!(buffer, [f16::from_f64(1.), f16::from_f64(2.), f16::from_f64(3.), f16::from_f64(4.)]);
    /// ```
    fn convert_from_f64_slice(&mut self, src: &[f64]);

    /// Converts all of the [`struct@f16`] or [`struct@bf16`] elements of `self` into [`f32`] values in `dst`.
    ///
    /// The length of `src` must be the same as `self`.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// // Initialize an empty buffer
    /// let mut buffer = [0f32; 4];
    ///
    /// let half_values = [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.), f16::from_f32(4.)];
    ///
    /// // Now convert
    /// half_values.convert_to_f32_slice(&mut buffer);
    ///
    /// assert_eq!(buffer, [1., 2., 3., 4.]);
    /// ```
    fn convert_to_f32_slice(&self, dst: &mut [f32]);

    /// Converts all of the [`struct@f16`] or [`struct@bf16`] elements of `self` into [`f64`] values in `dst`.
    ///
    /// The length of `src` must be the same as `self`.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// # Panics
    ///
    /// This function will panic if the two slices have different lengths.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// // Initialize an empty buffer
    /// let mut buffer = [0f64; 4];
    ///
    /// let half_values = [f16::from_f64(1.), f16::from_f64(2.), f16::from_f64(3.), f16::from_f64(4.)];
    ///
    /// // Now convert
    /// half_values.convert_to_f64_slice(&mut buffer);
    ///
    /// assert_eq!(buffer, [1., 2., 3., 4.]);
    /// ```
    fn convert_to_f64_slice(&self, dst: &mut [f64]);

    // Because trait is sealed, we can get away with different interfaces between features.

    /// Converts all of the [`struct@f16`] or [`struct@bf16`] elements of `self` into [`f32`] values in a new
    /// vector
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// This method is only available with the `std` or `alloc` feature.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// let half_values = [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.), f16::from_f32(4.)];
    /// let vec = half_values.to_f32_vec();
    ///
    /// assert_eq!(vec, vec![1., 2., 3., 4.]);
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    #[must_use]
    fn to_f32_vec(&self) -> Vec<f32>;

    /// Converts all of the [`struct@f16`] or [`struct@bf16`] elements of `self` into [`f64`] values in a new
    /// vector.
    ///
    /// The conversion operation is vectorized over the slice, meaning the conversion may be more
    /// efficient than converting individual elements on some hardware that supports SIMD
    /// conversions. See [crate documentation](crate) for more information on hardware conversion
    /// support.
    ///
    /// This method is only available with the `std` or `alloc` feature.
    ///
    /// # Examples
    /// ```rust
    /// # use half::prelude::*;
    /// let half_values = [f16::from_f64(1.), f16::from_f64(2.), f16::from_f64(3.), f16::from_f64(4.)];
    /// let vec = half_values.to_f64_vec();
    ///
    /// assert_eq!(vec, vec![1., 2., 3., 4.]);
    /// ```
    #[cfg(feature = "alloc")]
    #[must_use]
    fn to_f64_vec(&self) -> Vec<f64>;
}

/// Extensions to `[u16]` slices to support reinterpret operations.
///
/// This trait is sealed and cannot be implemented outside of this crate.
pub trait HalfBitsSliceExt: private::SealedHalfBitsSlice {
    /// Reinterprets a slice of [`u16`] bits as a slice of [`struct@f16`] or [`struct@bf16`] numbers.
    ///
    /// `H` is the type to cast to, and must be either the [`struct@f16`] or [`struct@bf16`] type.
    ///
    /// This is a zero-copy operation. The reinterpreted slice has the same lifetime and memory
    /// location as `self`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let int_buffer = [f16::from_f32(1.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()];
    /// let float_buffer: &[f16] = int_buffer.reinterpret_cast();
    ///
    /// assert_eq!(float_buffer, [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)]);
    ///
    /// // You may have to specify the cast type directly if the compiler can't infer the type.
    /// // The following is also valid in Rust.
    /// let typed_buffer = int_buffer.reinterpret_cast::<f16>();
    /// ```
    #[must_use]
    fn reinterpret_cast<H>(&self) -> &[H]
    where
        H: crate::private::SealedHalf;

    /// Reinterprets a mutable slice of [`u16`] bits as a mutable slice of [`struct@f16`] or [`struct@bf16`]
    /// numbers.
    ///
    /// `H` is the type to cast to, and must be either the [`struct@f16`] or [`struct@bf16`] type.
    ///
    /// This is a zero-copy operation. The transmuted slice has the same lifetime as the original,
    /// which prevents mutating `self` as long as the returned `&mut [f16]` is borrowed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use half::prelude::*;
    /// let mut int_buffer = [f16::from_f32(1.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()];
    ///
    /// {
    ///     let float_buffer: &mut [f16] = int_buffer.reinterpret_cast_mut();
    ///
    ///     assert_eq!(float_buffer, [f16::from_f32(1.), f16::from_f32(2.), f16::from_f32(3.)]);
    ///
    ///     // Mutating the f16 slice will mutating the original
    ///     float_buffer[0] = f16::from_f32(0.);
    /// }
    ///
    /// // Note that we need to drop float_buffer before using int_buffer again or we will get a borrow error.
    /// assert_eq!(int_buffer, [f16::from_f32(0.).to_bits(), f16::from_f32(2.).to_bits(), f16::from_f32(3.).to_bits()]);
    ///
    /// // You may have to specify the cast type directly if the compiler can't infer the type.
    /// // The following is also valid in Rust.
    /// let typed_buffer = int_buffer.reinterpret_cast_mut::<f16>();
    /// ```
    #[must_use]
    fn reinterpret_cast_mut<H>(&mut self) -> &mut [H]
    where
        H: crate::private::SealedHalf;
}

mod private {
    use crate::{bf16, f16};

    pub trait SealedHalfFloatSlice {}
    impl SealedHalfFloatSlice for [f16] {}
    impl SealedHalfFloatSlice for [bf16] {}

    pub trait SealedHalfBitsSlice {}
    impl SealedHalfBitsSlice for [u16] {}
}

impl HalfFloatSliceExt for [f16] {
    #[inline]
    fn reinterpret_cast(&self) -> &[u16] {
        let pointer = self.as_ptr() as *const u16;
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts(pointer, length) }
    }

    #[inline]
    fn reinterpret_cast_mut(&mut self) -> &mut [u16] {
        let pointer = self.as_mut_ptr().cast::<u16>();
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts_mut(pointer, length) }
    }

    #[inline]
    fn convert_from_f32_slice(&mut self, src: &[f32]) {
        assert_eq!(
            self.len(),
            src.len(),
            "destination and source slices have different lengths"
        );

        arch::f32_to_f16_slice(src, self.reinterpret_cast_mut())
    }

    #[inline]
    fn convert_from_f64_slice(&mut self, src: &[f64]) {
        assert_eq!(
            self.len(),
            src.len(),
            "destination and source slices have different lengths"
        );

        arch::f64_to_f16_slice(src, self.reinterpret_cast_mut())
    }

    #[inline]
    fn convert_to_f32_slice(&self, dst: &mut [f32]) {
        assert_eq!(
            self.len(),
            dst.len(),
            "destination and source slices have different lengths"
        );

        arch::f16_to_f32_slice(self.reinterpret_cast(), dst)
    }

    #[inline]
    fn convert_to_f64_slice(&self, dst: &mut [f64]) {
        assert_eq!(
            self.len(),
            dst.len(),
            "destination and source slices have different lengths"
        );

        arch::f16_to_f64_slice(self.reinterpret_cast(), dst)
    }

    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    #[allow(clippy::uninit_vec)]
    fn to_f32_vec(&self) -> Vec<f32> {
        let mut vec = vec![0f32; self.len()];
        self.convert_to_f32_slice(&mut vec);
        vec
    }

    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    #[allow(clippy::uninit_vec)]
    fn to_f64_vec(&self) -> Vec<f64> {
        let mut vec = vec![0f64; self.len()];
        self.convert_to_f64_slice(&mut vec);
        vec
    }
}

impl HalfFloatSliceExt for [bf16] {
    #[inline]
    fn reinterpret_cast(&self) -> &[u16] {
        let pointer = self.as_ptr() as *const u16;
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts(pointer, length) }
    }

    #[inline]
    fn reinterpret_cast_mut(&mut self) -> &mut [u16] {
        let pointer = self.as_mut_ptr().cast::<u16>();
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts_mut(pointer, length) }
    }

    #[inline]
    fn convert_from_f32_slice(&mut self, src: &[f32]) {
        assert_eq!(
            self.len(),
            src.len(),
            "destination and source slices have different lengths"
        );

        // Just use regular loop here until there's any bf16 SIMD support.
        for (i, f) in src.iter().enumerate() {
            self[i] = bf16::from_f32(*f);
        }
    }

    #[inline]
    fn convert_from_f64_slice(&mut self, src: &[f64]) {
        assert_eq!(
            self.len(),
            src.len(),
            "destination and source slices have different lengths"
        );

        // Just use regular loop here until there's any bf16 SIMD support.
        for (i, f) in src.iter().enumerate() {
            self[i] = bf16::from_f64(*f);
        }
    }

    #[inline]
    fn convert_to_f32_slice(&self, dst: &mut [f32]) {
        assert_eq!(
            self.len(),
            dst.len(),
            "destination and source slices have different lengths"
        );

        // Just use regular loop here until there's any bf16 SIMD support.
        for (i, f) in self.iter().enumerate() {
            dst[i] = f.to_f32();
        }
    }

    #[inline]
    fn convert_to_f64_slice(&self, dst: &mut [f64]) {
        assert_eq!(
            self.len(),
            dst.len(),
            "destination and source slices have different lengths"
        );

        // Just use regular loop here until there's any bf16 SIMD support.
        for (i, f) in self.iter().enumerate() {
            dst[i] = f.to_f64();
        }
    }

    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    #[allow(clippy::uninit_vec)]
    fn to_f32_vec(&self) -> Vec<f32> {
        let mut vec = vec![0f32; self.len()];
        self.convert_to_f32_slice(&mut vec);
        vec
    }

    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    #[allow(clippy::uninit_vec)]
    fn to_f64_vec(&self) -> Vec<f64> {
        let mut vec = vec![0f64; self.len()];
        self.convert_to_f64_slice(&mut vec);
        vec
    }
}

impl HalfBitsSliceExt for [u16] {
    // Since we sealed all the traits involved, these are safe.
    #[inline]
    fn reinterpret_cast<H>(&self) -> &[H]
    where
        H: crate::private::SealedHalf,
    {
        let pointer = self.as_ptr() as *const H;
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts(pointer, length) }
    }

    #[inline]
    fn reinterpret_cast_mut<H>(&mut self) -> &mut [H]
    where
        H: crate::private::SealedHalf,
    {
        let pointer = self.as_mut_ptr() as *mut H;
        let length = self.len();
        // SAFETY: We are reconstructing full length of original slice, using its same lifetime,
        // and the size of elements are identical
        unsafe { slice::from_raw_parts_mut(pointer, length) }
    }
}

#[allow(clippy::float_cmp)]
#[cfg(test)]
mod test {
    use super::{HalfBitsSliceExt, HalfFloatSliceExt};
    use crate::{bf16, f16};

    #[test]
    fn test_slice_conversions_f16() {
        let bits = &[
            f16::E.to_bits(),
            f16::PI.to_bits(),
            f16::EPSILON.to_bits(),
            f16::FRAC_1_SQRT_2.to_bits(),
        ];
        let numbers = &[f16::E, f16::PI, f16::EPSILON, f16::FRAC_1_SQRT_2];

        // Convert from bits to numbers
        let from_bits = bits.reinterpret_cast::<f16>();
        assert_eq!(from_bits, numbers);

        // Convert from numbers back to bits
        let to_bits = from_bits.reinterpret_cast();
        assert_eq!(to_bits, bits);
    }

    #[test]
    fn test_mutablility_f16() {
        let mut bits_array = [f16::PI.to_bits()];
        let bits = &mut bits_array[..];

        {
            // would not compile without these braces
            let numbers = bits.reinterpret_cast_mut();
            numbers[0] = f16::E;
        }

        assert_eq!(bits, &[f16::E.to_bits()]);

        bits[0] = f16::LN_2.to_bits();
        assert_eq!(bits, &[f16::LN_2.to_bits()]);
    }

    #[test]
    fn test_slice_conversions_bf16() {
        let bits = &[
            bf16::E.to_bits(),
            bf16::PI.to_bits(),
            bf16::EPSILON.to_bits(),
            bf16::FRAC_1_SQRT_2.to_bits(),
        ];
        let numbers = &[bf16::E, bf16::PI, bf16::EPSILON, bf16::FRAC_1_SQRT_2];

        // Convert from bits to numbers
        let from_bits = bits.reinterpret_cast::<bf16>();
        assert_eq!(from_bits, numbers);

        // Convert from numbers back to bits
        let to_bits = from_bits.reinterpret_cast();
        assert_eq!(to_bits, bits);
    }

    #[test]
    fn test_mutablility_bf16() {
        let mut bits_array = [bf16::PI.to_bits()];
        let bits = &mut bits_array[..];

        {
            // would not compile without these braces
            let numbers = bits.reinterpret_cast_mut();
            numbers[0] = bf16::E;
        }

        assert_eq!(bits, &[bf16::E.to_bits()]);

        bits[0] = bf16::LN_2.to_bits();
        assert_eq!(bits, &[bf16::LN_2.to_bits()]);
    }

    #[test]
    fn slice_convert_f16_f32() {
        // Exact chunks
        let vf32 = [1., 2., 3., 4., 5., 6., 7., 8.];
        let vf16 = [
            f16::from_f32(1.),
            f16::from_f32(2.),
            f16::from_f32(3.),
            f16::from_f32(4.),
            f16::from_f32(5.),
            f16::from_f32(6.),
            f16::from_f32(7.),
            f16::from_f32(8.),
        ];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf32 = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
        let vf16 = [
            f16::from_f32(1.),
            f16::from_f32(2.),
            f16::from_f32(3.),
            f16::from_f32(4.),
            f16::from_f32(5.),
            f16::from_f32(6.),
            f16::from_f32(7.),
            f16::from_f32(8.),
            f16::from_f32(9.),
        ];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf32 = [1., 2.];
        let vf16 = [f16::from_f32(1.), f16::from_f32(2.)];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);
    }

    #[test]
    fn slice_convert_bf16_f32() {
        // Exact chunks
        let vf32 = [1., 2., 3., 4., 5., 6., 7., 8.];
        let vf16 = [
            bf16::from_f32(1.),
            bf16::from_f32(2.),
            bf16::from_f32(3.),
            bf16::from_f32(4.),
            bf16::from_f32(5.),
            bf16::from_f32(6.),
            bf16::from_f32(7.),
            bf16::from_f32(8.),
        ];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf32 = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
        let vf16 = [
            bf16::from_f32(1.),
            bf16::from_f32(2.),
            bf16::from_f32(3.),
            bf16::from_f32(4.),
            bf16::from_f32(5.),
            bf16::from_f32(6.),
            bf16::from_f32(7.),
            bf16::from_f32(8.),
            bf16::from_f32(9.),
        ];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf32 = [1., 2.];
        let vf16 = [bf16::from_f32(1.), bf16::from_f32(2.)];
        let mut buf32 = vf32;
        let mut buf16 = vf16;

        vf16.convert_to_f32_slice(&mut buf32);
        assert_eq!(&vf32, &buf32);

        buf16.convert_from_f32_slice(&vf32);
        assert_eq!(&vf16, &buf16);
    }

    #[test]
    fn slice_convert_f16_f64() {
        // Exact chunks
        let vf64 = [1., 2., 3., 4., 5., 6., 7., 8.];
        let vf16 = [
            f16::from_f64(1.),
            f16::from_f64(2.),
            f16::from_f64(3.),
            f16::from_f64(4.),
            f16::from_f64(5.),
            f16::from_f64(6.),
            f16::from_f64(7.),
            f16::from_f64(8.),
        ];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf64 = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
        let vf16 = [
            f16::from_f64(1.),
            f16::from_f64(2.),
            f16::from_f64(3.),
            f16::from_f64(4.),
            f16::from_f64(5.),
            f16::from_f64(6.),
            f16::from_f64(7.),
            f16::from_f64(8.),
            f16::from_f64(9.),
        ];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf64 = [1., 2.];
        let vf16 = [f16::from_f64(1.), f16::from_f64(2.)];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);
    }

    #[test]
    fn slice_convert_bf16_f64() {
        // Exact chunks
        let vf64 = [1., 2., 3., 4., 5., 6., 7., 8.];
        let vf16 = [
            bf16::from_f64(1.),
            bf16::from_f64(2.),
            bf16::from_f64(3.),
            bf16::from_f64(4.),
            bf16::from_f64(5.),
            bf16::from_f64(6.),
            bf16::from_f64(7.),
            bf16::from_f64(8.),
        ];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf64 = [1., 2., 3., 4., 5., 6., 7., 8., 9.];
        let vf16 = [
            bf16::from_f64(1.),
            bf16::from_f64(2.),
            bf16::from_f64(3.),
            bf16::from_f64(4.),
            bf16::from_f64(5.),
            bf16::from_f64(6.),
            bf16::from_f64(7.),
            bf16::from_f64(8.),
            bf16::from_f64(9.),
        ];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);

        // Partial with chunks
        let vf64 = [1., 2.];
        let vf16 = [bf16::from_f64(1.), bf16::from_f64(2.)];
        let mut buf64 = vf64;
        let mut buf16 = vf16;

        vf16.convert_to_f64_slice(&mut buf64);
        assert_eq!(&vf64, &buf64);

        buf16.convert_from_f64_slice(&vf64);
        assert_eq!(&vf16, &buf16);
    }

    #[test]
    #[should_panic]
    fn convert_from_f32_slice_len_mismatch_panics() {
        let mut slice1 = [f16::ZERO; 3];
        let slice2 = [0f32; 4];
        slice1.convert_from_f32_slice(&slice2);
    }

    #[test]
    #[should_panic]
    fn convert_from_f64_slice_len_mismatch_panics() {
        let mut slice1 = [f16::ZERO; 3];
        let slice2 = [0f64; 4];
        slice1.convert_from_f64_slice(&slice2);
    }

    #[test]
    #[should_panic]
    fn convert_to_f32_slice_len_mismatch_panics() {
        let slice1 = [f16::ZERO; 3];
        let mut slice2 = [0f32; 4];
        slice1.convert_to_f32_slice(&mut slice2);
    }

    #[test]
    #[should_panic]
    fn convert_to_f64_slice_len_mismatch_panics() {
        let slice1 = [f16::ZERO; 3];
        let mut slice2 = [0f64; 4];
        slice1.convert_to_f64_slice(&mut slice2);
    }
}
