use core::mem::{self, MaybeUninit};

/// An array of at most `N` elements.
struct ArrayBuilder<T, const N: usize> {
    /// The (possibly uninitialized) elements of the `ArrayBuilder`.
    ///
    /// # Safety
    ///
    /// The elements of `arr[..len]` are valid `T`s.
    arr: [MaybeUninit<T>; N],

    /// The number of leading elements of `arr` that are valid `T`s, len <= N.
    len: usize,
}

impl<T, const N: usize> ArrayBuilder<T, N> {
    /// Initializes a new, empty `ArrayBuilder`.
    pub fn new() -> Self {
        // SAFETY: The safety invariant of `arr` trivially holds for `len = 0`.
        Self {
            arr: [(); N].map(|_| MaybeUninit::uninit()),
            len: 0,
        }
    }

    /// Pushes `value` onto the end of the array.
    ///
    /// # Panics
    ///
    /// This panics if `self.len >= N`.
    #[inline(always)]
    pub fn push(&mut self, value: T) {
        // PANICS: This will panic if `self.len >= N`.
        let place = &mut self.arr[self.len];
        // SAFETY: The safety invariant of `self.arr` applies to elements at
        // indices `0..self.len` — not to the element at `self.len`. Writing to
        // the element at index `self.len` therefore does not violate the safety
        // invariant of `self.arr`. Even if this line panics, we have not
        // created any intermediate invalid state.
        *place = MaybeUninit::new(value);
        // Lemma: `self.len < N`. By invariant, `self.len <= N`. Above, we index
        // into `self.arr`, which has size `N`, at index `self.len`. If `self.len == N`
        // at that point, that index would be out-of-bounds, and the index
        // operation would panic. Thus, `self.len != N`, and since `self.len <= N`,
        // that means that `self.len < N`.
        //
        // PANICS: Since `self.len < N`, and since `N <= usize::MAX`,
        // `self.len + 1 <= usize::MAX`, and so `self.len += 1` will not
        // overflow. Overflow is the only panic condition of `+=`.
        //
        // SAFETY:
        // - We are required to uphold the invariant that `self.len <= N`.
        //   Since, by the preceding lemma, `self.len < N` at this point in the
        //   code, `self.len += 1` results in `self.len <= N`.
        // - We are required to uphold the invariant that `self.arr[..self.len]`
        //   are valid instances of `T`. Since this invariant already held when
        //   this method was called, and since we only increment `self.len`
        //   by 1 here, we only need to prove that the element at
        //   `self.arr[self.len]` (using the value of `self.len` before incrementing)
        //   is valid. Above, we construct `place` to point to `self.arr[self.len]`,
        //   and then initialize `*place` to `MaybeUninit::new(value)`, which is
        //   a valid `T` by construction.
        self.len += 1;
    }

    /// Consumes the elements in the `ArrayBuilder` and returns them as an array
    /// `[T; N]`.
    ///
    /// If `self.len() < N`, this returns `None`.
    pub fn take(&mut self) -> Option<[T; N]> {
        if self.len == N {
            // SAFETY: Decreasing the value of `self.len` cannot violate the
            // safety invariant on `self.arr`.
            self.len = 0;

            // SAFETY: Since `self.len` is 0, `self.arr` may safely contain
            // uninitialized elements.
            let arr = mem::replace(&mut self.arr, [(); N].map(|_| MaybeUninit::uninit()));

            Some(arr.map(|v| {
                // SAFETY: We know that all elements of `arr` are valid because
                // we checked that `len == N`.
                unsafe { v.assume_init() }
            }))
        } else {
            None
        }
    }
}

impl<T, const N: usize> AsMut<[T]> for ArrayBuilder<T, N> {
    fn as_mut(&mut self) -> &mut [T] {
        let valid = &mut self.arr[..self.len];
        // SAFETY: By invariant on `self.arr`, the elements of `self.arr` at
        // indices `0..self.len` are in a valid state. Since `valid` references
        // only these elements, the safety precondition of
        // `slice_assume_init_mut` is satisfied.
        unsafe { slice_assume_init_mut(valid) }
    }
}

impl<T, const N: usize> Drop for ArrayBuilder<T, N> {
    // We provide a non-trivial `Drop` impl, because the trivial impl would be a
    // no-op; `MaybeUninit<T>` has no innate awareness of its own validity, and
    // so it can only forget its contents. By leveraging the safety invariant of
    // `self.arr`, we do know which elements of `self.arr` are valid, and can
    // selectively run their destructors.
    fn drop(&mut self) {
        // SAFETY:
        // - by invariant on `&mut [T]`, `self.as_mut()` is:
        //   - valid for reads and writes
        //   - properly aligned
        //   - non-null
        // - the dropped `T` are valid for dropping; they do not have any
        //   additional library invariants that we've violated
        // - no other pointers to `valid` exist (since we're in the context of
        //   `drop`)
        unsafe { core::ptr::drop_in_place(self.as_mut()) }
    }
}

/// Assuming all the elements are initialized, get a mutable slice to them.
///
/// # Safety
///
/// The caller guarantees that the elements `T` referenced by `slice` are in a
/// valid state.
unsafe fn slice_assume_init_mut<T>(slice: &mut [MaybeUninit<T>]) -> &mut [T] {
    // SAFETY: Casting `&mut [MaybeUninit<T>]` to `&mut [T]` is sound, because
    // `MaybeUninit<T>` is guaranteed to have the same size, alignment and ABI
    // as `T`, and because the caller has guaranteed that `slice` is in the
    // valid state.
    unsafe { &mut *(slice as *mut [MaybeUninit<T>] as *mut [T]) }
}

/// Equivalent to `it.next_array()`.
pub(crate) fn next_array<I, const N: usize>(it: &mut I) -> Option<[I::Item; N]>
where
    I: Iterator,
{
    let mut builder = ArrayBuilder::new();
    for _ in 0..N {
        builder.push(it.next()?);
    }
    builder.take()
}

#[cfg(test)]
mod test {
    use super::ArrayBuilder;

    #[test]
    fn zero_len_take() {
        let mut builder = ArrayBuilder::<(), 0>::new();
        let taken = builder.take();
        assert_eq!(taken, Some([(); 0]));
    }

    #[test]
    #[should_panic]
    fn zero_len_push() {
        let mut builder = ArrayBuilder::<(), 0>::new();
        builder.push(());
    }

    #[test]
    fn push_4() {
        let mut builder = ArrayBuilder::<(), 4>::new();
        assert_eq!(builder.take(), None);

        builder.push(());
        assert_eq!(builder.take(), None);

        builder.push(());
        assert_eq!(builder.take(), None);

        builder.push(());
        assert_eq!(builder.take(), None);

        builder.push(());
        assert_eq!(builder.take(), Some([(); 4]));
    }

    #[test]
    fn tracked_drop() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        use std::sync::atomic::{AtomicU16, Ordering};

        static DROPPED: AtomicU16 = AtomicU16::new(0);

        #[derive(Debug, PartialEq)]
        struct TrackedDrop;

        impl Drop for TrackedDrop {
            fn drop(&mut self) {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }

        {
            let builder = ArrayBuilder::<TrackedDrop, 0>::new();
            assert_eq!(DROPPED.load(Ordering::Relaxed), 0);
            drop(builder);
            assert_eq!(DROPPED.load(Ordering::Relaxed), 0);
        }

        {
            let mut builder = ArrayBuilder::<TrackedDrop, 2>::new();
            builder.push(TrackedDrop);
            assert_eq!(builder.take(), None);
            assert_eq!(DROPPED.load(Ordering::Relaxed), 0);
            drop(builder);
            assert_eq!(DROPPED.swap(0, Ordering::Relaxed), 1);
        }

        {
            let mut builder = ArrayBuilder::<TrackedDrop, 2>::new();
            builder.push(TrackedDrop);
            builder.push(TrackedDrop);
            assert!(matches!(builder.take(), Some(_)));
            assert_eq!(DROPPED.swap(0, Ordering::Relaxed), 2);
            drop(builder);
            assert_eq!(DROPPED.load(Ordering::Relaxed), 0);
        }

        {
            let mut builder = ArrayBuilder::<TrackedDrop, 2>::new();

            builder.push(TrackedDrop);
            builder.push(TrackedDrop);

            assert!(catch_unwind(AssertUnwindSafe(|| {
                builder.push(TrackedDrop);
            }))
            .is_err());

            assert_eq!(DROPPED.load(Ordering::Relaxed), 1);

            drop(builder);

            assert_eq!(DROPPED.swap(0, Ordering::Relaxed), 3);
        }

        {
            let mut builder = ArrayBuilder::<TrackedDrop, 2>::new();

            builder.push(TrackedDrop);
            builder.push(TrackedDrop);

            assert!(catch_unwind(AssertUnwindSafe(|| {
                builder.push(TrackedDrop);
            }))
            .is_err());

            assert_eq!(DROPPED.load(Ordering::Relaxed), 1);

            assert!(matches!(builder.take(), Some(_)));

            assert_eq!(DROPPED.load(Ordering::Relaxed), 3);

            builder.push(TrackedDrop);
            builder.push(TrackedDrop);

            assert!(matches!(builder.take(), Some(_)));

            assert_eq!(DROPPED.swap(0, Ordering::Relaxed), 5);
        }
    }
}
