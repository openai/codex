use core::cell::UnsafeCell;
use core::convert::Infallible;
use core::fmt;
use core::future::Future;
use core::mem::{forget, MaybeUninit};
use core::ptr;

use crate::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(loom))]
use crate::sync::WithMut;

#[cfg(all(feature = "std", not(target_family = "wasm")))]
use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use event_listener::Event;
use event_listener_strategy::{NonBlocking, Strategy};

#[cfg(all(feature = "std", not(target_family = "wasm")))]
use event_listener::Listener;

/// The current state of the `OnceCell`.
#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(usize)]
enum State {
    /// The `OnceCell` is uninitialized.
    Uninitialized = 0,
    /// The `OnceCell` is being initialized.
    Initializing = 1,
    /// The `OnceCell` is initialized.
    Initialized = 2,
}

impl From<usize> for State {
    fn from(val: usize) -> Self {
        match val {
            0 => State::Uninitialized,
            1 => State::Initializing,
            2 => State::Initialized,
            _ => unreachable!("Invalid state"),
        }
    }
}

impl From<State> for usize {
    fn from(val: State) -> Self {
        val as usize
    }
}

/// A memory location that can be written to at most once.
///
/// A `OnceCell` can be used to store a single value, and only once. However,
/// once the value is stored, it can be accessed directly through a reference
/// instead of needing an RAII guard like `Mutex` or `RwLock`.
///
/// # Examples
///
/// This structure is useful for a variety of patterns, most notably for one-time
/// initialization.
///
/// ```rust
/// use async_lock::OnceCell;
///
/// # struct Foobar;
///
/// async fn very_expensive_initialization() -> Foobar {
///     // Imagine this is very expensive to initialize,
///     // for instance, it requires a network request or
///     // a database call.
///     # Foobar
/// }
///
/// struct LazyFoobar {
///     inner: OnceCell<Foobar>,
/// }
///
/// impl LazyFoobar {
///     fn new() -> Self {
///         Self {
///             inner: OnceCell::new(),
///         }
///     }
///
///     async fn load(&self) -> &Foobar {
///         self.inner.get_or_init(|| async {
///             very_expensive_initialization().await
///         }).await
///     }
/// }
/// ```
pub struct OnceCell<T> {
    /// Listeners waiting for a chance to initialize the cell.
    ///
    /// These are the users of get_or_init() and similar functions.
    active_initializers: Event,

    /// Listeners waiting for the cell to be initialized.
    ///
    /// These are the users of wait().
    passive_waiters: Event,

    /// State associated with the cell.
    state: AtomicUsize,

    /// The value of the cell.
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for OnceCell<T> {}
unsafe impl<T: Send + Sync> Sync for OnceCell<T> {}

impl<T> OnceCell<T> {
    const_fn! {
        const_if: #[cfg(not(loom))];
        /// Create a new, uninitialized `OnceCell`.
        ///
        /// # Example
        ///
        /// ```rust
        /// use async_lock::OnceCell;
        ///
        /// let cell = OnceCell::new();
        /// # cell.set_blocking(1);
        /// ```
        pub const fn new() -> Self {
            Self {
                active_initializers: Event::new(),
                passive_waiters: Event::new(),
                state: AtomicUsize::new(State::Uninitialized as _),
                value: UnsafeCell::new(MaybeUninit::uninit()),
            }
        }
    }

    /// Tell whether or not the cell is initialized.
    ///
    /// This may not always be accurate. For instance, it is possible for
    /// another thread to initialize the cell between the time when this
    /// function is called and the time when the result is actually used.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    /// assert!(!cell.is_initialized());
    /// cell.set(1).await;
    /// assert!(cell.is_initialized());
    /// # });
    /// ```
    pub fn is_initialized(&self) -> bool {
        State::from(self.state.load(Ordering::Acquire)) == State::Initialized
    }

    /// Get a reference to the inner value, or `None` if the value
    /// is not yet initialized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    /// assert!(cell.get().is_none());
    /// cell.set(1).await;
    /// assert_eq!(cell.get(), Some(&1));
    /// # });
    /// ```
    pub fn get(&self) -> Option<&T> {
        if self.is_initialized() {
            // SAFETY: We know that the value is initialized, so it is safe to
            // read it.
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// Get a mutable reference to the inner value, or `None` if the value
    /// is not yet initialized.
    ///
    /// This function is useful for initializing the value inside the cell
    /// when we still have a mutable reference to the cell.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut cell = OnceCell::new();
    /// assert!(cell.get_mut().is_none());
    /// cell.set(1).await;
    /// assert_eq!(cell.get_mut(), Some(&mut 1));
    /// *cell.get_mut().unwrap() = 2;
    /// assert_eq!(cell.get(), Some(&2));
    /// # });
    /// ```
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.state.with_mut(|state| {
            if State::from(*state) == State::Initialized {
                // SAFETY: We know that the value is initialized, so it is safe to
                // read it.
                Some(unsafe { &mut *self.value.get().cast() })
            } else {
                None
            }
        })
    }

    /// Take the value out of this `OnceCell`, moving it back to the uninitialized
    /// state.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let mut cell = OnceCell::new();
    /// cell.set(1).await;
    /// assert_eq!(cell.take(), Some(1));
    /// assert!(!cell.is_initialized());
    /// # });
    /// ```
    pub fn take(&mut self) -> Option<T> {
        self.state.with_mut(|state| {
            if State::from(*state) == State::Initialized {
                // SAFETY: We know that the value is initialized, so it is safe to
                // read it.
                let value = unsafe { ptr::read(self.value.get().cast()) };
                *state = State::Uninitialized.into();
                Some(value)
            } else {
                None
            }
        })
    }

    /// Convert this `OnceCell` into the inner value, if it is initialized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    /// cell.set(1).await;
    /// assert_eq!(cell.into_inner(), Some(1));
    /// # });
    /// ```
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }

    /// Wait for the cell to be initialized, and then return a reference to the
    /// inner value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use std::thread::{sleep, spawn};
    ///
    /// let cell = Arc::new(OnceCell::new());
    /// let cell2 = cell.clone();
    ///
    /// spawn(move || {
    ///    sleep(Duration::from_millis(5));
    ///    cell2.set_blocking(1);
    /// });
    ///
    /// # futures_lite::future::block_on(async {
    /// assert_eq!(cell.wait().await, &1);
    /// # });
    /// ```
    pub async fn wait(&self) -> &T {
        // Fast path: see if the value is already initialized.
        if let Some(value) = self.get() {
            return value;
        }

        // Slow path: wait for the value to be initialized.
        event_listener::listener!(self.passive_waiters => listener);

        // Try again.
        if let Some(value) = self.get() {
            return value;
        }

        listener.await;
        debug_assert!(self.is_initialized());

        // SAFETY: We know that the value is initialized, so it is safe to
        // read it.
        unsafe { self.get_unchecked() }
    }

    /// Wait for the cell to be initialized, and then return a reference to the
    /// inner value.
    ///
    /// # Blocking
    ///
    /// In contrast to the `wait` method, this method blocks the current thread of
    /// execution instead of awaiting.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a `OnceCell` can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    /// use std::sync::Arc;
    /// use std::time::Duration;
    /// use std::thread::{sleep, spawn};
    ///
    /// let cell = Arc::new(OnceCell::new());
    /// let cell2 = cell.clone();
    ///
    /// spawn(move || {
    ///    sleep(Duration::from_millis(5));
    ///    cell2.set_blocking(1);
    /// });
    ///
    /// assert_eq!(cell.wait_blocking(), &1);
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub fn wait_blocking(&self) -> &T {
        // Fast path: see if the value is already initialized.
        if let Some(value) = self.get() {
            return value;
        }

        // Slow path: wait for the value to be initialized.
        event_listener::listener!(self.passive_waiters => listener);

        // Try again.
        if let Some(value) = self.get() {
            return value;
        }

        listener.wait();
        debug_assert!(self.is_initialized());

        // SAFETY: We know that the value is initialized, so it is safe to
        // read it.
        unsafe { self.get_unchecked() }
    }

    /// Either get the value or initialize it with the given closure.
    ///
    /// The cell will not be initialized if the closure returns an error.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    /// #
    /// # // Prevent explicit value errors.
    /// # fn _explicit(_: &Result<&i32, ()>) {}
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    ///
    /// let result = cell.get_or_try_init(|| async { Err(()) }).await;
    /// assert!(result.is_err());
    ///
    /// let result = cell.get_or_try_init(|| async { Ok(1) }).await;
    /// # _explicit(&result);
    /// assert_eq!(result.unwrap(), &1);
    ///
    /// let result = cell.get_or_try_init(|| async { Err(()) }).await;
    ///
    /// assert_eq!(result.unwrap(), &1);
    /// # });
    /// ```
    pub async fn get_or_try_init<E, Fut: Future<Output = Result<T, E>>>(
        &self,
        closure: impl FnOnce() -> Fut,
    ) -> Result<&T, E> {
        // Fast path: see if the value is already initialized.
        if let Some(value) = self.get() {
            return Ok(value);
        }

        // Slow path: initialize the value.
        self.initialize_or_wait(closure, &mut NonBlocking::default())
            .await?;
        debug_assert!(self.is_initialized());

        // SAFETY: We know that the value is initialized, so it is safe to
        // read it.
        Ok(unsafe { self.get_unchecked() })
    }

    /// Either get the value or initialize it with the given closure.
    ///
    /// The cell will not be initialized if the closure returns an error.
    ///
    /// # Blocking
    ///
    /// In contrast to the `get_or_try_init` method, this method blocks the current thread of
    /// execution instead of awaiting.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a `OnceCell` can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    /// #
    /// # // Prevent explicit type errors.
    /// # fn _explicit(_: &Result<&i32, ()>) {}
    ///
    /// let cell = OnceCell::new();
    ///
    /// let result = cell.get_or_try_init_blocking(|| Err(()));
    /// assert!(result.is_err());
    ///
    /// let result = cell.get_or_try_init_blocking(|| Ok(1));
    /// # _explicit(&result);
    /// assert_eq!(result.unwrap(), &1);
    ///
    /// let result = cell.get_or_try_init_blocking(|| Err(()));
    ///
    /// assert_eq!(result.unwrap(), &1);
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub fn get_or_try_init_blocking<E>(
        &self,
        closure: impl FnOnce() -> Result<T, E>,
    ) -> Result<&T, E> {
        // Fast path: see if the value is already initialized.
        if let Some(value) = self.get() {
            return Ok(value);
        }

        // Slow path: initialize the value.
        // The futures provided should never block, so we can use `now_or_never`.
        now_or_never(self.initialize_or_wait(
            move || core::future::ready(closure()),
            &mut event_listener_strategy::Blocking::default(),
        ))?;
        debug_assert!(self.is_initialized());

        // SAFETY: We know that the value is initialized, so it is safe to
        // read it.
        Ok(unsafe { self.get_unchecked() })
    }

    /// Either get the value or initialize it with the given closure.
    ///
    /// Many tasks may call this function, but the value will only be set once
    /// and only one closure will be invoked.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.get_or_init(|| async { 1 }).await, &1);
    /// assert_eq!(cell.get_or_init(|| async { 2 }).await, &1);
    /// # });
    /// ```
    pub async fn get_or_init<Fut: Future<Output = T>>(&self, closure: impl FnOnce() -> Fut) -> &T {
        // false positive: https://github.com/rust-lang/rust/issues/129352
        #[allow(unreachable_patterns)]
        match self
            .get_or_try_init(move || async move {
                let result: Result<T, Infallible> = Ok(closure().await);
                result
            })
            .await
        {
            Ok(value) => value,
            Err(infallible) => match infallible {},
        }
    }

    /// Either get the value or initialize it with the given closure.
    ///
    /// Many tasks may call this function, but the value will only be set once
    /// and only one closure will be invoked.
    ///
    /// # Blocking
    ///
    /// In contrast to the `get_or_init` method, this method blocks the current thread of
    /// execution instead of awaiting.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a `OnceCell` can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// let cell = OnceCell::new();
    /// assert_eq!(cell.get_or_init_blocking(|| 1), &1);
    /// assert_eq!(cell.get_or_init_blocking(|| 2), &1);
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub fn get_or_init_blocking(&self, closure: impl FnOnce() -> T + Unpin) -> &T {
        let result = self.get_or_try_init_blocking(move || {
            let result: Result<T, Infallible> = Ok(closure());
            result
        });

        // false positive: https://github.com/rust-lang/rust/issues/129352
        #[allow(unreachable_patterns)]
        match result {
            Ok(value) => value,
            Err(infallible) => match infallible {},
        }
    }

    /// Try to set the value of the cell.
    ///
    /// If the cell is already initialized, this method returns the original
    /// value back.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    ///
    /// assert_eq!(cell.set(1).await, Ok(&1));
    /// assert_eq!(cell.get(), Some(&1));
    /// assert_eq!(cell.set(2).await, Err(2));
    /// # });
    /// ```
    pub async fn set(&self, value: T) -> Result<&T, T> {
        let mut value = Some(value);
        self.get_or_init(|| async { value.take().unwrap() }).await;

        match value {
            Some(value) => Err(value),
            None => {
                // SAFETY: value was taken, so we are initialized
                Ok(unsafe { self.get_unchecked() })
            }
        }
    }

    /// Try to set the value of the cell.
    ///
    /// If the cell is already initialized, this method returns the original
    /// value back.
    ///
    /// # Blocking
    ///
    /// In contrast to the `set` method, this method blocks the current thread of
    /// execution instead of awaiting.
    ///
    /// This method should not be used in an asynchronous context. It is intended
    /// to be used such that a `OnceCell` can be used in both asynchronous and synchronous contexts.
    /// Calling this method in an asynchronous context may result in deadlocks.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// let cell = OnceCell::new();
    ///
    /// assert_eq!(cell.set_blocking(1), Ok(&1));
    /// assert_eq!(cell.get(), Some(&1));
    /// assert_eq!(cell.set_blocking(2), Err(2));
    /// ```
    #[cfg(all(feature = "std", not(target_family = "wasm")))]
    pub fn set_blocking(&self, value: T) -> Result<&T, T> {
        let mut value = Some(value);
        self.get_or_init_blocking(|| value.take().unwrap());

        match value {
            Some(value) => Err(value),
            None => {
                // SAFETY: value was taken, so we are initialized
                Ok(unsafe { self.get_unchecked() })
            }
        }
    }

    /// Wait for the cell to be initialized, optionally using a closure
    /// to initialize the cell if it is not initialized yet.
    #[cold]
    async fn initialize_or_wait<E, Fut: Future<Output = Result<T, E>>, F: FnOnce() -> Fut>(
        &self,
        closure: F,
        strategy: &mut impl for<'a> Strategy<'a>,
    ) -> Result<(), E> {
        // The event listener we're currently waiting on.
        let mut event_listener = None;

        let mut closure = Some(closure);

        loop {
            // Check the current state of the cell.
            let state = self.state.load(Ordering::Acquire);

            // Determine what we should do based on our state.
            match state.into() {
                State::Initialized => {
                    // The cell is initialized now, so we can return.
                    return Ok(());
                }
                State::Initializing => {
                    // The cell is currently initializing, or the cell is uninitialized
                    // but we do not have the ability to initialize it.
                    //
                    // We need to wait the initialization to complete.
                    if let Some(listener) = event_listener.take() {
                        strategy.wait(listener).await;
                    } else {
                        event_listener = Some(self.active_initializers.listen());
                    }
                }
                State::Uninitialized => {
                    // Try to move the cell into the initializing state.
                    if self
                        .state
                        .compare_exchange(
                            State::Uninitialized.into(),
                            State::Initializing.into(),
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_err()
                    {
                        // The cell was initialized while we were trying to
                        // initialize it.
                        continue;
                    }

                    // Now that we have an exclusive lock on the cell's value,
                    // we can try to initialize it.
                    let _guard = Guard(self);
                    let initializer = closure.take().unwrap();
                    match (initializer)().await {
                        Ok(value) => {
                            // Write the value into the cell and update the state.
                            unsafe {
                                ptr::write(self.value.get().cast(), value);
                            }
                            forget(_guard);
                            self.state
                                .store(State::Initialized.into(), Ordering::Release);

                            // Notify the listeners that the value is initialized.
                            self.active_initializers.notify_additional(usize::MAX);
                            self.passive_waiters.notify_additional(usize::MAX);

                            return Ok(());
                        }
                        Err(err) => {
                            // Update the state to indicate that the value is
                            // uninitialized.
                            drop(_guard);

                            return Err(err);
                        }
                    }
                }
            }
        }

        /// Set the cell's state back to `UNINITIALIZED on drop.
        ///
        /// If the closure panics, this ensures that the cell's state is set back to
        /// `UNINITIALIZED` and that the next listener is notified.
        struct Guard<'a, T>(&'a OnceCell<T>);

        impl<T> Drop for Guard<'_, T> {
            fn drop(&mut self) {
                self.0
                    .state
                    .store(State::Uninitialized.into(), Ordering::Release);

                // Notify the next initializer that it's their turn.
                self.0.active_initializers.notify(1);
            }
        }
    }

    /// Get a reference to the inner value.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the cell is initialized.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// # futures_lite::future::block_on(async {
    /// let cell = OnceCell::new();
    /// cell.set(1).await;
    ///
    /// // SAFETY: We know that the value is initialized, so it is safe to
    /// // read it.
    /// assert_eq!(unsafe { cell.get_unchecked() }, &1);
    /// # });
    /// ```
    pub unsafe fn get_unchecked(&self) -> &T {
        // SAFETY: The caller asserts that the value is initialized
        &*self.value.get().cast()
    }
}

impl<T> From<T> for OnceCell<T> {
    /// Create a new, initialized `OnceCell` from an existing value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use async_lock::OnceCell;
    ///
    /// let cell = OnceCell::from(42);
    /// assert_eq!(cell.get(), Some(&42));
    /// ```
    fn from(value: T) -> Self {
        Self {
            active_initializers: Event::new(),
            passive_waiters: Event::new(),
            state: AtomicUsize::new(State::Initialized.into()),
            value: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for OnceCell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Inner<'a, T>(&'a OnceCell<T>);

        impl<T: fmt::Debug> fmt::Debug for Inner<'_, T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0.state.load(Ordering::Acquire).into() {
                    State::Uninitialized => f.write_str("<uninitialized>"),
                    State::Initializing => f.write_str("<initializing>"),
                    State::Initialized => {
                        // SAFETY: "value" is initialized.
                        let value = unsafe { self.0.get_unchecked() };
                        fmt::Debug::fmt(value, f)
                    }
                }
            }
        }

        f.debug_tuple("OnceCell").field(&Inner(self)).finish()
    }
}

impl<T> Drop for OnceCell<T> {
    fn drop(&mut self) {
        self.state.with_mut(|state| {
            if State::from(*state) == State::Initialized {
                // SAFETY: We know that the value is initialized, so it is safe to
                // drop it.
                unsafe { self.value.get().cast::<T>().drop_in_place() }
            }
        });
    }
}

impl<T> Default for OnceCell<T> {
    // Calls `OnceCell::new`.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

/// Either return the result of a future now, or panic.
#[cfg(all(feature = "std", not(target_family = "wasm")))]
fn now_or_never<T>(f: impl Future<Output = T>) -> T {
    const NOOP_WAKER: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(ptr::null(), &NOOP_WAKER)
    }
    unsafe fn drop(_: *const ()) {}

    pin!(f);

    let waker = unsafe { Waker::from_raw(RawWaker::new(ptr::null(), &NOOP_WAKER)) };

    // Poll the future exactly once.
    let mut cx = Context::from_waker(&waker);

    match f.poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("future not ready"),
    }
}
