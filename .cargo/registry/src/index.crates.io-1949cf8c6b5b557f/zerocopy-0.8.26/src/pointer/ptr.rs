// Copyright 2023 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

use core::{
    fmt::{Debug, Formatter},
    marker::PhantomData,
};

use crate::{
    pointer::{
        inner::PtrInner,
        invariant::*,
        transmute::{MutationCompatible, SizeEq, TransmuteFromPtr},
    },
    AlignmentError, CastError, CastType, KnownLayout, SizeError, TryFromBytes, ValidityError,
};

/// Module used to gate access to [`Ptr`]'s fields.
mod def {
    #[cfg(doc)]
    use super::super::invariant;
    use super::*;

    /// A raw pointer with more restrictions.
    ///
    /// `Ptr<T>` is similar to [`NonNull<T>`], but it is more restrictive in the
    /// following ways (note that these requirements only hold of non-zero-sized
    /// referents):
    /// - It must derive from a valid allocation.
    /// - It must reference a byte range which is contained inside the
    ///   allocation from which it derives.
    ///   - As a consequence, the byte range it references must have a size
    ///     which does not overflow `isize`.
    ///
    /// Depending on how `Ptr` is parameterized, it may have additional
    /// invariants:
    /// - `ptr` conforms to the aliasing invariant of
    ///   [`I::Aliasing`](invariant::Aliasing).
    /// - `ptr` conforms to the alignment invariant of
    ///   [`I::Alignment`](invariant::Alignment).
    /// - `ptr` conforms to the validity invariant of
    ///   [`I::Validity`](invariant::Validity).
    ///
    /// `Ptr<'a, T>` is [covariant] in `'a` and invariant in `T`.
    ///
    /// [`NonNull<T>`]: core::ptr::NonNull
    /// [covariant]: https://doc.rust-lang.org/reference/subtyping.html
    pub struct Ptr<'a, T, I>
    where
        T: ?Sized,
        I: Invariants,
    {
        /// # Invariants
        ///
        /// 0. `ptr` conforms to the aliasing invariant of
        ///    [`I::Aliasing`](invariant::Aliasing).
        /// 1. `ptr` conforms to the alignment invariant of
        ///    [`I::Alignment`](invariant::Alignment).
        /// 2. `ptr` conforms to the validity invariant of
        ///    [`I::Validity`](invariant::Validity).
        // SAFETY: `PtrInner<'a, T>` is covariant in `'a` and invariant in `T`.
        ptr: PtrInner<'a, T>,
        _invariants: PhantomData<I>,
    }

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants,
    {
        /// Constructs a new `Ptr` from a [`PtrInner`].
        ///
        /// # Safety
        ///
        /// The caller promises that:
        ///
        /// 0. `ptr` conforms to the aliasing invariant of
        ///    [`I::Aliasing`](invariant::Aliasing).
        /// 1. `ptr` conforms to the alignment invariant of
        ///    [`I::Alignment`](invariant::Alignment).
        /// 2. `ptr` conforms to the validity invariant of
        ///    [`I::Validity`](invariant::Validity).
        pub(crate) unsafe fn from_inner(ptr: PtrInner<'a, T>) -> Ptr<'a, T, I> {
            // SAFETY: The caller has promised to satisfy all safety invariants
            // of `Ptr`.
            Self { ptr, _invariants: PhantomData }
        }

        /// Converts this `Ptr<T>` to a [`PtrInner<T>`].
        ///
        /// Note that this method does not consume `self`. The caller should
        /// watch out for `unsafe` code which uses the returned value in a way
        /// that violates the safety invariants of `self`.
        pub(crate) fn as_inner(&self) -> PtrInner<'a, T> {
            self.ptr
        }
    }
}

#[allow(unreachable_pub)] // This is a false positive on our MSRV toolchain.
pub use def::Ptr;

/// External trait implementations on [`Ptr`].
mod _external {
    use super::*;

    /// SAFETY: Shared pointers are safely `Copy`. `Ptr`'s other invariants
    /// (besides aliasing) are unaffected by the number of references that exist
    /// to `Ptr`'s referent. The notable cases are:
    /// - Alignment is a property of the referent type (`T`) and the address,
    ///   both of which are unchanged
    /// - Let `S(T, V)` be the set of bit values permitted to appear in the
    ///   referent of a `Ptr<T, I: Invariants<Validity = V>>`. Since this copy
    ///   does not change `I::Validity` or `T`, `S(T, I::Validity)` is also
    ///   unchanged.
    ///   
    ///   We are required to guarantee that the referents of the original `Ptr`
    ///   and of the copy (which, of course, are actually the same since they
    ///   live in the same byte address range) both remain in the set `S(T,
    ///   I::Validity)`. Since this invariant holds on the original `Ptr`, it
    ///   cannot be violated by the original `Ptr`, and thus the original `Ptr`
    ///   cannot be used to violate this invariant on the copy. The inverse
    ///   holds as well.
    impl<'a, T, I> Copy for Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants<Aliasing = Shared>,
    {
    }

    /// SAFETY: See the safety comment on `Copy`.
    impl<'a, T, I> Clone for Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants<Aliasing = Shared>,
    {
        #[inline]
        fn clone(&self) -> Self {
            *self
        }
    }

    impl<'a, T, I> Debug for Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants,
    {
        #[inline]
        fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
            self.as_inner().as_non_null().fmt(f)
        }
    }
}

/// Methods for converting to and from `Ptr` and Rust's safe reference types.
mod _conversions {
    use super::*;

    /// `&'a T` → `Ptr<'a, T>`
    impl<'a, T> Ptr<'a, T, (Shared, Aligned, Valid)>
    where
        T: 'a + ?Sized,
    {
        /// Constructs a `Ptr` from a shared reference.
        #[doc(hidden)]
        #[inline]
        pub fn from_ref(ptr: &'a T) -> Self {
            let inner = PtrInner::from_ref(ptr);
            // SAFETY:
            // 0. `ptr`, by invariant on `&'a T`, conforms to the aliasing
            //    invariant of `Shared`.
            // 1. `ptr`, by invariant on `&'a T`, conforms to the alignment
            //    invariant of `Aligned`.
            // 2. `ptr`'s referent, by invariant on `&'a T`, is a bit-valid `T`.
            //    This satisfies the requirement that a `Ptr<T, (_, _, Valid)>`
            //    point to a bit-valid `T`. Even if `T` permits interior
            //    mutation, this invariant guarantees that the returned `Ptr`
            //    can only ever be used to modify the referent to store
            //    bit-valid `T`s, which ensures that the returned `Ptr` cannot
            //    be used to violate the soundness of the original `ptr: &'a T`
            //    or of any other references that may exist to the same
            //    referent.
            unsafe { Self::from_inner(inner) }
        }
    }

    /// `&'a mut T` → `Ptr<'a, T>`
    impl<'a, T> Ptr<'a, T, (Exclusive, Aligned, Valid)>
    where
        T: 'a + ?Sized,
    {
        /// Constructs a `Ptr` from an exclusive reference.
        #[inline]
        pub(crate) fn from_mut(ptr: &'a mut T) -> Self {
            let inner = PtrInner::from_mut(ptr);
            // SAFETY:
            // 0. `ptr`, by invariant on `&'a mut T`, conforms to the aliasing
            //    invariant of `Exclusive`.
            // 1. `ptr`, by invariant on `&'a mut T`, conforms to the alignment
            //    invariant of `Aligned`.
            // 2. `ptr`'s referent, by invariant on `&'a mut T`, is a bit-valid
            //    `T`. This satisfies the requirement that a `Ptr<T, (_, _,
            //    Valid)>` point to a bit-valid `T`. This invariant guarantees
            //    that the returned `Ptr` can only ever be used to modify the
            //    referent to store bit-valid `T`s, which ensures that the
            //    returned `Ptr` cannot be used to violate the soundness of the
            //    original `ptr: &'a mut T`.
            unsafe { Self::from_inner(inner) }
        }
    }

    /// `Ptr<'a, T>` → `&'a T`
    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants<Alignment = Aligned, Validity = Valid>,
        I::Aliasing: Reference,
    {
        /// Converts `self` to a shared reference.
        // This consumes `self`, not `&self`, because `self` is, logically, a
        // pointer. For `I::Aliasing = invariant::Shared`, `Self: Copy`, and so
        // this doesn't prevent the caller from still using the pointer after
        // calling `as_ref`.
        #[allow(clippy::wrong_self_convention)]
        pub(crate) fn as_ref(self) -> &'a T {
            let raw = self.as_inner().as_non_null();
            // SAFETY: This invocation of `NonNull::as_ref` satisfies its
            // documented safety preconditions:
            //
            // 1. The pointer is properly aligned. This is ensured by-contract
            //    on `Ptr`, because the `I::Alignment` is `Aligned`.
            //
            // 2. If the pointer's referent is not zero-sized, then the pointer
            //    must be “dereferenceable” in the sense defined in the module
            //    documentation; i.e.:
            //
            //    > The memory range of the given size starting at the pointer
            //    > must all be within the bounds of a single allocated object.
            //    > [2]
            //
            //   This is ensured by contract on all `PtrInner`s.
            //
            // 3. The pointer must point to a validly-initialized instance of
            //    `T`. This is ensured by-contract on `Ptr`, because the
            //    `I::Validity` is `Valid`.
            //
            // 4. You must enforce Rust’s aliasing rules. This is ensured by
            //    contract on `Ptr`, because `I::Aliasing: Reference`. Either it
            //    is `Shared` or `Exclusive`. If it is `Shared`, other
            //    references may not mutate the referent outside of
            //    `UnsafeCell`s.
            //
            // [1]: https://doc.rust-lang.org/std/ptr/struct.NonNull.html#method.as_ref
            // [2]: https://doc.rust-lang.org/std/ptr/index.html#safety
            unsafe { raw.as_ref() }
        }
    }

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants,
        I::Aliasing: Reference,
    {
        /// Reborrows `self`, producing another `Ptr`.
        ///
        /// Since `self` is borrowed immutably, this prevents any mutable
        /// methods from being called on `self` as long as the returned `Ptr`
        /// exists.
        #[doc(hidden)]
        #[inline]
        #[allow(clippy::needless_lifetimes)] // Allows us to name the lifetime in the safety comment below.
        pub fn reborrow<'b>(&'b mut self) -> Ptr<'b, T, I>
        where
            'a: 'b,
        {
            // SAFETY: The following all hold by invariant on `self`, and thus
            // hold of `ptr = self.as_inner()`:
            // 0. SEE BELOW.
            // 1. `ptr` conforms to the alignment invariant of
            //    [`I::Alignment`](invariant::Alignment).
            // 2. `ptr` conforms to the validity invariant of
            //    [`I::Validity`](invariant::Validity). `self` and the returned
            //    `Ptr` permit the same bit values in their referents since they
            //    have the same referent type (`T`) and the same validity
            //    (`I::Validity`). Thus, regardless of what mutation is
            //    permitted (`Exclusive` aliasing or `Shared`-aliased interior
            //    mutation), neither can be used to write a value to the
            //    referent which violates the other's validity invariant.
            //
            // For aliasing (0 above), since `I::Aliasing: Reference`,
            // there are two cases for `I::Aliasing`:
            // - For `invariant::Shared`: `'a` outlives `'b`, and so the
            //   returned `Ptr` does not permit accessing the referent any
            //   longer than is possible via `self`. For shared aliasing, it is
            //   sound for multiple `Ptr`s to exist simultaneously which
            //   reference the same memory, so creating a new one is not
            //   problematic.
            // - For `invariant::Exclusive`: Since `self` is `&'b mut` and we
            //   return a `Ptr` with lifetime `'b`, `self` is inaccessible to
            //   the caller for the lifetime `'b` - in other words, `self` is
            //   inaccessible to the caller as long as the returned `Ptr`
            //   exists. Since `self` is an exclusive `Ptr`, no other live
            //   references or `Ptr`s may exist which refer to the same memory
            //   while `self` is live. Thus, as long as the returned `Ptr`
            //   exists, no other references or `Ptr`s which refer to the same
            //   memory may be live.
            unsafe { Ptr::from_inner(self.as_inner()) }
        }
    }

    /// `Ptr<'a, T>` → `&'a mut T`
    impl<'a, T> Ptr<'a, T, (Exclusive, Aligned, Valid)>
    where
        T: 'a + ?Sized,
    {
        /// Converts `self` to a mutable reference.
        #[allow(clippy::wrong_self_convention)]
        pub(crate) fn as_mut(self) -> &'a mut T {
            let mut raw = self.as_inner().as_non_null();
            // SAFETY: This invocation of `NonNull::as_mut` satisfies its
            // documented safety preconditions:
            //
            // 1. The pointer is properly aligned. This is ensured by-contract
            //    on `Ptr`, because the `ALIGNMENT_INVARIANT` is `Aligned`.
            //
            // 2. If the pointer's referent is not zero-sized, then the pointer
            //    must be “dereferenceable” in the sense defined in the module
            //    documentation; i.e.:
            //
            //    > The memory range of the given size starting at the pointer
            //    > must all be within the bounds of a single allocated object.
            //    > [2]
            //
            //   This is ensured by contract on all `PtrInner`s.
            //
            // 3. The pointer must point to a validly-initialized instance of
            //    `T`. This is ensured by-contract on `Ptr`, because the
            //    validity invariant is `Valid`.
            //
            // 4. You must enforce Rust’s aliasing rules. This is ensured by
            //    contract on `Ptr`, because the `ALIASING_INVARIANT` is
            //    `Exclusive`.
            //
            // [1]: https://doc.rust-lang.org/std/ptr/struct.NonNull.html#method.as_mut
            // [2]: https://doc.rust-lang.org/std/ptr/index.html#safety
            unsafe { raw.as_mut() }
        }
    }

    /// `Ptr<'a, T>` → `Ptr<'a, U>`
    impl<'a, T: ?Sized, I> Ptr<'a, T, I>
    where
        I: Invariants,
    {
        pub(crate) fn transmute<U, V, R>(self) -> Ptr<'a, U, (I::Aliasing, Unaligned, V)>
        where
            V: Validity,
            U: TransmuteFromPtr<T, I::Aliasing, I::Validity, V, R> + SizeEq<T> + ?Sized,
        {
            // SAFETY:
            // - `SizeEq::cast_from_raw` promises to preserve address,
            //   provenance, and the number of bytes in the referent
            // - If aliasing is `Shared`, then by `U: TransmuteFromPtr<T>`, at
            //   least one of the following holds:
            //   - `T: Immutable` and `U: Immutable`, in which case it is
            //     trivially sound for shared code to operate on a `&T` and `&U`
            //     at the same time, as neither can perform interior mutation
            //   - It is directly guaranteed that it is sound for shared code to
            //     operate on these references simultaneously
            // - By `U: TransmuteFromPtr<T, I::Aliasing, I::Validity, V>`, it is
            //   sound to perform this transmute.
            unsafe { self.transmute_unchecked(SizeEq::cast_from_raw) }
        }

        #[doc(hidden)]
        #[inline(always)]
        #[must_use]
        pub fn recall_validity<V, R>(self) -> Ptr<'a, T, (I::Aliasing, I::Alignment, V)>
        where
            V: Validity,
            T: TransmuteFromPtr<T, I::Aliasing, I::Validity, V, R>,
        {
            // SAFETY:
            // - This cast is a no-op, and so trivially preserves address,
            //   referent size, and provenance
            // - It is trivially sound to have multiple `&T` referencing the same
            //   referent simultaneously
            // - By `T: TransmuteFromPtr<T, I::Aliasing, I::Validity, V>`, it is
            //   sound to perform this transmute.
            let ptr = unsafe { self.transmute_unchecked(SizeEq::cast_from_raw) };
            // SAFETY: `self` and `ptr` have the same address and referent type.
            // Therefore, if `self` satisfies `I::Alignment`, then so does
            // `ptr`.
            unsafe { ptr.assume_alignment::<I::Alignment>() }
        }

        /// Casts to a different (unsized) target type without checking interior
        /// mutability.
        ///
        /// Callers should prefer [`cast_unsized`] where possible.
        ///
        /// [`cast_unsized`]: Ptr::cast_unsized
        ///
        /// # Safety
        ///
        /// The caller promises that `u = cast(p)` is a pointer cast with the
        /// following properties:
        /// - `u` addresses a subset of the bytes addressed by `p`
        /// - `u` has the same provenance as `p`
        /// - If `I::Aliasing` is [`Shared`], it must not be possible for safe
        ///   code, operating on a `&T` and `&U` with the same referent
        ///   simultaneously, to cause undefined behavior
        /// - It is sound to transmute a pointer of type `T` with aliasing
        ///   `I::Aliasing` and validity `I::Validity` to a pointer of type `U`
        ///   with aliasing `I::Aliasing` and validity `V`. This is a subtle
        ///   soundness requirement that is a function of `T`, `U`,
        ///   `I::Aliasing`, `I::Validity`, and `V`, and may depend upon the
        ///   presence, absence, or specific location of `UnsafeCell`s in `T`
        ///   and/or `U`. See [`Validity`] for more details.
        #[doc(hidden)]
        #[inline]
        pub unsafe fn transmute_unchecked<U: ?Sized, V, F>(
            self,
            cast: F,
        ) -> Ptr<'a, U, (I::Aliasing, Unaligned, V)>
        where
            V: Validity,
            F: FnOnce(PtrInner<'a, T>) -> PtrInner<'a, U>,
        {
            let ptr = cast(self.as_inner());

            // SAFETY:
            //
            // The following safety arguments rely on the fact that the caller
            // promises that `cast` returns a `PtrInner` which addresses a
            // prefix of the bytes of `*self`, and so properties that hold of
            // `*self` also hold of `*ptr`.
            //
            // 0. `ptr` conforms to the aliasing invariant of `I::Aliasing`:
            //    - `Exclusive`: `self` is the only `Ptr` or reference which is
            //      permitted to read or modify the referent for the lifetime
            //      `'a`. Since we consume `self` by value, the returned pointer
            //      remains the only `Ptr` or reference which is permitted to
            //      read or modify the referent for the lifetime `'a`.
            //    - `Shared`: Since `self` has aliasing `Shared`, we know that
            //      no other code may mutate the referent during the lifetime
            //      `'a`, except via `UnsafeCell`s, and except as permitted by
            //      `T`'s library safety invariants. The caller promises that
            //      any safe operations which can be permitted on a `&T` and a
            //      `&U` simultaneously must be sound. Thus, no operations on a
            //      `&U` could violate `&T`'s library safety invariants, and
            //      vice-versa. Since any mutation via shared references outside
            //      of `UnsafeCell`s is unsound, this must be impossible using
            //      `&T` and `&U`.
            //    - `Inaccessible`: There are no restrictions we need to uphold.
            // 1. `ptr` trivially satisfies the alignment invariant `Unaligned`.
            // 2. The caller promises that `ptr` conforms to the validity
            //    invariant `V` with respect to its referent type, `U`.
            unsafe { Ptr::from_inner(ptr) }
        }
    }

    /// `Ptr<'a, T, (_, _, _)>` → `Ptr<'a, Unalign<T>, (_, Aligned, _)>`
    impl<'a, T, I> Ptr<'a, T, I>
    where
        I: Invariants,
    {
        /// Converts a `Ptr` an unaligned `T` into a `Ptr` to an aligned
        /// `Unalign<T>`.
        pub(crate) fn into_unalign(
            self,
        ) -> Ptr<'a, crate::Unalign<T>, (I::Aliasing, Aligned, I::Validity)> {
            // SAFETY:
            // - This cast preserves provenance.
            // - This cast preserves address. `Unalign<T>` promises to have the
            //   same size as `T`, and so the cast returns a pointer addressing
            //   the same byte range as `p`.
            // - By the same argument, the returned pointer refers to
            //   `UnsafeCell`s at the same locations as `p`.
            // - `Unalign<T>` promises to have the same bit validity as `T`. By
            //   invariant on `Validity`, the set of bit patterns allowed in the
            //   referent of a `Ptr<X, (_, _, V)>` is only a function of the
            //   validity of `X` and of `V`. Thus, the set of bit patterns
            //   allowed in the referent of a `Ptr<T, (_, _, I::Validity)>` is
            //   the same as the set of bit patterns allowed in the referent of
            //   a `Ptr<Unalign<T>, (_, _, I::Validity)>`. As a result, `self`
            //   and the returned `Ptr` permit the same set of bit patterns in
            //   their referents, and so neither can be used to violate the
            //   validity of the other.
            let ptr = unsafe { self.transmute_unchecked(PtrInner::cast_sized) };
            ptr.bikeshed_recall_aligned()
        }
    }

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: ?Sized,
        I: Invariants<Validity = Valid>,
        I::Aliasing: Reference,
    {
        /// Reads the referent.
        #[must_use]
        #[inline]
        pub fn read_unaligned<R>(self) -> T
        where
            T: Copy,
            T: Read<I::Aliasing, R>,
        {
            (*self.into_unalign().as_ref()).into_inner()
        }

        /// Views the value as an aligned reference.
        ///
        /// This is only available if `T` is [`Unaligned`].
        #[must_use]
        #[inline]
        pub fn unaligned_as_ref(self) -> &'a T
        where
            T: crate::Unaligned,
        {
            self.bikeshed_recall_aligned().as_ref()
        }
    }
}

/// State transitions between invariants.
mod _transitions {
    use super::*;
    use crate::pointer::transmute::TryTransmuteFromPtr;

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants,
    {
        /// Returns a `Ptr` with [`Exclusive`] aliasing if `self` already has
        /// `Exclusive` aliasing, or generates a compile-time assertion failure.
        ///
        /// This allows code which is generic over aliasing to down-cast to a
        /// concrete aliasing.
        ///
        /// [`Exclusive`]: crate::pointer::invariant::Exclusive
        #[inline]
        pub(crate) fn into_exclusive_or_pme(
            self,
        ) -> Ptr<'a, T, (Exclusive, I::Alignment, I::Validity)> {
            // NOTE(https://github.com/rust-lang/rust/issues/131625): We do this
            // rather than just having `Aliasing::IS_EXCLUSIVE` have the panic
            // behavior because doing it that way causes rustdoc to fail while
            // attempting to document hidden items (since it evaluates the
            // constant - and thus panics).
            trait AliasingExt: Aliasing {
                const IS_EXCL: bool;
            }

            impl<A: Aliasing> AliasingExt for A {
                const IS_EXCL: bool = {
                    const_assert!(Self::IS_EXCLUSIVE);
                    true
                };
            }

            assert!(I::Aliasing::IS_EXCL);

            // SAFETY: We've confirmed that `self` already has the aliasing
            // `Exclusive`. If it didn't, either the preceding assert would fail
            // or evaluating `I::Aliasing::IS_EXCL` would fail. We're *pretty*
            // sure that it's guaranteed to fail const eval, but the `assert!`
            // provides a backstop in case that doesn't work.
            unsafe { self.assume_exclusive() }
        }

        /// Assumes that `self` satisfies the invariants `H`.
        ///
        /// # Safety
        ///
        /// The caller promises that `self` satisfies the invariants `H`.
        unsafe fn assume_invariants<H: Invariants>(self) -> Ptr<'a, T, H> {
            // SAFETY: The caller has promised to satisfy all parameterized
            // invariants of `Ptr`. `Ptr`'s other invariants are satisfied
            // by-contract by the source `Ptr`.
            unsafe { Ptr::from_inner(self.as_inner()) }
        }

        /// Helps the type system unify two distinct invariant types which are
        /// actually the same.
        pub(crate) fn unify_invariants<
            H: Invariants<Aliasing = I::Aliasing, Alignment = I::Alignment, Validity = I::Validity>,
        >(
            self,
        ) -> Ptr<'a, T, H> {
            // SAFETY: The associated type bounds on `H` ensure that the
            // invariants are unchanged.
            unsafe { self.assume_invariants::<H>() }
        }

        /// Assumes that `self` satisfies the aliasing requirement of `A`.
        ///
        /// # Safety
        ///
        /// The caller promises that `self` satisfies the aliasing requirement
        /// of `A`.
        #[inline]
        pub(crate) unsafe fn assume_aliasing<A: Aliasing>(
            self,
        ) -> Ptr<'a, T, (A, I::Alignment, I::Validity)> {
            // SAFETY: The caller promises that `self` satisfies the aliasing
            // requirements of `A`.
            unsafe { self.assume_invariants() }
        }

        /// Assumes `self` satisfies the aliasing requirement of [`Exclusive`].
        ///
        /// # Safety
        ///
        /// The caller promises that `self` satisfies the aliasing requirement
        /// of `Exclusive`.
        ///
        /// [`Exclusive`]: crate::pointer::invariant::Exclusive
        #[inline]
        pub(crate) unsafe fn assume_exclusive(
            self,
        ) -> Ptr<'a, T, (Exclusive, I::Alignment, I::Validity)> {
            // SAFETY: The caller promises that `self` satisfies the aliasing
            // requirements of `Exclusive`.
            unsafe { self.assume_aliasing::<Exclusive>() }
        }

        /// Assumes that `self`'s referent is validly-aligned for `T` if
        /// required by `A`.
        ///
        /// # Safety
        ///
        /// The caller promises that `self`'s referent conforms to the alignment
        /// invariant of `T` if required by `A`.
        #[inline]
        pub(crate) unsafe fn assume_alignment<A: Alignment>(
            self,
        ) -> Ptr<'a, T, (I::Aliasing, A, I::Validity)> {
            // SAFETY: The caller promises that `self`'s referent is
            // well-aligned for `T` if required by `A` .
            unsafe { self.assume_invariants() }
        }

        /// Checks the `self`'s alignment at runtime, returning an aligned `Ptr`
        /// on success.
        pub(crate) fn try_into_aligned(
            self,
        ) -> Result<Ptr<'a, T, (I::Aliasing, Aligned, I::Validity)>, AlignmentError<Self, T>>
        where
            T: Sized,
        {
            if let Err(err) =
                crate::util::validate_aligned_to::<_, T>(self.as_inner().as_non_null())
            {
                return Err(err.with_src(self));
            }

            // SAFETY: We just checked the alignment.
            Ok(unsafe { self.assume_alignment::<Aligned>() })
        }

        /// Recalls that `self`'s referent is validly-aligned for `T`.
        #[inline]
        // FIXME(#859): Reconsider the name of this method before making it
        // public.
        pub(crate) fn bikeshed_recall_aligned(
            self,
        ) -> Ptr<'a, T, (I::Aliasing, Aligned, I::Validity)>
        where
            T: crate::Unaligned,
        {
            // SAFETY: The bound `T: Unaligned` ensures that `T` has no
            // non-trivial alignment requirement.
            unsafe { self.assume_alignment::<Aligned>() }
        }

        /// Assumes that `self`'s referent conforms to the validity requirement
        /// of `V`.
        ///
        /// # Safety
        ///
        /// The caller promises that `self`'s referent conforms to the validity
        /// requirement of `V`.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub unsafe fn assume_validity<V: Validity>(
            self,
        ) -> Ptr<'a, T, (I::Aliasing, I::Alignment, V)> {
            // SAFETY: The caller promises that `self`'s referent conforms to
            // the validity requirement of `V`.
            unsafe { self.assume_invariants() }
        }

        /// A shorthand for `self.assume_validity<invariant::Initialized>()`.
        ///
        /// # Safety
        ///
        /// The caller promises to uphold the safety preconditions of
        /// `self.assume_validity<invariant::Initialized>()`.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub unsafe fn assume_initialized(
            self,
        ) -> Ptr<'a, T, (I::Aliasing, I::Alignment, Initialized)> {
            // SAFETY: The caller has promised to uphold the safety
            // preconditions.
            unsafe { self.assume_validity::<Initialized>() }
        }

        /// A shorthand for `self.assume_validity<Valid>()`.
        ///
        /// # Safety
        ///
        /// The caller promises to uphold the safety preconditions of
        /// `self.assume_validity<Valid>()`.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub unsafe fn assume_valid(self) -> Ptr<'a, T, (I::Aliasing, I::Alignment, Valid)> {
            // SAFETY: The caller has promised to uphold the safety
            // preconditions.
            unsafe { self.assume_validity::<Valid>() }
        }

        /// Recalls that `self`'s referent is initialized.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        // FIXME(#859): Reconsider the name of this method before making it
        // public.
        pub fn bikeshed_recall_initialized_from_bytes(
            self,
        ) -> Ptr<'a, T, (I::Aliasing, I::Alignment, Initialized)>
        where
            T: crate::IntoBytes + crate::FromBytes,
            I: Invariants<Validity = Valid>,
        {
            // SAFETY: The `T: IntoBytes + FromBytes` bound ensures that `T`'s
            // bit validity is equivalent to `[u8]`. In other words, the set of
            // allowed referents for a `Ptr<T, (_, _, Valid)>` is the set of
            // initialized bit patterns. The same is true of the set of allowed
            // referents for any `Ptr<_, (_, _, Initialized)>`. Thus, this call
            // does not change the set of allowed values in the referent.
            unsafe { self.assume_initialized() }
        }

        /// Recalls that `self`'s referent is initialized.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        // FIXME(#859): Reconsider the name of this method before making it
        // public.
        pub fn bikeshed_recall_initialized_immutable(
            self,
        ) -> Ptr<'a, T, (Shared, I::Alignment, Initialized)>
        where
            T: crate::IntoBytes + crate::Immutable,
            I: Invariants<Aliasing = Shared, Validity = Valid>,
        {
            // SAFETY: Let `O` (for "old") be the set of allowed bit patterns in
            // `self`'s referent, and let `N` (for "new") be the set of allowed
            // bit patterns in the referent of the returned `Ptr`. `T:
            // IntoBytes` and `I: Invariants<Validity = Valid>` ensures that `O`
            // cannot contain any uninitialized bit patterns. Since the returned
            // `Ptr` has validity `Initialized`, `N` is equal to the set of all
            // initialized bit patterns. Thus, `O` is a subset of `N`, and so
            // the returned `Ptr`'s validity invariant is upheld.
            //
            // Since `T: Immutable` and aliasing is `Shared`, the returned `Ptr`
            // cannot be used to modify the referent. Before this call, `self`'s
            // referent is guaranteed by invariant on `Ptr` to satisfy `self`'s
            // validity invariant. Since the returned `Ptr` cannot be used to
            // modify the referent, this guarantee cannot be violated by the
            // returned `Ptr` (even if `O` is a strict subset of `N`).
            unsafe { self.assume_initialized() }
        }

        /// Checks that `self`'s referent is validly initialized for `T`,
        /// returning a `Ptr` with `Valid` on success.
        ///
        /// # Panics
        ///
        /// This method will panic if
        /// [`T::is_bit_valid`][TryFromBytes::is_bit_valid] panics.
        ///
        /// # Safety
        ///
        /// On error, unsafe code may rely on this method's returned
        /// `ValidityError` containing `self`.
        #[inline]
        pub(crate) fn try_into_valid<R, S>(
            mut self,
        ) -> Result<Ptr<'a, T, (I::Aliasing, I::Alignment, Valid)>, ValidityError<Self, T>>
        where
            T: TryFromBytes
                + Read<I::Aliasing, R>
                + TryTransmuteFromPtr<T, I::Aliasing, I::Validity, Valid, S>,
            I::Aliasing: Reference,
            I: Invariants<Validity = Initialized>,
        {
            // This call may panic. If that happens, it doesn't cause any soundness
            // issues, as we have not generated any invalid state which we need to
            // fix before returning.
            if T::is_bit_valid(self.reborrow().forget_aligned()) {
                // SAFETY: If `T::is_bit_valid`, code may assume that `self`
                // contains a bit-valid instance of `T`. By `T:
                // TryTransmuteFromPtr<T, I::Aliasing, I::Validity, Valid>`, so
                // long as `self`'s referent conforms to the `Valid` validity
                // for `T` (which we just confired), then this transmute is
                // sound.
                Ok(unsafe { self.assume_valid() })
            } else {
                Err(ValidityError::new(self))
            }
        }

        /// Forgets that `self`'s referent is validly-aligned for `T`.
        #[doc(hidden)]
        #[must_use]
        #[inline]
        pub fn forget_aligned(self) -> Ptr<'a, T, (I::Aliasing, Unaligned, I::Validity)> {
            // SAFETY: `Unaligned` is less restrictive than `Aligned`.
            unsafe { self.assume_invariants() }
        }
    }
}

/// Casts of the referent type.
mod _casts {
    use super::*;

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized,
        I: Invariants,
    {
        /// Casts to a different (unsized) target type without checking interior
        /// mutability.
        ///
        /// Callers should prefer [`cast_unsized`] where possible.
        ///
        /// [`cast_unsized`]: Ptr::cast_unsized
        ///
        /// # Safety
        ///
        /// The caller promises that `u = cast(p)` is a pointer cast with the
        /// following properties:
        /// - `u` addresses a subset of the bytes addressed by `p`
        /// - `u` has the same provenance as `p`
        /// - If `I::Aliasing` is [`Shared`], it must not be possible for safe
        ///   code, operating on a `&T` and `&U` with the same referent
        ///   simultaneously, to cause undefined behavior
        ///
        /// `cast_unsized_unchecked` guarantees that the pointer passed to
        /// `cast` will reference a byte sequence which is either contained
        /// inside a single allocated object or is zero sized. In either case,
        /// this means that its size will fit in an `isize` and it will not wrap
        /// around the address space.
        #[doc(hidden)]
        #[inline]
        pub unsafe fn cast_unsized_unchecked<U, F: FnOnce(PtrInner<'a, T>) -> PtrInner<'a, U>>(
            self,
            cast: F,
        ) -> Ptr<'a, U, (I::Aliasing, Unaligned, I::Validity)>
        where
            U: 'a + CastableFrom<T, I::Validity, I::Validity> + ?Sized,
        {
            // SAFETY:
            // - The caller promises that `u = cast(p)` is a pointer which
            //   satisfies:
            //   - `u` addresses a subset of the bytes addressed by `p`
            //   - `u` has the same provenance as `p`
            //   - If `I::Aliasing` is [`Shared`], it must not be possible for
            //     safe code, operating on a `&T` and `&U` with the same
            //     referent simultaneously, to cause undefined behavior
            // - By `U: CastableFrom<T, I::Validity, I::Validity>`,
            //   `I::Validity` is either `Uninit` or `Initialized`. In both
            //   cases, the bit validity `I::Validity` has the same semantics
            //   regardless of referent type. In other words, the set of allowed
            //   referent values for `Ptr<T, (_, _, I::Validity)>` and `Ptr<U,
            //   (_, _, I::Validity)>` are identical. As a consequence, neither
            //   `self` nor the returned `Ptr` can be used to write values which
            //   are invalid for the other.
            //
            // `transmute_unchecked` guarantees that it will only pass pointers
            // to `cast` which either reference a zero-sized byte range or
            // reference a byte range which is entirely contained inside of an
            // allocated object.
            unsafe { self.transmute_unchecked(cast) }
        }

        /// Casts to a different (unsized) target type.
        ///
        /// # Safety
        ///
        /// The caller promises that `u = cast(p)` is a pointer cast with the
        /// following properties:
        /// - `u` addresses a subset of the bytes addressed by `p`
        /// - `u` has the same provenance as `p`
        #[doc(hidden)]
        #[inline]
        pub unsafe fn cast_unsized<U, F, R>(
            self,
            cast: F,
        ) -> Ptr<'a, U, (I::Aliasing, Unaligned, I::Validity)>
        where
            T: MutationCompatible<U, I::Aliasing, I::Validity, I::Validity, R>,
            U: 'a + ?Sized + CastableFrom<T, I::Validity, I::Validity>,
            F: FnOnce(PtrInner<'a, T>) -> PtrInner<'a, U>,
        {
            // SAFETY: Because `T: MutationCompatible<U, I::Aliasing, R>`, one
            // of the following holds:
            // - `T: Read<I::Aliasing>` and `U: Read<I::Aliasing>`, in which
            //   case one of the following holds:
            //   - `I::Aliasing` is `Exclusive`
            //   - `T` and `U` are both `Immutable`
            // - It is sound for safe code to operate on `&T` and `&U` with the
            //   same referent simultaneously
            //
            // The caller promises all other safety preconditions.
            unsafe { self.cast_unsized_unchecked(cast) }
        }
    }

    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + KnownLayout + ?Sized,
        I: Invariants<Validity = Initialized>,
    {
        /// Casts this pointer-to-initialized into a pointer-to-bytes.
        #[allow(clippy::wrong_self_convention)]
        #[must_use]
        #[inline]
        pub fn as_bytes<R>(self) -> Ptr<'a, [u8], (I::Aliasing, Aligned, Valid)>
        where
            T: Read<I::Aliasing, R>,
            I::Aliasing: Reference,
        {
            // SAFETY: `PtrInner::as_bytes` returns a pointer which addresses
            // the same byte range as its argument, and which has the same
            // provenance.
            let ptr = unsafe { self.cast_unsized(PtrInner::as_bytes) };
            ptr.bikeshed_recall_aligned().recall_validity::<Valid, (_, (_, _))>()
        }
    }

    impl<'a, T, I, const N: usize> Ptr<'a, [T; N], I>
    where
        T: 'a,
        I: Invariants,
    {
        /// Casts this pointer-to-array into a slice.
        #[allow(clippy::wrong_self_convention)]
        pub(crate) fn as_slice(self) -> Ptr<'a, [T], I> {
            let slice = self.as_inner().as_slice();
            // SAFETY: Note that, by post-condition on `PtrInner::as_slice`,
            // `slice` refers to the same byte range as `self.as_inner()`.
            //
            // 0. Thus, `slice` conforms to the aliasing invariant of
            //    `I::Aliasing` because `self` does.
            // 1. By the above lemma, `slice` conforms to the alignment
            //    invariant of `I::Alignment` because `self` does.
            // 2. Since `[T; N]` and `[T]` have the same bit validity [1][2],
            //    and since `self` and the returned `Ptr` have the same validity
            //    invariant, neither `self` nor the returned `Ptr` can be used
            //    to write a value to the referent which violates the other's
            //    validity invariant.
            //
            // [1] Per https://doc.rust-lang.org/1.81.0/reference/type-layout.html#array-layout:
            //
            //   An array of `[T; N]` has a size of `size_of::<T>() * N` and the
            //   same alignment of `T`. Arrays are laid out so that the
            //   zero-based `nth` element of the array is offset from the start
            //   of the array by `n * size_of::<T>()` bytes.
            //
            //   ...
            //
            //   Slices have the same layout as the section of the array they
            //   slice.
            //
            // [2] Per https://doc.rust-lang.org/1.81.0/reference/types/array.html#array-types:
            //
            //   All elements of arrays are always initialized
            unsafe { Ptr::from_inner(slice) }
        }
    }

    /// For caller convenience, these methods are generic over alignment
    /// invariant. In practice, the referent is always well-aligned, because the
    /// alignment of `[u8]` is 1.
    impl<'a, I> Ptr<'a, [u8], I>
    where
        I: Invariants<Validity = Valid>,
    {
        /// Attempts to cast `self` to a `U` using the given cast type.
        ///
        /// If `U` is a slice DST and pointer metadata (`meta`) is provided,
        /// then the cast will only succeed if it would produce an object with
        /// the given metadata.
        ///
        /// Returns `None` if the resulting `U` would be invalidly-aligned, if
        /// no `U` can fit in `self`, or if the provided pointer metadata
        /// describes an invalid instance of `U`. On success, returns a pointer
        /// to the largest-possible `U` which fits in `self`.
        ///
        /// # Safety
        ///
        /// The caller may assume that this implementation is correct, and may
        /// rely on that assumption for the soundness of their code. In
        /// particular, the caller may assume that, if `try_cast_into` returns
        /// `Some((ptr, remainder))`, then `ptr` and `remainder` refer to
        /// non-overlapping byte ranges within `self`, and that `ptr` and
        /// `remainder` entirely cover `self`. Finally:
        /// - If this is a prefix cast, `ptr` has the same address as `self`.
        /// - If this is a suffix cast, `remainder` has the same address as
        ///   `self`.
        #[inline(always)]
        pub(crate) fn try_cast_into<U, R>(
            self,
            cast_type: CastType,
            meta: Option<U::PointerMetadata>,
        ) -> Result<
            (Ptr<'a, U, (I::Aliasing, Aligned, Initialized)>, Ptr<'a, [u8], I>),
            CastError<Self, U>,
        >
        where
            I::Aliasing: Reference,
            U: 'a + ?Sized + KnownLayout + Read<I::Aliasing, R>,
        {
            let (inner, remainder) =
                self.as_inner().try_cast_into(cast_type, meta).map_err(|err| {
                    err.map_src(|inner|
                    // SAFETY: `PtrInner::try_cast_into` promises to return its
                    // original argument on error, which was originally produced
                    // by `self.as_inner()`, which is guaranteed to satisfy
                    // `Ptr`'s invariants.
                    unsafe { Ptr::from_inner(inner) })
                })?;

            // SAFETY:
            // 0. Since `U: Read<I::Aliasing, _>`, either:
            //    - `I::Aliasing` is `Exclusive`, in which case both `src` and
            //      `ptr` conform to `Exclusive`
            //    - `I::Aliasing` is `Shared` and `U` is `Immutable` (we already
            //      know that `[u8]: Immutable`). In this case, neither `U` nor
            //      `[u8]` permit mutation, and so `Shared` aliasing is
            //      satisfied.
            // 1. `ptr` conforms to the alignment invariant of `Aligned` because
            //    it is derived from `try_cast_into`, which promises that the
            //    object described by `target` is validly aligned for `U`.
            // 2. By trait bound, `self` - and thus `target` - is a bit-valid
            //    `[u8]`. `Ptr<[u8], (_, _, Valid)>` and `Ptr<_, (_, _,
            //    Initialized)>` have the same bit validity, and so neither
            //    `self` nor `res` can be used to write a value to the referent
            //    which violates the other's validity invariant.
            let res = unsafe { Ptr::from_inner(inner) };

            // SAFETY:
            // 0. `self` and `remainder` both have the type `[u8]`. Thus, they
            //    have `UnsafeCell`s at the same locations. Type casting does
            //    not affect aliasing.
            // 1. `[u8]` has no alignment requirement.
            // 2. `self` has validity `Valid` and has type `[u8]`. Since
            //    `remainder` references a subset of `self`'s referent, it is
            //    also a bit-valid `[u8]`. Thus, neither `self` nor `remainder`
            //    can be used to write a value to the referent which violates
            //    the other's validity invariant.
            let remainder = unsafe { Ptr::from_inner(remainder) };

            Ok((res, remainder))
        }

        /// Attempts to cast `self` into a `U`, failing if all of the bytes of
        /// `self` cannot be treated as a `U`.
        ///
        /// In particular, this method fails if `self` is not validly-aligned
        /// for `U` or if `self`'s size is not a valid size for `U`.
        ///
        /// # Safety
        ///
        /// On success, the caller may assume that the returned pointer
        /// references the same byte range as `self`.
        #[allow(unused)]
        #[inline(always)]
        pub(crate) fn try_cast_into_no_leftover<U, R>(
            self,
            meta: Option<U::PointerMetadata>,
        ) -> Result<Ptr<'a, U, (I::Aliasing, Aligned, Initialized)>, CastError<Self, U>>
        where
            I::Aliasing: Reference,
            U: 'a + ?Sized + KnownLayout + Read<I::Aliasing, R>,
        {
            // FIXME(#67): Remove this allow. See NonNulSlicelExt for more
            // details.
            #[allow(unstable_name_collisions)]
            match self.try_cast_into(CastType::Prefix, meta) {
                Ok((slf, remainder)) => {
                    if remainder.len() == 0 {
                        Ok(slf)
                    } else {
                        // Undo the cast so we can return the original bytes.
                        let slf = slf.as_bytes();
                        // Restore the initial alignment invariant of `self`.
                        //
                        // SAFETY: The referent type of `slf` is now equal to
                        // that of `self`, but the alignment invariants
                        // nominally differ. Since `slf` and `self` refer to the
                        // same memory and no actions have been taken that would
                        // violate the original invariants on `self`, it is
                        // sound to apply the alignment invariant of `self` onto
                        // `slf`.
                        let slf = unsafe { slf.assume_alignment::<I::Alignment>() };
                        let slf = slf.unify_invariants();
                        Err(CastError::Size(SizeError::<_, U>::new(slf)))
                    }
                }
                Err(err) => Err(err),
            }
        }
    }

    impl<'a, T, I> Ptr<'a, core::cell::UnsafeCell<T>, I>
    where
        T: 'a + ?Sized,
        I: Invariants<Aliasing = Exclusive>,
    {
        /// Converts this `Ptr` into a pointer to the underlying data.
        ///
        /// This call borrows the `UnsafeCell` mutably (at compile-time) which
        /// guarantees that we possess the only reference.
        ///
        /// This is like [`UnsafeCell::get_mut`], but for `Ptr`.
        ///
        /// [`UnsafeCell::get_mut`]: core::cell::UnsafeCell::get_mut
        #[must_use]
        #[inline(always)]
        pub fn get_mut(self) -> Ptr<'a, T, I> {
            // SAFETY:
            // - The closure uses an `as` cast, which preserves address range
            //   and provenance.
            // - Aliasing is `Exclusive`, and so we are not required to promise
            //   anything about the locations of `UnsafeCell`s.
            // - `UnsafeCell<T>` has the same bit validity as `T` [1].
            //   Technically the term "representation" doesn't guarantee this,
            //   but the subsequent sentence in the documentation makes it clear
            //   that this is the intention.
            //
            //   By invariant on `Validity`, since `T` and `UnsafeCell<T>` have
            //   the same bit validity, then the set of values which may appear
            //   in the referent of a `Ptr<T, (_, _, V)>` is the same as the set
            //   which may appear in the referent of a `Ptr<UnsafeCell<T>, (_,
            //   _, V)>`. Thus, neither `self` nor `ptr` may be used to write a
            //   value to the referent which would violate the other's validity
            //   invariant.
            //
            // [1] Per https://doc.rust-lang.org/1.81.0/core/cell/struct.UnsafeCell.html#memory-layout:
            //
            //   `UnsafeCell<T>` has the same in-memory representation as its
            //   inner type `T`. A consequence of this guarantee is that it is
            //   possible to convert between `T` and `UnsafeCell<T>`.
            #[allow(clippy::as_conversions)]
            let ptr = unsafe { self.transmute_unchecked(|ptr| cast!(ptr)) };

            // SAFETY: `UnsafeCell<T>` has the same alignment as `T` [1],
            // and so if `self` is guaranteed to be aligned, then so is the
            // returned `Ptr`.
            //
            // [1] Per https://doc.rust-lang.org/1.81.0/core/cell/struct.UnsafeCell.html#memory-layout:
            //
            //   `UnsafeCell<T>` has the same in-memory representation as
            //   its inner type `T`. A consequence of this guarantee is that
            //   it is possible to convert between `T` and `UnsafeCell<T>`.
            let ptr = unsafe { ptr.assume_alignment::<I::Alignment>() };
            ptr.unify_invariants()
        }
    }
}

/// Projections through the referent.
mod _project {
    use super::*;

    impl<'a, T, I> Ptr<'a, [T], I>
    where
        T: 'a,
        I: Invariants,
        I::Aliasing: Reference,
    {
        /// Iteratively projects the elements `Ptr<T>` from `Ptr<[T]>`.
        pub(crate) fn iter(&self) -> impl Iterator<Item = Ptr<'a, T, I>> {
            // SAFETY:
            // 0. `elem` conforms to the aliasing invariant of `I::Aliasing`
            //    because projection does not impact the aliasing invariant.
            // 1. `elem`, conditionally, conforms to the validity invariant of
            //    `I::Alignment`. If `elem` is projected from data well-aligned
            //    for `[T]`, `elem` will be valid for `T`.
            // 2. FIXME: Need to cite facts about `[T]`'s layout (same for the
            //    preceding points)
            self.as_inner().iter().map(|elem| unsafe { Ptr::from_inner(elem) })
        }
    }

    #[allow(clippy::needless_lifetimes)]
    impl<'a, T, I> Ptr<'a, T, I>
    where
        T: 'a + ?Sized + KnownLayout<PointerMetadata = usize>,
        I: Invariants,
    {
        /// The number of slice elements in the object referenced by `self`.
        pub(crate) fn len(&self) -> usize {
            self.as_inner().meta().get()
        }
    }
}

#[cfg(test)]
mod tests {
    use core::mem::{self, MaybeUninit};

    use super::*;
    #[allow(unused)] // Needed on our MSRV, but considered unused on later toolchains.
    use crate::util::AsAddress;
    use crate::{pointer::BecauseImmutable, util::testutil::AU64, FromBytes, Immutable};

    mod test_ptr_try_cast_into_soundness {
        use super::*;

        // This test is designed so that if `Ptr::try_cast_into_xxx` are
        // buggy, it will manifest as unsoundness that Miri can detect.

        // - If `size_of::<T>() == 0`, `N == 4`
        // - Else, `N == 4 * size_of::<T>()`
        //
        // Each test will be run for each metadata in `metas`.
        fn test<T, I, const N: usize>(metas: I)
        where
            T: ?Sized + KnownLayout + Immutable + FromBytes,
            I: IntoIterator<Item = Option<T::PointerMetadata>> + Clone,
        {
            let mut bytes = [MaybeUninit::<u8>::uninit(); N];
            let initialized = [MaybeUninit::new(0u8); N];
            for start in 0..=bytes.len() {
                for end in start..=bytes.len() {
                    // Set all bytes to uninitialized other than those in
                    // the range we're going to pass to `try_cast_from`.
                    // This allows Miri to detect out-of-bounds reads
                    // because they read uninitialized memory. Without this,
                    // some out-of-bounds reads would still be in-bounds of
                    // `bytes`, and so might spuriously be accepted.
                    bytes = [MaybeUninit::<u8>::uninit(); N];
                    let bytes = &mut bytes[start..end];
                    // Initialize only the byte range we're going to pass to
                    // `try_cast_from`.
                    bytes.copy_from_slice(&initialized[start..end]);

                    let bytes = {
                        let bytes: *const [MaybeUninit<u8>] = bytes;
                        #[allow(clippy::as_conversions)]
                        let bytes = bytes as *const [u8];
                        // SAFETY: We just initialized these bytes to valid
                        // `u8`s.
                        unsafe { &*bytes }
                    };

                    // SAFETY: The bytes in `slf` must be initialized.
                    unsafe fn validate_and_get_len<
                        T: ?Sized + KnownLayout + FromBytes + Immutable,
                    >(
                        slf: Ptr<'_, T, (Shared, Aligned, Initialized)>,
                    ) -> usize {
                        let t = slf.recall_validity().as_ref();

                        let bytes = {
                            let len = mem::size_of_val(t);
                            let t: *const T = t;
                            // SAFETY:
                            // - We know `t`'s bytes are all initialized
                            //   because we just read it from `slf`, which
                            //   points to an initialized range of bytes. If
                            //   there's a bug and this doesn't hold, then
                            //   that's exactly what we're hoping Miri will
                            //   catch!
                            // - Since `T: FromBytes`, `T` doesn't contain
                            //   any `UnsafeCell`s, so it's okay for `t: T`
                            //   and a `&[u8]` to the same memory to be
                            //   alive concurrently.
                            unsafe { core::slice::from_raw_parts(t.cast::<u8>(), len) }
                        };

                        // This assertion ensures that `t`'s bytes are read
                        // and compared to another value, which in turn
                        // ensures that Miri gets a chance to notice if any
                        // of `t`'s bytes are uninitialized, which they
                        // shouldn't be (see the comment above).
                        assert_eq!(bytes, vec![0u8; bytes.len()]);

                        mem::size_of_val(t)
                    }

                    for meta in metas.clone().into_iter() {
                        for cast_type in [CastType::Prefix, CastType::Suffix] {
                            if let Ok((slf, remaining)) = Ptr::from_ref(bytes)
                                .try_cast_into::<T, BecauseImmutable>(cast_type, meta)
                            {
                                // SAFETY: All bytes in `bytes` have been
                                // initialized.
                                let len = unsafe { validate_and_get_len(slf) };
                                assert_eq!(remaining.len(), bytes.len() - len);
                                #[allow(unstable_name_collisions)]
                                let bytes_addr = bytes.as_ptr().addr();
                                #[allow(unstable_name_collisions)]
                                let remaining_addr =
                                    remaining.as_inner().as_non_null().as_ptr().addr();
                                match cast_type {
                                    CastType::Prefix => {
                                        assert_eq!(remaining_addr, bytes_addr + len)
                                    }
                                    CastType::Suffix => assert_eq!(remaining_addr, bytes_addr),
                                }

                                if let Some(want) = meta {
                                    let got = KnownLayout::pointer_to_metadata(
                                        slf.as_inner().as_non_null().as_ptr(),
                                    );
                                    assert_eq!(got, want);
                                }
                            }
                        }

                        if let Ok(slf) = Ptr::from_ref(bytes)
                            .try_cast_into_no_leftover::<T, BecauseImmutable>(meta)
                        {
                            // SAFETY: All bytes in `bytes` have been
                            // initialized.
                            let len = unsafe { validate_and_get_len(slf) };
                            assert_eq!(len, bytes.len());

                            if let Some(want) = meta {
                                let got = KnownLayout::pointer_to_metadata(
                                    slf.as_inner().as_non_null().as_ptr(),
                                );
                                assert_eq!(got, want);
                            }
                        }
                    }
                }
            }
        }

        #[derive(FromBytes, KnownLayout, Immutable)]
        #[repr(C)]
        struct SliceDst<T> {
            a: u8,
            trailing: [T],
        }

        // Each test case becomes its own `#[test]` function. We do this because
        // this test in particular takes far, far longer to execute under Miri
        // than all of our other tests combined. Previously, we had these
        // execute sequentially in a single test function. We run Miri tests in
        // parallel in CI, but this test being sequential meant that most of
        // that parallelism was wasted, as all other tests would finish in a
        // fraction of the total execution time, leaving this test to execute on
        // a single thread for the remainder of the test. By putting each test
        // case in its own function, we permit better use of available
        // parallelism.
        macro_rules! test {
            ($test_name:ident: $ty:ty) => {
                #[test]
                #[allow(non_snake_case)]
                fn $test_name() {
                    const S: usize = core::mem::size_of::<$ty>();
                    const N: usize = if S == 0 { 4 } else { S * 4 };
                    test::<$ty, _, N>([None]);

                    // If `$ty` is a ZST, then we can't pass `None` as the
                    // pointer metadata, or else computing the correct trailing
                    // slice length will panic.
                    if S == 0 {
                        test::<[$ty], _, N>([Some(0), Some(1), Some(2), Some(3)]);
                        test::<SliceDst<$ty>, _, N>([Some(0), Some(1), Some(2), Some(3)]);
                    } else {
                        test::<[$ty], _, N>([None, Some(0), Some(1), Some(2), Some(3)]);
                        test::<SliceDst<$ty>, _, N>([None, Some(0), Some(1), Some(2), Some(3)]);
                    }
                }
            };
            ($ty:ident) => {
                test!($ty: $ty);
            };
            ($($ty:ident),*) => { $(test!($ty);)* }
        }

        test!(empty_tuple: ());
        test!(u8, u16, u32, u64, u128, usize, AU64);
        test!(i8, i16, i32, i64, i128, isize);
        test!(f32, f64);
    }

    #[test]
    fn test_try_cast_into_explicit_count() {
        macro_rules! test {
            ($ty:ty, $bytes:expr, $elems:expr, $expect:expr) => {{
                let bytes = [0u8; $bytes];
                let ptr = Ptr::from_ref(&bytes[..]);
                let res =
                    ptr.try_cast_into::<$ty, BecauseImmutable>(CastType::Prefix, Some($elems));
                if let Some(expect) = $expect {
                    let (ptr, _) = res.unwrap();
                    assert_eq!(
                        KnownLayout::pointer_to_metadata(ptr.as_inner().as_non_null().as_ptr()),
                        expect
                    );
                } else {
                    let _ = res.unwrap_err();
                }
            }};
        }

        #[derive(KnownLayout, Immutable)]
        #[repr(C)]
        struct ZstDst {
            u: [u8; 8],
            slc: [()],
        }

        test!(ZstDst, 8, 0, Some(0));
        test!(ZstDst, 7, 0, None);

        test!(ZstDst, 8, usize::MAX, Some(usize::MAX));
        test!(ZstDst, 7, usize::MAX, None);

        #[derive(KnownLayout, Immutable)]
        #[repr(C)]
        struct Dst {
            u: [u8; 8],
            slc: [u8],
        }

        test!(Dst, 8, 0, Some(0));
        test!(Dst, 7, 0, None);

        test!(Dst, 9, 1, Some(1));
        test!(Dst, 8, 1, None);

        // If we didn't properly check for overflow, this would cause the
        // metadata to overflow to 0, and thus the cast would spuriously
        // succeed.
        test!(Dst, 8, usize::MAX - 8 + 1, None);
    }
}
