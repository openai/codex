use super::*;

/// A trait which indicates that a type is a `#[repr(transparent)]` wrapper
/// around the `Inner` value.
///
/// This allows safely copy transmuting between the `Inner` type and the
/// `TransparentWrapper` type. Functions like `wrap_{}` convert from the inner
/// type to the wrapper type and `peel_{}` functions do the inverse conversion
/// from the wrapper type to the inner type. We deliberately do not call the
/// wrapper-removing methods "unwrap" because at this point that word is too
/// strongly tied to the Option/ Result methods.
///
/// # Safety
///
/// The safety contract of `TransparentWrapper` is relatively simple:
///
/// For a given `Wrapper` which implements `TransparentWrapper<Inner>`:
///
/// 1. `Wrapper` must be a wrapper around `Inner` with an identical data
///    representations. This    either means that it must be a
///    `#[repr(transparent)]` struct which    contains a either a field of type
///    `Inner` (or a field of some other    transparent wrapper for `Inner`) as
///    the only non-ZST field.
///
/// 2. Any fields *other* than the `Inner` field must be trivially constructable
///    ZSTs, for example `PhantomData`, `PhantomPinned`, etc. (When deriving
///    `TransparentWrapper` on a type with ZST fields, the ZST fields must be
///    [`Zeroable`]).
///
/// 3. The `Wrapper` may not impose additional alignment requirements over
///    `Inner`.
///     - Note: this is currently guaranteed by `repr(transparent)`, but there
///       have been discussions of lifting it, so it's stated here explicitly.
///
/// 4. All functions on `TransparentWrapper` **may not** be overridden.
///
/// ## Caveats
///
/// If the wrapper imposes additional constraints upon the inner type which are
/// required for safety, it's responsible for ensuring those still hold -- this
/// generally requires preventing access to instances of the inner type, as
/// implementing `TransparentWrapper<U> for T` means anybody can call
/// `T::cast_ref(any_instance_of_u)`.
///
/// For example, it would be invalid to implement TransparentWrapper for `str`
/// to implement `TransparentWrapper` around `[u8]` because of this.
///
/// # Examples
///
/// ## Basic
///
/// ```
/// use bytemuck::TransparentWrapper;
/// # #[derive(Default)]
/// # struct SomeStruct(u32);
///
/// #[repr(transparent)]
/// struct MyWrapper(SomeStruct);
///
/// unsafe impl TransparentWrapper<SomeStruct> for MyWrapper {}
///
/// // interpret a reference to &SomeStruct as a &MyWrapper
/// let thing = SomeStruct::default();
/// let inner_ref: &MyWrapper = MyWrapper::wrap_ref(&thing);
///
/// // Works with &mut too.
/// let mut mut_thing = SomeStruct::default();
/// let inner_mut: &mut MyWrapper = MyWrapper::wrap_mut(&mut mut_thing);
///
/// # let _ = (inner_ref, inner_mut); // silence warnings
/// ```
///
/// ## Use with dynamically sized types
///
/// ```
/// use bytemuck::TransparentWrapper;
///
/// #[repr(transparent)]
/// struct Slice<T>([T]);
///
/// unsafe impl<T> TransparentWrapper<[T]> for Slice<T> {}
///
/// let s = Slice::wrap_ref(&[1u32, 2, 3]);
/// assert_eq!(&s.0, &[1, 2, 3]);
///
/// let mut buf = [1, 2, 3u8];
/// let sm = Slice::wrap_mut(&mut buf);
/// ```
///
/// ## Deriving
///
/// When deriving, the non-wrapped fields must uphold all the normal
/// requirements, and must also be `Zeroable`.
#[cfg_attr(feature = "derive", doc = "```")]
#[cfg_attr(
  not(feature = "derive"),
  doc = "```ignore
// This example requires the `derive` feature."
)]
/// use bytemuck::TransparentWrapper;
/// use std::marker::PhantomData;
///
/// #[derive(TransparentWrapper)]
/// #[repr(transparent)]
/// #[transparent(usize)]
/// struct Wrapper<T: ?Sized>(usize, PhantomData<T>); // PhantomData<T> implements Zeroable for all T
/// ```
///
/// Here, an error will occur, because `MyZst` does not implement `Zeroable`.
#[cfg_attr(feature = "derive", doc = "```compile_fail")]
#[cfg_attr(
  not(feature = "derive"),
  doc = "```ignore
// This example requires the `derive` feature."
)]
/// use bytemuck::TransparentWrapper;
/// struct MyZst;
///
/// #[derive(TransparentWrapper)]
/// #[repr(transparent)]
/// #[transparent(usize)]
/// struct Wrapper(usize, MyZst); // MyZst does not implement Zeroable
/// ```
pub unsafe trait TransparentWrapper<Inner: ?Sized> {
  /// Convert the inner type into the wrapper type.
  #[inline]
  fn wrap(s: Inner) -> Self
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that `Self` and `Inner` have
    // identical representations.
    unsafe { transmute!(s) }
  }

  /// Convert a reference to the inner type into a reference to the wrapper
  /// type.
  #[inline]
  fn wrap_ref(s: &Inner) -> &Self {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is the best we can do to assert their metadata is the same type
    // on stable.
    assert!(size_of::<*const Inner>() == size_of::<*const Self>());
    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY: The unsafe contract requires that these two have
      // identical representations.
      let inner_ptr = s as *const Inner;
      let wrapper_ptr: *const Self = transmute!(inner_ptr);
      &*wrapper_ptr
    }
  }

  /// Convert a mutable reference to the inner type into a mutable reference to
  /// the wrapper type.
  #[inline]
  fn wrap_mut(s: &mut Inner) -> &mut Self {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is about the best we can do on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());
    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY: The unsafe contract requires that these two have
      // identical representations.
      let inner_ptr = s as *mut Inner;
      let wrapper_ptr: *mut Self = transmute!(inner_ptr);
      &mut *wrapper_ptr
    }
  }

  /// Convert a slice to the inner type into a slice to the wrapper type.
  #[inline]
  fn wrap_slice(s: &[Inner]) -> &[Self]
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that these two have
    // identical representations (size and alignment).
    unsafe { core::slice::from_raw_parts(s.as_ptr() as *const Self, s.len()) }
  }

  /// Convert a mutable slice to the inner type into a mutable slice to the
  /// wrapper type.
  #[inline]
  fn wrap_slice_mut(s: &mut [Inner]) -> &mut [Self]
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that these two have
    // identical representations (size and alignment).
    unsafe {
      core::slice::from_raw_parts_mut(s.as_mut_ptr() as *mut Self, s.len())
    }
  }

  /// Convert the wrapper type into the inner type.
  #[inline]
  fn peel(s: Self) -> Inner
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that `Self` and `Inner` have
    // identical representations.
    unsafe { transmute!(s) }
  }

  /// Convert a reference to the wrapper type into a reference to the inner
  /// type.
  #[inline]
  fn peel_ref(s: &Self) -> &Inner {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is about the best we can do on stable.
    assert!(size_of::<*const Inner>() == size_of::<*const Self>());
    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY: The unsafe contract requires that these two have
      // identical representations.
      let wrapper_ptr = s as *const Self;
      let inner_ptr: *const Inner = transmute!(wrapper_ptr);
      &*inner_ptr
    }
  }

  /// Convert a mutable reference to the wrapper type into a mutable reference
  /// to the inner type.
  #[inline]
  fn peel_mut(s: &mut Self) -> &mut Inner {
    // The unsafe contract requires that these two have
    // identical representations, and thus identical pointer metadata.
    // Assert that Self and Inner have the same pointer size,
    // which is about the best we can do on stable.
    assert!(size_of::<*mut Inner>() == size_of::<*mut Self>());
    unsafe {
      // A pointer cast doesn't work here because rustc can't tell that
      // the vtables match (because of the `?Sized` restriction relaxation).
      // A `transmute` doesn't work because the sizes are unspecified.
      //
      // SAFETY: The unsafe contract requires that these two have
      // identical representations.
      let wrapper_ptr = s as *mut Self;
      let inner_ptr: *mut Inner = transmute!(wrapper_ptr);
      &mut *inner_ptr
    }
  }

  /// Convert a slice to the wrapped type into a slice to the inner type.
  #[inline]
  fn peel_slice(s: &[Self]) -> &[Inner]
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that these two have
    // identical representations (size and alignment).
    unsafe { core::slice::from_raw_parts(s.as_ptr() as *const Inner, s.len()) }
  }

  /// Convert a mutable slice to the wrapped type into a mutable slice to the
  /// inner type.
  #[inline]
  fn peel_slice_mut(s: &mut [Self]) -> &mut [Inner]
  where
    Self: Sized,
    Inner: Sized,
  {
    assert!(size_of::<Inner>() == size_of::<Self>());
    assert!(align_of::<Inner>() == align_of::<Self>());
    // SAFETY: The unsafe contract requires that these two have
    // identical representations (size and alignment).
    unsafe {
      core::slice::from_raw_parts_mut(s.as_mut_ptr() as *mut Inner, s.len())
    }
  }
}

unsafe impl<T> TransparentWrapper<T> for core::num::Wrapping<T> {}
#[cfg(feature = "transparentwrapper_extra")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "transparentwrapper_extra"))
)]
unsafe impl<T> TransparentWrapper<T> for core::num::Saturating<T> {}

// Note that `Reverse` existed since Rust 1.19.0, but was only made `#[repr(transparent)]`
// in Rust 1.52.0 (PR: https://github.com/rust-lang/rust/pull/81879), so we have it under
// the same feature as `Saturating`, which was stabilized in Rust 1.74.0, so that this
// impl cannot be used on a version before 1.52.0 where it would be unsound.
#[cfg(feature = "transparentwrapper_extra")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "transparentwrapper_extra"))
)]
unsafe impl<T> TransparentWrapper<T> for core::cmp::Reverse<T> {}
