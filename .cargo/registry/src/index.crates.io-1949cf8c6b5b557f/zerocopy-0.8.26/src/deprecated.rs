// Copyright 2024 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Deprecated items. These are kept separate so that they don't clutter up
//! other modules.

use super::*;

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: KnownLayout + Immutable + ?Sized,
{
    #[deprecated(since = "0.8.0", note = "renamed to `Ref::from_bytes`")]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new(bytes: B) -> Option<Ref<B, T>> {
        Self::from_bytes(bytes).ok()
    }
}

impl<B, T> Ref<B, T>
where
    B: SplitByteSlice,
    T: KnownLayout + Immutable + ?Sized,
{
    #[deprecated(since = "0.8.0", note = "renamed to `Ref::from_prefix`")]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_from_prefix(bytes: B) -> Option<(Ref<B, T>, B)> {
        Self::from_prefix(bytes).ok()
    }
}

impl<B, T> Ref<B, T>
where
    B: SplitByteSlice,
    T: KnownLayout + Immutable + ?Sized,
{
    #[deprecated(since = "0.8.0", note = "renamed to `Ref::from_suffix`")]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_from_suffix(bytes: B) -> Option<(B, Ref<B, T>)> {
        Self::from_suffix(bytes).ok()
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: Unaligned + KnownLayout + Immutable + ?Sized,
{
    #[deprecated(
        since = "0.8.0",
        note = "use `Ref::from_bytes`; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_unaligned(bytes: B) -> Option<Ref<B, T>> {
        Self::from_bytes(bytes).ok()
    }
}

impl<B, T> Ref<B, T>
where
    B: SplitByteSlice,
    T: Unaligned + KnownLayout + Immutable + ?Sized,
{
    #[deprecated(
        since = "0.8.0",
        note = "use `Ref::from_prefix`; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_unaligned_from_prefix(bytes: B) -> Option<(Ref<B, T>, B)> {
        Self::from_prefix(bytes).ok()
    }
}

impl<B, T> Ref<B, T>
where
    B: SplitByteSlice,
    T: Unaligned + KnownLayout + Immutable + ?Sized,
{
    #[deprecated(
        since = "0.8.0",
        note = "use `Ref::from_suffix`; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_unaligned_from_suffix(bytes: B) -> Option<(B, Ref<B, T>)> {
        Self::from_suffix(bytes).ok()
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSlice,
    T: Immutable,
{
    #[deprecated(since = "0.8.0", note = "`Ref::from_bytes` now supports slices")]
    #[doc(hidden)]
    #[inline(always)]
    pub fn new_slice(bytes: B) -> Option<Ref<B, [T]>> {
        Self::from_bytes(bytes).ok()
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSlice,
    T: Unaligned + Immutable,
{
    #[deprecated(
        since = "0.8.0",
        note = "`Ref::from_bytes` now supports slices; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[inline(always)]
    pub fn new_slice_unaligned(bytes: B) -> Option<Ref<B, [T]>> {
        Ref::from_bytes(bytes).ok()
    }
}

impl<'a, B, T> Ref<B, [T]>
where
    B: 'a + IntoByteSlice<'a>,
    T: FromBytes + Immutable,
{
    #[deprecated(since = "0.8.0", note = "`Ref::into_ref` now supports slices")]
    #[doc(hidden)]
    #[inline(always)]
    pub fn into_slice(self) -> &'a [T] {
        Ref::into_ref(self)
    }
}

impl<'a, B, T> Ref<B, [T]>
where
    B: 'a + IntoByteSliceMut<'a>,
    T: FromBytes + IntoBytes + Immutable,
{
    #[deprecated(since = "0.8.0", note = "`Ref::into_mut` now supports slices")]
    #[doc(hidden)]
    #[inline(always)]
    pub fn into_mut_slice(self) -> &'a mut [T] {
        Ref::into_mut(self)
    }
}

impl<B, T> Ref<B, [T]>
where
    B: SplitByteSlice,
    T: Immutable,
{
    #[deprecated(since = "0.8.0", note = "replaced by `Ref::from_prefix_with_elems`")]
    #[must_use = "has no side effects"]
    #[doc(hidden)]
    #[inline(always)]
    pub fn new_slice_from_prefix(bytes: B, count: usize) -> Option<(Ref<B, [T]>, B)> {
        Ref::from_prefix_with_elems(bytes, count).ok()
    }

    #[deprecated(since = "0.8.0", note = "replaced by `Ref::from_suffix_with_elems`")]
    #[must_use = "has no side effects"]
    #[doc(hidden)]
    #[inline(always)]
    pub fn new_slice_from_suffix(bytes: B, count: usize) -> Option<(B, Ref<B, [T]>)> {
        Ref::from_suffix_with_elems(bytes, count).ok()
    }
}

impl<B, T> Ref<B, [T]>
where
    B: SplitByteSlice,
    T: Unaligned + Immutable,
{
    #[deprecated(
        since = "0.8.0",
        note = "use `Ref::from_prefix_with_elems`; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_slice_unaligned_from_prefix(bytes: B, count: usize) -> Option<(Ref<B, [T]>, B)> {
        Ref::from_prefix_with_elems(bytes, count).ok()
    }

    #[deprecated(
        since = "0.8.0",
        note = "use `Ref::from_suffix_with_elems`; for `T: Unaligned`, the returned `CastError` implements `Into<SizeError>`"
    )]
    #[doc(hidden)]
    #[must_use = "has no side effects"]
    #[inline(always)]
    pub fn new_slice_unaligned_from_suffix(bytes: B, count: usize) -> Option<(B, Ref<B, [T]>)> {
        Ref::from_suffix_with_elems(bytes, count).ok()
    }
}
