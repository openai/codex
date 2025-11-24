#![no_std]
#![warn(missing_docs)]
#![allow(unused_mut)]
#![allow(clippy::match_like_matches_macro)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::type_complexity)]
#![cfg_attr(feature = "nightly_docs", feature(doc_cfg))]
#![cfg_attr(feature = "nightly_portable_simd", feature(portable_simd))]
#![cfg_attr(feature = "nightly_float", feature(f16, f128))]
#![cfg_attr(
  all(
    feature = "nightly_stdsimd",
    any(target_arch = "x86_64", target_arch = "x86")
  ),
  feature(stdarch_x86_avx512)
)]

//! This crate gives small utilities for casting between plain data types.
//!
//! ## Basics
//!
//! Data comes in five basic forms in Rust, so we have five basic casting
//! functions:
//!
//! * `T` uses [`cast`]
//! * `&T` uses [`cast_ref`]
//! * `&mut T` uses [`cast_mut`]
//! * `&[T]` uses [`cast_slice`]
//! * `&mut [T]` uses [`cast_slice_mut`]
//!
//! Depending on the function, the [`NoUninit`] and/or [`AnyBitPattern`] traits
//! are used to maintain memory safety.
//!
//! **Historical Note:** When the crate first started the [`Pod`] trait was used
//! instead, and so you may hear people refer to that, but it has the strongest
//! requirements and people eventually wanted the more fine-grained system, so
//! here we are. All types that impl `Pod` have a blanket impl to also support
//! `NoUninit` and `AnyBitPattern`. The traits unfortunately do not have a
//! perfectly clean hierarchy for semver reasons.
//!
//! ## Failures
//!
//! Some casts will never fail, and other casts might fail.
//!
//! * `cast::<u32, f32>` always works (and [`f32::from_bits`]).
//! * `cast_ref::<[u8; 4], u32>` might fail if the specific array reference
//!   given at runtime doesn't have alignment 4.
//!
//! In addition to the "normal" forms of each function, which will panic on
//! invalid input, there's also `try_` versions which will return a `Result`.
//!
//! If you would like to statically ensure that a cast will work at runtime you
//! can use the `must_cast` crate feature and the `must_` casting functions. A
//! "must cast" that can't be statically known to be valid will cause a
//! compilation error (and sometimes a very hard to read compilation error).
//!
//! ## Using Your Own Types
//!
//! All the functions listed above are guarded by the [`Pod`] trait, which is a
//! sub-trait of the [`Zeroable`] trait.
//!
//! If you enable the crate's `derive` feature then these traits can be derived
//! on your own types. The derive macros will perform the necessary checks on
//! your type declaration, and trigger an error if your type does not qualify.
//!
//! The derive macros might not cover all edge cases, and sometimes they will
//! error when actually everything is fine. As a last resort you can impl these
//! traits manually. However, these traits are `unsafe`, and you should
//! carefully read the requirements before using a manual implementation.
//!
//! ## Cargo Features
//!
//! The crate supports Rust 1.34 when no features are enabled, and so there's
//! cargo features for thing that you might consider "obvious".
//!
//! The cargo features **do not** promise any particular MSRV, and they may
//! increase their MSRV in new versions.
//!
//! * `derive`: Provide derive macros for the various traits.
//! * `extern_crate_alloc`: Provide utilities for `alloc` related types such as
//!   Box and Vec.
//! * `zeroable_maybe_uninit` and `zeroable_atomics`: Provide more [`Zeroable`]
//!   impls.
//! * `pod_saturating`: Provide more [`Pod`] and [`Zeroable`] impls.
//! * `wasm_simd` and `aarch64_simd`: Support more SIMD types.
//! * `min_const_generics`: Provides appropriate impls for arrays of all lengths
//!   instead of just for a select list of array lengths.
//! * `must_cast`: Provides the `must_` functions, which will compile error if
//!   the requested cast can't be statically verified.
//! * `const_zeroed`: Provides a const version of the `zeroed` function.
//!
//! ## Related Crates
//!
//! - [`pack1`](https://docs.rs/pack1), which contains `bytemuck`-compatible
//!   packed little-endian, big-endian and native-endian integer and floating
//!   point number types.

#[cfg(all(target_arch = "aarch64", feature = "aarch64_simd"))]
use core::arch::aarch64;
#[cfg(all(target_arch = "wasm32", feature = "wasm_simd"))]
use core::arch::wasm32;
#[cfg(target_arch = "x86")]
use core::arch::x86;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64;
//
use core::{
  marker::*,
  mem::{align_of, size_of},
  num::*,
  ptr::*,
};

// Used from macros to ensure we aren't using some locally defined name and
// actually are referencing libcore. This also would allow pre-2018 edition
// crates to use our macros, but I'm not sure how important that is.
#[doc(hidden)]
pub use ::core as __core;

#[cfg(not(feature = "min_const_generics"))]
macro_rules! impl_unsafe_marker_for_array {
  ( $marker:ident , $( $n:expr ),* ) => {
    $(unsafe impl<T> $marker for [T; $n] where T: $marker {})*
  }
}

/// A macro to transmute between two types without requiring knowing size
/// statically.
macro_rules! transmute {
  ($val:expr) => {
    ::core::mem::transmute_copy(&::core::mem::ManuallyDrop::new($val))
  };
  // This arm is for use in const contexts, where the borrow required to use
  // transmute_copy poses an issue since the compiler hedges that the type
  // being borrowed could have interior mutability.
  ($srcty:ty; $dstty:ty; $val:expr) => {{
    #[repr(C)]
    union Transmute<A, B> {
      src: ::core::mem::ManuallyDrop<A>,
      dst: ::core::mem::ManuallyDrop<B>,
    }
    ::core::mem::ManuallyDrop::into_inner(
      Transmute::<$srcty, $dstty> { src: ::core::mem::ManuallyDrop::new($val) }
        .dst,
    )
  }};
}

/// A macro to implement marker traits for various simd types.
/// #[allow(unused)] because the impls are only compiled on relevant platforms
/// with relevant cargo features enabled.
#[allow(unused)]
macro_rules! impl_unsafe_marker_for_simd {
  ($(#[cfg($cfg_predicate:meta)])? unsafe impl $trait:ident for $platform:ident :: {}) => {};
  ($(#[cfg($cfg_predicate:meta)])? unsafe impl $trait:ident for $platform:ident :: { $first_type:ident $(, $types:ident)* $(,)? }) => {
    $( #[cfg($cfg_predicate)] )?
    $( #[cfg_attr(feature = "nightly_docs", doc(cfg($cfg_predicate)))] )?
    unsafe impl $trait for $platform::$first_type {}
    $( #[cfg($cfg_predicate)] )? // To prevent recursion errors if nothing is going to be expanded anyway.
    impl_unsafe_marker_for_simd!($( #[cfg($cfg_predicate)] )? unsafe impl $trait for $platform::{ $( $types ),* });
  };
}

/// A macro for conditionally const-ifying a function.
/// #[allow(unused)] because currently it is only used with the `must_cast` feature.
#[allow(unused)]
macro_rules! maybe_const_fn {
  (
      #[cfg($cfg_predicate:meta)]
      $(#[$attr:meta])*
      $vis:vis $(unsafe $($unsafe:lifetime)?)? fn $name:ident $($rest:tt)*
  ) => {
      #[cfg($cfg_predicate)]
      $(#[$attr])*
      $vis const $(unsafe $($unsafe)?)? fn $name $($rest)*

      #[cfg(not($cfg_predicate))]
      $(#[$attr])*
      $vis $(unsafe $($unsafe)?)? fn $name $($rest)*
    };
}

#[cfg(feature = "extern_crate_std")]
extern crate std;

#[cfg(feature = "extern_crate_alloc")]
extern crate alloc;
#[cfg(feature = "extern_crate_alloc")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "extern_crate_alloc")))]
pub mod allocation;
#[cfg(feature = "extern_crate_alloc")]
pub use allocation::*;

mod anybitpattern;
pub use anybitpattern::*;

pub mod checked;
pub use checked::CheckedBitPattern;

mod internal;

mod zeroable;
pub use zeroable::*;
mod zeroable_in_option;
pub use zeroable_in_option::*;

mod pod;
pub use pod::*;
mod pod_in_option;
pub use pod_in_option::*;

#[cfg(feature = "must_cast")]
mod must;
#[cfg(feature = "must_cast")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "must_cast")))]
pub use must::*;

mod no_uninit;
pub use no_uninit::*;

mod contiguous;
pub use contiguous::*;

mod offset_of;
// ^ no import, the module only has a macro_rules, which are cursed and don't
// follow normal import/export rules.

mod transparent;
pub use transparent::*;

#[cfg(feature = "derive")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "derive")))]
pub use bytemuck_derive::{
  AnyBitPattern, ByteEq, ByteHash, CheckedBitPattern, Contiguous, NoUninit,
  Pod, TransparentWrapper, Zeroable,
};

/// The things that can go wrong when casting between [`Pod`] data forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PodCastError {
  /// You tried to cast a reference into a reference to a type with a higher
  /// alignment requirement but the input reference wasn't aligned.
  TargetAlignmentGreaterAndInputNotAligned,
  /// If the element size of a slice changes, then the output slice changes
  /// length accordingly. If the output slice wouldn't be a whole number of
  /// elements, then the conversion fails.
  OutputSliceWouldHaveSlop,
  /// When casting an individual `T`, `&T`, or `&mut T` value the
  /// source size and destination size must be an exact match.
  SizeMismatch,
  /// For this type of cast the alignments must be exactly the same and they
  /// were not so now you're sad.
  ///
  /// This error is generated **only** by operations that cast allocated types
  /// (such as `Box` and `Vec`), because in that case the alignment must stay
  /// exact.
  AlignmentMismatch,
}
#[cfg(not(target_arch = "spirv"))]
impl core::fmt::Display for PodCastError {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    write!(f, "{:?}", self)
  }
}
#[cfg(feature = "extern_crate_std")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "extern_crate_std")))]
impl std::error::Error for PodCastError {}

// Rust 1.81+
#[cfg(all(feature = "impl_core_error", not(feature = "extern_crate_std")))]
impl core::error::Error for PodCastError {}

/// Re-interprets `&T` as `&[u8]`.
///
/// Any ZST becomes an empty slice, and in that case the pointer value of that
/// empty slice might not match the pointer value of the input reference.
#[inline]
pub fn bytes_of<T: NoUninit>(t: &T) -> &[u8] {
  unsafe { internal::bytes_of(t) }
}

/// Re-interprets `&mut T` as `&mut [u8]`.
///
/// Any ZST becomes an empty slice, and in that case the pointer value of that
/// empty slice might not match the pointer value of the input reference.
#[inline]
pub fn bytes_of_mut<T: NoUninit + AnyBitPattern>(t: &mut T) -> &mut [u8] {
  unsafe { internal::bytes_of_mut(t) }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Panics
///
/// This is like [`try_from_bytes`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn from_bytes<T: AnyBitPattern>(s: &[u8]) -> &T {
  unsafe { internal::from_bytes(s) }
}

/// Re-interprets `&mut [u8]` as `&mut T`.
///
/// ## Panics
///
/// This is like [`try_from_bytes_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn from_bytes_mut<T: NoUninit + AnyBitPattern>(s: &mut [u8]) -> &mut T {
  unsafe { internal::from_bytes_mut(s) }
}

/// Reads from the bytes as if they were a `T`.
///
/// Unlike [`from_bytes`], the slice doesn't need to respect alignment of `T`,
/// only sizes must match.
///
/// ## Failure
/// * If the `bytes` length is not equal to `size_of::<T>()`.
#[inline]
pub fn try_pod_read_unaligned<T: AnyBitPattern>(
  bytes: &[u8],
) -> Result<T, PodCastError> {
  unsafe { internal::try_pod_read_unaligned(bytes) }
}

/// Reads the slice into a `T` value.
///
/// Unlike [`from_bytes`], the slice doesn't need to respect alignment of `T`,
/// only sizes must match.
///
/// ## Panics
/// * This is like `try_pod_read_unaligned` but will panic on failure.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn pod_read_unaligned<T: AnyBitPattern>(bytes: &[u8]) -> T {
  unsafe { internal::pod_read_unaligned(bytes) }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
#[inline]
pub fn try_from_bytes<T: AnyBitPattern>(s: &[u8]) -> Result<&T, PodCastError> {
  unsafe { internal::try_from_bytes(s) }
}

/// Re-interprets `&mut [u8]` as `&mut T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
#[inline]
pub fn try_from_bytes_mut<T: NoUninit + AnyBitPattern>(
  s: &mut [u8],
) -> Result<&mut T, PodCastError> {
  unsafe { internal::try_from_bytes_mut(s) }
}

/// Cast `A` into `B`
///
/// ## Panics
///
/// * This is like [`try_cast`], but will panic on a size mismatch.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast<A: NoUninit, B: AnyBitPattern>(a: A) -> B {
  unsafe { internal::cast(a) }
}

/// Cast `&mut A` into `&mut B`.
///
/// ## Panics
///
/// This is [`try_cast_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_mut<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(
  a: &mut A,
) -> &mut B {
  unsafe { internal::cast_mut(a) }
}

/// Cast `&A` into `&B`.
///
/// ## Panics
///
/// This is [`try_cast_ref`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_ref<A: NoUninit, B: AnyBitPattern>(a: &A) -> &B {
  unsafe { internal::cast_ref(a) }
}

/// Cast `&[A]` into `&[B]`.
///
/// ## Panics
///
/// This is [`try_cast_slice`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_slice<A: NoUninit, B: AnyBitPattern>(a: &[A]) -> &[B] {
  unsafe { internal::cast_slice(a) }
}

/// Cast `&mut [A]` into `&mut [B]`.
///
/// ## Panics
///
/// This is [`try_cast_slice_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_slice_mut<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  a: &mut [A],
) -> &mut [B] {
  unsafe { internal::cast_slice_mut(a) }
}

/// As [`align_to`](https://doc.rust-lang.org/std/primitive.slice.html#method.align_to),
/// but safe because of the [`Pod`] bound.
#[inline]
pub fn pod_align_to<T: NoUninit, U: AnyBitPattern>(
  vals: &[T],
) -> (&[T], &[U], &[T]) {
  unsafe { vals.align_to::<U>() }
}

/// As [`align_to_mut`](https://doc.rust-lang.org/std/primitive.slice.html#method.align_to_mut),
/// but safe because of the [`Pod`] bound.
#[inline]
pub fn pod_align_to_mut<
  T: NoUninit + AnyBitPattern,
  U: NoUninit + AnyBitPattern,
>(
  vals: &mut [T],
) -> (&mut [T], &mut [U], &mut [T]) {
  unsafe { vals.align_to_mut::<U>() }
}

/// Try to cast `A` into `B`.
///
/// Note that for this particular type of cast, alignment isn't a factor. The
/// input value is semantically copied into the function and then returned to a
/// new memory location which will have whatever the required alignment of the
/// output type is.
///
/// ## Failure
///
/// * If the types don't have the same size this fails.
#[inline]
pub fn try_cast<A: NoUninit, B: AnyBitPattern>(
  a: A,
) -> Result<B, PodCastError> {
  unsafe { internal::try_cast(a) }
}

/// Try to convert a `&A` into `&B`.
///
/// ## Failure
///
/// * If the reference isn't aligned in the new type
/// * If the source type and target type aren't the same size.
#[inline]
pub fn try_cast_ref<A: NoUninit, B: AnyBitPattern>(
  a: &A,
) -> Result<&B, PodCastError> {
  unsafe { internal::try_cast_ref(a) }
}

/// Try to convert a `&mut A` into `&mut B`.
///
/// As [`try_cast_ref`], but `mut`.
#[inline]
pub fn try_cast_mut<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  a: &mut A,
) -> Result<&mut B, PodCastError> {
  unsafe { internal::try_cast_mut(a) }
}

/// Try to convert `&[A]` into `&[B]` (possibly with a change in length).
///
/// * `input.as_ptr() as usize == output.as_ptr() as usize`
/// * `input.len() * size_of::<A>() == output.len() * size_of::<B>()`
///
/// ## Failure
///
/// * If the target type has a greater alignment requirement and the input slice
///   isn't aligned.
/// * If the target element type is a different size from the current element
///   type, and the output slice wouldn't be a whole number of elements when
///   accounting for the size change (eg: 3 `u16` values is 1.5 `u32` values, so
///   that's a failure).
/// * Similarly, you can't convert between a [ZST](https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts)
///   and a non-ZST.
#[inline]
pub fn try_cast_slice<A: NoUninit, B: AnyBitPattern>(
  a: &[A],
) -> Result<&[B], PodCastError> {
  unsafe { internal::try_cast_slice(a) }
}

/// Try to convert `&mut [A]` into `&mut [B]` (possibly with a change in
/// length).
///
/// As [`try_cast_slice`], but `&mut`.
#[inline]
pub fn try_cast_slice_mut<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  a: &mut [A],
) -> Result<&mut [B], PodCastError> {
  unsafe { internal::try_cast_slice_mut(a) }
}

/// Fill all bytes of `target` with zeroes (see [`Zeroable`]).
///
/// This is similar to `*target = Zeroable::zeroed()`, but guarantees that any
/// padding bytes in `target` are zeroed as well.
///
/// See also [`fill_zeroes`], if you have a slice rather than a single value.
#[inline]
pub fn write_zeroes<T: Zeroable>(target: &mut T) {
  struct EnsureZeroWrite<T>(*mut T);
  impl<T> Drop for EnsureZeroWrite<T> {
    #[inline(always)]
    fn drop(&mut self) {
      unsafe {
        core::ptr::write_bytes(self.0, 0u8, 1);
      }
    }
  }
  unsafe {
    let guard = EnsureZeroWrite(target);
    core::ptr::drop_in_place(guard.0);
    drop(guard);
  }
}

/// Fill all bytes of `slice` with zeroes (see [`Zeroable`]).
///
/// This is similar to `slice.fill(Zeroable::zeroed())`, but guarantees that any
/// padding bytes in `slice` are zeroed as well.
///
/// See also [`write_zeroes`], which zeroes all bytes of a single value rather
/// than a slice.
#[inline]
pub fn fill_zeroes<T: Zeroable>(slice: &mut [T]) {
  if core::mem::needs_drop::<T>() {
    // If `T` needs to be dropped then we have to do this one item at a time, in
    // case one of the intermediate drops does a panic.
    slice.iter_mut().for_each(write_zeroes);
  } else {
    // Otherwise we can be really fast and just fill everthing with zeros.
    let len = slice.len();
    unsafe { core::ptr::write_bytes(slice.as_mut_ptr(), 0u8, len) }
  }
}

/// Same as [`Zeroable::zeroed`], but as a `const fn` const.
#[cfg(feature = "const_zeroed")]
#[inline]
#[must_use]
pub const fn zeroed<T: Zeroable>() -> T {
  unsafe { core::mem::zeroed() }
}
