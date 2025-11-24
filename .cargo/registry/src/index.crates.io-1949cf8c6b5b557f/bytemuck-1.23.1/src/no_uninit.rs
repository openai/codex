use crate::Pod;
use core::num::{
  NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize,
  NonZeroU128, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
};

/// Marker trait for "plain old data" types with no uninit (or padding) bytes.
///
/// The requirements for this is very similar to [`Pod`],
/// except that it doesn't require that all bit patterns of the type are valid,
/// i.e. it does not require the type to be [`Zeroable`][crate::Zeroable].
/// This limits what you can do with a type of this kind, but also broadens the
/// included types to things like C-style enums. Notably, you can only cast from
/// *immutable* references to a [`NoUninit`] type into *immutable* references of
/// any other type, no casting of mutable references or mutable references to
/// slices etc.
///
/// [`Pod`] is a subset of [`NoUninit`], meaning that any `T: Pod` is also
/// [`NoUninit`] but any `T: NoUninit` is not necessarily [`Pod`]. If possible,
/// prefer implementing [`Pod`] directly. To get more [`Pod`]-like functionality
/// for a type that is only [`NoUninit`], consider also implementing
/// [`CheckedBitPattern`][crate::CheckedBitPattern].
///
/// # Derive
///
/// A `#[derive(NoUninit)]` macro is provided under the `derive` feature flag
/// which will automatically validate the requirements of this trait and
/// implement the trait for you for both enums and structs. This is the
/// recommended method for implementing the trait, however it's also possible to
/// do manually. If you implement it manually, you *must* carefully follow the
/// below safety rules.
///
/// # Safety
///
/// The same as [`Pod`] except we disregard the rule about it must
/// allow any bit pattern (i.e. it does not need to be
/// [`Zeroable`][crate::Zeroable]). Still, this is a quite strong guarantee
/// about a type, so *be careful* whem implementing it manually.
///
/// * The type must be inhabited (eg: no
///   [Infallible](core::convert::Infallible)).
/// * The type must not contain any uninit (or padding) bytes, either in the
///   middle or on the end (eg: no `#[repr(C)] struct Foo(u8, u16)`, which has
///   padding in the middle, and also no `#[repr(C)] struct Foo(u16, u8)`, which
///   has padding on the end).
/// * Structs need to have all fields also be `NoUninit`.
/// * Structs need to be `repr(C)` or `repr(transparent)`. In the case of
///   `repr(C)`, the `packed` and `align` repr modifiers can be used as long as
///   all other rules end up being followed.
/// * Enums need to have an explicit `#[repr(Int)]`
/// * Enums must have only fieldless variants
/// * It is disallowed for types to contain pointer types, `Cell`, `UnsafeCell`,
///   atomics, and any other forms of interior mutability.
/// * More precisely: A shared reference to the type must allow reads, and
///   *only* reads. RustBelt's separation logic is based on the notion that a
///   type is allowed to define a sharing predicate, its own invariant that must
///   hold for shared references, and this predicate is the reasoning that allow
///   it to deal with atomic and cells etc. We require the sharing predicate to
///   be trivial and permit only read-only access.
/// * There's probably more, don't mess it up (I mean it).
pub unsafe trait NoUninit: Sized + Copy + 'static {}

unsafe impl<T: Pod> NoUninit for T {}

unsafe impl NoUninit for char {}

unsafe impl NoUninit for bool {}

unsafe impl NoUninit for NonZeroU8 {}
unsafe impl NoUninit for NonZeroI8 {}
unsafe impl NoUninit for NonZeroU16 {}
unsafe impl NoUninit for NonZeroI16 {}
unsafe impl NoUninit for NonZeroU32 {}
unsafe impl NoUninit for NonZeroI32 {}
unsafe impl NoUninit for NonZeroU64 {}
unsafe impl NoUninit for NonZeroI64 {}
unsafe impl NoUninit for NonZeroU128 {}
unsafe impl NoUninit for NonZeroI128 {}
unsafe impl NoUninit for NonZeroUsize {}
unsafe impl NoUninit for NonZeroIsize {}
