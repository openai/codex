#![cfg(feature = "extern_crate_alloc")]
#![allow(clippy::duplicated_attributes)]

//! Stuff to boost things in the `alloc` crate.
//!
//! * You must enable the `extern_crate_alloc` feature of `bytemuck` or you will
//!   not be able to use this module! This is generally done by adding the
//!   feature to the dependency in Cargo.toml like so:
//!
//!   `bytemuck = { version = "VERSION_YOU_ARE_USING", features =
//! ["extern_crate_alloc"]}`

use super::*;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;
use alloc::{
  alloc::{alloc_zeroed, Layout},
  boxed::Box,
  rc::Rc,
  vec,
  vec::Vec,
};
use core::{
  mem::{size_of_val, ManuallyDrop},
  ops::{Deref, DerefMut},
};

/// As [`try_cast_box`], but unwraps for you.
#[inline]
pub fn cast_box<A: NoUninit, B: AnyBitPattern>(input: Box<A>) -> Box<B> {
  try_cast_box(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a [`Box`].
///
/// On failure you get back an error along with the starting `Box`.
///
/// ## Failure
///
/// * The start and end content type of the `Box` must have the exact same
///   alignment.
/// * The start and end size of the `Box` must have the exact same size.
#[inline]
pub fn try_cast_box<A: NoUninit, B: AnyBitPattern>(
  input: Box<A>,
) -> Result<Box<B>, (PodCastError, Box<A>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    Err((PodCastError::SizeMismatch, input))
  } else {
    // Note(Lokathor): This is much simpler than with the Vec casting!
    let ptr: *mut B = Box::into_raw(input) as *mut B;
    Ok(unsafe { Box::from_raw(ptr) })
  }
}

/// Allocates a `Box<T>` with all of the contents being zeroed out.
///
/// This uses the global allocator to create a zeroed allocation and _then_
/// turns it into a Box. In other words, it's 100% assured that the zeroed data
/// won't be put temporarily on the stack. You can make a box of any size
/// without fear of a stack overflow.
///
/// ## Failure
///
/// This fails if the allocation fails.
#[inline]
pub fn try_zeroed_box<T: Zeroable>() -> Result<Box<T>, ()> {
  if size_of::<T>() == 0 {
    // This will not allocate but simply create an arbitrary non-null
    // aligned pointer, valid for Box for a zero-sized pointee.
    let ptr = core::ptr::NonNull::dangling().as_ptr();
    return Ok(unsafe { Box::from_raw(ptr) });
  }
  let layout = Layout::new::<T>();
  let ptr = unsafe { alloc_zeroed(layout) };
  if ptr.is_null() {
    // we don't know what the error is because `alloc_zeroed` is a dumb API
    Err(())
  } else {
    Ok(unsafe { Box::<T>::from_raw(ptr as *mut T) })
  }
}

/// As [`try_zeroed_box`], but unwraps for you.
#[inline]
pub fn zeroed_box<T: Zeroable>() -> Box<T> {
  try_zeroed_box().unwrap()
}

/// Allocates a `Vec<T>` of length and capacity exactly equal to `length` and
/// all elements zeroed.
///
/// ## Failure
///
/// This fails if the allocation fails, or if a layout cannot be calculated for
/// the allocation.
pub fn try_zeroed_vec<T: Zeroable>(length: usize) -> Result<Vec<T>, ()> {
  if length == 0 {
    Ok(Vec::new())
  } else {
    let boxed_slice = try_zeroed_slice_box(length)?;
    Ok(boxed_slice.into_vec())
  }
}

/// As [`try_zeroed_vec`] but unwraps for you
pub fn zeroed_vec<T: Zeroable>(length: usize) -> Vec<T> {
  try_zeroed_vec(length).unwrap()
}

/// Allocates a `Box<[T]>` with all contents being zeroed out.
///
/// This uses the global allocator to create a zeroed allocation and _then_
/// turns it into a Box. In other words, it's 100% assured that the zeroed data
/// won't be put temporarily on the stack. You can make a box of any size
/// without fear of a stack overflow.
///
/// ## Failure
///
/// This fails if the allocation fails, or if a layout cannot be calculated for
/// the allocation.
#[inline]
pub fn try_zeroed_slice_box<T: Zeroable>(
  length: usize,
) -> Result<Box<[T]>, ()> {
  if size_of::<T>() == 0 || length == 0 {
    // This will not allocate but simply create an arbitrary non-null aligned
    // slice pointer, valid for Box for a zero-sized pointee.
    let ptr = core::ptr::NonNull::dangling().as_ptr();
    let slice_ptr = core::ptr::slice_from_raw_parts_mut(ptr, length);
    return Ok(unsafe { Box::from_raw(slice_ptr) });
  }
  let layout = core::alloc::Layout::array::<T>(length).map_err(|_| ())?;
  let ptr = unsafe { alloc_zeroed(layout) };
  if ptr.is_null() {
    // we don't know what the error is because `alloc_zeroed` is a dumb API
    Err(())
  } else {
    let slice =
      unsafe { core::slice::from_raw_parts_mut(ptr as *mut T, length) };
    Ok(unsafe { Box::<[T]>::from_raw(slice) })
  }
}

/// As [`try_zeroed_slice_box`], but unwraps for you.
pub fn zeroed_slice_box<T: Zeroable>(length: usize) -> Box<[T]> {
  try_zeroed_slice_box(length).unwrap()
}

/// Allocates a `Arc<T>` with all contents being zeroed out.
#[cfg(all(feature = "alloc_uninit", target_has_atomic = "ptr"))]
pub fn zeroed_arc<T: Zeroable>() -> Arc<T> {
  let mut arc = Arc::new_uninit();
  crate::write_zeroes(Arc::get_mut(&mut arc).unwrap()); // unwrap never fails for a newly allocated Arc
  unsafe { arc.assume_init() }
}

/// Allocates a `Arc<[T]>` with all contents being zeroed out.
#[cfg(all(feature = "alloc_uninit", target_has_atomic = "ptr"))]
pub fn zeroed_arc_slice<T: Zeroable>(length: usize) -> Arc<[T]> {
  let mut arc = Arc::new_uninit_slice(length);
  crate::fill_zeroes(Arc::get_mut(&mut arc).unwrap()); // unwrap never fails for a newly allocated Arc
  unsafe { arc.assume_init() }
}

/// Allocates a `Rc<T>` with all contents being zeroed out.
#[cfg(feature = "alloc_uninit")]
pub fn zeroed_rc<T: Zeroable>() -> Rc<T> {
  let mut rc = Rc::new_uninit();
  crate::write_zeroes(Rc::get_mut(&mut rc).unwrap()); // unwrap never fails for a newly allocated Rc
  unsafe { rc.assume_init() }
}

/// Allocates a `Rc<[T]>` with all contents being zeroed out.
#[cfg(feature = "alloc_uninit")]
pub fn zeroed_rc_slice<T: Zeroable>(length: usize) -> Rc<[T]> {
  let mut rc = Rc::new_uninit_slice(length);
  crate::fill_zeroes(Rc::get_mut(&mut rc).unwrap()); // unwrap never fails for a newly allocated Rc
  unsafe { rc.assume_init() }
}

/// As [`try_cast_slice_box`], but unwraps for you.
#[inline]
pub fn cast_slice_box<A: NoUninit, B: AnyBitPattern>(
  input: Box<[A]>,
) -> Box<[B]> {
  try_cast_slice_box(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a `Box<[T]>`.
///
/// On failure you get back an error along with the starting `Box<[T]>`.
///
/// ## Failure
///
/// * The start and end content type of the `Box<[T]>` must have the exact same
///   alignment.
/// * The start and end content size in bytes of the `Box<[T]>` must be the
///   exact same.
#[inline]
pub fn try_cast_slice_box<A: NoUninit, B: AnyBitPattern>(
  input: Box<[A]>,
) -> Result<Box<[B]>, (PodCastError, Box<[A]>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    let input_bytes = size_of_val::<[A]>(&*input);
    if (size_of::<B>() == 0 && input_bytes != 0)
      || (size_of::<B>() != 0 && input_bytes % size_of::<B>() != 0)
    {
      // If the size in bytes of the underlying buffer does not match an exact
      // multiple of the size of B, we cannot cast between them.
      Err((PodCastError::OutputSliceWouldHaveSlop, input))
    } else {
      // Because the size is an exact multiple, we can now change the length
      // of the slice and recreate the Box
      // NOTE: This is a valid operation because according to the docs of
      // std::alloc::GlobalAlloc::dealloc(), the Layout that was used to alloc
      // the block must be the same Layout that is used to dealloc the block.
      // Luckily, Layout only stores two things, the alignment, and the size in
      // bytes. So as long as both of those stay the same, the Layout will
      // remain a valid input to dealloc.
      let length =
        if size_of::<B>() != 0 { input_bytes / size_of::<B>() } else { 0 };
      let box_ptr: *mut A = Box::into_raw(input) as *mut A;
      let ptr: *mut [B] =
        unsafe { core::slice::from_raw_parts_mut(box_ptr as *mut B, length) };
      Ok(unsafe { Box::<[B]>::from_raw(ptr) })
    }
  } else {
    let box_ptr: *mut [A] = Box::into_raw(input);
    let ptr: *mut [B] = box_ptr as *mut [B];
    Ok(unsafe { Box::<[B]>::from_raw(ptr) })
  }
}

/// As [`try_cast_vec`], but unwraps for you.
#[inline]
pub fn cast_vec<A: NoUninit, B: AnyBitPattern>(input: Vec<A>) -> Vec<B> {
  try_cast_vec(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a [`Vec`].
///
/// On failure you get back an error along with the starting `Vec`.
///
/// ## Failure
///
/// * The start and end content type of the `Vec` must have the exact same
///   alignment.
/// * The start and end content size in bytes of the `Vec` must be the exact
///   same.
/// * The start and end capacity in bytes of the `Vec` must be the exact same.
#[inline]
pub fn try_cast_vec<A: NoUninit, B: AnyBitPattern>(
  input: Vec<A>,
) -> Result<Vec<B>, (PodCastError, Vec<A>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    let input_size = size_of_val::<[A]>(&*input);
    let input_capacity = input.capacity() * size_of::<A>();
    if (size_of::<B>() == 0 && input_capacity != 0)
      || (size_of::<B>() != 0
        && (input_size % size_of::<B>() != 0
          || input_capacity % size_of::<B>() != 0))
    {
      // If the size in bytes of the underlying buffer does not match an exact
      // multiple of the size of B, we cannot cast between them.
      // Note that we have to pay special attention to make sure that both
      // length and capacity are valid under B, as we do not want to
      // change which bytes are considered part of the initialized slice
      // of the Vec
      Err((PodCastError::OutputSliceWouldHaveSlop, input))
    } else {
      // Because the size is an exact multiple, we can now change the length and
      // capacity and recreate the Vec
      // NOTE: This is a valid operation because according to the docs of
      // std::alloc::GlobalAlloc::dealloc(), the Layout that was used to alloc
      // the block must be the same Layout that is used to dealloc the block.
      // Luckily, Layout only stores two things, the alignment, and the size in
      // bytes. So as long as both of those stay the same, the Layout will
      // remain a valid input to dealloc.

      // Note(Lokathor): First we record the length and capacity, which don't
      // have any secret provenance metadata.
      let length: usize =
        if size_of::<B>() != 0 { input_size / size_of::<B>() } else { 0 };
      let capacity: usize =
        if size_of::<B>() != 0 { input_capacity / size_of::<B>() } else { 0 };
      // Note(Lokathor): Next we "pre-forget" the old Vec by wrapping with
      // ManuallyDrop, because if we used `core::mem::forget` after taking the
      // pointer then that would invalidate our pointer. In nightly there's a
      // "into raw parts" method, which we can switch this too eventually.
      let mut manual_drop_vec = ManuallyDrop::new(input);
      let vec_ptr: *mut A = manual_drop_vec.as_mut_ptr();
      let ptr: *mut B = vec_ptr as *mut B;
      Ok(unsafe { Vec::from_raw_parts(ptr, length, capacity) })
    }
  } else {
    // Note(Lokathor): First we record the length and capacity, which don't have
    // any secret provenance metadata.
    let length: usize = input.len();
    let capacity: usize = input.capacity();
    // Note(Lokathor): Next we "pre-forget" the old Vec by wrapping with
    // ManuallyDrop, because if we used `core::mem::forget` after taking the
    // pointer then that would invalidate our pointer. In nightly there's a
    // "into raw parts" method, which we can switch this too eventually.
    let mut manual_drop_vec = ManuallyDrop::new(input);
    let vec_ptr: *mut A = manual_drop_vec.as_mut_ptr();
    let ptr: *mut B = vec_ptr as *mut B;
    Ok(unsafe { Vec::from_raw_parts(ptr, length, capacity) })
  }
}

/// This "collects" a slice of pod data into a vec of a different pod type.
///
/// Unlike with [`cast_slice`] and [`cast_slice_mut`], this will always work.
///
/// The output vec will be of a minimal size/capacity to hold the slice given.
///
/// ```rust
/// # use bytemuck::*;
/// let halfwords: [u16; 4] = [5, 6, 7, 8];
/// let vec_of_words: Vec<u32> = pod_collect_to_vec(&halfwords);
/// if cfg!(target_endian = "little") {
///   assert_eq!(&vec_of_words[..], &[0x0006_0005, 0x0008_0007][..])
/// } else {
///   assert_eq!(&vec_of_words[..], &[0x0005_0006, 0x0007_0008][..])
/// }
/// ```
pub fn pod_collect_to_vec<A: NoUninit, B: NoUninit + AnyBitPattern>(
  src: &[A],
) -> Vec<B> {
  let src_size = core::mem::size_of_val(src);
  // Note(Lokathor): dst_count is rounded up so that the dest will always be at
  // least as many bytes as the src.
  let dst_count = src_size / size_of::<B>()
    + if src_size % size_of::<B>() != 0 { 1 } else { 0 };
  let mut dst = vec![B::zeroed(); dst_count];

  let src_bytes: &[u8] = cast_slice(src);
  let dst_bytes: &mut [u8] = cast_slice_mut(&mut dst[..]);
  dst_bytes[..src_size].copy_from_slice(src_bytes);
  dst
}

/// As [`try_cast_rc`], but unwraps for you.
#[inline]
pub fn cast_rc<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(
  input: Rc<A>,
) -> Rc<B> {
  try_cast_rc(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a [`Rc`].
///
/// On failure you get back an error along with the starting `Rc`.
///
/// The bounds on this function are the same as [`cast_mut`], because a user
/// could call `Rc::get_unchecked_mut` on the output, which could be observable
/// in the input.
///
/// ## Failure
///
/// * The start and end content type of the `Rc` must have the exact same
///   alignment.
/// * The start and end size of the `Rc` must have the exact same size.
#[inline]
pub fn try_cast_rc<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(
  input: Rc<A>,
) -> Result<Rc<B>, (PodCastError, Rc<A>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    Err((PodCastError::SizeMismatch, input))
  } else {
    // Safety: Rc::from_raw requires size and alignment match, which is met.
    let ptr: *const B = Rc::into_raw(input) as *const B;
    Ok(unsafe { Rc::from_raw(ptr) })
  }
}

/// As [`try_cast_arc`], but unwraps for you.
#[inline]
#[cfg(target_has_atomic = "ptr")]
pub fn cast_arc<A: NoUninit + AnyBitPattern, B: NoUninit + AnyBitPattern>(
  input: Arc<A>,
) -> Arc<B> {
  try_cast_arc(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a [`Arc`].
///
/// On failure you get back an error along with the starting `Arc`.
///
/// The bounds on this function are the same as [`cast_mut`], because a user
/// could call `Rc::get_unchecked_mut` on the output, which could be observable
/// in the input.
///
/// ## Failure
///
/// * The start and end content type of the `Arc` must have the exact same
///   alignment.
/// * The start and end size of the `Arc` must have the exact same size.
#[inline]
#[cfg(target_has_atomic = "ptr")]
pub fn try_cast_arc<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  input: Arc<A>,
) -> Result<Arc<B>, (PodCastError, Arc<A>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    Err((PodCastError::SizeMismatch, input))
  } else {
    // Safety: Arc::from_raw requires size and alignment match, which is met.
    let ptr: *const B = Arc::into_raw(input) as *const B;
    Ok(unsafe { Arc::from_raw(ptr) })
  }
}

/// As [`try_cast_slice_rc`], but unwraps for you.
#[inline]
pub fn cast_slice_rc<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  input: Rc<[A]>,
) -> Rc<[B]> {
  try_cast_slice_rc(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a `Rc<[T]>`.
///
/// On failure you get back an error along with the starting `Rc<[T]>`.
///
/// The bounds on this function are the same as [`cast_mut`], because a user
/// could call `Rc::get_unchecked_mut` on the output, which could be observable
/// in the input.
///
/// ## Failure
///
/// * The start and end content type of the `Rc<[T]>` must have the exact same
///   alignment.
/// * The start and end content size in bytes of the `Rc<[T]>` must be the exact
///   same.
#[inline]
pub fn try_cast_slice_rc<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  input: Rc<[A]>,
) -> Result<Rc<[B]>, (PodCastError, Rc<[A]>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    let input_bytes = size_of_val::<[A]>(&*input);
    if (size_of::<B>() == 0 && input_bytes != 0)
      || (size_of::<B>() != 0 && input_bytes % size_of::<B>() != 0)
    {
      // If the size in bytes of the underlying buffer does not match an exact
      // multiple of the size of B, we cannot cast between them.
      Err((PodCastError::OutputSliceWouldHaveSlop, input))
    } else {
      // Because the size is an exact multiple, we can now change the length
      // of the slice and recreate the Rc
      // NOTE: This is a valid operation because according to the docs of
      // std::rc::Rc::from_raw(), the type U that was in the original Rc<U>
      // acquired from Rc::into_raw() must have the same size alignment and
      // size of the type T in the new Rc<T>. So as long as both the size
      // and alignment stay the same, the Rc will remain a valid Rc.
      let length =
        if size_of::<B>() != 0 { input_bytes / size_of::<B>() } else { 0 };
      let rc_ptr: *const A = Rc::into_raw(input) as *const A;
      // Must use ptr::slice_from_raw_parts, because we cannot make an
      // intermediate const reference, because it has mutable provenance,
      // nor an intermediate mutable reference, because it could be aliased.
      let ptr = core::ptr::slice_from_raw_parts(rc_ptr as *const B, length);
      Ok(unsafe { Rc::<[B]>::from_raw(ptr) })
    }
  } else {
    let rc_ptr: *const [A] = Rc::into_raw(input);
    let ptr: *const [B] = rc_ptr as *const [B];
    Ok(unsafe { Rc::<[B]>::from_raw(ptr) })
  }
}

/// As [`try_cast_slice_arc`], but unwraps for you.
#[inline]
#[cfg(target_has_atomic = "ptr")]
pub fn cast_slice_arc<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  input: Arc<[A]>,
) -> Arc<[B]> {
  try_cast_slice_arc(input).map_err(|(e, _v)| e).unwrap()
}

/// Attempts to cast the content type of a `Arc<[T]>`.
///
/// On failure you get back an error along with the starting `Arc<[T]>`.
///
/// The bounds on this function are the same as [`cast_mut`], because a user
/// could call `Rc::get_unchecked_mut` on the output, which could be observable
/// in the input.
///
/// ## Failure
///
/// * The start and end content type of the `Arc<[T]>` must have the exact same
///   alignment.
/// * The start and end content size in bytes of the `Arc<[T]>` must be the
///   exact same.
#[inline]
#[cfg(target_has_atomic = "ptr")]
pub fn try_cast_slice_arc<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + AnyBitPattern,
>(
  input: Arc<[A]>,
) -> Result<Arc<[B]>, (PodCastError, Arc<[A]>)> {
  if align_of::<A>() != align_of::<B>() {
    Err((PodCastError::AlignmentMismatch, input))
  } else if size_of::<A>() != size_of::<B>() {
    let input_bytes = size_of_val::<[A]>(&*input);
    if (size_of::<B>() == 0 && input_bytes != 0)
      || (size_of::<B>() != 0 && input_bytes % size_of::<B>() != 0)
    {
      // If the size in bytes of the underlying buffer does not match an exact
      // multiple of the size of B, we cannot cast between them.
      Err((PodCastError::OutputSliceWouldHaveSlop, input))
    } else {
      // Because the size is an exact multiple, we can now change the length
      // of the slice and recreate the Arc
      // NOTE: This is a valid operation because according to the docs of
      // std::sync::Arc::from_raw(), the type U that was in the original Arc<U>
      // acquired from Arc::into_raw() must have the same size alignment and
      // size of the type T in the new Arc<T>. So as long as both the size
      // and alignment stay the same, the Arc will remain a valid Arc.
      let length =
        if size_of::<B>() != 0 { input_bytes / size_of::<B>() } else { 0 };
      let arc_ptr: *const A = Arc::into_raw(input) as *const A;
      // Must use ptr::slice_from_raw_parts, because we cannot make an
      // intermediate const reference, because it has mutable provenance,
      // nor an intermediate mutable reference, because it could be aliased.
      let ptr = core::ptr::slice_from_raw_parts(arc_ptr as *const B, length);
      Ok(unsafe { Arc::<[B]>::from_raw(ptr) })
    }
  } else {
    let arc_ptr: *const [A] = Arc::into_raw(input);
    let ptr: *const [B] = arc_ptr as *const [B];
    Ok(unsafe { Arc::<[B]>::from_raw(ptr) })
  }
}

/// An extension trait for `TransparentWrapper` and alloc types.
pub trait TransparentWrapperAlloc<Inner: ?Sized>:
  TransparentWrapper<Inner>
{
  /// Convert a vec of the inner type into a vec of the wrapper type.
  fn wrap_vec(s: Vec<Inner>) -> Vec<Self>
  where
    Self: Sized,
    Inner: Sized,
  {
    let mut s = ManuallyDrop::new(s);

    let length = s.len();
    let capacity = s.capacity();
    let ptr = s.as_mut_ptr();

    unsafe {
      // SAFETY:
      // * ptr comes from Vec (and will not be double-dropped)
      // * the two types have the identical representation
      // * the len and capacity fields are valid
      Vec::from_raw_parts(ptr as *mut Self, length, capacity)
    }
  }

  /// Convert a box to the inner type into a box to the wrapper
  /// type.
  #[inline]
  fn wrap_box(s: Box<Inner>) -> Box<Self> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations
      // * Box is guaranteed to have representation identical to a (non-null)
      //   pointer
      // * The pointer comes from a box (and thus satisfies all safety
      //   requirements of Box)
      let inner_ptr: *mut Inner = Box::into_raw(s);
      let wrapper_ptr: *mut Self = transmute!(inner_ptr);
      Box::from_raw(wrapper_ptr)
    }
  }

  /// Convert an [`Rc`] to the inner type into an `Rc` to the wrapper type.
  #[inline]
  fn wrap_rc(s: Rc<Inner>) -> Rc<Self> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the layout of Rc is unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations, and that the size and alignment of Inner
      //   and Self are the same, which meets the safety requirements of
      //   Rc::from_raw
      let inner_ptr: *const Inner = Rc::into_raw(s);
      let wrapper_ptr: *const Self = transmute!(inner_ptr);
      Rc::from_raw(wrapper_ptr)
    }
  }

  /// Convert an [`Arc`] to the inner type into an `Arc` to the wrapper type.
  #[inline]
  #[cfg(target_has_atomic = "ptr")]
  fn wrap_arc(s: Arc<Inner>) -> Arc<Self> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the layout of Arc is unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations, and that the size and alignment of Inner
      //   and Self are the same, which meets the safety requirements of
      //   Arc::from_raw
      let inner_ptr: *const Inner = Arc::into_raw(s);
      let wrapper_ptr: *const Self = transmute!(inner_ptr);
      Arc::from_raw(wrapper_ptr)
    }
  }

  /// Convert a vec of the wrapper type into a vec of the inner type.
  fn peel_vec(s: Vec<Self>) -> Vec<Inner>
  where
    Self: Sized,
    Inner: Sized,
  {
    let mut s = ManuallyDrop::new(s);

    let length = s.len();
    let capacity = s.capacity();
    let ptr = s.as_mut_ptr();

    unsafe {
      // SAFETY:
      // * ptr comes from Vec (and will not be double-dropped)
      // * the two types have the identical representation
      // * the len and capacity fields are valid
      Vec::from_raw_parts(ptr as *mut Inner, length, capacity)
    }
  }

  /// Convert a box to the wrapper type into a box to the inner
  /// type.
  #[inline]
  fn peel_box(s: Box<Self>) -> Box<Inner> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations
      // * Box is guaranteed to have representation identical to a (non-null)
      //   pointer
      // * The pointer comes from a box (and thus satisfies all safety
      //   requirements of Box)
      let wrapper_ptr: *mut Self = Box::into_raw(s);
      let inner_ptr: *mut Inner = transmute!(wrapper_ptr);
      Box::from_raw(inner_ptr)
    }
  }

  /// Convert an [`Rc`] to the wrapper type into an `Rc` to the inner type.
  #[inline]
  fn peel_rc(s: Rc<Self>) -> Rc<Inner> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the layout of Rc is unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations, and that the size and alignment of Inner
      //   and Self are the same, which meets the safety requirements of
      //   Rc::from_raw
      let wrapper_ptr: *const Self = Rc::into_raw(s);
      let inner_ptr: *const Inner = transmute!(wrapper_ptr);
      Rc::from_raw(inner_ptr)
    }
  }

  /// Convert an [`Arc`] to the wrapper type into an `Arc` to the inner type.
  #[inline]
  #[cfg(target_has_atomic = "ptr")]
  fn peel_arc(s: Arc<Self>) -> Arc<Inner> {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());

    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the layout of Arc is unspecified.
      //
      // SAFETY:
      // * The unsafe contract requires that pointers to Inner and Self have
      //   identical representations, and that the size and alignment of Inner
      //   and Self are the same, which meets the safety requirements of
      //   Arc::from_raw
      let wrapper_ptr: *const Self = Arc::into_raw(s);
      let inner_ptr: *const Inner = transmute!(wrapper_ptr);
      Arc::from_raw(inner_ptr)
    }
  }
}

impl<I: ?Sized, T: ?Sized + TransparentWrapper<I>> TransparentWrapperAlloc<I>
  for T
{
}

/// As `Box<[u8]>`, but remembers the original alignment.
pub struct BoxBytes {
  // SAFETY: `ptr` is aligned to `layout.align()`, points to
  // `layout.size()` initialized bytes, and, if `layout.size() > 0`,
  // is owned and was allocated with the global allocator with `layout`.
  ptr: NonNull<u8>,
  layout: Layout,
}

// SAFETY: `BoxBytes` is semantically a `Box<[u8], Global>` with a different allocation alignment,
// `Box<[u8], Global>` is `Send + Sync`, and changing the allocation alignment has no thread-safety implications.
unsafe impl Send for BoxBytes {}
// SAFETY: See `Send` impl
unsafe impl Sync for BoxBytes {}

impl Deref for BoxBytes {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    // SAFETY: See type invariant.
    unsafe {
      core::slice::from_raw_parts(self.ptr.as_ptr(), self.layout.size())
    }
  }
}

impl DerefMut for BoxBytes {
  fn deref_mut(&mut self) -> &mut Self::Target {
    // SAFETY: See type invariant.
    unsafe {
      core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.layout.size())
    }
  }
}

impl Drop for BoxBytes {
  fn drop(&mut self) {
    if self.layout.size() != 0 {
      // SAFETY: See type invariant: if `self.layout.size() != 0`, then
      // `self.ptr` is owned and was allocated with `self.layout`.
      unsafe { alloc::alloc::dealloc(self.ptr.as_ptr(), self.layout) };
    }
  }
}

impl<T: ?Sized + sealed::BoxBytesOf> From<Box<T>> for BoxBytes {
  fn from(value: Box<T>) -> Self {
    value.box_bytes_of()
  }
}

mod sealed {
  use crate::{BoxBytes, PodCastError};
  use alloc::boxed::Box;

  pub trait BoxBytesOf {
    fn box_bytes_of(self: Box<Self>) -> BoxBytes;
  }

  pub trait FromBoxBytes {
    fn try_from_box_bytes(
      bytes: BoxBytes,
    ) -> Result<Box<Self>, (PodCastError, BoxBytes)>;
  }
}

impl<T: NoUninit> sealed::BoxBytesOf for T {
  fn box_bytes_of(self: Box<Self>) -> BoxBytes {
    let layout = Layout::new::<T>();
    let ptr = Box::into_raw(self) as *mut u8;
    // SAFETY: Box::into_raw() returns a non-null pointer.
    let ptr = unsafe { NonNull::new_unchecked(ptr) };
    BoxBytes { ptr, layout }
  }
}

impl<T: NoUninit> sealed::BoxBytesOf for [T] {
  fn box_bytes_of(self: Box<Self>) -> BoxBytes {
    let layout = Layout::for_value::<[T]>(&self);
    let ptr = Box::into_raw(self) as *mut u8;
    // SAFETY: Box::into_raw() returns a non-null pointer.
    let ptr = unsafe { NonNull::new_unchecked(ptr) };
    BoxBytes { ptr, layout }
  }
}

impl sealed::BoxBytesOf for str {
  fn box_bytes_of(self: Box<Self>) -> BoxBytes {
    self.into_boxed_bytes().box_bytes_of()
  }
}

impl<T: AnyBitPattern> sealed::FromBoxBytes for T {
  fn try_from_box_bytes(
    bytes: BoxBytes,
  ) -> Result<Box<Self>, (PodCastError, BoxBytes)> {
    let layout = Layout::new::<T>();
    if bytes.layout.align() != layout.align() {
      Err((PodCastError::AlignmentMismatch, bytes))
    } else if bytes.layout.size() != layout.size() {
      Err((PodCastError::SizeMismatch, bytes))
    } else {
      let (ptr, _) = bytes.into_raw_parts();
      // SAFETY: See BoxBytes type invariant.
      Ok(unsafe { Box::from_raw(ptr.as_ptr() as *mut T) })
    }
  }
}

impl<T: AnyBitPattern> sealed::FromBoxBytes for [T] {
  fn try_from_box_bytes(
    bytes: BoxBytes,
  ) -> Result<Box<Self>, (PodCastError, BoxBytes)> {
    let single_layout = Layout::new::<T>();
    if bytes.layout.align() != single_layout.align() {
      Err((PodCastError::AlignmentMismatch, bytes))
    } else if (single_layout.size() == 0 && bytes.layout.size() != 0)
      || (single_layout.size() != 0
        && bytes.layout.size() % single_layout.size() != 0)
    {
      Err((PodCastError::OutputSliceWouldHaveSlop, bytes))
    } else {
      let (ptr, layout) = bytes.into_raw_parts();
      let length = if single_layout.size() != 0 {
        layout.size() / single_layout.size()
      } else {
        0
      };
      let ptr =
        core::ptr::slice_from_raw_parts_mut(ptr.as_ptr() as *mut T, length);
      // SAFETY: See BoxBytes type invariant.
      Ok(unsafe { Box::from_raw(ptr) })
    }
  }
}

/// Re-interprets `Box<T>` as `BoxBytes`.
///
/// `T` must be either [`Sized`] and [`NoUninit`],
/// [`[U]`](slice) where `U: NoUninit`, or [`str`].
#[inline]
pub fn box_bytes_of<T: sealed::BoxBytesOf + ?Sized>(input: Box<T>) -> BoxBytes {
  input.box_bytes_of()
}

/// Re-interprets `BoxBytes` as `Box<T>`.
///
/// `T` must be either [`Sized`] + [`AnyBitPattern`], or
/// [`[U]`](slice) where `U: AnyBitPattern`.
///
/// ## Panics
///
/// This is [`try_from_box_bytes`] but will panic on error and the input will be
/// dropped.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn from_box_bytes<T: sealed::FromBoxBytes + ?Sized>(
  input: BoxBytes,
) -> Box<T> {
  try_from_box_bytes(input).map_err(|(error, _)| error).unwrap()
}

/// Re-interprets `BoxBytes` as `Box<T>`.
///
/// `T` must be either [`Sized`] + [`AnyBitPattern`], or
/// [`[U]`](slice) where `U: AnyBitPattern`.
///
/// Returns `Err`:
/// * If the input isn't aligned for `T`.
/// * If `T: Sized` and the input's length isn't exactly the size of `T`.
/// * If `T = [U]` and the input's length isn't exactly a multiple of the size
///   of `U`.
#[inline]
pub fn try_from_box_bytes<T: sealed::FromBoxBytes + ?Sized>(
  input: BoxBytes,
) -> Result<Box<T>, (PodCastError, BoxBytes)> {
  T::try_from_box_bytes(input)
}

impl BoxBytes {
  /// Constructs a `BoxBytes` from its raw parts.
  ///
  /// # Safety
  ///
  /// The pointer is owned, has been allocated with the provided layout, and
  /// points to `layout.size()` initialized bytes.
  pub unsafe fn from_raw_parts(ptr: NonNull<u8>, layout: Layout) -> Self {
    BoxBytes { ptr, layout }
  }

  /// Deconstructs a `BoxBytes` into its raw parts.
  ///
  /// The pointer is owned, has been allocated with the provided layout, and
  /// points to `layout.size()` initialized bytes.
  pub fn into_raw_parts(self) -> (NonNull<u8>, Layout) {
    let me = ManuallyDrop::new(self);
    (me.ptr, me.layout)
  }

  /// Returns the original layout.
  pub fn layout(&self) -> Layout {
    self.layout
  }
}
