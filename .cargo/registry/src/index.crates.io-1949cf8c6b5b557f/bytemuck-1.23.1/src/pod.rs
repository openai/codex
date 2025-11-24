use super::*;

/// Marker trait for "plain old data".
///
/// The point of this trait is that once something is marked "plain old data"
/// you can really go to town with the bit fiddling and bit casting. Therefore,
/// it's a relatively strong claim to make about a type. Do not add this to your
/// type casually.
///
/// **Reminder:** The results of casting around bytes between data types are
/// _endian dependant_. Little-endian machines are the most common, but
/// big-endian machines do exist (and big-endian is also used for "network
/// order" bytes).
///
/// ## Safety
///
/// * The type must be inhabited (eg: no
///   [Infallible](core::convert::Infallible)).
/// * The type must allow any bit pattern (eg: no `bool` or `char`, which have
///   illegal bit patterns).
/// * The type must not contain any uninit (or padding) bytes, either in the
///   middle or on the end (eg: no `#[repr(C)] struct Foo(u8, u16)`, which has
///   padding in the middle, and also no `#[repr(C)] struct Foo(u16, u8)`, which
///   has padding on the end).
/// * The type needs to have all fields also be `Pod`.
/// * The type needs to be `repr(C)` or `repr(transparent)`. In the case of
///   `repr(C)`, the `packed` and `align` repr modifiers can be used as long as
///   all other rules end up being followed.
/// * It is disallowed for types to contain pointer types, `Cell`, `UnsafeCell`,
///   atomics, and any other forms of interior mutability.
/// * More precisely: A shared reference to the type must allow reads, and
///   *only* reads. RustBelt's separation logic is based on the notion that a
///   type is allowed to define a sharing predicate, its own invariant that must
///   hold for shared references, and this predicate is the reasoning that allow
///   it to deal with atomic and cells etc. We require the sharing predicate to
///   be trivial and permit only read-only access.
pub unsafe trait Pod: Zeroable + Copy + 'static {}

unsafe impl Pod for () {}
unsafe impl Pod for u8 {}
unsafe impl Pod for i8 {}
unsafe impl Pod for u16 {}
unsafe impl Pod for i16 {}
unsafe impl Pod for u32 {}
unsafe impl Pod for i32 {}
unsafe impl Pod for u64 {}
unsafe impl Pod for i64 {}
unsafe impl Pod for usize {}
unsafe impl Pod for isize {}
unsafe impl Pod for u128 {}
unsafe impl Pod for i128 {}
#[cfg(feature = "nightly_float")]
unsafe impl Pod for f16 {}
unsafe impl Pod for f32 {}
unsafe impl Pod for f64 {}
#[cfg(feature = "nightly_float")]
unsafe impl Pod for f128 {}
unsafe impl<T: Pod> Pod for Wrapping<T> {}

#[cfg(feature = "pod_saturating")]
unsafe impl<T: Pod> Pod for core::num::Saturating<T>{}

#[cfg(feature = "unsound_ptr_pod_impl")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "unsound_ptr_pod_impl"))
)]
unsafe impl<T: 'static> Pod for *mut T {}
#[cfg(feature = "unsound_ptr_pod_impl")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "unsound_ptr_pod_impl"))
)]
unsafe impl<T: 'static> Pod for *const T {}
#[cfg(feature = "unsound_ptr_pod_impl")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "unsound_ptr_pod_impl"))
)]
unsafe impl<T: 'static> PodInOption for NonNull<T> {}

unsafe impl<T: ?Sized + 'static> Pod for PhantomData<T> {}
unsafe impl Pod for PhantomPinned {}
unsafe impl<T: Pod> Pod for core::mem::ManuallyDrop<T> {}

// Note(Lokathor): MaybeUninit can NEVER be Pod.

#[cfg(feature = "min_const_generics")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "min_const_generics")))]
unsafe impl<T, const N: usize> Pod for [T; N] where T: Pod {}

#[cfg(not(feature = "min_const_generics"))]
impl_unsafe_marker_for_array!(
  Pod, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
  20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 48, 64, 96, 128, 256,
  512, 1024, 2048, 4096
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "wasm32", feature = "wasm_simd"))]
  unsafe impl Pod for wasm32::{v128}
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "aarch64", feature = "aarch64_simd"))]
  unsafe impl Pod for aarch64::{
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
  unsafe impl Pod for x86::{
    __m128i, __m128, __m128d,
    __m256i, __m256, __m256d,
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(target_arch = "x86_64")]
  unsafe impl Pod for x86_64::{
    __m128i, __m128, __m128d,
    __m256i, __m256, __m256d,
  }
);

#[cfg(feature = "nightly_portable_simd")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "nightly_portable_simd"))
)]
unsafe impl<T, const N: usize> Pod for core::simd::Simd<T, N>
where
  T: core::simd::SimdElement + Pod,
  core::simd::LaneCount<N>: core::simd::SupportedLaneCount,
{
}

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86", any(feature = "nightly_stdsimd", feature = "avx512_simd")))]
  unsafe impl Pod for x86::{
    __m512, __m512d, __m512i
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86_64", any(feature = "nightly_stdsimd", feature = "avx512_simd")))]
  unsafe impl Pod for x86_64::{
    __m512, __m512d, __m512i
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86", feature = "nightly_stdsimd"))]
  unsafe impl Pod for x86::{
    __m128bh, __m256bh, __m512bh
  }
);

impl_unsafe_marker_for_simd!(
  #[cfg(all(target_arch = "x86_64", feature = "nightly_stdsimd"))]
  unsafe impl Pod for x86_64::{
    __m128bh, __m256bh, __m512bh
  }
);
