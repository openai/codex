//! Checked versions of the casting functions exposed in crate root
//! that support [`CheckedBitPattern`] types.

use crate::{
  internal::{self, something_went_wrong},
  AnyBitPattern, NoUninit,
};

/// A marker trait that allows types that have some invalid bit patterns to be
/// used in places that otherwise require [`AnyBitPattern`] or [`Pod`] types by
/// performing a runtime check on a perticular set of bits. This is particularly
/// useful for types like fieldless ('C-style') enums, [`char`], bool, and
/// structs containing them.
///
/// To do this, we define a `Bits` type which is a type with equivalent layout
/// to `Self` other than the invalid bit patterns which disallow `Self` from
/// being [`AnyBitPattern`]. This `Bits` type must itself implement
/// [`AnyBitPattern`]. Then, we implement a function that checks whether a
/// certain instance of the `Bits` is also a valid bit pattern of `Self`. If
/// this check passes, then we can allow casting from the `Bits` to `Self` (and
/// therefore, any type which is able to be cast to `Bits` is also able to be
/// cast to `Self`).
///
/// [`AnyBitPattern`] is a subset of [`CheckedBitPattern`], meaning that any `T:
/// AnyBitPattern` is also [`CheckedBitPattern`]. This means you can also use
/// any [`AnyBitPattern`] type in the checked versions of casting functions in
/// this module. If it's possible, prefer implementing [`AnyBitPattern`] for
/// your type directly instead of [`CheckedBitPattern`] as it gives greater
/// flexibility.
///
/// # Derive
///
/// A `#[derive(CheckedBitPattern)]` macro is provided under the `derive`
/// feature flag which will automatically validate the requirements of this
/// trait and implement the trait for you for both enums and structs. This is
/// the recommended method for implementing the trait, however it's also
/// possible to do manually.
///
/// # Example
///
/// If manually implementing the trait, we can do something like so:
///
/// ```rust
/// use bytemuck::{CheckedBitPattern, NoUninit};
///
/// #[repr(u32)]
/// #[derive(Copy, Clone)]
/// enum MyEnum {
///     Variant0 = 0,
///     Variant1 = 1,
///     Variant2 = 2,
/// }
///
/// unsafe impl CheckedBitPattern for MyEnum {
///     type Bits = u32;
///
///     fn is_valid_bit_pattern(bits: &u32) -> bool {
///         match *bits {
///             0 | 1 | 2 => true,
///             _ => false,
///         }
///     }
/// }
///
/// // It is often useful to also implement `NoUninit` on our `CheckedBitPattern` types.
/// // This will allow us to do casting of mutable references (and mutable slices).
/// // It is not always possible to do so, but in this case we have no padding so it is.
/// unsafe impl NoUninit for MyEnum {}
/// ```
///
/// We can now use relevant casting functions. For example,
///
/// ```rust
/// # use bytemuck::{CheckedBitPattern, NoUninit};
/// # #[repr(u32)]
/// # #[derive(Copy, Clone, PartialEq, Eq, Debug)]
/// # enum MyEnum {
/// #     Variant0 = 0,
/// #     Variant1 = 1,
/// #     Variant2 = 2,
/// # }
/// # unsafe impl NoUninit for MyEnum {}
/// # unsafe impl CheckedBitPattern for MyEnum {
/// #     type Bits = u32;
/// #     fn is_valid_bit_pattern(bits: &u32) -> bool {
/// #         match *bits {
/// #             0 | 1 | 2 => true,
/// #             _ => false,
/// #         }
/// #     }
/// # }
/// use bytemuck::{bytes_of, bytes_of_mut};
/// use bytemuck::checked;
///
/// let bytes = bytes_of(&2u32);
/// let result = checked::try_from_bytes::<MyEnum>(bytes);
/// assert_eq!(result, Ok(&MyEnum::Variant2));
///
/// // Fails for invalid discriminant
/// let bytes = bytes_of(&100u32);
/// let result = checked::try_from_bytes::<MyEnum>(bytes);
/// assert!(result.is_err());
///
/// // Since we implemented NoUninit, we can also cast mutably from an original type
/// // that is `NoUninit + AnyBitPattern`:
/// let mut my_u32 = 2u32;
/// {
///   let as_enum_mut = checked::cast_mut::<_, MyEnum>(&mut my_u32);
///   assert_eq!(as_enum_mut, &mut MyEnum::Variant2);
///   *as_enum_mut = MyEnum::Variant0;
/// }
/// assert_eq!(my_u32, 0u32);
/// ```
///
/// # Safety
///
/// * `Self` *must* have the same layout as the specified `Bits` except for the
///   possible invalid bit patterns being checked during
///   [`is_valid_bit_pattern`].
/// * This almost certainly means your type must be `#[repr(C)]` or a similar
///   specified repr, but if you think you know better, you probably don't. If
///   you still think you know better, be careful and have fun. And don't mess
///   it up (I mean it).
/// * If [`is_valid_bit_pattern`] returns true, then the bit pattern contained
///   in `bits` must also be valid for an instance of `Self`.
/// * Probably more, don't mess it up (I mean it 2.0)
///
/// [`is_valid_bit_pattern`]: CheckedBitPattern::is_valid_bit_pattern
/// [`Pod`]: crate::Pod
pub unsafe trait CheckedBitPattern: Copy {
  /// `Self` *must* have the same layout as the specified `Bits` except for
  /// the possible invalid bit patterns being checked during
  /// [`is_valid_bit_pattern`].
  ///
  /// [`is_valid_bit_pattern`]: CheckedBitPattern::is_valid_bit_pattern
  type Bits: AnyBitPattern;

  /// If this function returns true, then it must be valid to reinterpret `bits`
  /// as `&Self`.
  fn is_valid_bit_pattern(bits: &Self::Bits) -> bool;
}

unsafe impl<T: AnyBitPattern> CheckedBitPattern for T {
  type Bits = T;

  #[inline(always)]
  fn is_valid_bit_pattern(_bits: &T) -> bool {
    true
  }
}

unsafe impl CheckedBitPattern for char {
  type Bits = u32;

  #[inline]
  fn is_valid_bit_pattern(bits: &Self::Bits) -> bool {
    core::char::from_u32(*bits).is_some()
  }
}

unsafe impl CheckedBitPattern for bool {
  type Bits = u8;

  #[inline]
  fn is_valid_bit_pattern(bits: &Self::Bits) -> bool {
    // DO NOT use the `matches!` macro, it isn't 1.34 compatible.
    match *bits {
      0 | 1 => true,
      _ => false,
    }
  }
}

// Rust 1.70.0 documents that NonZero[int] has the same layout as [int].
macro_rules! impl_checked_for_nonzero {
  ($($nonzero:ty: $primitive:ty),* $(,)?) => {
    $(
      unsafe impl CheckedBitPattern for $nonzero {
        type Bits = $primitive;

        #[inline]
        fn is_valid_bit_pattern(bits: &Self::Bits) -> bool {
          *bits != 0
        }
      }
    )*
  };
}
impl_checked_for_nonzero! {
  core::num::NonZeroU8: u8,
  core::num::NonZeroI8: i8,
  core::num::NonZeroU16: u16,
  core::num::NonZeroI16: i16,
  core::num::NonZeroU32: u32,
  core::num::NonZeroI32: i32,
  core::num::NonZeroU64: u64,
  core::num::NonZeroI64: i64,
  core::num::NonZeroI128: i128,
  core::num::NonZeroU128: u128,
  core::num::NonZeroUsize: usize,
  core::num::NonZeroIsize: isize,
}

/// The things that can go wrong when casting between [`CheckedBitPattern`] data
/// forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckedCastError {
  /// An error occurred during a true-[`Pod`] cast
  ///
  /// [`Pod`]: crate::Pod
  PodCastError(crate::PodCastError),
  /// When casting to a [`CheckedBitPattern`] type, it is possible that the
  /// original data contains an invalid bit pattern. If so, the cast will
  /// fail and this error will be returned. Will never happen on casts
  /// between [`Pod`] types.
  ///
  /// [`Pod`]: crate::Pod
  InvalidBitPattern,
}

#[cfg(not(target_arch = "spirv"))]
impl core::fmt::Display for CheckedCastError {
  fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
    write!(f, "{:?}", self)
  }
}
#[cfg(feature = "extern_crate_std")]
#[cfg_attr(feature = "nightly_docs", doc(cfg(feature = "extern_crate_std")))]
impl std::error::Error for CheckedCastError {}

// Rust 1.81+
#[cfg(all(feature = "impl_core_error", not(feature = "extern_crate_std")))]
impl core::error::Error for CheckedCastError {}

impl From<crate::PodCastError> for CheckedCastError {
  fn from(err: crate::PodCastError) -> CheckedCastError {
    CheckedCastError::PodCastError(err)
  }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
/// * If the slice contains an invalid bit pattern for `T`
#[inline]
pub fn try_from_bytes<T: CheckedBitPattern>(
  s: &[u8],
) -> Result<&T, CheckedCastError> {
  let pod = crate::try_from_bytes(s)?;

  if <T as CheckedBitPattern>::is_valid_bit_pattern(pod) {
    Ok(unsafe { &*(pod as *const <T as CheckedBitPattern>::Bits as *const T) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Re-interprets `&mut [u8]` as `&mut T`.
///
/// ## Failure
///
/// * If the slice isn't aligned for the new type
/// * If the slice's length isn’t exactly the size of the new type
/// * If the slice contains an invalid bit pattern for `T`
#[inline]
pub fn try_from_bytes_mut<T: CheckedBitPattern + NoUninit>(
  s: &mut [u8],
) -> Result<&mut T, CheckedCastError> {
  let pod = unsafe { internal::try_from_bytes_mut(s) }?;

  if <T as CheckedBitPattern>::is_valid_bit_pattern(pod) {
    Ok(unsafe { &mut *(pod as *mut <T as CheckedBitPattern>::Bits as *mut T) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Reads from the bytes as if they were a `T`.
///
/// ## Failure
/// * If the `bytes` length is not equal to `size_of::<T>()`.
/// * If the slice contains an invalid bit pattern for `T`
#[inline]
pub fn try_pod_read_unaligned<T: CheckedBitPattern>(
  bytes: &[u8],
) -> Result<T, CheckedCastError> {
  let pod = crate::try_pod_read_unaligned(bytes)?;

  if <T as CheckedBitPattern>::is_valid_bit_pattern(&pod) {
    Ok(unsafe { transmute!(pod) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
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
/// * If `a` contains an invalid bit pattern for `B` this fails.
#[inline]
pub fn try_cast<A: NoUninit, B: CheckedBitPattern>(
  a: A,
) -> Result<B, CheckedCastError> {
  let pod = crate::try_cast(a)?;

  if <B as CheckedBitPattern>::is_valid_bit_pattern(&pod) {
    Ok(unsafe { transmute!(pod) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Try to convert a `&A` into `&B`.
///
/// ## Failure
///
/// * If the reference isn't aligned in the new type
/// * If the source type and target type aren't the same size.
/// * If `a` contains an invalid bit pattern for `B` this fails.
#[inline]
pub fn try_cast_ref<A: NoUninit, B: CheckedBitPattern>(
  a: &A,
) -> Result<&B, CheckedCastError> {
  let pod = crate::try_cast_ref(a)?;

  if <B as CheckedBitPattern>::is_valid_bit_pattern(pod) {
    Ok(unsafe { &*(pod as *const <B as CheckedBitPattern>::Bits as *const B) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Try to convert a `&mut A` into `&mut B`.
///
/// As [`try_cast_ref`], but `mut`.
#[inline]
pub fn try_cast_mut<
  A: NoUninit + AnyBitPattern,
  B: CheckedBitPattern + NoUninit,
>(
  a: &mut A,
) -> Result<&mut B, CheckedCastError> {
  let pod = unsafe { internal::try_cast_mut(a) }?;

  if <B as CheckedBitPattern>::is_valid_bit_pattern(pod) {
    Ok(unsafe { &mut *(pod as *mut <B as CheckedBitPattern>::Bits as *mut B) })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
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
/// * If any element of the converted slice would contain an invalid bit pattern
///   for `B` this fails.
#[inline]
pub fn try_cast_slice<A: NoUninit, B: CheckedBitPattern>(
  a: &[A],
) -> Result<&[B], CheckedCastError> {
  let pod = crate::try_cast_slice(a)?;

  if pod.iter().all(|pod| <B as CheckedBitPattern>::is_valid_bit_pattern(pod)) {
    Ok(unsafe {
      core::slice::from_raw_parts(pod.as_ptr() as *const B, pod.len())
    })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Try to convert `&mut [A]` into `&mut [B]` (possibly with a change in
/// length).
///
/// As [`try_cast_slice`], but `&mut`.
#[inline]
pub fn try_cast_slice_mut<
  A: NoUninit + AnyBitPattern,
  B: CheckedBitPattern + NoUninit,
>(
  a: &mut [A],
) -> Result<&mut [B], CheckedCastError> {
  let pod = unsafe { internal::try_cast_slice_mut(a) }?;

  if pod.iter().all(|pod| <B as CheckedBitPattern>::is_valid_bit_pattern(pod)) {
    Ok(unsafe {
      core::slice::from_raw_parts_mut(pod.as_mut_ptr() as *mut B, pod.len())
    })
  } else {
    Err(CheckedCastError::InvalidBitPattern)
  }
}

/// Re-interprets `&[u8]` as `&T`.
///
/// ## Panics
///
/// This is [`try_from_bytes`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn from_bytes<T: CheckedBitPattern>(s: &[u8]) -> &T {
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
pub fn from_bytes_mut<T: NoUninit + CheckedBitPattern>(s: &mut [u8]) -> &mut T {
  match try_from_bytes_mut(s) {
    Ok(t) => t,
    Err(e) => something_went_wrong("from_bytes_mut", e),
  }
}

/// Reads the slice into a `T` value.
///
/// ## Panics
/// * This is like [`try_pod_read_unaligned`] but will panic on failure.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn pod_read_unaligned<T: CheckedBitPattern>(bytes: &[u8]) -> T {
  match try_pod_read_unaligned(bytes) {
    Ok(t) => t,
    Err(e) => something_went_wrong("pod_read_unaligned", e),
  }
}

/// Cast `A` into `B`
///
/// ## Panics
///
/// * This is like [`try_cast`], but will panic on a size mismatch.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast<A: NoUninit, B: CheckedBitPattern>(a: A) -> B {
  match try_cast(a) {
    Ok(t) => t,
    Err(e) => something_went_wrong("cast", e),
  }
}

/// Cast `&mut A` into `&mut B`.
///
/// ## Panics
///
/// This is [`try_cast_mut`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_mut<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + CheckedBitPattern,
>(
  a: &mut A,
) -> &mut B {
  match try_cast_mut(a) {
    Ok(t) => t,
    Err(e) => something_went_wrong("cast_mut", e),
  }
}

/// Cast `&A` into `&B`.
///
/// ## Panics
///
/// This is [`try_cast_ref`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_ref<A: NoUninit, B: CheckedBitPattern>(a: &A) -> &B {
  match try_cast_ref(a) {
    Ok(t) => t,
    Err(e) => something_went_wrong("cast_ref", e),
  }
}

/// Cast `&[A]` into `&[B]`.
///
/// ## Panics
///
/// This is [`try_cast_slice`] but will panic on error.
#[inline]
#[cfg_attr(feature = "track_caller", track_caller)]
pub fn cast_slice<A: NoUninit, B: CheckedBitPattern>(a: &[A]) -> &[B] {
  match try_cast_slice(a) {
    Ok(t) => t,
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
pub fn cast_slice_mut<
  A: NoUninit + AnyBitPattern,
  B: NoUninit + CheckedBitPattern,
>(
  a: &mut [A],
) -> &mut [B] {
  match try_cast_slice_mut(a) {
    Ok(t) => t,
    Err(e) => something_went_wrong("cast_slice_mut", e),
  }
}
