use std::{marker::PhantomData, ptr::NonNull};

/// An owned pointer
///
/// **NOTE**: Does not deallocate when dropped
pub(crate) struct OwnedPtr<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T: ?Sized> Copy for OwnedPtr<T> {}

impl<T: ?Sized> Clone for OwnedPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

unsafe impl<T> Send for OwnedPtr<T> where T: Send {}
unsafe impl<T> Sync for OwnedPtr<T> where T: Send {}

impl<T> OwnedPtr<T> {
    pub(crate) fn new(value: T) -> Self {
        Self::from_boxed(Box::new(value))
    }

    pub(crate) fn from_boxed(boxed: Box<T>) -> Self {
        // Safety: `Box::into_raw` is guaranteed to be non-null
        Self {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
        }
    }

    /// Convert the pointer to another type
    pub(crate) fn cast<U>(self) -> OwnedPtr<U> {
        OwnedPtr {
            ptr: self.ptr.cast(),
        }
    }

    /// Context the pointer into a Box
    ///
    /// # Safety
    ///
    /// Dropping the Box will deallocate a layout of `T` and run the destructor of `T`.
    ///
    /// A cast pointer must therefore be cast back to the original type before calling this method.
    pub(crate) unsafe fn into_box(self) -> Box<T> {
        unsafe { Box::from_raw(self.ptr.as_ptr()) }
    }

    pub(crate) const fn as_ref(&self) -> RefPtr<'_, T> {
        RefPtr {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }

    pub(crate) fn as_mut(&mut self) -> MutPtr<'_, T> {
        MutPtr {
            ptr: self.ptr,
            _marker: PhantomData,
        }
    }
}

/// Convenience lifetime annotated mutable pointer which facilitates returning an inferred lifetime
/// in a `fn` pointer.
pub(crate) struct RefPtr<'a, T: ?Sized> {
    pub(crate) ptr: NonNull<T>,
    _marker: PhantomData<&'a T>,
}

/// Safety: RefPtr indicates a shared reference to a value and as such exhibits the same Send +
/// Sync behavior of &'a T
unsafe impl<'a, T: ?Sized> Send for RefPtr<'a, T> where &'a T: Send {}
unsafe impl<'a, T: ?Sized> Sync for RefPtr<'a, T> where &'a T: Sync {}

impl<'a, T: ?Sized> Copy for RefPtr<'a, T> {}
impl<'a, T: ?Sized> Clone for RefPtr<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: ?Sized> RefPtr<'a, T> {
    pub(crate) fn new(ptr: &'a T) -> Self {
        Self {
            ptr: NonNull::from(ptr),
            _marker: PhantomData,
        }
    }

    /// Convert the pointer to another type
    pub(crate) fn cast<U>(self) -> RefPtr<'a, U> {
        RefPtr {
            ptr: self.ptr.cast(),
            _marker: PhantomData,
        }
    }

    /// Returns a shared reference to the owned value
    ///
    /// # Safety
    ///
    /// See: [`NonNull::as_ref`]
    #[inline]
    pub(crate) unsafe fn as_ref(&self) -> &'a T {
        unsafe { self.ptr.as_ref() }
    }
}

/// Convenience lifetime annotated mutable pointer which facilitates returning an inferred lifetime
/// in a `fn` pointer.
pub(crate) struct MutPtr<'a, T: ?Sized> {
    pub(crate) ptr: NonNull<T>,
    _marker: PhantomData<&'a mut T>,
}

/// Safety: RefPtr indicates an exclusive reference to a value and as such exhibits the same Send +
/// Sync behavior of &'a mut T
unsafe impl<'a, T: ?Sized> Send for MutPtr<'a, T> where &'a mut T: Send {}
unsafe impl<'a, T: ?Sized> Sync for MutPtr<'a, T> where &'a mut T: Sync {}

impl<'a, T: ?Sized> Copy for MutPtr<'a, T> {}
impl<'a, T: ?Sized> Clone for MutPtr<'a, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T: ?Sized> MutPtr<'a, T> {
    /// Convert the pointer to another type
    pub(crate) fn cast<U>(self) -> MutPtr<'a, U> {
        MutPtr {
            ptr: self.ptr.cast(),
            _marker: PhantomData,
        }
    }

    /// Returns a mutable reference to the owned value with the lifetime decoupled from self
    ///
    /// # Safety
    ///
    /// See: [`NonNull::as_mut`]
    #[inline]
    pub(crate) unsafe fn into_mut(mut self) -> &'a mut T {
        unsafe { self.ptr.as_mut() }
    }
}
