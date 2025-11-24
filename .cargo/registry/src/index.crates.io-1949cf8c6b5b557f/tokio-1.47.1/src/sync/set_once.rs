use super::Notify;

use crate::loom::cell::UnsafeCell;
use crate::loom::sync::{atomic::AtomicBool, Mutex};

use std::error::Error;
use std::fmt;
use std::mem::MaybeUninit;
use std::ops::Drop;
use std::ptr;
use std::sync::atomic::Ordering;

// This file contains an implementation of an SetOnce. The value of SetOnce
// can only be modified once during initialization.
//
//  1. When `value_set` is false, the `value` is not initialized and wait()
//      future will keep on waiting.
//  2. When `value_set` is true, the wait() future completes, get() will return
//      Some(&T)
//
// The value cannot be changed after set() is called. Subsequent calls to set()
// will return a `SetOnceError`.

/// A thread-safe cell that can be written to only once.
///
/// A `SetOnce` is inspired from python's [`asyncio.Event`] type. It can be
/// used to wait until the value of the `SetOnce` is set like a "Event" mechanism.
///
/// # Example
///
/// ```
/// use tokio::sync::{SetOnce, SetOnceError};
///
/// static ONCE: SetOnce<u32> = SetOnce::const_new();
///
/// #[tokio::main]
/// async fn main() -> Result<(), SetOnceError<u32>> {
///
///     // set the value inside a task somewhere...
///     tokio::spawn(async move { ONCE.set(20) });
///
///     // checking with .get doesn't block main thread
///     println!("{:?}", ONCE.get());
///
///     // wait until the value is set, blocks the thread
///     println!("{:?}", ONCE.wait().await);
///
///     Ok(())
/// }
/// ```
///
/// A `SetOnce` is typically used for global variables that need to be
/// initialized once on first use, but need no further changes. The `SetOnce`
/// in Tokio allows the initialization procedure to be asynchronous.
///
/// # Example
///
/// ```
/// use tokio::sync::{SetOnce, SetOnceError};
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), SetOnceError<u32>> {
///     let once = SetOnce::new();
///
///     let arc = Arc::new(once);
///     let first_cl = Arc::clone(&arc);
///     let second_cl = Arc::clone(&arc);
///
///     // set the value inside a task
///     tokio::spawn(async move { first_cl.set(20) }).await.unwrap()?;
///
///     // wait inside task to not block the main thread
///     tokio::spawn(async move {
///         // wait inside async context for the value to be set
///         assert_eq!(*second_cl.wait().await, 20);
///     }).await.unwrap();
///
///     // subsequent set calls will fail
///     assert!(arc.set(30).is_err());
///
///     println!("{:?}", arc.get());
///
///     Ok(())
/// }
/// ```
///
/// [`asyncio.Event`]: https://docs.python.org/3/library/asyncio-sync.html#asyncio.Event
pub struct SetOnce<T> {
    value_set: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
    notify: Notify,
    // we lock the mutex inside set to ensure
    // only one caller of set can run at a time
    lock: Mutex<()>,
}

impl<T> Default for SetOnce<T> {
    fn default() -> SetOnce<T> {
        SetOnce::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for SetOnce<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("SetOnce")
            .field("value", &self.get())
            .finish()
    }
}

impl<T: Clone> Clone for SetOnce<T> {
    fn clone(&self) -> SetOnce<T> {
        SetOnce::new_with(self.get().cloned())
    }
}

impl<T: PartialEq> PartialEq for SetOnce<T> {
    fn eq(&self, other: &SetOnce<T>) -> bool {
        self.get() == other.get()
    }
}

impl<T: Eq> Eq for SetOnce<T> {}

impl<T> Drop for SetOnce<T> {
    fn drop(&mut self) {
        // TODO: Use get_mut()
        if self.value_set.load(Ordering::Relaxed) {
            // SAFETY: If the value_set is true, then the value is initialized
            // then there is a value to be dropped and this is safe
            unsafe { self.value.with_mut(|ptr| ptr::drop_in_place(ptr as *mut T)) }
        }
    }
}

impl<T> From<T> for SetOnce<T> {
    fn from(value: T) -> Self {
        SetOnce {
            value_set: AtomicBool::new(true),
            value: UnsafeCell::new(MaybeUninit::new(value)),
            notify: Notify::new(),
            lock: Mutex::new(()),
        }
    }
}

impl<T> SetOnce<T> {
    /// Creates a new empty `SetOnce` instance.
    pub fn new() -> Self {
        Self {
            value_set: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            notify: Notify::new(),
            lock: Mutex::new(()),
        }
    }

    /// Creates a new empty `SetOnce` instance.
    ///
    /// Equivalent to `SetOnce::new`, except that it can be used in static
    /// variables.
    ///
    /// When using the `tracing` [unstable feature], a `SetOnce` created with
    /// `const_new` will not be instrumented. As such, it will not be visible
    /// in [`tokio-console`]. Instead, [`SetOnce::new`] should be used to
    /// create an instrumented object if that is needed.
    ///
    /// # Example
    ///
    /// ```
    /// use tokio::sync::{SetOnce, SetOnceError};
    ///
    /// static ONCE: SetOnce<u32> = SetOnce::const_new();
    ///
    /// fn get_global_integer() -> Result<Option<&'static u32>, SetOnceError<u32>> {
    ///     ONCE.set(2)?;
    ///     Ok(ONCE.get())
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), SetOnceError<u32>> {
    ///     let result = get_global_integer()?;
    ///
    ///     assert_eq!(result, Some(&2));
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new() -> Self {
        Self {
            value_set: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            notify: Notify::const_new(),
            lock: Mutex::const_new(()),
        }
    }

    /// Creates a new `SetOnce` that contains the provided value, if any.
    ///
    /// If the `Option` is `None`, this is equivalent to `SetOnce::new`.
    ///
    /// [`SetOnce::new`]: crate::sync::SetOnce::new
    pub fn new_with(value: Option<T>) -> Self {
        if let Some(v) = value {
            SetOnce::from(v)
        } else {
            SetOnce::new()
        }
    }

    /// Creates a new `SetOnce` that contains the provided value.
    ///
    /// # Example
    ///
    /// When using the `tracing` [unstable feature], a `SetOnce` created with
    /// `const_new_with` will not be instrumented. As such, it will not be
    /// visible in [`tokio-console`]. Instead, [`SetOnce::new_with`] should be
    /// used to create an instrumented object if that is needed.
    ///
    /// ```
    /// use tokio::sync::SetOnce;
    ///
    /// static ONCE: SetOnce<u32> = SetOnce::const_new_with(1);
    ///
    /// fn get_global_integer() -> Option<&'static u32> {
    ///     ONCE.get()
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let result = get_global_integer();
    ///
    ///     assert_eq!(result, Some(&1));
    /// }
    /// ```
    ///
    /// [`tokio-console`]: https://github.com/tokio-rs/console
    /// [unstable feature]: crate#unstable-features
    #[cfg(not(all(loom, test)))]
    pub const fn const_new_with(value: T) -> Self {
        Self {
            value_set: AtomicBool::new(true),
            value: UnsafeCell::new(MaybeUninit::new(value)),
            notify: Notify::const_new(),
            lock: Mutex::const_new(()),
        }
    }

    /// Returns `true` if the `SetOnce` currently contains a value, and `false`
    /// otherwise.
    pub fn initialized(&self) -> bool {
        // Using acquire ordering so we're able to read/catch any writes that
        // are done with `Ordering::Release`
        self.value_set.load(Ordering::Acquire)
    }

    // SAFETY: The SetOnce must not be empty.
    unsafe fn get_unchecked(&self) -> &T {
        &*self.value.with(|ptr| (*ptr).as_ptr())
    }

    /// Returns a reference to the value currently stored in the `SetOnce`, or
    /// `None` if the `SetOnce` is empty.
    pub fn get(&self) -> Option<&T> {
        if self.initialized() {
            // SAFETY: the SetOnce is initialized, so we can safely
            // call get_unchecked and return the value
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// Sets the value of the `SetOnce` to the given value if the `SetOnce` is
    /// empty.
    ///
    /// If the `SetOnce` already has a value, this call will fail with an
    /// [`SetOnceError`].
    ///
    /// [`SetOnceError`]: crate::sync::SetOnceError
    pub fn set(&self, value: T) -> Result<(), SetOnceError<T>> {
        if self.initialized() {
            return Err(SetOnceError(value));
        }

        // SAFETY: lock the mutex to ensure only one caller of set
        // can run at a time.
        let guard = self.lock.lock();

        if self.initialized() {
            // If the value is already set, we return an error
            drop(guard);

            return Err(SetOnceError(value));
        }

        // SAFETY: We have locked the mutex and checked if the value is
        // initalized or not, so we can safely write to the value
        unsafe {
            self.value.with_mut(|ptr| (*ptr).as_mut_ptr().write(value));
        }

        // Using release ordering so any threads that read a true from this
        // atomic is able to read the value we just stored.
        self.value_set.store(true, Ordering::Release);

        drop(guard);

        // notify the waiting wakers that the value is set
        self.notify.notify_waiters();

        Ok(())
    }

    /// Takes the value from the cell, destroying the cell in the process.
    /// Returns `None` if the cell is empty.
    pub fn into_inner(self) -> Option<T> {
        // TODO: Use get_mut()
        let value_set = self.value_set.load(Ordering::Relaxed);

        if value_set {
            // Since we have taken ownership of self, its drop implementation
            // will be called by the end of this function, to prevent a double
            // free we will set the value_set to false so that the drop
            // implementation does not try to drop the value again.
            self.value_set.store(false, Ordering::Relaxed);

            // SAFETY: The SetOnce is currently initialized, we can assume the
            // value is initialized and return that, when we return the value
            // we give the drop handler to the return scope.
            Some(unsafe { self.value.with_mut(|ptr| ptr::read(ptr).assume_init()) })
        } else {
            None
        }
    }

    /// Waits until set is called. The future returned will keep blocking until
    /// the `SetOnce` is initialized.
    ///
    /// If the `SetOnce` is already initialized, it will return the value
    /// immediately.
    ///
    /// # Note
    ///
    /// This will keep waiting until the `SetOnce` is initialized, so it
    /// should be used with care to avoid blocking the current task
    /// indefinitely.
    pub async fn wait(&self) -> &T {
        loop {
            if let Some(val) = self.get() {
                return val;
            }

            let notify_fut = self.notify.notified();
            {
                // Taking the lock here ensures that a concurrent call to `set`
                // will see the creation of `notify_fut` in case the check
                // fails.
                let _guard = self.lock.lock();

                if self.value_set.load(Ordering::Relaxed) {
                    // SAFETY: the state is initialized
                    return unsafe { self.get_unchecked() };
                }
            }

            // wait until the value is set
            notify_fut.await;
        }
    }
}

// Since `get` gives us access to immutable references of the SetOnce, SetOnce
// can only be Sync if T is Sync, otherwise SetOnce would allow sharing
// references of !Sync values across threads. We need T to be Send in order for
// SetOnce to by Sync because we can use `set` on `&SetOnce<T>` to send values
// (of type T) across threads.
unsafe impl<T: Sync + Send> Sync for SetOnce<T> {}

// Access to SetOnce's value is guarded by the Atomic boolean flag
// and atomic operations on `value_set`, so as long as T itself is Send
// it's safe to send it to another thread
unsafe impl<T: Send> Send for SetOnce<T> {}

/// Error that can be returned from [`SetOnce::set`].
///
/// This error means that the `SetOnce` was already initialized when
/// set was called
///
/// [`SetOnce::set`]: crate::sync::SetOnce::set
#[derive(Debug, PartialEq, Eq)]
pub struct SetOnceError<T>(pub T);

impl<T> fmt::Display for SetOnceError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SetOnceError")
    }
}

impl<T: fmt::Debug> Error for SetOnceError<T> {}
