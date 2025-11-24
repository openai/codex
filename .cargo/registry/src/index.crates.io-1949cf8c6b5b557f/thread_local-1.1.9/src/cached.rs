#![allow(deprecated)]

use super::{IntoIter, IterMut, ThreadLocal};
use std::fmt;
use std::panic::UnwindSafe;
use std::usize;

/// Wrapper around [`ThreadLocal`].
///
/// This used to add a fast path for a single thread, however that has been
/// obsoleted by performance improvements to [`ThreadLocal`] itself.
#[deprecated(since = "1.1.0", note = "Use `ThreadLocal` instead")]
pub struct CachedThreadLocal<T: Send> {
    inner: ThreadLocal<T>,
}

impl<T: Send> Default for CachedThreadLocal<T> {
    fn default() -> CachedThreadLocal<T> {
        CachedThreadLocal::new()
    }
}

impl<T: Send> CachedThreadLocal<T> {
    /// Creates a new empty `CachedThreadLocal`.
    #[inline]
    pub fn new() -> CachedThreadLocal<T> {
        CachedThreadLocal {
            inner: ThreadLocal::new(),
        }
    }

    /// Returns the element for the current thread, if it exists.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        self.inner.get()
    }

    /// Returns the element for the current thread, or creates it if it doesn't
    /// exist.
    #[inline]
    pub fn get_or<F>(&self, create: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.inner.get_or(create)
    }

    /// Returns the element for the current thread, or creates it if it doesn't
    /// exist. If `create` fails, that error is returned and no element is
    /// added.
    #[inline]
    pub fn get_or_try<F, E>(&self, create: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        self.inner.get_or_try(create)
    }

    /// Returns a mutable iterator over the local values of all threads.
    ///
    /// Since this call borrows the `ThreadLocal` mutably, this operation can
    /// be done safely---the mutable borrow statically guarantees no other
    /// threads are currently accessing their associated values.
    #[inline]
    pub fn iter_mut(&mut self) -> CachedIterMut<T> {
        CachedIterMut {
            inner: self.inner.iter_mut(),
        }
    }

    /// Removes all thread-specific values from the `ThreadLocal`, effectively
    /// reseting it to its original state.
    ///
    /// Since this call borrows the `ThreadLocal` mutably, this operation can
    /// be done safely---the mutable borrow statically guarantees no other
    /// threads are currently accessing their associated values.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl<T: Send> IntoIterator for CachedThreadLocal<T> {
    type Item = T;
    type IntoIter = CachedIntoIter<T>;

    fn into_iter(self) -> CachedIntoIter<T> {
        CachedIntoIter {
            inner: self.inner.into_iter(),
        }
    }
}

impl<'a, T: Send + 'a> IntoIterator for &'a mut CachedThreadLocal<T> {
    type Item = &'a mut T;
    type IntoIter = CachedIterMut<'a, T>;

    fn into_iter(self) -> CachedIterMut<'a, T> {
        self.iter_mut()
    }
}

impl<T: Send + Default> CachedThreadLocal<T> {
    /// Returns the element for the current thread, or creates a default one if
    /// it doesn't exist.
    pub fn get_or_default(&self) -> &T {
        self.get_or(T::default)
    }
}

impl<T: Send + fmt::Debug> fmt::Debug for CachedThreadLocal<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ThreadLocal {{ local_data: {:?} }}", self.get())
    }
}

impl<T: Send + UnwindSafe> UnwindSafe for CachedThreadLocal<T> {}

/// Mutable iterator over the contents of a `CachedThreadLocal`.
#[deprecated(since = "1.1.0", note = "Use `IterMut` instead")]
pub struct CachedIterMut<'a, T: Send + 'a> {
    inner: IterMut<'a, T>,
}

impl<'a, T: Send + 'a> Iterator for CachedIterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<&'a mut T> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, T: Send + 'a> ExactSizeIterator for CachedIterMut<'a, T> {}

/// An iterator that moves out of a `CachedThreadLocal`.
#[deprecated(since = "1.1.0", note = "Use `IntoIter` instead")]
pub struct CachedIntoIter<T: Send> {
    inner: IntoIter<T>,
}

impl<T: Send> Iterator for CachedIntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<T: Send> ExactSizeIterator for CachedIntoIter<T> {}
