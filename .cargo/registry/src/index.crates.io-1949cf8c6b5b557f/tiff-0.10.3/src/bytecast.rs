//! Trivial, internal byte transmutation.
//!
//! A dependency like bytemuck would give us extra assurance of the safety but overall would not
//! reduce the amount of total unsafety. We don't use it in the interface where the traits would
//! really become useful.
//!
//! SAFETY: These are benign casts as we apply them to fixed size integer types only. All of them
//! are naturally aligned, valid for all bit patterns and their alignment is surely at most their
//! size (we assert the latter fact since it is 'implementation defined' if following the letter of
//! the unsafe code guidelines).
//!
//! TODO: Would like to use std-lib here.
// Until we implement predictors for f16, we don't need f16_as_ne_bytes. (Due to the macro we do
// not apply this directly to the functions). And rust-version is not new enough for `expect`.
#![allow(dead_code)]
use std::{mem, slice};

use half::f16;

macro_rules! integral_slice_as_bytes{($int:ty, $const:ident $(,$mut:ident)*) => {
    pub(crate) fn $const(slice: &[$int]) -> &[u8] {
        assert!(mem::align_of::<$int>() <= mem::size_of::<$int>());
        unsafe { slice::from_raw_parts(slice.as_ptr() as *const u8, mem::size_of_val(slice)) }
    }
    $(pub(crate) fn $mut(slice: &mut [$int]) -> &mut [u8] {
        assert!(mem::align_of::<$int>() <= mem::size_of::<$int>());
        unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr() as *mut u8, mem::size_of_val(slice)) }
    })*
}}

integral_slice_as_bytes!(i8, i8_as_ne_bytes, i8_as_ne_mut_bytes);
integral_slice_as_bytes!(u16, u16_as_ne_bytes, u16_as_ne_mut_bytes);
integral_slice_as_bytes!(i16, i16_as_ne_bytes, i16_as_ne_mut_bytes);
integral_slice_as_bytes!(u32, u32_as_ne_bytes, u32_as_ne_mut_bytes);
integral_slice_as_bytes!(i32, i32_as_ne_bytes, i32_as_ne_mut_bytes);
integral_slice_as_bytes!(u64, u64_as_ne_bytes, u64_as_ne_mut_bytes);
integral_slice_as_bytes!(i64, i64_as_ne_bytes, i64_as_ne_mut_bytes);
integral_slice_as_bytes!(f32, f32_as_ne_bytes, f32_as_ne_mut_bytes);
integral_slice_as_bytes!(f16, f16_as_ne_bytes, f16_as_ne_mut_bytes);
integral_slice_as_bytes!(f64, f64_as_ne_bytes, f64_as_ne_mut_bytes);
