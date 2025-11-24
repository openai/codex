// Copyright 2025 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use core::{
    cell::{Cell, UnsafeCell},
    mem::{ManuallyDrop, MaybeUninit},
    num::Wrapping,
};

use crate::{
    pointer::{invariant::*, PtrInner},
    FromBytes, Immutable, IntoBytes, Unalign,
};

/// Transmutations which are sound to attempt, conditional on validating the bit
/// validity of the destination type.
///
/// If a `Ptr` transmutation is `TryTransmuteFromPtr`, then it is sound to
/// perform that transmutation so long as some additional mechanism is used to
/// validate that the referent is bit-valid for the destination type. That
/// validation mechanism could be a type bound (such as `TransmuteFrom`) or a
/// runtime validity check.
///
/// # Safety
///
/// ## Post-conditions
///
/// Given `Dst: TryTransmuteFromPtr<Src, A, SV, DV, _>`, callers may assume the
/// following:
///
/// Given `src: Ptr<'a, Src, (A, _, SV)>`, if the referent of `src` is
/// `DV`-valid for `Dst`, then it is sound to transmute `src` into `dst: Ptr<'a,
/// Dst, (A, Unaligned, DV)>` by preserving pointer address and metadata.
///
/// ## Pre-conditions
///
/// Given `src: Ptr<Src, (A, _, SV)>` and `dst: Ptr<Dst, (A, Unaligned, DV)>`,
/// `Dst: TryTransmuteFromPtr<Src, A, SV, DV, _>` is sound if all of the
/// following hold:
/// - Forwards transmutation: Either of the following hold:
///   - So long as `dst` is active, no mutation of `dst`'s referent is allowed
///     except via `dst` itself
///   - The set of `DV`-valid `Dst`s is a superset of the set of `SV`-valid
///     `Src`s
/// - Reverse transmutation: Either of the following hold:
///   - `dst` does not permit mutation of its referent
///   - The set of `DV`-valid `Dst`s is a subset of the set of `SV`-valid `Src`s
/// - No safe code, given access to `src` and `dst`, can cause undefined
///   behavior: Any of the following hold:
///   - `A` is `Exclusive`
///   - `Src: Immutable` and `Dst: Immutable`
///   - It is sound for shared code to operate on a `&Src` and `&Dst` which
///     reference the same byte range at the same time
///
/// ## Proof
///
/// Given:
/// - `src: Ptr<'a, Src, (A, _, SV)>`
/// - `src`'s referent is `DV`-valid for `Dst`
/// - `Dst: SizeEq<Src>`
///
/// We are trying to prove that it is sound to perform a pointer address- and
/// metadata-preserving transmute from `src` to a `dst: Ptr<'a, Dst, (A,
/// Unaligned, DV)>`. We need to prove that such a transmute does not violate
/// any of `src`'s invariants, and that it satisfies all invariants of the
/// destination `Ptr` type.
///
/// First, all of `src`'s `PtrInner` invariants are upheld. `src`'s address and
/// metadata are unchanged, so:
/// - If its referent is not zero sized, then it still has valid provenance for
///   its referent, which is still entirely contained in some Rust allocation,
///   `A`
/// - If its referent is not zero sized, `A` is guaranteed to live for at least
///   `'a`
///
/// Since `Dst: SizeEq<Src>`, and since `dst` has the same address and metadata
/// as `src`, `dst` addresses the same byte range as `src`. `dst` also has the
/// same lifetime as `src`. Therefore, all of the `PtrInner` invariants
/// mentioned above also hold for `dst`.
///
/// Second, since `src`'s address is unchanged, it still satisfies its
/// alignment. Since `dst`'s alignment is `Unaligned`, it trivially satisfies
/// its alignment.
///
/// Third, aliasing is either `Exclusive` or `Shared`:
/// - If it is `Exclusive`, then both `src` and `dst` satisfy `Exclusive`
///   aliasing trivially: since `src` and `dst` have the same lifetime, `src` is
///   inaccessible so long as `dst` is alive, and no other live `Ptr`s or
///   references may reference the same referent.
/// - If it is `Shared`, then either:
///   - `Src: Immutable` and `Dst: Immutable`, and so `UnsafeCell`s trivially
///     cover the same byte ranges in both types.
///   - It is explicitly sound for safe code to operate on a `&Src` and a `&Dst`
///     pointing to the same byte range at the same time.
///
/// Fourth, `src`'s validity is satisfied. By invariant, `src`'s referent began
/// as an `SV`-valid `Src`. It is guaranteed to remain so, as either of the
/// following hold:
/// - `dst` does not permit mutation of its referent.
/// - The set of `DV`-valid `Dst`s is a superset of the set of `SV`-valid
///   `Src`s. Thus, any value written via `dst` is guaranteed to be `SV`-valid
///   for `Src`.
///
/// Fifth, `dst`'s validity is satisfied. It is a given of this proof that the
/// referent is `DV`-valid for `Dst`. It is guaranteed to remain so, as either
/// of the following hold:
/// - So long as `dst` is active, no mutation of the referent is allowed except
///   via `dst` itself.
/// - The set of `DV`-valid `Dst`s is a superset of the set of `SV`-valid
///   `Src`s. Thus, any value written via `src` is guaranteed to be a `DV`-valid
///   `Dst`.
pub unsafe trait TryTransmuteFromPtr<Src: ?Sized, A: Aliasing, SV: Validity, DV: Validity, R>:
    SizeEq<Src>
{
}

#[allow(missing_copy_implementations, missing_debug_implementations)]
pub enum BecauseMutationCompatible {}

// SAFETY:
// - Forwards transmutation: By `Dst: MutationCompatible<Src, A, SV, DV, _>`, we
//   know that at least one of the following holds:
//   - So long as `dst: Ptr<Dst>` is active, no mutation of its referent is
//     allowed except via `dst` itself if either of the following hold:
//     - Aliasing is `Exclusive`, in which case, so long as the `Dst` `Ptr`
//       exists, no mutation is permitted except via that `Ptr`
//     - Aliasing is `Shared`, `Src: Immutable`, and `Dst: Immutable`, in which
//       case no mutation is possible via either `Ptr`
//   - `Dst: TransmuteFrom<Src, SV, DV>`. Since `Dst: SizeEq<Src>`, this bound
//     guarantees that the set of `DV`-valid `Dst`s is a supserset of the set of
//     `SV`-valid `Src`s.
// - Reverse transmutation: `Src: TransmuteFrom<Dst, DV, SV>`. Since `Dst:
//   SizeEq<Src>`, this guarantees that the set of `DV`-valid `Dst`s is a subset
//   of the set of `SV`-valid `Src`s.
// - No safe code, given access to `src` and `dst`, can cause undefined
//   behavior: By `Dst: MutationCompatible<Src, A, SV, DV, _>`, at least one of
//   the following holds:
//   - `A` is `Exclusive`
//   - `Src: Immutable` and `Dst: Immutable`
//   - `Dst: InvariantsEq<Src>`, which guarantees that `Src` and `Dst` have the
//     same invariants, and have `UnsafeCell`s covering the same byte ranges
unsafe impl<Src, Dst, SV, DV, A, R>
    TryTransmuteFromPtr<Src, A, SV, DV, (BecauseMutationCompatible, R)> for Dst
where
    A: Aliasing,
    SV: Validity,
    DV: Validity,
    Src: TransmuteFrom<Dst, DV, SV> + ?Sized,
    Dst: MutationCompatible<Src, A, SV, DV, R> + SizeEq<Src> + ?Sized,
{
}

// SAFETY:
// - Forwards transmutation: Since aliasing is `Shared` and `Src: Immutable`,
//   `src` does not permit mutation of its referent.
// - Reverse transmutation: Since aliasing is `Shared` and `Dst: Immutable`,
//   `dst` does not permit mutation of its referent.
// - No safe code, given access to `src` and `dst`, can cause undefined
//   behavior: `Src: Immutable` and `Dst: Immutable`
unsafe impl<Src, Dst, SV, DV> TryTransmuteFromPtr<Src, Shared, SV, DV, BecauseImmutable> for Dst
where
    SV: Validity,
    DV: Validity,
    Src: Immutable + ?Sized,
    Dst: Immutable + SizeEq<Src> + ?Sized,
{
}

/// Denotes that `src: Ptr<Src, (A, _, SV)>` and `dst: Ptr<Self, (A, _, DV)>`,
/// referencing the same referent at the same time, cannot be used by safe code
/// to break library safety invariants of `Src` or `Self`.
///
/// # Safety
///
/// At least one of the following must hold:
/// - `Src: Read<A, _>` and `Self: Read<A, _>`
/// - `Self: InvariantsEq<Src>`, and, for some `V`:
///   - `Dst: TransmuteFrom<Src, V, V>`
///   - `Src: TransmuteFrom<Dst, V, V>`
pub unsafe trait MutationCompatible<Src: ?Sized, A: Aliasing, SV, DV, R> {}

#[allow(missing_copy_implementations, missing_debug_implementations)]
pub enum BecauseRead {}

// SAFETY: `Src: Read<A, _>` and `Dst: Read<A, _>`.
unsafe impl<Src: ?Sized, Dst: ?Sized, A: Aliasing, SV: Validity, DV: Validity, R, S>
    MutationCompatible<Src, A, SV, DV, (BecauseRead, (R, S))> for Dst
where
    Src: Read<A, R>,
    Dst: Read<A, S>,
{
}

/// Denotes that two types have the same invariants.
///
/// # Safety
///
/// It is sound for safe code to operate on a `&T` and a `&Self` pointing to the
/// same referent at the same time - no such safe code can cause undefined
/// behavior.
pub unsafe trait InvariantsEq<T: ?Sized> {}

// SAFETY: Trivially sound to have multiple `&T` pointing to the same referent.
unsafe impl<T: ?Sized> InvariantsEq<T> for T {}

// SAFETY: `Dst: InvariantsEq<Src> + TransmuteFrom<Src, SV, DV>`, and `Src:
// TransmuteFrom<Dst, DV, SV>`.
unsafe impl<Src: ?Sized, Dst: ?Sized, A: Aliasing, SV: Validity, DV: Validity>
    MutationCompatible<Src, A, SV, DV, BecauseInvariantsEq> for Dst
where
    Src: TransmuteFrom<Dst, DV, SV>,
    Dst: TransmuteFrom<Src, SV, DV> + InvariantsEq<Src>,
{
}

pub(crate) enum BecauseInvariantsEq {}

macro_rules! unsafe_impl_invariants_eq {
    ($tyvar:ident => $t:ty, $u:ty) => {{
        crate::util::macros::__unsafe();
        // SAFETY: The caller promises that this is sound.
        unsafe impl<$tyvar> InvariantsEq<$t> for $u {}
        // SAFETY: The caller promises that this is sound.
        unsafe impl<$tyvar> InvariantsEq<$u> for $t {}
    }};
}

impl_transitive_transmute_from!(T => MaybeUninit<T> => T => Wrapping<T>);
impl_transitive_transmute_from!(T => Wrapping<T> => T => MaybeUninit<T>);

// SAFETY: `ManuallyDrop<T>` has the same size and bit validity as `T` [1], and
// implements `Deref<Target = T>` [2]. Thus, it is already possible for safe
// code to obtain a `&T` and a `&ManuallyDrop<T>` to the same referent at the
// same time.
//
// [1] Per https://doc.rust-lang.org/1.81.0/std/mem/struct.ManuallyDrop.html:
//
//   `ManuallyDrop<T>` is guaranteed to have the same layout and bit
//   validity as `T`
//
// [2] https://doc.rust-lang.org/1.81.0/std/mem/struct.ManuallyDrop.html#impl-Deref-for-ManuallyDrop%3CT%3E
unsafe impl<T: ?Sized> InvariantsEq<T> for ManuallyDrop<T> {}
// SAFETY: See previous safety comment.
unsafe impl<T: ?Sized> InvariantsEq<ManuallyDrop<T>> for T {}

/// Transmutations which are always sound.
///
/// `TransmuteFromPtr` is a shorthand for [`TryTransmuteFromPtr`] and
/// [`TransmuteFrom`].
///
/// # Safety
///
/// `Dst: TransmuteFromPtr<Src, A, SV, DV, _>` is equivalent to `Dst:
/// TryTransmuteFromPtr<Src, A, SV, DV, _> + TransmuteFrom<Src, SV, DV>`.
pub unsafe trait TransmuteFromPtr<Src: ?Sized, A: Aliasing, SV: Validity, DV: Validity, R>:
    TryTransmuteFromPtr<Src, A, SV, DV, R> + TransmuteFrom<Src, SV, DV>
{
}

// SAFETY: The `where` bounds are equivalent to the safety invariant on
// `TransmuteFromPtr`.
unsafe impl<Src: ?Sized, Dst: ?Sized, A: Aliasing, SV: Validity, DV: Validity, R>
    TransmuteFromPtr<Src, A, SV, DV, R> for Dst
where
    Dst: TransmuteFrom<Src, SV, DV> + TryTransmuteFromPtr<Src, A, SV, DV, R>,
{
}

/// Denotes that any `SV`-valid `Src` may soundly be transmuted into a
/// `DV`-valid `Self`.
///
/// # Safety
///
/// Given `src: Ptr<Src, (_, _, SV)>` and `dst: Ptr<Dst, (_, _, DV)>`, if the
/// referents of `src` and `dst` are the same size, then the set of bit patterns
/// allowed to appear in `src`'s referent must be a subset of the set allowed to
/// appear in `dst`'s referent.
///
/// If the referents are not the same size, then `Dst: TransmuteFrom<Src, SV,
/// DV>` conveys no safety guarantee.
pub unsafe trait TransmuteFrom<Src: ?Sized, SV, DV> {}

/// # Safety
///
/// `T` and `Self` must have the same vtable kind (`Sized`, slice DST, `dyn`,
/// etc) and have the same size. In particular:
/// - If `T: Sized` and `Self: Sized`, then their sizes must be equal
/// - If `T: ?Sized` and `Self: ?Sized`, then it must be the case that, given
///   any `t: PtrInner<'_, T>`, `<Self as SizeEq<T>>::cast_from_raw(t)` produces
///   a pointer which addresses the same number of bytes as `t`. *Note that it
///   is **not** guaranteed that an `as` cast preserves referent size: it may be
///   the case that `cast_from_raw` modifies the pointer's metadata in order to
///   preserve referent size, which an `as` cast does not do.*
pub unsafe trait SizeEq<T: ?Sized> {
    fn cast_from_raw(t: PtrInner<'_, T>) -> PtrInner<'_, Self>;
}

// SAFETY: `T` trivially has the same size and vtable kind as `T`, and since
// pointer `*mut T -> *mut T` pointer casts are no-ops, this cast trivially
// preserves referent size (when `T: ?Sized`).
unsafe impl<T: ?Sized> SizeEq<T> for T {
    #[inline(always)]
    fn cast_from_raw(t: PtrInner<'_, T>) -> PtrInner<'_, T> {
        t
    }
}

// SAFETY: Since `Src: IntoBytes`, the set of valid `Src`'s is the set of
// initialized bit patterns, which is exactly the set allowed in the referent of
// any `Initialized` `Ptr`.
unsafe impl<Src, Dst> TransmuteFrom<Src, Valid, Initialized> for Dst
where
    Src: IntoBytes + ?Sized,
    Dst: ?Sized,
{
}

// SAFETY: Since `Dst: FromBytes`, any initialized bit pattern may appear in the
// referent of a `Ptr<Dst, (_, _, Valid)>`. This is exactly equal to the set of
// bit patterns which may appear in the referent of any `Initialized` `Ptr`.
unsafe impl<Src, Dst> TransmuteFrom<Src, Initialized, Valid> for Dst
where
    Src: ?Sized,
    Dst: FromBytes + ?Sized,
{
}

// FIXME(#2354): This seems like a smell - the soundness of this bound has
// nothing to do with `Src` or `Dst` - we're basically just saying `[u8; N]` is
// transmutable into `[u8; N]`.

// SAFETY: The set of allowed bit patterns in the referent of any `Initialized`
// `Ptr` is the same regardless of referent type.
unsafe impl<Src, Dst> TransmuteFrom<Src, Initialized, Initialized> for Dst
where
    Src: ?Sized,
    Dst: ?Sized,
{
}

// FIXME(#2354): This seems like a smell - the soundness of this bound has
// nothing to do with `Dst` - we're basically just saying that any type is
// transmutable into `MaybeUninit<[u8; N]>`.

// SAFETY: A `Dst` with validity `Uninit` permits any byte sequence, and
// therefore can be transmuted from any value.
unsafe impl<Src, Dst, V> TransmuteFrom<Src, V, Uninit> for Dst
where
    Src: ?Sized,
    Dst: ?Sized,
    V: Validity,
{
}

// SAFETY:
// - `ManuallyDrop<T>` has the same size as `T` [1]
// - `ManuallyDrop<T>` has the same validity as `T` [1]
//
// [1] Per https://doc.rust-lang.org/1.81.0/std/mem/struct.ManuallyDrop.html:
//
//   `ManuallyDrop<T>` is guaranteed to have the same layout and bit validity as
//   `T`
const _: () = unsafe { unsafe_impl_for_transparent_wrapper!(T: ?Sized => ManuallyDrop<T>) };

// SAFETY:
// - `Unalign<T>` promises to have the same size as `T`.
// - `Unalign<T>` promises to have the same validity as `T`.
const _: () = unsafe { unsafe_impl_for_transparent_wrapper!(T => Unalign<T>) };
// SAFETY: `Unalign<T>` promises to have the same size and validity as `T`.
// Given `u: &Unalign<T>`, it is already possible to obtain `let t =
// u.try_deref().unwrap()`. Because `Unalign<T>` has the same size as `T`, the
// returned `&T` must point to the same referent as `u`, and thus it must be
// sound for these two references to exist at the same time since it's already
// possible for safe code to get into this state.
const _: () = unsafe { unsafe_impl_invariants_eq!(T => T, Unalign<T>) };

// SAFETY:
// - `Wrapping<T>` has the same size as `T` [1].
// - `Wrapping<T>` has only one field, which is `pub` [2]. We are also
//   guaranteed per that `Wrapping<T>` has the same layout as `T` [1]. The only
//   way for both of these to be true simultaneously is for `Wrapping<T>` to
//   have the same bit validity as `T`. In particular, in order to change the
//   bit validity, one of the following would need to happen:
//   - `Wrapping` could change its `repr`, but this would violate the layout
//     guarantee.
//   - `Wrapping` could add or change its fields, but this would be a
//     stability-breaking change.
//
// [1] Per https://doc.rust-lang.org/1.85.0/core/num/struct.Wrapping.html#layout-1:
//
//   `Wrapping<T>` is guaranteed to have the same layout and ABI as `T`.
//
// [2] Definition from https://doc.rust-lang.org/1.85.0/core/num/struct.Wrapping.html:
//
//   ```
//   #[repr(transparent)]
//   pub struct Wrapping<T>(pub T);
//   ```
const _: () = unsafe { unsafe_impl_for_transparent_wrapper!(T => Wrapping<T>) };

// SAFETY: By the preceding safety proof, `Wrapping<T>` and `T` have the same
// layout and bit validity. Since a `Wrapping<T>`'s `T` field is `pub`, given
// `w: &Wrapping<T>`, it's possible to do `let t = &w.t`, which means that it's
// already possible for safe code to obtain a `&Wrapping<T>` and a `&T` pointing
// to the same referent at the same time. Thus, this must be sound.
const _: () = unsafe { unsafe_impl_invariants_eq!(T => T, Wrapping<T>) };

// SAFETY:
// - `UnsafeCell<T>` has the same size as `T` [1].
// - Per [1], `UnsafeCell<T>` has the same bit validity as `T`. Technically the
//   term "representation" doesn't guarantee this, but the subsequent sentence
//   in the documentation makes it clear that this is the intention.
//
// [1] Per https://doc.rust-lang.org/1.81.0/core/cell/struct.UnsafeCell.html#memory-layout:
//
//   `UnsafeCell<T>` has the same in-memory representation as its inner type
//   `T`. A consequence of this guarantee is that it is possible to convert
//   between `T` and `UnsafeCell<T>`.
const _: () = unsafe { unsafe_impl_for_transparent_wrapper!(T: ?Sized => UnsafeCell<T>) };

// SAFETY:
// - `Cell<T>` has the same size as `T` [1].
// - Per [1], `Cell<T>` has the same bit validity as `T`. Technically the term
//   "representation" doesn't guarantee this, but it does promise to have the
//   "same memory layout and caveats as `UnsafeCell<T>`." The `UnsafeCell` docs
//   [2] make it clear that bit validity is the intention even if that phrase
//   isn't used.
//
// [1] Per https://doc.rust-lang.org/1.85.0/std/cell/struct.Cell.html#memory-layout:
//
//   `Cell<T>` has the same memory layout and caveats as `UnsafeCell<T>`. In
//   particular, this means that `Cell<T>` has the same in-memory representation
//   as its inner type `T`.
//
// [2] Per https://doc.rust-lang.org/1.81.0/core/cell/struct.UnsafeCell.html#memory-layout:
//
//   `UnsafeCell<T>` has the same in-memory representation as its inner type
//   `T`. A consequence of this guarantee is that it is possible to convert
//   between `T` and `UnsafeCell<T>`.
const _: () = unsafe { unsafe_impl_for_transparent_wrapper!(T: ?Sized => Cell<T>) };

impl_transitive_transmute_from!(T: ?Sized => Cell<T> => T => UnsafeCell<T>);
impl_transitive_transmute_from!(T: ?Sized => UnsafeCell<T> => T => Cell<T>);

// SAFETY: `MaybeUninit<T>` has no validity requirements. Currently this is not
// explicitly guaranteed, but it's obvious from `MaybeUninit`'s documentation
// that this is the intention:
// https://doc.rust-lang.org/1.85.0/core/mem/union.MaybeUninit.html
unsafe impl<T> TransmuteFrom<T, Uninit, Valid> for MaybeUninit<T> {}

// SAFETY: `MaybeUninit<T>` has the same size as `T` [1].
//
// [1] Per https://doc.rust-lang.org/1.81.0/std/mem/union.MaybeUninit.html#layout-1:
//
//   `MaybeUninit<T>` is guaranteed to have the same size, alignment, and ABI as
//   `T`
unsafe impl<T> SizeEq<T> for MaybeUninit<T> {
    #[inline(always)]
    fn cast_from_raw(t: PtrInner<'_, T>) -> PtrInner<'_, MaybeUninit<T>> {
        // SAFETY: Per preceding safety comment, `MaybeUninit<T>` and `T` have
        // the same size, and so this cast preserves referent size.
        unsafe { cast!(t) }
    }
}

// SAFETY: See previous safety comment.
unsafe impl<T> SizeEq<MaybeUninit<T>> for T {
    #[inline(always)]
    fn cast_from_raw(t: PtrInner<'_, MaybeUninit<T>>) -> PtrInner<'_, T> {
        // SAFETY: Per preceding safety comment, `MaybeUninit<T>` and `T` have
        // the same size, and so this cast preserves referent size.
        unsafe { cast!(t) }
    }
}
