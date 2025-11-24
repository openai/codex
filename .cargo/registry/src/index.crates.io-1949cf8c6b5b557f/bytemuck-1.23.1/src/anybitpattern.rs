use crate::{Pod, Zeroable};

/// Marker trait for "plain old data" types that are valid for any bit pattern.
///
/// The requirements for this is very similar to [`Pod`], except that the type
/// can allow uninit (or padding) bytes. This limits what you can do with a type
/// of this kind, but also broadens the included types to `repr(C)` `struct`s
/// that contain padding as well as `union`s. Notably, you can only cast
/// *immutable* references and *owned* values into [`AnyBitPattern`] types, not
/// *mutable* references.
///
/// [`Pod`] is a subset of [`AnyBitPattern`], meaning that any `T: Pod` is also
/// [`AnyBitPattern`] but any `T: AnyBitPattern` is not necessarily [`Pod`].
///
/// [`AnyBitPattern`] is a subset of [`Zeroable`], meaning that any `T:
/// AnyBitPattern` is also [`Zeroable`], but any `T: Zeroable` is not
/// necessarily [`AnyBitPattern`]
///
/// # Derive
///
/// A `#[derive(AnyBitPattern)]` macro is provided under the `derive` feature
/// flag which will automatically validate the requirements of this trait and
/// implement the trait for you for both structs and enums. This is the
/// recommended method for implementing the trait, however it's also possible to
/// do manually. If you implement it manually, you *must* carefully follow the
/// below safety rules.
///
/// * *NOTE: even `C-style`, fieldless enums are intentionally **excluded** from
///   this trait, since it is **unsound** for an enum to have a discriminant
///   value that is not one of its defined variants.
///
/// # Safety
///
/// Similar to [`Pod`] except we disregard the rule about it must not contain
/// uninit bytes. Still, this is a quite strong guarantee about a type, so *be
/// careful* when implementing it manually.
///
/// * The type must be inhabited (eg: no
///   [Infallible](core::convert::Infallible)).
/// * The type must be valid for any bit pattern of its backing memory.
/// * Structs need to have all fields also be `AnyBitPattern`.
/// * It is disallowed for types to contain pointer types, `Cell`, `UnsafeCell`,
///   atomics, and any other forms of interior mutability.
/// * More precisely: A shared reference to the type must allow reads, and
///   *only* reads. RustBelt's separation logic is based on the notion that a
///   type is allowed to define a sharing predicate, its own invariant that must
///   hold for shared references, and this predicate is the reasoning that allow
///   it to deal with atomic and cells etc. We require the sharing predicate to
///   be trivial and permit only read-only access.
/// * There's probably more, don't mess it up (I mean it).
pub unsafe trait AnyBitPattern:
  Zeroable + Sized + Copy + 'static
{
}

unsafe impl<T: Pod> AnyBitPattern for T {}

#[cfg(feature = "zeroable_maybe_uninit")]
#[cfg_attr(
  feature = "nightly_docs",
  doc(cfg(feature = "zeroable_maybe_uninit"))
)]
unsafe impl<T> AnyBitPattern for core::mem::MaybeUninit<T> where T: AnyBitPattern
{}
