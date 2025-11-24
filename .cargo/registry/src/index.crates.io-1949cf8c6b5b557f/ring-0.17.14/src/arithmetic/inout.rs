// Copyright 2025 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

pub(crate) use crate::error::LenMismatchError;
use core::num::NonZeroUsize;

pub(crate) trait AliasingSlices2<T> {
    /// The pointers passed to `f` will be valid and non-null, and will not
    /// be dangling, so they can be passed to C functions.
    ///
    /// The first pointer, `r`, may be pointing to uninitialized memory for
    /// `expected_len` elements of type `T`, properly aligned and writable.
    /// `f` must not read from `r` before writing to it.
    ///
    /// The second & third pointers, `a` and `b`, point to `expected_len`
    /// values of type `T`, properly aligned.
    ///
    /// `r`, `a`, and/or `b` may alias each other only in the following ways:
    /// `ptr::eq(r, a)`, `ptr::eq(r, b)`, and/or `ptr::eq(a, b)`; i.e. they
    /// will not be "overlapping."
    ///
    /// Implementations of this trait shouldn't override this default
    /// implementation.
    #[inline(always)]
    fn with_non_dangling_non_null_pointers_ra<R>(
        self,
        expected_len: NonZeroUsize,
        f: impl FnOnce(*mut T, *const T) -> R,
    ) -> Result<R, LenMismatchError>
    where
        Self: Sized,
    {
        self.with_potentially_dangling_non_null_pointers_ra(expected_len.get(), f)
    }

    /// If `expected_len == 0` then the pointers passed to `f` may be
    /// dangling pointers, which should not be passed to C functions. In all
    /// other respects, this works like
    /// `Self::with_non_dangling_non_null_pointers_rab`.
    ///
    /// Implementations of this trait should implement this method and not
    /// `with_non_dangling_non_null_pointers_rab`. Users of this trait should
    /// use `with_non_dangling_non_null_pointers_rab` and not this.
    fn with_potentially_dangling_non_null_pointers_ra<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T) -> R,
    ) -> Result<R, LenMismatchError>;
}

impl<T> AliasingSlices2<T> for &mut [T] {
    fn with_potentially_dangling_non_null_pointers_ra<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T) -> R,
    ) -> Result<R, LenMismatchError> {
        let r = self;
        if r.len() != expected_len {
            return Err(LenMismatchError::new(r.len()));
        }
        Ok(f(r.as_mut_ptr(), r.as_ptr()))
    }
}

impl<T> AliasingSlices2<T> for (&mut [T], &[T]) {
    fn with_potentially_dangling_non_null_pointers_ra<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T) -> R,
    ) -> Result<R, LenMismatchError> {
        let (r, a) = self;
        if r.len() != expected_len {
            return Err(LenMismatchError::new(r.len()));
        }
        if a.len() != expected_len {
            return Err(LenMismatchError::new(a.len()));
        }
        Ok(f(r.as_mut_ptr(), a.as_ptr()))
    }
}

pub(crate) trait AliasingSlices3<T> {
    /// The pointers passed to `f` will all be non-null and properly aligned,
    /// and will not be dangling.
    ///
    /// The first pointer, `r` points to potentially-uninitialized writable
    /// space for `expected_len` elements of type `T`. Accordingly, `f` must
    /// not read from `r` before writing to it.
    ///
    /// The second & third pointers, `a` and `b`, point to `expected_len`
    /// initialized values of type `T`.
    ///
    /// `r`, `a`, and/or `b` may alias each other, but only in the following
    /// ways: `ptr::eq(r, a)`, `ptr::eq(r, b)`, and/or `ptr::eq(a, b)`; they
    /// will not be "overlapping."
    ///
    /// Implementations of this trait shouldn't override this default
    /// implementation.
    #[inline(always)]
    fn with_non_dangling_non_null_pointers_rab<R>(
        self,
        expected_len: NonZeroUsize,
        f: impl FnOnce(*mut T, *const T, *const T) -> R,
    ) -> Result<R, LenMismatchError>
    where
        Self: Sized,
    {
        self.with_potentially_dangling_non_null_pointers_rab(expected_len.get(), f)
    }

    /// If `expected_len == 0` then the pointers passed to `f` may be
    /// dangling pointers, which should not be passed to C functions. In all
    /// other respects, this works like
    /// `Self::with_non_dangling_non_null_pointers_rab`.
    ///
    /// Implementations of this trait should implement this method and not
    /// `with_non_dangling_non_null_pointers_rab`. Users of this trait should
    /// use `with_non_dangling_non_null_pointers_rab` and not this.
    fn with_potentially_dangling_non_null_pointers_rab<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T, *const T) -> R,
    ) -> Result<R, LenMismatchError>;
}

impl<T> AliasingSlices3<T> for &mut [T] {
    fn with_potentially_dangling_non_null_pointers_rab<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T, *const T) -> R,
    ) -> Result<R, LenMismatchError> {
        <Self as AliasingSlices2<T>>::with_potentially_dangling_non_null_pointers_ra(
            self,
            expected_len,
            |r, a| f(r, r, a),
        )
    }
}

impl<T> AliasingSlices3<T> for (&mut [T], &[T], &[T]) {
    fn with_potentially_dangling_non_null_pointers_rab<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T, *const T) -> R,
    ) -> Result<R, LenMismatchError> {
        let (r, a, b) = self;
        ((r, a), b).with_potentially_dangling_non_null_pointers_rab(expected_len, f)
    }
}

impl<RA, T> AliasingSlices3<T> for (RA, &[T])
where
    RA: AliasingSlices2<T>,
{
    fn with_potentially_dangling_non_null_pointers_rab<R>(
        self,
        expected_len: usize,
        f: impl FnOnce(*mut T, *const T, *const T) -> R,
    ) -> Result<R, LenMismatchError> {
        let (ra, b) = self;
        if b.len() != expected_len {
            return Err(LenMismatchError::new(b.len()));
        }
        ra.with_potentially_dangling_non_null_pointers_ra(expected_len, |r, a| f(r, a, b.as_ptr()))
    }
}
