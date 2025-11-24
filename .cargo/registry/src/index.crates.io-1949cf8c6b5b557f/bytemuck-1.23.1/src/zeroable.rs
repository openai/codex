use super::*;

/// Trait for types that can be safely created with
/// [`zeroed`](core::mem::zeroed).
///
/// An all-zeroes value may or may not be the same value as the
/// [Default](core::default::Default) value of the type.
///
/// ## Safety
///
/// * Your type must be inhabited (eg: no
///   [Infallible](core::convert::Infallible)).
/// * Your type must be allowed to be an "all zeroes" bit pattern (eg: no
///   [`NonNull<T>`](core::ptr::NonNull)).
///
/// ## Features
///
/// Some `impl`s are feature gated due to the MSRV policy:
///
/// * `MaybeUninit<T>` was not available in 1.34.0, but is available under the
///   `zeroable_maybe_uninit` feature flag.
/// * `Atomic*` types require Rust 1.60.0 or later to work on certain platforms,
///   but is available under the `zeroable_atomics` feature flag.
/// * `[T; N]` for arbitrary `N` requires the `min_const_generics` feature flag.
pub unsafe trait Zeroable: Sized {
  /// Calls [`zeroed`](core::mem::zeroed).
  ///
  /// This is a trait method so that you can write `MyType::zeroed()` in your
  /// code. It is a contract of this trait that if you implement it on your type
  /// you **must not** override this method.
  #[inline]
  fn zeroed() -> Self {
    unsafe { core::mem::zeroed() }
  }
}
unsafe impl Zeroable for () {}
unsafe impl Zeroable for bool {}
unsafe impl Zeroable for char {}
unsafe impl Zeroable for u8 {}
unsafe impl Zeroable for i8 {}
unsafe impl Zeroable for u16 {}
unsafe impl Zeroable for i16 {}
unsafe impl Zeroable for u32 {}
unsafe impl Zeroable for i32 {}
unsafe impl Zeroable for u64 {}
unsafe impl Zeroable for i64 {}
unsafe impl Zeroable for usize {}
unsafe impl Zeroable for isize {}
unsafe impl Zeroable for u128 {}
unsafe impl Zeroable for i128 {}
#[cfg(feature = "nightly_float")]
unsafe impl Zeroable for f16 {}
unsafe impl Zeroable for f32 {}
unsafe impl Zeroable for f64 {}
#[cfg(feature = "nightly_float")]
unsafe impl Zeroable for f128 {}
unsafe impl<T: Zeroable> Zeroable for Wrapping<T> {}
unsafe impl<T: Zeroable> Zeroable for core::cmp::Reverse<T> {}
#[cfg(feature = "pod_saturating")]
unsafe impl<T: Zeroable> Zeroable for core::num::Saturating<T> {}

// Note: we can't implement this for all `T: ?Sized` types because it would
// create NULL pointers for vtables.
// Maybe one day this could be changed to be implemented for
// `T: ?Sized where <T as core::ptr::Pointee>::Metadata: Zeroable`.
unsafe impl<T> Zeroable for *mut T {}
unsafe impl<T> Zeroable for *const T {}
unsafe impl<T> Zeroable for *mut [T] {}
unsafe impl<T> Zeroable for *const [T] {}
unsafe impl Zeroable for *mut str {}
unsafe impl Zeroable for *const str {}

unsafe impl<T: ?Sized> Zeroable for PhantomData<T> {}
unsafe impl Zeroable for PhantomPinned {}
unsafe impl<T: Zeroable> Zeroable for core::mem::ManuallyDrop<T> {}
unsafe impl<T: Zeroable> Zeroable for core::cell::UnsafeCell<T> {}
unsafe impl<T: Zeroable> Zeroable for core::cell::Cell<T> {}

#[cfg(feature = "zeroable_atomics")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "zeroable_atomics")))]
mod atomic_impls {
  use super::Zeroable;

  #[cfg(target_has_atomic = "8")]
  unsafe impl Zeroable for core::sync::atomic::AtomicBool {}
  #[cfg(target_has_atomic = "8")]
  unsafe impl Zeroable for core::sync::atomic::AtomicU8 {}
  #[cfg(target_has_atomic = "8")]
  unsafe impl Zeroable for core::sync::atomic::AtomicI8 {}

  #[cfg(target_has_atomic = "16")]
  unsafe impl Zeroable for core::sync::atomic::AtomicU16 {}
  #[cfg(target_has_atomic = "16")]
  unsafe impl Zeroable for core::sync::atomic::AtomicI16 {}

  #[cfg(target_has_atomic = "32")]
  unsafe impl Zeroable for core::sync::atomic::AtomicU32 {}
  #[cfg(target_has_atomic = "32")]
  unsafe impl Zeroable for core::sync::atomic::AtomicI32 {}

  #[cfg(target_has_atomic = "64")]
  unsafe impl Zeroable for core::sync::atomic::AtomicU64 {}
  #[cfg(target_has_atomic = "64")]
  unsafe impl Zeroable for core::sync::atomic::AtomicI64 {}

  #[cfg(target_has_atomic = "ptr")]
  unsafe impl Zeroable for core::sync::atomic::AtomicUsize {}
  #[cfg(target_has_atomic = "ptr")]
  unsafe impl Zeroable for core::sync::atomic::AtomicIsize {}

  #[cfg(target_has_atomic = "ptr")]
  unsafe impl<T> Zeroable for core::sync::atomic::AtomicPtr<T> {}
}

#[cfg(feature = "zeroable_maybe_uninit")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "zeroable_maybe_uninit"))
)]
unsafe impl<T> Zeroable for core::mem::MaybeUninit<T> {}

unsafe impl<A: Zeroable> Zeroable for (A,) {}
unsafe impl<A: Zeroable, B: Zeroable> Zeroable for (A, B) {}
unsafe impl<A: Zeroable, B: Zeroable, C: Zeroable> Zeroable for (A, B, C) {}
unsafe impl<A: Zeroable, B: Zeroable, C: Zeroable, D: Zeroable> Zeroable
  for (A, B, C, D)
{
}
unsafe impl<A: Zeroable, B: Zeroable, C: Zeroable, D: Zeroable, E: Zeroable>
  Zeroable for (A, B, C, D, E)
{
}
unsafe impl<
    A: Zeroable,
    B: Zeroable,
    C: Zeroable,
    D: Zeroable,
    E: Zeroable,
    F: Zeroable,
  > Zeroable for (A, B, C, D, E, F)
{
}
unsafe impl<
    A: Zeroable,
    B: Zeroable,
    C: Zeroable,
    D: Zeroable,
    E: Zeroable,
    F: Zeroable,
    G: Zeroable,
  > Zeroable for (A, B, C, D, E, F, G)
{
}
unsafe impl<
    A: Zeroable,
    B: Zeroable,
    C: Zeroable,
    D: Zeroable,
    E: Zeroable,
    F: Zeroable,
    G: Zeroable,
    H: Zeroable,
  > Zeroable for (A, B, C, D, E, F, G, H)
{
}

#[cfg(feature = "min_const_generics")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "min_const_generics")))]
unsafe impl<T, const N: usize> Zeroable for [T; N] where T: Zeroable {}

#[cfg(not(feature = "min_const_generics"))]
impl_unsafe_marker_for_array!(
  Zeroable, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
  19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 48, 64, 96, 128, 256,
  512, 1024, 2048, 4096
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "wasm32", feature = "wasm_simd"))]
  unsafe impl Zeroable for wasm32::{v128}
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "aarch64", feature = "aarch64_simd"))]
  unsafe impl Zeroable for aarch64::{
    float32x2_t, float32x2x2_t, float32x2x3_t, float32x2x4_t, float32x4_t,
    float32x4x2_t, float32x4x3_t, float32x4x4_t, float64x1_t, float64x1x2_t,
    float64x1x3_t, float64x1x4_t, float64x2_t, float64x2x2_t, float64x2x3_t,
    float64x2x4_t, int16x4_t, int16x4x2_t, int16x4x3_t, int16x4x4_t, int16x8_t,
    int16x8x2_t, int16x8x3_t, int16x8x4_t, int32x2_t, int32x2x2_t, int32x2x3_t,
    int32x2x4_t, int32x4_t, int32x4x2_t, int32x4x3_t, int32x4x4_t, int64x1_t,
    int64x1x2_t, int64x1x3_t, int64x1x4_t, int64x2_t, int64x2x2_t, int64x2x3_t,
    int64x2x4_t, int8x16_t, int8x16x2_t, int8x16x3_t, int8x16x4_t, int8x8_t,
    int8x8x2_t, int8x8x3_t, int8x8x4_t, poly16x4_t, poly16x4x2_t, poly16x4x3_t,
    poly16x4x4_t, poly16x8_t, poly16x8x2_t, poly16x8x3_t, poly16x8x4_t,
    poly64x1_t, poly64x1x2_t, poly64x1x3_t, poly64x1x4_t, poly64x2_t,
    poly64x2x2_t, poly64x2x3_t, poly64x2x4_t, poly8x16_t, poly8x16x2_t,
    poly8x16x3_t, poly8x16x4_t, poly8x8_t, poly8x8x2_t, poly8x8x3_t, poly8x8x4_t,
    uint16x4_t, uint16x4x2_t, uint16x4x3_t, uint16x4x4_t, uint16x8_t,
    uint16x8x2_t, uint16x8x3_t, uint16x8x4_t, uint32x2_t, uint32x2x2_t,
    uint32x2x3_t, uint32x2x4_t, uint32x4_t, uint32x4x2_t, uint32x4x3_t,
    uint32x4x4_t, uint64x1_t, uint64x1x2_t, uint64x1x3_t, uint64x1x4_t,
    uint64x2_t, uint64x2x2_t, uint64x2x3_t, uint64x2x4_t, uint8x16_t,
    uint8x16x2_t, uint8x16x3_t, uint8x16x4_t, uint8x8_t, uint8x8x2_t,
    uint8x8x3_t, uint8x8x4_t,
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(target_arch = "x86")]
  unsafe impl Zeroable for x86::{
    __m128i, __m128, __m128d,
    __m256i, __m256, __m256d,
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(target_arch = "x86_64")]
    unsafe impl Zeroable for x86_64::{
        __m128i, __m128, __m128d,
        __m256i, __m256, __m256d,
    }
);

#[cfg(feature = "nightly_portable_simd")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "nightly_portable_simd"))
)]
unsafe impl<T, const N: usize> Zeroable for core::simd::Simd<T, N>
where
  T: core::simd::SimdElement + Zeroable,
  core::simd::LaneCount<N>: core::simd::SupportedLaneCount,
{
}

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86", any(feature = "nightly_stdsimd", feature = "avx512_simd")))]
  unsafe impl Zeroable for x86::{
    __m512, __m512d, __m512i
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86_64", any(feature = "nightly_stdsimd", feature = "avx512_simd")))]
  unsafe impl Zeroable for x86_64::{
    __m512, __m512d, __m512i
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86", feature = "nightly_stdsimd"))]
  unsafe impl Zeroable for x86::{
    __m128bh, __m256bh, __m512bh
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86_64", feature = "nightly_stdsimd"))]
  unsafe impl Zeroable for x86_64::{
    __m128bh, __m256bh, __m512bh
  }
);
