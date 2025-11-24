use crate::primitive::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use crate::primitive::sync::{Arc, Condvar, Mutex};
use std::fmt;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// A thread parking primitive.
///
/// Conceptually, each `Parker` has an associated token which is initially not present:
///
/// * The [`park`] method blocks the current thread unless or until the token is available, at
///   which point it automatically consumes the token.
///
/// * The [`park_timeout`] and [`park_deadline`] methods work the same as [`park`], but block for
///   a specified maximum time.
///
/// * The [`unpark`] method atomically makes the token available if it wasn't already. Because the
///   token is initially absent, [`unpark`] followed by [`park`] will result in the second call
///   returning immediately.
///
/// In other words, each `Parker` acts a bit like a spinlock that can be locked and unlocked using
/// [`park`] and [`unpark`].
///
/// # Examples
///
/// ```
/// use std::thread;
/// use std::time::Duration;
/// use crossbeam_utils::sync::Parker;
///
/// let p = Parker::new();
/// let u = p.unparker().clone();
///
/// // Make the token available.
/// u.unpark();
/// // Wakes up immediately and consumes the token.
/// p.park();
///
/// thread::spawn(move || {
///     thread::sleep(Duration::from_millis(500));
///     u.unpark();
/// });
///
/// // Wakes up when `u.unpark()` provides the token.
/// p.park();
/// # std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
/// ```
///
/// [`park`]: Parker::park
/// [`park_timeout`]: Parker::park_timeout
/// [`park_deadline`]: Parker::park_deadline
/// [`unpark`]: Unparker::unpark
pub struct Parker {
    unparker: Unparker,
    _marker: PhantomData<*const ()>,
}

unsafe impl Send for Parker {}

impl Default for Parker {
    fn default() -> Self {
        Self {
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
}

impl Parker {
    /// Creates a new `Parker`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// ```
    ///
    pub fn new() -> Parker {
        Self::default()
    }

    /// Blocks the current thread until the token is made available.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker().clone();
    ///
    /// // Make the token available.
    /// u.unpark();
    ///
    /// // Wakes up immediately and consumes the token.
    /// p.park();
    /// ```
    pub fn park(&self) {
        self.unparker.inner.park(None);
    }

    /// Blocks the current thread until the token is made available, but only for a limited time.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    ///
    /// // Waits for the token to become available, but will not wait longer than 500 ms.
    /// p.park_timeout(Duration::from_millis(500));
    /// ```
    pub fn park_timeout(&self, timeout: Duration) {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.park_deadline(deadline),
            None => self.park(),
        }
    }

    /// Blocks the current thread until the token is made available, or until a certain deadline.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::{Duration, Instant};
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let deadline = Instant::now() + Duration::from_millis(500);
    ///
    /// // Waits for the token to become available, but will not wait longer than 500 ms.
    /// p.park_deadline(deadline);
    /// ```
    pub fn park_deadline(&self, deadline: Instant) {
        self.unparker.inner.park(Some(deadline))
    }

    /// Returns a reference to an associated [`Unparker`].
    ///
    /// The returned [`Unparker`] doesn't have to be used by reference - it can also be cloned.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker().clone();
    ///
    /// // Make the token available.
    /// u.unpark();
    /// // Wakes up immediately and consumes the token.
    /// p.park();
    /// ```
    ///
    /// [`park`]: Parker::park
    /// [`park_timeout`]: Parker::park_timeout
    pub fn unparker(&self) -> &Unparker {
        &self.unparker
    }

    /// Converts a `Parker` into a raw pointer.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let raw = Parker::into_raw(p);
    /// # let _ = unsafe { Parker::from_raw(raw) };
    /// ```
    pub fn into_raw(this: Parker) -> *const () {
        Unparker::into_raw(this.unparker)
    }

    /// Converts a raw pointer into a `Parker`.
    ///
    /// # Safety
    ///
    /// This method is safe to use only with pointers returned by [`Parker::into_raw`].
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let raw = Parker::into_raw(p);
    /// let p = unsafe { Parker::from_raw(raw) };
    /// ```
    pub unsafe fn from_raw(ptr: *const ()) -> Parker {
        Parker {
            unparker: Unparker::from_raw(ptr),
            _marker: PhantomData,
        }
    }
}

impl fmt::Debug for Parker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("Parker { .. }")
    }
}

/// Unparks a thread parked by the associated [`Parker`].
pub struct Unparker {
    inner: Arc<Inner>,
}

unsafe impl Send for Unparker {}
unsafe impl Sync for Unparker {}

impl Unparker {
    /// Atomically makes the token available if it is not already.
    ///
    /// This method will wake up the thread blocked on [`park`] or [`park_timeout`], if there is
    /// any.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    /// use std::time::Duration;
    /// use crossbeam_utils::sync::Parker;
    ///
    /// let p = Parker::new();
    /// let u = p.unparker().clone();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     u.unpark();
    /// });
    ///
    /// // Wakes up when `u.unpark()` provides the token.
    /// p.park();
    /// # std::thread::sleep(std::time::Duration::from_millis(500)); // wait for background threads closed: https://github.com/rust-lang/miri/issues/1371
    /// ```
    ///
    /// [`park`]: Parker::park
    /// [`park_timeout`]: Parker::park_timeout
    pub fn unpark(&self) {
        self.inner.unpark()
    }

    /// Converts an `Unparker` into a raw pointer.
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::{Parker, Unparker};
    ///
    /// let p = Parker::new();
    /// let u = p.unparker().clone();
    /// let raw = Unparker::into_raw(u);
    /// # let _ = unsafe { Unparker::from_raw(raw) };
    /// ```
    pub fn into_raw(this: Unparker) -> *const () {
        Arc::into_raw(this.inner).cast::<()>()
    }

    /// Converts a raw pointer into an `Unparker`.
    ///
    /// # Safety
    ///
    /// This method is safe to use only with pointers returned by [`Unparker::into_raw`].
    ///
    /// # Examples
    ///
    /// ```
    /// use crossbeam_utils::sync::{Parker, Unparker};
    ///
    /// let p = Parker::new();
    /// let u = p.unparker().clone();
    ///
    /// let raw = Unparker::into_raw(u);
    /// let u = unsafe { Unparker::from_raw(raw) };
    /// ```
    pub unsafe fn from_raw(ptr: *const ()) -> Unparker {
        Unparker {
            inner: Arc::from_raw(ptr.cast::<Inner>()),
        }
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

const EMPTY: usize = 0;
const PARKED: usize = 1;
const NOTIFIED: usize = 2;

struct Inner {
    state: AtomicUsize,
    lock: Mutex<()>,
    cvar: Condvar,
}

impl Inner {
    fn park(&self, deadline: Option<Instant>) {
        // If we were previously notified then we consume this notification and return quickly.
        if self
            .state
            .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
            .is_ok()
        {
            return;
        }

        // If the timeout is zero, then there is no need to actually block.
        if let Some(deadline) = deadline {
            if deadline <= Instant::now() {
                return;
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
                return;
            }
            Err(n) => panic!("inconsistent park_timeout state: {}", n),
        }

        loop {
            // Block the current thread on the conditional variable.
            m = match deadline {
                None => self.cvar.wait(m).unwrap(),
                Some(deadline) => {
                    let now = Instant::now();
                    if now < deadline {
                        // We could check for a timeout here, in the return value of wait_timeout,
                        // but in the case that a timeout and an unpark arrive simultaneously, we
                        // prefer to report the former.
                        self.cvar.wait_timeout(m, deadline - now).unwrap().0
                    } else {
                        // We've timed out; swap out the state back to empty on our way out
                        match self.state.swap(EMPTY, SeqCst) {
                            NOTIFIED | PARKED => return,
                            n => panic!("inconsistent park_timeout state: {}", n),
                        };
                    }
                }
            };

            if self
                .state
                .compare_exchange(NOTIFIED, EMPTY, SeqCst, SeqCst)
                .is_ok()
            {
                // got a notification
                return;
            }

            // Spurious wakeup, go back to sleep. Alternatively, if we timed out, it will be caught
            // in the branch above, when we discover the deadline is in the past
        }
    }

    pub(crate) fn unpark(&self) {
        // To ensure the unparked thread will observe any writes we made before this call, we must
        // perform a release operation that `park` can synchronize with. To do that we must write
        // `NOTIFIED` even if `state` is already `NOTIFIED`. That is why this must be a swap rather
        // than a compare-and-swap that returns if it reads `NOTIFIED` on failure.
        match self.state.swap(NOTIFIED, SeqCst) {
            EMPTY => return,    // no one was waiting
            NOTIFIED => return, // already unparked
            PARKED => {}        // gotta go wake someone up
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
    }
}
