#![allow(clippy::legacy_numeric_constants)]

use super::*;

/// A trait indicating that:
///
/// 1. A type has an equivalent representation to some known integral type.
/// 2. All instances of this type fall in a fixed range of values.
/// 3. Within that range, there are no gaps.
///
/// This is generally useful for fieldless enums (aka "c-style" enums), however
/// it's important that it only be used for those with an explicit `#[repr]`, as
/// `#[repr(Rust)]` fieldess enums have an unspecified layout.
///
/// Additionally, you shouldn't assume that all implementations are enums. Any
/// type which meets the requirements above while following the rules under
/// "Safety" below is valid.
///
/// # Example
///
/// ```
/// # use bytemuck::Contiguous;
/// #[repr(u8)]
/// #[derive(Debug, Copy, Clone, PartialEq)]
/// enum Foo {
///   A = 0,
///   B = 1,
///   C = 2,
///   D = 3,
///   E = 4,
/// }
/// unsafe impl Contiguous for Foo {
///   type Int = u8;
///   const MIN_VALUE: u8 = Foo::A as u8;
///   const MAX_VALUE: u8 = Foo::E as u8;
/// }
/// assert_eq!(Foo::from_integer(3).unwrap(), Foo::D);
/// assert_eq!(Foo::from_integer(8), None);
/// assert_eq!(Foo::C.into_integer(), 2);
/// ```
/// # Safety
///
/// This is an unsafe trait, and incorrectly implementing it is undefined
/// behavior.
///
/// Informally, by implementing it, you're asserting that `C` is identical to
/// the integral type `C::Int`, and that every `C` falls between `C::MIN_VALUE`
/// and `C::MAX_VALUE` exactly once, without any gaps.
///
/// Precisely, the guarantees you must uphold when implementing `Contiguous` for
/// some type `C` are:
///
/// 1. The size of `C` and `C::Int` must be the same, and neither may be a ZST.
///    (Note: alignment is explicitly allowed to differ)
///
/// 2. `C::Int` must be a primitive integer, and not a wrapper type. In the
///    future, this may be lifted to include cases where the behavior is
///    identical for a relevant set of traits (Ord, arithmetic, ...).
///
/// 3. All `C::Int`s which are in the *inclusive* range between `C::MIN_VALUE`
///    and `C::MAX_VALUE` are bitwise identical to unique valid instances of
///    `C`.
///
/// 4. There exist no instances of `C` such that their bitpatterns, when
///    interpreted as instances of `C::Int`, fall outside of the `MAX_VALUE` /
///    `MIN_VALUE` range -- It is legal for unsafe code to assume that if it
///    gets a `C` that implements `Contiguous`, it is in the appropriate range.
///
/// 5. Finally, you promise not to provide overridden implementations of
///    `Contiguous::from_integer` and `Contiguous::into_integer`.
///
/// For clarity, the following rules could be derived from the above, but are
/// listed explicitly:
///
/// - `C::MAX_VALUE` must be greater or equal to `C::MIN_VALUE` (therefore, `C`
///   must be an inhabited type).
///
/// - There exist no two values between `MIN_VALUE` and `MAX_VALUE` such that
///   when interpreted as a `C` they are considered identical (by, say, match).
pub unsafe trait Contiguous: Copy + 'static {
  /// The primitive integer type with an identical representation to this
  /// type.
  ///
  /// Contiguous is broadly intended for use with fieldless enums, and for
  /// these the correct integer type is easy: The enum should have a
  /// `#[repr(Int)]` or `#[repr(C)]` attribute, (if it does not, it is
  /// *unsound* to implement `Contiguous`!).
  ///
  /// - For `#[repr(Int)]`, use the listed `Int`. e.g. `#[repr(u8)]` should use
  ///   `type Int = u8`.
  ///
  /// - For `#[repr(C)]`, use whichever type the C compiler will use to
  ///   represent the given enum. This is usually `c_int` (from `std::os::raw`
  ///   or `libc`), but it's up to you to make the determination as the
  ///   implementer of the unsafe trait.
  ///
  /// For precise rules, see the list under "Safety" above.
  type Int: Copy + Ord;

  /// The upper *inclusive* bound for valid instances of this type.
  const MAX_VALUE: Self::Int;

  /// The lower *inclusive* bound for valid instances of this type.
  const MIN_VALUE: Self::Int;

  /// If `value` is within the range for valid instances of this type,
  /// returns `Some(converted_value)`, otherwise, returns `None`.
  ///
  /// This is a trait method so that you can write `value.into_integer()` in
  /// your code. It is a contract of this trait that if you implement
  /// `Contiguous` on your type you **must not** override this method.
  ///
  /// # Panics
  ///
  /// We will not panic for any correct implementation of `Contiguous`, but
  /// *may* panic if we detect an incorrect one.
  ///
  /// This is undefined behavior regardless, so it could have been the nasal
  /// demons at that point anyway ;).
  #[inline]
  #[cfg_attr(feature = "track_caller", track_caller)]
  fn from_integer(value: Self::Int) -> Option<Self> {
    // Guard against an illegal implementation of Contiguous. Annoyingly we
    // can't rely on `transmute` to do this for us (see below), but
    // whatever, this gets compiled into nothing in release.
    assert!(size_of::<Self>() == size_of::<Self::Int>());
    if Self::MIN_VALUE <= value && value <= Self::MAX_VALUE {
      // SAFETY: We've checked their bounds (and their size, even though
      // they've sworn under the Oath Of Unsafe Rust that that already
      // matched) so this is allowed by `Contiguous`'s unsafe contract.
      //
      // So, the `transmute!`. ideally we'd use transmute here, which
      // is more obviously safe. Sadly, we can't, as these types still
      // have unspecified sizes.
      Some(unsafe { transmute!(value) })
    } else {
      None
    }
  }

  /// Perform the conversion from `C` into the underlying integral type. This
  /// mostly exists otherwise generic code would need unsafe for the `value as
  /// integer`
  ///
  /// This is a trait method so that you can write `value.into_integer()` in
  /// your code. It is a contract of this trait that if you implement
  /// `Contiguous` on your type you **must not** override this method.
  ///
  /// # Panics
  ///
  /// We will not panic for any correct implementation of `Contiguous`, but
  /// *may* panic if we detect an incorrect one.
  ///
  /// This is undefined behavior regardless, so it could have been the nasal
  /// demons at that point anyway ;).
  #[inline]
  #[cfg_attr(feature = "track_caller", track_caller)]
  fn into_integer(self) -> Self::Int {
    // Guard against an illegal implementation of Contiguous. Annoyingly we
    // can't rely on `transmute` to do the size check for us (see
    // `from_integer's comment`), but whatever, this gets compiled into
    // nothing in release. Note that we don't check the result of cast
    assert!(size_of::<Self>() == size_of::<Self::Int>());

    // SAFETY: The unsafe contract requires that these have identical
    // representations, and that the range be entirely valid. Using
    // transmute! instead of transmute here is annoying, but is required
    // as `Self` and `Self::Int` have unspecified sizes still.
    unsafe { transmute!(self) }
  }
}

macro_rules! impl_contiguous {
  ($($src:ty as $repr:ident in [$min:expr, $max:expr];)*) => {$(
    unsafe impl Contiguous for $src {
      type Int = $repr;
      const MAX_VALUE: $repr = $max;
      const MIN_VALUE: $repr = $min;
    }
  )*};
}

impl_contiguous! {
  bool as u8 in [0, 1];

  u8 as u8 in [0, u8::max_value()];
  u16 as u16 in [0, u16::max_value()];
  u32 as u32 in [0, u32::max_value()];
  u64 as u64 in [0, u64::max_value()];
  u128 as u128 in [0, u128::max_value()];
  usize as usize in [0, usize::max_value()];

  i8 as i8 in [i8::min_value(), i8::max_value()];
  i16 as i16 in [i16::min_value(), i16::max_value()];
  i32 as i32 in [i32::min_value(), i32::max_value()];
  i64 as i64 in [i64::min_value(), i64::max_value()];
  i128 as i128 in [i128::min_value(), i128::max_value()];
  isize as isize in [isize::min_value(), isize::max_value()];

  NonZeroU8 as u8 in [1, u8::max_value()];
  NonZeroU16 as u16 in [1, u16::max_value()];
  NonZeroU32 as u32 in [1, u32::max_value()];
  NonZeroU64 as u64 in [1, u64::max_value()];
  NonZeroU128 as u128 in [1, u128::max_value()];
  NonZeroUsize as usize in [1, usize::max_value()];
}
