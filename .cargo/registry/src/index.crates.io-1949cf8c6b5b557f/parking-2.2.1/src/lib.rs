//! Thread parking and unparking.
//!
//! A [`Parker`] is in either the notified or unnotified state. The [`park()`][`Parker::park()`] method blocks
//! the current thread until the [`Parker`] becomes notified and then puts it back into the unnotified
//! state. The [`unpark()`][`Unparker::unpark()`] method puts it into the notified state.
//!
//! This API is similar to [`thread::park()`] and [`Thread::unpark()`] from the standard library.
//! The difference is that the state "token" managed by those functions is shared across an entire
//! thread, and anyone can call [`thread::current()`] to access it. If you use `park` and `unpark`,
//! but you also call a function that uses `park` and `unpark` internally, that function could
//! cause a deadlock by consuming a wakeup that was intended for you. The [`Parker`] object in this
//! crate avoids that problem by managing its own state, which isn't shared with unrelated callers.
//!
//! [`thread::park()`]: https://doc.rust-lang.org/std/thread/fn.park.html
//! [`Thread::unpark()`]: https://doc.rust-lang.org/std/thread/struct.Thread.html#method.unpark
//! [`thread::current()`]: https://doc.rust-lang.org/std/thread/fn.current.html
//!
//! # Examples
//!
//! ```
//! use std::thread;
//! use std::time::Duration;
//! use parking::Parker;
//!
//! let p = Parker::new();
//! let u = p.unparker();
//!
//! // Notify the parker.
//! u.unpark();
//!
//! // Wakes up immediately because the parker is notified.
//! p.park();
//!
//! thread::spawn(move || {
//!     thread::sleep(Duration::from_millis(500));
//!     u.unpark();
//! });
//!
//! // Wakes up when `u.unpark()` notifies and then goes back into unnotified state.
//! p.park();
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]
#![doc(
    html_favicon_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/smol-rs/smol/master/assets/images/logo_fullsize_transparent.png"
)]

#[cfg(not(all(loom, feature = "loom")))]
use std::sync;

#[cfg(all(loom, feature = "loom"))]
use loom::sync;

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::task::{Wake, Waker};
use std::time::Duration;

#[cfg(not(all(loom, feature = "loom")))]
use std::time::Instant;

use sync::atomic::AtomicUsize;
use sync::atomic::Ordering::SeqCst;
use sync::{Condvar, Mutex};

/// Creates a parker and an associated unparker.
///
/// # Examples
///
/// ```
/// let (p, u) = parking::pair();
/// ```
pub fn pair() -> (Parker, Unparker) {
    let p = Parker::new();
    let u = p.unparker();
    (p, u)
}

/// Waits for a notification.
pub struct Parker {
    unparker: Unparker,
    _marker: PhantomData<Cell<()>>,
}

impl Parker {
    /// Creates a new parker.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// ```
    ///
    pub fn new() -> Parker {
        Parker {
            unparker: Unparker {
                inner: Arc::new(Inner {
                    state: AtomicUsize::new(EMPTY),
                    lock: Mutex::new(()),
                    cvar: Condvar::new(),
                }),
            },
            _marker: PhantomData,
        }
    }

    /// Blocks until notified and then goes back into unnotified state.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// // Notify the parker.
    /// u.unpark();
    ///
    /// // Wakes up immediately because the parker is notified.
    /// p.park();
    /// ```
    pub fn park(&self) {
        self.unparker.inner.park(None);
    }

    /// Blocks until notified and then goes back into unnotified state, or times out after
    /// `duration`.
    ///
    /// Returns `true` if notified before the timeout.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// // Wait for a notification, or time out after 500 ms.
    /// p.park_timeout(Duration::from_millis(500));
    /// ```
    #[cfg(not(loom))]
    pub fn park_timeout(&self, duration: Duration) -> bool {
        self.unparker.inner.park(Some(duration))
    }

    /// Blocks until notified and then goes back into unnotified state, or times out at `instant`.
    ///
    /// Returns `true` if notified before the deadline.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, Instant};
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// // Wait for a notification, or time out after 500 ms.
    /// p.park_deadline(Instant::now() + Duration::from_millis(500));
    /// ```
    #[cfg(not(loom))]
    pub fn park_deadline(&self, instant: Instant) -> bool {
        self.unparker
            .inner
            .park(Some(instant.saturating_duration_since(Instant::now())))
    }

    /// Notifies the parker.
    ///
    /// Returns `true` if this call is the first to notify the parker, or `false` if the parker
    /// was already notified.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// assert_eq!(p.unpark(), true);
    /// assert_eq!(p.unpark(), false);
    ///
    /// // Wakes up immediately.
    /// p.park();
    /// ```
    pub fn unpark(&self) -> bool {
        self.unparker.unpark()
    }

    /// Returns a handle for unparking.
    ///
    /// The returned [`Unparker`] can be cloned and shared among threads.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// // Notify the parker.
    /// u.unpark();
    ///
    /// // Wakes up immediately because the parker is notified.
    /// p.park();
    /// ```
    pub fn unparker(&self) -> Unparker {
        self.unparker.clone()
    }
}

impl Default for Parker {
    fn default() -> Parker {
        Parker::new()
    }
}

impl fmt::Debug for Parker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Parker { .. }")
    }
}

/// Notifies a parker.
pub struct Unparker {
    inner: Arc<Inner>,
}

impl Unparker {
    /// Notifies the associated parker.
    ///
    /// Returns `true` if this call is the first to notify the parker, or `false` if the parker
    /// was already notified.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     u.unpark();
    /// });
    ///
    /// // Wakes up when `u.unpark()` notifies and then goes back into unnotified state.
    /// p.park();
    /// ```
    pub fn unpark(&self) -> bool {
        self.inner.unpark()
    }

    /// Indicates whether this unparker will unpark the associated parker.
    ///
    /// This can be used to avoid unnecessary work before calling `unpark()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker();
    ///
    /// assert!(u.will_unpark(&p));
    /// ```
    pub fn will_unpark(&self, parker: &Parker) -> bool {
        Arc::ptr_eq(&self.inner, &parker.unparker.inner)
    }

    /// Indicates whether two unparkers will unpark the same parker.
    ///
    /// # Examples
    ///
    /// ```
    /// use parking::Parker;
    ///
    /// let p = Parker::new();
    /// let u1 = p.unparker();
    /// let u2 = p.unparker();
    ///
    /// assert!(u1.same_parker(&u2));
    /// ```
    pub fn same_parker(&self, other: &Unparker) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl fmt::Debug for Unparker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Unparker { .. }")
    }
}

impl Clone for Unparker {
    fn clone(&self) -> Unparker {
        Unparker {
            inner: self.inner.clone(),
        }
    }
}

impl From<Unparker> for Waker {
    fn from(up: Unparker) -> Self {
        Waker::from(up.inner)
    }
}

const EMPTY: usize = 0;
const PARKED: usize = 1;
const NOTIFIED: usize = 2;

struct Inner {
    state: AtomicUsize,
    lock: Mutex<()>,
    cvar: Condvar,
}

impl Inner {
    fn park(&self, timeout: Option<Duration>) -> bool {
        // If we were previously notified then we consume this notification and return quickly.
        if self
            .state
            .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
            .is_ok()
        {
            return true;
        }

        // If the timeout is zero, then there is no need to actually block.
        if let Some(dur) = timeout {
            if dur == Duration::from_millis(0) {
                return false;
            }
        }

        // Otherwise we need to coordinate going to sleep.
        let mut m = self.lock.lock().unwrap();

        match self.state.compare_exchange(EMPTY, PARKED, SeqCst, SeqCst) {
            Ok(_) => {}
            // Consume this notification to avoid spurious wakeups in the next park.
            Err(NOTIFIED) => {
                // We must read `state` here, even though we know it will be `NOTIFIED`. This is
                // because `unpark` may have been called again since we read `NOTIFIED` in the
                // `compare_exchange` above. We must perform an acquire operation that synchronizes
                // with that `unpark` to observe any writes it made before the call to `unpark`. To
                // do that we must read from the write it made to `state`.
                let old = self.state.swap(EMPTY, SeqCst);
                assert_eq!(old, NOTIFIED, "park state changed unexpectedly");
                return true;
            }
            Err(n) => panic!("inconsistent park_timeout state: {}", n),
        }

        match timeout {
            None => {
                loop {
                    // Block the current thread on the conditional variable.
                    m = self.cvar.wait(m).unwrap();

                    if self
                        .state
                        .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
                        .is_ok()
                    {
                        // got a notification
                        return true;
                    }
                }
            }
            Some(timeout) => {
                #[cfg(not(loom))]
                {
                    // Wait with a timeout, and if we spuriously wake up or otherwise wake up from a
                    // notification we just want to unconditionally set `state` back to `EMPTY`, either
                    // consuming a notification or un-flagging ourselves as parked.
                    let (_m, _result) = self.cvar.wait_timeout(m, timeout).unwrap();

                    match self.state.swap(EMPTY, SeqCst) {
                        NOTIFIED => true, // got a notification
                        PARKED => false,  // no notification
                        n => panic!("inconsistent park_timeout state: {}", n),
                    }
                }

                #[cfg(loom)]
                {
                    let _ = timeout;
                    panic!("park_timeout is not supported under loom");
                }
            }
        }
    }

    pub fn unpark(&self) -> bool {
        // To ensure the unparked thread will observe any writes we made before this call, we must
        // perform a release operation that `park` can synchronize with. To do that we must write
        // `NOTIFIED` even if `state` is already `NOTIFIED`. That is why this must be a swap rather
        // than a compare-and-swap that returns if it reads `NOTIFIED` on failure.
        match self.state.swap(NOTIFIED, SeqCst) {
            EMPTY => return true,     // no one was waiting
            NOTIFIED => return false, // already unparked
            PARKED => {}              // gotta go wake someone up
            _ => panic!("inconsistent state in unpark"),
        }

        // There is a period between when the parked thread sets `state` to `PARKED` (or last
        // checked `state` in the case of a spurious wakeup) and when it actually waits on `cvar`.
        // If we were to notify during this period it would be ignored and then when the parked
        // thread went to sleep it would never wake up. Fortunately, it has `lock` locked at this
        // stage so we can acquire `lock` to wait until it is ready to receive the notification.
        //
        // Releasing `lock` before the call to `notify_one` means that when the parked thread wakes
        // it doesn't get woken only to have to wait for us to release `lock`.
        drop(self.lock.lock().unwrap());
        self.cvar.notify_one();
        true
    }
}

impl Wake for Inner {
    #[inline]
    fn wake(self: Arc<Self>) {
        self.unpark();
    }

    #[inline]
    fn wake_by_ref(self: &Arc<Self>) {
        self.unpark();
    }
}
