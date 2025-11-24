use crate::sync::batch_semaphore::Semaphore;
use std::marker::PhantomData;
use std::{fmt, mem, ops};

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by [mapping] an [`RwLockWriteGuard`]. It is a
/// separate type from `RwLockWriteGuard` to disallow downgrading a mapped
/// guard, since doing so can cause undefined behavior.
///
/// [mapping]: method@crate::sync::RwLockWriteGuard::map
/// [`RwLockWriteGuard`]: struct@crate::sync::RwLockWriteGuard
#[clippy::has_significant_drop]
pub struct RwLockMappedWriteGuard<'a, T: ?Sized> {
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

impl<'a, T: ?Sized> RwLockMappedWriteGuard<'a, T> {
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

    /// Makes a new `RwLockMappedWriteGuard` for a component of the locked data.
    ///
    /// This operation cannot fail as the `RwLockMappedWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be used as
    /// `RwLockMappedWriteGuard::map(..)`. A method would interfere with methods
    /// of the same name on the contents of the locked data.
    ///
    /// This is an asynchronous version of [`RwLockWriteGuard::map`] from the
    /// [`parking_lot` crate].
    ///
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

    /// Attempts to make a new [`RwLockMappedWriteGuard`] for a component of
    /// the locked data. The original guard is returned if the closure returns
    /// `None`.
    ///
    /// This operation cannot fail as the `RwLockMappedWriteGuard` passed in already
    /// locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `RwLockMappedWriteGuard::try_map(...)`. A method would interfere
    /// with methods of the same name on the contents of the locked data.
    ///
    /// This is an asynchronous version of [`RwLockWriteGuard::try_map`] from
    /// the [`parking_lot` crate].
    ///
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

    // Note: No `downgrade`, `downgrade_map` nor `try_downgrade_map` because they would be unsound, as we're already
    //       potentially been mapped with internal mutability.
}

impl<T: ?Sized> ops::Deref for RwLockMappedWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized> ops::DerefMut for RwLockMappedWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data }
    }
}

impl<'a, T: ?Sized> fmt::Debug for RwLockMappedWriteGuard<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> fmt::Display for RwLockMappedWriteGuard<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Drop for RwLockMappedWriteGuard<'a, T> {
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
