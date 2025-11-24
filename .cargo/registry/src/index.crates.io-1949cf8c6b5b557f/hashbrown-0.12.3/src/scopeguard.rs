// Extracted from the scopeguard crate
use core::{
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

pub struct ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    dropfn: F,
    value: T,
}

#[inline]
pub fn guard<T, F>(value: T, dropfn: F) -> ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    ScopeGuard { dropfn, value }
}

impl<T, F> ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    #[inline]
    pub fn into_inner(guard: Self) -> T {
        // Cannot move out of Drop-implementing types, so
        // ptr::read the value and forget the guard.
        unsafe {
            let value = ptr::read(&guard.value);
            // read the closure so that it is dropped, and assign it to a local
            // variable to ensure that it is only dropped after the guard has
            // been forgotten. (In case the Drop impl of the closure, or that
            // of any consumed captured variable, panics).
            let _dropfn = ptr::read(&guard.dropfn);
            mem::forget(guard);
            value
        }
    }
}

impl<T, F> Deref for ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T, F> DerefMut for ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T, F> Drop for ScopeGuard<T, F>
where
    F: FnMut(&mut T),
{
    #[inline]
    fn drop(&mut self) {
        (self.dropfn)(&mut self.value);
    }
}
