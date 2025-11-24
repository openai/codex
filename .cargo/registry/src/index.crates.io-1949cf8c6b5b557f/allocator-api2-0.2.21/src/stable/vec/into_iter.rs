use core::fmt;
use core::iter::FusedIterator;
use core::marker::PhantomData;
use core::mem::{self, size_of, ManuallyDrop};

use core::ptr::{self, NonNull};
use core::slice::{self};

use crate::stable::addr;

use super::{Allocator, Global, RawVec};

#[cfg(not(no_global_oom_handling))]
use super::Vec;

/// An iterator that moves out of a vector.
///
/// This `struct` is created by the `into_iter` method on [`Vec`](super::Vec)
/// (provided by the [`IntoIterator`] trait).
///
/// # Example
///
/// ```
/// let v = vec![0, 1, 2];
/// let iter: std::vec::IntoIter<_> = v.into_iter();
/// ```
pub struct IntoIter<T, A: Allocator = Global> {
    pub(super) buf: NonNull<T>,
    pub(super) phantom: PhantomData<T>,
    pub(super) cap: usize,
    // the drop impl reconstructs a RawVec from buf, cap and alloc
    // to avoid dropping the allocator twice we need to wrap it into ManuallyDrop
    pub(super) alloc: ManuallyDrop<A>,
    pub(super) ptr: *const T,
    pub(super) end: *const T,
}

impl<T: fmt::Debug, A: Allocator> fmt::Debug for IntoIter<T, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

impl<T, A: Allocator> IntoIter<T, A> {
    /// Returns the remaining items of this iterator as a slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let vec = vec!['a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// let _ = into_iter.next().unwrap();
    /// assert_eq!(into_iter.as_slice(), &['b', 'c']);
    /// ```
    pub fn as_slice(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self.ptr, self.len()) }
    }

    /// Returns the remaining items of this iterator as a mutable slice.
    ///
    /// # Examples
    ///
    /// ```
    /// let vec = vec!['a', 'b', 'c'];
    /// let mut into_iter = vec.into_iter();
    /// assert_eq!(into_iter.as_slice(), &['a', 'b', 'c']);
    /// into_iter.as_mut_slice()[2] = 'z';
    /// assert_eq!(into_iter.next().unwrap(), 'a');
    /// assert_eq!(into_iter.next().unwrap(), 'b');
    /// assert_eq!(into_iter.next().unwrap(), 'z');
    /// ```
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        unsafe { &mut *self.as_raw_mut_slice() }
    }

    /// Returns a reference to the underlying allocator.
    #[inline(always)]
    pub fn allocator(&self) -> &A {
        &self.alloc
    }

    fn as_raw_mut_slice(&mut self) -> *mut [T] {
        ptr::slice_from_raw_parts_mut(self.ptr as *mut T, self.len())
    }
}

impl<T, A: Allocator> AsRef<[T]> for IntoIter<T, A> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

unsafe impl<T: Send, A: Allocator + Send> Send for IntoIter<T, A> {}

unsafe impl<T: Sync, A: Allocator + Sync> Sync for IntoIter<T, A> {}

impl<T, A: Allocator> Iterator for IntoIter<T, A> {
    type Item = T;

    #[inline(always)]
    fn next(&mut self) -> Option<T> {
        if self.ptr == self.end {
            None
        } else if size_of::<T>() == 0 {
            // purposefully don't use 'ptr.offset' because for
            // vectors with 0-size elements this would return the
            // same pointer.
            self.ptr = self.ptr.cast::<u8>().wrapping_add(1).cast();

            // Make up a value of this ZST.
            Some(unsafe { mem::zeroed() })
        } else {
            let old = self.ptr;
            self.ptr = unsafe { self.ptr.add(1) };

            Some(unsafe { ptr::read(old) })
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let exact = if size_of::<T>() == 0 {
            addr(self.end).wrapping_sub(addr(self.ptr))
        } else {
            unsafe { self.end.offset_from(self.ptr) as usize }
        };
        (exact, Some(exact))
    }

    #[inline(always)]
    fn count(self) -> usize {
        self.len()
    }
}

impl<T, A: Allocator> DoubleEndedIterator for IntoIter<T, A> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<T> {
        if self.end == self.ptr {
            None
        } else if size_of::<T>() == 0 {
            // See above for why 'ptr.offset' isn't used
            self.end = self.end.cast::<u8>().wrapping_add(1).cast();

            // Make up a value of this ZST.
            Some(unsafe { mem::zeroed() })
        } else {
            self.end = unsafe { self.end.sub(1) };

            Some(unsafe { ptr::read(self.end) })
        }
    }
}

impl<T, A: Allocator> ExactSizeIterator for IntoIter<T, A> {}

impl<T, A: Allocator> FusedIterator for IntoIter<T, A> {}

#[cfg(not(no_global_oom_handling))]
impl<T: Clone, A: Allocator + Clone> Clone for IntoIter<T, A> {
    fn clone(&self) -> Self {
        let mut vec = Vec::<T, A>::with_capacity_in(self.len(), (*self.alloc).clone());
        vec.extend(self.as_slice().iter().cloned());
        vec.into_iter()
    }
}

impl<T, A: Allocator> Drop for IntoIter<T, A> {
    fn drop(&mut self) {
        struct DropGuard<'a, T, A: Allocator>(&'a mut IntoIter<T, A>);

        impl<T, A: Allocator> Drop for DropGuard<'_, T, A> {
            fn drop(&mut self) {
                unsafe {
                    // `IntoIter::alloc` is not used anymore after this and will be dropped by RawVec
                    let alloc = ManuallyDrop::take(&mut self.0.alloc);
                    // RawVec handles deallocation
                    let _ = RawVec::from_raw_parts_in(self.0.buf.as_ptr(), self.0.cap, alloc);
                }
            }
        }

        let guard = DropGuard(self);
        // destroy the remaining elements
        unsafe {
            ptr::drop_in_place(guard.0.as_raw_mut_slice());
        }
        // now `guard` will be dropped and do the rest
    }
}
