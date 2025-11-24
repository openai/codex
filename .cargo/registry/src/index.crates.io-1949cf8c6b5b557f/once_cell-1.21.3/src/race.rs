//! Thread-safe, non-blocking, "first one wins" flavor of `OnceCell`.
//!
//! If two threads race to initialize a type from the `race` module, they
//! don't block, execute initialization function together, but only one of
//! them stores the result.
//!
//! This module does not require `std` feature.
//!
//! # Atomic orderings
//!
//! All types in this module use `Acquire` and `Release`
//! [atomic orderings](Ordering) for all their operations. While this is not
//! strictly necessary for types other than `OnceBox`, it is useful for users as
//! it allows them to be certain that after `get` or `get_or_init` returns on
//! one thread, any side-effects caused by the setter thread prior to them
//! calling `set` or `get_or_init` will be made visible to that thread; without
//! it, it's possible for it to appear as if they haven't happened yet from the
//! getter thread's perspective. This is an acceptable tradeoff to make since
//! `Acquire` and `Release` have very little performance overhead on most
//! architectures versus `Relaxed`.

// The "atomic orderings" section of the documentation above promises
// "happens-before" semantics. This drives the choice of orderings in the uses
// of `compare_exchange` below. On success, the value was zero/null, so there
// was nothing to acquire (there is never any `Ordering::Release` store of 0).
// On failure, the value was nonzero, so it was initialized previously (perhaps
// on another thread) using `Ordering::Release`, so we must use
// `Ordering::Acquire` to ensure that store "happens-before" this load.

#[cfg(not(feature = "portable-atomic"))]
use core::sync::atomic;
#[cfg(feature = "portable-atomic")]
use portable_atomic as atomic;

use atomic::{AtomicPtr, AtomicUsize, Ordering};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::num::NonZeroUsize;
use core::ptr;

/// A thread-safe cell which can be written to only once.
#[derive(Default, Debug)]
pub struct OnceNonZeroUsize {
    inner: AtomicUsize,
}

impl OnceNonZeroUsize {
    /// Creates a new empty cell.
    #[inline]
    pub const fn new() -> Self {
        Self { inner: AtomicUsize::new(0) }
    }

    /// Gets the underlying value.
    #[inline]
    pub fn get(&self) -> Option<NonZeroUsize> {
        let val = self.inner.load(Ordering::Acquire);
        NonZeroUsize::new(val)
    }

    /// Get the reference to the underlying value, without checking if the cell
    /// is initialized.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the cell is in initialized state, and that
    /// the contents are acquired by (synchronized to) this thread.
    pub unsafe fn get_unchecked(&self) -> NonZeroUsize {
        #[inline(always)]
        fn as_const_ptr(r: &AtomicUsize) -> *const usize {
            use core::mem::align_of;

            let p: *const AtomicUsize = r;
            // SAFETY: "This type has the same size and bit validity as
            // the underlying integer type, usize. However, the alignment of
            // this type is always equal to its size, even on targets where
            // usize has a lesser alignment."
            const _ALIGNMENT_COMPATIBLE: () =
                assert!(align_of::<AtomicUsize>() % align_of::<usize>() == 0);
            p.cast::<usize>()
        }

        // TODO(MSRV-1.70): Use `AtomicUsize::as_ptr().cast_const()`
        // See https://github.com/rust-lang/rust/issues/138246.
        let p = as_const_ptr(&self.inner);

        // SAFETY: The caller is responsible for ensuring that the value
        // was initialized and that the contents have been acquired by
        // this thread. Assuming that, we can assume there will be no
        // conflicting writes to the value since the value will never
        // change once initialized. This relies on the statement in
        // https://doc.rust-lang.org/1.83.0/core/sync/atomic/ that "(A
        // `compare_exchange` or `compare_exchange_weak` that does not
        // succeed is not considered a write."
        let val = unsafe { p.read() };

        // SAFETY: The caller is responsible for ensuring the value is
        // initialized and thus not zero.
        unsafe { NonZeroUsize::new_unchecked(val) }
    }

    /// Sets the contents of this cell to `value`.
    ///
    /// Returns `Ok(())` if the cell was empty and `Err(())` if it was
    /// full.
    #[inline]
    pub fn set(&self, value: NonZeroUsize) -> Result<(), ()> {
        match self.compare_exchange(value) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell was
    /// empty.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_init<F>(&self, f: F) -> NonZeroUsize
    where
        F: FnOnce() -> NonZeroUsize,
    {
        enum Void {}
        match self.get_or_try_init(|| Ok::<NonZeroUsize, Void>(f())) {
            Ok(val) => val,
            Err(void) => match void {},
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if
    /// the cell was empty. If the cell was empty and `f` failed, an
    /// error is returned.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<NonZeroUsize, E>
    where
        F: FnOnce() -> Result<NonZeroUsize, E>,
    {
        match self.get() {
            Some(it) => Ok(it),
            None => self.init(f),
        }
    }

    #[cold]
    #[inline(never)]
    fn init<E>(&self, f: impl FnOnce() -> Result<NonZeroUsize, E>) -> Result<NonZeroUsize, E> {
        let nz = f()?;
        let mut val = nz.get();
        if let Err(old) = self.compare_exchange(nz) {
            val = old;
        }
        Ok(unsafe { NonZeroUsize::new_unchecked(val) })
    }

    #[inline(always)]
    fn compare_exchange(&self, val: NonZeroUsize) -> Result<usize, usize> {
        self.inner.compare_exchange(0, val.get(), Ordering::Release, Ordering::Acquire)
    }
}

/// A thread-safe cell which can be written to only once.
#[derive(Default, Debug)]
pub struct OnceBool {
    inner: OnceNonZeroUsize,
}

impl OnceBool {
    /// Creates a new empty cell.
    #[inline]
    pub const fn new() -> Self {
        Self { inner: OnceNonZeroUsize::new() }
    }

    /// Gets the underlying value.
    #[inline]
    pub fn get(&self) -> Option<bool> {
        self.inner.get().map(Self::from_usize)
    }

    /// Sets the contents of this cell to `value`.
    ///
    /// Returns `Ok(())` if the cell was empty and `Err(())` if it was
    /// full.
    #[inline]
    pub fn set(&self, value: bool) -> Result<(), ()> {
        self.inner.set(Self::to_usize(value))
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell was
    /// empty.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_init<F>(&self, f: F) -> bool
    where
        F: FnOnce() -> bool,
    {
        Self::from_usize(self.inner.get_or_init(|| Self::to_usize(f())))
    }

    /// Gets the contents of the cell, initializing it with `f` if
    /// the cell was empty. If the cell was empty and `f` failed, an
    /// error is returned.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<bool, E>
    where
        F: FnOnce() -> Result<bool, E>,
    {
        self.inner.get_or_try_init(|| f().map(Self::to_usize)).map(Self::from_usize)
    }

    #[inline]
    fn from_usize(value: NonZeroUsize) -> bool {
        value.get() == 1
    }

    #[inline]
    fn to_usize(value: bool) -> NonZeroUsize {
        unsafe { NonZeroUsize::new_unchecked(if value { 1 } else { 2 }) }
    }
}

/// A thread-safe cell which can be written to only once.
pub struct OnceRef<'a, T> {
    inner: AtomicPtr<T>,
    ghost: PhantomData<UnsafeCell<&'a T>>,
}

// TODO: Replace UnsafeCell with SyncUnsafeCell once stabilized
unsafe impl<'a, T: Sync> Sync for OnceRef<'a, T> {}

impl<'a, T> core::fmt::Debug for OnceRef<'a, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "OnceRef({:?})", self.inner)
    }
}

impl<'a, T> Default for OnceRef<'a, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> OnceRef<'a, T> {
    /// Creates a new empty cell.
    pub const fn new() -> Self {
        Self { inner: AtomicPtr::new(ptr::null_mut()), ghost: PhantomData }
    }

    /// Gets a reference to the underlying value.
    pub fn get(&self) -> Option<&'a T> {
        let ptr = self.inner.load(Ordering::Acquire);
        unsafe { ptr.as_ref() }
    }

    /// Sets the contents of this cell to `value`.
    ///
    /// Returns `Ok(())` if the cell was empty and `Err(value)` if it was
    /// full.
    pub fn set(&self, value: &'a T) -> Result<(), ()> {
        match self.compare_exchange(value) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if the cell was
    /// empty.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_init<F>(&self, f: F) -> &'a T
    where
        F: FnOnce() -> &'a T,
    {
        enum Void {}
        match self.get_or_try_init(|| Ok::<&'a T, Void>(f())) {
            Ok(val) => val,
            Err(void) => match void {},
        }
    }

    /// Gets the contents of the cell, initializing it with `f` if
    /// the cell was empty. If the cell was empty and `f` failed, an
    /// error is returned.
    ///
    /// If several threads concurrently run `get_or_init`, more than one `f` can
    /// be called. However, all threads will return the same value, produced by
    /// some `f`.
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&'a T, E>
    where
        F: FnOnce() -> Result<&'a T, E>,
    {
        match self.get() {
            Some(val) => Ok(val),
            None => self.init(f),
        }
    }

    #[cold]
    #[inline(never)]
    fn init<E>(&self, f: impl FnOnce() -> Result<&'a T, E>) -> Result<&'a T, E> {
        let mut value: &'a T = f()?;
        if let Err(old) = self.compare_exchange(value) {
            value = unsafe { &*old };
        }
        Ok(value)
    }

    #[inline(always)]
    fn compare_exchange(&self, value: &'a T) -> Result<(), *const T> {
        self.inner
            .compare_exchange(
                ptr::null_mut(),
                <*const T>::cast_mut(value),
                Ordering::Release,
                Ordering::Acquire,
            )
            .map(|_: *mut T| ())
            .map_err(<*mut T>::cast_const)
    }

    /// ```compile_fail
    /// use once_cell::race::OnceRef;
    ///
    /// let mut l = OnceRef::new();
    ///
    /// {
    ///     let y = 2;
    ///     let mut r = OnceRef::new();
    ///     r.set(&y).unwrap();
    ///     core::mem::swap(&mut l, &mut r);
    /// }
    ///
    /// // l now contains a dangling reference to y
    /// eprintln!("uaf: {}", l.get().unwrap());
    /// ```
    fn _dummy() {}
}

#[cfg(feature = "alloc")]
pub use self::once_box::OnceBox;

#[cfg(feature = "alloc")]
mod once_box {
    use super::atomic::{AtomicPtr, Ordering};
    use core::{marker::PhantomData, ptr};

    use alloc::boxed::Box;

    /// A thread-safe cell which can be written to only once.
    pub struct OnceBox<T> {
        inner: AtomicPtr<T>,
        ghost: PhantomData<Option<Box<T>>>,
    }

    impl<T> core::fmt::Debug for OnceBox<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "OnceBox({:?})", self.inner.load(Ordering::Relaxed))
        }
    }

    impl<T> Default for OnceBox<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Drop for OnceBox<T> {
        fn drop(&mut self) {
            let ptr = *self.inner.get_mut();
            if !ptr.is_null() {
                drop(unsafe { Box::from_raw(ptr) })
            }
        }
    }

    impl<T> OnceBox<T> {
        /// Creates a new empty cell.
        pub const fn new() -> Self {
            Self { inner: AtomicPtr::new(ptr::null_mut()), ghost: PhantomData }
        }

        /// Creates a new cell with the given value.
        pub fn with_value(value: Box<T>) -> Self {
            Self { inner: AtomicPtr::new(Box::into_raw(value)), ghost: PhantomData }
        }

        /// Gets a reference to the underlying value.
        pub fn get(&self) -> Option<&T> {
            let ptr = self.inner.load(Ordering::Acquire);
            if ptr.is_null() {
                return None;
            }
            Some(unsafe { &*ptr })
        }

        /// Sets the contents of this cell to `value`.
        ///
        /// Returns `Ok(())` if the cell was empty and `Err(value)` if it was
        /// full.
        pub fn set(&self, value: Box<T>) -> Result<(), Box<T>> {
            let ptr = Box::into_raw(value);
            let exchange = self.inner.compare_exchange(
                ptr::null_mut(),
                ptr,
                Ordering::Release,
                Ordering::Acquire,
            );
            if exchange.is_err() {
                let value = unsafe { Box::from_raw(ptr) };
                return Err(value);
            }
            Ok(())
        }

        /// Gets the contents of the cell, initializing it with `f` if the cell was
        /// empty.
        ///
        /// If several threads concurrently run `get_or_init`, more than one `f` can
        /// be called. However, all threads will return the same value, produced by
        /// some `f`.
        pub fn get_or_init<F>(&self, f: F) -> &T
        where
            F: FnOnce() -> Box<T>,
        {
            enum Void {}
            match self.get_or_try_init(|| Ok::<Box<T>, Void>(f())) {
                Ok(val) => val,
                Err(void) => match void {},
            }
        }

        /// Gets the contents of the cell, initializing it with `f` if
        /// the cell was empty. If the cell was empty and `f` failed, an
        /// error is returned.
        ///
        /// If several threads concurrently run `get_or_init`, more than one `f` can
        /// be called. However, all threads will return the same value, produced by
        /// some `f`.
        pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
        where
            F: FnOnce() -> Result<Box<T>, E>,
        {
            match self.get() {
                Some(val) => Ok(val),
                None => self.init(f)
            }
        }

        #[cold]
        #[inline(never)]
        fn init<E>(&self, f: impl FnOnce() -> Result<Box<T>, E>) -> Result<&T, E> {
            let val = f()?;
            let mut ptr = Box::into_raw(val);
            let exchange = self.inner.compare_exchange(
                ptr::null_mut(),
                ptr,
                Ordering::Release,
                Ordering::Acquire,
            );
            if let Err(old) = exchange {
                drop(unsafe { Box::from_raw(ptr) });
                ptr = old;
            }
            Ok(unsafe { &*ptr })
        }
    }

    unsafe impl<T: Sync + Send> Sync for OnceBox<T> {}

    impl<T: Clone> Clone for OnceBox<T> {
        fn clone(&self) -> Self {
            match self.get() {
                Some(value) => OnceBox::with_value(Box::new(value.clone())),
                None => OnceBox::new(),
            }
        }
    }

    /// ```compile_fail
    /// struct S(*mut ());
    /// unsafe impl Sync for S {}
    ///
    /// fn share<T: Sync>(_: &T) {}
    /// share(&once_cell::race::OnceBox::<S>::new());
    /// ```
    fn _dummy() {}
}
