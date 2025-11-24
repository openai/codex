//! Internal implementation of casting functions not bound by marker traits
//! and therefore marked as unsafe. This is used so that we don't need to
//! duplicate the business logic contained in these functions between the
//! versions exported in the crate root, `checked`, and `relaxed` modules.
#![allow(unused_unsafe)]

use crate::PodCastError;
use core::{marker::*, mem::*};

/*

Note(Lokathor): We've switched all of the `unwrap` to `match` because there is
apparently a bug: https://github.com/rust-lang/rust/issues/68667
and it doesn't seem to show up in simple godbolt examples but has been reported
as having an impact when there's a cast mixed in with other more complicated
code around it. Rustc/LLVM ends up missing that the `Err` can't ever happen for
particular type combinations, and then it doesn't fully eliminated the panic
possibility code branch.

*/

/// Immediately panics.
#[cfg(not(target_arch = "spirv"))]
#[cold]
#[inline(never)]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) fn something_went_wrong<D: core::fmt::Display>(
  _src: &str, _err: D,
) -> ! {
  // Note(Lokathor): Keeping the panic here makes the panic _formatting_ go
  // here too, which helps assembly readability and also helps keep down
  // the inline pressure.
  panic!("{src}>{err}", src = _src, err = _err);
}

/// Immediately panics.
#[cfg(target_arch = "spirv")]
#[cold]
#[inline(never)]
pub(crate) fn something_went_wrong<D>(_src: &str, _err: D) -> ! {
  // Note: On the spirv targets from [rust-gpu](https://github.com/EmbarkStudios/rust-gpu)
  // panic formatting cannot be used. We we just give a generic error message
  // The chance that the panicking version of these functions will ever get
  // called on spir-v targets with invalid inputs is small, but giving a
  // simple error message is better than no error message at all.
  panic!("Called a panicing helper from bytemuck which paniced");
}

/// Re-interprets `&T` as `&[u8]`.
///
/// Any ZST becomes an empty slice, and in that case the pointer value of that
/// empty slice might not match the pointer value of the input reference.
#[inline(always)]
pub(crate) unsafe fn bytes_of<T: Copy>(t: &T) -> &[u8] {
  match try_cast_slice::<T, u8>(core::slice::from_ref(t)) {
    Ok(s) => s,
    Err(_) => unreachable!(),
  }
}

/// Re-interprets `&mut T` as `&mut [u8]`.
///
/// Any ZST becomes an empty slice, and in that case the pointer value of that
/// empty slice might not match the pointer value of the input reference.
#[inline]
pub(crate) unsafe fn bytes_of_mut<T: Copy>(t: &mut T) -> &mut [u8] {
  match try_cast_slice_mut::<T, u8>(core::slice::from_mut(t)) {
    Ok(s) => s,
    Err(_) => unreachable!(),
  }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Panics
///
/// This is [`try_from_bytes`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn from_bytes<T: Copy>(s: &[u8]) -> &T {
  match try_from_bytes(s) {
    Ok(t) => t,
    Err(e) => something_went_wrong("from_bytes", e),
  }
}

/// Re-interprets `&mut [u8]` as `&mut T`.
///
/// ## Panics
///
/// This is [`try_from_bytes_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn from_bytes_mut<T: Copy>(s: &mut [u8]) -> &mut T {
  match try_from_bytes_mut(s) {
    Ok(t) => t,
    Err(e) => something_went_wrong("from_bytes_mut", e),
  }
}

/// Reads from the bytes as if they were a `T`.
///
/// ## Failure
/// * If the `bytes` length is not equal to `size_of::<T>()`.
#[inline]
pub(crate) unsafe fn try_pod_read_unaligned<T: Copy>(
  bytes: &[u8],
) -> Result<T, PodCastError> {
  if bytes.len() != size_of::<T>() {
    Err(PodCastError::SizeMismatch)
  } else {
    Ok(unsafe { (bytes.as_ptr() as *const T).read_unaligned() })
  }
}

/// Reads the slice into a `T` value.
///
/// ## Panics
/// * This is like `try_pod_read_unaligned` but will panic on failure.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn pod_read_unaligned<T: Copy>(bytes: &[u8]) -> T {
  match try_pod_read_unaligned(bytes) {
    Ok(t) => t,
    Err(e) => something_went_wrong("pod_read_unaligned", e),
  }
}

/// Checks if `ptr` is aligned to an `align` memory boundary.
///
/// ## Panics
/// * If `align` is not a power of two. This includes when `align` is zero.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) fn is_aligned_to(ptr: *const (), align: usize) -> bool {
  #[cfg(feature = "align_offset")]
  {
    // This is in a way better than `ptr as usize % align == 0`,
    // because casting a pointer to an integer has the side effect that it
    // exposes the pointer's provenance, which may theoretically inhibit
    // some compiler optimizations.
    ptr.align_offset(align) == 0
  }
  #[cfg(not(feature = "align_offset"))]
  {
    ((ptr as usize) % align) == 0
  }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
#[inline]
pub(crate) unsafe fn try_from_bytes<T: Copy>(
  s: &[u8],
) -> Result<&T, PodCastError> {
  if s.len() != size_of::<T>() {
    Err(PodCastError::SizeMismatch)
  } else if !is_aligned_to(s.as_ptr() as *const (), align_of::<T>()) {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else {
    Ok(unsafe { &*(s.as_ptr() as *const T) })
  }
}

/// Re-interprets `&mut [u8]` as `&mut T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
#[inline]
pub(crate) unsafe fn try_from_bytes_mut<T: Copy>(
  s: &mut [u8],
) -> Result<&mut T, PodCastError> {
  if s.len() != size_of::<T>() {
    Err(PodCastError::SizeMismatch)
  } else if !is_aligned_to(s.as_ptr() as *const (), align_of::<T>()) {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else {
    Ok(unsafe { &mut *(s.as_mut_ptr() as *mut T) })
  }
}

/// Cast `A` into `B`
///
/// ## Panics
///
/// * This is like [`try_cast`](try_cast), but will panic on a size mismatch.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn cast<A: Copy, B: Copy>(a: A) -> B {
  if size_of::<A>() == size_of::<B>() {
    unsafe { transmute!(a) }
  } else {
    something_went_wrong("cast", PodCastError::SizeMismatch)
  }
}

/// Cast `&mut A` into `&mut B`.
///
/// ## Panics
///
/// This is [`try_cast_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn cast_mut<A: Copy, B: Copy>(a: &mut A) -> &mut B {
  if size_of::<A>() == size_of::<B>() && align_of::<A>() >= align_of::<B>() {
    // Plz mr compiler, just notice that we can't ever hit Err in this case.
    match try_cast_mut(a) {
      Ok(b) => b,
      Err(_) => unreachable!(),
    }
  } else {
    match try_cast_mut(a) {
      Ok(b) => b,
      Err(e) => something_went_wrong("cast_mut", e),
    }
  }
}

/// Cast `&A` into `&B`.
///
/// ## Panics
///
/// This is [`try_cast_ref`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn cast_ref<A: Copy, B: Copy>(a: &A) -> &B {
  if size_of::<A>() == size_of::<B>() && align_of::<A>() >= align_of::<B>() {
    // Plz mr compiler, just notice that we can't ever hit Err in this case.
    match try_cast_ref(a) {
      Ok(b) => b,
      Err(_) => unreachable!(),
    }
  } else {
    match try_cast_ref(a) {
      Ok(b) => b,
      Err(e) => something_went_wrong("cast_ref", e),
    }
  }
}

/// Cast `&[A]` into `&[B]`.
///
/// ## Panics
///
/// This is [`try_cast_slice`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn cast_slice<A: Copy, B: Copy>(a: &[A]) -> &[B] {
  match try_cast_slice(a) {
    Ok(b) => b,
    Err(e) => something_went_wrong("cast_slice", e),
  }
}

/// Cast `&mut [A]` into `&mut [B]`.
///
/// ## Panics
///
/// This is [`try_cast_slice_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub(crate) unsafe fn cast_slice_mut<A: Copy, B: Copy>(a: &mut [A]) -> &mut [B] {
  match try_cast_slice_mut(a) {
    Ok(b) => b,
    Err(e) => something_went_wrong("cast_slice_mut", e),
  }
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
pub(crate) unsafe fn try_cast<A: Copy, B: Copy>(
  a: A,
) -> Result<B, PodCastError> {
  if size_of::<A>() == size_of::<B>() {
    Ok(unsafe { transmute!(a) })
  } else {
    Err(PodCastError::SizeMismatch)
  }
}

/// Try to convert a `&A` into `&B`.
///
/// ## Failure
///
/// * If the reference isn't aligned in the new type
/// * If the source type and target type aren't the same size.
#[inline]
pub(crate) unsafe fn try_cast_ref<A: Copy, B: Copy>(
  a: &A,
) -> Result<&B, PodCastError> {
  // Note(Lokathor): everything with `align_of` and `size_of` will optimize away
  // after monomorphization.
  if align_of::<B>() > align_of::<A>()
    && !is_aligned_to(a as *const A as *const (), align_of::<B>())
  {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else if size_of::<B>() == size_of::<A>() {
    Ok(unsafe { &*(a as *const A as *const B) })
  } else {
    Err(PodCastError::SizeMismatch)
  }
}

/// Try to convert a `&mut A` into `&mut B`.
///
/// As [`try_cast_ref`], but `mut`.
#[inline]
pub(crate) unsafe fn try_cast_mut<A: Copy, B: Copy>(
  a: &mut A,
) -> Result<&mut B, PodCastError> {
  // Note(Lokathor): everything with `align_of` and `size_of` will optimize away
  // after monomorphization.
  if align_of::<B>() > align_of::<A>()
    && !is_aligned_to(a as *const A as *const (), align_of::<B>())
  {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else if size_of::<B>() == size_of::<A>() {
    Ok(unsafe { &mut *(a as *mut A as *mut B) })
  } else {
    Err(PodCastError::SizeMismatch)
  }
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
#[inline]
pub(crate) unsafe fn try_cast_slice<A: Copy, B: Copy>(
  a: &[A],
) -> Result<&[B], PodCastError> {
  let input_bytes = core::mem::size_of_val::<[A]>(a);
  // Note(Lokathor): everything with `align_of` and `size_of` will optimize away
  // after monomorphization.
  if align_of::<B>() > align_of::<A>()
    && !is_aligned_to(a.as_ptr() as *const (), align_of::<B>())
  {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else if size_of::<B>() == size_of::<A>() {
    Ok(unsafe { core::slice::from_raw_parts(a.as_ptr() as *const B, a.len()) })
  } else if (size_of::<B>() != 0 && input_bytes % size_of::<B>() == 0)
    || (size_of::<B>() == 0 && input_bytes == 0)
  {
    let new_len =
      if size_of::<B>() != 0 { input_bytes / size_of::<B>() } else { 0 };
    Ok(unsafe { core::slice::from_raw_parts(a.as_ptr() as *const B, new_len) })
  } else {
    Err(PodCastError::OutputSliceWouldHaveSlop)
  }
}

/// Try to convert `&mut [A]` into `&mut [B]` (possibly with a change in
/// length).
///
/// As [`try_cast_slice`], but `&mut`.
#[inline]
pub(crate) unsafe fn try_cast_slice_mut<A: Copy, B: Copy>(
  a: &mut [A],
) -> Result<&mut [B], PodCastError> {
  let input_bytes = core::mem::size_of_val::<[A]>(a);
  // Note(Lokathor): everything with `align_of` and `size_of` will optimize away
  // after monomorphization.
  if align_of::<B>() > align_of::<A>()
    && !is_aligned_to(a.as_ptr() as *const (), align_of::<B>())
  {
    Err(PodCastError::TargetAlignmentGreaterAndInputNotAligned)
  } else if size_of::<B>() == size_of::<A>() {
    Ok(unsafe {
      core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut B, a.len())
    })
  } else if (size_of::<B>() != 0 && input_bytes % size_of::<B>() == 0)
    || (size_of::<B>() == 0 && input_bytes == 0)
  {
    let new_len =
      if size_of::<B>() != 0 { input_bytes / size_of::<B>() } else { 0 };
    Ok(unsafe {
      core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut B, new_len)
    })
  } else {
    Err(PodCastError::OutputSliceWouldHaveSlop)
  }
}
