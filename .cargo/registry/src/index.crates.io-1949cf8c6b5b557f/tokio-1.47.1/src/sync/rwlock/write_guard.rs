use crate::sync::batch_semaphore::Semaphore;
use crate::sync::rwlock::read_guard::RwLockReadGuard;
use crate::sync::rwlock::write_guard_mapped::RwLockMappedWriteGuard;
use std::marker::PhantomData;
use std::{fmt, mem, ops};

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by the [`write`] method
/// on [`RwLock`].
///
/// [`write`]: method@crate::sync::RwLock::write
/// [`RwLock`]: struct@crate::sync::RwLock
#[clippy::has_significant_drop]
#[must_use = "if unused the RwLock will immediately unlock"]
pub struct RwLockWriteGuard<'a, T: ?Sized> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    pub(super) resource_span: tracing::Span,
    pub(super) permits_acquired: u32,
    pub(super) s: &'a Semaphore,
    pub(super) data: *mut T,
    pub(super) marker: PhantomData<&'a mut T>,
}

#[allow(dead_code)] // Unused fields are still used in Drop.
struct Inner<'a, T: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    permits_acquired: u32,
    s: &'a Semaphore,
    data: *mut T,
}

impl<'a, T: ?Sized> RwLockWriteGuard<'a, T> {
    fn skip_drop(self) -> Inner<'a, T> {
        let me = mem::ManuallyDrop::new(self);
        // SAFETY: This duplicates the values in every field of the guard, then
        // forgets the originals, so in the end no value is duplicated.
        Inner {
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: unsafe { std::ptr::read(&me.resource_span) },
            permits_acquired: me.permits_acquired,
            s: me.s,
            data: me.data,
        }
    }

    /// Makes a new [`RwLockMappedWriteGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be used as
    /// `RwLockWriteGuard::map(..)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    ///
    /// This is an asynchronous version of [`RwLockWriteGuard::map`] from the
    /// [`parking_lot` crate].
    ///
    /// [`RwLockMappedWriteGuard`]: struct@crate::sync::RwLockMappedWriteGuard
    /// [`RwLockWriteGuard::map`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.map
    /// [`parking_lot` crate]: https://crates.io/crates/parking_lot
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{RwLock, RwLockWriteGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = RwLock::new(Foo(1));
    ///
    /// {
    ///     let mut mapped = RwLockWriteGuard::map(lock.write().await, |f| &mut f.0);
    ///     *mapped = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *lock.read().await);
    /// # }
    /// ```
    #[inline]
    pub fn map<F, U: ?Sized>(mut this: Self, f: F) -> RwLockMappedWriteGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let data = f(&mut *this) as *mut U;
        let this = this.skip_drop();

        RwLockMappedWriteGuard {
            permits_acquired: this.permits_acquired,
            s: this.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        }
    }

    /// Makes a new [`RwLockReadGuard`] for a component of the locked data.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be used as
    /// `RwLockWriteGuard::downgrade_map(..)`. A method would interfere with methods of
    /// the same name on the contents of the locked data.
    ///
    /// This is equivalent to a combination of asynchronous [`RwLockWriteGuard::map`] and [`RwLockWriteGuard::downgrade`]
    /// from the [`parking_lot` crate].
    ///
    /// Inside of `f`, you retain exclusive access to the data, despite only being given a `&T`. Handing out a
    /// `&mut T` would result in unsoundness, as you could use interior mutability.
    ///
    /// [`RwLockMappedWriteGuard`]: struct@crate::sync::RwLockMappedWriteGuard
    /// [`RwLockWriteGuard::map`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.map
    /// [`RwLockWriteGuard::downgrade`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.downgrade
    /// [`parking_lot` crate]: https://crates.io/crates/parking_lot
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{RwLock, RwLockWriteGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = RwLock::new(Foo(1));
    ///
    /// let mapped = RwLockWriteGuard::downgrade_map(lock.write().await, |f| &f.0);
    /// let foo = lock.read().await;
    /// assert_eq!(foo.0, *mapped);
    /// # }
    /// ```
    #[inline]
    pub fn downgrade_map<F, U: ?Sized>(this: Self, f: F) -> RwLockReadGuard<'a, U>
    where
        F: FnOnce(&T) -> &U,
    {
        let data = f(&*this) as *const U;
        let this = this.skip_drop();
        let guard = RwLockReadGuard {
            s: this.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        };

        // Release all but one of the permits held by the write guard
        let to_release = (this.permits_acquired - 1) as usize;
        this.s.release(to_release);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = false,
            write_locked.op = "override",
            )
        });

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        guard
    }

    /// Attempts to make a new [`RwLockMappedWriteGuard`] for a component of
    /// the locked data. The original guard is returned if the closure returns
    /// `None`.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockWriteGuard::try_map(...)`. A method would interfere with
    /// methods of the same name on the contents of the locked data.
    ///
    /// This is an asynchronous version of [`RwLockWriteGuard::try_map`] from
    /// the [`parking_lot` crate].
    ///
    /// [`RwLockMappedWriteGuard`]: struct@crate::sync::RwLockMappedWriteGuard
    /// [`RwLockWriteGuard::try_map`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.try_map
    /// [`parking_lot` crate]: https://crates.io/crates/parking_lot
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{RwLock, RwLockWriteGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = RwLock::new(Foo(1));
    ///
    /// {
    ///     let guard = lock.write().await;
    ///     let mut guard = RwLockWriteGuard::try_map(guard, |f| Some(&mut f.0)).expect("should not fail");
    ///     *guard = 2;
    /// }
    ///
    /// assert_eq!(Foo(2), *lock.read().await);
    /// # }
    /// ```
    #[inline]
    pub fn try_map<F, U: ?Sized>(
        mut this: Self,
        f: F,
    ) -> Result<RwLockMappedWriteGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let data = match f(&mut *this) {
            Some(data) => data as *mut U,
            None => return Err(this),
        };
        let this = this.skip_drop();

        Ok(RwLockMappedWriteGuard {
            permits_acquired: this.permits_acquired,
            s: this.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        })
    }

    /// Attempts to make a new [`RwLockReadGuard`] for a component of
    /// the locked data. The original guard is returned if the closure returns
    /// `None`.
    ///
    /// This operation cannot fail as the `RwLockWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockWriteGuard::try_downgrade_map(...)`. A method would interfere with
    /// methods of the same name on the contents of the locked data.
    ///
    /// This is equivalent to a combination of asynchronous [`RwLockWriteGuard::try_map`] and [`RwLockWriteGuard::downgrade`]
    /// from the [`parking_lot` crate].
    ///
    /// Inside of `f`, you retain exclusive access to the data, despite only being given a `&T`. Handing out a
    /// `&mut T` would result in unsoundness, as you could use interior mutability.
    ///
    /// If this function returns `Err(...)`, the lock is never unlocked nor downgraded.
    ///
    /// [`RwLockMappedWriteGuard`]: struct@crate::sync::RwLockMappedWriteGuard
    /// [`RwLockWriteGuard::map`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.map
    /// [`RwLockWriteGuard::downgrade`]: https://docs.rs/lock_api/latest/lock_api/struct.RwLockWriteGuard.html#method.downgrade
    /// [`parking_lot` crate]: https://crates.io/crates/parking_lot
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::sync::{RwLock, RwLockWriteGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = RwLock::new(Foo(1));
    ///
    /// let guard = RwLockWriteGuard::try_downgrade_map(lock.write().await, |f| Some(&f.0)).expect("should not fail");
    /// let foo = lock.read().await;
    /// assert_eq!(foo.0, *guard);
    /// # }
    /// ```
    #[inline]
    pub fn try_downgrade_map<F, U: ?Sized>(this: Self, f: F) -> Result<RwLockReadGuard<'a, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        let data = match f(&*this) {
            Some(data) => data as *const U,
            None => return Err(this),
        };
        let this = this.skip_drop();
        let guard = RwLockReadGuard {
            s: this.s,
            data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        };

        // Release all but one of the permits held by the write guard
        let to_release = (this.permits_acquired - 1) as usize;
        this.s.release(to_release);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = false,
            write_locked.op = "override",
            )
        });

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        Ok(guard)
    }

    /// Converts this `RwLockWriteGuard` into an `RwLockMappedWriteGuard`. This
    /// method can be used to store a non-mapped guard in a struct field that
    /// expects a mapped guard.
    ///
    /// This is equivalent to calling `RwLockWriteGuard::map(guard, |me| me)`.
    #[inline]
    pub fn into_mapped(this: Self) -> RwLockMappedWriteGuard<'a, T> {
        RwLockWriteGuard::map(this, |me| me)
    }

    /// Atomically downgrades a write lock into a read lock without allowing
    /// any writers to take exclusive access of the lock in the meantime.
    ///
    /// **Note:** This won't *necessarily* allow any additional readers to acquire
    /// locks, since [`RwLock`] is fair and it is possible that a writer is next
    /// in line.
    ///
    /// Returns an RAII guard which will drop this read access of the `RwLock`
    /// when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # use tokio::sync::RwLock;
    /// # use std::sync::Arc;
    /// #
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = Arc::new(RwLock::new(1));
    ///
    /// let n = lock.write().await;
    ///
    /// let cloned_lock = lock.clone();
    /// let handle = tokio::spawn(async move {
    ///     *cloned_lock.write().await = 2;
    /// });
    ///
    /// let n = n.downgrade();
    /// assert_eq!(*n, 1, "downgrade is atomic");
    ///
    /// drop(n);
    /// handle.await.unwrap();
    /// assert_eq!(*lock.read().await, 2, "second writer obtained write lock");
    /// # }
    /// ```
    ///
    /// [`RwLock`]: struct@crate::sync::RwLock
    pub fn downgrade(self) -> RwLockReadGuard<'a, T> {
        let this = self.skip_drop();
        let guard = RwLockReadGuard {
            s: this.s,
            data: this.data,
            marker: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        };

        // Release all but one of the permits held by the write guard
        let to_release = (this.permits_acquired - 1) as usize;
        this.s.release(to_release);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = false,
            write_locked.op = "override",
            )
        });

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        guard.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "add",
            )
        });

        guard
    }
}

impl<T: ?Sized> ops::Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized> ops::DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized> fmt::Debug for RwLockWriteGuard<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> fmt::Display for RwLockWriteGuard<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        self.s.release(self.permits_acquired as usize);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            write_locked = false,
            write_locked.op = "override",
            )
        });
    }
}
