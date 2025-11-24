#![allow(clippy::module_name_repetitions)]
#![allow(clippy::let_unit_value)]
#![allow(clippy::let_underscore_untyped)]
#![allow(clippy::ptr_as_ptr)]

use crate::{AnyBitPattern, NoUninit};
use core::mem::{align_of, size_of};

struct Cast<A, B>((A, B));
impl<A, B> Cast<A, B> {
  const ASSERT_ALIGN_GREATER_THAN_EQUAL: () =
    assert!(align_of::<A>() >= align_of::<B>());
  const ASSERT_SIZE_EQUAL: () = assert!(size_of::<A>() == size_of::<B>());
  const ASSERT_SIZE_MULTIPLE_OF_OR_INPUT_ZST: () = assert!(
    (size_of::<A>() == 0)
      || (size_of::<B>() != 0 && size_of::<A>() % size_of::<B>() == 0)
  );
}

/// Cast `A` into `B` if infalliable, or fail to compile.
///
/// Note that for this particular type of cast, alignment isn't a factor. The
/// input value is semantically copied into the function and then returned to a
/// new memory location which will have whatever the required alignment of the
/// output type is.
///
/// ## Failure
///
/// * If the types don't have the same size this fails to compile.
///
/// ## Examples
/// ```
/// // compiles:
/// let bytes: [u8; 2] = bytemuck::must_cast(12_u16);
/// ```
/// ```compile_fail,E0080
/// // fails to compile (size mismatch):
/// let bytes : [u8; 3] = bytemuck::must_cast(12_u16);
/// ```
#[inline]
pub const fn must_cast<A: NoUninit, B: AnyBitPattern>(a: A) -> B {
  let _ = Cast::<A, B>::ASSERT_SIZE_EQUAL;
  unsafe { transmute!(A; B; a) }
}

/// Convert `&A` into `&B` if infalliable, or fail to compile.
///
/// ## Failure
///
/// * If the target type has a greater alignment requirement.
/// * If the source type and target type aren't the same size.
///
/// ## Examples
/// ```
/// // compiles:
/// let bytes: &[u8; 2] = bytemuck::must_cast_ref(&12_u16);
/// ```
/// ```compile_fail,E0080
/// // fails to compile (size mismatch):
/// let bytes : &[u8; 3] = bytemuck::must_cast_ref(&12_u16);
/// ```
/// ```compile_fail,E0080
/// // fails to compile (alignment requirements increased):
/// let bytes : &u16 = bytemuck::must_cast_ref(&[1u8, 2u8]);
/// ```
#[inline]
pub const fn must_cast_ref<A: NoUninit, B: AnyBitPattern>(a: &A) -> &B {
  let _ = Cast::<A, B>::ASSERT_SIZE_EQUAL;
  let _ = Cast::<A, B>::ASSERT_ALIGN_GREATER_THAN_EQUAL;
  unsafe { &*(a as *const A as *const B) }
}

maybe_const_fn! {
  #[cfg(feature = "must_cast_extra")]
  /// Convert a `&mut A` into `&mut B` if infalliable, or fail to compile.
  ///
  /// As [`must_cast_ref`], but `mut`.
  ///
  /// ## Examples
  /// ```
  /// let mut i = 12_u16;
  /// // compiles:
  /// let bytes: &mut [u8; 2] = bytemuck::must_cast_mut(&mut i);
  /// ```
  /// ```compile_fail,E0080
  /// # let mut bytes: &mut [u8; 2] = &mut [1, 2];
  /// // fails to compile (alignment requirements increased):
  /// let i : &mut u16 = bytemuck::must_cast_mut(bytes);
  /// ```
  /// ```compile_fail,E0080
  /// # let mut i = 12_u16;
  /// // fails to compile (size mismatch):
  /// let bytes : &mut [u8; 3] = bytemuck::must_cast_mut(&mut i);
  /// ```
  #[inline]
  pub fn must_cast_mut<
    A: NoUninit + AnyBitPattern,
    B: NoUninit + AnyBitPattern,
  >(
    a: &mut A,
  ) -> &mut B {
    let _ = Cast::<A, B>::ASSERT_SIZE_EQUAL;
    let _ = Cast::<A, B>::ASSERT_ALIGN_GREATER_THAN_EQUAL;
    unsafe { &mut *(a as *mut A as *mut B) }
  }
}

/// Convert `&[A]` into `&[B]` (possibly with a change in length) if
/// infalliable, or fail to compile.
///
/// * `input.as_ptr() as usize == output.as_ptr() as usize`
/// * `input.len() * size_of::<A>() == output.len() * size_of::<B>()`
///
/// ## Failure
///
/// * If the target type has a greater alignment requirement.
/// * If the target element type doesn't evenly fit into the the current element
///   type (eg: 3 `u16` values is 1.5 `u32` values, so that's a failure).
/// * Similarly, you can't convert from a non-[ZST](https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts)
///   to a ZST (e.g. 3 `u8` values is not any number of `()` values).
///
/// ## Examples
/// ```
/// let indicies: &[u16] = &[1, 2, 3];
/// // compiles:
/// let bytes: &[u8] = bytemuck::must_cast_slice(indicies);
/// ```
/// ```
/// let zsts: &[()] = &[(), (), ()];
/// // compiles:
/// let bytes: &[u8] = bytemuck::must_cast_slice(zsts);
/// ```
/// ```compile_fail,E0080
/// # let bytes : &[u8] = &[1, 0, 2, 0, 3, 0];
/// // fails to compile (bytes.len() might not be a multiple of 2):
/// let byte_pairs : &[[u8; 2]] = bytemuck::must_cast_slice(bytes);
/// ```
/// ```compile_fail,E0080
/// # let byte_pairs : &[[u8; 2]] = &[[1, 0], [2, 0], [3, 0]];
/// // fails to compile (alignment requirements increased):
/// let indicies : &[u16] = bytemuck::must_cast_slice(byte_pairs);
/// ```
/// ```compile_fail,E0080
/// let bytes: &[u8] = &[];
/// // fails to compile: (bytes.len() might not be 0)
/// let zsts: &[()] = bytemuck::must_cast_slice(bytes);
/// ```
#[inline]
pub const fn must_cast_slice<A: NoUninit, B: AnyBitPattern>(a: &[A]) -> &[B] {
  let _ = Cast::<A, B>::ASSERT_SIZE_MULTIPLE_OF_OR_INPUT_ZST;
  let _ = Cast::<A, B>::ASSERT_ALIGN_GREATER_THAN_EQUAL;
  let new_len = if size_of::<A>() == size_of::<B>() {
    a.len()
  } else {
    a.len() * (size_of::<A>() / size_of::<B>())
  };
  unsafe { core::slice::from_raw_parts(a.as_ptr() as *const B, new_len) }
}

maybe_const_fn! {
  #[cfg(feature = "must_cast_extra")]
  /// Convert `&mut [A]` into `&mut [B]` (possibly with a change in length) if
  /// infalliable, or fail to compile.
  ///
  /// As [`must_cast_slice`], but `&mut`.
  ///
  /// ## Examples
  /// ```
  /// let mut indicies = [1, 2, 3];
  /// let indicies: &mut [u16] = &mut indicies;
  /// // compiles:
  /// let bytes: &mut [u8] = bytemuck::must_cast_slice_mut(indicies);
  /// ```
  /// ```
  /// let zsts: &mut [()] = &mut [(), (), ()];
  /// // compiles:
  /// let bytes: &mut [u8] = bytemuck::must_cast_slice_mut(zsts);
  /// ```
  /// ```compile_fail,E0080
  /// # let mut bytes = [1, 0, 2, 0, 3, 0];
  /// # let bytes : &mut [u8] = &mut bytes[..];
  /// // fails to compile (bytes.len() might not be a multiple of 2):
  /// let byte_pairs : &mut [[u8; 2]] = bytemuck::must_cast_slice_mut(bytes);
  /// ```
  /// ```compile_fail,E0080
  /// # let mut byte_pairs = [[1, 0], [2, 0], [3, 0]];
  /// # let byte_pairs : &mut [[u8; 2]] = &mut byte_pairs[..];
  /// // fails to compile (alignment requirements increased):
  /// let indicies : &mut [u16] = bytemuck::must_cast_slice_mut(byte_pairs);
  /// ```
  /// ```compile_fail,E0080
  /// let bytes: &mut [u8] = &mut [];
  /// // fails to compile: (bytes.len() might not be 0)
  /// let zsts: &mut [()] = bytemuck::must_cast_slice_mut(bytes);
  /// ```
  #[inline]
  pub fn must_cast_slice_mut<
    A: NoUninit + AnyBitPattern,
    B: NoUninit + AnyBitPattern,
  >(
    a: &mut [A],
  ) -> &mut [B] {
    let _ = Cast::<A, B>::ASSERT_SIZE_MULTIPLE_OF_OR_INPUT_ZST;
    let _ = Cast::<A, B>::ASSERT_ALIGN_GREATER_THAN_EQUAL;
    let new_len = if size_of::<A>() == size_of::<B>() {
      a.len()
    } else {
      a.len() * (size_of::<A>() / size_of::<B>())
    };
    unsafe { core::slice::from_raw_parts_mut(a.as_mut_ptr() as *mut B, new_len) }
  }
}
