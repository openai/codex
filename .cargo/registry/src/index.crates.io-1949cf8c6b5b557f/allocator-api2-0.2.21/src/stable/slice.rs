use crate::{
    alloc::{Allocator, Global},
    vec::Vec,
};

/// Slice methods that use `Box` and `Vec` from this crate.
pub trait SliceExt<T> {
    /// Copies `self` into a new `Vec`.
    ///
    /// # Examples
    ///
    /// ```
    /// let s = [10, 40, 30];
    /// let x = s.to_vec();
    /// // Here, `s` and `x` can be modified independently.
    /// ```
    #[cfg(not(no_global_oom_handling))]
    #[inline(always)]
    fn to_vec(&self) -> Vec<T, Global>
    where
        T: Clone,
    {
        self.to_vec_in(Global)
    }

    /// Copies `self` into a new `Vec` with an allocator.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(allocator_api)]
    ///
    /// use std::alloc::System;
    ///
    /// let s = [10, 40, 30];
    /// let x = s.to_vec_in(System);
    /// // Here, `s` and `x` can be modified independently.
    /// ```
    #[cfg(not(no_global_oom_handling))]
    fn to_vec_in<A: Allocator>(&self, alloc: A) -> Vec<T, A>
    where
        T: Clone;

    /// Creates a vector by copying a slice `n` times.
    ///
    /// # Panics
    ///
    /// This function will panic if the capacity would overflow.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// assert_eq!([1, 2].repeat(3), vec![1, 2, 1, 2, 1, 2]);
    /// ```
    ///
    /// A panic upon overflow:
    ///
    /// ```should_panic
    /// // this will panic at runtime
    /// b"0123456789abcdef".repeat(usize::MAX);
    /// ```
    fn repeat(&self, n: usize) -> Vec<T, Global>
    where
        T: Copy;
}

impl<T> SliceExt<T> for [T] {
    #[cfg(not(no_global_oom_handling))]
    #[inline]
    fn to_vec_in<A: Allocator>(&self, alloc: A) -> Vec<T, A>
    where
        T: Clone,
    {
        struct DropGuard<'a, T, A: Allocator> {
            vec: &'a mut Vec<T, A>,
            num_init: usize,
        }
        impl<'a, T, A: Allocator> Drop for DropGuard<'a, T, A> {
            #[inline]
            fn drop(&mut self) {
                // SAFETY:
                // items were marked initialized in the loop below
                unsafe {
                    self.vec.set_len(self.num_init);
                }
            }
        }

        let mut vec = Vec::with_capacity_in(self.len(), alloc);
        let mut guard = DropGuard {
            vec: &mut vec,
            num_init: 0,
        };
        let slots = guard.vec.spare_capacity_mut();
        // .take(slots.len()) is necessary for LLVM to remove bounds checks
        // and has better codegen than zip.
        for (i, b) in self.iter().enumerate().take(slots.len()) {
            guard.num_init = i;
            slots[i].write(b.clone());
        }
        core::mem::forget(guard);
        // SAFETY:
        // the vec was allocated and initialized above to at least this length.
        unsafe {
            vec.set_len(self.len());
        }
        vec
    }

    #[cfg(not(no_global_oom_handling))]
    #[inline]
    fn repeat(&self, n: usize) -> Vec<T, Global>
    where
        T: Copy,
    {
        if n == 0 {
            return Vec::new();
        }

        // If `n` is larger than zero, it can be split as
        // `n = 2^expn + rem (2^expn > rem, expn >= 0, rem >= 0)`.
        // `2^expn` is the number represented by the leftmost '1' bit of `n`,
        // and `rem` is the remaining part of `n`.

        // Using `Vec` to access `set_len()`.
        let capacity = self.len().checked_mul(n).expect("capacity overflow");
        let mut buf = Vec::with_capacity(capacity);

        // `2^expn` repetition is done by doubling `buf` `expn`-times.
        buf.extend(self);
        {
            let mut m = n >> 1;
            // If `m > 0`, there are remaining bits up to the leftmost '1'.
            while m > 0 {
                // `buf.extend(buf)`:
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        buf.as_ptr(),
                        (buf.as_mut_ptr() as *mut T).add(buf.len()),
                        buf.len(),
                    );
                    // `buf` has capacity of `self.len() * n`.
                    let buf_len = buf.len();
                    buf.set_len(buf_len * 2);
                }

                m >>= 1;
            }
        }

        // `rem` (`= n - 2^expn`) repetition is done by copying
        // first `rem` repetitions from `buf` itself.
        let rem_len = capacity - buf.len(); // `self.len() * rem`
        if rem_len > 0 {
            // `buf.extend(buf[0 .. rem_len])`:
            unsafe {
                // This is non-overlapping since `2^expn > rem`.
                core::ptr::copy_nonoverlapping(
                    buf.as_ptr(),
                    (buf.as_mut_ptr() as *mut T).add(buf.len()),
                    rem_len,
                );
                // `buf.len() + rem_len` equals to `buf.capacity()` (`= self.len() * n`).
                buf.set_len(capacity);
            }
        }
        buf
    }
}
