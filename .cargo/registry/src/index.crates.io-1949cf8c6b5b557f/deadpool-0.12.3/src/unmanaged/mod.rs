//! Unmanaged version of the pool.
//!
//! "Unmanaged" means that no manager is used to create and recycle objects.
//! Objects either need to be created upfront or by adding them using the
//! [`Pool::add()`] or [`Pool::try_add()`] methods.
//!
//! # Example
//!
//! ```rust
//! use deadpool::unmanaged::Pool;
//!
//! struct Computer {}
//!
//! impl Computer {
//!     async fn get_answer(&self) -> i32 {
//!         42
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let pool = Pool::from(vec![
//!         Computer {},
//!         Computer {},
//!     ]);
//!     let s = pool.get().await.unwrap();
//!     assert_eq!(s.get_answer().await, 42);
//! }
//! ```

mod config;
mod errors;

use std::{
    convert::TryInto,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicIsize, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};

use tokio::sync::{Semaphore, TryAcquireError};

pub use crate::Status;

pub use self::{config::PoolConfig, errors::PoolError};

/// Wrapper around the actual pooled object which implements [`Deref`],
/// [`DerefMut`] and [`Drop`] traits.
///
/// Use this object just as if it was of type `T` and upon leaving a scope the
/// [`Drop::drop()`] will take care of returning it to the pool.
#[derive(Debug)]
#[must_use]
pub struct Object<T> {
    /// Actual pooled object.
    obj: Option<T>,

    /// Pool to return the pooled object to.
    pool: Weak<PoolInner<T>>,
}

impl<T> Object<T> {
    /// Takes this object from the pool permanently. This reduces the size of
    /// the pool. If needed, the object can later be added back to the pool
    /// using the [`Pool::add()`] or [`Pool::try_add()`] methods.
    #[must_use]
    pub fn take(mut this: Self) -> T {
        if let Some(pool) = this.pool.upgrade() {
            let _ = pool.size.fetch_sub(1, Ordering::Relaxed);
            pool.size_semaphore.add_permits(1);
        }
        this.obj.take().unwrap()
    }
}

impl<T> Drop for Object<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.obj.take() {
            if let Some(pool) = self.pool.upgrade() {
                {
                    let mut queue = pool.queue.lock().unwrap();
                    queue.push(obj);
                }
                let _ = pool.available.fetch_add(1, Ordering::Relaxed);
                pool.semaphore.add_permits(1);
                pool.clean_up();
            }
        }
    }
}

impl<T> Deref for Object<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.obj.as_ref().unwrap()
    }
}

impl<T> DerefMut for Object<T> {
    fn deref_mut(&mut self) -> &mut T {
        self.obj.as_mut().unwrap()
    }
}

impl<T> AsRef<T> for Object<T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T> AsMut<T> for Object<T> {
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

/// Generic object and connection pool. This is the static version of the pool
/// which doesn't include.
///
/// This struct can be cloned and transferred across thread boundaries and uses
/// reference counting for its internal state.
///
/// A pool of existing objects can be created from an existing collection of
/// objects if it has a known exact size:
/// ```rust
/// use deadpool::unmanaged::Pool;
/// let pool = Pool::from(vec![1, 2, 3]);
/// ```
#[derive(Debug)]
pub struct Pool<T> {
    inner: Arc<PoolInner<T>>,
}

impl<T> Clone for Pool<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self::from_config(&PoolConfig::default())
    }
}

impl<T> Pool<T> {
    /// Creates a new empty [`Pool`] with the given `max_size`.
    #[must_use]
    pub fn new(max_size: usize) -> Self {
        Self::from_config(&PoolConfig::new(max_size))
    }

    /// Create a new empty [`Pool`] using the given [`PoolConfig`].
    #[must_use]
    pub fn from_config(config: &PoolConfig) -> Self {
        Self {
            inner: Arc::new(PoolInner {
                config: *config,
                queue: Mutex::new(Vec::with_capacity(config.max_size)),
                size: AtomicUsize::new(0),
                size_semaphore: Semaphore::new(config.max_size),
                available: AtomicIsize::new(0),
                semaphore: Semaphore::new(0),
            }),
        }
    }

    /// Retrieves an [`Object`] from this [`Pool`] or waits for the one to
    /// become available.
    ///
    /// # Errors
    ///
    /// See [`PoolError`] for details.
    pub async fn get(&self) -> Result<Object<T>, PoolError> {
        self.timeout_get(self.inner.config.timeout).await
    }

    /// Retrieves an [`Object`] from this [`Pool`] and doesn't wait if there is
    /// currently no [`Object`] is available and the maximum [`Pool`] size has
    /// been reached.
    ///
    /// # Errors
    ///
    /// See [`PoolError`] for details.
    pub fn try_get(&self) -> Result<Object<T>, PoolError> {
        let inner = self.inner.as_ref();
        let permit = inner.semaphore.try_acquire().map_err(|e| match e {
            TryAcquireError::NoPermits => PoolError::Timeout,
            TryAcquireError::Closed => PoolError::Closed,
        })?;
        let obj = {
            let mut queue = inner.queue.lock().unwrap();
            queue.pop().unwrap()
        };
        permit.forget();
        let _ = inner.available.fetch_sub(1, Ordering::Relaxed);
        Ok(Object {
            pool: Arc::downgrade(&self.inner),
            obj: Some(obj),
        })
    }

    /// Retrieves an [`Object`] from this [`Pool`] using a different `timeout`
    /// than the configured one.
    ///
    /// # Errors
    ///
    /// See [`PoolError`] for details.
    pub async fn timeout_get(&self, timeout: Option<Duration>) -> Result<Object<T>, PoolError> {
        let inner = self.inner.as_ref();
        let permit = match (timeout, inner.config.runtime) {
            (None, _) => inner
                .semaphore
                .acquire()
                .await
                .map_err(|_| PoolError::Closed),
            (Some(timeout), _) if timeout.as_nanos() == 0 => {
                inner.semaphore.try_acquire().map_err(|e| match e {
                    TryAcquireError::NoPermits => PoolError::Timeout,
                    TryAcquireError::Closed => PoolError::Closed,
                })
            }
            (Some(timeout), Some(runtime)) => runtime
                .timeout(timeout, inner.semaphore.acquire())
                .await
                .ok_or(PoolError::Timeout)?
                .map_err(|_| PoolError::Closed),
            (Some(_), None) => Err(PoolError::NoRuntimeSpecified),
        }?;
        let obj = {
            let mut queue = inner.queue.lock().unwrap();
            queue.pop().unwrap()
        };
        permit.forget();
        let _ = inner.available.fetch_sub(1, Ordering::Relaxed);
        Ok(Object {
            pool: Arc::downgrade(&self.inner),
            obj: Some(obj),
        })
    }

    /// Adds an `object` to this [`Pool`].
    ///
    /// If the [`Pool`] size has already reached its maximum, then this function
    /// blocks until the `object` can be added to the [`Pool`].
    ///
    /// # Errors
    ///
    /// If the [`Pool`] has been closed a tuple containing the `object` and
    /// the [`PoolError`] is returned instead.
    pub async fn add(&self, object: T) -> Result<(), (T, PoolError)> {
        match self.inner.size_semaphore.acquire().await {
            Ok(permit) => {
                permit.forget();
                self._add(object);
                Ok(())
            }
            Err(_) => Err((object, PoolError::Closed)),
        }
    }

    /// Tries to add an `object` to this [`Pool`].
    ///
    /// # Errors
    ///
    /// If the [`Pool`] size has already reached its maximum, or the [`Pool`]
    /// has been closed, then a tuple containing the `object` and the
    /// [`PoolError`] is returned instead.
    pub fn try_add(&self, object: T) -> Result<(), (T, PoolError)> {
        match self.inner.size_semaphore.try_acquire() {
            Ok(permit) => {
                permit.forget();
                self._add(object);
                Ok(())
            }
            Err(e) => Err(match e {
                TryAcquireError::NoPermits => (object, PoolError::Timeout),
                TryAcquireError::Closed => (object, PoolError::Closed),
            }),
        }
    }

    /// Internal function which adds an `object` to this [`Pool`].
    ///
    /// Prior calling this it must be guaranteed that `size` doesn't exceed
    /// `max_size`. In the methods `add` and `try_add` this is ensured by using
    /// the `size_semaphore`.
    fn _add(&self, object: T) {
        let _ = self.inner.size.fetch_add(1, Ordering::Relaxed);
        {
            let mut queue = self.inner.queue.lock().unwrap();
            queue.push(object);
        }
        let _ = self.inner.available.fetch_add(1, Ordering::Relaxed);
        self.inner.semaphore.add_permits(1);
    }

    /// Removes an [`Object`] from this [`Pool`].
    pub async fn remove(&self) -> Result<T, PoolError> {
        self.get().await.map(Object::take)
    }

    /// Tries to remove an [`Object`] from this [`Pool`].
    pub fn try_remove(&self) -> Result<T, PoolError> {
        self.try_get().map(Object::take)
    }

    /// Removes an [`Object`] from this [`Pool`] using a different `timeout`
    /// than the configured one.
    pub async fn timeout_remove(&self, timeout: Option<Duration>) -> Result<T, PoolError> {
        self.timeout_get(timeout).await.map(Object::take)
    }

    /// Closes this [`Pool`].
    ///
    /// All current and future tasks waiting for [`Object`]s will return
    /// [`PoolError::Closed`] immediately.
    pub fn close(&self) {
        self.inner.semaphore.close();
        self.inner.size_semaphore.close();
        self.inner.clear();
    }

    /// Indicates whether this [`Pool`] has been closed.
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Retrieves [`Status`] of this [`Pool`].
    #[must_use]
    pub fn status(&self) -> Status {
        let max_size = self.inner.config.max_size;
        let size = self.inner.size.load(Ordering::Relaxed);
        let available = self.inner.available.load(Ordering::Relaxed);
        Status {
            max_size,
            size,
            available: if available > 0 { available as usize } else { 0 },
            waiting: if available < 0 {
                (-available) as usize
            } else {
                0
            },
        }
    }
}

#[derive(Debug)]
struct PoolInner<T> {
    config: PoolConfig,
    queue: Mutex<Vec<T>>,
    size: AtomicUsize,
    /// This semaphore has as many permits as `max_size - size`. Every time
    /// an [`Object`] is added to the [`Pool`] a permit is removed from the
    /// semaphore and every time an [`Object`] is removed a permit is returned
    /// back.
    size_semaphore: Semaphore,
    /// Number of available [`Object`]s in the [`Pool`]. If there are no
    /// [`Object`]s in the [`Pool`] this number can become negative and store
    /// the number of [`Future`]s waiting for an [`Object`].
    ///
    /// [`Future`]: std::future::Future
    available: AtomicIsize,
    semaphore: Semaphore,
}

impl<T> PoolInner<T> {
    /// Cleans up internals of this [`Pool`].
    ///
    /// This method is called after closing the [`Pool`] and whenever an
    /// [`Object`] is returned to the [`Pool`] and makes sure closed [`Pool`]s
    /// don't contain any [`Object`]s.
    fn clean_up(&self) {
        if self.is_closed() {
            self.clear();
        }
    }

    /// Removes all the [`Object`]s which are currently part of this [`Pool`].
    fn clear(&self) {
        let mut queue = self.queue.lock().unwrap();
        let _ = self.size.fetch_sub(queue.len(), Ordering::Relaxed);
        let _ = self
            .available
            .fetch_sub(queue.len() as isize, Ordering::Relaxed);
        queue.clear();
    }

    /// Indicates whether this [`Pool`] has been closed.
    fn is_closed(&self) -> bool {
        matches!(
            self.semaphore.try_acquire_many(0),
            Err(TryAcquireError::Closed)
        )
    }
}

impl<T, I> From<I> for Pool<T>
where
    I: IntoIterator<Item = T>,
    <I as IntoIterator>::IntoIter: ExactSizeIterator,
{
    /// Creates a new [`Pool`] from the given [`ExactSizeIterator`] of
    /// [`Object`]s.
    fn from(iter: I) -> Self {
        let queue = iter.into_iter().collect::<Vec<_>>();
        let len = queue.len();
        Self {
            inner: Arc::new(PoolInner {
                queue: Mutex::new(queue),
                config: PoolConfig::new(len),
                size: AtomicUsize::new(len),
                size_semaphore: Semaphore::new(0),
                available: AtomicIsize::new(len.try_into().unwrap()),
                semaphore: Semaphore::new(len),
            }),
        }
    }
}
