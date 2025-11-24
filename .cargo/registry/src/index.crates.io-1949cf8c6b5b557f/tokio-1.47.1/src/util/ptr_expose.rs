//! Utility for helping miri understand our exposed pointers.
//!
//! During normal execution, this module is equivalent to pointer casts. However, when running
//! under miri, pointer casts are replaced with lookups in a hash map. This makes Tokio compatible
//! with strict provenance when running under miri (which comes with a performance cost).

use std::marker::PhantomData;
#[cfg(miri)]
use {crate::loom::sync::Mutex, std::collections::BTreeMap};

pub(crate) struct PtrExposeDomain<T> {
    #[cfg(miri)]
    map: Mutex<BTreeMap<usize, *const T>>,
    _phantom: PhantomData<T>,
}

// SAFETY: Actually using the pointers is unsafe, so it's sound to transfer them across threads.
unsafe impl<T> Sync for PtrExposeDomain<T> {}

impl<T> PtrExposeDomain<T> {
    pub(crate) const fn new() -> Self {
        Self {
            #[cfg(miri)]
            map: Mutex::const_new(BTreeMap::new()),
            _phantom: PhantomData,
        }
    }

    #[inline]
    pub(crate) fn expose_provenance(&self, ptr: *const T) -> usize {
        #[cfg(miri)]
        {
            // FIXME: Use `pointer:addr` when it is stable.
            // SAFETY: Equivalent to `pointer::addr` which is safe.
            let addr: usize = unsafe { std::mem::transmute(ptr) };
            self.map.lock().insert(addr, ptr);
            addr
        }

        #[cfg(not(miri))]
        {
            ptr as usize
        }
    }

    #[inline]
    #[allow(clippy::wrong_self_convention)] // mirrors std name
    pub(crate) fn from_exposed_addr(&self, addr: usize) -> *const T {
        #[cfg(miri)]
        {
            let maybe_ptr = self.map.lock().get(&addr).copied();

            // SAFETY: Intentionally trigger a miri failure if the provenance we want is not
            // exposed.
            unsafe { maybe_ptr.unwrap_unchecked() }
        }

        #[cfg(not(miri))]
        {
            addr as *const T
        }
    }

    #[inline]
    pub(crate) fn unexpose_provenance(&self, _ptr: *const T) {
        #[cfg(miri)]
        {
            // SAFETY: Equivalent to `pointer::addr` which is safe.
            let addr: usize = unsafe { std::mem::transmute(_ptr) };
            let maybe_ptr = self.map.lock().remove(&addr);

            // SAFETY: Intentionally trigger a miri failure if the provenance we want is not
            // exposed.
            unsafe { maybe_ptr.unwrap_unchecked() };
        }
    }
}
