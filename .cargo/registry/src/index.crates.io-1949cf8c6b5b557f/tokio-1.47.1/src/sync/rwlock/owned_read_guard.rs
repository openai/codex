use crate::sync::rwlock::RwLock;
use std::marker::PhantomData;
use std::sync::Arc;
use std::{fmt, mem, ops, ptr};

/// Owned RAII structure used to release the shared read access of a lock when
/// dropped.
///
/// This structure is created by the [`read_owned`] method on
/// [`RwLock`].
///
/// [`read_owned`]: method@crate::sync::RwLock::read_owned
/// [`RwLock`]: struct@crate::sync::RwLock
#[clippy::has_significant_drop]
pub struct OwnedRwLockReadGuard<T: ?Sized, U: ?Sized = T> {
    // When changing the fields in this struct, make sure to update the
    // `skip_drop` method.
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    pub(super) resource_span: tracing::Span,
    pub(super) lock: Arc<RwLock<T>>,
    pub(super) data: *const U,
    pub(super) _p: PhantomData<T>,
}

#[allow(dead_code)] // Unused fields are still used in Drop.
struct Inner<T: ?Sized, U: ?Sized> {
    #[cfg(all(tokio_unstable, feature = "tracing"))]
    resource_span: tracing::Span,
    lock: Arc<RwLock<T>>,
    data: *const U,
}

impl<T: ?Sized, U: ?Sized> OwnedRwLockReadGuard<T, U> {
    fn skip_drop(self) -> Inner<T, U> {
        let me = mem::ManuallyDrop::new(self);
        // SAFETY: This duplicates the values in every field of the guard, then
        // forgets the originals, so in the end no value is duplicated.
        unsafe {
            Inner {
                #[cfg(all(tokio_unstable, feature = "tracing"))]
                resource_span: ptr::read(&me.resource_span),
                lock: ptr::read(&me.lock),
                data: me.data,
            }
        }
    }

    /// Makes a new `OwnedRwLockReadGuard` for a component of the locked data.
    /// This operation cannot fail as the `OwnedRwLockReadGuard` passed in
    /// already locked the data.
    ///
    /// This is an associated function that needs to be
    /// used as `OwnedRwLockReadGuard::map(...)`. A method would interfere with
    /// methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{RwLock, OwnedRwLockReadGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = Arc::new(RwLock::new(Foo(1)));
    ///
    /// let guard = lock.read_owned().await;
    /// let guard = OwnedRwLockReadGuard::map(guard, |f| &f.0);
    ///
    /// assert_eq!(1, *guard);
    /// # }
    /// ```
    #[inline]
    pub fn map<F, V: ?Sized>(this: Self, f: F) -> OwnedRwLockReadGuard<T, V>
    where
        F: FnOnce(&U) -> &V,
    {
        let data = f(&*this) as *const V;
        let this = this.skip_drop();

        OwnedRwLockReadGuard {
            lock: this.lock,
            data,
            _p: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        }
    }

    /// Attempts to make a new [`OwnedRwLockReadGuard`] for a component of the
    /// locked data. The original guard is returned if the closure returns
    /// `None`.
    ///
    /// This operation cannot fail as the `OwnedRwLockReadGuard` passed in
    /// already locked the data.
    ///
    /// This is an associated function that needs to be used as
    /// `OwnedRwLockReadGuard::try_map(..)`. A method would interfere with
    /// methods of the same name on the contents of the locked data.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{RwLock, OwnedRwLockReadGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = Arc::new(RwLock::new(Foo(1)));
    ///
    /// let guard = lock.read_owned().await;
    /// let guard = OwnedRwLockReadGuard::try_map(guard, |f| Some(&f.0)).expect("should not fail");
    ///
    /// assert_eq!(1, *guard);
    /// # }
    /// ```
    #[inline]
    pub fn try_map<F, V: ?Sized>(this: Self, f: F) -> Result<OwnedRwLockReadGuard<T, V>, Self>
    where
        F: FnOnce(&U) -> Option<&V>,
    {
        let data = match f(&*this) {
            Some(data) => data as *const V,
            None => return Err(this),
        };
        let this = this.skip_drop();

        Ok(OwnedRwLockReadGuard {
            lock: this.lock,
            data,
            _p: PhantomData,
            #[cfg(all(tokio_unstable, feature = "tracing"))]
            resource_span: this.resource_span,
        })
    }

    /// Returns a reference to the original `Arc<RwLock>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use tokio::sync::{RwLock, OwnedRwLockReadGuard};
    ///
    /// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// struct Foo(u32);
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let lock = Arc::new(RwLock::new(Foo(1)));
    ///
    /// let guard = lock.clone().read_owned().await;
    /// assert!(Arc::ptr_eq(&lock, OwnedRwLockReadGuard::rwlock(&guard)));
    ///
    /// let guard = OwnedRwLockReadGuard::map(guard, |f| &f.0);
    /// assert!(Arc::ptr_eq(&lock, OwnedRwLockReadGuard::rwlock(&guard)));
    /// # }
    /// ```
    pub fn rwlock(this: &Self) -> &Arc<RwLock<T>> {
        &this.lock
    }
}

impl<T: ?Sized, U: ?Sized> ops::Deref for OwnedRwLockReadGuard<T, U> {
    type Target = U;

    fn deref(&self) -> &U {
        unsafe { &*self.data }
    }
}

impl<T: ?Sized, U: ?Sized> fmt::Debug for OwnedRwLockReadGuard<T, U>
where
    U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized, U: ?Sized> fmt::Display for OwnedRwLockReadGuard<T, U>
where
    U: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized, U: ?Sized> Drop for OwnedRwLockReadGuard<T, U> {
    fn drop(&mut self) {
        self.lock.s.release(1);

        #[cfg(all(tokio_unstable, feature = "tracing"))]
        self.resource_span.in_scope(|| {
            tracing::trace!(
            target: "runtime::resource::state_update",
            current_readers = 1,
            current_readers.op = "sub",
            )
        });
    }
}
