use core::ffi::c_void;

use super::FREE_FN;

/// A raw pointer and a length, exposed as an iterator.
pub struct CBufferIter<T> {
    ptr: *mut T,
    count: usize,
    i: usize,
}

impl<T> CBufferIter<T> {
    pub const unsafe fn new(ptr: *mut T, count: usize) -> Self {
        Self { ptr, count, i: 0 }
    }
}

impl<T: Copy> Iterator for CBufferIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.i;
        if i >= self.count {
            None
        } else {
            self.i += 1;
            Some(unsafe { *self.ptr.add(i) })
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.i;
        (remaining, Some(remaining))
    }
}

impl<T: Copy> ExactSizeIterator for CBufferIter<T> {}

impl<T> Drop for CBufferIter<T> {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { (FREE_FN)(self.ptr.cast::<c_void>()) };
        }
    }
}
