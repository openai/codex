use core::borrow::Borrow;
use core::cell::UnsafeCell;
use core::fmt;
use core::marker::{PhantomData, PhantomPinned};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::task::Poll;

use alloc::sync::Arc;

// We don't use loom::UnsafeCell as that doesn't work with the Mutex API.
use crate::sync::atomic::{AtomicUsize, Ordering};

#[cfg(all(feature = "std", not(target_family = "wasm")))]
use std::time::{Duration, Instant};

use event_listener::{Event, EventListener};
use event_listener_strategy::{easy_wrapper, EventListenerFuture};

/// An async mutex.
///
/// The locking mechanism uses eventual fairness to ensure locking will be fair on average without
/// sacrificing performance. This is done by forcing a fair lock whenever a lock operation is
/// starved for longer than 0.5 milliseconds.
///
/// # Examples
///
/// ```
/// # futures_lite::future::block_on(async {
/// use async_lock::Mutex;
///
/// let m = Mutex::new(1);
///
/// let mut guard = m.lock().await;
/// *guard = 2;
///
/// assert!(m.try_lock().is_none());
/// drop(guard);
/// assert_eq!(*m.try_lock().unwrap(), 2);
/// # })
/// ```
pub struct Mutex<T: ?Sized> {
    /// Current state of the mutex.
    ///
    /// The least significant bit is set to 1 if the mutex is locked.
    /// The other bits hold the number of starved lock operations.
    state: AtomicUsize,

    /// Lock operations waiting for the mutex to be released.
    lock_ops: Event,

    /// The value inside the mutex.
    data: UnsafeCell<T>,
}

unsafe impl<T: Send + ?Sized> Send for Mutex<T> {}
unsafe impl<T: Send + ?Sized> Sync for Mutex<T> {}

impl<T> Mutex<T> {
    const_fn! {
        const_if: #[cfg(not(loom))];
        /// Creates a new async mutex.
        ///
        /// # Examples
        ///
        /// ```
        /// use async_lock::Mutex;
        ///
        /// let mutex = Mutex::new(0);
        /// ```
        pub const fn new(data: T) -> Mutex<T> {
            Mutex {
                state: AtomicUsize::new(0),
                lock_ops: Event::new(),
                data: UnsafeCell::new(data),
            }
        }
    }

    /// Consumes the mutex, returning the underlying data.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// assert_eq!(mutex.into_inner(), 10);
    /// ```
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquires the mutex.
    ///
    /// Returns a guard that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// let guard = mutex.lock().await;
    /// assert_eq!(*guard, 10);
    /// # })
    /// ```
    #[inline]
    pub fn lock(&self) -> Lock<'_, T> {
        Lock::_new(LockInner {
            mutex: self,
            acquire_slow: None,
        })
    }

    /// Acquires the mutex using the blocking strategy.
    ///
    /// Returns a guard that releases the mutex when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`lock`][Mutex::lock] method,
    /// this method will block the current thread until the lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a mutex can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// let guard = mutex.lock_blocking();
    /// assert_eq!(*guard, 10);
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn lock_blocking(&self) -> MutexGuard<'_, T> {
        self.lock().wait()
    }

    /// Attempts to acquire the mutex.
    ///
    /// If the mutex could not be acquired at this time, then [`None`] is returned. Otherwise, a
    /// guard is returned that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Mutex;
    ///
    /// let mutex = Mutex::new(10);
    /// if let Some(guard) = mutex.try_lock() {
    ///     assert_eq!(*guard, 10);
    /// }
    /// # ;
    /// ```
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self
            .state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            Some(MutexGuard(self))
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the mutex mutably, no actual locking takes place -- the mutable
    /// borrow statically guarantees the mutex is not already acquired.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut() = 10;
    /// assert_eq!(*mutex.lock().await, 10);
    /// # })
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// Unlocks the mutex directly.
    ///
    /// # Safety
    ///
    /// This function is intended to be used only in the case where the mutex is locked,
    /// and the guard is subsequently forgotten. Calling this while you don't hold a lock
    /// on the mutex will likely lead to UB.
    pub(crate) unsafe fn unlock_unchecked(&self) {
        // Remove the last bit and notify a waiting lock operation.
        self.state.fetch_sub(1, Ordering::Release);
        self.lock_ops.notify(1);
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Acquires the mutex and clones a reference to it.
    ///
    /// Returns an owned guard that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::Mutex;
    /// use std::sync::Arc;
    ///
    /// let mutex = Arc::new(Mutex::new(10));
    /// let guard = mutex.lock_arc().await;
    /// assert_eq!(*guard, 10);
    /// # })
    /// ```
    #[inline]
    pub fn lock_arc(self: &Arc<Self>) -> LockArc<T> {
        LockArc::_new(LockArcInnards::Unpolled {
            mutex: Some(self.clone()),
        })
    }

    /// Acquires the mutex and clones a reference to it using the blocking strategy.
    ///
    /// Returns an owned guard that releases the mutex when dropped.
    ///
    /// # Blocking
    ///
    /// Rather than using asynchronous waiting, like the [`lock_arc`][Mutex::lock_arc] method,
    /// this method will block the current thread until the lock is acquired.
    ///
    /// This method should not be used in an asynchronous context. It is intended to be
    /// used in a way that a mutex can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in a deadlock.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Mutex;
    /// use std::sync::Arc;
    ///
    /// let mutex = Arc::new(Mutex::new(10));
    /// let guard = mutex.lock_arc_blocking();
    /// assert_eq!(*guard, 10);
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    #[inline]
    pub fn lock_arc_blocking(self: &Arc<Self>) -> MutexGuardArc<T> {
        self.lock_arc().wait()
    }

    /// Attempts to acquire the mutex and clone a reference to it.
    ///
    /// If the mutex could not be acquired at this time, then [`None`] is returned. Otherwise, an
    /// owned guard is returned that releases the mutex when dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_lock::Mutex;
    /// use std::sync::Arc;
    ///
    /// let mutex = Arc::new(Mutex::new(10));
    /// if let Some(guard) = mutex.try_lock() {
    ///     assert_eq!(*guard, 10);
    /// }
    /// # ;
    /// ```
    #[inline]
    pub fn try_lock_arc(self: &Arc<Self>) -> Option<MutexGuardArc<T>> {
        if self
            .state
            .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
            .is_ok()
        {
            Some(MutexGuardArc(self.clone()))
        } else {
            None
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Locked;
        impl fmt::Debug for Locked {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("<locked>")
            }
        }

        match self.try_lock() {
            None => f.debug_struct("Mutex").field("data", &Locked).finish(),
            Some(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
        }
    }
}

impl<T> From<T> for Mutex<T> {
    fn from(val: T) -> Mutex<T> {
        Mutex::new(val)
    }
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

easy_wrapper! {
    /// The future returned by [`Mutex::lock`].
    pub struct Lock<'a, T: ?Sized>(LockInner<'a, T> => MutexGuard<'a, T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    /// Inner future for acquiring the mutex.
    struct LockInner<'a, T: ?Sized> {
        // Reference to the mutex.
        mutex: &'a Mutex<T>,

        // The future that waits for the mutex to become available.
        #[pin]
        acquire_slow: Option<AcquireSlow<&'a Mutex<T>, T>>,
    }
}

unsafe impl<T: Send + ?Sized> Send for Lock<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for Lock<'_, T> {}

impl<T: ?Sized> fmt::Debug for Lock<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Lock { .. }")
    }
}

impl<'a, T: ?Sized> EventListenerFuture for LockInner<'a, T> {
    type Output = MutexGuard<'a, T>;

    #[inline]
    fn poll_with_strategy<'x, S: event_listener_strategy::Strategy<'x>>(
        self: Pin<&mut Self>,
        strategy: &mut S,
        context: &mut S::Context,
    ) -> Poll<Self::Output> {
        let mut this = self.project();

        // This may seem weird, but the borrow checker complains otherwise.
        if this.acquire_slow.is_none() {
            match this.mutex.try_lock() {
                Some(guard) => return Poll::Ready(guard),
                None => {
                    this.acquire_slow.set(Some(AcquireSlow::new(this.mutex)));
                }
            }
        }

        ready!(this
            .acquire_slow
            .as_pin_mut()
            .unwrap()
            .poll_with_strategy(strategy, context));
        Poll::Ready(MutexGuard(this.mutex))
    }
}

easy_wrapper! {
    /// The future returned by [`Mutex::lock_arc`].
    pub struct LockArc<T: ?Sized>(LockArcInnards<T> => MutexGuardArc<T>);
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub(crate) wait();
}

pin_project_lite::pin_project! {
    #[project = LockArcInnardsProj]
    enum LockArcInnards<T: ?Sized> {
        /// We have not tried to poll the fast path yet.
        Unpolled { mutex: Option<Arc<Mutex<T>>> },

        /// We are acquiring the mutex through the slow path.
        AcquireSlow {
            #[pin]
            inner: AcquireSlow<Arc<Mutex<T>>, T>
        },
    }
}

unsafe impl<T: Send + ?Sized> Send for LockArc<T> {}
unsafe impl<T: Sync + ?Sized> Sync for LockArc<T> {}

impl<T: ?Sized> fmt::Debug for LockArcInnards<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LockArc { .. }")
    }
}

impl<T: ?Sized> EventListenerFuture for LockArcInnards<T> {
    type Output = MutexGuardArc<T>;

    fn poll_with_strategy<'a, S: event_listener_strategy::Strategy<'a>>(
        mut self: Pin<&mut Self>,
        strategy: &mut S,
        context: &mut S::Context,
    ) -> Poll<Self::Output> {
        // Set the inner future if needed.
        if let LockArcInnardsProj::Unpolled { mutex } = self.as_mut().project() {
            let mutex = mutex.take().expect("mutex taken more than once");

            // Try the fast path before trying to register slowly.
            if let Some(guard) = mutex.try_lock_arc() {
                return Poll::Ready(guard);
            }

            // Set the inner future to the slow acquire path.
            self.as_mut().set(LockArcInnards::AcquireSlow {
                inner: AcquireSlow::new(mutex),
            });
        }

        // Poll the inner future.
        let value = match self.project() {
            LockArcInnardsProj::AcquireSlow { inner } => {
                ready!(inner.poll_with_strategy(strategy, context))
            }
            _ => unreachable!(),
        };

        Poll::Ready(MutexGuardArc(value))
    }
}

pin_project_lite::pin_project! {
    /// Future for acquiring the mutex slowly.
    struct AcquireSlow<B: Borrow<Mutex<T>>, T: ?Sized> {
        // Reference to the mutex.
        mutex: Option<B>,

        // The event listener waiting on the mutex.
        listener: Option<EventListener>,

        // The point at which the mutex lock was started.
        start: Start,

        // This lock operation is starving.
        starved: bool,

        // Capture the `T` lifetime.
        #[pin]
        _marker: PhantomData<T>,

        // Keeping this type `!Unpin` enables future optimizations.
        #[pin]
        _pin: PhantomPinned
    }

    impl<T: ?Sized, B: Borrow<Mutex<T>>> PinnedDrop for AcquireSlow<B, T> {
        fn drop(this: Pin<&mut Self>) {
            // Make sure the starvation counter is decremented.
            this.take_mutex();
        }
    }
}

/// `pin_project_lite` doesn't support `#[cfg]` yet, so we have to do this manually.
struct Start {
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    start: Option<Instant>,
}

impl<T: ?Sized, B: Borrow<Mutex<T>>> AcquireSlow<B, T> {
    /// Create a new `AcquireSlow` future.
    #[cold]
    fn new(mutex: B) -> Self {
        AcquireSlow {
            mutex: Some(mutex),
            listener: None,
            start: Start {
                #[cfg(all(feature = "std", not(target_family = "wasm")))]
                start: None,
            },
            starved: false,
            _marker: PhantomData,
            _pin: PhantomPinned,
        }
    }

    /// Take the mutex reference out, decrementing the counter if necessary.
    fn take_mutex(self: Pin<&mut Self>) -> Option<B> {
        let this = self.project();
        let mutex = this.mutex.take();

        if *this.starved {
            if let Some(mutex) = mutex.as_ref() {
                // Decrement this counter before we exit.
                mutex.borrow().state.fetch_sub(2, Ordering::Release);
            }
        }

        mutex
    }
}

impl<T: ?Sized, B: Unpin + Borrow<Mutex<T>>> EventListenerFuture for AcquireSlow<B, T> {
    type Output = B;

    #[cold]
    fn poll_with_strategy<'a, S: event_listener_strategy::Strategy<'a>>(
        mut self: Pin<&mut Self>,
        strategy: &mut S,
        context: &mut S::Context,
    ) -> Poll<Self::Output> {
        let this = self.as_mut().project();
        #[cfg(all(feature = "std", not(target_family = "wasm")))]
        let start = *this.start.start.get_or_insert_with(Instant::now);
        let mutex = Borrow::<Mutex<T>>::borrow(
            this.mutex.as_ref().expect("future polled after completion"),
        );

        // Only use this hot loop if we aren't currently starved.
        if !*this.starved {
            loop {
                // Start listening for events.
                if this.listener.is_none() {
                    *this.listener = Some(mutex.lock_ops.listen());

                    // Try locking if nobody is being starved.
                    match mutex
                        .state
                        .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
                        .unwrap_or_else(|x| x)
                    {
                        // Lock acquired!
                        0 => return Poll::Ready(self.take_mutex().unwrap()),

                        // Lock is held and nobody is starved.
                        1 => {}

                        // Somebody is starved.
                        _ => break,
                    }
                } else {
                    ready!(strategy.poll(this.listener, context));

                    // Try locking if nobody is being starved.
                    match mutex
                        .state
                        .compare_exchange(0, 1, Ordering::Acquire, Ordering::Acquire)
                        .unwrap_or_else(|x| x)
                    {
                        // Lock acquired!
                        0 => return Poll::Ready(self.take_mutex().unwrap()),

                        // Lock is held and nobody is starved.
                        1 => {}

                        // Somebody is starved.
                        _ => {
                            // Notify the first listener in line because we probably received a
                            // notification that was meant for a starved task.
                            mutex.lock_ops.notify(1);
                            break;
                        }
                    }

                    // If waiting for too long, fall back to a fairer locking strategy that will prevent
                    // newer lock operations from starving us forever.
                    #[cfg(all(feature = "std", not(target_family = "wasm")))]
                    if start.elapsed() > Duration::from_micros(500) {
                        break;
                    }
                }
            }

            // Increment the number of starved lock operations.
            if mutex.state.fetch_add(2, Ordering::Release) > usize::MAX / 2 {
                // In case of potential overflow, abort.
                crate::abort();
            }

            // Indicate that we are now starving and will use a fairer locking strategy.
            *this.starved = true;
        }

        // Fairer locking loop.
        loop {
            if this.listener.is_none() {
                // Start listening for events.
                *this.listener = Some(mutex.lock_ops.listen());

                // Try locking if nobody else is being starved.
                match mutex
                    .state
                    .compare_exchange(2, 2 | 1, Ordering::Acquire, Ordering::Acquire)
                    .unwrap_or_else(|x| x)
                {
                    // Lock acquired!
                    2 => return Poll::Ready(self.take_mutex().unwrap()),

                    // Lock is held by someone.
                    s if s % 2 == 1 => {}

                    // Lock is available.
                    _ => {
                        // Be fair: notify the first listener and then go wait in line.
                        mutex.lock_ops.notify(1);
                    }
                }
            } else {
                // Wait for a notification.
                ready!(strategy.poll(this.listener, context));

                // Try acquiring the lock without waiting for others.
                if mutex.state.fetch_or(1, Ordering::Acquire) % 2 == 0 {
                    return Poll::Ready(self.take_mutex().unwrap());
                }
            }
        }
    }
}

/// A guard that releases the mutex when dropped.
#[clippy::has_significant_drop]
pub struct MutexGuard<'a, T: ?Sized>(&'a Mutex<T>);

unsafe impl<T: Send + ?Sized> Send for MutexGuard<'_, T> {}
unsafe impl<T: Sync + ?Sized> Sync for MutexGuard<'_, T> {}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    /// Returns a reference to the mutex a guard came from.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{Mutex, MutexGuard};
    ///
    /// let mutex = Mutex::new(10i32);
    /// let guard = mutex.lock().await;
    /// dbg!(MutexGuard::source(&guard));
    /// # })
    /// ```
    pub fn source(guard: &MutexGuard<'a, T>) -> &'a Mutex<T> {
        guard.0
    }
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping the mutex guard, therefore unlocking the mutex.
        unsafe {
            self.0.unlock_unchecked();
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.0.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.data.get() }
    }
}

/// An owned guard that releases the mutex when dropped.
#[clippy::has_significant_drop]
pub struct MutexGuardArc<T: ?Sized>(Arc<Mutex<T>>);

unsafe impl<T: Send + ?Sized> Send for MutexGuardArc<T> {}
unsafe impl<T: Sync + ?Sized> Sync for MutexGuardArc<T> {}

impl<T: ?Sized> MutexGuardArc<T> {
    /// Returns a reference to the mutex a guard came from.
    ///
    /// # Examples
    ///
    /// ```
    /// # futures_lite::future::block_on(async {
    /// use async_lock::{Mutex, MutexGuardArc};
    /// use std::sync::Arc;
    ///
    /// let mutex = Arc::new(Mutex::new(10i32));
    /// let guard = mutex.lock_arc().await;
    /// dbg!(MutexGuardArc::source(&guard));
    /// # })
    /// ```
    pub fn source(guard: &Self) -> &Arc<Mutex<T>>
    where
        // Required because `MutexGuardArc` implements `Sync` regardless of whether `T` is `Send`,
        // but this method allows dropping `T` from a different thead than it was created in.
        T: Send,
    {
        &guard.0
    }
}

impl<T: ?Sized> Drop for MutexGuardArc<T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: we are dropping the mutex guard, therefore unlocking the mutex.
        unsafe {
            self.0.unlock_unchecked();
        }
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for MutexGuardArc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for MutexGuardArc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: ?Sized> Deref for MutexGuardArc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.0.data.get() }
    }
}

impl<T: ?Sized> DerefMut for MutexGuardArc<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.data.get() }
    }
}
