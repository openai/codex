use crate::loom::cell::UnsafeCell;

use std::rc::Rc;

/// This is exactly like `Cell<Option<Rc<T>>>`, except that it provides a `get`
/// method even though `Rc` is not `Copy`.
pub(crate) struct RcCell<T> {
    inner: UnsafeCell<Option<Rc<T>>>,
}

impl<T> RcCell<T> {
    #[cfg(not(all(loom, test)))]
    pub(crate) const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(None),
        }
    }

    // The UnsafeCell in loom does not have a const `new` fn.
    #[cfg(all(loom, test))]
    pub(crate) fn new() -> Self {
        Self {
            inner: UnsafeCell::new(None),
        }
    }

    /// Safety: This method may not be called recursively.
    #[inline]
    unsafe fn with_inner<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Option<Rc<T>>) -> R,
    {
        // safety: This type is not Sync, so concurrent calls of this method
        // cannot happen. Furthermore, the caller guarantees that the method is
        // not called recursively. Finally, this is the only place that can
        // create mutable references to the inner Rc. This ensures that any
        // mutable references created here are exclusive.
        self.inner.with_mut(|ptr| f(&mut *ptr))
    }

    pub(crate) fn get(&self) -> Option<Rc<T>> {
        // safety: The `Rc::clone` method will not call any unknown user-code,
        // so it will not result in a recursive call to `with_inner`.
        unsafe { self.with_inner(|rc| rc.clone()) }
    }

    pub(crate) fn replace(&self, val: Option<Rc<T>>) -> Option<Rc<T>> {
        // safety: No destructors or other unknown user-code will run inside the
        // `with_inner` call, so no recursive call to `with_inner` can happen.
        unsafe { self.with_inner(|rc| std::mem::replace(rc, val)) }
    }

    pub(crate) fn set(&self, val: Option<Rc<T>>) {
        let old = self.replace(val);
        drop(old);
    }
}
