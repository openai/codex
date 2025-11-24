use core::fmt;
use core::marker::PhantomPinned;
use core::mem;
use core::pin::Pin;
use core::task::Poll;

use crate::sync::atomic::{AtomicUsize, Ordering};

use alloc::sync::Arc;

use event_listener::{Event, EventListener};
use event_listener_strategy::{easy_wrapper, EventListenerFuture, Strategy};

/// A counter for limiting the number of concurrent operations.
#[derive(Debug)]
pub struct Semaphore {
    count: AtomicUsize,
    event: Event,
}

impl Semaphore {
    const_fn! {
        const_if: #[cfg(not(loom))];
        /// Creates a new semaphore with a limit of `n` concurrent operations.
        ///
        /// # Examples
        ///
        /// ```
        /// use async_lock::Semaphore;
        ///
        /// let s = Semaphore::new(5);
        /// ```
        pub const fn new(n: usize) -> Semaphore {
            Semaphore {
                count: AtomicUsize::new(n),
                event: Event::new(),
            }
        }
    }

    /// Attempts to get a permit for a concurrent operation.
    ///
    /// If the permit could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Semaphore;
    ///
    /// let s = Semaphore::new(2);
    ///
    /// let g1 = s.try_acquire().unwrap();
    /// let g2 = s.try_acquire().unwrap();
    ///
    /// assert!(s.try_acquire().is_none());
    /// drop(g2);
    /// assert!(s.try_acquire().is_some());
    /// ```
    pub fn try_acquire(&self) -> Option<SemaphoreGuard<'_>> {
        let mut count = self.count.load(Ordering::Acquire);
        loop {
            if count == 0 {
                return None;
            }

            match self.count.compare_exchange_weak(
                count,
                count - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(SemaphoreGuard(self)),
                Err(c) => count = c,
            }
        }
    }

    /// Waits for a permit for a concurrent operation.
    ///
    /// Returns a guard that releases the permit when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Semaphore;
    ///
    /// let s = Semaphore::new(2);
    /// let guard = s.acquire().await;
    /// # });
    /// ```
    pub fn acquire(&self) -> Acquire<'_> {
        Acquire::_new(AcquireInner {
            semaphore: self,
            listener: None,
            _pin: PhantomPinned,
        })
    }

    /// Waits for a permit for a concurrent operation.
    ///
    /// Returns a guard that releases the permit when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`acquire`][Semaphore::acquire] method,
    /// this method will block the current thread until the permit is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a semaphore can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Semaphore;
    ///
    /// let s = Semaphore::new(2);
    /// let guard = s.acquire_blocking();
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn acquire_blocking(&self) -> SemaphoreGuard<'_> {
        self.acquire().wait()
    }

    /// Attempts to get an owned permit for a concurrent operation.
    ///
    /// If the permit could not be acquired at this time, then [`None`] is returned. Otherwise, an
    /// owned guard is returned that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Semaphore;
    /// use std::sync::Arc;
    ///
    /// let s = Arc::new(Semaphore::new(2));
    ///
    /// let g1 = s.try_acquire_arc().unwrap();
    /// let g2 = s.try_acquire_arc().unwrap();
    ///
    /// assert!(s.try_acquire_arc().is_none());
    /// drop(g2);
    /// assert!(s.try_acquire_arc().is_some());
    /// ```
    pub fn try_acquire_arc(self: &Arc<Self>) -> Option<SemaphoreGuardArc> {
        let mut count = self.count.load(Ordering::Acquire);
        loop {
            if count == 0 {
                return None;
            }

            match self.count.compare_exchange_weak(
                count,
                count - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(SemaphoreGuardArc(Some(self.clone()))),
                Err(c) => count = c,
            }
        }
    }

    /// Waits for an owned permit for a concurrent operation.
    ///
    /// Returns a guard that releases the permit when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Semaphore;
    /// use std::sync::Arc;
    ///
    /// let s = Arc::new(Semaphore::new(2));
    /// let guard = s.acquire_arc().await;
    /// # });
    /// ```
    pub fn acquire_arc(self: &Arc<Self>) -> AcquireArc {
        AcquireArc::_new(AcquireArcInner {
            semaphore: self.clone(),
            listener: None,
            _pin: PhantomPinned,
        })
    }

    /// Waits for an owned permit for a concurrent operation.
    ///
    /// Returns a guard that releases the permit when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`acquire_arc`][Semaphore::acquire_arc] method,
    /// this method will block the current thread until the permit is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a semaphore can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use async_lock::Semaphore;
    ///
    /// let s = Arc::new(Semaphore::new(2));
    /// let guard = s.acquire_arc_blocking();
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn acquire_arc_blocking(self: &Arc<Self>) -> SemaphoreGuardArc {
        self.acquire_arc().wait()
    }

    /// Adds `n` additional permits to the semaphore.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Semaphore;
    ///
    /// # futures_lite::future::block_on(async {
    /// let s = Semaphore::new(1);
    ///
    /// let _guard = s.acquire().await;
    /// assert!(s.try_acquire().is_none());
    ///
    /// s.add_permits(2);
    ///
    /// let _guard = s.acquire().await;
    /// let _guard = s.acquire().await;
    /// # });
    /// ```
    pub fn add_permits(&self, n: usize) {
        self.count.fetch_add(n, Ordering::AcqRel);
        self.event.notify(n);
    }
}

easy_wrapper! {
    /// The future returned by [`Semaphore::acquire`].
    pub struct Acquire<'a>(AcquireInner<'a> => SemaphoreGuard<'a>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    struct AcquireInner<'a> {
        // The semaphore being acquired.
        semaphore: &'a Semaphore,

        // The listener waiting on the semaphore.
        listener: Option<EventListener>,

        // Keeping this future `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }
}

impl fmt::Debug for Acquire<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Acquire { .. }")
    }
}

impl<'a> EventListenerFuture for AcquireInner<'a> {
    type Output = SemaphoreGuard<'a>;

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.project();

        loop {
            match this.semaphore.try_acquire() {
                Some(guard) => return Poll::Ready(guard),
                None => {
                    // Wait on the listener.
                    if this.listener.is_none() {
                        *this.listener = Some(this.semaphore.event.listen());
                    } else {
                        ready!(strategy.poll(this.listener, cx));
                    }
                }
            }
        }
    }
}

easy_wrapper! {
    /// The future returned by [`Semaphore::acquire_arc`].
    pub struct AcquireArc(AcquireArcInner => SemaphoreGuardArc);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    struct AcquireArcInner {
        // The semaphore being acquired.
        semaphore: Arc<Semaphore>,

        // The listener waiting on the semaphore.
        listener: Option<EventListener>,

        // Keeping this future `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }
}

impl fmt::Debug for AcquireArc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AcquireArc { .. }")
    }
}

impl EventListenerFuture for AcquireArcInner {
    type Output = SemaphoreGuardArc;

    fn poll_with_strategy<'x, S: Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        cx: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.project();

        loop {
            match this.semaphore.try_acquire_arc() {
                Some(guard) => return Poll::Ready(guard),
                None => {
                    // Wait on the listener.
                    if this.listener.is_none() {
                        *this.listener = Some(this.semaphore.event.listen());
                    } else {
                        ready!(strategy.poll(this.listener, cx));
                    }
                }
            }
        }
    }
}

/// A guard that releases the acquired permit.
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct SemaphoreGuard<'a>(&'a Semaphore);

impl SemaphoreGuard<'_> {
    /// Drops the guard _without_ releasing the acquired permit.
    #[inline]
    pub fn forget(self) {
        mem::forget(self);
    }
}

impl Drop for SemaphoreGuard<'_> {
    fn drop(&mut self) {
        self.0.count.fetch_add(1, Ordering::AcqRel);
        self.0.event.notify(1);
    }
}

/// An owned guard that releases the acquired permit.
#[clippy::has_significant_drop]
#[derive(Debug)]
pub struct SemaphoreGuardArc(Option<Arc<Semaphore>>);

impl SemaphoreGuardArc {
    /// Drops the guard _without_ releasing the acquired permit.
    /// (Will still decrement the `Arc` reference count.)
    #[inline]
    pub fn forget(mut self) {
        // Drop the inner `Arc` in order to decrement the reference count.
        // FIXME: get rid of the `Option` once RFC 3466 or equivalent becomes available.
        drop(self.0.take());
        mem::forget(self);
    }
}

impl Drop for SemaphoreGuardArc {
    fn drop(&mut self) {
        let opt = self.0.take().unwrap();
        opt.count.fetch_add(1, Ordering::AcqRel);
        opt.event.notify(1);
    }
}
